//! [`AudioReport`] — the audio analogue of [`crate::FrameReport`].
//!
//! One report is produced per processed block by the offline renderer (and, under
//! `audio-cpal`, per device callback). It is deliberately Rust-side and cheap to collect,
//! so it can back deterministic tests (peak/RMS/DC/voice-count receipts) and live
//! diagnostics without touching the C ABI.

use std::time::Duration;

/// Per-block audio diagnostics.
#[derive(Debug, Clone)]
pub struct AudioReport {
    /// Absolute sample position of the block's first frame (transport origin = 0).
    pub sample_clock: u64,
    /// Frames in this block.
    pub block_size: usize,
    /// The output sample rate in Hz.
    pub sample_rate: u32,
    /// Frames rendered this block (== `block_size` for a full block).
    pub rendered_samples: u64,
    /// Voices active while rendering this block.
    pub active_voices: usize,
    /// Peak absolute sample this block (`> 1.0` means the master stage let a peak through).
    pub peak: f32,
    /// RMS this block.
    pub rms: f32,
    /// Mean sample value this block (a DC-offset probe; should stay near zero).
    pub dc_offset: f32,
    /// Callback underruns attributed to this block (offline render: always 0).
    pub underruns: u64,
    /// Depth of the command queue feeding the graph (offline render: always 0).
    pub command_queue_depth: usize,
    /// Wall-clock time spent rendering this block — a coarse CPU-cost probe.
    pub render_duration: Duration,
}

impl AudioReport {
    /// True if this block peaked above full scale (a mastering failure).
    #[inline]
    pub fn clipped(&self) -> bool {
        self.peak > 1.0
    }

    /// This block's render time as a fraction of its real-time budget. `< 1.0` means the
    /// block rendered faster than real time (i.e. there is CPU headroom for live output).
    pub fn realtime_fraction(&self) -> f64 {
        if self.sample_rate == 0 || self.block_size == 0 {
            return 0.0;
        }
        let budget = self.block_size as f64 / self.sample_rate as f64;
        self.render_duration.as_secs_f64() / budget
    }
}
