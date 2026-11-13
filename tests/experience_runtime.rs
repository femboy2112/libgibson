//! `ExperienceRuntime` contract (milestone §48): built-in style switching in one
//! call, custom grammar registration, semantic-state and grammar-private-state
//! preservation across switches, the temporal `FrameDemand` for static/settling/
//! continuous grammars, and the deterministic dynamic `Custom` presentation time.

use gibson::capability::ColorDepth;
use gibson::ui::element::{screen, text, Element, Key};
use gibson::ui::experience::{
    apply_intent, Action, Content, Custom, CustomCx, Destination, Experience, ExperienceRuntime,
    ExperienceStyle, FrameDemand, Grammar, Intent, Item, Media, PresentationReceipt,
    PresentationState, Presented, SemanticInput, StyleId,
};
use gibson::ui::skin::UiEnvironment;
use gibson::ui::{skins, BuildCx};
use gibson::{Cell, Glyph, Style, Surface};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Play(String),
}

fn env(w: u16, h: u16, depth: ColorDepth) -> UiEnvironment {
    UiEnvironment {
        width: w,
        height: h,
        color_depth: depth,
        ..UiEnvironment::default()
    }
}

/// A library collection + an observatory custom instrument — enough to exercise the
/// collection grammars and the dynamic `Custom` path.
fn fixture() -> Experience<Msg> {
    let library = Content::Collection(vec![
        Item::new("a", "Alpha")
            .media(Media::new(1))
            .action(Action::new("pa", "Play", Msg::Play("a".into()))),
        Item::new("b", "Beta")
            .media(Media::new(2))
            .action(Action::new("pb", "Play", Msg::Play("b".into()))),
        Item::new("c", "Gamma")
            .media(Media::new(3))
            .action(Action::new("pc", "Play", Msg::Play("c".into()))),
    ]);
    Experience::new("FIXTURE")
        .destination(Destination::new("lib", "Library", library))
        .destination(Destination::new(
            "obs",
            "Observatory",
            Content::Custom(clock_instrument()),
        ))
}

/// A dynamic instrument whose every cell is the current 100ms-bucket digit of the
/// presentation time — a pure function of `cx.now`, never the wall clock.
fn clock_instrument() -> Custom {
    Custom::dynamic("clock", "Clock", |cx: &CustomCx| {
        let digit = (cx.now.as_millis() / 100 % 10) as u32;
        let ch = char::from_digit(digit, 10).unwrap_or('?');
        let mut surface = Surface::new(cx.width, cx.height);
        for y in 0..cx.height {
            for x in 0..cx.width {
                surface.set_cell(x, y, Cell::new(Glyph::new(&ch.to_string()), Style::new()));
            }
        }
        surface
    })
}

fn paint_surface(element: &Element<Msg>, e: UiEnvironment) -> Surface {
    let cx = BuildCx::new(skins::VAPOR95, e);
    let mut node = gibson::ui::compile(element, &cx)
        .expect("element lowers")
        .node;
    gibson::compute_layout(&mut node, e.width, e.height).expect("layout");
    let mut surface = Surface::new(e.width, e.height);
    gibson::paint(&node, &mut surface);
    surface
}

#[test]
fn builtin_style_switch_is_one_call() {
    let exp = fixture();
    let mut ui = ExperienceRuntime::with_builtins(&exp);
    // The reference oracle is active first.
    assert_eq!(
        ui.current_style(),
        Some(&StyleId::Builtin(ExperienceStyle::Standard))
    );
    assert_eq!(ui.style_count(), 6);

    assert!(ui.set_style(ExperienceStyle::Blades));
    assert_eq!(ui.style_name(), "BLADES");
    assert_eq!(
        ui.current_style(),
        Some(&StyleId::Builtin(ExperienceStyle::Blades))
    );

    // Every built-in is reachable by name in one call.
    for style in ExperienceStyle::ALL {
        assert!(ui.set_style(style));
        assert_eq!(ui.style_name(), style.name());
    }
}

#[test]
fn next_and_previous_cycle_the_styles() {
    let exp = fixture();
    let mut ui = ExperienceRuntime::with_builtins(&exp);
    assert_eq!(ui.style_name(), "STANDARD");
    ui.next_style();
    assert_eq!(ui.style_name(), "MEDIA_SHELF");
    ui.previous_style();
    assert_eq!(ui.style_name(), "STANDARD");
    // Wrapping both ways.
    ui.previous_style();
    assert_eq!(ui.style_name(), "BLADES");
    ui.next_style();
    assert_eq!(ui.style_name(), "STANDARD");
}

#[test]
fn custom_grammar_registration_is_possible() {
    let exp = fixture();
    let mut ui = ExperienceRuntime::empty(&exp);
    assert_eq!(ui.style_count(), 0);
    assert_eq!(ui.style_name(), "—");

    ui.register("tag", Box::new(TagGrammar));
    assert_eq!(ui.style_count(), 1);
    assert_eq!(
        ui.current_style(),
        Some(&StyleId::Custom("tag".to_string()))
    );
    // It presents without panicking, and its receipt carries the grammar's name.
    let e = env(80, 24, ColorDepth::TrueColor);
    let presented = ui.present(&exp, &e, Duration::ZERO);
    assert_eq!(presented.receipt.style, "TAG");

    // A second custom style can be registered and selected by its id.
    ui.register("tag2", Box::new(TagGrammar));
    assert_eq!(ui.style_count(), 2);
    assert!(ui.set_style_id(&StyleId::Custom("tag2".to_string())));
    assert_eq!(ui.style_name(), "tag2");
    assert!(ui.set_style_id(&StyleId::Custom("tag".to_string())));
    assert_eq!(ui.style_name(), "tag");
    // A built-in that was never registered is not selectable here.
    assert!(!ui.set_style(ExperienceStyle::Blades));
    assert_eq!(ui.style_name(), "tag");
}

#[test]
fn selection_survives_style_switch_a_b_a() {
    let exp = fixture();
    let mut ui = ExperienceRuntime::with_builtins(&exp);
    // Select Beta in the library.
    apply_intent(&exp, ui.state_mut(), Intent::Next);
    let selected = ui.state().selected_key(&exp);
    assert_eq!(selected, Some(Key::named("b")));

    let e = env(120, 40, ColorDepth::TrueColor);
    ui.set_style(ExperienceStyle::MediaShelf);
    let _ = ui.present(&exp, &e, Duration::from_millis(50));
    ui.set_style(ExperienceStyle::Orbital);
    let _ = ui.present(&exp, &e, Duration::from_millis(100));
    ui.set_style(ExperienceStyle::MediaShelf);
    let presented = ui.present(&exp, &e, Duration::from_millis(150));

    assert_eq!(
        ui.state().selected_key(&exp),
        selected,
        "A→B→A lost selection"
    );
    assert_eq!(presented.receipt.selected, selected);
}

#[test]
fn grammar_private_state_persists_across_switch_away_and_back() {
    use std::cell::Cell as StdCell;
    use std::rc::Rc;

    let exp = fixture();
    let mut ui = ExperienceRuntime::with_builtins(&exp);
    let calls = Rc::new(StdCell::new(0u32));
    ui.register("counter", Box::new(CountingGrammar(calls.clone())));

    let e = env(80, 24, ColorDepth::TrueColor);
    let counter = StyleId::Custom("counter".to_string());

    ui.set_style_id(&counter);
    let _ = ui.present(&exp, &e, Duration::ZERO); // calls = 1
    ui.set_style(ExperienceStyle::Standard);
    let _ = ui.present(&exp, &e, Duration::ZERO); // standard; counter not called
    ui.set_style_id(&counter);
    let _ = ui.present(&exp, &e, Duration::ZERO); // calls = 2 — SAME instance persisted

    assert_eq!(
        calls.get(),
        2,
        "switching away and back must not recreate (reset) the grammar instance"
    );
}

#[test]
fn frame_demand_reports_static_settling_and_continuous() {
    let exp = fixture();
    let mut ui = ExperienceRuntime::with_builtins(&exp);
    let e = env(120, 40, ColorDepth::TrueColor);

    // Static grammars rest immediately.
    for style in [
        ExperienceStyle::Standard,
        ExperienceStyle::CrossMedia,
        ExperienceStyle::Panorama,
    ] {
        ui.set_style(style);
        assert_eq!(
            ui.present(&exp, &e, Duration::from_secs(1)).demand,
            FrameDemand::OnChange,
            "{} should be time-invariant",
            style.name()
        );
    }

    // ORBITAL is continuously animated on the collection — it never rests.
    ui.set_style(ExperienceStyle::Orbital);
    assert!(
        ui.present(&exp, &e, Duration::from_secs(10))
            .demand
            .is_animating(),
        "ORBITAL should always ask for frames"
    );

    // MEDIA_SHELF settles: animating while gliding, resting once parked.
    ui.set_style(ExperienceStyle::MediaShelf);
    let _ = ui.present(&exp, &e, Duration::from_millis(0)); // init spring at selection 0
    apply_intent(&exp, ui.state_mut(), Intent::End); // jump the target to the last item
    let moving = ui.present(&exp, &e, Duration::from_millis(16));
    assert!(
        moving.demand.is_animating(),
        "MEDIA_SHELF should animate while settling"
    );
    let mut settled = false;
    for i in 2..1200 {
        let t = Duration::from_millis(16 * i);
        if ui.present(&exp, &e, t).demand == FrameDemand::OnChange {
            settled = true;
            break;
        }
    }
    assert!(settled, "MEDIA_SHELF should settle to OnChange");
}

#[test]
fn dynamic_custom_is_a_deterministic_function_of_presentation_time() {
    let instrument = clock_instrument();
    let at = |ms: u64| {
        instrument.render(&CustomCx::new(
            8,
            3,
            Duration::from_millis(ms),
            ColorDepth::TrueColor,
        ))
    };
    // Same presentation time → byte-identical surface (no wall-clock read).
    assert_eq!(at(500), at(500));
    // Different presentation time → different surface (it does depend on `now`).
    assert_ne!(at(500), at(1300));

    // The static convenience constructor ignores time entirely.
    let still = Custom::new("c", "C", Surface::new);
    let s0 = still.render(&CustomCx::new(8, 3, Duration::ZERO, ColorDepth::Mono));
    let s9 = still.render(&CustomCx::new(
        8,
        3,
        Duration::from_secs(9),
        ColorDepth::Mono,
    ));
    assert_eq!(s0, s9, "a static Custom must not vary with time");
}

#[test]
fn a_grammar_threads_presentation_time_to_a_dynamic_custom() {
    let exp = fixture();
    let mut ui = ExperienceRuntime::with_builtins(&exp);
    // Standard composites the Custom and is otherwise time-invariant, so any frame
    // difference across `now` must come from the instrument receiving the time.
    ui.set_style(ExperienceStyle::Standard);
    ui.state_mut().activate(&exp, &Key::named("obs"));
    let e = env(80, 24, ColorDepth::TrueColor);

    let early = paint_surface(&ui.present(&exp, &e, Duration::from_millis(500)).element, e);
    let later = paint_surface(
        &ui.present(&exp, &e, Duration::from_millis(1700)).element,
        e,
    );
    assert_ne!(
        early, later,
        "the grammar must forward presentation time to the dynamic Custom"
    );
}

// ---- test grammars -------------------------------------------------------------------

/// A trivial custom grammar: renders the title, attests every destination rastered
/// so the receipt is lawful, binds nothing.
struct TagGrammar;

impl Grammar<Msg> for TagGrammar {
    fn name(&self) -> &'static str {
        "TAG"
    }
    fn present(
        &mut self,
        experience: &Experience<Msg>,
        _state: &PresentationState,
        _env: &UiEnvironment,
        _now: Duration,
    ) -> Presented<Msg> {
        let active = experience
            .destinations
            .first()
            .map(|d| d.key.clone())
            .unwrap_or_else(|| Key::named("∅"));
        let mut receipt = PresentationReceipt::new("TAG", active);
        for destination in &experience.destinations {
            receipt.destinations.push(destination.key.clone());
            receipt.rastered.push(destination.key.clone());
        }
        Presented::new(
            screen::<Msg>().child(text::<Msg>(experience.title.clone())),
            receipt,
        )
    }
    fn interpret(
        &self,
        _key: &gibson::input::KeyEvent,
        _experience: &Experience<Msg>,
        _state: &PresentationState,
    ) -> Option<SemanticInput<Msg>> {
        None
    }
}

/// Counts how many times `present` was called on *this instance* — proof that the
/// runtime keeps one persistent instance across style switches.
struct CountingGrammar(std::rc::Rc<std::cell::Cell<u32>>);

impl Grammar<Msg> for CountingGrammar {
    fn name(&self) -> &'static str {
        "COUNTER"
    }
    fn present(
        &mut self,
        experience: &Experience<Msg>,
        _state: &PresentationState,
        _env: &UiEnvironment,
        _now: Duration,
    ) -> Presented<Msg> {
        self.0.set(self.0.get() + 1);
        let active = experience
            .destinations
            .first()
            .map(|d| d.key.clone())
            .unwrap_or_else(|| Key::named("∅"));
        Presented::new(
            screen::<Msg>().child(text::<Msg>("counter")),
            PresentationReceipt::new("COUNTER", active),
        )
    }
    fn interpret(
        &self,
        _key: &gibson::input::KeyEvent,
        _experience: &Experience<Msg>,
        _state: &PresentationState,
    ) -> Option<SemanticInput<Msg>> {
        None
    }
}
