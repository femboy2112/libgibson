//! Round XVII coordinates and admission helpers. Positions come from the pulse lattice;
//! the physical observer is a predicate and cannot manufacture an onset.
use super::expression::{
    self, ConnectiveViability, ExpressionDecision, ExpressionEvent, ExpressionStrategy,
};
use super::performance::PerformancePlan;
use super::phrase_expression::{groove_position, PhraseGrid};
use super::score::Note;
use super::world::MusicWorld;

/// Independent Round XVII factors. `stable_precursors` is a separate, bounded A/B,
/// excluded from the primary four-factor experiment and off in the listening arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PocketOptions {
    pub lattice_positions: bool,
    pub legato_connectives: bool,
    pub mono_voice: bool,
    pub support_top_voice: bool,
    pub stable_precursors: bool,
}
impl PocketOptions {
    /// Exact Round XVI control, including synthesis.
    pub const NONE: Self = Self {
        lattice_positions: false,
        legato_connectives: false,
        mono_voice: false,
        support_top_voice: false,
        stable_precursors: false,
    };
    pub(crate) fn changes_phrase(self) -> bool {
        self.lattice_positions
            || self.legato_connectives
            || self.mono_voice
            || self.stable_precursors
    }
}
impl Default for PocketOptions {
    fn default() -> Self {
        Self {
            lattice_positions: true,
            legato_connectives: true,
            mono_voice: true,
            support_top_voice: true,
            stable_precursors: false,
        }
    }
}

/// One groove position indexed from beat zero, before any destination is considered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatticeSlot {
    pub index: i64,
    pub subdivision: u32,
    pub position: f64,
    /// Transport from the straight pulse coordinate (beats).
    pub swing_phase: f64,
}
impl LatticeSlot {
    /// Metric phase within the beat, in subdivision ticks. Zero is a beat attack.
    pub fn phase(self) -> u32 {
        self.index.rem_euclid(i64::from(self.subdivision)) as u32
    }
}

/// Generate whole bars forward from the shared pulse origin, then restrict the corridor.
/// Physical latency has no role in construction.
pub fn slots(world: &MusicWorld, surface_subdivision: u32, from: f64, to: f64) -> Vec<LatticeSlot> {
    let mut divisions = vec![world.subdiv.max(1)];
    if (world.subdiv == 3 || surface_subdivision == 3) && !divisions.contains(&3) {
        divisions.push(3);
    }
    let mut positions = Vec::new();
    for division in divisions {
        let first_bar = (from / 4.0).floor() as i64;
        let last_bar = (to / 4.0).floor() as i64;
        for bar in first_bar..=last_bar {
            for tick in 0..4 * i64::from(division) {
                let index = bar * 4 * i64::from(division) + tick;
                let straight = index as f64 / f64::from(division);
                let position = groove_position(straight, world);
                if position >= from - 1e-6 && position < to - 1e-6 {
                    positions.push(LatticeSlot {
                        index,
                        subdivision: division,
                        position,
                        swing_phase: position - straight,
                    });
                }
            }
        }
    }
    positions.sort_by(|a, b| a.position.total_cmp(&b.position));
    positions.dedup_by(|a, b| (a.position - b.position).abs() < 1e-6);
    positions
}

pub fn lattice_positions(
    world: &MusicWorld,
    surface_subdivision: u32,
    from: f64,
    to: f64,
) -> Vec<f64> {
    slots(world, surface_subdivision, from, to)
        .into_iter()
        .map(|p| p.position)
        .collect()
}

/// Decomposed phrase-selection evidence. This is a lexicographic preference, not a quality
/// score. Source phase and IOI contour outrank accent preference; physics is a later veto.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotEvidence {
    pub phase_changes: usize,
    pub ioi_deviation: f64,
    pub source_displacement: f64,
    pub accent_deviation: f64,
    pub repeated_phase_change: bool,
}
fn phase(at: f64, w: &MusicWorld, surface: u32) -> f64 {
    slots(w, surface, at - 1.0, at + 1.0)
        .into_iter()
        .min_by(|a, b| (a.position - at).abs().total_cmp(&(b.position - at).abs()))
        .map_or(0.0, |s| f64::from(s.phase()) / f64::from(s.subdivision))
}

pub(crate) fn candidates(
    world: &MusicWorld,
    perf: &PerformancePlan,
    line: &[ExpressionEvent],
    start: usize,
    stop: usize,
    plans: &[super::phrase_expression::PhrasePlan],
) -> Vec<(PhraseGrid, Vec<f64>)> {
    use super::performance::AccentGrid;
    let from = line[start].note.start_beat;
    let to = line[stop].note.start_beat;
    let count = stop - start;
    let positions = slots(world, perf.language.surface_subdivision, from, to);
    if count == 0 || positions.len() < count {
        return Vec::new();
    }
    let source: Vec<_> = line[start..stop]
        .iter()
        .map(|e| e.note.start_beat)
        .collect();
    let previous = start
        .checked_sub(1)
        .map(|i| line[i].note.start_beat)
        .unwrap_or(from);
    let evidence = |candidate: &[LatticeSlot]| {
        let mut old = vec![previous];
        old.extend(&source);
        old.push(to);
        let mut new = vec![previous];
        new.extend(candidate.iter().map(|s| s.position));
        new.push(to);
        let (bar, step) = AccentGrid::step_of(from);
        let source_accent = perf.accent.at(bar, step);
        let kinetic = perf
            .ensemble
            .iter()
            .find(|e| e.bar == bar)
            .map_or(0.5, |e| e.kinetic);
        let accent_deviation = candidate
            .iter()
            .map(|s| {
                let (b, t) = AccentGrid::step_of(s.position);
                let a = perf.accent.at(b, t);
                // The shared drum pulse is authored by this same AccentGrid. Preserve its
                // structural/backbeat relation, and pickup intent more strongly in kinetic phrases.
                f64::from(
                    (a.structural - source_accent.structural).abs()
                        + (a.backbeat - source_accent.backbeat).abs()
                        + (a.pickup - source_accent.pickup).abs() * (0.5 + kinetic),
                )
            })
            .sum();
        let repeated_phase_change = plans.last().is_some_and(|p| {
            p.material == line[start].note.prov.material
                && p.source
                    .first()
                    .zip(p.performed.first())
                    .is_some_and(|(a, b)| {
                        phase(a.start_beat, world, perf.language.surface_subdivision)
                            != phase(b.start_beat, world, perf.language.surface_subdivision)
                            && candidate.first().is_some_and(|s| {
                                phase(source[0], world, perf.language.surface_subdivision)
                                    != f64::from(s.phase()) / f64::from(s.subdivision)
                            })
                    })
        });
        SlotEvidence {
            phase_changes: source
                .iter()
                .zip(candidate)
                .filter(|(a, b)| {
                    phase(**a, world, perf.language.surface_subdivision)
                        != f64::from(b.phase()) / f64::from(b.subdivision)
                })
                .count(),
            ioi_deviation: old
                .windows(2)
                .zip(new.windows(2))
                .map(|(a, b)| ((a[1] - a[0]) - (b[1] - b[0])).abs())
                .sum(),
            source_displacement: source
                .iter()
                .zip(candidate)
                .map(|(a, b)| (b.position - a).abs())
                .sum(),
            accent_deviation,
            repeated_phase_change,
        }
    };
    let mut candidates: Vec<_> = positions
        .windows(count)
        .map(|p| (evidence(p), p.to_vec()))
        .collect();
    candidates.sort_by(|(a, _), (b, _)| {
        a.phase_changes
            .cmp(&b.phase_changes)
            .then(a.ioi_deviation.total_cmp(&b.ioi_deviation))
            .then(a.source_displacement.total_cmp(&b.source_displacement))
            .then(a.accent_deviation.total_cmp(&b.accent_deviation))
            .then(a.repeated_phase_change.cmp(&b.repeated_phase_change))
    });
    candidates
        .into_iter()
        .map(|(_, p)| {
            let grid = if p[0].subdivision == 3 {
                PhraseGrid::EighthTriplet
            } else {
                PhraseGrid::Sixteenth
            };
            (grid, p.into_iter().map(|p| p.position).collect())
        })
        .collect()
}

/// A source-declared optional connective and its destination within one performed lane.
/// Different material identities do not acquire continuity merely by sharing a role.
pub(crate) fn candidate_links(
    event: &ExpressionEvent,
    target: Option<&Note>,
    enabled: bool,
) -> Vec<super::voice::VoiceContinuation> {
    if !enabled || event.structural || !expression::connective(event.note.function) {
        return Vec::new();
    }
    target
        .filter(|t| t.prov.material == event.note.prov.material)
        .and_then(|t| super::voice::VoiceContinuation::new(&event.note, t))
        .into_iter()
        .collect()
}

/// Commit only the continuations already owned by source phrase plans. Other same-role
/// figures are never inferred to be predecessors. Unison bass inherits the lead's gesture
/// as a separate lane with its own explicit endpoints.
pub(crate) fn continuations(
    notes: &[Note],
    plans: &[super::phrase_expression::PhrasePlan],
) -> Vec<super::voice::VoiceContinuation> {
    use super::{
        score::Role,
        voice::{VoiceContinuation, VoiceEventId},
    };
    let mut links = Vec::new();
    for plan in plans {
        for (i, n) in plan.performed.iter().enumerate() {
            if !expression::connective(n.function) {
                continue;
            }
            let target = plan.performed.get(i + 1).unwrap_or(&plan.destination);
            if n.prov.material != target.prov.material {
                continue;
            }
            if let Some(link) = VoiceContinuation::new(n, target) {
                if notes
                    .iter()
                    .filter(|n| VoiceEventId::of(n) == link.from)
                    .count()
                    == 1
                    && notes
                        .iter()
                        .filter(|n| VoiceEventId::of(n) == link.to)
                        .count()
                        == 1
                    && !links.contains(&link)
                {
                    links.push(link);
                }
            }
            if n.role == Role::Lead {
                let from = notes.iter().find(|b| {
                    b.role == Role::Bass
                        && b.prov.role_note == "unison"
                        && b.start_beat == n.start_beat
                });
                let to = notes.iter().find(|b| {
                    b.role == Role::Bass
                        && b.prov.role_note == "unison"
                        && b.start_beat == target.start_beat
                });
                if let Some(link) = from
                    .zip(to)
                    .filter(|(a, b)| a.prov.actions == b.prov.actions)
                    .and_then(|(a, b)| VoiceContinuation::new(a, b))
                {
                    if !links.contains(&link) {
                        links.push(link);
                    }
                }
            }
        }
    }
    links
}

/// The lattice arm has no R15 seconds fallback. An unplaceable optional event yields space,
/// subject to the same identity and surviving-neighbor obligations. Required Push/Hit notes
/// may retain their explicit short articulation at their authored onset.
pub(crate) fn fallback(
    perf: &PerformancePlan,
    world: &MusicWorld,
    mut events: Vec<ExpressionEvent>,
    support: &[Note],
    factors: PocketOptions,
    committed_links: &[super::voice::VoiceContinuation],
) -> expression::ExpressedLine {
    let source = events.clone();
    let mut decisions = Vec::new();
    let mut i = 0;
    while i < events.len() {
        let event = events[i];
        if event.structural || !expression::connective(event.note.function) {
            i += 1;
            continue;
        }
        let observe = |e: &ExpressionEvent| {
            let mut links = committed_links.to_vec();
            links.extend(candidate_links(
                e,
                events.get(i + 1).map(|e| &e.note),
                factors.mono_voice,
            ));
            expression::observe_with_voice_contract(
                perf,
                world,
                e,
                i.checked_sub(1).map(|j| &events[j].note),
                events.get(i + 1).map(|e| &e.note),
                support,
                &links,
            )
        };
        let before = observe(&event);
        if before.verdict == ConnectiveViability::AsWritten {
            i += 1;
            continue;
        }
        if super::phrase_expression::anchored(perf, &event.note) {
            let mut short = event;
            short.note.dur_beats = (0.025_f64
                .max(f64::from(expression::patch(world, event.note.role).adsr.0))
                * f64::from(world.tempo_bpm)
                / 60.0) as f32;
            let after = observe(&short);
            if after.verdict == ConnectiveViability::AsWritten {
                decisions.push(ExpressionDecision {
                    before,
                    after: Some(short.note),
                    after_observation: Some(after),
                    strategy: ExpressionStrategy::ShortPickup,
                    reason: "explicit authored Push/Hit retains its onset and short articulation",
                });
                events[i] = short;
            }
            i += 1;
            continue;
        }
        let remaining: Vec<_> = events
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, e)| *e)
            .collect();
        let neighbors_survive = remaining
            .iter()
            .enumerate()
            .filter(|(_, e)| expression::connective(e.note.function))
            .all(|(j, e)| {
                expression::valid_function(
                    perf,
                    j.checked_sub(1).map(|k| &remaining[k].note),
                    &e.note,
                    remaining.get(j + 1).map(|e| &e.note),
                )
            });
        let shared_figure_survives =
            perf.actions
                .of_kind(super::action::ActionKind::Unison)
                .all(|a| {
                    let in_figure = |e: &&ExpressionEvent| {
                        e.note.role == super::score::Role::Lead
                            && e.note.start_beat >= a.start_beat
                            && e.note.start_beat < a.end_beat()
                    };
                    source.iter().filter(in_figure).count() < 2
                        || remaining.iter().filter(in_figure).count() >= 2
                });
        if super::phrase_expression::identities_survive(&source, &remaining)
            && neighbors_survive
            && shared_figure_survives
        {
            events = remaining;
            decisions.push(ExpressionDecision { before, after: None, after_observation: None, strategy: ExpressionStrategy::Omitted, reason: "no admitted lattice/articulation candidate; optional source yields space without changing pitch" });
        } else {
            // A protected action or path is an explicit unresolved obligation, never permission
            // to manufacture an off-lattice onset or alter the accepted pitch. In particular,
            // dropping a second unison member would cause consumers to invent a fallback cell.
            decisions.push(ExpressionDecision { after_observation:Some(before.clone()), before, after:Some(event.note), strategy:ExpressionStrategy::RetainedObligation, reason:"unresolved physical obligation retained: omission would erase a path/action or trigger a replacement unison cell" });
            i += 1;
        }
    }
    expression::ExpressedLine { events, decisions }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lattice_is_forward_transported_and_never_invents_straight_32nds() {
        let mut w = MusicWorld::black_ice();
        assert_eq!(
            lattice_positions(&w, 4, 3.0, 4.0),
            vec![3.0, 3.25, 3.5, 3.75]
        );
        w.swing = 0.5;
        assert_eq!(
            lattice_positions(&w, 4, 3.0, 4.0),
            vec![3.0, 3.25, 3.625, 3.75]
        );
        w.swing = 0.0;
        assert!(!lattice_positions(&w, 4, 3.0, 4.0)
            .iter()
            .any(|p| (*p - 10.0 / 3.0).abs() < 1e-6));
        assert!(lattice_positions(&w, 3, 3.0, 4.0)
            .iter()
            .any(|p| (*p - 10.0 / 3.0).abs() < 1e-6));
    }
    #[test]
    fn triplet_and_sixteenth_phase_indices_are_not_conflated() {
        let w = MusicWorld::black_ice();
        assert_eq!(phase(1.0 / 3.0, &w, 3), 1.0 / 3.0);
        assert_eq!(phase(0.25, &w, 3), 0.25);
        assert_ne!(phase(1.0 / 3.0, &w, 3), phase(0.25, &w, 3));
    }

    #[test]
    fn two_physically_viable_slots_do_not_have_equal_phrase_authority() {
        use super::super::{
            composer::Composer, functor::perform_coherent, performance::PerformanceOptions,
            score::Role, semantic::deflected_lift_trace, SongMap,
        };
        let w = MusicWorld::swiss_signal();
        let song = SongMap::compose(
            &deflected_lift_trace(120.0),
            2112,
            None,
            Composer::StablePropulsion,
        );
        let c = perform_coherent(&song, &w, PerformanceOptions::default());
        let ns: Vec<_> = c.score.role_notes(Role::Lead).copied().collect();
        let line = expression::annotate(&c.perf, &ns);
        let start = line.iter().position(|e| e.note.start_beat == 24.5).unwrap();
        let stop = start + 1;
        assert_eq!(line[stop].note.start_beat, 25.5);
        let proposals = candidates(&w, &c.perf, &line, start, stop, &[]);
        // Target-nearest ordering would prefer 25.25; phrase phase/IOI preserves 24.5.
        assert_eq!(proposals[0].1, vec![24.5]);
        for onset in [24.5, 25.0] {
            assert!(proposals.iter().any(|(_, p)| p == &vec![onset]));
            let mut e = line[start];
            e.note.start_beat = onset;
            e.note.dur_beats = ((25.5 - onset) * 0.97) as f32;
            let obs = expression::observe_with_voice_contract(
                &c.perf,
                &w,
                &e,
                Some(&line[start - 1].note),
                Some(&line[stop].note),
                &[],
                &candidate_links(&e, Some(&line[stop].note), true),
            );
            assert_eq!(obs.verdict, ConnectiveViability::AsWritten, "{obs:?}");
        }
        assert_ne!(phase(24.5, &w, 4), phase(25.0, &w, 4));
    }
}
