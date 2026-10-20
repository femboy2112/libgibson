//! The section-form witness with TEETH (audit G6 / Priority Six): the genuine rehearsal backing's
//! verse -> chorus form certifies, and every false certificate of development — a one-chord drone, a
//! chorus that merely repeats the verse, a missing turnaround, a verse that never recurs, a missing
//! bass transition — is refused. Defeats the `loop_bars == route.len()` tautology.

#[path = "../examples/c137_song/rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use gibson::audio::human_music::{
    argument::CompiledArgument, section_witness::observe_section_form,
};

const SEEDS: [u64; 5] = [7, 42, 2112, 9, 13];

fn source(seed: u64) -> CompiledArgument {
    rehearsal::freeze(&rehearsal::rehearse(seed))
}

#[test]
fn the_genuine_backing_has_a_real_verse_chorus_form() {
    for seed in SEEDS {
        let w = observe_section_form(&source(seed));
        assert!(
            w.valid(),
            "seed {seed}: genuine backing fails the section law: {:?}",
            w.violations()
        );
    }
}

#[test]
fn a_one_chord_drone_verse_is_refused() {
    let mut s = source(2112);
    let c0 = s.route[0];
    for r in s.route.iter_mut() {
        *r = c0;
    }
    let w = observe_section_form(&s);
    assert!(!w.valid(), "a drone slipped past the section law");
    assert!(w.violations().iter().any(|v| v.reason.contains("drone")));
}

#[test]
fn a_chorus_that_merely_repeats_the_verse_is_refused() {
    let mut s = source(2112);
    let n = s.route.len();
    let half = n / 2;
    for i in 0..half {
        s.route[half + i] = s.route[i]; // chorus := verse
    }
    let w = observe_section_form(&s);
    assert!(!w.valid(), "a non-transforming chorus slipped past");
    assert!(w
        .violations()
        .iter()
        .any(|v| v.reason.contains("transform")));
}

#[test]
fn a_missing_turnaround_is_refused() {
    let mut s = source(2112);
    let n = s.route.len();
    s.route[n - 1] = s.route[0]; // replace the dominant close with a camp chord
    let w = observe_section_form(&s);
    assert!(!w.valid(), "a missing cadential close slipped past");
    assert!(w
        .violations()
        .iter()
        .any(|v| v.reason.contains("cadential")));
}

#[test]
fn a_verse_that_never_recurs_is_refused() {
    let mut s = source(2112);
    // Break the period-2 repeat: make bar 2 (which must recur bar 0) a different chord.
    s.route[2] = s.route[4];
    let w = observe_section_form(&s);
    assert!(!w.valid(), "a non-recurring verse slipped past");
    assert!(w.violations().iter().any(|v| v.reason.contains("camp")));
}

#[test]
fn a_missing_bass_transition_is_refused() {
    let mut s = source(2112);
    let groove = s
        .groove
        .as_mut()
        .expect("the rehearsal backing has a groove");
    groove.bass_cells[3] = groove.bass_cells[0].clone(); // flatten the transition bar
    let w = observe_section_form(&s);
    assert!(!w.valid(), "a missing bass transition slipped past");
    assert!(w
        .violations()
        .iter()
        .any(|v| v.reason.contains("transition")));
}
