//! Region-of-interest (pan/ROI) proofs for `wire_instrument` (RXIII Checkpoint C / Gate D —
//! consumer navigation ergonomics).
//!
//! Renaissance VIII/IX (Gate G, `wire_instrument_viewport.rs`) established that the display
//! transport is derived from the SAME source geometry the probe indexes, so "the value the user
//! probes corresponds to the field location actually displayed." That claim held only while the
//! whole field fit the display budget. A legal field may be far larger: up to `2^20` cells, so a
//! field can be hundreds or thousands of columns wide. `world_view_for` always pans to the field
//! ORIGIN and caps the viewport at `MAX_VIEW_COLS`/`MAX_VIEW_ROWS`, so on such a field the probe —
//! clamped only to `cols()-1` / `rows()-1` by the loop — can navigate to a cell BEYOND the displayed
//! window. The readout then reports an exact value for a cell that is not on screen: the Gate G
//! correspondence, broken again in the large-field regime (the overflow regime was closed by Gate
//! D3's `UnsupportedView`).
//!
//! The ergonomic fix is a region-of-interest viewport that FOLLOWS the probe: pan so the probed cell
//! is always inside the visible window (centered when the field is larger than the window). These
//! tests prove (a) the pure, stateless ROI pan keeps the probe inside the window on every axis
//! configuration, (b) the origin pan genuinely leaves a far probe off-screen (the defect, executed),
//! and (c) the ROI-panned `WorldView` maps every in-bounds probe's world center to a DISPLAYED cell,
//! while cells outside the window stay clipped (it is a real window, not a degenerate everything-fits
//! view).

#[path = "../examples/wire_instrument/viewport.rs"]
mod viewport;

use gibson::field_view::FieldExtent;
use gibson::world_view::WorldView;
use gibson::{Point, Rect};
use viewport::{roi_axis, roi_origin, world_view_for, world_view_for_probe, MAX_VIEW_COLS};

/// The world-space center of field cell `(pix, piy)` for an extent — the point the probe readout
/// describes, and the point whose displayed/clipped status decides whether the probe is on-screen.
fn cell_center(ext: &FieldExtent, pix: usize, piy: usize) -> (f64, f64) {
    let o = ext.origin();
    (
        o.x + (pix as f64 + 0.5) * ext.cell_w(),
        o.y + (piy as f64 + 0.5) * ext.cell_h(),
    )
}

/// `true` iff the probed cell's world center maps to a cell actually inside the viewport `wv` draws
/// — i.e. the operator can SEE the cell whose value the readout reports.
fn probe_is_displayed(ext: &FieldExtent, wv: &WorldView, pix: usize, piy: usize) -> bool {
    let (wx, wy) = cell_center(ext, pix, piy);
    wv.world_to_cell(wx, wy).is_mapped()
}

/// The pure ROI pan: on an axis of `len` cells showing a `view`-cell window, the chosen top-left
/// index keeps the probe inside `[pan, pan+view)`, never pans past `len-view`, and is zero when the
/// whole axis fits. These are the invariants the display correspondence rests on.
#[test]
fn roi_axis_keeps_the_probe_in_the_window_and_never_overscrolls() {
    // Whole axis fits the window: anchored at the origin, no pan.
    assert_eq!(roi_axis(10, 10, 7), 0, "a field that fits is not panned");
    assert_eq!(
        roi_axis(10, 40, 7),
        0,
        "a window wider than the field: no pan"
    );

    // Larger than the window: the probe is centered, clamped to the fill-the-window range.
    let len = 300usize;
    let view = 80usize;
    let max_pan = len - view;
    for probe in 0..len {
        let pan = roi_axis(len, view, probe);
        assert!(
            pan <= max_pan,
            "pan {pan} overscrolls past {max_pan} (probe {probe})"
        );
        assert!(
            probe >= pan,
            "probe {probe} fell off the left of the window at pan {pan}"
        );
        assert!(
            probe < pan + view,
            "probe {probe} fell off the right of the window [{pan}, {})",
            pan + view
        );
    }
    // Left edge anchors at 0; right edge anchors at max_pan (the window is full, not past the end).
    assert_eq!(roi_axis(len, view, 0), 0);
    assert_eq!(roi_axis(len, view, len - 1), max_pan);
    // A defensively out-of-range probe pins to the last window rather than overscrolling.
    assert_eq!(roi_axis(len, view, len + 50), max_pan);
}

/// `roi_origin` reads the visible cell counts off the rendered `area` and `ppc` and applies
/// `roi_axis` per axis. At `ppc=2` an 80-pixel-wide area shows 40 cells, so the column window is
/// half as wide as the pixel budget.
#[test]
fn roi_origin_derives_the_window_from_area_and_ppc() {
    let area = Rect::new(0, 0, 80, 40);
    // ppc 1: 80×40 cells visible; a 300×300 field centers the probe.
    assert_eq!(roi_origin(300, 300, area, 1, (150, 150)), (110, 130));
    // ppc 2: 40×20 cells visible — the window shrinks, so the pan to center the same probe grows.
    assert_eq!(roi_origin(300, 300, area, 2, (150, 150)), (130, 140));
    // Narrow field on one axis fits (no pan there), wide on the other pans.
    assert_eq!(roi_origin(300, 10, area, 1, (260, 5)), (220, 0));
}

/// The defect, executed. On a field wider than the display budget, the origin pan
/// (`world_view_for`) leaves a far — but in-bounds — probe OFF the displayed window: its readout
/// reports an exact value for a cell the operator cannot see. This is the Gate G correspondence
/// failing in the large-field regime, and it is exactly what the ROI pan repairs.
#[test]
fn the_origin_pan_leaves_a_far_in_bounds_probe_off_the_displayed_window() {
    let cols = (MAX_VIEW_COLS as usize) + 100; // wider than the 240-column budget
    let ext = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, cols, 8).unwrap();
    let (wv, area) = world_view_for(&ext, 1).unwrap();
    assert_eq!(
        area.width, MAX_VIEW_COLS as u16,
        "the viewport is capped at the budget"
    );

    // A near probe (inside the budget) is displayed under the origin pan...
    assert!(
        probe_is_displayed(&ext, &wv, 10, 3),
        "a near column is on-screen"
    );
    // ...but a far, still-in-bounds probe is NOT — the readout/display disagreement.
    let far = cols - 5;
    assert!(
        far < cols, // in-bounds by construction
        "far probe must be a valid field column"
    );
    assert!(
        !probe_is_displayed(&ext, &wv, far, 3),
        "defect: the origin pan leaves field column {far} off the displayed window"
    );
}

/// The fix. The ROI-panned view keeps EVERY in-bounds probe's world center on the displayed window
/// — across the whole width of a field far larger than the budget — restoring the Gate G
/// correspondence. The readout can never again describe a cell the operator cannot see.
#[test]
fn the_roi_pan_keeps_every_in_bounds_probe_on_the_displayed_window() {
    let cols = (MAX_VIEW_COLS as usize) + 100;
    let rows = 40usize; // taller than MAX_VIEW_ROWS (80)? no — exercise a modest pan on both axes
    let ext = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, cols, rows).unwrap();

    // Sweep the full column range at a few rows; every probe must be displayed.
    for &piy in &[0usize, rows / 2, rows - 1] {
        for pix in 0..cols {
            let (wv, _area) = world_view_for_probe(&ext, 1, (pix, piy)).unwrap();
            assert!(
                probe_is_displayed(&ext, &wv, pix, piy),
                "ROI pan must display probe ({pix},{piy}) on a {cols}x{rows} field"
            );
        }
    }
}

/// The ROI window is a REAL window, not a degenerate "everything maps." With the probe at the far
/// right edge, the left-most column of the field is scrolled OFF and clipped — proving the pan
/// actually moved the visible region rather than widening it to contain the whole field.
#[test]
fn the_roi_window_scrolls_so_distant_cells_clip() {
    let cols = (MAX_VIEW_COLS as usize) + 100;
    let ext = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, cols, 8).unwrap();
    let (wv, _area) = world_view_for_probe(&ext, 1, (cols - 1, 3)).unwrap();
    assert!(
        probe_is_displayed(&ext, &wv, cols - 1, 3),
        "the probe at the right edge is on-screen"
    );
    assert!(
        !probe_is_displayed(&ext, &wv, 0, 3),
        "column 0 is scrolled off when the probe is at the far right — a true window"
    );
}

/// When the whole field fits the display budget, the ROI pan is IDENTICAL to the origin pan: no
/// surprise scrolling for the ordinary small-field case that every prior test and the compact path
/// rely on. Panning is strictly an additive affordance for large fields.
#[test]
fn a_field_that_fits_is_panned_exactly_like_the_origin_view() {
    let ext = FieldExtent::new(Point::new(10.0, -5.0), 3.0, 2.0, 12, 9).unwrap();
    for probe in [(0usize, 0usize), (6, 4), (11, 8)] {
        let (origin_wv, _a) = world_view_for(&ext, 2).unwrap();
        let (roi_wv, _b) = world_view_for_probe(&ext, 2, probe).unwrap();
        assert_eq!(
            origin_wv.pan(),
            roi_wv.pan(),
            "a field within the budget is not panned by the ROI (probe {probe:?})"
        );
    }
}
