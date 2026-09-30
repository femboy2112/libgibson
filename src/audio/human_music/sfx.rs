//! Semantic reaction stings: source realization, harmonic ownership and independent audit.
//! The composition realizer calls this subsystem once after the pitched band is final.

use super::action::{ActionCause, ActionKind};
use super::ids::ActionId;
use super::performance::{EnsembleCoupling, PerformancePlan};
use super::plan::CompositionPlan;
use super::score::{PitchFunction, Provenance, Score, SfxEvent, SfxKind};
use super::semantic::{EventKind, SemanticTrace, Tone};
use super::theory::{pitch_class, Chord, Midi};
use super::world::MusicWorld;

/// One sting per significant semantic event, placed on the event's MUSICAL beat and pitched in the
/// harmony sounding there.
///
/// The beat is the event's quantized beat ([`super::timeline::quantize_event_beat`], the same map
/// the intent timeline — and so every action — uses); the raw semantic beat used to flam the sting
/// a few tens of milliseconds against the ensemble hit the same event produced.
pub(crate) fn add_sfx_and_provenance(
    score: &mut Score,
    trace: &SemanticTrace,
    plan: &CompositionPlan,
    perf: &PerformancePlan,
    world: &MusicWorld,
) {
    let mut prev = trace.events.first().map(|e| e.state);
    // `ti` is the event's intent-timeline transition: the walk emits exactly one per event.
    for (ti, ev) in trace.events.iter().enumerate() {
        let significant = ev.kind.requires_event()
            && prev
                .map(|p| p.is_significant_change(&ev.state))
                .unwrap_or(true);
        prev = Some(ev.state);
        if !significant {
            continue;
        }
        let kind = match (ev.kind, ev.state.tone) {
            (EventKind::Impact, _) | (_, Tone::Danger) => SfxKind::Impact,
            (EventKind::Confirmation, _) | (_, Tone::Success) => SfxKind::Confirm,
            (EventKind::ModalEntered, _) | (_, Tone::Warning) => SfxKind::Warning,
            (EventKind::FocusAcquired, _) => SfxKind::Acquire,
            (EventKind::ActChanged, _) | (EventKind::SectionResolved, _) => SfxKind::Transition,
            _ => SfxKind::Acquire,
        };
        let at = super::timeline::quantize_event_beat(ev.at_beat);
        if at >= score.total_beats - 1e-9 {
            continue; // the event lands on the end of the piece: no time left to sound it
        }
        let sec_kind = plan.form.phrase_at(at).family.to_section_kind();
        let mut v = voice_sfx(kind, at, ti, score.tempo_bpm, plan, perf);
        // Round VIII: the sting joins the band's one harmony — under the coupled realization it keeps
        // its shape and pitch classes (the chord verdict and an owned tritone are untouched) and moves
        // by whole octaves to the placement with the fewest unowned clashes against what sounds.
        if perf.coupling == EnsembleCoupling::CoupledR8 && v.pitches != SfxEvent::UNPITCHED {
            if let Some((shift, reason)) = sfx_octave(score, perf, world, kind, at, &v) {
                v.pitches = v.pitches.map(|p| p + shift);
                score
                    .vertical_decisions
                    .push(super::harmonic_state::VerticalDecision {
                        beat: at,
                        role: super::score::Role::Lead,
                        what: "sfx-octave",
                        reason,
                    });
            }
        }
        // When the band itself accents this beat (the same event's hit or push), the sting sits
        // under the ensemble instead of stacking on top of it: the accent is the band's.
        let band_accents = perf
            .actions_starting(
                &[
                    super::action::ActionKind::Hit,
                    super::action::ActionKind::Push,
                ],
                at,
                0.125,
                None,
            )
            .next()
            .is_some();
        score.sfx.push(SfxEvent {
            start_beat: at,
            kind,
            velocity: ev.state.dynamic() * if band_accents { 0.75 } else { 1.0 },
            prov: Provenance {
                section: sec_kind,
                role_note: "sfx",
                ..Provenance::new(sec_kind)
            },
            pitches: v.pitches,
            function: v.function,
            owned_by: v.owned_by,
            dissonance_beats: v.dissonance_beats,
        });
    }
}

/// The whole-octave shift (within MIDI 24..=96) that gives an SFX gesture the fewest unowned clashes
/// against the notes sounding during its gated life, if it beats the composer's own register —
/// with the reason. Ties keep the original register.
fn sfx_octave(
    score: &Score,
    perf: &PerformancePlan,
    world: &MusicWorld,
    kind: SfxKind,
    at: f64,
    v: &SfxVoicing,
) -> Option<(Midi, String)> {
    use super::sonority::{classify_clash, sfx_voice, Clash, VerticalClass, Voice};
    let (a, d, _, _) = kind.envelope();
    let end = at + (a + d + kind.hold_secs()) as f64 * score.tempo_bpm.max(1.0) as f64 / 60.0;
    let ctx = perf.context_at(at)?;
    // What actually rings under the sting: each note to its audible end at the masking floor (a
    // pad tail from the last chord is still there).
    let world_patch = |r: super::score::Role| match r {
        super::score::Role::Pad => &world.pad,
        super::score::Role::Keys => &world.keys,
        super::score::Role::Bass => &world.bass,
        super::score::Role::Lead => &world.lead,
    };
    let sounding: Vec<Voice> = score
        .notes
        .iter()
        .map(|n| {
            let mut v = super::harmonic_state::voice_of(n);
            v.end = super::sonority::audible_end_at(
                v.start,
                n.dur_beats as f64,
                world_patch(n.role),
                score.tempo_bpm,
                super::sonority::MASKING_FLOOR_DB,
            );
            v
        })
        .filter(|v| v.start < end - 1e-6 && v.end > at + 1e-6)
        .collect();
    let clashes = |shift: Midi| -> (u32, Vec<String>) {
        let mut n = 0;
        let mut why = Vec::new();
        for (k, &p) in v.pitches.iter().enumerate() {
            let me = sfx_voice(p + shift, v.function[k], at, end, v.owned_by.is_some());
            for o in &sounding {
                if Clash::of(me.pitch, o.pitch).is_some()
                    && classify_clash(ctx, &me, o, None) == VerticalClass::UnownedCollision
                {
                    n += 1;
                    why.push(format!(
                        "{} against {} {}",
                        super::theory::note_name(p),
                        o.role.label(),
                        super::theory::note_name(o.pitch)
                    ));
                }
            }
        }
        (n, why)
    };
    let (base, why) = clashes(0);
    if base == 0 {
        return None;
    }
    let lo = v.pitches.iter().min().copied().unwrap_or(60);
    let hi = v.pitches.iter().max().copied().unwrap_or(60);
    let best = [12, -12, 24, -24, 36, -36]
        .into_iter()
        .filter(|s| lo + s >= 24 && hi + s <= 96)
        .map(|s| (clashes(s).0, s.abs(), s))
        .min()?;
    (best.0 < base).then(|| {
        (
            best.2,
            format!(
                "moved {:+} semitones: {} -> {} unowned clashes ({})",
                best.2,
                base,
                best.0,
                why.join(", ")
            ),
        )
    })
}

/// An SFX gesture's pitches and their justification.
struct SfxVoicing {
    pitches: [Midi; 2],
    function: [Option<PitchFunction>; 2],
    owned_by: Option<ActionId>,
    dissonance_beats: Option<f32>,
}

impl SfxVoicing {
    fn chord_tones(pitches: [Midi; 2]) -> SfxVoicing {
        SfxVoicing {
            pitches,
            function: [Some(PitchFunction::ChordTone); 2],
            owned_by: None,
            dissonance_beats: None,
        }
    }
}

/// The lower pitch class of a tritone the chord itself contains (Dom7 3–b7, m7b5 1–b5, dim 1–b5),
/// if it has one.
fn own_tritone(chord: &Chord) -> Option<i32> {
    let iv = chord.quality.intervals();
    iv.iter().enumerate().find_map(|(k, &a)| {
        iv[k + 1..]
            .iter()
            .any(|&b| (b - a).rem_euclid(12) == 6)
            .then_some((chord.root_pc + a).rem_euclid(12))
    })
}

/// Pitch one SFX gesture against the harmony sounding at `at`, in the register the world-scale
/// version used (so a sting whose local chord IS the tonic sounds exactly as before):
///
/// - Acquire: the chord's root high, its fifth just below.
/// - Confirm: fifth → root, rising.
/// - Transition: the guide tones (3rd, 7th/6th — the 5th for a triad) above the root.
/// - Impact: the bass pitch class, low, doubled at the octave.
/// - Warning: the chord's OWN tritone when it has one (both chord tones). Otherwise the alarm
///   tritone is kept on the chord's root as an OWNED dissonance: owned by the planned action the
///   same semantic event produced (the Hold a Suspend lifted, the Deflect bound to it) whose window
///   holds the voice's whole bounded lifetime. With no such owner the Warning sounds the guide
///   tones instead — a dissonance nobody owns is not played.
/// - Danger (not produced by the functor today): root and fifth, low.
///
/// With no harmony at all (an empty chord plan) the gesture is left unpitched: the synth's
/// world-scale fallback.
fn voice_sfx(
    kind: SfxKind,
    at: f64,
    transition: usize,
    tempo_bpm: f32,
    plan: &CompositionPlan,
    perf: &PerformancePlan,
) -> SfxVoicing {
    let Some(ctx) = perf.context_at(at) else {
        return SfxVoicing {
            pitches: SfxEvent::UNPITCHED,
            function: [None; 2],
            owned_by: None,
            dissonance_beats: None,
        };
    };
    let chord = ctx.chord;
    let root = chord.root_pc;
    let iv = chord.quality.intervals();
    let fifth = iv
        .iter()
        .find(|&&i| i == 7)
        .or_else(|| iv.iter().find(|&&i| i == 6 || i == 8))
        .map(|&i| (root + i).rem_euclid(12))
        .unwrap_or(root);
    // The lowest pitch of class `pc` at or above `floor`.
    let place = |pc: i32, floor: Midi| floor + (pc - floor).rem_euclid(12);
    let guides = |base: Midi| {
        let g = super::context::guide_tones(&chord);
        let lo = g.first().copied().unwrap_or(fifth);
        let hi = g.get(1).copied().unwrap_or(fifth);
        [place(lo, base), place(hi, base)]
    };
    match kind {
        SfxKind::Acquire => {
            let r = place(root, 84);
            SfxVoicing::chord_tones([r, place(fifth, r - 12)])
        }
        SfxKind::Confirm => {
            let r = place(root, 84);
            SfxVoicing::chord_tones([place(fifth, r - 12), r])
        }
        SfxKind::Transition => SfxVoicing::chord_tones(guides(place(root, 60))),
        SfxKind::Impact => {
            let b = if chord.contains_pc(ctx.bass_pc) {
                ctx.bass_pc
            } else {
                root
            };
            SfxVoicing::chord_tones([place(b, 24), place(b, 36)])
        }
        SfxKind::Danger => {
            let r = place(root, 36);
            SfxVoicing::chord_tones([r, place(fifth, r)])
        }
        SfxKind::Warning => {
            if let Some(lo) = own_tritone(&chord) {
                let p = place(lo, 60);
                return SfxVoicing::chord_tones([p, p + 6]);
            }
            let life = kind.max_lifetime_beats(tempo_bpm);
            match sfx_owner(perf, plan, transition, at, life) {
                Some(owner) => {
                    let p = place(root, 60);
                    SfxVoicing {
                        pitches: [p, p + 6],
                        function: [Some(PitchFunction::ChordTone), None],
                        owned_by: Some(owner),
                        dissonance_beats: Some(life),
                    }
                }
                None => SfxVoicing::chord_tones(guides(place(root, 60))),
            }
        }
    }
}

/// The planned action that owns an SFX dissonance: one the SAME semantic event produced (a
/// morphism its transition applied, or a backbone gesture bound to it) whose window holds
/// `[at, at + span]`. The Hold a Suspend lifted is preferred (the alarm is the held tension), then
/// the event's own morphisms over its gesture, then the longest window.
fn sfx_owner(
    perf: &PerformancePlan,
    plan: &CompositionPlan,
    transition: usize,
    at: f64,
    span: f32,
) -> Option<ActionId> {
    let binding = plan
        .backbone
        .as_ref()
        .and_then(|bb| bb.bindings.iter().position(|b| b.transition == transition));
    perf.actions
        .actions
        .iter()
        .filter_map(|a| {
            let tier = match a.cause {
                ActionCause::Morphism { transition: t, .. } if t == transition => 0u8,
                ActionCause::Gesture { .. } if binding.is_some() && a.binding == binding => 1,
                _ => return None,
            };
            let holds = a.start_beat <= at + 1e-6 && at + span as f64 <= a.end_beat() + 1e-6;
            holds.then_some((a.kind != ActionKind::Hold, tier, a.end_beat(), a.id))
        })
        .min_by(|x, y| {
            x.0.cmp(&y.0)
                .then(x.1.cmp(&y.1))
                .then(y.2.total_cmp(&x.2))
                .then(x.3.cmp(&y.3))
        })
        .map(|c| c.3)
}

/// How one SFX gesture stands against the harmony under it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SfxVerdict {
    /// Every pitch is a tone of the chord sounding under it.
    Chord,
    /// A dissonant pitch, owned by a planned action whose window holds the voice's whole
    /// lifetime (the payload is the owned span in beats).
    Owned(f32),
    /// An unowned or outliving dissonance, a pitch whose consonant label is false, no harmony to
    /// judge against, or an unpitched (world-scale) gesture.
    Unjustified,
}

/// The **SFX audit** (Round VIIb): is every sting in the local harmony, or honestly owned? Judged
/// from the realized Score's own chords and the performance's actions — not from the labels the
/// composer wrote, which it only cross-checks.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SfxAudit {
    pub total: usize,
    /// Gestures whose every pitch is a chord tone of its context.
    pub chord: usize,
    /// Gestures carrying an owned dissonance that fits inside its owner.
    pub owned: usize,
    /// Everything else (target 0).
    pub unjustified: usize,
    /// The longest owned dissonance, in beats.
    pub max_owned_beats: f32,
}

impl SfxAudit {
    /// Audit every SFX of `score` against its chords and `perf`'s actions.
    pub fn measure(perf: &PerformancePlan, score: &Score) -> SfxAudit {
        let mut a = SfxAudit {
            total: score.sfx.len(),
            ..SfxAudit::default()
        };
        for e in &score.sfx {
            match SfxAudit::verdict(e, perf, score) {
                SfxVerdict::Chord => a.chord += 1,
                SfxVerdict::Owned(d) => {
                    a.owned += 1;
                    a.max_owned_beats = a.max_owned_beats.max(d);
                }
                SfxVerdict::Unjustified => a.unjustified += 1,
            }
        }
        a
    }

    /// The verdict on one gesture `e` of `score`.
    pub fn verdict(e: &SfxEvent, perf: &PerformancePlan, score: &Score) -> SfxVerdict {
        if !e.is_pitched() {
            return SfxVerdict::Unjustified;
        }
        let b = e.start_beat;
        let Some(chord) = score
            .chords
            .iter()
            .filter(|c| c.start_beat <= b + 1e-6 && b < c.start_beat + c.dur_beats as f64)
            .max_by(|x, y| x.start_beat.total_cmp(&y.start_beat))
            .map(|c| c.chord)
        else {
            return SfxVerdict::Unjustified;
        };
        let mut dissonant = false;
        for (p, f) in e.pitches.iter().zip(e.function) {
            let tone = chord.contains_pc(pitch_class(*p));
            if f.is_some_and(PitchFunction::is_consonant) && !tone {
                return SfxVerdict::Unjustified; // labelled consonant, but it is not
            }
            dissonant |= !tone;
        }
        if !dissonant {
            return SfxVerdict::Chord;
        }
        // An owned dissonance: an existing action whose window holds the whole voice lifetime,
        // and a claimed span no shorter than that lifetime.
        let life = e.kind.max_lifetime_beats(score.tempo_bpm);
        match (
            e.owned_by.and_then(|id| perf.actions.get(id)),
            e.dissonance_beats,
        ) {
            (Some(owner), Some(d))
                if d + 1e-4 >= life
                    && d as f64 <= owner.dur_beats + 1e-6
                    && owner.start_beat <= b + 1e-6
                    && b + d as f64 <= owner.end_beat() + 1e-6 =>
            {
                SfxVerdict::Owned(d)
            }
            _ => SfxVerdict::Unjustified,
        }
    }

    /// A one-line receipt.
    pub fn report(&self) -> String {
        format!(
            "sfx: total={} chord={} owned={} unjustified={} max_owned_beats={:.2}\n",
            self.total, self.chord, self.owned, self.unjustified, self.max_owned_beats
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_chord_names_its_own_tritone() {
        use super::super::theory::Quality;
        // Dom7: 3–b7 (E–Bb over C7); m7b5: 1–b5 (B–F over Bm7b5); Maj7 and a triad have none.
        assert_eq!(own_tritone(&Chord::new(0, Quality::Dom7)), Some(4));
        assert_eq!(own_tritone(&Chord::new(11, Quality::Min7b5)), Some(11));
        assert_eq!(own_tritone(&Chord::new(5, Quality::Maj7)), None);
        assert_eq!(own_tritone(&Chord::new(9, Quality::Min)), None);
    }
}
