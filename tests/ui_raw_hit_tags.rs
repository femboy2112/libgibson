//! Imported ordinary nodes do not carry another compilation's hit-target domain.
use gibson::scene::{Presentation, Scene, SceneEntity};
use gibson::ui::prelude::*;
use gibson::{compute_layout, paint, Node, Style, Surface};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Inner,
    Outer,
    Wrapper,
}

fn control(label: &str, key: &str, action: Action) -> Element<Action> {
    raw(Node::text(label, Style::new()).width(24.0).height(1.0))
        .key(key)
        .on_press(action)
}

fn compile_tree(tree: Element<Action>) -> Compiled<Action> {
    compile(
        &tree,
        &BuildCx::new(skins::SWISS_SIGNAL, UiEnvironment::default()),
    )
    .unwrap()
}

fn outer(import: Element<Action>) -> Compiled<Action> {
    compile_tree(
        column()
            .gap(0)
            .padding(0)
            .child(control("OUTER", "outer", Action::Outer))
            .child(import),
    )
}

fn paint_frame(frame: &mut Compiled<Action>) -> Surface {
    compute_layout(&mut frame.node, 40, 8).unwrap();
    let mut surface = Surface::new(40, 8);
    paint(&frame.node, &mut surface);
    surface
}

fn label_cell(surface: &Surface, label: &str) -> (u16, u16) {
    for y in 0..surface.height {
        let row: String = (0..surface.width)
            .map(|x| surface.get(x, y).unwrap().glyph.grapheme.as_str())
            .collect();
        // All fixture glyphs are single-cell ASCII.
        if let Some(x) = row.find(label) {
            return (x as u16, y);
        }
    }
    panic!("fixture label {label:?} did not paint");
}

fn assert_import_inert(mut frame: Compiled<Action>, labels: &[&str]) {
    let surface = paint_frame(&mut frame);
    let hits = frame.hit_map(surface.area()).unwrap();
    for label in labels {
        let (x, y) = label_cell(&surface, label);
        assert!(
            hits.hit(x, y).is_none(),
            "imported label {label} must resolve to neither inner nor outer action: {:?}",
            hits.hit(x, y)
        );
    }
    let (x, y) = label_cell(&surface, "OUTER");
    assert_eq!(hits.hit(x, y).unwrap().key, Key::named("outer"));
    assert_eq!(frame.targets.len(), 1);
    assert_eq!(frame.interactions.entries.len(), 1);
}

#[test]
fn raw_foreign_tag_cannot_alias_an_outer_action() {
    let inner = compile_tree(control("INNER", "inner", Action::Inner));
    assert_eq!(inner.node.tag, Some(0));
    assert_import_inert(outer(raw(inner.node)), &["INNER"]);
}

#[test]
fn out_of_range_hand_authored_tag_is_inert_instead_of_panicking() {
    let mut node = Node::text("MANUAL", Style::new()).width(24.0).height(1.0);
    node.tag = Some(u32::MAX);
    assert_import_inert(outer(raw(node)), &["MANUAL"]);
}

#[test]
fn nested_foreign_tags_are_cleared_through_the_entire_raw_subtree() {
    let inner = compile_tree(
        column()
            .gap(0)
            .padding(0)
            .child(control("NESTED_A", "inner-a", Action::Inner))
            .child(
                column()
                    .gap(0)
                    .padding(0)
                    .child(control("NESTED_B", "inner-b", Action::Inner)),
            ),
    );
    assert_eq!(inner.targets.len(), 2);
    assert_import_inert(outer(raw(inner.node)), &["NESTED_A", "NESTED_B"]);
}

#[test]
fn raw_wrapper_action_receives_a_fresh_host_tag() {
    let inner = compile_tree(control("WRAPPED", "inner", Action::Inner));
    let mut frame = outer(raw(inner.node).key("wrapper").on_press(Action::Wrapper));
    let surface = paint_frame(&mut frame);
    let (x, y) = label_cell(&surface, "WRAPPED");
    let hits = frame.hit_map(surface.area()).unwrap();
    assert_eq!(hits.hit(x, y).unwrap().key, Key::named("wrapper"));
    assert_eq!(
        frame
            .interactions
            .get(&Key::named("wrapper"))
            .unwrap()
            .on_press,
        Some(Action::Wrapper)
    );
    assert!(!frame.keys.contains_key(&Key::named("inner")));
}

#[test]
fn presented_callback_cannot_import_foreign_hit_tags() {
    let inner = compile_tree(control("CALLBACK", "inner", Action::Inner));
    assert_import_inert(outer(presented(move |_| inner.node.clone())), &["CALLBACK"]);
}

#[test]
fn presented_wrapper_action_survives_tag_sanitization() {
    let inner = compile_tree(control("CALLBACK_WRAPPED", "inner", Action::Inner));
    let mut frame = outer(
        presented(move |_| inner.node.clone())
            .key("wrapper")
            .on_press(Action::Wrapper),
    );
    let surface = paint_frame(&mut frame);
    let (x, y) = label_cell(&surface, "CALLBACK_WRAPPED");
    let hits = frame.hit_map(surface.area()).unwrap();
    assert_eq!(hits.hit(x, y).unwrap().key, Key::named("wrapper"));
}

#[test]
fn hand_authored_in_range_descendant_cannot_override_wrapper_action() {
    let mut child = Node::text("MANUAL_CHILD", Style::new())
        .width(24.0)
        .height(1.0);
    child.tag = Some(0);
    let node = Node::col().child(child);
    let mut frame = outer(raw(node).key("wrapper").on_press(Action::Wrapper));
    let surface = paint_frame(&mut frame);
    let (x, y) = label_cell(&surface, "MANUAL_CHILD");
    assert_eq!(
        frame
            .hit_map(surface.area())
            .unwrap()
            .hit(x, y)
            .unwrap()
            .key,
        Key::named("wrapper")
    );
}

#[test]
fn scene_translation_preserves_tags_with_their_original_compiled_sidecars() {
    let mut frame = compile_tree(control("SAME_DOMAIN", "inner", Action::Inner));
    let original_tag = frame.node.tag;
    let mut scene = Scene::new();
    let id = scene.add(SceneEntity::new("semantic", frame.node.clone()));
    scene.entity_mut(id).unwrap().offset = (3, 2);
    frame.node = scene.to_node(&Presentation::new(), 40.0, 8.0);
    let surface = paint_frame(&mut frame);
    let (x, y) = label_cell(&surface, "SAME_DOMAIN");
    assert_eq!((x, y), (3, 2));
    assert_eq!(original_tag, Some(0));
    let hits = frame.hit_map(surface.area()).unwrap();
    assert_eq!(hits.hit(x, y).unwrap().key, Key::named("inner"));
    assert!(hits.hit(0, 0).is_none());
    assert_eq!(frame.interactions.entries[0].on_press, Some(Action::Inner));
}
