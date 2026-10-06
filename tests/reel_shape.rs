//! Shape tests for the showcase film's cue sheet: the reel must be a well-formed
//! **single-clock** film — the right length (equal to the camera's budget, not a
//! second competing duration), establish/reveal bookends, full grammar+scene
//! coverage, readable cues, no blank frames, and panel windows aligned to the
//! camera's per-panel base times (the alignment that keeps the picture the camera
//! flies identical to the shot the timeline names). Pure `f(edit)` math over the cue
//! sheet — no TTY, no render.

#[path = "../examples/cinematic_showcase/reel.rs"]
mod reel;
#[path = "../examples/cinematic_showcase/shot.rs"]
mod shot;
#[path = "../examples/cinematic_showcase/show_timeline.rs"]
mod show_timeline;
#[path = "../examples/cinematic_showcase/stage.rs"]
mod stage;

use shot::{GrammarId, SceneId, Shot};

#[test]
fn duration_equals_the_single_camera_budget() {
    // The whole point of the consolidation: the reel's length is the camera's
    // length. One clock, not two.
    let d = reel::reel().duration();
    let stage = stage::duration();
    assert!(
        (d - stage).abs() < 1e-3,
        "reel duration {d} must equal the stage budget {stage}"
    );
}

#[test]
fn film_opens_on_establish_and_closes_on_reveal() {
    let tl = reel::reel();
    let first = tl.top(0.0).expect("a shot at t=0");
    assert!(
        matches!(first.payload, Shot::Establish),
        "opens on the establish"
    );
    let last = tl.top(tl.duration() - 0.5).expect("a shot near the end");
    assert!(matches!(last.payload, Shot::Reveal), "closes on the reveal");
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

#[test]
fn panel_cues_align_to_the_cameras_per_panel_base_times() {
    // establish + 6 grammars + 3 scenes + reveal = 11 gapless cues.
    let tl = reel::reel();
    let cues = tl.cues();
    assert_eq!(
        cues.len(),
        1 + GrammarId::REEL_ORDER.len() + SceneId::ALL.len() + 1
    );

    // The opening bracket is exactly the establish budget.
    assert!(matches!(cues[0].payload, Shot::Establish));
    assert!((cues[0].dur - stage::EST).abs() < 1e-4, "establish dur");

    // Each of the nine panels is one PANEL long and starts at EST + i*PANEL — the
    // same base time `stage::camera_at` keys the per-panel dolly to.
    for i in 0..stage::N_PLANES {
        let c = &cues[1 + i];
        assert!(
            matches!(c.payload, Shot::Grammar(_) | Shot::Observatory(_)),
            "cue {} is a panel",
            1 + i
        );
        assert!(
            (c.dur - stage::PANEL).abs() < 1e-4,
            "panel {i} dur {}",
            c.dur
        );
        let expect = stage::EST + i as f32 * stage::PANEL;
        assert!(
            (c.start - expect).abs() < 1e-3,
            "panel {i} start {} must equal camera base {expect}",
            c.start
        );
    }

    // The closing bracket is exactly the reveal budget.
    let last = cues.last().unwrap();
    assert!(matches!(last.payload, Shot::Reveal));
    assert!((last.dur - stage::REVEAL).abs() < 1e-4, "reveal dur");
}
