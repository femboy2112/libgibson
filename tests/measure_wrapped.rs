//! v0.9.0 `measure_wrapped`: RED slice (design in docs/design/v0.9-measure-wrapped.md).
//!
//! The helper must report the rows `ui::text()` paints. The oracle test paints a wrapped text
//! node through layout and counts the painted rows; the pinned cases are hand-computed against
//! the greedy word wrap.

use gibson::*;

const CAPTION: &str = "alpha bravo charlie delta echo foxtrot golf hotel ab";

fn painted_rows(text: &str, width: u16) -> u16 {
    let mut root =
        Node::text_wrapped(text, Style::new(), WrapMode::WordWrap).width(f32::from(width));
    compute_layout(&mut root, width, 200).unwrap();
    let mut surface = Surface::new(width, 200);
    paint(&root, &mut surface);
    let lines = surface.to_visible_lines();
    lines
        .iter()
        .rposition(|l| !l.is_empty())
        .map_or(0, |i| (i + 1) as u16)
}

#[test]
fn caption_fixture_is_52_cells() {
    assert_eq!(CAPTION.chars().count(), 52);
}

#[test]
fn red_caption_at_52_is_one_row() {
    assert_eq!(measure_wrapped(CAPTION, 52), 1);
}

#[test]
fn red_caption_at_40_is_two_rows() {
    assert_eq!(measure_wrapped(CAPTION, 40), 2);
}

#[test]
fn red_caption_at_26_is_two_rows() {
    assert_eq!(measure_wrapped(CAPTION, 26), 2);
}

#[test]
fn red_caption_at_10_is_eight_rows() {
    assert_eq!(measure_wrapped(CAPTION, 10), 8);
}

#[test]
fn red_paragraphs_count_blank_lines() {
    let t = "one two three\n\nfour five";
    assert_eq!(measure_wrapped(t, 8), 5);
    assert_eq!(measure_wrapped(t, 20), 3);
}

#[test]
fn red_long_word_is_hard_broken() {
    assert_eq!(measure_wrapped("abcdefghij", 4), 3);
    assert_eq!(measure_wrapped("ab abcdefghij", 4), 4);
}

#[test]
fn red_wide_glyphs_are_never_split() {
    let t = "日本語日本語";
    assert_eq!(measure_wrapped(t, 5), 3);
    assert_eq!(measure_wrapped(t, 12), 1);
    // A 2-cell glyph in a 1-cell column overflows rather than splitting: one glyph per row.
    assert_eq!(measure_wrapped(t, 1), 6);
}

#[test]
fn red_trailing_newline_adds_a_blank_row() {
    assert_eq!(measure_wrapped("a\n", 10), 2);
    assert_eq!(measure_wrapped("\n", 10), 2);
}

#[test]
fn red_empty_text_is_one_row() {
    assert_eq!(measure_wrapped("", 10), 1);
}

#[test]
fn red_width_zero_follows_wrap_text() {
    assert_eq!(measure_wrapped("abc", 0), 3);
    assert_eq!(measure_wrapped("", 0), 1);
}

#[test]
fn red_saturates_at_u16_max() {
    let t = "\n".repeat(70_000);
    assert_eq!(measure_wrapped(&t, 10), u16::MAX);
}

const ORACLE_TEXTS: [&str; 7] = [
    CAPTION,
    "one two three\n\nfour five",
    "abcdefghij",
    "ab abcdefghij klm",
    "a b c d e f g h i j k l m n o p",
    "short\nlonger line here\nx",
    "  leading and  double  spaces end",
];

const ORACLE_WIDE_TEXTS: [&str; 2] = ["日本語日本語", "漢字 and ascii 混ぜる text"];

#[test]
fn red_oracle_matches_painted_rows() {
    for t in ORACLE_TEXTS {
        for w in 1..=60u16 {
            assert_eq!(
                measure_wrapped(t, w),
                painted_rows(t, w),
                "text {t:?} width {w}"
            );
        }
    }
    for t in ORACLE_WIDE_TEXTS {
        for w in 2..=60u16 {
            assert_eq!(
                measure_wrapped(t, w),
                painted_rows(t, w),
                "text {t:?} width {w}"
            );
        }
    }
}

#[test]
fn red_equals_wrap_text_row_count() {
    for t in ORACLE_TEXTS.iter().chain(ORACLE_WIDE_TEXTS.iter()) {
        for w in 0..=60u16 {
            let want = wrap_text(t, WrapMode::WordWrap, w).len() as u16;
            assert_eq!(measure_wrapped(t, w), want, "text {t:?} width {w}");
        }
    }
}

#[test]
fn guard_wrap_text_is_unchanged() {
    assert_eq!(
        wrap_text("alpha bravo charlie", WrapMode::WordWrap, 10),
        ["alpha", "bravo", "charlie"]
    );
    assert_eq!(wrap_text("", WrapMode::WordWrap, 5), [""]);
    assert_eq!(wrap_text("a\n", WrapMode::WordWrap, 5), ["a", ""]);
    assert_eq!(wrap_text("abcdef", WrapMode::CharWrap, 4), ["abcd", "ef"]);
}
