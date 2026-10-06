//! The cinematic showcase's **look-dev layer** — the deep-space backdrop and the
//! post-processing that make a terminal film read as "wait, that's a TERMINAL?".
//!
//! Three pure, deterministic functions, all demo-local art direction (no new
//! library types; only [`gibson::raster::RgbRaster`] in, `RgbRaster` out):
//!
//! 1. [`starfield`] — a VAPOR95 nebula (indigo to ink, magenta accent cloud, a
//!    faint god-ray) under three parallax layers of stars that drift with `t`.
//! 2. [`bloom`] — in-place additive glow: bright texels shed their over-threshold
//!    excess into a soft two-scale halo that is added back, saturating at 255.
//! 3. [`grade`] — the final colour grade: a gentle S-curve, cool-violet shadows
//!    and faintly warm highlights, and a radial vignette.
//!
//! Everything is a function of its arguments alone: no wall clock, no RNG state,
//! integer hashing seeded from `(seed, cell, layer)`. Hostile inputs (NaN time,
//! absurd radii, 0-sized or 1x1 rasters) clamp or no-op; nothing here panics.
//!
//! Ooh yeah, look at me, I'm a look-dev layer! Existence is pain, but pretty pain.

#![allow(dead_code)] // not every caller uses every helper; the film picks and chooses.

use gibson::raster::{Rgb, RgbRaster};

// ---------------------------------------------------------------------------
// Palette (VAPOR95 register)
// ---------------------------------------------------------------------------

/// Nebula base at the "ink" end of the gradient.
const BASE_INK: Rgb = (18, 12, 32);
/// Nebula base at the "violet" end of the gradient.
const BASE_VIOLET: Rgb = (40, 20, 64);
/// Magenta accent the cloud glows toward (bloom-able once graded).
const MAGENTA: Rgb = (115, 38, 163);
/// God-ray tint, added faintly on top of the nebula.
const RAY: Rgb = (64, 34, 110);

/// Hard ceiling on the bloom radius, whatever the caller asks for.
const MAX_BLOOM_RADIUS: i32 = 256;
/// Hard ceiling on bloom intensity (a gain, so absurd values just saturate).
const MAX_BLOOM_INTENSITY: f32 = 16.0;

// ---------------------------------------------------------------------------
// Tiny deterministic toolbox
// ---------------------------------------------------------------------------

/// Integer hash of three words (a murmur-flavoured avalanche). Pure and total.
fn hash3(a: u32, b: u32, c: u32) -> u32 {
    let mut h =
        a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Map a hash to `[0, 1)` using its top 24 bits (exactly representable in f32).
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Hermite smoothstep of `x` between `lo` and `hi`, clamped to `[0, 1]`.
fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Linear interpolation of two scalars.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Round a float channel to `u8`, clamping first (NaN becomes 0).
fn to_u8(v: f32) -> u8 {
    if v.is_nan() {
        0
    } else {
        v.round().clamp(0.0, 255.0) as u8
    }
}

/// Smooth 2D value noise in `[0, 1)`, lattice values hashed from `(x, y, seed)`.
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (fx, fy) = (x - xf, y - yf);
    let (xi, yi) = (xf as i32 as u32, yf as i32 as u32);
    let corner = |dx: u32, dy: u32| unit(hash3(xi.wrapping_add(dx), yi.wrapping_add(dy), seed));
    // Smootherstep-ish fade keeps the lattice from showing through.
    let (u, v) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    lerp(
        lerp(corner(0, 0), corner(1, 0), u),
        lerp(corner(0, 1), corner(1, 1), u),
        v,
    )
}

/// Three-octave fractal value noise, normalised back to roughly `[0, 1)`.
fn fbm(x: f32, y: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let (mut amp, mut freq) = (0.5, 1.0);
    for octave in 0..3u32 {
        // Each octave gets its own seed and a rotated-ish offset to dodge alignment.
        sum += amp
            * value_noise(
                x * freq + 17.3 * octave as f32,
                y * freq - 9.1 * octave as f32,
                seed.wrapping_add(octave * 101),
            );
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / 0.875
}

/// Saturating additive plot of `color * k` onto a texel (clipped, never panics).
fn add_px(raster: &mut RgbRaster, x: i32, y: i32, color: (f32, f32, f32), k: f32) {
    if let Some((r, g, b)) = raster.get(x, y) {
        raster.set(
            x,
            y,
            (
                to_u8(r as f32 + color.0 * k),
                to_u8(g as f32 + color.1 * k),
                to_u8(b as f32 + color.2 * k),
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// starfield
// ---------------------------------------------------------------------------

/// One parallax depth of stars on a jittered grid.
struct StarLayer {
    /// Grid cell edge in texels; at most one star per cell.
    cell: u32,
    /// Probability that a cell holds a star.
    density: f32,
    /// Horizontal drift in texels per unit of `t` (nearer layers are faster).
    speed: f32,
    /// Brightness range, 0..1 of full white, before twinkle.
    lo: f32,
    hi: f32,
    /// Core disc radius in texels; 0 means a single faint texel.
    radius: f32,
    /// Strength of the 4-neighbour glow around the core (0 means none).
    halo: f32,
}

/// Far, mid, near: sparse and faint to sparse and bright.
const STAR_LAYERS: [StarLayer; 3] = [
    StarLayer {
        cell: 6,
        density: 0.20,
        speed: 1.2,
        lo: 0.16,
        hi: 0.36,
        radius: 0.0,
        halo: 0.0,
    },
    StarLayer {
        cell: 13,
        density: 0.20,
        speed: 3.5,
        lo: 0.40,
        hi: 0.72,
        radius: 0.0,
        halo: 0.16,
    },
    StarLayer {
        cell: 29,
        density: 0.14,
        speed: 8.0,
        lo: 0.85,
        hi: 1.00,
        radius: 0.9,
        halo: 0.30,
    },
];

/// A star's colour temperature as an RGB multiplier in `0..=1`.
fn star_tint(h: u32) -> (f32, f32, f32) {
    match h % 6 {
        0..=2 => (0.80, 0.85, 1.00), // blue-white, the common case
        3 => (1.00, 0.90, 0.78),     // warm
        4 => (1.00, 0.66, 0.94),     // magenta-pink, on brand
        _ => (0.65, 0.92, 1.00),     // cyan
    }
}

/// Scatter one parallax layer of stars onto `raster`, drifted by `t`.
fn plot_star_layer(raster: &mut RgbRaster, index: u32, layer: &StarLayer, t: f32, seed: u32) {
    let (w, h) = (u32::from(raster.width()), u32::from(raster.height()));
    if w == 0 || h == 0 {
        return;
    }
    let cols = w.div_ceil(layer.cell).max(1);
    let rows = h.div_ceil(layer.cell).max(1);
    let cell = layer.cell as f32;
    // The drift wraps over the (slightly wider than the frame) grid, so the SET of
    // stars is a pure function of (w, h, t, seed) and nothing pops at the seam.
    let period = (cols * layer.cell) as f32;
    let offset = (t * layer.speed).rem_euclid(period);
    let layer_seed = seed ^ index.wrapping_mul(0x6A09_E667).wrapping_add(0x1234_5679);

    for cy in 0..rows {
        for cx in 0..cols {
            let h0 = hash3(layer_seed, cx, cy);
            if unit(h0) >= layer.density {
                continue;
            }
            let h1 = hash3(h0, 0xA5A5, 1);
            let h2 = hash3(h0, 0x5A5A, 2);
            let h3 = hash3(h0, 0xC3C3, 3);
            let h4 = hash3(h0, 0x3C3C, 4);

            let x = (cx as f32 * cell + unit(h1) * cell - offset).rem_euclid(period);
            let y = cy as f32 * cell + unit(h2) * cell;
            let (xi, yi) = (x.floor() as i32, y.floor() as i32);
            if xi >= w as i32 || yi >= h as i32 {
                continue; // wrapped into the off-screen slack, it will drift back in
            }

            // Brightness with a slow deterministic twinkle (function of t only).
            let twinkle_rate = 0.7 + unit(h4) * 1.6;
            let twinkle = 1.0
                - 0.25 * (0.5 + 0.5 * (t * twinkle_rate + unit(h3) * std::f32::consts::TAU).sin());
            let b = lerp(layer.lo, layer.hi, unit(h3.rotate_left(11))) * twinkle;
            let tint = star_tint(h4 >> 3);
            let color = (255.0 * tint.0, 255.0 * tint.1, 255.0 * tint.2);

            if layer.radius > 0.0 {
                let core = (to_u8(color.0 * b), to_u8(color.1 * b), to_u8(color.2 * b));
                raster.disc(xi as f32, yi as f32, layer.radius, core);
            } else {
                add_px(raster, xi, yi, color, b);
            }
            if layer.halo > 0.0 {
                let k = layer.halo * b;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    add_px(raster, xi + dx, yi + dy, color, k);
                }
                if layer.radius > 0.0 {
                    // The brightest few get diagonal and 2-texel glints too.
                    for (dx, dy) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
                        add_px(raster, xi + dx, yi + dy, color, k * 0.4);
                    }
                    for (dx, dy) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
                        add_px(raster, xi + dx, yi + dy, color, k * 0.25);
                    }
                }
            }
        }
    }
}

/// A deep-space background of `w` x `h` texels: a subtle VAPOR95 nebula under
/// three parallax layers of stars.
///
/// Fully deterministic in `(w, h, t, seed)`. `t` is time in seconds: the nebula
/// creeps and the stars drift horizontally (nearer layers faster, so a moving
/// camera parallaxes), but the picture at a given `t` is always the same.
/// A non-finite `t` is treated as `0`. Dimensions are clamped by
/// [`RgbRaster::new`]; any `w, h >= 1` is safe, and `0` yields an empty raster.
pub fn starfield(w: u16, h: u16, t: f32, seed: u32) -> RgbRaster {
    let mut raster = RgbRaster::new(w, h);
    let t = if t.is_finite() { t } else { 0.0 };
    let (rw, rh) = (raster.width() as usize, raster.height() as usize);
    if rw == 0 || rh == 0 {
        return raster;
    }

    // --- nebula + god-ray -------------------------------------------------
    // Feature size follows the frame so the cloud composes the same at any res.
    let scale = (rw.max(rh) as f32 * 0.45).max(8.0);
    let drift = t * 0.004;
    let ray_centre = 0.58 + 0.05 * (t * 0.1).sin();
    let (inv_w, inv_h) = (1.0 / rw as f32, 1.0 / rh as f32);
    let seed_cloud = seed.wrapping_mul(0x9E37_79B9).wrapping_add(0xB5);
    let seed_base = seed_cloud ^ 0x5851_F42D;

    for (i, px) in raster.pixels_mut().iter_mut().enumerate() {
        let (x, y) = ((i % rw) as f32, (i / rw) as f32);
        let (nx, ny) = (x * inv_w, y * inv_h);

        // Ink-to-violet backdrop, a low-frequency wobble so it is not a pure ramp.
        let wobble = fbm(x / (scale * 2.0), y / (scale * 2.0), seed_base);
        let g = (0.55 * ny.powf(1.4) + 0.45 * wobble).clamp(0.0, 1.0);
        let mut c = [
            lerp(BASE_INK.0 as f32, BASE_VIOLET.0 as f32, g),
            lerp(BASE_INK.1 as f32, BASE_VIOLET.1 as f32, g),
            lerp(BASE_INK.2 as f32, BASE_VIOLET.2 as f32, g),
        ];

        // The magenta cloud: thresholded fbm so it clumps into wisps, kept subtle.
        let n = fbm(x / scale + drift, y / scale - drift * 0.6, seed_cloud);
        let cloud = smoothstep(0.46, 0.82, n) * 0.40;
        c[0] = lerp(c[0], MAGENTA.0 as f32, cloud);
        c[1] = lerp(c[1], MAGENTA.1 as f32, cloud);
        c[2] = lerp(c[2], MAGENTA.2 as f32, cloud);

        // A faint volumetric shaft slanting through the frame, fading downward.
        let along = nx * 0.85 + ny * 0.5;
        let d = (along - ray_centre) / 0.11;
        let shaft = (-d * d).exp() * (1.0 - ny).powf(1.5) * (0.55 + 0.45 * n) * 0.22;
        c[0] += RAY.0 as f32 * shaft;
        c[1] += RAY.1 as f32 * shaft;
        c[2] += RAY.2 as f32 * shaft;

        *px = (to_u8(c[0]), to_u8(c[1]), to_u8(c[2]));
    }

    // --- stars, far to near ------------------------------------------------
    for (index, layer) in STAR_LAYERS.iter().enumerate() {
        plot_star_layer(&mut raster, index as u32, layer, t, seed);
    }
    raster
}

// ---------------------------------------------------------------------------
// bloom
// ---------------------------------------------------------------------------

/// One sliding-window box blur along a line of `len` samples starting at `start`
/// with the given `stride`; samples outside the line count as zero. `tmp` must
/// hold at least `len` entries.
fn blur_line(
    data: &mut [[f32; 3]],
    tmp: &mut [[f32; 3]],
    start: usize,
    stride: usize,
    len: usize,
    r: usize,
) {
    for (i, slot) in tmp.iter_mut().take(len).enumerate() {
        *slot = data[start + i * stride];
    }
    let inv = 1.0 / (2 * r + 1) as f32;
    let mut sum = [0.0f32; 3];
    for s in tmp.iter().take((r + 1).min(len)) {
        for (acc, v) in sum.iter_mut().zip(s) {
            *acc += v;
        }
    }
    for i in 0..len {
        data[start + i * stride] = [
            (sum[0] * inv).max(0.0),
            (sum[1] * inv).max(0.0),
            (sum[2] * inv).max(0.0),
        ];
        // Slide the window [i-r, i+r] to [i+1-r, i+1+r].
        if i + r + 1 < len {
            for (acc, v) in sum.iter_mut().zip(&tmp[i + r + 1]) {
                *acc += v;
            }
        }
        if i >= r {
            for (acc, v) in sum.iter_mut().zip(&tmp[i - r]) {
                *acc -= v;
            }
        }
    }
}

/// One separable box-blur pass (rows, then columns) of radius `r`, in place.
fn box_blur(buf: &mut [[f32; 3]], tmp: &mut [[f32; 3]], w: usize, h: usize, r: usize) {
    if r == 0 {
        return;
    }
    for y in 0..h {
        blur_line(buf, tmp, y * w, 1, w, r);
    }
    for x in 0..w {
        blur_line(buf, tmp, x, w, h, r);
    }
}

/// In-place additive bloom.
///
/// Texels whose brightest channel exceeds `threshold` shed their excess (hue
/// preserved, ramping from 0 at the threshold to full at 255) into a two-scale
/// halo: a tight core of radius about `radius / 3` and a wider skirt reaching
/// `radius` texels (separable box blurs, so the cost is independent of radius).
/// The halo is added back onto the original scaled by `intensity`, saturating at
/// 255. The kernels are energy-conserving, so a lone texel's halo is faint while
/// a big bright shape glows convincingly; stars want `intensity` of several.
///
/// `radius <= 0`, a non-positive or non-finite `intensity`, or a raster with no
/// texel over `threshold` is an exact no-op. Radius clamps to 256 and intensity
/// to 16. One scratch allocation; deterministic; safe on 1x1.
pub fn bloom(raster: &mut RgbRaster, threshold: u8, radius: i32, intensity: f32) {
    if radius <= 0 || !intensity.is_finite() || intensity <= 0.0 {
        return;
    }
    let (w, h) = (raster.width() as usize, raster.height() as usize);
    let n = w * h;
    if n == 0
        || !raster
            .pixels()
            .iter()
            .any(|&(r, g, b)| r.max(g).max(b) > threshold)
    {
        return;
    }
    let intensity = intensity.min(MAX_BLOOM_INTENSITY);
    let r = (radius.min(MAX_BLOOM_RADIUS) as usize).min(w.max(h));

    // One allocation: [tight | wide | line scratch].
    let mut scratch = vec![[0.0f32; 3]; 2 * n + w.max(h)];
    let (tight, rest) = scratch.split_at_mut(n);
    let (wide, line) = rest.split_at_mut(n);

    // Extract the over-threshold excess, hue preserved.
    let span = f32::from(255 - threshold).max(1.0);
    for (slot, &(pr, pg, pb)) in tight.iter_mut().zip(raster.pixels()) {
        let m = pr.max(pg).max(pb);
        if m > threshold {
            let k = f32::from(m - threshold) / span;
            *slot = [f32::from(pr) * k, f32::from(pg) * k, f32::from(pb) * k];
        }
    }

    // Tight core, then widen a copy so the skirt reaches exactly `r` in total.
    let r_tight = (r / 3).max(1).min(r);
    box_blur(tight, line, w, h, r_tight);
    wide.copy_from_slice(tight);
    box_blur(wide, line, w, h, r - r_tight);

    const W_TIGHT: f32 = 0.5;
    const W_WIDE: f32 = 0.8;
    for ((px, t), wd) in raster
        .pixels_mut()
        .iter_mut()
        .zip(tight.iter())
        .zip(wide.iter())
    {
        let add =
            |c: u8, i: usize| to_u8(f32::from(c) + (t[i] * W_TIGHT + wd[i] * W_WIDE) * intensity);
        *px = (add(px.0, 0), add(px.1, 1), add(px.2, 2));
    }
}

// ---------------------------------------------------------------------------
// grade
// ---------------------------------------------------------------------------

/// Cool-violet gain applied in the shadows (multiplicative, so black stays black).
const SHADOW_GAIN: (f32, f32, f32) = (0.97, 0.96, 1.06);
/// Faintly warm gain applied in the highlights.
const HIGHLIGHT_GAIN: (f32, f32, f32) = (1.04, 1.01, 0.96);

/// Tone curve on `0..=1`: a gentle S (deeper shadows, brighter highlights) with a
/// small midtone lift. Monotone, fixes 0 and 1.
fn tone(x: f32) -> f32 {
    let s = x * x * (3.0 - 2.0 * x);
    let y = x + 0.38 * (s - x);
    (y + 0.03 * 4.0 * y * (1.0 - y)).clamp(0.0, 1.0)
}

/// In-place final colour grade: S-curve, split-tone, then vignette.
///
/// Each channel runs through a gentle S-curve (shadows deepen, midtones lift a
/// touch, highlights open). A luminance-weighted split-tone nudges shadows cool
/// violet and highlights faintly warm, multiplicatively, so pure black stays
/// black. Finally a radial vignette darkens toward the frame edge: `vignette` in
/// `0..=1` is its strength (`0` is none; out-of-range clamps, NaN counts as `0`).
/// Deterministic, bounded, safe on any size including 1x1 and empty.
///
/// Note the curve and tint always apply; `vignette == 0` only disables the
/// darkening.
pub fn grade(raster: &mut RgbRaster, vignette: f32) {
    let (w, h) = (raster.width() as usize, raster.height() as usize);
    if w == 0 || h == 0 {
        return;
    }
    let strength = if vignette.is_finite() {
        vignette.clamp(0.0, 1.0)
    } else {
        0.0
    };

    // A 256-entry LUT keeps the per-pixel work to lookups and a few multiplies.
    let mut lut = [0.0f32; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        *v = tone(i as f32 / 255.0);
    }

    let (half_w, half_h) = (w as f32 * 0.5, h as f32 * 0.5);
    for (i, px) in raster.pixels_mut().iter_mut().enumerate() {
        let (x, y) = ((i % w) as f32, (i / w) as f32);
        let (r, g, b) = (lut[px.0 as usize], lut[px.1 as usize], lut[px.2 as usize]);

        // Split-tone by luminance: shadows weigh (1-l)^2, highlights l^2.
        let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let (sw, hw) = ((1.0 - l) * (1.0 - l), l * l);
        let gain = |s: f32, hi: f32| lerp(1.0, s, sw) * lerp(1.0, hi, hw);

        // Elliptical radius: 0 at the centre, 1 at the corners.
        let dx = (x + 0.5 - half_w) / half_w;
        let dy = (y + 0.5 - half_h) / half_h;
        let d = ((dx * dx + dy * dy) * 0.5).sqrt();
        let v = 1.0 - strength * smoothstep(0.30, 1.0, d);

        *px = (
            to_u8(255.0 * r * gain(SHADOW_GAIN.0, HIGHLIGHT_GAIN.0) * v),
            to_u8(255.0 * g * gain(SHADOW_GAIN.1, HIGHLIGHT_GAIN.1) * v),
            to_u8(255.0 * b * gain(SHADOW_GAIN.2, HIGHLIGHT_GAIN.2) * v),
        );
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SIZES: [(u16, u16); 4] = [(1, 1), (2, 3), (200, 120), (37, 5)];

    fn uniform(w: u16, h: u16, c: Rgb) -> RgbRaster {
        let mut r = RgbRaster::new(w, h);
        r.clear(c);
        r
    }

    #[test]
    fn starfield_has_exact_dimensions_and_never_panics() {
        for &(w, h) in &SIZES {
            let r = starfield(w, h, 1.5, 7);
            assert_eq!((r.width(), r.height()), (w, h));
            assert_eq!(r.pixels().len(), w as usize * h as usize);
        }
        // Hostile inputs.
        assert_eq!(starfield(0, 0, 0.0, 1).pixels().len(), 0);
        assert_eq!(starfield(5, 4, f32::NAN, 1), starfield(5, 4, 0.0, 1));
        let _ = starfield(16, 9, f32::INFINITY, u32::MAX);
        let _ = starfield(16, 9, -1.0e9, 0);
        let _ = starfield(16, 9, 1.0e30, 3);
    }

    #[test]
    fn starfield_is_deterministic() {
        let a = starfield(200, 120, 3.25, 7);
        let b = starfield(200, 120, 3.25, 7);
        assert_eq!(a.pixels(), b.pixels());
    }

    #[test]
    fn starfield_varies_with_seed_and_time() {
        let base = starfield(200, 120, 0.0, 7);
        assert_ne!(base.pixels(), starfield(200, 120, 0.0, 8).pixels());
        assert_ne!(base.pixels(), starfield(200, 120, 30.0, 7).pixels());
    }

    #[test]
    fn starfield_is_a_dark_backdrop_with_bright_stars() {
        let r = starfield(200, 120, 0.0, 7);
        let px = r.pixels();
        let mean: f32 = px
            .iter()
            .map(|p| f32::from(p.0.max(p.1).max(p.2)))
            .sum::<f32>()
            / px.len() as f32;
        assert!(
            mean < 90.0,
            "backdrop should stay dark, mean peak was {mean}"
        );
        assert!(
            px.iter().any(|p| p.0.max(p.1).max(p.2) > 150),
            "no bright stars at all"
        );
        // The nebula never goes pure black anywhere: indigo floor.
        assert!(px.iter().all(|p| p.2 >= 20), "nebula floor broke");
    }

    #[test]
    fn starfield_drifts_but_is_not_scrambled() {
        let a = starfield(200, 120, 0.0, 7);
        let b = starfield(200, 120, 1.0, 7);
        let same = a
            .pixels()
            .iter()
            .zip(b.pixels())
            .filter(|(x, y)| x == y)
            .count();
        // A slow drift keeps most of the frame the same texel-for-texel at +1s...
        assert!(
            same * 2 > a.pixels().len(),
            "drift too violent: {same} texels equal"
        );
        // ...but it is not frozen.
        assert!(same < a.pixels().len());
    }

    #[test]
    fn bloom_is_safe_on_tiny_and_normal_rasters() {
        for &(w, h) in &SIZES {
            for (threshold, radius, intensity) in [
                (0u8, 1, 1.0),
                (128, 4, 2.0),
                (254, 300, 99.0),
                (255, 3, 1.0),
            ] {
                let mut r = starfield(w, h, 0.5, 3);
                bloom(&mut r, threshold, radius, intensity);
                assert_eq!(r.pixels().len(), w as usize * h as usize);
            }
        }
        let mut r = RgbRaster::new(0, 0);
        bloom(&mut r, 10, 5, 1.0);
        let mut r = starfield(9, 9, 0.0, 1);
        bloom(&mut r, 10, i32::MAX, f32::NAN);
        bloom(&mut r, 10, i32::MAX, f32::INFINITY);
        bloom(&mut r, 10, i32::MAX, 1.0);
    }

    #[test]
    fn bloom_noop_configurations_leave_pixels_untouched() {
        let original = starfield(64, 48, 2.0, 5);
        for (radius, intensity) in [(0, 1.0), (-4, 1.0), (5, 0.0), (5, -1.0), (5, f32::NAN)] {
            let mut r = original.clone();
            bloom(&mut r, 100, radius, intensity);
            assert_eq!(
                r.pixels(),
                original.pixels(),
                "radius {radius} intensity {intensity}"
            );
        }
        // Nothing over threshold is also a no-op.
        let mut dark = uniform(20, 20, (10, 10, 10));
        let before = dark.clone();
        bloom(&mut dark, 200, 6, 2.0);
        assert_eq!(dark.pixels(), before.pixels());
    }

    #[test]
    fn bloom_spreads_light_to_neighbours_and_only_adds() {
        let mut r = uniform(41, 41, (4, 2, 8));
        r.set(20, 20, (255, 255, 255));
        let before = r.clone();
        bloom(&mut r, 128, 6, 8.0);
        // Additive: no channel ever decreases.
        for (a, b) in before.pixels().iter().zip(r.pixels()) {
            assert!(b.0 >= a.0 && b.1 >= a.1 && b.2 >= a.2);
        }
        let neighbour = r.get(22, 20).unwrap();
        assert!(
            neighbour.0 > 4,
            "halo should reach 2 texels out, got {neighbour:?}"
        );
        // The halo never exceeds the requested reach.
        assert_eq!(r.get(20 + 7, 20), before.get(20 + 7, 20));
        assert_eq!(r.get(0, 0), before.get(0, 0));
        // And it falls off with distance.
        assert!(r.get(21, 20).unwrap().0 >= r.get(25, 20).unwrap().0);
    }

    #[test]
    fn bloom_is_deterministic_and_saturates() {
        let mut a = starfield(80, 50, 1.0, 9);
        let mut b = a.clone();
        bloom(&mut a, 120, 5, 3.0);
        bloom(&mut b, 120, 5, 3.0);
        assert_eq!(a.pixels(), b.pixels());
        let mut white = uniform(8, 8, (250, 250, 250));
        bloom(&mut white, 100, 3, 16.0);
        assert!(white.pixels().iter().all(|&p| p == (255, 255, 255)));
    }

    #[test]
    fn grade_is_safe_on_tiny_and_normal_rasters() {
        for &(w, h) in &SIZES {
            for vignette in [0.0, 0.5, 1.0, 7.0, -3.0, f32::NAN, f32::INFINITY] {
                let mut r = starfield(w, h, 0.5, 3);
                grade(&mut r, vignette);
                assert_eq!(r.pixels().len(), w as usize * h as usize);
            }
        }
        let mut r = RgbRaster::new(0, 3);
        grade(&mut r, 0.5);
    }

    #[test]
    fn grade_without_vignette_is_reasonable_and_deterministic() {
        let original = starfield(64, 48, 2.0, 5);
        let (mut a, mut b) = (original.clone(), original.clone());
        grade(&mut a, 0.0);
        grade(&mut b, 0.0);
        assert_eq!(a.pixels(), b.pixels());
        // Reasonable: a tasteful nudge, not a recolour. Nothing moves by more than ~40 levels.
        for (o, g) in original.pixels().iter().zip(a.pixels()) {
            for (co, cg) in [(o.0, g.0), (o.1, g.1), (o.2, g.2)] {
                assert!(co.abs_diff(cg) <= 40, "{o:?} -> {g:?}");
            }
        }
        // Black stays black, and the curve is monotone.
        let mut black = uniform(2, 2, (0, 0, 0));
        grade(&mut black, 0.0);
        assert!(black.pixels().iter().all(|&p| p == (0, 0, 0)));
        for i in 0..255 {
            assert!(tone((i + 1) as f32 / 255.0) >= tone(i as f32 / 255.0));
        }
    }

    #[test]
    fn grade_vignette_darkens_edges_not_centre() {
        let mut flat = uniform(101, 61, (128, 128, 128));
        grade(&mut flat, 0.0);
        let centre_ref = flat.get(50, 30).unwrap();
        let mut v = uniform(101, 61, (128, 128, 128));
        grade(&mut v, 1.0);
        assert_eq!(
            v.get(50, 30).unwrap(),
            centre_ref,
            "centre must be untouched"
        );
        let corner = v.get(0, 0).unwrap();
        assert!(
            corner.0 < centre_ref.0 / 4 && corner.2 < centre_ref.2 / 4,
            "{corner:?}"
        );
        // Strength is monotone: a half vignette sits between.
        let mut half = uniform(101, 61, (128, 128, 128));
        grade(&mut half, 0.5);
        let c = half.get(0, 0).unwrap();
        assert!(c.0 < centre_ref.0 && c.0 > corner.0);
    }

    #[test]
    fn grade_splits_tone_cool_shadows_warm_highlights() {
        let mut shadow = uniform(1, 1, (40, 40, 40));
        grade(&mut shadow, 0.0);
        let s = shadow.get(0, 0).unwrap();
        assert!(s.2 > s.0, "shadows should lean violet/blue, got {s:?}");
        let mut high = uniform(1, 1, (220, 220, 220));
        grade(&mut high, 0.0);
        let h = high.get(0, 0).unwrap();
        assert!(h.0 > h.2, "highlights should lean warm, got {h:?}");
    }

    #[test]
    fn full_pipeline_stays_in_bounds() {
        for &(w, h) in &SIZES {
            let mut r = starfield(w, h, 4.0, 11);
            bloom(&mut r, 140, 6, 3.0);
            grade(&mut r, 0.6);
            assert_eq!(r.pixels().len(), w as usize * h as usize);
            // Every in-bounds index is readable; the edges just outside are not.
            assert!(r.get(i32::from(w) - 1, i32::from(h) - 1).is_some());
            assert!(r.get(i32::from(w), 0).is_none());
            assert!(r.get(0, i32::from(h)).is_none());
            assert!(r.get(-1, 0).is_none());
        }
    }
}
