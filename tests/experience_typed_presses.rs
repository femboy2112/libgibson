//! Typed Experience buttons mounted in one host runtime. Headless, no physical pointer claim.
use gibson::stage::map_actions;
use gibson::ui::experience::{
    handle_key_outcome, Action, Blades, Content, Destination, Experience, Grammar, Item,
    KeyOutcome, PresentationState, Standard,
};
use gibson::ui::prelude::*;
use gibson::{Event, KeyCode, KeyEvent, KeyModifiers, TextInputState};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Inner {
    Open(String),
    Inspect(u32),
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Host {
    Inner(Inner),
    Outside,
    Edited(String),
    Dismiss,
}
fn key(s: &str) -> Key {
    Key::named(s)
}
fn env() -> UiEnvironment {
    UiEnvironment {
        width: 80,
        height: 24,
        ..Default::default()
    }
}
fn event(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}
fn collection(order: &[&str]) -> Experience<Inner> {
    Experience::new("Synthetic").destination(Destination::new(
        "items",
        "Items",
        Content::Collection(
            order
                .iter()
                .map(|id| {
                    Item::new(*id, *id).action(Action::new(
                        format!("open-{id}"),
                        "Open",
                        Inner::Open((*id).into()),
                    ))
                })
                .collect(),
        ),
    ))
}
fn detail() -> Experience<Inner> {
    Experience::new("Detail").destination(Destination::new(
        "detail",
        "Detail",
        Content::Detail {
            facets: vec![],
            actions: vec![
                Action::new("open", "Open", Inner::Open("detail".into())),
                Action::new("inspect", "Inspect", Inner::Inspect(17)),
            ],
        },
    ))
}
fn grammars() -> Vec<Box<dyn Grammar<Inner>>> {
    vec![Box::new(Standard), Box::new(Blades::new())]
}

// Executable host integration recipe, not a second runtime or production dispatcher.
// The double-delivery mutant removes this consumed guard; the once-only test must fail.
fn route(
    ui: &mut UiRuntime<Host>,
    grammar: &dyn Grammar<Inner>,
    experience: &Experience<Inner>,
    state: &mut PresentationState,
    event: &Event,
) -> EventOutcome<Host> {
    let mut out = ui.handle_event(event);
    if !out.consumed {
        if let Event::Key(k) = event {
            match handle_key_outcome(grammar, experience, state, k) {
                KeyOutcome::Action(a) => {
                    out.actions.push(Host::Inner(a));
                    out.consumed = true;
                }
                KeyOutcome::Consumed => out.consumed = true,
                KeyOutcome::Unhandled => {}
            }
        }
    }
    out
}

#[test]
fn standard_and_blades_primary_payloads_survive_mapping_and_headless_activation() {
    let e = collection(&["a", "b"]);
    let state = PresentationState::new(&e);
    for mut g in grammars() {
        let shown = g.present(&e, &state, &env(), Duration::ZERO);
        let tree = column()
            .child(map_actions(shown.element, Host::Inner))
            .child(button("Outside").key("outside").on_press(Host::Outside));
        let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
        ui.frame(&tree, env(), Duration::ZERO).unwrap();
        assert!(ui.set_focus(&key("open-a")));
        let out = ui.handle_event(&event(KeyCode::Enter));
        assert!(out.consumed);
        assert_eq!(out.actions, [Host::Inner(Inner::Open("a".into()))]);
        assert!(ui.set_focus(&key("outside")));
        assert_eq!(
            ui.handle_event(&event(KeyCode::Enter)).actions,
            [Host::Outside]
        );
    }
}

#[test]
fn standard_detail_primary_and_secondary_have_distinct_painted_hit_actions() {
    let e = detail();
    let state = PresentationState::new(&e);
    let mut g = Standard;
    let shown = g.present(&e, &state, &env(), Duration::ZERO);
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let mut c = ui
        .frame(
            &map_actions(shown.element, Host::Inner),
            env(),
            Duration::ZERO,
        )
        .unwrap();
    gibson::compute_layout(&mut c.node, 80, 24).unwrap();
    let mut surface = gibson::Surface::new(80, 24);
    gibson::paint(&c.node, &mut surface);
    let hits = c.hit_map(surface.area()).unwrap();
    for (name, action) in [
        ("open", Inner::Open("detail".into())),
        ("inspect", Inner::Inspect(17)),
    ] {
        let target = hits.targets().iter().find(|t| t.key == key(name)).unwrap();
        assert!(!target.rect.is_empty());
        let hit = hits.hit(target.rect.x, target.rect.y).unwrap();
        assert_eq!(hit.key, key(name));
        assert_eq!(
            c.interactions.get(&hit.key).unwrap().on_press,
            Some(Host::Inner(action))
        );
    }
}

#[test]
fn blades_detail_footer_keeps_typed_primary_and_unrepresented_secondary_inert() {
    let e = detail();
    let state = PresentationState::new(&e);
    let mut g = Blades::new();
    let shown = g.present(&e, &state, &env(), Duration::ZERO);
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let c = ui.frame(&shown.element, env(), Duration::ZERO).unwrap();
    assert_eq!(
        c.interactions.get(&key("open")).unwrap().on_press,
        Some(Inner::Open("detail".into()))
    );
    assert!(c.interactions.get(&key("inspect")).is_none());
}

#[test]
fn omitted_blades_footer_and_raster_chrome_have_no_press_callbacks() {
    let e = collection(&["a", "b"]);
    let state = PresentationState::new(&e);
    let mut g = Blades::new();
    let narrow = UiEnvironment { height: 9, ..env() };
    let shown = g.present(&e, &state, &narrow, Duration::ZERO);
    assert!(shown.receipt.omitted.contains(&key("open-a")));
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let c = ui.frame(&shown.element, narrow, Duration::ZERO).unwrap();
    assert!(c.interactions.entries.iter().all(|i| i.on_press.is_none()));
    assert!(!ui.set_focus(&key("open-a")));
}

#[test]
fn host_delivers_consumed_activation_once_and_keeps_unconsumed_navigation_fallback() {
    let e = collection(&["a", "b"]);
    for mut g in grammars() {
        let mut state = PresentationState::new(&e);
        let shown = g.present(&e, &state, &env(), Duration::ZERO);
        let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
        ui.frame(
            &map_actions(shown.element, Host::Inner),
            env(),
            Duration::ZERO,
        )
        .unwrap();
        assert!(ui.set_focus(&key("open-a")));
        let out = route(&mut ui, g.as_ref(), &e, &mut state, &event(KeyCode::Enter));
        assert!(out.consumed);
        assert_eq!(
            out.actions,
            [Host::Inner(Inner::Open("a".into()))],
            "one physical activation, one payload"
        );
        ui.frame(&column::<Host>(), env(), Duration::ZERO).unwrap();
        assert_eq!(
            route(&mut ui, g.as_ref(), &e, &mut state, &event(KeyCode::Enter)).actions,
            [Host::Inner(Inner::Open("a".into()))]
        );
    }
}

#[test]
fn modal_and_editor_consumption_prevent_background_navigation_or_invocation() {
    let e = collection(&["a", "b"]);
    let mut state = PresentationState::new(&e);
    let mut g = Standard;
    let shown = g.present(&e, &state, &env(), Duration::ZERO);
    let tree = column()
        .child(map_actions(shown.element, Host::Inner))
        .overlay(modal("Blocked").key("modal").on_dismiss(Host::Dismiss));
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    ui.frame(&tree, env(), Duration::ZERO).unwrap();
    let out = route(&mut ui, &g, &e, &mut state, &event(KeyCode::Enter));
    assert!(out.consumed);
    assert!(out.actions.is_empty());
    let input = TextInputState::with_text("draft");
    let tree = text_input(&input)
        .key("editor")
        .on_edit(|s| Host::Edited(s.text));
    ui.frame(&tree, env(), Duration::ZERO).unwrap();
    let before = state.selected_index(&e);
    let out = route(&mut ui, &g, &e, &mut state, &event(KeyCode::Down));
    assert!(out.consumed);
    assert_eq!(state.selected_index(&e), before);
    let out = route(&mut ui, &g, &e, &mut state, &Event::Paste("!".into()));
    assert!(out.consumed);
    assert_eq!(out.actions, [Host::Edited("draft!".into())]);
}

#[test]
fn selected_identity_and_latest_payload_follow_reorder_and_model_rebuild() {
    for mut g in grammars() {
        let e = collection(&["a", "b"]);
        let mut state = PresentationState::new(&e);
        assert!(state.select(&e, &key("b")));
        let shown = g.present(&e, &state, &env(), Duration::ZERO);
        let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
        ui.frame(&shown.element, env(), Duration::ZERO).unwrap();
        assert!(ui.set_focus(&key("open-b")));
        let mut changed = collection(&["b", "a"]);
        if let Content::Collection(items) = &mut changed.destinations[0].content {
            items[0].actions[0].invoke = Inner::Inspect(99);
        }
        let shown = g.present(&changed, &state, &env(), Duration::from_millis(50));
        ui.frame(&shown.element, env(), Duration::from_millis(50))
            .unwrap();
        assert_eq!(state.selected_index(&changed), Some(0));
        assert_eq!(ui.focus(), Some(&key("open-b")));
        assert_eq!(
            ui.handle_event(&event(KeyCode::Enter)).actions,
            [Inner::Inspect(99)]
        );
        let removed = collection(&["a"]);
        let shown = g.present(&removed, &state, &env(), Duration::from_millis(100));
        let c = ui
            .frame(&shown.element, env(), Duration::from_millis(100))
            .unwrap();
        assert!(c.interactions.get(&key("open-b")).is_none());
        assert!(ui
            .handle_event(&event(KeyCode::Enter))
            .actions
            .iter()
            .all(|a| *a != Inner::Inspect(99)));
    }
}
