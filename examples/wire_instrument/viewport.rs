// Shared by the example and by `tests/wire_instrument_viewport.rs` via `#[path]`; follow the
// channel/transport convention and quiet dead_code for the whole module.
#![allow(dead_code)]
//! Viewport sizing for `wire_instrument` (Renaissance VIII, Gate G).
//!
//! The field the producer streams may legally be large — up to `2^20` cells, so a dimension can
//! exceed `u16::MAX`. The display viewport is capped to the terminal budget. The one rule that
//! makes this honest: **do the cell arithmetic wide and clamp to the budget BEFORE narrowing to
//! `u16`.** The pre-fix code wrote `(ext.cols() as u16).saturating_mul(ppc).clamp(1, 240)`, which
//! truncates `cols` to its low 16 bits first — a 65536-column field wrapped to `0` and then
//! clamped *up* to width 1, and a 65537-column field to width 1, so two wildly different fields
//! rendered at the same tiny size and the displayed viewport no longer corresponded to the source
//! geometry the probe indexes.

/// The terminal display budget: a field wider/taller than this is shown clamped to the cap, not
/// wrapped to a sliver.
pub const MAX_VIEW_COLS: u64 = 240;
pub const MAX_VIEW_ROWS: u64 = 80;

/// Viewport cell dimensions for a `cols`×`rows` field at `ppc` pixels/cell, clamped to the display
/// budget. Computed in `u64` and clamped before the narrowing cast, so a legal field dimension past
/// `u16::MAX` saturates to the cap instead of wrapping. `ppc` is floored at 1 (a zero zoom would
/// otherwise collapse the view). Both outputs are in `[1, cap]`, so they always fit `u16`.
pub fn viewport_dims(cols: usize, rows: usize, ppc: u16) -> (u16, u16) {
    let ppc = (ppc.max(1)) as u64;
    let w = (cols as u64).saturating_mul(ppc).clamp(1, MAX_VIEW_COLS) as u16;
    let h = (rows as u64).saturating_mul(ppc).clamp(1, MAX_VIEW_ROWS) as u16;
    (w, h)
}

/// The display transport for a field, derived from the SAME source geometry the probe indexes
/// (Gate G). Sizing alone ([`viewport_dims`]) is not enough: a field carries a world **origin** and
/// possibly **anisotropic** cell sizes (`cell_w != cell_h`), and the display must honor both or the
/// rendered field and the index-based probe disagree. A WorldView built with the default pan `(0,0)`
/// and an isotropic `pixels_per_cell(ppc, ppc)` maps a nonzero-origin field entirely off-view (it
/// renders blank while `FieldView::sample` still returns values) and distorts an anisotropic one.
///
/// This builds the transport so terminal cell `(ix·ppc .. , iy·ppc ..)` maps back to field index
/// `(ix, iy)`: pan to the field `origin`; per-axis `zoom = ppc / cell_size` (pixels per world unit),
/// which transports anisotropy and magnifies one source cell to a `ppc × ppc` block; and
/// `pixels_per_cell = (1, 1)` so one terminal cell is one raster pixel. `cell_w`/`cell_h` are
/// validated strictly positive and finite by `FieldExtent::new`, so the divisions are safe.
/// Returns `(view, area)` for `FieldView::render`; `area` is the clamped viewport rectangle.
pub fn world_view_for(
    ext: &gibson::field_view::FieldExtent,
    ppc: u16,
) -> Result<(gibson::world_view::WorldView, gibson::Rect), UnsupportedView> {
    let ppc = ppc.max(1);
    let (w, h) = viewport_dims(ext.cols(), ext.rows(), ppc);
    let area = gibson::Rect::new(0, 0, w, h);
    let origin = ext.origin();
    let zoom_x = f64::from(ppc) / ext.cell_w();
    let zoom_y = f64::from(ppc) / ext.cell_h();
    // Gate D/D3: `FieldExtent::new` accepts ANY strictly-positive finite cell size — including a
    // subnormal one — so `ppc / cell_size` can overflow to a non-finite zoom (and a zero zoom is
    // likewise non-invertible). `WorldView` has no finite inverse for such a zoom, so every cell
    // would map to `Transport::Clipped`: the field renders blank while `FieldView::sample` still
    // returns values — the exact display/probe disagreement Gate G closed, resurfacing in the
    // overflow regime. Rather than hand back that silent contradiction, refuse with a typed
    // outcome so the caller can show an honest "unsupported view" instead of a mystery blank.
    if !zoom_x.is_finite() || !zoom_y.is_finite() || zoom_x == 0.0 || zoom_y == 0.0 {
        return Err(UnsupportedView { zoom_x, zoom_y });
    }
    let view = gibson::world_view::WorldView::new(area)
        .with_pan(origin.x, origin.y)
        .with_pixels_per_cell(1, 1)
        .with_zoom(zoom_x, zoom_y);
    Ok((view, area))
}

/// Top-left field-cell index of the region shown on ONE axis, so the probed cell stays inside the
/// `view`-cell window — centered when the field is larger than the window. Pure, stateless and
/// total: given the axis length `len`, the number of cells the window shows (`view`), and the probe
/// index, it returns a pan in `[0, len - view]` (or `0` when the whole axis fits). The invariants the
/// display correspondence rests on: the probe is always inside `[pan, pan + view)`, and the pan never
/// scrolls past `len - view` (so the window is always full, never showing past the end of the field).
pub fn roi_axis(len: usize, view: usize, probe: usize) -> usize {
    let view = view.max(1);
    if len <= view {
        return 0; // the whole axis fits the window — anchor at the origin, never pan
    }
    let max_pan = len - view; // the furthest top-left that still fills the window
    let probe = probe.min(len - 1); // defensive: an out-of-range probe pins to the last cell
                                    // Center the probe in the window, then clamp into the fill-the-window range.
    probe.saturating_sub(view / 2).min(max_pan)
}

/// The region-of-interest origin (top-left field cell) for a probe, derived from the rendered `area`
/// and `ppc`. The window shows `area.width / ppc` columns and `area.height / ppc` rows of field
/// cells (each source cell magnified to a `ppc × ppc` block), so the visible cell counts — not the
/// pixel budget — are what [`roi_axis`] centers the probe within. Stateless: the same probe always
/// yields the same window, so navigation is deterministic and needs no cross-frame scroll state.
pub fn roi_origin(
    cols: usize,
    rows: usize,
    area: gibson::Rect,
    ppc: u16,
    probe: (usize, usize),
) -> (usize, usize) {
    let ppc = ppc.max(1) as usize;
    let view_cols = (area.width as usize / ppc).max(1);
    let view_rows = (area.height as usize / ppc).max(1);
    (
        roi_axis(cols, view_cols, probe.0),
        roi_axis(rows, view_rows, probe.1),
    )
}

/// The display transport for a field that FOLLOWS the probe (RXIII Gate D, consumer navigation): a
/// region-of-interest viewport panned so the probed cell is always on-screen. Gate G
/// ([`world_view_for`]) derives the transport from the source geometry so the displayed field and
/// the index-based probe agree — but only while the whole field fits the display budget. A legal
/// field may be far larger than [`MAX_VIEW_COLS`]/[`MAX_VIEW_ROWS`]; the origin pan then leaves cells
/// beyond the window off-screen, and the probe (clamped only to `cols()-1`/`rows()-1`) can sit on
/// one — so the readout reports an exact value for a cell the operator cannot see. This re-pans the
/// Gate-G view by [`roi_origin`] so terminal cell `(ix·ppc, iy·ppc)` still maps back to field index
/// `(ix, iy)` for the WINDOW actually shown, and the probed cell is always within it.
///
/// The zoom, area and [`UnsupportedView`] refusal are inherited unchanged from [`world_view_for`] —
/// only the pan moves. When the whole field fits the budget the pan is identically the origin pan,
/// so the ordinary small-field case (and the compact path, and every prior Gate-G test) is untouched.
pub fn world_view_for_probe(
    ext: &gibson::field_view::FieldExtent,
    ppc: u16,
    probe: (usize, usize),
) -> Result<(gibson::world_view::WorldView, gibson::Rect), UnsupportedView> {
    let (view, area) = world_view_for(ext, ppc)?;
    let (pan_col, pan_row) = roi_origin(ext.cols(), ext.rows(), area, ppc, probe);
    if pan_col == 0 && pan_row == 0 {
        return Ok((view, area)); // the field fits on both axes — identical to the origin pan
    }
    let origin = ext.origin();
    // Pan in WORLD units: the window's top-left field cell `(pan_col, pan_row)` sits at world
    // `origin + cell·(pan_col, pan_row)`, and the inherited per-axis `zoom = ppc / cell` maps that
    // delta back to exactly `(pan_col·ppc, pan_row·ppc)` raster pixels.
    let view = view.with_pan(
        origin.x + pan_col as f64 * ext.cell_w(),
        origin.y + pan_row as f64 * ext.cell_h(),
    );
    Ok((view, area))
}

/// Why a field's geometry has no numerically invertible display mapping at a given `ppc`: the
/// per-axis `zoom = ppc / cell_size` is non-finite (overflowed) or zero, so `WorldView` cannot
/// invert it and there is no honest terminal-cell <-> field-index correspondence. Carries the
/// offending per-axis zoom values for a truthful diagnostic (never a silent blank render).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnsupportedView {
    pub zoom_x: f64,
    pub zoom_y: f64,
}
