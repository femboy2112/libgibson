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

/// How close (in beats) a note must sit to the arrival of the upcoming harmony to read as an
/// *anticipation* of it. Beyond this, membership in a chord several beats away is coincidence, not
/// anticipation — a future justification must have a deadline.
pub const ANTICIPATION_WINDOW: f64 = 1.0;

/// The full temporal + harmonic context of one realized note, so its function can be judged over
/// its ENTIRE sounding interval rather than merely at its onset. Carrying time is what lets the
/// classifier refuse to borrow legitimacy from a chord that is too far away (anticipation), require
/// a suspension to actually cross a harmonic boundary, and detect a note whose onset is consonant
/// but whose sustained body turns into an unexplained dissonance under the next chord.
#[derive(Debug, Clone, Copy)]
pub struct PitchContext {
    /// The realized pitch being classified.
    pub pitch: Midi,
    /// Onset beat.
    pub onset: f64,
    /// Nominal sounding duration in beats (the synth's release tail is a separate concern).
    pub duration: f64,
    /// The previous / next realized pitches in time (for stepwise-path reasoning).
    pub prev: Option<Midi>,
    pub next: Option<Midi>,
    /// The harmony sounding just BEFORE the current chord span (what a suspension is held from).
    pub prev_chord: Option<Chord>,
    /// The chord sounding at `onset`.
    pub cur: Option<Chord>,
    /// The upcoming harmony — the chord that begins at `next_boundary`.
    pub next_chord: Option<Chord>,
    /// The beat at which the current chord ends and `next_chord` begins, if a change is known.
    pub next_boundary: Option<f64>,
    /// Whether the onset falls on a strong beat.
    pub is_strong: bool,
}

impl PitchContext {
    /// Does this note's sounding body extend past the next harmonic boundary into `next_chord`?
    fn crosses_boundary(&self) -> bool {
        matches!(self.next_boundary, Some(b) if self.onset + self.duration > b + 1e-6)
    }

    /// Beats from this note's onset until the upcoming harmony arrives (`None` if no change known).
    fn beats_until_next(&self) -> Option<f64> {
        self.next_boundary.map(|b| b - self.onset)
    }
}

/// Classify the harmonic function of a realized note from its full [`PitchContext`] and the `scale`.
/// Returns the [`PitchFunction`] the note serves, or `None` when it is a non-chord tone with no
/// intelligible justification (an unjustified "wrong note").
///
/// Test order is precedence: a note is first a chord tone (and stays one only if its sustained body
/// does not turn dissonant across a chord change); else it is justified by the future it belongs to
/// (anticipation — only if that future is temporally close), the past it carries (suspension —
/// only across an actual boundary), or the stepwise path it walks (chromatic approach / neighbour /
/// passing / appoggiatura). What none of those explain is `None`.
pub fn classify(ctx: &PitchContext, scale: &Scale) -> Option<PitchFunction> {
    let pitch = ctx.pitch;

    // 1. A member of the sounding chord. Onset-legal — but if it SUSTAINS past the chord change
    //    into harmony where it is no longer a chord tone, its held body is an unexplained
    //    dissonance (a note cannot resolve a suspension by simply not moving). It stays justified
    //    only when it is also a tone of the next chord: a genuine common-tone tie / pedal.
    if in_chord(ctx.cur, pitch) {
        if ctx.crosses_boundary() && ctx.next_chord.is_some() && !in_chord(ctx.next_chord, pitch) {
            return None;
        }
        return Some(PitchFunction::ChordTone);
    }

    // 2. Anticipation: a tone of the UPCOMING chord, sounded before it arrives — but only when that
    //    chord is close enough (within ANTICIPATION_WINDOW) to hear the note as anticipating it. A
    //    distant future chord cannot retroactively justify a note here.
    if in_chord(ctx.next_chord, pitch) {
        if let Some(beats) = ctx.beats_until_next() {
            if (-1e-6..=ANTICIPATION_WINDOW).contains(&beats) {
                return Some(PitchFunction::Anticipation);
            }
        }
        // Too far away → fall through; the note must justify itself as a path or be `None`.
    }

    // 3. Suspension: a tone held from the PREVIOUS harmony (so the harmony must actually have
    //    changed) that resolves DOWN by step into a current chord tone.
    if in_chord(ctx.prev_chord, pitch) {
        if let Some(np) = ctx.next {
            if np < pitch && (pitch - np) <= 2 && in_chord(ctx.cur, np) {
                return Some(PitchFunction::Suspension);
            }
        }
    }

    // The stepwise-path functions need both neighbours in time.
    if let (Some(pp), Some(np)) = (ctx.prev, ctx.next) {
        let into_target = |t: Midi| in_chord(ctx.cur, t) || in_chord(ctx.next_chord, t);
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
        if ctx.is_strong
            && d_in.abs() > 2
            && d_out != 0
            && d_out.abs() <= 2
            && in_chord(ctx.cur, np)
        {
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

    /// A context with sensible defaults (short note, no neighbours/boundary), overridden per test
    /// with struct-update syntax.
    fn base(pitch: Midi, cur: Option<Chord>) -> PitchContext {
        PitchContext {
            pitch,
            onset: 0.0,
            duration: 0.5,
            prev: None,
            next: None,
            prev_chord: None,
            cur,
            next_chord: None,
            next_boundary: None,
            is_strong: true,
        }
    }

    #[test]
    fn a_chord_tone_is_a_chord_tone() {
        let c = PitchContext {
            prev: Some(60),
            next: Some(67),
            ..base(64, Some(c_major())) // E over C major
        };
        assert_eq!(classify(&c, &cmaj_scale()), Some(PitchFunction::ChordTone));
    }

    #[test]
    fn anticipation_belongs_to_the_next_chord_when_close() {
        // D (62, a G7 tone) over C major, one beat before G7 arrives — close enough to anticipate.
        let c = PitchContext {
            prev: Some(60),
            next: Some(67),
            prev_chord: Some(c_major()),
            next_chord: Some(g_dom7()),
            next_boundary: Some(1.0),
            is_strong: false,
            ..base(62, Some(c_major()))
        };
        assert_eq!(
            classify(&c, &cmaj_scale()),
            Some(PitchFunction::Anticipation)
        );
    }

    #[test]
    fn anticipation_needs_the_harmony_to_be_close() {
        // Same D, but G7 does not arrive for FOUR beats. Too far to hear as anticipation — and with
        // no stepwise path to justify it, it is an unjustified note, not a free pass.
        let c = PitchContext {
            prev: Some(60),
            next: Some(67),
            prev_chord: Some(c_major()),
            next_chord: Some(g_dom7()),
            next_boundary: Some(4.0),
            is_strong: false,
            ..base(62, Some(c_major()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), None);
    }

    #[test]
    fn a_sustained_chord_tone_that_turns_dissonant_across_the_boundary_is_unjustified() {
        // E (64) is a C-major tone at onset, but it is held for 2 beats across a boundary at beat 1
        // into G7, where E is NOT a chord tone. Its sustained body is an unexplained dissonance.
        let c = PitchContext {
            duration: 2.0,
            next_chord: Some(g_dom7()),
            next_boundary: Some(1.0),
            ..base(64, Some(c_major()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), None);
    }

    #[test]
    fn a_common_tone_tied_across_the_boundary_is_accepted() {
        // G (67) is a tone of BOTH C major and G7, so holding it across the change is a legitimate
        // common-tone tie, not a dissonance.
        let c = PitchContext {
            duration: 2.0,
            next_chord: Some(g_dom7()),
            next_boundary: Some(1.0),
            ..base(67, Some(c_major()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), Some(PitchFunction::ChordTone));
    }

    #[test]
    fn suspension_holds_from_the_previous_chord_and_resolves_down() {
        // D (62, a G7 tone) held over C major, resolving down a step to C (60, a C tone).
        let c = PitchContext {
            prev: Some(62),
            next: Some(60),
            prev_chord: Some(g_dom7()),
            ..base(62, Some(c_major()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), Some(PitchFunction::Suspension));
    }

    #[test]
    fn chromatic_approach_leads_by_semitone_into_a_chord_tone() {
        // D (62) -> D# (63) -> E (64, a C tone): the D# is a semitone approach to E.
        let c = PitchContext {
            prev: Some(62),
            next: Some(64),
            is_strong: false,
            ..base(63, Some(c_major()))
        };
        assert_eq!(
            classify(&c, &cmaj_scale()),
            Some(PitchFunction::ChromaticApproach)
        );
    }

    #[test]
    fn a_neighbour_leaves_and_returns() {
        // E (64) -> F (65) -> E (64): F is an upper neighbour of E.
        let c = PitchContext {
            prev: Some(64),
            next: Some(64),
            is_strong: false,
            ..base(65, Some(c_major()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), Some(PitchFunction::Neighbor));
    }

    #[test]
    fn a_diatonic_passing_tone_walks_between_chord_tones() {
        // C (60) -> D (62) -> E (64): D passes stepwise between two C-major tones, and is in scale.
        let c = PitchContext {
            prev: Some(60),
            next: Some(64),
            is_strong: false,
            ..base(62, Some(c_major()))
        };
        assert_eq!(
            classify(&c, &cmaj_scale()),
            Some(PitchFunction::DiatonicPassing)
        );
    }

    #[test]
    fn an_unjustified_strong_beat_leap_is_none() {
        // F# (66) leaped to from C (60) on a strong beat, leaping on to A (69): not a chord tone,
        // no next chord, no step in or out — nothing explains it.
        let c = PitchContext {
            prev: Some(60),
            next: Some(69),
            ..base(66, Some(c_major()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), None);
    }

    #[test]
    fn a_lone_non_chord_tone_with_no_neighbours_is_none() {
        // A single F (65) over C major with no temporal neighbours cannot be justified as a path.
        assert_eq!(classify(&base(65, Some(c_major())), &cmaj_scale()), None);
    }

    #[test]
    fn a_broken_suspension_that_resolves_upward_is_rejected() {
        // D (62, a G7 tone) held over C major but resolving UP to E (64) is not a suspension (those
        // resolve down); we do not emit Retardation, and no stepwise path fits, so it is unjustified.
        let c = PitchContext {
            prev: Some(62),
            next: Some(64),
            prev_chord: Some(g_dom7()),
            ..base(62, Some(c_major()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), None);
    }

    #[test]
    fn a_broken_slide_that_leaps_out_is_rejected() {
        // D# (63) with a stepwise entry (from D) but a LEAP out to G (67) is not a contiguous path
        // into a chord tone — an intentionally broken slide is not justified.
        let c = PitchContext {
            prev: Some(62),
            next: Some(67),
            is_strong: false,
            ..base(63, Some(c_major()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), None);
    }
}
