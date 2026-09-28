//! Pitch justification — the jazz principle in code: there are no forbidden pitches, only
//! unjustified ones.
//!
//! [`classify`] labels a realized note with the harmonic [`PitchFunction`] it serves against the
//! chord actually sounding beneath it and its neighbours in time. A non-chord tone is only a
//! "wrong note" when it has no intelligible past or future: not a chord tone, not borrowed from
//! the chord it is about to become (anticipation), not held from the chord it just was
//! (suspension), and not walking a stepwise path into a structural tone (approach / neighbour /
//! passing / appoggiatura). A note that classifies to `None` is exactly such an unjustified note —
//! what the listener registers as "wrong" — which realization diagnostics count as a defect and
//! (Round IV, later) the realizer's search learns to avoid choosing without a reason.

use super::score::PitchFunction;
use super::theory::{pitch_class, Chord, Midi, Scale};

/// Whether `t`'s pitch-class is a tone of `chord` (false when there is no chord).
fn in_chord(chord: Option<Chord>, t: Midi) -> bool {
    chord.is_some_and(|c| c.contains_pc(pitch_class(t)))
}

/// Classify the harmonic function of realized pitch `pitch` against: the chord sounding now
/// (`cur`), the chords under the previous and next notes (`prev_chord` / `next_chord`), the
/// previous and next *realized* pitches (`prev` / `next`), the `scale`, and whether the onset is
/// on a strong beat. Returns the [`PitchFunction`] the note serves, or `None` when it is a
/// non-chord tone with no intelligible justification (an unjustified "wrong note").
///
/// Test order is precedence: a note is first a chord tone; else it is justified by the future it
/// belongs to (anticipation), the past it carries (suspension), or the stepwise path it walks
/// (chromatic approach / neighbour / passing / appoggiatura). What none of those explain is `None`.
#[allow(clippy::too_many_arguments)]
pub fn classify(
    pitch: Midi,
    prev: Option<Midi>,
    next: Option<Midi>,
    prev_chord: Option<Chord>,
    cur: Option<Chord>,
    next_chord: Option<Chord>,
    scale: &Scale,
    is_strong: bool,
) -> Option<PitchFunction> {
    // 1. A member of the sounding chord needs no justification.
    if in_chord(cur, pitch) {
        return Some(PitchFunction::ChordTone);
    }

    // 2. Anticipation: the pitch belongs to the NEXT chord, sounded before that chord arrives.
    if in_chord(next_chord, pitch) {
        return Some(PitchFunction::Anticipation);
    }

    // 3. Suspension: a tone held from the PREVIOUS harmony that resolves DOWN by step into a
    //    current chord tone.
    if in_chord(prev_chord, pitch) {
        if let Some(np) = next {
            if np < pitch && (pitch - np) <= 2 && in_chord(cur, np) {
                return Some(PitchFunction::Suspension);
            }
        }
    }

    // The stepwise-path functions need both neighbours in time.
    if let (Some(pp), Some(np)) = (prev, next) {
        let into_target = |t: Midi| in_chord(cur, t) || in_chord(next_chord, t);
        let d_in = pitch - pp; // motion into this note
        let d_out = np - pitch; // motion out of this note

        // 4. Chromatic approach: a semitone step INTO a chord-tone target, same direction in/out.
        if d_out.abs() == 1 && into_target(np) && d_in != 0 && d_in.signum() == d_out.signum() {
            return Some(PitchFunction::ChromaticApproach);
        }

        // 5. Neighbour: a step away from a pitch and back to it.
        if pp == np && (pitch - pp).abs() <= 2 && pitch != pp {
            return Some(PitchFunction::Neighbor);
        }

        // 6. Passing tone: a stepwise, monotonic move between two structural pitches.
        let monotonic = d_in != 0 && d_out != 0 && d_in.signum() == d_out.signum();
        let stepwise = d_in.abs() <= 2 && d_out.abs() <= 2;
        if monotonic && stepwise && into_target(pp) && into_target(np) {
            return Some(if scale.contains_pc(pitch_class(pitch)) {
                PitchFunction::DiatonicPassing
            } else {
                PitchFunction::ChromaticPassing
            });
        }

        // 7. Appoggiatura: a strong-beat leap TO the non-chord tone that resolves by step to a
        //    current chord tone.
        if is_strong && d_in.abs() > 2 && d_out != 0 && d_out.abs() <= 2 && in_chord(cur, np) {
            return Some(PitchFunction::Appoggiatura);
        }
    }

    // No intelligible past or future: an unjustified note.
    None
}

#[cfg(test)]
mod tests {
    use super::super::theory::{Chord, Mode, Quality, Scale};
    use super::*;

    fn c_major() -> Chord {
        Chord::new(0, Quality::Maj) // C E G = {0,4,7}
    }
    fn g_dom7() -> Chord {
        Chord::new(7, Quality::Dom7) // G B D F = {7,11,2,5}
    }
    fn cmaj_scale() -> Scale {
        Scale::new(0, Mode::Ionian)
    }

    #[test]
    fn a_chord_tone_is_a_chord_tone() {
        // E (64) over C major.
        let f = classify(
            64,
            Some(60),
            Some(67),
            None,
            Some(c_major()),
            None,
            &cmaj_scale(),
            true,
        );
        assert_eq!(f, Some(PitchFunction::ChordTone));
    }

    #[test]
    fn anticipation_belongs_to_the_next_chord() {
        // Sound a D (62) over C major, just before G7 arrives — D is a G7 tone, not a C tone.
        let f = classify(
            62,
            Some(60),
            Some(67),
            Some(c_major()),
            Some(c_major()),
            Some(g_dom7()),
            &cmaj_scale(),
            false,
        );
        assert_eq!(f, Some(PitchFunction::Anticipation));
    }

    #[test]
    fn suspension_holds_from_the_previous_chord_and_resolves_down() {
        // D (62, a G7 tone) held over C major, resolving down a step to C (60, a C tone).
        let f = classify(
            62,
            Some(62),
            Some(60),
            Some(g_dom7()),
            Some(c_major()),
            None,
            &cmaj_scale(),
            true,
        );
        assert_eq!(f, Some(PitchFunction::Suspension));
    }

    #[test]
    fn chromatic_approach_leads_by_semitone_into_a_chord_tone() {
        // D (62) -> D# (63) -> E (64, a C tone): the D# is a semitone approach to E.
        let f = classify(
            63,
            Some(62),
            Some(64),
            None,
            Some(c_major()),
            None,
            &cmaj_scale(),
            false,
        );
        assert_eq!(f, Some(PitchFunction::ChromaticApproach));
    }

    #[test]
    fn a_neighbour_leaves_and_returns() {
        // E (64) -> F (65) -> E (64): F is an upper neighbour of E.
        let f = classify(
            65,
            Some(64),
            Some(64),
            None,
            Some(c_major()),
            None,
            &cmaj_scale(),
            false,
        );
        assert_eq!(f, Some(PitchFunction::Neighbor));
    }

    #[test]
    fn a_diatonic_passing_tone_walks_between_chord_tones() {
        // C (60) -> D (62) -> E (64): D passes stepwise between two C-major tones, and is in scale.
        let f = classify(
            62,
            Some(60),
            Some(64),
            None,
            Some(c_major()),
            None,
            &cmaj_scale(),
            false,
        );
        assert_eq!(f, Some(PitchFunction::DiatonicPassing));
    }

    #[test]
    fn an_unjustified_strong_beat_leap_is_none() {
        // F# (66) leaped to from C (60) on a strong beat, leaping on to A (69): not a chord tone,
        // no next chord, no step in or out — nothing explains it.
        let f = classify(
            66,
            Some(60),
            Some(69),
            None,
            Some(c_major()),
            None,
            &cmaj_scale(),
            true,
        );
        assert_eq!(f, None);
    }

    #[test]
    fn a_lone_non_chord_tone_with_no_neighbours_is_none() {
        // A single F (65) over C major with no temporal neighbours cannot be justified as a path.
        let f = classify(
            65,
            None,
            None,
            None,
            Some(c_major()),
            None,
            &cmaj_scale(),
            true,
        );
        assert_eq!(f, None);
    }
}
