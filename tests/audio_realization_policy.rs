//! Policy composition, historical adapters, and separation of source admission from rendering.
use gibson::audio::human_music::{
    composer::Composer,
    functor::{
        perform, perform_coherent, perform_expressive, perform_phrased, perform_pocket_experiment,
        perform_pocketed, perform_temporal, perform_with_profile, realize_with_profile,
        Composition,
    },
    performance::{EnsembleCoupling, PerformanceOptions},
    pocket::PocketOptions,
    policy::{
        ExpressionPolicy, PerformanceProfile, PitchPolicy, PulsePolicy, SourceEvidencePolicy,
        SupportPolicy, VoiceLifetimePolicy,
    },
    score::Role,
    semantic::deflected_lift_trace,
    MusicWorld, SongMap,
};

fn song(beats: f64) -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(beats),
        2112,
        None,
        Composer::StablePropulsion,
    )
}

#[test]
fn explicit_archival_observation_reproduces_historical_adapter_evidence() {
    type Historical = fn(&SongMap, &MusicWorld, PerformanceOptions) -> Composition;
    let cases: [(Historical, PerformanceProfile); 6] = [
        (perform, PerformanceProfile::WRITTEN),
        (perform_temporal, PerformanceProfile::TEMPORAL),
        (perform_coherent, PerformanceProfile::HEARD),
        (perform_expressive, PerformanceProfile::EXPRESSIVE),
        (perform_phrased, PerformanceProfile::PHRASED),
        (perform_pocketed, PerformanceProfile::POCKET),
    ];
    let song = song(48.0);
    let world = MusicWorld::black_ice();
    for (historical, mut profile) in cases {
        if profile.lifetime == VoiceLifetimePolicy::ReleaseEnvelope {
            profile.observation =
                gibson::audio::human_music::voice::ObservedLifetimePolicy::LegacyRoleMasking;
        }
        let old = historical(&song, &world, PerformanceOptions::default());
        let mut new =
            perform_with_profile(&song, &world, PerformanceOptions::default(), profile).unwrap();
        // The declared modern observation field is a deliberate receipt migration. Compare
        // every other event/evidence field against the old implicit compatibility receipt.
        new.score.observed_lifetime = None;
        assert_eq!(format!("{:?}", old.score), format!("{:?}", new.score));
        assert_eq!(old.perf.fingerprint(), new.perf.fingerprint());
        let mut from_plan = realize_with_profile(&song, &world, &new.perf, profile).unwrap();
        from_plan.observed_lifetime = None;
        assert_eq!(format!("{:?}", new.score), format!("{from_plan:?}"));
    }
    assert_eq!(PerformanceProfile::default(), PerformanceProfile::WRITTEN);
}

#[test]
fn changing_only_render_lifetime_preserves_source_phrases_and_semantic_ownership() {
    let song = song(120.0);
    let world = MusicWorld::black_ice();
    let linked = perform_with_profile(
        &song,
        &world,
        PerformanceOptions::default(),
        PerformanceProfile::POCKET,
    )
    .unwrap();
    let mut envelope_profile = PerformanceProfile::POCKET;
    envelope_profile.lifetime = VoiceLifetimePolicy::ReleaseEnvelope;
    let envelope = perform_with_profile(
        &song,
        &world,
        PerformanceOptions::default(),
        envelope_profile,
    )
    .unwrap();
    for role in [Role::Lead, Role::Bass] {
        let a: Vec<_> = linked.score.role_notes(role).collect();
        let b: Vec<_> = envelope.score.role_notes(role).collect();
        assert_eq!(
            format!("{a:?}"),
            format!("{b:?}"),
            "render lifetime changed source {role:?}"
        );
    }
    assert_eq!(
        format!("{:?}", linked.score.phrase_plans),
        format!("{:?}", envelope.score.phrase_plans)
    );
    assert_eq!(linked.score.occupancy, envelope.score.occupancy);
    assert_eq!(
        format!("{:?}", linked.score.expression_decisions),
        format!("{:?}", envelope.score.expression_decisions)
    );
    assert!(!linked.score.voice_continuity.is_empty());
    assert!(envelope.score.voice_continuity.is_empty());
    assert!(linked.score.stale_hearings().is_empty());
    assert!(envelope.score.stale_hearings().is_empty());
}

#[test]
fn unavailable_support_factor_remains_an_explicit_archived_non_treatment() {
    let song = song(48.0);
    let world = MusicWorld::black_ice();
    let on = perform_pocket_experiment(
        &song,
        &world,
        PerformanceOptions::default(),
        PocketOptions::default(),
    );
    let off = perform_pocket_experiment(
        &song,
        &world,
        PerformanceOptions::default(),
        PocketOptions {
            support_top_voice: false,
            ..PocketOptions::default()
        },
    );
    assert_eq!(format!("{:?}", on.score), format!("{:?}", off.score));
    assert_eq!(
        PerformanceProfile::POCKET.expression,
        ExpressionPolicy::Pulse(PulsePolicy::POCKET)
    );
}

#[test]
fn unlawful_profiles_are_rejected_at_one_boundary() {
    let mut written_expression = PerformanceProfile::POCKET;
    written_expression.pitch = PitchPolicy::Written;
    let mut unsupported_voice = PerformanceProfile::POCKET;
    unsupported_voice.support = SupportPolicy::HeardHarmony;
    let mut missing_ownership = PerformanceProfile::POCKET;
    missing_ownership.evidence = SourceEvidencePolicy::Events;
    let mut incompatible_observation = PerformanceProfile::POCKET;
    incompatible_observation.observation =
        gibson::audio::human_music::voice::ObservedLifetimePolicy::LegacyRoleMasking;
    for profile in [
        written_expression,
        unsupported_voice,
        missing_ownership,
        incompatible_observation,
    ] {
        assert!(profile.validate(EnsembleCoupling::Independent).is_err());
    }
    for coupling in [EnsembleCoupling::CoupledR8, EnsembleCoupling::Surgical] {
        assert!(PerformanceProfile::POCKET.validate(coupling).is_err());
    }
}

#[test]
fn modern_release_envelope_ignores_unlinked_same_role_attacks() {
    use gibson::audio::human_music::{
        form::SectionKind,
        identity::IdentityDiagnostics,
        mass::TemporalMass,
        score::{Note, Provenance},
        tension::TensionDiagnostics,
        voice::{HeardWindows, ObservedLifetimePolicy},
    };
    let world = MusicWorld::black_ice();
    let mut profile = PerformanceProfile::POCKET;
    profile.lifetime = VoiceLifetimePolicy::ReleaseEnvelope;
    let mut modern =
        perform_with_profile(&song(8.0), &world, PerformanceOptions::default(), profile).unwrap();
    modern.score.notes = vec![Note::new(
        0.0,
        1.0,
        60,
        0.7,
        Role::Pad,
        Provenance::new(SectionKind::A),
    )];
    modern.score.voice_continuity.clear();
    let before = HeardWindows::of_score(&modern.score, &world).windows()[0].1;
    modern.score.notes.push(Note::new(
        1.25,
        1.0,
        67,
        0.7,
        Role::Pad,
        Provenance::new(SectionKind::A),
    ));
    let after = HeardWindows::of_score(&modern.score, &world).windows()[0].1;
    let envelope = HeardWindows::explicit(&modern.score.notes, &world, modern.score.tempo_bpm, &[])
        .windows()[0]
        .1;
    assert_eq!(before, envelope);
    assert_eq!(
        after, envelope,
        "an unlinked same-role attack cannot shorten a canonical voice"
    );
    // The archived observer remains available as a named negative control.
    assert_eq!(
        HeardWindows::historical(&modern.score.notes, &world, modern.score.tempo_bpm).windows()[0]
            .1,
        1.25
    );
    modern.score.notes.push(Note::new(
        1.3,
        0.2,
        61,
        0.7,
        Role::Lead,
        Provenance::new(SectionKind::A),
    ));
    let canonical =
        HeardWindows::explicit(&modern.score.notes, &world, modern.score.tempo_bpm, &[]);
    assert_eq!(
        TemporalMass::of_score(&modern.score, &modern.perf.contexts, &world),
        TemporalMass::of_heard_windows(
            &canonical,
            &modern.perf.contexts,
            modern.score.beats_per_bar
        )
    );
    assert_eq!(
        format!(
            "{:?}",
            IdentityDiagnostics::measure_score(&modern.score, &modern.perf.contexts, &world)
        ),
        format!(
            "{:?}",
            IdentityDiagnostics::measure_heard_windows(&canonical, &modern.perf.contexts)
        )
    );
    let canonical_clashes =
        TensionDiagnostics::measure(&modern.score, &modern.perf.contexts, &world);
    let source_pair = |c: &gibson::audio::human_music::tension::Clash| {
        (c.tension == 0 && c.partner == 2) || (c.tension == 2 && c.partner == 0)
    };
    assert!(canonical_clashes.clashes.iter().any(source_pair));
    modern.score.observed_lifetime = Some(ObservedLifetimePolicy::LegacyRoleMasking);
    let archived_clashes =
        TensionDiagnostics::measure(&modern.score, &modern.perf.contexts, &world);
    assert!(!archived_clashes.clashes.iter().any(source_pair));
}

#[test]
fn accepted_modern_pocket_retains_archived_events_and_evidence() {
    let song = song(120.0);
    let world = MusicWorld::black_ice();
    let archived = perform_pocketed(&song, &world, PerformanceOptions::default());
    let mut modern = perform_with_profile(
        &song,
        &world,
        PerformanceOptions::default(),
        PerformanceProfile::POCKET,
    )
    .unwrap();
    // Only the explicit selection metadata migrates: compare the complete historical receipt.
    modern.score.observed_lifetime = None;
    assert_eq!(
        format!("{:?}", archived.score),
        format!("{:?}", modern.score)
    );
}

#[test]
fn observer_and_render_policy_have_distinct_canonical_coordinates() {
    use gibson::audio::human_music::{
        fingerprint::CanonicalFingerprint, voice::ObservedLifetimePolicy,
    };
    let profile = PerformanceProfile::POCKET;
    let mut observation = profile;
    observation.observation = ObservedLifetimePolicy::LegacyRoleMasking;
    let mut rendering = profile;
    rendering.lifetime = VoiceLifetimePolicy::ReleaseEnvelope;
    assert_ne!(
        profile.canonical_fingerprint(),
        observation.canonical_fingerprint()
    );
    assert_ne!(
        profile.canonical_fingerprint(),
        rendering.canonical_fingerprint()
    );
    assert_ne!(
        observation.canonical_fingerprint(),
        rendering.canonical_fingerprint()
    );
}

#[test]
fn canonical_lifetime_reaches_support_before_notes_are_chosen() {
    use gibson::audio::human_music::{comp, score::Note, voice::ObservedLifetimePolicy};
    let song = song(120.0);
    let mut world = MusicWorld::black_ice();
    // A declared long-release instrument separates the rivals: later same-role attacks do
    // not erase this direct sound. This is a mechanism falsifier, not a listening preset.
    world.pad.adsr.3 = 10.0;
    world.keys.adsr.3 = 10.0;
    let pitches_and_gates = |notes: &[Note]| {
        notes
            .iter()
            .map(|n| (n.start_beat, n.dur_beats, n.pitch))
            .collect::<Vec<_>>()
    };
    for mut profile in [PerformanceProfile::HEARD, PerformanceProfile::PHRASED] {
        let modern =
            perform_with_profile(&song, &world, PerformanceOptions::default(), profile).unwrap();
        profile.observation = ObservedLifetimePolicy::LegacyRoleMasking;
        let archived =
            perform_with_profile(&song, &world, PerformanceOptions::default(), profile).unwrap();
        for role in [Role::Lead, Role::Keys, Role::Bass] {
            assert_eq!(
                pitches_and_gates(&modern.score.role_notes(role).copied().collect::<Vec<_>>()),
                pitches_and_gates(&archived.score.role_notes(role).copied().collect::<Vec<_>>()),
            );
        }
        let pad = modern
            .score
            .role_notes(Role::Pad)
            .copied()
            .collect::<Vec<_>>();
        let old_pad = archived
            .score
            .role_notes(Role::Pad)
            .copied()
            .collect::<Vec<_>>();
        assert_ne!(pitches_and_gates(&pad), pitches_and_gates(&old_pad));
        let band = [Role::Lead, Role::Keys, Role::Bass]
            .into_iter()
            .flat_map(|role| modern.score.role_notes(role).copied())
            .collect::<Vec<_>>();
        let mut direct = if profile.support == SupportPolicy::SourceVoicePath {
            comp::realize_pad_pocketed(&modern.perf, &world, &band, &[]).0
        } else {
            comp::realize_pad_heard_with_continuity(&modern.perf, &world, &band, &[]).0
        };
        // The ordinary piece-end boundary applies after all source choices.
        direct.retain(|n| n.start_beat < modern.score.total_beats - 1e-9);
        for note in &mut direct {
            if note.start_beat + f64::from(note.dur_beats) > modern.score.total_beats + 1e-9 {
                note.dur_beats = (modern.score.total_beats - note.start_beat) as f32;
            }
        }
        assert_eq!(pitches_and_gates(&pad), pitches_and_gates(&direct));
        assert!(modern.score.stale_hearings().is_empty());
    }
}
