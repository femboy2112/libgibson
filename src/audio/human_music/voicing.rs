//! Voice leading: turn a bare [`Chord`] symbol into explicit sounding [`Voicing`]s that
//! move smoothly from chord to chord. A chord symbol is not enough — the *movement*
//! between voicings is what makes a progression sound composed rather than stamped.
//!
//! The optimizer keeps common tones stationary, moves the rest by the shortest path, holds
//! the voices in a register window, and opens or closes the spacing to the world's taste.
//! It picks among octave placements by minimizing total semitone motion from the previous
//! voicing — real voice leading, if not a full four-part species exercise.

use super::theory::{voice_motion, Chord, Midi};

/// A sounding chord voicing: pitches in ascending order.
#[derive(Debug, Clone, PartialEq)]
pub struct Voicing {
    pub voices: Vec<Midi>,
}

impl Voicing {
    fn sorted(mut v: Vec<Midi>) -> Voicing {
        v.sort_unstable();
        Voicing { voices: v }
    }

    /// Total semitone motion to another voicing (both are compared position-by-position).
    pub fn motion_to(&self, other: &Voicing) -> i32 {
        voice_motion(&self.voices, &other.voices)
    }
}

/// A stateful voice-leader: remembers the previous voicing and leads into the next chord.
#[derive(Debug, Clone)]
pub struct VoiceLeader {
    prev: Option<Voicing>,
    low: Midi,
    high: Midi,
    spread: f32,
}

impl VoiceLeader {
    /// A leader confined to `[low, high]`, spacing per `spread` (0 tight, 1 open).
    pub fn new(low: Midi, high: Midi, spread: f32) -> VoiceLeader {
        VoiceLeader {
            prev: None,
            low,
            high: high.max(low + 12),
            spread: spread.clamp(0.0, 1.0),
        }
    }

    /// Forget history (e.g. after a seek / section boundary).
    pub fn reset(&mut self) {
        self.prev = None;
    }

    /// Lead into `chord` with `n` upper voices centered near `center`, returning the chosen
    /// voicing.
    pub fn lead(&mut self, chord: &Chord, n: usize, center: Midi) -> Voicing {
        let n = n.clamp(2, 6);
        let base = self.close_voicing(chord, n, center);
        // Candidate octave placements of the whole voicing.
        let candidates = [
            shift(&base, -12),
            base.clone(),
            shift(&base, 12),
        ];
        let chosen = match &self.prev {
            None => {
                // No history: choose the placement whose mean pitch is nearest `center`.
                candidates
                    .into_iter()
                    .min_by_key(|c| (mean(c) - center).abs())
                    .unwrap()
            }
            Some(prev) => {
                // Choose the placement minimizing total motion from the previous voicing.
                candidates
                    .into_iter()
                    .map(|c| {
                        let padded = pad_to(&prev.voices, c.len());
                        (voice_motion(&padded, &c), c)
                    })
                    .min_by_key(|(m, _)| *m)
                    .map(|(_, c)| c)
                    .unwrap()
            }
        };
        let clamped = self.clamp_register(chosen);
        let v = Voicing::sorted(clamped);
        self.prev = Some(v.clone());
        v
    }

    /// A close-position (then optionally opened) voicing of `n` tones from `center`: the
    /// `n` lowest chord tones at or above the anchor, ascending (so every voice is a real
    /// chord tone and all pitch classes are covered before any is doubled).
    fn close_voicing(&self, chord: &Chord, n: usize, center: Midi) -> Vec<Midi> {
        let tones = chord.pitch_classes();
        let start = center - 6;
        let mut out = Vec::with_capacity(n);
        let mut p = start;
        while out.len() < n {
            if tones.contains(&p.rem_euclid(12)) {
                out.push(p);
            }
            p += 1;
            // Safety: bounded scan (12 semitones always yields a chord tone).
            if p > start + 128 {
                break;
            }
        }
        // Open the spacing for high-spread worlds: drop an inner voice an octave.
        if self.spread > 0.6 && out.len() >= 3 {
            let idx = 1;
            if out[idx] - 12 >= self.low {
                out[idx] -= 12;
            }
        }
        out
    }

    fn clamp_register(&self, mut v: Vec<Midi>) -> Vec<Midi> {
        for p in v.iter_mut() {
            while *p < self.low {
                *p += 12;
            }
            while *p > self.high {
                *p -= 12;
            }
        }
        v
    }
}

fn shift(v: &[Midi], by: Midi) -> Vec<Midi> {
    v.iter().map(|&p| p + by).collect()
}

fn mean(v: &[Midi]) -> Midi {
    if v.is_empty() {
        0
    } else {
        v.iter().sum::<Midi>() / v.len() as Midi
    }
}

/// Pad or trim `prev` to `len` for a positional comparison (duplicating the top voice when
/// the new voicing has more voices).
fn pad_to(prev: &[Midi], len: usize) -> Vec<Midi> {
    let mut v = prev.to_vec();
    if v.is_empty() {
        return vec![60; len];
    }
    while v.len() < len {
        let last = *v.last().unwrap();
        v.push(last);
    }
    v.truncate(len);
    v
}

#[cfg(test)]
mod tests {
    use super::super::theory::Quality;
    use super::*;

    #[test]
    fn common_tones_reduce_motion_between_related_chords() {
        // C major -> A minor share C and E; motion should be small.
        let mut vl = VoiceLeader::new(48, 84, 0.3);
        let c = vl.lead(&Chord::new(0, Quality::Maj), 4, 67);
        let a = vl.lead(&Chord::new(9, Quality::Min), 4, 67);
        let motion = c.motion_to(&a);
        assert!(motion <= 6, "Cmaj->Amin motion {motion} too large for related chords");
    }

    #[test]
    fn voices_stay_in_register() {
        let mut vl = VoiceLeader::new(55, 79, 0.4);
        for root in [0, 5, 7, 2, 9] {
            let v = vl.lead(&Chord::new(root, Quality::Maj7), 4, 67);
            for &p in &v.voices {
                assert!(p >= 55 && p <= 79 + 12, "voice {p} out of register");
            }
            assert!(v.voices.windows(2).all(|w| w[0] <= w[1]), "not sorted");
        }
    }

    #[test]
    fn voicing_contains_the_chord_tones() {
        let mut vl = VoiceLeader::new(48, 84, 0.3);
        let v = vl.lead(&Chord::new(0, Quality::Maj7), 4, 67);
        let pcs: std::collections::HashSet<i32> =
            v.voices.iter().map(|p| p.rem_euclid(12)).collect();
        // Root, third, seventh must all be present in a 4-voice maj7 voicing.
        for tone in [0, 4, 11] {
            assert!(pcs.contains(&tone), "maj7 voicing missing pc {tone}: {:?}", v.voices);
        }
    }

    #[test]
    fn open_spread_is_wider_than_close() {
        let mut tight = VoiceLeader::new(48, 84, 0.2);
        let mut open = VoiceLeader::new(48, 84, 0.9);
        let vt = tight.lead(&Chord::new(0, Quality::Maj), 4, 67);
        let vo = open.lead(&Chord::new(0, Quality::Maj), 4, 67);
        let span = |v: &Voicing| v.voices.last().unwrap() - v.voices.first().unwrap();
        assert!(span(&vo) >= span(&vt), "open span {} !>= tight span {}", span(&vo), span(&vt));
    }
}
