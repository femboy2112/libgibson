//! Wave 6 — the instrument workbench, driven headlessly as consumer code (#144).
//!
//! These are the cross-layer SEAM oracles: for each place where two subsystems meet
//! (painted layout ↔ pointer, world ↔ field cell, slider ↔ model, frame tick ↔ wall
//! time, drag episode ↔ undo transaction, receipt ↔ projection), we drive the real
//! composition and then DELIBERATELY MUTATE the input so the check fails — a passing
//! control plus a failing mutant, never a tautology. The workbench model/composition
//! is the SAME `workbench` module the runnable example uses, included here directly.

#[path = "../examples/instrument_workbench/workbench.rs"]
mod workbench;

use workbench::{
    field_extent, Cmd, Intent, SceneModel, TransportCmd, Workbench, FIELD_COLS, FIELD_ROWS,
    SOURCE_MAX,
};

use gibson::edit::{Compound, Reversible};
use gibson::field_view::{Colormap, FieldExtent, FieldView};
use gibson::input::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseKind};
use gibson::shape::{Point, ShapeId};
use gibson::FixedTimestep;
use std::time::Duration;

// ---- event helpers --------------------------------------------------------------------

fn mouse(kind: MouseKind, col: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    })
}
fn down(col: u16, row: u16) -> Event {
    mouse(MouseKind::Down(MouseButton::Left), col, row)
}
fn drag(col: u16, row: u16) -> Event {
    mouse(MouseKind::Drag(MouseButton::Left), col, row)
}
fn up(col: u16, row: u16) -> Event {
    mouse(MouseKind::Up(MouseButton::Left), col, row)
}
fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}

fn wb_ready(w: u16, h: u16) -> Workbench {
    let mut wb = Workbench::new();
    let _ = wb.refresh_layout(w, h);
    wb
}

// ---- S1: pointer resolves ONLY the painted canvas target (L2) -------------------------

#[test]
fn s1_pointer_targets_the_painted_canvas_not_an_app_rect() {
    let wb = wb_ready(80, 24);
    let r = wb.canvas_rect();
    // Control: a cell inside the painted canvas rect resolves to the canvas target.
    assert!(
        wb.canvas_hit(r.x, r.y),
        "a cell on the painted canvas must resolve to it"
    );
    // Mutation: shift the pointer off the painted canvas (one row below it). The
    // painted hit-map must NOT claim it — targeting is paint-derived, not invented.
    let below = r.y + r.height;
    assert!(
        !wb.canvas_hit(r.x, below),
        "a cell off the painted canvas must not resolve to it"
    );
}

// ---- S2: world point ↔ field cell reconcile three ways (L3) ----------------------------

#[test]
fn s2_world_cell_reconciles_pointer_probe_and_index() {
    let wb = wb_ready(80, 24);
    let r = wb.canvas_rect();
    let (col, row) = (r.x + 2, r.y + 3);

    // pointer-selection path: terminal cell -> world
    let p = wb
        .cell_to_world_point(col, row)
        .expect("a canvas cell maps to a world point");
    // field path A: world -> field index
    let idx = field_extent().world_to_index(p);
    assert!(idx.is_some(), "a canvas cell maps to an in-field index");

    // field path B: the FieldView probe agrees with world_to_index, and its sample
    // agrees with a direct sample at that index.
    let (data, mask) = wb.field_data();
    let fv = FieldView::new(field_extent(), &data, Some(mask.as_slice())).unwrap();
    let probe = fv.probe(p);
    assert_eq!(
        probe.index, idx,
        "probe index and world_to_index must agree"
    );
    if let Some((ix, iy)) = idx {
        assert_eq!(probe.sample, fv.sample(ix, iy), "sample must agree");
    }

    // Mutation: shift the extent by one whole cell — the agreement must break.
    let shifted = FieldExtent::new(Point::new(1.0, 0.0), 1.0, 1.0, FIELD_COLS, FIELD_ROWS).unwrap();
    assert_ne!(
        shifted.world_to_index(p),
        idx,
        "a one-cell extent shift must change the resolved index"
    );
}

// ---- S3: numeric inspector refuses, never clamps (Finding D) ---------------------------

#[test]
fn s3_numeric_inspector_refuses_out_of_range() {
    let mut wb = wb_ready(80, 24);
    let before = wb.model.source_value;

    // Mutation: inject an out-of-range value. It must be refused and the model left
    // exactly as it was — NOT silently clamped to the max.
    wb.apply_intent(Intent::SetSource(SOURCE_MAX + 50.0));
    assert_eq!(
        wb.model.source_value, before,
        "out-of-range source must be refused, not clamped"
    );
    assert!(wb.last_numeric_error.is_some(), "refusal must be disclosed");

    // Control: an in-range value is accepted.
    wb.apply_intent(Intent::SetSource(3.0));
    assert_eq!(wb.model.source_value, 3.0);
    assert!(wb.last_numeric_error.is_none());
}

// ---- S4: the field follows the MODEL tick, never wall time (L1 purity + L6) ------------

#[test]
fn s4_field_follows_model_tick_not_wall_time() {
    let mut wb = wb_ready(80, 24);
    let (d0, _) = wb.field_data();
    let before_digest = wb.digest();

    // Rendering and probing repeatedly must not advance or mutate the field (L1).
    for _ in 0..5 {
        let _ = wb.render_field(wb.canvas_rect());
        let _ = wb.probe(Point::new(5.0, 5.0));
    }
    let (d1, _) = wb.field_data();
    assert_eq!(d0, d1, "the field must not change without a model tick");
    assert_eq!(
        before_digest,
        wb.digest(),
        "rendering must not touch the model"
    );

    // Advancing the model tick DOES evolve the field.
    wb.apply_intent(Intent::Transport(TransportCmd::Step(7)));
    let (d2, _) = wb.field_data();
    assert_ne!(d1, d2, "advancing the model tick must evolve the field");
    assert_ne!(
        before_digest,
        wb.digest(),
        "the tick is part of the model state"
    );
}

// ---- S5: a pointer outside the viewport never becomes a bogus in-field coord (L2/L3) ---

#[test]
fn s5_pointer_outside_viewport_has_no_in_field_coordinate() {
    let wb = wb_ready(80, 24);
    let r = wb.canvas_rect();
    // Control: inside the canvas maps to a world point.
    assert!(wb.cell_to_world_point(r.x, r.y).is_some());

    // Mutation: a cell far past the canvas must EITHER clip (None) or map to a world
    // point that is outside the field — never a fabricated in-field cell.
    let far = r.x.saturating_add(r.width).saturating_add(60);
    match wb.cell_to_world_point(far, r.y) {
        None => {}
        Some(p) => assert!(
            field_extent().world_to_index(p).is_none(),
            "a far cell must not resolve to an in-field index"
        ),
    }
}

// ---- S6: a drag with a dropped release is cancelled, not left capturing ----------------

#[test]
fn s6_dropped_release_cancels_the_drag() {
    let mut wb = wb_ready(80, 24);
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(6.0, 5.0),
    });
    let r = wb.canvas_rect();
    let (col, row) = (r.x + 3, r.y + 3);
    let before = wb.digest();

    // Press and drag, but NEVER release; then Esc, then a resize — both must cancel the
    // drag. Nothing may have moved (no Commit was produced).
    wb.process(&down(col, row));
    wb.process(&drag(col + 2, row + 1));
    wb.process(&key(KeyCode::Esc));
    wb.process(&Event::Resize(80, 24));

    assert_eq!(
        before,
        wb.digest(),
        "a drag with no release must not move anything"
    );
}

// ---- S7: keyboard and pointer produce equivalent canonical state ----------------------

#[test]
fn s7_keyboard_and_pointer_moves_reach_the_same_state() {
    let shape = || Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(6.0, 5.0),
    };

    // Pointer path: a real drag of (+2, +1) terminal cells over the selected shape.
    let mut a = wb_ready(80, 24);
    a.apply_intent(shape());
    let r = a.canvas_rect();
    let (col, row) = (r.x + 3, r.y + 3);
    a.process(&down(col, row));
    a.process(&drag(col + 2, row + 1));
    a.process(&up(col + 2, row + 1));

    // Keyboard path: Right, Right, Down — the same net (+2, +1) world move.
    let mut b = wb_ready(80, 24);
    b.apply_intent(shape());
    b.process(&key(KeyCode::Right));
    b.process(&key(KeyCode::Right));
    b.process(&key(KeyCode::Down));

    assert_eq!(
        a.model, b.model,
        "the same operation via pointer or keyboard must reach the same canonical model"
    );
    assert_eq!(a.digest(), b.digest());
}

// ---- S8: model/command digests are invariant under render rate/size/colour (ACC #2) ----

#[test]
fn s8_digests_invariant_under_render_policy_size_and_colour() {
    // A world-space trace (no pointer coords), so it is independent of terminal size.
    let trace = [
        Intent::DrawRect {
            a: Point::new(2.0, 2.0),
            b: Point::new(6.0, 5.0),
        },
        Intent::SetSource(7.0),
        Intent::Transport(TransportCmd::Step(9)),
        Intent::SelectAt(Point::new(3.0, 3.0)),
        Intent::MoveSelected { dx: 1.0, dy: 1.0 },
        Intent::Undo,
        Intent::Redo,
    ];

    let run = |color: Colormap, w: u16, h: u16, renders_between: usize| -> (u64, u64) {
        let mut wb = Workbench::new();
        wb.color_policy = color;
        let _ = wb.refresh_layout(w, h);
        for it in trace.iter().copied() {
            wb.apply_intent(it);
            // Vary the "render rate": render/relayout N times per step. None may touch
            // the model (L1), so the digests must be unaffected.
            for _ in 0..renders_between {
                let _ = wb.render_field(wb.canvas_rect());
                let _ = wb.refresh_layout(w, h);
            }
        }
        (wb.digest(), wb.command_digest)
    };

    let base = run(Colormap::Heat, 100, 36, 1);
    assert_eq!(
        base,
        run(Colormap::Grayscale, 100, 36, 3),
        "colour must not matter"
    );
    assert_eq!(
        base,
        run(Colormap::Heat, 40, 15, 5),
        "narrow terminal must not matter"
    );
    assert_eq!(
        base,
        run(Colormap::Density, 120, 40, 0),
        "render rate must not matter"
    );
}

// ---- S9: undo/redo round-trips, and a failing Compound rolls back atomically (L5/Fnd B)

#[test]
fn s9_undo_redo_round_trip_and_atomic_compound_rollback() {
    let mut wb = wb_ready(80, 24);
    let d0 = wb.digest();
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(6.0, 5.0),
    });
    let d1 = wb.digest();
    assert_ne!(d0, d1);
    wb.apply_intent(Intent::Undo);
    assert_eq!(wb.digest(), d0, "undo restores the pre-draw state");
    wb.apply_intent(Intent::Redo);
    assert_eq!(wb.digest(), d1, "redo re-applies it");

    // Finding B: a Compound whose second command fails must leave the model EXACTLY
    // as it was — the first command's effect is rolled back, the failure surfaced.
    let mut m = SceneModel {
        shapes: Vec::new(),
        source_value: 1.0,
    };
    let before = m.clone();
    let comp = Compound::new(vec![
        Cmd::SetSource { from: 1.0, to: 2.0 },
        Cmd::Move {
            id: ShapeId(999), // no such shape -> apply fails
            dx: 1.0,
            dy: 1.0,
        },
    ]);
    assert!(
        comp.apply(&mut m).is_err(),
        "a compound with a failing member must report failure"
    );
    assert_eq!(
        m, before,
        "a failed compound must leave the model unchanged"
    );
}

// ---- S10: receipts disclose masked/off-field and conserve (L7) -------------------------

#[test]
fn s10_receipts_disclose_and_conserve() {
    let mut wb = wb_ready(80, 24);
    // A rectangle "wall" masks the cells it covers.
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(6.0, 5.0),
    });
    let frame = wb.render_field(wb.canvas_rect());
    let r = frame.report;
    assert!(r.conserves(), "the render receipt must conserve cells");
    assert!(
        r.masked_cells > 0,
        "the wall must be disclosed as masked, not silently dropped"
    );

    // A probe far off the field is disclosed as off-field (no index), never a fake value.
    let off = wb.probe(Point::new(-5.0, -5.0));
    assert_eq!(off.index, None, "an off-field probe must disclose no index");
}

// ---- S-time: the tick-debt law, bounded vs exact (Finding C) ---------------------------

#[test]
fn s_time_tick_debt_law_bounded_vs_exact() {
    let step = Duration::from_millis(16);

    // Bounded policy: a 160 ms burst overflows the 8-tick cap and DISCARDS the rest.
    let mut bounded = FixedTimestep::with_max_ticks(step, 8);
    let d = bounded.advance(Duration::from_millis(160));
    assert_eq!(
        d.owed,
        d.executed + d.deferred + d.discarded,
        "Law IV: owed == executed + deferred + discarded"
    );
    assert!(
        d.discarded > 0 && d.deferred == 0,
        "bounded discards the overflow"
    );

    // Exact policy: the same burst DEFERS the overflow and never discards.
    let mut exact = FixedTimestep::exact(step, 8);
    let e = exact.advance(Duration::from_millis(160));
    assert_eq!(
        e.owed,
        e.executed + e.deferred + e.discarded,
        "Law IV: owed == executed + deferred + discarded"
    );
    assert_eq!(e.discarded, 0, "exact never discards");
    assert!(e.deferred > 0, "exact defers the overflow");
}

// ---- meta: the digest is non-vacuous (each kind of edit moves it) ----------------------

#[test]
fn digest_is_sensitive_to_each_kind_of_edit() {
    let mut wb = wb_ready(80, 24);
    let mut seen = std::collections::HashSet::new();
    assert!(seen.insert(wb.digest()));
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(5.0, 5.0),
    });
    assert!(seen.insert(wb.digest()), "drawing changes the digest");
    wb.apply_intent(Intent::SetSource(3.0));
    assert!(
        seen.insert(wb.digest()),
        "a source change changes the digest"
    );
    wb.apply_intent(Intent::Transport(TransportCmd::Step(1)));
    assert!(seen.insert(wb.digest()), "a tick changes the digest");
    // Panning changes the view, which IS part of the model digest. (Selection is
    // deliberately ephemeral and excluded, so it is not tested here.)
    wb.apply_intent(Intent::Pan { dx: 1.0, dy: 1.0 });
    assert!(seen.insert(wb.digest()), "a pan changes the digest");
}

// ---- meta: a trace replays deterministically (ACC #2 / replay) -------------------------

#[test]
fn trace_replays_deterministically() {
    let trace = [
        Intent::DrawRect {
            a: Point::new(2.0, 2.0),
            b: Point::new(6.0, 5.0),
        },
        Intent::SetSource(8.0),
        Intent::Transport(TransportCmd::Step(5)),
        Intent::SelectAt(Point::new(3.0, 3.0)),
        Intent::MoveSelected { dx: 2.0, dy: 1.0 },
        Intent::Undo,
    ];
    let run = || {
        let mut wb = wb_ready(80, 24);
        for it in trace.iter().copied() {
            wb.apply_intent(it);
        }
        wb
    };
    let a = run();
    let b = run();
    assert_eq!(a.digest(), b.digest(), "same trace -> same model digest");
    assert_eq!(
        a.command_digest, b.command_digest,
        "same trace -> same command digest"
    );
    assert_eq!(a.model, b.model);
}

// ---- S11: keyboard Tab-cycle selection reaches the SAME selection as a pointer click,
//           closing the keyboard-equivalence gap (#140/#144) that a pointer-only select left.

#[test]
fn s11_keyboard_tab_select_reaches_the_same_selection_as_pointer() {
    let s0 = Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(5.0, 4.0),
    };
    let s1 = Intent::DrawRect {
        a: Point::new(10.0, 2.0),
        b: Point::new(13.0, 4.0),
    };

    // Pointer path: click (SelectAt) inside the FIRST shape.
    let mut a = wb_ready(80, 24);
    a.apply_intent(s0);
    let s0_id = a.selection.primary().expect("draw selects the new shape");
    a.apply_intent(s1);
    a.apply_intent(Intent::SelectAt(Point::new(3.5, 3.0)));
    assert_eq!(
        a.selection.primary(),
        Some(s0_id),
        "pointer selects shape 0"
    );

    // Keyboard ONLY: after both draws the selection sits on shape 1 (last drawn); one
    // Tab wraps the insertion-order cycle [s0, s1] forward to shape 0. No pointer.
    let mut b = wb_ready(80, 24);
    b.apply_intent(s0);
    b.apply_intent(s1);
    b.process(&key(KeyCode::Tab));
    assert_eq!(
        b.selection.primary(),
        Some(s0_id),
        "keyboard Tab reaches shape 0"
    );

    // Equivalent authoritative selection via different input traces (#140/#144).
    assert_eq!(
        a.selection.ids(),
        b.selection.ids(),
        "pointer and keyboard select the same shape"
    );
}

#[test]
fn s11b_tab_and_backtab_cycle_every_shape_deterministically() {
    let mut wb = wb_ready(80, 24);
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(5.0, 4.0),
    });
    let s0 = wb.selection.primary().unwrap();
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(10.0, 2.0),
        b: Point::new(13.0, 4.0),
    });
    let s1 = wb.selection.primary().unwrap();
    wb.apply_intent(Intent::DrawLine {
        a: Point::new(2.0, 8.0),
        b: Point::new(9.0, 8.0),
    });
    let s2 = wb.selection.primary().unwrap();
    assert!(s0 != s1 && s1 != s2 && s0 != s2, "three distinct shape ids");

    // Last-drawn is selected; Tab wraps forward s2 -> s0 -> s1 -> s2.
    wb.process(&key(KeyCode::Tab));
    assert_eq!(wb.selection.primary(), Some(s0));
    wb.process(&key(KeyCode::Tab));
    assert_eq!(wb.selection.primary(), Some(s1));
    wb.process(&key(KeyCode::Tab));
    assert_eq!(wb.selection.primary(), Some(s2));
    // BackTab (Shift+Tab) reverses.
    wb.process(&key(KeyCode::BackTab));
    assert_eq!(wb.selection.primary(), Some(s1));

    // Negative control: with nothing selected, Tab lands on the first shape.
    wb.apply_intent(Intent::ClearSelection);
    wb.process(&key(KeyCode::Tab));
    assert_eq!(
        wb.selection.primary(),
        Some(s0),
        "Tab from an empty selection selects the first shape"
    );
}

// ---- S12: topmost (last-drawn) overlapping shape wins the pick (L2/L3, narrow-phase) ----

#[test]
fn s12_pick_prefers_the_topmost_overlapping_shape() {
    let mut wb = wb_ready(80, 24);
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(12.0, 10.0),
    });
    let bottom = wb.selection.primary().unwrap();
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(5.0, 4.0),
        b: Point::new(9.0, 7.0),
    });
    let top = wb.selection.primary().unwrap();
    assert_ne!(bottom, top);

    // A point inside BOTH rects must select the LAST-drawn (topmost) one.
    wb.apply_intent(Intent::ClearSelection);
    wb.apply_intent(Intent::SelectAt(Point::new(7.0, 5.5)));
    assert_eq!(
        wb.selection.primary(),
        Some(top),
        "overlap selects the topmost (last-drawn) shape"
    );

    // Negative control: a point inside ONLY the bottom rect, well outside the top
    // (>= 2 world units, beyond any half-cell pick tolerance), selects the bottom.
    wb.apply_intent(Intent::ClearSelection);
    wb.apply_intent(Intent::SelectAt(Point::new(3.0, 6.0)));
    assert_eq!(
        wb.selection.primary(),
        Some(bottom),
        "a bottom-only region selects the lower shape"
    );
}

// ---- S13: the FULL interaction, not just the selected id — reaching shape 0 by a pointer
//           click vs a keyboard Tab and then applying the SAME move must reach the same
//           canonical edit, and recover identically under undo/redo (#140/#144).

#[test]
fn s13_tab_select_then_move_equals_pointer_select_then_move() {
    // We compare the authoritative SceneModel (shape geometry), not a pixel image and not
    // the probe channel a click also updates — raw input traces may differ, the edit must
    // not. The move leg is arrow keys in BOTH paths (drag ≡ arrow move is s7; click →
    // SelectAt is s1), so the SELECTION path is the only variable under test.
    let s0 = || Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(5.0, 4.0),
    };
    let s1 = || Intent::DrawRect {
        a: Point::new(10.0, 2.0),
        b: Point::new(13.0, 4.0),
    };

    // Pointer path: click inside shape 0 (SelectAt), then Right, Right, Down.
    let mut a = wb_ready(80, 24);
    a.apply_intent(s0());
    a.apply_intent(s1());
    a.apply_intent(Intent::SelectAt(Point::new(3.5, 3.0)));
    a.process(&key(KeyCode::Right));
    a.process(&key(KeyCode::Right));
    a.process(&key(KeyCode::Down));

    // Keyboard-only path: Tab selects shape 0 (wraps forward from the last-drawn s1).
    let mut b = wb_ready(80, 24);
    b.apply_intent(s0());
    b.apply_intent(s1());
    b.process(&key(KeyCode::Tab));
    b.process(&key(KeyCode::Right));
    b.process(&key(KeyCode::Right));
    b.process(&key(KeyCode::Down));

    assert_eq!(
        a.selection.ids(),
        b.selection.ids(),
        "both paths keep shape 0 selected"
    );
    assert_eq!(
        a.model, b.model,
        "select-then-move reaches the same canonical edit by either selection path"
    );

    // Undo/redo recovery is identical across the two input provenances: the move sits on
    // top of both undo stacks (selection is not an edit), so Undo/Redo move in lockstep.
    a.apply_intent(Intent::Undo);
    b.apply_intent(Intent::Undo);
    assert_eq!(
        a.model, b.model,
        "undo recovers the same model on both paths"
    );
    a.apply_intent(Intent::Redo);
    b.apply_intent(Intent::Redo);
    assert_eq!(
        a.model, b.model,
        "redo re-applies the same model on both paths"
    );
}

// ---- S14: selection identity survives an intervening add + delete — a keyboard cycle must
//           never target a retired shape, and must resume over the live set in order (L5).

#[test]
fn s14_selection_identity_survives_intervening_add_then_delete() {
    let mut wb = wb_ready(80, 24);
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(2.0, 2.0),
        b: Point::new(5.0, 4.0),
    });
    let s0 = wb.selection.primary().unwrap();
    wb.apply_intent(Intent::DrawRect {
        a: Point::new(10.0, 2.0),
        b: Point::new(13.0, 4.0),
    });
    let s1 = wb.selection.primary().unwrap();

    // Keyboard-select shape 0 (Tab wraps forward from the last-drawn s1).
    wb.process(&key(KeyCode::Tab));
    assert_eq!(wb.selection.primary(), Some(s0));

    // Intervening add (auto-selects the new shape), then undo retires it.
    wb.apply_intent(Intent::DrawLine {
        a: Point::new(2.0, 8.0),
        b: Point::new(9.0, 8.0),
    });
    let s2 = wb.selection.primary().unwrap();
    assert!(s2 != s0 && s2 != s1, "the added shape has a distinct id");
    wb.apply_intent(Intent::Undo); // retires s2
    assert_eq!(wb.model.shapes.len(), 2, "back to two live shapes");
    assert_ne!(
        wb.selection.primary(),
        Some(s2),
        "selection must not dangle on a retired shape (L5)"
    );

    // A fresh Tab cycle targets only the live set [s0, s1], in insertion order.
    wb.apply_intent(Intent::ClearSelection);
    wb.process(&key(KeyCode::Tab));
    assert_eq!(
        wb.selection.primary(),
        Some(s0),
        "Tab targets the first live shape"
    );
    wb.process(&key(KeyCode::Tab));
    assert_eq!(wb.selection.primary(), Some(s1), "then the second");
}

// ---- Phase 3 (Renaissance IV): model-tick arithmetic never silently rewinds --------
//
// The old `apply_transport` did `field_tick = (field_tick as i64 + n).max(0) as u64`,
// which is not valid over the full u64 domain: the `as i64` cast reinterprets a tick
// above i64::MAX as negative, so a forward step could WRAP DOWN. These pin a saturating
// policy over the whole domain (red-first: the first three fail on the old arithmetic).

#[test]
fn step_saturates_at_the_tick_ceiling_instead_of_rewinding() {
    // At field_tick = u64::MAX a forward Step must stay at the ceiling, not wrap to 0.
    // Old: (u64::MAX as i64 + 1).max(0) = (-1 + 1).max(0) = 0 — a full rewind from the
    // maximum representable tick on a single forward step.
    let mut wb = Workbench::new();
    wb.field_tick = u64::MAX;
    wb.apply_intent(Intent::Transport(TransportCmd::Step(1)));
    assert_eq!(
        wb.field_tick,
        u64::MAX,
        "a forward step must never rewind time"
    );
}

#[test]
fn step_does_not_overflow_near_i64_max() {
    // field_tick just above i64::MAX: the old `as i64` add overflowed (debug panic /
    // release wrap). A forward step must advance, saturating.
    let mut wb = Workbench::new();
    wb.field_tick = i64::MAX as u64;
    wb.apply_intent(Intent::Transport(TransportCmd::Step(1)));
    assert_eq!(
        wb.field_tick,
        i64::MAX as u64 + 1,
        "step must advance across the i64 boundary, not overflow"
    );
}

#[test]
fn large_forward_step_saturates_not_truncates() {
    // Old path truncated through i64 and produced a mid-range value; saturating is
    // correct: a huge forward step clamps at the ceiling.
    let mut wb = Workbench::new();
    wb.field_tick = u64::MAX - 3;
    wb.apply_intent(Intent::Transport(TransportCmd::Step(i64::MAX)));
    assert_eq!(
        wb.field_tick,
        u64::MAX,
        "a large forward step clamps at the ceiling"
    );
}

#[test]
fn step_back_from_zero_and_i64_min_saturate_at_zero() {
    // Boundary coverage (valid on both paths): never underflow below 0, and handle the
    // i64::MIN magnitude (whose abs does not fit in i64) without overflow.
    let mut wb = Workbench::new();
    wb.field_tick = 0;
    wb.apply_intent(Intent::Transport(TransportCmd::Step(-1)));
    assert_eq!(wb.field_tick, 0, "cannot step before the start");
    wb.field_tick = 10;
    wb.apply_intent(Intent::Transport(TransportCmd::Step(i64::MIN)));
    assert_eq!(
        wb.field_tick, 0,
        "i64::MIN magnitude handled without overflow"
    );
}

#[test]
fn tick_wall_saturates_at_the_ceiling_and_never_overflows() {
    // Wall-clock advance near the ceiling must clamp, not overflow (debug panic /
    // release wrap). Old: `field_tick += demand.executed as u64`.
    let mut wb = Workbench::new();
    wb.playing = true;
    wb.field_tick = u64::MAX;
    let demand = wb.tick_wall(Duration::from_millis(64));
    assert!(demand.is_some(), "transport is playing");
    assert_eq!(wb.field_tick, u64::MAX, "model time clamps at the ceiling");
}
