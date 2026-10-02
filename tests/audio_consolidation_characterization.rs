//! Cheap event/evidence characterization. PCM equivalence is a separate render gate.
use gibson::audio::human_music::{
    composer::Composer,
    functor::{perform_coherent, perform_phrased, perform_pocket_experiment, perform_pocketed},
    performance::PerformanceOptions,
    phrase_diagnostics::PhraseSurfaceDiagnostics,
    pocket::PocketOptions,
    pocket_diagnostics::PocketDiagnostics,
    semantic::deflected_lift_trace,
    song::{SongMap, SongMapConformance},
    witness, MusicWorld,
};

fn frozen(name: &str, suffix: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/docs/fixtures/humanmusic-r17/final/{name}_r17.{suffix}.txt",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn accepted_pocket_events_and_causal_evidence_are_frozen() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    assert_eq!(song.fingerprint(), 0xaf53_6044_2b9b_6729);
    for (name, world, r14, r16, r17) in [
        (
            "black_ice",
            MusicWorld::black_ice(),
            0x5624_62b4_c3c7_e728,
            0x71d6_b642_3a56_0637,
            0x8503_f682_f19f_c982,
        ),
        (
            "swiss",
            MusicWorld::swiss_signal(),
            0xbeb0_8c05_7ed8_7d94,
            0x0381_7af0_45ff_24d6,
            0x0381_7af0_45ff_24d6,
        ),
    ] {
        let opts = PerformanceOptions::default();
        assert_eq!(
            perform_coherent(&song, &world, opts).score.fingerprint(),
            r14
        );
        assert_eq!(
            perform_phrased(&song, &world, opts).score.fingerprint(),
            r16
        );
        let c = if name == "swiss" {
            perform_pocket_experiment(
                &song,
                &world,
                opts,
                PocketOptions {
                    support_top_voice: true,
                    ..PocketOptions::NONE
                },
            )
        } else {
            perform_pocketed(&song, &world, opts)
        };
        assert_eq!(c.score.fingerprint(), r17);
        assert_eq!(
            c.perf.fingerprint(),
            if name == "black_ice" {
                0xdd68_e9d7_339d_aba4
            } else {
                0x1e99_b6d9_458a_b484
            },
            "{name} performance"
        );
        assert!(c.score.stale_hearings().is_empty());
        assert_eq!(c.score.mono_voice, name == "black_ice");
        let score = &c.score;
        for (suffix, actual) in [
            (
                "notes",
                score.notes.iter().map(|n| format!("{n:?}\n")).collect(),
            ),
            (
                "drums",
                score.drums.iter().map(|n| format!("{n:?}\n")).collect(),
            ),
            ("continuity_links", format!("{:#?}", score.voice_continuity)),
            ("phrase_plans", format!("{:#?}", score.phrase_plans)),
            (
                "expression_decisions",
                format!("{:#?}", score.expression_decisions),
            ),
            ("occupancy", format!("{:#?}", score.occupancy)),
            (
                "hearings",
                format!("{:?}\nstale={:?}\n", score.hearings, score.stale_hearings()),
            ),
            (
                "actions",
                format!("{:#?}", witness::audit(&c.perf, score).rows),
            ),
            (
                "pocket",
                PocketDiagnostics::measure(&c.perf, score, &world).report(),
            ),
            (
                "phrase",
                PhraseSurfaceDiagnostics::measure(&song.plan, &c.perf, score, &world).report(),
            ),
            (
                "conformance",
                SongMapConformance::check(&song, &c.perf, score).report(),
            ),
        ] {
            assert_eq!(actual, frozen(name, suffix), "{name}.{suffix}");
        }
    }
}
