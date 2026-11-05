//! RXIV Gate F — terminal-glyph-safe clipping.
//!
//! `layout::clip` budgets by display columns (RXIII), but iterated Unicode SCALARS: a multi-codepoint
//! grapheme (a ZWJ family emoji, a flag, a base + several combining marks, an Indic conjunct) could
//! be cut mid-cluster — leaving a dangling ZWJ or a lone regional indicator on screen. Clipping must
//! stop on grapheme-cluster boundaries. Pinned two ways: on the string (the kept text is a
//! grapheme-boundary prefix within budget), and through the REAL renderer at monochrome and
//! truecolour depth via cell readback (every painted cell holds a whole source grapheme or the
//! ellipsis — the colour capability never changes which glyphs are placed).

#[path = "../examples/wire_instrument/layout.rs"]
mod layout;

use gibson::capability::ColorDepth;
use gibson::context::Context;
use gibson::renderer::RenderMode;
use gibson::{Node, Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";

fn cases() -> Vec<String> {
    vec![
        format!("ab{FAMILY}cd"),
        format!("{FAMILY}{FAMILY}{FAMILY}"),
        "e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}".to_string(),
        "a\u{308}\u{301}\u{323}b\u{308}\u{301}c".to_string(),
        "漢字テスト漢字".to_string(),
        "\u{1F1EF}\u{1F1F5}\u{1F1EB}\u{1F1F7}\u{1F1E9}\u{1F1EA}".to_string(),
        "क्षि क्षि क्षि".to_string(),
        "status: ack ✓ — résumé 漢 x".to_string(),
    ]
}

fn assert_grapheme_safe(s: &str, w: u16) {
    let out = layout::clip(s, w);
    let body = out.strip_suffix('…').unwrap_or(&out);
    assert!(
        s.starts_with(body),
        "{s:?}@{w}: clip must keep a prefix, got {out:?}"
    );
    let bounds: Vec<usize> = s
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([s.len()])
        .collect();
    assert!(
        bounds.contains(&body.len()),
        "{s:?}@{w}: cut inside a grapheme cluster -> {out:?}"
    );
    assert!(
        out.width() <= w as usize,
        "{s:?}@{w}: {out:?} is {} columns",
        out.width()
    );
}

#[test]
fn clipping_stops_on_grapheme_boundaries_at_every_width() {
    for s in cases() {
        for w in 0..=16u16 {
            assert_grapheme_safe(&s, w);
        }
    }
}

#[test]
fn the_renderer_places_only_whole_graphemes_at_either_colour_depth() {
    for s in cases() {
        let graphemes: Vec<&str> = s.graphemes(true).collect();
        for w in [3u16, 5, 7, 9] {
            let clipped = layout::clip(&s, w);
            let mut placed: Vec<Vec<String>> = Vec::new();
            for depth in [ColorDepth::Mono, ColorDepth::TrueColor] {
                let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 1);
                ctx.set_color_depth(depth);
                ctx.set_root(Node::text(clipped.clone(), Style::new().bold()));
                ctx.render().expect("headless render");
                let mut cells = Vec::new();
                for x in 0..20 {
                    let c = ctx.frame_cell(x, 0).expect("in-bounds cell");
                    if !c.glyph.is_empty() && c.glyph != " " {
                        assert!(
                            graphemes.contains(&c.glyph) || c.glyph == "…",
                            "{s:?}@{w} {depth:?}: painted a partial cluster {:?}",
                            c.glyph
                        );
                        cells.push(c.glyph.to_string());
                    }
                }
                placed.push(cells);
            }
            assert_eq!(
                placed[0], placed[1],
                "{s:?}@{w}: the colour capability never changes which glyphs are placed"
            );
        }
    }
}
