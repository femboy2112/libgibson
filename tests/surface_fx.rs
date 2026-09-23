//! Real ordinary-widget post-processing through layout, paint, diff and ANSI.
use gibson::*;
use std::time::Duration;

fn panel() -> Node {
    Node::panel("ORDINARY", BorderType::Single, Style::new().fg(Color::Cyan))
        .width(22.0)
        .height(6.0)
        .child(Node::text("hello 界 e\u{301}", Style::new().bold()))
        .child(
            Node::viewport(2, 0)
                .width(18.0)
                .height(2.0)
                .child(Node::text("0123456789ABCDEFGHIJ", Style::new()).width(24.0)),
        )
}
fn render(mut node: Node, w: u16, h: u16) -> Surface {
    compute_layout(&mut node, w, h).unwrap();
    let mut s = Surface::new_transparent(w, h);
    paint(&node, &mut s);
    s
}
fn valid(s: &Surface) {
    for y in 0..s.height {
        for x in 0..s.width {
            let c = s.get(x, y).unwrap();
            if c.is_continuation {
                assert!(x > 0);
                let prev = s.get(x - 1, y).unwrap();
                assert!(
                    !prev.transparent && !prev.is_continuation && prev.glyph.display_width == 2
                );
                assert_eq!(prev.style, c.style);
            } else if !c.transparent && !c.style_only && c.glyph.display_width == 2 {
                assert!(x + 1 < s.width);
                let next = s.get(x + 1, y).unwrap();
                assert!(next.is_continuation && !next.transparent);
            }
        }
    }
}
#[test]
fn overlay_preserves_ordinary_glyphs_layout_and_holes() {
    let base = render(panel(), 30, 10);
    let fx = render(
        panel().post_process([SurfaceFx::StyleOverlay(Style::new().fg(Color::Magenta))]),
        30,
        10,
    );
    for (a, b) in base.cells.iter().zip(&fx.cells) {
        assert_eq!(a.glyph, b.glyph);
        assert_eq!(a.transparent, b.transparent);
        assert_eq!(a.style_only, b.style_only);
        if !b.transparent {
            assert_eq!(b.style.fg, Some(Color::Magenta));
        }
    }
    valid(&fx);
}
#[test]
fn ordered_chains_are_associative_but_not_commutative() {
    let input = render(panel(), 22, 6);
    let a = SurfaceFx::StyleOverlay(Style::new().fg(Color::Magenta));
    let b = SurfaceFx::StyleOverlay(Style::new().fg(Color::Cyan));
    let c = SurfaceFx::Tear {
        row: 2,
        height: 2,
        amount: 2,
    };
    let mut left = input.clone();
    SurfaceFx::apply_chain(&[a.clone(), b.clone()], &mut left);
    c.apply(&mut left);
    let mut right = input.clone();
    a.apply(&mut right);
    SurfaceFx::apply_chain(&[b.clone(), c], &mut right);
    assert_eq!(left, right);
    let mut ab = input.clone();
    SurfaceFx::apply_chain(&[a.clone(), b.clone()], &mut ab);
    let mut ba = input.clone();
    SurfaceFx::apply_chain(&[b, a], &mut ba);
    assert_ne!(ab, ba);
    let mut identity = input.clone();
    SurfaceFx::apply_chain(&[], &mut identity);
    assert_eq!(identity, input);
}
#[test]
fn dissolve_endpoints_stability_and_monotone_masks() {
    let base = render(panel(), 22, 6);
    let full = render(
        panel().post_process([SurfaceFx::Dissolve {
            seed: 41,
            fraction: 1.0,
        }]),
        22,
        6,
    );
    assert_eq!(full, base);
    let hidden = render(
        panel().post_process([SurfaceFx::Dissolve {
            seed: 41,
            fraction: 0.0,
        }]),
        22,
        6,
    );
    assert!(hidden.cells.iter().all(|c| c.transparent));
    let mut previous = hidden;
    for fraction in [0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let make = || {
            render(
                panel().post_process([SurfaceFx::Dissolve { seed: 41, fraction }]),
                22,
                6,
            )
        };
        let next = make();
        assert_eq!(next, make());
        valid(&next);
        for (a, b) in previous.cells.iter().zip(&next.cells) {
            if !a.transparent {
                assert_eq!(a, b);
            }
        }
        previous = next;
    }
}
#[test]
fn scramble_is_seeded_preserves_styles_and_wide_glyphs() {
    let base = render(panel(), 22, 6);
    let fx = SurfaceFx::Scramble {
        seed: 987,
        intensity: 0.8,
    };
    let mut a = base.clone();
    fx.apply(&mut a);
    let mut b = base.clone();
    fx.apply(&mut b);
    assert_eq!(a, b);
    assert_ne!(a, base);
    valid(&a);
    for (a, b) in a.cells.iter().zip(&base.cells) {
        assert_eq!(a.style, b.style);
        assert_eq!(a.transparent, b.transparent);
        if b.glyph.display_width == 2 || b.is_continuation {
            assert_eq!(a, b);
        }
    }
}
#[test]
fn tear_is_strip_local_and_transparent_holes_remain_holes() {
    let base = render(panel(), 22, 6);
    let mut s = base.clone();
    SurfaceFx::Tear {
        row: 2,
        height: 1,
        amount: 2,
    }
    .apply(&mut s);
    for y in [0, 1, 3, 4, 5] {
        for x in 0..22 {
            assert_eq!(base.get(x, y), s.get(x, y));
        }
    }
    assert!(s.get(0, 2).unwrap().transparent);
    valid(&s);
    let mut empty = Surface::new_transparent(22, 6);
    SurfaceFx::apply_chain(
        &[
            SurfaceFx::Dim,
            SurfaceFx::Reverse,
            SurfaceFx::RowShift { amount: 2, seed: 4 },
            SurfaceFx::Scramble {
                seed: 5,
                intensity: 1.0,
            },
        ],
        &mut empty,
    );
    assert!(empty.cells.iter().all(|c| c.transparent));
}
#[test]
fn style_only_cells_are_never_promoted_into_opaque_glyphs() {
    let mut s = Surface::new_transparent(8, 2);
    s.set_cell(2, 0, Cell::style_overlay(Style::new().dim()));
    SurfaceFx::apply_chain(
        &[
            SurfaceFx::Scramble {
                seed: 5,
                intensity: 1.0,
            },
            SurfaceFx::RowShift { amount: 1, seed: 0 },
            SurfaceFx::Reverse,
        ],
        &mut s,
    );
    assert!(s.get(3, 0).unwrap().style_only);
    let mut dest = Surface::new(8, 2);
    dest.print_str(0, 0, "abcdefgh", Style::new(), None);
    dest.blit_transparent(&s);
    assert_eq!(dest.get(3, 0).unwrap().glyph.grapheme.as_str(), "d");
    assert!(dest.get(3, 0).unwrap().style.dim);
}
#[test]
fn wide_glyphs_survive_hostile_chains_and_extreme_parameters() {
    for seed in 0..40 {
        for amount in [i32::MIN, -7, -1, 0, 1, 7, i32::MAX] {
            let mut s = render(panel(), 22, 6);
            SurfaceFx::apply_chain(
                &[
                    SurfaceFx::RowShift { amount, seed },
                    SurfaceFx::Dissolve {
                        seed,
                        fraction: 0.6,
                    },
                    SurfaceFx::Scramble {
                        seed,
                        intensity: 0.8,
                    },
                    SurfaceFx::Scanline {
                        position: 0.5,
                        style: Style::new().reverse(),
                    },
                ],
                &mut s,
            );
            valid(&s);
        }
    }
    for fraction in [f32::NAN, f32::NEG_INFINITY, f32::INFINITY] {
        let mut s = render(panel(), 22, 6);
        SurfaceFx::Dissolve { seed: 1, fraction }.apply(&mut s);
        valid(&s);
    }
}
#[test]
fn scene_bundle_removal_restores_exact_original_and_local_damage() {
    let mut scene = Scene::new();
    scene.add(
        SceneEntity::new(
            "backdrop",
            Node::text("BACKGROUND".repeat(12), Style::new())
                .width(120.0)
                .height(1.0),
        )
        .offset(0, 20),
    );
    let id = scene.add(SceneEntity::new("panel", panel()).offset(7, 3));
    let story = Story::new("live")
        .bundle(EffectBundle::new("color").effect(Effect::post_process(
            SceneTarget::Id(id),
            SurfaceFx::StyleOverlay(Style::new().fg(Color::Magenta)),
        )))
        .beat(
            Beat::new("live")
                .reaction(Condition::command("on"), [StoryAction::mount("color")])
                .reaction(Condition::command("off"), [StoryAction::unmount("color")]),
        );
    let mut director = story.start();
    let build = |d: &StoryDirector| scene.to_node(&d.presentation(&scene), 120.0, 32.0);
    let clean = render(build(&director), 120, 32);
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut terminal = TerminalSession::headless(120, 32);
    let mut wire = Vec::new();
    renderer
        .render(&mut build(&director), &mut terminal, &mut wire)
        .unwrap();
    wire.clear();
    director.update(Duration::ZERO, &[StoryEvent::command("on")]);
    let infected = render(build(&director), 120, 32);
    let diff = compute_diff(Some(&clean), &infected);
    assert!(diff.exact_changed_cell_count() > 0);
    assert!(diff
        .exact_changed_cells()
        .iter()
        .all(|&(x, y)| (7..29).contains(&x) && (3..9).contains(&y)));
    let (_, _, bytes, _, _) = renderer
        .render(&mut build(&director), &mut terminal, &mut wire)
        .unwrap();
    assert!(bytes > 0);
    assert!(bytes < 1800);
    eprintln!(
        "panel: exact={} affected={} wire={bytes}",
        diff.exact_changed_cell_count(),
        diff.affected_cell_count()
    );
    wire.clear();
    let (_, _, bytes, _, _) = renderer
        .render(&mut build(&director), &mut terminal, &mut wire)
        .unwrap();
    assert_eq!(bytes, 0);
    assert!(wire.is_empty());
    director.update(Duration::ZERO, &[StoryEvent::command("off")]);
    assert_eq!(render(build(&director), 120, 32), clean);
    renderer
        .render(&mut build(&director), &mut terminal, &mut wire)
        .unwrap();
}
#[test]
fn identity_postprocess_matches_clipped_panel_on_every_edge() {
    for (x, y) in [(-3.0, 0.0), (0.0, -2.0), (15.0, 0.0), (0.0, 7.0)] {
        let make = |fx: Vec<SurfaceFx>| {
            Node::stack()
                .width(30.0)
                .height(10.0)
                .child(panel().post_process(fx).offset(x, y))
        };
        assert_eq!(
            render(make(vec![]), 30, 10),
            render(
                make(vec![SurfaceFx::Dissolve {
                    seed: 0,
                    fraction: 1.0
                }]),
                30,
                10
            ),
            "at {x},{y}"
        );
    }
}
#[test]
fn nested_postprocess_and_viewport_preserve_mask_coordinates() {
    let base = panel().post_process([SurfaceFx::Dissolve {
        seed: 7,
        fraction: 0.6,
    }]);
    let world = render(base.clone(), 22, 6);
    let viewed = render(
        Node::viewport(3, 1).width(12.0).height(4.0).child(base),
        12,
        4,
    );
    let mut expected = Surface::new_transparent(12, 4);
    expected.blit_transparent_clipped(&world, -3, -1, expected.area());
    assert_eq!(viewed, expected);
}

#[test]
fn style_mask_spreads_without_erasing_glyphs_or_transparency() {
    let base = render(panel(), 22, 6);
    let style = Style::new().fg(Color::Magenta).reverse();
    let mut previous = 0;
    for fraction in [0.0, 0.1, 0.3, 0.6, 1.0] {
        let mut s = base.clone();
        SurfaceFx::StyleMask {
            style,
            seed: 212,
            fraction,
        }
        .apply(&mut s);
        valid(&s);
        let affected = s.cells.iter().filter(|c| c.style.reverse).count();
        assert!(affected >= previous);
        previous = affected;
        for (a, b) in base.cells.iter().zip(&s.cells) {
            assert_eq!(a.glyph, b.glyph);
            assert_eq!(a.transparent, b.transparent);
        }
        if fraction == 0.0 {
            assert_eq!(s, base);
        }
        if fraction == 1.0 {
            let mut full = base.clone();
            SurfaceFx::StyleOverlay(style).apply(&mut full);
            assert_eq!(s, full);
        }
    }
}
