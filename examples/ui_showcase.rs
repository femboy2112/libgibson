//! One operations workspace, three design grammars, and inspectable transitions.
//!
//! cargo run --example ui_showcase -- --skin=vapor95
//! cargo run --example ui_showcase -- --dump --transition=modal --at-ms=90
//! cargo run --example ui_showcase -- --profile --width=120 --height=40
//!
//! Synthetic work only: buttons never execute commands or edit files. The app
//! owns domain selection, editor contents and permission state; UiRuntime owns
//! focus, capture and finite motion. The custom instrument retains ONE shared
//! Surface, invalidated by its actual size/style/glyph inputs.

#[path = "ui_support/options.rs"]
mod options;

use gibson::ui::prelude::*;
use gibson::{
    BrailleCanvas, Context, Event, KeyCode, KeyEvent, KeyModifiers, Node, Style, Surface,
    TextInputState,
};
use options::{present, Options};
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SIGNAL: [f32; 24] = [
    3., 4., 4., 6., 3., 8., 9., 5., 7., 12., 9., 7., 11., 14., 10., 13., 9., 11., 8., 12., 7., 9.,
    6., 8.,
];
const AGENTS: [(&str, &str); 3] = [
    ("SCOUT", "Trace the renderer"),
    ("BUILDER", "Prepare the patch"),
    ("REVIEWER", "Prove the boundary"),
];
const AGENT_STAGES: [(&str, u16, u16); 3] =
    [("inspected", 4, 4), ("prepared", 2, 3), ("reviewed", 1, 3)];
const REVIEW_CODE: &str = "fn write_pair(pair, clip) {\n    if clip.contains(pair.bounds()) {\n        frame.blit_pair(pair);\n    }\n}\n// invariant: no split graphemes\n\n#[test]\nfn clipped_pair_remains_complete() {\n    assert_complete(render_fixture());\n}\n// Synthetic review buffer; no files edited.";

#[derive(Clone)]
enum Action {
    Select(usize),
    Skin,
    Run,
    Open,
    Close,
    Approve,
    Fail,
    Edit(TextInputState),
    Send,
    Scroll(Event),
}

struct Workspace {
    selected: usize,
    editor: TextInputState,
    modal: bool,
    revision: u64,
    completed: u64,
    failed: bool,
    last_command: String,
    notification: Option<String>,
    code_y: i32,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            selected: 1,
            editor: TextInputState::new(),
            modal: false,
            revision: 0,
            completed: 24,
            failed: false,
            last_command: "Ready for the next instruction".into(),
            notification: None,
            code_y: 0,
        }
    }
}
impl Workspace {
    fn apply(&mut self, action: Action) {
        match action {
            Action::Select(selected) => self.selected = selected,
            Action::Open => self.modal = true,
            Action::Close => self.modal = false,
            Action::Approve => {
                self.modal = false;
                self.success();
            }
            Action::Run => {
                self.completed += 1;
                self.last_command = "Synthetic checks complete".into();
            }
            Action::Fail => {
                self.modal = false;
                self.failed = true;
                self.revision += 1;
            }
            Action::Edit(editor) => self.editor = editor,
            Action::Send if !self.editor.text.is_empty() => {
                self.last_command = std::mem::take(&mut self.editor).text;
                self.notification = None;
            }
            Action::Scroll(Event::Key(key)) => {
                self.code_y = match key.code {
                    KeyCode::Up => self.code_y - 1,
                    KeyCode::Down => self.code_y + 1,
                    KeyCode::PageUp => self.code_y - 6,
                    KeyCode::PageDown => self.code_y + 6,
                    KeyCode::Home => 0,
                    KeyCode::End => 6,
                    _ => self.code_y,
                }
                .clamp(0, 6);
            }
            _ => {}
        }
    }
    fn success(&mut self) {
        self.failed = false;
        self.revision += 1;
        self.completed += 1;
    }

    fn patch_result(&self) -> &'static str {
        if self.failed {
            "Check failed"
        } else if self.revision > 0 {
            "Approved"
        } else {
            "Awaiting review"
        }
    }
}

#[derive(PartialEq, Eq)]
struct CanvasKey {
    width: u16,
    height: u16,
    style: Style,
    glyph: gibson::SubcellGlyphMode,
}
#[derive(Default)]
struct CanvasCache {
    entry: Option<(CanvasKey, Arc<Surface>)>,
    builds: usize,
}
impl CanvasCache {
    fn get(&mut self, cx: &BuildCx, width: u16, height: u16) -> Arc<Surface> {
        let key = CanvasKey {
            width,
            height,
            style: cx
                .skin
                .style(Tone::Accent, Emphasis::Normal)
                .bg(cx.skin.surface),
            glyph: cx.environment.glyph_mode,
        };
        if let Some((old, surface)) = &self.entry {
            if old == &key {
                return Arc::clone(surface);
            }
        }
        let mut canvas = BrailleCanvas::new(width, height);
        let w = f32::from(canvas.pixel_width().saturating_sub(1));
        let h = f32::from(canvas.pixel_height().saturating_sub(1));
        // Three projected orbital paths with declared phase positions. This is
        // a deterministic synthetic routing diagram, not measured telemetry.
        for orbit in 0..3 {
            let angle = orbit as f32 * std::f32::consts::PI / 3.;
            let point = |phase: f32| {
                let x = 0.43 * phase.cos();
                let y = 0.16 * phase.sin();
                (
                    ((0.5 + x * angle.cos() - y * angle.sin()) * w) as i32,
                    ((0.5 + x * angle.sin() + y * angle.cos()) * h) as i32,
                )
            };
            let points: Vec<_> = (0..=128)
                .map(|i| point(i as f32 * std::f32::consts::TAU / 128.))
                .collect();
            canvas.polyline(&points);
            let (x, y) = point([0.7, 2.4, 4.3][orbit]);
            canvas.line((w * 0.5) as i32, (h * 0.5) as i32, x, y);
            canvas.circle(x, y, 2);
            canvas.filled_circle(x, y, 1);
        }
        canvas.circle((w * 0.5) as i32, (h * 0.5) as i32, 2);
        let mut painted = canvas.to_surface_mode(key.style, key.glyph);
        // A raw Surface owns its blanks too; otherwise they reset the designed
        // window/ivory background underneath the custom instrument.
        for cell in &mut painted.cells {
            cell.style = key.style;
        }
        let surface = Arc::new(painted);
        self.entry = Some((key, Arc::clone(&surface)));
        self.builds += 1;
        surface
    }
}

struct SignalInstrument(Arc<Surface>);
impl<A> Component<A> for SignalInstrument {
    fn build(self, _: &BuildCx) -> Element<A> {
        surface(self.0)
    }
}

fn mission_status(model: &Workspace, short: bool) -> Element<Action> {
    status(if model.failed {
        if short {
            "CHECK FAILED"
        } else {
            "Clipping check failed / review required"
        }
    } else if model.revision > 0 {
        if short {
            "PATCH APPROVED"
        } else {
            "Patch approved / all checks pass"
        }
    } else if short {
        "READY FOR REVIEW"
    } else {
        "Preflight verified / patch ready for review"
    })
    .tone(if model.failed {
        Tone::Danger
    } else {
        Tone::Success
    })
    .emphasis(Emphasis::Strong)
    .key("mission.status")
    .revision(model.revision)
    .motion(if model.failed {
        MotionRole::Error
    } else {
        MotionRole::Success
    })
}

fn controls(model: &Workspace, narrow: bool) -> Element<Action> {
    row()
        .gap(u16::from(!narrow))
        .child(
            button(if narrow { "Run" } else { "Run checks" })
                .key("run")
                .on_press(Action::Run),
        )
        .child(
            button(if narrow { "Review" } else { "Review patch" })
                .key("review")
                .on_press(Action::Open),
        )
        .child(button("Skin").key("skin").on_press(Action::Skin))
        .child(if !narrow {
            badge(format!("{} checks", model.completed)).tone(Tone::Info)
        } else {
            spacer()
        })
}

fn with_notice(mission: Element<Action>, model: &Workspace) -> Element<Action> {
    if let Some(message) = &model.notification {
        // Local placement temporarily covers secondary mission details, while
        // status, editor and action controls stay outside the notification.
        mission.overlay(toast(message.clone()).key("notice"))
    } else {
        mission
    }
}

fn view(model: &Workspace, cx: &BuildCx, cache: &mut CanvasCache) -> Element<Action> {
    let (width, height) = (cx.environment.width, cx.environment.height);
    let tiny = width < 48 || height < 18;
    let narrow = width < 80;
    let roomy = width >= 120 && height >= 35;
    let mut tree = screen()
        .height(height)
        .density(if height < 45 {
            Density::Compact
        } else {
            Density::Normal
        })
        .gap(u16::from(roomy))
        .child(
            row()
                .child(
                    heading(if narrow {
                        "ORBIT / OPERATIONS"
                    } else {
                        "ORBIT / OPERATIONS WORKSPACE"
                    })
                    .grow(1.0),
                )
                .child(if !narrow {
                    badge("SYNTHETIC / LOCAL").tone(Tone::Info)
                } else {
                    spacer()
                }),
        )
        .child(controls(model, narrow));

    if tiny {
        tree = tree
            .child(mission_status(model, true))
            .child(
                label(format!("{} / context 62%", AGENTS[model.selected].0))
                    .emphasis(Emphasis::Muted),
            )
            .child(with_notice(
                panel("ACTIVE MISSION")
                    .number(2)
                    .gap(0)
                    .child(text("Repair the clipping boundary"))
                    .child(status("Inspect complete").tone(Tone::Success))
                    .child(
                        status(format!("Patch {}", model.patch_result().to_lowercase())).tone(
                            if model.failed {
                                Tone::Danger
                            } else if model.revision > 0 {
                                Tone::Success
                            } else {
                                Tone::Warning
                            },
                        ),
                    ),
                model,
            ));
    } else if narrow || height < 25 {
        tree = tree.child(
            row()
                .gap(1)
                .child(with_notice(
                    panel("MISSION")
                        .number(2)
                        .grow(1.3)
                        .gap(0)
                        .child(mission_status(model, true))
                        .child(label("Clipping boundary / #024"))
                        .child(progress("context", 0.62)),
                    model,
                ))
                .child(panel("AGENTS").number(1).grow(1.0).gap(0).children(
                    AGENTS.iter().enumerate().map(|(i, (name, _))| {
                        choice(*name, model.selected == i)
                            .key(format!("agent.{i}"))
                            .on_press(Action::Select(i))
                    }),
                )),
        );
        if height >= 22 {
            let instrument = cache.get(
                cx,
                width.saturating_sub(12).saturating_div(2).clamp(8, 32),
                3,
            );
            tree = tree.child(
                row()
                    .gap(1)
                    .child(
                        panel("ROUTING FIELD")
                            .number(3)
                            .gap(0)
                            .grow(1.0)
                            .child(component(SignalInstrument(instrument), cx)),
                    )
                    .child(
                        panel("REVIEW NOTES")
                            .number(4)
                            .gap(0)
                            .grow(1.0)
                            .child(code("+ clip.contains(pair)\n+ keep_complete_pair()"))
                            .child(label("Synthetic / no disk writes").emphasis(Emphasis::Muted)),
                    ),
            );
        }
    } else {
        // Allocate inside the instrument's 1.3 / 4.0 row share, allowing for
        // ordinary chrome/padding rather than assuming a fixed 38-cell canvas.
        let instrument_width = ((u32::from(width.saturating_sub(14)) * 13 / 40) as u16)
            .saturating_sub(4)
            .clamp(8, 48);
        let instrument = cache.get(cx, instrument_width, if roomy { 11 } else { 4 });
        let agents = panel("AGENT ROSTER")
            .number(1)
            .grow(1.0)
            .children(AGENTS.iter().enumerate().map(|(i, (name, task))| {
                column()
                    .gap(0)
                    .child(
                        choice(*name, model.selected == i)
                            .key(format!("agent.{i}"))
                            .on_press(Action::Select(i)),
                    )
                    .child(label(*task).emphasis(Emphasis::Muted))
                    .child(progress(
                        format!(
                            "{}/{} {}",
                            AGENT_STAGES[i].1, AGENT_STAGES[i].2, AGENT_STAGES[i].0
                        ),
                        f32::from(AGENT_STAGES[i].1) / f32::from(AGENT_STAGES[i].2),
                    ))
            }))
            .child(divider())
            .child(status("3 active / ready").tone(Tone::Info));
        let mission = panel("MISSION CONTROL / 024")
            .number(2)
            .grow(1.7)
            .gap(0)
            .child(mission_status(model, false))
            .child(text(
                "The renderer keeps every complete grapheme inside its visible boundary.",
            ))
            .child(progress("context window", 0.62))
            .child(progress("review coverage", 0.88))
            .child(label("3 workers / 2 review gates").tone(Tone::Info))
            .child(table(
                ["STAGE", "RESULT"],
                [
                    ["Inspect", "Complete"],
                    ["Reproduce", "Confirmed"],
                    ["Patch", model.patch_result()],
                ],
            ))
            .child(label("KEY CONSTRAINTS").emphasis(Emphasis::Muted))
            .child(text("Complete pairs / clipped writes / stable anchor"));
        let mission = with_notice(mission, model);
        let signal = panel("ROUTING FIELD")
            .number(3)
            .grow(1.3)
            .gap(0)
            .child(component(SignalInstrument(instrument), cx))
            .child(label("3 workers / synthetic").emphasis(Emphasis::Muted))
            .child(sparkline(&SIGNAL))
            .child(label("Signal samples / synthetic"));
        tree = if width >= 100 {
            tree.child(
                row()
                    .gap(1)
                    .grow(1.0)
                    .child(agents)
                    .child(mission)
                    .child(signal),
            )
        } else {
            tree.child(row().gap(1).grow(1.0).child(agents).child(mission))
                .child(signal)
        };
        if roomy {
            tree = tree.child(
                row()
                    .gap(1)
                    .child(
                        section("REVIEW NOTES / ARROWS SCROLL")
                            .number(4)
                            .grow(1.5)
                            .child(
                                viewport(0, model.code_y)
                                    .height(6)
                                    .key("review.code")
                                    .on_event(Action::Scroll)
                                    .child(code(REVIEW_CODE)),
                            ),
                    )
                    .child(
                        section("EVENT LEDGER")
                            .number(5)
                            .grow(1.0)
                            .child(label("00:00  Read the pipeline").tone(Tone::Info))
                            .child(label("00:01  Reproduced edge pop").tone(Tone::Warning))
                            .child(label("00:02  Drafted bounded fix").tone(Tone::Success))
                            .child(label("00:03  Waiting for approval").tone(Tone::Warning))
                            .child(label(&model.last_command).emphasis(Emphasis::Muted)),
                    ),
            );
        }
    }

    if narrow || height < 25 {
        // The command stays at the bottom of a compact viewport even when a
        // skin's sparse chrome uses fewer rows than a physical window grammar.
        tree = tree.child(spacer());
    }
    tree = tree.child(
        panel("COMMAND").number(6).gap(0).child(
            row()
                .gap(1)
                .child(
                    text_input(&model.editor)
                        .placeholder(if narrow {
                            "Instruction…"
                        } else {
                            "Ask the workspace / Unicode welcome…"
                        })
                        .grow(1.0)
                        .key("command")
                        .on_edit(Action::Edit)
                        .on_press(Action::Send),
                )
                .child(button("Send").key("send").on_press(Action::Send)),
        ),
    );
    if height >= 18 {
        tree = tree.child(label(if narrow {"Tab focus / Enter activate / ^C quit"}
            else {"Tab / Shift-Tab focus   Enter activate   Ctrl-C quit   Demo data; no commands execute"})
            .emphasis(Emphasis::Muted));
    }
    if model.modal {
        tree = tree.overlay(
            modal("REVIEW / PERMISSION")
                .number(7)
                .key("permission")
                .density(Density::Compact)
                .on_dismiss(Action::Close)
                .height(if tiny { 10 } else { 12 })
                .child(text(if tiny {
                    "Approve simulated patch?"
                } else {
                    "Approve this simulated change? No files will be written."
                }))
                .child(button("Approve").key("approve").on_press(Action::Approve))
                .child(
                    button("Reject")
                        .key("reject")
                        .tone(Tone::Danger)
                        .on_press(Action::Close),
                )
                .child(
                    button("Test error")
                        .key("error")
                        .tone(Tone::Warning)
                        .on_press(Action::Fail),
                ),
        );
    }
    tree
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Transition {
    Initial,
    Settled,
    Focus,
    Input,
    Activate,
    Modal,
    Success,
    Error,
    Toast,
}
impl Transition {
    const ALL: [Self; 9] = [
        Self::Initial,
        Self::Settled,
        Self::Focus,
        Self::Input,
        Self::Activate,
        Self::Modal,
        Self::Success,
        Self::Error,
        Self::Toast,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Settled => "settled",
            Self::Focus => "focus",
            Self::Input => "input",
            Self::Activate => "activate",
            Self::Modal => "modal",
            Self::Success => "success",
            Self::Error => "error",
            Self::Toast => "toast",
        }
    }
    fn parse(value: &str) -> io::Result<Self> {
        Self::ALL
            .into_iter()
            .find(|v| v.name() == value)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unknown transition; use initial|settled|focus|input|activate|modal|success|error|toast",
                )
            })
    }
    fn role(self) -> MotionRole {
        match self {
            Self::Initial | Self::Settled | Self::Toast => MotionRole::Enter,
            Self::Focus | Self::Input => MotionRole::Focus,
            Self::Activate => MotionRole::Activate,
            Self::Modal => MotionRole::ModalEnter,
            Self::Success => MotionRole::Success,
            Self::Error => MotionRole::Error,
        }
    }
}

fn trigger(kind: Transition, runtime: &mut UiRuntime<Action>, model: &mut Workspace) {
    match kind {
        Transition::Focus => {
            runtime.set_focus(&Key::named("command"));
        }
        Transition::Input => {
            model.editor = TextInputState::with_text("inspect the boundary 🦀");
            runtime.set_focus(&Key::named("command"));
        }
        Transition::Activate => {
            runtime.set_focus(&Key::named("run"));
            let event = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
            for action in runtime.handle_event(&event).actions {
                model.apply(action);
            }
        }
        Transition::Modal => model.apply(Action::Open),
        Transition::Success => model.success(),
        Transition::Error => model.apply(Action::Fail),
        // This fixture mounts a notification explicitly. The model removes it
        // when the next command is sent; no runtime-owned timer is implied.
        Transition::Toast => model.notification = Some("Build complete / changes ready".into()),
        _ => {}
    }
}

fn count_nodes(node: &Node) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

struct Metrics {
    semantic: usize,
    nodes: usize,
    bytes: u64,
    changed: usize,
    affected: u64,
    motions: usize,
    retained: usize,
    view_us: u128,
    lower_us: u128,
    render_us: u128,
    canvas_builds: usize,
}

struct Capture {
    parser: vt100::Parser,
    consumed: usize,
}
impl Capture {
    fn new(width: u16, height: u16) -> Self {
        Self {
            parser: vt100::Parser::new(height, width, 0),
            consumed: 0,
        }
    }
    fn read(&mut self, context: &Context) -> usize {
        let before = self.parser.screen().clone();
        let bytes = context.rendered_bytes();
        self.parser.process(&bytes[self.consumed..]);
        self.consumed = bytes.len();
        let (rows, cols) = self.parser.screen().size();
        let mut changed = 0;
        for y in 0..rows {
            for x in 0..cols {
                if before.cell(y, x) != self.parser.screen().cell(y, x) {
                    changed += 1;
                }
            }
        }
        changed
    }
}

fn measured_frame(
    runtime: &mut UiRuntime<Action>,
    model: &Workspace,
    cache: &mut CanvasCache,
    context: &mut Context,
    environment: UiEnvironment,
    time: Duration,
    capture: &mut Capture,
) -> io::Result<Metrics> {
    let start = Instant::now();
    let cx = runtime.build_cx(environment, time);
    let tree = view(model, &cx, cache);
    let view_us = start.elapsed().as_micros();
    let start = Instant::now();
    let frame = runtime
        .frame(&tree, environment, time)
        .map_err(io::Error::other)?;
    let lower_us = start.elapsed().as_micros();
    let semantic = frame.keys.len();
    let nodes = count_nodes(&frame.node);
    let before = context.stats();
    context.set_root(frame.node);
    let start = Instant::now();
    context.render_now()?;
    let render_us = start.elapsed().as_micros();
    let after = context.stats();
    Ok(Metrics {
        semantic,
        nodes,
        bytes: after.frame_bytes - before.frame_bytes,
        changed: capture.read(context),
        affected: after.dirty_cells - before.dirty_cells,
        motions: runtime.active_animation_count(),
        retained: runtime.retained_key_count(),
        view_us,
        lower_us,
        render_us,
        canvas_builds: cache.builds,
    })
}

fn profile(options: &Options) -> io::Result<()> {
    println!("skin\twidth\theight\tdepth\tmotion\tscenario\telapsed_ms\tsemantic_elements\tordinary_nodes\tframe_bytes\texact_displayed_cells\taffected_cells\tactive_motions\tretained_keys\tview_us\tlower_us\trender_us\tcanvas_builds");
    for scenario in Transition::ALL {
        let mut context = options.context()?;
        let environment = options.environment(&context);
        let mut runtime = UiRuntime::new(options.skin);
        let mut model = Workspace::default();
        let mut cache = CanvasCache::default();
        let mut capture = Capture::new(options.width, options.height);
        let baseline = if scenario == Transition::Initial {
            Duration::ZERO
        } else {
            Duration::from_secs(1)
        };
        if scenario != Transition::Initial {
            for now in [Duration::ZERO, baseline] {
                measured_frame(
                    &mut runtime,
                    &model,
                    &mut cache,
                    &mut context,
                    environment,
                    now,
                    &mut capture,
                )?;
            }
        }
        trigger(scenario, &mut runtime, &mut model);
        let resolved = options.skin.resolve(&environment);
        let duration = if scenario == Transition::Initial {
            resolved
                .motion(scenario.role())
                .duration
                .max(resolved.motion(MotionRole::Success).duration)
                .max(resolved.motion(MotionRole::Focus).duration)
        } else if scenario == Transition::Modal {
            resolved
                .motion(scenario.role())
                .duration
                .max(resolved.motion(MotionRole::Enter).duration)
                .max(resolved.motion(MotionRole::Focus).duration)
        } else {
            resolved.motion(scenario.role()).duration
        }
        .as_millis() as u64;
        let samples = if scenario == Transition::Settled {
            vec![0, 16, 32]
        } else {
            vec![
                0,
                duration / 4,
                duration / 2,
                duration * 3 / 4,
                duration,
                duration + 100,
            ]
        };
        for elapsed in samples {
            let m = measured_frame(
                &mut runtime,
                &model,
                &mut cache,
                &mut context,
                environment,
                baseline + Duration::from_millis(elapsed),
                &mut capture,
            )?;
            println!(
                "{}\t{}\t{}\t{:?}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                options.skin.name,
                options.width,
                options.height,
                environment.color_depth,
                environment.motion,
                scenario.name(),
                elapsed,
                m.semantic,
                m.nodes,
                m.bytes,
                m.changed,
                m.affected,
                m.motions,
                m.retained,
                m.view_us,
                m.lower_us,
                m.render_us,
                m.canvas_builds
            );
        }
        context.restore()?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let mut options = Options::read();
    if options.has("--profile") {
        options.dump = true;
        return profile(&options);
    }
    let mut context = options.context()?;
    let mut runtime = UiRuntime::new(options.skin);
    let mut model = Workspace::default();
    let mut cache = CanvasCache::default();
    if options.dump {
        let kind = Transition::parse(options.value("--transition=").unwrap_or("settled"))?;
        let elapsed = options
            .value("--at-ms=")
            .map(|s| s.parse::<u64>())
            .transpose()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--at-ms must be an unsigned integer",
                )
            })?
            .unwrap_or(1000);
        let environment = options.environment(&context);
        let baseline = if kind == Transition::Initial {
            Duration::ZERO
        } else {
            Duration::from_secs(1)
        };
        if kind != Transition::Initial {
            for now in [Duration::ZERO, baseline] {
                let tree = view(&model, &runtime.build_cx(environment, now), &mut cache);
                present(&mut runtime, &tree, &mut context, environment, now)?;
            }
        }
        trigger(kind, &mut runtime, &mut model);
        // Establish the transition start even when the requested capture samples
        // its middle or end. Skipping this frame would restart it at capture time.
        for now in [baseline, baseline + Duration::from_millis(elapsed)] {
            let tree = view(&model, &runtime.build_cx(environment, now), &mut cache);
            present(&mut runtime, &tree, &mut context, environment, now)?;
        }
        options.print_capture(&context);
        context.restore()?;
        return Ok(());
    }
    let started = Instant::now();
    loop {
        let now = started.elapsed();
        let environment = options.environment(&context);
        let tree = view(&model, &runtime.build_cx(environment, now), &mut cache);
        present(&mut runtime, &tree, &mut context, environment, now)?;
        if let Some(event) = context.run_once(Duration::from_millis(33))? {
            if matches!(&event,Event::Key(k) if k.code==KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
            {
                break;
            }
            for action in runtime.handle_event(&event).actions {
                if matches!(action, Action::Skin) {
                    let skin = if runtime.skin().name == skins::BLACK_ICE.name {
                        skins::VAPOR95
                    } else if runtime.skin().name == skins::VAPOR95.name {
                        skins::SWISS_SIGNAL
                    } else {
                        skins::BLACK_ICE
                    };
                    runtime.set_skin(skin);
                } else {
                    model.apply(action);
                }
            }
        }
    }
    context.restore()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gibson::{ColorDepth, RenderMode, SubcellGlyphMode};

    const SIZES: [(u16, u16); 8] = [
        (160, 50),
        (120, 40),
        (100, 30),
        (80, 24),
        (60, 20),
        (48, 18),
        (36, 18),
        (32, 16),
    ];

    #[test]
    fn focused_editor_and_primary_controls_survive_all_requested_sizes() {
        for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
            for color_depth in [
                ColorDepth::TrueColor,
                ColorDepth::Ansi256,
                ColorDepth::Ansi16,
                ColorDepth::Mono,
            ] {
                for (width, height) in SIZES {
                    let environment = UiEnvironment {
                        width,
                        height: height - 1,
                        color_depth,
                        motion: MotionPreference::None,
                        ..UiEnvironment::default()
                    };
                    let mut runtime = UiRuntime::new(skin);
                    let mut cache = CanvasCache::default();
                    let model = Workspace {
                        editor: TextInputState::with_text("draft 🦀"),
                        ..Workspace::default()
                    };
                    let mut context = Context::headless(RenderMode::Inline, width, height);
                    context.set_color_depth(color_depth);
                    let mut capture = Capture::new(width, height);
                    measured_frame(
                        &mut runtime,
                        &model,
                        &mut cache,
                        &mut context,
                        environment,
                        Duration::ZERO,
                        &mut capture,
                    )
                    .unwrap();
                    assert!(runtime.set_focus(&Key::named("command")));
                    measured_frame(
                        &mut runtime,
                        &model,
                        &mut cache,
                        &mut context,
                        environment,
                        Duration::from_secs(1),
                        &mut capture,
                    )
                    .unwrap();
                    let visible = capture.parser.screen().contents();
                    for control in ["Run", "Review", "Skin", "Send", "draft 🦀"] {
                        assert!(
                            visible.contains(control),
                            "{} {width}x{height} {color_depth:?} missing {control}:\n{visible}",
                            skin.name
                        );
                    }
                    assert_eq!(runtime.focus(), Some(&Key::named("command")));
                    assert!(
                        !capture.parser.screen().hide_cursor(),
                        "focused editor cursor hidden"
                    );
                }
            }
        }
    }

    #[test]
    fn notification_keeps_editor_and_primary_controls_visible() {
        for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
            for (width, height) in [(80, 24), (36, 18), (32, 16)] {
                let environment = UiEnvironment {
                    width,
                    height: height - 1,
                    motion: MotionPreference::None,
                    ..UiEnvironment::default()
                };
                let model = Workspace {
                    editor: TextInputState::with_text("draft 🦀"),
                    notification: Some("Build complete / changes ready".into()),
                    ..Workspace::default()
                };
                let mut runtime = UiRuntime::new(skin);
                let mut context = Context::headless(RenderMode::Inline, width, height);
                let mut capture = Capture::new(width, height);
                let mut cache = CanvasCache::default();
                measured_frame(
                    &mut runtime,
                    &model,
                    &mut cache,
                    &mut context,
                    environment,
                    Duration::ZERO,
                    &mut capture,
                )
                .unwrap();
                assert!(runtime.set_focus(&Key::named("command")));
                measured_frame(
                    &mut runtime,
                    &model,
                    &mut cache,
                    &mut context,
                    environment,
                    Duration::from_secs(1),
                    &mut capture,
                )
                .unwrap();
                let visible = capture.parser.screen().contents();
                for value in [
                    "Run",
                    "Review",
                    "Skin",
                    "Send",
                    "draft 🦀",
                    "Build complete",
                    "READY FOR REVIEW",
                ] {
                    assert!(
                        visible.contains(value),
                        "{} {width}x{height} notification covers {value}: {visible}",
                        skin.name
                    );
                }
                assert!(!capture.parser.screen().hide_cursor());
            }
        }
    }

    #[test]
    fn success_and_error_agree_across_status_table_and_compact_summary() {
        for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
            for (width, height) in [(32, 16), (80, 24), (120, 40)] {
                for failed in [false, true] {
                    let environment = UiEnvironment {
                        width,
                        height: height - 1,
                        motion: MotionPreference::None,
                        ..UiEnvironment::default()
                    };
                    let mut model = Workspace::default();
                    if failed {
                        model.apply(Action::Fail);
                    } else {
                        model.success();
                    }
                    let mut context = Context::headless(RenderMode::Inline, width, height);
                    let mut capture = Capture::new(width, height);
                    measured_frame(
                        &mut UiRuntime::new(skin),
                        &model,
                        &mut CanvasCache::default(),
                        &mut context,
                        environment,
                        Duration::ZERO,
                        &mut capture,
                    )
                    .unwrap();
                    let visible = capture.parser.screen().contents().to_lowercase();
                    assert!(
                        visible.contains(if failed {
                            "check failed"
                        } else {
                            "patch approved"
                        }),
                        "{} {width}x{height} failed={failed}: {visible}",
                        skin.name
                    );
                    assert!(
                        !visible.contains("awaiting review"),
                        "stale patch result: {visible}"
                    );
                    assert!(
                        !visible.contains("ready for review"),
                        "stale mission status: {visible}"
                    );
                    if failed {
                        assert!(!visible.contains("patch approved"));
                    }
                }
            }
        }
    }

    #[test]
    fn custom_surface_cache_reuses_and_invalidates_actual_inputs() {
        let mut cache = CanvasCache::default();
        let mut environment = UiEnvironment::default();
        let cx = BuildCx::new(skins::BLACK_ICE, environment);
        let first = cache.get(&cx, 28, 5);
        let same = cache.get(&cx, 28, 5);
        assert!(Arc::ptr_eq(&first, &same));
        assert_eq!(cache.builds, 1);
        let resized = cache.get(&cx, 30, 5);
        assert!(!Arc::ptr_eq(&first, &resized));
        environment.glyph_mode = SubcellGlyphMode::Ascii;
        let realized = cache.get(&BuildCx::new(skins::BLACK_ICE, environment), 30, 5);
        assert!(!Arc::ptr_eq(&resized, &realized));
        assert_eq!(cache.builds, 3);
        assert!(cache
            .entry
            .as_ref()
            .is_some_and(|(_, surface)| Arc::ptr_eq(surface, &realized)));
    }

    fn transition_capture(
        skin: Skin,
        preference: MotionPreference,
        kind: Transition,
        elapsed: u64,
    ) -> (Vec<vt100::Cell>, usize) {
        let environment = UiEnvironment {
            width: 80,
            height: 23,
            motion: preference,
            ..UiEnvironment::default()
        };
        let mut context = Context::headless(RenderMode::Inline, 80, 24);
        let mut capture = Capture::new(80, 24);
        let mut runtime = UiRuntime::new(skin);
        let mut model = Workspace::default();
        let mut cache = CanvasCache::default();
        for now in [0, 1000] {
            measured_frame(
                &mut runtime,
                &model,
                &mut cache,
                &mut context,
                environment,
                Duration::from_millis(now),
                &mut capture,
            )
            .unwrap();
        }
        trigger(kind, &mut runtime, &mut model);
        for now in [1000, 1000 + elapsed] {
            measured_frame(
                &mut runtime,
                &model,
                &mut cache,
                &mut context,
                environment,
                Duration::from_millis(now),
                &mut capture,
            )
            .unwrap();
        }
        let cells = (0..24)
            .flat_map(|y| (0..80).map(move |x| (y, x)))
            .map(|(y, x)| capture.parser.screen().cell(y, x).unwrap().clone())
            .collect();
        (cells, runtime.active_animation_count())
    }

    #[test]
    fn transition_capture_is_deterministic_and_settles_to_no_motion() {
        for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
            for kind in [
                Transition::Focus,
                Transition::Input,
                Transition::Activate,
                Transition::Modal,
                Transition::Success,
                Transition::Error,
                Transition::Toast,
            ] {
                assert_eq!(
                    transition_capture(skin, MotionPreference::Full, kind, 60),
                    transition_capture(skin, MotionPreference::Full, kind, 60)
                );
                let full = transition_capture(skin, MotionPreference::Full, kind, 1000);
                assert_eq!(
                    full.1, 0,
                    "{} {kind:?} retained motion after deadline",
                    skin.name
                );
                let none = transition_capture(skin, MotionPreference::None, kind, 1000);
                let difference = full.0.iter().zip(&none.0).position(|(a, b)| a != b);
                assert!(
                    difference.is_none(),
                    "{} {kind:?} settled cell differs at {:?}",
                    skin.name,
                    difference.map(|i| (i % 80, i / 80))
                );
            }
        }
    }
}
