//! `F_skin : 𝒮 → 𝓜` — the (lax-monoidal) functor that turns a semantic trace into a
//! [`Score`], parameterized by a [`MusicWorld`]. This is where simultaneous semantic layers
//! (tone + emphasis + density + elevation) become simultaneous musical voices (harmony +
//! bass + drums + melody) via a shared coherence map (the form's energy/tension curve and
//! the one progression), rather than independent sums.
//!
//! Walking the trace, each semantic morphism maps to a sequence of intent morphisms that
//! evolve a running [`MusicIntent`]; those choices drive motif development, SFX placement
//! and local density, and their labels become event provenance. Skins are natural
//! transformations: swap the world and the form/motif/resolutions stay; the dialect changes.

use super::action::Agent;
use super::contract::{CoherenceContract, CompositionGrammar};
use super::form::{Section, BEATS_PER_BAR};
use super::intent::{IntentMorphism, MusicIntent};
use super::performance::{PerformanceOptions, PerformancePlan};
use super::plan::{ArrangementRole, CompositionPlan};
use super::score::{Provenance, Score, SfxEvent, SfxKind};
use super::semantic::{EventKind, SemanticTrace, Tone};
use super::timeline::IntentTimeline;
use super::world::MusicWorld;

/// Compose a full score for `trace` under `world`, deterministic in `seed`.
pub fn compose(trace: &SemanticTrace, world: &MusicWorld, seed: u64) -> Score {
    compose_with_plan(trace, world, seed).0
}

/// Everything one composition produced: the realized [`Score`], the [`CompositionPlan`] (the song)
/// and the [`PerformancePlan`] (the shared performance every instrument realized a projection of).
pub struct Composition {
    pub score: Score,
    pub plan: CompositionPlan,
    pub perf: PerformancePlan,
}

/// Like [`compose`], but also returns the [`CompositionPlan`] the score was realized from —
/// for structural dumps (`plan.dump()`) and coherence diagnostics.
///
/// The semantic trace becomes a causal [`IntentTimeline`]; the timeline yields one plan (contract,
/// form graph, discourse, arrangement and — for DeflectedLift — the world-independent backbone
/// timeline); the plan plus a world and a language yields one [`PerformancePlan`]; and every
/// instrument realizes its projection of that performance.
pub fn compose_with_plan(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
) -> (Score, CompositionPlan) {
    let c = compose_full(trace, world, seed, None, PerformanceOptions::default());
    (c.score, c.plan)
}

/// Like [`compose_with_plan`], but the grammar is **chosen**, not inferred — the calibration path.
/// The plan is built under [`CoherenceContract::for_grammar`], so the piece exercises that grammar's
/// `ResolutionPolicy` (Functional cadences vs a Loop's cyclic return vs a modal pedal), budgets and
/// anchors, whatever the trace shape would otherwise infer.
pub fn compose_with_grammar(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
    grammar: CompositionGrammar,
) -> (Score, CompositionPlan) {
    let c = compose_full(
        trace,
        world,
        seed,
        Some(grammar),
        PerformanceOptions::default(),
    );
    (c.score, c.plan)
}

/// The full composition path with every calibration knob: an optional forced grammar and the
/// performance options (language, actions on/off, free vs clockwork responses).
pub fn compose_full(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
    grammar: Option<CompositionGrammar>,
    opts: PerformanceOptions,
) -> Composition {
    let total_bars = ((trace.total_beats / BEATS_PER_BAR).round() as u32).max(1);
    let timeline = IntentTimeline::walk(trace);
    let plan = match grammar {
        Some(g) => CompositionPlan::build_with_contract(
            &timeline,
            total_bars,
            CoherenceContract::for_grammar(g),
        ),
        None => CompositionPlan::build(&timeline, total_bars),
    };
    let perf = PerformancePlan::build(&timeline, &plan, world, seed, opts);
    let score = realize(trace, world, seed, &plan, &perf);
    Composition { score, plan, perf }
}

/// Realize a score from a finished plan and its performance. The players are realized in
/// listening order — the lead first, then the keys (who hear the lead), the bass (who hears both)
/// and the drums (who hear the bass) — each reading the same [`PerformancePlan`]; then
/// `apply_arrangement` gates the voices.
fn realize(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
    plan: &CompositionPlan,
    perf: &PerformancePlan,
) -> Score {
    let total_beats = plan.form.total_bars as f64 * BEATS_PER_BAR;
    let mut score = Score::new(world.tempo_bpm, BEATS_PER_BAR, total_beats);
    score.sections = sections_from_plan(plan);
    score.chords = perf.chords.clone();

    let lead = super::melody::realize_lead(perf, plan);
    score.melody_repairs = lead.repairs;
    score.melody_rejudged = lead.rejudged;
    let keys = super::comp::realize_keys(perf, plan, world, &lead.notes, seed);
    let pad = super::comp::realize_pad(perf, plan, world);
    let bass = super::bass::realize_bass(perf, plan, world, &lead.notes, &keys);
    score.drums = super::groove::realize_drums(perf, plan, world, seed, &bass, &lead.notes);
    score.notes.extend(pad);
    score.notes.extend(keys);
    score.notes.extend(bass);
    score.notes.extend(lead.notes);

    // --- SFX + intent morphisms from significant semantic events. ---
    add_sfx_and_provenance(&mut score, trace, plan);
    // --- Provenance only: the stage already decided who plays and how loud. ---
    stamp_arrangement(&mut score, plan, perf);
    debug_assert!(
        orchestration_violations(perf, &score).is_empty(),
        "a realizer played somebody the stage had out"
    );
    score
}

/// Every Score event whose player the stage had OFF at its onset (neither seated nor admitted by
/// an action) — `(agent, beat)`. Empty by construction: the realizers ask the stage first. Kept as
/// an assertable invariant so a regression cannot sneak a second orchestration authority back in.
pub fn orchestration_violations(perf: &PerformancePlan, score: &Score) -> Vec<(Agent, f64)> {
    let mut v: Vec<(Agent, f64)> = score
        .notes
        .iter()
        .filter_map(|n| {
            let agent = match n.role {
                super::score::Role::Lead => Agent::Lead,
                super::score::Role::Keys => Agent::Keys,
                super::score::Role::Pad => Agent::Pad,
                super::score::Role::Bass => Agent::Bass,
            };
            (!perf.on_stage(agent, n.start_beat)).then_some((agent, n.start_beat))
        })
        .collect();
    v.extend(
        score
            .drums
            .iter()
            // Micro-timing can nudge a stroke a few ms before its bar line.
            .filter(|d| !perf.on_stage(Agent::Drums, d.start_beat + 0.01))
            .map(|d| (Agent::Drums, d.start_beat)),
    );
    v
}

/// Project the legacy [`Section`] list (for the Score IR and `Score::summary`) FROM the plan.
/// There is no independent `Form::from_trace` on the musical path any more: the summary and the
/// per-event provenance are two views of the *same* plan-derived decomposition.
fn sections_from_plan(plan: &CompositionPlan) -> Vec<Section> {
    plan.form
        .phrases
        .iter()
        .map(|p| Section {
            kind: p.family.to_section_kind(),
            start_bar: p.start_bar,
            bars: p.bars,
            energy: p.span.peak_energy.energy,
            tension: p.span.peak_tension.tension,
            density: p.intent.density,
        })
        .collect()
}

/// Stamp every event's phrase / family / role / closure provenance from the plan, and its
/// arrangement role from the STAGE seat it played in. Round VII's `apply_arrangement` also
/// deleted the notes of voices a phrase role had silenced and rescaled the rest — a second
/// orchestration authority acting after the performance was realized (it silently deleted a
/// planned intro fill and a coda pullback and let the witness count the silence). The stage now
/// decides before anybody plays; this pass only writes provenance.
fn stamp_arrangement(score: &mut Score, plan: &CompositionPlan, perf: &PerformancePlan) {
    let form = &plan.form;
    // The discourse debt an event helps settle: the obligation whose settlement witness is an
    // action the event performs.
    let owed: Vec<(super::ids::ActionId, super::ids::ObligationId)> = perf
        .obligations
        .obligations
        .iter()
        .filter_map(|o| Some((o.settlement?.witness?, o.id)))
        .collect();
    let stamp = |prov: &mut Provenance, beat: f64, agent: Option<Agent>| {
        if prov.obligation.is_none() {
            prov.obligation = owed
                .iter()
                .find(|(a, _)| prov.actions.has(*a))
                .map(|&(_, o)| o);
        }
        let phrase = *form.phrase_at(beat);
        prov.section = phrase.family.to_section_kind();
        prov.phrase = Some(phrase.ix);
        prov.family = Some(phrase.family.label());
        if let Some(a) = agent {
            let seat = perf.stage.seat(a, beat);
            prov.role_kind = Some(if seat.on {
                seat.role.label()
            } else {
                ArrangementRole::Punctuation.label()
            });
        }
        let goal = plan.discourse.goal(phrase.ix as usize);
        prov.role = Some(goal.role.label());
        prov.closure = Some(goal.closure.label());
    };
    for n in &mut score.notes {
        let agent = match n.role {
            super::score::Role::Lead => Agent::Lead,
            super::score::Role::Keys => Agent::Keys,
            super::score::Role::Pad => Agent::Pad,
            super::score::Role::Bass => Agent::Bass,
        };
        stamp(&mut n.prov, n.start_beat, Some(agent));
    }
    for d in &mut score.drums {
        stamp(&mut d.prov, d.start_beat, Some(Agent::Drums));
    }
    // SFX are punctuation tied to semantic beats: provenance only.
    for e in &mut score.sfx {
        stamp(&mut e.prov, e.start_beat, None);
    }
}

fn add_sfx_and_provenance(score: &mut Score, trace: &SemanticTrace, plan: &CompositionPlan) {
    let mut prev = trace.events.first().map(|e| e.state);
    for ev in &trace.events {
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
        let sec_kind = plan.form.phrase_at(ev.at_beat).family.to_section_kind();
        score.sfx.push(SfxEvent {
            start_beat: ev.at_beat,
            kind,
            velocity: ev.state.dynamic(),
            prov: Provenance {
                section: sec_kind,
                role_note: "sfx",
                ..Provenance::new(sec_kind)
            },
        });
    }
}

/// The intent-morphism gesture a semantic event maps to (used for provenance + testing the
/// functor's action on morphisms; the concrete note choices above are its realization).
pub fn event_to_morphisms(kind: EventKind, state_tone: Tone) -> Vec<IntentMorphism> {
    use IntentMorphism::*;
    match kind {
        EventKind::Prolong => vec![Prolong],
        EventKind::FocusAcquired => vec![Intensify, FragmentMotif],
        EventKind::ToneShift => vec![Reharmonize],
        EventKind::ModalEntered => vec![Suspend, ThickenTexture],
        EventKind::Impact => vec![Intensify, Syncopate, Modulate],
        EventKind::Confirmation => vec![Resolve, Cadence],
        EventKind::SectionResolved => vec![Relax, Cadence],
        EventKind::ActChanged => match state_tone {
            Tone::Danger => vec![Intensify, Modulate],
            _ => vec![Prepare],
        },
    }
}

/// The endpoint intent of the causal walk — delegates to [`super::timeline::IntentTimeline`],
/// the single source of truth for intent evolution. Kept for the category-law tests and any
/// caller that only wants the final settled intent rather than the whole timeline.
pub fn walk_intent(trace: &SemanticTrace) -> MusicIntent {
    super::timeline::IntentTimeline::walk(trace).final_intent
}

#[cfg(test)]
mod tests {
    use super::super::score::{PitchFunction, Role};
    use super::super::semantic::demo_trace;
    use super::super::theory::pitch_class;
    use super::*;

    #[test]
    fn the_realizer_leaves_no_unjustified_lead_notes() {
        // The jazz principle enforced: after the justify-or-snap repair, every lead note either is a
        // chord tone or carries a concrete non-chord justification (approach / passing / neighbour /
        // suspension / anticipation / appoggiatura). None is left `None` — no unjustified "wrong
        // notes". (Before the repair, BLACK_ICE alone left 7 of 36 lead notes unexplained.)
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let score = compose(&trace, &world, 2112);
            let unjustified = score
                .notes
                .iter()
                .filter(|n| n.role == Role::Lead && n.function.is_none())
                .count();
            assert_eq!(
                unjustified, 0,
                "{}: {} lead notes have no pitch justification",
                world.name, unjustified
            );
        }
    }

    #[test]
    fn every_bass_note_carries_a_justified_function() {
        // Bass used to leave `function = None` on every note, invisible to diagnostics. Now every
        // bass note is typed, and the line actually approaches or walks into a chord change — which
        // also proves the formerly-dead SlidePath / ChromaticApproach variants are emitted.
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let score = compose(&trace, &world, 2112);
            for n in score.notes.iter().filter(|n| n.role == Role::Bass) {
                assert!(
                    n.function.is_some(),
                    "{}: a bass note carries no PitchFunction",
                    world.name
                );
            }
            let approaches = score.notes.iter().any(|n| {
                n.role == Role::Bass
                    && matches!(
                        n.function,
                        Some(PitchFunction::SlidePath) | Some(PitchFunction::ChromaticApproach)
                    )
            });
            assert!(
                approaches,
                "{}: the bass never approaches or walks into a chord change",
                world.name
            );
        }
    }

    #[test]
    fn a_bass_chord_tone_is_really_a_chord_tone() {
        // The old code played `root_pc + 7` as the "fifth" even on a diminished chord, whose real
        // fifth is a semitone lower — a non-chord tone (a wrong note) no diagnostic ever saw. Every
        // bass note typed as a ChordTone must actually belong to the chord sounding beneath it.
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let score = compose(&trace, &world, 2112);
            for n in score.notes.iter().filter(|n| n.role == Role::Bass) {
                if n.function != Some(PitchFunction::ChordTone) {
                    continue; // approach / slide are justified by their path, not the local chord
                }
                let chord = score
                    .chords
                    .iter()
                    .filter(|c| c.start_beat <= n.start_beat + 1e-6)
                    .max_by(|a, b| a.start_beat.total_cmp(&b.start_beat))
                    .map(|c| c.chord);
                assert!(
                    chord.is_some_and(|c| c.contains_pc(pitch_class(n.pitch))),
                    "{}: bass ChordTone {} is not a tone of its sounding chord",
                    world.name,
                    n.pitch
                );
            }
        }
    }

    #[test]
    fn deflected_lift_recurs_the_hook_with_a_clean_ledger() {
        use super::super::contract::CompositionGrammar;
        use super::super::diagnostics::{DiscourseDiagnostics, RealizationDiagnostics};
        use super::super::discourse::DiscourseRole;
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let (score, plan) =
                compose_with_grammar(&trace, &world, 2112, CompositionGrammar::DeflectedLift);
            score
                .validate()
                .unwrap_or_else(|e| panic!("{}: {e}", world.name));
            // The hook recurs — the bittersweet bounce, not one cinematic climax.
            let hooks = plan
                .discourse
                .goals
                .iter()
                .filter(|g| g.role == DiscourseRole::Culminate)
                .count();
            assert!(
                hooks >= 2,
                "{}: the hook did not recur ({hooks} culminations)",
                world.name
            );
            // Each cycle settles the debt it opens — no cinematic obligation left hanging.
            assert_eq!(
                DiscourseDiagnostics::measure(&plan, &score).abandoned_obligations,
                0,
                "{}: DeflectedLift abandoned an obligation",
                world.name
            );
            // The jazz principle still holds across every audible role under the new backbone.
            assert_eq!(
                RealizationDiagnostics::measure(&plan, &score).unjustified_nonchord_notes,
                0,
                "{}: DeflectedLift left an unjustified note",
                world.name
            );
        }
    }

    #[test]
    fn compose_produces_a_valid_multivoice_score() {
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let score = compose(&trace, &world, 2112);
            score
                .validate()
                .unwrap_or_else(|e| panic!("{}: {e}", world.name));
            // All four roles present.
            for role in [Role::Pad, Role::Bass, Role::Lead, Role::Keys] {
                assert!(
                    score.role_notes(role).count() > 0,
                    "{}: role {:?} empty",
                    world.name,
                    role
                );
            }
            assert!(!score.drums.is_empty(), "{}: no drums", world.name);
            assert!(!score.sfx.is_empty(), "{}: no sfx", world.name);
        }
    }

    #[test]
    fn keys_comp_in_voiced_stabs_not_single_notes() {
        use super::super::contract::CompositionGrammar;
        use super::super::semantic::deflected_lift_trace;
        use std::collections::BTreeMap;
        // Keys are a comping voice now: at some onsets they sound a 2-3 note shell, not a lone note.
        let trace = deflected_lift_trace(120.0);
        let (score, _) = compose_with_grammar(
            &trace,
            &MusicWorld::black_ice(),
            2112,
            CompositionGrammar::DeflectedLift,
        );
        let mut by_onset: BTreeMap<u64, usize> = BTreeMap::new();
        for n in score.notes.iter().filter(|n| n.role == Role::Keys) {
            *by_onset
                .entry((n.start_beat * 1000.0).round() as u64)
                .or_default() += 1;
        }
        assert!(!by_onset.is_empty(), "no keys notes at all");
        let voiced = by_onset.values().filter(|&&c| c >= 2).count();
        assert!(
            voiced > 0,
            "keys never play a voiced stab — still a single-note arp"
        );
    }

    #[test]
    fn same_trace_gives_same_form_across_worlds() {
        // The natural-transformation invariant: form skeleton is world-independent.
        let trace = demo_trace(120.0);
        let a = compose(&trace, &MusicWorld::black_ice(), 1);
        let b = compose(&trace, &MusicWorld::vapor95(), 1);
        let c = compose(&trace, &MusicWorld::swiss_signal(), 1);
        let kinds = |s: &Score| s.sections.iter().map(|x| x.kind).collect::<Vec<_>>();
        assert_eq!(kinds(&a), kinds(&b));
        assert_eq!(kinds(&b), kinds(&c));
        // ...but the dialects differ (tempo).
        assert_ne!(a.tempo_bpm, b.tempo_bpm);
    }

    #[test]
    fn deterministic_for_seed() {
        let trace = demo_trace(120.0);
        let a = compose(&trace, &MusicWorld::black_ice(), 77);
        let b = compose(&trace, &MusicWorld::black_ice(), 77);
        assert_eq!(a.notes.len(), b.notes.len());
        assert_eq!(a.drums.len(), b.drums.len());
        for (x, y) in a.notes.iter().zip(b.notes.iter()) {
            assert_eq!(x.pitch, y.pitch);
            assert!((x.start_beat - y.start_beat).abs() < 1e-9);
        }
    }

    #[test]
    fn walk_intent_resolves_at_the_end() {
        // The demo arc ends in resolution (SectionResolved -> Relax, Cadence).
        let intent = walk_intent(&demo_trace(120.0));
        assert!(
            intent.tension < 0.5,
            "ends unresolved: tension {}",
            intent.tension
        );
        // And it developed the motif along the way (Impact -> Modulate bumps development).
        assert!(intent.motif.development > 0);
    }

    #[test]
    fn grammar_forces_the_resolution_policy_in_the_chords() {
        let trace = demo_trace(120.0);
        let world = MusicWorld::black_ice();
        let hook = compose_with_grammar(&trace, &world, 7, CompositionGrammar::HookArc).0;
        let loopy = compose_with_grammar(&trace, &world, 7, CompositionGrammar::LoopEvolution).0;
        let riff = compose_with_grammar(&trace, &world, 7, CompositionGrammar::RiffDrive).0;
        // Loop cycles home ("loop"), riff pedals ("pedal"), hook uses functional cadences (neither).
        assert!(
            loopy.chords.iter().any(|c| c.note == "loop"),
            "loop grammar never cycled home"
        );
        assert!(
            riff.chords.iter().any(|c| c.note == "pedal"),
            "riff grammar never pedalled"
        );
        assert!(
            hook.chords
                .iter()
                .all(|c| c.note != "loop" && c.note != "pedal"),
            "hook grammar leaked a non-functional cadence tag"
        );
        // The three grammars are genuinely different progressions, not one relabelled.
        assert!(
            hook.chords
                .iter()
                .map(|c| c.chord)
                .ne(loopy.chords.iter().map(|c| c.chord)),
            "hook and loop produced identical chords"
        );
    }
}
