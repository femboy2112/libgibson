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
    ///
    /// Computed as `step * frame` in full `u64` frame-index precision and
    /// saturated to [`Duration::MAX`] on overflow — never the old `u64 -> u32`
    /// narrowing that wrapped `now()` back to zero at frame 2^32 (issue #130).
    pub fn now(&self) -> Duration {
        mul_u64(self.step, self.frame.get())
    }

    /// Jumps the clock to an absolute `frame` index and returns the new time.
    ///
    /// Deterministic seek for scripted timelines: `seek(n)` yields the same time
    /// as a clock that has `advance`d `n` times, with no frames rendered between.
    pub fn seek(&self, frame: u64) -> Duration {
        self.frame.set(frame);
        self.now()
    }

    /// Advances one frame and returns the new time.
    pub fn advance(&self) -> Duration {
        self.frame.set(self.frame.get().saturating_add(1));
        self.now()
    }
}

// ---------------------------------------------------------------------------
// Overflow-safe duration arithmetic (issue #130).
// ---------------------------------------------------------------------------

/// `step * n` without the `Duration * u32` overflow panic or the silent
/// `u64 -> u32` frame truncation. Saturates to [`Duration::MAX`] on overflow so
/// time is monotonic non-decreasing — it never wraps back to zero.
fn mul_u64(step: Duration, n: u64) -> Duration {
    match step.as_nanos().checked_mul(n as u128) {
        Some(total) => nanos_u128_to_duration(total),
        None => Duration::MAX,
    }
}

/// u128 nanoseconds -> [`Duration`], saturating to [`Duration::MAX`] beyond the
/// representable range (`u64::MAX` seconds + 999_999_999 ns).
fn nanos_u128_to_duration(total_nanos: u128) -> Duration {
    const NANOS_PER_SEC: u128 = 1_000_000_000;
    let secs = total_nanos / NANOS_PER_SEC;
    if secs > u64::MAX as u128 {
        Duration::MAX
    } else {
        Duration::new(secs as u64, (total_nanos % NANOS_PER_SEC) as u32)
    }
}

/// Decouples a fixed-rate model clock from a variable render cadence (issue #130).
///
/// Feed each render frame's *real* elapsed time to [`advance`](Self::advance); it
/// returns how many fixed-size model ticks to run this frame, accumulating the
/// remainder so the model advances at a constant rate regardless of frame rate.
/// The total number of ticks over a span of real time is independent of how that
/// span is chopped into frames — the "no model tick change under variable FPS"
/// guarantee — up to the [`with_max_ticks`](Self::with_max_ticks) catch-up cap,
/// which bounds work per frame to prevent a spiral of death and reports when it
/// fires via [`TickDemand::clamped`].
///
/// ```
/// use std::time::Duration;
/// use gibson::FixedTimestep;
///
/// let mut ts = FixedTimestep::new(Duration::from_millis(16));
/// // A slow 50ms render frame owes 3 model ticks (48ms), 2ms carried over.
/// assert_eq!(ts.advance(Duration::from_millis(50)).ticks, 3);
/// ```
#[derive(Debug, Clone)]
pub struct FixedTimestep {
    step: Duration,
    accumulator: Duration,
    max_ticks_per_advance: u32,
}

/// How many fixed model ticks a frame demands, plus whether the catch-up cap
/// dropped a backlog (a discard receipt — see the architecture's "clear receipts
/// whenever realization loses, quantizes or discards information").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickDemand {
    /// Number of fixed model ticks to run this frame.
    pub ticks: u32,
    /// True if the catch-up cap was hit and leftover backlog was discarded.
    pub clamped: bool,
}

impl FixedTimestep {
    /// Default catch-up cap: at most this many model ticks run per `advance`.
    pub const DEFAULT_MAX_TICKS: u32 = 8;

    /// Fixed-timestep accumulator with the default catch-up cap. `step` is clamped
    /// to at least 1ns.
    pub fn new(step: Duration) -> Self {
        Self::with_max_ticks(step, Self::DEFAULT_MAX_TICKS)
    }

    /// Fixed-timestep accumulator with an explicit catch-up cap. `max_ticks` is
    /// clamped to at least 1; `step` to at least 1ns.
    pub fn with_max_ticks(step: Duration, max_ticks: u32) -> Self {
        Self {
            step: step.max(Duration::from_nanos(1)),
            accumulator: Duration::ZERO,
            max_ticks_per_advance: max_ticks.max(1),
        }
    }

    /// The fixed model timestep.
    pub fn step(&self) -> Duration {
        self.step
    }

    /// Feeds one render frame's real elapsed time and returns the ticks to run.
    pub fn advance(&mut self, real_dt: Duration) -> TickDemand {
        self.accumulator = self.accumulator.saturating_add(real_dt);
        let mut ticks = 0u32;
        while self.accumulator >= self.step && ticks < self.max_ticks_per_advance {
            self.accumulator -= self.step;
            ticks += 1;
        }
        let clamped = self.accumulator >= self.step;
        if clamped {
            // Drop the backlog beyond one step so a long stall cannot accrue an
            // unbounded debt; keep only the sub-step remainder for `alpha`.
            let rem = self.accumulator.as_nanos() % self.step.as_nanos();
            self.accumulator = nanos_u128_to_duration(rem);
        }
        TickDemand { ticks, clamped }
    }

    /// Interpolation factor in `[0, 1)`: how far the accumulator is into the next
    /// step, for smoothing render between fixed model ticks.
    pub fn alpha(&self) -> f32 {
        (self.accumulator.as_secs_f32() / self.step.as_secs_f32()).clamp(0.0, 1.0)
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
    fn now_is_exact_and_monotonic_past_the_u32_boundary() {
        // Issue #130: `now()` used `frame as u32`, so at frame 2^32 the time
        // wrapped back to ZERO while `frame()` kept climbing. These three frames
        // are the mandated boundary: 2^32 - 1, 2^32, 2^32 + 1.
        let step = Duration::from_millis(16);
        let c = FixedStepClock::new(step);

        let below = c.seek((1u64 << 32) - 1);
        let at = c.seek(1u64 << 32);
        let above = c.seek((1u64 << 32) + 1);

        // Strictly increasing across the boundary — never resets to zero.
        assert!(below < at, "now() went backwards at the u32 boundary");
        assert!(
            at < above,
            "now() went backwards just past the u32 boundary"
        );
        assert_ne!(at, Duration::ZERO, "now() wrapped to zero at frame 2^32");

        // Exact value at frame 2^32: 16ms * 4_294_967_296 = 68_719_476.736 s,
        // asserted as an independent literal, not via the formula under test.
        assert_eq!(at, Duration::new(68_719_476, 736_000_000));
        assert_eq!(at - below, step);
        assert_eq!(above - at, step);

        // frame() still reports full u64 precision.
        assert_eq!(c.frame(), (1u64 << 32) + 1);
    }

    #[test]
    fn now_saturates_without_panicking_on_overflow() {
        // A huge step and frame overflow even u128 nanoseconds; `now()` must
        // saturate to Duration::MAX, not panic (the old `Duration * u32` panics).
        let c = FixedStepClock::new(Duration::MAX);
        assert_eq!(c.seek(u64::MAX), Duration::MAX);
        // And a later seek never reports a time below an earlier one.
        let earlier = c.seek(1);
        assert!(c.seek(u64::MAX) >= earlier);
    }

    #[test]
    fn fixed_timestep_tick_count_is_fps_invariant() {
        // The same span of real time yields the same number of model ticks no
        // matter how it is chopped into frames (no catch-up clamp in play).
        let step = Duration::from_millis(16);
        let total = Duration::from_millis(1000); // 1000 / 16 = 62.5 -> 62 ticks

        let count = |chunks: &[Duration]| {
            let mut ts = FixedTimestep::with_max_ticks(step, 10_000);
            chunks.iter().map(|&dt| ts.advance(dt).ticks).sum::<u32>()
        };

        assert_eq!(count(&[total]), 62);
        assert_eq!(count(&[Duration::from_millis(100); 10]), 62);
        assert_eq!(count(&[Duration::from_millis(1); 1000]), 62);
    }

    #[test]
    fn fixed_timestep_clamps_the_spiral_of_death_and_reports_it() {
        // One enormous frame (a stall) must not demand unbounded ticks.
        let step = Duration::from_millis(16);
        let mut ts = FixedTimestep::new(step); // DEFAULT_MAX_TICKS = 8
        let demand = ts.advance(Duration::from_secs(10)); // would want 625 ticks
        assert_eq!(demand.ticks, FixedTimestep::DEFAULT_MAX_TICKS);
        assert!(
            demand.clamped,
            "backlog over the cap must report clamped=true"
        );

        // Backlog was dropped: a normal frame next does not replay the stall.
        assert!(ts.alpha() < 1.0);
        let next = ts.advance(step);
        assert_eq!(next.ticks, 1);
        assert!(!next.clamped);
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
