//! Actual-Rust negative controls for PR #102's hearing and source projections.
//! Registered predictions and exact baseline are in research/c137_kernel/hearing_baseline.md.
//! These intentionally characterize the archived path; a new opt-in kernel needs stronger laws.

use gibson::audio::human_music::{
    action::Agent,
    argument::MusicalArgument,
    comp::{keys_comp, keys_shell_n},
    composer::Composer,
    contract::CompositionGrammar,
    form::SectionKind,
    functor::{perform_argument, Composition},
    ids::MaterialId,
    interaction::Transform,
    material::{
        transform_material, InteractionMaterial, MaterialEvent, MaterialSource, PitchBasis,
    },
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::{Hearing, Note, Provenance, Role, Score},
    semantic::deflected_lift_trace,
    theory::Mode,
    voicing::keys_path,
    MusicWorld, SongMap,
};

fn world() -> MusicWorld {
    let mut world = MusicWorld::black_ice();
    world.tempo_bpm = 108.0;
    world.tonic_pc = 9;
    world.mode = Mode::Aeolian;
    world.use_sevenths = true;
    world.allow_modal_mixture = true;
    world
}

fn fusion(counterpoint: bool) -> Composition {
    let base = SongMap::compose(
        &deflected_lift_trace(128.0),
        2112,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    );
    let compiled = MusicalArgument::fusion(2112, &base)
        .unwrap()
        .compile(&base)
        .unwrap();
    perform_argument(
        &compiled,
        &world(),
        PerformanceOptions::default(),
        PerformanceProfile::BAND.with_counterpoint(counterpoint),
    )
    .expect("the actual fusion_gates construction must render")
}

fn heard_keys_score() -> Score {
    let mut score = Score::new(108.0, 4.0, 4.0);
    score.notes.push(Note::new(
        0.0,
        0.5,
        60,
        0.25,
        Role::Keys,
        Provenance::new(SectionKind::A),
    ));
    score
        .hearings
        .push(Hearing::of("drums", Role::Keys, &score.notes));
    score
}

#[test]
fn legacy_hearing_is_velocity_blind_but_pitch_sensitive() {
    let mut score = heard_keys_score();
    let fingerprint = score.fingerprint();
    score.notes[0].velocity = 0.75;
    assert_eq!(
        score.hearings[0],
        Hearing::of("drums", Role::Keys, &score.notes)
    );
    assert!(score.stale_hearings().is_empty());
    assert_ne!(fingerprint, score.fingerprint());

    // Calibrate the old observer: an event-field mutation is visible.
    score.notes[0].pitch += 1;
    let stale = score.stale_hearings();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].missing.len(), 1);
    assert_eq!(stale[0].unheard.len(), 1);
}

#[test]
#[ignore = "expected red at PR #102: legacy Hearing does not observe velocity"]
fn desired_dynamic_hearing_invariant_red_on_legacy() {
    let mut score = heard_keys_score();
    score.notes[0].velocity = 0.75;
    assert!(
        !score.stale_hearings().is_empty(),
        "legacy Hearing does not detect a post-observation velocity mutation"
    );
}

#[test]
fn archived_fusion_counterpoint_has_a_real_stale_drums_keys_witness() {
    let off = fusion(false);
    let on = fusion(true);
    for (name, c) in [("OFF", &off), ("ON", &on)] {
        let stale = c.score.stale_hearings();
        println!(
            "counterpoint {name}: song={:016x} score={:016x} hearings={} stale={}",
            c.song.fingerprint(),
            c.score.fingerprint(),
            c.score.hearings.len(),
            stale.len()
        );
        for h in &stale {
            println!(
                "  {} <- {:?}: missing={} unheard={}; first missing={:?}; first unheard={:?}",
                h.listener,
                h.source,
                h.missing.len(),
                h.unheard.len(),
                h.missing.first(),
                h.unheard.first()
            );
        }
    }
    assert!(off.score.stale_hearings().is_empty());
    let stale = on.score.stale_hearings();
    assert!(stale.iter().any(|h| {
        h.listener == "drums"
            && h.source == Role::Keys
            && (!h.missing.is_empty() || !h.unheard.is_empty())
    }));
    assert_ne!(off.score.fingerprint(), on.score.fingerprint());
    assert_eq!(
        format!("{:?}", off.score.drums),
        format!("{:?}", on.score.drums),
        "late keys counterpoint cannot affect already-generated drums"
    );
    assert_eq!(off.score.percussion, on.score.percussion);
    let stale_band_counts = |s: &Score| {
        s.percussion
            .as_ref()
            .unwrap()
            .bars
            .iter()
            .filter_map(|bar| {
                let start = f64::from(bar.bar) * 4.0;
                let actual = s
                    .notes
                    .iter()
                    .filter(|n| {
                        matches!(n.role, Role::Lead | Role::Bass | Role::Keys)
                            && n.start_beat >= start - 1e-6
                            && n.start_beat < start + 4.0 - 1e-6
                    })
                    .count();
                (actual != bar.evidence.band_onsets).then_some((
                    bar.bar,
                    bar.evidence.band_onsets,
                    actual,
                ))
            })
            .collect::<Vec<_>>()
    };
    assert!(stale_band_counts(&off.score).is_empty());
    let stale_counts = stale_band_counts(&on.score);
    println!("counterpoint ON stale drummer band counts (bar, heard, final): {stale_counts:?}");
    assert!(!stale_counts.is_empty());
    // With bass weaving held off, this toggle must not replace the bass floor.
    let bass = |s: &Score| format!("{:?}", s.role_notes(Role::Bass).collect::<Vec<_>>());
    assert_eq!(bass(&off.score), bass(&on.score));
}

#[test]
#[ignore = "expected red at PR #102: counterpoint is inserted after drummer hearing"]
fn desired_final_event_hearing_invariant_red_on_archived_counterpoint() {
    let c = fusion(true);
    assert!(c.score.stale_hearings().is_empty());
}

fn call(id: u32, direction: i32) -> InteractionMaterial {
    InteractionMaterial {
        id: MaterialId(id),
        owner: Agent::Lead,
        source: MaterialSource::Statement {
            statement: id as usize,
            motif: id as u8,
        },
        start_beat: 0.0,
        pitch_basis: PitchBasis::ScaleSteps,
        events: (0..4)
            .map(|i| MaterialEvent {
                onset: f64::from(i) * 0.5,
                dur: 0.4,
                accent: 0.6,
                step: Some(i * direction),
            })
            .collect(),
    }
}

fn call_notes(call: &InteractionMaterial, total_beats: f64) -> Vec<Note> {
    (0..(total_beats / 4.0) as u32)
        .flat_map(|bar| {
            call.events.iter().map(move |e| {
                let mut prov = Provenance::new(SectionKind::A);
                prov.material = Some(call.id);
                Note::new(
                    f64::from(bar) * 4.0 + e.onset,
                    e.dur as f32,
                    72 + e.step.unwrap(),
                    e.accent,
                    Role::Lead,
                    prov,
                )
            })
        })
        .collect()
}

#[test]
fn keys_listen_comp_projection_cannot_distinguish_equal_count_opposite_calls() {
    let c = fusion(false);
    let rising = call_notes(&call(0, 1), c.score.total_beats);
    let falling = call_notes(&call(1, -1), c.score.total_beats);
    let aggregate = |notes: &[Note]| {
        notes
            .iter()
            .map(|n| (n.start_beat, n.dur_beats, n.velocity))
            .collect::<Vec<_>>()
    };
    assert_eq!(aggregate(&rising), aggregate(&falling));
    assert_ne!(rising[1].pitch, falling[1].pitch);
    assert_ne!(rising[0].prov.material, falling[0].prov.material);

    // Freeze the independently selected harmonic voice path to isolate Listen/comp.
    let world = world();
    let path = keys_path(
        &c.perf,
        world.voicing_spread,
        &rising,
        keys_shell_n(&c.perf),
    );
    let a = keys_comp(&c.perf, &world, &rising, 2112, &path);
    let b = keys_comp(&c.perf, &world, &falling, 2112, &path);
    assert!(
        !a.is_empty(),
        "a silent instrument would be a vacuous control"
    );
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}

#[test]
fn existing_materials_distinguish_source_contours_and_admit_silence() {
    let rising = call(0, 1);
    let falling = call(1, -1);
    let complete = |src: &InteractionMaterial| {
        transform_material(
            MaterialId(2),
            src,
            Transform::Complete,
            Agent::Keys,
            2.0,
            2.0,
        )
        .expect("a bounded source-bearing answer")
    };
    let up = complete(&rising);
    let down = complete(&falling);
    assert_ne!(up.events, down.events);
    assert_eq!(up.final_direction(), 1);
    assert_eq!(down.final_direction(), -1);
    assert_eq!(
        up.source,
        MaterialSource::Derived {
            from: rising.id,
            transform: Transform::Complete,
        }
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

    let mut relabeled = rising.clone();
    relabeled.source = MaterialSource::Statement {
        statement: 97,
        motif: 42,
    };
    assert_eq!(up.events, complete(&relabeled).events);
}
