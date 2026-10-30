//! Phase 6 (#140) — keyboard-only workflow certification at 40×15 monochrome.
//!
//! The issue names **40×15 monochrome, no mouse** as the target, and
//! `docs/WORKBENCH_RECIPES.md` claims the workbench "keeps every core workflow"
//! there. One core step is "read the selected value, including missing/invalid
//! state" — which lives in the `probe` / `inspector` / `metrics` readout lines
//! below the canvas.
//!
//! STEP 1 (measurement / red-first): at 40×15 the ~83-char HUD word-wraps to
//! several rows; with 12 heatmap rows beneath it, the readouts may be pushed off
//! the bottom of a 15-row screen. This test boots the real example on a 40×15
//! mono PTY and asserts the readouts are actually on-screen. If they are clipped,
//! the panic carries the exact vt100 dump, and that clip is a real small-screen
//! layout defect to fix (L8 — usability must not depend on terminal luxury), not
//! a feature gap.

#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};
use vt100::Parser;

const COLS: u16 = 40;
const ROWS: u16 = 15;

/// A 40×15 monochrome PTY harness. Mono is forced via `NO_COLOR` (see
/// `src/capability.rs`: a non-empty `NO_COLOR` selects `ColorDepth::Mono`), so no
/// color SGR is emitted — the issue's exact target environment.
struct Mono {
    capture: pty_capture::Capture,
    parser: Parser,
    processed: usize,
}

impl Mono {
    fn boot() -> Self {
        let mut cmd = CommandBuilder::new(pty_capture::example_path("instrument_workbench"));
        cmd.env("TERM", "xterm-256color");
        cmd.env("NO_COLOR", "1");
        let capture = pty_capture::Capture::spawn(cmd, COLS, ROWS, Duration::from_secs(30));
        Mono {
            capture,
            parser: Parser::new(ROWS, COLS, 0),
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
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            self.pump(Duration::from_millis(50));
            if self.screen().contains(needle) {
                return;
            }
        }
        panic!(
            "timed out waiting for {needle:?}\n--- 40x15 mono screen ---\n{}",
            self.screen()
        );
    }

    /// Best-effort quit; `Capture`'s Drop force-reaps if the child lingers, so a
    /// slow exit never masks the measurement assertion that ran before it.
    fn quit(&mut self) {
        let _ = self.capture.write(b"q");
        self.pump(Duration::from_millis(120));
    }
}

#[test]
fn boot_at_40x15_mono_keeps_the_core_readouts_visible() {
    let mut h = Mono::boot();
    h.wait_text("WORKBENCH"); // the HUD has rendered
    h.pump(Duration::from_millis(250)); // let layout settle
    let screen = h.screen();
    eprintln!(
        "=== 40x15 mono boot — vt100 screen ({} visible rows) ===\n{screen}\n=== end 40x15 dump ===",
        screen.lines().count()
    );

    // #140 core step "read the selected value, incl. missing/invalid" lives in these
    // readout lines (workbench.rs: probe_line / inspector_line / metrics_line).
    let missing: Vec<&str> = ["probe", "inspector", "metrics"]
        .into_iter()
        .filter(|w| !screen.contains(w))
        .collect();
    assert!(
        missing.is_empty(),
        "readouts clipped at 40x15 (missing: {missing:?}); the HUD wrap pushes them past \
         row {ROWS}. A 40x15 mono terminal must still show the probe/inspector readouts \
         (#140, L8). Screen:\n{screen}"
    );

    h.quit();
}
