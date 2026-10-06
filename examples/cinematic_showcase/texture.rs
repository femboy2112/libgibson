//! Surface → RgbRaster texture adapter for the showcase's flying media planes.
//!
//! The experience grammars present to a terminal [`Surface`] — a grid of cells,
//! each a glyph plus foreground/background colours. A perspective plane flown
//! through 3-D space needs *pixels*, so this is the honest bridge: each cell
//! becomes a small pixel block whose body is its background colour and whose
//! inked glyph is stamped as a brighter foreground mark. The result reads as a
//! living UI panel seen *through* the glyph grid — the "(Qt + OpenGL) through a
//! screen door" ethos made literal and, now, perspective-mapped. Nothing here
//! fakes a screenshot; it rasterises exactly the cells the grammar produced.
//!
//! Demo-local art direction. The only library surface it touches is the public
//! `Surface`/`Cell`/`Color` read API and `RgbRaster`.

#![allow(dead_code)]

use gibson::cell::{Cell, Color};
use gibson::raster::{Rgb, RgbRaster};
use gibson::surface::Surface;
use gibson::Rect;

/// Uniformly scale a master `Surface` into a destination rectangle (nearest-cell
/// sampling) and blit it onto `dst`. Because the master is rendered once at a fixed
/// size and only *scaled* here, its whole layout grows as one image — no element
/// reflows or "spawns in" at its own rate as the rectangle changes size. At a
/// destination the master's own size it is a 1:1 crisp copy; smaller, it is a
/// uniformly blockier version of the same frame (the "blurry" far end of a zoom).
pub fn blit_surface_scaled(dst: &mut Surface, master: &Surface, rect: Rect) {
    if rect.width == 0 || rect.height == 0 || master.width == 0 || master.height == 0 {
        return;
    }
    for dy in 0..rect.height {
        let my = ((dy as u32 * master.height as u32) / rect.height as u32) as u16;
        let my = my.min(master.height - 1);
        for dx in 0..rect.width {
            let mx = ((dx as u32 * master.width as u32) / rect.width as u32) as u16;
            let mx = mx.min(master.width - 1);
            if let Some(cell) = master.get(mx, my) {
                dst.set_cell(rect.x + dx, rect.y + dy, cell.clone());
            }
        }
    }
}

/// Pixels per cell edge when a `Surface` is rasterised to a texture. A cell
/// becomes `CELL_PX × CELL_PX` pixels: enough to carry a legible ink stamp
/// without an enormous texture (a 72×36 grammar → 432×216 texels at 6).
pub const CELL_PX: u16 = 6;

/// The background colour a cell contributes. A transparent cell shows whatever
/// is beneath it; in a free-standing texture there is nothing beneath, so it
/// contributes the caller's `fill`.
fn cell_bg(cell: &Cell, fill: Rgb) -> Rgb {
    if cell.transparent {
        return fill;
    }
    cell.style.bg.map(Color::to_rgb).unwrap_or(fill)
}

/// Whether a cell carries a visible (non-blank, non-transparent) glyph.
fn cell_inked(cell: &Cell) -> bool {
    !cell.transparent && !cell.glyph.is_empty() && !cell.glyph.grapheme.trim().is_empty()
}

/// The foreground colour an inked glyph is stamped in.
fn cell_fg(cell: &Cell) -> Rgb {
    cell.style.fg.map(Color::to_rgb).unwrap_or((230, 230, 230))
}

/// Rasterise a terminal `Surface` into an `RgbRaster` texture.
///
/// Each cell fills a `CELL_PX²` block with its background colour; an inked glyph
/// lifts the block's centre toward the glyph's foreground colour (a 1-px margin
/// is left so the "grid" still shows through). At plane distance this reads as
/// lit text mass on a UI panel — in-ethos, not a faked letterform. The texture
/// is at least 1×1 and never panics for any surface size.
pub fn surface_to_raster(surf: &Surface, fill: Rgb) -> RgbRaster {
    let pw = ((surf.width as u32) * (CELL_PX as u32)).clamp(1, u16::MAX as u32) as u16;
    let ph = ((surf.height as u32) * (CELL_PX as u32)).clamp(1, u16::MAX as u32) as u16;
    let mut tex = RgbRaster::new(pw, ph);
    tex.clear(fill);
    let step = CELL_PX as i32;
    for cy in 0..surf.height {
        for cx in 0..surf.width {
            let Some(cell) = surf.get(cx, cy) else {
                continue;
            };
            let px0 = cx as i32 * step;
            let py0 = cy as i32 * step;
            let bg = cell_bg(cell, fill);
            for dy in 0..step {
                for dx in 0..step {
                    tex.set(px0 + dx, py0 + dy, bg);
                }
            }
            if cell_inked(cell) {
                let fg = cell_fg(cell);
                for dy in 1..step - 1 {
                    for dx in 1..step - 1 {
                        tex.blend(px0 + dx, py0 + dy, fg, 0.82);
                    }
                }
            }
        }
    }
    tex
}

/// A dimmed, far-fading copy of a texture for a plane's reflection. The
/// reflection's *geometry* is mirrored (so the same texture samples upside-down
/// below the plane); this only darkens it and fades it out toward the plane's
/// far (lower) edge. Texture row `v = 0` is the plane's top, which — once the
/// geometry is flipped — lands at the reflection's far end, so the fade
/// *brightens* with `v`, matching the `album_flow` reflection law.
pub fn reflection_of(tex: &RgbRaster) -> RgbRaster {
    let (w, h) = (tex.width(), tex.height());
    let mut r = RgbRaster::new(w.max(1), h.max(1));
    let denom = (h.max(2) - 1) as f32;
    for y in 0..h {
        let v = y as f32 / denom;
        let f = 0.08 + 0.30 * v;
        for x in 0..w {
            let c = tex.get(x as i32, y as i32).unwrap_or((0, 0, 0));
            r.set(
                x as i32,
                y as i32,
                (
                    (c.0 as f32 * f) as u8,
                    (c.1 as f32 * f) as u8,
                    (c.2 as f32 * f) as u8,
                ),
            );
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use gibson::cell::{Cell, Color, Glyph, Style};
    use gibson::surface::Surface;

    fn inked_cell(fg: Color, bg: Color) -> Cell {
        Cell::new(Glyph::new("X"), Style::new().fg(fg).bg(bg))
    }

    #[test]
    fn texture_is_cell_px_scaled_and_bounded() {
        let mut s = Surface::new(4, 3);
        s.set_cell(
            1,
            1,
            inked_cell(Color::Rgb(255, 255, 255), Color::Rgb(10, 20, 30)),
        );
        let tex = surface_to_raster(&s, (0, 0, 0));
        assert_eq!(tex.width(), 4 * CELL_PX);
        assert_eq!(tex.height(), 3 * CELL_PX);
    }

    #[test]
    fn background_colour_reaches_the_block() {
        let mut s = Surface::new(2, 2);
        s.set_cell(0, 0, Cell::space(Style::new().bg(Color::Rgb(7, 8, 9))));
        let tex = surface_to_raster(&s, (0, 0, 0));
        // A corner of cell (0,0) carries its background colour.
        assert_eq!(tex.get(0, 0), Some((7, 8, 9)));
    }

    #[test]
    fn inked_glyph_lifts_the_cell_centre() {
        let mut s = Surface::new(1, 1);
        s.set_cell(
            0,
            0,
            inked_cell(Color::Rgb(255, 255, 255), Color::Rgb(0, 0, 0)),
        );
        let tex = surface_to_raster(&s, (0, 0, 0));
        // Centre pixel is lifted toward white; a corner stays background-dark.
        let mid = tex.get((CELL_PX / 2) as i32, (CELL_PX / 2) as i32).unwrap();
        assert!(
            mid.0 > 100,
            "centre should be lifted toward fg, got {mid:?}"
        );
        assert_eq!(tex.get(0, 0), Some((0, 0, 0)));
    }

    #[test]
    fn empty_surface_yields_a_bounded_texture() {
        let s = Surface::new(0, 0);
        let tex = surface_to_raster(&s, (1, 2, 3));
        assert!(tex.width() >= 1 && tex.height() >= 1);
    }

    #[test]
    fn reflection_is_dimmer_than_its_source() {
        let mut src = RgbRaster::new(4, 4);
        src.clear((200, 200, 200));
        let refl = reflection_of(&src);
        assert_eq!((refl.width(), refl.height()), (4, 4));
        // Every reflected texel is no brighter than the source.
        for y in 0..4 {
            for x in 0..4 {
                let c = refl.get(x, y).unwrap();
                assert!(c.0 <= 200);
            }
        }
    }

    #[test]
    fn surface_to_raster_is_deterministic() {
        let mut s = Surface::new(5, 4);
        s.set_cell(
            2,
            1,
            inked_cell(Color::Rgb(120, 200, 255), Color::Rgb(20, 10, 40)),
        );
        let a = surface_to_raster(&s, (5, 5, 5));
        let b = surface_to_raster(&s, (5, 5, 5));
        assert_eq!(a.pixels(), b.pixels());
    }
}
