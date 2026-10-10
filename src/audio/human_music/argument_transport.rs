//! Required timed-source carriage for the opt-in musical-argument compiler.
//!
//! This transport is deliberately different from an optional conversational response. It copies
//! every source event to every declared carrier, retaining independent onsets, sounding gates,
//! rests and accents. Its only orchestration change is a declared octave register. It never crops
//! a source to its tail, selects one preferred carrier, maps semitones modulo chord-tone count,
//! applies another timing feel, or silently snaps an incompatible source into the chart.
//!
//! [`replace_required_role`] belongs at each player's source seam, **before** other players hear
//! that player. The complete phrase window replaces that role's ordinary backing, including the
//! written silence between events. A preceding backing sustain is explicitly shortened at the
//! reservation boundary; backing starting inside the reservation is removed. No new reattack is
//! invented after the reservation. Receipts report replacements even when the note-count delta is
//! zero. This is preservation at the Score IR's `f64` onset / `f32` gate-and-accent precision,
//! not proof of perceptual identity or audible exposure after synthesis.
//!
//! Argument source/step identities live in the compiler and transport receipts. They are not
//! forged into the historical interaction-material, action or obligation ID spaces.

use super::action::Agent;
use super::argument::{ArgumentPlacement, ArgumentStepId, ReferentId};
use super::context::HarmonicContext;
use super::ids::ActionStamp;
use super::occupancy::{AuthoredOccupancy, OwnershipKind, OwnershipSpan, RhythmReservation};
use super::plan::CompositionPlan;
use super::score::{Note, PitchFunction, Provenance, Role};
use super::theory::{pitch_class, PitchBasis};
use std::fmt;

/// A source reservation actually carried by one player.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementTransportReceipt {
    pub step: ArgumentStepId,
    pub referent: ReferentId,
    pub phrase: u32,
    pub start_beat: f64,
    pub end_beat: f64,
    /// The role's fixed register relative to the lead's middle-C register, in semitones.
    pub octave_semitones: i32,
    pub source_events: usize,
    pub inserted_notes: usize,
}

/// Evidence of a mandatory replacement; success never depends on adding more notes.
#[derive(Debug, Clone, PartialEq)]
pub struct CarrierTransportReceipt {
    pub role: Role,
    pub placements: Vec<PlacementTransportReceipt>,
    pub removed_backing: usize,
    pub clipped_backing: usize,
    pub inserted_notes: usize,
    /// Source pitches without a demonstrated local harmonic label. They remain unchanged and
    /// unclassified; the transport does not mislabel them as chord tones to obtain a green audit.
    pub unclassified_notes: usize,
}

/// A malformed or infeasible source is rejected atomically, before editing anybody's notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CarrierTransportError {
    UnknownPhrase {
        step: ArgumentStepId,
        phrase: u32,
    },
    InvalidSpan {
        step: ArgumentStepId,
    },
    OutsidePhrase {
        step: ArgumentStepId,
    },
    EmptyMaterial {
        step: ArgumentStepId,
    },
    MissingCarrier {
        step: ArgumentStepId,
    },
    UnsupportedCarrier {
        step: ArgumentStepId,
        carrier: Agent,
    },
    DuplicateCarrier {
        step: ArgumentStepId,
        carrier: Agent,
    },
    WrongPitchBasis {
        step: ArgumentStepId,
    },
    InvalidEvent {
        step: ArgumentStepId,
        event: usize,
    },
    UnpitchedEvent {
        step: ArgumentStepId,
        event: usize,
    },
    PitchOutOfRange {
        step: ArgumentStepId,
        event: usize,
        carrier: Agent,
    },
    MissingHarmonicContext {
        step: ArgumentStepId,
        event: usize,
    },
    OverlappingReservations {
        carrier: Agent,
        first: ArgumentStepId,
        second: ArgumentStepId,
    },
    InvalidBacking {
        note: usize,
    },
}

impl fmt::Display for CarrierTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "required musical-source transport failed: {self:?}")
    }
}

impl std::error::Error for CarrierTransportError {}

/// The only register normalization permitted by this transport: Lead C4, Keys C3, Bass C2,
/// subsequently shifted by the declared tonic and chromatic source offsets.
/// `Pad` is accompaniment and is not a thematic carrier in this finite implementation.
pub fn carrier_register(role: Role) -> Option<i32> {
    match role {
        Role::Lead => Some(60),
        Role::Keys => Some(48),
        Role::Bass => Some(36),
        Role::Pad => None,
    }
}

fn carrier_role(carrier: Agent) -> Option<Role> {
    match carrier {
        Agent::Lead => Some(Role::Lead),
        Agent::Keys => Some(Role::Keys),
        Agent::Bass => Some(Role::Bass),
        Agent::Drums | Agent::Pad | Agent::Ensemble => None,
    }
}

fn context_at(contexts: &[HarmonicContext], beat: f64) -> Option<&HarmonicContext> {
    contexts.iter().find(|context| {
        beat >= context.start_beat && beat < context.start_beat + f64::from(context.dur_beats)
    })
}

/// Validate the complete source and all its carrier reservations, without realizing it.
/// This includes carriers other than the player whose seam will next be lowered; a partial
/// multi-carrier payoff cannot become acceptable by checking the successful carrier alone.
pub fn validate_required_placements(
    placements: &[ArgumentPlacement],
    tonic_pc: i32,
    contexts: &[HarmonicContext],
    plan: &CompositionPlan,
) -> Result<(), CarrierTransportError> {
    for placement in placements {
        let step = placement.step;
        let Some(phrase) = plan.form.phrases.get(placement.phrase as usize) else {
            return Err(CarrierTransportError::UnknownPhrase {
                step,
                phrase: placement.phrase,
            });
        };
        let end = placement.start_beat + placement.span_beats;
        if !placement.start_beat.is_finite()
            || !placement.span_beats.is_finite()
            || placement.span_beats <= 0.0
            || !end.is_finite()
        {
            return Err(CarrierTransportError::InvalidSpan { step });
        }
        if placement.start_beat < phrase.start_beat()
            || end > phrase.end_beat() + 1e-9
            || end > plan.form.total_beats + 1e-9
        {
            return Err(CarrierTransportError::OutsidePhrase { step });
        }
        if placement.events.is_empty() {
            return Err(CarrierTransportError::EmptyMaterial { step });
        }
        if placement.carriers.is_empty() {
            return Err(CarrierTransportError::MissingCarrier { step });
        }
        if placement.pitch_basis != PitchBasis::Semitones {
            return Err(CarrierTransportError::WrongPitchBasis { step });
        }
        for (index, &carrier) in placement.carriers.iter().enumerate() {
            if carrier_role(carrier).is_none() {
                return Err(CarrierTransportError::UnsupportedCarrier { step, carrier });
            }
            if placement.carriers[..index].contains(&carrier) {
                return Err(CarrierTransportError::DuplicateCarrier { step, carrier });
            }
        }
        let mut previous_onset = None;
        for (event_index, event) in placement.events.iter().enumerate() {
            if !event.onset.is_finite()
                || event.onset < 0.0
                || !event.dur.is_finite()
                || event.dur <= 0.0
                || !(event.dur as f32).is_finite()
                || event.dur as f32 <= 0.0
                || event.onset + event.dur > placement.span_beats + 1e-9
                || previous_onset.is_some_and(|previous| event.onset <= previous)
                || !event.accent.is_finite()
                || !(0.0..=1.0).contains(&event.accent)
            {
                return Err(CarrierTransportError::InvalidEvent {
                    step,
                    event: event_index,
                });
            }
            previous_onset = Some(event.onset);
            let Some(offset) = event.step else {
                return Err(CarrierTransportError::UnpitchedEvent {
                    step,
                    event: event_index,
                });
            };
            for &carrier in &placement.carriers {
                let register = carrier_register(carrier_role(carrier).unwrap()).unwrap();
                let pitch = i64::from(register)
                    + i64::from(tonic_pc.rem_euclid(12))
                    + i64::from(placement.anchor_semitones)
                    + i64::from(offset);
                if !(0..=127).contains(&pitch) {
                    return Err(CarrierTransportError::PitchOutOfRange {
                        step,
                        event: event_index,
                        carrier,
                    });
                }
            }
            if context_at(contexts, placement.start_beat + event.onset).is_none() {
                return Err(CarrierTransportError::MissingHarmonicContext {
                    step,
                    event: event_index,
                });
            }
        }
    }
    for (index, first) in placements.iter().enumerate() {
        for second in &placements[index + 1..] {
            if first.start_beat < second.start_beat + second.span_beats - 1e-9
                && second.start_beat < first.start_beat + first.span_beats - 1e-9
            {
                if let Some(&carrier) = first
                    .carriers
                    .iter()
                    .find(|carrier| second.carriers.contains(carrier))
                {
                    return Err(CarrierTransportError::OverlappingReservations {
                        carrier,
                        first: first.step,
                        second: second.step,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Replace one player's backing with every mandatory, complete source assigned to it.
///
/// Events are not fit to room or transformed by a player-specific harmonic map: infeasible input
/// is a typed error. `velocity = accent`, `start = placement.start + event.onset`,
/// `gate = event.dur as f32`, and `pitch = carrier_register + tonic + anchor + event.step`.
/// The independent witness can therefore compare raw notes without consulting these receipts.
/// The operation is atomic and deterministic. It must run before this role is heard by another
/// player; applying it after dependent realization would invalidate those players' hearings.
pub fn replace_required_role(
    notes: &mut Vec<Note>,
    role: Role,
    placements: &[ArgumentPlacement],
    tonic_pc: i32,
    contexts: &[HarmonicContext],
    plan: &CompositionPlan,
) -> Result<CarrierTransportReceipt, CarrierTransportError> {
    validate_required_placements(placements, tonic_pc, contexts, plan)?;
    for (index, note) in notes
        .iter()
        .enumerate()
        .filter(|(_, note)| note.role == role)
    {
        if !note.start_beat.is_finite() || !note.dur_beats.is_finite() || note.dur_beats <= 0.0 {
            return Err(CarrierTransportError::InvalidBacking { note: index });
        }
    }
    let assigned: Vec<_> = placements
        .iter()
        .filter(|placement| {
            placement
                .carriers
                .iter()
                .any(|&carrier| carrier_role(carrier) == Some(role))
        })
        .collect();
    let mut receipt = CarrierTransportReceipt {
        role,
        placements: Vec::with_capacity(assigned.len()),
        removed_backing: 0,
        clipped_backing: 0,
        inserted_notes: 0,
        unclassified_notes: 0,
    };
    if assigned.is_empty() {
        return Ok(receipt);
    }
    let mut next_notes = Vec::with_capacity(notes.len());
    for &note in notes.iter() {
        if note.role != role {
            next_notes.push(note);
            continue;
        }
        if assigned.iter().any(|placement| {
            note.start_beat >= placement.start_beat
                && note.start_beat < placement.start_beat + placement.span_beats
        }) {
            receipt.removed_backing += 1;
            continue;
        }
        let crossing = assigned
            .iter()
            .filter(|placement| {
                note.start_beat < placement.start_beat
                    && note.start_beat + f64::from(note.dur_beats) > placement.start_beat
            })
            .map(|placement| placement.start_beat)
            .min_by(f64::total_cmp);
        if let Some(boundary) = crossing {
            next_notes.push(Note {
                dur_beats: (boundary - note.start_beat) as f32,
                ..note
            });
            receipt.clipped_backing += 1;
        } else {
            next_notes.push(note);
        }
    }
    let register = carrier_register(role).unwrap();
    for placement in assigned {
        let phrase = &plan.form.phrases[placement.phrase as usize];
        let goal = plan.discourse.goal(placement.phrase as usize);
        let mut provenance = Provenance::new(phrase.family.to_section_kind());
        provenance.phrase = Some(placement.phrase);
        provenance.family = Some(phrase.family.label());
        provenance.motif_id = Some(placement.referent.0);
        provenance.motif_xform = Some(placement.relation.label());
        provenance.role = Some(goal.role.label());
        provenance.closure = Some(goal.closure.label());
        provenance.role_note = "argument";
        let mut source_notes: Vec<_> = placement
            .events
            .iter()
            .map(|event| {
                Note::new(
                    placement.start_beat + event.onset,
                    event.dur as f32,
                    i32::try_from(
                        i64::from(register)
                            + i64::from(tonic_pc.rem_euclid(12))
                            + i64::from(placement.anchor_semitones)
                            + i64::from(event.step.unwrap()),
                    )
                    .expect("validated carrier pitch fits MIDI and i32"),
                    event.accent,
                    role,
                    provenance,
                )
            })
            .collect();
        for index in 0..source_notes.len() {
            source_notes[index].function = classify_source_note(&source_notes, index, contexts);
        }
        receipt.unclassified_notes += source_notes
            .iter()
            .filter(|note| note.function.is_none())
            .count();
        receipt.inserted_notes += source_notes.len();
        receipt.placements.push(PlacementTransportReceipt {
            step: placement.step,
            referent: placement.referent,
            phrase: placement.phrase,
            start_beat: placement.start_beat,
            end_beat: placement.start_beat + placement.span_beats,
            octave_semitones: register - 60,
            source_events: placement.events.len(),
            inserted_notes: source_notes.len(),
        });
        next_notes.extend(source_notes);
    }
    next_notes.sort_by(|a, b| {
        a.start_beat
            .total_cmp(&b.start_beat)
            .then(a.pitch.cmp(&b.pitch))
    });
    *notes = next_notes;
    Ok(receipt)
}

/// Install the same timed sources in the authored-occupancy ledger after a successful
/// [`validate_required_placements`] / [`replace_required_role`] call.
///
/// A carrier retains ownership of its whole reserved phrase, including its initial, internal and
/// trailing rests. These semantic reservations are not pretend acoustic notes. Historical
/// interaction IDs/actions are left absent; the compiler's source and step IDs remain in the
/// explicit argument wrapper. The ledger's other windows survive with overlapping portions
/// removed, so an unrelated old phrase cannot retrospectively claim the required source's space.
pub fn reserve_required_role(occupancy: &mut AuthoredOccupancy, placements: &[ArgumentPlacement]) {
    let assigned: Vec<_> = placements
        .iter()
        .filter(|placement| {
            placement
                .carriers
                .iter()
                .any(|&carrier| carrier_role(carrier) == Some(occupancy.role))
        })
        .collect();
    if assigned.is_empty() {
        return;
    }
    let windows: Vec<_> = assigned
        .iter()
        .map(|p| (p.start_beat, p.start_beat + p.span_beats))
        .collect();
    occupancy.rhythm.retain_mut(|reservation| {
        if windows
            .iter()
            .any(|&(start, end)| reservation.beat >= start && reservation.beat < end)
        {
            return false;
        }
        if let Some(boundary) = windows
            .iter()
            .filter(|&&(start, _)| reservation.beat < start && reservation.end_beat > start)
            .map(|&(start, _)| start)
            .min_by(f64::total_cmp)
        {
            reservation.end_beat = boundary;
        }
        true
    });
    let mut outside_spans = Vec::new();
    for span in &occupancy.spans {
        let mut fragments = vec![*span];
        for &(start, end) in &windows {
            let mut next = Vec::new();
            for fragment in fragments {
                if fragment.end <= start || fragment.start >= end {
                    next.push(fragment);
                } else {
                    if fragment.start < start {
                        next.push(OwnershipSpan {
                            end: start,
                            ..fragment
                        });
                    }
                    if fragment.end > end {
                        next.push(OwnershipSpan {
                            start: end,
                            ..fragment
                        });
                    }
                }
            }
            fragments = next;
        }
        outside_spans.extend(fragments);
    }
    occupancy.spans = outside_spans;
    for placement in assigned {
        let start = placement.start_beat;
        let end = start + placement.span_beats;
        occupancy.spans.push(OwnershipSpan {
            start,
            end,
            material: None,
            kind: OwnershipKind::Phrase,
        });
        let mut sounding_until = start;
        for event in &placement.events {
            let onset = start + event.onset;
            let gate_end = onset + f64::from(event.dur as f32);
            if sounding_until < onset - 1e-9 {
                occupancy.spans.push(OwnershipSpan {
                    start: sounding_until,
                    end: onset,
                    material: None,
                    kind: OwnershipKind::InternalRest,
                });
            }
            occupancy.rhythm.push(RhythmReservation {
                beat: onset,
                end_beat: gate_end,
                structural: event.accent >= 0.75 || (onset - onset.round()).abs() < 1e-6,
                material: None,
                actions: ActionStamp::NONE,
            });
            sounding_until = sounding_until.max(gate_end);
        }
        if sounding_until < end - 1e-9 {
            occupancy.spans.push(OwnershipSpan {
                start: sounding_until,
                end,
                material: None,
                kind: OwnershipKind::InternalRest,
            });
        }
    }
    occupancy.rhythm.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    occupancy
        .spans
        .sort_by(|a, b| a.start.total_cmp(&b.start).then(a.end.total_cmp(&b.end)));
}

/// Derive only supported local harmonic facts, without editing the copied source pitch/gate.
/// A note's `ChordTone` classification is onset membership, as specified by `PitchFunction`;
/// it does not assert consonance throughout a sustain crossing a later chord change.
fn classify_source_note(
    notes: &[Note],
    index: usize,
    contexts: &[HarmonicContext],
) -> Option<PitchFunction> {
    let note = &notes[index];
    let context = context_at(contexts, note.start_beat)?;
    let pc = pitch_class(note.pitch);
    if context.chord.contains_pc(pc) {
        return Some(PitchFunction::ChordTone);
    }
    if context.palette.tensions.contains(&pc) {
        return Some(PitchFunction::LicensedExtension);
    }
    let previous = index.checked_sub(1).map(|i| &notes[i]);
    let next = notes.get(index + 1);
    let is_chord_tone = |event: &Note| {
        context_at(contexts, event.start_beat)
            .is_some_and(|c| c.chord.contains_pc(pitch_class(event.pitch)))
    };
    let weak_beat = (note.start_beat - note.start_beat.round()).abs() > 1e-6;
    if let (Some(previous), Some(next)) = (previous, next) {
        let into = note.pitch - previous.pitch;
        let out = next.pitch - note.pitch;
        let step_into = (1..=2).contains(&into.abs());
        let step_out = (1..=2).contains(&out.abs());
        let nearby = next.start_beat - note.start_beat <= 1.0
            && note.start_beat - previous.start_beat <= 1.0;
        if nearby && step_into && step_out && is_chord_tone(previous) && is_chord_tone(next) {
            if previous.pitch == next.pitch {
                return Some(PitchFunction::Neighbor);
            }
            if weak_beat && into.signum() == out.signum() {
                return Some(if context.region.contains_pc(pc) {
                    PitchFunction::DiatonicPassing
                } else {
                    PitchFunction::ChromaticPassing
                });
            }
        }
    }
    if let Some(next) = next {
        if (next.pitch - note.pitch).abs() == 1
            && next.start_beat - note.start_beat <= 1.0
            && is_chord_tone(next)
        {
            return Some(PitchFunction::ChromaticApproach);
        }
        if !weak_beat
            && previous.is_some_and(|previous| (note.pitch - previous.pitch).abs() > 2)
            && (1..=2).contains(&(next.pitch - note.pitch).abs())
            && next.start_beat - note.start_beat <= 1.0
            && is_chord_tone(next)
        {
            return Some(PitchFunction::Appoggiatura);
        }
    }
    // No suitable harmonic/path proof. Preservation and compatibility are separate evidence.
    None
}

#[cfg(test)]
mod tests {
    use super::super::argument::ArgumentRelation;
    use super::super::context::analyze;
    use super::super::contract::{CoherenceContract, CompositionGrammar};
    use super::super::form::SectionKind;
    use super::super::harmony::ChordSpan;
    use super::super::material::MaterialEvent;
    use super::super::occupancy::Opportunity;
    use super::super::semantic::SemanticTrace;
    use super::super::theory::{Chord, Mode, Quality, Scale};
    use super::super::timeline::IntentTimeline;
    use super::*;

    // Transcribed from examples/band_story_probe.rs, VERSE and carry(), at fbdbffcee3f77dab.
    // This is a representational control, not a claim that this test hears or accepts music.
    // Onsets include that builder's one declared 0.022-beat lean; gates are independent and
    // accents include the original first-verse lead terrace gain 0.90 exactly once.
    const VERSE: &[(f64, i32, f32, f32)] = &[
        (0.0, 64, 0.5, 0.60),
        (0.5, 67, 0.5, 0.62),
        (1.0, 69, 0.5, 0.68),
        (1.5, 71, 1.0, 0.72),
        (3.0, 69, 0.5, 0.62),
        (3.5, 67, 0.5, 0.58),
        (4.0, 65, 0.5, 0.60),
        (4.5, 69, 0.5, 0.66),
        (5.0, 72, 0.5, 0.70),
        (5.5, 71, 0.25, 0.64),
        (5.75, 69, 0.25, 0.62),
        (6.0, 65, 1.0, 0.58),
        (8.0, 67, 0.5, 0.60),
        (8.5, 72, 0.5, 0.66),
        (9.0, 76, 0.25, 0.72),
        (9.25, 74, 0.25, 0.66),
        (9.5, 72, 0.25, 0.62),
        (9.75, 71, 0.25, 0.60),
        (10.0, 72, 1.0, 0.58),
        (12.0, 71, 0.5, 0.60),
        (12.5, 69, 0.5, 0.58),
        (13.0, 68, 0.5, 0.56),
        (13.5, 64, 0.5, 0.54),
        (14.0, 62, 1.5, 0.52),
    ];

    fn fixture() -> (CompositionPlan, Vec<HarmonicContext>, ArgumentPlacement) {
        let trace = SemanticTrace::new(Vec::new(), 32.0);
        let timeline = IntentTimeline::walk(&trace);
        let mut contract = CoherenceContract::for_grammar(CompositionGrammar::HookArc);
        contract.phrase_bars = 4;
        let plan = CompositionPlan::build_with_contract_for_beats(&timeline, 32.0, contract);
        let contexts = analyze(
            &[
                ChordSpan::test(0.0, 4.0, Chord::new(2, Quality::Min7)),
                ChordSpan::test(4.0, 4.0, Chord::new(7, Quality::Dom7)),
                ChordSpan::test(8.0, 4.0, Chord::new(0, Quality::Maj7)),
                ChordSpan::test(12.0, 4.0, Chord::new(4, Quality::Dom7)),
                ChordSpan::test(16.0, 16.0, Chord::new(9, Quality::Min7)),
            ],
            &Scale::new(9, Mode::Aeolian),
        );
        let placement = ArgumentPlacement {
            step: ArgumentStepId(0),
            referent: ReferentId(0),
            relation: ArgumentRelation::Establish,
            phrase: 0,
            start_beat: 0.0,
            span_beats: 16.0,
            events: VERSE
                .iter()
                .map(|&(onset, pitch, duration, accent)| MaterialEvent {
                    onset: onset + 0.022,
                    dur: f64::from(duration),
                    accent: accent * 0.90,
                    step: Some(pitch - 69),
                })
                .collect(),
            pitch_basis: PitchBasis::Semitones,
            carriers: vec![Agent::Lead],
            anchor_semitones: 0,
        };
        (plan, contexts, placement)
    }

    // An independent finite source oracle: direct original arrays rather than any projection,
    // provenance field, receipt, motif ID, chord map, or transport helper.
    fn original_payload_matches(notes: &[Note], role: Role, octave: i32) -> bool {
        let actual: Vec<_> = notes.iter().filter(|note| note.role == role).collect();
        actual.len() == VERSE.len()
            && actual
                .iter()
                .zip(VERSE)
                .all(|(note, &(onset, pitch, duration, velocity))| {
                    (note.start_beat - (onset + 0.022)).abs() < 1e-12
                        && note.pitch == pitch + octave
                        && note.dur_beats.to_bits() == duration.to_bits()
                        && note.velocity.to_bits() == (velocity * 0.90).to_bits()
                })
    }

    #[test]
    fn original_twenty_four_event_phrase_preserves_pitch_onset_gate_and_accent() {
        // Predeclared: every event survives, gaps and independent gates survive, and the known
        // chromatic note is never replaced by a scale/chord-tone approximation.
        let (plan, contexts, placement) = fixture();
        let mut notes = Vec::new();
        let receipt =
            replace_required_role(&mut notes, Role::Lead, &[placement], 9, &contexts, &plan)
                .unwrap();
        assert!(original_payload_matches(&notes, Role::Lead, 0));
        assert_eq!(receipt.inserted_notes, 24);
        assert_eq!(receipt.placements[0].source_events, 24);
        assert_eq!(notes[21].pitch, 68);
        assert_eq!(notes[0].prov.material, None);
        assert_eq!(notes[0].prov.role_note, "argument");
    }

    #[test]
    fn same_count_replacement_is_valid_carriage() {
        // Predeclared: an exact replacement succeeds despite a zero total-note delta.
        let (plan, contexts, placement) = fixture();
        let mut notes: Vec<_> = VERSE
            .iter()
            .map(|&(onset, _, duration, _)| {
                Note::new(
                    onset,
                    duration,
                    69,
                    0.5,
                    Role::Lead,
                    Provenance::new(SectionKind::A),
                )
            })
            .collect();
        let count_before = notes.len();
        let receipt =
            replace_required_role(&mut notes, Role::Lead, &[placement], 9, &contexts, &plan)
                .unwrap();
        assert_eq!(notes.len(), count_before);
        assert_eq!(receipt.removed_backing, 24);
        assert_eq!(receipt.inserted_notes, 24);
        assert!(original_payload_matches(&notes, Role::Lead, 0));
    }

    #[test]
    fn all_three_named_carriers_receive_the_whole_phrase() {
        // Predeclared: carriage is an AND over the declared carrier list, not one optional reply.
        let (plan, contexts, mut placement) = fixture();
        placement.carriers = vec![Agent::Lead, Agent::Keys, Agent::Bass];
        let mut notes = Vec::new();
        for role in [Role::Lead, Role::Keys, Role::Bass] {
            let receipt =
                replace_required_role(&mut notes, role, &[placement.clone()], 9, &contexts, &plan)
                    .unwrap();
            assert_eq!(receipt.inserted_notes, 24);
        }
        assert!(original_payload_matches(&notes, Role::Lead, 0));
        assert!(original_payload_matches(&notes, Role::Keys, -12));
        assert!(original_payload_matches(&notes, Role::Bass, -24));
        notes.retain(|note| note.role != Role::Keys);
        assert!(!original_payload_matches(&notes, Role::Keys, -12));
        assert!(original_payload_matches(&notes, Role::Lead, 0));
    }

    #[test]
    fn raw_oracle_rejects_wrong_pitch_rhythm_gate_accent_and_tail_quote() {
        let (plan, contexts, placement) = fixture();
        let mut notes = Vec::new();
        replace_required_role(&mut notes, Role::Lead, &[placement], 9, &contexts, &plan).unwrap();
        // Predeclared: copied provenance cannot make these unrelated or lossy copies preserved.
        let mut wrong = notes.clone();
        wrong[1].pitch = wrong[0].pitch;
        assert!(!original_payload_matches(&wrong, Role::Lead, 0));
        let mut wrong = notes.clone();
        wrong[4].start_beat -= 0.5;
        assert!(!original_payload_matches(&wrong, Role::Lead, 0));
        let mut wrong = notes.clone();
        wrong[3].dur_beats = 1.5;
        assert!(!original_payload_matches(&wrong, Role::Lead, 0));
        let mut wrong = notes.clone();
        wrong[3].velocity = 0.1;
        assert!(!original_payload_matches(&wrong, Role::Lead, 0));
        assert!(!original_payload_matches(&notes[20..], Role::Lead, 0));
    }

    #[test]
    fn full_span_reserves_gaps_and_clips_crossing_backing_without_inventing_reattack() {
        let (plan, contexts, mut placement) = fixture();
        placement.start_beat = 1.0;
        placement.span_beats = 15.0;
        placement
            .events
            .retain(|event| event.onset + event.dur <= 15.0);
        let mut notes = vec![
            Note::new(
                0.0,
                4.0,
                69,
                0.5,
                Role::Lead,
                Provenance::new(SectionKind::A),
            ),
            // This falls in the source's first internal rest, not at a copied event.
            Note::new(
                3.75,
                0.25,
                69,
                0.5,
                Role::Lead,
                Provenance::new(SectionKind::A),
            ),
            Note::new(
                16.0,
                1.0,
                69,
                0.5,
                Role::Lead,
                Provenance::new(SectionKind::A),
            ),
            Note::new(
                3.75,
                0.25,
                45,
                0.5,
                Role::Bass,
                Provenance::new(SectionKind::A),
            ),
        ];
        let receipt = replace_required_role(
            &mut notes,
            Role::Lead,
            &[placement.clone()],
            9,
            &contexts,
            &plan,
        )
        .unwrap();
        assert_eq!(receipt.clipped_backing, 1);
        assert_eq!(receipt.removed_backing, 1);
        assert_eq!(notes[0].dur_beats, 1.0);
        assert!(notes.iter().any(|note| note.start_beat == 16.0));
        assert!(notes
            .iter()
            .any(|note| note.role == Role::Bass && note.start_beat == 3.75));
        assert!(!notes
            .iter()
            .any(|note| note.role == Role::Lead && note.start_beat == 3.75));

        let mut occupancy = AuthoredOccupancy {
            role: Role::Lead,
            rhythm: Vec::new(),
            spans: Vec::new(),
        };
        reserve_required_role(&mut occupancy, &[placement]);
        assert_eq!(occupancy.opportunity(3.75), Opportunity::InternalRest);
        assert!(!occupancy.yields_at(3.75));
        assert_eq!(occupancy.opportunity(0.5), Opportunity::Yield);
        assert!(occupancy.spans.iter().all(|span| span.material.is_none()));
    }

    #[test]
    fn infeasible_sources_fail_atomically_and_harmonic_incompatibility_is_not_repitched() {
        let (plan, contexts, placement) = fixture();
        let backing = vec![Note::new(
            0.0,
            1.0,
            69,
            0.5,
            Role::Lead,
            Provenance::new(SectionKind::A),
        )];
        let mut broken = placement.clone();
        broken.events[0].step = Some(200);
        let mut notes = backing.clone();
        assert!(matches!(
            replace_required_role(&mut notes, Role::Lead, &[broken], 9, &contexts, &plan),
            Err(CarrierTransportError::PitchOutOfRange { .. })
        ));
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].pitch, backing[0].pitch);
        let mut broken = placement.clone();
        broken.pitch_basis = PitchBasis::ScaleSteps;
        assert!(matches!(
            replace_required_role(&mut notes, Role::Lead, &[broken], 9, &contexts, &plan),
            Err(CarrierTransportError::WrongPitchBasis { .. })
        ));
        let mut broken = placement.clone();
        broken.carriers.push(Agent::Lead);
        assert!(matches!(
            replace_required_role(&mut notes, Role::Lead, &[broken], 9, &contexts, &plan),
            Err(CarrierTransportError::DuplicateCarrier { .. })
        ));
        let mut broken = placement.clone();
        broken.carriers.clear();
        assert!(matches!(
            replace_required_role(&mut notes, Role::Lead, &[broken], 9, &contexts, &plan),
            Err(CarrierTransportError::MissingCarrier { .. })
        ));
        let mut broken = placement.clone();
        broken.events.clear();
        assert!(matches!(
            replace_required_role(&mut notes, Role::Lead, &[broken], 9, &contexts, &plan),
            Err(CarrierTransportError::EmptyMaterial { .. })
        ));
        let mut broken = placement;
        broken.events[0].step = Some(-11); // A#4 over Dm: unsupported by this source path.
        let receipt =
            replace_required_role(&mut notes, Role::Lead, &[broken], 9, &contexts, &plan).unwrap();
        assert_eq!(notes[0].pitch, 58);
        assert_eq!(notes[0].function, None);
        assert!(receipt.unclassified_notes > 0);
    }

    #[test]
    fn validated_chromatic_coordinates_cannot_overflow_at_lowering() {
        let (plan, contexts, mut placement) = fixture();
        placement.events.truncate(1);
        placement.anchor_semitones = i32::MAX;
        placement.events[0].step = Some(-i32::MAX);
        let mut notes = Vec::new();
        replace_required_role(&mut notes, Role::Lead, &[placement], 9, &contexts, &plan).unwrap();
        assert_eq!(notes[0].pitch, 69);
    }

    #[test]
    fn overlapping_same_carrier_reservations_and_missing_context_are_rejected() {
        let (plan, contexts, placement) = fixture();
        let mut duplicate = placement.clone();
        duplicate.step = ArgumentStepId(1);
        assert!(matches!(
            validate_required_placements(&[placement.clone(), duplicate], 9, &contexts, &plan),
            Err(CarrierTransportError::OverlappingReservations { .. })
        ));
        assert!(matches!(
            validate_required_placements(&[placement], 9, &[], &plan),
            Err(CarrierTransportError::MissingHarmonicContext { .. })
        ));
    }
}
