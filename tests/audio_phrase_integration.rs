//! Round XVI end-to-end invariants and deliberately separate hostile configurations.
use gibson::audio::human_music::{
    composer::Composer,
    expression::annotate,
    functor::{perform_coherent, perform_expressive, perform_phrased, Composition},
    identity::IdentityDiagnostics,
    performance::PerformanceOptions,
    score::{PitchFunction, Role},
    semantic::{deflected_lift_trace, demo_trace},
    song::SongMapConformance,
    temporal::TemporalPitchDiagnostics,
    witness, MusicWorld, SongMap,
};

fn check(a: &Composition, b: &Composition, world: &MusicWorld, label: &str) {
    assert_eq!(a.song.fingerprint(), b.song.fingerprint(), "{label}");
    assert_eq!(a.perf.fingerprint(), b.perf.fingerprint(), "{label}");
    assert_eq!(
        SongMapConformance::check(&a.song, &a.perf, &a.score).report(),
        SongMapConformance::check(&b.song, &b.perf, &b.score).report(),
        "{label}"
    );
    b.score.validate().unwrap();
    assert!(b.score.stale_hearings().is_empty(), "{label}");
    let r14 = perform_coherent(&a.song, world, PerformanceOptions::default());
    let source: Vec<_> = r14.score.role_notes(Role::Lead).copied().collect();
    for e in annotate(&a.perf, &source).iter().filter(|e| e.structural) {
        assert!(
            b.score
                .role_notes(Role::Lead)
                .any(|n| n.start_beat == e.note.start_beat
                    && n.pitch == e.note.pitch
                    && n.prov.material == e.note.prov.material),
            "structural destination {label}: {:?}",
            e.note
        );
    }
    assert!(
        !b.score
            .notes
            .iter()
            .any(|n| n.function == Some(PitchFunction::SlidePath)),
        "{label}"
    );
    let old = witness::audit(&a.perf, &a.score);
    let new = witness::audit(&b.perf, &b.score);
    for row in old.rows.iter().filter(|r| r.witnessed) {
        assert!(
            new.rows
                .iter()
                .any(|r| r.action == row.action && r.witnessed),
            "lost action {label}: {row:?}"
        );
    }
    let ia = IdentityDiagnostics::measure(&a.score.notes, &a.perf.contexts, world, world.tempo_bpm);
    let ib = IdentityDiagnostics::measure(&b.score.notes, &b.perf.contexts, world, world.tempo_bpm);
    for r in ib.flips() {
        assert!(
            ia.flips()
                .any(|old| old.start_beat <= r.start_beat + 1e-6
                    && old.end_beat >= r.end_beat - 1e-6),
            "new held identity flip {label}: {r:?}"
        );
    }
    let ta = TemporalPitchDiagnostics::measure(&a.perf, &a.score);
    let tb = TemporalPitchDiagnostics::measure(&b.perf, &b.score);
    assert!(
        tb.false_function_claims <= ta.false_function_claims,
        "new temporal claim {label}: {} -> {}",
        ta.false_function_claims,
        tb.false_function_claims
    );
}

#[test]
fn frozen_historical_arms_and_new_flagship_invariants() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    for (w, old14, old15) in [
        (
            MusicWorld::black_ice(),
            0x562462b4c3c7e728,
            0x32cecf0800d51ce5,
        ),
        (
            MusicWorld::swiss_signal(),
            0xbeb08c057ed87d94,
            0xbeb08c057ed87d94,
        ),
    ] {
        let r14 = perform_coherent(&song, &w, PerformanceOptions::default());
        let r15 = perform_expressive(&song, &w, PerformanceOptions::default());
        assert_eq!(r14.score.fingerprint(), old14);
        assert_eq!(r15.score.fingerprint(), old15);
        let r16 = perform_phrased(&song, &w, PerformanceOptions::default());
        check(&r15, &r16, &w, w.name);
        assert_eq!(
            IdentityDiagnostics::measure(&r16.score.notes, &r16.perf.contexts, &w, w.tempo_bpm)
                .flips()
                .count(),
            0
        );
    }
}

#[test]
#[ignore = "bounded 120-case fresh-seed/world/tempo holdout; run explicitly in release"]
fn r16_hostile_world_seed_tempo_sweep() {
    let mut count = 0;
    for seed in [3, 7, 19, 43, 101] {
        for story in [false, true] {
            for composer in [Composer::StablePropulsion, Composer::MeaningDirected] {
                let trace = if story {
                    deflected_lift_trace(120.0)
                } else {
                    demo_trace(96.0)
                };
                let song = SongMap::compose(&trace, seed, None, composer);
                for mut w in [
                    MusicWorld::swiss_signal(),
                    MusicWorld::black_ice(),
                    MusicWorld::vapor95(),
                ] {
                    for tempo_delta in [0.0, -12.0] {
                        let original = w.tempo_bpm;
                        w.tempo_bpm = original + tempo_delta;
                        let a = perform_expressive(&song, &w, PerformanceOptions::default());
                        let b = perform_phrased(&song, &w, PerformanceOptions::default());
                        let label = format!(
                            "seed={seed} story={story} composer={composer:?} world={} tempo={}",
                            w.name, w.tempo_bpm
                        );
                        check(&a, &b, &w, &label);
                        println!(
                            "PASS {label} notes={} plans={} voicings={}",
                            b.score.notes.len(),
                            b.score.phrase_plans.len(),
                            b.score.support_voicing_decisions.len()
                        );
                        count += 1;
                        w.tempo_bpm = original;
                    }
                }
            }
        }
    }
    assert_eq!(count, 120);
    println!("R16 holdout: {count} performances; no newly flipped held identity, lost action, stale hearing, structural target movement or extra false temporal claim");
}
