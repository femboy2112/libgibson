//! Drummer agency with TEETH (audit P4 / G7): a source-authorized drum decision changes a
//! discretionary performed stroke or a justified silence, while the kick/backbeat SKELETON never
//! moves — and an all-false plan is byte-identical to no plan at all.

#[path = "../examples/c137_song/rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use std::collections::BTreeSet;

use gibson::audio::human_music::{
    argument::DrumGesturePlan,
    kernel,
    score::{DrumVoice, Score},
    theory::Mode,
    MusicWorld,
};

fn world() -> MusicWorld {
    let mut w = MusicWorld::black_ice();
    w.tonic_pc = 9;
    w.mode = Mode::Aeolian;
    w.use_sevenths = true;
    w.allow_modal_mixture = true;
    w.tempo_bpm = 104.0;
    w
}

fn performed(seed: u64, plan: Option<DrumGesturePlan>) -> Score {
    let draft = rehearsal::rehearse(seed);
    let mut source = rehearsal::freeze(&draft);
    source.drum_plan = plan;
    kernel::perform_song(&source, &world())
        .expect("opt-in rehearsal Score is lawful")
        .score()
        .clone()
}

fn voice_id(v: DrumVoice) -> u8 {
    match v {
        DrumVoice::Kick => 0,
        DrumVoice::Snare => 1,
        DrumVoice::ClosedHat => 2,
        DrumVoice::OpenHat => 3,
        DrumVoice::Clap => 4,
    }
}

fn beat_key(b: f64) -> i64 {
    (b * 1e6).round() as i64
}

/// All drum hits as (voice, time) — ignores velocity, so a reinforcement shows as a same-position hit.
fn drum_set(score: &Score) -> BTreeSet<(u8, i64)> {
    score
        .drums
        .iter()
        .map(|d| (voice_id(d.voice), beat_key(d.start_beat)))
        .collect()
}

/// Just the kick positions — the pulse foundation no discretionary lever may move.
fn kick_positions(score: &Score) -> BTreeSet<i64> {
    score
        .drums
        .iter()
        .filter(|d| d.voice == DrumVoice::Kick)
        .map(|d| beat_key(d.start_beat))
        .collect()
}

fn busy_plan() -> DrumGesturePlan {
    DrumGesturePlan {
        reinforce_push: vec![true; 8],
        leave_gap: vec![true, false, true, false, true, false, true, false],
        fill: vec![false, true, false, true, false, true, false, true],
    }
}

#[test]
fn an_all_false_plan_is_byte_identical_to_no_plan() {
    let none = performed(2112, None);
    let inert = performed(
        2112,
        Some(DrumGesturePlan {
            reinforce_push: vec![false; 8],
            leave_gap: vec![false; 8],
            fill: vec![false; 8],
        }),
    );
    assert_eq!(
        drum_set(&none),
        drum_set(&inert),
        "an all-false plan must leave the kit byte-identical (off == historical)"
    );
}

#[test]
fn a_drummer_decision_changes_the_kit_and_keeps_the_skeleton() {
    let baseline = performed(2112, None);
    let busy = performed(2112, Some(busy_plan()));

    // The counterfactual: a different drummer decision yields a genuinely different performed kit.
    assert_ne!(
        drum_set(&baseline),
        drum_set(&busy),
        "the drummer's gaps and fills must change the performed kit, not merely a log"
    );
    // Evidence of a GAP: the busy plan withholds funk ghosts, so it has strictly fewer snares on at
    // least the gap bars; evidence of a FILL: it adds pickup strokes. Net: the snare sets differ.
    let snares = |s: &Score| -> BTreeSet<i64> {
        s.drums
            .iter()
            .filter(|d| d.voice == DrumVoice::Snare)
            .map(|d| beat_key(d.start_beat))
            .collect()
    };
    assert_ne!(
        snares(&baseline),
        snares(&busy),
        "snare content must differ"
    );

    // The SKELETON is untouched: every kick lands at exactly the same time in both renders. A fill or
    // a gap moves only discretionary snares; a reinforcement changes a kick's velocity, never its slot.
    assert_eq!(
        kick_positions(&baseline),
        kick_positions(&busy),
        "the kick pulse must not move under any discretionary decision"
    );
}

#[test]
fn a_changed_drummer_response_yields_a_different_kit() {
    // The P4 arrow, end to end: two different recorded DrumResponses, converted to plans, produce
    // different performed kits on the SAME song (same riff, chord, bass).
    let r_fill_early = rehearsal::DrumResponse {
        reinforce_push: vec![false; 8],
        leave_gap: vec![false; 8],
        fill: vec![true, false, false, false, false, false, false, false],
        reasons: Vec::new(),
    };
    let r_fill_late = rehearsal::DrumResponse {
        reinforce_push: vec![false; 8],
        leave_gap: vec![false; 8],
        fill: vec![false, false, false, false, true, false, false, false],
        reasons: Vec::new(),
    };
    let a = performed(
        2112,
        Some(rehearsal::drum_plan_from_response(&r_fill_early)),
    );
    let b = performed(2112, Some(rehearsal::drum_plan_from_response(&r_fill_late)));
    assert_ne!(
        drum_set(&a),
        drum_set(&b),
        "a changed drummer response must change the performed Score"
    );
    // ...but both still keep the same kick pulse.
    assert_eq!(kick_positions(&a), kick_positions(&b));
}
