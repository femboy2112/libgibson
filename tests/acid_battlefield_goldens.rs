//! Semantic battlefield snapshots include glyphs AND the realized ownership
//! style grammar. A text-only screenshot cannot detect a moving tint frontier.
#[allow(dead_code)]
#[path = "../examples/acid_vs_crash.rs"]
mod encounter;

use encounter::{
    battle::{Control, Goal, NodeId},
    Encounter,
};
use gibson::{compute_layout, paint, Color, Surface};
use std::{fmt::Write, path::PathBuf, time::Duration};

fn snapshot(encounter: &Encounter, mono: bool) -> String {
    let (width, height) = if mono { (80, 24) } else { (120, 32) };
    let mut root = encounter.frame(width, height);
    compute_layout(&mut root, width, height).unwrap();
    let mut surface = Surface::new(width, height);
    paint(&root, &mut surface);
    let mut out = String::new();
    writeln!(out, "GLYPHS {width}x{height}").unwrap();
    for y in 0..height {
        let row: String = (0..width)
            .filter_map(|x| {
                let cell = surface.get(x, y).unwrap();
                (!cell.is_continuation).then_some(cell.glyph.grapheme.as_str())
            })
            .collect();
        writeln!(out, "{}", row.trim_end()).unwrap();
    }
    writeln!(
        out,
        "STYLE GRAMMAR: A=Acid/reverse C=Crash/bright !=warning .=neutral"
    )
    .unwrap();
    for y in 0..height {
        let row: String = (0..width)
            .map(|x| {
                let c = surface.get(x, y).unwrap();
                if c.style.reverse || c.style.fg == Some(Color::Rgb(255, 79, 192)) {
                    'A'
                } else if c.style.fg == Some(Color::Rgb(76, 218, 255)) || (mono && c.style.bold) {
                    'C'
                } else if c.style.fg == Some(Color::Rgb(255, 200, 91)) {
                    '!'
                } else {
                    '.'
                }
            })
            .collect();
        writeln!(out, "{row}").unwrap();
    }
    out
}

#[test]
fn battlefield_semantic_goldens() {
    // The same public commands used by a human drive these states. No world
    // mutation or forced StoryDirector jump is used to obtain a picture.
    let cases: &[(&str, &str, &[&str], usize, bool)] = &[
        ("route_pivot", "first-breach", &["isolate"], 20, false),
        ("auth_frontier", "first-breach", &[], 3, false),
        (
            "isolation_break",
            "display-intrusion",
            &["hard isolate"],
            5,
            false,
        ),
        ("decoy_split", "first-breach", &["decoy"], 20, false),
        ("trace_reverse", "route-contested", &["trace"], 5, false),
        ("directed_takeover", "takeover", &[], 10, false),
        (
            "costly_crash_win",
            "climax",
            &["hard isolate", "cut link"],
            1,
            false,
        ),
        ("mono_pivot", "first-breach", &["isolate"], 20, true),
    ];
    let update = std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1");
    for (label, stage, commands, ticks, mono) in cases {
        let mut encounter = Encounter::new(stage, *mono);
        // Historical flat-machine snapshots remain a separate realization.
        encounter.set_visual_mode(encounter::cyber::VisualMode::Flat);
        for command in *commands {
            encounter.command(command);
        }
        for _ in 0..*ticks {
            encounter.tick(Duration::from_millis(100), false);
        }
        assert!(encounter.replay_matches(), "{label} cannot replay");
        let world = encounter.battle();
        match *label {
            "route_pivot" | "mono_pivot" => {
                assert!(world.graph.node(NodeId::Route).isolated);
                assert!(!world.planner.path.contains(&NodeId::Route));
            }
            "auth_frontier" => assert!(world.graph.node(NodeId::Auth).influence < 350),
            "isolation_break" => {
                assert!(world.graph.nodes[..6].iter().filter(|n| !n.visible).count() >= 4)
            }
            "decoy_split" => {
                assert!(world.graph.node(NodeId::Decoy).visible);
                assert_eq!(world.planner.target, NodeId::Decoy);
            }
            "trace_reverse" => {
                assert!(world.trace_confidence > 0);
                assert_eq!(world.planner.goal, Goal::EvadeTrace);
            }
            "directed_takeover" => {
                assert_eq!(world.graph.node(NodeId::Display).owner(), Control::Acid);
                assert!(world.graph.route(NodeId::Modem, NodeId::Display).is_some());
            }
            "costly_crash_win" => assert_eq!(world.outcome_quality, "costly contain"),
            _ => unreachable!(),
        }
        let actual = snapshot(&encounter, *mono);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/goldens")
            .join(format!("acid_battle_{label}.txt"));
        if update {
            std::fs::write(&path, &actual).unwrap();
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing {}; generate with UPDATE_GOLDENS=1 cargo test --test acid_battlefield_goldens and review", path.display()));
        assert_eq!(actual, expected, "battlefield golden {label}");
    }
}
