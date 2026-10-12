//! Frozen R0 observations at 29527a89. These tests distinguish the demonstrably
//! stale harmonic settlement from fields that do not consume chord pitch values.
use gibson::audio::human_music::{
    action::{ActionCause, ActionKind, Agent},
    argument::{prepare_performance, ArgumentFamily, CompiledArgument, MusicalArgument},
    composer::Composer,
    contract::CompositionGrammar,
    functor::perform_candidate,
    ids::MaterialId,
    interaction::Transform,
    material::{
        transform_material, InteractionMaterial, MaterialEvent, MaterialSource, PitchBasis,
    },
    performance::{PerformanceOptions, PerformancePlan},
    policy::PerformanceProfile,
    semantic::deflected_lift_trace,
    theory::Quality,
    MusicWorld, SongMap,
};

fn argument(family: ArgumentFamily) -> CompiledArgument {
    let song = SongMap::compose(
        &deflected_lift_trace(128.0),
        2112,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    );
    MusicalArgument::calibration(family, 91)
        .unwrap()
        .compile(&song)
        .unwrap()
}

fn plain_plan(argument: &CompiledArgument) -> PerformancePlan {
    PerformancePlan::from_song(
        &argument.song,
        &MusicWorld::black_ice(),
        PerformanceOptions {
            actions: false,
            ..PerformanceOptions::default()
        },
    )
}

#[test]
fn legacy_source_harmony_invalidates_a_previously_planned_home_arrival() {
    // Prediction: the pitch-sensitive settlement planner chooses the first HOME
    // chord before source harmony is installed. Late installation keeps that
    // action although its target is now a different chord.
    for (family, target) in [
        (ArgumentFamily::CallAndEarnedAnswer, 120.0),
        (ArgumentFamily::PromiseDeniedReturn, 104.0),
    ] {
        let source = argument(family);
        let world = MusicWorld::black_ice();
        let mut perf = perform_candidate(
            &source.song,
            &world,
            PerformanceOptions::default(),
            PerformanceProfile::BAND,
        )
        .unwrap()
        .perf;
        let settlement = perf
            .actions
            .of_kind(ActionKind::Resolve)
            .find(|a| {
                a.target_beat == Some(target) && matches!(a.cause, ActionCause::Discourse { .. })
            })
            .expect("fixture must exercise the pitch-sensitive settlement planner")
            .clone();
        let ActionCause::Discourse { obligation, .. } = settlement.cause else {
            unreachable!("selected discourse action")
        };
        let debt = perf
            .obligations
            .obligations
            .iter()
            .find(|d| d.id == obligation)
            .copied()
            .unwrap();
        assert_eq!(
            perf.context_at(target).unwrap().chord.root_pc,
            world.tonic_pc
        );
        prepare_performance(&source, &mut perf, &world).unwrap();
        assert_eq!(perf.actions.get(settlement.id), Some(&settlement));
        assert!(perf.obligations.obligations.contains(&debt));
        assert_eq!(perf.context_at(target).unwrap().chord.root_pc, 5);
        assert_ne!(
            perf.context_at(target).unwrap().chord.root_pc,
            world.tonic_pc
        );
    }
}

#[test]
fn root_mutation_does_not_establish_stale_accent_ensemble_or_budget() {
    // Counterprediction to the broad diagnosis: these fields do not read chord
    // pitch values, and a same-boundary root mutation does not invalidate them.
    let mut source = argument(ArgumentFamily::CallAndEarnedAnswer);
    let world = MusicWorld::black_ice();
    let mut perf = plain_plan(&source);
    prepare_performance(&source, &mut perf, &world).unwrap();
    let before = perf.clone();
    source.route[0] = (7, Quality::Dom7);
    prepare_performance(&source, &mut perf, &world).unwrap();
    assert_ne!(
        perf.context_at(0.0).unwrap().chord,
        before.context_at(0.0).unwrap().chord
    );
    assert_eq!(perf.actions, before.actions);
    assert_eq!(perf.statements, before.statements);
    assert_eq!(perf.accent, before.accent);
    assert_eq!(perf.interactions, before.interactions);
    assert_eq!(perf.ensemble, before.ensemble);
    assert_eq!(perf.budget, before.budget);
}

#[test]
fn legacy_preparation_accepts_a_foreign_plan_owner() {
    // Prediction: the old public mutator checks tonal/vocabulary compatibility,
    // but does not check that the plan belongs to this exact source song.
    let source = argument(ArgumentFamily::CallAndEarnedAnswer);
    let foreign = argument(ArgumentFamily::PromiseDeniedReturn);
    let mut perf = plain_plan(&foreign);
    assert_ne!(perf.song_fingerprint, source.song.fingerprint());
    let foreign_statements = perf.statements.clone();
    prepare_performance(&source, &mut perf, &MusicWorld::black_ice()).unwrap();
    assert_eq!(perf.song_fingerprint, foreign.song.fingerprint());
    assert_eq!(perf.statements, foreign_statements);
}

#[test]
fn legacy_preparation_silently_fills_missing_source_route_with_tonic() {
    // Prediction: missing route bars are not refused at this seam; every absent
    // entry falls back to (0, Min), changing the source's declared route.
    let mut source = argument(ArgumentFamily::CallAndEarnedAnswer);
    let mut perf = plain_plan(&source);
    source.route.clear();
    prepare_performance(&source, &mut perf, &MusicWorld::black_ice()).unwrap();
    assert!(perf
        .chords
        .iter()
        .all(|c| c.chord.root_pc == 9 && c.chord.quality == Quality::Min));
}

fn call(steps: [i32; 3]) -> InteractionMaterial {
    InteractionMaterial {
        id: MaterialId(0),
        owner: Agent::Lead,
        source: MaterialSource::Statement {
            statement: 0,
            motif: 0,
        },
        start_beat: 0.0,
        events: steps
            .into_iter()
            .enumerate()
            .map(|(i, step)| MaterialEvent {
                onset: i as f64 * 0.5,
                dur: 0.25,
                accent: 0.75,
                step: Some(step),
            })
            .collect(),
        pitch_basis: PitchBasis::Semitones,
    }
}

#[test]
fn existing_material_transport_distinguishes_equal_count_equal_energy_calls() {
    // This is a positive instrument control, not evidence that Listen uses it.
    // The existing material algebra retains exactly the information a note-count
    // policy throws away. No parallel Cue ontology is necessary for this slice.
    let rising = call([0, 4, 7]);
    let turning = call([0, 7, 4]);
    assert_eq!(rising.events.len(), turning.events.len());
    assert_eq!(rising.length(), turning.length());
    assert_eq!(
        rising.events.iter().map(|e| e.accent).sum::<f32>(),
        turning.events.iter().map(|e| e.accent).sum::<f32>()
    );
    let answer =
        |m| transform_material(MaterialId(1), m, Transform::Invert, Agent::Keys, 2.0, 2.0).unwrap();
    let a = answer(&rising);
    let b = answer(&turning);
    assert_ne!(a.events, b.events);
    assert_eq!(
        a.source,
        MaterialSource::Derived {
            from: rising.id,
            transform: Transform::Invert
        }
    );
    let mut relabeled = rising.clone();
    relabeled.source = MaterialSource::Statement {
        statement: 9,
        motif: 8,
    };
    assert_eq!(
        answer(&relabeled).events,
        a.events,
        "irrelevant call labels cannot change the reply content"
    );
    assert!(transform_material(
        MaterialId(2),
        &rising,
        Transform::Silence,
        Agent::Keys,
        2.0,
        2.0
    )
    .is_none());
}

#[test]
#[ignore = "preserved R0 red oracle: legacy preparation does not refuse a foreign owner"]
fn red_source_preparation_should_refuse_a_foreign_plan_owner() {
    let source = argument(ArgumentFamily::CallAndEarnedAnswer);
    let foreign = argument(ArgumentFamily::PromiseDeniedReturn);
    let mut perf = plain_plan(&foreign);
    assert!(
        prepare_performance(&source, &mut perf, &MusicWorld::black_ice()).is_err(),
        "source authority must refuse a performance owned by another song"
    );
}
