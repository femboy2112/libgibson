//! Deterministic time and a tiny motion toolkit.
//!
//! Animation must be a **pure function of time**: application state describes
//! the target, time describes motion, rendering stays reproducible. Two clocks
//! are provided:
//!
//! * [`RealClock`] — wall-clock time for interactive use.
//! * [`FixedStepClock`] — exact `frame * step` time for scripted/auto/test runs,
//!   so a frame sequence is bit-for-bit reproducible regardless of CPU speed or
//!   scheduler jitter.
//!
//! [`TimeSource`] unifies them: call [`TimeSource::advance`] once per frame and
//! use the returned [`Duration`] as the animation `t`.

use std::cell::Cell as StdCell;
use std::time::{Duration, Instant};

/// Wall-clock time since construction.
#[derive(Debug)]
pub struct RealClock {
    start: Instant,
}

impl RealClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn now(&self) -> Duration {
        self.start.elapsed()
    }
}

impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Exact, deterministic time: `frame_index * step`.
///
/// Advancing never depends on wall-clock time, so scripted frames reproduce
/// exactly.
#[derive(Debug)]
pub struct FixedStepClock {
    step: Duration,
    frame: StdCell<u64>,
}

impl FixedStepClock {
    /// Creates a fixed-step clock. `step` is clamped to at least 1ns.
    pub fn new(step: Duration) -> Self {
        Self {
            step: step.max(Duration::from_nanos(1)),
            frame: StdCell::new(0),
        }
    }

    /// The fixed timestep.
    pub fn step(&self) -> Duration {
        self.step
    }

    /// Current frame index.
    pub fn frame(&self) -> u64 {
        self.frame.get()
    }

    /// Current time without advancing.
    pub fn now(&self) -> Duration {
        self.step * self.frame.get() as u32
    }

    /// Advances one frame and returns the new time.
    pub fn advance(&self) -> Duration {
        self.frame.set(self.frame.get() + 1);
        self.now()
    }
}

/// Per-frame time source for applications.
///
/// In [`TimeSource::Real`] mode `advance` returns wall-clock time; in
/// [`TimeSource::Fixed`] mode it returns an exact fixed-step time.
#[derive(Debug)]
pub enum TimeSource {
    Real(RealClock),
    Fixed(FixedStepClock),
}

impl TimeSource {
    /// Wall-clock time source.
    pub fn real() -> Self {
        TimeSource::Real(RealClock::new())
    }

    /// Deterministic fixed-step time source.
    pub fn fixed(step: Duration) -> Self {
        TimeSource::Fixed(FixedStepClock::new(step))
    }

    /// True for deterministic fixed-step time.
    pub fn is_fixed(&self) -> bool {
        matches!(self, TimeSource::Fixed(_))
    }

    /// Current time.
    pub fn now(&self) -> Duration {
        match self {
            TimeSource::Real(c) => c.now(),
            TimeSource::Fixed(c) => c.now(),
        }
    }

    /// Advances one frame and returns the new animation time.
    pub fn advance(&self) -> Duration {
        match self {
            TimeSource::Real(c) => c.now(),
            TimeSource::Fixed(c) => c.advance(),
        }
    }
}

// ---------------------------------------------------------------------------
// Motion helpers — all pure functions of time.
// ---------------------------------------------------------------------------

/// Fractional progress through a period, in `[0, 1)`.
pub fn phase(t: Duration, period: Duration) -> f32 {
    let p = period.as_secs_f32().max(1e-6);
    let x = (t.as_secs_f32() / p).fract();
    if x < 0.0 {
        x + 1.0
    } else {
        x
    }
}

/// A smooth 0→1→0 pulse.
pub fn pulse(t: Duration, period: Duration) -> f32 {
    let p = phase(t, period);
    (std::f32::consts::TAU * p).sin() * 0.5 + 0.5
}

/// A linear sawtooth in `[0, 1)`.
pub fn saw(t: Duration, period: Duration) -> f32 {
    phase(t, period)
}

/// A 0→1→0 triangle wave.
pub fn triangle(t: Duration, period: Duration) -> f32 {
    let p = phase(t, period);
    1.0 - (2.0 * p - 1.0).abs()
}

/// Linear interpolation.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Cubic ease-in.
pub fn ease_in(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

/// Cubic ease-out.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease-in-out.
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Damped spring-ish interpolation from 0 to 1 (critically-damped-ish shape).
pub fn spring(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (-6.0 * t).exp() * (std::f32::consts::TAU * 0.9 * t).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_step_clock_is_exact_and_reproducible() {
        let a = FixedStepClock::new(Duration::from_millis(50));
        assert_eq!(a.now(), Duration::ZERO);
        assert_eq!(a.advance(), Duration::from_millis(50));
        assert_eq!(a.advance(), Duration::from_millis(100));
        assert_eq!(a.frame(), 2);

        let b = FixedStepClock::new(Duration::from_millis(50));
        for _ in 0..2 {
            b.advance();
        }
        assert_eq!(a.now(), b.now());
    }

    #[test]
    fn time_source_fixed_mode_advances_deterministically() {
        let ts = TimeSource::fixed(Duration::from_millis(16));
        assert!(ts.is_fixed());
        assert_eq!(ts.advance(), Duration::from_millis(16));
        assert_eq!(ts.advance(), Duration::from_millis(32));
        assert_eq!(ts.now(), Duration::from_millis(32));
    }

    #[test]
    fn real_mode_is_not_fixed() {
        let ts = TimeSource::real();
        assert!(!ts.is_fixed());
    }

    #[test]
    fn motion_helpers_are_bounded_and_periodic() {
        let period = Duration::from_millis(100);
        for i in 0..40 {
            let t = Duration::from_millis(i * 7);
            assert!((0.0..=1.0).contains(&pulse(t, period)));
            assert!((0.0..=1.0).contains(&saw(t, period)));
            assert!((0.0..=1.0).contains(&triangle(t, period)));
        }
        // Phase wraps exactly at the period.
        assert!((phase(Duration::ZERO, period)).abs() < 1e-6);
        assert!((phase(period, period)).abs() < 1e-6);
    }

    #[test]
    fn easings_hit_endpoints() {
        assert_eq!(ease_in(0.0), 0.0);
        assert_eq!(ease_in(1.0), 1.0);
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_in_out(0.0), 0.0);
        assert_eq!(ease_in_out(1.0), 1.0);
        assert!((spring(0.0) - 0.0).abs() < 1e-6);
        assert!((spring(1.0) - 1.0).abs() < 0.05);
    }
}
