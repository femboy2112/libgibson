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
fn historical_adapters_and_musical_profiles_have_identical_events_and_evidence() {
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
    for (historical, profile) in cases {
        let old = historical(&song, &world, PerformanceOptions::default());
        let new =
            perform_with_profile(&song, &world, PerformanceOptions::default(), profile).unwrap();
        assert_eq!(format!("{:?}", old.score), format!("{:?}", new.score));
        assert_eq!(old.perf.fingerprint(), new.perf.fingerprint());
        let from_plan = realize_with_profile(&song, &world, &new.perf, profile).unwrap();
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
    for profile in [written_expression, unsupported_voice, missing_ownership] {
        assert!(profile.validate(EnsembleCoupling::Independent).is_err());
    }
    for coupling in [EnsembleCoupling::CoupledR8, EnsembleCoupling::Surgical] {
        assert!(PerformanceProfile::POCKET.validate(coupling).is_err());
    }
}
