//! RXIV Gate G3 — partial terminal-write recovery through the REAL renderer.
//!
//! `TerminalTransaction::commit_counted` is a counted write loop + `flush` (before this gate, one
//! `write_all` + `flush`), and a write can fail after a
//! partial write: the terminal may hold a torn PREFIX of a frame (synchronized-update framing is a
//! display hint, not I/O rollback). Nothing can un-send those bytes. The question this battery
//! settles is what the renderer does NEXT.
//!
//! For every cut point `N` of a frame's byte stream, a fresh renderer paints frame 1 (healthy), then
//! frame 2 through a writer that accepts exactly `N` bytes and then fails, then frame 3 (healthy).
//! Every byte that reached the "terminal" is fed, in order, to a vt100 emulator, and the resulting
//! screen — glyphs AND attributes — is compared with a clean render of frame 3 alone.
//!
//! * **Fullscreen — recoverable.** After a failed commit the renderer's belief about the screen is
//!   void; the next frame must re-establish the whole canvas (reset SGR, clear, full repaint), so the
//!   screen converges to frame 3 exactly, whatever prefix was torn.
//! * **Inline — uncertain, fail-safe.** The live region is addressed by RELATIVE cursor motion; after
//!   a torn write the cursor row is unknown, so the region cannot be repaired in place. The torn
//!   region is abandoned to scrollback and a fresh region is allocated below the cursor: frame 3 is
//!   then intact immediately above the parked cursor. The renderer never claims the torn terminal
//!   was left unchanged.

use gibson::renderer::{RenderMode, Renderer};
use gibson::session::TerminalSession;
use gibson::{Color, Node, Style};
use std::io::{self, Write};
use vt100::Parser;

const W: u16 = 24;
const H: u16 = 6;

/// A writer that accepts exactly `budget` bytes, then fails every write.
struct FailAfter {
    budget: usize,
    sink: Vec<u8>,
}

impl Write for FailAfter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.budget == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "scripted torn write",
            ));
        }
        let n = b.len().min(self.budget);
        self.sink.extend_from_slice(&b[..n]);
        self.budget -= n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn frame(lines: &[(&str, Style)]) -> Node {
    let mut col = Node::col();
    for (t, s) in lines {
        col = col.child(Node::text(t.to_string(), *s));
    }
    col
}

fn f1() -> Node {
    frame(&[
        ("alpha alpha alpha", Style::new()),
        ("plain status line", Style::new().fg(Color::Green)),
        ("third row", Style::new()),
    ])
}
fn f2() -> Node {
    frame(&[
        ("BRAVO BRAVO", Style::new().bold().fg(Color::Red)),
        ("XXXXXXXXXXXXXXXXXXXXXX", Style::new().reverse()),
        ("third row changed", Style::new().fg(Color::Yellow)),
        ("a fourth row appears", Style::new().underline()),
    ])
}
fn f3_revert() -> Node {
    f1()
}
fn f3_other() -> Node {
    frame(&[
        ("alpha alpha alpha", Style::new()),
        ("charlie", Style::new().bold().fg(Color::Red)),
        ("third row", Style::new()),
    ])
}

fn render(
    r: &mut Renderer,
    s: &mut TerminalSession,
    node: Node,
    w: &mut dyn Write,
) -> io::Result<()> {
    let mut node = node;
    r.render(&mut node, s, w).map(|_| ())
}

/// The full clean byte stream of frame 2 after frame 1 (to know every cut point).
fn frame2_len(mode: RenderMode) -> usize {
    let (mut r, mut s) = (Renderer::new(mode), TerminalSession::headless(W, H));
    let mut sink = Vec::new();
    render(&mut r, &mut s, f1(), &mut sink).unwrap();
    let mut two = Vec::new();
    render(&mut r, &mut s, f2(), &mut two).unwrap();
    two.len()
}

/// Screen snapshot: per-cell glyph + the attributes that matter.
fn snapshot(p: &Parser) -> Vec<String> {
    let sc = p.screen();
    let mut out = Vec::new();
    for row in 0..H {
        let mut line = String::new();
        for col in 0..W {
            let c = sc.cell(row, col).unwrap();
            line.push_str(&format!(
                "{}|{:?}{:?}{}{}{};",
                c.contents(),
                c.fgcolor(),
                c.bgcolor(),
                c.bold() as u8,
                c.inverse() as u8,
                c.underline() as u8
            ));
        }
        out.push(line);
    }
    out
}

fn clean_fullscreen(f3: fn() -> Node) -> Vec<String> {
    let (mut r, mut s) = (
        Renderer::new(RenderMode::Fullscreen),
        TerminalSession::headless(W, H),
    );
    let mut bytes = Vec::new();
    render(&mut r, &mut s, f3(), &mut bytes).unwrap();
    let mut p = Parser::new(H, W, 0);
    p.process(&bytes);
    snapshot(&p)
}

fn torn_fullscreen(n: usize, f3: fn() -> Node) -> (Vec<String>, bool) {
    let (mut r, mut s) = (
        Renderer::new(RenderMode::Fullscreen),
        TerminalSession::headless(W, H),
    );
    let mut term = Vec::new();
    render(&mut r, &mut s, f1(), &mut term).unwrap();
    let mut torn = FailAfter {
        budget: n,
        sink: Vec::new(),
    };
    let failed = render(&mut r, &mut s, f2(), &mut torn).is_err();
    term.extend_from_slice(&torn.sink);
    render(&mut r, &mut s, f3(), &mut term).unwrap();
    let mut p = Parser::new(H, W, 0);
    p.process(&term);
    (snapshot(&p), failed)
}

#[test]
fn fullscreen_recovers_exactly_after_a_torn_frame_at_every_cut_point_reverting_frame() {
    let want = clean_fullscreen(f3_revert);
    let len = frame2_len(RenderMode::Fullscreen);
    let mut bad = Vec::new();
    for n in 0..len {
        let (got, failed) = torn_fullscreen(n, f3_revert);
        assert!(
            failed,
            "a writer that stops at {n}/{len} bytes must fail the render"
        );
        if got != want {
            bad.push(n);
        }
    }
    assert!(
        bad.is_empty(),
        "after a torn frame 2 the screen did not converge to frame 3 for {} of {len} cut points (first {:?})",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
}

#[test]
fn fullscreen_recovers_exactly_after_a_torn_frame_at_every_cut_point_new_frame() {
    let want = clean_fullscreen(f3_other);
    let len = frame2_len(RenderMode::Fullscreen);
    let mut bad = Vec::new();
    for n in 0..len {
        let (got, _) = torn_fullscreen(n, f3_other);
        if got != want {
            bad.push(n);
        }
    }
    assert!(
        bad.is_empty(),
        "styles/cells leaked from the torn frame for {} of {len} cut points (first {:?})",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
}

/// Inline: after a torn write the torn region is abandoned (honestly uncertain) and frame 3 is drawn
/// intact in a fresh region ending at the parked cursor.
#[test]
fn inline_abandons_the_torn_region_and_draws_the_next_frame_intact() {
    // The clean inline frame-3 rows (a fresh region at the top of an empty terminal).
    let clean_rows = {
        let (mut r, mut s) = (
            Renderer::new(RenderMode::Inline),
            TerminalSession::headless(W, 40),
        );
        let mut bytes = Vec::new();
        render(&mut r, &mut s, f3_other(), &mut bytes).unwrap();
        let mut p = Parser::new(40, W, 0);
        p.process(&bytes);
        let (crow, _) = p.screen().cursor_position();
        let rows: Vec<String> = p.screen().rows(0, W).take(crow as usize + 1).collect();
        rows
    };
    let len = {
        let (mut r, mut s) = (
            Renderer::new(RenderMode::Inline),
            TerminalSession::headless(W, 40),
        );
        let mut sink = Vec::new();
        render(&mut r, &mut s, f1(), &mut sink).unwrap();
        let mut two = Vec::new();
        render(&mut r, &mut s, f2(), &mut two).unwrap();
        two.len()
    };
    let mut bad = Vec::new();
    for n in 0..len {
        let (mut r, mut s) = (
            Renderer::new(RenderMode::Inline),
            TerminalSession::headless(W, 40),
        );
        let mut term = Vec::new();
        render(&mut r, &mut s, f1(), &mut term).unwrap();
        let mut torn = FailAfter {
            budget: n,
            sink: Vec::new(),
        };
        assert!(render(&mut r, &mut s, f2(), &mut torn).is_err());
        term.extend_from_slice(&torn.sink);
        render(&mut r, &mut s, f3_other(), &mut term).unwrap();
        let mut p = Parser::new(40, W, 0);
        p.process(&term);
        let (crow, _) = p.screen().cursor_position();
        let k = clean_rows.len();
        let all: Vec<String> = p.screen().rows(0, W).collect();
        let start = (crow as usize + 1).saturating_sub(k);
        if all[start..=crow as usize] != clean_rows[..] {
            bad.push(n);
        }
    }
    assert!(
        bad.is_empty(),
        "frame 3 was not drawn intact above the cursor for {} of {len} cut points (first {:?})",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
}

/// The uncertainty is observable and cleared only by a successful re-establishing frame.
#[test]
fn a_failed_commit_marks_the_physical_state_uncertain_until_a_frame_lands() {
    let (mut r, mut s) = (
        Renderer::new(RenderMode::Fullscreen),
        TerminalSession::headless(W, H),
    );
    let mut ok = Vec::new();
    render(&mut r, &mut s, f1(), &mut ok).unwrap();
    assert!(!r.physical_state_uncertain());
    let mut torn = FailAfter {
        budget: 7,
        sink: Vec::new(),
    };
    assert!(render(&mut r, &mut s, f2(), &mut torn).is_err());
    assert!(
        r.physical_state_uncertain(),
        "a torn write is never reported as harmless"
    );
    // A second failure keeps it uncertain.
    let mut dead = FailAfter {
        budget: 0,
        sink: Vec::new(),
    };
    assert!(render(&mut r, &mut s, f2(), &mut dead).is_err());
    assert!(r.physical_state_uncertain());
    let mut ok2 = Vec::new();
    render(&mut r, &mut s, f2(), &mut ok2).unwrap();
    assert!(!r.physical_state_uncertain());
    assert!(
        ok2.windows(4).any(|w| w == b"\x1b[2J"),
        "the recovery frame re-establishes the canvas"
    );
}

/// A failure that delivered ZERO bytes is CERTAIN (the terminal is unchanged): the renderer stays
/// certain and the retry diffs in place — the RXI Gate F1 in-place restyle contract. Only a prefix
/// (or an unconfirmed flush) makes the physical state uncertain.
#[test]
fn a_failure_that_delivered_nothing_keeps_the_physical_state_certain() {
    for mode in [RenderMode::Fullscreen, RenderMode::Inline] {
        let (mut r, mut s) = (Renderer::new(mode), TerminalSession::headless(W, H));
        let mut term = Vec::new();
        render(&mut r, &mut s, f1(), &mut term).unwrap();
        let mut dead = FailAfter {
            budget: 0,
            sink: Vec::new(),
        };
        assert!(render(&mut r, &mut s, f2(), &mut dead).is_err());
        assert!(dead.sink.is_empty());
        assert!(!r.physical_state_uncertain(), "{mode:?}: nothing was sent");
        let mut retry = Vec::new();
        render(&mut r, &mut s, f2(), &mut retry).unwrap();
        assert!(
            !retry.windows(4).any(|w| w == b"\x1b[2J"),
            "{mode:?}: a certain no-op failure needs no canvas reset"
        );
    }
}

// ---- RXIV review findings 3–5 (independent adversarial review of b70aa5c), red-first ----------

/// Finding 3: a RESIZE frame whose commit delivered nothing still owes the canvas reset the resize
/// required. Restoring the emission state must not drop that obligation (the retry would otherwise
/// leave reflowed old content in cells the fresh diff elides).
#[test]
fn review_a_zero_byte_failed_resize_frame_keeps_its_canvas_reset() {
    let (mut r, mut s) = (
        Renderer::new(RenderMode::Fullscreen),
        TerminalSession::headless(W, H),
    );
    let mut ok = Vec::new();
    render(&mut r, &mut s, f1(), &mut ok).unwrap();
    s.set_terminal_size(W - 4, H);
    let mut dead = FailAfter {
        budget: 0,
        sink: Vec::new(),
    };
    assert!(render(&mut r, &mut s, f2(), &mut dead).is_err());
    let mut retry = Vec::new();
    render(&mut r, &mut s, f2(), &mut retry).unwrap();
    assert!(
        retry.windows(4).any(|w| w == b"\x1b[2J"),
        "the retried resize frame must still clear the canvas"
    );
}

/// Finding 4: in FULLSCREEN mode a successful scrollback commit does not re-establish the canvas,
/// so it must not clear the "physical state uncertain" mark left by a torn frame.
#[test]
fn review_a_fullscreen_commit_does_not_clear_the_uncertainty() {
    let (mut r, mut s) = (
        Renderer::new(RenderMode::Fullscreen),
        TerminalSession::headless(W, H),
    );
    let mut ok = Vec::new();
    render(&mut r, &mut s, f1(), &mut ok).unwrap();
    let mut torn = FailAfter {
        budget: 9,
        sink: Vec::new(),
    };
    assert!(render(&mut r, &mut s, f2(), &mut torn).is_err());
    let mut committed = Vec::new();
    r.commit_text("a committed line", &mut s, &mut committed)
        .unwrap();
    assert!(r.physical_state_uncertain());
    let mut next = Vec::new();
    render(&mut r, &mut s, f3_other(), &mut next).unwrap();
    assert!(
        next.windows(4).any(|w| w == b"\x1b[2J"),
        "the canvas is still re-established after the commit"
    );
}

/// Finding 5: a torn frame can leave auto-wrap OFF (`?7l` delivered, `?7h` not). Every recovery
/// path must re-enable it before writing, or scrollback lines are clipped permanently.
#[test]
fn review_recovery_re_enables_autowrap_before_writing() {
    let (mut r, mut s) = (
        Renderer::new(RenderMode::Inline),
        TerminalSession::headless(W, 40),
    );
    let mut ok = Vec::new();
    render(&mut r, &mut s, f1(), &mut ok).unwrap();
    // Cut frame 2 right after its `?7l`.
    let cut = {
        let (mut r2, mut s2) = (
            Renderer::new(RenderMode::Inline),
            TerminalSession::headless(W, 40),
        );
        let mut a = Vec::new();
        render(&mut r2, &mut s2, f1(), &mut a).unwrap();
        let mut b = Vec::new();
        render(&mut r2, &mut s2, f2(), &mut b).unwrap();
        b.windows(5)
            .position(|w| w == b"\x1b[?7l")
            .expect("frame 2 disables autowrap")
            + 5
    };
    let mut torn = FailAfter {
        budget: cut,
        sink: Vec::new(),
    };
    assert!(render(&mut r, &mut s, f2(), &mut torn).is_err());
    let mut committed = Vec::new();
    r.commit_text(
        "a long committed line that is wider than the terminal",
        &mut s,
        &mut committed,
    )
    .unwrap();
    let wrap_on = committed.windows(5).position(|w| w == b"\x1b[?7h");
    let text_at = committed.windows(6).position(|w| w == b"a long");
    assert!(
        matches!((wrap_on, text_at), (Some(w), Some(t)) if w < t),
        "auto-wrap must be re-enabled before the committed text"
    );
}
