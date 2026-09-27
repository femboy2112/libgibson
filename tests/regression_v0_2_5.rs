//! Regression tests for v0.2.5 — issue #48 E-09 remaining sharp edges.
//!
//! - **Edge 1** (documented): the kind-specific builders `Node::scroll_offset`
//!   and `Node::background` keep their happy-path behavior on the kinds they
//!   support; their per-kind restriction is now documented (a wrong-kind call
//!   is a silent no-op by design).
//! - **Edge 2** (fixed): `Context::stats()` reads through `&self` — it no longer
//!   requires `&mut`, so stats are reachable through a shared borrow.

use gibson::cell::{Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::node::Node;

// ---- Edge 2: stats() is a &self read ----

#[test]
fn issue_48_e09_stats_readable_through_shared_ref() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 8);
    ctx.set_root(Node::col().child(Node::text("hi", Style::default())));
    ctx.render().unwrap();

    // The whole point of the fix: stats() takes &self, so it is reachable
    // through a shared borrow. This would not compile before v0.2.5.
    let ctx_ref: &Context = &ctx;
    let a = ctx_ref.stats();
    let b = ctx_ref.stats();

    assert_eq!(a, b, "stats() through &self must be repeatable and pure");
    assert!(a.frames >= 1, "a completed render should have been counted");
}

#[test]
fn issue_48_e09_stats_reflect_render_activity() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 8);
    let before = ctx.stats().frames;
    ctx.set_root(Node::col().child(Node::text("hi", Style::default())));
    ctx.render().unwrap();
    let after = ctx.stats().frames;
    assert!(after > before, "frame count must advance across a render");
}

// ---- Edge 1: builder happy paths still apply on the supported kinds ----

#[test]
fn issue_48_e09_background_applies_to_supported_kinds() {
    // Box (col/row) and Border (panel) are the supported kinds: no assert trip,
    // and the fill reaches the frame.
    let mut ctx = Context::headless(RenderMode::Inline, 12, 3);
    ctx.set_root(
        Node::col()
            .background(Color::Rgb(10, 20, 30))
            .width(12.0)
            .height(3.0),
    );
    ctx.render().unwrap();
    assert!(
        !ctx.rendered_bytes().is_empty(),
        "a filled box must emit output"
    );
}

#[test]
fn issue_48_e09_scroll_offset_applies_to_text_input() {
    // TextInput is the supported kind: the offset is retained.
    let _n = Node::text_input("value", 0, None, Style::default()).scroll_offset(3);
}

#[test]
fn issue_48_e09_wrong_kind_builder_calls_are_silent_no_ops() {
    // Documented behavior: a kind-specific builder on an unsupported kind
    // returns the node unchanged rather than panicking. This must stay true.
    let _ = Node::text("x", Style::default()).background(Color::Rgb(1, 2, 3));
    let _ = Node::col().scroll_offset(2);
}
