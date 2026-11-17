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

// Executable scoped host recipe; the caller chooses this Experience's pane.
// Compiled sidecars and receipt keys belong to the same frame as UiRuntime.
fn grammar_route(
    grammar: &dyn Grammar<Inner>,
    experience: &Experience<Inner>,
    state: &mut PresentationState,
    event: &Event,
) -> EventOutcome<Host> {
    let Event::Key(k) = event else {
        return EventOutcome::default();
    };
    match handle_key_outcome(grammar, experience, state, k) {
        KeyOutcome::Action(a) => EventOutcome {
            actions: vec![Host::Inner(a)],
            consumed: true,
        },
        KeyOutcome::Consumed => EventOutcome {
            actions: vec![],
            consumed: true,
        },
        KeyOutcome::Unhandled => EventOutcome::default(),
    }
}
fn route(
    ui: &mut UiRuntime<Host>,
    compiled: &Compiled<Host>,
    action_keys: &[Key],
    grammar: &dyn Grammar<Inner>,
    experience: &Experience<Inner>,
    state: &mut PresentationState,
    event: &Event,
) -> EventOutcome<Host> {
    let capture_first = compiled.interactions.active_modal().is_some()
        || ui
            .focus()
            .and_then(|k| compiled.interactions.get(k))
            .is_some_and(|i| i.input.is_some() || i.on_event.is_some());
    let owns_pane = !capture_first && ui.focus().is_none_or(|k| action_keys.contains(k));
    let owned_selection = owns_pane
        && matches!(event, Event::Key(k) if matches!(k.code,
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
        | KeyCode::Home | KeyCode::End | KeyCode::Char('h' | 'j' | 'k' | 'l')));
    if owned_selection {
        let out = grammar_route(grammar, experience, state, event);
        if out.consumed {
            return out;
        }
        return ui.handle_event(event); // Grammar did not bind it; do not try it twice.
    }
    let mut out = ui.handle_event(event);
    if !out.consumed && owns_pane {
        let fallback = grammar_route(grammar, experience, state, event);
        out.actions.extend(fallback.actions);
        out.consumed = fallback.consumed;
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
        let keys = shown.receipt.actions;
        let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
        let compiled = ui
            .frame(
                &map_actions(shown.element, Host::Inner),
                env(),
                Duration::ZERO,
            )
            .unwrap();
        assert!(ui.set_focus(&key("open-a")));
        let out = route(
            &mut ui,
            &compiled,
            &keys,
            g.as_ref(),
            &e,
            &mut state,
            &event(KeyCode::Enter),
        );
        assert!(out.consumed);
        assert_eq!(
            out.actions,
            [Host::Inner(Inner::Open("a".into()))],
            "one physical activation, one payload"
        );
        let compiled = ui.frame(&column::<Host>(), env(), Duration::ZERO).unwrap();
        assert_eq!(
            route(
                &mut ui,
                &compiled,
                &keys,
                g.as_ref(),
                &e,
                &mut state,
                &event(KeyCode::Enter)
            )
            .actions,
            [Host::Inner(Inner::Open("a".into()))]
        );
    }
}

#[test]
fn modal_editor_and_custom_capture_prevent_background_navigation_or_invocation() {
    let e = collection(&["a", "b"]);
    let mut state = PresentationState::new(&e);
    let mut g = Standard;
    let shown = g.present(&e, &state, &env(), Duration::ZERO);
    let keys = shown.receipt.actions;
    let tree = column()
        .child(map_actions(shown.element, Host::Inner))
        .overlay(modal("Blocked").key("modal").on_dismiss(Host::Dismiss));
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let compiled = ui.frame(&tree, env(), Duration::ZERO).unwrap();
    for code in [KeyCode::Enter, KeyCode::Down] {
        let out = route(&mut ui, &compiled, &keys, &g, &e, &mut state, &event(code));
        assert!(out.consumed);
        assert!(out.actions.is_empty());
        assert_eq!(state.selected_index(&e), Some(0));
    }
    let input = TextInputState::with_text("draft");
    let tree = text_input(&input)
        .key("editor")
        .on_edit(|s| Host::Edited(s.text));
    let compiled = ui.frame(&tree, env(), Duration::ZERO).unwrap();
    let out = route(
        &mut ui,
        &compiled,
        &keys,
        &g,
        &e,
        &mut state,
        &event(KeyCode::Down),
    );
    assert!(out.consumed);
    assert_eq!(state.selected_index(&e), Some(0));
    let out = route(
        &mut ui,
        &compiled,
        &keys,
        &g,
        &e,
        &mut state,
        &Event::Paste("!".into()),
    );
    assert!(out.consumed);
    assert_eq!(out.actions, [Host::Edited("draft!".into())]);
    let tree = column::<Host>().key("custom").on_event(|_| Host::Outside);
    let compiled = ui.frame(&tree, env(), Duration::ZERO).unwrap();
    let out = route(
        &mut ui,
        &compiled,
        &keys,
        &g,
        &e,
        &mut state,
        &event(KeyCode::Down),
    );
    assert!(out.consumed);
    assert_eq!(out.actions, [Host::Outside]);
    assert_eq!(state.selected_index(&e), Some(0));
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

fn mounted(element: Element<Inner>, extras: usize) -> Element<Host> {
    column()
        .child(map_actions(element, Host::Inner))
        .children((0..extras).map(|i| {
            button("Outside")
                .key(format!("outside-{i}"))
                .on_press(Host::Outside)
        }))
}

#[test]
fn scoped_host_preserves_experience_selection_with_other_focusables() {
    let e = collection(&["a", "b"]);
    for mut g in grammars() {
        for extras in [0, 2] {
            let mut state = PresentationState::new(&e);
            let shown = g.present(&e, &state, &env(), Duration::ZERO);
            let keys = shown.receipt.actions;
            let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
            let compiled = ui
                .frame(&mounted(shown.element, extras), env(), Duration::ZERO)
                .unwrap();
            assert!(ui.set_focus(&key("open-a")));
            let arrow = if g.name() == "STANDARD" {
                KeyCode::Down
            } else {
                KeyCode::Right
            };
            let out = route(
                &mut ui,
                &compiled,
                &keys,
                g.as_ref(),
                &e,
                &mut state,
                &event(arrow),
            );
            assert!(out.consumed);
            assert!(out.actions.is_empty());
            assert_eq!(
                state.selected_index(&e),
                Some(1),
                "owned arrow changes Experience selection"
            );
            assert_eq!(
                ui.focus(),
                Some(&key("open-a")),
                "selection does not run generic focus traversal"
            );
            for (code, want) in [(KeyCode::Home, 0), (KeyCode::End, 1)] {
                assert!(
                    route(
                        &mut ui,
                        &compiled,
                        &keys,
                        g.as_ref(),
                        &e,
                        &mut state,
                        &event(code)
                    )
                    .consumed
                );
                assert_eq!(state.selected_index(&e), Some(want));
            }
            let shown = g.present(&e, &state, &env(), Duration::from_millis(50));
            let keys = shown.receipt.actions;
            let compiled = ui
                .frame(
                    &mounted(shown.element, extras),
                    env(),
                    Duration::from_millis(50),
                )
                .unwrap();
            assert!(ui.set_focus(&key("open-b"))); // Host preserves its declared pane after rebuilding.
            assert_eq!(
                route(
                    &mut ui,
                    &compiled,
                    &keys,
                    g.as_ref(),
                    &e,
                    &mut state,
                    &event(KeyCode::Enter)
                )
                .actions,
                [Host::Inner(Inner::Open("b".into()))]
            );
            if extras > 0 {
                assert!(
                    route(
                        &mut ui,
                        &compiled,
                        &keys,
                        g.as_ref(),
                        &e,
                        &mut state,
                        &event(KeyCode::Tab)
                    )
                    .consumed
                );
                assert_eq!(ui.focus(), Some(&key("outside-0")), "Tab stays UI-first");
            }
        }
    }
}

#[test]
fn unrelated_host_focus_keeps_ui_arrow_traversal_and_does_not_navigate_the_pane() {
    let e = collection(&["a", "b"]);
    let mut state = PresentationState::new(&e);
    let mut g = Standard;
    let shown = g.present(&e, &state, &env(), Duration::ZERO);
    let keys = shown.receipt.actions;
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let compiled = ui
        .frame(&mounted(shown.element, 2), env(), Duration::ZERO)
        .unwrap();
    assert!(ui.set_focus(&key("outside-0")));
    let out = route(
        &mut ui,
        &compiled,
        &keys,
        &g,
        &e,
        &mut state,
        &event(KeyCode::Down),
    );
    assert!(out.consumed);
    assert_eq!(ui.focus(), Some(&key("outside-1")));
    assert_eq!(state.selected_index(&e), Some(0));
}

#[test]
fn one_runtime_routes_selection_to_the_owning_presented_receipt_only() {
    let mut first = collection(&["first-a", "first-b"]);
    first.destinations[0].key = key("first-items");
    let mut second = collection(&["second-a", "second-b"]);
    second.destinations[0].key = key("second-items");
    let mut a = PresentationState::new(&first);
    let mut b = PresentationState::new(&second);
    let mut standard = Standard;
    let mut blades = Blades::new();
    let p = standard.present(&first, &a, &env(), Duration::ZERO);
    let q = blades.present(&second, &b, &env(), Duration::ZERO);
    let akeys = p.receipt.actions;
    let bkeys = q.receipt.actions;
    let tree = column()
        .child(map_actions(p.element, Host::Inner))
        .child(map_actions(q.element, Host::Inner));
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let compiled = ui.frame(&tree, env(), Duration::ZERO).unwrap();
    assert!(ui.set_focus(&key("open-first-a")));
    assert!(
        route(
            &mut ui,
            &compiled,
            &akeys,
            &standard,
            &first,
            &mut a,
            &event(KeyCode::Down)
        )
        .consumed
    );
    assert_eq!(a.selected_index(&first), Some(1));
    assert_eq!(b.selected_index(&second), Some(0));
    assert!(ui.set_focus(&key("open-second-a")));
    assert!(
        route(
            &mut ui,
            &compiled,
            &bkeys,
            &blades,
            &second,
            &mut b,
            &event(KeyCode::Right)
        )
        .consumed
    );
    assert_eq!(a.selected_index(&first), Some(1));
    assert_eq!(b.selected_index(&second), Some(1));
}
