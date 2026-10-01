//! Final pre-main round: a truncated final bar promises nothing it has no room to state.
//!
//! A broadened fresh BAND sweep (seeds `90_500_000`, outside every declared sweep) exposed two
//! pre-existing, profile-independent song-conformance reds at partial final bars (they fail under
//! WRITTEN too, so they are composition-conformance bugs, not the BAND hardening):
//!
//! 1. **A truncated theme site.** `deflected + PropulsiveReturn + StructuralR9 + 13.25` placed the
//!    identity thesis in a final phrase whose *nominal* 2-bar extent was long enough, then the form
//!    truncated to 13.25 beats so the phrase (8.0..13.25 = 5.25 beats) could no longer hold the
//!    6-beat theme. The statement planner correctly dropped the statement ("a finite form does not
//!    promise an identity statement it has no room to make" — `ThemeSite::statable`), but
//!    `SongMapConformance` and `AnchorReport` still counted the site as *declared but missing*, so
//!    the BAND receipt rejected the take.
//!
//! 2. **A truncated Lift pointer.** `demo + DeflectedLift + StructuralR9/MeaningDirected + 33.25`
//!    left a Lift slot whose end lay beyond the piece, so the pointer chord (the arrival the Lift
//!    sets up) was never realized. `SongMap::landmarks()` clamped the pointer to the piece end,
//!    where the lift chord still sounds, and reported a wrong-root landmark at the very last beat.
//!
//! The law both falsifiers restate: conformance is judged against the FINAL, truncated form — an
//! element whose own room the form never had is structurally inapplicable, not omitted. This uses
//! the library's already-declared `ThemeSite::statable` rule and the pointer's own realized slot,
//! not a special case for either combination.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_checked, perform_with_profile, Composition},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    semantic::{deflected_lift_trace, demo_trace},
    song::{AnchorPresence, AnchorReport, SongMapConformance},
    MusicWorld, SemanticTrace, SongMap,
};

fn case(
    trace: SemanticTrace,
    seed: u64,
    grammar: CompositionGrammar,
    composer: Composer,
    world: MusicWorld,
    language: MusicalLanguage,
) -> (SongMap, MusicWorld, PerformanceOptions) {
    let song = SongMap::compose(&trace, seed, Some(grammar), composer);
    (
        song,
        world,
        PerformanceOptions {
            language,
            ..PerformanceOptions::default()
        },
    )
}

/// Family 1: the identity thesis placed in a truncated final phrase.
fn truncated_theme() -> (SongMap, MusicWorld, PerformanceOptions) {
    case(
        deflected_lift_trace(13.25),
        90_500_000,
        CompositionGrammar::PropulsiveReturn,
        Composer::StructuralR9,
        MusicWorld::black_ice(),
        MusicalLanguage::simple(),
    )
}

/// Family 2: the Lift pointer cut off by the piece end.
fn truncated_pointer() -> (SongMap, MusicWorld, PerformanceOptions) {
    case(
        demo_trace(33.25),
        90_500_000,
        CompositionGrammar::DeflectedLift,
        Composer::StructuralR9,
        MusicWorld::black_ice(),
        MusicalLanguage::simple(),
    )
}

fn admit(c: &(SongMap, MusicWorld, PerformanceOptions)) {
    match perform_checked(&c.0, &c.1, c.2, PerformanceProfile::BAND) {
        Ok(_) => {}
        Err(e) => panic!("the fresh truncated-form falsifier is not admitted: {e}"),
    }
}

#[test]
fn band_admits_the_truncated_theme_site_falsifier() {
    admit(&truncated_theme());
}

#[test]
fn band_admits_the_truncated_pointer_falsifier() {
    admit(&truncated_pointer());
}

/// Family 3: a swung/deflected short form whose kit enters late — the pocket's downbeat kick is
/// anticipated onto 7.99, so counting strokes by their performed float put the kick in the previous
/// bar and no full bar stated kick AND backbeat (`no full bar states the pocket's kick and backbeat
/// together`, 120 fresh cases). A pocket stroke belongs to the bar of its recorded METRIC source.
fn metric_groove() -> (SongMap, MusicWorld, PerformanceOptions) {
    case(
        deflected_lift_trace(13.25),
        90_500_002,
        CompositionGrammar::DeflectedLift,
        Composer::StructuralR9,
        MusicWorld::black_ice(),
        MusicalLanguage::fusion_conversation(),
    )
}

#[test]
fn band_admits_the_metric_groove_bar_falsifier() {
    admit(&metric_groove());
    let (song, world, opts) = metric_groove();
    let c = perform_with_profile(&song, &world, opts, PerformanceProfile::BAND).expect("lawful");
    let report = AnchorReport::check(&c.song, &c.perf, &c.score);
    let groove = report
        .conformance
        .iter()
        .find(|(a, _)| {
            matches!(
                a,
                gibson::audio::human_music::contract::CoherenceAnchor::Groove
            )
        })
        .map(|(_, c)| c.clone())
        .unwrap();
    assert!(
        !matches!(
            groove,
            gibson::audio::human_music::song::AnchorConformance::Deviates(_)
        ),
        "the groove deviates: {groove:?}"
    );
}

/// Family 4: at a truncated 33.25-bar a response material's last event was inflated past its own
/// room by the 0.1-beat floor (`material MaterialId(8) 32.5..33.266 outside 0..33.25`), and the
/// bass's reservation read from it tripped the strict occupancy check by 2.5e-9. Both are the H12
/// domain law: a planned window inhabits `[0, total_beats]`, with the domain's own epsilon.
fn interaction_material_domain() -> (SongMap, MusicWorld, PerformanceOptions) {
    case(
        demo_trace(33.25),
        90_500_001,
        CompositionGrammar::DeflectedLift,
        Composer::MeaningDirected,
        MusicWorld::black_ice(),
        MusicalLanguage::fusion_conversation(),
    )
}

#[test]
fn band_admits_the_interaction_material_domain_falsifier() {
    admit(&interaction_material_domain());
    let (song, world, opts) = interaction_material_domain();
    let c = perform_with_profile(&song, &world, opts, PerformanceProfile::BAND).expect("lawful");
    let bad: Vec<_> = c
        .perf
        .materials
        .iter()
        .filter(|m| m.start_beat + m.length() > c.score.total_beats + 1e-6)
        .map(|m| format!("{:?} {}..{}", m.id, m.start_beat, m.start_beat + m.length()))
        .collect();
    assert!(bad.is_empty(), "{bad:?}");
    assert!(c.perf.domain_violations().is_empty());
}

/// The restated law: a site the FINAL form has no room to state is not a "missing theme site", and
/// an element the piece truncates is not a wrong landmark.
#[test]
fn a_truncated_form_promises_nothing_it_has_no_room_to_state() {
    let (song, world, opts) = truncated_theme();
    let c: Composition =
        perform_with_profile(&song, &world, opts, PerformanceProfile::BAND).expect("lawful");
    let conf = SongMapConformance::check(&c.song, &c.perf, &c.score);
    assert!(conf.passes(), "{conf:?}");
    // A site the final form has no room to state must never be recorded as a missing theme site.
    for miss in &conf.missing_theme_sites {
        let site = song
            .thematic
            .sites
            .iter()
            .find(|s| s.phrase == miss.phrase && s.is_identity());
        assert!(
            site.is_some_and(|s| s.statable(&song.plan)),
            "a no-room site was counted as missing: {miss:?}"
        );
    }
    let report = AnchorReport::check(&c.song, &c.perf, &c.score);
    let presence = report
        .anchors
        .iter()
        .find(|(a, _)| {
            matches!(
                a,
                gibson::audio::human_music::contract::CoherenceAnchor::Motif
            )
        })
        .map(|(_, p)| *p)
        .unwrap();
    assert!(
        !matches!(presence, AnchorPresence::DeclaredButMissing(_)),
        "a no-room site must not be DeclaredButMissing: {presence:?}"
    );

    let (song, world, opts) = truncated_pointer();
    let c: Composition =
        perform_with_profile(&song, &world, opts, PerformanceProfile::BAND).expect("lawful");
    let conf = SongMapConformance::check(&c.song, &c.perf, &c.score);
    assert!(conf.passes(), "{conf:?}");
    // Every landmark the song declares must lie in a realized span when it is checked.
    for (beat, _landmark, _root) in song.landmarks() {
        assert!(
            beat < song.plan.form.total_beats + 1e-9,
            "a landmark was declared past the piece end"
        );
    }
}
