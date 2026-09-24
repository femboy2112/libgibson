//! Experimental software effects on opaque RGB subpixels, before cell realization.
//!
//! These operations blend RGB **inside** a temporary software raster; they do not
//! add alpha to terminal cells. `RasterFx` chains are ordered endomorphisms: the
//! empty chain is identity, and `[a, b]` applies `a` then `b`. They generally do
//! not commute. `FeedbackBuffer` is separate, explicit timeline state.

use std::time::Duration;

use crate::raster::{Rgb, RgbRaster, MAX_RASTER_DIMENSION};

/// A small, bounded RGB post-process. Invalid floating parameters are no-ops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RasterFx {
    /// Add a box-filtered bright pass. Radius is capped at three pixels and
    /// strength at four. Pixels below `threshold` contribute no light.
    Glow {
        radius: u8,
        threshold: u8,
        strength: f32,
    },
    /// Sample red at `x-offset`, green at `x`, blue at `x+offset`.
    /// Out-of-bounds samples clamp to the nearest image edge.
    ChromaticSplit { offset: i16 },
    /// Horizontal row displacement in pixels. Frequency is radians per row;
    /// phase is explicit visual time, never an internal clock.
    SineWarp {
        amplitude: f32,
        frequency: f32,
        phase: f32,
    },
    /// Darken toward the corners; strength is clamped to `[0, 1]`.
    Vignette { strength: f32 },
    /// Darken odd subpixel rows; strength is clamped to `[0, 1]`.
    Scanlines { strength: f32 },
}

impl RasterFx {
    /// Apply a chain using one temporary source buffer. Reuse
    /// [`RasterFxWorkspace`] to retain that allocation across frames.
    pub fn apply_chain(raster: &mut RgbRaster, effects: &[Self]) {
        RasterFxWorkspace::default().apply(raster, effects);
    }
}

/// Reusable source storage shared by every pass of an ordered effect chain.
#[derive(Default, Debug)]
pub struct RasterFxWorkspace {
    scratch: Vec<Rgb>,
}

impl RasterFxWorkspace {
    /// Apply effects in slice order. Memory is bounded by one input raster;
    /// all loops are bounded by dimensions and (for glow) a maximum 7×7 kernel.
    pub fn apply(&mut self, raster: &mut RgbRaster, effects: &[RasterFx]) {
        let w = usize::from(raster.width());
        let h = usize::from(raster.height());
        if w == 0 || h == 0 || effects.is_empty() {
            return;
        }
        self.scratch.resize(w * h, (0, 0, 0));
        for effect in effects {
            self.scratch.copy_from_slice(raster.pixels());
            let sample = |x: i64, y: usize| self.scratch[y * w + x.clamp(0, w as i64 - 1) as usize];
            match *effect {
                RasterFx::Glow {
                    radius,
                    threshold,
                    strength,
                } if strength.is_finite() && strength > 0.0 && radius > 0 => {
                    let r = usize::from(radius.min(3));
                    let strength = strength.min(4.0);
                    for y in 0..h {
                        for x in 0..w {
                            let mut sum = [0u32; 3];
                            let mut count = 0u32;
                            for sy in y.saturating_sub(r)..=(y + r).min(h - 1) {
                                for sx in x.saturating_sub(r)..=(x + r).min(w - 1) {
                                    let c = self.scratch[sy * w + sx];
                                    if c.0.max(c.1).max(c.2) >= threshold {
                                        sum[0] += u32::from(c.0);
                                        sum[1] += u32::from(c.1);
                                        sum[2] += u32::from(c.2);
                                    }
                                    count += 1;
                                }
                            }
                            let c = self.scratch[y * w + x];
                            let add = |base: u8, light: u32| {
                                (f32::from(base) + light as f32 / count as f32 * strength)
                                    .round()
                                    .min(255.0) as u8
                            };
                            raster.pixels_mut()[y * w + x] =
                                (add(c.0, sum[0]), add(c.1, sum[1]), add(c.2, sum[2]));
                        }
                    }
                }
                RasterFx::ChromaticSplit { offset } => {
                    for y in 0..h {
                        for x in 0..w {
                            raster.pixels_mut()[y * w + x] = (
                                sample(x as i64 - i64::from(offset), y).0,
                                sample(x as i64, y).1,
                                sample(x as i64 + i64::from(offset), y).2,
                            );
                        }
                    }
                }
                RasterFx::SineWarp {
                    amplitude,
                    frequency,
                    phase,
                } if amplitude.is_finite() && frequency.is_finite() && phase.is_finite() => {
                    // f64 keeps products of hostile but finite f32 inputs finite.
                    let amplitude = f64::from(amplitude).clamp(-(w as f64), w as f64);
                    for y in 0..h {
                        let shift = (amplitude
                            * (y as f64 * f64::from(frequency) + f64::from(phase)).sin())
                        .round() as i64;
                        for x in 0..w {
                            raster.pixels_mut()[y * w + x] = sample(x as i64 - shift, y);
                        }
                    }
                }
                RasterFx::Vignette { strength } if strength.is_finite() => {
                    let strength = strength.clamp(0.0, 1.0);
                    for y in 0..h {
                        for x in 0..w {
                            let nx = (2.0 * (x as f32 + 0.5) / w as f32 - 1.0).abs();
                            let ny = (2.0 * (y as f32 + 0.5) / h as f32 - 1.0).abs();
                            let gain = 1.0 - strength * (nx * nx + ny * ny) * 0.5;
                            raster.pixels_mut()[y * w + x] = scale(self.scratch[y * w + x], gain);
                        }
                    }
                }
                RasterFx::Scanlines { strength } if strength.is_finite() => {
                    let gain = 1.0 - strength.clamp(0.0, 1.0);
                    for y in (1..h).step_by(2) {
                        for x in 0..w {
                            raster.pixels_mut()[y * w + x] = scale(self.scratch[y * w + x], gain);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn scale(c: Rgb, gain: f32) -> Rgb {
    let channel = |v: u8| (f32::from(v) * gain).round().clamp(0.0, 255.0) as u8;
    (channel(c.0), channel(c.1), channel(c.2))
}

/// Explicit deterministic visual history: `history * 2^(-dt/half_life) + input`.
///
/// Input is an emission per update, not a per-second rate. Replay requires the
/// same ordered rasters and durations; splitting a step is not equivalent.
/// High-precision history avoids 8-bit rounding leaving immortal faint trails.
/// RGB energy is saturated at 255 per channel. No terminal alpha is involved.
///
/// Dimensions share [`MAX_RASTER_DIMENSION`], a hard allocation bound, not a
/// recommended operating size. At 2048 × 2048, floating energy uses 96 MiB and
/// RGB output another 12 MiB (about 108 MiB total, excluding allocator overhead).
/// Normal terminal rasters are orders of magnitude smaller.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedbackBuffer {
    output: RgbRaster,
    energy: Vec<[f64; 3]>,
    half_life: Duration,
    updates: u64,
}

impl FeedbackBuffer {
    /// Create black history at raster pixel dimensions. A zero half-life
    /// discards all previous history on each positive-duration update.
    pub fn new(width: u16, height: u16, half_life: Duration) -> Self {
        let output = RgbRaster::new(width, height);
        let energy = vec![[0.0; 3]; output.pixels().len()];
        Self {
            output,
            energy,
            half_life,
            updates: 0,
        }
    }

    /// Clear history and reset the update counter, preserving size and decay.
    pub fn reset(&mut self) {
        self.output.clear((0, 0, 0));
        self.energy.fill([0.0; 3]);
        self.updates = 0;
    }

    /// Change effective dimensions and clear history. Requests are clamped to
    /// [`MAX_RASTER_DIMENSION`]; unchanged effective dimensions preserve history.
    pub fn resize(&mut self, width: u16, height: u16) {
        let width = width.min(MAX_RASTER_DIMENSION);
        let height = height.min(MAX_RASTER_DIMENSION);
        if self.output.width() == width && self.output.height() == height {
            return;
        }
        *self = Self::new(width, height, self.half_life);
    }

    /// Advance one explicit timeline step. Zero duration is exact identity,
    /// even if input dimensions differ. Positive steps resize/clear history
    /// to match input before decaying and adding its emission.
    pub fn update(&mut self, dt: Duration, input: &RgbRaster) -> &RgbRaster {
        if dt.is_zero() {
            return &self.output;
        }
        self.resize(input.width(), input.height());
        let decay = if self.half_life.is_zero() {
            0.0
        } else {
            (-dt.as_secs_f64() / self.half_life.as_secs_f64()).exp2()
        };
        for ((energy, &c), out) in self
            .energy
            .iter_mut()
            .zip(input.pixels())
            .zip(self.output.pixels_mut())
        {
            let input = [c.0, c.1, c.2];
            for i in 0..3 {
                energy[i] = (energy[i] * decay + f64::from(input[i])).min(255.0);
            }
            *out = (
                energy[0].round() as u8,
                energy[1].round() as u8,
                energy[2].round() as u8,
            );
        }
        self.updates = self.updates.saturating_add(1);
        &self.output
    }

    /// Current realized history; reading it never advances the timeline.
    pub fn raster(&self) -> &RgbRaster {
        &self.output
    }

    /// Positive-duration updates since construction/reset/resize.
    pub fn updates(&self) -> u64 {
        self.updates
    }
}

/// Smooth radial falloff `exp(-distance²/radius²)` around the origin.
/// Non-finite coordinates or a nonpositive/non-finite radius produce zero.
pub fn radial_glow(x: f32, y: f32, radius: f32) -> f32 {
    if !finite(&[x, y, radius]) || radius <= 0.0 {
        return 0.0;
    }
    let (x, y, r) = (f64::from(x), f64::from(y), f64::from(radius));
    (-(x * x + y * y) / (r * r)).exp() as f32
}

/// Sum inverse-square sources `(center_x, center_y, strength)`.
///
/// Coordinates are caller-defined (usually normalized image space). A small
/// squared-distance epsilon prevents singularities. Invalid/negative sources
/// are ignored and output is capped at 1,000,000 to remain safely finite.
pub fn metaballs(x: f32, y: f32, sources: &[(f32, f32, f32)]) -> f32 {
    if !finite(&[x, y]) {
        return 0.0;
    }
    let mut sum = 0.0f64;
    for &(cx, cy, strength) in sources {
        if !finite(&[cx, cy, strength]) || strength <= 0.0 {
            continue;
        }
        let dx = f64::from(x) - f64::from(cx);
        let dy = f64::from(y) - f64::from(cy);
        sum = (sum + f64::from(strength) / (dx * dx + dy * dy + 0.0001)).min(1_000_000.0);
    }
    sum as f32
}

/// Spiral interference in `[0, 1]`; `arms` controls angular frequency and
/// explicit `time` advances its phase. Invalid inputs return zero.
pub fn vortex(x: f32, y: f32, time: f32, arms: f32) -> f32 {
    if !finite(&[x, y, time, arms]) {
        return 0.0;
    }
    let (x, y) = (f64::from(x), f64::from(y));
    ((y.atan2(x) * f64::from(arms) + x.hypot(y) * 8.0 - f64::from(time)).sin() * 0.5 + 0.5) as f32
}

/// Gaussian ring centered on the origin. Radius must be nonnegative, width
/// strictly positive; invalid inputs return zero. Output is in `[0, 1]`.
pub fn ring(x: f32, y: f32, radius: f32, width: f32) -> f32 {
    if !finite(&[x, y, radius, width]) || radius < 0.0 || width <= 0.0 {
        return 0.0;
    }
    let d = (f64::from(x).hypot(f64::from(y)) - f64::from(radius)) / f64::from(width);
    (-d * d).exp() as f32
}

/// Linear two-color palette, clamping finite values to `[0, 1]`.
/// Non-finite values select the low endpoint.
pub fn palette(value: f32, low: Rgb, high: Rgb) -> Rgb {
    let t = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    (mix(low.0, high.0), mix(low.1, high.1), mix(low.2, high.2))
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}
