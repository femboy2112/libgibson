//! RGB evidence for the same encounter's graphical realization. These probes
//! distinguish rendered light/depth from labels and preserve full visual replay.
#[allow(dead_code)]
#[path = "../examples/acid_vs_crash.rs"]
mod encounter;

use encounter::cyber::{self, VisualMode};
use encounter::Encounter;
use gibson::raster::RgbRaster;
use gibson::{
    compute_diff, compute_layout, paint, Node, RenderMode, Renderer, StoryEvent, Surface,
    TerminalSession,
};
use std::{collections::BTreeSet, time::Duration};

fn painted(mut root: Node, width: u16, height: u16) -> Surface {
    compute_layout(&mut root, width, height).unwrap();
    let mut surface = Surface::new(width, height);
    paint(&root, &mut surface);
    surface
}
fn prepared(stage: &str) -> Encounter {
    let mut encounter = Encounter::new(stage, false);
    encounter.set_visual_mode(VisualMode::Cyber);
    for millis in [13, 33, 71, 17, 107, 29, 53, 19, 83, 41, 137, 23] {
        encounter.update(Duration::from_millis(millis), &[]);
    }
    encounter
}
fn graphic(encounter: &Encounter, width: u16, height: u16) -> cyber::CyberFrame {
    cyber::render(
        encounter.battle(),
        encounter.visual_history(),
        width,
        height.saturating_sub(5),
        false,
        1.,
    )
}
fn dump(label: &str, raster: &RgbRaster) {
    if let Some(directory) = std::env::var_os("DUMP_ACID_RGB") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        raster
            .write_ppm(std::fs::File::create(directory.join(format!("{label}.ppm"))).unwrap())
            .unwrap();
    }
}

#[test]
fn three_hero_frames_have_real_rgb_volume_and_repeat_exactly() {
    for stage in ["first-breach", "trace", "takeover"] {
        let encounter = prepared(stage);
        let first = graphic(&encounter, 120, 32);
        let repeated = graphic(&encounter, 120, 32);
        assert_eq!(
            first.raster, repeated.raster,
            "paint must not advance trails: {stage}"
        );
        assert_eq!(first.surface, repeated.surface);
        let colors: BTreeSet<_> = first.raster.pixels().iter().copied().collect();
        let bright = first
            .raster
            .pixels()
            .iter()
            .filter(|&&(r, g, b)| r.max(g).max(b) > 150)
            .count();
        let dark = first
            .raster
            .pixels()
            .iter()
            .filter(|&&(r, g, b)| r.max(g).max(b) < 35)
            .count();
        eprintln!(
            "{stage}: colors={} bright={bright} dark={dark} triangles={} z={} fields={} trails={}",
            colors.len(),
            first.metrics.triangles.triangles_drawn,
            first.metrics.triangles.z_tests,
            first.metrics.field_samples,
            first.metrics.feedback_passes
        );
        // Broad structural checks, deliberately not a checksum of art direction.
        assert!(
            colors.len() > 100,
            "{stage}: shading and light require RGB diversity"
        );
        assert!(
            bright > 10 && dark > 100,
            "{stage}: light must have contrast and breathing room"
        );
        assert!(first.metrics.triangles.triangles_drawn > 10);
        assert!(first.metrics.triangles.z_tests > 100);
        assert_eq!(first.metrics.pixels, 120 * 54);
        assert_eq!(
            first.metrics.field_samples,
            120 * 54 * encounter.battle().graph.nodes.len()
        );
        assert_eq!(first.metrics.feedback_passes, 12);
        dump(stage, &first.raster);
    }
}

#[test]
fn irregular_input_replay_reconstructs_world_visual_history_and_full_frame() {
    let mut original = Encounter::new("first-breach", false);
    original.set_visual_mode(VisualMode::Cyber);
    for (millis, commands) in [
        (13, vec![]),
        (107, vec!["trace"]),
        (839, vec!["isolate", "decoy"]),
        (3, vec![]),
        (5003, vec![]),
        (41, vec!["trace token"]),
        (211, vec!["kill"]),
        (1301, vec!["hard isolate"]),
        (19, vec![]),
    ] {
        let events: Vec<_> = commands.into_iter().map(StoryEvent::command).collect();
        original.update(Duration::from_millis(millis), &events);
    }
    let replay = original.replay();
    assert_eq!(original.battle(), replay.battle());
    assert_eq!(original.encounter_trace(), replay.encounter_trace());
    assert_eq!(original.visual_history(), replay.visual_history());
    assert_eq!(original.facts(), replay.facts());
    assert_eq!(original.director().trace(), replay.director().trace());
    assert_eq!(
        original.director().mounted().collect::<Vec<_>>(),
        replay.director().mounted().collect::<Vec<_>>()
    );
    assert_eq!(original.presentation(), replay.presentation());
    assert!(original.replay_matches());
    for (width, height) in [(56, 24), (80, 24), (120, 32), (160, 40)] {
        assert_eq!(
            graphic(&original, width, height).raster,
            graphic(&replay, width, height).raster
        );
        assert_eq!(
            painted(original.frame(width, height), width, height),
            painted(replay.frame(width, height), width, height)
        );
    }
}

#[test]
fn defense_counterfactual_changes_the_picture_without_reading_labels() {
    let mut isolated = prepared("first-breach");
    let mut decoy = prepared("first-breach");
    assert_eq!(isolated.battle(), decoy.battle());
    assert_eq!(
        graphic(&isolated, 120, 32).raster,
        graphic(&decoy, 120, 32).raster
    );
    isolated.command("isolate");
    decoy.command("decoy");
    for _ in 0..30 {
        isolated.tick(Duration::from_millis(100), false);
        decoy.tick(Duration::from_millis(100), false);
    }
    assert_ne!(
        isolated.battle().planner.target,
        decoy.battle().planner.target
    );
    assert_ne!(isolated.battle().planner.path, decoy.battle().planner.path);
    assert_ne!(isolated.facts(), decoy.facts());
    let a = graphic(&isolated, 120, 32);
    let b = graphic(&decoy, 120, 32);
    let changed = a
        .raster
        .pixels()
        .iter()
        .zip(b.raster.pixels())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 100,
        "defense must alter graphical geography, not just HUD copy"
    );
    assert!(isolated.replay_matches() && decoy.replay_matches());
    dump("isolated", &a.raster);
    dump("decoy", &b.raster);
}

#[test]
fn influence_sign_changes_light_without_changing_topology_or_time() {
    let encounter = prepared("first-breach");
    let mut crash = encounter.battle().clone();
    let mut acid = crash.clone();
    for node in &mut crash.graph.nodes {
        node.influence = 1000;
    }
    for node in &mut acid.graph.nodes {
        node.influence = -1000;
    }
    let a = cyber::render(&crash, encounter.visual_history(), 120, 27, false, 1.);
    let b = cyber::render(&acid, encounter.visual_history(), 120, 27, false, 1.);
    let red_minus_green = |r: &RgbRaster| {
        r.pixels()
            .iter()
            .map(|&(red, green, _)| red as i64 - green as i64)
            .sum::<i64>()
    };
    assert!(
        red_minus_green(&b.raster) > red_minus_green(&a.raster) + 10000,
        "signed control must drive cyan versus magenta light"
    );
    assert_eq!(crash.graph.edges, acid.graph.edges);
    assert_eq!(crash.visual_time(), acid.visual_time());
}

#[test]
fn frozen_graphical_frame_has_zero_semantic_delta_footprint_and_wire_bytes() {
    let encounter = prepared("takeover");
    let a = painted(encounter.frame(120, 32), 120, 32);
    let b = painted(encounter.frame(120, 32), 120, 32);
    let diff = compute_diff(Some(&a), &b);
    assert_eq!(diff.exact_changed_cell_count(), 0);
    assert_eq!(diff.affected_cell_count(), 0);
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut terminal = TerminalSession::headless(120, 32);
    let mut wire = Vec::new();
    renderer
        .render(&mut encounter.frame(120, 32), &mut terminal, &mut wire)
        .unwrap();
    assert!(!wire.is_empty());
    wire.clear();
    let (_, _, bytes, _, _) = renderer
        .render(&mut encounter.frame(120, 32), &mut terminal, &mut wire)
        .unwrap();
    assert_eq!(bytes, 0);
    assert!(wire.is_empty());
}

#[test]
fn resize_reprojects_history_without_advancing_or_retaining_old_pixels() {
    let encounter = prepared("trace");
    let history = encounter.visual_history().clone();
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut terminal = TerminalSession::headless(120, 32);
    let mut parser = vt100::Parser::new(32, 120, 0);
    for (width, height) in [(120, 32), (56, 24), (160, 40), (80, 24), (120, 32)] {
        terminal.set_terminal_size(width, height);
        parser.set_size(height, width);
        let mut wire = Vec::new();
        renderer
            .render(
                &mut encounter.frame(width, height),
                &mut terminal,
                &mut wire,
            )
            .unwrap();
        parser.process(&wire);
        let expected = painted(encounter.replay().frame(width, height), width, height);
        let rows: Vec<_> = (0..height)
            .map(|y| {
                (0..width)
                    .filter_map(|x| {
                        let c = expected.get(x, y).unwrap();
                        (!c.is_continuation).then_some(c.glyph.grapheme.as_str())
                    })
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect();
        assert_eq!(
            parser
                .screen()
                .rows(0, width)
                .map(|r| r.trim_end().to_string())
                .collect::<Vec<_>>(),
            rows,
            "resize to {width}x{height}"
        );
        assert_eq!(
            graphic(&encounter, width, height).raster,
            graphic(&encounter.replay(), width, height).raster
        );
        assert_eq!(encounter.visual_history(), &history);
    }
}

#[test]
fn visual_mode_is_a_projection_and_feedback_history_is_bounded() {
    let mut encounter = prepared("first-breach");
    for _ in 0..70 {
        encounter.update(Duration::from_millis(17), &[]);
    }
    let history = encounter.visual_history().clone();
    let world = encounter.battle().clone();
    let trace = encounter.encounter_trace().clone();
    assert_eq!(graphic(&encounter, 120, 32).metrics.feedback_passes, 48);
    let cyber = painted(encounter.frame(120, 32), 120, 32);
    encounter.set_visual_mode(VisualMode::Flat);
    let flat = painted(encounter.frame(120, 32), 120, 32);
    assert_ne!(cyber, flat);
    encounter.set_visual_mode(VisualMode::Cyber);
    assert_eq!(cyber, painted(encounter.frame(120, 32), 120, 32));
    assert_eq!(encounter.battle(), &world);
    assert_eq!(encounter.visual_history(), &history);
    assert_eq!(encounter.encounter_trace(), &trace);
}

#[test]
fn legacy_ending_reassembles_ui_on_recorded_visual_time_without_mutating_final_world() {
    for stage in ["crash-win", "stalemate"] {
        let mut encounter = Encounter::new(stage, false);
        encounter.set_legacy(true);
        assert!(encounter.director().is_finished());
        let world = encounter.battle().clone();
        let initial = painted(encounter.frame(120, 32), 120, 32);
        assert_eq!(
            cyber::immersion(
                encounter.battle(),
                encounter.visual_history(),
                VisualMode::Auto,
                0.
            ),
            1.
        );
        for dt in [13, 107, 839, 3001, 1040] {
            encounter.update(Duration::from_millis(dt), &[]);
        }
        assert_eq!(
            encounter.battle(),
            &world,
            "aftermath must preserve scars/outcome"
        );
        assert_eq!(
            cyber::immersion(
                encounter.battle(),
                encounter.visual_history(),
                VisualMode::Auto,
                0.
            ),
            0.
        );
        let reconstructed = painted(encounter.frame(120, 32), 120, 32);
        assert_ne!(initial, reconstructed);
        assert_eq!(
            encounter.visual_history(),
            encounter.replay().visual_history()
        );
        assert_eq!(
            reconstructed,
            painted(encounter.replay().frame(120, 32), 120, 32)
        );
        encounter.set_visual_mode(VisualMode::Flat);
        assert_eq!(reconstructed, painted(encounter.frame(120, 32), 120, 32));
    }
}

#[test]
fn autonomous_default_visits_cyberspace_and_reconstructs_without_input() {
    let mut e = Encounter::new("quiet", false);
    let mut seen = false;
    for _ in 0..1000 {
        e.tick(Duration::from_millis(100), true);
        if e.battle().outcome.is_none()
            && cyber::immersion(e.battle(), e.visual_history(), VisualMode::Auto, 0.0) > 0.99
        {
            seen = true;
        }
    }
    assert!(
        seen,
        "the default movie must actually enter the graphical realization before resolution"
    );
    assert!(e.director().is_finished());
    assert_eq!(
        cyber::immersion(e.battle(), e.visual_history(), VisualMode::Auto, 0.0),
        0.0
    );
    assert!(e.replay_matches());
}

#[test]
fn moving_graphics_report_real_damage_and_wire_cost() {
    for stage in ["first-breach", "trace", "takeover"] {
        let mut e = prepared(stage);
        let previous = painted(e.frame(160, 40), 160, 40);
        let mut renderer = Renderer::new(RenderMode::Fullscreen);
        let mut terminal = TerminalSession::headless(160, 40);
        let mut wire = Vec::new();
        renderer
            .render(&mut e.frame(160, 40), &mut terminal, &mut wire)
            .unwrap();
        e.update(Duration::from_millis(17), &[]);
        let start = std::time::Instant::now();
        let node = e.frame(160, 40);
        let generation = start.elapsed();
        let next = painted(node.clone(), 160, 40);
        let diff = compute_diff(Some(&previous), &next);
        wire.clear();
        let (_, _, bytes, _, _) = renderer
            .render(&mut node.clone(), &mut terminal, &mut wire)
            .unwrap();
        eprintln!("{stage} 160x40/17ms: exact={} affected={} bytes={bytes} generation_us={} (observation, not a timing assertion)",diff.exact_changed_cell_count(),diff.affected_cell_count(),generation.as_micros());
        assert!(diff.exact_changed_cell_count() > 0);
        assert!(diff.affected_cell_count() >= diff.exact_changed_cell_count());
        assert!(bytes > 0);
        wire.clear();
        let (_, _, frozen, _, _) = renderer
            .render(&mut node.clone(), &mut terminal, &mut wire)
            .unwrap();
        assert_eq!(frozen, 0);
        assert!(wire.is_empty());
    }
}
