use gibson::scene::{Presentation, Scene, SceneEntity};
use gibson::stage::map_actions;
use gibson::ui::prelude::*;
use gibson::{
    compute_layout, paint, Event, KeyCode, KeyEvent, KeyModifiers, Node, Style, Surface,
    TextInputState,
};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Local {
    Press,
    Edit(String),
    Custom(Event),
    Dismiss,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Host {
    Local(Local),
    Outer,
    Draft(String),
}

fn event(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}

fn local_tree() -> Element<Local> {
    let mut input = TextInputState::new();
    input.text = "kept draft".into();
    input.cursor_grapheme = 3;
    column()
        .key("local-root")
        .gap(0)
        .padding(0)
        .child(
            button("Activate")
                .key("inner-press")
                .on_press(Local::Press)
                .selected(true)
                .revision(42),
        )
        .child(
            text_input(&input)
                .key("inner-edit")
                .on_edit(|s| Local::Edit(s.text)),
        )
        .child(
            raw(Node::text("CUSTOM", Style::new()).width(12.0).height(1.0))
                .key("inner-custom")
                .on_event(Local::Custom),
        )
        .overlay(
            modal("Details")
                .key("inner-modal")
                .on_dismiss(Local::Dismiss)
                .child(
                    button("Disabled")
                        .key("inner-disabled")
                        .on_press(Local::Press)
                        .disabled(true),
                ),
        )
}

fn mapped_local() -> Compiled<Host> {
    let cx = BuildCx::new(skins::SWISS_SIGNAL, UiEnvironment::default());
    compile(&map_actions(local_tree(), Host::Local), &cx).unwrap()
}

#[test]
fn maps_press_actions() {
    let frame = mapped_local();
    assert_eq!(
        frame
            .interactions
            .get(&Key::named("inner-press"))
            .unwrap()
            .on_press,
        Some(Host::Local(Local::Press))
    );
    assert!(
        frame
            .interactions
            .get(&Key::named("inner-disabled"))
            .unwrap()
            .disabled
    );
}

#[test]
fn maps_edit_actions_and_preserves_input_snapshot() {
    let frame = mapped_local();
    let editor = frame.interactions.get(&Key::named("inner-edit")).unwrap();
    assert_eq!(editor.input.as_ref().unwrap().cursor_grapheme, 3);
    assert_eq!(editor.input.as_ref().unwrap().text, "kept draft");
    let mut next = TextInputState::new();
    next.text = "new draft".into();
    assert_eq!(
        editor.on_edit.as_ref().unwrap()(next),
        Host::Local(Local::Edit("new draft".into()))
    );
}

#[test]
fn maps_custom_events() {
    let frame = mapped_local();
    let custom = event(KeyCode::Char('x'));
    assert_eq!(
        frame
            .interactions
            .get(&Key::named("inner-custom"))
            .unwrap()
            .on_event
            .as_ref()
            .unwrap()(custom.clone()),
        Host::Local(Local::Custom(custom))
    );
}

#[test]
fn maps_modal_dismissal_in_overlays() {
    let frame = mapped_local();
    assert_eq!(
        frame.interactions.active_modal().unwrap().on_dismiss,
        Some(Host::Local(Local::Dismiss))
    );
}

#[test]
fn mapping_preserves_keys_target_specs_state_and_painted_cells() {
    let cx = BuildCx::new(skins::SWISS_SIGNAL, UiEnvironment::default());
    let local = local_tree();
    let mut before = compile(&local, &cx).unwrap();
    let mut after = compile(&map_actions(local, Host::Local), &cx).unwrap();
    assert_eq!(before.keys, after.keys);
    assert_eq!(before.targets, after.targets);
    compute_layout(&mut before.node, 80, 24).unwrap();
    compute_layout(&mut after.node, 80, 24).unwrap();
    let mut a = Surface::new(80, 24);
    let mut b = Surface::new(80, 24);
    paint(&before.node, &mut a);
    paint(&after.node, &mut b);
    for y in 0..24 {
        for x in 0..80 {
            assert_eq!(a.get(x, y), b.get(x, y), "cell {x},{y}");
        }
    }
}

fn host_tree(open: bool) -> Element<Host> {
    let mut tree = column()
        .key("host")
        .child(
            text_input(&TextInputState::new())
                .key("composer")
                .on_edit(|s| Host::Draft(s.text)),
        )
        .child(button("Outer").key("outer").on_press(Host::Outer));
    if open {
        tree = tree.overlay(map_actions(
            modal("Details")
                .key("dialog")
                .on_dismiss(Local::Dismiss)
                .child(button("Inner").key("inner").on_press(Local::Press)),
            Host::Local,
        ));
    }
    tree
}

#[test]
fn one_host_runtime_routes_inner_actions_and_restores_outer_focus() {
    let env = UiEnvironment::default();
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    ui.frame(&host_tree(false), env, Duration::ZERO).unwrap();
    assert!(ui.set_focus(&Key::named("composer")));
    ui.frame(&host_tree(true), env, Duration::from_millis(1))
        .unwrap();
    assert_eq!(ui.focus(), Some(&Key::named("inner")));
    assert!(!ui.set_focus(&Key::named("outer")));
    let activation = ui.handle_event(&event(KeyCode::Enter));
    assert!(activation.consumed);
    assert_eq!(activation.actions, [Host::Local(Local::Press)]);
    assert_eq!(
        ui.handle_event(&event(KeyCode::Esc)).actions,
        [Host::Local(Local::Dismiss)]
    );
    ui.frame(&host_tree(false), env, Duration::from_millis(2))
        .unwrap();
    assert_eq!(ui.focus(), Some(&Key::named("composer")));
    assert_eq!(
        ui.handle_event(&Event::Paste("typed".into())).actions,
        [Host::Draft("typed".into())]
    );
}

#[test]
fn duplicate_inner_and_outer_keys_are_rejected_without_renaming_or_focus_change() {
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let env = UiEnvironment::default();
    ui.frame(&host_tree(false), env, Duration::ZERO).unwrap();
    ui.set_focus(&Key::named("composer"));
    let callbacks = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = std::rc::Rc::clone(&callbacks);
    let invalid = column()
        .child(button("Outer").key("same").on_press(Host::Outer))
        .child(map_actions(
            button("Inner").key("same").on_press(Local::Press),
            Host::Local,
        ))
        .child(
            presented(move |_| {
                observed.set(observed.get() + 1);
                Node::col()
            })
            .key("callback"),
        );
    assert!(
        matches!(ui.frame(&invalid, env, Duration::from_millis(1)), Err(UiError::DuplicateKey(k)) if k==Key::named("same"))
    );
    assert_eq!(ui.focus(), Some(&Key::named("composer")));
    assert_eq!(callbacks.get(), 0);
    assert_eq!(
        ui.handle_event(&Event::Paste("still owned".into())).actions,
        [Host::Draft("still owned".into())]
    );
}

#[test]
fn mapped_target_moves_with_paint_and_keeps_its_typed_action() {
    let tree = map_actions(
        raw(Node::text("MOUNTED", Style::new()).width(16.0).height(1.0))
            .key("mounted-action")
            .on_press(Local::Press),
        Host::Local,
    );
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let mut frame = ui
        .frame(&tree, UiEnvironment::default(), Duration::ZERO)
        .unwrap();
    let mut scene = Scene::new();
    let id = scene.add(SceneEntity::new("mount", frame.node.clone()));
    scene.entity_mut(id).unwrap().offset = (4, 2);
    frame.node = scene.to_node(&Presentation::new(), 40.0, 8.0);
    compute_layout(&mut frame.node, 40, 8).unwrap();
    let mut surface = Surface::new(40, 8);
    paint(&frame.node, &mut surface);
    assert_eq!(surface.get(4, 2).unwrap().glyph.grapheme.as_str(), "M");
    let hits = frame.hit_map(surface.area()).unwrap();
    assert_eq!(hits.hit(4, 2).unwrap().key, Key::named("mounted-action"));
    assert!(hits.hit(0, 0).is_none());
    assert_eq!(ui.focus(), Some(&Key::named("mounted-action")));
    assert_eq!(
        ui.handle_event(&event(KeyCode::Enter)).actions,
        [Host::Local(Local::Press)]
    );
}

fn sibling_mounts(reverse: bool) -> Element<Host> {
    let inner = map_actions(
        raw(Node::text("INNER", Style::new()).width(12.0).height(1.0))
            .key("inner")
            .on_press(Local::Press),
        Host::Local,
    );
    let outer = raw(Node::text("OUTER", Style::new()).width(12.0).height(1.0))
        .key("outer")
        .on_press(Host::Outer);
    let children = if reverse {
        vec![inner, outer]
    } else {
        vec![outer, inner]
    };
    row()
        .key("host-row")
        .width(40)
        .height(2)
        .gap(0)
        .padding(0)
        .children(children)
}

fn assert_sibling_paint_and_actions(frame: &mut Compiled<Host>) -> u16 {
    compute_layout(&mut frame.node, 40, 8).unwrap();
    let mut surface = Surface::new(40, 8);
    paint(&frame.node, &mut surface);
    let hits = frame.hit_map(surface.area()).unwrap();
    let mut inner_x = None;
    for (key, initial, action) in [
        ("inner", "I", Host::Local(Local::Press)),
        ("outer", "O", Host::Outer),
    ] {
        let target = hits
            .targets()
            .iter()
            .find(|t| t.key == Key::named(key))
            .unwrap();
        assert!(!target.rect.is_empty());
        assert_eq!(
            surface
                .get(target.rect.x, target.rect.y)
                .unwrap()
                .glyph
                .grapheme
                .as_str(),
            initial
        );
        let hit = hits.hit(target.rect.x, target.rect.y).unwrap();
        assert_eq!(hit.key, Key::named(key));
        assert_eq!(
            frame.interactions.get(&hit.key).unwrap().on_press,
            Some(action)
        );
        if key == "inner" {
            inner_x = Some(target.rect.x);
        }
    }
    inner_x.unwrap()
}

#[test]
fn typed_inner_and_outer_actions_have_distinct_paint_derived_targets() {
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let mut frame = ui
        .frame(
            &sibling_mounts(false),
            UiEnvironment::default(),
            Duration::ZERO,
        )
        .unwrap();
    assert_sibling_paint_and_actions(&mut frame);
    assert!(ui.set_focus(&Key::named("inner")));
    assert_eq!(
        ui.handle_event(&event(KeyCode::Enter)).actions,
        [Host::Local(Local::Press)]
    );
    assert!(ui.set_focus(&Key::named("outer")));
    assert_eq!(
        ui.handle_event(&event(KeyCode::Enter)).actions,
        [Host::Outer]
    );
}

#[test]
fn reordered_named_mounts_keep_keys_focus_and_action_identity() {
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let env = UiEnvironment::default();
    let mut before = ui
        .frame(&sibling_mounts(false), env, Duration::ZERO)
        .unwrap();
    let before_x = assert_sibling_paint_and_actions(&mut before);
    let keys: Vec<_> = before.keys.keys().cloned().collect();
    ui.set_focus(&Key::named("inner"));
    let mut after = ui
        .frame(&sibling_mounts(true), env, Duration::from_millis(1))
        .unwrap();
    let after_x = assert_sibling_paint_and_actions(&mut after);
    assert_ne!(before_x, after_x, "the named mount actually moves");
    assert_eq!(after.keys.keys().cloned().collect::<Vec<_>>(), keys);
    assert_eq!(ui.focus(), Some(&Key::named("inner")));
    assert_eq!(
        ui.handle_event(&event(KeyCode::Enter)).actions,
        [Host::Local(Local::Press)]
    );
}

#[test]
fn unit_payload_adaptation_keeps_an_existing_grammar_selection_and_receipt() {
    use gibson::ui::experience::{
        Action, Content, Destination, Experience, ExperienceRuntime, Item,
    };
    let experience = Experience::new("Work").destination(Destination::new(
        "tasks",
        "Tasks",
        Content::Collection(vec![Item::new("task", "One task").action(Action::new(
            "open",
            "Open",
            (),
        ))]),
    ));
    let mut nav = ExperienceRuntime::with_builtins(&experience);
    let env = UiEnvironment::default();
    let presented = nav.present(&experience, &env, Duration::ZERO);
    let receipt = presented.receipt;
    let before = (
        nav.state().active_key(&experience),
        nav.state().selected_key(&experience),
    );
    let tree = map_actions(presented.element, |()| Host::Local(Local::Press));
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let frame = ui.frame(&tree, env, Duration::ZERO).unwrap();
    assert!(!frame.keys.is_empty());
    assert_eq!(
        (
            nav.state().active_key(&experience),
            nav.state().selected_key(&experience)
        ),
        before
    );
    assert_eq!(receipt.active_destination, Key::named("tasks"));
    // Unit payload mapping does not invent per-item actions or take over navigation.
    assert_eq!(
        nav.handle_key(
            &experience,
            &KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())
        ),
        Some(())
    );
}
