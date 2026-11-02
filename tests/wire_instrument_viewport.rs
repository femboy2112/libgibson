//! Viewport proofs for `wire_instrument` (Renaissance VIII sizing + Renaissance IX geometry, Gate G).
//!
//! Two layers. (1) SIZING — the `u16`-wrap defect: a legal field dimension past `u16::MAX` must
//! saturate to the display cap, not truncate to a sliver (Rsn VIII, #226). (2) GEOMETRY — the
//! display transport must be derived from the SAME source geometry the probe indexes: the field's
//! world **origin** and its possibly **anisotropic** cell sizes, plus `ppc` magnification. If it is
//! not, the rendered field and the index-based probe disagree (a nonzero-origin field renders off
//! view while `sample` still returns values; an anisotropic field distorts). `viewport::world_view_for`
//! transports all three; these tests prove the disagreement is gone and that terminal cell
//! `(ix·ppc.., iy·ppc..)` maps back to field index `(ix, iy)`.
//!
//! Still open (noted, not faked): the real 40×15 mono-terminal walkthrough on a live PTY.

#[path = "../examples/wire_instrument/viewport.rs"]
mod viewport;

use gibson::field_view::{Colormap, FieldExtent, FieldView, RangePolicy, Sample};
use gibson::world_view::{Transport, WorldView};
use gibson::{Point, Rect};
use viewport::{viewport_dims, world_view_for};

/// The exact defect, executed: the pre-fix formula narrowed `cols` to `u16` *before* clamping, so a
/// 65536-column field wrapped to 0 and then clamped *up* to width 1. The fix clamps wide first.
#[test]
fn the_pre_fix_u16_cast_wrapped_a_huge_field_and_the_fix_does_not() {
    // Pre-fix: `(ext.cols() as u16).saturating_mul(ppc).clamp(1, 240)` with cols = 65536, ppc = 1.
    let buggy = (65536usize as u16).saturating_mul(1).clamp(1, 240);
    assert_eq!(
        buggy, 1,
        "documents the wrap: 65536 as u16 == 0 → clamped up to 1"
    );

    // Fixed: wide arithmetic, clamped before the narrowing cast.
    assert_eq!(
        viewport_dims(65536, 4, 1).0,
        240,
        "a 65536-wide field saturates to the display cap, not a sliver"
    );
}

/// Both `u16`-boundary field widths, and a full 2^20-cell field, size to the cap — never wrap.
#[test]
fn legal_u16_overflow_widths_saturate_not_wrap() {
    assert_eq!(viewport_dims(65536, 4, 1).0, 240);
    assert_eq!(viewport_dims(65537, 4, 1).0, 240);
    // 2^20 cells as 1024x1024 — the max legal field.
    assert_eq!(viewport_dims(1024, 1024, 1), (240, 80));
}

/// Small fields are untouched by the fix (regression anchor on the common case).
#[test]
fn small_fields_are_unchanged() {
    assert_eq!(viewport_dims(10, 5, 2), (20, 10));
    assert_eq!(viewport_dims(1, 1, 1), (1, 1));
}

/// Dimensions clamp to the display budget at and past the cap; `ppc` multiplies before clamping.
#[test]
fn dimensions_clamp_to_the_display_budget() {
    assert_eq!(viewport_dims(240, 80, 1), (240, 80));
    assert_eq!(viewport_dims(241, 81, 1), (240, 80));
    assert_eq!(viewport_dims(200, 50, 2), (240, 80)); // 400x100 → capped
}

/// Width and height clamp independently — a wide, short field keeps its exact (small) height.
#[test]
fn anisotropic_dimensions_clamp_independently() {
    assert_eq!(viewport_dims(100_000, 3, 1), (240, 3));
    assert_eq!(viewport_dims(2, 100_000, 1), (2, 80));
}

/// A zero zoom floors to 1 rather than collapsing the viewport to nothing.
#[test]
fn ppc_zero_is_floored_to_one() {
    assert_eq!(viewport_dims(10, 5, 0), (10, 5));
}

// ---- Geometry: origin + anisotropy must be transported into the display (Rsn IX) -------------

/// The decisive disagreement. A field with a nonzero world origin renders ENTIRELY off-view under
/// the old transport (default pan `(0,0)`, isotropic `pixels_per_cell`), yet the index-based probe
/// still returns real values — the display and the probe contradict each other. `world_view_for`
/// pans to the origin, so every in-bounds cell renders and the contradiction is gone.
#[test]
fn nonzero_origin_field_renders_off_view_before_the_fix_and_on_view_after() {
    let origin = Point::new(1000.0, 500.0);
    let ext = FieldExtent::new(origin, 1.0, 1.0, 4, 3).unwrap();
    let data: Vec<f64> = (0..12).map(|i| i as f64).collect();
    let view = FieldView::new(ext, &data, None).unwrap();

    // The probe reads a real value at (0,0) regardless of display geometry.
    assert!(matches!(view.sample(0, 0), Some(Sample::Value(_))));

    // OLD (buggy) construction: pan 0, isotropic ppc — the whole field is off-view.
    let (w, h) = viewport_dims(ext.cols(), ext.rows(), 1);
    let old_wv = WorldView::new(Rect::new(0, 0, w, h)).with_pixels_per_cell(1, 1);
    let old = view.render(
        &old_wv,
        Rect::new(0, 0, w, h),
        Colormap::Density,
        RangePolicy::Auto,
    );
    assert_eq!(
        old.report.value_cells, 0,
        "RED: a field at origin (1000,500) renders entirely off-view with pan 0"
    );
    assert!(old.report.off_field_cells > 0);

    // FIXED construction: pan to the origin — every in-bounds cell renders.
    let (wv, area) = world_view_for(&ext, 1);
    let fixed = view.render(&wv, area, Colormap::Density, RangePolicy::Auto);
    assert_eq!(
        fixed.report.value_cells, 12,
        "every in-bounds cell of the 4x3 field must render after the fix"
    );
    assert_eq!(fixed.report.off_field_cells, 0);
}

/// Correspondence under the full geometry: nonzero origin, ANISOTROPIC cells (`cell_w != cell_h`),
/// and `ppc` magnification. Terminal cell `(ix·ppc + k, iy·ppc + k)` must map back — through the
/// SAME transport the display renders with — to field index `(ix, iy)`. This is "the value the
/// user probes corresponds to the field location actually displayed", proven cell by cell.
#[test]
fn terminal_cells_map_back_to_their_source_index_under_origin_anisotropy_and_zoom() {
    let ext = FieldExtent::new(Point::new(10.0, -5.0), 3.0, 2.0, 3, 2).unwrap();
    let ppc: u16 = 2;
    let (wv, _area) = world_view_for(&ext, ppc);

    for iy in 0..ext.rows() {
        for ix in 0..ext.cols() {
            // Every terminal cell in this source cell's ppc×ppc block resolves to (ix, iy).
            for ky in 0..ppc {
                for kx in 0..ppc {
                    let cx = ix as u16 * ppc + kx;
                    let cy = iy as u16 * ppc + ky;
                    let world = match wv.cell_to_world(cx, cy) {
                        Transport::Mapped { value, .. } => value,
                        other => panic!("terminal cell ({cx},{cy}) must map, got {other:?}"),
                    };
                    let idx = ext
                        .world_to_index(Point::new(world.0, world.1))
                        .unwrap_or_else(|| panic!("world {world:?} must land in the field"));
                    assert_eq!(
                        idx,
                        (ix, iy),
                        "terminal cell ({cx},{cy}) must display source index ({ix},{iy})"
                    );
                }
            }
        }
    }
}

/// An anisotropic field renders without distortion-induced off-field holes: a 3×2 field with
/// `cell_w=3, cell_h=2` at `ppc=2` fills its whole 6×4 viewport with real values.
#[test]
fn anisotropic_field_fills_its_viewport_after_the_fix() {
    let ext = FieldExtent::new(Point::new(0.0, 0.0), 3.0, 2.0, 3, 2).unwrap();
    let data = vec![1.0; ext.cols() * ext.rows()];
    let view = FieldView::new(ext, &data, None).unwrap();
    let ppc = 2;
    let (wv, area) = world_view_for(&ext, ppc);
    assert_eq!(
        (area.width, area.height),
        (6, 4),
        "3x2 field at 2x zoom = 6x4 viewport"
    );
    let frame = view.render(&wv, area, Colormap::Density, RangePolicy::Auto);
    assert_eq!(
        frame.report.value_cells, 24,
        "all 6x4 = 24 terminal cells map onto the 3x2 field, none off-field"
    );
    assert_eq!(frame.report.off_field_cells, 0);
}
