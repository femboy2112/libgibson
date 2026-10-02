//! Hostile read-only timing witnesses: being on a lattice is weaker than keeping metric phase,
//! and a stale plan/ledger must never impersonate the actual performed notes.
use gibson::audio::human_music::{
    composer::Composer,
    functor::perform_phrased,
    performance::PerformanceOptions,
    pocket::slots,
    pocket_diagnostics::{metric_phase_deviation, PocketDiagnostics},
    semantic::deflected_lift_trace,
    MusicWorld, SongMap,
};

#[test]
fn on_lattice_shift_can_change_metric_phase_and_whole_beat_motion_is_separate() {
    let world = MusicWorld::black_ice();
    let lattice = slots(&world, 4, 0.0, 2.0);
    assert!(lattice.iter().any(|s| s.position == 0.5));
    assert!(lattice.iter().any(|s| s.position == 0.75));
    assert_eq!(metric_phase_deviation(0.5, 0.75), 0.25);
    assert_eq!(metric_phase_deviation(0.75, 0.5), -0.25);
    // Phase alone cannot witness the full-beat rewrite of the restatement rhythm.
    assert_eq!(metric_phase_deviation(26.5, 27.5), 0.0);
    assert_eq!(27.5 - 26.5, 1.0);
}

#[test]
fn frozen_r16_has_nonzero_lattice_residual_without_erasing_source_motion() {
    let world = MusicWorld::black_ice();
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let c = perform_phrased(&song, &world, PerformanceOptions::default());
    let evidence = PocketDiagnostics::measure(&c.perf, &c.score, &world);
    let notes: Vec<_> = evidence.phrases.iter().flat_map(|p| &p.notes).collect();
    assert!(
        notes
            .iter()
            .all(|n| !n.ledger_status.starts_with("unresolved")),
        "frozen source ledger must align with actual R16 notes"
    );
    assert!(notes.iter().any(|n| n
        .performed_coordinate
        .as_ref()
        .is_some_and(|p| p.residual_beats.abs() > 0.1)));
    assert!(notes
        .iter()
        .any(|n| n.signed_displacement_beats.is_some_and(|d| d >= 1.0)));
    assert!(evidence
        .phrases
        .iter()
        .any(|p| p.source_timeline.iois != p.performed_timeline.iois));
    assert!(evidence
        .phrases
        .iter()
        .any(|p| p.source_timeline.events.iter().any(|e| e.structural)));
}

#[test]
fn stale_ledger_cannot_supply_a_performed_note_missing_from_actual_score() {
    let world = MusicWorld::black_ice();
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let mut c = perform_phrased(&song, &world, PerformanceOptions::default());
    let before = PocketDiagnostics::measure(&c.perf, &c.score, &world);
    let (phrase_index, note_index, score_index) = before
        .phrases
        .iter()
        .enumerate()
        .find_map(|(pi, p)| {
            p.notes
                .iter()
                .enumerate()
                .find_map(|(ni, n)| n.score_index.map(|si| (pi, ni, si)))
        })
        .expect("frozen R16 has a surviving transformed note");
    // Corrupt the actual consumer, leaving the plan and its expression receipt unchanged.
    c.score.notes[score_index].start_beat += 0.0625;
    let after = PocketDiagnostics::measure(&c.perf, &c.score, &world);
    let row = &after.phrases[phrase_index].notes[note_index];
    assert_eq!(row.ledger_status, "unresolved-no-actual-note");
    assert!(row.score_index.is_none() && row.performed.is_none());
    assert!(row.performed_coordinate.is_none() && row.signed_displacement_beats.is_none());

    // Duplicate event identities are unresolved too; taking the first would invent a match.
    c.score.notes[score_index].start_beat -= 0.0625;
    c.score.notes.push(c.score.notes[score_index]);
    let duplicate = PocketDiagnostics::measure(&c.perf, &c.score, &world);
    let row = &duplicate.phrases[phrase_index].notes[note_index];
    assert_eq!(row.actual_match_count, 2);
    assert_eq!(row.ledger_status, "unresolved-ambiguous-actual-notes");
    assert!(row.score_index.is_none() && row.performed.is_none());
}
