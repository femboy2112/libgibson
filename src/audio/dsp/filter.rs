//! A TPT (topology-preserving transform) state-variable filter — the Zavalishin/Cytomic
//! SVF. Unconditionally stable across the whole audio range and modulatable per sample
//! without blowing up, which is exactly what a synth voice's moving cutoff needs.
//!
//! One filter yields low-, high- and band-pass simultaneously ([`Svf::process_modes`]);
//! [`Svf::process`] returns the low-pass, the common voice use.

/// Low/high/band-pass outputs of one SVF sample.
#[derive(Debug, Clone, Copy)]
pub struct SvfOut {
    /// Low-pass output.
    pub lp: f32,
    /// High-pass output.
    pub hp: f32,
    /// Band-pass output.
    pub bp: f32,
}

/// A state-variable filter.
#[derive(Debug, Clone)]
pub struct Svf {
    sr: f32,
    // Coefficients (recomputed on set).
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
    // State (the two integrator memories).
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// A new filter at sample rate `sr`, defaulting to a gently-damped low-pass at ~1 kHz.
    pub fn new(sr: f32) -> Svf {
        let mut f = Svf {
            sr: sr.max(1.0),
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            k: 1.0,
            ic1: 0.0,
            ic2: 0.0,
        };
        f.set(1000.0, 0.2);
        f
    }

    /// Set cutoff (Hz) and resonance in `[0, 1]` (0 = maximally damped, →1 = highly
    /// resonant). Cutoff is clamped just below Nyquist.
    pub fn set(&mut self, cutoff_hz: f32, resonance: f32) {
        let fc = cutoff_hz.clamp(10.0, self.sr * 0.49);
        let g = (std::f32::consts::PI * fc / self.sr).tan();
        // Map resonance -> damping k = 1/Q. k=2 (Q≈0.5) at res 0, k→0.04 (very resonant).
        let k = 2.0 * (1.0 - resonance.clamp(0.0, 0.98));
        let a1 = 1.0 / (1.0 + g * (g + k));
        self.a1 = a1;
        self.a2 = g * a1;
        self.a3 = g * self.a2;
        self.k = k;
    }

    /// Reset the filter state (clears ringing).
    #[inline]
    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }

    /// Process one sample, returning all three responses.
    #[inline]
    pub fn process_modes(&mut self, v0: f32) -> SvfOut {
        let v3 = v0 - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut {
            lp: v2,
            hp: v0 - self.k * v1 - v2,
            bp: v1,
        }
    }

    /// Process one sample, returning the low-pass output.
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.process_modes(x).lp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;
    const SR: f32 = 48_000.0;

    fn rms_of_sine_through(cutoff: f32, res: f32, freq: f32) -> f32 {
        let mut f = Svf::new(SR);
        f.set(cutoff, res);
        let n = 4800;
        let mut acc = 0.0f64;
        // Warm up to steady state, then measure.
        for i in 0..(n * 2) {
            let x = (TAU * freq * i as f32 / SR).sin();
            let y = f.process(x);
            if i >= n {
                acc += (y as f64) * (y as f64);
            }
        }
        (acc / n as f64).sqrt() as f32
    }

    #[test]
    fn stays_finite_under_extreme_modulation() {
        let mut f = Svf::new(SR);
        for i in 0..48_000 {
            // Sweep cutoff wildly every sample; TPT must stay stable.
            f.set(20.0 + (i % 20000) as f32, 0.95);
            let y = f.process(if i % 2 == 0 { 1.0 } else { -1.0 });
            assert!(y.is_finite(), "SVF diverged at sample {i}");
        }
    }

    #[test]
    fn lowpass_passes_lows_and_attenuates_highs() {
        // Low-pass at 500 Hz: a 100 Hz tone should pass mostly, a 10 kHz tone be crushed.
        let low = rms_of_sine_through(500.0, 0.1, 100.0);
        let high = rms_of_sine_through(500.0, 0.1, 10_000.0);
        assert!(low > 0.5, "100Hz through 500Hz LP too weak: {low}");
        assert!(high < 0.1, "10kHz through 500Hz LP not attenuated: {high}");
        assert!(low > high * 5.0);
    }

    #[test]
    fn highpass_attenuates_lows() {
        let mut f = Svf::new(SR);
        f.set(2000.0, 0.1);
        let n = 4800;
        let mut acc = 0.0f64;
        for i in 0..(n * 2) {
            let x = (TAU * 100.0 * i as f32 / SR).sin();
            let hp = f.process_modes(x).hp;
            if i >= n {
                acc += (hp as f64) * (hp as f64);
            }
        }
        let rms = (acc / n as f64).sqrt() as f32;
        assert!(rms < 0.15, "100Hz through 2kHz HP not attenuated: {rms}");
    }

    #[test]
    fn resonance_boosts_energy_near_cutoff() {
        let flat = rms_of_sine_through(1000.0, 0.05, 1000.0);
        let resonant = rms_of_sine_through(1000.0, 0.9, 1000.0);
        assert!(resonant > flat, "resonance should boost at cutoff: {resonant} vs {flat}");
        assert!(resonant.is_finite());
    }
}
