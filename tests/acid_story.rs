//! Macro-story and semantic projection checks. Round II deliberately replaces
//! scripted tactical micro-beats with a continuous topology reducer.
#[allow(dead_code)]
#[path = "../examples/acid_vs_crash.rs"]
mod encounter;
use encounter::{
    battle::{Control, NodeId, Outcome},
    Encounter, STAGES,
};
use gibson::StoryEvent;
use std::time::Duration;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn mounted(world: &Encounter, bundle: &str) -> bool {
    world.director().mounted().any(|b| b == bundle)
}
fn advance(world: &mut Encounter, millis: u64) {
    for _ in 0..millis / 100 {
        world.tick(ms(100), false);
    }
}
fn assert_projection(world: &Encounter) {
    assert!(world.replay_matches());
    let replay = world.story().replay(world.director().trace());
    assert_eq!(replay.trace(), world.director().trace());
    assert_eq!(replay.facts(), world.facts());
    for node in &world.battle().graph.nodes {
        assert_eq!(
            world.facts().text(&format!("{}-control", node.id.name())),
            node.owner().as_str(),
            "{} ownership projection",
            node.id.name()
        );
        assert_eq!(
            world.facts().bool(&format!("{}-isolated", node.id.name())),
            node.isolated
        );
        assert!((-1000..=1000).contains(&node.influence));
        assert!(node.integrity <= 1000);
    }
}
#[test]
fn every_inspection_stage_has_valid_state_and_exact_irregular_replay() {
    for stage in STAGES {
        let mut world = Encounter::new(stage, false);
        assert_eq!(world.story().validate(), Ok(()));
        assert_projection(&world);
        for (i, dt) in [13, 107, 3, 839, 41, 5003, 19].into_iter().enumerate() {
            if i == 1 {
                world.command("trace");
            }
            if i == 3 {
                world.command("decoy");
            }
            world.tick(ms(dt), false);
            assert_projection(&world);
        }
    }
}
#[test]
fn branch_inspection_stages_include_real_topology_and_defense_causes() {
    let trace = Encounter::new("trace", false);
    assert!(trace.battle().trace_confidence >= 650);
    assert!(trace.battle().planner.memory.trace_attempts > 0);
    let isolated = Encounter::new("sidepath", false);
    assert!(isolated.battle().graph.node(NodeId::Route).isolated);
    let decoy = Encounter::new("decoy", false);
    assert!(decoy.battle().graph.node(NodeId::Decoy).visible);
    assert!(decoy
        .battle()
        .graph
        .route(NodeId::Modem, NodeId::Decoy)
        .is_some());
    for world in [&trace, &isolated, &decoy] {
        assert_projection(world);
    }
}
#[test]
fn counterplay_is_in_beat_and_routes_reconverge_at_macro_finale() {
    let mut worlds = [
        Encounter::new("first-breach", false),
        Encounter::new("first-breach", false),
    ];
    for (world, command) in worlds.iter_mut().zip(["isolate", "decoy"]) {
        let before = world.director().current_beat().to_owned();
        world.command(command);
        assert_eq!(world.director().current_beat(), before);
        advance(world, 3000);
    }
    assert_ne!(
        worlds[0].battle().planner.path,
        worlds[1].battle().planner.path
    );
    for world in &mut worlds {
        for _ in 0..700 {
            if world.director().current_beat() == "takeover" {
                break;
            }
            world.tick(ms(100), false);
        }
        assert_eq!(world.director().current_beat(), "takeover");
        assert_projection(world);
    }
}
#[test]
fn ordered_multiple_commands_are_recorded_with_costs_and_replay() {
    let mut world = Encounter::new("first-breach", false);
    let events = [
        StoryEvent::command("trace"),
        StoryEvent::command("isolate"),
        StoryEvent::command("decoy"),
    ];
    world.update(ms(1), &events);
    assert_eq!(world.encounter_trace().steps.last().unwrap().events, events);
    assert!(world.battle().graph.node(NodeId::Route).isolated);
    assert!(world.battle().graph.node(NodeId::Decoy).visible);
    assert!(world.battle().awareness > 0);
    assert!(world.battle().resources < 1000);
    assert_projection(&world);
}
#[test]
fn cut_link_contains_subsystems_but_retains_structural_scars() {
    let mut world = Encounter::new("climax", false);
    let integrity = world.battle().graph.nodes.each_ref().map(|n| n.integrity);
    let scars = world.battle().quality.altered_files;
    world.command("cut link");
    assert_eq!(world.battle().outcome, Some(Outcome::Crash));
    assert!(world.director().is_finished());
    assert_eq!(
        world.battle().graph.nodes.each_ref().map(|n| n.integrity),
        integrity
    );
    assert_eq!(world.battle().quality.altered_files, scars);
    assert!(scars > 0);
    assert!(world
        .battle()
        .graph
        .nodes
        .iter()
        .all(|n| n.owner() == Control::Crash));
    for bundle in ["auth-presence", "display-presence", "takeover", "ghost"] {
        assert!(!mounted(&world, bundle));
    }
    assert_projection(&world);
}
#[test]
fn acid_surrender_is_temporary_and_release_does_not_repair_damage() {
    let mut world = Encounter::new("climax", false);
    let integrity = world.battle().graph.node(NodeId::Auth).integrity;
    world.command("let her in");
    assert_eq!(world.battle().outcome, Some(Outcome::Acid));
    assert_eq!(
        world.battle().graph.node(NodeId::Display).owner(),
        Control::Acid
    );
    assert!(mounted(&world, "acid-victory"));
    advance(&mut world, 8000);
    assert!(!world.battle().remote_active);
    assert!(!mounted(&world, "acid-victory"));
    assert_eq!(world.battle().graph.node(NodeId::Auth).integrity, integrity);
    assert_projection(&world);
}
#[test]
fn trace_is_earned_and_changes_ending_quality() {
    let mut world = Encounter::new("climax", false);
    world.command("turn trace");
    assert!(world.battle().outcome.is_none());
    for _ in 0..3 {
        world.command("trace");
        advance(&mut world, 1300);
    }
    assert!(world.battle().trace_confidence >= 800);
    world.command("turn trace");
    assert_eq!(world.battle().outcome, Some(Outcome::Crash));
    assert_eq!(world.battle().outcome_quality, "traced contain");
    assert_projection(&world);
}
#[test]
fn final_decoy_requires_an_occupied_mirror_not_just_a_command_name() {
    let mut world = Encounter::new("climax", false);
    world.command("spring decoy");
    assert!(world.battle().outcome.is_none());
    world.command("decoy");
    world.command("spring decoy");
    assert!(
        world.battle().outcome.is_none(),
        "empty decoy cannot trap an absent lease"
    );
    for _ in 0..160 {
        if world.battle().graph.node(NodeId::Decoy).influence < 0 {
            break;
        }
        world.tick(ms(100), false);
    }
    assert!(world.battle().graph.node(NodeId::Decoy).influence < 0);
    world.command("spring decoy");
    assert_eq!(world.battle().outcome, Some(Outcome::Crash));
    assert_projection(&world);
}
#[test]
fn deterministic_auto_and_idle_complete_with_repeatable_worlds() {
    for auto in [true, false] {
        let mut a = Encounter::new("quiet", false);
        let mut b = Encounter::new("quiet", false);
        for _ in 0..2000 {
            if a.director().is_finished() {
                break;
            }
            a.tick(ms(100), auto);
            b.tick(ms(100), auto);
        }
        assert!(a.director().is_finished());
        assert!(a.battle().outcome.is_some());
        assert_eq!(a.battle(), b.battle());
        assert_eq!(a.encounter_trace(), b.encounter_trace());
        assert_projection(&a);
    }
}
#[test]
fn killing_one_lease_recovers_ground_without_erasing_other_footholds() {
    let mut world = Encounter::new("first-breach", false);
    let before = world.battle().graph.nodes.each_ref().map(|n| n.influence);
    world.command("kill");
    assert_eq!(
        world
            .battle()
            .graph
            .nodes
            .iter()
            .zip(before)
            .filter(|(n, prior)| n.influence > *prior)
            .count(),
        1
    );
    assert!(world.battle().graph.node(NodeId::Modem).influence < 0);
    assert!(world.battle().resources < 1000);
    assert_projection(&world);
}
#[test]
fn early_route_isolation_cannot_be_overridden_by_an_act_entry() {
    let mut world = Encounter::new("signature", false);
    world.command("isolate");
    advance(&mut world, 16000);
    assert!(world
        .battle()
        .graph
        .route(NodeId::Modem, NodeId::Route)
        .is_none());
    assert_eq!(
        world.battle().graph.node(NodeId::Route).owner(),
        Control::Crash
    );
    assert_projection(&world);
}
#[test]
fn early_auth_isolation_prevents_unexplained_reacquisition() {
    let mut world = Encounter::new("route-contested", false);
    world.command("isolate auth");
    advance(&mut world, 18000);
    assert_eq!(
        world.battle().graph.node(NodeId::Auth).owner(),
        Control::Crash
    );
    assert!(!mounted(&world, "auth-presence"));
    assert_projection(&world);
}
#[test]
fn hard_isolation_cleanly_interrupts_takeover_and_costs_telemetry() {
    let mut world = Encounter::new("takeover", false);
    assert!(mounted(&world, "takeover"));
    world.command("hard isolate");
    assert!(world.facts().bool("crash-blind"));
    assert!(world
        .battle()
        .graph
        .route(NodeId::Modem, NodeId::Display)
        .is_none());
    for bundle in ["takeover", "display-presence", "auth-presence"] {
        assert!(!mounted(&world, bundle), "{bundle}");
    }
    assert_projection(&world);
}
#[test]
fn dramatic_timeout_never_reopens_a_severed_display_path() {
    let mut world = Encounter::new("trap", false);
    world.command("hard isolate");
    advance(&mut world, 18000);
    assert_eq!(
        world.battle().graph.node(NodeId::Display).owner(),
        Control::Crash
    );
    assert!(!mounted(&world, "takeover"));
    assert!(!mounted(&world, "display-presence"));
    assert!(world
        .battle()
        .graph
        .route(NodeId::Modem, NodeId::Display)
        .is_none());
    assert_projection(&world);
}
