//! A fictional local machine contested by two deterministic operators.
//! BattleGraph owns tactical truth; StoryDirector owns the dramatic acts.
//! No sockets, host commands, credentials or real intrusion exist here.
//!
//! Crash fights by default; `--manual` gives the operator control.
//! `--auto` exits after the ending; `--deterministic`, `--debug-battle`, `--color=mono`.

#[path = "acid_vs_crash/battle.rs"]
pub mod battle;

use battle::{Control, EncounterModel, NodeId};
use gibson::cell::{Color, Line, RichText, Span, Style};
use gibson::input::{Event, KeyCode, KeyModifiers, TextInputState};
use gibson::node::{Node, WrapMode};
use gibson::scene::{Effect, EffectBundle, Presentation, Scene, SceneEntity, SceneTarget};
use gibson::story::{Beat, Condition, Facts, Story, StoryAction, StoryDirector, StoryEvent};
use gibson::surface::{BorderType, Surface};
use gibson::surface_fx::{FxMask, SurfaceFx};
use gibson::{BrailleCanvas, ColorDepth, Context, FixedStepClock};
use std::time::Duration;

const SEED: u64 = 0xAC1D_C2A5;
pub const STAGES: &[&str] = &[
    "quiet",
    "knock",
    "signature",
    "route-contested",
    "first-breach",
    "adapt",
    "sidepath",
    "decoy",
    "trace",
    "pressure",
    "display-intrusion",
    "ghost",
    "trap",
    "climax",
    "takeover",
    "crash-win",
    "acid-win",
    "stalemate",
];
fn seconds(n: f32) -> Duration {
    Duration::from_secs_f32(n)
}
fn act(stage: &str) -> &str {
    match stage {
        "adapt" | "sidepath" | "decoy" | "trace" | "pressure" | "trap" => "counterplay",
        "display-intrusion" | "ghost" => "escalation",
        "climax" => "takeover",
        other => other,
    }
}

#[derive(Clone, Copy)]
struct Palette {
    crash: Style,
    acid: Style,
    infection: Style,
    muted: Style,
    warning: Style,
    white: Style,
}
impl Palette {
    fn new(mono: bool) -> Self {
        let rgb = |r, g, b| {
            if mono {
                Color::Reset
            } else {
                Color::Rgb(r, g, b)
            }
        };
        Self {
            crash: Style::new().fg(rgb(76, 218, 255)).bold(),
            acid: Style::new().fg(rgb(255, 79, 192)).bold(),
            infection: if mono {
                Style::new().bold().reverse()
            } else {
                Style::new().fg(rgb(255, 79, 192)).bold()
            },
            muted: Style::new().fg(rgb(105, 124, 158)),
            warning: Style::new().fg(rgb(255, 200, 91)).bold(),
            white: Style::new().fg(rgb(223, 235, 255)),
        }
    }
    fn owner(self, owner: Control) -> Style {
        match owner {
            Control::Crash => self.crash,
            Control::Contested => self.warning,
            Control::Acid => self.acid,
        }
    }
}
fn scene_template() -> Scene {
    let mut scene = Scene::new();
    for tag in [
        "display-sensitive",
        "auth",
        "map",
        "remote",
        "decoy",
        "cursor",
        "header",
    ] {
        scene.tag(tag);
    }
    for (id, tag) in [
        ("header", "header"),
        ("system-map", "map"),
        ("session", "auth"),
        ("event-trace", "trace"),
        ("remote-session", "remote"),
    ] {
        scene.add(
            SceneEntity::new(id, Node::col())
                .tag("ui")
                .tag(tag)
                .tag("display-sensitive"),
        );
    }
    scene.add(
        SceneEntity::new("decoy", Node::col())
            .tag("decoy")
            .z(3)
            .hidden(),
    );
    scene.add(
        SceneEntity::new("ghost-cursor", Node::col())
            .tag("cursor")
            .z(4)
            .hidden(),
    );
    scene.add(
        SceneEntity::new("actions", Node::col())
            .tag("crash-island")
            .z(10),
    );
    scene.add(
        SceneEntity::new("prompt", Node::col())
            .tag("crash-island")
            .z(11),
    );
    scene
}

// Only categorical milestones cross the simulation/story boundary. Influence,
// integrity, cooldowns and clocks remain in BattleGraph, never duplicated in Facts.
fn semantic_state(world: &EncounterModel) -> Vec<(String, String)> {
    let mut values = Vec::new();
    for node in &world.graph.nodes {
        values.push((
            format!("{}-control", node.id.name()),
            node.owner().as_str().into(),
        ));
        values.push((
            format!("{}-isolated", node.id.name()),
            node.isolated.to_string(),
        ));
    }
    let auth = &world.graph.nodes[NodeId::Auth.index()];
    let display = &world.graph.nodes[NodeId::Display.index()];
    let decoy = &world.graph.nodes[NodeId::Decoy.index()];
    let display_connected =
        world.remote_active && world.graph.route(NodeId::Modem, NodeId::Display).is_some();
    values.extend([
        (
            "identity".into(),
            if world.elapsed_ms >= 6500 {
                "ACID BURN"
            } else {
                "UNRESOLVED"
            }
            .into(),
        ),
        (
            "auth-pressure".into(),
            (auth.influence < 900 && !auth.isolated && world.remote_active).to_string(),
        ),
        (
            "display-pressure".into(),
            (display.influence < 900 && display_connected).to_string(),
        ),
        (
            "display-contested".into(),
            (display.influence < 350 && display_connected).to_string(),
        ),
        (
            "takeover-ready".into(),
            (display.influence < -400 && display_connected).to_string(),
        ),
        (
            "trace-high".into(),
            (world.trace_confidence >= 800).to_string(),
        ),
        ("tracing".into(), (world.trace_confidence > 40).to_string()),
        (
            "remote-present".into(),
            (world.remote_active && world.elapsed_ms >= 6500).to_string(),
        ),
        (
            "release-complete".into(),
            (!world.remote_active
                && world.graph.nodes[..6]
                    .iter()
                    .all(|n| n.owner() == Control::Crash))
            .to_string(),
        ),
        ("decoy-open".into(), decoy.visible.to_string()),
        (
            "acid-victory".into(),
            (world.remote_active && world.outcome == Some(battle::Outcome::Acid)).to_string(),
        ),
        (
            "decoy-taken".into(),
            (decoy.visible && decoy.influence < 0).to_string(),
        ),
        (
            "crash-blind".into(),
            (world.graph.nodes[..6].iter().filter(|n| !n.visible).count() >= 3).to_string(),
        ),
        (
            "foothold".into(),
            world.graph.nodes[1..6]
                .iter()
                .any(|n| n.influence < 0)
                .to_string(),
        ),
        (
            "two-footholds".into(),
            (world.graph.nodes[1..6]
                .iter()
                .filter(|n| n.influence < 0)
                .count()
                >= 2)
                .to_string(),
        ),
    ]);
    values
}
fn semantic_event(key: &str, value: &str) -> StoryEvent {
    StoryEvent::custom(format!("world:{key}={value}"))
}
fn semantic_actions(key: &str, value: &str) -> Vec<StoryAction> {
    let mut actions = vec![if value == "true" || value == "false" {
        StoryAction::set_bool(key, value == "true")
    } else {
        StoryAction::set_text(key, value)
    }];
    if let Some(bundle) = match key {
        "auth-pressure" => Some("auth-presence"),
        "acid-victory" => Some("acid-victory"),
        "remote-present" => Some("ghost"),
        "display-pressure" => Some("display-presence"),
        "decoy-open" => Some("decoy-reveal"),
        "decoy-taken" => Some("decoy-occupied"),
        "route-isolated" => Some("route-isolation"),
        "auth-isolated" => Some("auth-isolation"),
        "tracing" => Some("trace-token"),
        "takeover-ready" => Some("takeover"),
        _ => None,
    } {
        actions.push(if value == "true" {
            StoryAction::mount(bundle)
        } else {
            StoryAction::unmount(bundle)
        });
    }
    if key == "remote-present" && value == "false" {
        actions.push(StoryAction::unmount("acid-victory"));
    }
    actions
}
fn battle_story(scene: &mut Scene, stage: &str, world: &EncounterModel, p: Palette) -> Story {
    let target = |scene: &mut Scene, tag| SceneTarget::Tag(scene.tag(tag));
    let auth = target(scene, "auth");
    let map = target(scene, "map");
    let decoy = target(scene, "decoy");
    let cursor = target(scene, "cursor");
    let sensitive = target(scene, "display-sensitive");
    // Presence bundles own lifecycle; continuous scope is supplied by the pure
    // presentation projection below. Widgets never inspect ownership for FX.
    let mut story = Story::new(act(stage))
        .bundle(EffectBundle::new("auth-presence").effect(Effect::set_custom(auth, 1, 1.0)))
        .bundle(EffectBundle::new("display-presence").effect(Effect::set_custom(sensitive, 2, 1.0)))
        .bundle(EffectBundle::new("takeover").effect(Effect::set_custom(sensitive, 3, 1.0)))
        .bundle(
            EffectBundle::new("route-isolation").effect(Effect::displace(
                map,
                (-1.0, 0.0),
                (0.0, 0.0),
                seconds(0.4),
            )),
        )
        .bundle(
            EffectBundle::new("auth-isolation").effect(Effect::post_process(auth, SurfaceFx::Dim)),
        )
        .bundle(EffectBundle::new("trace-token").effect(Effect::set_custom(map, 4, 1.0)))
        .bundle(EffectBundle::new("decoy-reveal").effects([
            Effect::reveal(decoy, 1.0, 1.0, Duration::ZERO),
            Effect::dissolve(decoy, 0.0, 1.0, SEED + 8, seconds(0.8)),
        ]))
        .bundle(EffectBundle::new("decoy-occupied").effect(Effect::set_custom(decoy, 1, 1.0)))
        .bundle(EffectBundle::new("ghost").effect(Effect::reveal(cursor, 1.0, 1.0, Duration::ZERO)))
        .bundle(EffectBundle::new("acid-victory").effects([
            Effect::post_process(sensitive, SurfaceFx::StyleOverlay(p.infection)),
            Effect::displace(map, (0.0, 0.0), (2.0, 0.0), seconds(1.0)),
        ]));
    let specs = [
        ("quiet", "LOCAL NODE / ordinary night", Some(("knock", 4.0))),
        (
            "knock",
            "INBOUND / one packet disagrees",
            Some(("signature", 4.5)),
        ),
        (
            "signature",
            "CONTACT / hello, Crash",
            Some(("route-contested", 4.0)),
        ),
        (
            "route-contested",
            "CONTEST / choose what to protect",
            Some(("first-breach", 12.0)),
        ),
        (
            "first-breach",
            "INTRUSION / the machine has geography",
            Some(("counterplay", 10.0)),
        ),
        (
            "counterplay",
            "COUNTERPLAY / every door has a price",
            Some(("escalation", 13.0)),
        ),
        (
            "escalation",
            "ESCALATION / she noticed",
            Some(("takeover", 12.0)),
        ),
        (
            "takeover",
            "FINAL MOVE / keep one hand on the controls",
            None,
        ),
        ("crash-win", "CRASH CONTAINS / keep the scars", None),
        ("acid-win", "ACID WINS THE ROUND / borrowed display", None),
        ("stalemate", "CARRIER HOLD / mutual respect", None),
        ("release", "AFTERMATH / screen returned", None),
    ];
    let initial = semantic_state(world);
    for (id, label, next) in specs {
        let mut beat = Beat::new(id).label(label);
        if id == act(stage) {
            for (key, value) in &initial {
                for action in semantic_actions(key, value) {
                    beat = beat.on_enter(action);
                }
            }
        }
        if let Some((next, after)) = next {
            beat = beat.after(seconds(after), next);
        }
        for (key, value) in &initial {
            let options: Vec<&str> = if key.ends_with("-control") {
                vec!["Crash", "Contested", "Acid"]
            } else if key == "identity" {
                vec!["UNRESOLVED", "ACID BURN"]
            } else {
                vec!["false", "true"]
            };
            let _ = value;
            for v in options {
                beat = beat.reaction(
                    Condition::on(semantic_event(key, v)),
                    semantic_actions(key, v),
                );
            }
        }
        for milestone in [
            "RouteProbe",
            "IdentityResolved",
            "Foothold",
            "DisplayIntrusion",
            "DecoyTriggered",
            "TraceAttempt",
            "Adapted",
            "TakeoverReady",
        ] {
            beat = beat.reaction(
                Condition::on(StoryEvent::custom(milestone)),
                [StoryAction::set_bool(milestone, true)],
            );
        }
        match id {
            "route-contested" => {
                beat = beat.transition(
                    Condition::on(StoryEvent::custom("paced:foothold")),
                    "first-breach",
                )
            }
            "first-breach" => {
                beat = beat.transition(
                    Condition::on(StoryEvent::custom("paced:adapted")),
                    "counterplay",
                )
            }
            "counterplay" => {
                beat = beat.transition(
                    Condition::on(StoryEvent::custom("paced:display")),
                    "escalation",
                )
            }
            "escalation" => {
                beat = beat.transition(
                    Condition::on(StoryEvent::custom("paced:takeover")),
                    "takeover",
                )
            }
            "acid-win" => {
                beat = beat.transition(Condition::fact_true("release-complete"), "release")
            }
            _ => (),
        }
        if !["crash-win", "acid-win", "stalemate", "release"].contains(&id) {
            for (event, next) in [
                ("CrashResolved", "crash-win"),
                ("AcidResolved", "acid-win"),
                ("MutualResolved", "stalemate"),
            ] {
                beat = beat.transition(Condition::on(StoryEvent::custom(event)), next);
            }
        }
        if ["crash-win", "acid-win", "stalemate", "release"].contains(&id) {
            for name in [
                "auth-presence",
                "display-presence",
                "takeover",
                "ghost",
                "decoy-occupied",
                "trace-token",
                "route-isolation",
            ] {
                beat = beat.on_enter(StoryAction::unmount(name));
            }
            beat = beat.on_enter(StoryAction::set_text(
                "outcome",
                match id {
                    "crash-win" => "CRASH CONTAINS",
                    "stalemate" => "STALEMATE / MUTUAL RESPECT",
                    _ => "ACID WINS THE ROUND",
                },
            ));
            if id != "acid-win" {
                beat = beat
                    .on_enter(StoryAction::unmount("acid-victory"))
                    .terminal();
            }
        }
        story = story.beat(beat);
    }
    story.validate().expect("valid macro story");
    story
}

/// Exact external inputs, including ordered commands and irregular dt. Derived
/// planner/milestone events are recomputed, so replay verifies the reducer too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterTrace {
    pub stage: String,
    pub seed: u64,
    pub steps: Vec<EncounterStep>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterStep {
    pub dt: Duration,
    pub events: Vec<StoryEvent>,
}

pub struct Encounter {
    story: Story,
    director: StoryDirector,
    scene: Scene,
    palette: Palette,
    world: EncounterModel,
    trace: EncounterTrace,
    pub input: TextInputState,
    inspector: String,
    mono: bool,
    debug_battle: bool,
    watching: bool,
}
impl Encounter {
    pub fn new(stage: &str, mono: bool) -> Self {
        let stage = if STAGES.contains(&stage) {
            stage
        } else {
            "quiet"
        };
        let world = EncounterModel::new(stage, SEED);
        let mut scene = scene_template();
        let palette = Palette::new(mono);
        let story = battle_story(&mut scene, stage, &world, palette);
        let director = story.start();
        Self {
            story,
            director,
            scene,
            palette,
            world,
            trace: EncounterTrace {
                stage: stage.into(),
                seed: SEED,
                steps: Vec::new(),
            },
            input: TextInputState::new(),
            inspector: String::new(),
            mono,
            debug_battle: false,
            watching: false,
        }
    }
    pub fn director(&self) -> &StoryDirector {
        &self.director
    }
    pub fn story(&self) -> &Story {
        &self.story
    }
    pub fn facts(&self) -> &Facts {
        self.director.facts()
    }
    pub fn battle(&self) -> &EncounterModel {
        &self.world
    }
    pub fn trace(&self) -> &EncounterTrace {
        &self.trace
    }
    pub fn encounter_trace(&self) -> &EncounterTrace {
        &self.trace
    }
    pub fn set_debug_battle(&mut self, enabled: bool) {
        self.debug_battle = enabled;
    }
    pub fn set_watching(&mut self, enabled: bool) {
        self.watching = enabled;
    }
    pub fn update(&mut self, dt: Duration, events: &[StoryEvent]) {
        self.trace.steps.push(EncounterStep {
            dt,
            events: events.to_vec(),
        });
        if self.director.is_finished() {
            return;
        }
        let before = semantic_state(&self.world);
        let commands = events
            .iter()
            .filter_map(|e| match e {
                StoryEvent::Command(c) => Some(c.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let milestones = self.world.update(dt, &commands);
        let mut derived = events
            .iter()
            .filter(|e| matches!(e, StoryEvent::Command(_)))
            .cloned()
            .collect::<Vec<_>>();
        for (key, value) in semantic_state(&self.world) {
            if !before.iter().any(|(k, v)| *k == key && *v == value) {
                derived.push(semantic_event(&key, &value));
            }
        }
        derived.extend(
            milestones
                .into_iter()
                .map(|m| StoryEvent::custom(m.as_str())),
        );
        if let Some(outcome) = &self.world.outcome {
            derived.push(StoryEvent::custom(match outcome.as_str() {
                "CRASH CONTAINS" => "CrashResolved",
                "ACID WINS THE ROUND" => "AcidResolved",
                _ => "MutualResolved",
            }));
        }
        if self.director.time_in_beat().saturating_add(dt) >= seconds(3.0) {
            let state = semantic_state(&self.world);
            for (key, event) in [
                ("foothold", "paced:foothold"),
                ("display-contested", "paced:display"),
                ("takeover-ready", "paced:takeover"),
            ] {
                if state.iter().any(|(k, v)| k == key && v == "true") {
                    derived.push(StoryEvent::custom(event));
                }
            }
            if self.world.quality.adaptations >= 2 {
                derived.push(StoryEvent::custom("paced:adapted"));
            }
        }
        self.director.update(dt, &derived);
    }
    pub fn tick(&mut self, dt: Duration, auto: bool) {
        let events = if auto {
            self.world
                .defender_command()
                .map(StoryEvent::command)
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        self.update(dt, &events);
    }
    pub fn replay(&self) -> Self {
        let mut replay = Self::replay_trace(&self.trace, self.mono);
        replay.watching = self.watching;
        replay.debug_battle = self.debug_battle;
        replay
    }
    pub fn replay_trace(trace: &EncounterTrace, mono: bool) -> Self {
        // The seed is versioned with this demo trace, not a general disk format.
        assert_eq!(trace.seed, SEED, "unsupported encounter seed");
        let mut fresh = Self::new(&trace.stage, mono);
        for step in &trace.steps {
            fresh.update(step.dt, &step.events);
        }
        fresh
    }
    pub fn replay_matches(&self) -> bool {
        let replay = self.replay();
        let story = self.story.replay(self.director.trace());
        replay.world == self.world
            && replay.trace == self.trace
            && replay.director.trace() == self.director.trace()
            && replay.facts() == self.facts()
            && story.facts() == self.facts()
            && replay.director.current_beat() == self.director.current_beat()
            && replay.director.mounted().collect::<Vec<_>>()
                == self.director.mounted().collect::<Vec<_>>()
            && replay.presentation() == self.presentation()
    }
    pub fn command(&mut self, command: &str) -> bool {
        let command = command.trim().to_ascii_lowercase();
        match command.as_str() {
            "exit" | "quit" => return false,
            "reset" => {
                let watching = self.watching;
                let debug = self.debug_battle;
                *self = Self::new("quiet", self.mono);
                self.watching = watching;
                self.debug_battle = debug;
            }
            "replay" => {
                self.inspector = if self.replay_matches() {
                    "REPLAY VERIFIED / world, planner, facts, beats, bundles, presentation"
                } else {
                    "REPLAY MISMATCH"
                }
                .into()
            }
            "facts" | "damage" | "scene" | "acid" | "crash" => self.inspector = command,
            "trace" if self.director.is_finished() => self.inspector = command,
            _ => self.update(Duration::ZERO, &[StoryEvent::command(command)]),
        }
        true
    }
    pub fn handle(&mut self, event: &Event) -> bool {
        if let Event::Key(key) = event {
            if (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
                || key.code == KeyCode::Esc
            {
                return false;
            }
            if key.code == KeyCode::Enter {
                let command = self.input.text.clone();
                self.input = TextInputState::new();
                return self.command(&command);
            }
        }
        self.input.handle_event(event);
        true
    }
    fn panel(&self, title: &str, width: u16, height: u16, content: Node) -> Node {
        Node::panel(title, BorderType::Single, self.palette.muted)
            .background(Color::Reset)
            .width(width as f32)
            .height(height as f32)
            .child(content)
    }
    fn lines(&self, lines: Vec<Line>) -> Node {
        Node::rich_text_wrapped(RichText::from_lines(lines), WrapMode::NoWrap)
    }
    fn positions(width: u16, height: u16) -> [(i32, i32); 7] {
        let w = i32::from(width.saturating_sub(2).max(8));
        let h = i32::from(height.saturating_sub(2).max(8));
        let cx = w / 2;
        let left = (cx / 2).max(4);
        let right = (cx + cx / 2).min(w - 6);
        [
            (cx, 1),
            (cx, (h / 3).max(3)),
            (left, (h * 3 / 5).max(5)),
            (right, (h * 3 / 5).max(5)),
            (cx, (h * 4 / 5).min(h - 3)),
            (cx, h - 2),
            ((left / 2).max(4), (h * 4 / 5).min(h - 3)),
        ]
    }
    fn map(&self, width: u16, height: u16) -> Node {
        let w = width.saturating_sub(2).max(8);
        let h = height.saturating_sub(2).max(8);
        let mut surface = Surface::new(w, h);
        let p = self.palette;
        let world = &self.world;
        let positions = Self::positions(width, height);
        let t = world.visual_time().as_secs_f32();
        for (index, edge) in world.graph.edges.iter().enumerate() {
            if (edge.from == NodeId::Decoy || edge.to == NodeId::Decoy)
                && !world.graph.nodes[NodeId::Decoy.index()].visible
            {
                continue;
            }
            let (x0, y0) = positions[edge.from.index()];
            let (x1, y1) = positions[edge.to.index()];
            let point = |q: f32| {
                (
                    (x0 as f32 + (x1 - x0) as f32 * q) * 2.0,
                    (y0 as f32 + (y1 - y0) as f32 * q) * 4.0 + 2.0,
                )
            };
            let mut line = BrailleCanvas::new(w, h);
            let active = world.elapsed_ms >= 3500
                && world.remote_active
                && (world.planner.path.windows(2).any(|pair| {
                    (pair[0] == edge.from && pair[1] == edge.to)
                        || (pair[1] == edge.from && pair[0] == edge.to)
                }) || (world.graph.node(edge.from).influence < 0
                    && world.graph.node(edge.to).influence < 0));
            let held = world.outcome == Some(battle::Outcome::Stalemate)
                && world.graph.node(edge.from).owner() == Control::Contested
                && world.graph.node(edge.to).owner() == Control::Contested;
            if edge.connected {
                if active || held {
                    // Acid grammar is a broken carrier, distinguishable without RGB.
                    for segment in 0..12 {
                        if segment % 3 != 2 {
                            let a = point(segment as f32 / 12.0);
                            let b = point((segment + 1) as f32 / 12.0);
                            line.line(a.0 as i32, a.1 as i32, b.0 as i32, b.1 as i32);
                        }
                    }
                } else {
                    let a = point(0.0);
                    let b = point(1.0);
                    line.line(a.0 as i32, a.1 as i32, b.0 as i32, b.1 as i32);
                }
            } else {
                for (a, b) in [(0.0, 0.35), (0.65, 1.0)] {
                    let a = point(a);
                    let b = point(b);
                    line.line(a.0 as i32, a.1 as i32, b.0 as i32, b.1 as i32);
                }
                let mid = point(0.5);
                surface.print_str(
                    (mid.0 / 2.0).max(0.0) as u16,
                    (mid.1 / 4.0).max(0.0) as u16,
                    "×",
                    p.warning,
                    Some(w),
                );
            }
            line.paint_into(
                &mut surface,
                (0, 0),
                if !edge.connected {
                    p.muted.dim()
                } else if active || held {
                    p.acid
                } else {
                    p.muted
                },
            );
            if edge.connected {
                let q = (t * 0.28 + index as f32 * 0.137).fract();
                let a = point(q);
                let mut packet = BrailleCanvas::new(w, h);
                let tail = point((q - 0.035).max(0.0));
                packet.line(tail.0 as i32, tail.1 as i32, a.0 as i32, a.1 as i32);
                packet.paint_into(&mut surface, (0, 0), p.crash);
                if active {
                    let direction = world
                        .planner
                        .path
                        .windows(2)
                        .any(|pair| pair[0] == edge.from && pair[1] == edge.to);
                    let a = point(if direction { q } else { 1.0 - q });
                    let mut packet = BrailleCanvas::new(w, h);
                    packet.set(a.0 as i32, a.1 as i32);
                    let tail = point(if direction {
                        (q - 0.045).max(0.0)
                    } else {
                        (1.0 - q + 0.045).min(1.0)
                    });
                    packet.set(tail.0 as i32, tail.1 as i32);
                    packet.paint_into(&mut surface, (0, 0), p.acid);
                    if world.trace_confidence > 40 {
                        let a = point(if direction { 1.0 - q } else { q });
                        let mut pulse = BrailleCanvas::new(w, h);
                        pulse.line(a.0 as i32 - 1, a.1 as i32, a.0 as i32 + 1, a.1 as i32);
                        pulse.paint_into(&mut surface, (0, 0), p.crash);
                    }
                }
            }
        }
        for node in &world.graph.nodes {
            if node.id == NodeId::Decoy && !node.visible {
                continue;
            }
            let (mut x, y) = positions[node.id.index()];
            if node.isolated {
                x += if x < i32::from(w) / 2 { -2 } else { 2 };
            }
            let marker = if node.isolated {
                "×"
            } else if node.id == NodeId::Decoy {
                "◇◇"
            } else {
                match node.owner() {
                    Control::Crash => "●",
                    Control::Contested => "≋",
                    Control::Acid => "‹",
                }
            };
            let label = format!("{marker} {}", node.id.name().to_ascii_uppercase());
            surface.print_str(
                (x - 4).max(0) as u16,
                y.max(0) as u16,
                &label,
                if node.visible {
                    p.owner(node.owner())
                } else {
                    p.muted.dim()
                },
                Some(w),
            );
            // A cell frontier inhabits the node itself; integrity stays independent.
            if y + 1 < i32::from(h) && world.remote_active && node.influence < 900 && !node.isolated
            {
                let dots = ((1000 - i32::from(node.influence)) * 12 / 2000).clamp(1, 12);
                let mut frontier = BrailleCanvas::new(w, h);
                let start = (x - 3).max(0) * 2;
                frontier.line(start, (y + 1) * 4, start + dots - 1, (y + 1) * 4);
                frontier.paint_into(&mut surface, (0, 0), p.acid);
            }
        }
        if world.trace_confidence > 40 {
            surface.print_str(
                1,
                0,
                &format!("TRACE {:02}% ↗", world.trace_confidence / 10),
                p.crash,
                Some(w),
            );
        }
        if world.graph.nodes[NodeId::Route.index()].isolated {
            surface.print_str(1, 2, "ROUTE CUT", p.warning, Some(w));
        }
        if self.facts().bool("crash-blind") {
            surface.print_str(1, 0, "LOCAL TELEMETRY LOST", p.warning, Some(w));
        }
        self.panel(
            " LOCAL FABRIC / live topology ",
            width,
            height,
            Node::raster(surface),
        )
    }
    fn sessions(&self, width: u16, height: u16) -> Node {
        let p = self.palette;
        let w = &self.world;
        let mut lines = vec![
            Line::new()
                .span(Span::styled("REMOTE  ", p.muted))
                .span(Span::styled(self.facts().text("identity"), p.white)),
            Line::styled(
                format!(
                    "HERE {} → {}",
                    w.planner.location.name().to_ascii_uppercase(),
                    w.planner.target.name().to_ascii_uppercase()
                ),
                p.white,
            ),
            Line::styled(
                format!(
                    "TRACE {:3}% / reserve {:3}%",
                    w.trace_confidence / 10,
                    w.resources / 10
                ),
                p.crash,
            ),
            Line::styled("SYSTEM    HOLD       INTEGRITY", p.muted),
        ];
        if w.elapsed_ms < 3500 {
            lines[0] = Line::new()
                .span(Span::styled("LOCAL   ", p.muted))
                .span(Span::styled("CRASH OVERRIDE", p.crash));
            lines[1] = Line::styled("1 local lease / no remote session", p.white);
        }
        if self.world.outcome.is_some() && !self.world.remote_active {
            lines[0] = Line::styled("REMOTE  DISCONNECTED", p.muted);
            lines[1] = Line::styled("lease closed / history retained", p.white);
        }
        if self.facts().bool("crash-blind") {
            lines[2] = Line::styled("LOCAL TELEMETRY LOST", p.warning);
        }
        for node in &w.graph.nodes[..6] {
            lines.push(Line::styled(
                if node.visible {
                    format!(
                        "{:<8}  {:<9} {:3}%",
                        node.id.name().to_ascii_uppercase(),
                        node.owner().as_str(),
                        node.integrity / 10
                    )
                } else {
                    format!("{:<8}  TELEMETRY LOST", node.id.name().to_ascii_uppercase())
                },
                if node.visible {
                    p.owner(node.owner())
                } else {
                    p.muted.dim()
                },
            ));
        }
        lines.push(Line::styled(
            format!("FILES / {} altered entries", w.quality.altered_files),
            p.muted,
        ));
        if self.debug_battle {
            lines.clear();
            lines.push(Line::styled(
                format!("GOAL {:?}", w.planner.goal),
                p.warning,
            ));
            lines.push(Line::styled(
                format!("TACTIC {:?}", w.planner.tactic),
                p.acid,
            ));
            for candidate in w.planner.candidates.iter().take(3) {
                lines.push(Line::styled(
                    format!(
                        "{:?} {} {:+}",
                        candidate.tactic,
                        candidate.target.name().to_ascii_uppercase(),
                        candidate.score
                    ),
                    p.muted,
                ));
            }
            lines.push(Line::styled(
                format!(
                    "PATH {}",
                    w.planner
                        .path
                        .iter()
                        .map(|n| n.name())
                        .collect::<Vec<_>>()
                        .join(" > ")
                ),
                p.white,
            ));
            for group in w.graph.nodes[..6].chunks(3) {
                lines.push(Line::styled(
                    group
                        .iter()
                        .map(|node| {
                            format!(
                                "{} {:+}",
                                &node.id.name()[..1].to_ascii_uppercase(),
                                node.influence
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("  "),
                    p.warning,
                ));
            }
            if w.graph.node(NodeId::Decoy).visible {
                lines.push(Line::styled(
                    format!("MIRROR {:+}", w.graph.node(NodeId::Decoy).influence),
                    p.warning,
                ));
            }
        }
        self.panel(
            if self.debug_battle {
                " BATTLE INSPECTOR "
            } else {
                " SESSION / PROCESS VIEW "
            },
            width,
            height,
            self.lines(lines),
        )
    }
    fn remote_line(&self) -> String {
        if self.world.elapsed_ms < 3500 {
            return "no remote lease".into();
        }
        if self.world.outcome.is_some() && !self.world.remote_active {
            return self.world.remote_line.clone();
        }
        let n = self
            .world
            .elapsed_ms
            .saturating_sub(self.world.remote_line_at_ms)
            / 45;
        let text = self
            .world
            .remote_line
            .chars()
            .take(n as usize)
            .collect::<String>();
        format!(
            "{text}{}",
            if (self.world.elapsed_ms / 450).is_multiple_of(2) {
                "▍"
            } else {
                " "
            }
        )
    }
    fn event_trace(&self, width: u16, height: u16) -> Node {
        let lines = self
            .world
            .receipts
            .iter()
            .rev()
            .take(height.saturating_sub(2) as usize)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|s| Line::styled(s, self.palette.muted))
            .collect();
        self.panel(
            " RECEIPTS / the machine remembers ",
            width,
            height,
            self.lines(lines),
        )
    }
    fn actions(&self, width: u16, height: u16) -> Node {
        let p = self.palette;
        let mut lines = if self.director.is_finished() {
            vec![
                Line::styled("replay · facts · trace · damage", p.crash),
                Line::styled("scene · acid · crash · reset · exit", p.white),
            ]
        } else {
            let commands =
                if self.world.elapsed_ms >= 45000 || self.director.current_beat() == "takeover" {
                    vec!["cut link", "turn trace", "spring decoy", "let her in"]
                } else {
                    vec![
                        "trace",
                        if self.world.planner.target == NodeId::Auth {
                            "isolate auth"
                        } else {
                            "isolate"
                        },
                        "decoy",
                        if self.world.graph.nodes[NodeId::Display.index()].influence < 350 {
                            "hard isolate"
                        } else {
                            "kill"
                        },
                    ]
                };
            if height <= 5 {
                let labels = commands
                    .iter()
                    .map(|command| {
                        let status = self.world.action_status(command);
                        format!(
                            "{} {}",
                            command.to_ascii_uppercase(),
                            if status.available { "✓" } else { "·" }
                        )
                    })
                    .collect::<Vec<_>>();
                vec![
                    Line::styled(labels[..2].join(" / "), p.crash),
                    Line::styled(labels[2..].join(" / "), p.white),
                ]
            } else {
                commands
                    .into_iter()
                    .take(height.saturating_sub(3) as usize)
                    .map(|command| {
                        let status = self.world.action_status(command);
                        let state = if status.available {
                            "READY".to_string()
                        } else if status.cooldown_ms > 0 {
                            format!("WAIT {:.1}s", status.cooldown_ms as f32 / 1000.0)
                        } else {
                            status.reason.to_string()
                        };
                        Line::styled(
                            format!(
                                "{:<12} {} / {}",
                                command.to_ascii_uppercase(),
                                state,
                                status.cost
                            ),
                            if status.available { p.crash } else { p.muted },
                        )
                    })
                    .collect()
            }
        };
        lines.push(Line::styled(&self.world.last_action, p.warning));
        self.panel(
            if self.watching {
                " CRASH WORKING / LOCAL CONTROL "
            } else {
                " CRASH / LOCAL CONTROL "
            },
            width,
            height,
            self.lines(lines),
        )
    }
    /// Pure model→Scene projection. Scope and ghost trajectory are deterministic
    /// presentation inputs; ordinary widgets have no post-processing knowledge.
    pub fn presentation(&self) -> Presentation {
        self.realized_presentation(&self.scene)
    }
    fn realized_presentation(&self, scene: &Scene) -> Presentation {
        let mut presentation = self.director.presentation(scene);
        let elapsed = Duration::from_millis(self.world.elapsed_ms);
        let p = self.palette;
        let auth = &self.world.graph.nodes[NodeId::Auth.index()];
        let display = &self.world.graph.nodes[NodeId::Display.index()];
        for (entity, node, key) in [
            ("session", auth, 1),
            ("decoy", &self.world.graph.nodes[NodeId::Decoy.index()], 1),
        ] {
            let id = scene.id_of(entity).unwrap();
            if presentation.custom(id, key).is_some() {
                let fraction = (1000.0 - node.influence as f32) / 2000.0;
                let target = SceneTarget::Id(id);
                for fx in [
                    SurfaceFx::StyleOverlay(p.infection)
                        .scoped(FxMask::HorizontalWipe { fraction }),
                    SurfaceFx::Scramble {
                        seed: SEED + node.id.index() as u64,
                        intensity: 0.025,
                    }
                    .scoped(FxMask::HorizontalWipe { fraction }),
                    SurfaceFx::Scanline {
                        position: (self.world.visual_time().as_secs_f32() / 2.2).fract(),
                        style: p.acid,
                    }
                    .scoped(FxMask::HorizontalWipe { fraction }),
                ] {
                    Effect::post_process(target, fx).eval(elapsed, scene, &mut presentation);
                }
            }
        }
        let amount = (1000.0 - display.influence as f32) / 2000.0;
        let sensitive = scene
            .entities()
            .iter()
            .filter(|entity| presentation.custom(entity.id, 2).is_some())
            .map(|e| e.id)
            .collect::<Vec<_>>();
        for id in sensitive {
            let entity = scene.entity(id).unwrap();
            let fraction = match entity.label.as_str() {
                "header" => ((amount - 0.6) / 0.4).clamp(0.0, 1.0),
                "event-trace" => ((amount - 0.3) / 0.7).clamp(0.0, 1.0),
                _ => amount,
            };
            let mask = if entity.label == "system-map" {
                FxMask::Radial {
                    center: (0.5, 1.0),
                    fraction,
                }
            } else {
                FxMask::VerticalWipe { fraction }
            };
            let target = SceneTarget::Id(id);
            Effect::post_process(
                target,
                SurfaceFx::StyleOverlay(p.infection).scoped(mask.clone()),
            )
            .eval(elapsed, scene, &mut presentation);
            if fraction > 0.3 {
                Effect::post_process(
                    target,
                    SurfaceFx::Tear {
                        row: 3,
                        height: 1,
                        amount: 1,
                    }
                    .scoped(mask),
                )
                .eval(elapsed, scene, &mut presentation);
                // Cell-sized motion is a climax accent, not constant camera noise.
                if fraction > 0.85 && self.world.remote_active {
                    Effect::jitter(target, fraction, seconds(0.7), seconds(1.0))
                        .looping()
                        .eval(elapsed, scene, &mut presentation);
                }
            }
        }
        presentation
    }
    pub fn frame(&self, width: u16, height: u16) -> Node {
        let mut scene = self.scene.clone();
        let p = self.palette;
        let w = &self.world;
        let width = width.max(1);
        let height = height.max(1);
        let narrow = width < 76;
        let tension = matches!(
            self.director.current_beat(),
            "escalation" | "takeover" | "acid-win"
        );
        let header_h = 3u16;
        let prompt_h = 2u16;
        let lower_h = if narrow {
            7
        } else {
            7.min(height.saturating_sub(5) / 2)
        };
        let upper_h = height.saturating_sub(header_h + prompt_h + lower_h).max(1);
        let left_w = if narrow || tension {
            width
        } else {
            (width * 3 / 5).min(width.saturating_sub(30))
        };
        let right_w = width.saturating_sub(left_w);
        let title = if self.director.current_beat() == "acid-win" && w.remote_active {
            "A C I D   B U R N  //  borrowed display"
        } else {
            "C R A S H   O V E R R I D E  //  LOCAL NODE"
        };
        let summary = if w.elapsed_ms < 4000 {
            "MODEM LINK / 0 anomalous sessions / integrity 100%".to_string()
        } else {
            format!(
                "LOCAL FABRIC / TRACE {}% / integrity {}% / {}",
                w.trace_confidence / 10,
                w.graph.nodes[..6]
                    .iter()
                    .map(|n| u32::from(n.integrity))
                    .sum::<u32>()
                    / 60,
                if w.outcome.is_some() {
                    w.outcome_quality.as_str()
                } else {
                    "CONTEST IN PROGRESS"
                }
            )
        };
        let header = Node::col()
            .width(width as f32)
            .height(3.0)
            .background(Color::Reset)
            .child(Node::text(title, p.crash).height(1.0))
            .child(Node::text(summary, p.muted).height(1.0))
            .child(Node::rule(Some(self.director.beat_label()), p.muted).height(1.0));
        let place = |scene: &mut Scene, id: &str, node: Node, x: u16, y: u16, visible: bool| {
            let entity = scene.entity_mut(scene.id_of(id).unwrap()).unwrap();
            entity.node = node;
            entity.offset = (i32::from(x), i32::from(y));
            entity.visible = visible;
        };
        place(&mut scene, "header", header, 0, 0, true);
        place(
            &mut scene,
            "system-map",
            self.map(left_w, upper_h),
            0,
            header_h,
            true,
        );
        if narrow {
            place(&mut scene, "session", Node::col(), 0, 0, false);
            place(&mut scene, "event-trace", Node::col(), 0, 0, false);
            let node = &w.graph.nodes[w.planner.target.index()];
            place(
                &mut scene,
                "remote-session",
                self.lines(vec![
                    Line::styled(
                        format!("{} > {}", self.facts().text("identity"), self.remote_line()),
                        p.white,
                    ),
                    Line::styled(
                        format!(
                            "{} {} / TRACE {}% / {}",
                            node.id.name().to_ascii_uppercase(),
                            node.owner().as_str(),
                            w.trace_confidence / 10,
                            if w.graph.nodes[NodeId::Route.index()].isolated {
                                "ROUTE CUT"
                            } else if w.graph.nodes[NodeId::Decoy.index()].visible {
                                "DECOY OPEN"
                            } else {
                                "LINK OPEN"
                            }
                        ),
                        p.warning,
                    ),
                ])
                .width(width as f32)
                .height(2.0),
                0,
                header_h + upper_h,
                true,
            );
            place(
                &mut scene,
                "actions",
                self.actions(width, lower_h.saturating_sub(2)),
                0,
                header_h + upper_h + 2,
                true,
            );
        } else if tension {
            // The same fabric becomes a hero view. Auxiliary entities reflow
            // into small witness panels; Crash retains a full-width island.
            let witness_w = (width / 4).clamp(22, 36);
            place(
                &mut scene,
                "session",
                self.sessions(
                    witness_w,
                    if self.debug_battle {
                        upper_h.saturating_sub(2)
                    } else {
                        5
                    },
                ),
                1,
                header_h + 1,
                true,
            );
            place(
                &mut scene,
                "remote-session",
                self.panel(
                    " REMOTE / read only ",
                    witness_w + 3,
                    4,
                    self.lines(vec![Line::styled(self.remote_line(), p.white)]),
                ),
                width.saturating_sub(witness_w + 4),
                header_h + 1,
                true,
            );
            place(&mut scene, "event-trace", Node::col(), 0, 0, false);
            place(
                &mut scene,
                "actions",
                self.actions(width, lower_h),
                0,
                header_h + upper_h,
                true,
            );
        } else {
            let remote_h = 4.min(upper_h / 3);
            place(
                &mut scene,
                "session",
                self.sessions(right_w, upper_h.saturating_sub(remote_h)),
                left_w,
                header_h,
                true,
            );
            place(
                &mut scene,
                "remote-session",
                self.panel(
                    " REMOTE / read only ",
                    right_w,
                    remote_h,
                    self.lines(vec![Line::styled(self.remote_line(), p.white)]),
                ),
                left_w,
                header_h + upper_h - remote_h,
                true,
            );
            place(
                &mut scene,
                "event-trace",
                self.event_trace(left_w, lower_h),
                0,
                header_h + upper_h,
                true,
            );
            place(
                &mut scene,
                "actions",
                self.actions(right_w, lower_h),
                left_w,
                header_h + upper_h,
                true,
            );
        }
        let decoy_width = (left_w / 3).clamp(15, 26).min(width);
        place(
            &mut scene,
            "decoy",
            self.panel(
                " MIRROR FILES ",
                decoy_width,
                3,
                self.lines(vec![Line::styled(
                    if w.graph.nodes[NodeId::Decoy.index()].influence < 0 {
                        "< ACID / mirror lease"
                    } else {
                        "◇◇ lure / 0 real files"
                    },
                    p.white,
                )]),
            ),
            1,
            header_h + upper_h.saturating_sub(4),
            false,
        );
        let positions = Self::positions(left_w, upper_h);
        let path = &w.planner.path;
        let scaled = w.planner.progress as f32 / 1000.0 * path.len().saturating_sub(1) as f32;
        let segment = (scaled.floor() as usize).min(path.len().saturating_sub(1));
        let from = positions[path
            .get(segment)
            .copied()
            .unwrap_or(w.planner.location)
            .index()];
        let to = positions[path
            .get(segment + 1)
            .copied()
            .unwrap_or(w.planner.location)
            .index()];
        let progress = scaled.fract();
        let ghost = (
            from.0 as f32 + (to.0 - from.0) as f32 * progress,
            from.1 as f32 + (to.1 - from.1) as f32 * progress,
        );
        place(
            &mut scene,
            "ghost-cursor",
            Node::text("◀ AB", p.acid).width(4.0).height(1.0),
            (ghost.0.max(0.0) as u16 + 5).min(left_w.saturating_sub(4)),
            header_h + (ghost.1.max(0.0) as u16).min(upper_h.saturating_sub(1)),
            false,
        );
        let last_command = self
            .trace
            .steps
            .iter()
            .rev()
            .flat_map(|step| step.events.iter().rev())
            .find_map(|event| match event {
                StoryEvent::Command(command) => Some(command.as_str()),
                _ => None,
            });
        let placeholder = if self.watching {
            last_command.unwrap_or("observing local fabric")
        } else {
            "simulated action · Enter"
        };
        let input = Node::row()
            .width(width as f32)
            .height(1.0)
            .child(Node::text("crash > ", p.crash).width(8.0))
            .child(
                Node::text_input(
                    &self.input.text,
                    self.input.cursor_grapheme,
                    Some(placeholder),
                    p.white,
                )
                .flex_grow(1.0),
            );
        let inspector = if self.inspector.is_empty() {
            if self.debug_battle {
                format!(
                    "AI {:?}/{:?} {} {:+} / CD {:.1}/{:.1}/{:.1}s / {}",
                    w.planner.goal,
                    w.planner.tactic,
                    w.planner.target.name(),
                    w.graph.nodes[w.planner.target.index()].influence,
                    w.cooldowns[0] as f32 / 1000.0,
                    w.cooldowns[1] as f32 / 1000.0,
                    w.cooldowns[2] as f32 / 1000.0,
                    self.director.current_beat()
                )
            } else if self.watching {
                "CRASH WORKING / type to intervene · Enter · Esc / Ctrl-C exit".to_string()
            } else {
                "FICTIONAL / LOCAL SIMULATION     Enter · Esc / Ctrl-C exit".to_string()
            }
        } else {
            match self.inspector.as_str() {
                "facts" => format!(
                    "FACTS / {} semantic keys / {}",
                    self.facts().iter().count(),
                    self.facts().text("outcome")
                ),
                "trace" => format!(
                    "TRACE / {} raw steps / {} beats / {}%",
                    self.trace.steps.len(),
                    self.director.trace().beats.len(),
                    w.trace_confidence / 10
                ),
                "damage" => format!(
                    "DAMAGE / AUTH {}% / FILES {}% / DISPLAY {}%",
                    w.graph.nodes[2].integrity / 10,
                    w.graph.nodes[4].integrity / 10,
                    w.graph.nodes[5].integrity / 10
                ),
                "scene" => format!(
                    "SCENE / {} entities / {}",
                    scene.len(),
                    self.director.mounted().collect::<Vec<_>>().join(", ")
                ),
                "acid" => format!(
                    "ACID / {:?} / {:?} / {}",
                    w.planner.goal, w.planner.tactic, w.remote_line
                ),
                "crash" => format!("CRASH / {} / reserve {}%", w.last_action, w.resources / 10),
                _ => self.inspector.clone(),
            }
        };
        place(
            &mut scene,
            "prompt",
            Node::col()
                .width(width as f32)
                .height(2.0)
                .background(Color::Reset)
                .child(input)
                .child(Node::text(inspector, p.muted).height(1.0)),
            0,
            height.saturating_sub(prompt_h),
            true,
        );
        scene.to_node(
            &self.realized_presentation(&scene),
            width as f32,
            height as f32,
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|s| s == flag);
    let value = |prefix: &str| args.iter().find_map(|s| s.strip_prefix(prefix));
    let auto = !has("--manual");
    let exit_after = has("--auto") && auto;
    let deterministic = has("--deterministic");
    let depth = match value("--color=") {
        Some("mono") => Some(ColorDepth::Mono),
        Some("ansi16") => Some(ColorDepth::Ansi16),
        Some("ansi256") => Some(ColorDepth::Ansi256),
        Some("truecolor") => Some(ColorDepth::TrueColor),
        _ if has("--mono") || has("--no-color") => Some(ColorDepth::Mono),
        _ if has("--ansi16") => Some(ColorDepth::Ansi16),
        _ if has("--ansi256") => Some(ColorDepth::Ansi256),
        _ if has("--truecolor") => Some(ColorDepth::TrueColor),
        _ => None,
    };
    let freeze: Option<u64> = value("--freeze-at=").and_then(|s| s.parse().ok());
    let limit: Option<f32> = value("--seconds=").and_then(|s| s.parse().ok());
    let speed: f32 = value("--speed=")
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|s| s.is_finite())
        .unwrap_or(1.0)
        .clamp(0.1, 100.0);
    let step = seconds(speed / 60.0);
    let clock = FixedStepClock::new(step);
    let mut encounter = Encounter::new(
        value("--stage=").unwrap_or("quiet"),
        depth == Some(ColorDepth::Mono),
    );
    encounter.set_watching(auto);
    encounter.set_debug_battle(has("--debug-battle") || has("--debug-ai"));
    let mut ctx = Context::fullscreen()?;
    if let Some(depth) = depth {
        ctx.set_color_depth(depth);
    }
    let cadence = seconds(1.0 / 60.0);
    ctx.set_max_fps(60);
    ctx.set_animation_interval(cadence);
    let began = std::time::Instant::now();
    let mut last = began;
    let mut frames = 0u64;
    let mut end_frames = 0u64;
    loop {
        let frozen = freeze.is_some_and(|n| frames >= n);
        if !frozen {
            let dt = if deterministic {
                clock.advance();
                step
            } else {
                let now = std::time::Instant::now();
                let dt = now.duration_since(last);
                last = now;
                dt.mul_f32(speed)
            };
            encounter.tick(dt, auto);
            frames = frames.saturating_add(1);
        }
        let (width, height) = ctx.session.terminal_size();
        ctx.set_root(encounter.frame(width, height));
        if let Some(event) = ctx.run_once(cadence)? {
            if !encounter.handle(&event) {
                break;
            }
        }
        if limit.is_some_and(|n| began.elapsed().as_secs_f32() >= n) {
            break;
        }
        if exit_after && encounter.director.is_finished() && !frozen {
            end_frames += 1;
            if end_frames >= 40 {
                break;
            }
        }
    }
    ctx.restore()?;
    Ok(())
}
