//! Gate K · K1 — a mid-session color-depth change restyles the live frame.
//!
//! J1a's `gibson_set_color_depth` (and J1b's cell grid) carry a documented caveat: "set the color
//! depth before the first render," because the renderer diffs RAW cells and only quantizes to the
//! depth at byte-emission time. So a depth change AFTER a frame exists left every unchanged cell
//! painted with the OLD depth's SGR — the on-screen frame disagreed with the new depth until some
//! unrelated content change happened to touch each cell.
//!
//! This fences the fix: on the first frame whose depth differs from what was last emitted, the
//! renderer forces an in-place RESTYLE repaint (erase the region, re-emit every content cell at the
//! new depth) — WITHOUT reallocating Inline rows (which would ghost the old region below). No ABI
//! change; the seam is internal to the renderer. Headless, so the emitted wire is the observable.

use gibson::capability::ColorDepth;
use gibson::cell::{Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::node::{Node, WrapMode};

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.len() <= hay.len() && hay.windows(needle.len()).any(|w| w == needle)
}

fn styled_root() -> Node {
    let style = Style::new()
        .fg(Color::Rgb(255, 128, 0))
        .bg(Color::Rgb(16, 32, 64))
        .bold();
    Node::col().child(Node::text_wrapped("Static", style, WrapMode::NoWrap))
}

/// Inline: after a frame exists, changing the depth and re-rendering the SAME tree must re-emit the
/// content at the new depth — not leave the truecolor SGR stale on screen.
#[test]
fn inline_depth_change_restyles_in_place() {
    let mut ctx = Context::headless(RenderMode::Inline, 24, 4); // defaults to truecolor
    ctx.set_root(styled_root());
    ctx.render().unwrap();
    let first = ctx.rendered_bytes().to_vec();
    assert!(
        contains(&first, b"38;2;255;128;0"),
        "frame A must paint truecolor fg\n{}",
        String::from_utf8_lossy(&first)
    );
    let len_a = first.len();

    // Mid-session depth change, then re-render the identical tree.
    ctx.set_color_depth(ColorDepth::Ansi16);
    ctx.render().unwrap();
    let delta = &ctx.rendered_bytes()[len_a..];

    // RED without the fix: the raw diff is empty, so the depth change emits nothing and the content
    // is never restyled (delta would not carry "Static").
    assert!(
        contains(delta, b"Static"),
        "a mid-session depth change must re-emit the content; got {} delta bytes\n{}",
        delta.len(),
        String::from_utf8_lossy(delta)
    );
    // And the re-emitted cells must use Ansi16 SGR — no truecolor (`38;2;`/`48;2;`) survives.
    assert!(
        !contains(delta, b"38;2;") && !contains(delta, b"48;2;"),
        "restyle must drop truecolor SGR for Ansi16\n{}",
        String::from_utf8_lossy(delta)
    );
}

/// Fullscreen takes the clear-canvas restyle path (home + erase + re-emit at the new depth).
#[test]
fn fullscreen_depth_change_restyles() {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 24, 4);
    ctx.set_root(styled_root());
    ctx.render().unwrap();
    let len_a = ctx.rendered_bytes().len();

    ctx.set_color_depth(ColorDepth::Ansi16);
    ctx.render().unwrap();
    let delta = &ctx.rendered_bytes()[len_a..];

    assert!(
        contains(delta, b"Static"),
        "fullscreen depth change must re-emit the content\n{}",
        String::from_utf8_lossy(delta)
    );
    assert!(
        !contains(delta, b"38;2;") && !contains(delta, b"48;2;"),
        "fullscreen restyle must drop truecolor SGR\n{}",
        String::from_utf8_lossy(delta)
    );
}

/// Guard against over-triggering: re-rendering the SAME tree at the SAME depth is still a no-op
/// (the restyle fires only on an actual depth CHANGE, not on every frame).
#[test]
fn same_depth_rerender_is_still_a_noop() {
    let mut ctx = Context::headless(RenderMode::Inline, 24, 4);
    ctx.set_root(styled_root());
    ctx.render().unwrap();
    let len_a = ctx.rendered_bytes().len();

    // No depth change, identical tree.
    ctx.render().unwrap();
    let delta_len = ctx.rendered_bytes().len() - len_a;
    assert_eq!(
        delta_len, 0,
        "an unchanged re-render must stay a zero-byte no-op (restyle must not fire spuriously)"
    );
}
