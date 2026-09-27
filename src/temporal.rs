//! Experimental temporal cell modulation.
//!
//! This module is deliberately **not** a terminal protocol and does not change
//! [`crate::Cell`]. It generates ordinary [`crate::Surface`] frames whose glyph
//! masks vary over time. The first implementation is intentionally narrow:
//! phase-decorrelated sigma-delta over logical 2x4 subcell duty cycles.
//!
//! The practical target is residual temporal dithering (small corrections over a
//! good static frame), not general spatial super-resolution. Presentation cadence
//! cannot be observed through a PTY, so callers must provide an externally
//! measured [`PresentationProfile`] and fall back to static rendering when its
//! safety/quality gate fails.

use crate::cell::{Cell, Glyph, Style};
use crate::glyph::SubcellGlyphMode;
use crate::surface::Surface;

/// Measured properties of the terminal/display presentation path.
///
/// These are **presentation** measurements, not application emission settings.
/// LibGibson cannot infer them from PTY writes; use an external cadence beacon or
/// other measurement process and store the result as a profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentationProfile {
    /// Observed terminal presentation cadence.
    pub presentation_hz: f32,
    /// Fraction of emitted frames observed as distinct presented frames (0..=1).
    pub survival_rate: f32,
    /// 95th-percentile inter-presentation jitter, in milliseconds.
    pub jitter_p95_ms: f32,
    /// Whether this profile came from a real measurement rather than a guess.
    pub measured: bool,
}

impl PresentationProfile {
    pub const fn unmeasured() -> Self {
        Self {
            presentation_hz: 0.0,
            survival_rate: 0.0,
            jitter_p95_ms: 0.0,
            measured: false,
        }
    }

    pub fn measured(presentation_hz: f32, survival_rate: f32, jitter_p95_ms: f32) -> Self {
        Self {
            presentation_hz: presentation_hz.max(0.0),
            survival_rate: survival_rate.clamp(0.0, 1.0),
            jitter_p95_ms: jitter_p95_ms.max(0.0),
            measured: true,
        }
    }
}

impl Default for PresentationProfile {
    fn default() -> Self {
        Self::unmeasured()
    }
}

/// Conservative gate for residual **luminance** modulation.
///
/// Defaults encode the research-stage contract: never luminance-modulate an
/// unmeasured path; require a high presentation cadence and high frame survival;
/// cap requested linear-light modulation depth to 10%. This is an engineering
/// guardrail, not medical certification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemporalSafetyPolicy {
    pub min_luminance_hz: f32,
    pub min_survival_rate: f32,
    pub max_luminance_depth: f32,
}

impl Default for TemporalSafetyPolicy {
    fn default() -> Self {
        Self {
            min_luminance_hz: 100.0,
            min_survival_rate: 0.90,
            max_luminance_depth: 0.10,
        }
    }
}

/// Why an application should fall back to its static realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalGate {
    Enabled,
    ReducedMotion,
    Unmeasured,
    CadenceTooLow,
    SurvivalTooLow,
    DepthTooHigh,
}

impl TemporalSafetyPolicy {
    /// Evaluates whether residual luminance modulation should run.
    ///
    /// `requested_depth` is a 0..=1 **linear-light** luminance swing relative to
    /// the display's full range. Callers should pass `reduced_motion = true` for
    /// any accessibility/static policy that forbids temporal noise.
    pub fn gate_luminance(
        self,
        profile: PresentationProfile,
        requested_depth: f32,
        reduced_motion: bool,
    ) -> TemporalGate {
        if reduced_motion {
            return TemporalGate::ReducedMotion;
        }
        if !profile.measured {
            return TemporalGate::Unmeasured;
        }
        if !profile.presentation_hz.is_finite()
            || profile.presentation_hz < self.min_luminance_hz
        {
            return TemporalGate::CadenceTooLow;
        }
        if !profile.survival_rate.is_finite()
            || profile.survival_rate < self.min_survival_rate
        {
            return TemporalGate::SurvivalTooLow;
        }
        if !requested_depth.is_finite()
            || requested_depth < 0.0
            || requested_depth > self.max_luminance_depth
        {
            return TemporalGate::DepthTooHigh;
        }
        TemporalGate::Enabled
    }
}

/// Stateful temporal modulation over a grid of logical 2x4 subcell masks.
///
/// Each of the eight logical dots in each terminal cell stores a target duty
/// cycle in `0..=1`. [`TemporalBrailleField::advance`] runs one first-order
/// sigma-delta step per dot, packs the resulting mask through the selected
/// [`SubcellGlyphMode`], and returns an ordinary [`Surface`].
///
/// Accumulators receive deterministic per-dot starting phases so a uniform field
/// does not flash in perfect lockstep. This is phase decorrelation, not a claim
/// of a perceptually optimal spatiotemporal blue-noise-sequence.
#[derive(Debug, Clone)]
pub struct TemporalBrailleField {
    width: u16,
    height: u16,
    duty: Vec<[f32; 8]>,
    accumulator: Vec<[f32; 8]>,
    frame_index: u64,
}

impl TemporalBrailleField {
    pub fn new(width: u16, height: u16, seed: u64) -> Self {
        let cells = (width as usize).saturating_mul(height as usize);
        let mut accumulator = vec![[0.0; 8]; cells];
        for (cell_index, phases) in accumulator.iter_mut().enumerate() {
            for (dot, phase) in phases.iter_mut().enumerate() {
                *phase = unit_hash(seed, cell_index as u64, dot as u64);
            }
        }
        Self {
            width,
            height,
            duty: vec![[0.0; 8]; cells],
            accumulator,
            frame_index: 0,
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Sets all eight target subcell duty cycles for one terminal cell.
    ///
    /// Non-finite values become zero; finite values clamp to `0..=1`.
    pub fn set_cell_duty(&mut self, x: u16, y: u16, duty: [f32; 8]) -> bool {
        let Some(index) = self.index(x, y) else {
            return false;
        };
        self.duty[index] = duty.map(sanitize_duty);
        true
    }

    pub fn cell_duty(&self, x: u16, y: u16) -> Option<[f32; 8]> {
        self.index(x, y).map(|i| self.duty[i])
    }

    pub fn clear(&mut self) {
        self.duty.fill([0.0; 8]);
    }

    /// A deterministic static control: dots at or above 50% duty are on.
    pub fn static_surface(&self, style: Style, mode: SubcellGlyphMode) -> Surface {
        let masks = self
            .duty
            .iter()
            .map(|dots| {
                dots.iter()
                    .enumerate()
                    .fold(0u8, |mask, (bit, duty)| {
                        if *duty >= 0.5 {
                            mask | (1u8 << bit)
                        } else {
                            mask
                        }
                    })
            })
            .collect();
        self.surface_from_masks(masks, style, mode)
    }

    /// Advances one temporal phase and returns one ordinary LibGibson surface.
    pub fn advance(&mut self, style: Style, mode: SubcellGlyphMode) -> Surface {
        let masks = self.advance_masks();
        self.surface_from_masks(masks, style, mode)
    }

    fn advance_masks(&mut self) -> Vec<u8> {
        let mut masks = Vec::with_capacity(self.duty.len());
        for (targets, accumulators) in self.duty.iter().zip(self.accumulator.iter_mut()) {
            let mut mask = 0u8;
            for bit in 0..8 {
                let target = targets[bit];
                let acc = &mut accumulators[bit];
                *acc += target;
                if *acc >= 1.0 {
                    mask |= 1u8 << bit;
                    *acc -= 1.0;
                }
            }
            masks.push(mask);
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        masks
    }

    fn surface_from_masks(
        &self,
        masks: Vec<u8>,
        style: Style,
        mode: SubcellGlyphMode,
    ) -> Surface {
        let mut surface = Surface::new(self.width, self.height);
        if self.width == 0 {
            return surface;
        }
        for (index, mask) in masks.into_iter().enumerate() {
            let x = (index % self.width as usize) as u16;
            let y = (index / self.width as usize) as u16;
            let cell = match mode.subcell_glyph(mask) {
                Some(ch) => Cell::new(Glyph::from_char(ch), style),
                None => Cell::space(style),
            };
            surface.set_cell(x, y, cell);
        }
        surface
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y as usize) * self.width as usize + x as usize)
        } else {
            None
        }
    }
}

fn sanitize_duty(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Deterministic SplitMix-style hash mapped to `[0, 1)`.
fn unit_hash(seed: u64, cell: u64, dot: u64) -> f32 {
    let mut z = seed
        ^ cell.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ dot.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 40) as f32) / ((1u32 << 24) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conservative_gate_rejects_unmeasured_low_rate_and_excess_depth() {
        let policy = TemporalSafetyPolicy::default();
        assert_eq!(
            policy.gate_luminance(PresentationProfile::unmeasured(), 0.02, false),
            TemporalGate::Unmeasured
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(60.0, 1.0, 0.1), 0.02, false),
            TemporalGate::CadenceTooLow
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(120.0, 0.80, 0.1), 0.02, false),
            TemporalGate::SurvivalTooLow
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(120.0, 0.99, 0.1), 0.20, false),
            TemporalGate::DepthTooHigh
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(120.0, 0.99, 0.1), 0.02, true),
            TemporalGate::ReducedMotion
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(120.0, 0.99, 0.1), 0.02, false),
            TemporalGate::Enabled
        );
    }

    #[test]
    fn sigma_delta_mean_converges_to_requested_duty() {
        let mut field = TemporalBrailleField::new(1, 1, 7);
        assert!(field.set_cell_duty(
            0,
            0,
            [0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        ));
        let mut on = 0usize;
        let frames = 400usize;
        for _ in 0..frames {
            let mask = field.advance_masks()[0];
            if mask & 1 != 0 {
                on += 1;
            }
        }
        assert!(
            (on as isize - 100).abs() <= 1,
            "25% duty should converge to 100/400 on frames, got {on}"
        );
    }

    #[test]
    fn zero_and_full_duty_are_temporally_stable() {
        let mut field = TemporalBrailleField::new(1, 1, 11);
        assert!(field.set_cell_duty(
            0,
            0,
            [0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]
        ));
        for _ in 0..64 {
            let mask = field.advance_masks()[0];
            assert_eq!(mask, 0b1010_1010);
        }
    }

    #[test]
    fn seeded_phases_decorrelate_uniform_half_duty_field() {
        let mut field = TemporalBrailleField::new(8, 4, 0xC0FFEE);
        for y in 0..field.height() {
            for x in 0..field.width() {
                field.set_cell_duty(x, y, [0.5; 8]);
            }
        }
        let masks = field.advance_masks();
        assert!(
            masks.windows(2).any(|w| w[0] != w[1]),
            "uniform targets should not start every cell in the same temporal phase"
        );
    }

    #[test]
    fn advance_emits_ordinary_surface_cells() {
        let mut field = TemporalBrailleField::new(2, 1, 1);
        field.set_cell_duty(0, 0, [1.0; 8]);
        field.set_cell_duty(1, 0, [0.0; 8]);
        let surface = field.advance(Style::default(), SubcellGlyphMode::Braille2x4);
        assert_eq!(surface.width, 2);
        assert_eq!(surface.height, 1);
        assert_ne!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), " ");
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), " ");
    }
}
