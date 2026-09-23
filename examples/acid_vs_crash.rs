//! Acid vs Crash: a fictional terminal short film. No sockets, shell execution,
//! network probes or real intrusion. Every world mutation is a recorded story action.
//!
//! Run `cargo run --example acid_vs_crash -- --auto --deterministic`.
//! Interactive commands: trace, isolate, isolate auth, decoy, kill, hard isolate;
//! final moves: cut link, turn trace, spring decoy, let her in.
//! Inspection: --stage=climax --freeze-at=60 --color=mono --seconds=3.

use gibson::cell::{Color, Line, RichText, Span, Style};
use gibson::input::{Event, KeyCode, KeyModifiers, TextInputState};
use gibson::node::{Node, WrapMode};
use gibson::scene::{Effect, EffectBundle, Scene, SceneEntity, SceneTarget};
use gibson::story::{Beat, Condition, Facts, Story, StoryAction, StoryDirector, StoryEvent};
use gibson::surface::{BorderType, Surface};
use gibson::surface_fx::SurfaceFx;
use gibson::{BrailleCanvas, ColorDepth, Context, FixedStepClock};
use std::time::Duration;

const SEED: u64 = 0xAC1D_C2A5;
const SYSTEMS: [&str; 6] = ["modem", "route", "auth", "shell", "files", "display"];
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
fn text(key: &str, value: &str) -> StoryAction {
    StoryAction::set_text(key, value)
}
fn number(key: &str, value: f32) -> StoryAction {
    StoryAction::set_number(key, value)
}
fn flag(key: &str, value: bool) -> StoryAction {
    StoryAction::set_bool(key, value)
}
fn owner(system: &str, value: &str) -> StoryAction {
    text(&format!("{system}-control"), value)
}
fn integrity(system: &str, value: f32) -> StoryAction {
    number(&format!("{system}-integrity"), value)
}

#[derive(Clone, Copy)]
struct Palette {
    crash: Style,
    acid: Style,
    muted: Style,
    warning: Style,
    white: Style,
    bg: Color,
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
            acid: Style::new().fg(rgb(255, 79, 192)).bold().reverse(),
            muted: Style::new().fg(rgb(105, 124, 158)),
            warning: Style::new().fg(rgb(255, 200, 91)).bold(),
            white: Style::new().fg(rgb(223, 235, 255)),
            bg: Color::Reset,
        }
    }
}

fn scene_template() -> Scene {
    let mut scene = Scene::new();
    // Tags are interned before entities, so every frame and story has the same targets.
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

fn controls(mut beat: Beat) -> Beat {
    for (command, actions) in [
        (
            "watch",
            vec![text("last-action", "WATCH / observe inbound route")],
        ),
        (
            "trace",
            vec![
                number("trace-confidence", 64.0),
                flag("tracing", true),
                text("last-action", "TRACE / return token following remote route"),
                StoryAction::mount("trace-token"),
            ],
        ),
        (
            "isolate",
            vec![
                flag("route-isolated", true),
                owner("route", "Crash"),
                text("last-action", "ISOLATE / ROUTE disconnected"),
                StoryAction::mount("route-isolation"),
            ],
        ),
        (
            "isolate auth",
            vec![
                flag("auth-isolated", true),
                owner("auth", "Crash"),
                text("last-action", "ISOLATE AUTH / session quarantined"),
                StoryAction::unmount("auth-presence"),
                StoryAction::mount("auth-isolation"),
            ],
        ),
        (
            "decoy",
            vec![
                flag("decoy-open", true),
                text("last-action", "DECOY / mirror FILES mounted"),
                StoryAction::mount("decoy-reveal"),
            ],
        ),
        (
            "kill",
            vec![
                owner("auth", "Crash"),
                flag("session-killed", true),
                text(
                    "last-action",
                    "KILL SESSION / lease revoked; remote link remains",
                ),
                StoryAction::unmount("auth-presence"),
            ],
        ),
        (
            "hard isolate",
            vec![
                flag("route-isolated", true),
                flag("hard-isolated", true),
                flag("auth-isolated", true),
                owner("route", "Crash"),
                owner("display", "Crash"),
                owner("auth", "Crash"),
                owner("modem", "Crash"),
                text(
                    "last-action",
                    "HARD ISOLATE / DISPLAY clean; local visibility reduced",
                ),
                StoryAction::unmount("display-presence"),
                StoryAction::unmount("takeover"),
                StoryAction::unmount("ghost"),
                StoryAction::unmount("auth-presence"),
                StoryAction::mount("route-isolation"),
            ],
        ),
        (
            "trace token",
            vec![
                number("trace-confidence", 92.0),
                flag("tracing", true),
                text("last-action", "TRACE TOKEN / source corridor resolved"),
                StoryAction::mount("trace-token"),
            ],
        ),
    ] {
        beat = beat.reaction(Condition::command(command), actions);
    }
    beat.reaction(
        Condition::on(StoryEvent::custom("TraceDeepen")),
        [
            number("trace-confidence", 92.0),
            text(
                "last-action",
                "TRACE / source corridor narrowed to one return path",
            ),
        ],
    )
}

fn defaults() -> Vec<StoryAction> {
    let mut actions = vec![
        text("identity", "UNRESOLVED"),
        text("outcome", "LIVE"),
        text("acid-target", "route"),
        number("trace-confidence", 0.0),
        number("pressure", 0.0),
        number("altered-files", 0.0),
        text("remote-line", "link idle"),
        text("last-action", "LOCAL CONTROL / awaiting input"),
    ];
    for system in SYSTEMS {
        actions.push(owner(system, "Crash"));
        actions.push(integrity(system, 100.0));
    }
    actions
}

/// Immutable graph; semantic controls and ownership remain inside Story Facts.
/// Stage starts use coherent on-enter initialization, so the normal StoryTrace
/// contains the complete input model and can be replayed without hidden jumps.
fn battle_story(scene: &mut Scene, stage: &str, p: Palette) -> Story {
    let target = |scene: &mut Scene, tag| SceneTarget::Tag(scene.tag(tag));
    let sensitive = target(scene, "display-sensitive");
    let auth = target(scene, "auth");
    let map = target(scene, "map");
    let remote = target(scene, "remote");
    let decoy = target(scene, "decoy");
    let cursor = target(scene, "cursor");
    let header = target(scene, "header");
    let mut story = Story::new(stage)
        .bundle(EffectBundle::new("signature").effect(Effect::post_process(
            remote,
            SurfaceFx::StyleOverlay(p.acid),
        )))
        .bundle(EffectBundle::new("auth-presence").effects([
            Effect::post_process(auth, SurfaceFx::StyleOverlay(p.acid)),
            Effect::jitter(auth, 1.0, seconds(0.23), seconds(2.0)).looping(),
            Effect::post_process(
                auth,
                SurfaceFx::Tear {
                    row: 4,
                    height: 1,
                    amount: 1,
                },
            ),
            Effect::post_process(
                auth,
                SurfaceFx::Scramble {
                    seed: SEED,
                    intensity: 0.018,
                },
            ),
            Effect::scanline(auth, p.acid, seconds(2.7)).looping(),
        ]))
        .bundle(EffectBundle::new("display-presence").effects([
            Effect::style_mask(sensitive, p.acid, 0.04, 0.38, SEED + 2, seconds(5.0)),
            Effect::jitter(map, 1.0, seconds(0.19), seconds(3.0)).looping(),
            Effect::scanline(sensitive, p.acid, seconds(3.6)).looping(),
            Effect::post_process(
                map,
                SurfaceFx::Tear {
                    row: 7,
                    height: 2,
                    amount: 2,
                },
            ),
            Effect::post_process(
                header,
                SurfaceFx::Scramble {
                    seed: SEED + 1,
                    intensity: 0.08,
                },
            ),
        ]))
        .bundle(EffectBundle::new("takeover").effects([
            Effect::style_mask(sensitive, p.acid, 0.1, 0.96, SEED + 2, seconds(5.0)),
            Effect::dissolve(sensitive, 1.0, 0.82, SEED, seconds(5.0)),
            Effect::post_process(
                sensitive,
                SurfaceFx::RowShift {
                    amount: 2,
                    seed: SEED + 3,
                },
            ),
            Effect::scanline(sensitive, p.acid, seconds(0.8)).looping(),
            Effect::jitter(sensitive, 1.0, seconds(0.13), seconds(2.0)).looping(),
        ]))
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
        .bundle(
            EffectBundle::new("trace-token")
                .effect(Effect::scanline(map, p.crash, seconds(2.2)).looping()),
        )
        .bundle(EffectBundle::new("decoy-reveal").effects([
            Effect::reveal(decoy, 1.0, 1.0, Duration::ZERO),
            Effect::dissolve(decoy, 0.0, 1.0, SEED + 8, seconds(1.2)),
        ]))
        .bundle(EffectBundle::new("decoy-occupied").effects([
            Effect::post_process(decoy, SurfaceFx::StyleOverlay(p.acid)),
            Effect::jitter(decoy, 1.0, seconds(0.12), seconds(1.5)).looping(),
            Effect::scanline(decoy, p.acid, seconds(1.1)).looping(),
        ]))
        .bundle(
            EffectBundle::new("ghost").effects([
                Effect::reveal(cursor, 1.0, 1.0, Duration::ZERO),
                Effect::displace(cursor, (0.0, 0.0), (12.0, 4.0), seconds(3.0))
                    .then(Effect::displace(
                        cursor,
                        (12.0, 4.0),
                        (0.0, 0.0),
                        seconds(3.0),
                    ))
                    .looping(),
            ]),
        )
        .bundle(EffectBundle::new("counter-recoil").effect(Effect::displace(
            map,
            (2.0, 0.0),
            (0.0, 0.0),
            seconds(0.5),
        )))
        .bundle(EffectBundle::new("acid-victory").effects([
            Effect::displace(map, (0.0, 0.0), (2.0, 1.0), seconds(1.0)),
            Effect::displace(auth, (0.0, 0.0), (-2.0, 0.0), seconds(1.0)),
            Effect::post_process(sensitive, SurfaceFx::StyleOverlay(p.acid)),
            Effect::scanline(sensitive, p.acid, seconds(2.0)).looping(),
        ]));

    type BeatSpec = (
        &'static str,
        &'static str,
        Vec<StoryAction>,
        Option<(&'static str, f32)>,
    );
    let entries: Vec<BeatSpec> = vec![
        (
            "quiet",
            "LOCAL NODE / all routes nominal",
            vec![],
            Some(("knock", 4.0)),
        ),
        (
            "knock",
            "UNKNOWN INBOUND HANDSHAKE",
            vec![
                number("pressure", 8.0),
                text("remote-line", "inbound carrier / no identity"),
                text("last-action", "WATCH / TRACE / DROP available"),
            ],
            Some(("signature", 3.0)),
        ),
        (
            "signature",
            "SESSION IDENTITY RESOLVED",
            vec![
                text("identity", "ACID BURN"),
                text("remote-line", "hello crash."),
                number("pressure", 16.0),
                StoryAction::mount("signature"),
                StoryAction::mount("ghost"),
            ],
            Some(("route-contested", 3.4)),
        ),
        (
            "route-contested",
            "ROUTE / two hands on one switch",
            vec![number("pressure", 28.0)],
            Some(("first-breach", 6.0)),
        ),
        (
            "first-breach",
            "SESSION HIJACK / AUTH lease contested",
            vec![
                number("pressure", 43.0),
                text("remote-line", "your windows have good acoustics."),
            ],
            Some(("adapt", 5.0)),
        ),
        (
            "adapt",
            "REMOTE SESSION / choosing a new route",
            vec![],
            None,
        ),
        (
            "sidepath",
            "ACID ADAPTS / bypass via MODEM",
            vec![
                owner("modem", "Contested"),
                text("acid-target", "display"),
                text("remote-line", "you closed a door. i found a window."),
                flag("acid-adapted", true),
            ],
            Some(("display-intrusion", 3.8)),
        ),
        (
            "decoy",
            "DECOY TRIGGERED / mirror occupied",
            vec![
                flag("decoy-open", true),
                flag("decoy-taken", true),
                owner("auth", "Crash"),
                text("acid-target", "decoy"),
                number("trace-confidence", 82.0),
                text("remote-line", "this room looks familiar."),
                flag("acid-adapted", true),
                StoryAction::unmount("auth-presence"),
                StoryAction::mount("decoy-reveal"),
                StoryAction::mount("decoy-occupied"),
            ],
            Some(("display-intrusion", 4.0)),
        ),
        (
            "trace",
            "TRACE NOTICED / source route splits",
            vec![
                flag("false-paths", true),
                StoryAction::mount("trace-token"),
                flag("tracing", true),
                number("trace-confidence", 64.0),
                text("acid-target", "display"),
                text("remote-line", "following me? keep up."),
                number("pressure", 62.0),
                flag("acid-adapted", true),
            ],
            Some(("display-intrusion", 4.0)),
        ),
        (
            "pressure",
            "FILE CORRUPTION / one directory scar",
            vec![
                owner("files", "Contested"),
                integrity("files", 84.0),
                number("altered-files", 3.0),
                text("acid-target", "files"),
                number("pressure", 65.0),
                flag("acid-adapted", true),
            ],
            Some(("display-intrusion", 3.8)),
        ),
        (
            "display-intrusion",
            "DISPLAY INTRUSION / existing surfaces contested",
            vec![
                owner("display", "Contested"),
                integrity("display", 71.0),
                text("acid-target", "display"),
                number("pressure", 72.0),
                text("remote-line", "still there?"),
                StoryAction::mount("display-presence"),
            ],
            Some(("ghost", 5.2)),
        ),
        (
            "ghost",
            "REMOTE CURSOR / she can see this room",
            vec![
                StoryAction::mount("ghost"),
                text("remote-line", "that little cyan line is yours. for now."),
            ],
            Some(("trap", 4.0)),
        ),
        (
            "trap",
            "SET THE TRAP / choose what to sacrifice",
            vec![text("last-action", "DECOY / HARD ISOLATE / TRACE TOKEN")],
            None,
        ),
        (
            "climax",
            "CONTESTED CONTROL / route collision",
            vec![
                owner("shell", "Contested"),
                number("pressure", 88.0),
                StoryAction::mount("counter-recoil"),
            ],
            Some(("takeover", 5.0)),
        ),
        (
            "takeover",
            "DISPLAY TAKEOVER / command island survives",
            vec![
                owner("display", "Acid"),
                integrity("display", 31.0),
                number("pressure", 100.0),
                text("remote-line", "i can almost fit the whole room in my hand."),
                StoryAction::mount("takeover"),
            ],
            Some(("acid-win", 8.0)),
        ),
        (
            "crash-win",
            "LINK CUT / Crash contains",
            vec![
                text("outcome", "CRASH CONTAINS"),
                text("remote-line", "nice catch. keep the receipt."),
                number("pressure", 0.0),
                number("trace-confidence", 100.0),
                flag("link-cut", true),
            ],
            None,
        ),
        (
            "acid-win",
            "DISPLAY OWNER / Acid wins the round",
            vec![
                text("outcome", "ACID WINS THE ROUND"),
                owner("display", "Acid"),
                text("remote-line", "borrowed your screen. left it better."),
                StoryAction::mount("acid-victory"),
            ],
            Some(("release", 3.5)),
        ),
        (
            "stalemate",
            "CARRIER HOLD / mutual respect",
            vec![
                text("outcome", "STALEMATE / MUTUAL RESPECT"),
                owner("route", "Contested"),
                text("remote-line", "same time. different door."),
                number("pressure", 0.0),
                number("trace-confidence", 92.0),
            ],
            None,
        ),
        (
            "release",
            "REMOTE DISCONNECTED / the machine remembers",
            vec![
                owner("display", "Crash"),
                text("remote-line", "screen returned. rivalry retained."),
                StoryAction::unmount("acid-victory"),
                number("pressure", 0.0),
            ],
            None,
        ),
    ];
    // Inspection stages reconstruct prior semantic causes at time zero. A stage's
    // initialization is part of its Story definition, not out-of-band mutation.
    let mut prelude = defaults();
    let common = [
        "quiet",
        "knock",
        "signature",
        "route-contested",
        "first-breach",
        "display-intrusion",
        "ghost",
        "trap",
        "climax",
        "takeover",
    ];
    let rank = match stage {
        "adapt" | "sidepath" | "decoy" | "trace" | "pressure" => 5,
        "crash-win" | "acid-win" | "stalemate" | "release" => common.len(),
        _ => common.iter().position(|s| *s == stage).unwrap_or(0),
    };
    if rank >= 1 {
        prelude.push(number("trace-confidence", 4.0));
    }
    if rank >= 3 {
        prelude.push(owner("route", "Contested"));
    }
    if rank >= 4 {
        prelude.extend([
            owner("auth", "Acid"),
            integrity("auth", 62.0),
            text("acid-target", "auth"),
            StoryAction::mount("auth-presence"),
        ]);
    }
    for prior in common.iter().take(rank) {
        if let Some((_, _, actions, _)) = entries.iter().find(|(id, _, _, _)| id == prior) {
            prelude.extend(actions.clone());
        }
    }
    if stage == "sidepath" {
        prelude.extend([
            flag("route-isolated", true),
            owner("route", "Crash"),
            StoryAction::mount("route-isolation"),
        ]);
    }
    for (id, label, actions, next) in entries {
        let mut beat = Beat::new(id).label(label);
        if id == stage {
            for action in &prelude {
                beat = beat.on_enter(action.clone());
            }
        }
        for action in actions {
            beat = beat.on_enter(action);
        }
        if let Some((next, after)) = next {
            beat = beat.after(seconds(after), next);
        }
        if id == "knock" {
            beat = beat.reaction(
                Condition::on(StoryEvent::custom("HandshakeObserved")),
                [number("trace-confidence", 4.0)],
            );
        }
        if id == "climax" {
            beat = beat.effect(Effect::translate(map, (0.0, 3.0), (1.0, 3.0), seconds(2.0)));
        }
        if id == "signature" {
            beat = beat.effect(Effect::displace(auth, (1.0, 0.0), (0.0, 0.0), seconds(0.3)));
        }
        if id == "route-contested" {
            beat = beat.reaction(
                Condition::on(StoryEvent::custom("RouteProbe")),
                [owner("route", "Contested"), flag("route-probed", true)],
            );
        }
        if id == "first-breach" {
            beat = beat.reaction(
                Condition::on(StoryEvent::custom("SessionHijack")),
                [
                    owner("auth", "Acid"),
                    integrity("auth", 62.0),
                    flag("auth-breached", true),
                    text("acid-target", "auth"),
                    StoryAction::mount("auth-presence"),
                ],
            );
        }
        if id == "adapt" {
            for (event, next) in [
                ("DecoyTriggered", "decoy"),
                ("SidePath", "sidepath"),
                ("TraceAttempt", "trace"),
                ("FileCorruption", "pressure"),
            ] {
                beat = beat.transition(Condition::on(StoryEvent::custom(event)), next);
            }
            beat = beat.after(seconds(2.0), "pressure");
        }
        if id == "trap" {
            for event in ["TrapDecoy", "TrapIsolate", "TrapTrace", "TrapPressure"] {
                beat = beat.transition(Condition::on(StoryEvent::custom(event)), "climax");
            }
            beat = beat.after(seconds(7.0), "climax");
            beat = beat.reaction(
                Condition::on(StoryEvent::custom("TrapDecoy")),
                [
                    flag("decoy-taken", true),
                    text("acid-target", "decoy"),
                    text("remote-line", "a mirror. you built me a mirror."),
                    StoryAction::mount("decoy-occupied"),
                    number("trace-confidence", 94.0),
                ],
            );
            beat = beat.reaction(
                Condition::on(StoryEvent::custom("TrapIsolate")),
                [
                    text("acid-target", "display"),
                    text("remote-line", "then we fight over the glass."),
                    owner("display", "Contested"),
                    StoryAction::mount("display-presence"),
                ],
            );
            beat = beat.reaction(
                Condition::on(StoryEvent::custom("TrapTrace")),
                [
                    flag("false-paths", true),
                    text("remote-line", "pick a reflection."),
                    number("trace-confidence", 92.0),
                ],
            );
        }
        if !["crash-win", "acid-win", "stalemate", "release"].contains(&id) {
            beat = controls(beat);
            // Early cut/drop is a meaningful route counter, not an early finale.
            if ["climax", "takeover"].contains(&id) {
                for (command, next) in [("cut link", "crash-win"), ("let her in", "acid-win")] {
                    beat = beat.transition(Condition::command(command), next);
                }
                beat = beat
                    .transition(Condition::on(StoryEvent::custom("TurnTrace")), "stalemate")
                    .transition(
                        Condition::on(StoryEvent::custom("SpringDecoy")),
                        "crash-win",
                    )
                    .reaction(
                        Condition::on(StoryEvent::custom("TurnTrace")),
                        [text(
                            "last-action",
                            "TURN TRACE / source resolved; carrier held",
                        )],
                    )
                    .reaction(
                        Condition::on(StoryEvent::custom("SpringDecoy")),
                        [text(
                            "last-action",
                            "SPRING DECOY / mirror sealed; remote lease contained",
                        )],
                    )
                    .reaction(
                        Condition::command("cut link"),
                        [text(
                            "last-action",
                            "CUT LINK / remote carrier disconnected",
                        )],
                    )
                    .reaction(
                        Condition::command("let her in"),
                        [text(
                            "last-action",
                            "LET HER IN / display lease voluntarily surrendered",
                        )],
                    )
                    .reaction(
                        Condition::command("turn trace"),
                        [text(
                            "last-action",
                            "TURN TRACE needs TRACE or TRACE TOKEN first",
                        )],
                    )
                    .reaction(
                        Condition::command("spring decoy"),
                        [text("last-action", "SPRING DECOY needs DECOY first")],
                    );
            } else {
                beat = beat.reaction(
                    Condition::command("drop"),
                    [
                        flag("route-isolated", true),
                        owner("route", "Crash"),
                        text("last-action", "DROP / inbound route closed"),
                    ],
                );
            }
        }
        if ["crash-win", "acid-win", "stalemate"].contains(&id) {
            for bundle in [
                "takeover",
                "display-presence",
                "auth-presence",
                "ghost",
                "decoy-occupied",
                "trace-token",
            ] {
                beat = beat.on_enter(StoryAction::unmount(bundle));
            }
            if id != "acid-win" {
                beat = beat.on_enter(flag("remote-disconnected", true));
                for system in SYSTEMS {
                    if id != "stalemate" || system != "route" {
                        beat = beat.on_enter(owner(system, "Crash"));
                    }
                }
            }
            if id != "acid-win" {
                beat = beat.terminal();
            }
        }
        if id == "release" {
            beat = beat.on_enter(flag("remote-disconnected", true)).terminal();
            for system in SYSTEMS {
                beat = beat.on_enter(owner(system, "Crash"));
            }
        }
        story = story.beat(beat);
    }
    story
        .validate()
        .expect("the fictional battle graph is valid");
    story
}

/// Rules consume only replayable facts and beat time; the seed chooses a stable
/// passive preference. No random state, wall clock, networking or external AI.
pub struct AcidController;
impl AcidController {
    pub fn decide(director: &StoryDirector) -> Option<StoryEvent> {
        let f = director.facts();
        let time = director.time_in_beat().as_secs_f32();
        let event = match director.current_beat() {
            "knock" if f.number("trace-confidence") == 0.0 => "HandshakeObserved",
            "route-contested" if !f.bool("route-isolated") && !f.bool("route-probed") => {
                "RouteProbe"
            }
            "first-breach"
                if !f.bool("route-isolated")
                    && !f.bool("auth-isolated")
                    && !f.bool("session-killed")
                    && !f.bool("auth-breached") =>
            {
                "SessionHijack"
            }
            "adapt" if time >= 0.8 => {
                if f.bool("decoy-open") {
                    "DecoyTriggered"
                } else if f.bool("route-isolated")
                    || f.bool("session-killed")
                    || f.bool("auth-isolated")
                {
                    "SidePath"
                } else if f.bool("tracing") {
                    "TraceAttempt"
                } else if SEED & 1 == 1 {
                    "FileCorruption"
                } else {
                    "SidePath"
                }
            }
            "trap" if time >= 3.0 => {
                if f.bool("decoy-open") {
                    "TrapDecoy"
                } else if f.bool("hard-isolated") || f.bool("route-isolated") {
                    "TrapIsolate"
                } else if f.bool("tracing") {
                    "TrapTrace"
                } else {
                    "TrapPressure"
                }
            }
            _ => return None,
        };
        Some(StoryEvent::custom(event))
    }
}

pub struct CrashController;
impl CrashController {
    pub fn decide(d: &StoryDirector) -> Option<StoryEvent> {
        let f = d.facts();
        let command = match d.current_beat() {
            "route-contested" if !f.bool("tracing") => "trace",
            "first-breach" if !f.bool("route-isolated") && d.time_in_beat() >= seconds(1.4) => {
                "isolate"
            }
            "ghost" if !f.bool("decoy-open") => "decoy",
            "trap" if f.number("trace-confidence") < 90.0 => "trace token",
            "takeover" if d.time_in_beat() >= seconds(2.5) => "cut link",
            _ => return None,
        };
        Some(StoryEvent::command(command))
    }
}

/// Testable demo model. Every important world field is a Story Fact. UI editor
/// and inspector selection are deliberately outside replay: neither alters the world.
pub struct Encounter {
    story: Story,
    director: StoryDirector,
    scene: Scene,
    palette: Palette,
    pub input: TextInputState,
    inspector: String,
    mono: bool,
}
impl Encounter {
    pub fn new(stage: &str, mono: bool) -> Self {
        let stage = if STAGES.contains(&stage) {
            stage
        } else {
            "quiet"
        };
        let mut scene = scene_template();
        let palette = Palette::new(mono);
        let story = battle_story(&mut scene, stage, palette);
        let director = story.start();
        Self {
            story,
            director,
            scene,
            palette,
            input: TextInputState::new(),
            inspector: String::new(),
            mono,
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
    pub fn update(&mut self, dt: Duration, events: &[StoryEvent]) {
        let mut recorded = Vec::new();
        let mut has_decoy = self.facts().bool("decoy-open");
        let mut has_trace = self.facts().bool("tracing");
        for event in events {
            recorded.push(event.clone());
            if let StoryEvent::Command(command) = event {
                if command == "trace" && has_trace {
                    recorded.push(StoryEvent::custom("TraceDeepen"));
                }
                if command == "decoy" {
                    has_decoy = true;
                }
                if command == "trace" || command == "trace token" {
                    has_trace = true;
                }
                if command == "spring decoy" && has_decoy {
                    recorded.push(StoryEvent::custom("SpringDecoy"));
                } else if command == "turn trace" && has_trace {
                    recorded.push(StoryEvent::custom("TurnTrace"));
                }
            }
        }
        self.director.update(dt, &recorded);
    }
    pub fn tick(&mut self, dt: Duration, auto: bool) {
        if auto {
            if let Some(event) = CrashController::decide(&self.director) {
                self.update(Duration::ZERO, &[event]);
            }
        }
        let events = AcidController::decide(&self.director)
            .into_iter()
            .collect::<Vec<_>>();
        self.update(dt, &events);
    }
    pub fn replay_matches(&self) -> bool {
        let replay = self.story.replay(self.director.trace());
        replay.facts() == self.facts()
            && replay.current_beat() == self.director.current_beat()
            && replay.trace() == self.director.trace()
            && replay.mounted().collect::<Vec<_>>() == self.director.mounted().collect::<Vec<_>>()
            && replay.presentation(&self.scene) == self.director.presentation(&self.scene)
    }
    /// Returns false on exit. Commands are semantic simulated actions only.
    pub fn command(&mut self, command: &str) -> bool {
        let command = command.trim().to_ascii_lowercase();
        match command.as_str() {
            "exit" | "quit" => return false,
            "reset" => {
                *self = Self::new("quiet", self.mono);
            }
            "replay" => {
                self.inspector = if self.replay_matches() {
                    "REPLAY VERIFIED / exact steps, facts, beats, bundles, presentation"
                } else {
                    "REPLAY MISMATCH"
                }
                .into();
            }
            "facts" | "trace" | "damage" | "scene" | "acid" | "crash"
                if self.director.is_finished() =>
            {
                self.inspector = command
            }
            _ => self.update(Duration::ZERO, &[StoryEvent::command(command)]),
        }
        true
    }
    pub fn handle(&mut self, event: &Event) -> bool {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                return false;
            }
            if key.code == KeyCode::Esc {
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
            .background(self.palette.bg)
            .width(width as f32)
            .height(height as f32)
            .child(content)
    }
    fn lines(&self, lines: Vec<Line>) -> Node {
        Node::rich_text_wrapped(RichText::from_lines(lines), WrapMode::NoWrap)
    }
    fn control_style(&self, system: &str) -> Style {
        match self.facts().text(&format!("{system}-control")) {
            "Acid" => self.palette.acid,
            "Contested" => self.palette.warning,
            _ => self.palette.crash,
        }
    }

    fn map(&self, width: u16, height: u16) -> Node {
        let w = width.saturating_sub(2).max(8);
        let h = height.saturating_sub(2).max(8);
        let p = self.palette;
        let f = self.facts();
        let mut surface = Surface::new(w, h);
        let cx = i32::from(w) / 2;
        let left = (cx / 2).max(5);
        let right = (cx + cx / 2).min(i32::from(w) - 6);
        let top = 1;
        let route_y = (i32::from(h) / 3).max(3);
        let fork_y = (i32::from(h) * 3 / 5).max(route_y + 2);
        let files_y = (i32::from(h) * 4 / 5).min(i32::from(h) - 3);
        let positions = [
            (cx, top),
            (cx, route_y),
            (left, fork_y),
            (right, fork_y),
            (cx, files_y),
            (cx, i32::from(h) - 2),
        ];
        let t = self.director.elapsed().as_secs_f32();
        let active = f.number("pressure") > 0.0;
        for (index, (a, b)) in [(0, 1), (1, 2), (1, 3), (2, 4), (3, 4), (4, 5)]
            .into_iter()
            .enumerate()
        {
            let isolated = f.bool("hard-isolated")
                || (f.bool("route-isolated") && (a == 1 || b == 1))
                || (f.bool("auth-isolated") && (a == 2 || b == 2));
            let (x0, y0) = positions[a];
            let (x1, y1) = positions[b];
            let mut line = BrailleCanvas::new(w, h);
            let mid = ((x0 + x1), (y0 + y1) * 2);
            if !isolated {
                line.polyline(&[(x0 * 2, y0 * 4 + 2), mid, (x1 * 2, y1 * 4 + 2)]);
            } else {
                line.line(x0 * 2, y0 * 4 + 2, (x0 * 3 + x1) / 2, (y0 * 3 + y1) + 2);
                line.line((x0 + x1 * 3) / 2, (y0 + y1 * 3) + 2, x1 * 2, y1 * 4 + 2);
            }
            line.paint_into(
                &mut surface,
                (0, 0),
                if isolated { p.muted.dim() } else { p.muted },
            );
            if !isolated {
                let progress = (t * 0.45 + index as f32 * 0.19).fract();
                let mut packet = BrailleCanvas::new(w, h);
                let x = (x0 as f32 + (x1 - x0) as f32 * progress) * 2.0;
                let y = (y0 as f32 + (y1 - y0) as f32 * progress) * 4.0 + 2.0;
                packet.filled_circle(x as i32, y as i32, 1);
                packet.paint_into(&mut surface, (0, 0), p.crash);
                let hostile_route = active
                    && (index == 0
                        || f.number("pressure") >= 65.0
                        || f.text(&format!("{}-control", SYSTEMS[a])) != "Crash"
                        || f.text(&format!("{}-control", SYSTEMS[b])) != "Crash");
                if hostile_route {
                    let q = 1.0 - progress;
                    let mut hostile = BrailleCanvas::new(w, h);
                    let x = (x0 as f32 + (x1 - x0) as f32 * q) * 2.0;
                    let y = (y0 as f32 + (y1 - y0) as f32 * q) * 4.0 + 2.0;
                    hostile.rect(x as i32 - 1, y as i32 - 1, 3, 3);
                    hostile.paint_into(&mut surface, (0, 0), p.acid);
                }
            }
        }
        // Alternate MODEM -> DISPLAY is a real semantic adaptation, not decoration.
        if f.bool("acid-adapted") && f.text("acid-target") == "display" {
            let mut bypass = BrailleCanvas::new(w, h);
            bypass.polyline(&[
                (cx * 2, 6),
                (i32::from(w) * 2 - 4, 6),
                (i32::from(w) * 2 - 4, (i32::from(h) - 2) * 4),
                (cx * 2, (i32::from(h) - 2) * 4),
            ]);
            bypass.paint_into(&mut surface, (0, 0), p.acid);
        }
        if f.bool("false-paths") {
            let mut paths = BrailleCanvas::new(w, h);
            for dx in [-6, 0, 6] {
                paths.line(cx * 2, route_y * 4, cx * 2 + dx * 2, 2);
            }
            paths.paint_into(&mut surface, (0, 0), p.acid);
        }
        for (index, (x, y)) in positions.into_iter().enumerate() {
            let system = SYSTEMS[index];
            let control = f.text(&format!("{system}-control"));
            let marker = match control {
                "Acid" => "<",
                "Contested" => "~",
                _ => "■",
            };
            let label = format!("{marker} {}", system.to_ascii_uppercase());
            surface.print_str(
                (x - 4).max(0) as u16,
                y.max(0) as u16,
                &label,
                self.control_style(system),
                Some(w),
            );
        }
        if f.bool("route-isolated") {
            surface.print_str(1, route_y as u16, "CUT", p.warning, Some(w));
        }
        if f.bool("tracing") {
            surface.print_str(
                1,
                0,
                &format!("TRACE {:02.0}% ↗", f.number("trace-confidence")),
                p.crash,
                Some(w),
            );
        }
        self.panel(
            " SYSTEM MAP / LOCAL FABRIC ",
            width,
            height,
            Node::raster(surface),
        )
    }

    fn sessions(&self, width: u16, height: u16) -> Node {
        let p = self.palette;
        let f = self.facts();
        let mut lines = vec![
            Line::new()
                .span(Span::styled("SESSION  ", p.muted))
                .span(Span::styled(
                    f.text("identity"),
                    if f.text("identity") == "UNRESOLVED" {
                        p.white
                    } else {
                        p.acid
                    },
                )),
            Line::styled(
                format!("TARGET   {}", f.text("acid-target").to_ascii_uppercase()),
                p.white,
            ),
            Line::styled(
                format!(
                    "TRACE    {:>3.0}%  {}",
                    f.number("trace-confidence"),
                    if f.bool("tracing") {
                        "RETURN TOKEN"
                    } else {
                        "PASSIVE"
                    }
                ),
                p.crash,
            ),
            Line::styled("SUBSYSTEM    OWNER      INTEGRITY", p.muted),
        ];
        if height < 12 {
            lines.pop();
            let focal = f.text("acid-target");
            let focus_system = if SYSTEMS.contains(&focal) {
                focal
            } else {
                "files"
            };
            lines.push(Line::styled(
                format!(
                    "{}  {}  {:.0}%",
                    focus_system.to_ascii_uppercase(),
                    f.text(&format!("{focus_system}-control")),
                    f.number(&format!("{focus_system}-integrity"))
                ),
                self.control_style(focus_system),
            ));
            lines.push(Line::styled(
                format!(
                    "ROUTE {} / AUTH {}",
                    f.text("route-control"),
                    f.text("auth-control")
                ),
                p.muted,
            ));
            lines.push(Line::styled(
                if f.bool("route-isolated") {
                    "ROUTE DISCONNECTED"
                } else {
                    "LOCAL FABRIC ONLINE"
                },
                p.warning,
            ));
            return self.panel(" SESSION / PROCESS VIEW ", width, height, self.lines(lines));
        }
        for system in SYSTEMS {
            lines.push(Line::styled(
                format!(
                    "{:<9}    {:<9} {:>3.0}%",
                    system.to_ascii_uppercase(),
                    f.text(&format!("{system}-control")),
                    f.number(&format!("{system}-integrity"))
                ),
                self.control_style(system),
            ));
        }
        lines.push(Line::styled(
            format!(
                "FILES / {} altered entries",
                f.number("altered-files") as u32
            ),
            p.muted,
        ));
        self.panel(" SESSION / PROCESS VIEW ", width, height, self.lines(lines))
    }

    fn event_trace(&self, width: u16, height: u16) -> Node {
        let p = self.palette;
        let capacity = height.saturating_sub(3) as usize;
        let mut events = Vec::new();
        for (at, beat) in &self.director.trace().beats {
            events.push((
                at.as_millis(),
                format!(
                    "{:05.1}  {}",
                    at.as_secs_f32(),
                    beat.replace('-', " ").to_ascii_uppercase()
                ),
                p.muted,
            ));
        }
        let mut at = Duration::ZERO;
        for step in &self.director.trace().steps {
            at = at.saturating_add(step.dt);
            for event in &step.events {
                let style = if matches!(event, StoryEvent::Command(_)) {
                    p.crash
                } else {
                    p.acid
                };
                events.push((
                    at.as_millis(),
                    format!("{:05.1}  {}", at.as_secs_f32(), event.label()),
                    style,
                ));
            }
        }
        events.sort_by_key(|e| e.0);
        let lines = events
            .iter()
            .rev()
            .take(capacity)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|(_, line, style)| Line::styled(line, *style))
            .collect();
        self.panel(
            " EVENT TRACE / causal receipts ",
            width,
            height,
            self.lines(lines),
        )
    }

    fn actions(&self, width: u16, height: u16) -> Node {
        let p = self.palette;
        let ending = self.director.is_finished();
        let final_move = ["climax", "takeover"].contains(&self.director.current_beat());
        let (first, second) = if ending {
            (
                "replay · facts · trace · damage",
                "scene · acid · crash · reset · exit",
            )
        } else if final_move {
            match (
                self.facts().bool("tracing"),
                self.facts().bool("decoy-open"),
            ) {
                (true, true) => ("CUT LINK / TURN TRACE", "SPRING DECOY / LET HER IN"),
                (true, false) => ("CUT LINK / TURN TRACE", "LET HER IN / DECOY to set a trap"),
                (false, true) => (
                    "CUT LINK / SPRING DECOY",
                    "LET HER IN / TRACE to follow her",
                ),
                (false, false) => (
                    "CUT LINK / LET HER IN",
                    "TRACE or DECOY to prepare a counter",
                ),
            }
        } else if self.director.current_beat() == "trap" {
            ("DECOY / HARD ISOLATE", "TRACE TOKEN / hold your ground")
        } else {
            (
                "TRACE / ISOLATE / DECOY",
                "ISOLATE AUTH / KILL / HARD ISOLATE",
            )
        };
        let mut lines = vec![Line::styled(first, p.crash), Line::styled(second, p.white)];
        if height >= 5 {
            lines.push(Line::styled(self.facts().text("last-action"), p.muted));
        }
        self.panel(
            if ending {
                " CRASH / BATTLE SHELL "
            } else {
                " CRASH / LOCAL CONTROL "
            },
            width,
            height,
            self.lines(lines),
        )
    }

    /// An ordinary Scene → Node frame. Rebuilding content never changes a widget
    /// to compensate for corruption; unmounting a bundle restores that content.
    pub fn frame(&self, width: u16, height: u16) -> Node {
        let mut scene = self.scene.clone();
        let p = self.palette;
        let f = self.facts();
        let width = width.max(1);
        let height = height.max(1);
        let narrow = width < 76;
        let header_h = 3u16;
        let prompt_h = 2u16;
        let body_h = height.saturating_sub(header_h + prompt_h).max(1);
        let lower_h = if narrow { 7 } else { 7.min(body_h / 2) };
        let upper_h = body_h.saturating_sub(lower_h).max(1);
        let left_w = if narrow {
            width
        } else {
            (width * 3 / 5).min(width.saturating_sub(30))
        };
        let right_w = width.saturating_sub(left_w);
        let title = if f.text("outcome") == "ACID WINS THE ROUND"
            && self.director.current_beat() == "acid-win"
        {
            "A C I D   B U R N  //  borrowed display"
        } else {
            "C R A S H   O V E R R I D E  //  LOCAL NODE"
        };
        let summary = format!(
            "{} / {} / {:05.1}s / integrity {:03.0}% / {} anomalous sessions",
            if f.bool("link-cut") {
                "LINK CUT"
            } else {
                "MODEM LINK"
            },
            f.text("outcome"),
            self.director.elapsed().as_secs_f32(),
            SYSTEMS
                .iter()
                .map(|s| f.number(&format!("{s}-integrity")))
                .sum::<f32>()
                / 6.0,
            if f.number("pressure") > 0.0 && !f.bool("remote-disconnected") {
                1
            } else {
                0
            }
        );
        let summary = if narrow && self.director.current_beat() == "quiet" {
            "MODEM LINK / 0 anomalous sessions / integrity 100%".to_owned()
        } else {
            summary
        };
        let header = Node::col()
            .width(width as f32)
            .height(header_h as f32)
            .background(p.bg)
            .child(Node::text(title, p.crash).height(1.0))
            .child(Node::text(summary, p.muted).height(1.0))
            .child(Node::rule(Some(self.director.beat_label()), p.muted).height(1.0));
        let place = |scene: &mut Scene, id: &str, node: Node, x: u16, y: u16, visible: bool| {
            let entity = scene
                .entity_mut(scene.id_of(id).expect("template entity"))
                .expect("template entity");
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
            // Intentional narrow composition: geometry plus focal ownership in
            // the map, one remote line and a full-width command island.
            place(&mut scene, "session", Node::col(), 0, 0, false);
            place(&mut scene, "event-trace", Node::col(), 0, 0, false);
            place(
                &mut scene,
                "remote-session",
                self.lines(vec![
                    Line::styled(
                        format!("{} > {}", f.text("identity"), f.text("remote-line")),
                        p.white,
                    ),
                    Line::styled(
                        format!(
                            "{} {} / TRACE {:.0}% / {}",
                            f.text("acid-target").to_ascii_uppercase(),
                            f.text(&format!("{}-control", f.text("acid-target"))),
                            f.number("trace-confidence"),
                            if f.bool("route-isolated") {
                                "ROUTE CUT"
                            } else if f.bool("decoy-open") {
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
            let remote = self.panel(
                " REMOTE SESSION / read only ",
                right_w,
                remote_h,
                self.lines(vec![Line::styled(f.text("remote-line"), p.white)]),
            );
            place(
                &mut scene,
                "remote-session",
                remote,
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
        let decoy_width = (left_w / 2).clamp(15, 28).min(width);
        let decoy = self.panel(
            " MIRROR FILES / DECOY ",
            decoy_width,
            4,
            self.lines(vec![
                Line::styled(
                    if f.bool("decoy-taken") {
                        "< ACID / trapped lease"
                    } else {
                        "■ CRASH / lure ready"
                    },
                    p.white,
                ),
                Line::styled("0 real files exposed", p.muted),
            ]),
        );
        place(
            &mut scene,
            "decoy",
            decoy,
            1,
            header_h + upper_h.saturating_sub(5),
            false,
        );
        place(
            &mut scene,
            "ghost-cursor",
            Node::text("◀ AB", p.acid).width(5.0).height(1.0),
            (left_w / 2).saturating_sub(9),
            header_h + 2,
            false,
        );
        let input = Node::row()
            .width(width as f32)
            .height(1.0)
            .child(Node::text("crash > ", p.crash).width(8.0))
            .child(
                Node::text_input(
                    &self.input.text,
                    self.input.cursor_grapheme,
                    Some("type a simulated action · Enter"),
                    p.white,
                )
                .flex_grow(1.0),
            );
        let inspector = if self.inspector.is_empty() {
            "FICTIONAL / LOCAL SIMULATION     Enter command · Esc / Ctrl-C exit".to_string()
        } else {
            match self.inspector.as_str() {
                "facts" => format!(
                    "FACTS / {} keys / outcome {} / target {}",
                    f.iter().count(),
                    f.text("outcome"),
                    f.text("acid-target")
                ),
                "trace" => format!(
                    "TRACE / {} exact steps / {} beats / confidence {:.0}%",
                    self.director.trace().steps.len(),
                    self.director.trace().beats.len(),
                    f.number("trace-confidence")
                ),
                "scene" => format!(
                    "SCENE / {} entities / bundles {}",
                    scene.len(),
                    self.director.mounted().collect::<Vec<_>>().join(", ")
                ),
                "damage" => format!(
                    "DAMAGE / {} altered files / AUTH {:.0}% / DISPLAY {:.0}%",
                    f.number("altered-files"),
                    f.number("auth-integrity"),
                    f.number("display-integrity")
                ),
                "acid" => format!(
                    "ACID / target {} / adapted {} / {}",
                    f.text("acid-target"),
                    f.bool("acid-adapted"),
                    f.text("remote-line")
                ),
                "crash" => format!(
                    "CRASH / {} / trace {:.0}%",
                    f.text("last-action"),
                    f.number("trace-confidence")
                ),
                _ => self.inspector.clone(),
            }
        };
        let prompt = Node::col()
            .width(width as f32)
            .height(prompt_h as f32)
            .background(p.bg)
            .child(input)
            .child(Node::text(inspector, p.muted).height(1.0));
        place(
            &mut scene,
            "prompt",
            prompt,
            0,
            height.saturating_sub(prompt_h),
            true,
        );
        scene.to_node(
            &self.director.presentation(&scene),
            width as f32,
            height as f32,
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|s| s == flag);
    let value = |prefix: &str| args.iter().find_map(|s| s.strip_prefix(prefix));
    let auto = has("--auto");
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
    let step = seconds(speed / 30.0);
    let clock = FixedStepClock::new(step);
    let mut encounter = Encounter::new(
        value("--stage=").unwrap_or("quiet"),
        depth == Some(ColorDepth::Mono),
    );
    let mut ctx = Context::fullscreen()?;
    if let Some(depth) = depth {
        ctx.set_color_depth(depth);
    }
    ctx.set_max_fps(30);
    ctx.set_animation_interval(Duration::from_millis(33));
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
        if let Some(event) = ctx.run_once(Duration::from_millis(33))? {
            if !encounter.handle(&event) {
                break;
            }
        }
        if limit.is_some_and(|n| began.elapsed().as_secs_f32() >= n) {
            break;
        }
        if auto && encounter.director.is_finished() && !frozen {
            end_frames += 1;
            if end_frames >= 20 {
                break;
            }
        }
    }
    ctx.restore()?;
    Ok(())
}
