//! Round XV human-calibration falsifiers, committed before the production ruler.
use gibson::audio::human_music::{
    composer::Composer,
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
    assert!(
        life(n, &w) <= 0.20,
        "slow neighbor sounds for {:.6} s",
        life(n, &w)
    );
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
    assert!(
        life(n, &w) <= 0.33,
        "bass approach sounds for {:.6} s",
        life(n, &w)
    );
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
