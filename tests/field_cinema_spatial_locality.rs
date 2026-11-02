//! Spatial causal-locality battery for the source-driven cinematic slice (#191, v0.6 Gate J).
//!
//! Gate I's `field_cinema_causal_locality.rs` fenced TEMPORAL locality (an edit at tick `T` cannot
//! leak to ticks `< T`) and explicitly ceded SPATIAL confinement. This fences the spatial axis: a
//! *sub-threshold* field edit at ONE cell must change only the terminal cells that cell maps to —
//! it must not recolour or redraw cells elsewhere in the same frame.
//!
//! The honest scope, stated up front. The shipped pipeline couples the whole frame through global
//! scalars — the `Auto` colour range (`finite_extremes`), the camera focus (global argmax), the
//! zoom (global span), the colormap (global masked fraction) and the probe's own reading. A
//! super-threshold edit that moves any of those legitimately recolours/reframes the whole frame;
//! that is a feature, not a leak. So the guarantee carries an explicit PRECONDITION: the edit
//! leaves observation, shot AND the resolved range unchanged. Under it, at the render stage, each
//! terminal cell is an independent point-sample of ONE field cell (no kernel, no neighbour read,
//! no inter-cell state), so confinement holds BY CONSTRUCTION today — this battery is a REGRESSION
//! FENCE for the first spatially-coupling feature #191 adds (a blur/neighbourhood kernel, a
//! windowed or probe-relative normalization, a smoothed camera). The teeth test exhibits exactly
//! such a coupling (a colour scale pinned to the edited cell's value) and shows the spatial
//! assertion catches it while determinism cannot.
//!
//! The mapped set is computed with the `probe_cell` ORACLE — the same `terminal-cell → world →
//! field-index` path the renderer paints through — never a hard-coded 1:1, because at the shot's
//! zoom one field cell spans several terminal columns (or none, off-frame).
//!
//! Perturbation goes through the same test-local `stage_from` twin as the temporal battery (the
//! shipped `stage_tick` builds its own buffers and has no injection point); a control pins the twin.

#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;

use std::collections::HashSet;

use cinema::{
    compose, observe, render_shot, stage_tick, view_focused_on, Model, SourceId, Staged, COLS, ROWS,
};
use gibson::field_view::{FieldView, RangePolicy};
use gibson::plot::FiniteRange;
use gibson::shape::Point;
use gibson::{Rect, Surface};

const ANCHOR: Rect = Rect {
    x: 0,
    y: 0,
    width: 56,
    height: 18,
};

const T: u64 = 4;

fn model() -> Model {
    Model::new(SourceId(7), 1)
}

/// A faithful twin of `cinema::stage_tick` that renders from CALLER-supplied buffers, so a
/// perturbation can be injected (identical to the temporal battery's seam).
fn stage_from(
    m: &Model,
    tick: u64,
    data: &[f64],
    mask: &[bool],
    sel: Point,
    anchor: Rect,
) -> Staged {
    let view = FieldView::new(m.extent(), data, Some(mask)).expect("geometry matches data/mask");
    let observation = observe(&view, m.source(), m.epoch(), tick);
    let probe = view.probe(sel);
    let shot = compose(&observation, &probe, m.extent());
    let frame = render_shot(&view, &shot, anchor);
    Staged {
        observation,
        shot,
        frame,
    }
}

/// Shared base state for tick `T`: the unperturbed staged frame, the global range, a chosen
/// interior cell `p` to perturb sub-threshold, its replacement value, and the terminal cells `p`
/// maps to (via the `probe_cell` oracle). `p` is chosen so the edit is sub-threshold BY
/// CONSTRUCTION: its value is strictly inside `(lo, hi)` (so no in-range replacement can move the
/// global extremes), it is visible, unmasked, and is not the probe's own cell.
struct Setup {
    m: Model,
    tick: u64,
    sel: Point,
    data: Vec<f64>,
    mask: Vec<bool>,
    base: Staged,
    lo: f64,
    span: f64,
    p: usize,
    p_tuple: (usize, usize),
    new: f64,
    mapped: Vec<(u16, u16)>,
}

fn setup() -> Setup {
    let m = model();
    let tick = T;
    let sel = m.hotspot_center(tick);
    let (data, mask) = m.frame_buffers(tick);
    let base = stage_from(&m, tick, &data, &mask, sel, ANCHOR);

    let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let (lo, hi) = view
        .finite_extremes()
        .expect("the field has finite samples");
    let span = hi - lo;
    assert!(span > 0.0, "the field must have a non-degenerate range");
    let wv = view_focused_on(base.shot.focus, base.shot.zoom, ANCHOR);
    let sel_idx = view.probe(sel).index;

    // Field cells the view actually shows (mapped from at least one terminal cell).
    let mut visible: HashSet<(usize, usize)> = HashSet::new();
    for row in 0..ANCHOR.height {
        for col in 0..ANCHOR.width {
            if let Some(rep) = view.probe_cell(&wv, col, row) {
                if let Some(idx) = rep.index {
                    visible.insert(idx);
                }
            }
        }
    }

    // An interior, unmasked, visible cell whose value is STRICTLY inside (lo, hi) — so any in-range
    // replacement preserves the global extremes — and which is not the probe's own cell.
    let p = (0..COLS * ROWS)
        .find(|&i| {
            mask[i]
                && lo < data[i]
                && data[i] < hi
                && visible.contains(&(i % COLS, i / COLS))
                && Some((i % COLS, i / COLS)) != sel_idx
        })
        .expect("an interior, visible, unmasked, non-probe cell exists in a 48x24 field");
    let p_tuple = (p % COLS, p / COLS);

    // A visibly different in-range replacement (>= 0.4*span away, strictly inside the edges).
    let old = data[p];
    let new = if old < lo + 0.5 * span {
        hi - 0.1 * span
    } else {
        lo + 0.1 * span
    };
    assert!(
        lo < new && new < hi,
        "replacement must stay strictly in range"
    );
    assert!(
        (new - old).abs() >= 0.3 * span,
        "edit must be visible under the Heat colormap"
    );

    // The mapped set: terminal cells whose probe resolves to field cell p.
    let mut mapped = Vec::new();
    for row in 0..ANCHOR.height {
        for col in 0..ANCHOR.width {
            if let Some(rep) = view.probe_cell(&wv, col, row) {
                if rep.index == Some(p_tuple) {
                    mapped.push((col, row));
                }
            }
        }
    }

    Setup {
        m,
        tick,
        sel,
        data,
        mask,
        base,
        lo,
        span,
        p,
        p_tuple,
        new,
        mapped,
    }
}

/// Cells where two surfaces differ, excluding an allowed set.
fn leaked_cells(base: &Surface, edit: &Surface, allowed: &HashSet<(u16, u16)>) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    for row in 0..base.height {
        for col in 0..base.width {
            if !allowed.contains(&(col, row)) && base.get(col, row) != edit.get(col, row) {
                out.push((col, row));
            }
        }
    }
    out
}

// --- control: the test-local seam is a faithful twin of the shipped stage_tick ---------------

#[test]
fn stage_from_twin_matches_stage_tick_unperturbed() {
    let m = model();
    for t in 0..6u64 {
        let (data, mask) = m.frame_buffers(t);
        let sel = m.hotspot_center(t);
        let twin = stage_from(&m, t, &data, &mask, sel, ANCHOR);
        let real = stage_tick(&m, t, sel, ANCHOR);
        assert_eq!(
            twin.observation, real.observation,
            "twin observation diverged @ {t}"
        );
        assert_eq!(twin.shot, real.shot, "twin shot diverged @ {t}");
        assert_eq!(
            twin.frame.surface, real.frame.surface,
            "twin pixels diverged @ {t} — the seam is not faithful"
        );
    }
}

// --- spatial locality: a sub-threshold edit confines to its mapped cells ---------------------

#[test]
fn a_subthreshold_interior_edit_confines_to_its_mapped_cells() {
    let s = setup();

    let mut edit_data = s.data.clone();
    edit_data[s.p] = s.new;
    let edit = stage_from(&s.m, s.tick, &edit_data, &s.mask, s.sel, ANCHOR);

    // Sub-threshold PRECONDITIONS — asserted, never skipped. The edit moved no global scalar, so
    // observation, shot and the resolved range are unchanged.
    assert_eq!(
        s.base.observation, edit.observation,
        "edit moved a global observation scalar — not sub-threshold"
    );
    assert_eq!(
        s.base.shot, edit.shot,
        "edit moved the shot — not sub-threshold"
    );
    let edit_view = FieldView::new(s.m.extent(), &edit_data, Some(&s.mask)).unwrap();
    let (lo2, hi2) = edit_view.finite_extremes().unwrap();
    assert!(
        (lo2 - s.lo).abs() < f64::EPSILON && (hi2 - (s.lo + s.span)).abs() < f64::EPSILON,
        "edit moved the global range — not sub-threshold"
    );

    // Non-vacuity: the cell is actually on screen and actually changed colour.
    assert!(
        !s.mapped.is_empty(),
        "field cell {:?} maps to no terminal cell — pick a visible cell",
        s.p_tuple
    );
    let mapped_set: HashSet<(u16, u16)> = s.mapped.iter().copied().collect();
    let changed_in_mapped = s
        .mapped
        .iter()
        .any(|&(c, r)| s.base.frame.surface.get(c, r) != edit.frame.surface.get(c, r));
    assert!(
        changed_in_mapped,
        "the edit changed no mapped cell — not a visible perturbation"
    );

    // Confinement: every cell OUTSIDE the mapped set is byte-identical.
    let leaked = leaked_cells(&s.base.frame.surface, &edit.frame.surface, &mapped_set);
    assert!(
        leaked.is_empty(),
        "sub-threshold edit at field cell {:?} (mapped to {} terminal cells) leaked to {} non-mapped cells: {:?}",
        s.p_tuple,
        s.mapped.len(),
        leaked.len(),
        &leaked[..leaked.len().min(12)]
    );
}

// --- spatial locality: a masked cell reaches no terminal cell --------------------------------

#[test]
fn a_masked_cell_edit_reaches_no_terminal_cell() {
    let m = model();
    let tick = T;
    let sel = m.hotspot_center(tick);
    let (data, mask) = m.frame_buffers(tick);
    let base = stage_from(&m, tick, &data, &mask, sel, ANCHOR);

    let mp = (0..COLS * ROWS)
        .find(|&i| !mask[i])
        .expect("the model has a dropout patch of masked cells");
    let mut edit_data = data.clone();
    edit_data[mp] = 1.0e6; // enormous — but masked, so never read
    let edit = stage_from(&m, tick, &edit_data, &mask, sel, ANCHOR);

    assert_eq!(
        base.observation, edit.observation,
        "a masked cell reached observation"
    );
    assert_eq!(base.shot, edit.shot, "a masked cell reached the shot");
    assert_eq!(
        base.frame.surface, edit.frame.surface,
        "a masked cell reached the pixels — Sample::Masked must win over the stored value"
    );
}

// --- teeth: a global-coupled pipeline recolours distant cells; determinism cannot see it ------

/// A deterministic but spatially NON-local renderer: the colour scale top is pinned to `v_top` (as
/// a "normalize to the edited/probed cell" coupling would), so that one value drives every cell's
/// colour. A local edit that changes `v_top` then recolours cells far from it.
fn render_coupled(
    m: &Model,
    data: &[f64],
    mask: &[bool],
    sel: Point,
    tick: u64,
    anchor: Rect,
    v_top: f64,
) -> Surface {
    let view = FieldView::new(m.extent(), data, Some(mask)).unwrap();
    let (lo, _) = view.finite_extremes().unwrap();
    let obs = observe(&view, m.source(), m.epoch(), tick);
    let shot = compose(&obs, &view.probe(sel), m.extent());
    let wv = view_focused_on(shot.focus, shot.zoom, anchor);
    let range = FiniteRange::new(lo, v_top).expect("lo < v_top for the coupled scale");
    view.render(&wv, anchor, shot.colormap, RangePolicy::Fixed(range))
        .surface
}

#[test]
fn determinism_is_blind_to_a_global_recolor_but_spatial_locality_is_not() {
    let s = setup();
    let old = s.data[s.p];

    // Base: coupled to the cell's current value. Edit: the cell becomes `new`, and (the coupling)
    // the scale top follows it. Only field cell p's DATA changed, yet the scale top moved globally.
    let base_surf = render_coupled(&s.m, &s.data, &s.mask, s.sel, s.tick, ANCHOR, old);
    let mut edit_data = s.data.clone();
    edit_data[s.p] = s.new;
    let edit_surf = render_coupled(&s.m, &edit_data, &s.mask, s.sel, s.tick, ANCHOR, s.new);

    // Determinism is BLIND: the coupled pipeline replays byte-identically, so a determinism law
    // would happily certify it.
    let edit_again = render_coupled(&s.m, &edit_data, &s.mask, s.sel, s.tick, ANCHOR, s.new);
    assert_eq!(
        edit_surf, edit_again,
        "the coupled pipeline is still deterministic"
    );

    // Spatial locality CATCHES it: cells OUTSIDE p's mapped set recolour under the shifted scale.
    let mapped_set: HashSet<(u16, u16)> = s.mapped.iter().copied().collect();
    let leaked = leaked_cells(&base_surf, &edit_surf, &mapped_set);
    assert!(
        !leaked.is_empty(),
        "a global-coupled pipeline must recolour cells outside the edited cell's mapped set — the spatial fence caught nothing"
    );
}
