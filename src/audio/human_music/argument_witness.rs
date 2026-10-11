//! Independent, bounded witnesses for optional musical arguments.
//!
//! Raw score content, not motif/action/material labels, is the evidence. The observer
//! reconstructs the declared rational timing and chromatic contour directly from the
//! original referent; it never calls the compiler's event transformation. This checks
//! source preservation and a finite contrast domain, not human comprehension.

use super::action::Agent;
use super::argument::{ArgumentEnding, ArgumentRelation, ArgumentStepId, CompiledArgument};
use super::score::{Note, Role, Score};
use super::theory::PitchBasis;
use crate::audio::buffer::StereoBlock;
use std::collections::BTreeSet;

const EPS: f64 = 0.00001;

/// A content witness failure; the step names locate failures, never establish identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentViolation {
    pub step: ArgumentStepId,
    pub carrier: Option<Agent>,
    pub reason: &'static str,
}

/// Source identity, declared dependencies and observed content are reported
/// separately from acoustic exposure and all human listening verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentWitness {
    violations: Vec<ArgumentViolation>,
    signature: Vec<ObservedStatement>,
}

impl ArgumentWitness {
    pub fn valid(&self) -> bool {
        self.violations.is_empty()
    }
    pub fn violations(&self) -> &[ArgumentViolation] {
        &self.violations
    }
    pub fn signature(&self) -> &[ObservedStatement] {
        &self.signature
    }
}

/// Observe the required material in a fixed tonic frame. Constant octave changes
/// preserve identity; a semitone shift of an answer in that same frame fails.
pub fn observe_argument(
    compiled: &CompiledArgument,
    score: &Score,
    tonic_pc: i32,
) -> ArgumentWitness {
    let mut violations = verify(compiled, score);
    for placement in &compiled.placements {
        let Some(expected) = expected_events(compiled, placement.step) else {
            continue;
        };
        let span = placement.span_beats;
        for agent in &placement.carriers {
            let Some(role) = role(*agent) else {
                continue;
            };
            let notes = in_window(score, role, placement.start_beat, span);
            let register = match role {
                Role::Lead => 60,
                Role::Keys => 48,
                Role::Bass => 36,
                Role::Pad => 48,
            };
            if let (Some(note), Some(event)) = (notes.first(), expected.first()) {
                let source_pitch = register + tonic_pc.rem_euclid(12) + event.2;
                if (note.pitch - source_pitch).rem_euclid(12) != 0 {
                    violations.push(violation(
                        placement.step,
                        Some(*agent),
                        "source changed pitch class in the declared tonic frame",
                    ));
                }
            }
        }
    }
    violations.extend(harmonic_violations(compiled, score, tonic_pc));
    ArgumentWitness {
        violations,
        signature: observe(compiled, score, tonic_pc),
    }
}

/// One section cycle of the traveling chart: four bars of four beats. A verse/home loop resolves
/// (to its V7 / tonic) only on the last bar of a full cycle; a phrase shorter than this truncates
/// the progression before that resolving bar.
const BEATS_PER_BAR: f64 = 4.0;
const SECTION_CYCLE_BEATS: f64 = BEATS_PER_BAR * 4.0;

// The argument compiler reserves functional V7 preparation and tonic completion. Under R3 the chart
// TRAVELS, so the destination is realized at the phrase's cadence bar (its last bar), not at the
// theme's early last-event beat — the material states in the opening bars while the chart keeps
// moving to its functional goal. Inspect actual chord pitches in the Score; labels are not proof.
fn harmonic_violations(
    compiled: &CompiledArgument,
    score: &Score,
    tonic_pc: i32,
) -> Vec<ArgumentViolation> {
    let mut failures = Vec::new();
    let tonic = tonic_pc.rem_euclid(12);
    let dominant = (tonic + 7).rem_euclid(12);
    // The prepared dominant seventh: root on the dominant, with the leading tone and the resolving
    // tritone fifth. Reconstructed from theory and the fixed tonic, never from the compiler's chart.
    let is_dominant = |c: super::theory::Chord| {
        let pcs = c.pitch_classes();
        c.root_pc == dominant
            && pcs.contains(&(tonic + 11).rem_euclid(12))
            && pcs.contains(&(tonic + 5).rem_euclid(12))
    };
    for step in &compiled.argument.steps {
        let Some(placement) = compiled.placements.iter().find(|p| p.step == step.id) else {
            continue;
        };
        let Some(events) = expected_events(compiled, step.id) else {
            continue;
        };
        let Some(last) = events.last() else {
            continue;
        };
        // A sub-cycle phrase never renders the progression's resolving bar, so it makes no cadential
        // claim and is not checked. The full-cycle phrases carry the destination, unchanged.
        if placement.span_beats < SECTION_CYCLE_BEATS - EPS {
            continue;
        }
        let cadence_bar =
            ((placement.start_beat + placement.span_beats - EPS) / BEATS_PER_BAR).floor();
        let at = cadence_bar * BEATS_PER_BAR;
        let chord = score
            .chords
            .iter()
            .find(|c| c.start_beat <= at + EPS && c.start_beat + f64::from(c.dur_beats) > at + EPS)
            .map(|c| c.chord);
        let valid = match step.relation {
            ArgumentRelation::Question { .. } => chord.is_some_and(is_dominant),
            // The opening STATES the theme over the verse travel and hangs on its prepared dominant
            // just as the question does; the chart no longer prolongs a tonic home under it. Guarded
            // by its own completion so the fusion verse — which carries no completion and makes no
            // such cadential claim — never enters this branch and its render is untouched.
            ArgumentRelation::Establish if step.transform.completion.is_some() => {
                chord.is_some_and(is_dominant)
            }
            ArgumentRelation::Denial { .. } => chord.is_some_and(|c| c.root_pc != tonic),
            ArgumentRelation::Answer { .. } | ArgumentRelation::Return { .. }
                if step.transform.completion.is_some() =>
            {
                chord.is_some_and(|c| c.root_pc == tonic && c.contains_pc(tonic + last.2))
            }
            _ => true,
        };
        if !valid {
            failures.push(violation(
                step.id,
                None,
                "actual chord does not prepare or realize the declared destination",
            ));
        }
    }
    failures
}

/// Relative, octave-independent notes on a microbeat observation lattice.
/// This explicitly declares the finite observer's precision.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObservedNote {
    pub onset_microbeats: i64,
    pub gate_microbeats: i64,
    pub semitones_from_first: i32,
    pub accent_millionths: i64,
}

/// One musical statement observed without semantic labels or source IDs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObservedStatement {
    pub start_microbeats: i64,
    /// Initial pitch class in the explicitly supplied tonic frame; quotient only
    /// octave/register changes, not an unrelated semitone displacement.
    pub first_pitch_class_from_tonic: i32,
    pub notes: Vec<ObservedNote>,
}

/// The restricted content observer ignores lawful octave moves and duplicate
/// orchestration of the same statement. It retains order, rests, gates and accents.
pub fn observe(
    compiled: &CompiledArgument,
    score: &Score,
    tonic_pc: i32,
) -> Vec<ObservedStatement> {
    let mut result = Vec::new();
    for placement in &compiled.placements {
        let Some(_expected) = expected_events(compiled, placement.step) else {
            continue;
        };
        let span = placement.span_beats;
        for carrier in &placement.carriers {
            let Some(role) = role(*carrier) else {
                continue;
            };
            let notes = in_window(score, role, placement.start_beat, span);
            result.push(ObservedStatement {
                start_microbeats: quantize(placement.start_beat),
                first_pitch_class_from_tonic: notes
                    .first()
                    .map(|n| (n.pitch - tonic_pc).rem_euclid(12))
                    .unwrap_or(-1),
                notes: signature(&notes, placement.start_beat),
            });
        }
    }
    result.sort();
    result.dedup();
    result
}

/// Check each required carrier from independently reconstructed source events,
/// plus temporal dependencies and the actual source-specific answer relation.
/// Extra onsets inside declared rests fail just as missing notes do.
pub fn verify(compiled: &CompiledArgument, score: &Score) -> Vec<ArgumentViolation> {
    let mut failures = Vec::new();
    let mut open = BTreeSet::new();
    let mut steps_seen = BTreeSet::new();
    if compiled.argument.steps.is_empty() {
        failures.push(violation(
            ArgumentStepId(0),
            None,
            "empty argument has no witnessed proposition",
        ));
    }
    for step in &compiled.argument.steps {
        if !steps_seen.insert(step.id) {
            failures.push(violation(step.id, None, "duplicate step identity"));
        }
        let Some(placement) = compiled.placements.iter().find(|p| p.step == step.id) else {
            failures.push(violation(step.id, None, "missing placement"));
            continue;
        };
        let Some(expected) = expected_events(compiled, step.id) else {
            failures.push(violation(
                step.id,
                None,
                "invalid referent or transformation",
            ));
            continue;
        };
        let span = placement.span_beats;
        if placement.carriers != step.carriers
            || placement.referent != step.referent
            || placement.relation != step.relation
            || placement.anchor_semitones != 0
            || !span.is_finite()
            || span <= 0.0
            || !placement.start_beat.is_finite()
            || placement.start_beat < 0.0
            || placement.start_beat + span > score.total_beats + EPS
            || expected.iter().any(|e| e.0 + e.1 > span + EPS)
            || step.carriers.is_empty()
            || step.carriers.iter().collect::<BTreeSet<_>>().len() != step.carriers.len()
        {
            failures.push(violation(
                step.id,
                None,
                "placement does not preserve the source commitment",
            ));
        }
        let required: Vec<ArgumentStepId> = match step.relation {
            ArgumentRelation::Establish => Vec::new(),
            ArgumentRelation::Question { source, .. }
            | ArgumentRelation::Develop { source }
            | ArgumentRelation::Return { source } => vec![source],
            ArgumentRelation::Denial { question } | ArgumentRelation::Answer { question } => {
                vec![question]
            }
            // A consequent is intentionally a DIFFERENT referent (the hook, not the verse), so it is
            // exempt from the same-referent carriage check here; its kinship to the antecedent is a
            // symbolic contract property proven in `MusicalArgument::validate`, while this witness
            // still verifies the hook's OWN notes are actually played (below). The antecedent's
            // ordering is enforced via `depends_on`.
            ArgumentRelation::Consequent { .. } | ArgumentRelation::Depart { .. } => Vec::new(),
            ArgumentRelation::Reconcile { left, right } => vec![left, right],
        };
        if required.iter().any(|id| !step.depends_on.contains(id)) {
            failures.push(violation(
                step.id,
                None,
                "relation lacks declared source dependency",
            ));
        }
        for id in &required {
            if !same_referent(compiled, *id, step.id) {
                failures.push(violation(
                    step.id,
                    None,
                    "relationship changes the defining source",
                ));
            }
        }
        let mut predecessors = step.depends_on.clone();
        predecessors.extend(required);
        predecessors.sort();
        predecessors.dedup();
        for predecessor in &predecessors {
            let prior = compiled.placements.iter().find(|p| p.step == *predecessor);
            let end = prior.and_then(|p| {
                expected_events(compiled, p.step).map(|events| {
                    p.start_beat + events.iter().map(|e| e.0 + e.1).fold(0.0, f64::max)
                })
            });
            if end.is_none_or(|end| end > placement.start_beat + EPS) {
                failures.push(violation(step.id, None, "unearned dependency order"));
            }
        }
        for carrier in &step.carriers {
            let Some(role) = role(*carrier) else {
                failures.push(violation(
                    step.id,
                    Some(*carrier),
                    "unsupported pitched carrier",
                ));
                continue;
            };
            let notes = in_window(score, role, placement.start_beat, span);
            if !matches_source(&notes, &expected, placement.start_beat) {
                failures.push(violation(
                    step.id,
                    Some(*carrier),
                    "raw notes do not preserve timed source",
                ));
            }
        }
        if matches!(step.relation, ArgumentRelation::Reconcile { .. }) {
            failures.push(violation(
                step.id,
                None,
                "reconciliation has no implemented compatibility witness",
            ));
        }
        if let ArgumentRelation::Question { .. } = step.relation {
            open.insert(step.id);
        }
        if let ArgumentRelation::Denial { question } = step.relation {
            let q = compiled.argument.steps.iter().find(|s| s.id == question);
            let prepared = q.is_some_and(|q| {
                if let ArgumentRelation::Question { source, expected } = q.relation {
                    let taught = compiled.argument.steps.iter().find(|s| s.id == source);
                    open.contains(&question)
                        && taught.is_some_and(|s| s.transform.completion == Some(expected))
                        && step
                            .transform
                            .completion
                            .is_some_and(|c| c.step != expected.step)
                } else {
                    false
                }
            });
            if !prepared {
                failures.push(violation(
                    step.id,
                    None,
                    "denied destination was never taught",
                ));
            }
        }
        if let ArgumentRelation::Answer { question } = step.relation {
            let question_step = compiled.argument.steps.iter().find(|s| s.id == question);
            let question_events = expected_events(compiled, question);
            let expected_closure = question_step.and_then(|q| match q.relation {
                ArgumentRelation::Question { expected, .. } => Some(expected),
                _ => None,
            });
            let related = match (question_events, expected_closure) {
                (Some(q), Some(closure)) if expected.len() == q.len() + 1 => {
                    let qstep = question_step.expect("question step established above");
                    let qscale = f64::from(qstep.transform.time_numerator)
                        / f64::from(qstep.transform.time_denominator);
                    let ascale = f64::from(step.transform.time_numerator)
                        / f64::from(step.transform.time_denominator);
                    // `expected_events` bakes `accent * section_gain` into the accent field (R4b: the
                    // lead breathes across sections). Head identity is the motif SHAPE, not the
                    // section loudness — the same head restated at the answer's full gain must still
                    // read as the question's head at its held-back gain. Divide each side back out by
                    // its own section dynamic so the comparison is between raw motif accents; a
                    // genuinely wrong accent shape still fails. Gains are the structured 0.70–1.0 arc,
                    // never zero.
                    let qgain = f64::from(
                        compiled
                            .placements
                            .iter()
                            .find(|p| p.step == question)
                            .map_or(1.0_f32, |p| p.section_gain),
                    );
                    let again = f64::from(
                        compiled
                            .placements
                            .iter()
                            .find(|p| p.step == step.id)
                            .map_or(1.0_f32, |p| p.section_gain),
                    );
                    let head_matches = q.iter().zip(&expected).all(|(a, b)| {
                        (a.0 / qscale - b.0 / ascale).abs() <= EPS
                            && (a.1 / qscale - b.1 / ascale).abs() <= EPS
                            && a.2 - 12 * i32::from(qstep.transform.transpose_octaves)
                                == b.2 - 12 * i32::from(step.transform.transpose_octaves)
                            && (f64::from(a.3) / qgain - f64::from(b.3) / again).abs() <= EPS
                    });
                    let final_event = expected.last().expect("nonempty answer");
                    head_matches
                        && open.contains(&question)
                        && final_event.2 - 12 * i32::from(step.transform.transpose_octaves)
                            == closure.step
                        && (final_event.1 / ascale - closure.dur).abs() <= EPS
                        && step.depends_on.contains(&question)
                }
                _ => false,
            };
            if !related {
                failures.push(violation(
                    step.id,
                    None,
                    "answer is not bound to the whole question and closure",
                ));
            }
            open.remove(&question);
        }
    }
    if compiled.argument.ending == ArgumentEnding::Resolved {
        for question in open {
            failures.push(violation(
                question,
                None,
                "resolved ending leaves its question unanswered",
            ));
        }
    }
    failures
}

fn same_referent(compiled: &CompiledArgument, left: ArgumentStepId, right: ArgumentStepId) -> bool {
    let source = |id| {
        let step = compiled.argument.steps.iter().find(|s| s.id == id)?;
        compiled
            .argument
            .referents
            .iter()
            .find(|r| r.id == step.referent)
    };
    let (Some(a), Some(b)) = (source(left), source(right)) else {
        return false;
    };
    if a.events.len() != b.events.len() || a.events.is_empty() {
        return false;
    }
    let (Some(pa), Some(pb)) = (a.events[0].step, b.events[0].step) else {
        return false;
    };
    let scale = b.events[0].dur / a.events[0].dur;
    (pb - pa).rem_euclid(12) == 0
        && scale.is_finite()
        && scale > 0.0
        && a.events.iter().zip(&b.events).all(|(x, y)| {
            x.step.map(|p| p - pa) == y.step.map(|p| p - pb)
                && (x.onset * scale - y.onset).abs() <= EPS
                && (x.dur * scale - y.dur).abs() <= EPS
                && (x.accent - y.accent).abs() <= EPS as f32
        })
}

// (onset, gate, relative semitones, accent). Separate arithmetic implementation:
// inspect original source, scale times, then append the declared completion.
fn expected_events(
    compiled: &CompiledArgument,
    id: ArgumentStepId,
) -> Option<Vec<(f64, f64, i32, f32)>> {
    let step = compiled.argument.steps.iter().find(|s| s.id == id)?;
    let source = compiled
        .argument
        .referents
        .iter()
        .find(|r| r.id == step.referent)?;
    if source.pitch_basis != PitchBasis::Semitones {
        return None;
    }
    // The carrier's sounding velocity is its authored accent scaled by this placement's declared
    // section dynamic (the same product the transport applies). The witness verifies that exact
    // product rather than the bare accent, so the lead may breathe across sections AND stay checked.
    let gain = compiled
        .placements
        .iter()
        .find(|p| p.step == id)
        .map_or(1.0, |p| p.section_gain);
    if step.transform.time_denominator == 0 || step.transform.time_numerator == 0 {
        return None;
    }
    let scale =
        f64::from(step.transform.time_numerator) / f64::from(step.transform.time_denominator);
    let mut result: Vec<_> = source
        .events
        .iter()
        .map(|e| {
            Some((
                e.onset * scale,
                e.dur * scale,
                e.step? + 12 * i32::from(step.transform.transpose_octaves),
                (e.accent * gain).clamp(0.0, 1.0),
            ))
        })
        .collect::<Option<_>>()?;
    if let Some(completion) = step.transform.completion {
        let start = source
            .events
            .iter()
            .map(|e| e.onset + e.dur)
            .fold(0.0, f64::max)
            .ceil()
            * scale;
        result.push((
            start,
            completion.dur * scale,
            completion.step + 12 * i32::from(step.transform.transpose_octaves),
            gain.clamp(0.0, 1.0),
        ));
    }
    Some(result)
}

fn matches_source(notes: &[&Note], expected: &[(f64, f64, i32, f32)], start: f64) -> bool {
    if notes.len() != expected.len() || notes.is_empty() {
        return false;
    }
    let pitch_shift = notes[0].pitch - expected[0].2;
    // Only constant octave displacement is quotiented out; interval destruction is not.
    notes.iter().zip(expected).all(|(n, e)| {
        n.start_beat.is_finite()
            && n.dur_beats.is_finite()
            && n.velocity.is_finite()
            && (n.start_beat - start - e.0).abs() <= EPS
            && (f64::from(n.dur_beats) - e.1).abs() <= EPS
            && n.pitch - e.2 == pitch_shift
            && (n.velocity - e.3).abs() <= EPS as f32
    })
}

fn in_window(score: &Score, role: Role, start: f64, span: f64) -> Vec<&Note> {
    let mut notes: Vec<_> = score
        .notes
        .iter()
        .filter(|n| {
            n.role == role && n.start_beat >= start - EPS && n.start_beat < start + span - EPS
        })
        .collect();
    notes.sort_by(|a, b| {
        a.start_beat
            .total_cmp(&b.start_beat)
            .then(a.pitch.cmp(&b.pitch))
    });
    notes
}

fn signature(notes: &[&Note], start: f64) -> Vec<ObservedNote> {
    let first = notes.first().map(|n| n.pitch).unwrap_or(0);
    notes
        .iter()
        .map(|n| ObservedNote {
            onset_microbeats: quantize(n.start_beat - start),
            gate_microbeats: quantize(f64::from(n.dur_beats)),
            semitones_from_first: n.pitch - first,
            accent_millionths: quantize(f64::from(n.velocity)),
        })
        .collect()
}

fn quantize(x: f64) -> i64 {
    (x * 1_000_000.0).round() as i64
}

fn role(agent: Agent) -> Option<Role> {
    match agent {
        Agent::Lead => Some(Role::Lead),
        Agent::Keys => Some(Role::Keys),
        Agent::Bass => Some(Role::Bass),
        Agent::Pad => Some(Role::Pad),
        _ => None,
    }
}

fn violation(
    step: ArgumentStepId,
    carrier: Option<Agent>,
    reason: &'static str,
) -> ArgumentViolation {
    ArgumentViolation {
        step,
        carrier,
        reason,
    }
}

/// PCM evidence is a separate layer. A detectable ablation residual can only avoid
/// an exposure rejection; it does not establish salience, identity recognition or
/// comprehension, especially through a nonlinear shared master bus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcousticExposure {
    pub finite: bool,
    pub mix_rms: f64,
    pub ablation_residual_rms: f64,
    pub reject_absent_signal: bool,
}

/// Measure raw rendered PCM against an otherwise identical thematic ablation.
/// The frame interval is explicitly supplied rather than inferred from metadata.
pub fn acoustic_exposure(
    full: &StereoBlock,
    ablated: &StereoBlock,
    start: usize,
    end: usize,
) -> AcousticExposure {
    let stop = end
        .min(full.frames())
        .min(ablated.frames())
        .min(full.right.len())
        .min(ablated.right.len());
    if start >= stop {
        return AcousticExposure {
            finite: false,
            mix_rms: 0.0,
            ablation_residual_rms: 0.0,
            reject_absent_signal: true,
        };
    }
    let mut finite = true;
    let (mut mix, mut difference) = (0.0, 0.0);
    for i in start..stop {
        for (a, b) in [
            (full.left[i], ablated.left[i]),
            (full.right[i], ablated.right[i]),
        ] {
            finite &= a.is_finite() && b.is_finite();
            mix += f64::from(a).powi(2);
            difference += f64::from(a - b).powi(2);
        }
    }
    let count = ((stop - start) * 2) as f64;
    let mix_rms = (mix / count).sqrt();
    let ablation_residual_rms = (difference / count).sqrt();
    AcousticExposure {
        finite,
        mix_rms,
        ablation_residual_rms,
        reject_absent_signal: !finite || mix_rms <= 1e-7 || ablation_residual_rms <= 1e-7,
    }
}
