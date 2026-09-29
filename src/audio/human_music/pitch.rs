//! Pitch justification — the jazz principle in code: there are no forbidden pitches, only
//! unjustified ones.
//!
//! [`classify`] reconstructs local pitch syntax from a realized note, its timed neighbours,
//! and the harmony sounding beneath it. A function names a local relationship; it does not prove
//! ensemble compatibility or ownership along the song's harmonic path. Those are separate audits.
//! In particular, a licensed extension is available colour, not automatically a justified
//! structural destination. A suspension requires actual temporal carry across a harmony change.

use super::score::PitchFunction;
use super::theory::{pitch_class, Chord, Midi, Scale};

/// Whether `t`'s pitch-class is a tone of `chord` (false when there is no chord). Allocation-free:
/// the realizer's search calls the classifier inside its inner loop.
fn in_chord(chord: Option<Chord>, t: Midi) -> bool {
    let pc = pitch_class(t);
    chord.is_some_and(|c| {
        c.quality
            .intervals()
            .iter()
            .any(|&i| (c.root_pc + i).rem_euclid(12) == pc)
    })
}

/// A pitch-class set as a 12-bit mask (bit `pc` set), for [`PitchContext::licensed`].
pub fn pc_mask(pcs: &[i32]) -> u16 {
    pcs.iter()
        .fold(0u16, |m, &pc| m | (1u16 << pc.rem_euclid(12)))
}

/// Whether `t`'s pitch class is in the 12-bit `mask`.
fn in_mask(mask: u16, t: Midi) -> bool {
    mask & (1u16 << pitch_class(t)) != 0
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
    /// Actual onset of the next realized pitch, when known.
    pub next_onset: Option<f64>,
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
    /// The tensions the local chord-scale licenses over `cur` (a [`pc_mask`]; `0` = none): a
    /// 9th/11th/13th a whole step above a chord tone that clashes a semitone above none of them.
    /// Such a tone is consonant colour over this harmony, not a path that needs neighbours.
    pub licensed: u16,
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
/// Returns the local [`PitchFunction`], or `None` when no supported local relationship explains
/// the note. A returned label is not a proof of temporal harmonic ownership.
///
/// A physically held suspension is recognized first. Otherwise chord membership and available
/// colour precede anticipation and melodic connector syntax. Anticipation must connect to the
/// arriving harmony through the held note or a timed successor. What none explain is `None`.
pub fn classify(ctx: &PitchContext, scale: &Scale) -> Option<PitchFunction> {
    classify_inner(ctx, scale, false)
}

/// Frozen Round XI local semantics for the exact control realization.
pub(super) fn classify_r11(ctx: &PitchContext, scale: &Scale) -> Option<PitchFunction> {
    classify_inner(ctx, scale, true)
}

fn classify_inner(ctx: &PitchContext, scale: &Scale, r11: bool) -> Option<PitchFunction> {
    let pitch = ctx.pitch;
    if !r11 && in_chord(ctx.cur, pitch) && ctx.crosses_boundary() {
        if let (Some(boundary), Some(next), Some(next_onset)) =
            (ctx.next_boundary, ctx.next, ctx.next_onset)
        {
            let end = ctx.onset + ctx.duration;
            if ctx.onset < boundary - 1e-6
                && !in_chord(ctx.next_chord, pitch)
                && next < pitch
                && pitch - next <= 2
                && in_chord(ctx.next_chord, next)
                && next_onset >= boundary
                && next_onset >= end - 1e-6
                && next_onset <= end + 1.0
            {
                return Some(PitchFunction::Suspension);
            }
        }
    }

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

    // 1b. A licensed tension of the local palette (the 9th over a minor 7th, the 13th over a
    //     major chord): locally available colour, under the same sustain rule as a chord tone — its held
    //     body may not smear into a following harmony it does not belong to.
    if in_mask(ctx.licensed, pitch) {
        if ctx.crosses_boundary() && ctx.next_chord.is_some() && !in_chord(ctx.next_chord, pitch) {
            return None;
        }
        return Some(PitchFunction::LicensedExtension);
    }

    // 2. Anticipation: a tone of the UPCOMING chord, sounded before it arrives — but only when that
    //    chord is close enough (within ANTICIPATION_WINDOW) to hear the note as anticipating it. A
    //    distant future chord cannot retroactively justify a note here.
    if in_chord(ctx.next_chord, pitch) {
        if let Some(beats) = ctx.beats_until_next() {
            if (-1e-6..=ANTICIPATION_WINDOW).contains(&beats) {
                let boundary = ctx.onset + beats;
                let connects = ctx.onset + ctx.duration >= boundary - 1e-6
                    || matches!((ctx.next, ctx.next_onset), (Some(p), Some(at))
                        if at >= boundary - 1e-6
                            && at <= boundary + ANTICIPATION_WINDOW
                            && (p - pitch).abs() <= 2
                            && in_chord(ctx.next_chord, p));
                if r11 || connects {
                    return Some(PitchFunction::Anticipation);
                }
            }
        }
        // Too far away → fall through; the note must justify itself as a path or be `None`.
    }

    // Frozen R11 control only: historical membership was incorrectly treated as temporal carry.
    // Current semantics require the physical cross-boundary proof above.
    if r11 && in_chord(ctx.prev_chord, pitch) {
        if let Some(np) = ctx.next {
            if np < pitch && (pitch - np) <= 2 && in_chord(ctx.cur, np) {
                return Some(PitchFunction::Suspension);
            }
        }
    }

    // The stepwise-path functions need both neighbours in time.
    if let (Some(pp), Some(np)) = (ctx.prev, ctx.next) {
        // A structural pitch: a chord tone (now or of the arriving harmony) or a licensed tension.
        let into_target = |t: Midi| {
            in_chord(ctx.cur, t) || in_chord(ctx.next_chord, t) || in_mask(ctx.licensed, t)
        };
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

        // 8. Enclosure: the previous note and this one bracket a target from opposite sides, each
        //    within a step of it (above-then-below or below-then-above), and the line then lands on
        //    the target — the bebop surround.
        let side_prev = (pp - np).signum();
        let side_here = (pitch - np).signum();
        if into_target(np)
            && side_prev != 0
            && side_here == -side_prev
            && (pp - np).abs() <= 2
            && (pitch - np).abs() <= 2
        {
            return Some(PitchFunction::Enclosure);
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
            next_onset: None,
            prev_chord: None,
            cur,
            next_chord: None,
            next_boundary: None,
            is_strong: true,
            licensed: 0,
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
            next: Some(62),
            next_onset: Some(1.0),
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
    fn anticipation_can_end_exactly_at_the_actual_harmonic_arrival() {
        let c = PitchContext {
            onset: 3.5,
            duration: 0.5,
            next_chord: Some(g_dom7()),
            next_boundary: Some(4.0),
            is_strong: false,
            ..base(62, Some(c_major()))
        };
        assert_eq!(
            classify(&c, &cmaj_scale()),
            Some(PitchFunction::Anticipation)
        );
        assert_eq!(
            classify(
                &PitchContext {
                    duration: 0.25,
                    ..c
                },
                &cmaj_scale()
            ),
            None
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
            onset: 3.5,
            duration: 1.0,
            prev: Some(62),
            next: Some(60),
            next_onset: Some(4.5),
            next_chord: Some(c_major()),
            next_boundary: Some(4.0),
            ..base(62, Some(g_dom7()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), Some(PitchFunction::Suspension));
    }

    #[test]
    fn historical_membership_does_not_prove_a_suspension() {
        let c = PitchContext {
            onset: 5.0,
            next: Some(60),
            next_onset: Some(5.5),
            prev_chord: Some(g_dom7()),
            ..base(62, Some(c_major()))
        };
        assert_ne!(classify(&c, &cmaj_scale()), Some(PitchFunction::Suspension));
        assert_eq!(
            classify_r11(&c, &cmaj_scale()),
            Some(PitchFunction::Suspension)
        );
    }

    #[test]
    fn anticipation_must_connect_to_the_actual_arrival() {
        let c = PitchContext {
            onset: 3.5,
            // Release before the boundary; the distant successor cannot supply
            // the missing connection. An exact-boundary gate is tested above.
            duration: 0.25,
            next: Some(67),
            next_onset: Some(4.0),
            next_chord: Some(g_dom7()),
            next_boundary: Some(4.0),
            ..base(62, Some(c_major()))
        };
        assert_ne!(
            classify(&c, &cmaj_scale()),
            Some(PitchFunction::Anticipation)
        );
        assert_eq!(
            classify_r11(&c, &cmaj_scale()),
            Some(PitchFunction::Anticipation)
        );
        let held = PitchContext { duration: 1.0, ..c };
        assert_eq!(
            classify(&held, &cmaj_scale()),
            Some(PitchFunction::Anticipation)
        );
        let canceled = PitchContext {
            next_chord: Some(c_major()),
            ..held
        };
        assert_ne!(
            classify(&canceled, &cmaj_scale()),
            Some(PitchFunction::Anticipation)
        );
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
    fn a_licensed_tension_is_colour_not_a_wrong_note() {
        // D (62) over C major: not a chord tone, but when the local palette licenses the 9th it is
        // consonant colour even on a strong beat with no stepwise path.
        let unlicensed = PitchContext {
            prev: Some(55),
            next: Some(67),
            ..base(62, Some(c_major()))
        };
        assert_eq!(classify(&unlicensed, &cmaj_scale()), None);
        let licensed = PitchContext {
            licensed: pc_mask(&[2, 9]),
            ..unlicensed
        };
        assert_eq!(
            classify(&licensed, &cmaj_scale()),
            Some(PitchFunction::LicensedExtension)
        );
        // ...but it may not sustain into a harmony it does not belong to (Fmaj7 has no D).
        let smeared = PitchContext {
            duration: 2.0,
            next_chord: Some(Chord::new(5, Quality::Maj7)),
            next_boundary: Some(1.0),
            ..licensed
        };
        assert_eq!(classify(&smeared, &cmaj_scale()), None);
    }

    #[test]
    fn an_enclosure_surrounds_the_target() {
        // F (65) -> D# (63) -> E (64): above-then-below the E, landing on it.
        let c = PitchContext {
            prev: Some(65),
            next: Some(64),
            is_strong: false,
            ..base(63, Some(c_major()))
        };
        assert_eq!(classify(&c, &cmaj_scale()), Some(PitchFunction::Enclosure));
        // A surround whose second note overshoots by more than a step is not an enclosure.
        let wide = PitchContext {
            prev: Some(65),
            next: Some(64),
            is_strong: false,
            ..base(61, Some(c_major()))
        };
        assert_eq!(classify(&wide, &cmaj_scale()), None);
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
