//! Final pre-main round: the BAND support-lifetime law.
//!
//! A fresh-seed falsifier (seed `90_000_015`, outside every declared sweep) reproduced the last
//! internal BAND rejection: at a rootless pad voicing, the moment the bass leaves the chart's
//! root, the pad's *release tail* crosses the next structural harmony and turns a slice from
//! `implied` to `passing` in that next harmony. The pad's own identity repair refuses to root the
//! voicing because that tail regression makes the "no worse anywhere" guard fail — so a genuine
//! held-identity flip survives into the take and the receipt rejects it.
//!
//! The law this file falsifies (stated independently of the library's own predicate): under BAND
//! (`FunctionPolicy::Earned`) a pad note's acoustic lifetime must not cross a structural harmony
//! boundary unless the pad itself states that exact pitch in the harmony it would ring into (a
//! held common tone / re-attacked member). A support tail with no such continuation is a tail
//! whose harmonic situation has ended.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_checked, perform_with_profile, Composition},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::Role,
    semantic::deflected_lift_trace,
    voice::{effective_audible_end_at, patch, AUDIBLE_FLOOR_DB},
    MusicWorld, SongMap,
};

/// The exact fresh case that failed at the final Claude source.
fn falsifier() -> (SongMap, MusicWorld, PerformanceOptions) {
    let song = SongMap::compose(
        &deflected_lift_trace(16.0),
        90_000_015,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    );
    let world = MusicWorld::vapor95();
    let opts = PerformanceOptions {
        language: MusicalLanguage::fusion_conversation(),
        ..PerformanceOptions::default()
    };
    (song, world, opts)
}

/// Restated, not borrowed from the library's judge: every pad note whose acoustic lifetime
/// crosses the next structural harmony boundary where the pad does not state the same pitch.
fn unjustified_support_tails(c: &Composition, world: &MusicWorld) -> Vec<String> {
    let mut out = Vec::new();
    for n in c.score.notes.iter().filter(|n| n.role == Role::Pad) {
        let Some(ctx) = c.perf.context_at(n.start_beat) else {
            continue;
        };
        let boundary = ctx.start_beat + f64::from(ctx.dur_beats);
        let end = effective_audible_end_at(
            n,
            patch(world, n.role),
            c.score.tempo_bpm,
            AUDIBLE_FLOOR_DB,
            &c.score.voice_continuity,
        );
        if boundary <= n.start_beat + 1e-6 || end <= boundary + 1e-6 {
            continue;
        }
        let stated = c.score.notes.iter().any(|m| {
            m.role == Role::Pad && m.pitch == n.pitch && (m.start_beat - boundary).abs() < 1e-6
        });
        if !stated {
            out.push(format!(
                "pad {} at {} rings to {} past boundary {} with no stated continuation",
                n.pitch, n.start_beat, end, boundary
            ));
        }
    }
    out
}

#[test]
fn band_admits_the_fresh_support_tail_falsifier() {
    let (song, world, opts) = falsifier();
    match perform_checked(&song, &world, opts, PerformanceProfile::BAND) {
        Ok(_) => {}
        Err(e) => panic!("the fresh BAND falsifier is not admitted: {e}"),
    }
}

#[test]
fn band_support_tails_are_justified_by_a_stated_continuation() {
    let (song, world, opts) = falsifier();
    let c = perform_with_profile(&song, &world, opts, PerformanceProfile::BAND)
        .expect("the falsifier is a lawful performance");
    let bad = unjustified_support_tails(&c, &world);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
