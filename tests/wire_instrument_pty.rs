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
//! (40×15) workflow — every readout visible + the edit lifecycle + the inspector pages at that size
//! — is now certified by `pty_full_workflow_at_40x15_mono` (RXII Checkpoint 2 / Gate B): `build_root`
//! has a compact layout that pins the essentials and pages the detail. `pty_boot_at_40x15_mono…`
//! remains the minimal boot/restore anchor.

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
        // portable-pty's CommandBuilder defaults a child's cwd to $HOME when none is set
        // (cmdbuilder.rs: `self.cwd … .unwrap_or(home)`), NOT the parent's cwd. The example
        // resolves its producer as the repo-relative `bindings/python/producer.py`, so without a
        // pinned cwd it looks under $HOME, fails to launch Python, streams no field, and this
        // walkthrough times out on the first frame. Pin cwd to the crate root where the producer
        // actually lives.
        cmd.cwd(env!("CARGO_MANIFEST_DIR"));
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

    /// Quit with `q` and assert the terminal session is restored cleanly, checked on the raw byte
    /// stream (the restore sequences the app emits on the way out), not the vt100 screen.
    ///
    /// `wire_instrument` is **keyboard-only**: `Context::run_once → poll_event → enter_interactive`
    /// enables raw mode and bracketed paste, and `Context::fullscreen` enters the alternate screen.
    /// It never calls `set_mouse_capture`/`set_focus_reporting`, so SGR-mouse (`?1006`) and focus
    /// (`?1004`) reporting are never turned on and nothing restores them — asserting their *disable*
    /// would demand sequences this binary has no reason to emit (that is `instrument_workbench`'s
    /// contract, from which this idiom was first copied). We assert exactly the modes this binary
    /// uses: alternate screen left, cursor shown, and bracketed paste enabled then disabled. Raw
    /// mode is a termios ioctl, not an output byte sequence, so it is covered by `TerminalSession`'s
    /// Drop (`disable_raw_mode`) rather than asserted from the captured stream.
    ///
    /// `Capture`'s Drop force-reaps the binary if it lingers; the binary reaps its Python producer
    /// on the graceful `q` exit.
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
            has(b"\x1b[?2004h") && has(b"\x1b[?2004l"),
            "bracketed paste enabled then disabled"
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

/// H1 (#143, Gate H) instrumented the `--selfcheck` delivery leg; I2 wires the SAME
/// issue->confirmed-write+flush leg into the INTERACTIVE path. The HUD shows
/// `command-dispatch leg: count N`: zero before any send, N after N confirmed Enter deliveries.
/// Only the COUNT is asserted — p50/max are wall-clock and host-dependent (L10). The oracle is the
/// HUD text through the vt100 screen, never a color.
#[test]
fn pty_interactive_records_the_command_dispatch_leg() {
    let mut h = Walk::boot(80, 30, 16, 8, false);

    // The field arrives and the latency funnel is live but empty — nothing dispatched yet.
    h.wait_text("source 1");
    h.wait_text("command-dispatch leg: count 0");

    // A valid edit (pending 2.0, in range) crosses the pipe; the interactive path records the leg.
    h.send(b"\r");
    h.wait_text("Acknowledged");
    h.wait_text("command-dispatch leg: count 1");
    // One delivery so far — the recording is per-delivery, not double-counted.
    assert!(
        !h.screen().contains("command-dispatch leg: count 2"),
        "exactly one delivery has been recorded:\n{}",
        h.screen()
    );

    // A second confirmed send advances the counter — proving each Enter records once.
    h.send(b"\r");
    h.wait_text("command-dispatch leg: count 2");

    h.assert_quit_restored();
}

#[test]
fn pty_boot_at_40x15_mono_streams_and_restores() {
    // The compact-terminal boot/restore anchor: at 40x15 monochrome the instrument boots, streams
    // a source-identified field, and restores cleanly. The FULL compact workflow (every readout
    // visible + the edit lifecycle + the inspector pages at that size) is proven by
    // `pty_full_workflow_at_40x15_mono` below (RXII Checkpoint 2 / Gate B C1–C3).
    let mut h = Walk::boot(40, 15, 8, 4, true);
    h.wait_text("source 1");
    eprintln!(
        "=== wire_instrument 40x15 mono boot ===\n{}\n=== end ===",
        h.screen()
    );
    h.assert_quit_restored();
}

/// RXII Checkpoint 2 / Gate B (C1–C3): the COMPLETE 40×15 scientific-instrument workflow on a live
/// PTY — not just boot/restore. The compact layout must keep every essential readout (source,
/// probe, edit verdict, transport status) on a 15-row screen while the edit lifecycle runs end to
/// end, and an inspector page must surface the full detail on demand.
///
/// Against the pre-C1 layout this is RED: `build_root` has no compact mode, so at 40×15 the
/// raster plus five full-width HUD lines wrap past 15 rows and the edit-verdict line ("Acknowledged"
/// / the `ack N` receipt) is pushed off the bottom — `wait_text("Acknowledged")` times out. The
/// compact layout (essentials pinned above the capped raster, detail paged under `Tab`) turns it
/// green.
///
/// Monochrome, so the oracle is strictly the HUD *text* through the vt100 screen — never a color.
#[test]
fn pty_full_workflow_at_40x15_mono() {
    let mut h = Walk::boot(40, 15, 8, 4, true);

    // Boot: the source-identified field reaches the compact screen.
    h.wait_text("source 1");

    // Submit a valid edit (pending starts at 2.0, in range). THE 40×15 TEST: the producer's
    // Acknowledged verdict and the `ack 1` receipt must be VISIBLE on the 15-row screen — the exact
    // readout the pre-compact layout pushed off the bottom.
    h.send(b"\r");
    h.wait_text("Acknowledged");
    assert!(
        h.screen().contains("ack 1"),
        "the accepted edit's receipt is visible at 40×15:\n{}",
        h.screen()
    );

    // Zoom responds and the compact source line still shows it (`z2x`), probe readout survives.
    h.send(b"+");
    h.wait_text("z2x");
    assert!(
        h.screen().contains("probe ("),
        "the probe readout stays visible at 40×15:\n{}",
        h.screen()
    );

    // Out-of-range edit: `]` caps pending at 12.0 (past the producer's [0,10] contract); submit and
    // read the producer's Refused verdict and its `refused 1` receipt — both on-screen at 40×15.
    for _ in 0..20 {
        h.send(b"]");
    }
    h.send(b"\r");
    h.wait_text("Refused");
    assert!(
        h.screen().contains("refused 1"),
        "the refused edit's receipt is visible at 40×15:\n{}",
        h.screen()
    );

    // Inspect: Tab pages to the full edit ledger (the "inspector" step of the C2 workflow) while
    // the pinned essentials — the source identity — remain visible. Every readout is accessible.
    h.send(b"\t");
    h.wait_text("inspect 1/3");
    let screen = h.screen();
    assert!(
        screen.contains("issued"),
        "the inspector surfaces the full edit ledger:\n{screen}"
    );
    assert!(
        screen.contains("source 1"),
        "the pinned essentials stay visible while inspecting:\n{screen}"
    );

    // Quit and confirm the terminal is restored cleanly.
    h.assert_quit_restored();
}
