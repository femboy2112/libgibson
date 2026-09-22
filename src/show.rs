//! Optional visual composition helpers.
//!
//! These are deliberately **not** a widget library. Each helper returns plain
//! [`Span`]s / [`Line`]s that you compose with the normal node tree, so they add
//! no new layout or painting semantics. They exist because the showcase demos
//! needed dope-looking gradients, meters and charts without hand-rolling escape
//! sequences, and they are broadly useful for any dashboard.

use crate::cell::{Color, Line, RichText, Span};

/// Partial-block glyphs for sub-cell precision, indexed by eighths.
const PARTIALS: [&str; 8] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];

/// Block glyphs for sparklines, indexed by intensity.
const SPARKS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// Splits `text` into one styled span per grapheme, colored by a linear
/// gradient from `from` to `to`.
pub fn gradient_spans(text: &str, from: Color, to: Color) -> Vec<Span> {
    use unicode_segmentation::UnicodeSegmentation;
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let n = graphemes.len().max(1);
    let plain = from == Color::Reset && to == Color::Reset;
    graphemes
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let t = if n <= 1 {
                0.0
            } else {
                i as f32 / (n - 1) as f32
            };
            let style = if plain {
                crate::cell::Style::default()
            } else {
                crate::cell::Style::new().fg(from.lerp(to, t))
            };
            Span::styled(*g, style)
        })
        .collect()
}

/// Builds a horizontal progress meter as spans.
///
/// * `fraction` is clamped to `[0, 1]`.
/// * The leading edge uses partial blocks for sub-cell smoothness.
/// * The filled region is colored with a `from → to` gradient; `track` colors
///   the remainder (pass [`Color::Reset`] for the terminal default).
pub fn progress_spans(
    fraction: f32,
    width: usize,
    from: Color,
    to: Color,
    track: Color,
) -> Vec<Span> {
    let width = width.max(1);
    let fraction = fraction.clamp(0.0, 1.0);
    let eighths = (fraction * (width as f32) * 8.0).round() as usize;
    let full = eighths / 8;
    let rem = eighths % 8;

    let plain = from == Color::Reset && to == Color::Reset && track == Color::Reset;
    let styled = |ch: &str, c: Color| {
        if plain {
            Span::raw(ch)
        } else {
            Span::styled(ch, crate::cell::Style::new().fg(c))
        }
    };
    let mut spans = Vec::with_capacity(width + 1);
    for x in 0..width {
        let t = x as f32 / (width.max(2) - 1) as f32;
        let color = from.lerp(to, t);
        if x < full {
            spans.push(styled("█", color));
        } else if x == full && rem > 0 {
            spans.push(styled(PARTIALS[rem], color));
        } else {
            spans.push(styled("─", track));
        }
    }
    spans
}

/// Resamples `values` to `width` columns and renders a sparkline using
/// `▁▂▃▄▅▆▇█`, colored with a `from → to` gradient.
pub fn sparkline_spans(values: &[f32], width: usize, from: Color, to: Color) -> Vec<Span> {
    let width = width.max(1);
    let max = values.iter().cloned().fold(0.0_f32, f32::max).max(1e-6);
    let plain = from == Color::Reset && to == Color::Reset;
    let mut spans = Vec::with_capacity(width);
    for x in 0..width {
        // Nearest-neighbour resample.
        let src = if values.is_empty() {
            0.0
        } else {
            values[(x * values.len()) / width]
        };
        let t = (src / max).clamp(0.0, 1.0);
        let idx = (t * 7.0).round() as usize;
        let style = if plain {
            crate::cell::Style::default()
        } else {
            crate::cell::Style::new().fg(from.lerp(to, x as f32 / (width.max(2) - 1) as f32))
        };
        spans.push(Span::styled(SPARKS[idx.min(7)], style));
    }
    spans
}

/// A single sparkline as a [`Line`].
pub fn sparkline(values: &[f32], width: usize, from: Color, to: Color) -> Line {
    Line::from_spans(sparkline_spans(values, width, from, to))
}

/// A single progress meter as a [`Line`].
pub fn progress(fraction: f32, width: usize, from: Color, to: Color, track: Color) -> Line {
    Line::from_spans(progress_spans(fraction, width, from, to, track))
}

/// A gradient text [`Line`].
pub fn gradient_line(text: &str, from: Color, to: Color) -> Line {
    Line::from_spans(gradient_spans(text, from, to))
}

/// Deterministic pseudo-random hex-dump row ("garbage.bin" style), shaded from
/// `from` (dim) to `to` (bright) left-to-right.
pub fn hex_dump_line(seed: u64, columns: usize, from: Color, to: Color) -> Line {
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let mut text = String::with_capacity(columns * 3);
    for i in 0..columns {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let byte = (state >> 33) as u8;
        text.push_str(&format!("{:02x}", byte));
        if i + 1 < columns {
            text.push(' ');
        }
    }
    Line::from_spans(gradient_spans(&text, from, to))
}

/// Convenience: build a [`RichText`] of `rows` gradient lines.
pub fn gradient_block(rows: &[&str], from: Color, to: Color) -> RichText {
    let mut rt = RichText::new();
    let n = rows.len().max(1);
    for (i, r) in rows.iter().enumerate() {
        let t = if n <= 1 {
            0.0
        } else {
            i as f32 / (n - 1) as f32
        };
        rt = rt.line(gradient_line(r, from.lerp(to, t), to));
    }
    rt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_spans_cover_all_graphemes() {
        let spans = gradient_spans("hello", Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255));
        assert_eq!(spans.len(), 5);
        assert_eq!(spans[0].text.as_str(), "h");
        assert_eq!(spans[4].text.as_str(), "o");
        // Endpoints hit the exact colors.
        assert_eq!(spans[0].style.fg, Some(Color::Rgb(0, 0, 0)));
        assert_eq!(spans[4].style.fg, Some(Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn progress_reaches_zero_and_full() {
        let empty = progress_spans(0.0, 10, Color::Green, Color::Green, Color::BrightBlack);
        assert_eq!(empty.len(), 10);
        assert!(empty.iter().all(|s| s.text.as_str() == "─"));

        let full = progress_spans(1.0, 10, Color::Green, Color::Green, Color::BrightBlack);
        assert!(full.iter().all(|s| s.text.as_str() == "█"));

        let half = progress_spans(0.5, 10, Color::Green, Color::Cyan, Color::BrightBlack);
        let filled = half.iter().filter(|s| s.text.as_str() == "█").count();
        assert_eq!(filled, 5);
    }

    #[test]
    fn sparkline_uses_block_glyphs() {
        let vals = [0.0, 0.5, 1.0, 0.25];
        let spans = sparkline_spans(&vals, 4, Color::Red, Color::Yellow);
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].text.as_str(), "▁");
        assert_eq!(spans[2].text.as_str(), "█");
    }

    #[test]
    fn hex_dump_is_deterministic() {
        let a = hex_dump_line(42, 4, Color::Black, Color::White);
        let b = hex_dump_line(42, 4, Color::Black, Color::White);
        assert_eq!(a.plain_text(), b.plain_text());
        assert_eq!(a.plain_text().split(' ').count(), 4);
    }

    #[test]
    fn gradient_block_shapes() {
        let block = gradient_block(&["a", "b", "c"], Color::Black, Color::White);
        assert_eq!(block.lines.len(), 3);
    }

    #[test]
    fn helpers_produce_zero_raw_ansi() {
        let line = gradient_line("LIBGIBSON", Color::Green, Color::Magenta);
        assert!(!line.plain_text().contains('\x1b'));
    }
}
