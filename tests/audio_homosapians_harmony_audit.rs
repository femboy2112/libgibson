//! The Homosapians: source-independent CHARACTERIZATION of the live smooth-pop verticals.
//!
//! These tests do NOT assert that current color is artistically desirable. They preserve
//! reproducible evidence for a human-ear decision: a melodically closed hook can still be
//! harmonically unresolved and can simultaneously sound G-natural against E7 G-sharp.
//! Update the characterization deliberately when a new, ear-approved color is authored.
//!
//! This is deliberately outside source_pop labels: inspect the COMMITTED Score's actual
//! pitches, roles, sounding intervals and chord qualities.

#[path = "../examples/c137_song/source_pop.rs"]
mod source_pop;

use gibson::audio::human_music::{
    argument::ArgumentEnding,
    kernel,
    score::{Role, Score},
    theory::{Chord, Mode, Quality},
    MusicWorld,
};

const EPS: f64 = 1e-6;

fn world() -> MusicWorld {
    let mut w = MusicWorld::black_ice();
    w.tonic_pc = 9;
    w.mode = Mode::Aeolian;
    w.use_sevenths = true;
    w.allow_modal_mixture = true;
    w
}

fn chord_at(score: &Score, at: f64) -> Chord {
    score
        .chords
        .iter()
        .find(|c| c.start_beat - EPS <= at && at < c.start_beat + f64::from(c.dur_beats) - EPS)
        .expect("every sounding test event needs a real chord")
        .chord
}

/// A role's actually scheduled Score note; a chord-label-only assertion does not suffice.
fn sounding_pc(score: &Score, role: Role, pc: i32, at: f64) -> bool {
    score.role_notes(role).any(|n| {
        n.start_beat - EPS <= at
            && at < n.start_beat + f64::from(n.dur_beats) - EPS
            && n.pitch.rem_euclid(12) == pc
    })
}

#[test]
fn current_circle_hook_has_real_g_natural_against_e7_g_sharp() {
    let source = source_pop::source_pop_song(2112, source_pop::PopFlavor::Circle);
    let take = kernel::perform_song(&source, &world()).expect("current pop source admitted");
    let score = take.score();

    // Hook starts at beat 16; local 13.5 is global beat 29.5.
    // At beat 29.75, the written G is sounding in Lead; E7 contains G#,
    // and its chord-tone Pad has G# sounding too. This is a genuine
    // cross-voice semitone friction, not a G# erased by a lead-only edit.
    let at = 29.75;
    let c = chord_at(score, at);
    assert_eq!(c.root_pc, 4, "cadence is E-rooted");
    assert_eq!(c.quality, Quality::Dom7, "it is actually an E7");
    assert!(c.contains_pc(8), "E7 has G# as its major third");
    assert!(sounding_pc(score, Role::Lead, 7, at), "the hook still sings G natural");
    assert!(
        sounding_pc(score, Role::Pad, 8, at),
        "the pad really sustains the E7 G#, not merely a chart symbol"
    );

    // The current clash is CHARACTERIZED, not endorsed: if Claude changes it,
    // revise the fixture and explain the changed harmonic intent with an ear A/B.
}

#[test]
fn a_melodically_closed_last_note_is_not_a_tonic_chord_arrival() {
    let source = source_pop::source_pop_song(2112, source_pop::PopFlavor::Circle);
    assert_eq!(source.argument.ending, ArgumentEnding::Resolved);
    let take = kernel::perform_song(&source, &world()).expect("current pop source admitted");
    let score = take.score();

    // Final melody A (tonic pitch) at local hook beat 14.5 -> global beat 30.5;
    // the underlying harmony is still E7 until the end of this 32-beat sketch.
    let at = 30.75;
    assert!(sounding_pc(score, Role::Lead, 9, at), "lead resolves to pitch A");
    let c = chord_at(score, at);
    assert_eq!(c, Chord::new(4, Quality::Dom7), "final sounding chord remains E7");
    assert_ne!(c.root_pc, world().tonic_pc, "no final Am tonic-chord arrival");

    // This is NOT automatically a compositional error. It means the three
    // predicates 'melodic home', 'harmonic home', 'narrative completed' are
    // different propositions; future source laws must label them separately.
}

#[test]
fn a_note_mutation_changes_the_vertical_without_changing_the_chart_but_is_refused() {
    let source = source_pop::source_pop_song(2112, source_pop::PopFlavor::Circle);
    let take = kernel::perform_song(&source, &world()).expect("current pop source admitted");
    let score = take.score();
    let at = 29.75;
    assert!(sounding_pc(score, Role::Lead, 7, at));

    let mut candidate = score.clone();
    let altered = candidate
        .notes
        .iter_mut()
        .find(|n| n.role == Role::Lead && (n.start_beat - 29.5).abs() < EPS)
        .expect("the source-authored hook G onset");
    altered.pitch += 1; // G -> G#, isolated pitch mutation for an independent observer
    assert!(!sounding_pc(&candidate, Role::Lead, 7, at));
    assert!(sounding_pc(&candidate, Role::Lead, 8, at));
    assert_eq!(chord_at(score, at), chord_at(&candidate, at));

    // A later edit to already-committed source events cannot be legitimized by
    // the existing chord chart. This negative control must remain load-bearing.
    assert!(
        take.validate_candidate(&candidate).is_err(),
        "the committed source/witness MUST reject post-hoc melodic repair"
    );
}

#[test]
fn the_latest_circle_bass_turnaround_truly_moves_below_its_e7_root() {
    let source = source_pop::source_pop_song(2112, source_pop::PopFlavor::Circle);
    let score = kernel::perform_song(&source, &world())
        .expect("current source admitted")
        .score()
        .clone();

    // R12 BassDegree::Seventh should realize D2 below E2, rather
    // than only changing the semantic label of the cell.
    let root = score
        .role_notes(Role::Bass)
        .find(|n| (n.start_beat - 12.0).abs() < EPS)
        .expect("cadence root");
    assert_eq!(root.pitch.rem_euclid(12), 4, "E foundation");
    let dark = score
        .role_notes(Role::Bass)
        .find(|n| (n.start_beat - 13.0).abs() < 0.08)
        .expect("turnaround bass seventh attack");
    assert_eq!(dark.pitch.rem_euclid(12), 2, "D natural, E7 flat seventh");
    assert!(dark.pitch < root.pitch, "the note must actually descend in register");
}
