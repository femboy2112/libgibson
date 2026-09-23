#[allow(dead_code)]
#[path = "../examples/acid_vs_crash/battle.rs"]
mod battle;

use battle::{AcidPlanner, Control, EncounterModel, Goal, NodeId, Tactic};
use std::time::Duration;

fn advance(model: &mut EncounterModel, ms: u64) {
    model.update(Duration::from_millis(ms), &[]);
}
fn command(model: &mut EncounterModel, command: &str) {
    model.update(Duration::ZERO, &[command.into()]);
}

#[test]
fn severed_edges_cannot_carry_pressure_or_reachability() {
    let mut model = EncounterModel::new("first-breach", 42);
    command(&mut model, "isolate");
    advance(&mut model, 6000);
    assert!(model.graph.route(NodeId::Modem, NodeId::Route).is_none());
    assert!(model
        .graph
        .edges
        .iter()
        .filter(|e| e.from == NodeId::Route || e.to == NodeId::Route)
        .all(|e| !e.connected && e.pressure == 0));
    assert!(!model.planner.path.contains(&NodeId::Route));
    assert!(model.graph.route(NodeId::Modem, NodeId::Display).is_some());
}

#[test]
fn no_route_means_no_magical_display_occupation() {
    let mut model = EncounterModel::new("route-contested", 42);
    command(&mut model, "hard isolate");
    advance(&mut model, 30000);
    assert_eq!(model.graph.node(NodeId::Display).owner(), Control::Crash);
    assert_eq!(model.takeover, 0);
    assert!(!model.planner.path.contains(&NodeId::Display));
}

#[test]
fn decoy_is_a_reachable_target_with_a_resource_cost() {
    let mut model = EncounterModel::new("first-breach", 42);
    assert!(model.graph.route(NodeId::Modem, NodeId::Decoy).is_none());
    command(&mut model, "decoy");
    assert_eq!(model.resources, 750);
    assert!(model.graph.route(NodeId::Modem, NodeId::Decoy).is_some());
    assert_eq!(model.planner.target, NodeId::Decoy);
    assert_eq!(model.planner.tactic, Tactic::AttackDecoy);
    assert!(!model.action_status("decoy").available);
}

#[test]
fn hard_isolate_sacrifices_visibility_and_capabilities() {
    let mut model = EncounterModel::new("display-intrusion", 42);
    command(&mut model, "hard isolate");
    assert_eq!(model.quality.collateral_isolation, 4);
    for id in [NodeId::Route, NodeId::Auth, NodeId::Files, NodeId::Display] {
        assert!(!model.graph.node(id).visible);
    }
    assert!(!model.action_status("decoy").available);
    advance(&mut model, 2500);
    assert!(model.graph.node(NodeId::Display).influence > 0);
}

#[test]
fn trace_cost_provokes_evasion_without_free_progress() {
    let mut model = EncounterModel::new("route-contested", 42);
    let time = model.elapsed_ms;
    command(&mut model, "trace");
    assert_eq!(model.elapsed_ms, time);
    assert_eq!(model.trace_confidence, 280);
    assert_eq!(model.awareness, 180);
    assert_eq!(model.resources, 900);
    assert_eq!(model.planner.goal, Goal::EvadeTrace);
    assert_eq!(model.planner.tactic, Tactic::SplitRoute);
    command(&mut model, "trace");
    assert_eq!(
        model.trace_confidence, 280,
        "cooldown rejects repeated input"
    );
}

#[test]
fn repeated_strategy_changes_memory_and_decoy_recognition() {
    let mut model = EncounterModel::new("first-breach", 42);
    command(&mut model, "isolate");
    advance(&mut model, 900);
    command(&mut model, "isolate");
    assert_eq!(model.planner.memory.route_isolations, 2);
    assert_eq!(model.planner.goal, Goal::PunishIsolation);
    command(&mut model, "decoy");
    advance(&mut model, 7000);
    assert!(model.planner.memory.decoys_seen > 0);
    assert_ne!(model.planner.target, NodeId::Decoy);
}

#[test]
fn planner_is_deterministic_and_every_candidate_is_legal() {
    let mut a = EncounterModel::new("first-breach", 923);
    command(&mut a, "isolate");
    let mut p: AcidPlanner = a.planner.clone();
    let first = a.planner.choose(&a.graph, 530, 200, a.elapsed_ms);
    let second = p.choose(&a.graph, 530, 200, a.elapsed_ms);
    assert_eq!(first, second);
    assert_eq!(a.planner, p);
    for candidate in &p.candidates {
        assert!(a.graph.route(NodeId::Modem, candidate.target).is_some());
        assert!(candidate
            .path
            .windows(2)
            .all(|p| a.graph.connected(p[0], p[1])));
    }
}

#[test]
fn continuous_control_is_separate_from_integrity() {
    let mut model = EncounterModel::new("quiet", 42);
    let node = &mut model.graph.nodes[NodeId::Auth.index()];
    for (influence, owner) in [
        (1000, Control::Crash),
        (351, Control::Crash),
        (350, Control::Contested),
        (0, Control::Contested),
        (-350, Control::Contested),
        (-351, Control::Acid),
        (-1000, Control::Acid),
    ] {
        node.influence = influence;
        assert_eq!(node.owner(), owner);
        assert_eq!(node.integrity, 1000);
    }
}

#[test]
fn raw_irregular_update_inputs_replay_every_world_field() {
    let inputs = [
        (31, Some("trace")),
        (127, None),
        (1571, Some("decoy")),
        (2813, None),
        (907, Some("isolate")),
        (3377, None),
        (2701, Some("kill")),
        (4349, None),
        (5001, Some("hard isolate")),
        (871, None),
    ];
    let mut original = EncounterModel::new("first-breach", 178);
    let mut recorded = vec![];
    for (ms, command) in inputs {
        let commands: Vec<String> = command.into_iter().map(str::to_owned).collect();
        let dt = Duration::from_millis(ms);
        let milestones = original.update(dt, &commands);
        recorded.push((dt, commands, milestones, original.clone()));
    }
    let mut replay = EncounterModel::new("first-breach", 178);
    for (dt, commands, milestones, state) in recorded {
        assert_eq!(replay.update(dt, &commands), milestones);
        assert_eq!(replay, state);
    }
}

#[test]
fn isolation_and_decoy_counterfactuals_change_tactics_and_world() {
    let prefix = EncounterModel::new("first-breach", 42);
    let mut isolate = prefix.clone();
    let mut decoy = prefix;
    command(&mut isolate, "isolate");
    command(&mut decoy, "decoy");
    advance(&mut isolate, 2000);
    advance(&mut decoy, 2000);
    assert_ne!(isolate.planner.target, decoy.planner.target);
    assert_ne!(isolate.planner.path, decoy.planner.path);
    assert_ne!(isolate.graph, decoy.graph);
    assert!(isolate
        .planner
        .path
        .windows(2)
        .all(|p| isolate.graph.connected(p[0], p[1])));
    assert!(decoy
        .planner
        .path
        .windows(2)
        .all(|p| decoy.graph.connected(p[0], p[1])));
}

#[test]
fn huge_duration_is_bounded_and_quiet_time_partition_is_exact() {
    let mut huge = EncounterModel::new("quiet", 42);
    advance(&mut huge, u64::MAX);
    assert!(huge.outcome.is_some());
    assert!(huge.elapsed_ms <= 90000);
    let mut a = EncounterModel::new("quiet", 42);
    let mut b = a.clone();
    advance(&mut a, 137);
    advance(&mut a, 863);
    advance(&mut b, 1000);
    assert_eq!(a, b);
}

#[test]
fn automatic_defender_uses_state_and_finishes() {
    let mut model = EncounterModel::new("quiet", 42);
    for _ in 0..1500 {
        let commands = model.defender_command().into_iter().collect::<Vec<_>>();
        model.update(Duration::from_millis(50), &commands);
        if model.outcome.is_some() {
            break;
        }
    }
    assert!(model.outcome.is_some());
    assert!(model.planner.memory.trace_attempts > 0);
    assert!(model.quality.collateral_isolation > 0);
}

#[test]
fn fortified_intermediate_node_stops_forward_progress() {
    let mut model = EncounterModel::new("first-breach", 42);
    // The route to DISPLAY passes FILES. A strong FILES defense must be
    // pressured before the remote lease may continue into DISPLAY.
    model.graph.nodes[NodeId::Files.index()].influence = 1000;
    model.graph.nodes[NodeId::Display.index()].influence = 1000;
    let mut reached_files = false;
    for _ in 0..160 {
        advance(&mut model, 50);
        if model.planner.location == NodeId::Files
            && model.graph.node(NodeId::Files).influence > 350
        {
            reached_files = true;
            assert_eq!(model.graph.node(NodeId::Display).influence, 1000);
            assert_ne!(model.planner.location, NodeId::Display);
        }
    }
    assert!(reached_files);
}

#[test]
fn final_moves_and_unknown_inputs_have_honest_availability() {
    let mut model = EncounterModel::new("quiet", 42);
    for command_name in ["cut link", "let her in", "turn trace", "spring decoy"] {
        assert!(!model.action_status(command_name).available);
        command(&mut model, command_name);
    }
    command(&mut model, "unknown command");
    assert!(model.outcome.is_none());
    assert_eq!(model.quality.first_response_ms, None);
    assert_eq!(model.planner.memory.passive_ticks, 0);
}

#[test]
fn released_acid_leaves_scars_without_stale_traffic() {
    let mut model = EncounterModel::new("takeover", 42);
    advance(&mut model, 600);
    let integrity = model.graph.node(NodeId::Auth).integrity;
    command(&mut model, "let her in");
    advance(&mut model, 10000);
    assert!(!model.remote_active);
    assert_eq!(model.takeover, 0);
    assert!(model.graph.edges.iter().all(|edge| edge.pressure == 0));
    assert!(model.graph.nodes.iter().all(|node| node.activity == 0));
    assert_eq!(model.graph.node(NodeId::Auth).integrity, integrity);
    assert!(integrity < 1000);
}

#[test]
fn only_explicit_invitation_reopens_a_severed_display_corridor() {
    let mut model = EncounterModel::new("climax", 42);
    command(&mut model, "hard isolate");
    advance(&mut model, 6000);
    assert!(model.graph.route(NodeId::Modem, NodeId::Display).is_none());
    assert!(!model.graph.node(NodeId::Display).visible);
    command(&mut model, "let her in");
    assert_eq!(
        model.graph.route(NodeId::Modem, NodeId::Display),
        Some(vec![
            NodeId::Modem,
            NodeId::Shell,
            NodeId::Files,
            NodeId::Display
        ])
    );
    assert!(model.graph.node(NodeId::Files).visible);
    assert_eq!(model.graph.node(NodeId::Display).owner(), Control::Acid);
    assert!(model
        .receipts
        .iter()
        .any(|line| line.starts_with("INVITE /")));
    assert!(
        model.graph.node(NodeId::Route).isolated,
        "invitation opens only the explicit corridor"
    );
}

#[test]
fn recognized_mirror_draws_a_real_approach_then_retreat_without_capture() {
    let mut model = EncounterModel::new("first-breach", 42);
    command(&mut model, "decoy");
    advance(&mut model, 8000);
    assert_eq!(model.planner.memory.decoys_seen, 1);
    command(&mut model, "decoy");
    assert_eq!(model.planner.tactic, Tactic::Feint);
    let mirror_influence = model.graph.node(NodeId::Decoy).influence;
    let mut approached = false;
    for _ in 0..200 {
        advance(&mut model, 50);
        if model.planner.tactic == Tactic::Feint && model.planner.progress > 0 {
            approached = true;
            assert!(model
                .planner
                .path
                .windows(2)
                .all(|p| model.graph.connected(p[0], p[1])));
        }
        assert_eq!(model.graph.node(NodeId::Decoy).influence, mirror_influence);
        if model.planner.memory.feints_completed > 0 {
            break;
        }
    }
    assert!(approached);
    assert_eq!(model.planner.memory.feints_completed, 1);
    assert_ne!(model.planner.target, NodeId::Decoy);
    assert_ne!(model.planner.location, NodeId::Decoy);
    assert!(model
        .receipts
        .iter()
        .any(|line| line.starts_with("FEINT /")));
}
