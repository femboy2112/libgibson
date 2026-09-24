//! Independent encounter-level probes: raw inputs must reconstruct both worlds,
//! and a player's defense must change geography rather than only narrative copy.
#[allow(dead_code)]
#[path = "../examples/acid_vs_crash.rs"]
mod encounter;

use encounter::Encounter;
use gibson::{compute_layout, paint, Node, StoryEvent, Surface};
use std::time::Duration;

fn painted(mut node: Node, w: u16, h: u16) -> Surface {
    compute_layout(&mut node, w, h).unwrap();
    let mut surface = Surface::new(w, h);
    paint(&node, &mut surface);
    surface
}

fn same_complete_world(original: &Encounter) {
    let replay = original.replay();
    assert_eq!(original.encounter_trace(), replay.encounter_trace());
    assert_eq!(original.battle(), replay.battle());
    assert_eq!(original.director().trace(), replay.director().trace());
    assert_eq!(original.facts(), replay.facts());
    assert_eq!(
        original.director().current_beat(),
        replay.director().current_beat()
    );
    assert_eq!(
        original.director().mounted().collect::<Vec<_>>(),
        replay.director().mounted().collect::<Vec<_>>()
    );
    assert!(original.replay_matches());
    for (w, h) in [(56, 24), (80, 24), (120, 32), (160, 40)] {
        assert_eq!(
            painted(original.frame(w, h), w, h),
            painted(replay.frame(w, h), w, h),
            "replayed presentation at {w}x{h}"
        );
    }
    // Story's narrower exact-step contract remains valid independently of the
    // encounter reducer. It reproduces the semantic projection, not the graph.
    let story_replay = original.story().replay(original.director().trace());
    assert_eq!(story_replay.facts(), original.facts());
    assert_eq!(story_replay.trace(), original.director().trace());
}

#[test]
fn exact_irregular_inputs_replay_graph_memory_facts_bundles_and_realized_scene() {
    for stage in [
        "quiet",
        "route-contested",
        "first-breach",
        "display-intrusion",
        "climax",
    ] {
        let mut encounter = Encounter::new(stage, false);
        let steps = [
            (13, vec![]),
            (107, vec![StoryEvent::command("trace")]),
            (
                839,
                vec![StoryEvent::command("isolate"), StoryEvent::command("decoy")],
            ),
            (3, vec![]),
            (5003, vec![]),
            (41, vec![StoryEvent::command("trace token")]),
            (211, vec![StoryEvent::command("kill")]),
            (1301, vec![StoryEvent::command("hard isolate")]),
            (19, vec![]),
        ];
        for (millis, events) in steps {
            encounter.update(Duration::from_millis(millis), &events);
            same_complete_world(&encounter);
        }
    }
}

#[test]
fn identical_prefix_then_isolation_or_decoy_changes_world_and_screen() {
    let mut isolated = Encounter::new("first-breach", false);
    let mut baited = Encounter::new("first-breach", false);
    for dt in [137, 53, 29] {
        isolated.tick(Duration::from_millis(dt), false);
        baited.tick(Duration::from_millis(dt), false);
    }
    assert_eq!(isolated.battle(), baited.battle());
    assert_eq!(isolated.director().trace(), baited.director().trace());
    isolated.command("isolate");
    baited.command("decoy");
    for _ in 0..30 {
        isolated.tick(Duration::from_millis(100), false);
        baited.tick(Duration::from_millis(100), false);
    }
    assert_ne!(
        isolated.battle().planner.target,
        baited.battle().planner.target
    );
    assert_ne!(isolated.battle().planner.path, baited.battle().planner.path);
    assert_ne!(isolated.battle().graph, baited.battle().graph);
    assert_ne!(isolated.facts(), baited.facts());
    assert_ne!(
        painted(isolated.frame(120, 32), 120, 32),
        painted(baited.frame(120, 32), 120, 32)
    );
    assert!(!isolated.director().is_finished());
    assert!(!baited.director().is_finished());
    same_complete_world(&isolated);
    same_complete_world(&baited);
}

#[test]
fn auto_and_passive_runs_replay_complete_state_at_resolution() {
    for auto in [false, true] {
        let mut encounter = Encounter::new("quiet", true);
        for index in 0..2400 {
            if encounter.director().is_finished() {
                break;
            }
            encounter.tick(Duration::from_millis([83, 137, 31, 249][index % 4]), auto);
        }
        assert!(
            encounter.director().is_finished(),
            "auto={auto} did not resolve"
        );
        eprintln!("auto={auto} outcome={:?} quality={} lost={} recovered={} adaptations={} decoys={} trace={}",
            encounter.battle().outcome, encounter.battle().outcome_quality,
            encounter.battle().quality.systems_lost, encounter.battle().quality.systems_recovered,
            encounter.battle().quality.adaptations, encounter.battle().quality.decoy_success,
            encounter.battle().trace_confidence);
        same_complete_world(&encounter);
    }
}

#[test]
fn watching_preserves_operator_pacing_intervention_and_replay() {
    let mut encounter = Encounter::new("first-breach", false);
    encounter.set_watching(true);
    encounter.tick(Duration::from_millis(16), true);
    let commands = |e: &Encounter| {
        e.encounter_trace()
            .steps
            .iter()
            .flat_map(|s| &s.events)
            .filter(|e| matches!(e, StoryEvent::Command(_)))
            .count()
    };
    assert_eq!(commands(&encounter), 1);
    for _ in 0..60 {
        encounter.tick(Duration::from_millis(16), true);
    }
    assert_eq!(
        commands(&encounter),
        1,
        "Crash must let each defense read on screen"
    );
    encounter.command("trace");
    assert_eq!(
        commands(&encounter),
        2,
        "human intervention stays available"
    );
    for _ in 0..60 {
        encounter.tick(Duration::from_millis(16), true);
    }
    assert_eq!(
        commands(&encounter),
        2,
        "Crash observes the human intervention"
    );
    encounter.tick(Duration::from_millis(300), true);
    encounter.tick(Duration::from_millis(16), true);
    assert!(commands(&encounter) > 2, "autonomous work resumes");
    same_complete_world(&encounter);
    encounter.command("reset");
    let frame = painted(encounter.frame(120, 32), 120, 32);
    let text: String = (0..32)
        .flat_map(|y| (0..120).map(move |x| (x, y)))
        .map(|(x, y)| frame.get(x, y).unwrap().glyph.grapheme.as_str())
        .collect();
    assert!(text.contains("CRASH WORKING"));
}
