//! Round XVI: plan a short piece of phrasing before playing it. Grid transformations belong
//! to musical time; the unchanged Round XV predicate decides whether their physical attacks
//! are viable. No finished Score is repaired, and no pitch vocabulary or synth is replaced.
use super::action::{ActionKind, Agent};
use super::discourse::DiscourseRole;
use super::expression::{
    self, ConnectiveViability, ExpressionDecision, ExpressionEvent, ExpressionStrategy,
};
use super::ids::MaterialId;
use super::performance::PerformancePlan;
use super::score::{Note, PitchFunction, Role};
use super::world::MusicWorld;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhraseTransform {
    AsWritten,
    CompressedFragment,
    GridPickup,
    MetricalPush,
    CoordinatedSpace,
    /// Optional connective yields where an authored hole divides the thought.
    PhraseSpace,
    PhysicalFallback,
}

/// Exact target-relative musical subdivisions, never a seconds-derived approximate grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhraseGrid {
    ThirtySecond,
    Sixteenth,
    EighthTriplet,
    DottedSixteenth,
}
impl PhraseGrid {
    pub fn beats(self) -> f64 {
        match self {
            Self::ThirtySecond => 0.125,
            Self::Sixteenth => 0.25,
            Self::EighthTriplet => 1.0 / 3.0,
            Self::DottedSixteenth => 0.375,
        }
    }
}

/// Bounded position within the authored thought, not a new emotion model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhrasePhase {
    Opening,
    Continuation,
    Arrival,
}

/// An inspectable plan for one target-bearing fragment inside an authored thought.
#[derive(Debug, Clone)]
pub struct PhrasePlan {
    pub role: Role,
    pub phrase: Option<u32>,
    pub material: Option<MaterialId>,
    pub thought_start: f64,
    pub thought_end: f64,
    pub phase: PhrasePhase,
    /// Authored onset intervals across the entire owning thought, before expression.
    pub thought_iois: Vec<f64>,
    /// Previous chosen transform in this authored thought (explicit repetition memory).
    pub previous_transform: Option<PhraseTransform>,
    pub source: Vec<Note>,
    pub performed: Vec<Note>,
    /// Both structural and optional stable local destinations retain their exact onset.
    pub destination: Note,
    pub transform: PhraseTransform,
    pub grid: Option<PhraseGrid>,
    pub kinetic: f32,
    pub pickup: f32,
    pub rejected_candidates: Vec<&'static str>,
    pub reason: &'static str,
}

pub struct PhraseRealization {
    pub events: Vec<ExpressionEvent>,
    pub decisions: Vec<ExpressionDecision>,
    pub plans: Vec<PhrasePlan>,
}

// Check the entire relocation corridor, not just its landing. Grid half-steps and every
// admission boundary partition the piecewise-constant stage/accent authority exactly.
pub(crate) fn clear_corridor(perf: &PerformancePlan, role: Role, from: f64, to: f64) -> bool {
    let agent = if role == Role::Bass {
        Agent::Bass
    } else {
        Agent::Lead
    };
    let mut cuts = vec![from, to];
    cuts.extend(
        ((from * 8.0).floor() as i64..=(to * 8.0).ceil() as i64)
            .map(|k| k as f64 / 8.0)
            .filter(|b| *b > from && *b < to),
    );
    cuts.extend(
        perf.stage
            .windows
            .iter()
            .filter(|w| w.agent == agent)
            .flat_map(|w| [w.start_beat, w.end_beat])
            .filter(|b| *b > from && *b < to),
    );
    cuts.sort_by(f64::total_cmp);
    cuts.iter()
        .copied()
        .chain(cuts.windows(2).map(|w| (w[0] + w[1]) * 0.5))
        .all(|b| perf.on_stage(agent, b) && !perf.accent.is_hole(b))
}

pub(crate) fn anchored(perf: &PerformancePlan, n: &Note) -> bool {
    perf.actions.actions.iter().any(|a| {
        n.prov.actions.has(a.id)
            && matches!(a.kind, ActionKind::Push | ActionKind::Hit)
            && (a.start_beat - n.start_beat).abs() < 1e-6
    })
}

pub(crate) fn identities_survive(before: &[ExpressionEvent], after: &[ExpressionEvent]) -> bool {
    before.iter().all(|e| {
        e.note
            .prov
            .actions
            .iter()
            .all(|a| after.iter().any(|n| n.note.prov.actions.has(a)))
            && e.note
                .prov
                .material
                .is_none_or(|m| after.iter().any(|n| n.note.prov.material == Some(m)))
    })
}

fn stable(n: &Note) -> bool {
    matches!(
        n.function,
        Some(PitchFunction::ChordTone | PitchFunction::LicensedExtension)
    )
}

/// Express source-owned optional details. The caller supplies already final support and the
/// lead's plans when realizing bass. This function never receives or mutates a Score.
pub fn realize(
    perf: &PerformancePlan,
    world: &MusicWorld,
    line: Vec<ExpressionEvent>,
    support: &[Note],
    lead_plans: &[PhrasePlan],
) -> PhraseRealization {
    realize_impl(perf, world, line, support, lead_plans, None)
}

/// Opt-in Round XVII source planner; historical factors-off dispatches exactly to Round XVI.
pub fn realize_pocket(
    perf: &PerformancePlan,
    world: &MusicWorld,
    line: Vec<ExpressionEvent>,
    support: &[Note],
    lead_plans: &[PhrasePlan],
    factors: super::pocket::PocketOptions,
) -> PhraseRealization {
    realize_impl(
        perf,
        world,
        line,
        support,
        lead_plans,
        factors.changes_source_admission().then_some(factors),
    )
}

fn realize_impl(
    perf: &PerformancePlan,
    world: &MusicWorld,
    line: Vec<ExpressionEvent>,
    support: &[Note],
    lead_plans: &[PhrasePlan],
    pocket: Option<super::pocket::PocketOptions>,
) -> PhraseRealization {
    let committed_links = if pocket.is_some_and(|p| p.mono_voice) {
        super::pocket::continuations(support, lead_plans)
    } else {
        Vec::new()
    };
    let observe = |perf: &PerformancePlan,
                   world: &MusicWorld,
                   event: &ExpressionEvent,
                   prev: Option<&Note>,
                   target: Option<&Note>,
                   support: &[Note]| {
        let mut links = committed_links.clone();
        links.extend(super::pocket::candidate_links(
            event,
            target,
            pocket.is_some_and(|p| p.mono_voice),
        ));
        expression::observe_with_voice_contract(perf, world, event, prev, target, support, &links)
    };
    let skeleton = expression::project(&line);
    let mut working = line.clone();
    let mut removed = vec![false; line.len()];
    let mut decisions = Vec::new();
    let mut plans: Vec<PhrasePlan> = Vec::new();
    let mut i = 0;
    while i < line.len() {
        if line[i].structural || !expression::connective(line[i].note.function) {
            i += 1;
            continue;
        }
        let first_connector = i;
        let mut stop = i + 1;
        while stop < line.len()
            && !line[stop].structural
            && expression::connective(line[stop].note.function)
            && line[stop].note.prov.material == line[i].note.prov.material
            && line[stop].note.prov.actions == line[i].note.prov.actions
            && perf
                .context_at(line[stop].note.start_beat)
                .map(|c| c.start_beat)
                == perf
                    .context_at(line[i].note.start_beat)
                    .map(|c| c.start_beat)
            && !anchored(perf, &line[stop].note)
        {
            stop += 1;
        }
        i = stop;
        let Some(destination) = line.get(stop).map(|e| e.note) else {
            continue;
        };
        let first = line[first_connector].note;
        let observation = observe(
            perf,
            world,
            &line[first_connector],
            first_connector.checked_sub(1).map(|j| &line[j].note),
            line.get(first_connector + 1).map(|n| &n.note),
            support,
        );
        let statement = perf.statements.iter().find(|s| {
            Some(s.material) == first.prov.material
                && s.start_beat <= first.start_beat
                && s.end_beat() > first.start_beat
        });
        let context = perf.context_at(first.start_beat);
        let thought_notes: Vec<_> = line
            .iter()
            .filter(|e| {
                statement.map_or_else(
                    || {
                        context.is_some_and(|c| {
                            perf.context_at(e.note.start_beat)
                                .is_some_and(|h| h.start_beat == c.start_beat)
                        })
                    },
                    |s| e.note.prov.material == Some(s.material),
                )
            })
            .map(|e| e.note)
            .collect();
        let phase = if thought_notes
            .last()
            .is_some_and(|n| (n.start_beat - destination.start_beat).abs() < 1e-6)
        {
            PhrasePhase::Arrival
        } else if thought_notes
            .iter()
            .take(3)
            .any(|n| n.start_beat == first.start_beat)
        {
            PhrasePhase::Opening
        } else {
            PhrasePhase::Continuation
        };
        let previous_transform = plans
            .iter()
            .rev()
            .find(|p| {
                p.role == first.role
                    && p.material == first.prov.material
                    && statement.is_none_or(|s| p.thought_start == s.start_beat)
            })
            .map(|p| p.transform);
        let mut plan = PhrasePlan {
            role: first.role,
            phrase: statement.map(|s| s.phrase),
            material: first.prov.material,
            thought_start: statement.map_or_else(
                || context.map_or(first.start_beat, |c| c.start_beat),
                |s| s.start_beat,
            ),
            thought_end: statement.map_or(destination.start_beat, |s| s.end_beat()),
            phase,
            thought_iois: thought_notes
                .windows(2)
                .map(|w| w[1].start_beat - w[0].start_beat)
                .collect(),
            previous_transform,
            source: line[first_connector..stop].iter().map(|e| e.note).collect(),
            performed: line[first_connector..stop].iter().map(|e| e.note).collect(),
            destination,
            transform: PhraseTransform::AsWritten,
            grid: None,
            kinetic: observation.kinetic,
            pickup: observation.pickup,
            rejected_candidates: Vec::new(),
            reason: "authored phrase already physically viable",
        };
        let all_viable = (first_connector..stop).all(|k| {
            observe(
                perf,
                world,
                &line[k],
                k.checked_sub(1).map(|j| &line[j].note),
                line.get(k + 1).map(|n| &n.note),
                support,
            )
            .verdict
                == ConnectiveViability::AsWritten
        });
        if all_viable {
            plans.push(plan);
            continue;
        }
        if !clear_corridor(perf, first.role, first.start_beat, destination.start_beat)
            && (first_connector..stop).all(|k| !anchored(perf, &line[k].note))
        {
            let remaining: Vec<_> = working
                .iter()
                .enumerate()
                .filter(|(k, _)| !removed[*k] && !(*k >= first_connector && *k < stop))
                .map(|(_, e)| *e)
                .collect();
            if identities_survive(&line, &remaining) {
                for k in first_connector..stop {
                    removed[k] = true;
                    decisions.push(ExpressionDecision {
                        before: observe(perf,world,&line[k],k.checked_sub(1).map(|j|&line[j].note),line.get(k+1).map(|e|&e.note),support),
                        after: None, after_observation: None, strategy: ExpressionStrategy::Omitted,
                        reason: "authored stage hole divides the thought; optional connector yields instead of jumping the boundary",
                    });
                }
                plan.performed.clear();
                plan.transform = PhraseTransform::PhraseSpace;
                plan.reason = "internal stage boundary owns the silence; fixed destination remains";
                plans.push(plan);
                continue;
            }
        }
        // A bass can leave the pickup to an already committed lead fragment. An actual shared
        // action is a unison obligation, not a reason to delete a participant.
        let lead_owns = first.role == Role::Bass
            && first.prov.actions.is_empty()
            && lead_plans.iter().any(|p| {
                p.transform == PhraseTransform::CompressedFragment
                    && (p.destination.start_beat - destination.start_beat).abs() < 1e-6
                    && !p.performed.is_empty()
                    && p.performed.iter().any(|n| n.start_beat >= first.start_beat)
            });
        if lead_owns && first_connector + 1 == stop {
            let after: Vec<_> = working
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != first_connector && !removed[*j])
                .map(|(_, e)| *e)
                .collect();
            if identities_survive(&line, &after) {
                removed[first_connector] = true;
                decisions.push(ExpressionDecision { before: observation, after: None, after_observation: None, strategy: ExpressionStrategy::Omitted, reason: "lead owns this destination with a planned fragment; bass leaves optional pickup space and arrives unchanged" });
                plan.performed.clear();
                plan.transform = PhraseTransform::CoordinatedSpace;
                plan.reason="committed lead fragment owns the pickup; unshared optional bass ornament yields";
                plans.push(plan);
                continue;
            }
        }
        if observation.kinetic < 0.18 {
            plan.transform = PhraseTransform::PhysicalFallback;
            plan.reason =
                "low kinetic thought leaves connective space through the unchanged source ladder";
            plans.push(plan);
            continue;
        }
        if anchored(perf, &first) {
            plan.transform = PhraseTransform::MetricalPush;
            plan.reason="stamped Push/Hit onset is a metric obligation; physical source fallback shortens its gate";
            plans.push(plan);
            continue;
        }
        // Bring an optional stable precursor into the same fragment, but never steal a stable
        // destination from another connector or cross material/action ownership.
        let mut start = first_connector;
        if first.role == Role::Lead && start > 0 && pocket.is_none_or(|p| p.stable_precursors) {
            let p = &line[start - 1];
            if !p.structural
                // A distant stable precursor is a separate authored rhythm, not fragment fodder.
                && (pocket.is_none() || first.start_beat-p.note.start_beat <= 1.0/f64::from(world.subdiv.max(1)) + 1e-6)
                && stable(&p.note)
                && !anchored(perf, &p.note)
                && p.note.prov.material == first.prov.material
                && p.note.prov.actions == first.prov.actions
                && (start < 2 || !expression::connective(line[start - 2].note.function))
                && perf.context_at(p.note.start_beat).map(|c| c.start_beat)
                    == context.map(|c| c.start_beat)
            {
                start -= 1;
            }
        }
        let last_grid = plans
            .iter()
            .rev()
            .find(|p| p.role == first.role && p.material == first.prov.material && p.grid.is_some())
            .and_then(|p| p.grid);
        let returning = statement.is_some_and(|s| {
            matches!(
                s.role,
                DiscourseRole::Return | DiscourseRole::Answer | DiscourseRole::Dissolve
            )
        });
        let triplets = world.subdiv == 3 || perf.language.surface_subdivision == 3;
        let preferred = if start < first_connector {
            PhraseGrid::Sixteenth
        } else if phase == PhrasePhase::Arrival && first.pitch >= destination.pitch
            || observation.pickup >= 0.5
        {
            PhraseGrid::DottedSixteenth
        } else if (returning || observation.kinetic < 0.65) && triplets {
            PhraseGrid::EighthTriplet
        } else {
            PhraseGrid::Sixteenth
        };
        let mut grids = vec![
            preferred,
            PhraseGrid::EighthTriplet,
            PhraseGrid::Sixteenth,
            PhraseGrid::DottedSixteenth,
            PhraseGrid::ThirtySecond,
        ];
        grids.retain(|g| *g != PhraseGrid::EighthTriplet || triplets);
        grids.dedup();
        // Repeat is musically legal, but a second disconnected ornament in the same thought
        // first tries a complementary subdivision. No seed jitter or canned pitch lick.
        if start == first_connector && last_grid == Some(preferred) {
            grids.retain(|g| *g != preferred);
            grids.push(preferred);
        }
        let mut accepted = None;
        let positions: Vec<_> = if pocket.is_some_and(|p| p.lattice_positions) {
            super::pocket::candidates(world, perf, &line, start, stop, &plans)
        } else {
            grids
                .into_iter()
                .map(|grid| {
                    (
                        grid,
                        (start..stop)
                            .map(|k| {
                                groove_position(
                                    destination.start_beat - (stop - k) as f64 * grid.beats(),
                                    world,
                                )
                            })
                            .collect(),
                    )
                })
                .collect()
        };
        for (grid, positions) in positions {
            let step = grid.beats();
            let mut candidate = working.clone();
            for (k, event) in candidate.iter_mut().enumerate().take(stop).skip(start) {
                let n = &mut event.note;
                n.start_beat = positions[k - start];
                // Meter supplies the gate; frozen envelope viability determines admissibility.
                // Stable precursor notes get more body than the chromatic continuation.
                n.dur_beats = (step
                    * if stable(n) {
                        0.72
                    } else if grid == PhraseGrid::DottedSixteenth {
                        0.375
                    } else {
                        0.40
                    }) as f32;
                let rise = (k - start + 1) as f32 / (stop - start) as f32;
                n.velocity *= 0.76 + 0.12 * rise;
                if pocket.is_some_and(|p| p.legato_connectives) {
                    let target = positions
                        .get(k - start + 1)
                        .copied()
                        .unwrap_or(destination.start_beat);
                    n.dur_beats = ((target - n.start_beat) * 0.97) as f32;
                }
            }
            let valid = (start..stop).all(|k| {
                let old = line[k].note;
                let n = candidate[k].note;
                let agent = if n.role == Role::Bass {
                    Agent::Bass
                } else {
                    Agent::Lead
                };
                (!anchored(perf, &old) || (n.start_beat - old.start_beat).abs() < 1e-6)
                    && (pocket.is_none()
                        || !stable(&old)
                        || n.start_beat - old.start_beat
                            <= 1.0 / f64::from(world.subdiv.max(1)) + 1e-6)
                    && n.start_beat >= old.start_beat
                    && n.start_beat < destination.start_beat
                    && (k == 0 || candidate[k - 1].note.start_beat < n.start_beat)
                    && perf.context_at(old.start_beat).map(|c| c.start_beat)
                        == perf.context_at(n.start_beat).map(|c| c.start_beat)
                    && perf.on_stage(agent, n.start_beat)
                    && !perf.accent.is_hole(n.start_beat)
                    && clear_corridor(perf, n.role, old.start_beat, n.start_beat)
                    && expression::valid_function(
                        perf,
                        k.checked_sub(1).map(|j| &candidate[j].note),
                        &n,
                        candidate.get(k + 1).map(|e| &e.note),
                    )
                    && (!expression::connective(n.function)
                        || observe(
                            perf,
                            world,
                            &candidate[k],
                            k.checked_sub(1).map(|j| &candidate[j].note),
                            candidate.get(k + 1).map(|e| &e.note),
                            support,
                        )
                        .verdict
                            == ConnectiveViability::AsWritten)
            });
            if valid {
                accepted = Some((grid, candidate));
                break;
            }
            plan.rejected_candidates.push("grid candidate violates physical viability, harmonic/stage boundary, local path or immutable order");
        }
        if let Some((grid, candidate)) = accepted {
            plan.source = line[start..stop].iter().map(|e| e.note).collect();
            plan.performed = candidate[start..stop].iter().map(|e| e.note).collect();
            plan.transform = if stop - start > 1 {
                PhraseTransform::CompressedFragment
            } else {
                PhraseTransform::GridPickup
            };
            plan.grid = Some(grid);
            plan.reason = if stop - start > 1 {
                "optional stable precursor and connector form one metrical fragment into fixed destination"
            } else {
                "phrase role, pickup accent and previous subdivision choose a metrical pickup"
            };
            for k in start..stop {
                let before = observe(
                    perf,
                    world,
                    &line[k],
                    k.checked_sub(1).map(|j| &line[j].note),
                    line.get(k + 1).map(|e| &e.note),
                    support,
                );
                let after = observe(
                    perf,
                    world,
                    &candidate[k],
                    k.checked_sub(1).map(|j| &candidate[j].note),
                    candidate.get(k + 1).map(|e| &e.note),
                    support,
                );
                decisions.push(ExpressionDecision {
                    before,
                    after: Some(candidate[k].note),
                    after_observation: Some(after),
                    strategy: if stop - start > 1 {
                        ExpressionStrategy::Burst
                    } else {
                        ExpressionStrategy::Grace
                    },
                    reason: plan.reason,
                });
                working[k] = candidate[k];
            }
        } else {
            plan.transform = PhraseTransform::PhysicalFallback;
            plan.reason = if pocket.is_some() {
                "no admitted pocket candidate; source yields space or preserves a protected authored obligation"
            } else {
                "no admissible metrical candidate; unchanged Round XV source ladder decides articulation or space"
            };
        }
        plans.push(plan);
    }
    // This is still the responsible source realizer. It receives only candidates whose new
    // phrase plan failed; accepted grid/fragment connectives already satisfy its unchanged ruler.
    let prepared: Vec<_> = working
        .into_iter()
        .enumerate()
        .filter(|(j, _)| !removed[*j])
        .map(|(_, e)| e)
        .collect();
    let mut fallback = if let Some(factors) = pocket {
        super::pocket::fallback(perf, world, prepared, support, factors, &committed_links)
    } else {
        expression::realize(perf, world, prepared, support)
    };
    for plan in plans.iter_mut().filter(|p| {
        matches!(
            p.transform,
            PhraseTransform::PhysicalFallback | PhraseTransform::MetricalPush
        )
    }) {
        plan.performed = plan
            .source
            .iter()
            .filter_map(|source| {
                fallback
                    .decisions
                    .iter()
                    .find(|d| {
                        d.before.note.role == source.role
                            && (d.before.note.start_beat - source.start_beat).abs() < 1e-6
                            && d.before.note.pitch == source.pitch
                    })
                    .map_or(Some(*source), |d| d.after)
            })
            .collect();
    }
    decisions.extend(fallback.decisions);
    // Omitting an optional note changes the surviving neighbors. Re-judge the actual source
    // result before any player hears it; a formerly valid approach cannot survive as B -> B.
    // Stable pitch membership is a truthful substitute justification, not a moved target.
    let mut at = 0;
    while at < fallback.events.len() {
        let event = fallback.events[at];
        if !expression::connective(event.note.function) {
            at += 1;
            continue;
        }
        let prev = at.checked_sub(1).map(|j| &fallback.events[j].note);
        let next = fallback.events.get(at + 1).map(|e| &e.note);
        if expression::valid_function(perf, prev, &event.note, next) {
            at += 1;
            continue;
        }
        if event.structural {
            // No structural relabel is authorized. Reject the destructive fallback decision
            // that erased its original neighbor, retaining that source obligation.
            if let Some(index) = line.iter().position(|e| {
                e.note.start_beat == event.note.start_beat && e.note.pitch == event.note.pitch
            }) {
                if let Some(required) = line.get(index + 1) {
                    if !fallback.events.iter().any(|e| {
                        e.note.start_beat == required.note.start_beat
                            && e.note.pitch == required.note.pitch
                    }) {
                        fallback.events.insert(at + 1, *required);
                        decisions.retain(|d| {
                            !(d.after.is_none()
                                && d.before.note.start_beat == required.note.start_beat
                                && d.before.note.role == required.note.role)
                        });
                        at += 1;
                        continue;
                    }
                }
            }
            at += 1;
            continue;
        }
        let before = observe(perf, world, &event, prev, next, support);
        let stable_function = [PitchFunction::ChordTone, PitchFunction::LicensedExtension]
            .into_iter()
            .find(|f| {
                let mut n = event.note;
                n.function = Some(*f);
                expression::valid_function(perf, prev, &n, next)
            });
        if let Some(function) = stable_function {
            fallback.events[at].note.function = Some(function);
            let after = fallback.events[at];
            decisions.push(ExpressionDecision {
                before,after:Some(after.note),after_observation:Some(observe(perf,world,&after,at.checked_sub(1).map(|j|&fallback.events[j].note),fallback.events.get(at+1).map(|e|&e.note),support)),
                strategy:ExpressionStrategy::Substituted,
                reason:"source omission changed local path; same pitch is now an actual stable chord/extension continuation, not an approach",
            });
            at += 1;
        } else {
            let remaining: Vec<_> = fallback
                .events
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != at)
                .map(|(_, e)| *e)
                .collect();
            let protects_neighbors = remaining
                .iter()
                .enumerate()
                .filter(|(_, e)| e.structural && expression::connective(e.note.function))
                .all(|(j, e)| {
                    expression::valid_function(
                        perf,
                        j.checked_sub(1).map(|k| &remaining[k].note),
                        &e.note,
                        remaining.get(j + 1).map(|e| &e.note),
                    )
                });
            if !anchored(perf, &event.note)
                && identities_survive(&line, &remaining)
                && protects_neighbors
            {
                fallback.events = remaining;
                decisions.push(ExpressionDecision {before,after:None,after_observation:None,strategy:ExpressionStrategy::Omitted,reason:"source fallback changed neighboring path; optional unsupported connector yields to space"});
                at = at.saturating_sub(1);
            } else {
                at += 1;
            }
        }
    }
    // Plans describe the final emitted source, including any fallback reclassification/space.
    for plan in &mut plans {
        plan.performed = plan
            .performed
            .iter()
            .filter_map(|n| {
                fallback
                    .events
                    .iter()
                    .find(|e| {
                        e.note.role == n.role
                            && e.note.pitch == n.pitch
                            && (e.note.start_beat - n.start_beat).abs() < 1e-6
                    })
                    .map(|e| e.note)
            })
            .collect();
    }
    for decision in &decisions {
        if let Some(after) = decision.after {
            if after.start_beat != decision.before.note.start_beat
                && fallback.events.iter().any(|e| {
                    e.note.role == after.role
                        && e.note.pitch == after.pitch
                        && e.note.start_beat == after.start_beat
                })
            {
                assert!(
                    clear_corridor(
                        perf,
                        after.role,
                        decision.before.note.start_beat,
                        after.start_beat
                    ),
                    "phrase relocation crossed a stage boundary"
                );
            }
        }
    }
    assert_eq!(
        skeleton,
        expression::project(&fallback.events),
        "phrase expression moved the structural skeleton"
    );
    assert!(
        identities_survive(&line, &fallback.events),
        "phrase expression lost material/action identity"
    );
    PhraseRealization {
        events: fallback.events,
        decisions,
        plans,
    }
}

/// Transport the offbeat into the same swing pocket as `groove::realize_drums`.
/// Straight worlds are exact identity; fixed destinations are never transported here.
pub fn groove_position(beat: f64, world: &MusicWorld) -> f64 {
    let eighth = (beat * 2.0).round() as i64;
    if world.swing > 0.0 && (beat * 2.0 - eighth as f64).abs() < 1e-6 && eighth.rem_euclid(2) == 1 {
        beat + f64::from(world.swing) * 0.25
    } else {
        beat
    }
}
