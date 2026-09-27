use std::time::{Duration, Instant};

/// Accumulated metrics on rendering performance and terminal operations.
///
/// ## Byte accounting
///
/// Byte counters measure **wire bytes written for that operation**, including the
/// control sequences (synchronized-update markers, cursor motion, SGR resets)
/// that the engine itself emits. They are grouped by *operation*, not by
/// character class:
///
/// * [`RenderStats::frame_bytes`] — bytes emitted by live differential frames.
/// * [`RenderStats::commit_bytes`] — bytes emitted by commits to scrollback.
/// * [`RenderStats::insertion_bytes`] — bytes emitted by
///   [`crate::Context::insert_raw_lines_before_live_unchecked`] operations.
/// * [`RenderStats::control_bytes`] — bytes emitted by standalone control
///   operations such as [`crate::Context::clear_live_region`].
/// * [`RenderStats::total_terminal_bytes`] — the sum of the four above.
///
/// There is deliberately no field called "total wire output" separate from the
/// sum, because the sum is the total for operations the engine accounts for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct RenderStats {
    pub frames: u64,
    pub skipped_frames: u64,
    /// Logical **affected footprint**, not an exact state delta: the cells
    /// *addressed* by update semantics (explicit changed runs ∪ erase-to-EOL
    /// region ∪ cleared trailing rows). A single `CSI K` can address many
    /// already-blank cells, and removing rows can make this exceed
    /// [`RenderStats::total_cells`]. See [`crate::SurfaceDiff`] for the exact
    /// (`exact_changed_cell_count`) vs affected (`affected_cell_count`) ontology.
    pub dirty_cells: u64,
    pub total_cells: u64,
    /// Bytes emitted by live differential frames (control sequences included).
    pub frame_bytes: u64,
    pub full_repaints: u64,
    pub last_render_duration_micros: u64,

    // --- scrollback insertion accounting ---
    pub history_insertions: u64,
    /// Number of insertions that had to repaint the live framebuffer
    /// (`RepaintFallback`). Counts physical live repaints caused by insertion.
    pub insertion_repaints: u64,
    /// Insertions that used the `InsertLineFastPath` (no live repaint).
    pub fast_insertions: u64,
    pub insertion_bytes: u64,

    // --- anchor / resize accounting ---
    /// Number of times the physical live-region anchor was invalidated and
    /// re-established (e.g. after a terminal resize).
    pub anchor_resyncs: u64,

    // --- other operation byte accounting ---
    pub commit_bytes: u64,
    pub control_bytes: u64,
}

impl RenderStats {
    /// Total bytes accounted for across frame, commit, insertion and control operations.
    pub fn total_terminal_bytes(&self) -> u64 {
        self.frame_bytes + self.commit_bytes + self.insertion_bytes + self.control_bytes
    }

    /// Backwards-compatible alias for [`RenderStats::frame_bytes`].
    #[deprecated(note = "use `frame_bytes`; the old name implied total wire output")]
    pub fn bytes_emitted(&self) -> u64 {
        self.frame_bytes
    }
}

/// Default recommended cadence for decorative animations such as spinners.
///
/// 60 FPS is a *ceiling* for input latency, not a target for a spinner. Running a
/// spinner at 12.5 Hz is visually smooth and an order of magnitude cheaper.
pub const DEFAULT_ANIMATION_INTERVAL: Duration = Duration::from_millis(80);

/// How frame deadlines are derived from a requested maximum frame rate.
///
/// `CompletionRelative` preserves LibGibson's historical behavior: the next
/// budget starts when the previous render completes. `PhaseLocked` keeps a
/// cadence anchored to scheduled deadlines instead, so render/write time consumes
/// part of the current period rather than silently stretching every period. This
/// is useful for high-cadence animation and temporal-rendering experiments; it is
/// still an application emission clock, **not** a claim of terminal/display vsync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FramePacing {
    #[default]
    CompletionRelative,
    PhaseLocked,
}

/// Throttles and coalesces rendering updates to a target frame rate.
pub struct FrameScheduler {
    /// FPS ceiling. Private since 0.3.0: mutate only through
    /// [`FrameScheduler::set_max_fps`], which resets the timing epoch so a stale
    /// phase-locked deadline cannot leak across a cadence change. Read it with
    /// [`FrameScheduler::max_fps`].
    max_fps: u32,
    pub is_dirty: bool,
    last_frame_instant: Option<Instant>,
    next_frame_deadline: Option<Instant>,
    pacing: FramePacing,
    /// Scheduled emission deadlines the most recent phase-locked frame overran.
    /// Rust-side observability only; deliberately not part of ABI-v1 RenderStats.
    missed_periods_last_frame: u32,
    /// Cumulative phase-locked missed deadlines since construction (monotonic).
    missed_periods_total: u64,
    /// Recommended interval between decorative animation steps.
    pub animation_interval: Duration,
    pub stats: RenderStats,
}

impl FrameScheduler {
    pub fn new(max_fps: u32) -> Self {
        Self {
            max_fps: max_fps.max(1),
            is_dirty: false,
            last_frame_instant: None,
            next_frame_deadline: None,
            pacing: FramePacing::CompletionRelative,
            missed_periods_last_frame: 0,
            missed_periods_total: 0,
            animation_interval: DEFAULT_ANIMATION_INTERVAL,
            stats: RenderStats::default(),
        }
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    /// Explicit request to schedule a frame render pass.
    pub fn request_render(&mut self) {
        self.is_dirty = true;
    }

    /// Returns the active frame-pacing policy.
    pub fn pacing(&self) -> FramePacing {
        self.pacing
    }

    /// Scheduled emission deadlines the most recent phase-locked frame overran.
    ///
    /// Under [`FramePacing::PhaseLocked`], a frame whose generation+write took
    /// longer than one budget period causes the scheduler to skip the deadlines
    /// it blew past (without drifting). This reports how many were skipped by the
    /// last recorded frame; it is always `0` under `CompletionRelative` and `0`
    /// immediately after a timing-epoch reset. A temporal controller can use a
    /// nonzero value to reduce depth or fall back to static realization.
    pub fn missed_periods_last_frame(&self) -> u32 {
        self.missed_periods_last_frame
    }

    /// Cumulative phase-locked missed deadlines since construction (monotonic).
    ///
    /// Intended for hysteresis: a controller can sample the delta over a window
    /// to decide whether local cadence is healthy enough to keep modulating.
    pub fn missed_periods_total(&self) -> u64 {
        self.missed_periods_total
    }

    /// Selects the frame-pacing policy and starts a fresh timing epoch.
    ///
    /// Switching policy deliberately resets only scheduler timing, not dirty state
    /// or accumulated metrics. The next requested frame is therefore immediately
    /// eligible and establishes the new phase.
    pub fn set_pacing(&mut self, pacing: FramePacing) {
        if self.pacing != pacing {
            self.pacing = pacing;
            self.last_frame_instant = None;
            self.next_frame_deadline = None;
            self.missed_periods_last_frame = 0;
        }
    }

    /// Changes the FPS ceiling and starts a fresh timing epoch when it changes.
    ///
    /// Resetting the epoch matters for phase-locked pacing: an old deadline was
    /// derived from the old period and must not leak into the new cadence.
    pub fn set_max_fps(&mut self, max_fps: u32) {
        let max_fps = max_fps.max(1);
        if self.max_fps != max_fps {
            self.max_fps = max_fps;
            self.last_frame_instant = None;
            self.next_frame_deadline = None;
            self.missed_periods_last_frame = 0;
        }
    }

    /// The current FPS ceiling (always at least 1).
    pub fn max_fps(&self) -> u32 {
        self.max_fps
    }

    pub fn frame_budget(&self) -> Duration {
        Duration::from_nanos((1_000_000_000u64) / (self.max_fps as u64))
    }

    /// The recommended cadence for decorative animation (spinners, pulsing
    /// status). Always at least as slow as the frame budget.
    pub fn animation_interval(&self) -> Duration {
        self.animation_interval.max(self.frame_budget())
    }

    /// Sets a custom animation cadence, clamped to the frame budget.
    pub fn set_animation_interval(&mut self, interval: Duration) {
        self.animation_interval = interval.max(self.frame_budget());
    }

    /// Time remaining until the next frame can be rendered under the FPS budget.
    pub fn time_until_next_frame(&self) -> Duration {
        self.time_until_next_frame_at(Instant::now())
    }

    fn time_until_next_frame_at(&self, now: Instant) -> Duration {
        match self.pacing {
            FramePacing::CompletionRelative => match self.last_frame_instant {
                None => Duration::ZERO,
                Some(last) => {
                    let budget = self.frame_budget();
                    let elapsed = now.saturating_duration_since(last);
                    if elapsed >= budget {
                        Duration::ZERO
                    } else {
                        budget - elapsed
                    }
                }
            },
            FramePacing::PhaseLocked => self
                .next_frame_deadline
                .map(|deadline| deadline.saturating_duration_since(now))
                .unwrap_or(Duration::ZERO),
        }
    }

    /// True when the frame budget currently permits a render, regardless of dirty state.
    pub fn frame_due(&self) -> bool {
        self.time_until_next_frame().is_zero()
    }

    /// Checks if a frame should be rendered now, respecting the frame rate budget.
    pub fn should_render(&mut self) -> bool {
        self.should_render_at(Instant::now())
    }

    fn should_render_at(&mut self, now: Instant) -> bool {
        if !self.is_dirty {
            return false;
        }

        if self.pacing == FramePacing::PhaseLocked && self.next_frame_deadline.is_none() {
            // Anchor the phase at the *start* of the first eligible frame. Its
            // render/write cost therefore consumes this period instead of being
            // added after it.
            self.next_frame_deadline = Some(now);
            return true;
        }

        if self.time_until_next_frame_at(now).is_zero() {
            true
        } else {
            self.stats.skipped_frames += 1;
            false
        }
    }

    /// Records metrics from a completed frame.
    pub fn record_frame(
        &mut self,
        dirty_cells: usize,
        total_cells: usize,
        bytes_emitted: usize,
        is_full_repaint: bool,
        duration: Duration,
    ) {
        self.record_frame_at(
            Instant::now(),
            dirty_cells,
            total_cells,
            bytes_emitted,
            is_full_repaint,
            duration,
        );
    }

    fn record_frame_at(
        &mut self,
        now: Instant,
        dirty_cells: usize,
        total_cells: usize,
        bytes_emitted: usize,
        is_full_repaint: bool,
        duration: Duration,
    ) {
        self.is_dirty = false;
        self.last_frame_instant = Some(now);

        if self.pacing == FramePacing::PhaseLocked {
            let budget = self.frame_budget();
            let budget_ns = budget.as_nanos();
            let scheduled = self.next_frame_deadline.unwrap_or(now);
            let mut next = scheduled + budget;
            if budget_ns > 0 {
                if next <= now {
                    // Preserve the original phase while skipping every deadline we
                    // already missed. Use the remainder rather than a loop so a long
                    // process stall cannot turn catch-up into unbounded work.
                    let late = now.saturating_duration_since(next);
                    let rem_ns = late.as_nanos() % budget_ns;
                    let until = if rem_ns == 0 {
                        budget
                    } else {
                        budget - Duration::from_nanos(rem_ns as u64)
                    };
                    next = now + until;
                }
                // `next` is `scheduled + k*budget` for some `k >= 1`; the frame's
                // generation+write skipped the `k - 1` deadlines in between.
                let periods = next.saturating_duration_since(scheduled).as_nanos() / budget_ns;
                self.missed_periods_last_frame = periods.saturating_sub(1) as u32;
            } else {
                // Degenerate ceiling (max_fps beyond ~1e9): no real period to miss.
                next = now;
                self.missed_periods_last_frame = 0;
            }
            self.next_frame_deadline = Some(next);
            self.missed_periods_total = self
                .missed_periods_total
                .saturating_add(self.missed_periods_last_frame as u64);
        } else {
            self.next_frame_deadline = None;
            self.missed_periods_last_frame = 0;
        }

        self.stats.frames += 1;
        self.stats.dirty_cells += dirty_cells as u64;
        self.stats.total_cells += total_cells as u64;
        self.stats.frame_bytes += bytes_emitted as u64;
        if is_full_repaint {
            self.stats.full_repaints += 1;
        }
        self.stats.last_render_duration_micros = duration.as_micros() as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_budget_and_throttling() {
        let mut scheduler = FrameScheduler::new(30);
        assert!(!scheduler.should_render());

        scheduler.request_render();
        assert!(scheduler.should_render());

        // Record a frame
        scheduler.record_frame(10, 100, 50, false, Duration::from_micros(200));
        assert_eq!(scheduler.stats.frames, 1);
        assert_eq!(scheduler.stats.frame_bytes, 50);
        assert!(!scheduler.is_dirty);

        // Mark dirty immediately; frame budget has not elapsed
        scheduler.request_render();
        assert!(!scheduler.should_render());
        assert_eq!(scheduler.stats.skipped_frames, 1);
    }

    #[test]
    fn phase_locked_pacing_charges_render_time_to_the_current_period() {
        let start = Instant::now();
        let mut scheduler = FrameScheduler::new(100); // 10 ms budget
        scheduler.set_pacing(FramePacing::PhaseLocked);
        scheduler.request_render();
        assert!(scheduler.should_render_at(start));

        // A frame that completes 3 ms after its scheduled start leaves 7 ms,
        // rather than historical completion-relative pacing's fresh 10 ms.
        let completed = start + Duration::from_millis(3);
        scheduler.record_frame_at(completed, 0, 0, 0, false, Duration::from_millis(3));
        assert_eq!(
            scheduler.time_until_next_frame_at(completed),
            Duration::from_millis(7)
        );
    }

    #[test]
    fn phase_locked_pacing_skips_missed_deadlines_without_drifting() {
        let start = Instant::now();
        let mut scheduler = FrameScheduler::new(100); // 10 ms budget
        scheduler.set_pacing(FramePacing::PhaseLocked);
        scheduler.request_render();
        assert!(scheduler.should_render_at(start));

        // Completion at t=23 ms has missed t=10 and t=20. The next deadline
        // remains on the original phase at t=30, i.e. 7 ms away.
        let completed = start + Duration::from_millis(23);
        scheduler.record_frame_at(completed, 0, 0, 0, false, Duration::from_millis(23));
        assert_eq!(
            scheduler.time_until_next_frame_at(completed),
            Duration::from_millis(7)
        );
    }

    #[test]
    fn changing_fps_resets_phase_locked_epoch() {
        let start = Instant::now();
        let mut scheduler = FrameScheduler::new(100);
        scheduler.set_pacing(FramePacing::PhaseLocked);
        scheduler.request_render();
        assert!(scheduler.should_render_at(start));
        scheduler.record_frame_at(
            start + Duration::from_millis(2),
            0,
            0,
            0,
            false,
            Duration::from_millis(2),
        );
        assert!(
            scheduler.time_until_next_frame_at(start + Duration::from_millis(2)) > Duration::ZERO
        );

        scheduler.set_max_fps(50);
        scheduler.request_render();
        assert!(scheduler.should_render_at(start + Duration::from_millis(2)));
        assert_eq!(scheduler.frame_budget(), Duration::from_millis(20));
    }

    #[test]
    fn phase_locked_reports_missed_periods_after_overrun() {
        let start = Instant::now();
        let mut scheduler = FrameScheduler::new(100); // 10 ms budget
        scheduler.set_pacing(FramePacing::PhaseLocked);
        scheduler.request_render();
        assert!(scheduler.should_render_at(start));

        // A frame that finishes at t=23 ms overran the t=10 and t=20 deadlines.
        let completed = start + Duration::from_millis(23);
        scheduler.record_frame_at(completed, 0, 0, 0, false, Duration::from_millis(23));
        assert_eq!(scheduler.missed_periods_last_frame(), 2);
        assert_eq!(scheduler.missed_periods_total(), 2);
    }

    #[test]
    fn phase_locked_reports_zero_missed_within_budget() {
        let start = Instant::now();
        let mut scheduler = FrameScheduler::new(100);
        scheduler.set_pacing(FramePacing::PhaseLocked);
        scheduler.request_render();
        assert!(scheduler.should_render_at(start));

        let completed = start + Duration::from_millis(3);
        scheduler.record_frame_at(completed, 0, 0, 0, false, Duration::from_millis(3));
        assert_eq!(scheduler.missed_periods_last_frame(), 0);
        assert_eq!(scheduler.missed_periods_total(), 0);
    }

    #[test]
    fn completion_relative_never_reports_missed_periods() {
        let start = Instant::now();
        let mut scheduler = FrameScheduler::new(100); // default CompletionRelative
        scheduler.request_render();
        assert!(scheduler.should_render_at(start));

        // Even a frame far longer than the budget reports nothing: completion-
        // relative pacing has no scheduled deadlines to miss.
        let completed = start + Duration::from_millis(85);
        scheduler.record_frame_at(completed, 0, 0, 0, false, Duration::from_millis(85));
        assert_eq!(scheduler.missed_periods_last_frame(), 0);
        assert_eq!(scheduler.missed_periods_total(), 0);
    }

    #[test]
    fn missed_periods_accumulate_and_last_frame_resets_on_epoch_change() {
        let start = Instant::now();
        let mut scheduler = FrameScheduler::new(100); // 10 ms budget
        scheduler.set_pacing(FramePacing::PhaseLocked);
        scheduler.request_render();
        assert!(scheduler.should_render_at(start));

        // First overrun finishes at t=23, missing t=10 and t=20 (2 periods).
        let first = start + Duration::from_millis(23);
        scheduler.record_frame_at(first, 0, 0, 0, false, Duration::from_millis(23));
        assert_eq!(scheduler.missed_periods_last_frame(), 2);

        // Next deadline is t=30; the second frame becomes due there and overruns
        // to t=45, missing t=40 (1 period).
        scheduler.request_render();
        let due = start + Duration::from_millis(30);
        assert!(scheduler.should_render_at(due));
        let second = start + Duration::from_millis(45);
        scheduler.record_frame_at(second, 0, 0, 0, false, Duration::from_millis(15));
        assert_eq!(scheduler.missed_periods_last_frame(), 1);
        assert_eq!(scheduler.missed_periods_total(), 3);

        // A ceiling change resets the per-frame gauge but preserves the monotonic
        // cumulative health counter a controller uses for hysteresis.
        scheduler.set_max_fps(50);
        assert_eq!(scheduler.missed_periods_last_frame(), 0);
        assert_eq!(scheduler.missed_periods_total(), 3);
    }

    #[test]
    fn test_animation_interval_clamped_to_budget() {
        let mut scheduler = FrameScheduler::new(120);
        // 120 FPS budget ~= 8.3ms; a 1ms animation must be clamped up to the budget.
        scheduler.set_animation_interval(Duration::from_millis(1));
        assert!(scheduler.animation_interval() >= scheduler.frame_budget());
        assert!(scheduler.animation_interval() >= Duration::from_millis(8));
    }

    #[test]
    fn test_total_terminal_bytes_is_sum() {
        let s = RenderStats {
            frame_bytes: 10,
            commit_bytes: 20,
            insertion_bytes: 30,
            control_bytes: 40,
            ..Default::default()
        };
        assert_eq!(s.total_terminal_bytes(), 100);
    }
}
