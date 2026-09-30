//! Directed adversarial cases independent of the flagship phrase examples.
use gibson::audio::human_music::{
    action::{ActionKind, Agent},
    composer::Composer,
    expression::{
        connective, observe, project, valid_function, ConnectiveViability, ExpressionEvent,
    },
    functor::perform_coherent,
    ids::ActionStamp,
    performance::PerformanceOptions,
    phrase_expression,
    score::{Note, PitchFunction, Role},
    semantic::deflected_lift_trace,
    MusicWorld, SongMap,
};

#[test]
fn r16_a_later_connector_cannot_move_its_stamped_push() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let world = MusicWorld::black_ice();
    let mut c = perform_coherent(&song, &world, PerformanceOptions::default());
    let template = *c
        .score
        .role_notes(Role::Lead)
        .find(|n| n.start_beat == 24.0)
        .unwrap();
    let mut push = c.perf.actions.actions[0].clone();
    push.kind = ActionKind::Push;
    push.initiator = Agent::Lead;
    push.start_beat = 25.0;
    push.dur_beats = 0.5;
    push.target_beat = Some(25.5);
    let push_id = c.perf.actions.push(push);
    let event = |at, pitch, structural, function| {
        let mut n: Note = template;
        n.start_beat = at;
        n.pitch = pitch;
        n.dur_beats = 0.45;
        n.function = Some(function);
        n.prov.actions = ActionStamp::NONE;
        ExpressionEvent {
            note: n,
            structural,
        }
    };
    // A# approaches stable B (available ninth), which approaches chord-tone C. The second
    // optional note owns a metric Push, so a valid pitch path cannot license moving that beat.
    let mut line = vec![
        event(24.0, 69, true, PitchFunction::ChordTone),
        event(24.5, 70, false, PitchFunction::ChromaticApproach),
        event(25.0, 71, false, PitchFunction::ChromaticApproach),
        event(25.5, 72, true, PitchFunction::ChordTone),
    ];
    line[2].note.prov.actions = ActionStamp::of(push_id);
    let skeleton = project(&line);
    let result = phrase_expression::realize(&c.perf, &world, line, &[], &[]);
    assert_eq!(project(&result.events), skeleton);
    let anchored = result
        .events
        .iter()
        .find(|n| n.note.prov.actions.has(push_id))
        .unwrap();
    assert_eq!(
        anchored.note.start_beat, 25.0,
        "a later connector's Push was shifted by a phrase-grid candidate: {:?}",
        result.plans
    );
}

#[test]
fn r16_phrase_candidates_respect_frozen_physics_across_tempo_patch_and_kinetic() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let c = perform_coherent(
        &song,
        &MusicWorld::black_ice(),
        PerformanceOptions::default(),
    );
    let template = *c
        .score
        .role_notes(Role::Lead)
        .find(|n| n.start_beat == 24.0)
        .unwrap();
    for tempo in [60.0, 88.0, 118.0, 176.0] {
        for (profile, patch) in [
            ("pluck", MusicWorld::swiss_signal().lead),
            ("sustain", MusicWorld::black_ice().lead),
        ] {
            for kinetic in [0.0, 0.8] {
                let mut world = MusicWorld::black_ice();
                world.tempo_bpm = tempo;
                world.lead = patch;
                let mut perf = c.perf.clone();
                for bar in &mut perf.ensemble {
                    bar.kinetic = kinetic;
                }
                let line: Vec<_> = [(24.0, 72, true), (24.5, 73, false), (25.5, 72, true)]
                    .into_iter()
                    .map(|(at, pitch, structural)| {
                        let mut note = template;
                        note.start_beat = at;
                        note.pitch = pitch;
                        note.dur_beats = 0.5;
                        note.function = Some(if structural {
                            PitchFunction::ChordTone
                        } else {
                            PitchFunction::Neighbor
                        });
                        note.prov.actions = ActionStamp::NONE;
                        ExpressionEvent { note, structural }
                    })
                    .collect();
                let skeleton = project(&line);
                let result = phrase_expression::realize(&perf, &world, line, &[], &[]);
                assert_eq!(project(&result.events), skeleton);
                for (i, e) in result
                    .events
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| connective(e.note.function))
                {
                    assert_eq!(
                        observe(
                            &perf,
                            &world,
                            e,
                            i.checked_sub(1).map(|j| &result.events[j].note),
                            result.events.get(i + 1).map(|n| &n.note),
                            &[]
                        )
                        .verdict,
                        ConnectiveViability::AsWritten,
                        "tempo={tempo} profile={profile} kinetic={kinetic}"
                    );
                }
            }
        }
    }
}

#[test]
fn r16_harmony_boundary_and_stage_hole_do_not_move_the_destination() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let world = MusicWorld::black_ice();
    let c = perform_coherent(&song, &world, PerformanceOptions::default());
    let template = *c
        .score
        .role_notes(Role::Lead)
        .find(|n| n.start_beat == 24.0)
        .unwrap();
    for (source, destination, hole) in [(27.5, 28.5, false), (24.5, 25.5, true)] {
        let mut perf = c.perf.clone();
        if hole {
            perf.accent.bars[6][5].hole = 1.0;
        }
        let line: Vec<_> = [
            (source - 0.5, 72, true),
            (source, 73, false),
            (destination, 72, true),
        ]
        .into_iter()
        .map(|(at, pitch, structural)| {
            let mut note = template;
            note.start_beat = at;
            note.pitch = pitch;
            note.dur_beats = 0.5;
            note.function = Some(if structural {
                PitchFunction::ChordTone
            } else {
                PitchFunction::Neighbor
            });
            note.prov.actions = ActionStamp::NONE;
            ExpressionEvent { note, structural }
        })
        .collect();
        let skeleton = project(&line);
        let original_harmony = perf.context_at(source).unwrap().start_beat;
        let result = phrase_expression::realize(&perf, &world, line, &[], &[]);
        assert_eq!(project(&result.events), skeleton);
        for e in result.events.iter().filter(|e| !e.structural) {
            assert_eq!(
                perf.context_at(e.note.start_beat).unwrap().start_beat,
                original_harmony
            );
            assert!(!perf.accent.is_hole(e.note.start_beat));
        }
    }
}

#[test]
fn r16_zero_and_one_structural_event_are_identity_cases() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let world = MusicWorld::black_ice();
    let c = perform_coherent(&song, &world, PerformanceOptions::default());
    let note = *c.score.role_notes(Role::Lead).next().unwrap();
    for line in [
        vec![],
        vec![ExpressionEvent {
            note,
            structural: true,
        }],
    ] {
        let skeleton = project(&line);
        let result = phrase_expression::realize(&c.perf, &world, line, &[], &[]);
        assert_eq!(project(&result.events), skeleton);
        assert!(result.plans.is_empty() && result.decisions.is_empty());
    }
}

#[test]
fn r16_fallback_omission_must_revalidate_the_surviving_connective_path() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let mut world = MusicWorld::black_ice();
    world.lead.adsr.3 = 0.31;
    let mut c = perform_coherent(&song, &world, PerformanceOptions::default());
    for bar in &mut c.perf.ensemble {
        bar.kinetic = 0.0;
    }
    let template = *c
        .score
        .role_notes(Role::Lead)
        .find(|n| n.start_beat == 24.0)
        .unwrap();
    let event = |at, dur, pitch, structural, function| {
        let mut n = template;
        n.start_beat = at;
        n.dur_beats = dur;
        n.pitch = pitch;
        n.function = Some(function);
        n.prov.actions = ActionStamp::NONE;
        ExpressionEvent {
            note: n,
            structural,
        }
    };
    // The very short B genuinely approaches C. C then approaches the later B, but hangs too
    // long. If fallback removes C, the remaining B->B cannot still claim ChromaticApproach.
    let line = vec![
        event(24.0, 0.45, 69, true, PitchFunction::ChordTone),
        event(24.5, 0.02, 71, false, PitchFunction::ChromaticApproach),
        event(24.6, 0.45, 72, false, PitchFunction::ChromaticApproach),
        event(25.5, 0.45, 71, true, PitchFunction::LicensedExtension),
    ];
    let result = phrase_expression::realize(&c.perf, &world, line, &[], &[]);
    for (i, e) in result
        .events
        .iter()
        .enumerate()
        .filter(|(_, e)| connective(e.note.function))
    {
        assert!(
            valid_function(
                &c.perf,
                i.checked_sub(1).map(|j| &result.events[j].note),
                &e.note,
                result.events.get(i + 1).map(|n| &n.note),
            ),
            "source fallback left a stale pitch-function justification after omission: {:?}",
            result.events
        );
    }
}
