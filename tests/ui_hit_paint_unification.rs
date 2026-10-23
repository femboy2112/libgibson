//! Wave 2 / Finding E / Categorical Law L2 — target soundness (issue #127).
//!
//! The decisive proof that pointer targeting is derived from the *actually
//! painted* layout, not from caller-supplied geometry. The oracle is independent
//! of the hit map: it reads the painted [`Surface`] itself. Every keyed target is
//! a `raw` node filled with a unique sentinel background color; the key that
//! *should* win at a cell is the topmost sentinel the surface actually shows there
//! (the surface already resolves z-order for us — later paints overwrite earlier).
//! For every cell we assert `hit_map.hit(x, y).key == painted_sentinel(x, y)`.
//!
//! A rich **control** scene (a bordered panel that clips its child, a `stack`
//! overlap, a clipped `viewport`, a disabled occluder) must agree everywhere — it
//! can only do so if the walk reproduces nesting, border-clip, z-order and
//! viewport clipping exactly. Then a battery of **mutation arms** tampers one
//! public `PaintedRegion` field at a time and asserts the oracle bleeds, so the
//! agreement is not vacuous. Finally the modal arm proves a modal overlay blocks
//! clicks to the base beneath it, with a no-modal companion for contrast.

use gibson::ui::prelude::*;
use gibson::ui::{HitMap, HitMapError, ResolvedTarget};
use gibson::{compute_layout, paint, Color, Node, Rect, Surface};

// Distinctive sentinel backgrounds, one per keyed target. Saturated, odd values
// no skin is likely to paint, so a non-target cell never looks like a target.
const SOLO: Color = Color::Rgb(210, 10, 10);
const OUTER: Color = Color::Rgb(10, 210, 10);
const INNER: Color = Color::Rgb(10, 10, 210);
const PANEL: Color = Color::Rgb(210, 210, 10);
const CLIP: Color = Color::Rgb(10, 210, 210);
const DIS: Color = Color::Rgb(130, 130, 130);
const MODL: Color = Color::Rgb(210, 10, 210);

fn sentinels() -> Vec<(Color, &'static str)> {
    vec![
        (SOLO, "SOLO"),
        (OUTER, "OUTER"),
        (INNER, "INNER"),
        (PANEL, "PANEL"),
        (CLIP, "CLIP"),
        (DIS, "DIS"),
        (MODL, "MOD"),
    ]
}

/// A keyed, interactive target that fills its whole rect with a sentinel color.
/// `raw` so it carries exactly its own background and is never wrapped in chrome.
fn target(key: &str, bg: Color, w: u16, h: u16) -> Element<()> {
    raw(Node::col().background(bg).width(w as f32).height(h as f32))
        .key(key)
        .on_press(())
}

/// The control scene: nesting/border-clip (panel), a `stack` overlap (INNER over
/// OUTER), a clipped `viewport` (CLIP larger than its camera), and a disabled
/// occluder (DIS).
fn base_scene() -> Element<()> {
    // Every container is explicitly sized: a `Stack` with no known size collapses
    // to 0x0 (layout.rs), which would silently empty OUTER/INNER and make the
    // overlap/z-order arms vacuous. With room to spare (80x30), nothing is
    // flex-squeezed, so each target paints at a predictable, non-empty rect.
    screen().child(
        column()
            .child(
                row()
                    .height(8)
                    .child(
                        panel("P")
                            .width(14)
                            .height(8)
                            .child(target("PANEL", PANEL, 6, 3)),
                    )
                    .child(
                        // Sized so it does not collapse; OUTER fills it and INNER
                        // sits on top at the same origin -> a guaranteed overlap.
                        stack()
                            .width(16)
                            .height(8)
                            .child(target("OUTER", OUTER, 16, 8))
                            .child(target("INNER", INNER, 6, 3)),
                    )
                    .child(target("SOLO", SOLO, 10, 4)),
            )
            .child(
                row()
                    .height(8)
                    .child(
                        viewport(1, 1)
                            .width(8)
                            .height(4)
                            .child(target("CLIP", CLIP, 16, 8)),
                    )
                    .child(target("DIS", DIS, 8, 4).disabled(true)),
            ),
    )
}

fn modal_scene() -> Element<()> {
    base_scene().overlay(modal("M").child(target("MOD", MODL, 10, 4)))
}

/// Compiles, lays out and paints a scene; returns the compiled tree and the
/// painted surface so the oracle can read real pixels.
fn render(el: Element<()>, w: u16, h: u16) -> (Compiled<()>, Surface) {
    let env = UiEnvironment {
        width: w,
        height: h,
        ..UiEnvironment::default()
    };
    let cx = BuildCx::new(skins::SWISS_SIGNAL, env);
    let mut compiled = compile(&el, &cx).expect("scene compiles");
    compute_layout(&mut compiled.node, w, h).expect("layout resolves");
    let mut surface = Surface::new(w, h);
    paint(&compiled.node, &mut surface);
    (compiled, surface)
}

/// The sentinel key painted at a cell, if any — the oracle, independent of the map.
fn painted_key(surface: &Surface, x: u16, y: u16) -> Option<&'static str> {
    let bg = surface.get(x, y)?.style.bg?;
    sentinels()
        .into_iter()
        .find(|(c, _)| *c == bg)
        .map(|(_, k)| k)
}

/// The key the hit map reports at a cell.
fn hit_key(t: Option<&ResolvedTarget>) -> Option<String> {
    t.map(|t| match &t.key {
        Key::Named(s) => s.clone(),
        Key::Path(p) => format!("path:{p:?}"),
    })
}

/// Every cell where the painted sentinel and the hit map disagree.
fn disagreements(hit: &HitMap, surface: &Surface) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    for y in 0..surface.height {
        for x in 0..surface.width {
            let painted = painted_key(surface, x, y).map(str::to_string);
            if painted != hit_key(hit.hit(x, y)) {
                out.push((x, y));
            }
        }
    }
    out
}

/// Does `tag` resolve to a target with this key?
fn key_is(compiled: &Compiled<()>, tag: u32, name: &str) -> bool {
    matches!(&compiled.targets[tag as usize].key, Key::Named(s) if s == name)
}

/// Half-open rectangle intersection (both non-empty and sharing at least a cell).
fn rects_overlap(a: Rect, b: Rect) -> bool {
    !a.is_empty()
        && !b.is_empty()
        && a.x < b.x + b.width
        && b.x < a.x + a.width
        && a.y < b.y + b.height
        && b.y < a.y + a.height
}

// ============================================================================
// Precondition
// ============================================================================

#[test]
fn hit_map_refuses_before_layout() {
    let env = UiEnvironment {
        width: 40,
        height: 12,
        ..UiEnvironment::default()
    };
    let cx = BuildCx::new(skins::SWISS_SIGNAL, env);
    let compiled = compile(&base_scene(), &cx).expect("compiles");
    // No compute_layout: the root collapses to a zero rect. The map must refuse
    // rather than answer every query None (Law L7 — no silent empty).
    assert!(matches!(
        compiled.hit_map(Rect::new(0, 0, 40, 12)),
        Err(HitMapError::LayoutNotResolved)
    ));
}

// ============================================================================
// The control: painted surface and hit map agree everywhere
// ============================================================================

#[test]
fn painted_surface_and_hit_map_agree_everywhere() {
    let (compiled, surface) = render(base_scene(), 80, 30);
    let hit = compiled.hit_map(surface.area()).expect("laid out");

    // Non-vacuity: the scene really does paint targets that really are hittable.
    let hittable = (0..surface.height)
        .flat_map(|y| (0..surface.width).map(move |x| (x, y)))
        .filter(|&(x, y)| painted_key(&surface, x, y).is_some() && hit.hit(x, y).is_some())
        .count();
    assert!(
        hittable > 0,
        "scene painted no hittable targets — oracle is vacuous"
    );

    let d = disagreements(&hit, &surface);
    assert!(
        d.is_empty(),
        "painted surface and hit map disagree at {} cells; first few: {:?}",
        d.len(),
        &d[..d.len().min(24)]
    );
}

#[test]
fn painted_region_visible_is_the_real_painted_extent() {
    // A lone target painted alone: every cell of its reported `visible` rect must
    // actually carry its sentinel on the surface — so `painted_regions.visible`
    // tracks what the painter drew, through the scratch/clip path, not a guess.
    let (compiled, surface) = render(screen().child(target("SOLO", SOLO, 8, 4)), 40, 12);
    let regions = compiled.painted_regions(surface.area());
    let solo = regions
        .iter()
        .find(|r| r.tag.is_some_and(|t| key_is(&compiled, t, "SOLO")))
        .expect("SOLO region present");
    assert!(!solo.visible.is_empty(), "SOLO must have a visible rect");
    for y in solo.visible.y..solo.visible.y + solo.visible.height {
        for x in solo.visible.x..solo.visible.x + solo.visible.width {
            assert_eq!(
                surface.get(x, y).and_then(|c| c.style.bg),
                Some(SOLO),
                "cell ({x},{y}) inside SOLO.visible must be painted with SOLO's sentinel"
            );
        }
    }
}

// ============================================================================
// Mutation arms — tamper one PaintedRegion field, the oracle must bleed
// ============================================================================

#[test]
fn m1_corrupting_a_visible_targets_hit_rect_is_caught() {
    let (compiled, surface) = render(base_scene(), 80, 30);
    let mut regions = compiled.painted_regions(surface.area());
    // Pick a target that is actually visible on the surface (the topmost at some
    // cell, so its sentinel shows there), then corrupt its hit rect to empty. Those
    // painted cells must become unreachable -> the oracle reports a disagreement.
    // This is positional non-vacuity: the hit rect is load-bearing, not decorative.
    let idx = regions
        .iter()
        .position(|r| {
            r.tag.is_some_and(|t| {
                matches!(&compiled.targets[t as usize].key, Key::Named(name)
                    if (0..surface.height).any(|y| (0..surface.width)
                        .any(|x| painted_key(&surface, x, y) == Some(name.as_str()))))
            })
        })
        .expect("a visibly-painted target");
    regions[idx].visible = Rect::new(0, 0, 0, 0);
    let hit = compiled.hit_map_from_regions(regions);
    assert!(
        !disagreements(&hit, &surface).is_empty(),
        "corrupting a visibly-painted target's hit rect must break agreement"
    );
}

#[test]
fn m3_inverting_paint_order_breaks_overlap_resolution() {
    let (compiled, surface) = render(base_scene(), 80, 30);
    let regions = compiled.painted_regions(surface.area());
    // There must be a genuine overlap between two targets for z-order to matter;
    // the sized stack (OUTER + INNER at a shared origin) guarantees one. Assert it
    // so a future layout change that collapses the overlap fails loudly, not
    // vacuously.
    let overlap = regions.iter().enumerate().any(|(i, a)| {
        a.tag.is_some()
            && regions
                .iter()
                .skip(i + 1)
                .any(|b| b.tag.is_some() && rects_overlap(a.visible, b.visible))
    });
    assert!(
        overlap,
        "scene must contain an overlapping pair of targets (the sized stack)"
    );
    // Invert z everywhere: at the overlap the originally-lower target now wins the
    // cell the surface still shows as the originally-higher one.
    let max = regions.iter().map(|r| r.order).max().unwrap_or(0);
    let mut inverted = regions.clone();
    for r in inverted.iter_mut() {
        r.order = max - r.order;
    }
    let hit = compiled.hit_map_from_regions(inverted);
    assert!(
        !disagreements(&hit, &surface).is_empty(),
        "inverting z-order must misresolve the overlapping targets"
    );
}

#[test]
fn m4_using_natural_instead_of_visible_breaks_clipping() {
    let (compiled, surface) = render(base_scene(), 80, 30);
    let mut regions = compiled.painted_regions(surface.area());
    // Pretend every target occupies its unclipped natural rect: the viewport's
    // CLIP target now claims cells the camera never painted it into.
    let clipped_exists = regions
        .iter()
        .any(|r| r.tag.is_some() && r.visible != r.natural);
    assert!(
        clipped_exists,
        "the viewport must actually clip CLIP, or this arm is vacuous"
    );
    for r in regions.iter_mut() {
        r.visible = r.natural;
    }
    let hit = compiled.hit_map_from_regions(regions);
    assert!(
        !disagreements(&hit, &surface).is_empty(),
        "using natural (unclipped) rects must break the clipped viewport target"
    );
}

#[test]
fn m8_dropping_a_disabled_target_lets_clicks_fall_through() {
    let (compiled, surface) = render(base_scene(), 80, 30);
    let mut regions = compiled.painted_regions(surface.area());
    // A disabled target still occludes — it swallows the click. Drop its tag and
    // its painted cells wrongly become empty space / fall through.
    let mut dropped = false;
    for r in regions.iter_mut() {
        if r.tag.is_some_and(|t| key_is(&compiled, t, "DIS")) {
            r.tag = None;
            dropped = true;
        }
    }
    assert!(dropped, "DIS target region must exist");
    let hit = compiled.hit_map_from_regions(regions);
    assert!(
        !disagreements(&hit, &surface).is_empty(),
        "dropping the disabled occluder must break agreement on its painted cells"
    );
}

// ============================================================================
// Modal occlusion — the overlay blocks the base beneath it
// ============================================================================

#[test]
fn m5_a_modal_overlay_blocks_clicks_to_the_base_beneath_it() {
    let (compiled, surface) = render(modal_scene(), 80, 30);
    let hit = compiled.hit_map(surface.area()).expect("laid out");

    // Keyed off the hit map's own resolved targets (robust to whatever the modal
    // paints over the base): every *base* target — not the modal barrier itself,
    // not the target living inside the modal (MOD) — must be unreachable at its own
    // centre while the modal is up. The modal layer ends the walk, so a base cell
    // that misses the modal's targets yields None, never the base.
    let mut checked = 0;
    for t in hit.targets() {
        if t.modal {
            continue; // the modal barrier itself
        }
        let name = match &t.key {
            Key::Named(s) => s.clone(),
            Key::Path(p) => format!("path:{p:?}"),
        };
        if name == "MOD" || t.rect.is_empty() {
            continue; // MOD lives inside the modal; skip it and empty rects
        }
        let cx = t.rect.x + t.rect.width / 2;
        let cy = t.rect.y + t.rect.height / 2;
        assert_ne!(
            hit_key(hit.hit(cx, cy)),
            Some(name.clone()),
            "modal up: base target {name} must be blocked at its own centre ({cx},{cy})"
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no base targets resolved under the modal scene"
    );
}

#[test]
fn without_a_modal_the_base_is_hittable() {
    // Contrast for the modal arm: the only difference is the overlay, so this
    // proves the blocking above is the modal's doing, not a broken base.
    let (compiled, surface) = render(base_scene(), 80, 30);
    let hit = compiled.hit_map(surface.area()).expect("laid out");
    let any_base_hittable = (0..surface.height).any(|y| {
        (0..surface.width).any(|x| {
            let painted = painted_key(&surface, x, y).map(str::to_string);
            painted.is_some() && hit_key(hit.hit(x, y)) == painted
        })
    });
    assert!(
        any_base_hittable,
        "with no modal, at least one painted base target must be hittable"
    );
}
