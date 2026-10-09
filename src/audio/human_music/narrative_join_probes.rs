#![cfg(test)]
//! **Rick-C137 GEN-STORY-4 — the theme/harmony narrative join, characterized then repaired.**
//!
//! GEN-STORY-1..3 built [`super::narrative::NarrativePlan`] off μ(song). But its `from_observation`
//! keeps only [`Lane::Theme`] events, and the kinds it keys a *harmonic* carrier on — `Prepare`
//! (the dominant lifts) and `Miss` (the lead withholds, a carrier makes the deflection
//! intelligible) — are emitted by μ ONLY on [`Lane::Harmony`] ([`super::meaning::observe`]). So
//! those arms are unreachable under the real pipeline, and the module's own unit tests fake them
//! with synthetic theme-lane events `observe()` can never produce.
//!
//! These tests CHARACTERIZE the true state on REAL generated songs (all green — they pin the
//! defect, not a fix), so GEN-STORY-4's lane-join repair has ground truth to build against:
//!
//! 1. the judged voice, [`Composer::MeaningDirected`] at `grammar=None`, has NO harmony lane at
//!    all (`harmonic=false`) — so a lane-join alone is a no-op there;
//! 2. only a song composed WITH a deflecting grammar ([`CompositionGrammar::DeflectedLift`]) gives
//!    the harmonic `Prepare→Miss` cycle, and every `Prepare`/`Miss` is `Lane::Harmony`, never
//!    `Theme`;
//! 3. today's `NarrativePlan` ignores that harmony lane entirely: no carriage carries a deflection,
//!    none withholds the lead — the ear-proved "keys carry the dark" beat is unreachable from the
//!    generator.

use super::composer::Composer;
use super::contract::CompositionGrammar;
use super::meaning::{Lane, MeaningKind as K, MeaningPlan};
use super::narrative::{LeadRole, NarrativePlan};
use super::semantic::{
    calm_loop, deflected_lift_trace, false_climax, rise_unresolved, SemanticTrace,
};
use super::song::SongMap;

const SEED: u64 = 2112;
const BEATS: f64 = 96.0;

/// A stock trace constructor, keyed by beats.
type TraceFn = fn(f64) -> SemanticTrace;

/// The deflecting stock traces — the ones whose chart lifts a dominant and deflects it, so a real
/// harmonic `Prepare→Miss` exists for the narrative to read.
fn deflecting_traces() -> [(&'static str, TraceFn); 3] {
    [
        ("deflected_lift", deflected_lift_trace),
        ("rise_unresolved", rise_unresolved),
        ("false_climax", false_climax),
    ]
}

/// A real generated narrative song: the judged voice composed WITH the deflecting grammar, so it
/// has both a meaning-directed theme lane and the harmonic deflection the story turns on.
fn narrative_song(tf: TraceFn) -> SongMap {
    SongMap::compose(
        &tf(BEATS),
        SEED,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    )
}

/// CHARACTERIZATION 1 — the judged voice has no harmony lane. At `grammar=None` the base
/// [`SongMap::build`] yields no backbone/harmonic, so `compose_meaning`'s chart stage is skipped
/// (composer.rs guard `if let Some(hm) = song.harmonic`) and μ observes zero harmony events. A
/// lane-join that only re-reads the harmony lane therefore changes NOTHING for the default render.
#[test]
fn judged_voice_has_no_harmony_lane() {
    for (name, tf) in [
        ("deflected_lift", deflected_lift_trace as TraceFn),
        ("calm_loop", calm_loop),
    ] {
        let song = SongMap::compose(&tf(BEATS), SEED, None, Composer::MeaningDirected);
        assert!(
            song.harmonic.is_none() && song.plan.backbone.is_none(),
            "{name}: judged voice (grammar=None) must have no chart"
        );
        let obs = MeaningPlan::observe(&song);
        assert!(
            obs.events.iter().all(|w| w.event.lane == Lane::Theme),
            "{name}: judged voice observes a theme lane only"
        );
    }
}

/// CHARACTERIZATION 2 — `Prepare`/`Miss` exist only when composed with a deflecting grammar, and
/// only ever on the harmony lane. This is the ground truth the narrative module's synthetic
/// theme-lane `Miss`/`Prepare` tests contradict.
#[test]
fn prepare_and_miss_are_harmony_only() {
    let mut saw_prepare = false;
    let mut saw_miss = false;
    for (name, tf) in deflecting_traces() {
        let obs = MeaningPlan::observe(&narrative_song(tf));
        for w in &obs.events {
            match w.event.kind {
                K::Prepare(_) => {
                    saw_prepare = true;
                    assert_eq!(
                        w.event.lane,
                        Lane::Harmony,
                        "{name}: Prepare must be harmony-lane"
                    );
                }
                K::Miss(_) => {
                    saw_miss = true;
                    assert_eq!(
                        w.event.lane,
                        Lane::Harmony,
                        "{name}: Miss must be harmony-lane"
                    );
                }
                _ => {}
            }
        }
        // ...and the theme lane never carries a Prepare or a Miss.
        assert!(
            obs.events
                .iter()
                .filter(|w| w.event.lane == Lane::Theme)
                .all(|w| !matches!(w.event.kind, K::Prepare(_) | K::Miss(_))),
            "{name}: no theme-lane Prepare/Miss"
        );
    }
    assert!(
        saw_prepare && saw_miss,
        "the deflecting songs must give a real Prepare and Miss"
    );
}

/// CHARACTERIZATION 3 — THE DEFECT. Even on a song full of harmonic `Prepare→Miss`, today's
/// `NarrativePlan` reads only the theme lane: no carriage carries a deflection and none withholds
/// the lead. The ear-proved "lead withholds at the Miss, keys carry the dark" is unreachable from
/// the generator. GEN-STORY-4 (stage 2) replaces this test's assertions with the join's.
#[test]
fn todays_narrative_ignores_the_harmony_lane() {
    for (name, tf) in deflecting_traces() {
        let song = narrative_song(tf);
        let obs = MeaningPlan::observe(&song);
        let np = NarrativePlan::from_observation(&obs);

        // One carriage per theme event — the harmony lane contributes nothing.
        let theme_events = obs
            .events
            .iter()
            .filter(|w| w.event.lane == Lane::Theme)
            .count();
        assert_eq!(
            np.carriages.len(),
            theme_events,
            "{name}: today one carriage per theme event, harmony ignored"
        );
        // And despite real Misses, the lead never withholds.
        assert!(
            np.carriages
                .iter()
                .all(|c| c.lead_role == LeadRole::Stating),
            "{name}: today the lead never withholds (the Miss arm is dead code)"
        );
    }
}
