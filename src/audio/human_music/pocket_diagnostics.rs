//! Separated Round XVII timing observations. Lattice membership does not establish metric
//! phase, phrase timing, or human acceptance. These observers share realization provenance.
use super::expression::{annotate, ExpressionEvent};
use super::ids::ActionId;
use super::performance::{PerformancePlan, StepWeight};
use super::phrase_expression::PhraseTransform;
use super::pocket::{slots, LatticeSlot};
use super::score::{DrumVoice, Note, Role, Score};
use super::voice::VoiceEventId;
use super::world::MusicWorld;
use std::fmt::Write;

/// Signed shortest change in pulse phase, in beats, without conflating whole-beat displacement.
/// Inputs are unswung pulse coordinates. A half-beat tie retains displacement's sign.
pub fn metric_phase_deviation(source: f64, performed: f64) -> f64 {
    let delta = performed - source;
    let phase = delta.rem_euclid(1.0);
    if phase > 0.5 || (phase == 0.5 && delta < 0.0) {
        phase - 1.0
    } else {
        phase
    }
}

/// A nearest lattice coordinate and the signed distance from it. Nonzero residual is retained.
#[derive(Debug, Clone)]
pub struct PocketCoordinate {
    pub slot: LatticeSlot,
    pub residual_beats: f64,
    pub phase_class: &'static str,
}

/// One source note, its actual surviving event if found, and orthogonal timing measurements.
#[derive(Debug, Clone)]
pub struct PocketNote {
    pub source: Note,
    pub optional: bool,
    pub source_coordinate: PocketCoordinate,
    /// Exact index in Score.notes; absent for omission or an unresolved source-ledger match.
    pub score_index: Option<usize>,
    pub actual_match_count: usize,
    pub performed: Option<Note>,
    pub performed_coordinate: Option<PocketCoordinate>,
    pub signed_displacement_beats: Option<f64>,
    pub phase_deviation_beats: Option<f64>,
    pub source_accent: StepWeight,
    pub performed_accent: Option<StepWeight>,
    /// Actual nearest drum attack, with signed (pitched onset minus drum onset) difference.
    pub nearest_drum: Option<(DrumVoice, f64, f64)>,
    pub ledger_status: &'static str,
}

/// Raw same-role onsets with structural bits; neighboring structural notes bound the IOI view.
#[derive(Debug, Clone)]
pub struct PocketTimeline {
    pub events: Vec<ExpressionEvent>,
    pub iois: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct PocketPhrase {
    pub plan_index: usize,
    pub role: Role,
    pub phrase: Option<u32>,
    pub transform: PhraseTransform,
    pub destination: Note,
    pub destination_score_index: Option<usize>,
    pub destination_coordinate: PocketCoordinate,
    pub notes: Vec<PocketNote>,
    pub source_timeline: PocketTimeline,
    pub performed_timeline: PocketTimeline,
}

/// Same action identity is required; temporal proximity alone never invents a coordination.
#[derive(Debug, Clone)]
pub struct SharedActionTiming {
    pub action: ActionId,
    pub lead_score_index: usize,
    pub bass_score_index: usize,
    pub source_lead_minus_bass: f64,
    pub performed_lead_minus_bass: f64,
}

/// Repeated source-relative motion with the same role and source/performed metric phase.
/// This descriptive grouping has no pass/fail threshold or musical quality interpretation.
#[derive(Debug, Clone)]
pub struct PhaseMorphology {
    pub role: Role,
    pub direction: &'static str,
    pub source_phase: (i64, u32),
    pub performed_phase: (i64, u32),
    pub occurrences: Vec<(usize, f64, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct PocketDiagnostics {
    pub phrases: Vec<PocketPhrase>,
    pub shared_actions: Vec<SharedActionTiming>,
    pub repeated_phase_morphologies: Vec<PhaseMorphology>,
    /// Runs in source-onset order within each role; another transformed phase ends the run.
    pub consecutive_phase_morphologies: Vec<PhaseMorphology>,
}

fn same(a: &Note, b: &Note) -> bool {
    // Final arrangement stamps enrich provenance. Use the declared stable event identity,
    // retaining exact articulation, function and interaction in addition to that identity.
    VoiceEventId::of(a) == VoiceEventId::of(b)
        && a.dur_beats == b.dur_beats
        && a.velocity == b.velocity
        && a.prov.interaction == b.prov.interaction
        && a.function == b.function
}

fn unique_actual<'a>(score: &'a Score, candidate: &Note) -> Option<(usize, &'a Note)> {
    let mut matches = score
        .notes
        .iter()
        .enumerate()
        .filter(|(_, n)| same(candidate, n));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn coordinate(perf: &PerformancePlan, world: &MusicWorld, onset: f64) -> PocketCoordinate {
    let slot = slots(
        world,
        perf.language.surface_subdivision,
        onset - 2.0,
        onset + 2.0,
    )
    .into_iter()
    .min_by(|a, b| {
        (a.position - onset)
            .abs()
            .total_cmp(&(b.position - onset).abs())
    })
    .expect("a four-beat corridor contains a declared lattice slot");
    let phase = slot.index.rem_euclid(i64::from(slot.subdivision));
    let phase_class = if phase == 0 {
        "pulse"
    } else if 2 * phase == i64::from(slot.subdivision) {
        "half-beat"
    } else {
        "subdivision"
    };
    PocketCoordinate {
        residual_beats: onset - slot.position,
        slot,
        phase_class,
    }
}

fn source_for(score: &Score, n: &Note) -> Note {
    score
        .expression_decisions
        .iter()
        .rev()
        .find(|d| d.after.as_ref().is_some_and(|a| same(a, n)))
        .map_or(*n, |d| d.before.note)
}

fn structural(perf: &PerformancePlan, score: &Score, source: &Note) -> bool {
    score
        .expression_decisions
        .iter()
        .find(|d| same(&d.before.note, source))
        .map_or_else(
            || annotate(perf, &[*source])[0].structural,
            |d| !d.before.optional,
        )
}

fn timeline(mut events: Vec<ExpressionEvent>, lo: f64, hi: f64) -> PocketTimeline {
    events.retain(|e| e.note.start_beat >= lo && e.note.start_beat <= hi);
    events.sort_by(|a, b| a.note.start_beat.total_cmp(&b.note.start_beat));
    let iois = events
        .windows(2)
        .map(|p| p[1].note.start_beat - p[0].note.start_beat)
        .collect();
    PocketTimeline { events, iois }
}

impl PocketDiagnostics {
    pub fn measure(perf: &PerformancePlan, score: &Score, world: &MusicWorld) -> Self {
        let mut out = Self::default();
        let mut source: Vec<Note> = score.notes.iter().map(|n| source_for(score, n)).collect();
        for note in score
            .expression_decisions
            .iter()
            .map(|d| d.before.note)
            .chain(
                score
                    .phrase_plans
                    .iter()
                    .flat_map(|p| p.source.iter().copied()),
            )
        {
            if !source.iter().any(|n| same(n, &note)) {
                source.push(note);
            }
        }
        for (plan_index, plan) in score
            .phrase_plans
            .iter()
            .enumerate()
            .filter(|(_, p)| p.transform != PhraseTransform::AsWritten)
        {
            let source_events: Vec<_> = source
                .iter()
                .filter(|n| n.role == plan.role)
                .map(|n| ExpressionEvent {
                    note: *n,
                    structural: structural(perf, score, n),
                })
                .collect();
            let first = plan
                .source
                .iter()
                .map(|n| n.start_beat)
                .fold(plan.destination.start_beat, f64::min);
            let last = plan
                .source
                .iter()
                .map(|n| n.start_beat)
                .fold(plan.destination.start_beat, f64::max);
            let lo = source_events
                .iter()
                .filter(|e| e.structural && e.note.start_beat < first)
                .map(|e| e.note.start_beat)
                .max_by(f64::total_cmp)
                .unwrap_or(first);
            let hi = source_events
                .iter()
                .filter(|e| e.structural && e.note.start_beat > last)
                .map(|e| e.note.start_beat)
                .min_by(f64::total_cmp)
                .unwrap_or(last);
            let actual_events = score
                .role_notes(plan.role)
                .map(|n| ExpressionEvent {
                    note: *n,
                    structural: structural(perf, score, &source_for(score, n)),
                })
                .collect();
            let mut notes = Vec::new();
            for n in &plan.source {
                let decision = score
                    .expression_decisions
                    .iter()
                    .rev()
                    .find(|d| same(&d.before.note, n));
                let candidate = decision.map_or(Some(*n), |d| d.after);
                let found = candidate.and_then(|a| unique_actual(score, &a));
                let actual_match_count =
                    candidate.map_or(0, |a| score.notes.iter().filter(|n| same(&a, n)).count());
                let before = coordinate(perf, world, n.start_beat);
                let after = found.map(|(_, a)| coordinate(perf, world, a.start_beat));
                let phase_deviation_beats = after.as_ref().map(|a| {
                    metric_phase_deviation(
                        before.slot.index as f64 / f64::from(before.slot.subdivision),
                        a.slot.index as f64 / f64::from(a.slot.subdivision),
                    )
                });
                let nearest_drum = found.and_then(|(_, a)| {
                    score
                        .drums
                        .iter()
                        .min_by(|x, y| {
                            (x.start_beat - a.start_beat)
                                .abs()
                                .total_cmp(&(y.start_beat - a.start_beat).abs())
                        })
                        .map(|d| (d.voice, d.start_beat, a.start_beat - d.start_beat))
                });
                notes.push(PocketNote {
                    source: *n,
                    optional: !structural(perf, score, n),
                    source_coordinate: before,
                    score_index: found.map(|(i, _)| i),
                    actual_match_count,
                    performed: found.map(|(_, a)| *a),
                    performed_coordinate: after,
                    signed_displacement_beats: found.map(|(_, a)| a.start_beat - n.start_beat),
                    phase_deviation_beats,
                    source_accent: perf.accent.at_beat(n.start_beat),
                    performed_accent: found.map(|(_, a)| perf.accent.at_beat(a.start_beat)),
                    nearest_drum,
                    ledger_status: if found.is_some() {
                        "actual-note-match"
                    } else if candidate.is_none() {
                        "explicit-omission"
                    } else if actual_match_count > 1 {
                        "unresolved-ambiguous-actual-notes"
                    } else {
                        "unresolved-no-actual-note"
                    },
                });
            }
            out.phrases.push(PocketPhrase {
                plan_index,
                role: plan.role,
                phrase: plan.phrase,
                transform: plan.transform,
                destination: plan.destination,
                destination_score_index: unique_actual(score, &plan.destination).map(|(i, _)| i),
                destination_coordinate: coordinate(perf, world, plan.destination.start_beat),
                notes,
                source_timeline: timeline(source_events, lo, hi),
                performed_timeline: timeline(actual_events, lo, hi),
            });
        }
        for (li, lead) in score
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.role == Role::Lead)
        {
            for (bi, bass) in score
                .notes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.role == Role::Bass)
            {
                for action in lead
                    .prov
                    .actions
                    .iter()
                    .filter(|a| bass.prov.actions.has(*a))
                {
                    out.shared_actions.push(SharedActionTiming {
                        action,
                        lead_score_index: li,
                        bass_score_index: bi,
                        source_lead_minus_bass: source_for(score, lead).start_beat
                            - source_for(score, bass).start_beat,
                        performed_lead_minus_bass: lead.start_beat - bass.start_beat,
                    });
                }
            }
        }
        let mut groups: Vec<PhaseMorphology> = Vec::new();
        for phrase in &out.phrases {
            for n in &phrase.notes {
                let (Some(a), Some(displacement)) =
                    (&n.performed_coordinate, n.signed_displacement_beats)
                else {
                    continue;
                };
                if displacement.abs() < 1e-9 {
                    continue;
                }
                let direction = if displacement < 0.0 { "early" } else { "late" };
                let s = &n.source_coordinate.slot;
                let source_phase = (s.index.rem_euclid(i64::from(s.subdivision)), s.subdivision);
                let performed_phase = (
                    a.slot.index.rem_euclid(i64::from(a.slot.subdivision)),
                    a.slot.subdivision,
                );
                let occurrence = (phrase.plan_index, n.source.start_beat, displacement);
                if let Some(group) = groups.iter_mut().find(|g| {
                    g.role == phrase.role
                        && g.direction == direction
                        && g.source_phase == source_phase
                        && g.performed_phase == performed_phase
                }) {
                    if !group
                        .occurrences
                        .iter()
                        .any(|(_, source, _)| *source == n.source.start_beat)
                    {
                        group.occurrences.push(occurrence);
                    }
                } else {
                    groups.push(PhaseMorphology {
                        role: phrase.role,
                        direction,
                        source_phase,
                        performed_phase,
                        occurrences: vec![occurrence],
                    });
                }
            }
        }
        for role in [Role::Lead, Role::Bass] {
            let mut events: Vec<_> = groups
                .iter()
                .filter(|g| g.role == role)
                .flat_map(|g| g.occurrences.iter().map(move |o| (g, *o)))
                .collect();
            events.sort_by(|(_, a), (_, b)| a.1.total_cmp(&b.1));
            let mut runs: Vec<PhaseMorphology> = Vec::new();
            for (group, occurrence) in events {
                if let Some(last) = runs.last_mut().filter(|r| {
                    r.direction == group.direction
                        && r.source_phase == group.source_phase
                        && r.performed_phase == group.performed_phase
                }) {
                    last.occurrences.push(occurrence);
                } else {
                    runs.push(PhaseMorphology {
                        role,
                        direction: group.direction,
                        source_phase: group.source_phase,
                        performed_phase: group.performed_phase,
                        occurrences: vec![occurrence],
                    });
                }
            }
            out.consecutive_phase_morphologies
                .extend(runs.into_iter().filter(|g| g.occurrences.len() > 1));
        }
        out.repeated_phase_morphologies = groups
            .into_iter()
            .filter(|g| g.occurrences.len() > 1)
            .collect();
        out
    }

    pub fn report(&self) -> String {
        let mut s=String::from("Pocket timing observations: independent columns, no scalar quality verdict.\n\
            Positive displacement=late versus source; negative=early. Lattice residual is onset-minus-nearest-slot.\n\
            Metric phase uses unswung pulse coordinates; whole-beat displacement remains a separate field.\n\
            Structural bits come from source ledger or authored annotation; missing actual matches stay unresolved.\n\
            Event matching retains exact note data and source identity; later chart-description provenance enrichment is ignored.\n\
            AccentGrid weights are raw nearest grid samples; nearest drums are actual attacks including authored jitter.\n\
            Boundary: shared implementation provenance; human pocket acceptance Not Established.\n");
        for phrase in &self.phrases {
            writeln!(
                s,
                "phrase_plan={} role={:?} phrase={:?} transform={:?}",
                phrase.plan_index, phrase.role, phrase.phrase, phrase.transform
            )
            .unwrap();
            writeln!(
                s,
                "destination={:?} score_index={:?} coordinate={:?}",
                phrase.destination, phrase.destination_score_index, phrase.destination_coordinate
            )
            .unwrap();
            for n in &phrase.notes {
                writeln!(s, "  {n:?}").unwrap();
            }
            writeln!(
                s,
                "  source_timeline={:?}\n  performed_timeline={:?}",
                phrase.source_timeline, phrase.performed_timeline
            )
            .unwrap();
        }
        for row in &self.shared_actions {
            writeln!(s, "shared_action={row:?}").unwrap();
        }
        for row in &self.repeated_phase_morphologies {
            writeln!(s, "repeated_phase={row:?}").unwrap();
        }
        for row in &self.consecutive_phase_morphologies {
            writeln!(s, "consecutive_transformed_phase={row:?}").unwrap();
        }
        s
    }
}
