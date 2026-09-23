//! Safe framebuffer glitch effects.
//!
//! Glitch happens on **cell content**, never on the terminal protocol: these
//! functions mutate a [`Surface`] only, so the ANSI compiler and
//! `TerminalTransaction` remain the sole owners of the wire. Wide glyphs are
//! re-sanitized after every mutation, so a glitch can never leave a dangling
//! continuation cell.

use crate::cell::{Cell, Glyph, Style};
use crate::particles::Rng;
use crate::surface::{Rect, Surface};

/// Repairs wide-glyph invariants in `rect`: a width-2 lead must be followed by a
/// continuation, and every continuation must follow a width-2 lead.
pub fn sanitize_wide(surface: &mut Surface, rect: Rect) {
    let inter = surface.area().intersection(&rect);
    if inter.is_empty() {
        return;
    }
    let x0 = inter.x;
    let x1 = inter.x + inter.width;
    for y in inter.y..(inter.y + inter.height) {
        for x in x0..x1 {
            let is_lead = surface
                .get(x, y)
                .map(|c| c.glyph.display_width == 2 && !c.is_continuation)
                .unwrap_or(false);
            let is_cont = surface
                .get(x, y)
                .map(|c| c.is_continuation)
                .unwrap_or(false);
            if is_lead {
                let next_ok = surface
                    .get(x + 1, y)
                    .map(|c| c.is_continuation)
                    .unwrap_or(false);
                if !next_ok {
                    if let Some(c) = surface.get_mut(x, y) {
                        *c = Cell::space(c.style);
                    }
                }
            } else if is_cont {
                let prev_ok = if x > 0 {
                    surface
                        .get(x - 1, y)
                        .map(|c| c.glyph.display_width == 2 && !c.is_continuation)
                        .unwrap_or(false)
                } else {
                    false
                };
                if !prev_ok {
                    if let Some(c) = surface.get_mut(x, y) {
                        *c = Cell::space(c.style);
                    }
                }
            }
        }
    }
}

/// Shifts rows horizontally by `amount` cells, filled from the opposite edge with
/// blanks. Whole cells move together, then wide glyphs are sanitized.
pub fn row_shift(surface: &mut Surface, rect: Rect, amount: i32, seed: u64) {
    let inter = surface.area().intersection(&rect);
    if inter.is_empty() || amount == 0 {
        return;
    }
    let mut rng = Rng::new(seed);
    for y in inter.y..(inter.y + inter.height) {
        let row: Vec<Cell> = (inter.x..(inter.x + inter.width))
            .map(|x| surface.get(x, y).cloned().unwrap_or_default())
            .collect();
        // Deterministic per-row jitter on top of the base shift.
        let jitter = if seed != 0 {
            (rng.next_f32() * 3.0).round() as i32 - 1
        } else {
            0
        };
        let shift = amount + jitter;
        // Scatter whole cells into a blank target row, then write back. This
        // keeps lead/continuation pairs adjacent (sanitized afterwards).
        let mut out = vec![Cell::default(); inter.width as usize];
        for (i, cell) in row.into_iter().enumerate() {
            let target = i as i32 + shift;
            if target >= 0 && target < inter.width as i32 {
                out[target as usize] = cell;
            }
        }
        for (i, cell) in out.into_iter().enumerate() {
            if let Some(c) = surface.get_mut(inter.x + i as u16, y) {
                *c = cell;
            }
        }
    }
    sanitize_wide(surface, inter);
}

/// Inverts the style of every opaque cell in `rect`.
pub fn invert_rect(surface: &mut Surface, rect: Rect) {
    let inter = surface.area().intersection(&rect);
    for y in inter.y..(inter.y + inter.height) {
        for x in inter.x..(inter.x + inter.width) {
            if let Some(c) = surface.get_mut(x, y) {
                if !c.transparent {
                    c.style = Style { ..c.style }.overlay(Style::new().reverse());
                }
            }
        }
    }
}

/// Seeded glyph substitution over narrow cells. Wide glyphs are left intact so
/// they cannot be half-replaced. Deterministic for a given `seed`.
pub fn scramble_rect(
    surface: &mut Surface,
    rect: Rect,
    alphabet: &[&str],
    density: f32,
    seed: u64,
    style: Style,
) {
    if alphabet.is_empty() {
        return;
    }
    let inter = surface.area().intersection(&rect);
    let mut rng = Rng::new(seed);
    let density = density.clamp(0.0, 1.0);
    for y in inter.y..(inter.y + inter.height) {
        for x in inter.x..(inter.x + inter.width) {
            let eligible = surface
                .get(x, y)
                .map(|c| !c.transparent && !c.is_continuation && c.glyph.display_width == 1)
                .unwrap_or(false);
            if !eligible {
                continue;
            }
            if rng.next_f32() < density {
                let g = alphabet[(rng.next_u64() as usize) % alphabet.len()];
                if let Some(c) = surface.get_mut(x, y) {
                    *c = Cell::new(Glyph::new(g), style);
                }
            }
        }
    }
    sanitize_wide(surface, inter);
}

/// Horizontal tearing: a few seeded rows jump sideways by small amounts.
pub fn tear(surface: &mut Surface, rect: Rect, seed: u64) {
    let inter = surface.area().intersection(&rect);
    if inter.is_empty() {
        return;
    }
    let mut rng = Rng::new(seed);
    for y in inter.y..(inter.y + inter.height) {
        if rng.next_f32() < 0.25 {
            let amount = (rng.next_f32() * 7.0).round() as i32 - 3;
            row_shift(surface, Rect::new(inter.x, y, inter.width, 1), amount, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Style;

    fn surface_with(text: &str, w: u16, h: u16) -> Surface {
        let mut s = Surface::new(w, h);
        s.print_str(0, 0, text, Style::default(), None);
        s
    }

    #[test]
    fn row_shift_moves_content_and_fills_with_spaces() {
        let mut s = surface_with("ABCDEF", 8, 1);
        row_shift(&mut s, Rect::new(0, 0, 8, 1), 2, 0);
        assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), " ");
        assert_eq!(s.get(2, 0).unwrap().glyph.grapheme.as_str(), "A");
        assert_eq!(s.get(7, 0).unwrap().glyph.grapheme.as_str(), "F");
    }

    #[test]
    fn scramble_is_deterministic_and_bounded() {
        let mut a = surface_with("HACK THE PLANET", 20, 1);
        let mut b = surface_with("HACK THE PLANET", 20, 1);
        scramble_rect(
            &mut a,
            Rect::new(0, 0, 20, 1),
            &["#", "?", "@"],
            1.0,
            5,
            Style::default(),
        );
        scramble_rect(
            &mut b,
            Rect::new(0, 0, 20, 1),
            &["#", "?", "@"],
            1.0,
            5,
            Style::default(),
        );
        assert_eq!(
            a.get(0, 0).unwrap().glyph.grapheme,
            b.get(0, 0).unwrap().glyph.grapheme
        );
    }

    #[test]
    fn glitch_never_leaves_dangling_wide_glyphs() {
        let mut s = Surface::new(10, 1);
        s.print_str(0, 0, "你好世界", Style::default(), None);
        // Aggressive tearing/shifting must not leave a lead without continuation.
        for seed in 0..20 {
            row_shift(&mut s, Rect::new(0, 0, 10, 1), 1, seed);
        }
        for x in 0..10u16 {
            let c = s.get(x, 0).unwrap();
            if c.is_continuation && x > 0 {
                let prev = s.get(x - 1, 0).unwrap();
                assert!(
                    prev.glyph.display_width == 2 && !prev.is_continuation,
                    "continuation at {x} without lead"
                );
            }
            if c.glyph.display_width == 2 && !c.is_continuation {
                let next = s.get(x + 1, 0).unwrap();
                assert!(next.is_continuation, "lead at {x} without continuation");
            }
        }
    }

    #[test]
    fn invert_only_changes_opaque_cells() {
        let mut s = Surface::new_transparent(4, 1);
        s.print_str(0, 0, "AB", Style::default(), None);
        invert_rect(&mut s, Rect::new(0, 0, 4, 1));
        assert!(s.get(0, 0).unwrap().style.reverse);
        assert!(!s.get(2, 0).unwrap().style.reverse); // transparent cell untouched
    }

    #[test]
    fn empty_rect_is_a_noop() {
        let mut s = surface_with("X", 4, 1);
        row_shift(&mut s, Rect::new(0, 0, 0, 0), 3, 1);
        invert_rect(&mut s, Rect::new(0, 0, 0, 0));
        tear(&mut s, Rect::new(0, 0, 0, 0), 1);
        assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "X");
    }
}
