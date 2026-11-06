//! Pure parts of the raster-ingest bench: a deterministic moving 320×200 source and a
//! consumer-side box-filter fit onto a terminal grid. Integer arithmetic only, so every result
//! is identical on every machine (the oracle test relies on that).
//!
//! **Consumer-side, labelled.** LibGibson has no public "fit a W×H raster into a cell
//! rectangle" primitive (#128 / #129). This module is the bench's own resampler, used to
//! measure what such a primitive would cost and need — not a proposed API.
use gibson::glyph::{transcode_surface_glyphs, SubcellGlyphMode};
use gibson::raster::RgbRaster;
use gibson::surface::Surface;

pub const SOURCE_W: u16 = 320;
pub const SOURCE_H: u16 = 200;

/// Frame `n` of the moving source: a diagonal gradient, a drifting 16 px checker band, a
/// bouncing disc and a hard white/black edge that sweeps 3 px per frame.
pub fn source_frame(n: u32) -> RgbRaster {
    let mut r = RgbRaster::new(SOURCE_W, SOURCE_H);
    let (w, h) = (u32::from(SOURCE_W), u32::from(SOURCE_H));
    let shift = n.wrapping_mul(2);
    let edge = (n.wrapping_mul(3)) % w;
    for y in 0..h {
        for x in 0..w {
            let g = ((x + y + shift) % 256) as u8;
            let mut c = (g / 3, g / 2, 96u8.wrapping_add(g / 4));
            if (40..72).contains(&y) && ((x + shift) / 16 + y / 16) % 2 == 0 {
                c = (230, 200, 40);
            }
            if x >= edge && x < edge + 2 {
                c = (255, 255, 255);
            } else if x >= edge + 2 && x < edge + 4 {
                c = (0, 0, 0);
            }
            r.set(x as i32, y as i32, c);
        }
    }
    // Disc: centre bounces on a triangle wave; integer-only inside test.
    let period = 2 * (w - 80);
    let p = (n.wrapping_mul(4)) % period;
    let tri = if p < w - 80 { p } else { period - p };
    let cx = 40 + i64::from(tri);
    let cy = 130i64;
    let rad = 28i64;
    for y in (cy - rad).max(0)..(cy + rad + 1).min(i64::from(SOURCE_H)) {
        for x in (cx - rad).max(0)..(cx + rad + 1).min(i64::from(SOURCE_W)) {
            if (x - cx) * (x - cx) + (y - cy) * (y - cy) <= rad * rad {
                r.set(x as i32, y as i32, (220, 40, 90));
            }
        }
    }
    r
}

/// Box-filter (area-average) fit of `src` onto a `dw × dh` raster, stretching both axes.
/// Destination pixel `(dx, dy)` averages the source rectangle
/// `[dx·sw/dw, (dx+1)·sw/dw) × [dy·sh/dh, (dy+1)·sh/dh)` (at least one pixel each way), with
/// round-half-up integer division. `dw`/`dh` are clamped to at least 1.
pub fn fit_box(src: &RgbRaster, dw: u16, dh: u16) -> RgbRaster {
    let (dw, dh) = (dw.max(1), dh.max(1));
    let (sw, sh) = (u32::from(src.width()), u32::from(src.height()));
    let mut out = RgbRaster::new(dw, dh);
    let px = src.pixels();
    for dy in 0..u32::from(dh) {
        let y0 = dy * sh / u32::from(dh);
        let y1 = ((dy + 1) * sh / u32::from(dh)).max(y0 + 1).min(sh);
        for dx in 0..u32::from(dw) {
            let x0 = dx * sw / u32::from(dw);
            let x1 = ((dx + 1) * sw / u32::from(dw)).max(x0 + 1).min(sw);
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for y in y0..y1 {
                let row = (y * sw) as usize;
                for x in x0..x1 {
                    let (pr, pg, pb) = px[row + x as usize];
                    r += u32::from(pr);
                    g += u32::from(pg);
                    b += u32::from(pb);
                }
            }
            let n = (y1 - y0) * (x1 - x0);
            let avg = |s: u32| ((s + n / 2) / n) as u8;
            out.set(dx as i32, dy as i32, (avg(r), avg(g), avg(b)));
        }
    }
    out
}

/// How a fitted raster becomes cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lowering {
    /// Colour half-blocks: 1 px per column, 2 px per row (`RgbRaster::to_surface`).
    HalfBlockColor,
    /// Monochrome density in a sub-cell glyph family (from `RgbRaster::to_mono_surface`'s
    /// Braille masks, transcoded for Block / Ascii / HalfBlock fallbacks).
    Mono(SubcellGlyphMode),
}

impl Lowering {
    pub fn name(self) -> &'static str {
        match self {
            Lowering::HalfBlockColor => "halfblock-color",
            Lowering::Mono(SubcellGlyphMode::Braille2x4) => "braille-mono",
            Lowering::Mono(SubcellGlyphMode::HalfBlock1x2) => "halfblock-mono",
            Lowering::Mono(SubcellGlyphMode::Block) => "block-mono",
            Lowering::Mono(SubcellGlyphMode::Ascii) => "ascii-mono",
            Lowering::Mono(_) => "mono-other",
        }
    }
}

/// The raster size a `cols × rows` grid needs for `lowering` (both families read 1 × 2 RGB
/// samples per cell).
pub fn fitted_size(cols: u16, rows: u16) -> (u16, u16) {
    (cols, rows.saturating_mul(2))
}

/// Lowers a fitted raster into a `Surface` of `fitted.width() × fitted.height()/2` cells.
pub fn lower(fitted: &RgbRaster, lowering: Lowering) -> Surface {
    match lowering {
        Lowering::HalfBlockColor => fitted.to_surface(),
        Lowering::Mono(mode) => {
            let mut s = fitted.to_mono_surface();
            if mode != SubcellGlyphMode::Braille2x4 {
                transcode_surface_glyphs(&mut s, mode);
            }
            s
        }
    }
}

/// FNV-1a over bytes, for frame digests.
pub fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}
