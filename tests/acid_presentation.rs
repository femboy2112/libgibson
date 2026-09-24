//! Cinematic composition remains a replayable observation of the same machine.
#[allow(dead_code)]
#[path = "../examples/acid_vs_crash.rs"]
mod encounter;

use encounter::battle::{EncounterModel, NodeId};
use encounter::cyber::{self, VisualHistory};
use encounter::presentation::{self, ShotKind, ShotPlan};
use encounter::Encounter;
use gibson::{
    compute_diff, compute_layout, paint, Node, RenderMode, Renderer, StoryEvent, Surface,
    TerminalSession,
};
use std::{collections::BTreeSet, time::Duration};

fn shot(encounter: &Encounter, width: u16, height: u16) -> ShotPlan {
    presentation::plan(
        encounter.battle(),
        encounter.visual_history(),
        width,
        height,
    )
}
fn painted(mut node: Node, width: u16, height: u16) -> Surface {
    compute_layout(&mut node, width, height).unwrap();
    let mut surface = Surface::new(width, height);
    paint(&node, &mut surface);
    surface
}
fn text(surface: &Surface) -> String {
    (0..surface.height)
        .map(|y| {
            (0..surface.width)
                .filter_map(|x| {
                    let cell = surface.get(x, y).unwrap();
                    (!cell.is_continuation).then_some(cell.glyph.grapheme.as_str())
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn finite(plan: &ShotPlan) {
    let camera = plan.camera;
    assert!(camera.position.is_finite() && camera.target.is_finite());
    assert!(camera.fov_y.is_finite() && camera.fov_y > 0.1 && camera.fov_y < 2.0);
    let d = camera.position.minus(camera.target);
    assert!(d.x * d.x + d.y * d.y + d.z * d.z > 0.01);
}

#[test]
fn semantic_world_selects_distinct_focal_shots() {
    for (stage, expected, focal) in [
        ("quiet", ShotKind::Establishing, None),
        ("signature", ShotKind::Arrival, Some(NodeId::Modem)),
        (
            "route-contested",
            ShotKind::RouteContest,
            Some(NodeId::Route),
        ),
        ("first-breach", ShotKind::NodeCloseup, Some(NodeId::Auth)),
        ("decoy", ShotKind::Decoy, Some(NodeId::Decoy)),
        ("sidepath", ShotKind::Isolation, Some(NodeId::Route)),
        ("trace", ShotKind::Trace, Some(NodeId::Auth)),
        (
            "display-intrusion",
            ShotKind::DisplayAssault,
            Some(NodeId::Display),
        ),
        ("climax", ShotKind::FinalDuel, Some(NodeId::Display)),
        ("crash-win", ShotKind::CrashWin, Some(NodeId::Display)),
        ("acid-win", ShotKind::AcidWin, Some(NodeId::Display)),
        ("stalemate", ShotKind::Stalemate, Some(NodeId::Display)),
    ] {
        let encounter = Encounter::new(stage, false);
        let before = encounter.battle().clone();
        let plan = shot(&encounter, 120, 28);
        assert_eq!(plan.kind, expected, "{stage}");
        assert_eq!(plan.focal, focal, "{stage}");
        finite(&plan);
        assert_eq!(
            encounter.battle(),
            &before,
            "paint cannot mutate semantic world"
        );
    }
}

#[test]
fn accepted_zero_time_defenses_change_shot_without_a_camera_cut() {
    for (command, expected) in [
        ("trace", ShotKind::Trace),
        ("isolate", ShotKind::Isolation),
        ("decoy", ShotKind::Decoy),
    ] {
        let mut encounter = Encounter::new("route-contested", false);
        let before = shot(&encounter, 120, 28);
        encounter.update(Duration::ZERO, &[StoryEvent::command(command)]);
        let after = shot(&encounter, 120, 28);
        assert_eq!(after.kind, expected, "{command}");
        assert_eq!(
            before.camera, after.camera,
            "{command} must begin at the presented camera"
        );
        assert_eq!(before.field_strength, after.field_strength);
        assert_eq!(before.light_strength, after.light_strength);
        encounter.update(Duration::from_millis(137), &[]);
        let moving = shot(&encounter, 120, 28);
        finite(&moving);
        assert_ne!(moving.camera, before.camera);
        assert!(moving.transition > 0.0 && moving.transition < 1.0);
    }
}

#[test]
fn trace_confidence_is_not_an_endless_trace_action() {
    let mut encounter = Encounter::new("route-contested", false);
    encounter.update(Duration::ZERO, &[StoryEvent::command("trace")]);
    assert_eq!(shot(&encounter, 120, 28).kind, ShotKind::Trace);
    for _ in 0..31 {
        encounter.update(Duration::from_millis(100), &[]);
    }
    assert!(encounter.battle().trace_confidence > 0);
    assert_ne!(shot(&encounter, 120, 28).kind, ShotKind::Trace);
}

#[test]
fn irregular_input_replays_every_shot_camera_and_final_raster() {
    let mut encounter = Encounter::new("route-contested", false);
    let mut sequence = Vec::new();
    for (millis, commands) in [
        (13, vec![]),
        (0, vec!["trace"]),
        (137, vec![]),
        (71, vec![]),
        (401, vec![]),
        (0, vec!["isolate"]),
        (991, vec![]),
        (0, vec!["decoy"]),
        (79, vec![]),
        (1301, vec![]),
        (17, vec![]),
    ] {
        let events = commands
            .into_iter()
            .map(StoryEvent::command)
            .collect::<Vec<_>>();
        encounter.update(Duration::from_millis(millis), &events);
        sequence.push(shot(&encounter, 120, 28));
    }
    let mut replay = Encounter::new(&encounter.encounter_trace().stage, false);
    for (step, expected) in encounter.encounter_trace().steps.iter().zip(&sequence) {
        replay.update(step.dt, &step.events);
        assert_eq!(shot(&replay, 120, 28), *expected);
    }
    assert!(encounter.replay_matches());
    assert_eq!(replay.battle(), encounter.battle());
    assert_eq!(replay.visual_history(), encounter.visual_history());
    assert_eq!(replay.facts(), encounter.facts());
    assert_eq!(
        replay.director().current_beat(),
        encounter.director().current_beat()
    );
    assert_eq!(
        painted(replay.frame(120, 32), 120, 32),
        painted(encounter.frame(120, 32), 120, 32)
    );
}

#[test]
fn five_hero_shots_have_geometry_light_and_distinct_camera_compositions() {
    let mut cameras = Vec::new();
    for stage in [
        "quiet",
        "first-breach",
        "trace",
        "display-intrusion",
        "climax",
    ] {
        let mut encounter = Encounter::new(
            if stage == "trace" {
                "route-contested"
            } else {
                stage
            },
            false,
        );
        if stage == "trace" {
            encounter.command("trace");
            encounter.update(Duration::from_millis(1400), &[]);
        }
        let plan = shot(&encounter, 160, 36);
        let began = std::time::Instant::now();
        let frame = cyber::render_shot(
            encounter.battle(),
            encounter.visual_history(),
            160,
            36,
            false,
            &plan,
        );
        let generation_us = began.elapsed().as_micros();
        let colors = frame
            .raster
            .pixels()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert!(
            colors.len() > 100,
            "{stage}: only {} RGB values",
            colors.len()
        );
        assert!(
            frame.metrics.triangles.triangles_drawn > 5,
            "{stage}: geometry must survive projection"
        );
        assert!(frame.metrics.triangles.z_tests > 100, "{stage}");
        eprintln!(
            "{stage}: shot={:?} RGB={} triangles={} z={} generation_us={generation_us}",
            plan.kind,
            colors.len(),
            frame.metrics.triangles.triangles_drawn,
            frame.metrics.triangles.z_tests
        );
        cameras.push(plan.camera);
        if let Ok(path) = std::env::var("DUMP_ACID_SHOTS") {
            std::fs::create_dir_all(&path).unwrap();
            frame
                .raster
                .write_ppm(std::fs::File::create(format!("{path}/{stage}.ppm")).unwrap())
                .unwrap();
        }
    }
    for (i, a) in cameras.iter().enumerate() {
        for b in &cameras[i + 1..] {
            assert_ne!(a, b, "hero moments need different spatial compositions");
        }
    }
}

#[test]
fn cinematic_controls_survive_all_shots_and_responsive_framing() {
    for stage in [
        "quiet",
        "signature",
        "first-breach",
        "decoy",
        "display-intrusion",
        "climax",
        "crash-win",
        "acid-win",
        "stalemate",
    ] {
        let encounter = Encounter::new(stage, false);
        let history = encounter.visual_history().clone();
        for (width, height) in [(56u16, 24u16), (80, 24), (120, 32), (160, 40)] {
            finite(&shot(&encounter, width, height.saturating_sub(3)));
            let surface = painted(encounter.frame(width, height), width, height);
            let content = text(&surface);
            assert!(
                content.contains("crash >"),
                "{stage}, {width}x{height}: missing control island"
            );
            assert!(
                !content.contains("SYSTEM MAP"),
                "default presentation must not restore the old dashboard"
            );
        }
        assert_eq!(
            encounter.visual_history(),
            &history,
            "resize is observation only"
        );
    }
}

#[test]
fn completed_world_reassembles_as_scarred_aftermath_not_a_dashboard() {
    for stage in ["crash-win", "acid-win", "stalemate"] {
        let mut encounter = Encounter::new(stage, false);
        let integrity = encounter
            .battle()
            .graph
            .nodes
            .clone()
            .map(|node| node.integrity);
        let outcome = encounter.battle().outcome;
        for _ in 0..46 {
            encounter.update(Duration::from_millis(100), &[]);
        }
        assert_eq!(shot(&encounter, 120, 28).kind, ShotKind::Aftermath);
        assert_eq!(
            encounter
                .battle()
                .graph
                .nodes
                .clone()
                .map(|node| node.integrity),
            integrity
        );
        assert_eq!(encounter.battle().outcome, outcome);
        let after = encounter.battle().clone();
        let _ = encounter.frame(120, 32);
        assert_eq!(encounter.battle(), &after);
        assert!(encounter.replay_matches());
        let content = text(&painted(encounter.frame(120, 32), 120, 32));
        assert!(content.contains("crash >"));
        assert!(!content.contains("SYSTEM MAP"));
    }
}

#[test]
fn frozen_default_cinema_has_zero_delta_footprint_and_wire_cost() {
    let encounter = Encounter::new("climax", false);
    let a = painted(encounter.frame(120, 32), 120, 32);
    let b = painted(encounter.frame(120, 32), 120, 32);
    let delta = compute_diff(Some(&a), &b);
    assert_eq!(delta.exact_changed_cell_count(), 0);
    assert_eq!(delta.affected_cell_count(), 0);
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut terminal = TerminalSession::headless(120, 32);
    let mut bytes = Vec::new();
    renderer
        .render(&mut encounter.frame(120, 32), &mut terminal, &mut bytes)
        .unwrap();
    bytes.clear();
    let (_, _, emitted, _, _) = renderer
        .render(&mut encounter.frame(120, 32), &mut terminal, &mut bytes)
        .unwrap();
    assert_eq!(emitted, 0);
    assert!(bytes.is_empty());
}

#[test]
fn narrow_shot_framing_changes_without_modifying_semantic_anchor() {
    let world = EncounterModel::new("first-breach", 42);
    let history = VisualHistory::new(&world);
    let narrow = presentation::plan(&world, &history, 56, 21);
    let wide = presentation::plan(&world, &history, 160, 37);
    assert_eq!(narrow.kind, wide.kind);
    assert_eq!(narrow.focal, wide.focal);
    assert_ne!(narrow.camera, wide.camera);
    finite(&narrow);
    finite(&wide);
}
