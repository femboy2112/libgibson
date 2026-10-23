//! Wave 4 — the FieldView observation slice, as consumer code.
//!
//! The load-bearing law: a field cell resolved under the pointer is the SAME cell
//! the renderer painted there (Law L3, identity transport). We prove it by taking
//! `terminal-cell → world` through a `WorldView` (the pointer-selection path) and
//! then `world → field-index` three independent ways that must agree — plus a
//! mutation arm that shifts the field by one cell and shows the check then fails.
//! The rest are synthetic analytic fixtures (gradient, impulse, constant), the
//! masked/non-finite disclosure, a deterministic receipt, and the linked
//! time-series plot built from a probed cell through `gibson::plot`.

use gibson::field_view::{Colormap, FieldExtent, FieldView, RangePolicy, Sample};
use gibson::plot::{self, AxisScale, AxisSpec, FiniteRange, PlotSpec, PlotView, Series};
use gibson::shape::Point;
use gibson::world_view::WorldView;
use gibson::{Rect, SubcellGlyphMode};

// A view whose canvas is offset from the origin, 1 cell == 1 world unit, so the
// closed-form expected index is `(col - CX0, row - CY0)`.
const CX0: u16 = 3;
const CY0: u16 = 2;
const W: u16 = 8;
const H: u16 = 6;

fn view() -> WorldView {
    WorldView::new(Rect::new(CX0, CY0, W, H))
        .with_zoom(1.0, 1.0)
        .with_pixels_per_cell(1, 1)
}

fn aligned_extent() -> FieldExtent {
    FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, W as usize, H as usize).unwrap()
}

/// The three-way reconciliation: FieldView's full cell→world→index path, an
/// independent closed form, and the editor's own pick predicate (the field cell
/// whose world `Bounds` contain the point) all agree, for every painted cell.
#[test]
fn field_cells_reconcile_with_pointer_selection() {
    let e = aligned_extent();
    let data = vec![1.0; e.len()];
    let fv = FieldView::new(e, &data, None).unwrap();
    let wv = view();

    for ry in 0..H {
        for rx in 0..W {
            let (col, row) = (CX0 + rx, CY0 + ry);

            // (1) FieldView's full path: terminal cell -> world -> field index.
            let probe = fv.probe_cell(&wv, col, row).expect("cell is in view");
            let fv_index = probe.index.expect("cell is in field");

            // (2) Independent closed form for a 1:1 origin-aligned view.
            assert_eq!(
                fv_index,
                (rx as usize, ry as usize),
                "cell ({col},{row}) must resolve to field index ({rx},{ry})"
            );

            // (3) The editor's pick predicate: the field cell whose world Bounds
            // contain the same world point. This is `shape::Bounds::contains`, the
            // exact test pointer selection uses — a second, independent machinery.
            let world = probe.world;
            let picked = (0..e.rows())
                .flat_map(|iy| (0..e.cols()).map(move |ix| (ix, iy)))
                .find(|&(ix, iy)| e.cell_bounds(ix, iy).unwrap().contains(world));
            assert_eq!(
                picked,
                Some(fv_index),
                "Bounds-pick must agree with world_to_index at cell ({col},{row})"
            );
        }
    }
}

/// Non-vacuity: shift the field origin by exactly one cell and the resolved index
/// must disagree with the aligned closed form for at least one painted cell — so
/// the reconciliation above is a real check, not a tautology.
#[test]
fn reconciliation_detects_a_one_cell_field_shift() {
    let e = FieldExtent::new(Point::new(1.0, 0.0), 1.0, 1.0, (W + 1) as usize, H as usize).unwrap();
    let data = vec![1.0; e.len()];
    let fv = FieldView::new(e, &data, None).unwrap();
    let wv = view();

    let mut disagreed = false;
    for ry in 0..H {
        for rx in 0..W {
            let (col, row) = (CX0 + rx, CY0 + ry);
            if let Some(p) = fv.probe_cell(&wv, col, row) {
                if p.index != Some((rx as usize, ry as usize)) {
                    disagreed = true;
                }
            }
        }
    }
    assert!(
        disagreed,
        "a one-cell field shift must change at least one resolved index"
    );
}

/// A linear gradient: the sample at a cell is exactly the authored value, the
/// auto-range is the true extremes, and every field cell paints a value.
#[test]
fn linear_gradient_samples_and_auto_range() {
    let e = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 10, 1).unwrap();
    let data: Vec<f64> = (0..10).map(|ix| ix as f64).collect();
    let fv = FieldView::new(e, &data, None).unwrap();

    assert_eq!(
        fv.probe(Point::new(3.5, 0.5)).sample,
        Some(Sample::Value(3.0))
    );
    assert_eq!(
        fv.probe(Point::new(9.5, 0.5)).sample,
        Some(Sample::Value(9.0))
    );
    assert_eq!(fv.finite_extremes(), Some((0.0, 9.0)));

    let wv = WorldView::new(Rect::new(0, 0, 10, 1))
        .with_zoom(1.0, 1.0)
        .with_pixels_per_cell(1, 1);
    let frame = fv.render(
        &wv,
        Rect::new(0, 0, 10, 1),
        Colormap::Heat,
        RangePolicy::Auto,
    );
    assert_eq!(frame.report.resolved_range, FiniteRange::new(0.0, 9.0));
    assert_eq!(frame.report.value_cells, 10);
    assert!(frame.report.conserves());
}

/// A single impulse: one hot cell, the rest cold. The impulse reads back as the
/// peak and its neighbour as the floor, and the auto-range spans both.
#[test]
fn single_impulse_is_localized() {
    let e = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 5, 5).unwrap();
    let mut data = vec![0.0; e.len()];
    let (ix, iy) = (2usize, 3usize);
    data[iy * e.cols() + ix] = 100.0;
    let fv = FieldView::new(e, &data, None).unwrap();

    assert_eq!(
        fv.probe(Point::new(ix as f64 + 0.5, iy as f64 + 0.5))
            .sample,
        Some(Sample::Value(100.0))
    );
    assert_eq!(
        fv.probe(Point::new(0.5, 0.5)).sample,
        Some(Sample::Value(0.0))
    );
    assert_eq!(fv.finite_extremes(), Some((0.0, 100.0)));
}

/// Masked and non-finite cells are disclosed in the receipt, never painted as a
/// value or coerced to a number.
#[test]
fn masked_and_nonfinite_are_disclosed() {
    let e = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 3, 3).unwrap();
    let mut data = vec![1.0; e.len()];
    data[0] = f64::NAN; // (0,0) non-finite
    data[4] = f64::INFINITY; // (1,1) non-finite
    let mut mask = vec![true; e.len()];
    mask[8] = false; // (2,2) masked
    let fv = FieldView::new(e, &data, Some(&mask)).unwrap();

    let wv = WorldView::new(Rect::new(0, 0, 3, 3))
        .with_zoom(1.0, 1.0)
        .with_pixels_per_cell(1, 1);
    let frame = fv.render(
        &wv,
        Rect::new(0, 0, 3, 3),
        Colormap::Density,
        RangePolicy::Auto,
    );
    assert_eq!(frame.report.cells_in_area, 9);
    assert_eq!(frame.report.nonfinite_cells, 2);
    assert_eq!(frame.report.masked_cells, 1);
    assert_eq!(frame.report.value_cells, 6);
    assert!(frame.report.conserves());
    // The range is derived only from the 6 finite value cells (all 1.0 -> widened).
    assert!(frame.report.range_degenerate_widened);
}

/// The render receipt is deterministic: the same inputs give an equal receipt.
#[test]
fn render_receipt_is_deterministic() {
    let e = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 6, 4).unwrap();
    let data: Vec<f64> = (0..e.len()).map(|i| (i % 7) as f64).collect();
    let fv = FieldView::new(e, &data, None).unwrap();
    let wv = WorldView::new(Rect::new(0, 0, 6, 4))
        .with_zoom(1.0, 1.0)
        .with_pixels_per_cell(1, 1);
    let a = fv.render(
        &wv,
        Rect::new(0, 0, 6, 4),
        Colormap::Heat,
        RangePolicy::Auto,
    );
    let b = fv.render(
        &wv,
        Rect::new(0, 0, 6, 4),
        Colormap::Heat,
        RangePolicy::Auto,
    );
    assert_eq!(a.report, b.report, "the receipt must be reproducible");
}

/// The linked time-series: probe one field cell across a sequence of data frames,
/// then render the resulting series through `gibson::plot`. This exercises the
/// merged plot module and proves the field→series→plot link end to end.
#[test]
fn linked_time_series_plots_a_probed_cell() {
    let e = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 4, 4).unwrap();
    let probe_pt = Point::new(2.5, 1.5); // cell (2,1)
    let flat = e.cols() + 2; // row 1, col 2

    // Three snapshots; the probed cell rises 1 -> 4 -> 9.
    let mut frames_data: Vec<Vec<f64>> = vec![vec![0.0; e.len()]; 3];
    frames_data[0][flat] = 1.0;
    frames_data[1][flat] = 4.0;
    frames_data[2][flat] = 9.0;
    let frames: Vec<&[f64]> = frames_data.iter().map(|v| v.as_slice()).collect();

    let series_pts = gibson::field_view::probe_series(&frames, e, probe_pt, 1.0);
    assert_eq!(series_pts, vec![(0.0, 1.0), (1.0, 4.0), (2.0, 9.0)]);

    // Feed it straight to the plot module as a line series.
    let spec = PlotSpec::new(
        AxisSpec::new(AxisScale::Linear, "t"),
        AxisSpec::new(AxisScale::Linear, "value"),
    )
    .series(Series::line(series_pts.clone()));
    let pview = PlotView::new(
        FiniteRange::new(0.0, 2.0).unwrap(),
        FiniteRange::new(0.0, 10.0).unwrap(),
    );
    let (_surface, report) = plot::plot(
        &spec,
        &pview,
        Rect::new(0, 0, 40, 10),
        SubcellGlyphMode::Braille2x4,
    )
    .expect("the probed series is a valid plot");

    // Every probed point is finite and in range, so all are accounted for; the
    // three points form a line of two drawn segments.
    assert_eq!(report.finite_samples, series_pts.len());
    assert!(report.segments_emitted > 0, "the linked series must draw");
}

#[test]
fn extent_accessors_return_constructor_inputs_bit_exact() {
    // A pathological geometry: an origin whose magnitude dwarfs the cell size, so
    // that reconstructing it from a derived corner (origin + 0·cell) could lose
    // low mantissa bits. The accessors must hand back the EXACT bits that went in,
    // which is what lets a transport round-trip the geometry without coercion.
    let ox = 1.234_567_890_123_456_7e12;
    let oy = -9.876_543_210_987_654e11;
    let cw = 3.141_592_653_589_793e-7;
    let ch = 2.718_281_828_459_045e-7;
    let e = FieldExtent::new(Point::new(ox, oy), cw, ch, 4, 3).unwrap();

    // Bit-exact, not approximately-equal: these are the literal constructor args.
    assert_eq!(e.origin().x.to_bits(), ox.to_bits());
    assert_eq!(e.origin().y.to_bits(), oy.to_bits());
    assert_eq!(e.cell_w().to_bits(), cw.to_bits());
    assert_eq!(e.cell_h().to_bits(), ch.to_bits());

    // The derived-corner path (cell_bounds(0,0).min) must still be a valid finite
    // corner — demonstrating WHY the accessors exist rather than reusing the corner.
    let corner = e.cell_bounds(0, 0).unwrap().min;
    assert!(corner.x.is_finite() && corner.y.is_finite());
}
