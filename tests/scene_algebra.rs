//! Integration tests for the Scene Algebra and Story Director.
//!
//! These verify the *engineering laws* the abstractions are designed to
//! preserve, at the level the demos actually use them:
//!
//! * the render functor emits ordinary `Node`s that flow through the existing
//!   layout/paint/diff pipeline (no second renderer, no raw ANSI);
//! * animating one scene entity produces a *bounded* framebuffer diff;
//! * effects are deterministic;
//! * a story graph branches and reconverges;
//! * a recorded trace replays to the identical beat sequence.

use gibson::scene::{Easing, Effect, EffectBundle, Scene, SceneEntity, SceneTarget};
use gibson::story::{Beat, Condition, Story, StoryAction, StoryEvent, StoryTrace};
use gibson::{Node, RenderMode, Renderer, Style, TerminalSession};
use std::time::Duration;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn sprite(label: &str, color: (u8, u8, u8)) -> Node {
    Node::text(
        label,
        Style::new().fg(gibson::Color::Rgb(color.0, color.1, color.2)),
    )
    .width(6.0)
    .height(1.0)
}

#[test]
fn render_functor_emits_ordinary_nodes() {
    let mut scene = Scene::new();
    let bg = scene.add(
        SceneEntity::new(
            "bg",
            Node::text("background layer", Style::default())
                .width(20.0)
                .height(1.0),
        )
        .z(0),
    );
    let hero = scene.add(
        SceneEntity::new("hero", sprite("HERO", (255, 0, 0)))
            .z(10)
            .offset(2, 1),
    );

    // Empty presentation: baseline offset is honoured, both entities present.
    let p = gibson::scene::Presentation::new();
    let node = scene.to_node(&p, 40.0, 8.0);
    assert!(matches!(node.kind, gibson::NodeKind::Stack));
    assert_eq!(node.children.len(), 2);
    assert_eq!(p.offset_of(&scene, hero), (2, 1));

    // Effects change only presentation; the nodes themselves are untouched.
    let e = Effect::translate(SceneTarget::Id(hero), (2.0, 1.0), (10.0, 1.0), ms(100));
    let mut p2 = gibson::scene::Presentation::new();
    e.eval(ms(100), &scene, &mut p2);
    assert_eq!(p2.offset_of(&scene, hero), (10, 1));
    assert_eq!(p2.offset_of(&scene, bg), (0, 0));

    // A camera pan wraps the stack in an ordinary viewport node.
    let cam = Effect::camera_pan((0.0, 0.0), (3.0, 0.0), ms(100));
    let mut p3 = gibson::scene::Presentation::new();
    cam.eval(ms(100), &scene, &mut p3);
    let node3 = scene.to_node(&p3, 40.0, 8.0);
    assert!(matches!(node3.kind, gibson::NodeKind::Viewport { .. }));
}

#[test]
fn translating_one_entity_yields_bounded_damage() {
    let cols = 60u16;
    let rows = 12u16;

    let mut scene = Scene::new();
    // A large static backdrop plus one moving sprite.
    let backdrop: String = std::iter::repeat_n('=', cols as usize).collect();
    scene.add(
        SceneEntity::new(
            "backdrop",
            Node::text(backdrop, Style::default())
                .width(cols as f32)
                .height(rows as f32),
        )
        .z(0),
    );
    let mover = scene.add(
        SceneEntity::new("mover", sprite("██████", (0, 255, 0)))
            .z(10)
            .offset(1, rows as i32 - 2),
    );

    let effect = Effect::translate(
        SceneTarget::Id(mover),
        (1.0, (rows as i32 - 2) as f32),
        (cols as f32 - 8.0, (rows as i32 - 2) as f32),
        ms(1000),
    );

    let build = |t: Duration| {
        let mut p = gibson::scene::Presentation::new();
        effect.eval(t, &scene, &mut p);
        scene.to_node(&p, cols as f32, rows as f32)
    };

    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(cols, rows);
    let mut out = Vec::new();

    renderer
        .render(&mut build(ms(0)), &mut session, &mut out)
        .unwrap();
    out.clear();
    let (dirty, total, _bytes, _, _) = renderer
        .render(&mut build(ms(160)), &mut session, &mut out)
        .unwrap();

    assert!(total > 0);
    // One 6-cell sprite moved ~9 cells: the damage must be a small fraction of
    // the screen, proving scene evaluation does not force whole-screen repaints.
    assert!(
        dirty * 4 <= total,
        "moving one sprite dirtied {dirty}/{total} cells — scene evaluation is not local"
    );
    assert!(dirty > 0, "the sprite did move, so damage must not be zero");
}

#[test]
fn scene_evaluation_is_deterministic() {
    let mut scene = Scene::new();
    let a = scene.add(SceneEntity::new("a", sprite("A", (1, 2, 3))));
    let e = Effect::sequence([
        Effect::translate(SceneTarget::Id(a), (0.0, 0.0), (5.0, 2.0), ms(100))
            .eased(Easing::EaseInOut),
        Effect::reveal(SceneTarget::Id(a), 1.0, 0.0, ms(100)),
    ]);
    let mut p1 = gibson::scene::Presentation::new();
    let mut p2 = gibson::scene::Presentation::new();
    // Independent evaluations at the same time are identical, even interleaved.
    for ms_t in (0..=200).step_by(25) {
        e.eval(ms(ms_t), &scene, &mut p1);
        let snap1 = format!("{:?}", p1);
        let _ = p2; // silence unused on the first pass
        e.eval(ms(ms_t), &scene, &mut p2);
        assert_eq!(snap1, format!("{:?}", p2));
    }
}

/// A branch/join story: two user choices differ locally, then reconverge.
fn tactical_story() -> Story {
    Story::new("grand-central")
        .bundle(EffectBundle::new("plague-presence").effect(Effect::identity()))
        .beat(
            Beat::new("grand-central")
                .min_duration(ms(60))
                .on_enter(StoryAction::set_bool("plague-active", true))
                .on_enter(StoryAction::mount("plague-presence"))
                .transition(Condition::user("pool"), "pool-distraction")
                .transition(Condition::user("crew"), "crew-reinforcement")
                .after(ms(600), "download"),
        )
        .beat(
            Beat::new("pool-distraction")
                .on_enter(StoryAction::set_bool("pool-distraction", true))
                .after(ms(120), "download"),
        )
        .beat(
            Beat::new("crew-reinforcement")
                .on_enter(StoryAction::set_bool("crew-reinforced", true))
                .after(ms(120), "download"),
        )
        .beat(Beat::new("download").terminal())
}

#[test]
fn tactical_choices_branch_and_reconverge() {
    let story = tactical_story();

    let mut pool = story.start();
    pool.update(ms(80), &[StoryEvent::user_selected("pool")]);
    let mut crew = story.start();
    crew.update(ms(80), &[StoryEvent::user_selected("crew")]);

    assert!(pool.facts().bool("pool-distraction"));
    assert!(!pool.facts().bool("crew-reinforced"));
    assert!(crew.facts().bool("crew-reinforced"));
    assert!(!crew.facts().bool("pool-distraction"));
    // Both share the persistent Plague fact from the join beat.
    assert!(pool.facts().bool("plague-active"));
    assert!(crew.facts().bool("plague-active"));

    for _ in 0..30 {
        pool.update(ms(10), &[]);
        crew.update(ms(10), &[]);
    }
    assert_eq!(pool.current_beat(), "download");
    assert_eq!(crew.current_beat(), "download");
    // The mounted bundle persisted across the branch until the terminal beat.
    assert!(pool.mounted().any(|m| m == "plague-presence"));
}

#[test]
fn default_timeout_selects_a_route_without_user_input() {
    let story = tactical_story();
    let mut d = story.start();
    for _ in 0..80 {
        d.update(ms(10), &[]);
    }
    assert_eq!(d.current_beat(), "download");
    assert!(!d.facts().bool("pool-distraction"));
    assert!(!d.facts().bool("crew-reinforced"));
}

#[test]
fn story_trace_replays_identically() {
    let story = tactical_story();
    let mut d = story.start();
    d.update(ms(80), &[StoryEvent::user_selected("crew")]);
    for _ in 0..30 {
        d.update(ms(10), &[]);
    }
    let original = d.trace().beat_sequence().join(">");
    assert_eq!(original, "grand-central>crew-reinforcement>download");

    let replayed = story.replay(d.trace(), ms(10));
    assert_eq!(replayed.trace().beat_sequence().join(">"), original);
    assert_eq!(replayed.current_beat(), d.current_beat());
    assert!(replayed.facts().bool("crew-reinforced"));
}

#[test]
fn effect_identity_and_associativity_through_the_functor() {
    let mut scene = Scene::new();
    let a = scene.add(SceneEntity::new("a", sprite("A", (9, 9, 9))));
    let t = SceneTarget::Id(a);

    // identity does not change the rendered node.
    let base = scene.to_node(&gibson::scene::Presentation::new(), 20.0, 4.0);
    let mut p = gibson::scene::Presentation::new();
    Effect::identity().eval(ms(50), &scene, &mut p);
    let after_identity = scene.to_node(&p, 20.0, 4.0);
    assert_eq!(format!("{base:?}"), format!("{after_identity:?}"));

    // ((e1;e2);e3) == (e1;(e2;e3)) through the functor at checkpoints.
    let e1 = Effect::translate(t, (0.0, 0.0), (4.0, 0.0), ms(100));
    let e2 = Effect::translate(t, (4.0, 0.0), (4.0, 2.0), ms(100));
    let e3 = Effect::translate(t, (4.0, 2.0), (1.0, 1.0), ms(100));
    let left = Effect::sequence([Effect::sequence([e1.clone(), e2.clone()]), e3.clone()]);
    let right = Effect::sequence([e1, Effect::sequence([e2, e3])]);
    for ms_t in (0..=300).step_by(30) {
        let mut pl = gibson::scene::Presentation::new();
        let mut pr = gibson::scene::Presentation::new();
        left.eval(ms(ms_t), &scene, &mut pl);
        right.eval(ms(ms_t), &scene, &mut pr);
        let nl = scene.to_node(&pl, 20.0, 4.0);
        let nr = scene.to_node(&pr, 20.0, 4.0);
        assert_eq!(format!("{nl:?}"), format!("{nr:?}"), "at {ms_t}ms");
    }
}

#[test]
fn trace_label_round_trips_events() {
    let mut trace = StoryTrace::default();
    trace.record(ms(10), StoryEvent::user_selected("pool"));
    trace.record(ms(20), StoryEvent::command("city"));
    assert_eq!(trace.events[0].1.label(), "user:pool");
    assert_eq!(trace.events[1].1.label(), "cmd:city");
}
