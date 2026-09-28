//! Deterministic chroma key + alpha compositing primitives for the keyed-video
//! moonshot. Pure arithmetic over `[u8; 3]` samples — no `gibson` dependency, no
//! I/O, no floating hazards (every path clamps and produces finite output) — so it
//! is exhaustively unit-testable without a terminal, a clip, or ffmpeg.
//!
//! The key is intentionally simple: a greenscreen background is *transparency*,
//! not content, so we discard it and composite the keyed foreground over a
//! separately rendered LibGibson background. This module makes NO perceptual claim;
//! it is a robust, tunable, reproducible key for terminal compositing.
//!
//! The green-ness metric is `G - max(R, B)`: large and positive on saturated green
//! (measured ~250 on this clip's clean field), small on non-green foreground
//! (< 40 on the subject). Alpha ramps linearly between two thresholds so the matte
//! edge is soft rather than a hard 1-bit cut.

#![allow(dead_code)] // Shared by the example binary and several test targets; not

// every target exercises every helper.

/// Tunable key parameters. `thr_lo`/`thr_hi` bracket the soft matte edge in
/// green-ness units (`G - max(R, B)`); `despill` toggles green-fringe suppression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyParams {
    /// At or below this green-ness the pixel is fully opaque foreground (alpha 1).
    pub thr_lo: f32,
    /// At or above this green-ness the pixel is fully keyed background (alpha 0).
    pub thr_hi: f32,
    /// Suppress residual green cast on kept foreground by clamping G to max(R, B).
    pub despill: bool,
}

impl Default for KeyParams {
    fn default() -> Self {
        // Measured on the source clip: clean key green-ness ~250, foreground < 40.
        // A 30..90 ramp keeps the whole subject opaque while softening the matte.
        Self {
            thr_lo: 30.0,
            thr_hi: 90.0,
            despill: true,
        }
    }
}

/// The green-ness of a pixel: `G - max(R, B)`. Positive on green, <= 0 on neutral
/// or warm content. Returned as `f32` so the alpha ramp is continuous.
#[inline]
pub fn greenness(rgb: [u8; 3]) -> f32 {
    let r = rgb[0] as f32;
    let g = rgb[1] as f32;
    let b = rgb[2] as f32;
    g - r.max(b)
}

/// Foreground coverage in `0.0..=1.0`: 1 where the pixel is opaque subject, 0 where
/// it is pure key. Monotonically non-increasing in green-ness, and always finite
/// (the divisor is guarded so a degenerate `thr_hi <= thr_lo` collapses to a hard
/// 1-bit cut at `thr_lo` rather than dividing by zero).
#[inline]
pub fn alpha_of(rgb: [u8; 3], p: &KeyParams) -> f32 {
    let g = greenness(rgb);
    let span = p.thr_hi - p.thr_lo;
    if span <= f32::EPSILON {
        return if g <= p.thr_lo { 1.0 } else { 0.0 };
    }
    ((p.thr_hi - g) / span).clamp(0.0, 1.0)
}

/// Suppress green spill by clamping the green channel to at most `max(R, B)`. This
/// removes the green fringe compression leaves around a matte edge without touching
/// pixels that have no green excess. A no-op when `despill` is disabled.
#[inline]
pub fn despill(rgb: [u8; 3], enabled: bool) -> [u8; 3] {
    if !enabled {
        return rgb;
    }
    let cap = rgb[0].max(rgb[2]);
    [rgb[0], rgb[1].min(cap), rgb[2]]
}

/// The keyed foreground: despilled color plus its alpha. `alpha == 0.0` means the
/// pixel is background and the color is irrelevant to the final composite.
#[inline]
pub fn key_pixel(rgb: [u8; 3], p: &KeyParams) -> ([u8; 3], f32) {
    (despill(rgb, p.despill), alpha_of(rgb, p))
}

/// Alpha-over composite of `fg` on `bg` at coverage `alpha` (`0.0..=1.0`, clamped).
/// `alpha == 0` returns `bg` exactly; `alpha == 1` returns `fg` exactly. Rounds to
/// the nearest byte so the result is stable and reversible at the endpoints.
#[inline]
pub fn composite_over(fg: [u8; 3], alpha: f32, bg: [u8; 3]) -> [u8; 3] {
    let a = alpha.clamp(0.0, 1.0);
    let ia = 1.0 - a;
    let mix = |f: u8, b: u8| -> u8 { (f as f32 * a + b as f32 * ia + 0.5) as u8 };
    [mix(fg[0], bg[0]), mix(fg[1], bg[1]), mix(fg[2], bg[2])]
}

/// An inclusive-exclusive pixel rectangle in some sample grid. `w`/`h` are the
/// extent; an empty box has `w == 0 || h == 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bbox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Bbox {
    /// The empty box at the origin.
    pub const EMPTY: Bbox = Bbox {
        x: 0,
        y: 0,
        w: 0,
        h: 0,
    };

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    #[inline]
    pub fn right(&self) -> u32 {
        self.x + self.w
    }

    #[inline]
    pub fn bottom(&self) -> u32 {
        self.y + self.h
    }

    /// The smallest box covering both. The union with an empty box is the other
    /// box, so a first frame (no previous bbox) reprojects only its own region.
    pub fn union(&self, other: &Bbox) -> Bbox {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = self.right().max(other.right());
        let y1 = self.bottom().max(other.bottom());
        Bbox {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
}
