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

// ---------------------------------------------------------------------------
// FX substrate damage bounds
// ---------------------------------------------------------------------------

fn wireframe_node(t: f32, cols: u16, rows: u16) -> Node {
    let mut canvas = gibson::BrailleCanvas::new(cols, rows);
    let tf = gibson::Transform3::rotation(t * 0.9, t * 1.3, t * 0.4);
    gibson::Projector::default().draw(
        &gibson::Mesh::torus(1.0, 0.38, 16, 10),
        &tf,
        &mut canvas,
        1.0,
    );
    Node::raster(canvas.to_surface(gibson::Style::default()))
        .width(cols as f32)
        .height(rows as f32)
}

#[test]
fn wireframe_rotation_damage_is_bounded_and_static_frame_is_free() {
    let cols = 30u16;
    let rows = 10u16;
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(cols, rows);
    let mut out = Vec::new();

    renderer
        .render(&mut wireframe_node(0.0, cols, rows), &mut session, &mut out)
        .unwrap();
    out.clear();
    let (dirty, total, bytes, _, _) = renderer
        .render(
            &mut wireframe_node(0.05, cols, rows),
            &mut session,
            &mut out,
        )
        .unwrap();
    assert!(bytes > 0);
    assert!(
        dirty * 2 <= total,
        "wireframe rotation dirtied too much: {dirty}/{total}"
    );

    // A frozen frame must emit nothing.
    out.clear();
    let (_, _, bytes_same, _, _) = renderer
        .render(
            &mut wireframe_node(0.05, cols, rows),
            &mut session,
            &mut out,
        )
        .unwrap();
    assert_eq!(bytes_same, 0, "identical wireframe must be clean");
}

#[test]
fn particle_field_motion_is_bounded() {
    let cols = 40u16;
    let rows = 10u16;
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(cols, rows);
    let mut out = Vec::new();

    let build = |step: usize| {
        let mut ps = gibson::ParticleSystem::new(0x1234);
        // Sparse: 60 dots on a 40x10 cell grid. Advance in a visible step
        // (sub-dot motion would legitimately round back to the same cell).
        ps.burst(60, cols as f32, rows as f32 * 0.5, 8.0, 2.0);
        ps.update(step as f32 * 0.1);
        let mut canvas = gibson::BrailleCanvas::new(cols, rows);
        ps.render_braille(&mut canvas);
        Node::raster(canvas.to_surface(gibson::Style::default()))
            .width(cols as f32)
            .height(rows as f32)
    };

    renderer
        .render(&mut build(0), &mut session, &mut out)
        .unwrap();
    out.clear();
    let (dirty, total, _b, _, _) = renderer
        .render(&mut build(1), &mut session, &mut out)
        .unwrap();
    assert!(total > 0);
    // A sparse field advancing a 0.1s step must not repaint the whole screen.
    // (A deliberately visible step: sub-dot motion can legitimately round back to
    // the same cell.) The bound is generous but catches an accidental
    // whole-screen regression.
    assert!(
        dirty * 2 <= total,
        "sparse particle motion dirtied the whole field: {dirty}/{total}"
    );
    assert!(dirty > 0, "particles did move; damage must not be zero");
}

#[test]
fn frozen_particle_state_is_zero_damage_and_zero_bytes() {
    let cols = 40u16;
    let rows = 10u16;
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(cols, rows);
    let mut out = Vec::new();

    let build = || {
        let mut ps = gibson::ParticleSystem::new(0xABCD);
        ps.burst(80, cols as f32, rows as f32 * 0.5, 6.0, 2.0);
        for _ in 0..3 {
            ps.update(1.0 / 60.0);
        }
        let mut canvas = gibson::BrailleCanvas::new(cols, rows);
        ps.render_braille(&mut canvas);
        Node::raster(canvas.to_surface(gibson::Style::default()))
            .width(cols as f32)
            .height(rows as f32)
    };

    renderer
        .render(&mut build(), &mut session, &mut out)
        .unwrap();
    out.clear();
    let (dirty, _total, bytes, _, _) = renderer
        .render(&mut build(), &mut session, &mut out)
        .unwrap();
    assert_eq!(dirty, 0, "a frozen particle field has zero logical damage");
    assert_eq!(bytes, 0, "a frozen particle field emits zero frame bytes");
}

/// A line that shrinks to nothing is erased by a single `CSI K`, but it
/// logically clears many cells. Logical damage and wire cost must not be
/// collapsed into one number.
#[test]
fn erase_to_eol_has_large_logical_damage_but_tiny_wire_cost() {
    let cols = 120u16;
    let rows = 4u16;
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(cols, rows);
    let mut out = Vec::new();

    let full = "X".repeat(cols as usize);
    let mut long = Node::text(full.clone(), gibson::Style::new())
        .width(cols as f32)
        .height(1.0);
    renderer.render(&mut long, &mut session, &mut out).unwrap();
    out.clear();

    // Next frame: the same row is blank. The whole row is logically erased.
    let mut blank = Node::text(" ", gibson::Style::new()).width(1.0).height(1.0);
    let (dirty, total, bytes, _, _) = renderer.render(&mut blank, &mut session, &mut out).unwrap();

    // Logical damage should be a large fraction of the row ...
    assert!(
        dirty >= (cols as usize) - 2,
        "erase must report the erased cells as logical damage: {dirty}"
    );
    let _ = total;
    // ... while the wire cost is a handful of bytes (one CSI K plus cursor move
    // and the synchronized-update terminator), far below the 120 erased cells.
    assert!(
        bytes < 48,
        "CSI-K erase should cost few bytes, emitted {bytes}"
    );
}

/// A one-cell sprite move must dirty roughly the union of its old and new
/// footprints, not the whole framebuffer.
#[test]
fn one_cell_sprite_move_is_bounded_to_old_plus_new_footprint() {
    let cols = 60u16;
    let rows = 12u16;
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(cols, rows);
    let mut out = Vec::new();

    let sprite = |x: f32| {
        Node::stack()
            .percent_width(100.0)
            .percent_height(100.0)
            .child(Node::text("BACKGROUND", gibson::Style::new()))
            .child(
                Node::text("X", gibson::Style::new())
                    .width(1.0)
                    .height(1.0)
                    .offset(x, 6.0),
            )
    };

    renderer
        .render(&mut sprite(20.0), &mut session, &mut out)
        .unwrap();
    out.clear();
    let (dirty, total, _bytes, _, _) = renderer
        .render(&mut sprite(21.0), &mut session, &mut out)
        .unwrap();
    // Old + new cell + a small allowance for any control/cursor cell.
    assert!(
        dirty <= 4,
        "1-cell sprite move dirtied {dirty} cells (total {total}); expected a tiny footprint"
    );
}

#[test]
fn identical_frame_reports_zero_logical_damage() {
    let cols = 50u16;
    let rows = 8u16;
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(cols, rows);
    let mut out = Vec::new();
    let mut root = Node::rich_text_wrapped(
        gibson::RichText::raw("static dashboard\nno animation here"),
        WrapMode::NoWrap,
    )
    .width(cols as f32)
    .height(rows as f32);
    renderer.render(&mut root, &mut session, &mut out).unwrap();
    out.clear();
    let (dirty, _total, bytes, _, _) = renderer.render(&mut root, &mut session, &mut out).unwrap();
    assert_eq!(dirty, 0);
    assert_eq!(bytes, 0);
}

#[test]
fn resize_during_wireframe_animation_is_safe_and_reanchors() {
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(60, 20);
    let mut parser = vt100::Parser::new(20, 60, 0);
    let mut out = Vec::new();

    renderer
        .render(&mut wireframe_node(0.1, 40, 12), &mut session, &mut out)
        .unwrap();
    parser.process(&out);
    let before = renderer.anchor_resyncs;

    session.set_terminal_size(30, 10);
    parser.screen_mut().set_size(10, 30);
    let mut out2 = Vec::new();
    renderer
        .render(&mut wireframe_node(0.2, 20, 8), &mut session, &mut out2)
        .unwrap();
    parser.process(&out2);

    assert!(renderer.anchor_resyncs > before, "resize must re-anchor");
    for line in parser.screen().rows(0, 30) {
        assert!(
            unicode_width::UnicodeWidthStr::width(line.as_str()) <= 30,
            "line overflows after resize: {line:?}"
        );
    }
}
