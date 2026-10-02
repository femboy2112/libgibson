//! Synth **patch** descriptors — declarative voice recipes the synthesizer turns into
//! live DSP voices. A patch is a small subtractive/FM synth: an oscillator (optionally
//! detuned unison + a sub octave), a resonant filter with envelope modulation, an ADSR,
//! and gain/pan. No samples; every timbre is these knobs.

use super::super::dsp::osc::Wave;

/// The oscillator core of a patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OscKind {
    /// A classic waveform.
    Shape(Wave),
    /// 2-operator FM: modulator:carrier ratio and modulation index.
    Fm { ratio: f32, index: f32 },
}

/// A synth voice recipe.
#[derive(Debug, Clone, Copy)]
pub struct Patch {
    /// Oscillator core.
    pub osc: OscKind,
    /// Add a sine one octave below (body for bass/pads).
    pub sub: bool,
    /// Unison voice count (1 = mono); >1 detunes for width.
    pub unison: u8,
    /// Unison detune spread in cents.
    pub detune_cents: f32,
    /// Filter cutoff base (Hz).
    pub cutoff_hz: f32,
    /// Envelope → cutoff modulation depth (Hz).
    pub cutoff_env: f32,
    /// Filter resonance `[0,1]`.
    pub resonance: f32,
    /// ADSR (attack, decay, sustain, release) in seconds / level.
    pub adsr: (f32, f32, f32, f32),
    /// Output gain.
    pub gain: f32,
    /// Static pan `[-1,1]`.
    pub pan: f32,
}

impl Patch {
    /// A plain sine — a safe default.
    pub fn sine() -> Patch {
        Patch {
            osc: OscKind::Shape(Wave::Sine),
            sub: false,
            unison: 1,
            detune_cents: 0.0,
            cutoff_hz: 8000.0,
            cutoff_env: 0.0,
            resonance: 0.1,
            adsr: (0.005, 0.1, 0.7, 0.2),
            gain: 0.7,
            pan: 0.0,
        }
    }
}
