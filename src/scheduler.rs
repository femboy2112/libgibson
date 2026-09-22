use std::time::{Duration, Instant};

/// Accumulated metrics on rendering performance and operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct RenderStats {
    pub frames: u64,
    pub skipped_frames: u64,
    pub dirty_cells: u64,
    pub total_cells: u64,
    pub bytes_emitted: u64,
    pub full_repaints: u64,
    pub last_render_duration_micros: u64,
}

/// Throttles and coalesces rendering updates to a target frame rate.
pub struct FrameScheduler {
    pub max_fps: u32,
    pub is_dirty: bool,
    last_frame_instant: Option<Instant>,
    pub stats: RenderStats,
}

impl FrameScheduler {
    pub fn new(max_fps: u32) -> Self {
        Self {
            max_fps: max_fps.max(1),
            is_dirty: false,
            last_frame_instant: None,
            stats: RenderStats::default(),
        }
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn frame_budget(&self) -> Duration {
        Duration::from_nanos((1_000_000_000u64) / (self.max_fps as u64))
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
        self.stats.bytes_emitted += bytes_emitted as u64;
        if is_full_repaint {
            self.stats.full_repaints += 1;
        }
        self.stats.last_render_duration_micros = duration.as_micros() as u64;
    }
}
