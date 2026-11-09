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
    /// Pause gate for [`advance`](Self::advance); `Cell` so `advance(&self)` can read it.
    paused: StdCell<bool>,
}

impl FixedStepClock {
    /// Creates a fixed-step clock. `step` is clamped to at least 1ns.
    pub fn new(step: Duration) -> Self {
        Self {
            step: step.max(Duration::from_nanos(1)),
            frame: StdCell::new(0),
            paused: StdCell::new(false),
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
    ///
    /// While [`paused`](Self::pause) this is a no-op: the model tick does not move and
    /// the unchanged time is returned. Use [`step`](Self::step) to move exactly one
    /// frame regardless of pause state.
    pub fn advance(&self) -> Duration {
        if self.paused.get() {
            return self.now();
        }
        self.step_one()
    }

    /// Freezes [`advance`](Self::advance). [`seek`](Self::seek) and
    /// [`step`](Self::step) still work — they are explicit.
    pub fn pause(&mut self) {
        self.paused.set(true);
    }

    /// Undoes [`pause`](Self::pause); `advance` moves the clock again.
    pub fn resume(&mut self) {
        self.paused.set(false);
    }

    /// True while `advance` is frozen.
    pub fn is_paused(&self) -> bool {
        self.paused.get()
    }

    /// Single-step control: advances exactly one frame whether or not the clock is
    /// paused (it does not change the pause state), and returns the new time.
    pub fn step_frame(&mut self) -> Duration {
        self.step_one()
    }

    fn step_one(&self) -> Duration {
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
    /// Exact catch-up: whole ticks beyond the cap are deferred, never discarded.
    exact: bool,
    /// Multiplier on the render dt feeding the accumulator (`1.0` = real time).
    time_scale: f64,
}

/// How many fixed model ticks a frame demands, plus whether the catch-up cap
/// dropped a backlog (a discard receipt — see the architecture's "clear receipts
/// whenever realization loses, quantizes or discards information").
///
/// Tick conservation (Law IV): `owed == executed + deferred + discarded`, always
/// (counts saturate at `u32::MAX`; unreachable for sane steps). `ticks` equals
/// `executed`; `clamped == (discarded > 0)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TickDemand {
    /// Number of fixed model ticks to run this frame (same as `executed`).
    pub ticks: u32,
    /// True if the catch-up cap was hit and leftover backlog was discarded.
    pub clamped: bool,
    /// Whole ticks the accumulated time represented at this advance.
    pub owed: u32,
    /// Whole ticks actually run this advance.
    pub executed: u32,
    /// Whole ticks kept in the accumulator for a later advance (exact policy only).
    pub deferred: u32,
    /// Whole ticks dropped by the catch-up cap (bounded policy only).
    pub discarded: u32,
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
            exact: false,
            time_scale: 1.0,
        }
    }

    /// Exact catch-up policy: like [`with_max_ticks`](Self::with_max_ticks) for
    /// per-advance work, but ticks over the cap are *deferred* (kept owed for later
    /// advances), never discarded. Total executed ticks are then independent of how
    /// real time is split across `advance` calls.
    pub fn exact(step: Duration, max_ticks: u32) -> Self {
        Self {
            exact: true,
            ..Self::with_max_ticks(step, max_ticks)
        }
    }

    /// The fixed model timestep.
    pub fn step(&self) -> Duration {
        self.step
    }

    /// Sets the model-time scale: each `advance` dt is multiplied by `scale` before it
    /// reaches the accumulator, so a producer can slow (`< 1`), fast-forward (`> 1`) or
    /// freeze (`0`) model time deterministically. Non-finite or negative values
    /// become `0.0`. The default `1.0` is bit-exact (no float math touches the dt).
    /// Already-accumulated time is not rescaled.
    pub fn set_time_scale(&mut self, scale: f64) {
        self.time_scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            0.0
        };
    }

    /// The current model-time scale (default `1.0`).
    pub fn time_scale(&self) -> f64 {
        self.time_scale
    }

    /// Feeds one render frame's real elapsed time (scaled by
    /// [`time_scale`](Self::set_time_scale)) and returns the ticks to run.
    pub fn advance(&mut self, real_dt: Duration) -> TickDemand {
        let dt = if self.time_scale == 1.0 {
            real_dt
        } else {
            // `mul_f64` panics on overflow; saturate through f64 secs instead.
            let secs = real_dt.as_secs_f64() * self.time_scale;
            Duration::try_from_secs_f64(secs).unwrap_or(Duration::MAX)
        };
        self.accumulator = self.accumulator.saturating_add(dt);
        let mut ticks = 0u32;
        while self.accumulator >= self.step && ticks < self.max_ticks_per_advance {
            self.accumulator -= self.step;
            ticks += 1;
        }
        let step_ns = self.step.as_nanos();
        let backlog = u32::try_from(self.accumulator.as_nanos() / step_ns).unwrap_or(u32::MAX);
        let (mut deferred, mut discarded) = (0u32, 0u32);
        if backlog > 0 {
            if self.exact {
                // Keep the whole-tick backlog in the accumulator; nothing is lost.
                deferred = backlog;
            } else {
                // Drop the backlog beyond one step so a long stall cannot accrue an
                // unbounded debt; keep only the sub-step remainder for `alpha`.
                discarded = backlog;
                let rem = self.accumulator.as_nanos() % step_ns;
                self.accumulator = nanos_u128_to_duration(rem);
            }
        }
        TickDemand {
            ticks,
            clamped: discarded > 0,
            owed: ticks.saturating_add(backlog),
            executed: ticks,
            deferred,
            discarded,
        }
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
    fn tick_demand_conserves_owed_under_bounded_discard() {
        let s = Duration::from_millis(16);
        let mut ts = FixedTimestep::with_max_ticks(s, 4);
        let d = ts.advance(s * 10);
        assert_eq!((d.executed, d.discarded, d.deferred, d.owed), (4, 6, 0, 10));
        assert!(d.clamped);
        assert_eq!(d.owed, d.executed + d.deferred + d.discarded);
    }

    #[test]
    fn exact_policy_total_executed_is_frame_partition_invariant() {
        let s = Duration::from_millis(16);
        // Run to quiescence: keep feeding zero time until nothing more executes.
        let total = |chunks: &[Duration]| {
            let mut ts = FixedTimestep::exact(s, 4);
            let mut sum = 0u32;
            for &dt in chunks.iter().chain(std::iter::repeat_n(&Duration::ZERO, 8)) {
                let d = ts.advance(dt);
                assert_eq!(d.discarded, 0);
                assert!(!d.clamped);
                sum += d.executed;
            }
            sum
        };
        let dt = s * 10;
        assert_eq!(total(&[dt]), 10);
        assert_eq!(total(&[dt / 2, dt / 2]), total(&[dt]));
    }

    #[test]
    fn exact_policy_defers_rather_than_discards() {
        let s = Duration::from_millis(16);
        let mut ts = FixedTimestep::exact(s, 4);
        let d = ts.advance(s * 10);
        assert_eq!(d.discarded, 0);
        assert_eq!(d.owed, 10);
        assert_eq!(d.deferred, d.owed - d.executed);
        assert_eq!(d.deferred, 6);
        // A zero-length advance drains deferred ticks (4 more, then 2).
        let next = ts.advance(Duration::ZERO);
        assert_eq!((next.executed, next.deferred, next.owed), (4, 2, 6));
        assert_eq!(ts.advance(Duration::ZERO).executed, 2);
    }

    #[test]
    fn sub_step_advance_reports_all_zero_counts() {
        let s = Duration::from_millis(16);
        let mut ts = FixedTimestep::new(s);
        let d = ts.advance(s / 2);
        assert_eq!((d.executed, d.owed, d.deferred, d.discarded), (0, 0, 0, 0));
        assert!(!d.clamped);
    }

    #[test]
    fn pause_freezes_advance_and_resume_restores_it() {
        let mut c = FixedStepClock::new(Duration::from_millis(10));
        c.advance();
        assert!(!c.is_paused());
        c.pause();
        assert!(c.is_paused());
        let frozen = c.advance();
        c.advance();
        assert_eq!(c.frame(), 1);
        assert_eq!(frozen, Duration::from_millis(10));
        c.resume();
        assert!(!c.is_paused());
        assert_eq!(c.advance(), Duration::from_millis(20));
        assert_eq!(c.frame(), 2);
    }

    #[test]
    fn step_frame_moves_exactly_one_regardless_of_pause() {
        let mut c = FixedStepClock::new(Duration::from_millis(10));
        assert_eq!(c.step_frame(), Duration::from_millis(10)); // running
        c.pause();
        assert_eq!(c.step_frame(), Duration::from_millis(20)); // paused
        assert_eq!(c.frame(), 2);
        assert!(c.is_paused(), "step must not resume");
        c.advance();
        assert_eq!(c.frame(), 2);
    }

    #[test]
    fn seek_is_unaffected_by_pause() {
        let mut c = FixedStepClock::new(Duration::from_millis(10));
        c.pause();
        assert_eq!(c.seek(7), Duration::from_millis(70));
        assert_eq!(c.frame(), 7);
        assert!(c.is_paused(), "seek must not change pause state");
    }

    #[test]
    fn time_scale_slows_speeds_and_freezes_model_time() {
        let step = Duration::from_millis(10);
        let run = |scale: f64| {
            let mut ts = FixedTimestep::with_max_ticks(step, 10_000);
            ts.set_time_scale(scale);
            (0..100)
                .map(|_| ts.advance(Duration::from_millis(10)).ticks)
                .sum::<u32>()
        };
        assert_eq!(run(1.0), 100);
        assert_eq!(run(0.5), 50);
        assert_eq!(run(2.0), 200);
        assert_eq!(run(0.0), 0);
        assert_eq!(run(f64::NAN), 0);
        assert_eq!(run(-3.0), 0);
        assert_eq!(FixedTimestep::new(step).time_scale(), 1.0);
    }

    // ---- Falsifier #2 (#130): multi-fps digest invariance --------------------

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Edit {
        Insert(u32),
    }

    fn fnv(h: u64, x: u64) -> u64 {
        (h ^ x).wrapping_mul(0x0000_0100_0000_01b3)
    }

    /// Runs one fixed logical trace (1 s of model time, 16 ms ticks, an edit on every
    /// 10th tick) "presented" at `fps` (`0` = unlimited = 1 ms frames). The consumer
    /// drains the edit stream every `drain_every` frames (`0` = blocked until the end).
    /// Returns (ticks, model-event digest, consumer digest, edit order, superseded, frames).
    fn presented(fps: u64, drain_every: u64) -> (u64, u64, u64, Vec<u32>, u64, u64) {
        use crate::frame::{Frame, FrameSource};
        use crate::snapshot::SnapshotBridge;

        let total_ns: u128 = 1_000_000_000;
        let frames = if fps == 0 { 1000 } else { fps };
        let mut ts = FixedTimestep::with_max_ticks(Duration::from_millis(16), 10_000);
        let mut bridge: SnapshotBridge<Edit> = SnapshotBridge::new();
        let (mut tick, mut model_digest, mut consumer_digest) =
            (0u64, 0xcbf2_9ce4_8422_2325u64, 0xcbf2_9ce4_8422_2325u64);
        let mut order = Vec::new();
        let drain = |bridge: &mut SnapshotBridge<Edit>, d: &mut u64, order: &mut Vec<u32>| {
            for Edit::Insert(n) in bridge.drain_commands() {
                *d = fnv(*d, n as u64);
                order.push(n);
            }
        };
        let mut prev_ns = 0u128;
        for i in 1..=frames {
            let t_ns = total_ns * i as u128 / frames as u128;
            let dt = Duration::from_nanos((t_ns - prev_ns) as u64);
            prev_ns = t_ns;
            for _ in 0..ts.advance(dt).ticks {
                tick += 1;
                model_digest = fnv(model_digest, tick); // tick event
                if tick % 10 == 0 {
                    model_digest = fnv(model_digest, 1_000_000 + tick); // edit event
                    assert!(bridge.issue(Edit::Insert(tick as u32)).is_accepted());
                }
            }
            // The presentation side: one frame per render, coalesced by seq.
            let f = Frame::from_rgb_bytes(FrameSource(0), i, 1, 1, 3, &[0, 0, 0]).unwrap();
            bridge.publish_frame(f);
            if drain_every != 0 && i % drain_every == 0 {
                drain(&mut bridge, &mut consumer_digest, &mut order);
            }
        }
        drain(&mut bridge, &mut consumer_digest, &mut order);
        assert!(bridge.commands_conserved());
        (
            tick,
            model_digest,
            consumer_digest,
            order,
            bridge.frames_superseded(),
            frames,
        )
    }

    #[test]
    fn model_digest_and_edit_order_are_invariant_across_fps_and_slow_consumers() {
        let base = presented(60, 1);
        assert_eq!(base.0, 62, "1000ms / 16ms = 62 ticks");
        assert_eq!(base.3, vec![10, 20, 30, 40, 50, 60]);

        // 15 / 30 / 60 / unlimited, plus 60fps with a slow (every 7th) and a
        // fully blocked (drain only at the end) consumer.
        let runs = [
            presented(15, 1),
            presented(30, 1),
            presented(0, 1),
            presented(60, 7),
            presented(60, 0),
        ];
        for r in &runs {
            assert_eq!((r.0, r.1, r.2, &r.3), (base.0, base.1, base.2, &base.3));
        }
        // ...while the presented-frame count (and so the coalescing) genuinely differs.
        let frames: Vec<u64> = [
            presented(15, 1),
            presented(30, 1),
            base.clone(),
            presented(0, 1),
        ]
        .iter()
        .map(|r| r.5)
        .collect();
        assert_eq!(frames, vec![15, 30, 60, 1000]);
        let sup: Vec<u64> = [
            presented(15, 1),
            presented(30, 1),
            base.clone(),
            presented(0, 1),
        ]
        .iter()
        .map(|r| r.4)
        .collect();
        assert_eq!(sup, vec![14, 29, 59, 999]);
    }

    // ---- Falsifier #3 (#130): pause/step/seek from an explicit checkpoint ----

    #[derive(Debug, Clone, Copy)]
    enum Ev {
        Tick,
        Redraw,
        Resize(u16, u16),
    }

    /// Consumer whose model state may change ONLY when the model clock moves.
    struct Consumer {
        clock: FixedStepClock,
        state: u64,
        model_updates: u32,
        non_tick_events: u32,
        size: (u16, u16),
    }

    impl Consumer {
        fn model_step(&mut self, moved: bool) {
            if moved {
                self.state = fnv(self.state, self.clock.frame());
                self.model_updates += 1;
            }
        }
        fn handle(&mut self, ev: Ev) {
            match ev {
                Ev::Tick => {
                    let before = self.clock.frame();
                    self.clock.advance();
                    self.model_step(self.clock.frame() != before);
                }
                Ev::Redraw => self.non_tick_events += 1,
                Ev::Resize(w, h) => {
                    self.size = (w, h); // view state only; never touches `state`
                    self.non_tick_events += 1;
                }
            }
        }
        fn single_step(&mut self) {
            self.clock.step_frame();
            self.model_step(true);
        }
    }

    #[test]
    fn pause_step_seek_from_checkpoint_updates_model_only_on_ticks() {
        let mut c = Consumer {
            clock: FixedStepClock::new(Duration::from_millis(10)),
            state: 0,
            model_updates: 0,
            non_tick_events: 0,
            size: (0, 0),
        };
        for ev in [Ev::Tick, Ev::Redraw, Ev::Tick, Ev::Resize(80, 24), Ev::Tick] {
            c.handle(ev);
        }
        assert_eq!(
            (c.clock.frame(), c.model_updates, c.non_tick_events),
            (3, 3, 2)
        );

        // Explicit checkpoint: frame index + consumer model state.
        let (cp_frame, cp_state, cp_updates) = (c.clock.frame(), c.state, c.model_updates);

        // Paused: ticks and non-tick events change nothing.
        c.clock.pause();
        for ev in [Ev::Tick, Ev::Tick, Ev::Redraw, Ev::Resize(120, 40)] {
            c.handle(ev);
        }
        assert_eq!(
            (c.clock.frame(), c.state, c.model_updates),
            (cp_frame, cp_state, cp_updates)
        );

        // One explicit step moves exactly one frame, exactly one model update.
        c.single_step();
        assert_eq!(
            (c.clock.frame(), c.model_updates),
            (cp_frame + 1, cp_updates + 1)
        );
        let stepped_state = c.state;
        assert_ne!(stepped_state, cp_state);
        assert!(c.clock.is_paused());

        // Resume: the next tick is one more update; a redraw in between adds none.
        c.clock.resume();
        c.handle(Ev::Redraw);
        assert_eq!(c.model_updates, cp_updates + 1);
        c.handle(Ev::Tick);
        assert_eq!(
            (c.clock.frame(), c.model_updates),
            (cp_frame + 2, cp_updates + 2)
        );

        // Seek back to the checkpoint (even while paused) and restore its state.
        c.clock.pause();
        c.clock.seek(cp_frame);
        c.state = cp_state;
        c.model_updates = cp_updates;
        assert_eq!(c.clock.frame(), cp_frame);
        c.handle(Ev::Resize(40, 10));
        assert_eq!(c.size, (40, 10));
        assert_eq!((c.state, c.model_updates), (cp_state, cp_updates));

        // Replaying forward from the checkpoint reproduces the earlier state exactly.
        c.single_step();
        assert_eq!((c.clock.frame(), c.state), (cp_frame + 1, stepped_state));
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
