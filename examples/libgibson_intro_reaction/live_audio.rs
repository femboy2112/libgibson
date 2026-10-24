//! Live realtime audio for the reaction cut — the first in-tree caller of
//! `gibson::audio`'s optional `audio-cpal` device backend, and the example's first
//! A/V-sync driver.
//!
//! This module compiles only under the `audio-cpal` feature (its `mod` declaration in the
//! example root is `#[cfg]`-gated). With the feature off — the default — the example builds
//! and runs exactly as before, silently; a tiny `LiveAudio` stub in the example root
//! provides the same method surface so the live loop needs no `#[cfg]` of its own.
//!
//! # The audio playhead is the master A/V clock
//!
//! [`AudioDevice`] renders an [`AudioSource`] on its own realtime thread, paced by the
//! sound card. There is no drift-prone wall clock: the source advances an integer sample
//! playhead by exactly the frames it rendered, and the visual loop *reads* that playhead to
//! choose which edit-second to draw. A slow terminal frame can never pull speech out of
//! sync — the audio plays on and the next video frame catches up to wherever it is. This is
//! `gibson::audio`'s own A/V contract (see `src/audio/time.rs`): video may stutter, audio
//! stays monotonic.
//!
//! Pause / seek / restart live *inside* the source behind lock-free atomics, because the
//! device exposes no transport of its own after [`AudioDevice::play`].

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use gibson::audio::buffer::StereoBlock;
use gibson::audio::device::AudioDevice;
use gibson::audio::media::{extract_clip_audio, DialogueTrack};
use gibson::audio::render::{AudioSource, RenderCtx};
use gibson::audio::{SampleRate, SampleTime};

use crate::director;

/// A short click-safe fade applied to each dialogue clip's edges. The cut's in/out points
/// already sit in breath-gaps and clear of dissolves, but a hard PCM edit still steps the
/// waveform; ~6 ms of raised edge removes the click without shifting timing or audibly
/// softening a word. Set to `0.0` to reproduce the exact hard-cut assembly.
const EDGE_FADE_S: f32 = 0.006;

/// Sentinel in `seek_to` meaning "no seek pending" (a real seek is clamped into `[0, end]`).
const NO_SEEK: u64 = u64::MAX;

/// Shared transport state between the main (video) thread and the audio render thread.
///
/// Every field is lock-free: the realtime render thread must never block. The playhead is
/// the single source of A/V truth — written by the render thread, read by the video loop.
struct Control {
    /// Master playhead, in samples at `sr`. The A/V clock.
    playhead: AtomicU64,
    /// Whether playback advances (false = paused: render emits silence, playhead frozen).
    playing: AtomicBool,
    /// A pending absolute seek target in samples, or [`NO_SEEK`].
    seek_to: AtomicU64,
    /// End of content in samples; the playhead holds here rather than running off the tail.
    end: u64,
    /// The rate everything is clocked at (the rate the device actually negotiated).
    sr: SampleRate,
}

impl Control {
    /// The master A/V position in seconds.
    fn seconds(&self) -> f64 {
        self.playhead.load(Ordering::Relaxed) as f64 / self.sr.as_f64()
    }

    /// Request a seek to an absolute `secs`, clamped into the cut.
    fn request_seek_seconds(&self, secs: f64) {
        let t = (secs.max(0.0) * self.sr.as_f64()).round() as u64;
        self.seek_to.store(t.min(self.end), Ordering::Relaxed);
    }
}

/// The interactive [`AudioSource`]: a [`DialogueTrack`] played through the shared
/// [`Control`]'s playhead, with pause/seek applied at block boundaries.
struct FinaleSource {
    track: DialogueTrack,
    ctl: Arc<Control>,
}

impl AudioSource for FinaleSource {
    fn render(&mut self, out: &mut StereoBlock, ctx: &RenderCtx) {
        // Apply a pending seek first, so scrubbing moves the playhead even while paused.
        let seek = self.ctl.seek_to.swap(NO_SEEK, Ordering::Relaxed);
        if seek != NO_SEEK {
            self.ctl.playhead.store(seek, Ordering::Relaxed);
        }
        if !self.ctl.playing.load(Ordering::Relaxed) {
            return; // paused: the block is pre-zeroed; playhead frozen.
        }
        let head = self.ctl.playhead.load(Ordering::Relaxed);
        if head >= self.ctl.end {
            return; // held at the end: silent, clock frozen at total.
        }
        // Render the placed dialogue for [head, head + frames) at the *real* playhead, not
        // the device's free-running counter (`ctx.start`), so pause/seek stay authoritative.
        let inner = RenderCtx {
            sr: ctx.sr,
            start: SampleTime(head),
        };
        self.track.render(out, &inner);
        let next = head.saturating_add(out.frames() as u64).min(self.ctl.end);
        self.ctl.playhead.store(next, Ordering::Relaxed);
    }
}

/// Decode every cue's source window at `sr` and place it on the edit timeline, mirroring
/// [`director::CUES`] exactly (each cue's source span at its edit-start, natural speed).
/// Returns `None` if ffmpeg is unavailable or any window fails to decode.
fn build_track(clip: &Path, sr: SampleRate) -> Option<DialogueTrack> {
    let mut track = DialogueTrack::new();
    for c in director::CUES.iter() {
        match extract_clip_audio(clip, c.src_start as f64, c.src_dur as f64, sr) {
            Ok(mut clip_audio) => {
                if EDGE_FADE_S > 0.0 {
                    clip_audio.apply_fades(EDGE_FADE_S, EDGE_FADE_S);
                }
                track.place(clip_audio, c.edit_start as f64, 1.0);
            }
            Err(e) => {
                eprintln!("live audio: cue '{}' extract failed: {e}", c.name);
                return None;
            }
        }
    }
    if track.is_empty() {
        None
    } else {
        Some(track)
    }
}

/// Decode a whole pre-assembled dialogue file (the embedded `.m4a`) at `sr` as one clip
/// placed at edit 0 — the embed carries the finished 132 s mix, not per-cue windows.
fn build_track_from_file(path: &Path, sr: SampleRate) -> Option<DialogueTrack> {
    // dur huge => ffmpeg decodes to end-of-file.
    match extract_clip_audio(path, 0.0, 100_000.0, sr) {
        Ok(clip) => {
            let mut track = DialogueTrack::new();
            track.place(clip, 0.0, 1.0);
            Some(track)
        }
        Err(e) => {
            eprintln!("live audio: embedded dialogue decode failed: {e}");
            None
        }
    }
}

/// Live audio handle: owns the device stream and the shared transport. Dropping it stops
/// the stream (via [`AudioDevice`]'s own `Drop`); [`LiveAudio::stop`] makes that explicit.
pub struct LiveAudio {
    // Field order is load-bearing for drop: the device (and its render thread) is torn down
    // before the `Arc<Control>` it shares is released. `AudioDevice` is not `Send` and stays
    // on the thread that opened it — which is the same thread that drives the video loop.
    device: AudioDevice,
    ctl: Arc<Control>,
}

impl LiveAudio {
    /// Build the dialogue from `clip` and start playback, with the audio playhead as the
    /// master A/V clock.
    ///
    /// Returns `None` (never an error) if no output device is available or the stream fails
    /// to build: the caller then plays the film silently, exactly as before. `initial_seconds`
    /// seeds the playhead; `playing` starts it running (pass `false` for `--freeze`).
    ///
    /// The device does **not** resample, so the PCM must match the negotiated rate. Common
    /// case: decode at 48 kHz, open, and the device honors it — a single open. On the rare
    /// machine that negotiates a different rate, re-decode once at that rate and reopen.
    pub fn start(clip: &Path, initial_seconds: f32, playing: bool) -> Option<LiveAudio> {
        Self::start_with(initial_seconds, playing, |sr| build_track(clip, sr))
    }

    /// Like [`LiveAudio::start`], but from the embedded, pre-assembled dialogue bytes (the
    /// `.m4a` the self-contained finale carries). Writes them to a temp file so ffmpeg can
    /// decode a path, decodes the whole mix as one clip, then removes the temp.
    pub fn start_from_embedded(
        m4a: &[u8],
        initial_seconds: f32,
        playing: bool,
    ) -> Option<LiveAudio> {
        let tmp = std::env::temp_dir().join(format!(
            "libgibson_finale_dialogue_{}.m4a",
            std::process::id()
        ));
        if std::fs::write(&tmp, m4a).is_err() {
            return None;
        }
        let out = Self::start_with(initial_seconds, playing, |sr| {
            build_track_from_file(&tmp, sr)
        });
        // The decode has already read the file into memory by now; clean up best-effort.
        let _ = std::fs::remove_file(&tmp);
        out
    }

    /// Shared device-open path: build a track at the requested rate, open the stream, and
    /// verify the device honored the rate (it does not resample); re-decode once at the
    /// negotiated rate otherwise. `make_track` is the only thing that differs between the
    /// clip and embedded constructors.
    fn start_with(
        initial_seconds: f32,
        playing: bool,
        mut make_track: impl FnMut(SampleRate) -> Option<DialogueTrack>,
    ) -> Option<LiveAudio> {
        let mut sr = SampleRate::STUDIO;
        for _attempt in 0..2 {
            let track = make_track(sr)?;
            let end = track.end_sample();
            let start_sample = ((initial_seconds.max(0.0) as f64) * sr.as_f64()).round() as u64;
            let ctl = Arc::new(Control {
                playhead: AtomicU64::new(start_sample.min(end)),
                playing: AtomicBool::new(playing),
                seek_to: AtomicU64::new(NO_SEEK),
                end,
                sr,
            });
            let source = FinaleSource {
                track,
                ctl: Arc::clone(&ctl),
            };
            let device = AudioDevice::play(Box::new(source), sr).ok()?;
            let got = device.sample_rate();
            if got == sr {
                return Some(LiveAudio { device, ctl });
            }
            // Rare: the device negotiated a different rate than requested. Stop, re-decode
            // at its rate, and try once more (requesting a device's own rate should hold).
            device.stop();
            sr = got;
        }
        None
    }

    /// The master A/V position in seconds — the value the video loop should draw.
    pub fn clock_seconds(&self) -> Option<f64> {
        Some(self.ctl.seconds())
    }

    /// Toggle play/pause; returns the new playing state.
    pub fn toggle_pause(&self) -> bool {
        let next = !self.ctl.playing.load(Ordering::Relaxed);
        self.ctl.playing.store(next, Ordering::Relaxed);
        next
    }

    /// Seek by `delta` seconds from the current position (clamped to the cut).
    pub fn seek_by(&self, delta: f64) {
        self.ctl.request_seek_seconds(self.ctl.seconds() + delta);
    }

    /// Restart from the top.
    pub fn restart(&self) {
        self.ctl.request_seek_seconds(0.0);
    }

    /// Samples the callback had to zero-fill for want of data — 0 on healthy playback.
    pub fn underruns(&self) -> u64 {
        self.device.underruns()
    }

    /// Stop the stream explicitly (dropping the handle also stops it).
    pub fn stop(self) {
        self.device.stop();
    }
}
