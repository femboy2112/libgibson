//! Shape tests for the showcase film's cue sheet: the reel must be a well-formed
//! film (right length, title bookends, full coverage, readable cues, no blank
//! frames). Pure `f(edit)` math over the cue sheet — no TTY, no render.

#[path = "../examples/cinematic_showcase/reel.rs"]
mod reel;
#[path = "../examples/cinematic_showcase/shot.rs"]
mod shot;
#[path = "../examples/cinematic_showcase/show_timeline.rs"]
mod show_timeline;

use shot::{GrammarId, SceneId, Shot};

#[test]
fn duration_is_within_the_target_window() {
    let d = reel::reel().duration();
    assert!((70.0..=95.0).contains(&d), "duration {d} outside 70..=95 s");
}

#[test]
fn film_opens_and_closes_on_a_title() {
    let tl = reel::reel();
    let first = tl.top(0.0).expect("a shot at t=0");
    assert!(matches!(first.payload, Shot::Title(_)), "opens on a title");
    let last = tl.top(tl.duration() - 0.5).expect("a shot near the end");
    assert!(matches!(last.payload, Shot::Title(_)), "closes on a title");
}

#[test]
fn every_grammar_and_scene_appears() {
    let tl = reel::reel();
    for g in GrammarId::REEL_ORDER {
        assert!(
            tl.cues().iter().any(|c| c.payload == Shot::Grammar(g)),
            "grammar {g:?} missing from the reel"
        );
    }
    for s in SceneId::ALL {
        assert!(
            tl.cues().iter().any(|c| c.payload == Shot::Observatory(s)),
            "scene {s:?} missing from the reel"
        );
    }
}

#[test]
fn every_cue_is_readable() {
    for (i, c) in reel::reel().cues().iter().enumerate() {
        assert!(c.dur >= 2.0, "cue {i} is only {} s", c.dur);
    }
}

#[test]
fn no_gap_blanks_the_film() {
    let tl = reel::reel();
    let end = tl.duration();
    let mut k = 0u32;
    loop {
        let t = k as f32 * 0.25;
        if t >= end {
            break;
        }
        assert!(tl.top(t).is_some(), "gap in the film at t={t}");
        k += 1;
    }
}
