//! Wave 3 PTY walkthrough: the `scene_editor` example driven through a real Unix
//! pseudo-terminal. The headless `tests/scene_editor_slice.rs` proves the editor
//! *composition*; this proves the SAME editor actually runs in a live terminal —
//! real key/mouse decoding, a real painted frame, and session-owned restoration
//! of raw mode, the alternate screen, bracketed paste, and opt-in mouse/focus
//! reporting (Finding F). These assert semantic screen state, not image goldens.
#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

const COLS: u16 = 100;
const ROWS: u16 = 36;

struct Edit {
    capture: pty_capture::Capture,
    parser: vt100::Parser,
    processed: usize,
    initial_termios: Option<String>,
}

impl Edit {
    fn spawn() -> Self {
        let mut cmd = CommandBuilder::new(pty_capture::example_path("scene_editor"));
        cmd.env("TERM", "xterm-256color");
        let capture = pty_capture::Capture::spawn(cmd, COLS, ROWS, Duration::from_secs(30));
        let initial_termios = capture.initial_termios.clone();
        Self {
            capture,
            parser: vt100::Parser::new(ROWS, COLS, 0),
            processed: 0,
            initial_termios,
        }
    }

    fn pump(&mut self, duration: Duration) {
        self.capture
            .collect_for(duration)
            .expect("drain editor PTY");
        let raw = self.capture.raw();
        self.parser.process(&raw[self.processed..]);
        self.processed = raw.len();
    }

    fn screen(&self) -> String {
        self.parser.screen().contents()
    }

    /// Pumps until `needle` appears on screen, or fails with the last screen.
    fn wait_text(&mut self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            self.pump(Duration::from_millis(10));
            if self.screen().contains(needle) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "missing {needle:?} on editor screen:\n{}",
                self.screen()
            );
        }
    }

    fn key(&mut self, bytes: &[u8]) {
        self.capture.write(bytes).expect("send editor input");
        // Let the child decode the event and repaint before the next assertion.
        self.pump(Duration::from_millis(30));
    }

    /// Parses the `canvas=x,y,w,h` rect the editor discloses in its HUD.
    fn canvas_rect(&mut self) -> (u16, u16, u16, u16) {
        self.wait_text("canvas=");
        let screen = self.screen();
        let tail = screen.split("canvas=").nth(1).expect("canvas= present");
        let field: String = tail
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == ',')
            .collect();
        let nums: Vec<u16> = field.split(',').filter_map(|s| s.parse().ok()).collect();
        assert_eq!(nums.len(), 4, "canvas rect should be x,y,w,h: {field:?}");
        (nums[0], nums[1], nums[2], nums[3])
    }

    /// Sends an SGR left-button click at gibson cell `(gx, gy)` (0-based). SGR
    /// mouse coordinates are 1-based, so we offset by one.
    fn click(&mut self, gx: u16, gy: u16) {
        let (cx, cy) = (gx + 1, gy + 1);
        self.key(format!("\x1b[<0;{cx};{cy}M").as_bytes()); // press
        self.key(format!("\x1b[<0;{cx};{cy}m").as_bytes()); // release
    }

    fn assert_restored(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            self.pump(Duration::from_millis(10));
            if let Some(status) = self.capture.try_wait().expect("poll editor") {
                assert!(status.success(), "editor exited unsuccessfully: {status:?}");
                break;
            }
            assert!(Instant::now() < deadline, "editor did not exit on quit");
        }
        self.pump(Duration::from_millis(30));
        let raw = String::from_utf8_lossy(self.capture.raw());
        // Session lifecycle: entered and left the alternate screen, and restored
        // bracketed paste and the cursor.
        assert!(raw.contains("\x1b[?1049h"), "alternate screen not entered");
        assert!(raw.contains("\x1b[?1049l"), "alternate screen not restored");
        assert!(raw.contains("\x1b[?2004l"), "bracketed paste not restored");
        assert!(raw.contains("\x1b[?25h"), "cursor not restored");
        // Finding F: opt-in mouse and focus reporting were enabled for the session
        // and then disabled on teardown.
        assert!(
            raw.contains("\x1b[?1006h"),
            "SGR mouse reporting not enabled"
        );
        assert!(
            raw.contains("\x1b[?1006l"),
            "SGR mouse reporting not restored"
        );
        assert!(raw.contains("\x1b[?1004h"), "focus reporting not enabled");
        assert!(raw.contains("\x1b[?1004l"), "focus reporting not restored");
        assert!(!self.parser.screen().alternate_screen());
        assert_eq!(
            self.capture.termios(),
            self.initial_termios,
            "termios (raw mode) not restored"
        );
    }
}

/// Boots the seeded scene, then a keyboard edit moves the selection and undo/redo
/// round-trips it — the canonical model state, read off the live screen.
#[test]
fn boots_and_keyboard_edits_round_trip_through_a_real_terminal() {
    let mut ed = Edit::spawn();
    ed.wait_text("SCENE EDITOR");
    ed.wait_text("shapes=4"); // the four seeded shapes painted
    ed.wait_text("undo=4"); // one Add transaction each
    ed.wait_text("(9,9)-(11,11)"); // the source at its seeded bounds

    // Tab cycles the selection in draw order: FIELD, CHAMBER_A, CHAMBER_B, SOURCE.
    for _ in 0..4 {
        ed.key(b"\t");
    }
    ed.wait_text("sel=SOURCE");

    // Three Right nudges move the source +3 in x (three undoable transactions).
    for _ in 0..3 {
        ed.key(b"\x1b[C");
    }
    ed.wait_text("(12,9)-(14,11)");
    ed.wait_text("undo=7");

    // Undo all three nudges -> back to the seeded source bounds.
    for _ in 0..3 {
        ed.key(b"\x1a"); // Ctrl-Z
    }
    ed.wait_text("(9,9)-(11,11)");

    // Redo all three -> the moved bounds return.
    for _ in 0..3 {
        ed.key(b"\x19"); // Ctrl-Y
    }
    ed.wait_text("(12,9)-(14,11)");

    ed.key(b"q");
    ed.assert_restored();
}

/// The bounded source commits in range and *refuses* out of range (no clamp).
#[test]
fn numeric_source_commits_in_range_and_refuses_over_range() {
    let mut ed = Edit::spawn();
    ed.wait_text("source=1");

    ed.key(b"]"); // 1 -> 2
    ed.wait_text("source=2");
    ed.key(b"["); // 2 -> 1
    ed.wait_text("source=1");

    // Drive well past the ceiling: the value saturates at the validated max by
    // refusal, never clamping past it or wrapping.
    for _ in 0..15 {
        ed.key(b"]");
    }
    ed.wait_text("source=10");
    ed.wait_text("source refused"); // the last over-range step was reported, not applied

    ed.key(b"q");
    ed.assert_restored();
}

/// A real SGR mouse click on the painted canvas selects the shape under the
/// pointer — pointer input decoded and resolved through the live terminal.
#[test]
fn pointer_click_selects_the_painted_shape() {
    let mut ed = Edit::spawn();
    ed.wait_text("shapes=4");
    let (x, y, _w, _h) = ed.canvas_rect();

    // World (10,10) is inside the source (9,9)-(11,11), which is topmost there.
    // With 1 cell == 1 world unit and no pan, that is canvas cell (x+10, y+10).
    ed.click(x + 10, y + 10);
    ed.wait_text("sel=SOURCE");

    ed.key(b"q");
    ed.assert_restored();
}
