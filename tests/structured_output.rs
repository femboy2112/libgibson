//! Structured static-output tests: `commit_text` / `commit_rich_text` /
//! `commit_node` must share the same width-aware layout semantics and must not
//! let untrusted control characters reach the terminal.

use gibson::cell::{Line, RichText, Span, Style, Theme};
use gibson::node::{Node, WrapMode};
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;

fn harness(cols: u16, rows: u16) -> (Renderer, TerminalSession) {
    (
        Renderer::new(gibson::RenderMode::Inline),
        TerminalSession::headless(cols, rows),
    )
}

fn screen_of(bytes: &[u8], cols: u16, rows: u16) -> String {
    let mut parser = vt100::Parser::new(rows, cols, 4000);
    parser.process(bytes);
    parser.screen().contents()
}

#[test]
fn commit_text_is_width_aware() {
    let (mut r, mut s) = harness(24, 12);
    let mut out = Vec::new();
    r.commit_text(
        "the quick brown fox jumps over the lazy dog and keeps going for a while",
        &mut s,
        &mut out,
    )
    .unwrap();
    let screen = screen_of(&out, 24, 12);
    assert!(screen.contains("quick"), "screen: {screen:?}");
    for line in screen.lines() {
        assert!(
            unicode_width::UnicodeWidthStr::width(line) <= 24,
            "line escaped width: {line:?}"
        );
    }
}

#[test]
fn commit_text_and_commit_node_wrap_identically() {
    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu";

    let (mut r1, mut s1) = harness(20, 12);
    let mut out1 = Vec::new();
    r1.commit_text(text, &mut s1, &mut out1).unwrap();

    let (mut r2, mut s2) = harness(20, 12);
    let mut node = Node::text_wrapped(text, Style::default(), WrapMode::WordWrap);
    node.layout_style.width = gibson::node::Dimension::Length(20.0);
    let mut out2 = Vec::new();
    r2.commit_node(&mut node, &mut s2, &mut out2).unwrap();

    assert_eq!(
        screen_of(&out1, 20, 12),
        screen_of(&out2, 20, 12),
        "structured commit_text and commit_node must share layout semantics"
    );
}

#[test]
fn commit_rich_text_preserves_styles_and_neutralizes_control_chars() {
    let (mut r, mut s) = harness(40, 8);
    let theme = Theme::default();
    let st = theme.styles();

    let rich = RichText::new().line(Line::new().span(Span::styled("safe ", st.text)).span(
        Span::styled("text \u{1b}[31mRED\u{1b}[0m \u{1b}]0;pwn\u{07}", st.text),
    ));
    let mut out = Vec::new();
    r.commit_rich_text(&rich, &mut s, &mut out).unwrap();
    let raw = String::from_utf8_lossy(&out);
    // The literal letters survive, but the injected sequences must not.
    assert!(raw.contains("RED"));
    assert!(!raw.contains("\u{1b}[31m"), "injected SGR leaked: {raw:?}");
    assert!(!raw.contains("\u{1b}]"), "injected OSC leaked: {raw:?}");
}

#[test]
fn commit_text_wraps_long_unicode_without_panic() {
    let (mut r, mut s) = harness(16, 10);
    let mut out = Vec::new();
    r.commit_text(
        "🦀 你好世界 a very long 🚀 line with combining e\u{0301} marks",
        &mut s,
        &mut out,
    )
    .unwrap();
    let screen = screen_of(&out, 16, 10);
    assert!(screen.contains("🦀"));
    for line in screen.lines() {
        assert!(unicode_width::UnicodeWidthStr::width(line) <= 16);
    }
}
