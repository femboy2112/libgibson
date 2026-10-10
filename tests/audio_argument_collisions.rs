//! Bounded falsifiers of the LEGACY semantic witnesses, pinned to the semantic-resurrection
//! base fbdbffcee3f77dab3cef0234666192cadc7da370. These tests intentionally preserve and expose
//! the coarse model's limits; they are not acceptance tests of the optional argument compiler.
//!
//! Predeclared outcomes:
//! 1. Under a FIXED CompositionPlan, different intermediate traces with the same last pressure
//!    peak / later release yield exactly equal MeaningPlans; their IntentTimelines may differ.
//! 2. An unrelated or unbound action labelled Answer can witness a paid MotifQuestion. Missing,
//!    wrong-kind and out-of-window actions cannot. No whole-performance conformance is claimed.
//! 3. The legacy thematic classifier ignores pitch_basis, so equal numeric coordinates in two
//!    different units can be called Literal. A unit-aware source witness must distinguish them.

use gibson::audio::human_music::{
    action::{ActionCause, ActionKind, ActionPlan, Agent, EffectVector, MusicalAction},
    composer::Composer,
    contract::{CoherenceContract, CompositionGrammar},
    discourse::{
        bind_settlement_witnesses, DiscourseRole, ObligationKind, ObligationLedger,
        ObligationStatus, SettleHow,
    },
    ids::{ActionId, MaterialId},
    material::{InteractionMaterial, MaterialSource},
    meaning::{landing, story_arc, Close, Commutation, Level, MeaningPlan, ThemeRelation},
    motif::Motif,
    plan::CompositionPlan,
    semantic::{
        deflected_lift_trace, demo_trace, EventKind, SemanticEvent, SemanticState, SemanticTrace,
        Tone,
    },
    theory::{Mode, PitchBasis, Scale},
    timeline::IntentTimeline,
    SongMap,
};

fn event(at_beat: f64, tone: Tone, kind: EventKind) -> SemanticEvent {
    SemanticEvent {
        at_beat,
        state: SemanticState::toned(tone),
        kind,
    }
}

/// Same initial state, peak state/time, final confirmation and length. The middle goes toward
/// suspension in A and toward explicit early resolution in B; neither affects the shared peak.
fn contrasted_traces() -> (SemanticTrace, SemanticTrace) {
    let a = SemanticTrace::new(
        vec![
            event(0.0, Tone::Neutral, EventKind::ActChanged),
            event(16.0, Tone::Warning, EventKind::ModalEntered),
            event(32.0, Tone::Accent, EventKind::FocusAcquired),
            event(48.0, Tone::Danger, EventKind::Impact),
            event(72.0, Tone::Success, EventKind::Confirmation),
        ],
        96.0,
    );
    let b = SemanticTrace::new(
        vec![
            event(0.0, Tone::Neutral, EventKind::ActChanged),
            event(16.0, Tone::Success, EventKind::SectionResolved),
            event(32.0, Tone::Info, EventKind::ToneShift),
            event(48.0, Tone::Danger, EventKind::Impact),
            event(72.0, Tone::Success, EventKind::Confirmation),
        ],
        96.0,
    );
    (a, b)
}

fn fixed_plan(grammar: CompositionGrammar) -> CompositionPlan {
    CompositionPlan::build_with_contract_for_beats(
        &IntentTimeline::walk(&demo_trace(96.0)),
        96.0,
        CoherenceContract::for_grammar(grammar),
    )
}

#[test]
fn legacy_fixed_plan_loses_opposed_intermediate_stories() {
    let (a, b) = contrasted_traces();
    assert_ne!(a.events, b.events, "the contrast must actually alter input");
    assert_eq!(story_arc(&a), (Level::High, Close::Home));
    assert_eq!(story_arc(&a), story_arc(&b));
    let ta = IntentTimeline::walk(&a);
    let tb = IntentTimeline::walk(&b);
    assert_ne!(
        ta.intent_at(20.0),
        tb.intent_at(20.0),
        "control: the richer timeline must still distinguish this intermediate contrast"
    );
    assert_ne!(ta.transitions[1].applied, tb.transitions[1].applied);

    for grammar in [
        CompositionGrammar::HookArc,
        CompositionGrammar::DeflectedLift,
        CompositionGrammar::PropulsiveReturn,
    ] {
        let p = fixed_plan(grammar);
        let fa = MeaningPlan::target(&a, &p);
        let fb = MeaningPlan::target(&b, &p);
        assert!(!fa.events.is_empty(), "avoid a vacuous empty-target test");
        assert_eq!(fa, fb, "fixed-plan target collision for {grammar:?}");
    }
}

#[test]
fn legacy_fixed_plan_kernel_includes_event_identity_timing_and_subpeak_changes() {
    let (reference, _) = contrasted_traces();
    let p = fixed_plan(CompositionGrammar::DeflectedLift);
    let expected = MeaningPlan::target(&reference, &p);
    // Fresh finite domain: 6 event kinds × 4 intermediate tones × 3 event times = 72 inputs.
    // The peak and its later release remain fixed. Every mutation must collide at F_P only.
    let mut cases = 0;
    for kind in [
        EventKind::Prolong,
        EventKind::ToneShift,
        EventKind::FocusAcquired,
        EventKind::ModalEntered,
        EventKind::SectionResolved,
        EventKind::ActChanged,
    ] {
        for tone in [Tone::Neutral, Tone::Success, Tone::Info, Tone::Warning] {
            for at in [8.0, 20.5, 40.0] {
                let trace = SemanticTrace::new(
                    vec![
                        event(0.0, Tone::Neutral, EventKind::ActChanged),
                        event(at, tone, kind),
                        event(48.0, Tone::Danger, EventKind::Impact),
                        event(72.0, Tone::Success, EventKind::Confirmation),
                    ],
                    96.0,
                );
                assert_eq!(MeaningPlan::target(&trace, &p), expected);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 72);
}

#[test]
fn legacy_fixed_plan_is_sensitive_to_its_declared_peak_and_release_controls() {
    let (resolved, _) = contrasted_traces();
    let p = fixed_plan(CompositionGrammar::DeflectedLift);
    let mut unresolved = resolved.clone();
    unresolved.events.last_mut().unwrap().kind = EventKind::ToneShift;
    let mut lower = resolved.clone();
    lower.events[3].state.tone = Tone::Warning;
    assert_eq!(story_arc(&unresolved), (Level::High, Close::Open));
    assert_eq!(story_arc(&lower), (Level::Mid, Close::Home));
    assert_ne!(
        MeaningPlan::target(&resolved, &p),
        MeaningPlan::target(&unresolved, &p)
    );
    assert_ne!(
        MeaningPlan::target(&resolved, &p),
        MeaningPlan::target(&lower, &p)
    );

    // A confirmation between two equal maxima does not release the LAST maximum.
    let repeated = SemanticTrace::new(
        vec![
            event(0.0, Tone::Danger, EventKind::Impact),
            event(16.0, Tone::Success, EventKind::Confirmation),
            event(32.0, Tone::Danger, EventKind::Impact),
        ],
        96.0,
    );
    assert_eq!(story_arc(&repeated), (Level::High, Close::Open));
}

fn action(kind: ActionKind, start_beat: f64) -> MusicalAction {
    MusicalAction {
        id: ActionId(0),
        cause: ActionCause::Statement { phrase: 2 },
        initiator: Agent::Keys,
        start_beat,
        dur_beats: 4.0,
        kind,
        target_beat: None,
        responders: Vec::new(),
        binding: None,
        pays: None,
        effect: EffectVector::NEUTRAL,
    }
}

fn settled_question() -> ObligationLedger {
    let mut ledger = ObligationLedger::new(4);
    let debt = ledger.open_debt(ObligationKind::MotifQuestion, 0, Some(2), 1.0, false);
    ledger
        .settle(debt, 2, DiscourseRole::Answer, SettleHow::Paid)
        .unwrap();
    ledger
}

fn span(phrase: u32) -> (f64, f64) {
    (phrase as f64 * 8.0, (phrase + 1) as f64 * 8.0)
}

/// An independent interval/onset signature reads material contents rather than nominal motif
/// IDs, transforms, the legacy ThemeRelation classifier or the composer certification.
fn source_signature(m: &InteractionMaterial) -> (Vec<i32>, Vec<(u64, u64)>) {
    let pitches: Vec<_> = m.events.iter().filter_map(|e| e.step).collect();
    (
        pitches.windows(2).map(|w| w[1] - w[0]).collect(),
        m.events
            .iter()
            .map(|e| (e.onset.to_bits(), e.dur.to_bits()))
            .collect(),
    )
}

#[test]
fn legacy_generic_answer_witnesses_a_question_without_referent_or_source_relation() {
    let question = InteractionMaterial::from_motif(
        MaterialId(0),
        Agent::Lead,
        &Motif::seed_a(),
        0.0,
        MaterialSource::Statement {
            statement: 0,
            motif: 0,
        },
    );
    let unrelated = Motif {
        id: 91,
        pitch_basis: PitchBasis::ScaleSteps,
        degrees: vec![0, -1, -2, -3, -4],
        rhythm: vec![0.25, 0.25, 0.25, 0.25, 3.0],
    };
    let answer = InteractionMaterial::from_motif(
        MaterialId(1),
        Agent::Keys,
        &unrelated,
        16.0,
        MaterialSource::Statement {
            statement: 99,
            motif: unrelated.id,
        },
    );
    assert_ne!(source_signature(&question), source_signature(&answer));
    assert!(!matches!(answer.source, MaterialSource::Derived { .. }));

    let mut ledger = settled_question();
    let actions = ActionPlan {
        actions: vec![action(ActionKind::Answer, 16.0)],
        ..ActionPlan::default()
    };
    assert_eq!(
        actions.actions[0].pays, None,
        "even the action link is absent"
    );
    bind_settlement_witnesses(&mut ledger, &actions, span);
    let debt = &ledger.obligations[0];
    assert_eq!(debt.settlement.unwrap().witness, Some(ActionId(0)));
    assert_eq!(
        ledger.final_status(debt.id),
        Some(ObligationStatus::Settled(SettleHow::Paid))
    );
    assert_eq!(ledger.unwitnessed_settlements().count(), 0);
    // Scope: the ledger does not take material as input. This proves its stated-action witness
    // cannot certify material relation; it does not claim a full SongMap/Score checker passes.
}

#[test]
fn legacy_question_witness_controls_reject_absence_wrong_kind_and_wrong_window() {
    for actions in [
        ActionPlan::none(),
        ActionPlan {
            actions: vec![action(ActionKind::Resolve, 16.0)],
            ..ActionPlan::default()
        },
        ActionPlan {
            actions: vec![action(ActionKind::Answer, 24.0)],
            ..ActionPlan::default()
        },
    ] {
        let mut ledger = settled_question();
        bind_settlement_witnesses(&mut ledger, &actions, span);
        assert_eq!(ledger.obligations[0].settlement.unwrap().witness, None);
        assert_eq!(ledger.unwitnessed_settlements().count(), 1);
    }
    let mut ledger = ObligationLedger::new(4);
    let id = ledger.open_debt(ObligationKind::MotifQuestion, 1, Some(3), 1.0, false);
    assert!(ledger
        .settle(id, 0, DiscourseRole::Answer, SettleHow::Paid)
        .is_err());
}

#[test]
fn legacy_theme_observer_collides_declared_pitch_units() {
    let scale_steps = Motif::seed_a();
    let mut semitones = scale_steps.clone();
    semitones.pitch_basis = PitchBasis::Semitones;
    assert_eq!(
        ThemeRelation::of(&scale_steps, &semitones),
        ThemeRelation::Literal,
        "legacy classifier compares numbers/rhythm without their declared unit"
    );
    let scale = Scale::new(0, Mode::Ionian);
    let actual_intervals = |m: &Motif| {
        let notes: Vec<_> = m
            .degrees
            .iter()
            .map(|&d| m.pitch_at(&scale, d, 4))
            .collect();
        notes.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>()
    };
    assert_ne!(actual_intervals(&scale_steps), actual_intervals(&semitones));

    let chromatic_octave = Motif {
        id: 12,
        pitch_basis: PitchBasis::Semitones,
        degrees: vec![0, 12],
        rhythm: vec![1.0, 1.0],
    };
    assert_eq!(
        chromatic_octave.pitch_at(&scale, 12, 4).rem_euclid(12),
        0,
        "the actual chromatic endpoint is tonic"
    );
    assert_eq!(landing(&chromatic_octave), Close::Open);
}

#[test]
fn legacy_full_commutation_accepts_a_pitch_unit_mutation_of_its_flagship() {
    // The baseline is the existing meaning_probes acceptance fixture, not a newly fitted song.
    // Predeclared mutation: change only the first site's declared pitch unit. The old complete
    // observation and commutation still pass; independently realized intervals must differ.
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    );
    let control = Commutation::check(&song);
    assert!(
        control.commutes(),
        "the established control: {}",
        control.report()
    );
    let mut mutated = song.clone();
    mutated.thematic.sites[0].motif.pitch_basis = PitchBasis::Semitones;
    assert_ne!(
        song.thematic.sites[0].motif,
        mutated.thematic.sites[0].motif
    );

    let scale = Scale::new(0, Mode::Ionian);
    let pitches = |m: &Motif| {
        m.degrees
            .iter()
            .map(|&d| m.pitch_at(&scale, d, 4))
            .collect::<Vec<_>>()
    };
    let intervals = |m: &Motif| {
        pitches(m)
            .windows(2)
            .map(|w| w[1] - w[0])
            .collect::<Vec<_>>()
    };
    assert_ne!(
        intervals(&song.thematic.sites[0].motif),
        intervals(&mutated.thematic.sites[0].motif)
    );
    let collision = Commutation::check(&mutated);
    assert_eq!(control.observed, collision.observed);
    assert!(collision.commutes(), "{}", collision.report());
    // This is a full legacy F/μ certification collision. Score lowering and PCM are separate
    // boundaries: no assertion here says they coincide, or that both performances are lawful.
}
