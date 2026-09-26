//! Behavioral probes of control appearance, event ownership, and live lowering.
//! The oracles observe rendered cells/actions rather than effect implementation.
use gibson::ui::prelude::*;
use gibson::ui::{compile_presented, Key, PresentationCx};
use gibson::{
    compute_layout, paint, Cell, Color, ColorDepth, Event, KeyCode, KeyEvent, KeyModifiers, Node,
    Rect, Style, Surface, SurfaceFx, TextInputState,
};
use std::time::Duration;

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
fn key(value: &str) -> Key {
    Key::named(value)
}
fn input(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}
fn environment(depth: ColorDepth, motion: MotionPreference) -> UiEnvironment {
    UiEnvironment {
        color_depth: depth,
        motion,
        ..UiEnvironment::default()
    }
}
fn paint_node(mut node: Node, env: UiEnvironment) -> Surface {
    compute_layout(&mut node, env.width, env.height).unwrap();
    let mut surface = Surface::new(env.width, env.height);
    paint(&node, &mut surface);
    surface
}
fn text_of(surface: &Surface) -> String {
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
fn cells_in(surface: &Surface, rect: Rect) -> Vec<Cell> {
    let mut cells = Vec::new();
    for y in rect.y..rect.y.saturating_add(rect.height).min(surface.height) {
        for x in rect.x..rect.x.saturating_add(rect.width).min(surface.width) {
            cells.push(surface.get(x, y).unwrap().clone());
        }
    }
    cells
}
fn control_tree(selected: bool, disabled: bool) -> Element<u8> {
    column()
        .gap(0)
        .child(button("Anchor").key("anchor").on_press(0))
        .child(
            choice("IDENTICAL LABEL", selected)
                .key("target")
                .disabled(disabled)
                .on_press(1)
                .width(28),
        )
}
fn control_frame(
    runtime: &mut UiRuntime<u8>,
    tree: &Element<u8>,
    env: UiEnvironment,
    time: Duration,
) -> (Surface, Vec<Cell>) {
    let mut node = runtime.frame(tree, env, time).unwrap().node;
    compute_layout(&mut node, env.width, env.height).unwrap();
    let rect = node.children[1].computed_rect;
    let mut surface = Surface::new(env.width, env.height);
    paint(&node, &mut surface);
    let cells = cells_in(&surface, rect);
    assert!(cells
        .iter()
        .map(|cell| cell.glyph.grapheme.as_str())
        .collect::<String>()
        .contains("IDENTICAL LABEL"));
    (surface, cells)
}

#[test]
fn persistent_control_states_remain_distinct_in_mono_and_ansi16() {
    for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
        for depth in [ColorDepth::Mono, ColorDepth::Ansi16] {
            let env = environment(depth, MotionPreference::None);
            let mut signatures = Vec::new();
            for (focused, selected, disabled) in [
                (false, false, false),
                (true, false, false),
                (false, true, false),
                (true, true, false),
                (false, false, true),
            ] {
                let tree = control_tree(selected, disabled);
                let mut runtime = UiRuntime::new(skin);
                runtime.frame(&tree, env, ms(0)).unwrap();
                assert!(runtime.set_focus(&key(if focused { "target" } else { "anchor" })));
                let (surface, cells) = control_frame(&mut runtime, &tree, env, ms(1));
                if depth == ColorDepth::Mono {
                    assert!(surface.cells.iter().all(|cell| matches!(
                        cell.style.fg,
                        None | Some(Color::Reset)
                    ) && matches!(
                        cell.style.bg,
                        None | Some(Color::Reset)
                    )));
                } else {
                    assert!(surface.cells.iter().all(|cell| !matches!(
                        cell.style.fg,
                        Some(Color::Rgb(..) | Color::Ansi256(_))
                    ) && !matches!(
                        cell.style.bg,
                        Some(Color::Rgb(..) | Color::Ansi256(_))
                    )));
                }
                if disabled {
                    assert!(!runtime.set_focus(&key("target")));
                }
                signatures.push(cells);
            }
            for left in 0..signatures.len() {
                for right in left + 1..signatures.len() {
                    assert_ne!(
                        signatures[left], signatures[right],
                        "{} {depth:?}: states {left} and {right} became indistinguishable",
                        skin.name
                    );
                }
            }
        }
    }
}

#[test]
fn activation_is_visible_but_finite_and_reduced_preserves_all_glyphs() {
    for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
        for depth in [ColorDepth::Mono, ColorDepth::Ansi16] {
            for preference in [MotionPreference::Full, MotionPreference::Reduced] {
                for selected in [false, true] {
                    let env = environment(depth, preference);
                    let tree = control_tree(selected, false);
                    let mut runtime = UiRuntime::new(skin);
                    runtime.frame(&tree, env, ms(0)).unwrap();
                    runtime.set_focus(&key("target"));
                    runtime.frame(&tree, env, ms(1)).unwrap();
                    let (settled, before) = control_frame(&mut runtime, &tree, env, ms(1000));
                    let outcome = runtime.handle_event(&input(KeyCode::Enter));
                    assert!(outcome.consumed);
                    assert_eq!(outcome.actions, [1]);
                    let (active, during) = control_frame(&mut runtime, &tree, env, ms(1001));
                    assert!(
                    before != during,
                    "{} {depth:?} {preference:?} selected={selected}: activation has no visible feedback",
                    skin.name
                );
                    if preference == MotionPreference::Reduced {
                        assert_eq!(
                            text_of(&active),
                            text_of(&settled),
                            "reduced motion moved or hid information"
                        );
                    }
                    let (after, _) = control_frame(&mut runtime, &tree, env, ms(3000));
                    assert_eq!(after, settled, "activation left persistent styling behind");
                    assert_eq!(runtime.active_animation_count(), 0);
                }
            }
        }
    }
}

#[test]
fn switching_skins_clears_old_feedback_and_preserves_controlled_selection_and_action() {
    let tree = control_tree(true, false);
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    let env = environment(ColorDepth::Mono, MotionPreference::Full);
    runtime.frame(&tree, env, ms(0)).unwrap();
    runtime.set_focus(&key("target"));
    runtime.frame(&tree, env, ms(1)).unwrap();
    for (index, skin) in [skins::VAPOR95, skins::SWISS_SIGNAL, skins::BLACK_ICE]
        .into_iter()
        .cycle()
        .take(30)
        .enumerate()
    {
        assert_eq!(runtime.handle_event(&input(KeyCode::Enter)).actions, [1]);
        let time = ms(1000 + index as u64 * 10);
        runtime.frame(&tree, env, time).unwrap();
        runtime.set_skin(skin);
        let (actual, _) = control_frame(&mut runtime, &tree, env, time);
        let mut reference = UiRuntime::new(skin);
        let immediate = environment(ColorDepth::Mono, MotionPreference::None);
        reference.frame(&tree, immediate, ms(0)).unwrap();
        reference.set_focus(&key("target"));
        let (expected, _) = control_frame(&mut reference, &tree, immediate, ms(1));
        assert_eq!(
            actual, expected,
            "previous skin leaked effects into new grammar"
        );
        assert_eq!(runtime.focus(), Some(&key("target")));
        assert_eq!(runtime.active_animation_count(), 0);
        assert_eq!(runtime.retained_key_count(), 3);
    }
}

#[test]
fn handlerless_and_submit_only_editors_own_edit_events_without_mutating_model() {
    let state = TextInputState::with_text("UNCHANGED界");
    for submit in [false, true] {
        let editor = text_input::<String>(&state).key("editor");
        let tree = if submit {
            editor.on_press("submit".into())
        } else {
            editor
        };
        let mut runtime = UiRuntime::new(skins::BLACK_ICE);
        runtime
            .frame(
                &tree,
                environment(ColorDepth::Mono, MotionPreference::None),
                ms(0),
            )
            .unwrap();
        assert_eq!(runtime.focus(), Some(&key("editor")));
        for event in [
            input(KeyCode::Char('x')),
            Event::Paste("e\u{301}\r\n👩‍🔬".into()),
            input(KeyCode::Backspace),
            input(KeyCode::Delete),
            input(KeyCode::Left),
            input(KeyCode::Right),
            input(KeyCode::Up),
            input(KeyCode::Down),
            input(KeyCode::Home),
            input(KeyCode::End),
        ] {
            let routed = runtime.handle_event(&event);
            assert!(routed.consumed, "editor leaked {event:?}");
            assert!(
                routed.actions.is_empty(),
                "read-only controlled editor fabricated application action"
            );
            assert_eq!(state.text, "UNCHANGED界");
        }
        let unsupported = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(!runtime.handle_event(&unsupported).consumed);
        let submit_result = runtime.handle_event(&input(KeyCode::Enter));
        assert_eq!(submit_result.actions.len(), usize::from(submit));
        let compiled = runtime
            .frame(
                &tree,
                environment(ColorDepth::Mono, MotionPreference::None),
                ms(1),
            )
            .unwrap();
        assert!(text_of(&paint_node(
            compiled.node,
            environment(ColorDepth::Mono, MotionPreference::None)
        ))
        .contains("UNCHANGED界"));
    }
}

#[test]
fn editor_callbacks_cannot_steal_edit_keys_but_can_bind_unsupported_shortcuts() {
    let env = environment(ColorDepth::Mono, MotionPreference::None);
    let tree = text_input::<Event>(&TextInputState::new())
        .key("editor")
        .on_event(std::convert::identity);
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime.frame(&tree, env, ms(0)).unwrap();
    for event in [
        input(KeyCode::Char('x')),
        Event::Paste("text".into()),
        input(KeyCode::Backspace),
        input(KeyCode::Up),
        input(KeyCode::Down),
    ] {
        let routed = runtime.handle_event(&event);
        assert!(routed.consumed);
        assert!(
            routed.actions.is_empty(),
            "fallback callback stole edit ownership"
        );
    }
    let shortcut = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert_eq!(runtime.handle_event(&shortcut).actions, [shortcut]);
}

fn probe_node(cx: &PresentationCx) -> Node {
    let focus = cx
        .focused
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "none".into());
    Node::text(format!("observed:{focus}"), Style::new()).height(1.0)
}
fn phase_view(modal_open: bool) -> Element<u8> {
    let base = column()
        .gap(0)
        .width(80)
        .height(24)
        .child(button("first").key("first").on_press(1))
        .child(button("second").key("second").on_press(2))
        .child(presented(probe_node));
    if modal_open {
        base.overlay(
            modal("scope")
                .key("dialog")
                .child(button("inside").key("inside").on_press(3))
                .child(presented(probe_node)),
        )
    } else {
        base
    }
}

#[test]
fn presented_custom_nodes_observe_current_reconciled_focus_in_the_rendered_frame() {
    let env = environment(ColorDepth::Mono, MotionPreference::None);
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    let check = |runtime: &mut UiRuntime<u8>, modal: bool, time: u64, expected: &str| {
        let frame = runtime.frame(&phase_view(modal), env, ms(time)).unwrap();
        assert!(text_of(&paint_node(frame.node, env)).contains(&format!("observed:{expected}")));
    };
    check(&mut runtime, false, 0, "first");
    runtime.handle_event(&input(KeyCode::Tab));
    check(&mut runtime, false, 1, "second");
    check(&mut runtime, true, 2, "inside");
    assert_eq!(runtime.handle_event(&input(KeyCode::Enter)).actions, [3]);
    check(&mut runtime, false, 3, "second");
    let changed = column()
        .gap(0)
        .child(button("replacement").key("new").on_press(4))
        .child(presented(probe_node));
    let frame = runtime.frame(&changed, env, ms(4)).unwrap();
    assert!(text_of(&paint_node(frame.node, env)).contains("observed:new"));
    assert_eq!(runtime.handle_event(&input(KeyCode::Enter)).actions, [4]);
}

#[test]
fn explicit_presentation_compilation_and_plain_compilation_have_distinct_phase_contracts() {
    let env = environment(ColorDepth::Mono, MotionPreference::None);
    let tree = phase_view(false);
    let build = BuildCx::new(skins::SWISS_SIGNAL, env);
    let plain = compile(&tree, &build).unwrap();
    assert!(text_of(&paint_node(plain.node.clone(), env)).contains("observed:none"));
    let mut presentation = PresentationCx::new(build);
    let unselected = compile_presented(&tree, &presentation).unwrap();
    assert_eq!(
        paint_node(plain.node, env),
        paint_node(unselected.node, env)
    );
    presentation.focused = Some(key("second"));
    let selected = compile_presented(&tree, &presentation).unwrap();
    assert!(text_of(&paint_node(selected.node, env)).contains("observed:second"));
}

#[test]
fn presented_custom_nodes_observe_current_skin_geometry_and_clamped_clock() {
    let tree: Element<()> = presented(|cx| {
        Node::text(
            format!(
                "{} {}@{}",
                cx.build.skin.name,
                cx.build.environment.width,
                cx.build.time.as_millis()
            ),
            Style::new(),
        )
        .height(1.0)
    });
    let mut runtime = UiRuntime::new(skins::VAPOR95);
    let mut env = environment(ColorDepth::Mono, MotionPreference::None);
    let first = runtime.frame(&tree, env, ms(25)).unwrap();
    assert!(text_of(&paint_node(first.node, env)).contains("Vapor95 80@25"));
    runtime.set_skin(skins::SWISS_SIGNAL);
    env.width = 36;
    let second = runtime.frame(&tree, env, ms(150)).unwrap();
    assert!(text_of(&paint_node(second.node, env)).contains("Swiss Signal 36@150"));
    let earlier = runtime.frame(&tree, env, ms(130)).unwrap();
    assert!(text_of(&paint_node(earlier.node, env)).contains("Swiss Signal 36@150"));
}

#[test]
fn presented_custom_content_receives_the_same_automatic_motion_as_raw_content() {
    let primitive = || Node::text("CUSTOM界", Style::new()).width(20.0).height(2.0);
    let raw_tree: Element<()> = raw(primitive()).key("visual");
    let custom_tree: Element<()> = presented(move |_| primitive()).key("visual");
    for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
        let none = environment(ColorDepth::Mono, MotionPreference::None);
        let expected = compile(&raw_tree, &BuildCx::new(skin, none)).unwrap();
        let expected = paint_node(expected.node, none);
        for preference in [MotionPreference::Full, MotionPreference::Reduced] {
            let env = environment(ColorDepth::Mono, preference);
            let mut raw_runtime = UiRuntime::new(skin);
            let mut custom_runtime = UiRuntime::new(skin);
            for time in [0, 20, 80, 1000] {
                let ordinary = raw_runtime.frame(&raw_tree, env, ms(time)).unwrap();
                let custom = custom_runtime.frame(&custom_tree, env, ms(time)).unwrap();
                let ordinary = paint_node(ordinary.node, env);
                let custom = paint_node(custom.node, env);
                assert_eq!(
                    custom, ordinary,
                    "custom boundary changed motion at t={time}"
                );
                if time == 20 {
                    assert_ne!(custom, expected, "custom content bypassed automatic motion");
                }
                if time == 1000 {
                    assert_eq!(custom, expected);
                }
            }
            assert_eq!(custom_runtime.active_animation_count(), 0);
        }
    }
}

#[test]
fn presented_raw_unicode_effects_and_local_overlays_still_preserve_complete_glyphs() {
    for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
        let env = environment(ColorDepth::Mono, MotionPreference::None);
        let tree: Element<()> = row()
            .gap(0)
            .child(
                panel("host")
                    .width(24)
                    .height(8)
                    .child(
                        presented(|_| {
                            Node::text("界 e\u{301} 👩‍🔬", Style::new())
                                .width(16.0)
                                .height(1.0)
                        })
                        .post_process([SurfaceFx::Reverse]),
                    )
                    .overlay(raw(Node::text("端", Style::new())
                        .width(2.0)
                        .height(1.0)
                        .offset(20.0, 5.0)))
                    .overlay(presented(|_| {
                        Node::text("P", Style::new())
                            .width(1.0)
                            .height(1.0)
                            .offset(22.0, 6.0)
                    })),
            )
            .child(raw(Node::text("NEIGHBOR", Style::new())
                .width(10.0)
                .height(1.0)));
        let frame = compile(&tree, &BuildCx::new(skin, env)).unwrap();
        let surface = paint_node(frame.node, env);
        let text = text_of(&surface);
        assert!(text.contains("界 e\u{301} 👩‍🔬"));
        assert!(text.contains("端"));
        assert!(text.contains("NEIGHBOR"));
        assert_eq!(
            surface.get(20, 5).unwrap().glyph.grapheme,
            "端",
            "raw overlay lost its local placement"
        );
        assert_eq!(
            surface.get(22, 6).unwrap().glyph.grapheme,
            "P",
            "presented overlay lost its local placement"
        );
        for y in 0..surface.height {
            for x in 0..surface.width {
                let cell = surface.get(x, y).unwrap();
                if cell.is_continuation {
                    assert!(x > 0);
                    let lead = surface.get(x - 1, y).unwrap();
                    assert_eq!(lead.glyph.display_width, 2);
                    assert!(!lead.is_continuation);
                    assert_eq!(lead.style, cell.style);
                } else if cell.glyph.display_width == 2 {
                    assert!(x + 1 < surface.width);
                    assert!(surface.get(x + 1, y).unwrap().is_continuation);
                }
            }
        }
    }
}

#[test]
fn modal_motion_is_local_and_the_underlying_veil_is_stable() {
    let pattern = (0..24)
        .map(|row| {
            if row % 2 == 0 {
                "AB".repeat(40)
            } else {
                "12".repeat(40)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let tree: Element<()> = raw(Node::text(pattern, Style::new()).width(80.0).height(24.0))
        .overlay(
            modal("DIALOG")
                .key("dialog")
                .width(20)
                .height(8)
                .child(text("BODY")),
        );
    // Includes the 20x8 panel plus a two-cell allowance for local displacement
    // and skin chrome. Outside this envelope belongs solely to the underlay.
    let envelope = Rect::new(28, 6, 24, 12);
    for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
        for depth in [ColorDepth::Mono, ColorDepth::Ansi16] {
            let none = environment(depth, MotionPreference::None);
            let settled = UiRuntime::new(skin).frame(&tree, none, ms(0)).unwrap();
            let settled = paint_node(settled.node, none);
            assert!(settled.get(0, 0).unwrap().style.dim, "modal lost its veil");
            assert!(
                text_of(&settled).contains("BODY"),
                "modal body was never rendered"
            );
            for preference in [MotionPreference::Full, MotionPreference::Reduced] {
                let env = environment(depth, preference);
                let mut runtime = UiRuntime::new(skin);
                runtime.frame(&tree, env, ms(0)).unwrap();
                for time in [1, 35, 70] {
                    let active = runtime.frame(&tree, env, ms(time)).unwrap();
                    let active = paint_node(active.node, env);
                    for y in 0..24 {
                        for x in 0..80 {
                            if !envelope.contains(x, y) {
                                assert_eq!(active.get(x,y),settled.get(x,y),"{} {depth:?} {preference:?} t={time} repainted underlay at({x},{y})",skin.name);
                            }
                        }
                    }
                }
                let final_frame = runtime.frame(&tree, env, ms(2000)).unwrap();
                assert_eq!(paint_node(final_frame.node, env), settled);
                assert_eq!(runtime.active_animation_count(), 0);
            }
        }
    }
}

#[test]
fn toast_motion_cannot_restyle_or_erase_the_rest_of_its_parent() {
    let pattern = (0..24)
        .map(|_| "AB".repeat(40))
        .collect::<Vec<_>>()
        .join("\n");
    let tree: Element<()> = raw(Node::text(pattern, Style::new()).width(80.0).height(24.0))
        .overlay(toast("SAVED").key("toast").width(20).height(3));
    let envelope = Rect::new(58, 19, 22, 5);
    for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
        let none = environment(ColorDepth::Mono, MotionPreference::None);
        let settled = UiRuntime::new(skin).frame(&tree, none, ms(0)).unwrap();
        let settled = paint_node(settled.node, none);
        assert!(
            text_of(&settled).contains("SAVED"),
            "toast body was never rendered"
        );
        for preference in [MotionPreference::Full, MotionPreference::Reduced] {
            let env = environment(ColorDepth::Mono, preference);
            let mut runtime = UiRuntime::new(skin);
            runtime.frame(&tree, env, ms(0)).unwrap();
            let active = runtime.frame(&tree, env, ms(35)).unwrap();
            let active = paint_node(active.node, env);
            for y in 0..24 {
                for x in 0..80 {
                    if !envelope.contains(x, y) {
                        assert_eq!(
                            active.get(x, y),
                            settled.get(x, y),
                            "{} {preference:?} toast affected parent at({x},{y})",
                            skin.name
                        );
                    }
                }
            }
            let final_frame = runtime.frame(&tree, env, ms(2000)).unwrap();
            assert_eq!(paint_node(final_frame.node, env), settled);
        }
    }
}

#[test]
fn explicit_editorial_section_numbers_survive_reordering_and_conditional_omission() {
    let env = environment(ColorDepth::Mono, MotionPreference::None);
    let build = BuildCx::new(skins::SWISS_SIGNAL, env);
    for names in [vec!["ALPHA", "BETA"], vec!["BETA", "ALPHA"], vec!["BETA"]] {
        let tree: Element<()> = column()
            .density(Density::Compact)
            .children(names.iter().map(|name| {
                section(*name)
                    .number(if *name == "ALPHA" { 1 } else { 24 })
                    .child(text("body"))
            }));
        let frame = compile(&tree, &build).unwrap();
        let visible = text_of(&paint_node(frame.node, env));
        assert!(visible.contains("24 / BETA"));
        assert_eq!(visible.contains("01 / ALPHA"), names.contains(&"ALPHA"));
        assert!(!visible.contains("02 / BETA"));
    }
    let automatic: Element<()> = column()
        .density(Density::Compact)
        .child(section("ALPHA").child(text("body")))
        .child(section("BETA").child(text("body")));
    let frame = compile(&automatic, &build).unwrap();
    let visible = text_of(&paint_node(frame.node, env));
    assert!(visible.contains("01 / ALPHA"));
    assert!(visible.contains("02 / BETA"));
}
