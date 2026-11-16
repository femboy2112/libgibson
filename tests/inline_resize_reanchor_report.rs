//! Issue #403: the typed re-anchor report on `FrameReport`, through the public `Context` API.
//! (Kept apart from `inline_resize_reanchor.rs`, which uses only API that predates the fix so it
//! can be run red against the unfixed renderer.)

use gibson::cell::Style;
use gibson::context::{Context, RenderMode};
use gibson::node::Node;
use gibson::ReanchorReport;

fn rows(n: u16) -> Node {
    Node::col().children(
        (0..n)
            .map(|i| Node::text(format!("row-{i}"), Style::default()))
            .collect(),
    )
}

#[test]
fn the_reanchoring_frame_reports_its_rewind_and_geometry() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 15);
    ctx.commit_text("history").unwrap();
    ctx.set_root(rows(14));
    ctx.render().unwrap();
    assert_eq!(
        ctx.last_frame_report().reanchor,
        None,
        "a first frame is not a re-anchor"
    );

    ctx.resize(80, 24).unwrap();
    ctx.set_root(rows(23));
    ctx.render().unwrap();
    let r: ReanchorReport = ctx
        .last_frame_report()
        .reanchor
        .expect("the frame after a resize re-anchors");
    assert_eq!(r.from, (40, 15));
    assert_eq!(r.to, (80, 24));
    assert_eq!(r.previous_live_height, 14);
    // The cursor parked on the old region's bottom row (no widget cursor).
    assert_eq!(r.rewound_rows, 13);

    // The next frame at the same geometry is an ordinary one.
    ctx.set_root(rows(23));
    ctx.render().unwrap();
    assert_eq!(ctx.last_frame_report().reanchor, None);
}

#[test]
fn a_commit_after_the_resize_settles_the_reanchor() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 15);
    ctx.set_root(rows(5));
    ctx.render().unwrap();
    ctx.resize(30, 10).unwrap();
    ctx.commit_text("after").unwrap();
    ctx.set_root(rows(5));
    ctx.render().unwrap();
    assert_eq!(
        ctx.last_frame_report().reanchor,
        None,
        "the commit already erased the old region; the frame only allocates a fresh one"
    );
}

#[test]
fn fullscreen_reports_the_reanchor_without_a_rewind() {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 40, 15);
    ctx.set_root(rows(5));
    ctx.render().unwrap();
    ctx.resize(50, 20).unwrap();
    ctx.render().unwrap();
    let r = ctx
        .last_frame_report()
        .reanchor
        .expect("fullscreen re-anchors too");
    assert_eq!((r.from, r.to, r.rewound_rows), ((40, 15), (50, 20), 0));
}
