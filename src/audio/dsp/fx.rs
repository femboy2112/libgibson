//! Production effects: equal-power [`pan`], [`soft_saturate`], a [`Delay`], a [`Chorus`],
//! an FDN [`Reverb`], a bus [`Compressor`] and a peak [`Limiter`].
//!
//! Enough processing to make a synthesized mix sound *finished* without any convolution
//! IR or sampled assets. All state is preallocated at construction; nothing allocates on
//! the audio path.

/// Equal-power pan of a mono sample to stereo. `pan` in `[-1, 1]` (−1 = hard left).
#[inline]
pub fn pan(x: f32, pan: f32) -> (f32, f32) {
    let p = (pan.clamp(-1.0, 1.0) + 1.0) * 0.25 * std::f32::consts::PI; // 0..π/2
    (x * p.cos(), x * p.sin())
}

/// A `tanh` soft-saturator. `drive >= 1` pushes harder; output stays in `(-1, 1)`.
#[inline]
pub fn soft_saturate(x: f32, drive: f32) -> f32 {
    (x * drive.max(1.0)).tanh()
}

/// A fractional-delay ring buffer (the shared primitive under delay/chorus/reverb).
#[derive(Debug, Clone)]
struct VarDelay {
    buf: Vec<f32>,
    widx: usize,
}

impl VarDelay {
    fn new(max_samples: usize) -> VarDelay {
        VarDelay {
            buf: vec![0.0; max_samples.max(2)],
            widx: 0,
        }
    }

    #[inline]
    fn write(&mut self, x: f32) {
        self.buf[self.widx] = x;
        self.widx += 1;
        if self.widx >= self.buf.len() {
            self.widx = 0;
        }
    }

    /// Read `d` samples in the past (linear-interpolated). `d >= 1`.
    #[inline]
    fn tap(&self, d: f32) -> f32 {
        let n = self.buf.len();
        let d = d.clamp(1.0, (n - 1) as f32);
        let di = d.floor();
        let frac = d - di;
        let i0 = (self.widx + n - di as usize) % n;
        let i1 = (i0 + n - 1) % n;
        self.buf[i0] * (1.0 - frac) + self.buf[i1] * frac
    }
}

/// A mono feedback delay.
#[derive(Debug, Clone)]
pub struct Delay {
    sr: f32,
    line: VarDelay,
    time: f32,
    feedback: f32,
    mix: f32,
}

impl Delay {
    /// A delay at `sr` with up to `max_secs` of delay time.
    pub fn new(sr: f32, max_secs: f32) -> Delay {
        Delay {
            sr: sr.max(1.0),
            line: VarDelay::new((sr * max_secs.max(0.01)) as usize + 4),
            time: sr * 0.25,
            feedback: 0.3,
            mix: 0.25,
        }
    }

    /// Set delay time (s), feedback `[0, 0.99]` and wet/dry mix `[0, 1]`.
    pub fn set(&mut self, time_secs: f32, feedback: f32, mix: f32) {
        self.time = (time_secs.max(0.001) * self.sr).max(1.0);
        self.feedback = feedback.clamp(0.0, 0.99);
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Process one mono sample.
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let echo = self.line.tap(self.time);
        self.line.write(x + echo * self.feedback);
        x * (1.0 - self.mix) + echo * self.mix
    }
}

/// A stereo chorus (two LFO-modulated delay lines with a width offset).
#[derive(Debug, Clone)]
pub struct Chorus {
    sr: f32,
    l: VarDelay,
    r: VarDelay,
    lfo_phase: f32,
    rate: f32,
    base: f32,
    depth: f32,
    mix: f32,
}

impl Chorus {
    /// A chorus at `sr`.
    pub fn new(sr: f32) -> Chorus {
        let sr = sr.max(1.0);
        Chorus {
            sr,
            l: VarDelay::new((sr * 0.05) as usize + 4),
            r: VarDelay::new((sr * 0.05) as usize + 4),
            lfo_phase: 0.0,
            rate: 0.6,
            base: sr * 0.018,
            depth: sr * 0.006,
            mix: 0.4,
        }
    }

    /// Set LFO rate (Hz), modulation depth (ms) and wet mix `[0,1]`.
    pub fn set(&mut self, rate_hz: f32, depth_ms: f32, mix: f32) {
        self.rate = rate_hz.clamp(0.01, 10.0);
        self.depth = (depth_ms.max(0.0) * 0.001 * self.sr).min(self.base - 2.0);
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Process one stereo frame.
    #[inline]
    pub fn process_stereo(&mut self, l: f32, r: f32) -> (f32, f32) {
        let tau = std::f32::consts::TAU;
        let lfo_l = (self.lfo_phase * tau).sin();
        let lfo_r = (self.lfo_phase * tau + 1.0).sin(); // ~57° offset for width
        let dl = self.base + self.depth * lfo_l;
        let dr = self.base + self.depth * lfo_r;
        let wl = self.l.tap(dl);
        let wr = self.r.tap(dr);
        self.l.write(l);
        self.r.write(r);
        self.lfo_phase += self.rate / self.sr;
        if self.lfo_phase >= 1.0 {
            self.lfo_phase -= 1.0;
        }
        (
            l * (1.0 - self.mix) + wl * self.mix,
            r * (1.0 - self.mix) + wr * self.mix,
        )
    }
}

/// A 4-line feedback delay network reverb with per-line damping and an orthonormal
/// (Hadamard) feedback matrix — stable for any feedback gain `< 1`.
#[derive(Debug, Clone)]
pub struct Reverb {
    sr: f32,
    lines: [VarDelay; 4],
    base_ms: [f32; 4],
    size: f32,
    damp: f32,
    damp_state: [f32; 4],
    feedback: f32,
    mix: f32,
}

impl Reverb {
    /// A reverb at `sr`.
    pub fn new(sr: f32) -> Reverb {
        let sr = sr.max(1.0);
        let cap = (sr * 0.12) as usize + 4;
        Reverb {
            sr,
            lines: [
                VarDelay::new(cap),
                VarDelay::new(cap),
                VarDelay::new(cap),
                VarDelay::new(cap),
            ],
            base_ms: [29.7, 37.1, 41.1, 43.7],
            size: 1.0,
            damp: 0.35,
            damp_state: [0.0; 4],
            feedback: 0.78,
            mix: 0.25,
        }
    }

    /// Set size `[0.3, 1.5]`, damping `[0,1]` (higher = darker tail) and wet mix `[0,1]`.
    pub fn set(&mut self, size: f32, damping: f32, mix: f32) {
        self.size = size.clamp(0.3, 1.5);
        // Higher damping -> smaller lowpass coefficient -> darker.
        self.damp = (1.0 - damping.clamp(0.0, 0.99)).clamp(0.02, 1.0);
        self.feedback = (0.7 + 0.12 * self.size).clamp(0.0, 0.92);
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Process one stereo frame.
    #[inline]
    pub fn process_stereo(&mut self, l: f32, r: f32) -> (f32, f32) {
        let input = (l + r) * 0.5;
        for i in 0..4 {
            let d = (self.base_ms[i] * 0.001 * self.sr * self.size).max(1.0);
            let raw = self.lines[i].tap(d);
            // One-pole lowpass damping in the feedback path.
            self.damp_state[i] += self.damp * (raw - self.damp_state[i]);
        }
        let o = self.damp_state;
        // Normalized Hadamard mix (0.5 factor keeps it orthonormal / energy-preserving).
        let m0 = 0.5 * (o[0] + o[1] + o[2] + o[3]);
        let m1 = 0.5 * (o[0] - o[1] + o[2] - o[3]);
        let m2 = 0.5 * (o[0] + o[1] - o[2] - o[3]);
        let m3 = 0.5 * (o[0] - o[1] - o[2] + o[3]);
        self.lines[0].write(input + self.feedback * m0);
        self.lines[1].write(input + self.feedback * m1);
        self.lines[2].write(input + self.feedback * m2);
        self.lines[3].write(input + self.feedback * m3);
        let wet_l = (o[0] + o[2]) * 0.5;
        let wet_r = (o[1] + o[3]) * 0.5;
        (
            l * (1.0 - self.mix) + wet_l * self.mix,
            r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }
}

/// A stereo bus compressor (peak-detecting, feed-forward, dB-domain gain computer).
#[derive(Debug, Clone)]
pub struct Compressor {
    sr: f32,
    threshold_db: f32,
    ratio: f32,
    atk_coef: f32,
    rel_coef: f32,
    makeup_db: f32,
    env: f32,
}

impl Compressor {
    /// A compressor at `sr` (gentle default: -12 dB / 3:1).
    pub fn new(sr: f32) -> Compressor {
        let mut c = Compressor {
            sr: sr.max(1.0),
            threshold_db: -12.0,
            ratio: 3.0,
            atk_coef: 0.0,
            rel_coef: 0.0,
            makeup_db: 0.0,
            env: 0.0,
        };
        c.set(-12.0, 3.0, 10.0, 120.0, 0.0);
        c
    }

    /// Set threshold (dBFS), ratio, attack/release (ms) and makeup gain (dB).
    pub fn set(
        &mut self,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_db: f32,
    ) {
        self.threshold_db = threshold_db;
        self.ratio = ratio.max(1.0);
        self.atk_coef = coef_ms(attack_ms, self.sr);
        self.rel_coef = coef_ms(release_ms, self.sr);
        self.makeup_db = makeup_db;
    }

    /// Process one stereo frame.
    #[inline]
    pub fn process_stereo(&mut self, l: f32, r: f32) -> (f32, f32) {
        let level = l.abs().max(r.abs());
        let coef = if level > self.env {
            self.atk_coef
        } else {
            self.rel_coef
        };
        self.env += coef * (level - self.env);
        let level_db = 20.0 * (self.env.max(1e-6)).log10();
        let over = level_db - self.threshold_db;
        let reduction = if over > 0.0 {
            -over * (1.0 - 1.0 / self.ratio)
        } else {
            0.0
        };
        let gain_db = reduction + self.makeup_db;
        let g = 10f32.powf(gain_db / 20.0);
        (l * g, r * g)
    }
}

/// A feed-forward peak limiter: instant gain reduction, smooth release, hard ceiling.
#[derive(Debug, Clone)]
pub struct Limiter {
    ceiling: f32,
    gain: f32,
    rel_coef: f32,
}

impl Limiter {
    /// A limiter at `sr` with the given `ceiling` (linear, e.g. 0.98).
    pub fn new(sr: f32) -> Limiter {
        Limiter {
            ceiling: 0.98,
            gain: 1.0,
            rel_coef: coef_ms(80.0, sr.max(1.0)),
        }
    }

    /// Set the output ceiling (linear `(0,1]`).
    pub fn set_ceiling(&mut self, ceiling: f32) {
        self.ceiling = ceiling.clamp(0.05, 1.0);
    }

    /// Process one stereo frame; the output magnitude never exceeds the ceiling.
    #[inline]
    pub fn process_stereo(&mut self, l: f32, r: f32) -> (f32, f32) {
        let peak = l.abs().max(r.abs()).max(1e-9);
        let target = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        if target < self.gain {
            self.gain = target; // instant attack
        } else {
            self.gain += self.rel_coef * (target - self.gain); // smooth release
        }
        (
            (l * self.gain).clamp(-self.ceiling, self.ceiling),
            (r * self.gain).clamp(-self.ceiling, self.ceiling),
        )
    }
}

/// One-pole smoothing coefficient reaching ~63% in `ms` milliseconds.
#[inline]
fn coef_ms(ms: f32, sr: f32) -> f32 {
    let t = (ms.max(0.01) * 0.001) * sr;
    1.0 - (-1.0 / t).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 48_000.0;

    #[test]
    fn pan_is_equal_power() {
        let (l, r) = pan(1.0, 0.0);
        // Center: equal, and L²+R² ≈ 1 (equal-power law).
        assert!((l - r).abs() < 1e-6);
        assert!((l * l + r * r - 1.0).abs() < 1e-5);
        let (l, _r) = pan(1.0, -1.0);
        assert!((l - 1.0).abs() < 1e-5); // hard left
    }

    #[test]
    fn saturate_is_bounded() {
        for x in [-100.0, -1.0, 0.0, 1.0, 100.0] {
            let y = soft_saturate(x, 4.0);
            // tanh is in [-1,1]; f32 tanh of a large argument saturates to exactly ±1.
            assert!(y.abs() <= 1.0 && y.is_finite());
        }
    }

    #[test]
    fn delay_produces_an_echo_and_stays_finite() {
        let mut d = Delay::new(SR, 1.0);
        d.set(0.01, 0.5, 0.5); // 10ms = 480 samples
        let mut out = Vec::new();
        out.push(d.process(1.0)); // impulse
        for _ in 0..2000 {
            out.push(d.process(0.0));
        }
        assert!(out.iter().all(|s| s.is_finite()));
        // An echo must appear ~480 samples later (the write-then-advance ring puts it at
        // index 480; check a small window to be robust to the fractional-tap boundary).
        let echo = out[478..=483].iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!(echo > 0.1, "expected echo near sample 480, got {echo}");
    }

    #[test]
    fn reverb_tail_decays_and_never_diverges() {
        let mut rv = Reverb::new(SR);
        rv.set(1.0, 0.4, 1.0); // full wet
                               // Impulse in, then silence for 3s: the tail must decay, never blow up.
        let (mut l, mut r) = rv.process_stereo(1.0, 1.0);
        let mut peak_early = l.abs().max(r.abs());
        for i in 0..(SR as usize * 3) {
            let (nl, nr) = rv.process_stereo(0.0, 0.0);
            l = nl;
            r = nr;
            assert!(l.is_finite() && r.is_finite(), "reverb diverged at {i}");
            if i < 4800 {
                peak_early = peak_early.max(l.abs().max(r.abs()));
            }
        }
        let late = l.abs().max(r.abs());
        assert!(
            late < peak_early * 0.5 + 1e-4,
            "tail did not decay: {late} vs {peak_early}"
        );
    }

    #[test]
    fn chorus_is_safe_and_widens() {
        let mut c = Chorus::new(SR);
        c.set(0.8, 4.0, 0.5);
        let mut maxd = 0.0f32;
        for i in 0..SR as usize {
            let x = (i as f32 * 0.01).sin();
            let (l, r) = c.process_stereo(x, x);
            assert!(l.is_finite() && r.is_finite());
            maxd = maxd.max((l - r).abs());
        }
        assert!(maxd > 0.0, "chorus produced no stereo width");
    }

    #[test]
    fn compressor_reduces_gain_above_threshold() {
        let mut comp = Compressor::new(SR);
        comp.set(-20.0, 4.0, 5.0, 50.0, 0.0);
        // Feed a loud tone; after the attack, output should be quieter than input.
        let mut in_peak = 0.0f32;
        let mut out_peak = 0.0f32;
        for i in 0..SR as usize {
            let x = 0.9 * (i as f32 * 0.05).sin();
            let (l, _) = comp.process_stereo(x, x);
            if i > SR as usize / 2 {
                in_peak = in_peak.max(x.abs());
                out_peak = out_peak.max(l.abs());
            }
        }
        assert!(
            out_peak < in_peak,
            "comp did not reduce: {out_peak} vs {in_peak}"
        );
    }

    #[test]
    fn limiter_never_exceeds_ceiling() {
        let mut lim = Limiter::new(SR);
        lim.set_ceiling(0.9);
        for i in 0..SR as usize {
            let x = 3.0 * (i as f32 * 0.03).sin(); // way over full scale
            let (l, r) = lim.process_stereo(x, x);
            assert!(
                l.abs() <= 0.9 + 1e-6 && r.abs() <= 0.9 + 1e-6,
                "limiter overshoot {l}"
            );
        }
    }
}
