//! Headless output capture (issue #27) and the `gibson::context::RenderMode`
//! re-export (issue #28).
//!
//! A headless `Context` must render into an in-memory buffer the caller can read
//! back — the foundation for snapshot/assertion testing of layouts — while
//! touching no real file descriptor. The `RenderMode` import below deliberately
//! uses the `gibson::context` path (co-located with the methods that consume it)
//! to prove that path resolves.

use gibson::cell::Style;
use gibson::context::{Context, RenderMode};
use gibson::node::Node;

#[test]
fn headless_render_is_captured_not_written_to_stdout() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 8);

    // Nothing rendered yet: the capture buffer is empty.
    assert!(ctx.rendered_bytes().is_empty());

    ctx.set_root(Node::col().child(Node::text("HELLO", Style::default())));
    ctx.render().unwrap();

    let bytes = ctx.rendered_bytes();
    assert!(!bytes.is_empty(), "headless render captured no bytes");
    assert!(
        String::from_utf8_lossy(bytes).contains("HELLO"),
        "captured output is missing the rendered text"
    );
}

#[test]
fn take_output_returns_and_drains_the_buffer() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 8);
    ctx.set_root(Node::col().child(Node::text("DRAIN", Style::default())));
    ctx.render().unwrap();

    let out = ctx.take_output();
    assert!(out.contains("DRAIN"), "take_output missing text: {out:?}");

    // Draining leaves the buffer empty until the next render.
    assert!(ctx.take_output().is_empty());
    assert!(ctx.rendered_bytes().is_empty());
}

#[test]
fn render_mode_is_reexported_from_context_module() {
    // Compile-time proof of #28: the intuitive import path resolves.
    let mode: RenderMode = RenderMode::Inline;
    let _ctx = Context::headless(mode, 10, 4);
}
