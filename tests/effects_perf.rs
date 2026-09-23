//! Bounded-cost checks for animated sub-cell effects.
//!
//! These are not micro-benchmarks. They assert that moving a small instrument by
//! one phase does not accidentally dirty the entire framebuffer.

use gibson::node::{Node, WrapMode};
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;
use gibson::{BrailleCanvas, RenderMode, Style};

fn oscilloscope_node(phase: f32, cols: u16, lines: u16, style: Style) -> Node {
    // A braille waveform whose vertical offset shifts with `phase`.
    let mut c = BrailleCanvas::new(cols, lines);
    let pw = c.pixel_width() as f32;
    let ph = c.pixel_height() as f32;
    let mut prev: Option<(i32, i32)> = None;
    let mut x = 0i32;
    while x < c.pixel_width() as i32 {
        let t = x as f32 / pw;
        let y = (ph * 0.5 + (t * 12.0 + phase).sin() * (ph * 0.35)).clamp(0.0, ph - 1.0);
        let yi = y.round() as i32;
        if let Some((px, py)) = prev {
            c.line(px, py, x, yi);
        } else {
            c.set(x, yi);
        }
        prev = Some((x, yi));
        x += 1;
    }
    Node::rich_text_wrapped(c.to_rich_text(style), WrapMode::NoWrap)
        .width(cols as f32)
        .height(lines as f32)
}

#[test]
fn braille_phase_update_has_bounded_diff_cost() {
    let cols = 40u16;
    let rows = 6u16;
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(cols, rows);
    let mut out = Vec::new();

    let mut root = oscilloscope_node(0.0, cols, 4, Style::new());
    let (dirty, total, bytes, _, _) = renderer.render(&mut root, &mut session, &mut out).unwrap();
    assert!(dirty <= total);
    assert!(bytes > 0);
    out.clear();

    // Advance the phase by a small amount: only a fraction of cells should move.
    let mut root2 = oscilloscope_node(0.2, cols, 4, Style::new());
    let (dirty2, total2, _bytes2, _, _) =
        renderer.render(&mut root2, &mut session, &mut out).unwrap();
    assert!(total2 > 0);
    assert!(
        dirty2 < total2,
        "phase update dirtied the whole framebuffer: {dirty2}/{total2}"
    );
    assert!(
        dirty2 * 2 <= total2,
        "phase update should dirty a minority of cells: {dirty2}/{total2}"
    );
}

#[test]
fn scanline_overlay_moves_with_small_diff() {
    let cols = 60u16;
    let rows = 12u16;
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(cols, rows);
    let mut out = Vec::new();

    let build = |y: u16| {
        Node::stack()
            .percent_width(100.0)
            .percent_height(100.0)
            .child(
                Node::col()
                    .percent_width(100.0)
                    .percent_height(100.0)
                    .child(Node::text("CONTENT", Style::new())),
            )
            .child(
                Node::col()
                    .percent_width(100.0)
                    .percent_height(100.0)
                    .padding_top(y as f32)
                    .child(Node::dim().percent_width(100.0).height(1.0)),
            )
    };

    let mut root = build(2);
    let (_d, total, _b, _, _) = renderer.render(&mut root, &mut session, &mut out).unwrap();
    out.clear();
    let mut root2 = build(3);
    let (dirty, total2, _b2, _, _) = renderer.render(&mut root2, &mut session, &mut out).unwrap();
    assert!(total2 == total);
    // One dim row moved by one: the changed cells should be ~2 rows, far below
    // the whole screen.
    assert!(
        dirty < (cols as usize) * 4,
        "scanline dirtied too much: {dirty} of {total2}"
    );
}
