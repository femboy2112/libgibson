//! Round XVI: silence in a player's gesture does not automatically invite a comping stab.
use gibson::audio::human_music::{
    comp::realize_keys_owned,
    composer::Composer,
    functor::{perform_coherent, perform_expressive, perform_phrase_experiment, PhraseOptions},
    groove::realize_drums_owned,
    occupancy::{AuthoredOccupancy, Opportunity, OwnershipKind},
    performance::PerformanceOptions,
    score::Role,
    semantic::deflected_lift_trace,
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

#[test]
fn r16_frozen_r15_micro_holes_are_not_authored_opportunities() {
    let world = MusicWorld::black_ice();
    let song = song();
    let before = perform_coherent(&song, &world, PerformanceOptions::default());
    let after = perform_expressive(&song, &world, PerformanceOptions::default());
    let original: Vec<_> = before.score.role_notes(Role::Lead).copied().collect();
    let final_lead: Vec<_> = after.score.role_notes(Role::Lead).copied().collect();
    let occupancy = AuthoredOccupancy::from_lead(&before.perf, &original);
    for beat in [27.5, 59.5, 96.5, 99.5] {
        assert!(!before
            .score
            .role_notes(Role::Keys)
            .any(|n| { n.start_beat == beat && n.prov.role_note == "comp" }));
        // Known-red R15 consumer behavior, preserved as a falsifier of acoustic-gap inference.
        assert!(after
            .score
            .role_notes(Role::Keys)
            .any(|n| { n.start_beat == beat && n.prov.role_note == "comp" }));
        assert_eq!(occupancy.opportunity(beat), Opportunity::ReservedAttack);
        assert!(!occupancy.allows_comp_at(beat, &final_lead));
    }
    // A previously free subdivision can also become acoustically unavailable. Intent does not
    // license a collision with the final grace, and is never used as a substitute hearing.
    assert!(occupancy.allows_comp_at(27.75, &original));
    assert!(!occupancy.allows_comp_at(27.75, &final_lead));
}

#[test]
fn r16_retiming_or_omitting_a_sound_does_not_transfer_ownership() {
    let c = perform_coherent(
        &song(),
        &MusicWorld::black_ice(),
        PerformanceOptions::default(),
    );
    let source: Vec<_> = c.score.role_notes(Role::Lead).copied().collect();
    let ownership = AuthoredOccupancy::from_lead(&c.perf, &source);
    let mut final_notes = source.clone();
    final_notes.retain(|n| n.start_beat != 24.5);
    assert!(!ownership.allows_comp_at(24.5, &final_notes));
    assert!(!ownership.yields_at(24.9));
    assert_eq!(ownership.opportunity(26.25), Opportunity::PhraseOwned);
    assert!(ownership.allows_comp_at(26.25, &source));
}

#[test]
fn r16_authored_internal_rests_and_real_yields_are_distinct() {
    let c = perform_coherent(
        &song(),
        &MusicWorld::black_ice(),
        PerformanceOptions::default(),
    );
    let source: Vec<_> = c.score.role_notes(Role::Lead).copied().collect();
    let ownership = AuthoredOccupancy::from_lead(&c.perf, &source);
    let rest = ownership
        .spans
        .iter()
        .find(|s| {
            s.kind == OwnershipKind::InternalRest
                && ownership.opportunity((s.start + s.end) * 0.5) == Opportunity::InternalRest
        })
        .expect("fixture contains a source-authored breath");
    let at = (rest.start + rest.end) * 0.5;
    assert!(!ownership.yields_at(at));
    // Existing accompaniment may coexist; the lead's rest is not a whole-band break.
    assert!(ownership.allows_comp_at(at, &[]));
    // The lead's absence at the opening is a real opportunity, not an invented owner.
    assert!(ownership.yields_at(0.0));
    assert!(ownership.allows_comp_at(0.0, &source));
}

#[test]
fn r16_null_source_has_no_phantom_phrase_ownership() {
    let c = perform_coherent(
        &song(),
        &MusicWorld::swiss_signal(),
        PerformanceOptions::default(),
    );
    let ownership = AuthoredOccupancy::from_lead(&c.perf, &[]);
    assert!(ownership.rhythm.is_empty() && ownership.spans.is_empty());
    assert!(ownership.yields_at(24.5));
    assert!(ownership.allows_comp_at(24.5, &[]));
}

#[test]
fn r16_swiss_existing_comp_opportunities_remain_legal() {
    let c = perform_coherent(
        &song(),
        &MusicWorld::swiss_signal(),
        PerformanceOptions::default(),
    );
    let source: Vec<_> = c.score.role_notes(Role::Lead).copied().collect();
    let ownership = AuthoredOccupancy::from_lead(&c.perf, &source);
    for n in c
        .score
        .role_notes(Role::Keys)
        .filter(|n| n.prov.role_note == "comp")
    {
        assert!(
            ownership.allows_comp_at(n.start_beat, &source),
            "accepted Swiss comp at {} became {:?}",
            n.start_beat,
            ownership.opportunity(n.start_beat)
        );
    }
}

#[test]
fn r16_semantic_bass_pulses_are_not_retimed_acoustic_attacks() {
    let world = MusicWorld::black_ice();
    let song = song();
    let c = perform_coherent(&song, &world, PerformanceOptions::default());
    let x = perform_expressive(&song, &world, PerformanceOptions::default());
    let source: Vec<_> = c.score.role_notes(Role::Bass).copied().collect();
    let ownership = AuthoredOccupancy::from_role(&c.perf, &source, Role::Bass);
    assert!(ownership.rhythm_steps(0, false).contains(&14));
    assert!(!ownership.rhythm_steps(0, true).contains(&14));
    assert!(x.score.role_notes(Role::Bass).all(|n| n.start_beat != 3.5));
    assert!(x.score.stale_hearings().is_empty());
}

#[test]
fn r16_owned_keys_reject_micro_holes_at_source() {
    let song = song();
    let world = MusicWorld::black_ice();
    let before = perform_coherent(&song, &world, PerformanceOptions::default());
    let after = perform_expressive(&song, &world, PerformanceOptions::default());
    let source: Vec<_> = before.score.role_notes(Role::Lead).copied().collect();
    let acoustic: Vec<_> = after.score.role_notes(Role::Lead).copied().collect();
    let ownership = AuthoredOccupancy::from_lead(&before.perf, &source);
    let keys = realize_keys_owned(
        &after.perf,
        &song.plan,
        &world,
        &acoustic,
        &ownership,
        song.seed,
    );
    for beat in [27.5, 59.5, 96.5, 99.5] {
        assert!(!keys
            .iter()
            .any(|n| n.start_beat == beat && n.prov.role_note == "comp"));
    }
    let original_comp: Vec<_> = before
        .score
        .role_notes(Role::Keys)
        .filter(|n| n.prov.role_note == "comp")
        .copied()
        .collect();
    let final_comp: Vec<_> = keys
        .iter()
        .filter(|n| n.prov.role_note == "comp")
        .copied()
        .collect();
    // Final score provenance is enriched later; compare the actual accompaniment geometry.
    let geometry = |notes: &[gibson::audio::human_music::score::Note]| {
        notes
            .iter()
            .map(|n| (n.start_beat, n.dur_beats, n.pitch, n.velocity))
            .collect::<Vec<_>>()
    };
    assert_eq!(geometry(&original_comp), geometry(&final_comp));
}

#[test]
fn r16_owned_drums_preserve_pocket_under_bass_retiming() {
    let song = song();
    let world = MusicWorld::black_ice();
    let before = perform_coherent(&song, &world, PerformanceOptions::default());
    let after = perform_expressive(&song, &world, PerformanceOptions::default());
    let source: Vec<_> = before.score.role_notes(Role::Bass).copied().collect();
    let lead: Vec<_> = before.score.role_notes(Role::Lead).copied().collect();
    let bass: Vec<_> = after.score.role_notes(Role::Bass).copied().collect();
    let ownership = AuthoredOccupancy::from_role(&before.perf, &source, Role::Bass);
    let original = realize_drums_owned(
        &before.perf,
        &song.plan,
        &world,
        song.seed,
        &source,
        &lead,
        &ownership,
    );
    let retimed = realize_drums_owned(
        &before.perf,
        &song.plan,
        &world,
        song.seed,
        &bass,
        &lead,
        &ownership,
    );
    assert_eq!(format!("{original:?}"), format!("{retimed:?}"));
}

#[test]
fn r16_phrase_and_ownership_factors_are_independent_and_final_hearings_are_current() {
    let song = song();
    let world = MusicWorld::black_ice();
    let legacy = perform_expressive(&song, &world, PerformanceOptions::default());
    let mut cells = Vec::new();
    for phrase_expression in [false, true] {
        for semantic_occupancy in [false, true] {
            let cell = perform_phrase_experiment(
                &song,
                &world,
                PerformanceOptions::default(),
                PhraseOptions {
                    phrase_expression,
                    semantic_occupancy,
                    support_voicing: false,
                },
            );
            assert!(cell.score.stale_hearings().is_empty());
            if semantic_occupancy {
                let intent = cell
                    .score
                    .occupancy
                    .iter()
                    .find(|o| o.role == Role::Lead)
                    .expect("source intent recorded independently of heard notes");
                let lead: Vec<_> = cell.score.role_notes(Role::Lead).copied().collect();
                for n in cell
                    .score
                    .role_notes(Role::Keys)
                    .filter(|n| n.prov.role_note == "comp")
                {
                    assert!(intent.allows_comp_at(n.start_beat, &lead));
                }
            }
            let comp_attacks: std::collections::BTreeSet<_> = cell
                .score
                .role_notes(Role::Keys)
                .filter(|n| n.prov.role_note == "comp")
                .map(|n| (n.start_beat * 1000.0).round() as i64)
                .collect();
            eprintln!(
                "phrase={phrase_expression} ownership={semantic_occupancy} comp_attacks={} stale=0",
                comp_attacks.len()
            );
            cells.push(cell);
        }
    }
    assert_eq!(
        format!("{:?}", cells[0].score.notes),
        format!("{:?}", legacy.score.notes)
    );
    assert_eq!(
        format!("{:?}", cells[0].score.drums),
        format!("{:?}", legacy.score.drums)
    );
    // Ownership alone cannot rewrite the lead's physical performance.
    let lead = |i: usize| {
        cells[i]
            .score
            .role_notes(Role::Lead)
            .copied()
            .collect::<Vec<_>>()
    };
    assert_eq!(format!("{:?}", lead(0)), format!("{:?}", lead(1)));
    assert_eq!(format!("{:?}", lead(2)), format!("{:?}", lead(3)));
}
