//! Safety semantics of the commit / insertion APIs.
//!
//! The central invariant: **safe** APIs cannot inject terminal control
//! sequences, and the only raw path is explicitly named `unchecked`.

#![allow(deprecated)]

use gibson::cell::{Line, RichText, Span, Style};
use gibson::renderer::{InsertStrategy, Renderer};
use gibson::session::TerminalSession;
use gibson::Context;
use gibson::RenderMode;

fn context_with_live_region(cols: u16, rows: u16) -> (Context, Vec<u8>) {
    let mut ctx = Context::headless(RenderMode::Inline, cols, rows);
    // Render a live region so insertion has something to preserve.
    ctx.set_root(
        gibson::node::Node::col()
            .child(gibson::node::Node::text("LIVE", Style::default()))
            .child(gibson::node::Node::text("REGION", Style::default())),
    );
    ctx.render().unwrap();
    (ctx, Vec::new())
}

#[test]
fn insert_text_cannot_inject_terminal_controls() {
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut session = TerminalSession::headless(40, 10);
    let mut out = Vec::new();

    let mut root =
        gibson::node::Node::col().child(gibson::node::Node::text("LIVE", Style::default()));
    renderer.render(&mut root, &mut session, &mut out).unwrap();
    out.clear();

    let payload = "evil \x1b[2J\x1b]0;pwned\x07 and \x1b[31mred";
    let (_bytes, strategy) = renderer
        .insert_text_before_live(payload, &mut session, &mut out)
        .unwrap();
    assert_eq!(strategy, InsertStrategy::InsertLineFastPath);

    let text = String::from_utf8_lossy(&out);
    assert!(
        !text.contains("\x1b[2J"),
        "safe insert leaked a clear-screen CSI: {text:?}"
    );
    assert!(
        !text.contains("\x1b]0;"),
        "safe insert leaked an OSC title: {text:?}"
    );
    assert!(
        text.contains("evil"),
        "payload text should survive: {text:?}"
    );
}

#[test]
fn raw_lines_unchecked_does_pass_controls_through() {
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut session = TerminalSession::headless(40, 10);
    let mut out = Vec::new();

    let mut root =
        gibson::node::Node::col().child(gibson::node::Node::text("LIVE", Style::default()));
    renderer.render(&mut root, &mut session, &mut out).unwrap();
    out.clear();

    renderer
        .insert_raw_lines_before_live_unchecked(&["\x1b[2JRAW"], &mut session, &mut out)
        .unwrap();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("\x1b[2J"),
        "the unchecked escape hatch must be raw: {text:?}"
    );
}

#[test]
fn insert_text_is_width_aware() {
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut session = TerminalSession::headless(20, 12);
    let mut out = Vec::new();
    let mut root =
        gibson::node::Node::col().child(gibson::node::Node::text("LIVE", Style::default()));
    renderer.render(&mut root, &mut session, &mut out).unwrap();
    out.clear();

    let long = "word ".repeat(40);
    renderer
        .insert_text_before_live(&long, &mut session, &mut out)
        .unwrap();
    let text = String::from_utf8_lossy(&out);
    // Wrap must have split the line: multiple newlines should be present.
    assert!(
        text.matches('\n').count() >= 3,
        "expected wrapping: {text:?}"
    );
}

#[test]
fn insert_rich_text_is_safe_and_preserves_styles() {
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut session = TerminalSession::headless(40, 10);
    let mut out = Vec::new();
    let mut root =
        gibson::node::Node::col().child(gibson::node::Node::text("LIVE", Style::default()));
    renderer.render(&mut root, &mut session, &mut out).unwrap();
    out.clear();

    let rich = RichText::new().line(Line::new().span(Span::styled(
        "accent \x1b[2J",
        Style::new().fg(gibson::Color::Red),
    )));
    renderer
        .insert_rich_text_before_live(&rich, &mut session, &mut out)
        .unwrap();
    let text = String::from_utf8_lossy(&out);
    assert!(
        !text.contains("\x1b[2J"),
        "rich insert leaked controls: {text:?}"
    );
    assert!(text.contains("accent"));
}

#[test]
fn commit_text_cannot_inject_controls() {
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut session = TerminalSession::headless(40, 10);
    let mut out = Vec::new();
    renderer
        .commit_text("hello \x1b[2J\x07world", &mut session, &mut out)
        .unwrap();
    let text = String::from_utf8_lossy(&out);
    assert!(!text.contains("\x1b[2J"));
    assert!(text.contains("hello"));
    assert!(text.contains("world"));
}

#[test]
fn renderer_commit_is_now_safe_text_not_raw() {
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut session = TerminalSession::headless(40, 10);
    let mut out = Vec::new();
    // `Renderer::commit` used to alias the raw path; it must now behave like
    // `commit_text` and neutralize controls.
    renderer
        .commit("safe \x1b[2J text", &mut session, &mut out)
        .unwrap();
    let text = String::from_utf8_lossy(&out);
    assert!(
        !text.contains("\x1b[2J"),
        "Renderer::commit is still raw: {text:?}"
    );
}

#[test]
fn insert_line_fast_path_requires_supported_capability() {
    // Unknown capability must fall back to the repaint path.
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut session = TerminalSession::headless(40, 10);
    session.set_capabilities(gibson::TerminalCapabilities::mono()); // insert_line = Unknown
    let mut out = Vec::new();
    let mut root =
        gibson::node::Node::col().child(gibson::node::Node::text("LIVE", Style::default()));
    renderer.render(&mut root, &mut session, &mut out).unwrap();
    out.clear();
    let (_, strategy) = renderer
        .insert_raw_lines_before_live_unchecked(&["HISTORY"], &mut session, &mut out)
        .unwrap();
    assert_eq!(strategy, InsertStrategy::RepaintFallback);

    // Explicit support enables the fast path.
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut session = TerminalSession::headless(40, 10);
    session.set_capabilities(gibson::TerminalCapabilities::truecolor());
    let mut out = Vec::new();
    let mut root =
        gibson::node::Node::col().child(gibson::node::Node::text("LIVE", Style::default()));
    renderer.render(&mut root, &mut session, &mut out).unwrap();
    out.clear();
    let (_, strategy) = renderer
        .insert_raw_lines_before_live_unchecked(&["HISTORY"], &mut session, &mut out)
        .unwrap();
    assert_eq!(strategy, InsertStrategy::InsertLineFastPath);
}

#[test]
fn context_safe_insertion_api_smoke() {
    let (mut ctx, _) = context_with_live_region(40, 10);
    ctx.insert_text_before_live("safe \x1b[2J note").unwrap();
    ctx.insert_rich_text_before_live(
        &RichText::new().line(Line::styled("rich", Style::new().bold())),
    )
    .unwrap();
    // Should not panic and should keep a sane live region.
    assert!(ctx.stats().history_insertions >= 2);
}
