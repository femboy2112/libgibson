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
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::ui::experience::{
    handle_key, Action, Blades, Content, CrossMedia, Destination, Experience, Facet, Grammar, Item,
    Media, MediaShelf, Orbital, Panorama, PresentationState, Standard,
};
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

/// A named, swappable grammar. `RefCell` because `present` needs `&mut` (spring
/// state) while the `view` only borrows the model immutably.
type GrammarSlot = (&'static str, RefCell<Box<dyn Grammar<Msg>>>);

/// Every grammar the lab can wear, in cycle order. STANDARD is the reference
/// oracle; the rest are the cinematic styles.
fn grammars() -> Vec<GrammarSlot> {
    let mk = |b: Box<dyn Grammar<Msg>>| RefCell::new(b);
    vec![
        ("STANDARD", mk(Box::new(Standard::new()))),
        ("MEDIA_SHELF", mk(Box::new(MediaShelf::new()))),
        ("CROSS_MEDIA", mk(Box::new(CrossMedia::new()))),
        ("PANORAMA", mk(Box::new(Panorama::new()))),
        ("ORBITAL", mk(Box::new(Orbital::new()))),
        ("BLADES", mk(Box::new(Blades::new()))),
    ]
}

/// The application model. Note what is *not* here: no per-style branch, no
/// per-style layout. Just the semantic value, the identity-keyed navigation state,
/// and which grammar is currently worn.
struct Lab {
    experience: Experience<Msg>,
    state: PresentationState,
    grammars: Vec<GrammarSlot>,
    current: usize,
    last_action: Option<Msg>,
    quit: bool,
}

impl Lab {
    fn new() -> Self {
        let experience = build_experience();
        let state = PresentationState::new(&experience);
        Self {
            experience,
            state,
            grammars: grammars(),
            current: 0,
            last_action: None,
            quit: false,
        }
    }

    fn style_name(&self) -> &'static str {
        self.grammars[self.current].0
    }

    fn set_style(&mut self, index: usize) {
        self.current = index.min(self.grammars.len() - 1);
    }

    fn next_style(&mut self) {
        self.current = (self.current + 1) % self.grammars.len();
    }
}

/// The one and only view. It picks the active grammar, presents the shared
/// semantic value through it, and overlays a thin lab status line. There is no
/// `match` on the style here — the grammar is a value, swapped underneath.
fn view(lab: &Lab, cx: &BuildCx) -> Element<Msg> {
    let presented = {
        let mut grammar = lab.grammars[lab.current].1.borrow_mut();
        grammar.present(&lab.experience, &lab.state, &cx.environment, cx.time)
    };

    let last = lab
        .last_action
        .as_ref()
        .map(|m| format!("{m:?}"))
        .unwrap_or_else(|| "—".into());
    let bar = row::<Msg>()
        .gap(2)
        .child(status::<Msg>(format!(
            "EXPERIENCE LAB  [{}/{}] {}",
            lab.current + 1,
            lab.grammars.len(),
            lab.style_name()
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
                    lab.next_style();
                    return Control::Continue;
                }
                KeyCode::Char(d @ '1'..='6') => {
                    lab.set_style(d as usize - '1' as usize);
                    return Control::Continue;
                }
                _ => {}
            }
            // Everything else: the active grammar decides what the key means, and
            // `apply_intent` applies it identically for every grammar.
            let Lab {
                experience,
                state,
                grammars,
                current,
                ..
            } = lab;
            let grammar = grammars[*current].1.borrow();
            if let Some(msg) = handle_key(&**grammar, experience, state, &key) {
                drop(grammar);
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
        for (name, cell) in &lab.grammars {
            let presented = cell.borrow_mut().present(
                &lab.experience,
                &lab.state,
                &env,
                Duration::from_millis(500),
            );
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
        let mut lab = Lab::new();
        // Move off the defaults: select the third library item.
        apply_intent(&lab.experience, &mut lab.state, Intent::Next);
        apply_intent(&lab.experience, &mut lab.state, Intent::Next);
        let selected = lab.state.selected_key(&lab.experience);
        let active = lab.state.active_key(&lab.experience);
        assert_eq!(selected, Some(key_named("tidal-automata")));

        let e = env(120, 40, ColorDepth::TrueColor);
        for index in 0..lab.grammars.len() {
            lab.set_style(index);
            let name = lab.style_name();
            let presented = lab.grammars[index].1.borrow_mut().present(
                &lab.experience,
                &lab.state,
                &e,
                Duration::from_millis(250),
            );
            assert_eq!(
                presented.receipt.selected, selected,
                "{name} moved the selection on a live switch"
            );
            assert_eq!(
                Some(presented.receipt.active_destination.clone()),
                active,
                "{name} moved the active destination on a live switch"
            );
            let required = required_semantics(&lab.experience, &lab.state);
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
        for index in 0..grammars().len() {
            let mut lab = Lab::new();
            lab.set_style(index);
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
        apply_intent(&lab.experience, &mut lab.state, Intent::Next); // select 2nd
        let before = lab.state.selected_key(&lab.experience);

        // Direct style pick and cycle leave the selection alone.
        update(
            &mut lab,
            AppEvent::Input(Event::Key(press(KeyCode::Char('4')))),
        );
        assert_eq!(lab.style_name(), "PANORAMA");
        update(
            &mut lab,
            AppEvent::Input(Event::Key(press(KeyCode::Char('s')))),
        );
        assert_eq!(lab.style_name(), "ORBITAL");
        assert_eq!(
            lab.state.selected_key(&lab.experience),
            before,
            "switching grammar must not move the selection"
        );

        // A grammar key navigates. ORBITAL binds Right to Next item.
        update(&mut lab, AppEvent::Input(Event::Key(press(KeyCode::Right))));
        assert_ne!(
            lab.state.selected_key(&lab.experience),
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
