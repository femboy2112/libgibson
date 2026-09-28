//! Synthesized percussion — kick, snare, hat, clap — built from the DSP primitives. No
//! samples: a kick is a pitch-enveloped sine plus a click, a snare is band-passed noise
//! plus a tonal body, a hat is high-passed noise, a clap is several timed noise bursts.
//!
//! Each voice: construct once, [`trigger`](Kick::trigger) with a velocity `[0,1]`, then
//! pull mono samples with `next()`; `is_active()` reports whether it still sounds. Voices
//! are one-shot and retriggerable. World-specific character comes from the `*Params`.

// `next()` is the deliberate per-sample generator idiom shared by every DSP voice.
#![allow(clippy::should_implement_trait)]

use super::env::ExpDecay;
use super::filter::Svf;
use super::osc::{Noise, Osc, Wave};

/// Kick voice tuning.
#[derive(Debug, Clone, Copy)]
pub struct KickParams {
    /// Settled body frequency (Hz).
    pub base_hz: f32,
    /// Peak of the initial downward pitch sweep above `base_hz` (Hz).
    pub pitch_sweep_hz: f32,
    /// Pitch-sweep time constant (s).
    pub pitch_tau: f32,
    /// Amplitude decay time constant (s).
    pub amp_tau: f32,
    /// Transient click amount `[0,1]`.
    pub click: f32,
}

impl Default for KickParams {
    fn default() -> Self {
        KickParams {
            base_hz: 50.0,
            pitch_sweep_hz: 180.0,
            pitch_tau: 0.03,
            amp_tau: 0.13,
            click: 0.3,
        }
    }
}

/// A synthesized kick drum.
#[derive(Debug, Clone)]
pub struct Kick {
    p: KickParams,
    body: Osc,
    amp: ExpDecay,
    pitch: ExpDecay,
    click_env: ExpDecay,
    click_noise: Noise,
}

impl Kick {
    /// A kick at sample rate `sr` with default tuning.
    pub fn new(sr: f32) -> Kick {
        Kick::with_params(sr, KickParams::default())
    }

    /// A kick with explicit tuning.
    pub fn with_params(sr: f32, p: KickParams) -> Kick {
        let mut body = Osc::new(sr);
        body.set_shape(Wave::Sine);
        Kick {
            p,
            body,
            amp: ExpDecay::new(sr),
            pitch: ExpDecay::new(sr),
            click_env: ExpDecay::new(sr),
            click_noise: Noise::new(0x4b1c_c0de),
        }
    }

    /// Trigger the kick at velocity `[0,1]`.
    pub fn trigger(&mut self, vel: f32) {
        let v = vel.clamp(0.0, 1.0);
        self.amp.trigger(v, self.p.amp_tau);
        self.pitch.trigger(self.p.pitch_sweep_hz, self.p.pitch_tau);
        self.click_env.trigger(v * self.p.click, 0.006);
        self.body.reset_phase();
    }

    /// Next mono sample.
    #[inline]
    pub fn next(&mut self) -> f32 {
        self.body.set_freq(self.p.base_hz + self.pitch.next());
        let body = self.body.next() * self.amp.next();
        let click = self.click_noise.next() * self.click_env.next();
        body + click * 0.5
    }

    /// Whether the kick is still sounding.
    pub fn is_active(&self) -> bool {
        self.amp.is_active() || self.click_env.is_active()
    }
}

/// Snare voice tuning.
#[derive(Debug, Clone, Copy)]
pub struct SnareParams {
    /// Noise high-pass cutoff (Hz).
    pub noise_hp_hz: f32,
    /// Noise decay time constant (s).
    pub noise_tau: f32,
    /// Tonal body decay time constant (s).
    pub tone_tau: f32,
    /// Lower/upper tonal body frequencies (Hz).
    pub tone_lo_hz: f32,
    pub tone_hi_hz: f32,
    /// Noise vs tone balance `[0,1]` (1 = all noise).
    pub noise_mix: f32,
}

impl Default for SnareParams {
    fn default() -> Self {
        SnareParams {
            noise_hp_hz: 1500.0,
            noise_tau: 0.12,
            tone_tau: 0.09,
            tone_lo_hz: 180.0,
            tone_hi_hz: 330.0,
            noise_mix: 0.7,
        }
    }
}

/// A synthesized snare.
#[derive(Debug, Clone)]
pub struct Snare {
    p: SnareParams,
    noise: Noise,
    hp: Svf,
    noise_env: ExpDecay,
    tone1: Osc,
    tone2: Osc,
    tone_env: ExpDecay,
}

impl Snare {
    /// A snare at sample rate `sr` with default tuning.
    pub fn new(sr: f32) -> Snare {
        Snare::with_params(sr, SnareParams::default())
    }

    /// A snare with explicit tuning.
    pub fn with_params(sr: f32, p: SnareParams) -> Snare {
        let mut tone1 = Osc::new(sr);
        tone1.set_shape(Wave::Triangle);
        let mut tone2 = Osc::new(sr);
        tone2.set_shape(Wave::Triangle);
        let mut hp = Svf::new(sr);
        hp.set(p.noise_hp_hz, 0.2);
        Snare {
            p,
            noise: Noise::new(0x5ba5_e001),
            hp,
            noise_env: ExpDecay::new(sr),
            tone1,
            tone2,
            tone_env: ExpDecay::new(sr),
        }
    }

    /// Trigger at velocity `[0,1]`.
    pub fn trigger(&mut self, vel: f32) {
        let v = vel.clamp(0.0, 1.0);
        self.noise_env.trigger(v, self.p.noise_tau);
        self.tone_env.trigger(v, self.p.tone_tau);
        self.tone1.set_freq(self.p.tone_lo_hz);
        self.tone2.set_freq(self.p.tone_hi_hz);
        self.tone1.reset_phase();
        self.tone2.reset_phase();
    }

    /// Next mono sample.
    #[inline]
    pub fn next(&mut self) -> f32 {
        let n = self.hp.process_modes(self.noise.next()).hp * self.noise_env.next();
        let tone = (self.tone1.next() + self.tone2.next()) * 0.5 * self.tone_env.next();
        n * self.p.noise_mix + tone * (1.0 - self.p.noise_mix) * 0.9
    }

    /// Whether the snare is still sounding.
    pub fn is_active(&self) -> bool {
        self.noise_env.is_active() || self.tone_env.is_active()
    }
}

/// A synthesized hi-hat (open or closed, chosen at trigger).
#[derive(Debug, Clone)]
pub struct Hat {
    noise: Noise,
    hp: Svf,
    env: ExpDecay,
    closed_tau: f32,
    open_tau: f32,
}

impl Hat {
    /// A hat at sample rate `sr`.
    pub fn new(sr: f32) -> Hat {
        Hat::with_cutoff(sr, 7000.0)
    }

    /// A hat with an explicit high-pass cutoff (brighter/darker metal).
    pub fn with_cutoff(sr: f32, hp_hz: f32) -> Hat {
        let mut hp = Svf::new(sr);
        hp.set(hp_hz, 0.3);
        Hat {
            noise: Noise::new(0x0ba7_5eed),
            hp,
            env: ExpDecay::new(sr),
            closed_tau: 0.03,
            open_tau: 0.22,
        }
    }

    /// Trigger at velocity `[0,1]`; `open` selects the long (open) decay.
    pub fn trigger(&mut self, vel: f32, open: bool) {
        let tau = if open { self.open_tau } else { self.closed_tau };
        self.env.trigger(vel.clamp(0.0, 1.0), tau);
    }

    /// Next mono sample.
    #[inline]
    pub fn next(&mut self) -> f32 {
        self.hp.process_modes(self.noise.next()).hp * self.env.next()
    }

    /// Whether the hat is still sounding.
    pub fn is_active(&self) -> bool {
        self.env.is_active()
    }
}

/// A synthesized hand-clap: three fast band-passed noise bursts plus a decaying tail.
#[derive(Debug, Clone)]
pub struct Clap {
    noise: Noise,
    bp: Svf,
    burst: ExpDecay,
    tail: ExpDecay,
    burst_gap: u32,
    t: u32,
    bursts_left: u32,
    vel: f32,
}

impl Clap {
    /// A clap at sample rate `sr`.
    pub fn new(sr: f32) -> Clap {
        let mut bp = Svf::new(sr);
        bp.set(1200.0, 0.5);
        Clap {
            noise: Noise::new(0xc1a9_b005),
            bp,
            burst: ExpDecay::new(sr),
            tail: ExpDecay::new(sr),
            burst_gap: (0.009 * sr) as u32,
            t: 0,
            bursts_left: 0,
            vel: 0.0,
        }
    }

    /// Trigger at velocity `[0,1]`.
    pub fn trigger(&mut self, vel: f32) {
        self.vel = vel.clamp(0.0, 1.0);
        self.t = 0;
        self.bursts_left = 2; // plus the immediate one below = 3 bursts total
        self.burst.trigger(self.vel, 0.012);
    }

    /// Next mono sample.
    #[inline]
    pub fn next(&mut self) -> f32 {
        self.t = self.t.wrapping_add(1);
        if self.bursts_left > 0 && self.burst_gap > 0 && self.t % self.burst_gap == 0 {
            self.burst.trigger(self.vel * 0.9, 0.012);
            self.bursts_left -= 1;
            if self.bursts_left == 0 {
                self.tail.trigger(self.vel * 0.6, 0.12);
            }
        }
        let amp = self.burst.next() + self.tail.next();
        self.bp.process_modes(self.noise.next()).bp * amp * 0.9
    }

    /// Whether the clap is still sounding.
    pub fn is_active(&self) -> bool {
        self.burst.is_active() || self.tail.is_active() || self.bursts_left > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 48_000.0;

    fn play<F: FnMut() -> (f32, bool)>(mut step: F, max: usize) -> (Vec<f32>, usize) {
        let mut out = Vec::new();
        let mut active_frames = 0;
        for _ in 0..max {
            let (s, active) = step();
            assert!(s.is_finite(), "non-finite drum sample");
            out.push(s);
            if active {
                active_frames += 1;
            } else if !out.is_empty() {
                break;
            }
        }
        (out, active_frames)
    }

    #[test]
    fn kick_is_bounded_terminates_and_is_low_biased() {
        let mut k = Kick::new(SR);
        k.trigger(1.0);
        let (v, frames) = play(|| (k.next(), k.is_active()), SR as usize * 2);
        assert!(frames > 100 && frames < SR as usize, "kick frames {frames}");
        assert!(v.iter().fold(0.0f32, |a, &s| a.max(s.abs())) <= 1.6);
        assert!(!k.is_active());
    }

    #[test]
    fn snare_and_hat_and_clap_are_safe_and_terminate() {
        let mut s = Snare::new(SR);
        s.trigger(0.9);
        let (_v, sf) = play(|| (s.next(), s.is_active()), SR as usize);
        assert!(sf > 50 && !s.is_active());

        let mut h = Hat::new(SR);
        h.trigger(0.8, false);
        let (_v, hf_closed) = play(|| (h.next(), h.is_active()), SR as usize);
        let mut h2 = Hat::new(SR);
        h2.trigger(0.8, true);
        let (_v, hf_open) = play(|| (h2.next(), h2.is_active()), SR as usize);
        assert!(hf_open > hf_closed, "open hat should ring longer than closed");

        let mut c = Clap::new(SR);
        c.trigger(0.9);
        let (_v, cf) = play(|| (c.next(), c.is_active()), SR as usize);
        assert!(cf > 100 && !c.is_active());
    }

    #[test]
    fn idle_drums_are_silent() {
        let mut k = Kick::new(SR);
        assert_eq!(k.next(), 0.0);
        assert!(!k.is_active());
    }
}
