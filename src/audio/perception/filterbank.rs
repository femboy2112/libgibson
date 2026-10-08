//! A deterministic, inspectable **ERB-spaced auditory filterbank**.
//!
//! Human frequency selectivity is not uniform FFT bins: auditory-filter bandwidth grows
//! with centre frequency. We place centre frequencies equally on the ERB-rate scale
//! (Glasberg & Moore 1990) and realise each band as a 4th-order bandpass (two cascaded
//! RBJ biquad sections) whose bandwidth tracks the equivalent rectangular bandwidth
//! (ERB) at that centre. This is a **declared gammatone-like approximation**, not a claim
//! to be the cochlea: the architectural requirement (per the design note) is only that
//! the coordinate system approximate auditory frequency selectivity and stay deterministic
//! and inspectable. Nothing here is random or stateful across calls.
//!
//! References (constrain the model; none is a drop-in spec):
//! Moore & Glasberg (1983) JASA 74(3) 750; Glasberg & Moore (1990) auditory-filter shapes.

/// Equivalent rectangular bandwidth (Hz) of the auditory filter centred at `f` Hz
/// (Glasberg & Moore 1990): `ERB = 24.7 (4.37 f/1000 + 1)`.
pub fn erb_hz(f: f64) -> f64 {
    24.7 * (4.37 * f / 1000.0 + 1.0)
}

/// Position of `f` Hz on the ERB-rate scale (number of ERBs below `f`):
/// `21.4 log10(4.37 f/1000 + 1)`.
pub fn erb_rate(f: f64) -> f64 {
    21.4 * (4.37 * f / 1000.0 + 1.0).log10()
}

/// Inverse of [`erb_rate`]: the frequency (Hz) at ERB-rate position `e`.
pub fn erb_rate_to_hz(e: f64) -> f64 {
    (10f64.powf(e / 21.4) - 1.0) / 4.37 * 1000.0
}

/// One biquad section in Direct Form I (`f32` state).
#[derive(Debug, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    /// An RBJ constant-0-dB-peak-gain bandpass at `fc` Hz, quality `q`, sample rate `sr`.
    fn bandpass(fc: f64, q: f64, sr: f64) -> Biquad {
        let w0 = 2.0 * std::f64::consts::PI * fc / sr;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;
        Biquad {
            b0: (alpha / a0) as f32,
            b1: 0.0,
            b2: (-alpha / a0) as f32,
            a1: (-2.0 * cos / a0) as f32,
            a2: ((1.0 - alpha) / a0) as f32,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// A fixed, deterministic bank of ERB-spaced bandpass channels. Coefficients are computed
/// once at construction; analysis allocates fresh filter state per call (so repeated
/// analysis of the same input is bit-identical and order-independent).
#[derive(Debug, Clone)]
pub struct ErbBank {
    sr: f64,
    centers: Vec<f32>,
    /// Per band, the (fc, q) used to build its two cascaded sections.
    design: Vec<(f64, f64)>,
}

impl ErbBank {
    /// A bank of `n_bands` channels spaced equally on the ERB-rate scale between `f_lo`
    /// and `f_hi` Hz, at sample rate `sr_hz`. Centres are clamped below `0.45*sr` for
    /// stability. `n_bands` is clamped to at least 1.
    pub fn new(sr_hz: u32, n_bands: usize, f_lo: f64, f_hi: f64) -> ErbBank {
        let sr = sr_hz as f64;
        let n = n_bands.max(1);
        let hi = f_hi.min(0.45 * sr).max(f_lo + 1.0);
        let (e_lo, e_hi) = (erb_rate(f_lo), erb_rate(hi));
        let mut centers = Vec::with_capacity(n);
        let mut design = Vec::with_capacity(n);
        for k in 0..n {
            let e = if n == 1 {
                0.5 * (e_lo + e_hi)
            } else {
                e_lo + (e_hi - e_lo) * (k as f64) / ((n - 1) as f64)
            };
            let fc = erb_rate_to_hz(e).clamp(f_lo, hi);
            // Quality from the auditory bandwidth at this centre: Q = fc / ERB(fc).
            let q = (fc / erb_hz(fc)).max(0.5);
            centers.push(fc as f32);
            design.push((fc, q));
        }
        ErbBank {
            sr,
            centers,
            design,
        }
    }

    /// A sensible default bank: 32 channels from 50 Hz to 16 kHz.
    pub fn default_for(sr_hz: u32) -> ErbBank {
        ErbBank::new(sr_hz, 32, 50.0, 16_000.0)
    }

    /// The channel centre frequencies (Hz), strictly ascending.
    pub fn center_freqs(&self) -> &[f32] {
        &self.centers
    }

    /// Number of channels.
    pub fn len(&self) -> usize {
        self.centers.len()
    }

    /// Whether the bank has no channels (it never does; `new` clamps to ≥1).
    pub fn is_empty(&self) -> bool {
        self.centers.is_empty()
    }

    /// Filter `mono` through channel `band` (two cascaded bandpass sections, fresh state).
    /// Returns the band-limited signal, same length as the input.
    pub fn filter_band(&self, band: usize, mono: &[f32]) -> Vec<f32> {
        let (fc, q) = self.design[band];
        let mut s1 = Biquad::bandpass(fc, q, self.sr);
        let mut s2 = Biquad::bandpass(fc, q, self.sr);
        mono.iter().map(|&x| s2.process(s1.process(x))).collect()
    }

    /// Total energy (sum of squares) in channel `band` for `mono`.
    pub fn band_energy(&self, band: usize, mono: &[f32]) -> f32 {
        let (fc, q) = self.design[band];
        let mut s1 = Biquad::bandpass(fc, q, self.sr);
        let mut s2 = Biquad::bandpass(fc, q, self.sr);
        mono.iter().fold(0.0f32, |acc, &x| {
            let y = s2.process(s1.process(x));
            acc + y * y
        })
    }

    /// Per-channel energy for `mono` (one value per channel, channel order = ascending fc).
    pub fn energies(&self, mono: &[f32]) -> Vec<f32> {
        (0..self.len()).map(|b| self.band_energy(b, mono)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn sine(freq: f64, sr: u32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| ((2.0 * std::f64::consts::PI * freq * i as f64 / sr as f64).sin()) as f32)
            .collect()
    }

    #[test]
    fn erb_rate_roundtrip() {
        for &f in &[50.0, 220.0, 440.0, 1000.0, 4000.0, 12000.0] {
            let back = erb_rate_to_hz(erb_rate(f));
            assert!((back - f).abs() < 1e-3, "{f} -> {back}");
        }
    }

    #[test]
    fn centers_strictly_ascending_and_in_range() {
        let bank = ErbBank::new(48_000, 32, 50.0, 16_000.0);
        let c = bank.center_freqs();
        assert_eq!(c.len(), 32);
        for w in c.windows(2) {
            assert!(w[1] > w[0], "centers must strictly ascend: {w:?}");
        }
        assert!(c[0] >= 50.0 && *c.last().unwrap() <= 16_000.0);
    }

    #[test]
    fn centers_clamped_below_nyquist() {
        // Ask for 20 kHz top at 44.1 kHz: must clamp below 0.45*sr ≈ 19.8 kHz.
        let bank = ErbBank::new(44_100, 24, 50.0, 20_000.0);
        assert!(*bank.center_freqs().last().unwrap() <= 0.45 * 44_100.0);
    }

    #[test]
    fn pure_tone_localizes_to_nearest_band() {
        let sr = 48_000;
        let bank = ErbBank::new(sr, 32, 50.0, 16_000.0);
        let x = sine(1000.0, sr, 24_000); // 0.5 s @ 1 kHz
        let e = bank.energies(&x);
        let argmax = e
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        // nearest band by centre
        let nearest = bank
            .center_freqs()
            .iter()
            .enumerate()
            .min_by(|a, b| {
                (a.1 - 1000.0)
                    .abs()
                    .partial_cmp(&(b.1 - 1000.0).abs())
                    .unwrap()
            })
            .unwrap()
            .0;
        assert!(
            (argmax as i32 - nearest as i32).abs() <= 1,
            "1 kHz tone peaked at band {argmax} (fc={}), nearest-centre band is {nearest} (fc={})",
            bank.center_freqs()[argmax],
            bank.center_freqs()[nearest],
        );
    }

    #[test]
    fn silence_gives_zero_energy() {
        let bank = ErbBank::default_for(48_000);
        let e = bank.energies(&vec![0.0f32; 4096]);
        assert!(e.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn impulse_stays_finite_and_bounded() {
        let bank = ErbBank::default_for(48_000);
        let mut x = vec![0.0f32; 2048];
        x[0] = 1.0;
        for b in 0..bank.len() {
            let y = bank.filter_band(b, &x);
            assert!(
                y.iter().all(|v| v.is_finite()),
                "band {b} produced non-finite"
            );
            let peak = y.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(
                peak < 10.0,
                "band {b} peak {peak} unreasonably large (unstable?)"
            );
        }
    }

    #[test]
    fn deterministic_repeat() {
        let bank = ErbBank::default_for(48_000);
        let x: Vec<f32> = (0..4096)
            .map(|i| (0.3 * (i as f32 * 0.07).sin()) + 0.1 * (i as f32 * PI * 0.013).cos())
            .collect();
        assert_eq!(bank.energies(&x), bank.energies(&x));
        assert_eq!(bank.filter_band(5, &x), bank.filter_band(5, &x));
    }

    #[test]
    fn broadband_energy_bookkeeping_bounded() {
        // A deterministic broadband signal (summed primes-ish sinusoids). The summed
        // band energy must stay within a declared, generous factor of the input energy:
        // overlapping auditory bands do not form a tight frame, so we assert only that
        // the bank neither swallows nor explodes the energy (tolerance 0.05x .. 50x).
        let sr = 48_000;
        let n = 24_000;
        let x: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / sr as f64;
                let mut s = 0.0;
                for &f in &[110.0, 277.0, 523.0, 1100.0, 2200.0, 4400.0, 8800.0] {
                    s += (2.0 * std::f64::consts::PI * f * t).sin();
                }
                (s / 7.0) as f32
            })
            .collect();
        let ein: f32 = x.iter().map(|v| v * v).sum();
        let eband: f32 = ErbBank::new(sr, 40, 50.0, 16_000.0)
            .energies(&x)
            .iter()
            .sum();
        let ratio = eband / ein.max(1e-9);
        assert!(
            ratio > 0.05 && ratio < 50.0,
            "energy ratio {ratio} out of declared tolerance"
        );
        assert!(eband.is_finite());
    }
}
