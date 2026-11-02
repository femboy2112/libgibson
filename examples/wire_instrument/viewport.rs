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
) -> (gibson::world_view::WorldView, gibson::Rect) {
    let ppc = ppc.max(1);
    let (w, h) = viewport_dims(ext.cols(), ext.rows(), ppc);
    let area = gibson::Rect::new(0, 0, w, h);
    let origin = ext.origin();
    let zoom_x = f64::from(ppc) / ext.cell_w();
    let zoom_y = f64::from(ppc) / ext.cell_h();
    let view = gibson::world_view::WorldView::new(area)
        .with_pan(origin.x, origin.y)
        .with_pixels_per_cell(1, 1)
        .with_zoom(zoom_x, zoom_y);
    (view, area)
}
