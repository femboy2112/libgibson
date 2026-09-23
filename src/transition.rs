//! Deterministic, grapheme-safe text transitions.
//!
//! Every transition is a pure function `(text, t, seed) -> styled text`. All
//! operations work on **grapheme clusters**, so emoji, CJK, combining marks and
//! flags are never split. At `t >= 1.0` the result is exactly the input text.

use crate::cell::{Line, Style};
use unicode_segmentation::UnicodeSegmentation;

/// Number of graphemes revealed by a type-on transition at time `t`.
pub fn revealed_count(text: &str, t: f32, chars_per_second: f32) -> usize {
    let total = text.graphemes(true).count();
    let n = (t.max(0.0) * chars_per_second.max(0.0)).floor() as usize;
    n.min(total)
}

/// Type-on: reveals graphemes left to right.
pub fn type_on(text: &str, t: f32, chars_per_second: f32) -> String {
    let n = revealed_count(text, t, chars_per_second);
    text.graphemes(true).take(n).collect()
}

/// A small deterministic glyph alphabet for scramble/glitch effects.
pub const SCRAMBLE_GLYPHS: &[&str] = &[
    "#", "%", "&", "*", "+", "=", "?", "@", "$", "/", "\\", "<", ">", "~", "^",
];

/// Scramble-to-final: positions resolve left-to-right; unresolved positions show
/// seeded placeholder glyphs that change over time. At `t >= 1.0` the exact input
/// is returned.
pub fn scramble(text: &str, t: f32, seed: u64, alphabet: &[&str]) -> String {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let total = graphemes.len();
    if total == 0 {
        return String::new();
    }
    if t >= 1.0 {
        return text.to_string();
    }

    let resolved = ((t.max(0.0) * total as f32 * 1.6).floor() as usize).min(total);
    // Time bucket so placeholders churn a few times per second.
    let bucket = (t.max(0.0) * 12.0).floor() as u64;

    let mut out = String::with_capacity(text.len());
    for (i, g) in graphemes.iter().enumerate() {
        if i < resolved || alphabet.is_empty() {
            out.push_str(g);
        } else {
            let mut h = seed
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(i as u64)
                .wrapping_add(bucket.wrapping_mul(0xBF58_476D_1CE4_E5B9));
            h ^= h >> 29;
            out.push_str(alphabet[(h as usize) % alphabet.len()]);
        }
    }
    out
}

/// Scramble-to-final producing a styled [`Line`].
pub fn scramble_line(text: &str, t: f32, seed: u64, alphabet: &[&str], style: Style) -> Line {
    Line::styled(scramble(text, t, seed, alphabet), style)
}

/// A dissolve: graphemes disappear left-to-right (the inverse of type-on).
pub fn dissolve(text: &str, t: f32) -> String {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let total = graphemes.len();
    if total == 0 {
        return String::new();
    }
    let gone = ((t.clamp(0.0, 1.0)) * total as f32).floor() as usize;
    let keep_from = gone.min(total);
    graphemes[keep_from..].concat()
}

/// A wipe mask: returns `true` when the cell at `index` should be shown.
pub fn wipe_visible(index: usize, total: usize, t: f32) -> bool {
    if total == 0 {
        return true;
    }
    let revealed = ((t.clamp(0.0, 1.0)) * total as f32).ceil() as usize;
    index < revealed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_on_ends_exact() {
        let s = "héllo 你好 🦀";
        assert_eq!(type_on(s, 1.0, 100.0), s);
        assert_eq!(type_on(s, 0.0, 100.0), "");
    }

    #[test]
    fn type_on_never_splits_graphemes() {
        let s = "🇺🇸👨‍👩‍👧 e\u{0301} 你好";
        let clusters: Vec<&str> = s.graphemes(true).collect();
        let full = clusters.len();
        for k in 0..=full {
            // Drive `revealed_count` to exactly k for two different rates so the
            // mapping is exercised, not just `take(n)`. A huge rate (the old
            // test used 1e9) would reveal the entire string on the first step.
            for (t, cps) in [(k as f32, 1.0_f32), (k as f32 / 10.0, 10.0)] {
                let out = type_on(s, t, cps);
                let expected: String = clusters[..k].concat();
                assert_eq!(
                    out, expected,
                    "type_on split a cluster at k={k} for {s:?} (t={t}, cps={cps})"
                );
                assert!(s.starts_with(&out));
            }
        }
    }

    #[test]
    fn type_on_covers_hostile_cluster_boundaries() {
        // Combining marks, a ZWJ family, flags, CJK and an emoji modifier.
        let cases = [
            "e\u{0301}\u{0327}",
            "a\u{0308}\u{0301}xyz",
            "👨\u{200D}👩\u{200D}👧\u{200D}👦",
            "🇺🇸🇯🇵",
            "你好世界",
            "👍🏽🦀",
        ];
        for s in cases {
            let clusters: Vec<&str> = s.graphemes(true).collect();
            for k in 0..=clusters.len() {
                let out = type_on(s, k as f32, 1.0);
                assert_eq!(out, clusters[..k].concat(), "k={k} for {s:?}");
            }
        }
    }

    #[test]
    fn revealed_count_is_monotonic_and_capped() {
        let s = "👨\u{200D}👩\u{200D}👧\u{200D}👦🇺🇸你好";
        let total = s.graphemes(true).count();
        let mut last = 0;
        for step in 0..=40 {
            let n = revealed_count(s, step as f32 * 0.1, 7.0);
            assert!(n >= last, "revealed_count must be monotonic");
            assert!(n <= total, "revealed_count must not exceed the buffer");
            last = n;
        }
        assert_eq!(revealed_count(s, 100.0, 7.0), total);
    }

    #[test]
    fn scramble_ends_exact_and_preserves_grapheme_count() {
        let s = "HACK THE PLANET 🦀 你好";
        assert_eq!(scramble(s, 1.0, 7, SCRAMBLE_GLYPHS), s);
        let mid = scramble(s, 0.3, 7, SCRAMBLE_GLYPHS);
        assert_eq!(
            mid.graphemes(true).count(),
            s.graphemes(true).count(),
            "scramble must not split or merge graphemes"
        );
    }

    #[test]
    fn scramble_is_deterministic() {
        let a = scramble("GIBSON", 0.42, 99, SCRAMBLE_GLYPHS);
        let b = scramble("GIBSON", 0.42, 99, SCRAMBLE_GLYPHS);
        assert_eq!(a, b);
        let c = scramble("GIBSON", 0.42, 100, SCRAMBLE_GLYPHS);
        assert_ne!(a, c);
    }

    #[test]
    fn dissolve_is_the_inverse_of_type_on() {
        let s = "abcdef";
        assert_eq!(dissolve(s, 0.0), s);
        assert_eq!(dissolve(s, 1.0), "");
        assert_eq!(dissolve(s, 0.5), "def");
    }

    #[test]
    fn empty_input_is_safe() {
        assert_eq!(scramble("", 0.5, 1, SCRAMBLE_GLYPHS), "");
        assert_eq!(dissolve("", 0.5), "");
        assert_eq!(type_on("", 0.5, 10.0), "");
    }
}
