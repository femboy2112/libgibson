use gibson::raster::{RgbRaster, RgbaRaster};
use gibson::stage::{resolve_raster, RasterAnchor, RasterError, RasterLimits, RasterPlane};
use gibson::{Node, Rect, Style};
use std::sync::Arc;

fn limits() -> RasterLimits {
    RasterLimits {
        planes: 16,
        nodes: 100,
        pixels: 100_000,
    }
}
fn rgb(w: u16, h: u16) -> RgbRaster {
    let mut r = RgbRaster::new(w, 2 * h);
    for y in 0..2 * h {
        for x in 0..w {
            r.set(
                x.into(),
                y.into(),
                if (x + y) % 2 == 0 {
                    (20, 40, 60)
                } else {
                    (100, 120, 140)
                },
            );
        }
    }
    r
}
fn source(p: &[[u8; 4]], w: u16) -> Arc<RgbaRaster> {
    Arc::new(RgbaRaster::from_pixels(w, (p.len() / usize::from(w)) as u16, p.to_vec()).unwrap())
}
fn plane(s: Arc<RgbaRaster>) -> RasterPlane {
    RasterPlane {
        anchor: RasterAnchor::FramePath(vec![]),
        source: s,
        offset_px: (0, 0),
        z: 0,
    }
}
fn root(w: u16, h: u16) -> Node {
    Node::col().width(w as f32).height(h as f32)
}

#[test]
fn alpha_uses_the_current_checkerboard_and_immutable_source() {
    let s = source(
        &[[200, 10, 30, 0], [200, 10, 30, 128], [200, 10, 30, 255]],
        3,
    );
    let p = plane(Arc::clone(&s));
    let a = rgb(4, 2);
    let mut b = a.clone();
    b.clear((8, 16, 24));
    for under in [&a, &b] {
        let out = resolve_raster(
            &root(4, 2),
            (4, 2),
            under,
            std::slice::from_ref(&p),
            limits(),
        )
        .unwrap();
        for x in 0..3 {
            let old = under.get(x, 0).unwrap();
            let alpha = [0u32, 128, 255][x as usize];
            let mix = |src: u8, dst: u8| {
                ((u32::from(src) * alpha + u32::from(dst) * (255 - alpha) + 127) / 255) as u8
            };
            assert_eq!(
                out.pixels.get(x, 0),
                Some((mix(200, old.0), mix(10, old.1), mix(30, old.2)))
            );
        }
        assert_eq!(out.pixels.get(3, 0), under.get(3, 0));
        assert_eq!(out.surface, out.pixels.to_surface());
    }
    assert!(Arc::ptr_eq(&s, &p.source));
    assert_eq!(s.get(1, 0), Some([200, 10, 30, 128]));
}

#[test]
fn negative_odd_pixel_offset_clips_before_one_half_block_conversion() {
    let mut p = plane(source(
        &[
            [1, 2, 3, 255],
            [4, 5, 6, 255],
            [7, 8, 9, 255],
            [10, 11, 12, 255],
        ],
        2,
    ));
    p.offset_px = (-1, -1);
    let under = rgb(3, 2);
    let out = resolve_raster(&root(3, 2), (3, 2), &under, &[p], limits()).unwrap();
    assert_eq!(out.placements[0].origin_px, (-1, -1));
    assert_eq!(out.placements[0].clip_px, Rect::new(0, 0, 1, 1));
    assert_eq!(out.pixels.get(0, 0), Some((10, 11, 12)));
    assert_eq!(out.pixels.get(0, 1), under.get(0, 1));
    assert_eq!(out.surface, out.pixels.to_surface());
}

#[test]
fn stable_z_order_and_equal_z_input_order() {
    let red = source(&[[255, 0, 0, 255]], 1);
    let blue = source(&[[0, 0, 255, 128]], 1);
    let mut a = plane(red);
    a.z = 4;
    let mut b = plane(blue);
    b.z = -2;
    let out = resolve_raster(
        &root(2, 1),
        (2, 1),
        &rgb(2, 1),
        &[a.clone(), b.clone()],
        limits(),
    )
    .unwrap();
    assert_eq!(
        out.placements.iter().map(|p| p.plane).collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(out.pixels.get(0, 0), Some((255, 0, 0)));
    b.z = 4;
    let out = resolve_raster(&root(2, 1), (2, 1), &rgb(2, 1), &[a, b], limits()).unwrap();
    assert_eq!(out.pixels.get(0, 0), Some((127, 0, 128)));
}

#[test]
fn layout_makes_room_and_offsets_do_not_reflow_siblings() {
    for width in [6, 10, 20] {
        let tree = Node::row()
            .width(width as f32)
            .height(2.)
            .child(root(2, 2).flex_shrink(0.))
            .child(Node::text("reader", Style::default()).flex_grow(1.));
        let mut p = plane(source(&[[200, 0, 0, 255]; 4], 2));
        p.anchor = RasterAnchor::FramePath(vec![0]);
        p.offset_px = (1, 1);
        let out = resolve_raster(&tree, (width, 2), &rgb(width, 2), &[p], limits()).unwrap();
        assert_eq!(out.placements[0].layout_rect, Rect::new(0, 0, 2, 2));
        assert_eq!(out.placements[0].origin_px, (1, 1));
        assert_eq!(out.layout.children[1].computed_rect.x, 2);
        assert_eq!(out.pixels.get(1, 1), Some((200, 0, 0)));
    }
}

#[test]
fn nested_allocation_clips_large_source_without_resizing_it() {
    let s = source(&[[30, 50, 70, 255]; 32], 8);
    let tree = root(8, 3).child(root(2, 1));
    let mut p = plane(Arc::clone(&s));
    p.anchor = RasterAnchor::FramePath(vec![0]);
    let under = rgb(8, 3);
    let out = resolve_raster(&tree, (8, 3), &under, &[p], limits()).unwrap();
    assert_eq!(out.placements[0].clip_px, Rect::new(0, 0, 2, 2));
    assert_eq!(out.pixels.get(2, 0), under.get(2, 0));
    assert_eq!((s.width(), s.height()), (8, 4));
}

#[test]
fn offscreen_and_integer_extremes_are_explicit_and_nonpanicking() {
    for offset in [(i32::MIN, i32::MIN), (i32::MAX, i32::MAX)] {
        let mut p = plane(source(&[[1, 2, 3, 255]], 1));
        p.offset_px = offset;
        let under = rgb(2, 2);
        let out = resolve_raster(&root(2, 2), (2, 2), &under, &[p], limits()).unwrap();
        assert_eq!(out.placements[0].clip_px, Rect::default());
        assert_eq!(out.pixels, under);
    }
}

#[test]
fn shared_and_offscreen_sources_still_consume_aggregate_budget() {
    let s = source(&[[1, 2, 3, 255]; 4], 2);
    let a = plane(Arc::clone(&s));
    let mut b = a.clone();
    b.offset_px = (100, 100);
    let budget = RasterLimits {
        pixels: 15,
        ..limits()
    }; // canvas8 + 2*4 per plane
    assert_eq!(
        resolve_raster(
            &root(2, 2),
            (2, 2),
            &rgb(2, 2),
            std::slice::from_ref(&a),
            budget
        )
        .unwrap_err(),
        RasterError::Budget
    );
    let budget = RasterLimits {
        pixels: 16,
        ..limits()
    };
    assert!(resolve_raster(
        &root(2, 2),
        (2, 2),
        &rgb(2, 2),
        std::slice::from_ref(&a),
        budget
    )
    .is_ok());
    assert_eq!(
        resolve_raster(&root(2, 2), (2, 2), &rgb(2, 2), &[a, b], budget).unwrap_err(),
        RasterError::Budget
    );
}

#[test]
fn admission_rejects_missing_paths_nodes_layers_and_viewports() {
    let r = root(2, 2);
    let under = rgb(2, 2);
    let mut p = plane(source(&[[1, 2, 3, 255]], 1));
    p.anchor = RasterAnchor::FramePath(vec![9]);
    assert_eq!(
        resolve_raster(&r, (2, 2), &under, &[p], limits()).unwrap_err(),
        RasterError::MissingAnchor(0)
    );
    for cap in [
        RasterLimits {
            nodes: 0,
            ..limits()
        },
        RasterLimits {
            pixels: 0,
            ..limits()
        },
    ] {
        assert_eq!(
            resolve_raster(&r, (2, 2), &under, &[], cap).unwrap_err(),
            RasterError::Budget
        );
    }
    assert_eq!(
        resolve_raster(
            &r,
            (2, 2),
            &under,
            &[plane(source(&[[0; 4]], 1))],
            RasterLimits {
                planes: 0,
                ..limits()
            }
        )
        .unwrap_err(),
        RasterError::Budget
    );
    for viewport in [(0, 2), (2, 0), (2, 1025), (2049, 2), (3, 2)] {
        assert_eq!(
            resolve_raster(&r, viewport, &under, &[], limits()).unwrap_err(),
            RasterError::Viewport
        );
    }
}

#[test]
fn border_ancestor_clip_blocks_pixels_even_after_presentation_move() {
    let tree = Node::border_box(gibson::BorderType::Single, Style::default())
        .width(6.)
        .height(4.)
        .child(root(4, 2));
    let mut p = plane(source(&[[230, 0, 0, 255]; 32], 4));
    p.anchor = RasterAnchor::FramePath(vec![0]);
    p.offset_px = (-1, -2);
    let under = rgb(6, 4);
    let out = resolve_raster(&tree, (6, 4), &under, &[p], limits()).unwrap();
    assert_eq!(out.pixels.get(1, 3), Some((230, 0, 0)));
    assert_eq!(out.placements[0].clip_px, Rect::new(1, 2, 3, 2));
    assert_eq!(out.pixels.get(0, 2), under.get(0, 2));
    assert_eq!(out.pixels.get(1, 1), under.get(1, 1));
    assert_eq!(out.pixels.get(1, 2), Some((230, 0, 0)));
}

#[test]
fn keyed_plane_follows_unequal_siblings_after_reorder_and_resize() {
    use gibson::stage::resolve_compiled_raster;
    use gibson::ui::prelude::*;
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let mut p = plane(source(&[[240, 0, 0, 255]], 1));
    p.anchor = RasterAnchor::Key(Key::named("sprite"));
    for (swapped, width, expected_x) in [(false, 6, 0), (true, 6, 4), (false, 10, 0), (true, 10, 4)]
    {
        let sprite = column::<()>()
            .key("sprite")
            .width(2)
            .height(1)
            .padding(0)
            .gap(0);
        let other = column::<()>()
            .key("other")
            .width(4)
            .height(1)
            .padding(0)
            .gap(0);
        let tree = row()
            .key("host")
            .width(width)
            .height(1)
            .padding(0)
            .gap(0)
            .children(if swapped {
                vec![other, sprite]
            } else {
                vec![sprite, other]
            });
        let compiled = ui
            .frame(
                &tree,
                UiEnvironment {
                    width,
                    height: 1,
                    ..Default::default()
                },
                std::time::Duration::ZERO,
            )
            .unwrap();
        let out = resolve_compiled_raster(
            &compiled,
            (width, 1),
            &rgb(width, 1),
            std::slice::from_ref(&p),
            limits(),
        )
        .unwrap();
        assert_eq!(
            out.pixels.get(expected_x, 0),
            Some((240, 0, 0)),
            "keyed pixel follows sprite"
        );
        assert_eq!(out.placements[0].layout_rect.x, expected_x as u16);
        assert_eq!(compiled.keys.len(), 3);
        assert!(
            compiled.targets.is_empty(),
            "noninteractive anchors need no fabricated target"
        );
    }
}

#[test]
fn absent_key_is_refused_and_duplicate_keys_fail_before_lowering() {
    use gibson::stage::resolve_compiled_raster;
    use gibson::ui::prelude::*;
    let cx = BuildCx::new(skins::SWISS_SIGNAL, UiEnvironment::default());
    let tree = column::<()>().key("existing");
    let compiled = compile(&tree, &cx).unwrap();
    let mut p = plane(source(&[[0; 4]], 1));
    p.anchor = RasterAnchor::Key(Key::named("absent"));
    assert_eq!(
        resolve_compiled_raster(
            &compiled,
            (2, 2),
            &rgb(2, 2),
            std::slice::from_ref(&p),
            limits()
        )
        .unwrap_err(),
        RasterError::MissingAnchor(0)
    );
    p.anchor = RasterAnchor::Key(Key::named("existing"));
    assert_eq!(
        resolve_raster(&compiled.node, (2, 2), &rgb(2, 2), &[p], limits()).unwrap_err(),
        RasterError::MissingAnchor(0),
        "Key anchoring requires the Compiled frame, not an unrelated lookup table"
    );
    let duplicate = column::<()>()
        .child(column().key("same"))
        .child(column().key("same"));
    assert!(
        matches!(compile(&duplicate,&cx),Err(UiError::DuplicateKey(key)) if key==Key::named("same"))
    );
}

#[test]
fn same_lowering_join_tracks_chrome_overlays_and_restores_all_interaction_tags() {
    use gibson::ui::prelude::*;
    for skin in [skins::SWISS_SIGNAL, skins::VAPOR95, skins::BLACK_ICE] {
        let tree = panel::<u8>("Panel")
            .key("panel")
            .child(
                column()
                    .key("noninteractive")
                    .child(button("run").key("run").on_press(7)),
            )
            .overlay(
                modal("Modal")
                    .key("modal")
                    .on_dismiss(8)
                    .child(button("close").key("close").on_press(9)),
            );
        let compiled = compile(&tree, &BuildCx::new(skin, UiEnvironment::default())).unwrap();
        assert_eq!(compiled.geometry_paths.len(), 5);
        for name in ["panel", "noninteractive", "run", "modal", "close"] {
            let key = Key::named(name);
            let mut node = &compiled.node;
            for &i in &compiled.geometry_paths[&key] {
                node = &node.children[i];
            }
            match node.tag {
                Some(tag) => assert_eq!(compiled.targets[tag as usize].key, key),
                None => assert!(name == "panel" || name == "noninteractive"),
            }
        }
        let mut stack = vec![&compiled.node];
        while let Some(node) = stack.pop() {
            assert!(
                node.tag
                    .is_none_or(|tag| (tag as usize) < compiled.targets.len()),
                "no temporary geometry marker escapes"
            );
            stack.extend(&node.children);
        }
    }
}
