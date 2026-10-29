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

/// Wider boot for the canvas-pick test, so the `canvas=` HUD suffix (and the full HUD)
/// is never truncated at the right edge and `parse_canvas` reads complete digits.
const PICK_COLS: u16 = 120;
const PICK_ROWS: u16 = 36;

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

    /// Boot with `WORKBENCH_EMIT_CANVAS` set and a wide terminal, so the HUD exposes the
    /// painted canvas rect and a pick test can derive a real on-canvas click cell.
    fn boot_canvas() -> Self {
        let mut cmd = CommandBuilder::new(pty_capture::example_path("instrument_workbench"));
        cmd.env("TERM", "xterm-256color");
        cmd.env("WORKBENCH_EMIT_CANVAS", "1");
        let capture =
            pty_capture::Capture::spawn(cmd, PICK_COLS, PICK_ROWS, Duration::from_secs(30));
        let initial_termios = capture.initial_termios.clone();
        Harness {
            capture,
            parser: Parser::new(PICK_ROWS, PICK_COLS, 0),
            processed: 0,
            initial_termios,
        }
    }

    /// Send a REAL SGR left press+release at the canvas-local cell `(ix, iy)`: the
    /// renderer-confirmed canvas origin `(rect.x, rect.y)` plus `(ix, iy)`, converted to
    /// 1-based SGR mouse coordinates. The click cell is DERIVED from the painted rect,
    /// never a hard-coded screen coordinate (#144 / L2).
    fn click_local(&mut self, rect: (u16, u16, u16, u16), ix: u16, iy: u16) {
        let (cx, cy, _, _) = rect;
        let col = cx + ix + 1; // SGR mouse coordinates are 1-based
        let row = cy + iy + 1;
        self.send(format!("\x1b[<0;{col};{row}M").as_bytes());
        self.send(format!("\x1b[<0;{col};{row}m").as_bytes());
    }

    /// Poll until the HUD `sel=` token equals `want` (parsed as an integer, so `sel=1`
    /// never spuriously matches `sel=10`), else panic with the screen.
    fn wait_sel(&mut self, want: i64) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            self.pump(Duration::from_millis(50));
            if parse_sel(&self.screen()) == Some(want) {
                return;
            }
        }
        panic!(
            "timed out waiting for sel={want}; got sel={:?}\n--- screen ---\n{}",
            parse_sel(&self.screen()),
            self.screen()
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

    // Probe the field centre via the keyboard — coordinate-independent — and confirm the
    // readout updates. (The full mouse click -> specific-shape pick pipeline, with the
    // click cell DERIVED from the painted canvas rect, is proven end-to-end in
    // `pty_click_selects_the_specific_shape_under_the_pointer` below.)
    h.send(b"p"); // keyboard probe of the field centre
    h.wait_text("world=");

    // Quit and verify the terminal session is fully restored.
    h.send(b"q");
    h.assert_restored();
}

/// Finding F: the session-owned reporting lifecycle must unwind mouse + focus
/// reporting even when the interactive loop never runs. Under
/// `WORKBENCH_FAIL_AFTER_REPORTING` the example errors out immediately after
/// enabling both; the only way `?1006l` / `?1004l` reach the wire is through
/// `Context`'s drop -> `TerminalSession::restore`. When reporting was enabled via a
/// bare free function (pre-fix) this path leaked — no disable was ever emitted.
#[test]
fn workbench_pty_unwinds_reporting_on_early_failure() {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("instrument_workbench"));
    cmd.env("TERM", "xterm-256color");
    cmd.env("WORKBENCH_FAIL_AFTER_REPORTING", "1");
    let mut capture = pty_capture::Capture::spawn(cmd, COLS, ROWS, Duration::from_secs(20));

    // The child enables reporting, then errors out; wait for it to exit.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        capture
            .collect_for(Duration::from_millis(50))
            .expect("collect PTY output");
        if matches!(capture.try_wait(), Ok(Some(_))) {
            break;
        }
        if Instant::now() >= deadline {
            panic!("workbench did not exit on the injected failure");
        }
    }
    // Drain any trailing restore bytes emitted during Drop.
    for _ in 0..4 {
        let _ = capture.collect_for(Duration::from_millis(30));
    }

    let raw = capture.raw().to_vec();
    let has = |needle: &[u8]| raw.windows(needle.len()).any(|w| w == needle);
    assert!(has(b"\x1b[?1006h"), "SGR mouse reporting was enabled");
    assert!(has(b"\x1b[?1004h"), "focus reporting was enabled");
    assert!(
        has(b"\x1b[?1006l"),
        "SGR mouse reporting must be disabled on the failure path, not leaked"
    );
    assert!(
        has(b"\x1b[?1004l"),
        "focus reporting must be disabled on the failure path, not leaked"
    );
}

/// `canvas=x,y,w,h` out of the HUD (emitted only under WORKBENCH_EMIT_CANVAS).
fn parse_canvas(screen: &str) -> Option<(u16, u16, u16, u16)> {
    let tok = screen
        .split_whitespace()
        .find(|t| t.starts_with("canvas="))?;
    let mut it = tok.strip_prefix("canvas=")?.split(',');
    let x = it.next()?.parse().ok()?;
    let y = it.next()?.parse().ok()?;
    let w = it.next()?.parse().ok()?;
    let h = it.next()?.parse().ok()?;
    Some((x, y, w, h))
}

/// The HUD `sel=<id>` token as an integer (`-1` when nothing is selected). Parsed as a
/// whole token so `sel=1` never substring-matches `sel=10`.
fn parse_sel(screen: &str) -> Option<i64> {
    screen
        .split_whitespace()
        .find_map(|t| t.strip_prefix("sel=").and_then(|v| v.parse().ok()))
}

/// The probe line's `world=(X.X,Y.Y)` as a pair (the `{:.1}` HUD format has no spaces).
/// An independent witness that the SGR bytes decoded to the expected world point.
fn parse_probe_world(screen: &str) -> Option<(f64, f64)> {
    let tok = screen
        .split_whitespace()
        .find(|t| t.starts_with("world=("))?;
    let inner = tok.strip_prefix("world=(")?.strip_suffix(')')?;
    let (x, y) = inner.split_once(',')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

/// A whitespace-delimited `prefix<u64>` HUD token (e.g. `undo=`), parsed as an integer.
fn parse_token_u64(screen: &str, prefix: &str) -> Option<u64> {
    screen
        .split_whitespace()
        .find_map(|t| t.strip_prefix(prefix).and_then(|v| v.parse().ok()))
}

/// Finding E / #144 acceptance #3: a REAL SGR mouse click must change a known model
/// readout — not merely stand in for a keyboard probe. With WORKBENCH_EMIT_CANVAS the
/// HUD exposes the painted canvas rect, so we target a known on-canvas cell. We draw a
/// shape (which auto-selects it), then click an EMPTY canvas cell and assert the
/// selection CLEARS (`sel=-1`) — a change only the pointer can make; the keyboard never
/// clears the selection by clicking. Deleting the mouse bytes leaves `sel` on the drawn
/// shape, so this cannot pass vacuously (the dead-send guard the old test lacked).
#[test]
fn workbench_pty_mouse_click_drives_the_model() {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("instrument_workbench"));
    cmd.env("TERM", "xterm-256color");
    cmd.env("WORKBENCH_EMIT_CANVAS", "1");
    let capture = pty_capture::Capture::spawn(cmd, COLS, ROWS, Duration::from_secs(30));
    let initial_termios = capture.initial_termios.clone();
    let mut h = Harness {
        capture,
        parser: Parser::new(ROWS, COLS, 0),
        processed: 0,
        initial_termios,
    };

    // Boot; read the painted canvas rect straight from the HUD.
    h.wait_text("canvas=");
    let (cx, cy, cw, ch) = parse_canvas(&h.screen()).expect("canvas rect in the HUD");
    assert!(
        cw >= 4 && ch >= 4,
        "canvas must be big enough to pick a far-corner empty cell: {cw}x{ch}"
    );

    // Draw a rectangle wall ('n'); the new shape is auto-selected, so sel != -1.
    h.send(b"n");
    h.wait_text("shapes=1");
    assert!(
        !h.screen().contains("sel=-1"),
        "a freshly drawn shape must be selected\n--- screen ---\n{}",
        h.screen()
    );

    // Click an EMPTY canvas cell — the far corner, well away from the top-left shape
    // at world (2,2)-(5,4). SGR is 1-based; button 0 press (M) then release (m).
    let (col, row) = (cx + cw - 1, cy + ch - 1);
    h.send(format!("\x1b[<0;{};{}M", col + 1, row + 1).as_bytes());
    h.send(format!("\x1b[<0;{};{}m", col + 1, row + 1).as_bytes());

    // The pointer cleared the selection: a model change no keystroke in this test made.
    h.wait_text("sel=-1");

    h.send(b"q");
    h.assert_restored();
}

/// #144 acceptance: prove the FULL mouse pick pipeline in a real terminal —
/// terminal bytes -> SGR decode -> renderer-confirmed canvas geometry (via
/// `WORKBENCH_EMIT_CANVAS`) -> world coordinate -> the SPECIFIC shape under the pointer
/// -> the `sel=` model readout. Every click cell is DERIVED from the emitted canvas rect
/// (L2: painted geometry is interaction geometry); no screen coordinate is hard-coded.
/// Covers a positive pick, a negative control, an overlap/topmost case, and a
/// retire/restore mutation control, and shows selection records no undoable edit.
#[test]
fn pty_click_selects_the_specific_shape_under_the_pointer() {
    let mut h = Harness::boot_canvas();
    h.wait_text("canvas=");
    let rect = parse_canvas(&h.screen()).expect("canvas rect in the HUD");
    let (_cx, _cy, cw, ch) = rect;
    assert!(
        cw >= 8 && ch >= 6,
        "canvas must fit the two test shapes: {cw}x{ch}"
    );

    // Draw shape A at the default spot: world rect (2,2)-(5,4). It auto-selects, so we
    // learn its id from `sel=` rather than assuming the counter's start value.
    h.send(b"n");
    h.wait_text("shapes=1");
    let id_a = parse_sel(&h.screen()).expect("sel= after draw");
    assert!(id_a >= 0, "a freshly drawn shape is selected (sel={id_a})");

    // Negative control: click an EMPTY far-corner cell. The selection CLEARS — a change
    // only the pointer makes — so the positive pick that follows is a real -1 -> id_a
    // transition, never a vacuous re-assert of the auto-selection.
    h.click_local(rect, cw - 1, ch - 1);
    h.wait_sel(-1);

    // Positive pick: click A's interior. World (3.5,3.5) is canvas-local cell (3,3) at
    // unit zoom (world = ix + 0.5) and lies inside rect (2,2)-(5,4). The click must
    // re-select exactly A, AND the probe must read the clicked world centre — an
    // independent witness that bytes -> decode -> canvas geometry -> world all hold.
    h.click_local(rect, 3, 3);
    h.wait_sel(id_a);
    assert_eq!(
        parse_probe_world(&h.screen()),
        Some((3.5, 3.5)),
        "probe reads the clicked world centre\n--- screen ---\n{}",
        h.screen()
    );

    // The draw was the only undoable edit so far; pin the depth to prove the clicks
    // above recorded none.
    let undo_after_a = parse_token_u64(&h.screen(), "undo=").expect("undo= token");

    // Overlap / topmost. Click inside A (world 4.5,3.5 = local (4,3)) to set the probe,
    // then 'n' draws B = rect (4,3)-(7,5), overlapping A's right half; B auto-selects.
    h.click_local(rect, 4, 3);
    h.wait_sel(id_a);
    h.send(b"n");
    h.wait_text("shapes=2");
    let id_b = parse_sel(&h.screen()).expect("sel= after 2nd draw");
    assert!(id_b != id_a, "B has a distinct id (A={id_a}, B={id_b})");

    // A-only cell (local (2,3) = world 2.5,3.5): inside A, left of B -> selects A.
    h.click_local(rect, 2, 3);
    h.wait_sel(id_a);
    // Overlap cell (local (4,3) = world 4.5,3.5): inside both -> the topmost (last-drawn) B.
    h.click_local(rect, 4, 3);
    h.wait_sel(id_b);

    // Mutation control: Ctrl-Z retires B, so the overlap click now finds only A; Ctrl-Y
    // restores B, so it finds the topmost B again. Picking tracks the live scene.
    h.send(b"\x1a");
    h.wait_text("shapes=1");
    h.click_local(rect, 4, 3);
    h.wait_sel(id_a);
    h.send(b"\x19");
    h.wait_text("shapes=2");
    h.click_local(rect, 4, 3);
    h.wait_sel(id_b);

    // Selection is not an edit: the undo depth only ever moved for the two draws and the
    // undo/redo, never for a click. (undo_after_a counted shape A; +1 is shape B.)
    let undo_now = parse_token_u64(&h.screen(), "undo=").expect("undo= token");
    assert_eq!(
        undo_now,
        undo_after_a + 1,
        "clicks record no edits; only the 2nd draw added one (undo {undo_after_a} -> {undo_now})"
    );

    h.send(b"q");
    h.assert_restored();
}
