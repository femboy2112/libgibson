//! Wave 6 — the instrument workbench through a REAL terminal (#144 acceptance #3).
//!
//! Spawns the built `instrument_workbench` example on a PTY and drives it as a human
//! would — boot, draw, undo/redo, step the field, change the source, click to probe —
//! reading the screen back through a vt100 parser, then asserts the terminal session is
//! restored cleanly on quit (alt screen left, cursor shown, bracketed paste / SGR mouse
//! / focus reporting all disabled, raw mode reverted). No screenshot-only knowledge.

#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};
use vt100::Parser;

const COLS: u16 = 100;
const ROWS: u16 = 36;

struct Harness {
    capture: pty_capture::Capture,
    parser: Parser,
    processed: usize,
    initial_termios: Option<String>,
}

impl Harness {
    fn boot() -> Self {
        let mut cmd = CommandBuilder::new(pty_capture::example_path("instrument_workbench"));
        cmd.env("TERM", "xterm-256color");
        let capture = pty_capture::Capture::spawn(cmd, COLS, ROWS, Duration::from_secs(30));
        let initial_termios = capture.initial_termios.clone();
        Harness {
            capture,
            parser: Parser::new(ROWS, COLS, 0),
            processed: 0,
            initial_termios,
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
            "timed out waiting for {needle:?}\n--- screen ---\n{}",
            self.screen()
        );
    }

    fn send(&mut self, bytes: &[u8]) {
        self.capture.write(bytes).expect("write to PTY");
        self.pump(Duration::from_millis(80));
    }

    fn assert_restored(&mut self) {
        // Wait for the child to exit after `q`.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.pump(Duration::from_millis(50));
            if matches!(self.capture.try_wait(), Ok(Some(_))) {
                break;
            }
            if Instant::now() >= deadline {
                panic!("workbench did not exit after 'q'\n{}", self.screen());
            }
        }
        // Drain any trailing restore bytes.
        for _ in 0..4 {
            self.pump(Duration::from_millis(30));
        }

        let raw = self.capture.raw().to_vec();
        let has = |needle: &[u8]| raw.windows(needle.len()).any(|w| w == needle);
        assert!(has(b"\x1b[?1049h"), "entered the alternate screen");
        assert!(has(b"\x1b[?1049l"), "left the alternate screen");
        assert!(has(b"\x1b[?2004l"), "bracketed paste disabled on restore");
        assert!(has(b"\x1b[?25h"), "cursor shown on restore");
        assert!(
            has(b"\x1b[?1006h") && has(b"\x1b[?1006l"),
            "SGR mouse reporting enabled then disabled"
        );
        assert!(
            has(b"\x1b[?1004h") && has(b"\x1b[?1004l"),
            "focus reporting enabled then disabled"
        );
        assert!(
            !self.parser.screen().alternate_screen(),
            "vt100 sees the primary screen again"
        );
        assert_eq!(
            self.capture.termios(),
            self.initial_termios,
            "raw mode restored to the entry termios"
        );
    }
}

#[test]
fn workbench_pty_walkthrough() {
    let mut h = Harness::boot();

    // Boot: the HUD renders and the scene starts empty.
    h.wait_text("WORKBENCH");
    assert!(h.screen().contains("shapes=0"), "starts with no shapes");

    // Draw a rectangle wall with 'n'.
    h.send(b"n");
    h.wait_text("shapes=1");

    // Undo removes it (Ctrl-Z); redo restores it (Ctrl-Y).
    h.send(b"\x1a");
    h.wait_text("shapes=0");
    h.send(b"\x19");
    h.wait_text("shapes=1");

    // Step the deterministic field forward with '.' — the model tick advances.
    h.send(b".");
    h.wait_text("tick=1");

    // Raise the source parameter with ']' (5.0 -> 6.0), refused-not-clamped at bounds.
    h.send(b"]");
    h.wait_text("source=6.0");

    // Click the canvas (SGR 1-based) to select + probe a cell; the probe line updates.
    h.send(b"\x1b[<0;6;4M"); // press at terminal (5,3)
    h.send(b"\x1b[<0;6;4m"); // release
    h.wait_text("world=");

    // Quit and verify the terminal session is fully restored.
    h.send(b"q");
    h.assert_restored();
}
