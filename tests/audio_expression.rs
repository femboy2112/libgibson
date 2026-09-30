//! Round XV human-calibration falsifiers, committed before the production ruler.
use gibson::audio::human_music::{
    composer::Composer,
    expression::{annotate, project, ConnectiveViability as V, ExpressionDiagnostics},
    functor::perform_coherent,
    performance::PerformanceOptions,
    score::{Note, PitchFunction as F, Role},
    semantic::deflected_lift_trace,
    sonority::audible_end,
    MusicWorld, SongMap,
};

fn song() -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    )
}
fn life(n: &Note, w: &MusicWorld) -> f64 {
    let patch = if n.role == Role::Bass {
        &w.bass
    } else {
        &w.lead
    };
    (audible_end(n.start_beat, n.dur_beats.into(), patch, w.tempo_bpm) - n.start_beat) * 60.0
        / f64::from(w.tempo_bpm)
}

#[test]
fn r15_black_ice_slow_neighbor_must_be_subordinate() {
    let w = MusicWorld::black_ice();
    let x = perform_coherent(&song(), &w, PerformanceOptions::default());
    let n = x
        .score
        .role_notes(Role::Lead)
        .find(|n| n.start_beat == 24.5)
        .unwrap();
    assert_eq!((n.pitch, n.function), (73, Some(F::Neighbor)));
    // A pitch label alone cannot justify this separately attacked 440-ms object.
    assert!(life(n, &w) > 0.4);
    let d = ExpressionDiagnostics::measure(&x.perf, &x.score, &w);
    let r = d
        .rows
        .iter()
        .find(|r| r.note.role == Role::Lead && r.note.start_beat == 24.5)
        .unwrap();
    assert!(r.pitch_valid && r.optional);
    assert_eq!(r.verdict, V::NeedsCompression);
}

#[test]
fn r15_black_ice_bass_approach_must_be_subordinate() {
    let w = MusicWorld::black_ice();
    let x = perform_coherent(&song(), &w, PerformanceOptions::default());
    let n = x
        .score
        .role_notes(Role::Bass)
        .find(|n| n.start_beat == 3.5)
        .unwrap();
    assert_eq!((n.pitch, n.function), (44, Some(F::ChromaticApproach)));
    assert!(life(n, &w) > 0.38);
    let d = ExpressionDiagnostics::measure(&x.perf, &x.score, &w);
    let bass: Vec<_> = d
        .rows
        .iter()
        .filter(|r| r.note.role == Role::Bass)
        .collect();
    assert_eq!(bass.len(), 18);
    assert!(bass.iter().all(|r| r.verdict == V::NeedsCompression));
}

#[test]
fn r15_swiss_accepted_analogue_is_short() {
    let w = MusicWorld::swiss_signal();
    let x = perform_coherent(&song(), &w, PerformanceOptions::default());
    let ns: Vec<_> = x
        .score
        .role_notes(Role::Lead)
        .filter(|n| matches!(n.function, Some(F::Neighbor | F::ChromaticApproach)))
        .collect();
    assert!(!ns.is_empty());
    assert!(ns.iter().all(|n| life(n, &w) < 0.1));
}

#[test]
fn r15_optional_space_preserves_the_structural_line() {
    // q forgets an explicitly optional middle event, never the two destinations.
    let abstract_line = [(0.0, 72, true), (0.5, 73, false), (1.5, 72, true)];
    let q = |line: &[(f64, i32, bool)]| {
        line.iter()
            .filter(|n| n.2)
            .map(|n| (n.0, n.1))
            .collect::<Vec<_>>()
    };
    let space = [abstract_line[0], abstract_line[2]];
    assert_eq!(q(&abstract_line), q(&space));
}

#[test]
fn r15_frozen_ruler_distinguishes_worlds_without_production_changes() {
    for w in [MusicWorld::black_ice(), MusicWorld::swiss_signal()] {
        let x = perform_coherent(&song(), &w, PerformanceOptions::default());
        let d = ExpressionDiagnostics::measure(&x.perf, &x.score, &w);
        println!("{}\n{}", w.name, d.report(w.tempo_bpm));
        if w.tempo_bpm == 118.0 {
            assert!(d.rows.iter().all(|r| r.verdict == V::AsWritten));
        } else {
            assert_eq!(
                d.rows.iter().filter(|r| r.note.role == Role::Lead).count(),
                17
            );
            assert!(d.rows.iter().all(|r| r.verdict == V::NeedsCompression));
        }
        let notes: Vec<_> = x.score.role_notes(Role::Lead).copied().collect();
        let line = annotate(&x.perf, &notes);
        let space: Vec<_> = line.iter().filter(|e| e.structural).copied().collect();
        assert_eq!(project(&line), project(&space));
    }
}

#[test]
fn r15_lead_expression_keeps_skeleton_and_final_hearings() {
    use gibson::audio::human_music::{
        expression::ExpressionStrategy,
        functor::perform_expressive,
        identity::{keeps_identity, IdentityDiagnostics},
    };
    for w in [MusicWorld::black_ice(), MusicWorld::swiss_signal()] {
        let a = perform_coherent(&song(), &w, PerformanceOptions::default());
        let b = perform_expressive(&song(), &w, PerformanceOptions::default());
        assert_eq!(a.song.fingerprint(), b.song.fingerprint());
        assert_eq!(a.perf.fingerprint(), b.perf.fingerprint());
        assert!(b.score.stale_hearings().is_empty());
        let notes: Vec<_> = a.score.role_notes(Role::Lead).copied().collect();
        for event in annotate(&a.perf, &notes).iter().filter(|e| e.structural) {
            let n = b
                .score
                .role_notes(Role::Lead)
                .find(|n| n.start_beat == event.note.start_beat)
                .unwrap();
            assert_eq!(format!("{n:?}"), format!("{:?}", event.note));
        }
        let id = |x: &gibson::audio::human_music::functor::Composition| {
            IdentityDiagnostics::measure(&x.score.notes, &x.perf.contexts, &w, w.tempo_bpm)
        };
        assert!(keeps_identity(&id(&a), &id(&b), 0.0, 1e9));
        if w.tempo_bpm == 118.0 {
            assert_eq!(a.score.fingerprint(), b.score.fingerprint());
            assert!(b.score.expression_decisions.is_empty());
        } else {
            let edits: Vec<_> = b
                .score
                .expression_decisions
                .iter()
                .filter(|e| e.before.note.role == Role::Lead)
                .collect();
            assert_eq!(edits.len(), 17);
            for e in edits {
                assert_eq!(e.strategy, ExpressionStrategy::Grace);
                assert_eq!(e.after_observation.as_ref().unwrap().verdict, V::AsWritten);
                assert!(e.after.unwrap().start_beat > e.before.note.start_beat);
                assert!(e.after.unwrap().dur_beats < e.before.note.dur_beats);
                assert!(e.after.unwrap().velocity < e.before.note.velocity);
                assert!(b
                    .score
                    .role_notes(Role::Lead)
                    .any(|n| n.start_beat == e.before.target.unwrap().start_beat
                        && n.pitch == e.before.target.unwrap().pitch));
            }
        }
    }
}

fn synthetic() -> (
    gibson::audio::human_music::performance::PerformancePlan,
    Vec<gibson::audio::human_music::expression::ExpressionEvent>,
) {
    use gibson::audio::human_music::{
        context::analyze,
        expression::ExpressionEvent,
        form::SectionKind,
        score::Provenance,
        theory::{Chord, Mode, Quality, Scale},
    };
    let x = perform_coherent(
        &song(),
        &MusicWorld::swiss_signal(),
        PerformanceOptions::default(),
    );
    let mut perf = x.perf;
    let mut span = perf.chords[0];
    span.start_beat = 0.0;
    span.dur_beats = 120.0;
    span.chord = Chord::new(0, Quality::Maj);
    perf.chords = vec![span];
    perf.contexts = analyze(&perf.chords, &Scale::new(0, Mode::Ionian));
    let events = [
        (24.0, 0.45, 72, true, F::ChordTone),
        (24.5, 0.5, 73, false, F::Neighbor),
        (25.5, 0.45, 72, true, F::ChordTone),
    ]
    .into_iter()
    .map(|(beat, dur, pitch, structural, function)| {
        let mut note = Note::new(
            beat,
            dur,
            pitch,
            0.8,
            Role::Lead,
            Provenance::new(SectionKind::A),
        );
        note.function = Some(function);
        ExpressionEvent { note, structural }
    })
    .collect();
    (perf, events)
}

#[test]
fn r15_tempo_patch_factorial_and_compressed_neighbor() {
    use gibson::audio::human_music::expression::{observe, realize};
    let (perf, line) = synthetic();
    for bpm in [118.0, 88.0] {
        for sustaining in [false, true] {
            let mut w = MusicWorld::swiss_signal();
            w.tempo_bpm = bpm;
            if sustaining {
                w.lead = MusicWorld::black_ice().lead;
            }
            let r = observe(
                &perf,
                &w,
                &line[1],
                Some(&line[0].note),
                Some(&line[2].note),
                &[],
            );
            println!(
                "factorial bpm={bpm} sustain={sustaining} latency={} audible={} {:?}",
                r.target_latency_secs, r.audible_duration_secs, r.verdict
            );
            assert_eq!(
                r.verdict,
                if bpm == 118.0 && !sustaining {
                    V::AsWritten
                } else {
                    V::NeedsCompression
                }
            );
            let out = realize(&perf, &w, line.clone(), &[]);
            assert_eq!(project(&line), project(&out.events));
            assert_eq!(out.events.last().unwrap().note.start_beat, 25.5);
            let r = observe(
                &perf,
                &w,
                &out.events[1],
                Some(&out.events[0].note),
                Some(&out.events[2].note),
                &[],
            );
            assert!(r.pitch_valid);
            assert_eq!(r.verdict, V::AsWritten);
        }
    }
}

#[test]
fn r15_nonviable_release_and_low_kinetic_choose_legal_space() {
    use gibson::audio::human_music::expression::{realize, ExpressionStrategy};
    for long_tail in [true, false] {
        let (mut perf, line) = synthetic();
        let mut w = MusicWorld::black_ice();
        if long_tail {
            w.lead.adsr.3 = 1.0;
        } else {
            for bar in &mut perf.ensemble {
                bar.kinetic = 0.0;
            }
        }
        let out = realize(&perf, &w, line.clone(), &[]);
        assert_eq!(out.events.len(), 2);
        assert_eq!(out.decisions[0].strategy, ExpressionStrategy::Omitted);
        assert!(out.decisions[0].before.optional);
        assert_eq!(project(&line), project(&out.events));
    }
}

#[test]
fn r15_compact_run_keeps_contour_destination_and_structure() {
    use gibson::audio::human_music::expression::{realize, ExpressionStrategy};
    let (perf, mut line) = synthetic();
    line[0].note.pitch = 76;
    line[1].note.pitch = 74;
    line[1].note.function = Some(F::DiatonicPassing);
    let mut extra = line[1];
    extra.note.start_beat = 25.0;
    extra.note.pitch = 73;
    extra.note.function = Some(F::ChromaticPassing);
    line.insert(2, extra);
    let out = realize(&perf, &MusicWorld::black_ice(), line.clone(), &[]);
    assert_eq!(
        out.events.iter().map(|e| e.note.pitch).collect::<Vec<_>>(),
        vec![76, 74, 73, 72]
    );
    assert_eq!(project(&line), project(&out.events));
    assert_eq!(out.decisions.len(), 2);
    assert!(out
        .decisions
        .iter()
        .all(|e| e.strategy == ExpressionStrategy::Burst
            && e.after_observation.as_ref().unwrap().pitch_valid));
}

#[test]
fn r15_structural_connective_is_never_moved_to_save_ornament() {
    use gibson::audio::human_music::expression::realize;
    let (perf, mut line) = synthetic();
    line[1].structural = true;
    let out = realize(&perf, &MusicWorld::black_ice(), line.clone(), &[]);
    assert_eq!(project(&line), project(&out.events));
    assert!(out.decisions.is_empty());
}
