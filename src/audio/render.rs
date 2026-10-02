//! The deterministic, device-free **offline renderer** and the [`AudioSource`] trait it
//! drives.
//!
//! The critical architectural rule (see the module docs): the *same* [`AudioSource`] is
//! driven by both this offline renderer and — under `audio-cpal` — the realtime device
//! callback. There is no separate "test synthesizer". Offline rendering is what CI,
//! deterministic tests, profiling and listening review all use, so correctness proven
//! offline is correctness on the device.

use std::time::{Duration, Instant};

use crate::audio::buffer::StereoBlock;
use crate::audio::report::AudioReport;
use crate::audio::time::{SampleRate, SampleTime};

/// Per-block context handed to an [`AudioSource`]: the sample rate and the absolute
/// sample position of the block's first frame.
#[derive(Debug, Clone, Copy)]
pub struct RenderCtx {
    /// The output sample rate.
    pub sr: SampleRate,
    /// Absolute sample position of `out.left[0]` from the transport origin.
    pub start: SampleTime,
}

/// A block-processing audio producer.
///
/// Deterministic given its own state and the [`RenderCtx`]. The renderer pre-zeroes the
/// output block; a source is free to write or accumulate into it.
pub trait AudioSource {
    /// Fill `out` with audio for `[ctx.start, ctx.start + out.frames())`.
    fn render(&mut self, out: &mut StereoBlock, ctx: &RenderCtx);

    /// Currently active voice count — diagnostics only. Default `0`.
    fn active_voices(&self) -> usize {
        0
    }

    /// True once the source will produce only silence from `at` onward, letting a caller
    /// stop early. Default: never finished (the caller bounds by an explicit duration).
    fn is_finished(&self, _at: SampleTime) -> bool {
        false
    }
}

/// A fixed-block-size offline renderer.
#[derive(Debug, Clone, Copy)]
pub struct OfflineRenderer {
    sr: SampleRate,
    block: usize,
}

impl OfflineRenderer {
    /// A renderer at `sr` using `block`-frame processing chunks (min 1).
    pub fn new(sr: SampleRate, block: usize) -> OfflineRenderer {
        OfflineRenderer {
            sr,
            block: block.max(1),
        }
    }

    /// The sample rate.
    #[inline]
    pub fn sample_rate(&self) -> SampleRate {
        self.sr
    }

    /// The block size in frames.
    #[inline]
    pub fn block_size(&self) -> usize {
        self.block
    }

    /// Render exactly `frames` frames of `source` starting at sample 0.
    pub fn render(&self, source: &mut dyn AudioSource, frames: u64) -> RenderResult {
        let mut audio = StereoBlock::new(0);
        audio.left.reserve(frames as usize);
        audio.right.reserve(frames as usize);
        let mut work = StereoBlock::new(self.block);
        let mut reports = Vec::with_capacity((frames / self.block as u64 + 1) as usize);
        let mut pos = 0u64;
        let mut max_voices = 0usize;
        let mut had_nonfinite = false;

        while pos < frames {
            let n = ((frames - pos) as usize).min(self.block);
            if work.frames() != n {
                work = StereoBlock::new(n);
            } else {
                work.clear();
            }
            let ctx = RenderCtx {
                sr: self.sr,
                start: SampleTime(pos),
            };
            let t0 = Instant::now();
            source.render(&mut work, &ctx);
            let render_duration = t0.elapsed();

            let voices = source.active_voices();
            max_voices = max_voices.max(voices);
            if work.has_nonfinite() {
                had_nonfinite = true;
            }
            let (ldc, rdc) = work.dc_offset();
            reports.push(AudioReport {
                sample_clock: pos,
                block_size: n,
                sample_rate: self.sr.get(),
                rendered_samples: n as u64,
                active_voices: voices,
                peak: work.peak(),
                rms: work.rms(),
                dc_offset: (ldc + rdc) * 0.5,
                underruns: 0,
                command_queue_depth: 0,
                render_duration,
            });

            work.append_to(&mut audio);
            pos += n as u64;
        }

        let peak = audio.peak();
        let rms = audio.rms();
        RenderResult {
            audio,
            sr: self.sr,
            reports,
            peak,
            rms,
            max_active_voices: max_voices,
            had_nonfinite,
        }
    }

    /// Render `secs` seconds of `source`.
    pub fn render_seconds(&self, source: &mut dyn AudioSource, secs: f64) -> RenderResult {
        self.render(source, self.sr.samples_in(secs))
    }
}

/// The result of an offline render: the full stereo buffer plus per-block diagnostics.
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// The complete rendered stereo buffer.
    pub audio: StereoBlock,
    /// The sample rate it was rendered at.
    pub sr: SampleRate,
    /// One [`AudioReport`] per processed block.
    pub reports: Vec<AudioReport>,
    /// Peak absolute sample across the whole render.
    pub peak: f32,
    /// RMS across the whole render.
    pub rms: f32,
    /// Maximum simultaneous active voices seen across all blocks.
    pub max_active_voices: usize,
    /// True if any block contained a NaN/Inf — a hard correctness failure.
    pub had_nonfinite: bool,
}

impl RenderResult {
    /// Total rendered frame count.
    #[inline]
    pub fn frames(&self) -> usize {
        self.audio.frames()
    }

    /// Total rendered duration.
    #[inline]
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames() as f64 / self.sr.as_f64())
    }

    /// Worst-case per-block render time — a coarse CPU-headroom probe.
    pub fn worst_block_time(&self) -> Duration {
        self.reports
            .iter()
            .map(|r| r.render_duration)
            .max()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test source: a constant DC value so we can assert exact frame counts and safety.
    struct Const(f32);
    impl AudioSource for Const {
        fn render(&mut self, out: &mut StereoBlock, _ctx: &RenderCtx) {
            for i in 0..out.frames() {
                out.left[i] = self.0;
                out.right[i] = self.0;
            }
        }
        fn active_voices(&self) -> usize {
            1
        }
    }

    #[test]
    fn renders_exact_frame_count_across_partial_last_block() {
        let r = OfflineRenderer::new(SampleRate::STUDIO, 100);
        let out = r.render(&mut Const(0.25), 250); // 2 full + 1 partial(50)
        assert_eq!(out.frames(), 250);
        assert_eq!(out.reports.len(), 3);
        assert_eq!(out.reports[2].block_size, 50);
        assert!(!out.had_nonfinite);
        assert_eq!(out.max_active_voices, 1);
        assert!((out.peak - 0.25).abs() < 1e-6);
    }

    #[test]
    fn render_seconds_matches_sample_math() {
        let r = OfflineRenderer::new(SampleRate::STUDIO, 512);
        let out = r.render_seconds(&mut Const(0.0), 0.5);
        assert_eq!(out.frames(), 24_000);
    }

    #[test]
    fn deterministic_same_source_same_output() {
        let r = OfflineRenderer::new(SampleRate::STUDIO, 128);
        let a = r.render(&mut Const(0.1), 1000);
        let b = r.render(&mut Const(0.1), 1000);
        assert_eq!(a.audio, b.audio);
    }
}
