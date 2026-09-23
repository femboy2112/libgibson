//! Reactions mutate semantic state in place; only transitions change beats.
use gibson::scene::{Effect, EffectBundle, Scene, SceneEntity, SceneTarget};
use gibson::story::{Beat, Condition, Story, StoryAction, StoryError, StoryEvent};
use gibson::{Node, Style};
use std::time::Duration;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn reaction_changes_fact_without_resetting_beat_or_running_entry_again() {
    let story = Story::new("inside").beat(
        Beat::new("inside")
            .on_enter(StoryAction::set_text("owner", "local"))
            .reaction(
                Condition::command("isolate"),
                [StoryAction::set_text("owner", "isolated")],
            ),
    );
    let mut director = story.start();
    director.update(ms(37), &[StoryEvent::command("isolate")]);
    assert_eq!(director.current_beat(), "inside");
    assert_eq!(director.time_in_beat(), ms(37));
    assert_eq!(director.facts().text("owner"), "isolated");
    assert_eq!(director.trace().beat_sequence(), ["inside"]);
}

#[test]
fn reaction_mounts_and_removes_bundle_without_leaving_beat() {
    let story = Story::new("inside")
        .bundle(EffectBundle::new("isolation"))
        .beat(
            Beat::new("inside")
                .reaction(
                    Condition::command("isolate"),
                    [StoryAction::mount("isolation")],
                )
                .reaction(
                    Condition::command("restore"),
                    [StoryAction::unmount("isolation")],
                ),
        );
    assert_eq!(story.validate(), Ok(()));
    let mut director = story.start();
    director.update(ms(1), &[StoryEvent::command("isolate")]);
    assert_eq!(director.mounted().collect::<Vec<_>>(), ["isolation"]);
    assert_eq!(director.current_beat(), "inside");
    director.update(ms(1), &[StoryEvent::command("restore")]);
    assert_eq!(director.mounted().count(), 0);
    assert_eq!(director.current_beat(), "inside");
}

#[test]
fn reaction_fact_persists_through_later_transition() {
    let story = Story::new("inside")
        .beat(
            Beat::new("inside")
                .reaction(
                    Condition::command("trace"),
                    [StoryAction::set_number("confidence", 42.0)],
                )
                .transition(Condition::command("finish"), "done"),
        )
        .beat(Beat::new("done").terminal());
    let mut director = story.start();
    director.update(ms(7), &[StoryEvent::command("trace")]);
    assert_eq!(director.current_beat(), "inside");
    director.update(ms(11), &[StoryEvent::command("finish")]);
    assert_eq!(director.current_beat(), "done");
    assert_eq!(director.facts().number("confidence"), 42.0);
}

#[test]
fn reaction_can_enable_one_automatic_arrow_but_cannot_chain() {
    let story = Story::new("a")
        .beat(
            Beat::new("a")
                .reaction(
                    Condition::command("ready"),
                    [StoryAction::set_bool("ready", true)],
                )
                .transition(Condition::fact_true("ready"), "b"),
        )
        .beat(Beat::new("b").after(Duration::ZERO, "c"))
        .beat(Beat::new("c").terminal());
    let mut director = story.start();
    director.update(ms(1), &[StoryEvent::command("ready")]);
    assert_eq!(director.current_beat(), "b");
    director.update(Duration::ZERO, &[]);
    assert_eq!(director.current_beat(), "c");
}

#[test]
fn replay_preserves_reactions_facts_beats_and_bundle_clock_exactly() {
    let mut scene = Scene::new();
    let panel = scene.add(SceneEntity::new("panel", Node::text("panel", Style::new())));
    let story = Story::new("a")
        .bundle(EffectBundle::new("scan").effect(Effect::custom_ramp(
            SceneTarget::Id(panel),
            9,
            0.0,
            1.0,
            ms(100),
        )))
        .beat(
            Beat::new("a")
                .reaction(
                    Condition::command("scan"),
                    [
                        StoryAction::mount("scan"),
                        StoryAction::set_bool("seen", true),
                    ],
                )
                .transition(Condition::command("next"), "b"),
        )
        .beat(Beat::new("b").reaction(
            Condition::command("label"),
            [StoryAction::set_text("label", "recorded")],
        ));
    let mut director = story.start();
    director.update(ms(13), &[StoryEvent::command("scan")]);
    director.update(ms(7), &[]);
    director.update(ms(3), &[StoryEvent::command("next")]);
    director.update(ms(41), &[StoryEvent::command("label")]);
    let replay = story.replay(director.trace());
    assert_eq!(replay.trace(), director.trace());
    assert_eq!(replay.facts(), director.facts());
    assert_eq!(replay.current_beat(), director.current_beat());
    assert_eq!(replay.time_in_beat(), director.time_in_beat());
    assert_eq!(replay.elapsed(), director.elapsed());
    assert_eq!(replay.mounted().collect::<Vec<_>>(), ["scan"]);
    assert_eq!(
        replay.presentation(&scene).custom(panel, 9),
        director.presentation(&scene).custom(panel, 9)
    );
    assert_eq!(replay.presentation(&scene).custom(panel, 9), Some(0.51));
}

#[test]
fn event_then_declaration_then_action_order_is_deterministic() {
    let story = Story::new("a").beat(
        Beat::new("a")
            .reaction(
                Condition::command("x"),
                [StoryAction::set_number("order", 1.0)],
            )
            .reaction(
                Condition::command("x"),
                [
                    StoryAction::set_number("order", 2.0),
                    StoryAction::set_number("order", 3.0),
                ],
            )
            .reaction(
                Condition::command("y"),
                [StoryAction::set_number("order", 4.0)],
            ),
    );
    for (events, expected) in [(["x", "y"], 4.0), (["y", "x"], 3.0)] {
        let mut director = story.start();
        director.update(ms(1), &events.map(StoryEvent::command));
        assert_eq!(director.facts().number("order"), expected);
    }
}

#[test]
fn all_reactions_use_original_beat_even_after_winning_transition_event() {
    let story = Story::new("a")
        .beat(
            Beat::new("a")
                .transition(Condition::command("go"), "b")
                .reaction(
                    Condition::command("late"),
                    [StoryAction::set_bool("old-beat", true)],
                ),
        )
        .beat(
            Beat::new("b")
                .reaction(
                    Condition::command("late"),
                    [StoryAction::set_bool("new-beat", true)],
                )
                .after(Duration::ZERO, "c"),
        )
        .beat(Beat::new("c").terminal());
    let mut director = story.start();
    director.update(
        ms(1),
        &[StoryEvent::command("go"), StoryEvent::command("late")],
    );
    assert_eq!(director.current_beat(), "b");
    assert!(director.facts().bool("old-beat"));
    assert!(!director.facts().bool("new-beat"));
    assert_eq!(director.trace().steps[0].events.len(), 2);
}

#[test]
fn automatic_guard_observes_final_facts_after_all_reactions() {
    let story = Story::new("a")
        .beat(
            Beat::new("a")
                .reaction(
                    Condition::command("arm"),
                    [StoryAction::set_bool("armed", true)],
                )
                .reaction(
                    Condition::command("cancel"),
                    [StoryAction::set_bool("armed", false)],
                )
                .transition(Condition::fact_true("armed"), "b"),
        )
        .beat(Beat::new("b").terminal());
    let mut director = story.start();
    director.update(
        ms(1),
        &[StoryEvent::command("arm"), StoryEvent::command("cancel")],
    );
    assert_eq!(director.current_beat(), "a");
    assert!(!director.facts().bool("armed"));
}

#[test]
fn reactions_ignore_min_duration_but_event_and_fact_transitions_respect_it() {
    let story = Story::new("a")
        .beat(
            Beat::new("a")
                .min_duration(ms(100))
                .reaction(
                    Condition::command("go"),
                    [StoryAction::set_bool("ready", true)],
                )
                .transition(Condition::command("go"), "event")
                .transition(Condition::fact_true("ready"), "fact"),
        )
        .beat(Beat::new("event").terminal())
        .beat(Beat::new("fact").terminal());
    let mut director = story.start();
    director.update(ms(1), &[StoryEvent::command("go")]);
    assert!(director.facts().bool("ready"));
    assert_eq!(director.current_beat(), "a");
    director.update(ms(99), &[]);
    assert_eq!(director.current_beat(), "fact");
}

#[test]
fn first_event_transition_wins_and_new_entry_actions_run_last() {
    let story = Story::new("a")
        .beat(
            Beat::new("a")
                .reaction(
                    Condition::command("go"),
                    [
                        StoryAction::set_text("owner", "reaction"),
                        StoryAction::set_bool("ready", true),
                    ],
                )
                .transition(Condition::command("other"), "other")
                .transition(Condition::command("go"), "b")
                .transition(Condition::fact_true("ready"), "other"),
        )
        .beat(Beat::new("b").on_enter(StoryAction::set_text("owner", "entry")))
        .beat(Beat::new("other").terminal());
    let mut director = story.start();
    director.update(
        ms(1),
        &[StoryEvent::command("go"), StoryEvent::command("other")],
    );
    assert_eq!(director.current_beat(), "b");
    assert_eq!(director.facts().text("owner"), "entry");
}

#[test]
fn validation_checks_reaction_bundle_references() {
    let story = Story::new("a").beat(
        Beat::new("a").reaction(Condition::command("mount"), [StoryAction::mount("missing")]),
    );
    assert_eq!(
        story.validate(),
        Err(StoryError::UnknownBundle {
            beat: "a".into(),
            bundle: "missing".into(),
        })
    );
}

#[test]
fn validation_rejects_non_event_reaction_conditions() {
    for condition in [Condition::fact_true("ready"), Condition::after(ms(1))] {
        let story = Story::new("a").beat(Beat::new("a").reaction(condition, []));
        assert_eq!(
            story.validate(),
            Err(StoryError::InvalidReactionCondition {
                beat: "a".into(),
                reaction: 0,
            })
        );
    }
}

#[test]
fn exact_event_kinds_do_not_alias_and_empty_updates_do_not_react() {
    let story = Story::new("a").beat(
        Beat::new("a")
            .reaction(
                Condition::on(StoryEvent::custom("x")),
                [StoryAction::set_bool("custom", true)],
            )
            .reaction(Condition::user("x"), [StoryAction::set_bool("user", true)]),
    );
    let mut director = story.start();
    director.update(ms(1000), &[]);
    assert_eq!(director.facts().iter().count(), 0);
    director.update(ms(1), &[StoryEvent::command("x")]);
    assert_eq!(director.facts().iter().count(), 0);
    director.update(ms(1), &[StoryEvent::custom("x")]);
    assert!(director.facts().bool("custom"));
    assert!(!director.facts().bool("user"));
    director.update(ms(1), &[StoryEvent::user_selected("x")]);
    assert!(director.facts().bool("user"));
}

#[test]
fn remount_restarts_bundle_time_and_action_order_controls_final_mount() {
    let mut scene = Scene::new();
    let panel = scene.add(SceneEntity::new("panel", Node::text("panel", Style::new())));
    let story = Story::new("a")
        .bundle(EffectBundle::new("scan").effect(Effect::custom_ramp(
            SceneTarget::Id(panel),
            9,
            0.0,
            1.0,
            ms(100),
        )))
        .beat(
            Beat::new("a")
                .reaction(Condition::command("scan"), [StoryAction::mount("scan")])
                .reaction(
                    Condition::command("stop"),
                    [StoryAction::mount("scan"), StoryAction::unmount("scan")],
                ),
        );
    let mut director = story.start();
    director.update(ms(1), &[StoryEvent::command("scan")]);
    director.update(ms(50), &[]);
    assert_eq!(director.presentation(&scene).custom(panel, 9), Some(0.5));
    director.update(ms(1), &[StoryEvent::command("scan")]);
    assert_eq!(director.presentation(&scene).custom(panel, 9), Some(0.0));
    director.update(ms(1), &[StoryEvent::command("stop")]);
    assert_eq!(director.presentation(&scene).custom(panel, 9), None);
}

#[test]
fn terminal_director_does_not_accept_reactions_or_record_more_steps() {
    let story = Story::new("done").beat(
        Beat::new("done")
            .reaction(
                Condition::command("x"),
                [StoryAction::set_bool("changed", true)],
            )
            .terminal(),
    );
    let mut director = story.start();
    let trace = director.trace().clone();
    director.update(ms(1), &[StoryEvent::command("x")]);
    assert!(!director.facts().bool("changed"));
    assert_eq!(director.trace(), &trace);
}
