//! Cover-side contracts exposed by the consolidated holdout v1, reproduced on fresh seeds
//! (77_500_0xx) outside its rows. Lawful refusals stay refusals; these are the structural bugs.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    cover::{
        cover_candidate, CoverAxis, CoverConformance, CoverError, CoverMap, CoverSpec, CoverTarget,
    },
    functor::{perform_with_profile, Composition},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::Role,
    semantic::{deflected_lift_trace, demo_trace},
    song::SongMapConformance,
    MusicWorld, SongMap,
};

fn source(
    beats: f64,
    seed: u64,
    deflected: bool,
    grammar: CompositionGrammar,
) -> (SongMap, Composition) {
    let trace = if deflected {
        deflected_lift_trace(beats)
    } else {
        demo_trace(beats)
    };
    let song = SongMap::compose(&trace, seed, Some(grammar), Composer::StructuralR9);
    let c = perform_with_profile(
        &song,
        &MusicWorld::swiss_signal(),
        PerformanceOptions::default(),
        PerformanceProfile::POCKET,
    )
    .expect("pocket source");
    (song, c)
}

fn target(world: &MusicWorld, seed: u64, grammar: CompositionGrammar) -> CoverTarget<'_> {
    CoverTarget {
        world,
        seed,
        grammar,
        options: PerformanceOptions::default(),
        profile: PerformanceProfile::POCKET,
    }
}

/// G17: a Riff pinned on the bass lane is re-extracted from the cover on the SAME lane. Before,
/// conformance re-derived the lane from whatever the cover happened to sound (a freely generated
/// lead) and failed a correctly performed bass riff.
#[test]
fn riff_lane_is_the_pinned_lane_not_the_covers_heuristic() {
    let world = MusicWorld::swiss_signal();
    let mut checked = 0;
    for seed in 77_500_001..77_500_011u64 {
        let (_, c) = source(16.0, seed, true, CompositionGrammar::RiffDrive);
        let Ok(bass) = CoverMap::extract(&c, &world, CoverSpec::new([CoverAxis::BassFigure]))
        else {
            continue;
        };
        // The same observed bass line, pinned as the song's riff on its own lane.
        let mut map = bass.clone();
        map.spec = CoverSpec::new([CoverAxis::Riff]);
        map.riff = map.bass.take();
        map.validate().expect("a bass-lane riff map is well formed");
        let Ok(cover) = cover_candidate(
            &map,
            target(&world, seed + 1000, CompositionGrammar::HookArc),
        ) else {
            continue;
        };
        if cover.score.role_notes(Role::Lead).next().is_none() {
            continue; // not discriminating: the cover sounds no lead to be confused with
        }
        checked += 1;
        let conformance = CoverConformance::check(&map, &cover, &world);
        assert!(
            conformance.passes(),
            "seed {seed}: {}",
            conformance.report()
        );
    }
    assert!(
        checked > 0,
        "the sweep contains a bass-lane riff whose cover sounds a lead"
    );
}

/// G23: a map pinning Groove and Orchestration whose strokes fall in bars whose pinned drum seat
/// is Silent (the source sounded them through an action window) is contradictory. It must be
/// refused at extraction — never lifted and then have its pinned strokes dropped.
#[test]
fn groove_strokes_in_silent_pinned_seats_are_refused_not_dropped() {
    let world = MusicWorld::swiss_signal();
    let mut refused = 0;
    for seed in 77_500_101..77_500_121u64 {
        for (beats, deflected, grammar) in [
            (16.0, true, CompositionGrammar::PropulsiveReturn),
            (28.0, false, CompositionGrammar::PropulsiveReturn),
            (28.0, true, CompositionGrammar::HookArc),
            (32.5, true, CompositionGrammar::DeflectedLift),
        ] {
            let (_, c) = source(beats, seed, deflected, grammar);
            match CoverMap::extract(
                &c,
                &world,
                CoverSpec::new([CoverAxis::Groove, CoverAxis::Orchestration]),
            ) {
                Err(CoverError::ConflictingPins) => refused += 1,
                Err(_) => {}
                Ok(map) => {
                    let cover =
                        cover_candidate(&map, target(&world, seed, CompositionGrammar::HookArc))
                            .expect("consistent groove/orchestration cover");
                    let conformance = CoverConformance::check(&map, &cover, &world);
                    assert!(
                        conformance.passes(),
                        "seed {seed} {beats}: pinned strokes lost\n{}",
                        conformance.report()
                    );
                }
            }
        }
    }
    assert!(
        refused > 0,
        "the sweep contains a contradictory groove/seat pin"
    );
}

/// G14/G24: a cover whose pinned line constrains its harmony (no pinned chart) plays that
/// constrained harmony, so its song must not also claim the target grammar's generated chart:
/// one harmony authority. Before, a PropulsiveReturn target audited a chart it never played.
#[test]
fn a_line_constrained_cover_does_not_claim_an_unplayed_chart() {
    let world = MusicWorld::black_ice();
    for seed in 77_500_201..77_500_211u64 {
        let (_, c) = source(20.0, seed, true, CompositionGrammar::HookArc);
        let Ok(map) = CoverMap::extract(
            &c,
            &MusicWorld::swiss_signal(),
            CoverSpec::new([CoverAxis::Motif]),
        ) else {
            continue;
        };
        for grammar in [
            CompositionGrammar::PropulsiveReturn,
            CompositionGrammar::DeflectedLift,
        ] {
            let Ok(cover) = cover_candidate(&map, target(&world, seed, grammar)) else {
                continue; // an explicit lift refusal is lawful
            };
            let song = SongMapConformance::check(&cover.song, &cover.perf, &cover.score);
            assert!(
                song.illegal_harmonic_transforms.is_empty(),
                "seed {seed} {grammar:?}: {:?}",
                song.illegal_harmonic_transforms
            );
        }
    }
}
