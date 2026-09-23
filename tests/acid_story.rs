//! Independent semantic checks of the fictional encounter's public input model.
//! No terminal, wall clock, network or alternate simulation implementation.
#[allow(dead_code)]
#[path = "../examples/acid_vs_crash.rs"]
mod encounter;

use encounter::{Encounter, STAGES};
use gibson::StoryEvent;
use std::time::Duration;

const SYSTEMS: [&str; 6] = ["modem", "route", "auth", "shell", "files", "display"];

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn advance_to(world: &mut Encounter, beat: &str) {
    for _ in 0..1200 {
        if world.director().current_beat() == beat {
            return;
        }
        assert!(!world.director().is_finished(), "ended before {beat}");
        world.tick(ms(100), false);
    }
    panic!(
        "never reached {beat}; at {}",
        world.director().current_beat()
    );
}

fn mounted(world: &Encounter, bundle: &str) -> bool {
    world.director().mounted().any(|name| name == bundle)
}

fn assert_replay(world: &Encounter) {
    assert!(world.replay_matches(), "encounter replay contract diverged");
    let replay = world.story().replay(world.director().trace());
    assert_eq!(replay.trace(), world.director().trace());
    assert_eq!(replay.facts(), world.facts());
    assert_eq!(replay.current_beat(), world.director().current_beat());
    assert_eq!(replay.elapsed(), world.director().elapsed());
    assert_eq!(replay.time_in_beat(), world.director().time_in_beat());
    assert_eq!(replay.is_finished(), world.director().is_finished());
    for system in SYSTEMS {
        let key = format!("{system}-control");
        assert_eq!(replay.facts().text(&key), world.facts().text(&key));
        let key = format!("{system}-integrity");
        assert_eq!(replay.facts().number(&key), world.facts().number(&key));
    }
}

#[test]
fn every_inspection_stage_has_valid_state_and_exact_irregular_replay() {
    for stage in STAGES {
        let mut world = Encounter::new(stage, false);
        assert_eq!(world.director().current_beat(), *stage);
        assert_eq!(world.story().validate(), Ok(()));
        assert_replay(&world);
        for system in SYSTEMS {
            assert!(
                ["Crash", "Contested", "Acid"]
                    .contains(&world.facts().text(&format!("{system}-control"))),
                "stage {stage} omitted owner for {system}"
            );
            assert!((0.0..=100.0).contains(&world.facts().number(&format!("{system}-integrity"))));
        }
        for (index, dt) in [13, 107, 3, 839, 41, 5003, 19].into_iter().enumerate() {
            if index == 1 {
                world.command("trace");
            }
            if index == 3 {
                world.command("decoy");
            }
            world.tick(ms(dt), false);
            assert_replay(&world);
        }
    }
}

#[test]
fn branch_inspection_stages_include_their_semantic_causes() {
    let traced = Encounter::new("trace", false);
    assert!(traced.facts().bool("tracing"));
    assert!(traced.facts().number("trace-confidence") >= 64.0);
    assert!(mounted(&traced, "trace-token"));
    let sidepath = Encounter::new("sidepath", false);
    assert!(
        sidepath.facts().bool("route-isolated") || sidepath.facts().bool("session-killed"),
        "bypass inspection omitted the defense that caused the bypass"
    );
    let decoy = Encounter::new("decoy", false);
    assert!(decoy.facts().bool("decoy-open"));
    assert!(decoy.facts().bool("decoy-taken"));
    assert!(mounted(&decoy, "decoy-reveal"));
    assert!(mounted(&decoy, "decoy-occupied"));
    for world in [&traced, &sidepath, &decoy] {
        assert_replay(world);
    }
}

#[test]
fn player_choices_produce_distinct_local_branches_then_rejoin() {
    for (command, branch, target) in [
        (Some("trace"), "trace", "display"),
        (Some("isolate"), "sidepath", "display"),
        (Some("decoy"), "decoy", "decoy"),
        (None, "pressure", "files"),
    ] {
        let mut world = Encounter::new("route-contested", false);
        if let Some(command) = command {
            world.command(command);
            assert_eq!(world.director().current_beat(), "route-contested");
        }
        advance_to(&mut world, branch);
        assert_eq!(world.facts().text("acid-target"), target);
        assert!(world.facts().bool("acid-adapted"));
        assert!(world.director().trace().beat_sequence().contains(&branch));
        advance_to(&mut world, "display-intrusion");
        assert_eq!(world.facts().text("display-control"), "Contested");
        assert!(mounted(&world, "display-presence"));
        assert_replay(&world);
    }
}

#[test]
fn ordered_multiple_controls_are_recorded_and_adversary_prefers_active_decoy() {
    let mut world = Encounter::new("adapt", false);
    world.update(
        ms(1),
        &[
            StoryEvent::command("trace"),
            StoryEvent::command("isolate"),
            StoryEvent::command("decoy"),
        ],
    );
    assert!(world.facts().bool("tracing"));
    assert!(world.facts().bool("route-isolated"));
    assert!(world.facts().bool("decoy-open"));
    advance_to(&mut world, "decoy");
    assert_eq!(world.facts().text("acid-target"), "decoy");
    assert!(mounted(&world, "decoy-occupied"));
    assert_replay(&world);
}

#[test]
fn cut_link_contains_every_subsystem_preserves_damage_and_replays() {
    let mut world = Encounter::new("pressure", false);
    let damaged_auth = world.facts().number("auth-integrity");
    let altered_files = world.facts().number("altered-files");
    advance_to(&mut world, "climax");
    world.command("cut link");
    assert!(world.director().is_finished());
    assert_eq!(world.facts().text("outcome"), "CRASH CONTAINS");
    assert!(world.facts().bool("link-cut"));
    assert_eq!(world.facts().number("auth-integrity"), damaged_auth);
    assert_eq!(world.facts().number("altered-files"), altered_files);
    assert!(altered_files > 0.0, "the history must retain a real scar");
    for system in SYSTEMS {
        assert_eq!(world.facts().text(&format!("{system}-control")), "Crash");
    }
    for bundle in ["auth-presence", "display-presence", "takeover", "ghost"] {
        assert!(!mounted(&world, bundle), "stale influence: {bundle}");
    }
    assert_replay(&world);
}

#[test]
fn letting_acid_in_owns_display_temporarily_then_releases_the_machine() {
    let mut world = Encounter::new("climax", false);
    world.command("let her in");
    assert_eq!(world.director().current_beat(), "acid-win");
    assert_eq!(world.facts().text("display-control"), "Acid");
    assert!(mounted(&world, "acid-victory"));
    assert!(!world.director().is_finished());
    advance_to(&mut world, "release");
    assert!(world.director().is_finished());
    assert!(!mounted(&world, "acid-victory"));
    for system in SYSTEMS {
        assert_ne!(
            world.facts().text(&format!("{system}-control")),
            "Acid",
            "voluntary release left {system} remotely owned"
        );
    }
    assert_replay(&world);
}

#[test]
fn earned_trace_produces_stalemate_without_lingering_remote_possession() {
    let mut world = Encounter::new("climax", false);
    world.command("trace token");
    world.command("turn trace");
    assert_eq!(world.director().current_beat(), "stalemate");
    assert!(world.director().is_finished());
    assert_eq!(world.facts().text("route-control"), "Contested");
    assert!(world.facts().number("trace-confidence") >= 90.0);
    assert!(!mounted(&world, "display-presence"));
    assert!(!mounted(&world, "takeover"));
    assert!(
        !world.facts().text("last-action").contains("needs"),
        "successful trace turn retained a prerequisite rejection"
    );
    for system in SYSTEMS {
        assert_ne!(world.facts().text(&format!("{system}-control")), "Acid");
    }
    assert_replay(&world);
}

#[test]
fn final_strategy_requires_the_resource_it_claims_to_use() {
    for command in ["spring decoy", "turn trace"] {
        let mut world = Encounter::new("climax", false);
        assert!(!world.facts().bool("decoy-open"));
        assert!(!world.facts().bool("tracing"));
        world.command(command);
        assert_eq!(
            world.director().current_beat(),
            "climax",
            "{command} fabricated an earned strategy without its prerequisite"
        );
        assert!(!world.director().is_finished());
        assert_replay(&world);
    }
    let mut prepared = Encounter::new("climax", false);
    prepared.command("decoy");
    prepared.command("spring decoy");
    assert_eq!(prepared.facts().text("outcome"), "CRASH CONTAINS");
    assert!(
        !prepared.facts().text("last-action").contains("needs"),
        "successful decoy retained a prerequisite rejection"
    );
    assert_replay(&prepared);
}

#[test]
fn deterministic_auto_and_idle_complete_with_distinct_repeatable_outcomes() {
    for (auto, expected) in [(true, "CRASH CONTAINS"), (false, "ACID WINS THE ROUND")] {
        let mut worlds = [
            Encounter::new("quiet", false),
            Encounter::new("quiet", false),
        ];
        for world in &mut worlds {
            for _ in 0..1200 {
                if world.director().is_finished() {
                    break;
                }
                world.tick(ms(100), auto);
            }
            assert!(
                world.director().is_finished(),
                "auto={auto} never completed"
            );
            assert_eq!(world.facts().text("outcome"), expected);
            assert_replay(world);
        }
        assert_eq!(worlds[0].director().trace(), worlds[1].director().trace());
        assert_eq!(worlds[0].facts(), worlds[1].facts());
    }
}

#[test]
fn ownership_and_bundles_agree_when_auth_is_countered() {
    for command in ["isolate auth", "kill"] {
        let mut world = Encounter::new("first-breach", false);
        assert_eq!(world.facts().text("auth-control"), "Acid");
        assert!(mounted(&world, "auth-presence"));
        world.command(command);
        assert_eq!(world.facts().text("auth-control"), "Crash");
        assert!(!mounted(&world, "auth-presence"));
        assert_eq!(world.director().current_beat(), "first-breach");
        assert_replay(&world);
    }
}

#[test]
fn route_isolation_remains_truthful_when_applied_before_the_contest() {
    let mut world = Encounter::new("signature", false);
    world.command("isolate");
    advance_to(&mut world, "route-contested");
    assert!(world.facts().bool("route-isolated"));
    assert_eq!(
        world.facts().text("route-control"),
        "Crash",
        "disconnected route cannot silently become contested"
    );
    assert_replay(&world);
}

#[test]
fn early_auth_isolation_prevents_unexplained_reacquisition() {
    let mut world = Encounter::new("route-contested", false);
    world.command("isolate auth");
    advance_to(&mut world, "first-breach");
    assert!(world.facts().bool("auth-isolated"));
    assert_eq!(world.facts().text("auth-control"), "Crash");
    assert!(!mounted(&world, "auth-presence"));
    assert_replay(&world);
}

#[test]
fn hard_isolation_cleanly_interrupts_takeover() {
    let mut world = Encounter::new("takeover", false);
    assert!(mounted(&world, "takeover"));
    world.command("hard isolate");
    for system in ["route", "auth", "display", "modem"] {
        assert_eq!(
            world.facts().text(&format!("{system}-control")),
            "Crash",
            "hard isolation left {system} controlled remotely"
        );
    }
    for bundle in ["takeover", "display-presence", "auth-presence"] {
        assert!(!mounted(&world, bundle));
    }
    assert_replay(&world);
}

#[test]
fn renewed_display_pressure_changes_ownership_when_bundle_returns() {
    let mut world = Encounter::new("trap", false);
    world.command("hard isolate");
    assert_eq!(world.facts().text("display-control"), "Crash");
    advance_to(&mut world, "climax");
    assert!(mounted(&world, "display-presence"));
    assert_eq!(
        world.facts().text("display-control"),
        "Contested",
        "new foreign display effects need an actual change of control"
    );
    assert_replay(&world);
}
