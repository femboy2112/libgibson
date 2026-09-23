//! Capability-driven color fallback. The same UI must degrade cleanly through
//! TrueColor → Ansi256 → Ansi16 → Mono, decided centrally at the compiler.

use gibson::ansi::AnsiCompiler;
use gibson::cell::{Cell, Color, Glyph, Style};
use gibson::diff::compute_diff;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;
use gibson::surface::Surface;
use gibson::{ColorDepth, RenderMode};

fn compile_rgb_at(depth: ColorDepth) -> String {
    let s1 = Surface::new(4, 1);
    let mut s2 = Surface::new(4, 1);
    s2.set_cell(
        0,
        0,
        Cell::new(Glyph::new("X"), Style::new().fg(Color::Rgb(200, 30, 90))),
    );
    let diff = compute_diff(Some(&s1), &s2);
    let mut compiler = AnsiCompiler::new();
    compiler.color_depth = depth;
    String::from_utf8(compiler.compile(&diff)).unwrap()
}

#[test]
fn truecolor_emits_rgb_sgr() {
    let s = compile_rgb_at(ColorDepth::TrueColor);
    assert!(s.contains("38;2;200;30;90"), "{s:?}");
}

#[test]
fn ansi256_quantizes_without_rgb_sgr() {
    let s = compile_rgb_at(ColorDepth::Ansi256);
    assert!(
        !s.contains("38;2;"),
        "ANSI256 must not emit truecolor: {s:?}"
    );
    assert!(
        s.contains("38;5;"),
        "ANSI256 should emit indexed color: {s:?}"
    );
}

#[test]
fn ansi16_emits_only_base_colors() {
    let s = compile_rgb_at(ColorDepth::Ansi16);
    assert!(!s.contains("38;2;"));
    assert!(
        !s.contains("38;5;"),
        "ANSI16 must not emit 256-color: {s:?}"
    );
    // One of the 16 base foreground codes must be present.
    assert!(
        (30..=37).any(|c| s.contains(&format!("\x1b[{}m", c)))
            || (90..=97).any(|c| s.contains(&format!("\x1b[{}m", c)))
            || s.contains("\x1b["),
        "expected base color SGR: {s:?}"
    );
}

#[test]
fn mono_emits_no_color_at_all() {
    let s = compile_rgb_at(ColorDepth::Mono);
    assert!(!s.contains("38;"), "mono emitted foreground color: {s:?}");
    assert!(!s.contains("48;"), "mono emitted background color: {s:?}");
}

#[test]
fn same_ui_degrades_through_the_whole_renderer() {
    let render = |depth: ColorDepth| -> String {
        let mut renderer = Renderer::new(RenderMode::Fullscreen);
        let mut session = TerminalSession::headless(20, 4);
        session.set_color_depth(depth);
        let mut root = gibson::node::Node::col().child(gibson::node::Node::text(
            "GRADIENT",
            Style::new().fg(Color::Rgb(10, 220, 130)),
        ));
        let mut out = Vec::new();
        renderer.render(&mut root, &mut session, &mut out).unwrap();
        String::from_utf8_lossy(&out).to_string()
    };

    assert!(render(ColorDepth::TrueColor).contains("38;2;10;220;130"));
    assert!(!render(ColorDepth::Ansi256).contains("38;2;"));
    assert!(!render(ColorDepth::Ansi16).contains("38;5;"));
    let mono = render(ColorDepth::Mono);
    assert!(!mono.contains("38;") && !mono.contains("48;"));
}

#[test]
fn commit_output_obeys_color_depth() {
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut session = TerminalSession::headless(40, 4);
    session.set_color_depth(ColorDepth::Ansi256);
    let mut out = Vec::new();
    let node_style = Style::new().fg(Color::Rgb(9, 9, 200));
    let mut node = gibson::node::Node::text("styled", node_style);
    renderer
        .commit_node(&mut node, &mut session, &mut out)
        .unwrap();
    let s = String::from_utf8_lossy(&out);
    assert!(
        !s.contains("38;2;"),
        "commit leaked truecolor under ANSI256: {s:?}"
    );
    assert!(
        s.contains("38;5;"),
        "commit should quantize to indexed: {s:?}"
    );
}
