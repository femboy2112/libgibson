//! A2 (Renaissance VI): real-terminal PTY walkthrough of `wire_instrument`.
//!
//! Milestone A proved the loop headlessly (`wire_instrument_loop.rs`) and the `--selfcheck`
//! binary end to end (`wire_instrument_selfcheck.rs`). This drives the *interactive* binary as a
//! real user would, on an actual pseudo-terminal: a Python-published field reaches the screen
//! with its source identity; the probe readout is present; zoom responds; a valid edit crosses
//! the process boundary and the producer's **Acknowledged** verdict returns and is shown; an
//! out-of-range edit returns **Refused** with the edit-receipt counts proving the lifecycle.
//!
//! The oracle is the HUD *text* and the producer's real ack verdict — never a color.
//! `RangePolicy::Auto` renormalizes the palette per frame, so two distinct amplitudes can look
//! identical; the readout text (`probe (x,y) = v`, `ack N refused N`, and the `ack: edit resolved
//! → …` status) is what we assert. Synchronization is deadline-polled on observable screen
//! content (`wait_text`), never a fixed sleep.
//!
//! Scope boundary (honest, L10): the *authoritative-field-doubles* causal proof lives in
//! `wire_instrument_loop.rs` (lockstep, equal-sequence, 2x factor, with the `--freeze-amplitude`
//! suppression control). Here the serve-mode Gaussian center moves each tick, so this asserts the
//! edit *lifecycle* through the real binary, not per-cell doubling. The full compact-terminal
//! (40x15) workflow — every readout visible + the edit lifecycle at that size — is Phase C3;
//! `build_root` has no compact layout yet, so this file certifies only that 40x15 monochrome
//! boots, streams a source-identified field, and restores cleanly.

#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};
use vt100::Parser;

fn python() -> String {
    std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

struct Walk {
    capture: pty_capture::Capture,
    parser: Parser,
    processed: usize,
}

impl Walk {
    fn boot(term_cols: u16, term_rows: u16, field_cols: u32, field_rows: u32, mono: bool) -> Self {
        let mut cmd = CommandBuilder::new(pty_capture::example_path("wire_instrument"));
        cmd.env("TERM", "xterm-256color");
        if mono {
            // A non-empty NO_COLOR selects ColorDepth::Mono (src/capability.rs): no color SGR.
            cmd.env("NO_COLOR", "1");
        }
        cmd.env("GIBSON_PYTHON", python());
        // Size the producer's field small enough that the field + HUD fit the terminal, and keep
        // it streaming well past the end of the walkthrough.
        cmd.arg("--cols");
        cmd.arg(field_cols.to_string());
        cmd.arg("--rows");
        cmd.arg(field_rows.to_string());
        cmd.arg("--frames");
        cmd.arg("2000");
        let capture =
            pty_capture::Capture::spawn(cmd, term_cols, term_rows, Duration::from_secs(60));
        Walk {
            capture,
            parser: Parser::new(term_rows, term_cols, 0),
            processed: 0,
        }
    }

    fn pump(&mut self, d: Duration) {
        self.capture.collect_for(d).expect("collect PTY output");
        let raw = self.capture.raw();
        self.parser.process(&raw[self.processed..]);
        self.processed = raw.len();
    }

    fn screen(&self) -> String {
        self.parser.screen().contents()
    }

    fn wait_text(&mut self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            self.pump(Duration::from_millis(50));
            if self.screen().contains(needle) {
                return;
            }
        }
        panic!(
            "timed out waiting for {needle:?}\n--- screen ---\n{}",
            self.screen()
        );
    }

    fn send(&mut self, bytes: &[u8]) {
        self.capture.write(bytes).expect("write to PTY");
        self.pump(Duration::from_millis(90));
    }

    /// Quit with `q` and assert the terminal session is restored cleanly: alt screen left, cursor
    /// shown, SGR mouse and focus reporting disabled. Checked on the raw byte stream (the restore
    /// sequences the app emits on the way out), not the vt100 screen. `Capture`'s Drop force-reaps
    /// the binary if it lingers; the binary reaps its Python producer on the graceful `q` exit.
    fn assert_quit_restored(&mut self) {
        let _ = self.capture.write(b"q");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            self.pump(Duration::from_millis(50));
            if matches!(self.capture.try_wait(), Ok(Some(_))) {
                break;
            }
            if Instant::now() >= deadline {
                panic!("wire_instrument did not exit after 'q'\n{}", self.screen());
            }
        }
        for _ in 0..4 {
            self.pump(Duration::from_millis(30));
        }
        let raw = self.capture.raw().to_vec();
        let has = |n: &[u8]| raw.windows(n.len()).any(|w| w == n);
        assert!(has(b"\x1b[?1049l"), "left the alternate screen on quit");
        assert!(has(b"\x1b[?25h"), "cursor shown on restore");
        assert!(
            has(b"\x1b[?1006h") && has(b"\x1b[?1006l"),
            "SGR mouse reporting enabled then disabled"
        );
        assert!(
            has(b"\x1b[?1004h") && has(b"\x1b[?1004l"),
            "focus reporting enabled then disabled"
        );
    }
}

#[test]
fn pty_walkthrough_truecolor_80x30() {
    let mut h = Walk::boot(80, 30, 16, 8, false);

    // A Python-published field reaches the screen, with its source identity (steps 2-3).
    h.wait_text("source 1");
    let screen = h.screen();
    eprintln!("=== wire_instrument 80x30 truecolor boot ===\n{screen}\n=== end ===");
    assert!(
        screen.contains("model_tick"),
        "the model tick is shown:\n{screen}"
    );
    assert!(
        screen.contains("probe ("),
        "the probe readout is shown:\n{screen}"
    );

    // Move the probe (step 4): Right then Down. The readout stays present.
    h.send(b"\x1b[C");
    h.send(b"\x1b[B");
    assert!(
        h.screen().contains("probe ("),
        "probe readout survives movement:\n{}",
        h.screen()
    );

    // Zoom (step 12).
    h.send(b"+");
    h.wait_text("zoom 2x");

    // Submit a valid edit (pending starts at 2.0, in range): the producer Acknowledges, and the
    // verdict returns across the process boundary (steps 6-9).
    h.send(b"\r");
    h.wait_text("Acknowledged");
    assert!(
        h.screen().contains("ack 1"),
        "the accepted edit is counted in the HUD receipt:\n{}",
        h.screen()
    );

    // Submit an out-of-range edit (steps 10-11): the `]` key caps pending at 12.0, past the
    // producer's [0,10] contract; submit and read the producer's Refused verdict. The receipt
    // proves a model-authoritative refusal, not a mere color change.
    for _ in 0..20 {
        h.send(b"]");
    }
    h.send(b"\r");
    h.wait_text("Refused");
    assert!(
        h.screen().contains("refused 1"),
        "the refused edit is counted in the HUD receipt:\n{}",
        h.screen()
    );

    // Quit and confirm the terminal is restored cleanly (steps 13-14).
    h.assert_quit_restored();
}

#[test]
fn pty_boot_at_40x15_mono_streams_and_restores() {
    // Phase A2 certifies only that the compact monochrome terminal boots, streams a
    // source-identified field, and restores. The full 40x15 workflow (all readouts visible + the
    // edit lifecycle at that size) is Phase C3 — build_root has no compact layout yet, so the
    // lower HUD lines may wrap off a 15-row screen; this test does not assert them.
    let mut h = Walk::boot(40, 15, 8, 4, true);
    h.wait_text("source 1");
    eprintln!(
        "=== wire_instrument 40x15 mono boot ===\n{}\n=== end ===",
        h.screen()
    );
    h.assert_quit_restored();
}
