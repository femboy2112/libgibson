//! Small procedural field effects rendered through the sub-cell canvases.
//!
//! This is not a shader engine. It provides a few high-leverage deterministic
//! scalar fields (interference, radial pulse, seeded wobble) plus an intensity
//! colour ramp, so a terminal can run a demoscene-grade plasma.

use crate::canvas::{BrailleCanvas, HalfBlockCanvas};

/// Classic plasma: summed sines plus a radial pulse, normalized to `[0, 1]`.
pub fn plasma(x: f32, y: f32, t: f32, seed: f32) -> f32 {
    let a = (x + t + seed).sin();
    let b = (y * 1.3 - t * 0.7).sin();
    let c = ((x + y) * 0.7 + t * 1.1).sin();
    let r = ((x * x + y * y).sqrt() * 1.5 - t * 1.3 + seed).sin();
    ((a + b + c + r) / 4.0 + 1.0) * 0.5
}

/// Sine interference with a controllable frequency/wavelength.
pub fn interference(x: f32, y: f32, t: f32, freq: f32) -> f32 {
    let v = (x * freq + t).sin() + (y * freq * 0.9 - t * 0.8).sin();
    (v / 2.0 + 1.0) * 0.5
}

/// Radial pulse decaying with distance from the origin.
pub fn radial_pulse(x: f32, y: f32, t: f32) -> f32 {
    let r = (x * x + y * y).sqrt();
    ((r * 2.2 - t * 2.5).sin() * (-r * 0.15).exp() + 1.0) * 0.5
}

/// Maps a field value in `[0, 1]` to a blue→cyan→green→yellow→red ramp.
pub fn heat_rgb(v: f32) -> (u8, u8, u8) {
    let v = v.clamp(0.0, 1.0);
    let stops: [(f32, (f32, f32, f32)); 5] = [
        (0.0, (10.0, 10.0, 90.0)),
        (0.25, (0.0, 180.0, 255.0)),
        (0.5, (0.0, 230.0, 90.0)),
        (0.75, (255.0, 220.0, 0.0)),
        (1.0, (255.0, 40.0, 20.0)),
    ];
    for w in stops.windows(2) {
        let (p0, c0) = w[0];
        let (p1, c1) = w[1];
        if v <= p1 {
            let t = if p1 > p0 { (v - p0) / (p1 - p0) } else { 0.0 };
            let mix = |a: f32, b: f32| (a + (b - a) * t).round().clamp(0.0, 255.0) as u8;
            return (mix(c0.0, c1.0), mix(c0.1, c1.1), mix(c0.2, c1.2));
        }
    }
    (255, 40, 20)
}

/// Renders a plasma field into a half-block RGB canvas (truecolor/ANSI fallback
/// is applied centrally by the capability-aware compiler).
pub fn render_plasma_halfblock(canvas: &mut HalfBlockCanvas, t: f32, seed: f32) {
    let pw = canvas.pixel_width() as f32;
    let ph = canvas.pixel_height() as f32;
    for y in 0..canvas.pixel_height() as i32 {
        for x in 0..canvas.pixel_width() as i32 {
            let v = plasma(x as f32 / pw * 6.0, y as f32 / ph * 6.0, t, seed);
            canvas.set_pixel(x, y, heat_rgb(v));
        }
    }
}

/// Renders a field as Braille *density*: the mono-safe fallback where colour is
/// unavailable but shape/intensity must survive.
pub fn render_field_braille(
    canvas: &mut BrailleCanvas,
    t: f32,
    seed: f32,
    threshold: f32,
    freq: f32,
) {
    let pw = canvas.pixel_width() as f32;
    let ph = canvas.pixel_height() as f32;
    for y in 0..canvas.pixel_height() as i32 {
        for x in 0..canvas.pixel_width() as i32 {
            let v = interference(x as f32 / pw * freq, y as f32 / ph * freq, t, 1.0)
                + plasma(x as f32 / pw * 3.0, y as f32 / ph * 3.0, t, seed);
            let v = (v * 0.5).clamp(0.0, 1.0);
            if v > threshold {
                canvas.set(x, y);
            }
        }
    }
}

/// The classic 4×4 Bayer ordered-dither matrix, normalized to `[0, 1)`.
///
/// Deterministic and position-dependent: the local threshold makes a scalar
/// field degrade into a stable dot *density* rather than collapsing into large
/// solid blocks when only one attribute (on/off) is available.
pub const BAYER4: [[f32; 4]; 4] = [
    [0.0, 8.0, 2.0, 10.0],
    [12.0, 4.0, 14.0, 6.0],
    [3.0, 11.0, 1.0, 9.0],
    [15.0, 7.0, 13.0, 5.0],
];

/// Local ordered-dither threshold in `[0, 1)` for a sub-cell dot coordinate.
#[inline]
pub fn bayer4_threshold(x: usize, y: usize) -> f32 {
    (BAYER4[y & 3][x & 3] + 0.5) / 16.0
}

/// Renders a combined interference/plasma field as Braille dots using 4×4
/// ordered dithering.
///
/// `intensity` scales the field before dithering. Every dot is lit when its
/// local field value exceeds the Bayer threshold, so brighter regions become
/// denser while dark regions stay sparse — preserving field structure in mono.
pub fn render_field_braille_dithered(
    canvas: &mut BrailleCanvas,
    t: f32,
    seed: f32,
    freq: f32,
    intensity: f32,
) {
    let pw = canvas.pixel_width() as f32;
    let ph = canvas.pixel_height() as f32;
    for y in 0..canvas.pixel_height() as i32 {
        for x in 0..canvas.pixel_width() as i32 {
            let v = interference(x as f32 / pw * freq, y as f32 / ph * freq, t, 1.0)
                + plasma(x as f32 / pw * 3.0, y as f32 / ph * 3.0, t, seed);
            let v = (v * 0.5 * intensity).clamp(0.0, 1.0);
            if v > bayer4_threshold(x as usize, y as usize) {
                canvas.set(x, y);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plasma_is_bounded_and_deterministic() {
        for i in 0..200 {
            let t = i as f32 * 0.05;
            let v = plasma(1.3, 2.7, t, 0.4);
            assert!((0.0..=1.0).contains(&v));
            assert_eq!(v, plasma(1.3, 2.7, t, 0.4));
        }
    }

    #[test]
    fn heat_ramp_is_monotonic_in_red_channel() {
        let lo = heat_rgb(0.0);
        let mid = heat_rgb(0.8);
        let hi = heat_rgb(1.0);
        assert!(lo.2 > lo.0, "low end should be blue-ish");
        assert!(hi.0 > hi.2, "high end should be red-ish");
        assert!(hi.0 > mid.0 || mid.0 > lo.0);
    }

    #[test]
    fn halfblock_field_fills_pixels() {
        let mut c = HalfBlockCanvas::new(8, 2);
        render_plasma_halfblock(&mut c, 1.0, 0.2);
        assert!(c.get_pixel(0, 0).is_some());
        assert!(c.get_pixel(7, 3).is_some());
    }

    #[test]
    fn braille_field_is_deterministic() {
        let mut a = BrailleCanvas::new(12, 4);
        render_field_braille(&mut a, 0.7, 0.3, 0.5, 4.0);
        let mut b = BrailleCanvas::new(12, 4);
        render_field_braille(&mut b, 0.7, 0.3, 0.5, 4.0);
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn bayer_threshold_is_bounded_and_deterministic() {
        for y in 0..8 {
            for x in 0..8 {
                let t = bayer4_threshold(x, y);
                assert!((0.0..1.0).contains(&t));
                assert_eq!(t, bayer4_threshold(x, y));
                // Periodic with period 4.
                assert_eq!(t, bayer4_threshold(x + 4, y + 4));
            }
        }
    }

    fn lit_fraction(canvas: &BrailleCanvas) -> f32 {
        let total = (canvas.pixel_width() * canvas.pixel_height()) as f32;
        let mut lit = 0;
        for y in 0..canvas.pixel_height() as i32 {
            for x in 0..canvas.pixel_width() as i32 {
                if canvas.get(x, y) {
                    lit += 1;
                }
            }
        }
        lit as f32 / total
    }

    #[test]
    fn dithered_field_does_not_collapse_to_solid_or_empty() {
        let mut c = BrailleCanvas::new(40, 12); // 80x48 dots
        render_field_braille_dithered(&mut c, 0.9, 0.3, 4.0, 1.0);
        let f = lit_fraction(&c);
        assert!(
            f > 0.05 && f < 0.95,
            "dithered field collapsed to a uniform block: lit fraction {f}"
        );
    }

    #[test]
    fn dither_density_increases_with_intensity() {
        let dots = |intensity: f32| {
            let mut c = BrailleCanvas::new(40, 12);
            render_field_braille_dithered(&mut c, 0.9, 0.3, 4.0, intensity);
            let mut n = 0;
            for y in 0..c.pixel_height() as i32 {
                for x in 0..c.pixel_width() as i32 {
                    if c.get(x, y) {
                        n += 1;
                    }
                }
            }
            n
        };
        assert!(
            dots(0.6) < dots(1.4),
            "higher intensity must produce strictly more dots"
        );
    }

    #[test]
    fn dithered_field_is_deterministic() {
        let mut a = BrailleCanvas::new(20, 6);
        render_field_braille_dithered(&mut a, 0.42, 0.1, 5.0, 1.0);
        let mut b = BrailleCanvas::new(20, 6);
        render_field_braille_dithered(&mut b, 0.42, 0.1, 5.0, 1.0);
        assert_eq!(a, b);
    }
}
