//! Round XVI: the chart can remain identifiable while its realized register/path is suspect.
use gibson::audio::human_music::{
    composer::Composer,
    functor::{perform_expressive, Composition},
    identity::IdentityDiagnostics,
    performance::PerformanceOptions,
    score::Role,
    semantic::deflected_lift_trace,
    voicing_diagnostics::VoicingSurfaceDiagnostics,
    MusicWorld, SongMap,
};

fn frozen() -> (MusicWorld, Composition) {
    let world = MusicWorld::swiss_signal();
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let x = perform_expressive(&song, &world, PerformanceOptions::default());
    (world, x)
}

#[test]
fn r16_frozen_identity_passes_but_high_seventh_excursion_distinguishes_sites() {
    let (w, x) = frozen();
    let identity = IdentityDiagnostics::measure(&x.score.notes, &x.perf.contexts, &w, w.tempo_bpm);
    assert_eq!(identity.flips().count(), 0);
    let d = VoicingSurfaceDiagnostics::measure(&x.perf, &x.score, &w);
    for beat in [28.0, 36.0, 44.0] {
        let r = d.rows.iter().find(|r| r.beat == beat).unwrap();
        assert_eq!(r.pad, [64, 72, 79, 83]);
        assert_eq!(r.root_pitches, [72]);
        assert_eq!(r.guide_pitches, [64, 83]);
        assert_eq!(r.pad_inversion, Some(4));
        assert_eq!(r.span, 19);
        assert_eq!(r.register_center, Some(74.5));
        assert_eq!(r.top_incoming_semitones, Some(4));
        assert_eq!(r.top_outgoing_semitones, Some(-4));
        assert!(r.top_audible_seconds > 2.2);
        assert!(r.high_seventh_excursion);
        // The seventh/root major seventh is sustained and co-attacked; the old hard-cluster
        // detector's absence of a minor second/ninth is not a perceptual success certificate.
        assert!(r.contacts.iter().any(|c| c.other_role == Role::Pad
            && c.semitones == 11
            && c.overlap_seconds > 1.9
            && c.coincident_attack));
        assert!(r.doubled_pitch_classes.is_empty());
    }
    for beat in [4.0, 12.0, 60.0, 68.0, 76.0, 100.0, 108.0] {
        let r = d.rows.iter().find(|r| r.beat == beat).unwrap();
        assert_eq!(r.pad, [60, 67, 71, 76]);
        assert_eq!(r.root_pitches, [60]);
        assert_eq!(r.guide_pitches, [71, 76]);
        assert_eq!(r.pad_inversion, Some(0));
        assert_eq!(r.register_center, Some(68.5));
        assert!(!r.high_seventh_excursion);
        assert!(r
            .contacts
            .iter()
            .any(|c| c.other_role == Role::Pad && c.semitones == 11));
    }
}

#[test]
fn r16_a_changed_voicing_is_observed_without_changing_chord_identity() {
    let (w, mut x) = frozen();
    // Diagnostic counterfactual only: same pitch classes, all four voices, same attacks/gates.
    // Production must select this kind of alternative BEFORE note emission.
    for n in x
        .score
        .notes
        .iter_mut()
        .filter(|n| n.role == Role::Pad && [28.0, 36.0, 44.0].contains(&n.start_beat))
    {
        if n.pitch == 72 || n.pitch == 83 {
            n.pitch -= 12;
        }
    }
    let d = VoicingSurfaceDiagnostics::measure(&x.perf, &x.score, &w);
    for beat in [28.0, 36.0, 44.0] {
        let r = d.rows.iter().find(|r| r.beat == beat).unwrap();
        assert_eq!(r.pad, [60, 64, 71, 79]);
        assert_eq!(r.top_incoming_semitones, Some(0));
        assert_eq!(r.top_outgoing_semitones, Some(0));
        assert!(!r.high_seventh_excursion);
        assert!(r
            .contacts
            .iter()
            .filter(|c| c.other_role == Role::Pad && c.coincident_attack)
            .all(|c| c.semitones != 1 && c.semitones != 13));
        // Prior C5's release overlaps the incoming B4 briefly; report it separately rather
        // than pretending absence of a within-voicing cluster means no audible contact.
        assert!(r
            .contacts
            .iter()
            .filter(|c| c.other_role == Role::Pad && matches!(c.semitones, 1 | 13))
            .all(|c| !c.coincident_attack && c.overlap_seconds < 0.3));
    }
    let identity = IdentityDiagnostics::measure(&x.score.notes, &x.perf.contexts, &w, w.tempo_bpm);
    assert_eq!(identity.flips().count(), 0);
}

#[test]
fn r16_production_should_discharge_the_reported_register_path_witness() {
    let (w, x) = frozen();
    let d = VoicingSurfaceDiagnostics::measure(&x.perf, &x.score, &w);
    let sites: Vec<_> = d
        .rows
        .iter()
        .filter(|r| r.high_seventh_excursion)
        .map(|r| r.beat)
        .collect();
    // Active red witness until the new opt-in realization is wired here. R15 stays frozen.
    assert!(
        sites.is_empty(),
        "identity passes, but reported voicing family survives: {sites:?}"
    );
}
