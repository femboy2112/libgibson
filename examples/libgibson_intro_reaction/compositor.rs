//! The surface-local reaction compositor.
//!
//! Given a freshly generated `libgibson_intro` [`Surface`] and a keyed source
//! frame placed by a [`Transform`], it patches **only** the cells the transformed
//! subject actually covers, and leaves every other cell of the base intro exactly
//! as the intro produced it. This is the whole reason the reaction cut does not
//! degrade the film underneath: there is one renderer (the ordinary `Surface` ->
//! ANSI path), and the projector only ever re-touches cells with real subject ink.
//!
//! Per touched cell:
//!   1. inverse-sample the transformed keyed foreground at each of the 8 logical
//!      Braille subpixels -> `(fg, alpha)`;
//!   2. if every alpha is 0, the subject does not reach this cell -> leave the base
//!      cell untouched (byte-identical), so a moving subject's vacated trail is the
//!      original intro again the instant it leaves;
//!   3. otherwise reconstruct the base cell into a logical 2x4 RGB tile, alpha-
//!      composite the foreground over it per subpixel, and re-project that one tile
//!      through the same subcell projector every other surface uses.

#![allow(dead_code)]

use super::film::KeyedFilm;
use super::key::{key_pixel, KeyParams};
use super::transform::Transform;
use gibson::cell::{Cell, Color, Glyph};
use gibson::temporal::project_rgb_subcells;
use gibson::{SubcellGlyphMode, Surface};

/// (dy, dx) -> index into the 8-element Braille tile, mirroring the projector's own
/// dot order (`project_rgb_subcells` consumes tiles in this layout). Kept as a local
/// copy so the compositor does not depend on a private crate constant.
const DOT: [[usize; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];

/// sRGB 8-bit -> linear light.
fn s2l(c: u8) -> f32 {
    let x = c as f32 / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light -> sRGB 8-bit (rounded, clamped).
fn l2s(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let x = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (x * 255.0 + 0.5) as u8
}

fn rgb_of(c: Option<Color>, fallback: [u8; 3]) -> [u8; 3] {
    c.map(|c| {
        let (r, g, b) = c.to_rgb();
        [r, g, b]
    })
    .unwrap_or(fallback)
}

fn blend(fg: [u8; 3], bg: [u8; 3], cov: f32) -> [u8; 3] {
    let mut o = [0u8; 3];
    for i in 0..3 {
        o[i] = (fg[i] as f32 * cov + bg[i] as f32 * (1.0 - cov) + 0.5) as u8;
    }
    o
}

/// Reconstruct a logical 2x4 RGB tile (in projector dot order) from an existing
/// cell. Terminal-native subcell glyphs are decoded exactly; unknown/ordinary text
/// falls back to a documented conservative estimate. Only cells under the reaction
/// subject are ever reconstructed, so the fallback never touches the visible intro.
pub fn reconstruct_tile(cell: &Cell) -> [[u8; 3]; 8] {
    let fg = rgb_of(cell.style.fg, [204, 204, 204]);
    let bg = rgb_of(cell.style.bg, [0, 0, 0]);
    let mut tile = [bg; 8];
    let mut chars = cell.glyph.grapheme.chars();
    match chars.next() {
        // Braille U+2800..=U+28FF: decode the 8-dot mask exactly (each dot fg|bg).
        Some(c) if ('\u{2800}'..='\u{28FF}').contains(&c) => {
            let mask = (c as u32 - 0x2800) as u8;
            for (i, t) in tile.iter_mut().enumerate() {
                *t = if mask & (1 << i) != 0 { fg } else { bg };
            }
        }
        // Half blocks: top/bottom coverage.
        Some('\u{2580}') => set_halves(&mut tile, fg, bg), // upper half block
        Some('\u{2584}') => set_halves(&mut tile, bg, fg), // lower half block
        Some('\u{2588}') => tile = [fg; 8],                // full block
        Some(' ') | None => {}                             // space -> all bg
        // Shade blocks: a uniform coverage estimate.
        Some('\u{2591}') => tile = [blend(fg, bg, 0.25); 8],
        Some('\u{2592}') => tile = [blend(fg, bg, 0.5); 8],
        Some('\u{2593}') => tile = [blend(fg, bg, 0.75); 8],
        // Ordinary/unknown glyph: no coverage is knowable from a Cell, so use a
        // conservative half-ink estimate. Documented fallback; only ever applied to
        // a cell already under the subject, never to a visible intro cell.
        Some(_) => tile = [blend(fg, bg, 0.5); 8],
    }
    tile
}

fn set_halves(tile: &mut [[u8; 3]; 8], top: [u8; 3], bottom: [u8; 3]) {
    for (dy, half) in [top, top, bottom, bottom].into_iter().enumerate() {
        for dx in 0..2 {
            tile[DOT[dy][dx]] = half;
        }
    }
}

/// Key one source sample (nearest, or bilinear on the premultiplied foreground to
/// suppress green edge halos when the subject is enlarged). Returns straight
/// `(fg, alpha)`.
fn sample_keyed(
    film: &KeyedFilm,
    frame: &[u8],
    fx: f32,
    fy: f32,
    kp: &KeyParams,
    bilinear: bool,
) -> ([u8; 3], f32) {
    if !bilinear {
        let px = film.sample(
            frame,
            fx.round().max(0.0) as u32,
            fy.round().max(0.0) as u32,
        );
        return key_pixel(px, kp);
    }
    let x0 = fx.floor();
    let y0 = fy.floor();
    let tx = fx - x0;
    let ty = fy - y0;
    let mut acc = [0f32; 3];
    let mut aacc = 0f32;
    for (wy, oy) in [(1.0 - ty, 0i32), (ty, 1i32)] {
        for (wx, ox) in [(1.0 - tx, 0i32), (tx, 1i32)] {
            let w = wx * wy;
            if w <= 0.0 {
                continue;
            }
            let sx = (x0 as i32 + ox).max(0) as u32;
            let sy = (y0 as i32 + oy).max(0) as u32;
            let (fg, a) = key_pixel(film.sample(frame, sx, sy), kp);
            // Premultiply so transparent green never bleeds colour across the edge.
            acc[0] += fg[0] as f32 * a * w;
            acc[1] += fg[1] as f32 * a * w;
            acc[2] += fg[2] as f32 * a * w;
            aacc += a * w;
        }
    }
    if aacc <= 1e-4 {
        return ([0, 0, 0], 0.0);
    }
    let fg = [
        (acc[0] / aacc).round().clamp(0.0, 255.0) as u8,
        (acc[1] / aacc).round().clamp(0.0, 255.0) as u8,
        (acc[2] / aacc).round().clamp(0.0, 255.0) as u8,
    ];
    (fg, aacc.clamp(0.0, 1.0))
}

/// Alpha-composite `fg` over `bg` at coverage `a`. Byte-space by default; when
/// `linear` is set, the blend is done in linear light (physically correct at the
/// matte edge, which the projector itself reasons in).
fn composite_px(fg: [u8; 3], a: f32, bg: [u8; 3], linear: bool) -> [u8; 3] {
    let a = a.clamp(0.0, 1.0);
    if a <= 0.0 {
        return bg;
    }
    if a >= 1.0 {
        return fg;
    }
    if linear {
        let mut o = [0u8; 3];
        for i in 0..3 {
            let v = s2l(fg[i]) * a + s2l(bg[i]) * (1.0 - a);
            o[i] = l2s(v);
        }
        o
    } else {
        let mut o = [0u8; 3];
        for i in 0..3 {
            o[i] = (fg[i] as f32 * a + bg[i] as f32 * (1.0 - a) + 0.5) as u8;
        }
        o
    }
}

/// Options controlling the compositing math (chosen by measured visual quality).
/// The default is sharp and deterministic (nearest, byte-space); the reaction cut
/// opts into bilinear smoothing and linear-light matte compositing where a zoom
/// needs it.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompositeOpts {
    pub bilinear: bool,
    pub linear_light: bool,
}

/// The result of a composite pass, for the performance receipts.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompositeStats {
    /// Cells inside the transformed subject bbox that were visited.
    pub cells_touched: usize,
    /// Cells actually re-projected (had non-zero subject coverage).
    pub cells_composited: usize,
}

/// Patch the transformed keyed subject into `surface` in place. Every cell outside
/// the subject's coverage is left byte-identical to the base intro.
pub fn composite_into(
    surface: &mut Surface,
    film: &KeyedFilm,
    frame: &[u8],
    t: &Transform,
    kp: &KeyParams,
    mode: SubcellGlyphMode,
    opts: CompositeOpts,
) -> CompositeStats {
    let subw = surface.width as u32 * 2;
    let subh = surface.height as u32 * 4;
    let (bx, by, bw, bh) = t.dest_bbox_sub(film.film_w, film.film_h, subw, subh);
    if bw == 0 || bh == 0 {
        return CompositeStats::default();
    }
    let cx0 = bx / 2;
    let cy0 = by / 4;
    let cx1 = (bx + bw).div_ceil(2).min(surface.width as u32);
    let cy1 = (by + bh).div_ceil(4).min(surface.height as u32);
    let fw = film.film_w as f32 - 1.0;
    let fh = film.film_h as f32 - 1.0;
    let mut stats = CompositeStats::default();
    for cy in cy0..cy1 {
        for cx in cx0..cx1 {
            stats.cells_touched += 1;
            let mut fg_tile = [[0u8; 3]; 8];
            let mut a_tile = [0f32; 8];
            let mut any = false;
            for dy in 0..4u32 {
                for dx in 0..2u32 {
                    let dsx = (cx * 2 + dx) as f32 + 0.5;
                    let dsy = (cy * 4 + dy) as f32 + 0.5;
                    let (sx, sy) = t.inverse(dsx, dsy, film.film_w, film.film_h);
                    if sx < 0.0 || sy < 0.0 || sx > fw || sy > fh {
                        continue;
                    }
                    let (fg, a) = sample_keyed(film, frame, sx, sy, kp, opts.bilinear);
                    if a > 0.0 {
                        let i = DOT[dy as usize][dx as usize];
                        fg_tile[i] = fg;
                        a_tile[i] = a;
                        any = true;
                    }
                }
            }
            if !any {
                // No subject ink here: leave the base intro cell exactly as it was.
                continue;
            }
            let base = reconstruct_tile(surface.get(cx as u16, cy as u16).unwrap());
            let mut out = [[0u8; 3]; 8];
            for i in 0..8 {
                out[i] = composite_px(fg_tile[i], a_tile[i], base[i], opts.linear_light);
            }
            let proj = project_rgb_subcells(out);
            let cell = match mode.subcell_glyph(proj.static_mask) {
                Some(ch) => Cell::new(Glyph::from_char(ch), proj.style),
                None => Cell::space(proj.style),
            };
            surface.set_cell(cx as u16, cy as u16, cell);
            stats.cells_composited += 1;
        }
    }
    stats
}

/// Pick the source frame index for a normalized progress `p in [0,1]` across a
/// baked cue window. Deterministic: replaying an edit time reproduces the frame.
pub fn frame_index(nframes: usize, p: f32) -> usize {
    if nframes <= 1 {
        return 0;
    }
    let p = p.clamp(0.0, 1.0);
    ((p * (nframes - 1) as f32).round() as usize).min(nframes - 1)
}
