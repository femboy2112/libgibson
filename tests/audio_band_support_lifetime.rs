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
//! (`FunctionPolicy::Earned`) a support tail must not sound into a structural harmony that
//! EXCLUDES its pitch — that is a smear and a false function. This is the bass's and keys' own
//! release law (`release_support`'s membership test) extended to the pad. A tail the next harmony
//! ADMITS (a chord tone or licensed tension) is consonant and may be load-bearing — the pad's own
//! root ringing under a bar it does not re-voice — and must be kept: clipping it removed the root
//! and flipped the chart at a fresh VAPOR95 case (seed `90_500_002`).
//!
//! The VAPOR95 case was derived under the archival VAPOR95 v1 world and is performed under it.
#[path = "common/vapor95_v1.rs"]
mod vapor95_v1;

use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_checked, perform_with_profile, Composition},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::Role,
    semantic::deflected_lift_trace,
    theory::pitch_class,
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
    let world = vapor95_v1::vapor95_v1();
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
        // The next harmony admits the pitch (a chord tone or a palette tension): the tail is
        // consonant there and may be load-bearing (the pad's own root). Excluded: it is a smear.
        let pc = pitch_class(n.pitch);
        let admitted = c
            .perf
            .context_at(boundary + 1e-6)
            .is_some_and(|next| next.chord.contains_pc(pc) || next.palette.tensions.contains(&pc));
        if !admitted {
            out.push(format!(
                "pad {} at {} rings to {} past boundary {} into a harmony that excludes it",
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
fn band_support_tails_do_not_smear_into_a_harmony_that_excludes_them() {
    let (song, world, opts) = falsifier();
    let c = perform_with_profile(&song, &world, opts, PerformanceProfile::BAND)
        .expect("the falsifier is a lawful performance");
    let bad = unjustified_support_tails(&c, &world);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
