//! Public-contract tests for the semantic facade. These deliberately exercise
//! changes across frames and the ordinary layout/paint path, not private helpers.
use gibson::ui::prelude::*;
use gibson::ui::{compile, BuildCx, ChangeKind, Key, UiError};
use gibson::{
    compute_layout, paint, BrailleCanvas, Color, Event, KeyCode, KeyEvent, KeyModifiers, Node,
    Scene, SceneEntity, Style, Surface, SurfaceFx, TextInputState,
};
use std::{sync::Arc, time::Duration};

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
fn key(value: &str) -> Key {
    Key::named(value)
}
fn event(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}
fn env() -> UiEnvironment {
    UiEnvironment {
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    }
}
fn painted(mut node: Node, width: u16, height: u16) -> Surface {
    compute_layout(&mut node, width, height).unwrap();
    let mut surface = Surface::new(width, height);
    paint(&node, &mut surface);
    surface
}
fn plain(surface: &Surface) -> String {
    (0..surface.height)
        .map(|y| {
            (0..surface.width)
                .filter_map(|x| {
                    let cell = surface.get(x, y).unwrap();
                    (!cell.is_continuation).then_some(cell.glyph.grapheme.as_str())
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn controls(order: &[&str]) -> Element<String> {
    row().responsive(60).children(
        order
            .iter()
            .map(|id| button(*id).key(*id).on_press((*id).to_string())),
    )
}

#[test]
fn keys_keep_the_focused_action_through_reorder_and_responsive_reflow() {
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime
        .frame(&controls(&["a", "b", "c"]), env(), ms(0))
        .unwrap();
    assert!(runtime.set_focus(&key("b")));
    for width in [120, 32, 80] {
        let mut resized = env();
        resized.width = width;
        runtime
            .frame(&controls(&["c", "a", "b"]), resized, ms(20))
            .unwrap();
        assert_eq!(runtime.focus(), Some(&key("b")));
        let outcome = runtime.handle_event(&event(KeyCode::Enter));
        assert!(outcome.consumed);
        assert_eq!(outcome.actions, ["b"]);
    }
}

#[test]
fn traversal_skips_disabled_controls_and_forgets_removed_targets() {
    let mut runtime = UiRuntime::new(skins::VAPOR95);
    let tree = column()
        .child(button("a").key("a").on_press(1))
        .child(button("disabled").key("b").disabled(true).on_press(2))
        .child(button("c").key("c").on_press(3));
    runtime.frame(&tree, env(), ms(0)).unwrap();
    assert_eq!(runtime.focus(), Some(&key("a")));
    assert!(!runtime.set_focus(&key("b")));
    runtime.handle_event(&event(KeyCode::Tab));
    assert_eq!(runtime.focus(), Some(&key("c")));
    runtime.handle_event(&event(KeyCode::BackTab));
    assert_eq!(runtime.focus(), Some(&key("a")));
    runtime.set_focus(&key("c"));
    runtime
        .frame(&button("replacement").key("d").on_press(4), env(), ms(50))
        .unwrap();
    assert_eq!(runtime.focus(), Some(&key("d")));
    assert_eq!(runtime.handle_event(&event(KeyCode::Enter)).actions, [4]);
    assert!(!runtime.set_focus(&key("c")));
}

#[test]
fn same_key_replacement_updates_the_action_and_focusability() {
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime
        .frame(&button("old").key("slot").on_press(1), env(), ms(0))
        .unwrap();
    runtime
        .frame(&button("new").key("slot").on_press(2), env(), ms(1))
        .unwrap();
    assert_eq!(runtime.handle_event(&event(KeyCode::Enter)).actions, [2]);
    runtime
        .frame(&text::<i32>("passive").key("slot"), env(), ms(2))
        .unwrap();
    assert_eq!(runtime.focus(), None);
    let outcome = runtime.handle_event(&event(KeyCode::Enter));
    assert!(outcome.actions.is_empty());
    assert!(!outcome.consumed);
}

#[test]
fn duplicate_keys_are_rejected_transactionally_even_across_overlays() {
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime.frame(&controls(&["a", "b"]), env(), ms(0)).unwrap();
    runtime.set_focus(&key("b"));
    let invalid =
        controls(&["duplicate"]).overlay(button("other").key("duplicate").on_press("wrong".into()));
    let error = runtime
        .frame(&invalid, env(), ms(1))
        .err()
        .expect("duplicate key accepted");
    assert_eq!(error, UiError::DuplicateKey(key("duplicate")));
    assert_eq!(runtime.focus(), Some(&key("b")));
    assert_eq!(runtime.handle_event(&event(KeyCode::Enter)).actions, ["b"]);
}

fn modal_tree(outer: bool, inner: bool) -> Element<&'static str> {
    let base = column()
        .child(button("first").key("base.first").on_press("base.first"))
        .child(button("last").key("base.last").on_press("base.last"));
    if !outer {
        return base;
    }
    let mut dialog = modal("outer")
        .key("outer")
        .on_dismiss("close.outer")
        .child(button("allow").key("outer.allow").on_press("allow"))
        .child(button("deny").key("outer.deny").on_press("deny"));
    if inner {
        dialog = dialog.overlay(
            modal("inner")
                .key("inner")
                .on_dismiss("close.inner")
                .child(button("confirm").key("inner.confirm").on_press("confirm")),
        );
    }
    base.overlay(dialog)
}

#[test]
fn nested_modal_capture_restores_each_previous_focus_without_business_state() {
    let mut runtime = UiRuntime::new(skins::SWISS_SIGNAL);
    runtime
        .frame(&modal_tree(false, false), env(), ms(0))
        .unwrap();
    runtime.set_focus(&key("base.last"));
    runtime
        .frame(&modal_tree(true, false), env(), ms(1))
        .unwrap();
    assert_eq!(runtime.modal_depth(), 1);
    assert!(!runtime.set_focus(&key("base.last")));
    runtime.set_focus(&key("outer.deny"));
    runtime
        .frame(&modal_tree(true, true), env(), ms(2))
        .unwrap();
    assert_eq!(runtime.modal_depth(), 2);
    assert_eq!(runtime.focus(), Some(&key("inner.confirm")));
    runtime.handle_event(&event(KeyCode::Tab));
    assert_eq!(runtime.focus(), Some(&key("inner.confirm")));
    assert_eq!(
        runtime.handle_event(&event(KeyCode::Esc)).actions,
        ["close.inner"]
    );
    runtime
        .frame(&modal_tree(true, false), env(), ms(3))
        .unwrap();
    assert_eq!(runtime.focus(), Some(&key("outer.deny")));
    runtime
        .frame(&modal_tree(false, false), env(), ms(4))
        .unwrap();
    assert_eq!(runtime.modal_depth(), 0);
    assert_eq!(runtime.focus(), Some(&key("base.last")));
}

#[test]
fn empty_modal_still_blocks_background_shortcuts_and_restores_focus() {
    let mut runtime = UiRuntime::new(skins::VAPOR95);
    let base = button("background").key("background").on_press(1);
    runtime.frame(&base, env(), ms(0)).unwrap();
    runtime
        .frame(
            &base
                .clone()
                .overlay(modal("busy").key("busy").child(text("Wait"))),
            env(),
            ms(1),
        )
        .unwrap();
    assert_eq!(runtime.focus(), None);
    for input in [
        event(KeyCode::Enter),
        Event::Key(KeyEvent::char('q')),
        event(KeyCode::Esc),
    ] {
        let outcome = runtime.handle_event(&input);
        assert!(outcome.consumed);
        assert!(outcome.actions.is_empty());
    }
    runtime.frame(&base, env(), ms(2)).unwrap();
    assert_eq!(runtime.focus(), Some(&key("background")));
}

#[test]
fn removing_modal_and_saved_control_chooses_a_live_replacement() {
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime
        .frame(&modal_tree(false, false), env(), ms(0))
        .unwrap();
    runtime.set_focus(&key("base.last"));
    runtime
        .frame(&modal_tree(true, true), env(), ms(1))
        .unwrap();
    runtime
        .frame(
            &button("new").key("replacement").on_press("new"),
            env(),
            ms(2),
        )
        .unwrap();
    assert_eq!(runtime.focus(), Some(&key("replacement")));
    assert_eq!(runtime.modal_depth(), 0);
    assert_eq!(
        runtime.handle_event(&event(KeyCode::Enter)).actions,
        ["new"]
    );
}

#[derive(Clone, Debug)]
enum Edit {
    Value(TextInputState),
    Submit,
}
#[test]
fn unicode_edit_actions_are_owned_by_the_application_and_submission_is_typed() {
    let mut input = TextInputState::with_text("e\u{301}界");
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    let make = |state: &TextInputState| {
        text_input(state)
            .key("prompt")
            .on_edit(Edit::Value)
            .on_press(Edit::Submit)
    };
    runtime.frame(&make(&input), env(), ms(0)).unwrap();
    let outcome = runtime.handle_event(&event(KeyCode::Backspace));
    assert!(outcome.consumed);
    assert_eq!(input.text, "e\u{301}界", "runtime mutated app state");
    let Edit::Value(next) = outcome.actions.into_iter().next().unwrap() else {
        panic!("expected edit")
    };
    input = next;
    assert_eq!(input.text, "e\u{301}");
    assert_eq!(input.cursor_grapheme, 1);
    runtime.frame(&make(&input), env(), ms(1)).unwrap();
    let outcome = runtime.handle_event(&Event::Paste("\r\n👩‍🔬".into()));
    let Edit::Value(next) = outcome.actions.into_iter().next().unwrap() else {
        panic!("expected paste")
    };
    assert_eq!(next.text, "e\u{301} 👩‍🔬");
    assert_eq!(next.cursor_grapheme, 3);
    runtime.frame(&make(&next), env(), ms(2)).unwrap();
    assert!(matches!(
        runtime
            .handle_event(&event(KeyCode::Enter))
            .actions
            .as_slice(),
        [Edit::Submit]
    ));
}

#[test]
fn selection_is_controlled_and_never_mutated_by_activation() {
    let tree = choice("one", false).key("choice").on_press(1);
    let mut runtime = UiRuntime::new(skins::VAPOR95);
    let before = runtime.frame(&tree, env(), ms(0)).unwrap();
    assert_eq!(runtime.handle_event(&event(KeyCode::Enter)).actions, [1]);
    let after = runtime.frame(&tree, env(), ms(1)).unwrap();
    assert_eq!(painted(before.node, 80, 24), painted(after.node, 80, 24));
    let selected = runtime
        .frame(&choice("one", true).key("choice").on_press(1), env(), ms(2))
        .unwrap();
    assert_ne!(
        painted(selected.node, 80, 24),
        painted(
            compile(&tree, &BuildCx::new(skins::VAPOR95, env()))
                .unwrap()
                .node,
            80,
            24
        )
    );
    assert!(runtime
        .changes()
        .iter()
        .any(|change| change.key == key("choice") && change.kind == ChangeKind::Change));
}

#[test]
fn custom_control_receives_unhandled_focused_events_only() {
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    let tree = column()
        .child(
            viewport(0, 0)
                .key("viewport")
                .on_event(std::convert::identity)
                .child(text("custom viewport")),
        )
        .child(button("other").key("other").on_press(Event::Tick));
    runtime.frame(&tree, env(), ms(0)).unwrap();
    assert!(runtime.set_focus(&key("viewport")));
    assert_eq!(
        runtime.handle_event(&event(KeyCode::PageDown)).actions,
        [event(KeyCode::PageDown)]
    );
    runtime.handle_event(&event(KeyCode::Tab));
    assert!(runtime
        .handle_event(&event(KeyCode::PageDown))
        .actions
        .is_empty());
}

#[test]
fn deterministic_motion_settles_to_the_exact_no_motion_surface() {
    let tree: Element<()> = screen()
        .child(heading("Stable final state"))
        .child(
            panel("Instrument")
                .key("instrument")
                .child(status("Ready").tone(Tone::Success)),
        )
        .child(button("go").key("go").on_press(()));
    for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
        let settled = UiRuntime::new(skin).frame(&tree, env(), ms(0)).unwrap();
        let expected = painted(settled.node, 80, 24);
        for preference in [MotionPreference::Full, MotionPreference::Reduced] {
            let mut animated_env = env();
            animated_env.motion = preference;
            let mut left = UiRuntime::new(skin);
            let mut right = UiRuntime::new(skin);
            for time in [0, 37, 97, 350, 5000] {
                let a = left.frame(&tree, animated_env, ms(time)).unwrap();
                let b = right.frame(&tree, animated_env, ms(time)).unwrap();
                assert_eq!(painted(a.node, 80, 24), painted(b.node, 80, 24));
            }
            let final_frame = left.frame(&tree, animated_env, ms(5000)).unwrap();
            assert_eq!(painted(final_frame.node, 80, 24), expected);
            assert_eq!(left.active_animation_count(), 0);
        }
    }
}

#[test]
fn removed_keys_and_finite_animation_state_do_not_accumulate_under_churn() {
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    let mut animated_env = env();
    animated_env.motion = MotionPreference::Full;
    for index in 0..300 {
        let tree = button("replace")
            .key(format!("key.{index}"))
            .on_press(index);
        runtime
            .frame(&tree, animated_env, ms(index as u64 * 5))
            .unwrap();
        assert!(runtime.retained_key_count() <= 1);
        assert!(runtime.active_animation_count() <= 1);
    }
    runtime
        .frame(&column::<i32>(), animated_env, ms(2000))
        .unwrap();
    assert_eq!(runtime.focus(), None);
    assert_eq!(runtime.active_animation_count(), 0);
    assert!(
        runtime.retained_key_count() <= 1,
        "only the current empty root may remain"
    );
    assert!(runtime
        .handle_event(&event(KeyCode::Enter))
        .actions
        .is_empty());
}

#[test]
fn raw_node_surface_canvas_and_scene_keep_the_existing_composition_path() {
    let cx = BuildCx::new(skins::BLACK_ICE, env());
    let custom = Node::col()
        .width(12.0)
        .height(2.0)
        .child(Node::text("raw界", Style::new().bold()))
        .child(Node::text("second", Style::new()));
    let compiled = compile(&raw::<()>(custom.clone()), &cx).unwrap();
    assert_eq!(painted(compiled.node, 20, 4), painted(custom, 20, 4));

    let mut canvas = BrailleCanvas::new(8, 2);
    canvas.line(0, 0, 15, 7);
    let original = canvas.to_surface(Style::new().fg(Color::Cyan));
    let via_surface = compile(&surface::<()>(Arc::new(original.clone())), &cx).unwrap();
    let via_raster = compile(&raster::<()>(original.clone()), &cx).unwrap();
    assert_eq!(painted(via_surface.node, 8, 2), original);
    assert_eq!(painted(via_raster.node, 8, 2), original);

    let mut scene = Scene::new();
    scene.add(
        SceneEntity::new(
            "alien",
            Node::text("ALIEN", Style::new()).width(5.0).height(1.0),
        )
        .offset(3, 1),
    );
    let scene_node = scene.to_node(&gibson::Presentation::new(), 12.0, 4.0);
    let embedded = compile(&raw::<()>(scene_node.clone()), &cx).unwrap();
    assert_eq!(painted(embedded.node, 12, 4), painted(scene_node, 12, 4));
}

#[test]
fn local_post_processing_preserves_layout_glyphs_and_neighbor_styles() {
    let cx = BuildCx::new(skins::BLACK_ICE, env());
    let tree: Element<()> = column()
        .gap(0)
        .child(
            raw(Node::text("LOCAL", Style::new()).height(1.0)).post_process([SurfaceFx::Reverse]),
        )
        .child(raw(Node::text("NEIGHBOR", Style::new()).height(1.0)));
    let frame = painted(compile(&tree, &cx).unwrap().node, 20, 4);
    assert!(plain(&frame).contains("LOCAL"));
    assert!(plain(&frame).contains("NEIGHBOR"));
    for cell in &frame.cells {
        if cell.glyph.grapheme == "L" {
            assert!(cell.style.reverse);
        }
        if cell.glyph.grapheme == "N" {
            assert!(!cell.style.reverse);
        }
    }
}

struct UnexpectedComponent;
impl Component<()> for UnexpectedComponent {
    fn build(self, _cx: &BuildCx) -> Element<()> {
        raw(Node::viewport(2, 0)
            .width(6.0)
            .height(1.0)
            .child(Node::text("0123456789", Style::new()).width(10.0)))
    }
}
#[test]
fn custom_component_can_embed_a_clipped_substrate_viewport() {
    let cx = BuildCx::new(skins::SWISS_SIGNAL, env());
    let tree = column()
        .gap(0)
        .child(component(UnexpectedComponent, &cx))
        .child(text("semantic neighbor"));
    let result = painted(compile(&tree, &cx).unwrap().node, 24, 6);
    assert!(plain(&result).contains("234567"));
    assert!(plain(&result).contains("semantic neighbor"));
}

#[test]
fn settled_panel_editor_keeps_one_cursor_and_inactive_editor_cannot_steal_it() {
    for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
        let tree = panel("Editors")
            .child(
                text_input(&TextInputState::with_text("FIRST"))
                    .key("first")
                    .on_edit(Edit::Value),
            )
            .child(
                text_input(&TextInputState::with_text("SECOND"))
                    .key("second")
                    .on_edit(Edit::Value),
            );
        let mut runtime = UiRuntime::new(skin);
        let mut environment = env();
        environment.motion = MotionPreference::Full;
        runtime.frame(&tree, environment, ms(0)).unwrap();
        let mut compiled = runtime.frame(&tree, environment, ms(1000)).unwrap();
        compute_layout(&mut compiled.node, 80, 24).unwrap();
        let mut surface = Surface::new(80, 24);
        let first_cursor = paint(&compiled.node, &mut surface)
            .cursor_position
            .expect("settled panel lost cursor");
        let first_row = (0..80)
            .filter_map(|x| {
                surface
                    .get(x, first_cursor.1)
                    .map(|c| c.glyph.grapheme.as_str())
            })
            .collect::<String>();
        assert!(
            first_row.contains("FIRST"),
            "inactive second editor stole cursor"
        );
        runtime.set_focus(&key("second"));
        runtime.frame(&tree, environment, ms(1001)).unwrap();
        let mut compiled = runtime.frame(&tree, environment, ms(2000)).unwrap();
        compute_layout(&mut compiled.node, 80, 24).unwrap();
        let mut surface = Surface::new(80, 24);
        let second_cursor = paint(&compiled.node, &mut surface)
            .cursor_position
            .expect("second editor lost cursor");
        assert!(second_cursor.1 > first_cursor.1);
        let second_row = (0..80)
            .filter_map(|x| {
                surface
                    .get(x, second_cursor.1)
                    .map(|c| c.glyph.grapheme.as_str())
            })
            .collect::<String>();
        assert!(second_row.contains("SECOND"));
    }
}

#[test]
fn excessive_trees_are_rejected_before_lowering() {
    let cx = BuildCx::new(skins::BLACK_ICE, env());
    let mut deep: Element<()> = text("leaf");
    for _ in 0..130 {
        deep = column().child(deep);
    }
    assert!(matches!(compile(&deep, &cx), Err(UiError::TooDeep { .. })));
    let broad: Element<()> = column().children((0..16385).map(|_| text("item")));
    assert!(matches!(
        compile(&broad, &cx),
        Err(UiError::TooManyElements { .. })
    ));
}

#[test]
fn custom_spacing_tokens_control_actual_geometry_and_builder_override_wins() {
    let mut skin = skins::BLACK_ICE;
    skin.spacing.compact_gap = 3;
    skin.spacing.normal_gap = 4;
    skin.spacing.spacious_gap = 5;
    for (density, gap) in [
        (Density::Compact, 3),
        (Density::Normal, 4),
        (Density::Spacious, 5),
    ] {
        let tree: Element<()> = column()
            .density(density)
            .child(label("A"))
            .child(label("B"));
        let mut compiled = compile(&tree, &BuildCx::new(skin, env())).unwrap();
        compute_layout(&mut compiled.node, 80, 24).unwrap();
        let children = &compiled.node.children;
        assert_eq!(
            children[1].computed_rect.y - children[0].computed_rect.y,
            1 + gap
        );
        let mut overridden = compile(&tree.gap(1), &BuildCx::new(skin, env())).unwrap();
        compute_layout(&mut overridden.node, 80, 24).unwrap();
        assert_eq!(
            overridden.node.children[1].computed_rect.y
                - overridden.node.children[0].computed_rect.y,
            2
        );
    }
}

#[test]
fn leaf_children_cannot_create_invisible_actions() {
    let cx = BuildCx::new(skins::BLACK_ICE, env());
    for leaf in [
        label("VISIBLE"),
        text("VISIBLE"),
        heading("VISIBLE"),
        status("VISIBLE"),
        badge("VISIBLE"),
        button("VISIBLE"),
        code("VISIBLE"),
        progress("VISIBLE", 0.5),
        sparkline(&[1.0]),
        toast("VISIBLE"),
        raw(Node::text("VISIBLE", Style::new())),
    ] {
        let invalid = leaf
            .key("leaf")
            .child(button("INVISIBLE").key("hidden").on_press(42));
        assert!(matches!(
            compile(&invalid, &cx),
            Err(UiError::InvalidChildren(_))
        ));
    }
    let too_many = viewport(0, 0)
        .child(button("one").on_press(1))
        .child(button("two").on_press(2));
    assert!(matches!(
        compile(&too_many, &cx),
        Err(UiError::InvalidViewport(_))
    ));
}

#[test]
fn adding_a_local_overlay_preserves_fixed_and_growing_layout_constraints() {
    let cx = BuildCx::new(skins::BLACK_ICE, env());
    for grow in [false, true] {
        let host: Element<()> = if grow {
            panel("HOST").grow(1.0).height(8)
        } else {
            panel("HOST").width(20).height(8)
        };
        let neighbor = if grow {
            panel("NEIGHBOR").grow(1.0).height(8)
        } else {
            panel("NEIGHBOR").width(20).height(8)
        };
        let make = |host| row().gap(0).child(host).child(neighbor.clone());
        let mut before = compile(&make(host.clone()), &cx).unwrap().node;
        let mut after = compile(&make(host.overlay(label("FLOAT"))), &cx)
            .unwrap()
            .node;
        compute_layout(&mut before, 80, 24).unwrap();
        compute_layout(&mut after, 80, 24).unwrap();
        assert_eq!(before.computed_rect, after.computed_rect);
        assert_eq!(
            before.children[0].computed_rect,
            after.children[0].computed_rect
        );
        assert_eq!(
            before.children[1].computed_rect,
            after.children[1].computed_rect
        );
        let capture = plain(&painted(after, 80, 24));
        assert!(
            capture.contains("NEIGHBOR"),
            "grow={grow}, capture={capture}"
        );
    }
}

#[test]
fn local_modal_stays_inside_host_and_cannot_cover_neighbor() {
    let env = env();
    let tree = row()
        .gap(0)
        .child(
            panel("HOST").width(30).height(12).overlay(
                modal("LOCAL")
                    .width(22)
                    .height(8)
                    .child(button("Inside").key("inside").on_press(())),
            ),
        )
        .child(
            panel("NEIGHBOR")
                .width(20)
                .height(12)
                .child(text("untouched")),
        );
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    let compiled = runtime.frame(&tree, env, ms(0)).unwrap();
    let surface = painted(compiled.node, 80, 24);
    assert!(plain(&surface).contains("Inside"));
    assert!(plain(&surface).contains("NEIGHBOR"));
    assert!(plain(&surface).contains("untouched"));
    for row in plain(&surface).lines() {
        if let Some(index) = row.find("Inside") {
            assert!(
                unicode_width::UnicodeWidthStr::width(&row[..index]) < 30,
                "local modal escaped host"
            );
        }
    }
    assert_eq!(runtime.focus(), Some(&key("inside")));
}

#[test]
fn grow_weights_share_row_width_without_intrinsic_content_bias() {
    let tree = row::<()>()
        .responsive(60)
        .child(
            panel("Very long title that would otherwise bias flex sizing")
                .grow(1.0)
                .child(text("a")),
        )
        .child(panel("Short").grow(1.0).child(text("b")));
    let mut node = compile(&tree, &BuildCx::new(skins::VAPOR95, env()))
        .unwrap()
        .node;
    compute_layout(&mut node, 80, 24).unwrap();
    assert!(
        node.children[0]
            .computed_rect
            .width
            .abs_diff(node.children[1].computed_rect.width)
            <= 1
    );
    let narrow = UiEnvironment { width: 32, ..env() };
    let mut node = compile(&tree, &BuildCx::new(skins::VAPOR95, narrow))
        .unwrap()
        .node;
    compute_layout(&mut node, 32, 24).unwrap();
    assert!(node.children.iter().all(|n| n.computed_rect.width > 0));
}
