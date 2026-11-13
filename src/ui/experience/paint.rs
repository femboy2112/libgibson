//! Shared painting helpers for raster grammars.
//!
//! The house rule: when crisp terminal text sits over a rendered raster, it must
//! never punch an opaque terminal block. Either the glyph's cell background is the
//! raster colour beneath it (so the block vanishes and only the character shows),
//! or the text is not drawn over raster at all. [`bake`] implements the former:
//! it writes a string as real terminal glyphs whose backgrounds are sampled from
//! the surface cells they land on.

use super::model::Media;
use crate::cell::{Cell, Color, Glyph, Style};
use crate::raster::{Rgb, RgbRaster};
use crate::surface::Surface;
use crate::ui::element::Key;

/// Side length, in raster pixels, of a procedural cover.
pub(crate) const ART_SIZE: u16 = 64;

/// Linear interpolation between two colours.
pub(crate) fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// HSV (h in degrees, s/v in 0..1) → RGB.
pub(crate) fn hsv(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |z: f32| ((z + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r), to(g), to(b))
}

/// A seed for an item: its declared media seed, else a deterministic hash of its key.
pub(crate) fn seed_for(key: &Key, media: Option<&Media>) -> u64 {
    media.map(|m| m.seed).unwrap_or_else(|| {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in key.to_string().bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    })
}

/// A deterministic abstract cover for a seed: golden-angle hues, one of six motifs,
/// a one-texel tonal rim. Pure function of the seed. (Mirrors the shelf's covers.)
pub(crate) fn cover_art(seed: u64) -> RgbRaster {
    let seed32 = (seed ^ (seed >> 32)) as u32;
    let mut r = RgbRaster::new(ART_SIZE, ART_SIZE);
    let mut state = seed32
        .wrapping_mul(2_654_435_761)
        .wrapping_add(40_503)
        .max(1);
    let mut rng = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let jitter = (rng() % 24) as f32;
    let hue = (seed32 as f32 * 137.508 + 14.0 + jitter) % 360.0;
    let hue2 = (hue + 150.0 + (rng() % 60) as f32) % 360.0;
    let motif = seed32 % 6;
    let wobble = (rng() % 100) as f32 / 100.0;
    let denom = (ART_SIZE - 1) as f32;
    for y in 0..ART_SIZE {
        for x in 0..ART_SIZE {
            let fx = x as f32 / denom;
            let fy = y as f32 / denom;
            let t = fy * 0.7 + fx * 0.3;
            let mut c = hsv(hue + (hue2 - hue) * 0.35 * t, 0.74, 0.50 + 0.38 * (1.0 - t));
            let (dx, dy) = (fx - 0.5, fy - 0.5);
            let d = (dx * dx + dy * dy).sqrt();
            let ink = hsv(hue2, 0.55, 1.0);
            let pale = hsv(hue + 20.0, 0.18, 0.98);
            match motif {
                0 => {
                    let cy = 0.44 + 0.08 * wobble;
                    let dd = ((fx - 0.5).powi(2) + (fy - cy).powi(2)).sqrt();
                    if dd < 0.25 {
                        c = ink;
                    } else if dd < 0.31 {
                        c = mix(c, ink, 0.35);
                    }
                }
                1 => {
                    let horizon = 0.58;
                    if fy < horizon && ((fx - 0.5).powi(2) + (fy - horizon).powi(2)).sqrt() < 0.30 {
                        c = ink;
                    } else if fy >= horizon {
                        let band = ((fy - horizon) * 17.0) as i32 % 2 == 0;
                        c = mix(c, hsv(hue2, 0.7, 0.35), if band { 0.65 } else { 0.25 });
                    }
                }
                2 => {
                    let k = (fx - fy).abs();
                    if k < 0.13 {
                        c = ink;
                    } else if (k - 0.24).abs() < 0.025 {
                        c = pale;
                    }
                }
                3 => {
                    let ring = (d * 7.0) as i32;
                    if d < 0.43 && ring % 2 == 0 {
                        c = mix(c, ink, if ring == 0 { 1.0 } else { 0.8 });
                    }
                }
                4 => {
                    let apex = 0.22 + 0.10 * wobble;
                    let half = (fy - apex) * 0.62;
                    if fy > apex && fy < 0.82 && dx.abs() < half {
                        c = if fy < apex + 0.12 { pale } else { ink };
                    }
                }
                _ => {
                    let col = (fx * 7.0) as usize;
                    let within = (fx * 7.0).fract();
                    let h = 0.25 + 0.55 * (((col as f32 * 1.7 + wobble * 6.0).sin() + 1.0) * 0.5);
                    if within > 0.18 && within < 0.88 && fy > 0.88 - h && fy < 0.88 {
                        c = if fy < 0.88 - h + 0.05 { pale } else { ink };
                    }
                }
            }
            let edge = x == 0 || y == 0 || x == ART_SIZE - 1 || y == ART_SIZE - 1;
            if edge {
                c = mix(c, (255, 255, 255), 0.28);
            }
            r.set(x as i32, y as i32, c);
        }
    }
    r
}

/// Nearest-neighbour scale-blit of `src` into `dst` at `(dx,dy)` sized `dw×dh`.
pub(crate) fn scale_blit(dst: &mut RgbRaster, src: &RgbRaster, dx: i32, dy: i32, dw: i32, dh: i32) {
    if dw <= 0 || dh <= 0 {
        return;
    }
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    for yy in 0..dh {
        let sy = (yy as f32 / dh as f32 * sh) as i32;
        for xx in 0..dw {
            let sx = (xx as f32 / dw as f32 * sw) as i32;
            if let Some(c) = src.get(sx, sy) {
                dst.set(dx + xx, dy + yy, c);
            }
        }
    }
}

/// A soft radial glow centred at `(cx,cy)`.
pub(crate) fn glow(dst: &mut RgbRaster, cx: i32, cy: i32, radius: f32, color: Rgb, strength: f32) {
    let r = radius.ceil() as i32;
    for y in (cy - r)..=(cy + r) {
        for x in (cx - r)..=(cx + r) {
            let d = (((x - cx) as f32).powi(2) + ((y - cy) as f32).powi(2)).sqrt();
            let a = (1.0 - d / radius).clamp(0.0, 1.0).powi(2) * strength;
            if a > 0.0 {
                dst.blend(x, y, color, a);
            }
        }
    }
}

/// The RGB of a concrete colour, or `None` for `Color::Reset` / palette indices.
pub(crate) fn resolve_rgb(c: Color) -> Option<Rgb> {
    if let Color::Rgb(r, g, b) = c {
        Some((r, g, b))
    } else {
        None
    }
}

/// The representative colour of a (half-block) surface cell — the mean of its two
/// pixels — used as a baked glyph's background so the glyph's cell blends into the
/// raster behind it instead of punching an opaque terminal block.
pub(crate) fn cell_bg(surf: &Surface, x: u16, y: u16) -> Rgb {
    surf.get(x, y)
        .map(|cell| {
            let top = cell.style.fg.and_then(resolve_rgb);
            let bot = cell.style.bg.and_then(resolve_rgb);
            match (top, bot) {
                (Some(t), Some(b)) => (t.0 / 2 + b.0 / 2, t.1 / 2 + b.1 / 2, t.2 / 2 + b.2 / 2),
                (Some(t), None) => t,
                (None, Some(b)) => b,
                (None, None) => (10, 8, 18),
            }
        })
        .unwrap_or((10, 8, 18))
}

/// Bake `s` onto the surface at `(x0,y)` as crisp glyphs whose backgrounds match the
/// raster beneath each cell — the block vanishes, only the character shows. Stops at
/// the surface edge.
pub(crate) fn bake(surf: &mut Surface, x0: u16, y: u16, s: &str, fg: Rgb, bold: bool) {
    let mut x = x0;
    for ch in s.chars() {
        let bg = cell_bg(surf, x, y);
        let mut style = Style::new()
            .fg(Color::Rgb(fg.0, fg.1, fg.2))
            .bg(Color::Rgb(bg.0, bg.1, bg.2));
        if bold {
            style = style.bold();
        }
        if !surf.set_cell(x, y, Cell::new(Glyph::new(&ch.to_string()), style)) {
            break;
        }
        x = x.saturating_add(1);
    }
}

/// Bake a string centred on cell column `cx`.
pub(crate) fn bake_centered(surf: &mut Surface, cx: u16, y: u16, s: &str, fg: Rgb, bold: bool) {
    let half = (s.chars().count() as u16) / 2;
    bake(surf, cx.saturating_sub(half), y, s, fg, bold);
}
