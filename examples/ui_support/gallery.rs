//! One semantic tree, all three design grammars. No raw colors.

use super::options::{present, Options};
use gibson::ui::prelude::*;
use gibson::{BrailleCanvas, Event, KeyCode, KeyModifiers, SurfaceFx, TextInputState};
use std::time::{Duration, Instant};

#[derive(Clone)]
enum Action {
    Tab(usize),
    Skin,
    Open,
    Close,
    Choose(&'static str),
    Edit(TextInputState),
    Submit,
}

struct Model {
    tab: usize,
    selected: &'static str,
    input: TextInputState,
    modal: bool,
    notification: Option<(String, Duration)>,
}

/// A phase portrait is deliberately outside the built-in widget vocabulary.
/// The custom component stays local: a regular BrailleCanvas produces a Surface,
/// followed by an ordinary scoped SurfaceFx. Layout and the surrounding controls
/// are still semantic UI components.
struct PhasePortrait;
impl<A> Component<A> for PhasePortrait {
    fn build(self, cx: &BuildCx) -> Element<A> {
        let width = cx.environment.width.saturating_sub(12).clamp(8, 48);
        let mut canvas = BrailleCanvas::new(width, if cx.environment.height < 24 { 3 } else { 5 });
        let (w, h) = (
            f32::from(canvas.pixel_width()),
            f32::from(canvas.pixel_height()),
        );
        let points: Vec<_> = (0..=240)
            .map(|i| {
                let t = i as f32 * std::f32::consts::TAU / 240.0;
                (
                    (w * (0.5 + 0.46 * (3.0 * t).sin())) as i32,
                    (h * (0.5 + 0.42 * (2.0 * t).sin())) as i32,
                )
            })
            .collect();
        canvas.polyline(&points);
        raster(canvas.to_surface_mode(
            cx.skin.style(Tone::Accent, Emphasis::Normal),
            cx.environment.glyph_mode,
        ))
        .post_process([SurfaceFx::Scoped {
            mask: gibson::surface_fx::FxMask::Rect(gibson::Rect::new(0, 2, width, 1)),
            effect: Box::new(SurfaceFx::Reverse),
        }])
    }
}

fn view(model: &Model, cx: &BuildCx) -> Element<Action> {
    let narrow = cx.environment.width < 60;
    let mut tree = screen()
        .gap(u16::from(!narrow))
        .density(if cx.environment.height < 30 {
            Density::Compact
        } else {
            Density::Normal
        })
        .child(
            row()
                .child(
                    heading(if narrow {
                        "OPERATIONS"
                    } else {
                        "NEURAL OPERATIONS"
                    })
                    .grow(1.0),
                )
                .child(
                    button(if narrow { "Skin" } else { "Switch skin" })
                        .key("skin")
                        .on_press(Action::Skin),
                ),
        )
        .child(
            row().gap(1).children(
                if narrow {
                    ["View", "Controls", "Canvas"]
                } else {
                    ["Overview", "Components", "Custom canvas"]
                }
                .into_iter()
                .enumerate()
                .map(|(i, title)| {
                    choice(title, model.tab == i)
                        .key(format!("tab.{i}"))
                        .on_press(Action::Tab(i))
                }),
            ),
        );
    let content = match model.tab {
        1 if narrow => panel("SEMANTIC CONTROLS")
            .child(status("Build complete").tone(Tone::Success))
            .child(status("Review requested").tone(Tone::Warning))
            .child(status("Offline worker").tone(Tone::Danger))
            .child(
                list().children(["Scout", "Builder", "Reviewer"].into_iter().map(|name| {
                    choice(name, model.selected == name)
                        .key(format!("agent.{name}"))
                        .on_press(Action::Choose(name))
                })),
            ),
        1 => row()
            .gap(1)
            .child(
                card("SEMANTIC CONTROLS")
                    .grow(1.0)
                    .child(
                        status("Build complete")
                            .tone(Tone::Success)
                            .emphasis(Emphasis::Strong),
                    )
                    .child(status("Review requested").tone(Tone::Warning))
                    .child(status("Offline worker").tone(Tone::Danger))
                    .child(
                        row()
                            .gap(1)
                            .child(badge("EXPERIMENTAL").tone(Tone::Info))
                            .child(badge("RUST ONLY").emphasis(Emphasis::Muted)),
                    )
                    .child(button("Unavailable action").disabled(true)),
            )
            .child(
                section("CONTROLLED SELECTION")
                    .grow(1.0)
                    .child(
                        list().children(["Scout", "Builder", "Reviewer"].into_iter().map(|name| {
                            choice(name, model.selected == name)
                                .key(format!("agent.{name}"))
                                .on_press(Action::Choose(name))
                        })),
                    )
                    .child(divider())
                    .child(code("selected = model.agent_id"))
                    .child(
                        text("Selection belongs to the app. Focus belongs to the UI.")
                            .emphasis(Emphasis::Muted),
                    ),
            ),
        2 if narrow => panel("LOCAL ESCAPE HATCH")
            .child(label("Canvas / 3:2 phase portrait"))
            .child(component(PhasePortrait, cx))
            .child(label("Surface + local SurfaceFx")),
        2 => panel("LOCAL ESCAPE HATCH")
            .child(text(
                "A 3:2 orbital phase portrait, drawn with BrailleCanvas.",
            ))
            .child(component(PhasePortrait, cx))
            .child(code("Component -> Surface -> ordinary Node -> framebuffer"))
            .child(
                text("The highlighted band is local SurfaceFx; no terminal shortcuts.")
                    .emphasis(Emphasis::Muted),
            ),
        _ if narrow => panel("LIVE OPERATIONS")
            .child(status("3 workers / context 62%").tone(Tone::Success))
            .child(table(
                ["ROLE", "TASK"],
                [["Scout", "Read pipeline"], ["Builder", "Patch clipping"]],
            ))
            .child(
                button("Command menu")
                    .key("open-menu")
                    .on_press(Action::Open),
            ),
        _ => row()
            .gap(1)
            .child(
                panel("AGENTS")
                    .grow(1.0)
                    .child(status("CONNECTED / 3 workers").tone(Tone::Success))
                    .child(table(
                        ["ROLE", "TASK"],
                        [
                            ["Scout", "Read pipeline"],
                            ["Builder", "Patch clipping"],
                            ["Reviewer", "Check invariants"],
                        ],
                    ))
                    .child(
                        button("Command menu")
                            .key("open-menu")
                            .on_press(Action::Open),
                    ),
            )
            .child(
                panel("THROUGHPUT")
                    .grow(1.0)
                    .child(label("SYNTHETIC FRAME BYTES"))
                    .child(sparkline(&[
                        4., 7., 3., 9., 5., 12., 8., 13., 7., 10., 6., 4.,
                    ]))
                    .child(progress("context", 0.62))
                    .child(progress("review", 0.88))
                    .child(status("All systems nominal").tone(Tone::Info)),
            ),
    };
    tree = tree
        .child(content)
        .child(
            panel("COMMAND").child(
                row()
                    .gap(1)
                    .child(
                        text_input(&model.input)
                            .placeholder("Type Unicode here…")
                            .key("command")
                            .grow(1.0)
                            .on_edit(Action::Edit)
                            .on_press(Action::Submit),
                    )
                    .child(button("Send").key("send").on_press(Action::Submit)),
            ),
        )
        .child(label("Tab focus • Enter activate • Ctrl-C quit").emphasis(Emphasis::Muted));
    if model.modal {
        tree = tree.overlay(
            modal("COMMAND MENU")
                .key("menu")
                .on_dismiss(Action::Close)
                .child(text("Choose a worker. Escape closes and restores focus."))
                .children(["Scout", "Builder", "Reviewer"].into_iter().map(|name| {
                    button(name)
                        .key(format!("menu.{name}"))
                        .on_press(Action::Choose(name))
                })),
        );
    }
    if let Some((message, _)) = &model.notification {
        tree = tree.overlay(toast(message.clone()).key("notification"));
    }
    tree
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::read();
    let mut context = options.context()?;
    let mut runtime = UiRuntime::new(options.skin);
    let mut skin = options.skin;
    let mut model = Model {
        tab: options
            .value("--page=")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        selected: "Scout",
        input: TextInputState::new(),
        modal: options.has("--modal"),
        notification: None,
    };
    let started = Instant::now();
    loop {
        let now = if options.dump {
            Duration::from_secs(1)
        } else {
            started.elapsed()
        };
        if model
            .notification
            .as_ref()
            .is_some_and(|(_, expiry)| *expiry <= now)
        {
            model.notification = None;
        }
        let environment = options.environment(&context);
        let cx = BuildCx::new(skin, environment);
        let tree = view(&model, &cx);
        // Prime and settle the deterministic capture without changing the tree.
        if options.dump {
            present(
                &mut runtime,
                &tree,
                &mut context,
                environment,
                Duration::ZERO,
            )?;
        }
        present(&mut runtime, &tree, &mut context, environment, now)?;
        if options.dump {
            options.print_capture(&context);
            break;
        }
        if let Some(event) = context.run_once(Duration::from_millis(33))? {
            if matches!(&event, Event::Key(k) if k.code == KeyCode::Char('c')
                && k.modifiers.contains(KeyModifiers::CONTROL))
            {
                break;
            }
            for action in runtime.handle_event(&event).actions {
                match action {
                    Action::Tab(tab) => model.tab = tab,
                    Action::Open => model.modal = true,
                    Action::Close => model.modal = false,
                    Action::Choose(name) => {
                        model.selected = name;
                        model.modal = false;
                        model.notification =
                            Some((format!("Selected {name}"), now + Duration::from_secs(3)));
                    }
                    Action::Edit(input) => model.input = input,
                    Action::Submit => {
                        if !model.input.text.is_empty() {
                            context.insert_text_before_live(&format!("> {}", model.input.text))?;
                            model.input = TextInputState::new();
                            model.notification = Some((
                                "Command recorded in scrollback".into(),
                                now + Duration::from_secs(3),
                            ));
                        }
                    }
                    Action::Skin => {
                        skin = if skin.name == skins::BLACK_ICE.name {
                            skins::VAPOR95
                        } else if skin.name == skins::VAPOR95.name {
                            skins::SWISS_SIGNAL
                        } else {
                            skins::BLACK_ICE
                        };
                        runtime.set_skin(skin);
                    }
                }
            }
        }
    }
    context.restore()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gibson::{ColorDepth, Context, RenderMode};

    #[test]
    fn every_gallery_page_retains_primary_controls_across_sizes_and_skins() {
        // These are visible-screen assertions through the real headless renderer,
        // not assertions about labels that may exist below the clipped viewport.
        for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
            for (width, height) in [(80, 24), (120, 40), (36, 18)] {
                for tab in 0..3 {
                    let environment = UiEnvironment {
                        width,
                        height: height - 1,
                        color_depth: ColorDepth::Mono,
                        motion: MotionPreference::None,
                        ..UiEnvironment::default()
                    };
                    let model = Model {
                        tab,
                        selected: "Scout",
                        input: TextInputState::new(),
                        modal: false,
                        notification: None,
                    };
                    let tree = view(&model, &BuildCx::new(skin, environment));
                    let mut context = Context::headless(RenderMode::Inline, width, height);
                    context.set_color_depth(ColorDepth::Mono);
                    present(
                        &mut UiRuntime::new(skin),
                        &tree,
                        &mut context,
                        environment,
                        Duration::ZERO,
                    )
                    .unwrap();
                    let mut parser = vt100::Parser::new(height, width, 0);
                    parser.process(context.rendered_bytes());
                    let visible = parser.screen().contents();
                    assert!(
                        visible.contains("Send"),
                        "{} {width}x{height} tab {tab}: {visible}",
                        skin.name
                    );
                    assert!(visible.contains("Skin") || visible.contains("Switch skin"));
                }
            }
        }
    }
}
