//! Homosapians Riff-Comp Pocket NATURALITY — read-only CHARACTERIZATION of current code.
//!
//! These tests are not endorsement of audible quality. They document currently
//! observable timing/ownership inconsistencies that the next opt-in phase contract
//! must replace deliberately, with a matched ear A/B and unchanged C137 goldens.
//!
//! P1: riff comp is an automatic 0.03 beat early relative to kick/bass at a
//!     structural bar boundary, even without a declared anticipation.
//! P2: the chord of the NEXT bar can therefore sound inside the previous bar.
//! P3: two Keys note families (required carrier and comp) can flam by 0.03 beat.
//! P4: DrumResponse choices in a rehearsal draft currently have no DIRECT
//!     effect on the frozen performed drum Score.
//!
//! Score facts and categorical causality are not PCM onset or perceptual proof.

#[path = "../examples/c137_song/rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use gibson::audio::human_music::{
    kernel,
    score::{DrumVoice, Role, Score},
    theory::Mode,
    MusicWorld,
};

const EPS: f64 = 1e-6;

fn world() -> MusicWorld {
    let mut w = MusicWorld::black_ice();
    w.tonic_pc = 9;
    w.mode = Mode::Aeolian;
    w.use_sevenths = true;
    w.allow_modal_mixture = true;
    w.tempo_bpm = 104.0;
    w
}

fn performed(seed: u64) -> Score {
    let draft = rehearsal::rehearse(seed);
    let source = rehearsal::freeze(&draft);
    kernel::perform_song(&source, &world())
        .expect("opt-in rehearsal Score is lawful")
        .score()
        .clone()
}

#[test]
fn p1_riff_chord_automatically_precedes_structural_kick_by_seventeen_milliseconds() {
    let score = performed(2112);

    // Every generated two-bar riff retains its second-bar downbeat (onset 4.0).
    // Riff-comp schedules that bar's chord at 4 + pocket(0) - 0.03 == 3.97.
    // The bass foundation and drum kick retain their hard grid anchor at 4.0.
    let comp = score
        .role_notes(Role::Keys)
        .find(|n| n.prov.role_note == "comp" && (n.start_beat - 3.97).abs() < EPS)
        .expect("a Keys comp chord must be scheduled just before the new bar");
    let bass = score
        .role_notes(Role::Bass)
        .find(|n| (n.start_beat - 4.0).abs() < EPS)
        .expect("foundation root keeps bar 1 downbeat");
    let kick = score
        .drums
        .iter()
        .find(|d| d.voice == DrumVoice::Kick && (d.start_beat - 4.0).abs() < EPS)
        .expect("kit kick keeps bar 1 downbeat");

    assert!((bass.start_beat - kick.start_beat).abs() < EPS);
    let advance_beats = bass.start_beat - comp.start_beat;
    assert!((advance_beats - 0.03).abs() < EPS);
    let advance_ms = advance_beats * 60_000.0 / f64::from(score.tempo_bpm);
    assert!((advance_ms - 17.3076923077).abs() < 0.01);
}

#[test]
fn p2_early_chord_uses_next_bar_harmony_before_the_bar_boundary() {
    let score = performed(2112);
    let at = 3.97;
    let old = score
        .chords
        .iter()
        .find(|c| c.start_beat <= at && at < c.start_beat + f64::from(c.dur_beats))
        .expect("old bar harmony exists");
    let next = score
        .chords
        .iter()
        .find(|c| c.start_beat <= 4.0 && 4.0 < c.start_beat + f64::from(c.dur_beats))
        .expect("new bar harmony exists");
    assert_ne!(
        old.chord, next.chord,
        "verse camp really changes chord here"
    );

    let early_comp = score
        .role_notes(Role::Keys)
        .find(|n| n.prov.role_note == "comp" && (n.start_beat - at).abs() < EPS);
    assert!(
        early_comp.is_some(),
        "an unlicensed early note has entered the previous bar"
    );

    // Source deliberately contains no typed "AnticipateNextChord" operation.
    // The current realizer takes NEXT harmony from at_grid=4 while scheduling
    // its physical note at at_grid-0.03. This is a characterization of the
    // boundary mismatch, not evidence that every anticipation is bad.
}

#[test]
fn p3_keys_carrier_and_comp_can_derive_different_phases_for_the_same_slot() {
    // Find a rehearsed 2-bar riff with a 5.5-beat attack. The Keys source
    // carrier *always* attacks there (+pocket), while the riff-comp uses the
    // same beat minus KEYS_ATTACK_COMP. This is NOT a shifted instrument role:
    // two overlapping voices of the very same Keys pool disagree by 0.03 beat.
    let seed = (0u64..256)
        .find(|&s| {
            rehearsal::rehearse(s)
                .riff()
                .events
                .iter()
                .any(|e| (e.onset - 5.5).abs() < EPS)
        })
        .expect("the finite riff vocabulary includes a sparse 5.5-beat note");
    let score = performed(seed);

    let carrier = score
        .role_notes(Role::Keys)
        .find(|n| n.prov.role_note == "argument" && (n.start_beat - 5.534).abs() < EPS)
        .expect("the required Keys material rides the unadjusted pocket");
    let comp = score
        .role_notes(Role::Keys)
        .find(|n| n.prov.role_note == "comp" && (n.start_beat - 5.504).abs() < EPS)
        .expect("the comp copies riff placement but advances by 0.03 beat");

    assert!(((carrier.start_beat - comp.start_beat) - 0.03).abs() < EPS);
}

#[test]
fn p4_drummer_choice_is_not_yet_a_direct_frozen_performance_cause() {
    let a = rehearsal::rehearse(2112);
    let mut b = a.clone();
    assert!(!b.drums.fill.is_empty());
    for x in &mut b.drums.fill {
        *x = !*x;
    }
    for x in &mut b.drums.reinforce_push {
        *x = !*x;
    }
    for x in &mut b.drums.leave_gap {
        *x = !*x;
    }
    assert_ne!(a.drums, b.drums, "the drummer made different decisions");

    let perform = |draft: &rehearsal::RehearsalDraft| {
        kernel::perform_song(&rehearsal::freeze(draft), &world())
            .expect("both frozen drafts are admissible")
            .score()
            .drums
            .iter()
            .map(|h| (h.start_beat, h.voice, h.velocity, h.prov))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        perform(&a),
        perform(&b),
        "currently the real kit reads only the bass groove and stock funk rules"
    );
}
