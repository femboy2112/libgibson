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

use gibson::renderer::{BoundTarget, RenderMode, Renderer};
use gibson::session::TerminalSession;
use gibson::{Color, Node, Style};
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;
use vt100::Parser;

const W: u16 = 24;
const H: u16 = 6;

/// One stable destination with separately controlled fault injection.
#[derive(Default)]
struct WriterState {
    budget: Option<usize>,
    accepted: Vec<u8>,
}
struct ControlledWriter(Rc<RefCell<WriterState>>);
impl Write for ControlledWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let mut state = self.0.borrow_mut();
        let n = match state.budget {
            Some(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "scripted torn write",
                ))
            }
            Some(left) => b.len().min(left),
            None => b.len(),
        };
        state.accepted.extend_from_slice(&b[..n]);
        if let Some(left) = &mut state.budget {
            *left -= n;
        }
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn with_target<T>(
    mode: RenderMode,
    rows: u16,
    f: impl FnOnce(&mut BoundTarget<'_>, &Rc<RefCell<WriterState>>) -> T,
) -> T {
    let mut renderer = Renderer::new(mode);
    let mut session = TerminalSession::headless(W, rows);
    let state = Rc::new(RefCell::new(WriterState::default()));
    let mut writer = ControlledWriter(state.clone());
    let mut target = renderer.bind_target(&mut session, &mut writer).unwrap();
    f(&mut target, &state)
}
fn take(state: &Rc<RefCell<WriterState>>) -> Vec<u8> {
    std::mem::take(&mut state.borrow_mut().accepted)
}
fn arm(state: &Rc<RefCell<WriterState>>, budget: usize) {
    let mut s = state.borrow_mut();
    assert!(s.accepted.is_empty(), "drain before fault injection");
    s.budget = Some(budget);
}
fn heal(state: &Rc<RefCell<WriterState>>) {
    state.borrow_mut().budget = None;
}
fn render(target: &mut BoundTarget<'_>, mut node: Node) -> io::Result<()> {
    target.render(&mut node).map(|_| ())
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

/// The full clean byte stream of frame 2 after frame 1 (to know every cut point).
fn frame2_stream(mode: RenderMode, rows: u16) -> Vec<u8> {
    with_target(mode, rows, |target, state| {
        render(target, f1()).unwrap();
        let initial = take(state);
        render(target, f2()).unwrap();
        let second = take(state);
        assert!(!second.is_empty(), "the cut-point matrix must not be empty");
        let mut p = Parser::new(rows, W, 0);
        p.process(&initial);
        p.process(&second);
        assert!(
            p.screen().contents().contains("BRAVO"),
            "healthy frame 2 control"
        );
        second
    })
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

fn clean_fullscreen(f3: fn() -> Node, expected: &str) -> Vec<String> {
    with_target(RenderMode::Fullscreen, H, |target, state| {
        render(target, f3()).unwrap();
        let mut p = Parser::new(H, W, 0);
        p.process(&take(state));
        assert!(p.screen().contents().contains("alpha alpha alpha"));
        assert!(p.screen().contents().contains(expected));
        let cell = p.screen().cell(1, 0).unwrap();
        assert_ne!(cell.fgcolor(), vt100::Color::Default);
        if expected == "charlie" {
            assert!(cell.bold());
        }
        snapshot(&p)
    })
}
fn torn_fullscreen(n: usize, f3: fn() -> Node, frame2: &[u8]) -> (Vec<String>, bool) {
    with_target(RenderMode::Fullscreen, H, |target, state| {
        render(target, f1()).unwrap();
        let mut term = take(state);
        arm(state, n);
        let failed = render(target, f2()).is_err();
        let torn = take(state);
        assert_eq!(torn.len(), n, "fault must reach its requested cut");
        assert_eq!(torn, frame2[..n], "the injected prefix must match frame 2");
        term.extend_from_slice(&torn);
        heal(state);
        render(target, f3()).unwrap();
        term.extend_from_slice(&take(state));
        let mut p = Parser::new(H, W, 0);
        p.process(&term);
        (snapshot(&p), failed)
    })
}

#[test]
fn fullscreen_recovers_exactly_after_a_torn_frame_at_every_cut_point_reverting_frame() {
    let want = clean_fullscreen(f3_revert, "plain status line");
    let frame2 = frame2_stream(RenderMode::Fullscreen, H);
    let len = frame2.len();
    let mut bad = Vec::new();
    for n in 0..len {
        let (got, failed) = torn_fullscreen(n, f3_revert, &frame2);
        assert!(
            failed,
            "a writer that stops at {n}/{len} bytes must fail the render"
        );
        if got != want {
            bad.push(n);
        }
    }
    assert!(bad.is_empty(), "after a torn frame 2 the screen did not converge to frame 3 for {} of {len} cut points (first {:?})", bad.len(), &bad[..bad.len().min(8)]);
}

#[test]
fn fullscreen_recovers_exactly_after_a_torn_frame_at_every_cut_point_new_frame() {
    let want = clean_fullscreen(f3_other, "charlie");
    let frame2 = frame2_stream(RenderMode::Fullscreen, H);
    let len = frame2.len();
    let mut bad = Vec::new();
    for n in 0..len {
        let (got, failed) = torn_fullscreen(n, f3_other, &frame2);
        assert!(failed);
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

#[test]
fn inline_abandons_the_torn_region_and_draws_the_next_frame_intact() {
    let clean_rows = with_target(RenderMode::Inline, 40, |target, state| {
        render(target, f3_other()).unwrap();
        let mut p = Parser::new(40, W, 0);
        p.process(&take(state));
        assert!(p.screen().contents().contains("alpha alpha alpha"));
        assert!(p.screen().contents().contains("charlie"));
        let (crow, _) = p.screen().cursor_position();
        p.screen()
            .rows(0, W)
            .take(crow as usize + 1)
            .collect::<Vec<String>>()
    });
    let frame2 = frame2_stream(RenderMode::Inline, 40);
    let len = frame2.len();
    let mut bad = Vec::new();
    for n in 0..len {
        let actual = with_target(RenderMode::Inline, 40, |target, state| {
            render(target, f1()).unwrap();
            let mut term = take(state);
            arm(state, n);
            assert!(render(target, f2()).is_err());
            let torn = take(state);
            assert_eq!(torn.len(), n);
            assert_eq!(torn, frame2[..n]);
            term.extend_from_slice(&torn);
            heal(state);
            render(target, f3_other()).unwrap();
            term.extend_from_slice(&take(state));
            let mut p = Parser::new(40, W, 0);
            p.process(&term);
            let (crow, _) = p.screen().cursor_position();
            let all: Vec<String> = p.screen().rows(0, W).collect();
            let start = (crow as usize + 1).saturating_sub(clean_rows.len());
            all[start..=crow as usize].to_vec()
        });
        if actual != clean_rows {
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

#[test]
fn a_failed_commit_marks_the_physical_state_uncertain_until_a_frame_lands() {
    with_target(RenderMode::Fullscreen, H, |target, state| {
        render(target, f1()).unwrap();
        take(state);
        assert!(!target.physical_state_uncertain());
        arm(state, 7);
        assert!(render(target, f2()).is_err());
        assert_eq!(take(state).len(), 7);
        assert!(
            target.physical_state_uncertain(),
            "a torn write is never reported as harmless"
        );
        arm(state, 0);
        assert!(render(target, f2()).is_err());
        assert!(take(state).is_empty());
        assert!(target.physical_state_uncertain());
        heal(state);
        render(target, f2()).unwrap();
        assert!(!target.physical_state_uncertain());
        assert!(
            take(state).windows(4).any(|w| w == b"\x1b[2J"),
            "the recovery frame re-establishes the canvas"
        );
    });
}

#[test]
fn a_failure_that_delivered_nothing_keeps_the_physical_state_certain() {
    for mode in [RenderMode::Fullscreen, RenderMode::Inline] {
        with_target(mode, H, |target, state| {
            render(target, f1()).unwrap();
            take(state);
            arm(state, 0);
            assert!(render(target, f2()).is_err());
            assert!(take(state).is_empty());
            assert!(
                !target.physical_state_uncertain(),
                "{mode:?}: nothing was sent"
            );
            heal(state);
            render(target, f2()).unwrap();
            let retry = take(state);
            assert!(
                !retry.windows(4).any(|w| w == b"\x1b[2J"),
                "{mode:?}: a certain no-op failure needs no canvas reset"
            );
        });
    }
}

#[test]
fn review_a_zero_byte_failed_resize_frame_keeps_its_canvas_reset() {
    with_target(RenderMode::Fullscreen, H, |target, state| {
        render(target, f1()).unwrap();
        take(state);
        target.set_terminal_size(W - 4, H);
        arm(state, 0);
        assert!(render(target, f2()).is_err());
        assert!(take(state).is_empty());
        heal(state);
        render(target, f2()).unwrap();
        assert!(
            take(state).windows(4).any(|w| w == b"\x1b[2J"),
            "the retried resize frame must still clear the canvas"
        );
    });
}

#[test]
fn review_a_fullscreen_commit_does_not_clear_the_uncertainty() {
    with_target(RenderMode::Fullscreen, H, |target, state| {
        render(target, f1()).unwrap();
        take(state);
        arm(state, 9);
        assert!(render(target, f2()).is_err());
        assert_eq!(take(state).len(), 9);
        heal(state);
        target.commit_text("a committed line").unwrap();
        take(state);
        assert!(target.physical_state_uncertain());
        render(target, f3_other()).unwrap();
        assert!(
            take(state).windows(4).any(|w| w == b"\x1b[2J"),
            "the canvas is still re-established after the commit"
        );
    });
}

#[test]
fn review_recovery_re_enables_autowrap_before_writing() {
    let cut = with_target(RenderMode::Inline, 40, |target, state| {
        render(target, f1()).unwrap();
        take(state);
        render(target, f2()).unwrap();
        let b = take(state);
        b.windows(5)
            .position(|w| w == b"\x1b[?7l")
            .expect("frame 2 disables autowrap")
            + 5
    });
    with_target(RenderMode::Inline, 40, |target, state| {
        render(target, f1()).unwrap();
        take(state);
        arm(state, cut);
        assert!(render(target, f2()).is_err());
        assert_eq!(take(state).len(), cut);
        heal(state);
        target
            .commit_text("a long committed line that is wider than the terminal")
            .unwrap();
        let committed = take(state);
        let wrap_on = committed.windows(5).position(|w| w == b"\x1b[?7h");
        let text_at = committed.windows(6).position(|w| w == b"a long");
        assert!(
            matches!((wrap_on, text_at), (Some(w), Some(t)) if w < t),
            "auto-wrap must be re-enabled before the committed text"
        );
    });
}
