//! Wave B / Finding E / Law L2 — pointer-action ELIGIBILITY and AUTHORITY.
//!
//! A painted-but-*disabled* target occludes — it swallows the press, so a click
//! never falls through to an enabled target beneath it — yet it carries NO
//! authority: the [`DragGesture`] must refuse to open or continue an edit episode
//! on it. Before the fix, `HitLayers` stripped eligibility and the gesture
//! captured any hit target by id alone, so a disabled occluder emitted a full
//! `Begin`/`Commit` carrying its `TargetId`. These tests pin the repaired
//! contract and are red-first against that old behaviour.
//!
//! Two levels. First, direct gesture units over hand-built inert layers (fast,
//! and they isolate the swallow / deny-authority / cancel-on-inert rules).
//! Second, the decisive end-to-end fixture: an enabled background target covered
//! by a disabled top target, resolved from the REAL painted layout, with an
//! enabled sibling as the positive control.

use gibson::edit::{CancelReason, EditPhase};
use gibson::gesture::{DragGesture, DragIntent};
use gibson::hit_test::{HitLayers, TargetId};
use gibson::input::{Event, KeyModifiers, MouseButton, MouseEvent, MouseKind};
use gibson::ui::prelude::*;
use gibson::ui::Eligibility;
use gibson::world_view::WorldView;
use gibson::{compute_layout, Color, Node, Rect};

// ---------------------------------------------------------------------------
// Shared event / view / run helpers (mirrors tests/edit_gesture_slice.rs).
// ---------------------------------------------------------------------------

const BG: TargetId = TargetId(1);
const TOP: TargetId = TargetId(2);

fn mouse(kind: MouseKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })
}

fn view() -> WorldView {
    WorldView::new(Rect::new(0, 0, 40, 20)).with_pixels_per_cell(1, 1)
}

fn run(
    g: &mut DragGesture,
    layers: &HitLayers,
    view: &WorldView,
    evs: &[Event],
) -> Vec<EditPhase<DragIntent>> {
    evs.iter()
        .filter_map(|e| g.on_event(e, layers, view))
        .collect()
}

fn count<F: Fn(&EditPhase<DragIntent>) -> bool>(ps: &[EditPhase<DragIntent>], f: F) -> usize {
    ps.iter().filter(|p| f(p)).count()
}

// ===========================================================================
// Level 1 — gesture units over inert layers
// ===========================================================================

#[test]
fn down_on_a_lone_inert_target_is_swallowed_no_begin() {
    // Unlike a press on empty space, the target EXISTS and is hit — but it is
    // inert, so the press is swallowed: no capture, no Begin, no Commit.
    let mut layers = HitLayers::new();
    layers.push_layer(false);
    layers.add_inert_target(BG, Rect::new(0, 0, 40, 20), 0);
    assert!(layers.is_inert(BG));
    assert_eq!(
        layers.hit_test(5, 5),
        Some(BG),
        "an inert target still occludes and wins hit-testing"
    );

    let mut g = DragGesture::new();
    let phases = run(
        &mut g,
        &layers,
        &view(),
        &[
            mouse(MouseKind::Down(MouseButton::Left), 5, 5),
            mouse(MouseKind::Drag(MouseButton::Left), 6, 6),
            mouse(MouseKind::Up(MouseButton::Left), 6, 6),
        ],
    );
    assert!(
        phases.is_empty(),
        "an inert target authorizes no edit phase, got {phases:?}"
    );
    assert!(!g.is_active());
}

#[test]
fn inert_occluder_denies_authority_without_letting_clicks_fall_through() {
    // THE counterexample: an enabled BG covered by an inert (disabled) TOP at a
    // higher z. The press must be SWALLOWED by TOP (no Begin) and must NOT fall
    // through to the enabled BG beneath — simply dropping the disabled target
    // instead of marking it inert would wrongly let BG take the edit.
    let mut layers = HitLayers::new();
    layers.push_layer(false);
    layers.add_target(BG, Rect::new(0, 0, 40, 20), 0); // enabled, beneath
    layers.add_inert_target(TOP, Rect::new(0, 0, 40, 20), 1); // disabled, above

    // Occlusion: the inert TOP wins the cell; BG is never reached -> no fall-through.
    assert_eq!(layers.hit_test(5, 5), Some(TOP));
    assert!(layers.is_inert(TOP));
    assert!(!layers.is_inert(BG));

    let mut g = DragGesture::new();
    let phases = run(
        &mut g,
        &layers,
        &view(),
        &[
            mouse(MouseKind::Down(MouseButton::Left), 5, 5),
            mouse(MouseKind::Drag(MouseButton::Left), 7, 8),
            mouse(MouseKind::Up(MouseButton::Left), 7, 8),
        ],
    );
    assert!(
        phases.is_empty(),
        "a disabled occluder must authorize no edit, and BG must not fall through; got {phases:?}"
    );
    assert!(!g.is_active());
}

#[test]
fn down_on_an_enabled_target_begins_and_commits() {
    // POSITIVE CONTROL / mutation guard: identical geometry, TOP enabled. Without
    // it the swallow tests could pass vacuously (if drags never fired at all).
    let mut layers = HitLayers::new();
    layers.push_layer(false);
    layers.add_target(BG, Rect::new(0, 0, 40, 20), 0);
    layers.add_target(TOP, Rect::new(0, 0, 40, 20), 1); // enabled now
    assert!(!layers.is_inert(TOP));

    let mut g = DragGesture::new();
    let phases = run(
        &mut g,
        &layers,
        &view(),
        &[
            mouse(MouseKind::Down(MouseButton::Left), 5, 5),
            mouse(MouseKind::Up(MouseButton::Left), 5, 5),
        ],
    );
    assert_eq!(count(&phases, |p| matches!(p, EditPhase::Begin { .. })), 1);
    assert_eq!(count(&phases, |p| matches!(p, EditPhase::Commit { .. })), 1);
    match phases.last().unwrap() {
        EditPhase::Commit { intent, .. } => assert_eq!(intent.target, TOP),
        other => panic!("expected Commit, got {other:?}"),
    }
}

#[test]
fn a_target_going_inert_mid_drag_cancels_with_target_disabled() {
    // Begin on an enabled target, then the same id goes inert under the drag (the
    // app disables it mid-gesture): the episode must cancel with TargetDisabled,
    // never route the edit to a now-disabled target. Mirrors the modal-takeover
    // and target-removed mid-drag cancellation tests.
    let view = view();
    let mut g = DragGesture::new();

    let mut live = HitLayers::new();
    live.push_layer(false);
    live.add_target(BG, Rect::new(0, 0, 40, 20), 0);
    let begin = g.on_event(
        &mouse(MouseKind::Down(MouseButton::Left), 5, 5),
        &live,
        &view,
    );
    assert!(matches!(begin, Some(EditPhase::Begin { .. })));

    let mut disabled = HitLayers::new();
    disabled.push_layer(false);
    disabled.add_inert_target(BG, Rect::new(0, 0, 40, 20), 0); // same id, now inert
    let next = g.on_event(
        &mouse(MouseKind::Drag(MouseButton::Left), 6, 6),
        &disabled,
        &view,
    );
    assert!(
        matches!(
            next,
            Some(EditPhase::Cancel {
                reason: CancelReason::TargetDisabled,
                ..
            })
        ),
        "a target going inert mid-drag must cancel with TargetDisabled, got {next:?}"
    );
    assert!(!g.is_active());
}

// ===========================================================================
// Level 2 — the decisive painted-layout fixture
// ===========================================================================

/// A keyed, interactive target that fills its whole rect with a sentinel color
/// (same idiom as tests/ui_hit_paint_unification.rs).
fn ui_target(key: &str, bg: Color, w: u16, h: u16) -> Element<()> {
    raw(Node::col().background(bg).width(w as f32).height(h as f32))
        .key(key)
        .on_press(())
}

fn render(el: Element<()>, w: u16, h: u16) -> Compiled<()> {
    let env = UiEnvironment {
        width: w,
        height: h,
        ..UiEnvironment::default()
    };
    let cx = BuildCx::new(skins::SWISS_SIGNAL, env);
    let mut compiled = compile(&el, &cx).expect("scene compiles");
    compute_layout(&mut compiled.node, w, h).expect("layout resolves");
    compiled
}

#[test]
fn painted_disabled_occluder_is_inert_end_to_end_and_an_enabled_sibling_still_edits() {
    // The decisive fixture (Finding E): an enabled BG fully covered by a disabled
    // TOP (a `stack` overlay puts TOP on top), plus an enabled LIVE sibling.
    // Resolve the REAL painted layout; a drag on TOP must be denied (no
    // Begin/Commit) while a drag on LIVE commits.
    let bg = Color::Rgb(40, 40, 40);
    let top = Color::Rgb(130, 130, 130);
    let live = Color::Rgb(10, 210, 10);
    let scene = screen().child(
        column()
            .child(
                stack()
                    .width(16)
                    .height(8)
                    .child(ui_target("BG", bg, 16, 8))
                    .child(ui_target("TOP", top, 16, 8).disabled(true)),
            )
            .child(ui_target("LIVE", live, 16, 4)),
    );
    let compiled = render(scene, 40, 20);
    let hit = compiled.hit_map(Rect::new(0, 0, 40, 20)).expect("laid out");

    // The painted TOP is a disabled occluder, carried into the index as inert.
    let top_t = hit
        .targets()
        .iter()
        .find(|t| matches!(&t.key, Key::Named(s) if s == "TOP"))
        .expect("TOP resolved");
    assert_eq!(top_t.eligibility, Eligibility::Disabled);
    assert!(
        hit.layers().is_inert(top_t.id),
        "a painted disabled target must be inert in the spatial index"
    );

    // At TOP's centre the hit map resolves TOP (Disabled), NOT the enabled BG
    // beneath it: the occluder swallows the click, there is no fall-through.
    let (tx, ty) = (
        top_t.rect.x + top_t.rect.width / 2,
        top_t.rect.y + top_t.rect.height / 2,
    );
    let at_top = hit.hit(tx, ty).expect("a target at TOP's centre");
    assert!(matches!(&at_top.key, Key::Named(s) if s == "TOP"));
    assert_eq!(at_top.eligibility, Eligibility::Disabled);

    let wview = WorldView::new(Rect::new(0, 0, 40, 20)).with_pixels_per_cell(1, 1);
    let mut g = DragGesture::new();
    let denied = run(
        &mut g,
        hit.layers(),
        &wview,
        &[
            mouse(MouseKind::Down(MouseButton::Left), tx, ty),
            mouse(MouseKind::Drag(MouseButton::Left), tx + 1, ty + 1),
            mouse(MouseKind::Up(MouseButton::Left), tx + 1, ty + 1),
        ],
    );
    assert!(
        denied.is_empty(),
        "a drag on the painted disabled occluder must be denied, got {denied:?}"
    );

    // Control: the enabled LIVE sibling still takes a drag end-to-end — so the
    // denial above is eligibility talking, not a broken pipeline.
    let live_t = hit
        .targets()
        .iter()
        .find(|t| matches!(&t.key, Key::Named(s) if s == "LIVE"))
        .expect("LIVE resolved");
    assert_eq!(live_t.eligibility, Eligibility::Hittable);
    let (lx, ly) = (
        live_t.rect.x + live_t.rect.width / 2,
        live_t.rect.y + live_t.rect.height / 2,
    );
    let mut g2 = DragGesture::new();
    let allowed = run(
        &mut g2,
        hit.layers(),
        &wview,
        &[
            mouse(MouseKind::Down(MouseButton::Left), lx, ly),
            mouse(MouseKind::Up(MouseButton::Left), lx, ly),
        ],
    );
    assert_eq!(
        count(&allowed, |p| matches!(p, EditPhase::Commit { .. })),
        1,
        "the enabled sibling must still commit, got {allowed:?}"
    );
}
