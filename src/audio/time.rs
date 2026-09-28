//! Sample-accurate audio time: [`SampleRate`], [`SampleTime`], [`AudioClock`],
//! [`AudioTransport`] and the beat<->sample [`TempoMap`].
//!
//! The governing invariant is **no accumulated float drift**. A playing transport tracks
//! an integer [`SampleTime`] (samples since the transport origin) and advances by exactly
//! the number of frames a block rendered. Seconds are *derived* from that integer on
//! demand; we never step a running `f64` seconds accumulator, because at 48 kHz over a
//! five-minute film the rounding error of an accumulator is audible drift, and A/V sync
//! depends on the visual director reading a position that is exactly the audio playhead.

use std::time::Duration;

/// A non-zero audio sample rate in Hz.
///
/// Constructed via [`SampleRate::new`] (rejects zero) or the [`SampleRate::STUDIO`] /
/// [`SampleRate::CD`] constants. 48 kHz is LibGibson's canonical internal rate (it
/// matches the reaction source clip and most modern devices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleRate(u32);

impl SampleRate {
    /// 48 000 Hz — the canonical internal rate.
    pub const STUDIO: SampleRate = SampleRate(48_000);
    /// 44 100 Hz — CD rate, offered for device negotiation.
    pub const CD: SampleRate = SampleRate(44_100);

    /// Construct a sample rate, rejecting zero.
    #[inline]
    pub const fn new(hz: u32) -> Option<SampleRate> {
        if hz == 0 {
            None
        } else {
            Some(SampleRate(hz))
        }
    }

    /// The rate in Hz.
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The rate in Hz as `f64`, for conversions.
    #[inline]
    pub fn as_f64(self) -> f64 {
        self.0 as f64
    }

    /// Number of whole samples in `secs` seconds at this rate (rounded to nearest).
    #[inline]
    pub fn samples_in(self, secs: f64) -> u64 {
        (secs * self.as_f64()).round().max(0.0) as u64
    }
}

impl Default for SampleRate {
    fn default() -> Self {
        SampleRate::STUDIO
    }
}

/// An integer sample position — samples elapsed since a transport origin.
///
/// This is the atom of audio time. Arithmetic is exact; seconds are derived, never
/// accumulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SampleTime(pub u64);

impl SampleTime {
    /// The origin (sample 0).
    pub const ZERO: SampleTime = SampleTime(0);

    /// This position in seconds at `sr`.
    #[inline]
    pub fn seconds(self, sr: SampleRate) -> f64 {
        self.0 as f64 / sr.as_f64()
    }

    /// The nearest sample position to `secs` seconds at `sr`.
    #[inline]
    pub fn from_seconds(secs: f64, sr: SampleRate) -> SampleTime {
        SampleTime(sr.samples_in(secs))
    }

    /// Saturating add of `n` samples.
    #[inline]
    pub fn add_samples(self, n: u64) -> SampleTime {
        SampleTime(self.0.saturating_add(n))
    }

    /// Saturating subtraction; never wraps below zero.
    #[inline]
    pub fn saturating_sub(self, other: SampleTime) -> u64 {
        self.0.saturating_sub(other.0)
    }
}

/// Stateless conversions between sample counts and wall-clock [`Duration`] at a fixed
/// [`SampleRate`]. The offline renderer needs no wall clock at all (the sample counter
/// *is* time); this helper exists for the realtime path and for diagnostics.
#[derive(Debug, Clone, Copy)]
pub struct AudioClock {
    sr: SampleRate,
}

impl AudioClock {
    /// A clock at `sr`.
    #[inline]
    pub fn new(sr: SampleRate) -> AudioClock {
        AudioClock { sr }
    }

    /// The sample rate.
    #[inline]
    pub fn sample_rate(&self) -> SampleRate {
        self.sr
    }

    /// Wall-clock duration of `n` samples.
    #[inline]
    pub fn duration_of(&self, n: u64) -> Duration {
        Duration::from_secs_f64(n as f64 / self.sr.as_f64())
    }

    /// Whole samples spanning `d` (rounded to nearest).
    #[inline]
    pub fn samples_of(&self, d: Duration) -> u64 {
        self.sr.samples_in(d.as_secs_f64())
    }
}

/// A play/pause/seek playhead over sample time — the **master media clock** during
/// audible playback.
///
/// The playhead is an integer [`SampleTime`]. [`AudioTransport::advance`] moves it by an
/// exact frame count after each rendered block, so a slow terminal frame can never pull
/// the audio timeline out of alignment: the visual director reads its edit position from
/// [`AudioTransport::seconds`], not from an independent wall clock.
#[derive(Debug, Clone)]
pub struct AudioTransport {
    sr: SampleRate,
    playhead: SampleTime,
    playing: bool,
}

impl AudioTransport {
    /// A stopped transport at the origin.
    pub fn new(sr: SampleRate) -> AudioTransport {
        AudioTransport {
            sr,
            playhead: SampleTime::ZERO,
            playing: false,
        }
    }

    /// The sample rate this transport runs at.
    #[inline]
    pub fn sample_rate(&self) -> SampleRate {
        self.sr
    }

    /// The current playhead in samples.
    #[inline]
    pub fn playhead(&self) -> SampleTime {
        self.playhead
    }

    /// The current playhead in seconds — the master A/V position.
    #[inline]
    pub fn seconds(&self) -> f64 {
        self.playhead.seconds(self.sr)
    }

    /// Whether the transport is playing.
    #[inline]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Begin playback.
    #[inline]
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Pause playback (the playhead is retained).
    #[inline]
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Advance the playhead by exactly `frames` samples. Called once per rendered block.
    #[inline]
    pub fn advance(&mut self, frames: u64) {
        self.playhead = self.playhead.add_samples(frames);
    }

    /// Seek to an absolute sample position. Callers coordinating A/V must also flush and
    /// reschedule voices (see [`crate::audio::graph`]); this only moves the clock.
    #[inline]
    pub fn seek(&mut self, to: SampleTime) {
        self.playhead = to;
    }

    /// Seek to `secs` seconds.
    #[inline]
    pub fn seek_seconds(&mut self, secs: f64) {
        self.seek(SampleTime::from_seconds(secs, self.sr));
    }
}

/// A constant-tempo beat<->sample map with a fixed meter.
///
/// Round I ships a single-tempo map (the common case for a scored film cue). The
/// conversions are computed from `f64` samples-per-beat but the *map is authoritative*:
/// callers convert a beat position to a [`SampleTime`] once, at scheduling time, and
/// never accumulate per-beat rounding. A future milestone can generalize this to tempo
/// segments without changing the call sites.
#[derive(Debug, Clone, Copy)]
pub struct TempoMap {
    sr: SampleRate,
    bpm: f64,
    beats_per_bar: u32,
}

impl TempoMap {
    /// A map at `bpm` beats per minute with `beats_per_bar` (e.g. 4 for 4/4).
    ///
    /// `bpm` is clamped to a sane positive range; `beats_per_bar` to at least 1.
    pub fn new(sr: SampleRate, bpm: f64, beats_per_bar: u32) -> TempoMap {
        TempoMap {
            sr,
            bpm: bpm.clamp(1.0, 1_000.0),
            beats_per_bar: beats_per_bar.max(1),
        }
    }

    /// The sample rate.
    #[inline]
    pub fn sample_rate(&self) -> SampleRate {
        self.sr
    }

    /// Beats per minute.
    #[inline]
    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    /// Beats per bar (the meter numerator).
    #[inline]
    pub fn beats_per_bar(&self) -> u32 {
        self.beats_per_bar
    }

    /// Samples per beat (fractional).
    #[inline]
    pub fn samples_per_beat(&self) -> f64 {
        self.sr.as_f64() * 60.0 / self.bpm
    }

    /// Samples per bar (fractional).
    #[inline]
    pub fn samples_per_bar(&self) -> f64 {
        self.samples_per_beat() * self.beats_per_bar as f64
    }

    /// Convert a (fractional) beat position to the nearest sample.
    #[inline]
    pub fn beat_to_sample(&self, beat: f64) -> SampleTime {
        SampleTime((beat * self.samples_per_beat()).round().max(0.0) as u64)
    }

    /// Convert a sample position to a fractional beat position.
    #[inline]
    pub fn sample_to_beat(&self, s: SampleTime) -> f64 {
        s.0 as f64 / self.samples_per_beat()
    }

    /// Convert a (bar, beat-within-bar) position to the nearest sample. `beat` is
    /// 0-based within the bar.
    #[inline]
    pub fn bar_beat_to_sample(&self, bar: u32, beat: f64) -> SampleTime {
        let total_beats = bar as f64 * self.beats_per_bar as f64 + beat;
        self.beat_to_sample(total_beats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rate_rejects_zero_and_reports_hz() {
        assert!(SampleRate::new(0).is_none());
        assert_eq!(SampleRate::new(48_000).unwrap().get(), 48_000);
        assert_eq!(SampleRate::STUDIO.get(), 48_000);
        assert_eq!(SampleRate::default(), SampleRate::STUDIO);
    }

    #[test]
    fn sample_time_seconds_round_trip_is_exact_on_whole_samples() {
        let sr = SampleRate::STUDIO;
        // 48000 samples == exactly 1.0s.
        assert_eq!(SampleTime(48_000).seconds(sr), 1.0);
        assert_eq!(SampleTime::from_seconds(1.0, sr), SampleTime(48_000));
        assert_eq!(SampleTime::from_seconds(2.5, sr), SampleTime(120_000));
    }

    #[test]
    fn transport_advance_never_drifts_over_many_blocks() {
        // Advancing by an odd block size a million times must equal the exact product,
        // because we count integer samples and never accumulate float seconds.
        let sr = SampleRate::STUDIO;
        let mut t = AudioTransport::new(sr);
        t.play();
        let block = 512u64;
        let blocks = 1_000_000u64;
        for _ in 0..blocks {
            t.advance(block);
        }
        assert_eq!(t.playhead(), SampleTime(block * blocks));
        // 512_000_000 samples / 48000 == 10666.666... seconds, derived, not accumulated.
        let expected_secs = (block * blocks) as f64 / sr.as_f64();
        assert_eq!(t.seconds(), expected_secs);
    }

    #[test]
    fn transport_seek_and_pause_retain_playhead() {
        let mut t = AudioTransport::new(SampleRate::STUDIO);
        t.seek_seconds(3.0);
        assert_eq!(t.playhead(), SampleTime(144_000));
        assert!(!t.is_playing());
        t.play();
        t.pause();
        assert_eq!(t.playhead(), SampleTime(144_000)); // pause keeps position
    }

    #[test]
    fn tempo_map_beat_sample_conversions() {
        let sr = SampleRate::STUDIO;
        // 120 BPM -> 0.5s per beat -> 24000 samples per beat.
        let tm = TempoMap::new(sr, 120.0, 4);
        assert_eq!(tm.samples_per_beat(), 24_000.0);
        assert_eq!(tm.samples_per_bar(), 96_000.0);
        assert_eq!(tm.beat_to_sample(1.0), SampleTime(24_000));
        assert_eq!(tm.beat_to_sample(4.0), SampleTime(96_000));
        assert_eq!(tm.bar_beat_to_sample(2, 0.0), SampleTime(192_000));
        assert!((tm.sample_to_beat(SampleTime(24_000)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn tempo_map_clamps_insane_input() {
        let sr = SampleRate::STUDIO;
        let tm = TempoMap::new(sr, 0.0, 0);
        assert!(tm.bpm() >= 1.0);
        assert!(tm.beats_per_bar() >= 1);
    }

    #[test]
    fn audio_clock_duration_round_trip() {
        let c = AudioClock::new(SampleRate::STUDIO);
        assert_eq!(c.samples_of(Duration::from_secs(1)), 48_000);
        assert_eq!(c.duration_of(48_000), Duration::from_secs(1));
    }
}
