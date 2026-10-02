use super::*;
use crate::audio::human_music::action::ActionKind;
use crate::audio::human_music::{policy::PerformanceProfile, reference_song::ReferenceSong};
fn source() -> ReferenceSong {
    ReferenceSong::from_tsv(
        include_str!("../../../docs/fixtures/humanmusic-cover/ode-import/reference.tsv"),
        "sop",
    )
    .unwrap()
}
fn lift(map: &CoverMap, world: &MusicWorld, seed: u64) -> Composition {
    let candidate = cover(
        map,
        CoverTarget {
            world,
            seed,
            grammar: CompositionGrammar::HookArc,
            options: PerformanceOptions::default(),
            profile: PerformanceProfile::POCKET,
        },
    )
    .unwrap();
    assert!(
        CoverPipelineReceipt::measure(&candidate, world).passes(),
        "{}\n{:?}",
        super::super::song::SongMapConformance::check(
            &candidate.song,
            &candidate.perf,
            &candidate.score
        )
        .report(),
        CoverPipelineReceipt::measure(&candidate, world)
    );
    candidate
}
#[test]
fn external_melody_survives_three_worlds_key_tempo_and_seed() {
    let map = source()
        .extract(CoverSpec::new([CoverAxis::Motif]))
        .unwrap();
    let mut prior = None;
    for (i, mut world) in MusicWorld::all().into_iter().enumerate() {
        world.tonic_pc = (world.tonic_pc + 3) % 12;
        world.tempo_bpm += 11.0;
        let c = lift(&map, &world, 711 + i as u64);
        let law = CoverConformance::check(&map, &c, &world);
        assert!(law.passes(), "{}", law.report());
        let pipeline = CoverPipelineReceipt::measure(&c, &world);
        assert!(pipeline.passes(), "{pipeline:?}");
        assert!(c.score.role_notes(Role::Lead).all(|n| n.function.is_some()));
        let temporal = super::super::temporal::TemporalPitchDiagnostics::measure(&c.perf, &c.score);
        let actions = super::super::witness::audit(&c.perf, &c.score);
        eprintln!(
            "{} notes={} false={} actions={}/{}",
            world.name,
            c.score.notes.len(),
            temporal.false_function_claims,
            actions.rows.iter().filter(|r| r.witnessed).count(),
            actions.rows.len()
        );
        if temporal.false_function_claims > 0 {
            eprintln!("{}", temporal.report());
        }
        if let Some(p) = prior {
            assert!(CoverFreedom::compare(&p, &c).has_freedom());
        }
        prior = Some(c);
    }
}
#[test]
fn actual_motif_mutation_fails_while_free_voicing_does_not() {
    let world = MusicWorld::black_ice();
    let map = source()
        .extract(CoverSpec::new([CoverAxis::Motif]))
        .unwrap();
    let mut c = lift(&map, &world, 11);
    let n = c
        .score
        .notes
        .iter_mut()
        .find(|n| n.role == Role::Pad)
        .unwrap();
    n.pitch += 12;
    assert!(CoverConformance::check(&map, &c, &world).passes());
    let n = c
        .score
        .notes
        .iter_mut()
        .find(|n| n.role == Role::Lead)
        .unwrap();
    n.pitch += 1;
    let law = CoverConformance::check(&map, &c, &world);
    assert!(!law.passes());
    assert_eq!(
        law.checks
            .iter()
            .filter(|r| !r.passed)
            .map(|r| r.axis)
            .collect::<Vec<_>>(),
        [CoverAxis::Motif]
    );
}
#[test]
fn equal_quotients_cannot_leak_free_reference_data() {
    let world = MusicWorld::black_ice();
    let map = source()
        .extract(CoverSpec::new([CoverAxis::Motif]))
        .unwrap();
    let a = lift(&map, &world, 111);
    let mut b = lift(&map, &world, 111);
    b.song.seed = 998;
    b.song.trace.events.clear();
    for n in b.score.notes.iter_mut().filter(|n| n.role != Role::Lead) {
        n.pitch += 12;
        n.velocity *= 0.5;
    }
    b.score.tempo_bpm += 40.0;
    let spec = CoverSpec::new([CoverAxis::Motif]);
    let ca = CoverMap::extract(&a, &world, spec.clone()).unwrap();
    let cb = CoverMap::extract(&b, &world, spec).unwrap();
    assert_eq!(ca, cb);
    drop((a, b));
    let p = lift(&ca, &world, 999);
    let q = lift(&cb, &world, 999);
    assert_eq!(all_evidence(&p), all_evidence(&q));
    assert_eq!(
        p.score.canonical_fingerprint(),
        q.score.canonical_fingerprint()
    );
    assert_eq!(
        p.perf.canonical_fingerprint(),
        q.perf.canonical_fingerprint()
    );
}
#[test]
fn sparse_swing_collision_is_rejected_before_source() {
    let mut world = MusicWorld::vapor95();
    world.swing = 0.5;
    let mut map = source()
        .extract(CoverSpec::new([CoverAxis::Motif]))
        .unwrap();
    let line = map.motif.as_mut().unwrap();
    line.notes.truncate(2);
    line.notes[0].at = MetricPosition::new(1, 2).unwrap();
    line.notes[0].reserved_until = None;
    line.notes[1].at = MetricPosition::new(9, 16).unwrap();
    line.notes[1].reserved_until = None;
    let result = cover(
        &map,
        CoverTarget {
            world: &world,
            seed: 1,
            grammar: CompositionGrammar::HookArc,
            options: PerformanceOptions::default(),
            profile: PerformanceProfile::POCKET,
        },
    );
    assert!(matches!(
        result,
        Err(CoverError::Invalid(
            "target groove is not strictly monotone on pinned attacks"
        ))
    ));
}
#[test]
fn observed_rest_is_not_filled_by_free_articulation() {
    let r=ReferenceSong::from_tsv("meter 4 4\nkey 0 major\ntempo 100\nlength 4/1\nnote melody 0/1 1/2 60\nnote melody 2/1 1/2 64\n","melody").unwrap();
    let map = r.extract(CoverSpec::new([CoverAxis::Motif])).unwrap();
    let world = MusicWorld::swiss_signal();
    let mut c = lift(&map, &world, 9);
    assert!(CoverConformance::check(&map, &c, &world).passes());
    let note = c
        .score
        .notes
        .iter_mut()
        .find(|n| n.role == Role::Lead)
        .unwrap();
    note.dur_beats = 1.5;
    assert!(!CoverConformance::check(&map, &c, &world).passes());
}
#[test]
fn ordered_unknown_chart_retains_family_equivalence() {
    let chord = OrderedChord {
        relative_root: 0,
        quality: Quality::Maj,
    };
    let chart = OrderedChart {
        sections: ["intro", "verse", "verse", "chorus", "chorus"]
            .into_iter()
            .map(|family| OrderedSection {
                family: family.into(),
                chords: Some(vec![chord.clone()]),
            })
            .collect(),
    };
    let map = CoverMap::from_ordered_chart(chart).unwrap();
    assert_eq!(map.knowledge(CoverAxis::Motif), CoverKnowledge::Unknown);
    assert!(map.length.is_none());
    let world = MusicWorld::swiss_signal();
    let mut c = cover_skeleton(
        &map,
        CoverTarget {
            world: &world,
            seed: 3,
            grammar: CompositionGrammar::HookArc,
            options: PerformanceOptions::default(),
            profile: PerformanceProfile::POCKET,
        },
        SkeletonSchedule { bars_per_chord: 1 },
    )
    .unwrap();
    assert!(CoverConformance::check(&map, &c, &world).passes());
    c.song.plan.form.phrases[4].family = SectionFamily::Named { identity: 1 };
    assert!(!CoverConformance::check(&map, &c, &world).passes());
}

fn generated_source(world: &MusicWorld) -> Composition {
    let song = SongMap::build(
        &super::super::semantic::demo_trace(128.0),
        2112,
        Some(CompositionGrammar::HookArc),
    );
    super::super::functor::perform_pocketed(&song, world, PerformanceOptions::default())
}
fn target(world: &MusicWorld, seed: u64) -> CoverTarget<'_> {
    CoverTarget {
        world,
        seed,
        grammar: CompositionGrammar::HookArc,
        options: PerformanceOptions::default(),
        profile: PerformanceProfile::POCKET,
    }
}
#[test]
fn generated_default_pins_contract_and_mutations_are_localized() {
    let world = MusicWorld::black_ice();
    let source = generated_source(&world);
    let spec = CoverSpec::from_contract(&source.song.plan.contract);
    let map = CoverMap::extract(&source, &world, spec).unwrap();
    let c = cover_candidate(&map, target(&world, 2112)).unwrap();
    assert!(CoverConformance::check(&map, &c, &world).passes());
    let mut harmonic = copy_composition(&c);
    harmonic.score.chords[0].chord.root_pc = (harmonic.score.chords[0].chord.root_pc + 1) % 12;
    let failed: Vec<_> = CoverConformance::check(&map, &harmonic, &world)
        .checks
        .into_iter()
        .filter(|c| !c.passed)
        .map(|c| c.axis)
        .collect();
    assert_eq!(failed, vec![CoverAxis::HarmonicContour]);
    let mut form = copy_composition(&c);
    form.score.sections[0].bars += 1;
    let failed: Vec<_> = CoverConformance::check(&map, &form, &world)
        .checks
        .into_iter()
        .filter(|c| !c.passed)
        .map(|c| c.axis)
        .collect();
    assert_eq!(failed, vec![CoverAxis::Form]);
}
#[test]
fn all_axes_freeze_identity_but_not_articulation_and_none_is_not_a_cover() {
    let world = MusicWorld::black_ice();
    let source = generated_source(&world);
    let all = CoverMap::extract(&source, &world, CoverSpec::all()).unwrap();
    let p = cover_candidate(&all, target(&world, 21)).unwrap();
    let q = cover_candidate(&all, target(&world, 22)).unwrap();
    assert!(CoverConformance::check(&all, &p, &world).passes());
    assert!(CoverConformance::check(&all, &q, &world).passes());
    assert!(CoverFreedom::compare(&p, &q).has_freedom());
    let again = cover_candidate(&all, target(&world, 21)).unwrap();
    assert_eq!(all_evidence(&p), all_evidence(&again));
    let none = CoverMap::extract(&source, &world, CoverSpec::none()).unwrap();
    assert!(!none.spec.has_song_identity());
    let free = cover_candidate(&none, target(&world, 22)).unwrap();
    assert!(CoverConformance::check(&none, &free, &world).passes());
    let melody = |c: &Composition| {
        c.score
            .role_notes(Role::Lead)
            .map(|n| (n.start_beat.to_bits(), n.pitch))
            .collect::<Vec<_>>()
    };
    assert_ne!(melody(&source), melody(&free));
    assert!(!CoverConformance::check(&all, &free, &world).passes());
}
#[test]
fn conflicting_stage_and_motif_constraints_are_rejected_before_planning() {
    let world = MusicWorld::black_ice();
    let source = generated_source(&world);
    let mut map = CoverMap::extract(
        &source,
        &world,
        CoverSpec::new([CoverAxis::Motif, CoverAxis::Orchestration]),
    )
    .unwrap();
    let bar = (map.motif.as_ref().unwrap().notes[0].at.beats() / 4.0) as usize;
    map.orchestration.as_mut().unwrap()[bar].roles[0] = ArrangementRole::Silent;
    assert_eq!(map.validate(), Err(CoverError::ConflictingPins));
    assert!(matches!(
        cover_candidate(&map, target(&world, 1)),
        Err(CoverError::ConflictingPins)
    ));
}
#[test]
fn chromatic_caller_is_the_same_exact_object_in_song_material_and_score() {
    let r = ReferenceSong::from_tsv("meter 4 4\nkey 0 major\ntempo 100\nlength 8/1\nnote m 0/1 1/1 60\nnote m 1/1 1/1 61\nnote m 2/1 1/1 65\nnote m 3/1 1/1 64\nnote m 4/1 1/1 60\nnote m 5/1 1/1 61\nnote m 6/1 1/1 65\nnote m 7/1 1/1 64\n","m").unwrap();
    let map = r.extract(CoverSpec::new([CoverAxis::Motif])).unwrap();
    let world = MusicWorld::black_ice();
    let c = cover_candidate(&map, target(&world, 8)).unwrap();
    let statement = &c.perf.statements[0];
    assert_eq!(
        statement.motif.pitch_basis,
        super::super::theory::PitchBasis::Semitones
    );
    assert_eq!(&statement.motif.degrees[..4], &[0, 1, 5, 4]);
    assert_eq!(c.song.thematic.sites[0].motif, statement.motif);
    let material = c
        .perf
        .materials
        .iter()
        .find(|m| m.id == statement.material)
        .unwrap();
    assert_eq!(
        material.pitch_basis,
        super::super::theory::PitchBasis::Semitones
    );
    assert_eq!(
        material
            .events
            .iter()
            .take(4)
            .map(|e| e.step)
            .collect::<Vec<_>>(),
        vec![Some(0), Some(1), Some(5), Some(4)]
    );
    let pitches: Vec<_> = c
        .score
        .role_notes(Role::Lead)
        .take(4)
        .map(|n| n.pitch)
        .collect();
    assert_eq!(
        pitches.iter().map(|p| p - pitches[0]).collect::<Vec<_>>(),
        vec![0, 1, 5, 4]
    );
    assert!(c.score.stale_hearings().is_empty());
    let transformed = statement.motif.invert().retrograde().transpose(2);
    assert_eq!(transformed.pitch_basis, statement.motif.pitch_basis);
    assert!(CoverConformance::check(&map, &c, &world).passes());
}
#[test]
fn source_carry_law_releases_foreign_tone_without_changing_attack_or_pitch() {
    let world = MusicWorld::black_ice();
    let map = source()
        .extract(CoverSpec::new([CoverAxis::Motif]))
        .unwrap();
    let c = cover_candidate(&map, target(&world, 1)).unwrap();
    let mut perf = c.perf.clone();
    perf.chords = vec![
        ChordSpan {
            start_beat: 0.0,
            dur_beats: 1.0,
            chord: Chord::new(0, Quality::Maj),
            function: super::super::theory::Function::Tonic,
            degree: 0,
            note: "test",
        },
        ChordSpan {
            start_beat: 1.0,
            dur_beats: 3.0,
            chord: Chord::new(1, Quality::Maj),
            function: super::super::theory::Function::Tonic,
            degree: 0,
            note: "test",
        },
    ];
    let mut note = Note::new(
        0.5,
        2.0,
        60,
        0.7,
        Role::Bass,
        Provenance::new(super::super::form::SectionKind::A),
    );
    note.function = Some(super::super::score::PitchFunction::ChordTone);
    let mut notes = vec![note];
    bound_source_gates(&perf, &mut notes);
    assert_eq!(
        (notes[0].start_beat, notes[0].pitch, notes[0].dur_beats),
        (0.5, 60, 0.5)
    );
}
#[test]
fn hostile_source_leak_would_fail_the_output_equality_witness() {
    let world = MusicWorld::black_ice();
    let map = source()
        .extract(CoverSpec::new([CoverAxis::Motif]))
        .unwrap();
    let a = cover_candidate(&map, target(&world, 8)).unwrap();
    let b = cover_candidate(&map, target(&world, 8)).unwrap();
    let mut forbidden = copy_composition(&a);
    // Deliberately hostile alternate route: retain and apply excluded source dynamics.
    forbidden.score.notes[0].velocity *= 0.5;
    assert!(CoverConformance::check(&map, &forbidden, &world).passes());
    assert_eq!(all_evidence(&a), all_evidence(&b));
    assert_ne!(all_evidence(&forbidden), all_evidence(&b));
}

fn copy_composition(c: &Composition) -> Composition {
    Composition {
        score: c.score.clone(),
        song: c.song.clone(),
        perf: c.perf.clone(),
    }
}
fn all_evidence(c: &Composition) -> String {
    format!("{:?}", (&c.song, &c.perf, &c.score))
}

#[test]
fn occupancy_evidence_is_required_and_canonical_rest_never_yields() {
    let world = MusicWorld::vapor95();
    let map = source()
        .extract(CoverSpec::new([CoverAxis::Motif]))
        .unwrap();
    let mut c = cover_candidate(&map, target(&world, 711)).unwrap();
    assert!(CoverPipelineReceipt::measure(&c, &world).passes());
    let lead = c
        .score
        .occupancy
        .iter()
        .find(|o| o.role == Role::Lead)
        .unwrap();
    for pin in &map.motif.as_ref().unwrap().notes {
        assert!(lead.rhythm.iter().any(|r| r.beat == pin.at.beats()));
        assert!(!lead.yields_at(pin.at.beats()));
        assert!(!lead.allows_comp_at(pin.at.beats(), &[]));
    }
    let mut missing_attacks = copy_composition(&c);
    missing_attacks
        .score
        .occupancy
        .iter_mut()
        .for_each(|o| o.rhythm.clear());
    assert!(!CoverPipelineReceipt::measure(&missing_attacks, &world).passes());
    c.score.occupancy.clear();
    assert!(!CoverPipelineReceipt::measure(&c, &world).passes());
}
#[test]
fn target_vocabulary_does_not_secretly_allow_mixture_or_extensions() {
    let world = MusicWorld::swiss_signal();
    let base = source().extract(CoverSpec::none()).unwrap();
    let mut map = base.clone();
    map.spec = CoverSpec::new([CoverAxis::HarmonicContour]);
    map.unknown_axes
        .retain(|a| *a != CoverAxis::HarmonicContour);
    map.harmony = Some(vec![CoverChord {
        at: MetricPosition::new(0, 1).unwrap(),
        end: map.length.unwrap(),
        relative_root: 1,
        quality: Quality::Maj,
    }]);
    assert!(matches!(
        cover_candidate(&map, target(&world, 1)),
        Err(CoverError::Invalid(
            "pinned harmony outside target vocabulary"
        ))
    ));
    map.harmony.as_mut().unwrap()[0].relative_root = 0;
    map.harmony.as_mut().unwrap()[0].quality = Quality::Maj9;
    let mut mixed = MusicWorld::black_ice();
    mixed.mode = super::super::theory::Mode::Ionian;
    let mut t = target(&mixed, 1);
    t.options.language = super::super::language::MusicalLanguage::simple();
    assert!(matches!(
        cover_candidate(&map, t),
        Err(CoverError::Invalid(
            "pinned harmony outside target vocabulary"
        ))
    ));
}

// ---- Groove-debt honesty: a pinned kit never promises a return it cannot perform. ----

/// The lab's generated source exactly: HookArc on `demo_trace(64)`, seed 2112, BLACK_ICE.
fn lab_generated_map() -> (MusicWorld, CoverMap) {
    let world = MusicWorld::black_ice();
    let song = SongMap::build(
        &super::super::semantic::demo_trace(64.0),
        2112,
        Some(CompositionGrammar::HookArc),
    );
    let source =
        super::super::functor::perform_pocketed(&song, &world, PerformanceOptions::default());
    let map = CoverMap::extract(
        &source,
        &world,
        CoverSpec::from_contract(&song.plan.contract),
    )
    .unwrap();
    (world, map)
}
fn groove_debts(ledger: &super::super::discourse::ObligationLedger) -> usize {
    use super::super::discourse::ObligationKind;
    ledger
        .obligations
        .iter()
        .filter(|o| o.kind == ObligationKind::GrooveDestabilization)
        .count()
}
#[test]
fn pinned_groove_plans_no_groove_debt_and_the_generated_cover_is_admitted() {
    let (black, map) = lab_generated_map();
    assert!(
        map.groove.is_some(),
        "the contract default pins Groove here"
    );
    let mut world = black;
    world.tempo_bpm = 95.0;
    for seed in [901, 902] {
        let c = cover_candidate(&map, target(&world, seed)).unwrap();
        // The planner never opened the debt it could not discharge.
        assert_eq!(groove_debts(&c.song.plan.discourse.ledger), 0);
        assert_eq!(groove_debts(&c.perf.obligations), 0);
        let song = super::super::song::SongMapConformance::check(&c.song, &c.perf, &c.score);
        assert_eq!(song.unwitnessed_song_obligations, 0, "{}", song.report());
        assert!(song.passes(), "{}", song.report());
        assert!(
            CoverPipelineReceipt::measure(&c, &world).passes(),
            "{:?}",
            CoverPipelineReceipt::measure(&c, &world)
        );
        assert!(CoverConformance::check(&map, &c, &world).passes());
        // No witness is manufactured: no Drums ReEntry, no action stamped on any drum stroke.
        assert!(!c
            .perf
            .actions
            .of_kind(ActionKind::ReEntry)
            .any(|a| a.initiator == Agent::Drums));
        assert!(c.score.drums.iter().all(|h| h.prov.actions.is_empty()));
    }
}
#[test]
fn freed_groove_keeps_its_debt_and_a_real_reentry_witnesses_it() {
    use super::super::discourse::ObligationKind;
    let world = MusicWorld::black_ice();
    let song = SongMap::build(
        &super::super::semantic::demo_trace(64.0),
        2112,
        Some(CompositionGrammar::HookArc),
    );
    let source =
        super::super::functor::perform_pocketed(&song, &world, PerformanceOptions::default());
    let spec = CoverSpec::from_contract(&song.plan.contract).with(CoverAxis::Groove, false);
    let map = CoverMap::extract(&source, &world, spec).unwrap();
    assert!(map.groove.is_none());
    let c = cover_candidate(&map, target(&world, 901)).unwrap();
    // Same pinned form, so the same Withhold phrase: the role-driven kit really strips, so the
    // debt stays declared — the repair is not "delete groove obligations everywhere".
    assert_eq!(groove_debts(&c.perf.obligations), 1);
    let debt = c
        .perf
        .obligations
        .obligations
        .iter()
        .find(|o| o.kind == ObligationKind::GrooveDestabilization)
        .unwrap();
    let witness = debt
        .settlement
        .and_then(|s| s.witness)
        .expect("a witnessed return");
    let action = c.perf.actions.get(witness).unwrap();
    assert_eq!(action.kind, ActionKind::ReEntry);
    let song = super::super::song::SongMapConformance::check(&c.song, &c.perf, &c.score);
    assert!(song.passes(), "{}", song.report());
}
#[test]
fn an_unwitnessed_groove_settlement_still_fails_admission() {
    use super::super::discourse::{DiscourseRole, ObligationKind, SettleHow};
    let (black, map) = lab_generated_map();
    let mut world = black;
    world.tempo_bpm = 95.0;
    let c = cover_candidate(&map, target(&world, 901)).unwrap();
    let mut forged = copy_composition(&c);
    let n = forged.perf.obligations.phrases;
    let id = forged.perf.obligations.open_debt(
        ObligationKind::GrooveDestabilization,
        n - 2,
        Some(n - 1),
        0.4,
        false,
    );
    forged
        .perf
        .obligations
        .settle(id, n - 1, DiscourseRole::Dissolve, SettleHow::Paid)
        .unwrap();
    let song =
        super::super::song::SongMapConformance::check(&forged.song, &forged.perf, &forged.score);
    assert_eq!(song.unwitnessed_song_obligations, 1);
    assert!(!song.passes());
    assert!(!CoverPipelineReceipt::measure(&forged, &world).passes());
}
#[test]
fn pinned_kit_ledger_is_the_role_driven_ledger_without_groove_debts() {
    use super::super::discourse::{
        resolve_obligations, resolve_obligations_for, DiscourseRole as R, KitMotion, ObligationKind,
    };
    let orderings: [&[R]; 5] = [
        &[R::Establish, R::Culminate, R::Withhold, R::Dissolve],
        &[R::Establish, R::Question, R::Depart, R::Answer, R::Dissolve],
        &[R::Question, R::Withhold, R::Culminate, R::Answer, R::Return],
        &[
            R::Establish,
            R::Withhold,
            R::Establish,
            R::Question,
            R::Answer,
        ],
        &[R::Establish, R::Intensify, R::Culminate, R::Return],
    ];
    let shape = |o: &super::super::discourse::Obligation| {
        (
            o.kind,
            o.source_phrase,
            o.deadline,
            o.settlement.map(|s| (s.by_phrase, s.how)),
        )
    };
    for roles in orderings {
        let driven = resolve_obligations(roles);
        let pinned = resolve_obligations_for(roles, KitMotion::Pinned);
        assert_eq!(groove_debts(&pinned), 0, "{roles:?}");
        let expected: Vec<_> = driven
            .obligations
            .iter()
            .filter(|o| o.kind != ObligationKind::GrooveDestabilization)
            .map(shape)
            .collect();
        let got: Vec<_> = pinned.obligations.iter().map(shape).collect();
        assert_eq!(got, expected, "{roles:?}");
        // Ids stay the ledger's dense index, so `get` resolves every surviving debt.
        for (i, o) in pinned.obligations.iter().enumerate() {
            assert_eq!(o.id.index(), i);
            assert_eq!(pinned.get(o.id).map(shape), Some(shape(o)));
        }
        assert_eq!(
            resolve_obligations_for(roles, KitMotion::RoleDriven).obligations,
            driven.obligations
        );
    }
}
#[test]
fn ordinary_song_planning_is_unchanged_by_the_kit_seam() {
    use super::super::discourse::{DiscoursePlan, KitMotion};
    use super::super::plan::CompositionPlan;
    for grammar in [
        CompositionGrammar::HookArc,
        CompositionGrammar::DeflectedLift,
        CompositionGrammar::PropulsiveReturn,
    ] {
        let song = SongMap::build(&super::super::semantic::demo_trace(96.0), 17, Some(grammar));
        let p = &song.plan;
        let rebuilt = CompositionPlan::from_form_for_kit(
            &song.timeline,
            p.form.clone(),
            p.contract.clone(),
            KitMotion::RoleDriven,
        );
        assert_eq!(format!("{p:?}"), format!("{rebuilt:?}"));
        let d = DiscoursePlan::build(&song.timeline, &p.form, &p.contract);
        let k = DiscoursePlan::build_for_kit(
            &song.timeline,
            &p.form,
            &p.contract,
            KitMotion::RoleDriven,
        );
        assert_eq!(format!("{d:?}"), format!("{k:?}"));
    }
}
