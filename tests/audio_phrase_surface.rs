//! Round XVI falsifiers: physical subordination alone does not establish phrase morphology.
use gibson::audio::human_music::{
    composer::Composer,
    expression::{ConnectiveViability, ExpressionDiagnostics},
    functor::perform_expressive,
    performance::PerformanceOptions,
    phrase_diagnostics::{
        grid_at, occupied_beats, temporal_shape, PhraseSurfaceDiagnostics, SurfaceGrid,
    },
    score::Role,
    semantic::deflected_lift_trace,
    MusicWorld, SongMap,
};

fn baseline() -> gibson::audio::human_music::functor::Composition {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    perform_expressive(
        &song,
        &MusicWorld::black_ice(),
        PerformanceOptions::default(),
    )
}

#[test]
fn frozen_r15_individually_viable_connectives_repeat_one_temporal_shape() {
    let c = baseline();
    let w = MusicWorld::black_ice();
    let local = ExpressionDiagnostics::measure(&c.perf, &c.score, &w);
    assert!(local
        .rows
        .iter()
        .filter(|r| r.note.role == Role::Lead)
        .all(|r| r.verdict == ConnectiveViability::AsWritten));
    let d = PhraseSurfaceDiagnostics::measure(&c.song.plan, &c.perf, &c.score, &w);
    let first_bass = d
        .roles
        .iter()
        .filter(|r| r.role == Role::Bass)
        .flat_map(|r| &r.notes)
        .next()
        .unwrap();
    assert_eq!(first_bass.note.start_beat, 0.0);
    assert_eq!(first_bass.silence_before_secs, 0.0);
    let shapes = d.shapes_for(Role::Lead);
    assert_eq!(
        shapes.iter().map(|s| s.source_onsets.len()).sum::<usize>(),
        17
    );
    assert!(shapes.iter().any(|s| s.source_onsets.len() >= 16));
    let isolated: usize = d
        .roles
        .iter()
        .filter(|r| r.role == Role::Lead)
        .map(|r| r.isolated_grace_onsets.len())
        .sum();
    println!("R15 lead repeated_shape={shapes:?} isolated_graces={isolated}");
    assert!(isolated >= 8);
    let offgrid = d
        .roles
        .iter()
        .filter(|r| r.role == Role::Lead)
        .flat_map(|r| &r.notes)
        .filter(|n| {
            !n.structural && n.source_onset != n.note.start_beat && n.grid == SurfaceGrid::OffGrid
        })
        .count();
    assert_eq!(offgrid, 17);
}

#[test]
fn r16_acceptance_requires_more_than_repeated_individually_legal_graces() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let c = gibson::audio::human_music::functor::perform_phrased(
        &song,
        &MusicWorld::black_ice(),
        PerformanceOptions::default(),
    );
    let d = PhraseSurfaceDiagnostics::measure(
        &c.song.plan,
        &c.perf,
        &c.score,
        &MusicWorld::black_ice(),
    );
    let shapes = d.shapes_for(Role::Lead);
    let total: usize = shapes.iter().map(|s| s.source_onsets.len()).sum();
    let repeated = shapes
        .iter()
        .map(|s| s.source_onsets.len())
        .max()
        .unwrap_or(0);
    // Frozen R15 is intentionally red here. This checks a concrete human-calibrated failure,
    // not a universal ban on repetition, nor a standalone certification of musical quality.
    let isolated: usize = d
        .roles
        .iter()
        .filter(|r| r.role == Role::Lead)
        .map(|r| r.isolated_grace_onsets.len())
        .sum();
    assert!(total > 0 && !(repeated * 4 > total * 3 && isolated * 2 >= total), "R15 repeated-grace witness: {repeated}/{total} optional connectives share one temporal shape, {isolated} have long acoustic space then tiny grace then target");
}

#[test]
fn ruler_distinguishes_real_subdivision_changes_from_float_jitter() {
    assert_eq!(temporal_shape(0.1675, 0.045), temporal_shape(0.1681, 0.049));
    assert_ne!(temporal_shape(0.1675, 0.045), temporal_shape(0.250, 0.090));
    assert_eq!(grid_at(25.25), SurfaceGrid::Sixteenth);
    assert_eq!(grid_at(25.253445), SurfaceGrid::OffGrid);
    assert_eq!(grid_at(25.0 + 1.0 / 3.0), SurfaceGrid::Triplet);
}

#[test]
fn keys_occupation_is_union_not_polyphonic_note_count() {
    assert_eq!(occupied_beats(&[], 0.0, 4.0), 0.0);
    assert_eq!(
        occupied_beats(&[(0.0, 2.0), (0.0, 2.0), (1.0, 3.0)], 0.0, 4.0),
        3.0
    );
    assert_eq!(occupied_beats(&[(-2.0, 1.0), (3.0, 8.0)], 0.0, 4.0), 2.0);
}

#[test]
fn empty_surface_is_not_a_repetition_failure() {
    let mut c = baseline();
    c.score.notes.clear();
    c.score.expression_decisions.clear();
    let d = PhraseSurfaceDiagnostics::measure(
        &c.song.plan,
        &c.perf,
        &c.score,
        &MusicWorld::black_ice(),
    );
    assert!(d.shapes_for(Role::Lead).is_empty());
    assert!(d
        .roles
        .iter()
        .all(|r| r.attacks_per_beat == 0.0 && r.audible_occupied_beats == 0.0));
    assert!(d.contacts.iter().all(|r| r.synchronized_onsets.is_empty()));
}

#[test]
fn degenerate_space_grace_target_is_distinct_from_legitimate_repeating_eighths() {
    use gibson::audio::human_music::phrase_diagnostics::isolated_grace;
    // Identical pitches/strategy names are unnecessary: the observed temporal geometry is key.
    assert!(isolated_grace(0.45, 0.045, 0.17));
    assert!(isolated_grace(0.90, 0.050, 0.168));
    // Ordinary repeating eighths can have one repeated shape without this pathology.
    assert!(!isolated_grace(0.01, 0.20, 0.25));
    assert!(!isolated_grace(0.45, 0.20, 0.25));
    assert!(!isolated_grace(0.01, 0.045, 0.17));
}

#[test]
fn phrase_boundary_does_not_hide_bass_target_relationships() {
    let c = baseline();
    let d = PhraseSurfaceDiagnostics::measure(
        &c.song.plan,
        &c.perf,
        &c.score,
        &MusicWorld::black_ice(),
    );
    let shapes = d.shapes_for(Role::Bass);
    assert_eq!(
        shapes.iter().map(|s| s.source_onsets.len()).sum::<usize>(),
        18
    );
    for onset in [15.5, 63.5, 79.5] {
        assert!(shapes.iter().any(|s| s.source_onsets.contains(&onset)));
    }
}
