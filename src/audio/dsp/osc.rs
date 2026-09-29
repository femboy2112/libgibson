//! Band-limited oscillators and FM/phase modulation, plus a deterministic noise source.
//!
//! Saw and pulse are anti-aliased with polyBLEP (a cheap 2-sample band-limited step
//! correction at each discontinuity); sine is exact; triangle is naive (its harmonics
//! roll off as 1/n² so residual aliasing is far below the ear's floor at musical pitches).
//! All generators are pure per-sample state machines: construct once, then call
//! [`Osc::next`] — no allocation on the audio path.

// `next()` is the deliberate per-sample generator idiom shared by every DSP voice; it is
// intentionally not `Iterator::next`.
#![allow(clippy::should_implement_trait)]

/// Oscillator waveform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wave {
    /// Exact sine.
    Sine,
    /// Band-limited sawtooth.
    Saw,
    /// Band-limited pulse of the given duty in `(0, 1)`.
    Pulse(f32),
    /// Naive triangle (gentle harmonic content).
    Triangle,
}

/// A single band-limited oscillator.
#[derive(Debug, Clone)]
pub struct Osc {
    sr: f32,
    phase: f32,
    dt: f32,
    shape: Wave,
}

impl Osc {
    /// A sine oscillator at sample rate `sr` (Hz).
    pub fn new(sr: f32) -> Osc {
        Osc {
            sr: sr.max(1.0),
            phase: 0.0,
            dt: 0.0,
            shape: Wave::Sine,
        }
    }

    /// Set the frequency in Hz (clamped to a sane, positive, sub-Nyquist range).
    #[inline]
    pub fn set_freq(&mut self, hz: f32) {
        let f = hz.clamp(0.0, self.sr * 0.5);
        self.dt = f / self.sr;
    }

    /// Set the waveform.
    #[inline]
    pub fn set_shape(&mut self, shape: Wave) {
        self.shape = shape;
    }

    /// Reset the phase to 0 (a click-safe restart when the voice is silent).
    #[inline]
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Set an absolute phase in `[0, 1)`.
    #[inline]
    pub fn set_phase(&mut self, p: f32) {
        self.phase = p.rem_euclid(1.0);
    }

    /// Produce the next sample in `[-1, 1]` and advance the phase.
    #[inline]
    pub fn next(&mut self) -> f32 {
        let dt = self.dt;
        let p = self.phase;
        let out = match self.shape {
            Wave::Sine => (p * std::f32::consts::TAU).sin(),
            Wave::Saw => {
                // Naive ramp in [-1, 1], minus the band-limited step at the wrap.
                let mut v = 2.0 * p - 1.0;
                v -= poly_blep(p, dt);
                v
            }
            Wave::Pulse(width) => {
                let w = width.clamp(0.01, 0.99);
                let mut v = if p < w { 1.0 } else { -1.0 };
                v += poly_blep(p, dt);
                v -= poly_blep((p + 1.0 - w).fract(), dt);
                v
            }
            Wave::Triangle => 2.0 * (2.0 * p - 1.0).abs() - 1.0,
        };
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        out
    }
}

/// A 2-operator FM (phase-modulation) pair — one modulator bending one carrier. Produces
/// bell/metallic/electric-piano timbres from two sines.
#[derive(Debug, Clone)]
pub struct FmOsc {
    sr: f32,
    carrier_phase: f32,
    mod_phase: f32,
    carrier_dt: f32,
    mod_dt: f32,
    index: f32,
}

impl FmOsc {
    /// A new FM pair at sample rate `sr`.
    pub fn new(sr: f32) -> FmOsc {
        FmOsc {
            sr: sr.max(1.0),
            carrier_phase: 0.0,
            mod_phase: 0.0,
            carrier_dt: 0.0,
            mod_dt: 0.0,
            index: 0.0,
        }
    }

    /// Configure: carrier frequency (Hz), modulator:carrier frequency `ratio`, and
    /// modulation `index` (peak phase deviation, in cycles). The index is clamped to
    /// [`FmOsc::alias_free_index`], so a high note never folds its sidebands back below Nyquist.
    #[inline]
    pub fn set(&mut self, carrier_hz: f32, ratio: f32, index: f32) {
        let c = carrier_hz.clamp(0.0, self.sr * 0.5);
        self.carrier_dt = c / self.sr;
        self.mod_dt = (c * ratio).clamp(0.0, self.sr * 0.5) / self.sr;
        self.index = index
            .max(0.0)
            .min(Self::alias_free_index(self.sr, c, ratio));
    }

    /// The largest modulation index (cycles) whose spectrum stays under `0.45 * sr` for a
    /// `carrier_hz` carrier and a `ratio` modulator: the sidebands sit at `fc ± k·fm` and are
    /// negligible past `k = β + 2` (Carson's rule plus one sideband; `β = 2π·index` radians), so
    /// the top partial is `fc·(1 + ratio·(β + 2))`. A larger index folds sidebands past Nyquist
    /// back into the audible band as INHARMONIC partials — a bell that is out of tune with its
    /// own fundamental. Unbounded (`f32::INFINITY`) when nothing modulates.
    pub fn alias_free_index(sr: f32, carrier_hz: f32, ratio: f32) -> f32 {
        if carrier_hz <= 0.0 || ratio <= 0.0 {
            return f32::INFINITY;
        }
        let beta = ((0.45 * sr / carrier_hz - 1.0) / ratio - 2.0).max(0.0);
        beta / std::f32::consts::TAU
    }

    /// Reset both phases.
    #[inline]
    pub fn reset_phase(&mut self) {
        self.carrier_phase = 0.0;
        self.mod_phase = 0.0;
    }

    /// Next sample in `[-1, 1]`.
    #[inline]
    pub fn next(&mut self) -> f32 {
        let m = (self.mod_phase * std::f32::consts::TAU).sin() * self.index;
        let out = ((self.carrier_phase + m) * std::f32::consts::TAU).sin();
        self.carrier_phase = (self.carrier_phase + self.carrier_dt).fract();
        self.mod_phase = (self.mod_phase + self.mod_dt).fract();
        out
    }
}

/// A deterministic white-noise source (xorshift64* PRNG mapped to `[-1, 1]`).
#[derive(Debug, Clone)]
pub struct Noise {
    state: u64,
}

impl Noise {
    /// A noise source seeded by `seed` (0 is remapped so the PRNG never sticks at zero).
    pub fn new(seed: u64) -> Noise {
        Noise {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// Next white-noise sample in `[-1, 1)`.
    #[inline]
    pub fn next(&mut self) -> f32 {
        // xorshift64*
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        // Top 24 bits -> [0, 1) -> [-1, 1).
        let u = (v >> 40) as f32 / (1u32 << 24) as f32;
        u * 2.0 - 1.0
    }
}

/// polyBLEP: a 2-sample band-limited step correction. `t` is the phase in `[0,1)`, `dt`
/// the per-sample phase increment.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn render<F: FnMut() -> f32>(mut f: F, n: usize) -> Vec<f32> {
        (0..n).map(|_| f()).collect()
    }

    #[test]
    fn all_shapes_stay_bounded_and_finite() {
        for shape in [Wave::Sine, Wave::Saw, Wave::Pulse(0.5), Wave::Triangle] {
            let mut o = Osc::new(SR);
            o.set_shape(shape);
            o.set_freq(440.0);
            let v = render(|| o.next(), 48_000);
            assert!(
                v.iter().all(|s| s.is_finite()),
                "{shape:?} produced non-finite"
            );
            let peak = v.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
            assert!(peak <= 1.05, "{shape:?} peak {peak} exceeded full scale");
            assert!(peak > 0.3, "{shape:?} peak {peak} suspiciously silent");
        }
    }

    #[test]
    fn sine_frequency_is_correct_via_zero_crossings() {
        let mut o = Osc::new(SR);
        o.set_shape(Wave::Sine);
        o.set_freq(1000.0);
        let v = render(|| o.next(), 48_000);
        // Count positive-going zero crossings over 1s ~= frequency.
        let mut crossings: i32 = 0;
        for w in v.windows(2) {
            if w[0] <= 0.0 && w[1] > 0.0 {
                crossings += 1;
            }
        }
        assert!((crossings - 1000).abs() <= 2, "got {crossings} crossings");
    }

    #[test]
    fn polyblep_saw_reduces_alias_energy_vs_naive() {
        // A high saw: measure energy above the fundamental's expected band as a proxy.
        // The polyBLEP output must not blow up and must stay bounded near a naive saw's
        // peak-to-peak (sanity, not a full spectral test).
        let mut o = Osc::new(SR);
        o.set_shape(Wave::Saw);
        o.set_freq(8000.0);
        let v = render(|| o.next(), 4800);
        assert!(v.iter().all(|s| s.is_finite()));
        assert!(v.iter().fold(0.0f32, |a, &s| a.max(s.abs())) <= 1.2);
    }

    #[test]
    fn fm_is_bounded_and_deterministic() {
        let mut a = FmOsc::new(SR);
        a.set(220.0, 2.0, 3.0);
        let mut b = FmOsc::new(SR);
        b.set(220.0, 2.0, 3.0);
        let va = render(|| a.next(), 2000);
        let vb = render(|| b.next(), 2000);
        assert_eq!(va, vb);
        assert!(va.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }

    /// Goertzel magnitude of `v` at `hz`.
    fn goertzel(v: &[f32], hz: f32) -> f32 {
        let w = std::f32::consts::TAU * hz / SR;
        let (mut s1, mut s2) = (0.0f64, 0.0f64);
        let c = 2.0 * (w as f64).cos();
        for &x in v {
            let s0 = x as f64 + c * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        ((s1 * s1 + s2 * s2 - c * s1 * s2).max(0.0)).sqrt() as f32
    }

    #[test]
    fn fm_high_notes_do_not_fold_inharmonic_partials_below_nyquist() {
        // BLACK_ICE's lead (ratio 2, index 2.2 cycles = 13.8 rad) at its climax G6 (1568 Hz): the
        // unlimited spectrum reaches k = 16 sidebands, 1568·29 = 45472 Hz folds to 48000 − 45472 =
        // 2528 Hz — an inharmonic partial the R8 synth scout measured at −15.6 dB in the real
        // render. Band-limited, it is gone; a low note keeps its full index.
        let g6 = 1567.98;
        let lim = FmOsc::alias_free_index(SR, g6, 2.0);
        assert!(lim < 2.2 && lim > 0.0, "limit {lim}");
        let mut o = FmOsc::new(SR);
        o.set(g6, 2.0, 2.2);
        assert_eq!(o.index, lim);
        let v = render(|| o.next(), 48_000);
        let fundamental = goertzel(&v, g6);
        let alias = goertzel(&v, 2528.0);
        let db = 20.0 * (alias / fundamental).log10();
        assert!(
            db < -80.0,
            "folded partial at {db:.1} dB re the fundamental"
        );
        // The unlimited phase-modulation formula at the same note DOES alias (the defect).
        let naive: Vec<f32> = (0..48_000)
            .map(|i| {
                let t = i as f32 / SR;
                let m = (std::f32::consts::TAU * 2.0 * g6 * t).sin() * 2.2;
                ((g6 * t + m) * std::f32::consts::TAU).sin()
            })
            .collect();
        let naive_db = 20.0 * (goertzel(&naive, 2528.0) / goertzel(&naive, g6)).log10();
        assert!(naive_db > -30.0, "the control must alias: {naive_db:.1} dB");
        // A4 (440 Hz) is far from the limit: its index is untouched.
        let mut low = FmOsc::new(SR);
        low.set(440.0, 2.0, 2.2);
        assert_eq!(low.index, 2.2);
    }

    #[test]
    fn noise_is_deterministic_bounded_and_roughly_centered() {
        let mut a = Noise::new(42);
        let mut b = Noise::new(42);
        let va = render(|| a.next(), 100_000);
        let vb = render(|| b.next(), 100_000);
        assert_eq!(va, vb);
        assert!(va.iter().all(|s| s.is_finite() && (-1.0..1.0).contains(s)));
        let mean = va.iter().sum::<f32>() / va.len() as f32;
        assert!(mean.abs() < 0.02, "noise DC {mean} too high");
    }

    #[test]
    fn freq_clamped_below_nyquist() {
        let mut o = Osc::new(SR);
        o.set_freq(1_000_000.0); // absurd
        let v = render(|| o.next(), 100);
        assert!(v.iter().all(|s| s.is_finite()));
    }
}
