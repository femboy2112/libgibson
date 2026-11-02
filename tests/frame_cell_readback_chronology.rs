//! Gate F2 — a rendered-frame readback reports the depth the frame was EMITTED at, not a color
//! depth requested *after* painting.
//!
//! `Context::frame_cell` once quantized the last-painted surface with `session.color_depth()` — the
//! *currently requested* depth. So `set_color_depth(X)` with no re-render silently reinterpreted an
//! already-emitted frame as if it had been painted at X: the readback contradicted the ANSI wire
//! actually written (the doc on `FrameCell::style` promises `style` matches the emitted SGR). The
//! fix quantizes with the renderer's `last_emitted_depth` — the depth of the last *committed* frame,
//! which corresponds to the very surface `frame_cell` reads back — so an old frame keeps its emitted
//! style until a real repaint changes it.

use gibson::capability::ColorDepth;
use gibson::cell::{Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::node::{Node, WrapMode};

fn styled_root() -> Node {
    let style = Style::new()
        .fg(Color::Rgb(255, 128, 0))
        .bg(Color::Rgb(16, 32, 64))
        .bold();
    Node::col().child(Node::text_wrapped("Static", style, WrapMode::NoWrap))
}

fn readback_tracks_emitted_depth(mode: RenderMode) {
    let mut ctx = Context::headless(mode, 24, 4); // headless defaults to truecolor
    ctx.set_root(styled_root());
    ctx.render().unwrap();

    // The frame was emitted at truecolor, so the readback is truecolor.
    let fg0 = ctx.frame_cell(0, 0).expect("cell (0,0) resolves").style.fg;
    assert_eq!(
        fg0,
        Some(Color::Rgb(255, 128, 0)),
        "the emitted frame is truecolor; readback must report truecolor"
    );

    // Request Ansi16 but DO NOT repaint. The frame physically on screen is still the truecolor one.
    ctx.set_color_depth(ColorDepth::Ansi16);
    let fg1 = ctx.frame_cell(0, 0).expect("cell (0,0) resolves").style.fg;
    // RED before the fix: frame_cell quantized the OLD surface with the NEW (unemitted) Ansi16
    // depth, so fg1 came back a downgraded 16-color — the readback lied about what the wire carried.
    assert_eq!(
        fg1,
        Some(Color::Rgb(255, 128, 0)),
        "a depth request with no repaint must NOT reinterpret the already-emitted frame; got {fg1:?}"
    );

    // Now actually repaint: the depth change forces an in-place restyle (Gate F1/K1), the new frame
    // commits at Ansi16, and the readback follows the newly emitted depth.
    ctx.render().unwrap();
    let fg2 = ctx.frame_cell(0, 0).expect("cell (0,0) resolves").style.fg;
    assert!(
        !matches!(fg2, Some(Color::Rgb(_, _, _))),
        "after a real repaint at Ansi16 the readback must be downgraded, not truecolor; got {fg2:?}"
    );
    assert!(
        fg2.is_some(),
        "Ansi16 keeps a (downgraded) color on a bright fg, not None; got {fg2:?}"
    );
}

#[test]
fn inline_readback_tracks_emitted_depth_not_pending_request() {
    readback_tracks_emitted_depth(RenderMode::Inline);
}

#[test]
fn fullscreen_readback_tracks_emitted_depth_not_pending_request() {
    readback_tracks_emitted_depth(RenderMode::Fullscreen);
}
