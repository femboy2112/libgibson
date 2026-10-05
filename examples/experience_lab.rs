//! `experience_lab` — the Experience-Grammar flagship (milestone §24).
//!
//! **One** application, described once as a semantic [`Experience`], rendered live
//! through **six** radically different interface grammars — a boring reference
//! list, a cover shelf, a cross-media bar, a typographic panorama, an orbital
//! focal field and an occluding blade stack — switchable on a keystroke, with the
//! selection surviving every switch. There is deliberately **no** `match style`
//! anywhere in the application's `view` or `update`: the app speaks semantics, the
//! grammar is swapped underneath it, and that is the whole point.
//!
//! ## Controls
//!
//! - `s` — next grammar; `1`..`6` — jump straight to a grammar
//! - arrows (and `h`/`j`/`k`/`l` where a grammar binds them) — navigate
//! - `Enter` — activate the focused item's primary action
//! - `Esc` / `Backspace` — back
//! - `q` / `Ctrl-C` — quit
//!
//! ## Headless
//!
//! Run without a TTY (or under `cargo run --example experience_lab -- frames DIR`)
//! and it renders deterministic frames instead of taking the terminal. The
//! `#[test]`s below are the CI contract: they prove — headlessly — that the
//! selection and active destination survive a live switch through every grammar,
//! and that every grammar's rendered frame honours the preservation law.

use gibson::capability::ColorDepth;
use gibson::context::{Context, RenderMode};
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::ui::experience::{
    Action, Content, Destination, Experience, ExperienceRuntime, ExperienceStyle, Facet, Item,
    Media,
};
use gibson::ui::prelude::UiRuntime;
use gibson::ui::skin::UiEnvironment;
use gibson::ui::{
    column, label, row, screen, skins, status, App, AppEvent, BuildCx, Control, Element,
};
use std::cell::RefCell;
use std::io;
use std::time::Duration;

/// The one application action type. Every grammar hands exactly these back; none
/// of them knows what the actions *mean* — that is the application's business.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Play(String),
    Toggle(String),
    Open(String),
}

/// The single semantic description of the application. Authored once; never forked
/// per grammar. A media library (collection), a now-playing inspector (detail), a
/// settings sheet (collection of value-cycling items) and an about page (prose) —
/// exercising every [`Content`] kind.
fn build_experience() -> Experience<Msg> {
    let library = Content::Collection(vec![
        Item::new("neon-harbour", "Neon Harbour")
            .subtitle("Vapor Cartography")
            .media(Media::new(1))
            .action(Action::new(
                "play-0",
                "Play",
                Msg::Play("neon-harbour".into()),
            )),
        Item::new("glass-arcades", "Glass Arcades")
            .subtitle("Mono Lake")
            .media(Media::new(2))
            .action(Action::new(
                "play-1",
                "Play",
                Msg::Play("glass-arcades".into()),
            )),
        Item::new("tidal-automata", "Tidal Automata")
            .subtitle("Subaqueous")
            .media(Media::new(3))
            .action(Action::new(
                "play-2",
                "Play",
                Msg::Play("tidal-automata".into()),
            )),
        Item::new("low-orbit-choir", "Low Orbit Choir")
            .subtitle("Ascent")
            .media(Media::new(4))
            .action(Action::new(
                "play-3",
                "Play",
                Msg::Play("low-orbit-choir".into()),
            )),
        Item::new("hidden-track", "Hidden Track")
            .subtitle("—")
            .media(Media::new(5))
            .action(Action::new(
                "play-4",
                "Play",
                Msg::Play("hidden-track".into()),
            )),
    ]);

    let now_playing = Content::Detail {
        facets: vec![
            Facet::new("track", "Track", "Neon Harbour"),
            Facet::new("artist", "Artist", "Vapor Cartography"),
            Facet::new("time", "Elapsed", "01:12 / 03:48"),
        ],
        actions: vec![Action::new("open-np", "Open", Msg::Open("now".into()))],
    };

    let settings = Content::Collection(vec![
        Item::new("set-color", "Colour depth")
            .subtitle("TrueColor")
            .action(Action::new(
                "cyc-color",
                "Cycle",
                Msg::Toggle("color".into()),
            )),
        Item::new("set-motion", "Motion")
            .subtitle("Full")
            .action(Action::new(
                "cyc-motion",
                "Cycle",
                Msg::Toggle("motion".into()),
            )),
    ]);

    let about = Content::Prose(vec![
        "Gibson Library — one semantic experience, many grammars.".into(),
        "The application describes itself once; each grammar is a change of".into(),
        "representation, never a fork of application state.".into(),
    ]);

    Experience::new("GIBSON LIBRARY")
        .destination(Destination::new("library", "Library", library))
        .destination(Destination::new("now", "Now Playing", now_playing))
        .destination(Destination::new("settings", "Settings", settings))
        .destination(Destination::new("about", "About", about))
}

/// The application model. Note what is *not* here: no per-style branch, no
/// per-style layout, and no hand-rolled grammar registry. Just the semantic value
/// and an [`ExperienceRuntime`] that owns the presentation orchestration (the
/// grammar-independent navigation state and the persistent grammar instances).
///
/// The runtime is behind a `RefCell` only because this App harness hands `view` an
/// immutable `&Lab` while a grammar's `present` needs `&mut` for its spring state —
/// one cell around the whole runtime, not the old `Vec<RefCell<Box<dyn Grammar>>>`.
struct Lab {
    experience: Experience<Msg>,
    ui: RefCell<ExperienceRuntime<Msg>>,
    last_action: Option<Msg>,
    quit: bool,
}

impl Lab {
    fn new() -> Self {
        let experience = build_experience();
        let ui = ExperienceRuntime::with_builtins(&experience);
        Self {
            experience,
            ui: RefCell::new(ui),
            last_action: None,
            quit: false,
        }
    }
}

/// The one and only view. It picks the active grammar, presents the shared
/// semantic value through it, and overlays a thin lab status line. There is no
/// `match` on the style here — the grammar is a value, swapped underneath.
fn view(lab: &Lab, cx: &BuildCx) -> Element<Msg> {
    // One call: present the shared semantic value through the active grammar. The
    // grammar is a value the runtime swaps underneath — there is no `match style`.
    let presented = lab
        .ui
        .borrow_mut()
        .present(&lab.experience, &cx.environment, cx.time);

    let (style_name, count) = {
        let ui = lab.ui.borrow();
        (ui.style_name().to_string(), ui.style_count())
    };
    let last = lab
        .last_action
        .as_ref()
        .map(|m| format!("{m:?}"))
        .unwrap_or_else(|| "—".into());
    let bar = row::<Msg>()
        .gap(2)
        .child(status::<Msg>(format!(
            "EXPERIENCE LAB  {style_name}  ({count} styles)"
        )))
        .child(label::<Msg>(
            "s style · 1-6 pick · arrows nav · ↵ act · Esc back · q quit",
        ))
        .child(label::<Msg>(format!("last: {last}")));

    screen::<Msg>().child(
        column::<Msg>()
            .gap(0)
            .child(bar)
            .child(presented.element.grow(1.0)),
    )
}

/// The one and only update. Lab-global keys (style switch / quit) are handled
/// here; every other key is routed, unmodified, through the active grammar's own
/// interaction metaphor via `handle_key`. The returned `Msg` is the application's
/// to interpret — here we just record it for the status line.
fn update(lab: &mut Lab, event: AppEvent<Msg>) -> Control {
    match event {
        AppEvent::Action(msg) => {
            lab.last_action = Some(msg);
            Control::Continue
        }
        AppEvent::Input(Event::Key(key)) => {
            // Quit.
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
                || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                lab.quit = true;
                return Control::Quit;
            }
            // Live style switch — the heart of the demo. The semantic state is
            // untouched, so the selection survives the switch.
            match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    lab.ui.get_mut().next_style();
                    return Control::Continue;
                }
                KeyCode::Char(d @ '1'..='6') => {
                    let style = ExperienceStyle::ALL[d as usize - '1' as usize];
                    lab.ui.get_mut().set_style(style);
                    return Control::Continue;
                }
                _ => {}
            }
            // Everything else: the active grammar decides what the key means, and
            // the runtime applies it to the shared state identically for every
            // grammar. The returned `Msg` is the application's to interpret.
            if let Some(msg) = lab.ui.get_mut().handle_key(&lab.experience, &key) {
                lab.last_action = Some(msg);
            }
            Control::Continue
        }
        AppEvent::Input(_) | AppEvent::Tick(_) => Control::Continue,
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Headless text snapshot for visual acceptance (§43) and quick eyeballing:
    // `experience_lab dump [W H]` prints every grammar's frame as visible text at
    // a fixed state, deterministically. (A Surface is terminal cells, not pixels,
    // so this is the faithful headless capture; the live demo carries the colour.)
    if args.get(1).map(String::as_str) == Some("dump") {
        let w: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
        let h: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(28);
        let lab = Lab::new();
        let env = UiEnvironment {
            width: w,
            height: h,
            color_depth: ColorDepth::TrueColor,
            ..UiEnvironment::default()
        };
        for style in ExperienceStyle::ALL {
            lab.ui.borrow_mut().set_style(style);
            let name = style.name();
            let presented =
                lab.ui
                    .borrow_mut()
                    .present(&lab.experience, &env, Duration::from_millis(500));
            let cx = BuildCx::new(skins::VAPOR95, env);
            let mut node = gibson::ui::compile(&presented.element, &cx)
                .expect("grammar lowers")
                .node;
            gibson::compute_layout(&mut node, w, h).expect("layout");
            let mut surface = gibson::Surface::new(w, h);
            gibson::paint(&node, &mut surface);
            println!("\n===== {name} {w}x{h} =====");
            for line in surface.to_visible_lines() {
                println!("{line}");
            }
        }
        return Ok(());
    }

    // Truecolor capture for real visual acceptance (§43): `experience_lab ansi <1-6>
    // [W H]` renders ONE grammar's full lab frame — grammar *and* chrome, exactly as
    // the live demo paints it — and writes the settled screen as ANSI to stdout. A
    // Surface flattens to colourless text; this path keeps the colour and the raster
    // half-block pixels, so an external renderer (docs/assets/render_ui_skins.py) can
    // turn it into an honest PNG instead of the lossy text dump.
    if args.get(1).map(String::as_str) == Some("ansi") {
        let idx = args
            .get(2)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let w: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100);
        let h: u16 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(28);
        let lab = Lab::new();
        let style = ExperienceStyle::ALL
            .get(idx)
            .copied()
            .unwrap_or(ExperienceStyle::Standard);
        lab.ui.borrow_mut().set_style(style);
        let env = UiEnvironment {
            width: w,
            height: h,
            color_depth: ColorDepth::TrueColor,
            ..UiEnvironment::default()
        };
        // A settled clock: springy grammars have parked, so this is the still frame.
        let now = Duration::from_secs(5);
        let mut cx = BuildCx::new(skins::VAPOR95, env);
        cx.time = now;
        // `App::fullscreen` forces the root to the viewport; mirror that here so the
        // capture shows the real live framing (the skin's screen fills), not a
        // content-height clump floating over a black void.
        let element = view(&lab, &cx).height(h);

        let mut runtime = UiRuntime::new(skins::VAPOR95);
        let compiled = runtime.frame(&element, env, now).expect("frame lowers");
        let mut ctx = Context::headless(RenderMode::Fullscreen, w, h);
        ctx.set_color_depth(ColorDepth::TrueColor);
        ctx.set_root(compiled.node);
        ctx.render_now().expect("headless render");
        use std::io::Write;
        io::stdout().write_all(ctx.rendered_bytes())?;
        return Ok(());
    }

    // Live demo.
    let lab = Lab::new();
    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(60)
        .run(lab, update, view)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gibson::input::KeyEvent;
    use gibson::ui::experience::{apply_intent, required_semantics, Intent};

    fn env(w: u16, h: u16, depth: ColorDepth) -> UiEnvironment {
        UiEnvironment {
            width: w,
            height: h,
            color_depth: depth,
            ..UiEnvironment::default()
        }
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
        }
    }

    /// The §24 invariant: driving the *same* semantic state through every grammar
    /// in turn keeps the selection and the active destination fixed, and every
    /// grammar's rendered frame honours the preservation law. This is the headless
    /// CI proof that a live switch never retargets the selection.
    #[test]
    fn selection_and_active_survive_every_live_switch() {
        let lab = Lab::new();
        // Move off the defaults: select the third library item.
        apply_intent(
            &lab.experience,
            lab.ui.borrow_mut().state_mut(),
            Intent::Next,
        );
        apply_intent(
            &lab.experience,
            lab.ui.borrow_mut().state_mut(),
            Intent::Next,
        );
        let selected = lab.ui.borrow().state().selected_key(&lab.experience);
        let active = lab.ui.borrow().state().active_key(&lab.experience);
        assert_eq!(selected, Some(key_named("tidal-automata")));

        let e = env(120, 40, ColorDepth::TrueColor);
        for style in ExperienceStyle::ALL {
            lab.ui.borrow_mut().set_style(style);
            let name = style.name();
            let presented =
                lab.ui
                    .borrow_mut()
                    .present(&lab.experience, &e, Duration::from_millis(250));
            assert_eq!(
                presented.receipt.selected, selected,
                "{name} moved the selection on a live switch"
            );
            assert_eq!(
                Some(presented.receipt.active_destination.clone()),
                active,
                "{name} moved the active destination on a live switch"
            );
            let required = required_semantics(&lab.experience, lab.ui.borrow().state());
            assert!(
                presented.check(&required).is_empty(),
                "{name} violated the preservation law in the lab"
            );
        }
    }

    /// Every grammar's lab frame (status chrome + presented content) lowers,
    /// lays out and paints across the responsive/capability matrix without panic
    /// or a compile error — the whole demo path, headless.
    #[test]
    fn every_style_renders_through_the_full_lab_frame() {
        let sizes = [(160, 50), (120, 40), (80, 24), (60, 20), (42, 15)];
        let depths = [
            ColorDepth::TrueColor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
            ColorDepth::Mono,
        ];
        for style in ExperienceStyle::ALL {
            let lab = Lab::new();
            lab.ui.borrow_mut().set_style(style);
            for (w, h) in sizes {
                for depth in depths {
                    let e = env(w, h, depth);
                    let cx = BuildCx::new(skins::VAPOR95, e);
                    let element = view(&lab, &cx);
                    let mut node = gibson::ui::compile(&element, &cx)
                        .expect("lab frame lowers to a valid tree")
                        .node;
                    gibson::compute_layout(&mut node, w, h).expect("lab frame lays out");
                    let mut surface = gibson::Surface::new(w, h);
                    gibson::paint(&node, &mut surface);
                    assert!(surface.width == w && surface.height == h);
                }
            }
        }
    }

    /// Lab-global keys switch grammar without touching the semantic state; grammar
    /// keys navigate. Proves the two input layers are cleanly separated.
    #[test]
    fn style_keys_switch_without_disturbing_navigation() {
        let mut lab = Lab::new();
        apply_intent(&lab.experience, lab.ui.get_mut().state_mut(), Intent::Next); // select 2nd
        let before = lab.ui.borrow().state().selected_key(&lab.experience);

        // Direct style pick and cycle leave the selection alone.
        update(
            &mut lab,
            AppEvent::Input(Event::Key(press(KeyCode::Char('4')))),
        );
        assert_eq!(lab.ui.borrow().style_name(), "PANORAMA");
        update(
            &mut lab,
            AppEvent::Input(Event::Key(press(KeyCode::Char('s')))),
        );
        assert_eq!(lab.ui.borrow().style_name(), "ORBITAL");
        assert_eq!(
            lab.ui.borrow().state().selected_key(&lab.experience),
            before,
            "switching grammar must not move the selection"
        );

        // A grammar key navigates. ORBITAL binds Right to Next item.
        update(&mut lab, AppEvent::Input(Event::Key(press(KeyCode::Right))));
        assert_ne!(
            lab.ui.borrow().state().selected_key(&lab.experience),
            before,
            "a navigation key should move the selection"
        );

        // Quit is honoured.
        let control = update(
            &mut lab,
            AppEvent::Input(Event::Key(press(KeyCode::Char('q')))),
        );
        assert_eq!(control, Control::Quit);
        assert!(lab.quit);
    }

    fn key_named(name: &str) -> gibson::ui::element::Key {
        gibson::ui::element::Key::named(name)
    }
}
