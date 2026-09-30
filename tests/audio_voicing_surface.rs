//! Round XVI: the chart can remain identifiable while its realized register/path is suspect.
use gibson::audio::human_music::{
    composer::Composer,
    expression::{observe, ConnectiveViability, ExpressionEvent},
    functor::{perform_expressive, Composition},
    identity::IdentityDiagnostics,
    performance::PerformanceOptions,
    score::{PitchFunction, Role},
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

#[test]
fn r16_resolving_bass_tradeoff_has_frozen_within_song_controls_and_tempo_limit() {
    let (w, x) = frozen();
    let d = VoicingSurfaceDiagnostics::measure(&x.perf, &x.score, &w);
    let bass: Vec<_> = x.score.role_notes(Role::Bass).collect();
    let pad: Vec<_> = x.score.role_notes(Role::Pad).copied().collect();
    // These accepted within-song controls already contain C4 versus B2→C3. They share
    // provenance and are not independent human trials; they calibrate only this machine rule.
    for (context_beat, bass_beat) in [(4.0, 7.0), (108.0, 111.0)] {
        let row = d.rows.iter().find(|r| r.beat == context_beat).unwrap();
        let contact = row
            .contacts
            .iter()
            .find(|c| {
                c.pad_pitch == 60
                    && c.other_pitch == 47
                    && c.other_onset_beat == bass_beat
                    && c.other_role == Role::Bass
                    && !c.other_is_sfx
            })
            .unwrap();
        assert!((contact.overlap_seconds - 0.312741454).abs() < 1e-6);
        let i = bass.iter().position(|n| n.start_beat == bass_beat).unwrap();
        let event = ExpressionEvent {
            note: *bass[i],
            structural: true,
        };
        let evidence = observe(
            &x.perf,
            &w,
            &event,
            Some(bass[i - 1]),
            Some(bass[i + 1]),
            &pad,
        );
        assert_eq!(event.note.function, Some(PitchFunction::ChordTone));
        assert_eq!(evidence.verdict, ConnectiveViability::AsWritten);
        let mut slow = w.clone();
        slow.tempo_bpm = 88.0;
        let exposed = observe(
            &x.perf,
            &slow,
            &event,
            Some(bass[i - 1]),
            Some(bass[i + 1]),
            &pad,
        );
        assert_ne!(exposed.verdict, ConnectiveViability::AsWritten);
    }
    let mut slow = w.clone();
    slow.tempo_bpm = 88.0;
    let band: Vec<_> = x
        .score
        .notes
        .iter()
        .filter(|n| n.role != Role::Pad)
        .copied()
        .collect();
    let (_, _, decisions) =
        gibson::audio::human_music::comp::realize_pad_phrased(&x.perf, &x.song.plan, &slow, &band);
    assert!(
        !decisions.iter().any(|d| d.start_beat == 28.0),
        "slower bass cannot borrow SWISS's accepted physical performance"
    );
    let mut unresolved_band = band;
    unresolved_band
        .iter_mut()
        .find(|n| n.role == Role::Bass && n.start_beat == 31.5)
        .unwrap()
        .pitch = 47;
    let (_, _, decisions) = gibson::audio::human_music::comp::realize_pad_phrased(
        &x.perf,
        &x.song.plan,
        &w,
        &unresolved_band,
    );
    assert!(
        !decisions.iter().any(|d| d.start_beat == 28.0),
        "a repeated seventh cannot borrow the later root: the immediate target must resolve"
    );
}
