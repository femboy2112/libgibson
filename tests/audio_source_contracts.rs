//! Source-generation contracts exposed by the consolidated holdout v1 (post-contact falsifiers).
//!
//! Holdout v1 is known data; these tests reproduce each diagnosed mechanism on NEW seeds
//! (77_100_0xx) outside its rows. Each one was committed red before its repair.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_with_profile, Composition},
    language::MusicalLanguage,
    occupancy,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::Role,
    semantic::{deflected_lift_trace, demo_trace},
    MusicWorld, SongMap,
};

fn song(beats: f64, seed: u64, deflected: bool, grammar: CompositionGrammar) -> SongMap {
    let trace = if deflected {
        deflected_lift_trace(beats)
    } else {
        demo_trace(beats)
    };
    SongMap::compose(&trace, seed, Some(grammar), Composer::StructuralR9)
}

fn pocket(song: &SongMap, world: &MusicWorld, language: MusicalLanguage) -> Composition {
    let opts = PerformanceOptions {
        language,
        ..PerformanceOptions::default()
    };
    perform_with_profile(song, world, opts, PerformanceProfile::POCKET).expect("pocket source")
}

/// Every reservation and every final note of a role lies inside `[0, total]`, and the shared
/// occupancy contract (never loosened) accepts the score.
fn assert_domain(c: &Composition, tag: &str) {
    let total = c.perf.total_beats;
    let v = occupancy::violations(&c.perf, &c.score, true);
    assert!(v.is_empty(), "{tag}: {v:?}");
    for role in [Role::Lead, Role::Bass] {
        for owner in c.score.occupancy.iter().filter(|o| o.role == role) {
            for r in &owner.rhythm {
                assert!(
                    r.beat < total - 1e-9 && r.end_beat <= total + 1e-9,
                    "{tag} {role:?} reservation {}..{} outside 0..{total}",
                    r.beat,
                    r.end_beat
                );
            }
        }
        for n in c.score.role_notes(role) {
            let end = n.start_beat + f64::from(n.dur_beats);
            assert!(
                end <= total + 1e-6,
                "{tag} {role:?} note ends {end} past {total}"
            );
        }
    }
}

const WORLDS: fn() -> [MusicWorld; 3] = || {
    [
        MusicWorld::black_ice(),
        MusicWorld::swiss_signal(),
        MusicWorld::vapor95(),
    ]
};

/// U1: a partial final bar. Holdout v1 failed 16/21 partial-final-bar sources (0/9 bar-aligned)
/// with `invalid reservation for Bass`: the bass planner's last-onset gate ran to the bar line,
/// past the requested end, and the final clip trimmed the note but not the reservation read from it.
#[test]
fn u1_partial_final_bar_reservations_share_the_clipped_domain() {
    for world in WORLDS() {
        for (i, beats) in [4.5, 5.25, 7.5, 9.25, 11.75, 13.0, 15.5, 32.5]
            .into_iter()
            .enumerate()
        {
            for deflected in [false, true] {
                for (language, lang) in [
                    (MusicalLanguage::simple(), "simple"),
                    (MusicalLanguage::fusion_conversation(), "fusion"),
                ] {
                    let s = song(
                        beats,
                        77_100_001 + i as u64,
                        deflected,
                        CompositionGrammar::LoopEvolution,
                    );
                    let c = pocket(&s, &world, language);
                    assert_domain(
                        &c,
                        &format!("{} {beats} deflected={deflected} {lang}", world.name),
                    );
                }
            }
        }
    }
}

/// Bar-aligned control: the same law on pieces whose last bar is complete.
#[test]
fn u1_bar_aligned_control_keeps_the_same_law() {
    for world in WORLDS() {
        for beats in [8.0, 16.0, 28.0] {
            let s = song(beats, 77_100_011, true, CompositionGrammar::HookArc);
            assert_domain(
                &pocket(&s, &world, MusicalLanguage::fusion_conversation()),
                &format!("{} {beats}", world.name),
            );
        }
    }
}

/// Pieces shorter than one full bar never outlast themselves either.
#[test]
fn u1_pieces_shorter_than_one_bar() {
    for world in WORLDS() {
        for beats in [2.0, 3.0, 3.75] {
            let s = song(beats, 77_100_012, false, CompositionGrammar::HookArc);
            assert_domain(
                &pocket(&s, &world, MusicalLanguage::fusion_conversation()),
                &format!("{} {beats}", world.name),
            );
        }
    }
}

/// A phrase-expressed (retimed) predecessor reaches the unison reservation rewrite
/// (`AuthoredOccupancy::from_bass` with expression decisions); it too stays inside the piece.
#[test]
fn u1_phrase_retimed_predecessor_stays_in_domain() {
    for beats in [7.5, 9.25, 15.5] {
        let s = song(beats, 77_100_013, true, CompositionGrammar::DeflectedLift);
        let c = perform_with_profile(
            &s,
            &MusicWorld::black_ice(),
            PerformanceOptions::default(),
            PerformanceProfile::PHRASED,
        )
        .expect("phrased source");
        assert_domain(&c, &format!("phrased {beats}"));
    }
}

/// The observer is not loosened: a reservation one epsilon past the end is still rejected, and
/// one ending exactly at the end is lawful.
#[test]
fn u1_the_checker_still_rejects_a_reservation_past_the_end() {
    let s = song(16.0, 77_100_014, false, CompositionGrammar::HookArc);
    let c = pocket(
        &s,
        &MusicWorld::black_ice(),
        MusicalLanguage::fusion_conversation(),
    );
    let total = c.perf.total_beats;
    let mut score = c.score.clone();
    let bass = score
        .occupancy
        .iter_mut()
        .find(|o| o.role == Role::Bass)
        .expect("bass owner");
    let last = bass.rhythm.last_mut().expect("bass sounds");
    last.end_beat = total + 1e-3;
    assert!(occupancy::violations(&c.perf, &score, true)
        .iter()
        .any(|s| s == "invalid reservation for Bass"));
    let bass = score
        .occupancy
        .iter_mut()
        .find(|o| o.role == Role::Bass)
        .unwrap();
    bass.rhythm.last_mut().unwrap().end_beat = total;
    assert!(!occupancy::violations(&c.perf, &score, true)
        .iter()
        .any(|s| s.starts_with("invalid reservation")));
}
