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
use gibson::audio::human_music::form::SectionKind;
use gibson::audio::human_music::score::{DrumHit, DrumVoice, Note, Provenance, Role, Score};
use gibson::audio::human_music::synth::HumanMusicSynth;
use gibson::audio::human_music::{MusicWorld, WorldId};
use gibson::audio::media::{extract_clip_audio, DialogueTrack};
use gibson::audio::render::{AudioSource, OfflineRenderer, RenderCtx};
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
    /// Pre-rendered, pre-ducked music bed (planar L/R, one sample per playhead sample).
    /// Empty when `--music` is off. Indexed by the same master playhead as the dialogue, so
    /// seek/pause/restart stay in sync for free — it is static PCM, not a live synth.
    music_l: Vec<f32>,
    music_r: Vec<f32>,
    /// Linear gain the bed is mixed in at (under the dialogue).
    music_gain: f32,
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
        // Mix the pre-rendered Spybreak bed under the dialogue at the same playhead. The bed is
        // already ducked under speech and already production-limited, so a hard clamp catches
        // only the rare dialogue+music overflow peak — transparent below unity, so the accepted
        // dialogue is untouched wherever the bed is quiet.
        if !self.music_l.is_empty() {
            let g = self.music_gain;
            let base = head as usize;
            for i in 0..out.frames() {
                let idx = base + i;
                if idx >= self.music_l.len() {
                    break;
                }
                out.left[i] = (out.left[i] + self.music_l[idx] * g).clamp(-1.0, 1.0);
                out.right[i] = (out.right[i] + self.music_r[idx] * g).clamp(-1.0, 1.0);
            }
        }
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
    pub fn start(
        clip: &Path,
        initial_seconds: f32,
        playing: bool,
        music: bool,
        music_gain: f32,
    ) -> Option<LiveAudio> {
        Self::start_with(initial_seconds, playing, music, music_gain, |sr| {
            build_track(clip, sr)
        })
    }

    /// Like [`LiveAudio::start`], but from the embedded, pre-assembled dialogue bytes (the
    /// `.m4a` the self-contained finale carries). Writes them to a temp file so ffmpeg can
    /// decode a path, decodes the whole mix as one clip, then removes the temp.
    pub fn start_from_embedded(
        m4a: &[u8],
        initial_seconds: f32,
        playing: bool,
        music: bool,
        music_gain: f32,
    ) -> Option<LiveAudio> {
        let tmp = std::env::temp_dir().join(format!(
            "libgibson_finale_dialogue_{}.m4a",
            std::process::id()
        ));
        if std::fs::write(&tmp, m4a).is_err() {
            return None;
        }
        let out = Self::start_with(initial_seconds, playing, music, music_gain, |sr| {
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
        music: bool,
        music_gain: f32,
        mut make_track: impl FnMut(SampleRate) -> Option<DialogueTrack>,
    ) -> Option<LiveAudio> {
        let mut sr = SampleRate::STUDIO;
        for _attempt in 0..2 {
            let track = make_track(sr)?;
            let end = track.end_sample();
            // Pre-render the music bed at this rate (one-time, on the main thread) so the live
            // callback only reads static PCM. Rebuilt on the rare rate-renegotiation retry.
            let (music_l, music_r) = if music {
                eprintln!("composing the music bed…");
                build_music_bed(sr, &track, end).unwrap_or_default()
            } else {
                (Vec::new(), Vec::new())
            };
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
                music_l,
                music_r,
                music_gain,
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

// ---------------------------------------------------------------------------
// The "Spybreak!"-style music bed (step 4): a dark, driving big-beat groove.
//
// Why pre-render instead of driving `HumanMusicSynth` live? The synth is forward-only — a
// forward seek burst-fires every skipped event in one sample and a backward seek goes silent —
// which fights `FinaleSource`'s seek/pause/restart. So we render the whole bed to PCM ONCE at
// startup (ducked under the dialogue), then treat it exactly like the dialogue: static samples
// indexed by the master playhead. Seek/pause/restart then stay in sync for free, and the
// realtime callback does no synthesis — only a buffer read and a multiply-add.
// ---------------------------------------------------------------------------

/// Render ONLY the music bed (no dialogue, no ducking) to planar L/R PCM for `end` samples —
/// the headless audition/regeneration path behind `--render-music`.
pub fn render_bed(sr: SampleRate, end: u64) -> Option<(Vec<f32>, Vec<f32>)> {
    build_music_bed(sr, &DialogueTrack::new(), end)
}

/// Pre-render the music bed to planar L/R PCM at `sr`, ducked under `dialogue`, for `end`
/// samples (the finale length). `None` if there is nothing to score. Runs on the main thread.
fn build_music_bed(
    sr: SampleRate,
    dialogue: &DialogueTrack,
    end: u64,
) -> Option<(Vec<f32>, Vec<f32>)> {
    if end == 0 {
        return None;
    }
    const BPM: f32 = 132.0; // big-beat tempo (also ~the 132 s finale length)
    let total_secs = end as f64 / sr.as_f64();
    let bars = ((total_secs * BPM as f64 / 60.0) / 4.0).ceil().max(1.0) as usize;
    let score = spybreak_score(BPM, bars);
    let mut world = MusicWorld::from_id(WorldId::BlackIce); // dark, Aeolian, sub-heavy palette
    world.tempo_bpm = BPM;
    let mut synth = HumanMusicSynth::new(&score, &world, sr);
    if !dialogue.is_empty() {
        // Sidechain the bed down under speech so Frank always cuts through.
        let duck = dialogue.duck_curve(end as usize, sr, 0.40, 10.0, 240.0);
        synth.set_duck(duck);
    }
    let rendered = OfflineRenderer::new(sr, 512).render(&mut synth, end);
    let audio = rendered.audio;
    Some((audio.left, audio.right))
}

fn push_drum(s: &mut Score, at: f64, v: DrumVoice, vel: f32, sec: SectionKind) {
    s.drums.push(DrumHit {
        start_beat: at,
        voice: v,
        velocity: vel,
        prov: Provenance::new(sec),
    });
}

fn push_note(s: &mut Score, at: f64, dur: f32, pitch: i32, vel: f32, role: Role, sec: SectionKind) {
    s.notes
        .push(Note::new(at, dur, pitch, vel, role, Provenance::new(sec)));
}

/// Hand-author a driving big-beat / breakbeat score in A Aeolian — the Matrix lobby-shootout
/// "Spybreak!" energy the built-in `compose()` straight-backbeat groove can't reach: a
/// syncopated kick, backbeat snare with breakbeat ghosts, 8th-note hats, a dark sub-bass riff,
/// and minor stabs. The cell loops per bar with a 2-bar intro build and an 8-bar snare fill.
fn spybreak_score(bpm: f32, bars: usize) -> Score {
    let total_beats = bars as f64 * 4.0;
    let mut s = Score::new(bpm, 4.0, total_beats);

    // A natural minor (Aeolian), tonic A: low sub-bass degrees and a mid stab triad.
    const A1: i32 = 33;
    const G1: i32 = 31;
    const F1: i32 = 29;
    const E1: i32 = 28;
    const A3: i32 = 57;
    const C4: i32 = 60;
    const E4: i32 = 64;
    const A4: i32 = 69;
    const G4: i32 = 67;
    use DrumVoice::{ClosedHat, Kick, OpenHat, Snare};

    for bar in 0..bars {
        let b = bar as f64 * 4.0;
        let intro = bar < 2;
        let sec = if intro {
            SectionKind::Intro
        } else {
            SectionKind::A
        };

        // Hats: straight 8ths, accented on the beat.
        for k in 0..8 {
            let vel = if k % 2 == 0 { 0.50 } else { 0.32 };
            push_drum(&mut s, b + k as f64 * 0.5, ClosedHat, vel, sec);
        }

        // Kick: downbeat always; the signature syncopated push + mid drive after the intro.
        push_drum(&mut s, b, Kick, 0.92, sec);
        if !intro {
            push_drum(&mut s, b + 2.5, Kick, 0.85, sec);
            if bar % 2 == 0 {
                push_drum(&mut s, b + 1.5, Kick, 0.68, sec);
            }
        }

        // Snare: backbeat + breakbeat ghosts, an open hat sizzle, an 8-bar turnaround fill.
        if !intro {
            push_drum(&mut s, b + 1.0, Snare, 0.86, sec);
            push_drum(&mut s, b + 3.0, Snare, 0.86, sec);
            push_drum(&mut s, b + 1.75, Snare, 0.33, sec);
            push_drum(&mut s, b + 3.5, Snare, 0.30, sec);
            if bar % 2 == 1 {
                push_drum(&mut s, b + 3.5, OpenHat, 0.50, sec);
            }
            if bar % 8 == 7 {
                for q in 0..4 {
                    push_drum(
                        &mut s,
                        b + 3.0 + q as f64 * 0.25,
                        Snare,
                        0.5 + 0.12 * q as f32,
                        sec,
                    );
                }
            }
        }

        // Bass: dark driving riff (enters after bar 0). Even bars pedal the root; odd bars walk
        // down A–G–F–E for movement.
        if bar >= 1 {
            if bar % 2 == 0 {
                for &t in &[0.0, 0.5, 1.0, 1.75, 2.5, 3.0] {
                    push_note(&mut s, b + t, 0.42, A1, 0.80, Role::Bass, sec);
                }
            } else {
                push_note(&mut s, b, 0.6, A1, 0.80, Role::Bass, sec);
                push_note(&mut s, b + 0.75, 0.4, A1, 0.74, Role::Bass, sec);
                push_note(&mut s, b + 1.5, 0.6, G1, 0.78, Role::Bass, sec);
                push_note(&mut s, b + 2.5, 0.5, F1, 0.78, Role::Bass, sec);
                push_note(&mut s, b + 3.0, 0.9, E1, 0.80, Role::Bass, sec);
            }
        }

        // Minor stabs (Keys): short Am triad on the off-beats — the lobby tension.
        if !intro {
            for &t in &[0.5, 2.5] {
                for &p in &[A3, C4, E4] {
                    push_note(&mut s, b + t, 0.22, p, 0.58, Role::Keys, sec);
                }
            }
        }

        // Sparse dark lead stab every 4 bars, kept minimal so it never fights the dialogue.
        if !intro && bar % 4 == 2 {
            push_note(&mut s, b, 0.5, A4, 0.50, Role::Lead, sec);
            push_note(&mut s, b + 1.0, 0.5, G4, 0.50, Role::Lead, sec);
        }
    }
    s
}
