//! Media clip audio: extract real source audio for cue windows and play it back
//! synchronized to an edit timeline, plus the dialogue-driven ducking sidechain.
//!
//! The only recorded audio LibGibson ever touches is a local source clip the host points
//! at (never committed, never shipped). Extraction shells out to `ffmpeg` (already the
//! video-bake dependency) to decode a span to canonical f32 stereo PCM; everything else
//! in the audio axis is synthesized.
//!
//! The governing A/V rule lives here in spirit: **video may stutter, audio stays
//! monotonic.** A [`DialogueTrack`] plays placed clips at sample-accurate positions and
//! never re-triggers or reverses them for a visual effect, so a freeze/punch on screen
//! never turns speech into "st-st-stop".

use std::io;
use std::path::Path;
use std::process::Command;

use super::buffer::StereoBlock;
use super::render::{AudioSource, RenderCtx};
use super::time::SampleRate;

/// Decoded stereo f32 PCM of a source span.
#[derive(Debug, Clone)]
pub struct ClipAudio {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub sr: SampleRate,
}

impl ClipAudio {
    /// Frame count.
    pub fn frames(&self) -> usize {
        self.left.len()
    }

    /// Duration in seconds.
    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.sr.as_f64()
    }

    /// Build from interleaved little-endian f32 bytes (`L,R,L,R,…`).
    pub fn from_f32le_interleaved(bytes: &[u8], sr: SampleRate) -> ClipAudio {
        let n = bytes.len() / 8; // 2 channels * 4 bytes
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);
        for i in 0..n {
            let o = i * 8;
            let l = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
            let r = f32::from_le_bytes([bytes[o + 4], bytes[o + 5], bytes[o + 6], bytes[o + 7]]);
            left.push(l);
            right.push(r);
        }
        ClipAudio { left, right, sr }
    }

    /// Peak absolute sample.
    pub fn peak(&self) -> f32 {
        self.left
            .iter()
            .chain(self.right.iter())
            .fold(0.0f32, |a, &s| a.max(s.abs()))
    }

    /// Apply click-safe linear fades to the edges (seconds). Prevents edit clicks; does
    /// NOT fade off the end of a phrase — keep these short (~5–20 ms) unless intentional.
    pub fn apply_fades(&mut self, fade_in_s: f32, fade_out_s: f32) {
        let fi = (fade_in_s * self.sr.as_f64() as f32) as usize;
        let fo = (fade_out_s * self.sr.as_f64() as f32) as usize;
        let n = self.frames();
        for i in 0..fi.min(n) {
            let g = i as f32 / fi.max(1) as f32;
            self.left[i] *= g;
            self.right[i] *= g;
        }
        for i in 0..fo.min(n) {
            let g = i as f32 / fo.max(1) as f32;
            let idx = n - 1 - i;
            self.left[idx] *= g;
            self.right[idx] *= g;
        }
    }
}

/// Extract `[start_s, start_s+dur_s)` of `clip` to f32 stereo PCM at `sr` via ffmpeg.
///
/// Returns an error if ffmpeg is missing or fails. The decoded audio is held in memory;
/// nothing is written into the repository.
pub fn extract_clip_audio(
    clip: &Path,
    start_s: f64,
    dur_s: f64,
    sr: SampleRate,
) -> io::Result<ClipAudio> {
    // Decode to raw f32le stereo on stdout: fast, dependency-free to parse.
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &format!("{start_s}"), "-t", &format!("{dur_s}"), "-i"])
        .arg(clip)
        .args([
            "-vn",
            "-ac",
            "2",
            "-ar",
            &sr.get().to_string(),
            "-f",
            "f32le",
            "-acodec",
            "pcm_f32le",
            "-",
        ])
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "ffmpeg audio extract failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let clip = ClipAudio::from_f32le_interleaved(&out.stdout, sr);
    if clip.frames() == 0 {
        return Err(io::Error::other("ffmpeg produced no audio samples"));
    }
    Ok(clip)
}

/// A clip placed on the edit timeline at a sample offset, with a gain.
struct Placed {
    start: u64,
    clip: ClipAudio,
    gain: f32,
}

/// A track of placed dialogue clips — an [`AudioSource`] for the Dialogue bus, plus the
/// ducking sidechain source.
#[derive(Default)]
pub struct DialogueTrack {
    placed: Vec<Placed>,
    sr_hz: u32,
}

impl DialogueTrack {
    /// An empty track.
    pub fn new() -> DialogueTrack {
        DialogueTrack::default()
    }

    /// Place `clip` starting at `start_seconds` on the edit timeline with linear `gain`.
    pub fn place(&mut self, clip: ClipAudio, start_seconds: f64, gain: f32) {
        self.sr_hz = clip.sr.get();
        let start = (start_seconds * clip.sr.as_f64()).round().max(0.0) as u64;
        self.placed.push(Placed { start, clip, gain });
    }

    /// The last sample any placed clip reaches (for sizing the render).
    pub fn end_sample(&self) -> u64 {
        self.placed
            .iter()
            .map(|p| p.start + p.clip.frames() as u64)
            .max()
            .unwrap_or(0)
    }

    /// Number of placed clips.
    pub fn len(&self) -> usize {
        self.placed.len()
    }

    /// True if no clips are placed.
    pub fn is_empty(&self) -> bool {
        self.placed.is_empty()
    }

    /// A per-sample **music duck gain** in `[floor, 1]` derived from the dialogue level:
    /// where speech is present the music is pulled down toward `floor`, with a fast attack
    /// and a musical release, so the score stays perceptible without competing with speech.
    pub fn duck_curve(
        &self,
        total_samples: usize,
        sr: SampleRate,
        floor: f32,
        attack_ms: f32,
        release_ms: f32,
    ) -> Vec<f32> {
        // 1) Raw dialogue amplitude envelope (max |sample| across placed clips).
        let mut level = vec![0.0f32; total_samples];
        for p in &self.placed {
            for i in 0..p.clip.frames() {
                let s = (p.start as usize + i).min(total_samples.saturating_sub(1));
                if s >= total_samples {
                    break;
                }
                let a = p.clip.left[i].abs().max(p.clip.right[i].abs()) * p.gain;
                if a > level[s] {
                    level[s] = a;
                }
            }
        }
        // 2) Smooth with attack/release, 3) map to a duck gain.
        let atk = coef_ms(attack_ms, sr.as_f64() as f32);
        let rel = coef_ms(release_ms, sr.as_f64() as f32);
        let thresh = 0.06f32;
        let mut env = 0.0f32;
        let mut out = vec![1.0f32; total_samples];
        for i in 0..total_samples {
            let target = level[i];
            let c = if target > env { atk } else { rel };
            env += c * (target - env);
            let amt = (env / thresh).clamp(0.0, 1.0);
            out[i] = 1.0 - (1.0 - floor) * amt;
        }
        out
    }
}

impl AudioSource for DialogueTrack {
    fn render(&mut self, out: &mut StereoBlock, ctx: &RenderCtx) {
        let block_start = ctx.start.0;
        let frames = out.frames();
        for p in &self.placed {
            let clip_frames = p.clip.frames() as u64;
            // Overlap of [block_start, block_start+frames) with [p.start, p.start+clip).
            let lo = block_start.max(p.start);
            let hi = (block_start + frames as u64).min(p.start + clip_frames);
            if lo >= hi {
                continue;
            }
            for abs in lo..hi {
                let oi = (abs - block_start) as usize;
                let ci = (abs - p.start) as usize;
                out.left[oi] += p.clip.left[ci] * p.gain;
                out.right[oi] += p.clip.right[ci] * p.gain;
            }
        }
    }
}

#[inline]
fn coef_ms(ms: f32, sr: f32) -> f32 {
    let t = (ms.max(0.01) * 0.001) * sr;
    1.0 - (-1.0 / t).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::render::OfflineRenderer;

    fn tone_clip(sr: SampleRate, secs: f64, amp: f32) -> ClipAudio {
        let n = sr.samples_in(secs) as usize;
        let l: Vec<f32> = (0..n).map(|i| amp * (i as f32 * 0.05).sin()).collect();
        ClipAudio {
            left: l.clone(),
            right: l,
            sr,
        }
    }

    #[test]
    fn interleave_parse_round_trips() {
        let sr = SampleRate::STUDIO;
        let mut bytes = Vec::new();
        for i in 0..4 {
            bytes.extend_from_slice(&(i as f32).to_le_bytes());
            bytes.extend_from_slice(&((i as f32) + 0.5).to_le_bytes());
        }
        let c = ClipAudio::from_f32le_interleaved(&bytes, sr);
        assert_eq!(c.frames(), 4);
        assert_eq!(c.left[2], 2.0);
        assert_eq!(c.right[2], 2.5);
    }

    #[test]
    fn fades_zero_the_edges() {
        let sr = SampleRate::STUDIO;
        let mut c = tone_clip(sr, 0.5, 0.9);
        c.apply_fades(0.01, 0.01);
        assert!(c.left[0].abs() < 1e-6);
        assert!(c.left[c.frames() - 1].abs() < 1e-6);
    }

    #[test]
    fn placed_clip_renders_at_its_offset() {
        let sr = SampleRate::STUDIO;
        let mut track = DialogueTrack::new();
        track.place(tone_clip(sr, 0.1, 0.8), 1.0, 1.0); // starts at 1.0s = 48000
        let total = sr.samples_in(1.2);
        let out = OfflineRenderer::new(sr, 512).render(&mut track, total);
        // Silent before 1.0s, energetic after.
        let before = &out.audio.left[..48_000];
        let after = &out.audio.left[48_000..48_000 + 4800];
        assert!(before.iter().all(|&s| s.abs() < 1e-6));
        assert!(after.iter().any(|&s| s.abs() > 0.1));
    }

    #[test]
    fn duck_curve_pulls_music_down_under_dialogue() {
        let sr = SampleRate::STUDIO;
        let mut track = DialogueTrack::new();
        track.place(tone_clip(sr, 0.5, 0.9), 0.5, 1.0); // dialogue 0.5..1.0s
        let total = sr.samples_in(1.5) as usize;
        let duck = track.duck_curve(total, sr, 0.3, 5.0, 120.0);
        // Before dialogue: full gain. During: ducked toward floor.
        let idle = duck[sr.samples_in(0.1) as usize];
        let ducked = duck[sr.samples_in(0.75) as usize];
        assert!(idle > 0.95, "idle music not full: {idle}");
        assert!(ducked < 0.5, "music not ducked under speech: {ducked}");
        assert!(duck.iter().all(|&g| (0.3..=1.0001).contains(&g)));
    }
}
