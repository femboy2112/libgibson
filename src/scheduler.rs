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

/// Throttles and coalesces rendering updates to a target frame rate.
pub struct FrameScheduler {
    pub max_fps: u32,
    pub is_dirty: bool,
    last_frame_instant: Option<Instant>,
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
        match self.last_frame_instant {
            None => Duration::ZERO,
            Some(last) => {
                let budget = self.frame_budget();
                let elapsed = last.elapsed();
                if elapsed >= budget {
                    Duration::ZERO
                } else {
                    budget - elapsed
                }
            }
        }
    }

    /// True when the frame budget currently permits a render, regardless of dirty state.
    pub fn frame_due(&self) -> bool {
        self.time_until_next_frame().is_zero()
    }

    /// Checks if a frame should be rendered now, respecting the frame rate budget.
    pub fn should_render(&mut self) -> bool {
        if !self.is_dirty {
            return false;
        }

        match self.last_frame_instant {
            None => true,
            Some(last) => {
                if last.elapsed() >= self.frame_budget() {
                    true
                } else {
                    self.stats.skipped_frames += 1;
                    false
                }
            }
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
        self.is_dirty = false;
        self.last_frame_instant = Some(Instant::now());

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
