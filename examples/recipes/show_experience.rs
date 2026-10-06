//! Recipe: **one experience, many grammars, directed over time.**
//!
//! The one law: an [`ExperienceRuntime`] holds a single semantic model and a single
//! selection, and *re-presents* it in whatever [`ExperienceStyle`] you ask for. A
//! [`Timeline`] acts as the director, switching the style over the edit clock — and
//! crucially, switching the style does **not** touch the selection. The same item
//! stays focused as the list becomes a shelf becomes a ring. The director changes
//! the *representation*; the semantics are preserved.
//!
//! Teaches: building an `Experience`, `ExperienceRuntime::present`, `set_style` from
//! a timeline, and selection surviving a change of grammar.
//!
//!   cargo run --example show_experience            # live, cycles the grammars
//!   cargo run --example show_experience -- at 5.0  # one grammar, framed headless

use std::cell::RefCell;
use std::time::Duration;

use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::timeline::Timeline;
use gibson::ui::experience::{
    apply_intent, Action, Content, Destination, Experience, ExperienceRuntime, ExperienceStyle,
    Intent, Item, Media,
};
use gibson::ui::skin::UiEnvironment;
use gibson::ui::{screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;

/// The experience's actions. The director ignores them; they exist so items are
/// real, actionable content.
#[derive(Clone)]
enum Msg {
    Play(String),
}

/// The director's cue sheet: which grammar is on screen when. The payload is the
/// public [`ExperienceStyle`] itself.
fn film() -> Timeline<ExperienceStyle> {
    Timeline::new()
        .cut(4.0, ExperienceStyle::Standard)
        .cut(4.0, ExperienceStyle::MediaShelf)
        .cut(5.0, ExperienceStyle::Orbital)
}

/// A tiny five-item library — the one semantic model every grammar re-presents.
fn library() -> Experience<Msg> {
    let items = [
        ("neon-harbour", "Neon Harbour", "Vaporwave"),
        ("midnight-run", "Midnight Run", "Synthwave"),
        ("glass-arcade", "Glass Arcade", "Chiptune"),
        ("velvet-static", "Velvet Static", "Dreampop"),
        ("chrome-sunrise", "Chrome Sunrise", "Outrun"),
    ];
    let mut collection = Vec::new();
    for (i, (key, title, genre)) in items.into_iter().enumerate() {
        collection.push(
            Item::new(key, title)
                .subtitle(genre)
                .media(Media::new(i as u64 + 1))
                .action(Action::new("play", "Play", Msg::Play(key.to_string()))),
        );
    }
    Experience::new("LIBRARY").destination(Destination::new(
        "library",
        "Library",
        Content::Collection(collection),
    ))
}

/// The show: the semantic model plus its runtime. The runtime is behind a RefCell
/// because `present` needs `&mut` while the `view` closure only gets `&Show`.
struct Show {
    experience: Experience<Msg>,
    ui: RefCell<ExperienceRuntime<Msg>>,
}

impl Show {
    fn new() -> Self {
        let experience = library();
        let mut ui = ExperienceRuntime::with_builtins(&experience);
        // Park the selection on the third item, OFF the default, so it is visibly
        // the same item that stays focused as the grammar changes.
        apply_intent(&experience, ui.state_mut(), Intent::Next);
        apply_intent(&experience, ui.state_mut(), Intent::Next);
        Self {
            experience,
            ui: RefCell::new(ui),
        }
    }

    /// Present the library in the grammar the director wants at `edit`.
    fn present_at(&self, env: &UiEnvironment, now: Duration, edit: f32) -> Element<Msg> {
        let style = film()
            .top(edit)
            .map(|a| *a.payload)
            .unwrap_or(ExperienceStyle::Standard);
        let mut ui = self.ui.borrow_mut();
        ui.set_style(style);
        ui.present(&self.experience, env, now).element
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).map(String::as_str) == Some("at") {
        let t: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let (w, h) = (100u16, 30u16);
        let show = Show::new();
        let env = UiEnvironment {
            width: w,
            height: h,
            ..UiEnvironment::default()
        };
        let element = show.present_at(&env, Duration::from_secs_f32(t), t);
        for line in render_headless(element, &env, w, h).to_visible_lines() {
            println!("{line}");
        }
        return Ok(());
    }

    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(30)
        .run(Show::new(), update, view)?;
    Ok(())
}

/// Lower an element to a Surface without a terminal, for the headless `at` frame.
/// The presented grammar is wrapped in a full-height `screen` so layout gives it the
/// whole viewport (compiling the bare element collapses it to its title).
fn render_headless(element: Element<Msg>, env: &UiEnvironment, w: u16, h: u16) -> Surface {
    let cx = BuildCx::new(skins::VAPOR95, *env);
    let root = screen::<Msg>().child(element.grow(1.0)).height(h);
    let mut surface =
        gibson::raster::RgbRaster::new(w.max(1), h.saturating_mul(2).max(2)).to_surface();
    if let Ok(compiled) = gibson::ui::compile(&root, &cx) {
        let mut node = compiled.node;
        if gibson::compute_layout(&mut node, w, h).is_ok() {
            gibson::paint(&node, &mut surface);
        }
    }
    surface
}

fn view(show: &Show, cx: &BuildCx) -> Element<Msg> {
    let edit = cx.time.as_secs_f32() % film().duration().max(0.001);
    screen::<Msg>().child(show.present_at(&cx.environment, cx.time, edit).grow(1.0))
}

fn update(_show: &mut Show, event: AppEvent<Msg>) -> Control {
    match event {
        AppEvent::Input(Event::Key(key)) => {
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
                || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                return Control::Quit;
            }
        }
        // This show is director-driven, so it doesn't act on selections — but a real
        // app would start playback for `key` here.
        AppEvent::Action(Msg::Play(key)) => {
            let _ = key;
        }
        _ => {}
    }
    Control::Continue
}
