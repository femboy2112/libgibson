//! Amplitude/modulation envelopes: a gated [`Adsr`] and a one-shot [`ExpDecay`].
//!
//! Times are in seconds and internally converted to per-sample coefficients, so an
//! envelope's shape is sample-rate independent. Outputs are click-safe (continuous) as
//! long as the gate is toggled while the envelope is running.

// `next()` is the deliberate per-sample generator idiom shared by every DSP voice.
#![allow(clippy::should_implement_trait)]

/// ADSR stage.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// A linear-attack, exponential-decay/release ADSR envelope.
#[derive(Debug, Clone)]
pub struct Adsr {
    sr: f32,
    stage: Stage,
    level: f32,
    // Per-sample increments / coefficients.
    attack_rate: f32,
    decay_coef: f32,
    sustain: f32,
    release_coef: f32,
}

impl Adsr {
    /// A new envelope at sample rate `sr`, defaulting to a short pluck.
    pub fn new(sr: f32) -> Adsr {
        let mut a = Adsr {
            sr: sr.max(1.0),
            stage: Stage::Idle,
            level: 0.0,
            attack_rate: 0.0,
            decay_coef: 0.0,
            sustain: 0.8,
            release_coef: 0.0,
        };
        a.set(0.005, 0.08, 0.7, 0.15);
        a
    }

    /// Set attack/decay/release times (seconds) and sustain level `[0,1]`.
    pub fn set(&mut self, attack: f32, decay: f32, sustain: f32, release: f32) {
        let a = attack.max(1e-4);
        self.attack_rate = 1.0 / (a * self.sr);
        self.sustain = sustain.clamp(0.0, 1.0);
        self.decay_coef = time_to_coef(decay, self.sr);
        self.release_coef = time_to_coef(release, self.sr);
    }

    /// Trigger the attack stage.
    #[inline]
    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
    }

    /// Enter the release stage.
    #[inline]
    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    /// True while the envelope is producing meaningful (non-negligible) output.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    /// Next amplitude in `[0, 1]`.
    #[inline]
    pub fn next(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {
                self.level = 0.0;
            }
            Stage::Attack => {
                self.level += self.attack_rate;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                // Exponential approach to the sustain level.
                self.level = self.sustain + (self.level - self.sustain) * self.decay_coef;
                if (self.level - self.sustain).abs() < 1e-4 {
                    self.level = self.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {
                self.level = self.sustain;
            }
            Stage::Release => {
                self.level *= self.release_coef;
                if self.level < 1e-4 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}

/// A one-shot exponential decay from a start value toward zero — for percussive amplitude
/// and pitch envelopes.
#[derive(Debug, Clone)]
pub struct ExpDecay {
    sr: f32,
    value: f32,
    coef: f32,
    active: bool,
}

impl ExpDecay {
    /// A new (idle) decay at sample rate `sr`.
    pub fn new(sr: f32) -> ExpDecay {
        ExpDecay {
            sr: sr.max(1.0),
            value: 0.0,
            coef: 0.0,
            active: false,
        }
    }

    /// Start decaying from `from`, reaching ~37% after `tau_secs` (the time constant).
    #[inline]
    pub fn trigger(&mut self, from: f32, tau_secs: f32) {
        self.value = from;
        // Per-sample multiplier for exp(-1) after tau seconds.
        let tau = tau_secs.max(1e-4);
        self.coef = (-1.0 / (tau * self.sr)).exp();
        self.active = from.abs() > 1e-5;
    }

    /// Current value; advances one sample.
    #[inline]
    pub fn next(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }
        let v = self.value;
        self.value *= self.coef;
        // Free the voice at ~-60 dB: inaudible in a mix, and keeps percussive tails from
        // holding a voice slot for the many extra time-constants it takes to reach -100 dB.
        if self.value.abs() < 1e-3 {
            self.active = false;
            self.value = 0.0;
        }
        v
    }

    /// Whether the decay is still producing output.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Per-sample exponential coefficient reaching ~1% error after `t` seconds.
#[inline]
fn time_to_coef(t: f32, sr: f32) -> f32 {
    let t = t.max(1e-4);
    (-1.0 / (t * sr) * 4.6).exp() // ~99% settled after t seconds (ln(0.01) ≈ -4.6)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 48_000.0;

    #[test]
    fn adsr_rises_sustains_and_releases_bounded() {
        let mut e = Adsr::new(SR);
        e.set(0.01, 0.05, 0.5, 0.05);
        e.gate_on();
        let mut max = 0.0f32;
        // 0.02s attack+decay window.
        for _ in 0..(SR as usize / 50) {
            let v = e.next();
            assert!((0.0..=1.0).contains(&v));
            max = max.max(v);
        }
        assert!(max > 0.9, "attack should reach near 1.0, got {max}");
        // Let it settle to sustain.
        for _ in 0..(SR as usize / 5) {
            e.next();
        }
        let s = e.next();
        assert!((s - 0.5).abs() < 0.05, "sustain {s} not near 0.5");
        e.gate_off();
        for _ in 0..(SR as usize) {
            e.next();
        }
        assert!(!e.is_active(), "envelope should be idle after release");
        assert!(e.next() <= 1e-3);
    }

    #[test]
    fn exp_decay_monotone_and_terminates() {
        let mut d = ExpDecay::new(SR);
        d.trigger(1.0, 0.05);
        let mut prev = f32::INFINITY;
        let mut steps = 0;
        while d.is_active() {
            let v = d.next();
            assert!(v.is_finite() && v >= 0.0);
            assert!(v <= prev + 1e-6, "decay must be monotone non-increasing");
            prev = v;
            steps += 1;
            assert!(steps < SR as usize * 4, "decay never terminated");
        }
        assert!(steps > 100);
    }

    #[test]
    fn idle_envelopes_are_silent() {
        let mut e = Adsr::new(SR);
        assert_eq!(e.next(), 0.0);
        let mut d = ExpDecay::new(SR);
        assert_eq!(d.next(), 0.0);
    }
}
