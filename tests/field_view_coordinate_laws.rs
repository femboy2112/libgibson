//! RXIII Checkpoint A / Gate A6 — coordinate maps as partial morphisms.
//!
//! The map chain `field index → world location → terminal cell → source probe` must be an honest
//! partial morphism: where it promises an inverse it delivers the exact one, and where it does not
//! (off-field, masked, non-finite) it returns the correct *failure class* rather than a fabricated
//! value. The terminal↔index leg under origin/anisotropy/zoom is already proven in
//! `wire_instrument_viewport.rs::terminal_cells_map_back_to_their_source_index_under_origin_anisotropy_and_zoom`,
//! and the Masked/NonFinite disclosure in `field_view_observation.rs::masked_and_nonfinite_are_disclosed`.
//! These are the discriminating cases those do not reach: the round-trip as an identity on the WHOLE
//! lattice, the half-open cell geometry at every edge (no fake inverse across quantization), the four
//! sample classes held distinct (Value / Masked / NonFinite / OffField — a 0.0 reading is a Value,
//! never conflated with absence), the disclosed quantization granularity, and a non-finite probe
//! point as a clean off-field, not a panic.

use gibson::field_view::{FieldExtent, FieldView, Sample};
use gibson::Point;

// A nonzero-origin, anisotropic 3×2 field (cell_w ≠ cell_h), so origin and anisotropy are both
// exercised rather than assumed away. Cell (0,0) deliberately holds a genuine 0.0.
const ORIGIN: (f64, f64) = (2.0, -1.0);
const CW: f64 = 1.5;
const CH: f64 = 2.0;
const COLS: usize = 3;
const ROWS: usize = 2;

fn extent() -> FieldExtent {
    FieldExtent::new(Point::new(ORIGIN.0, ORIGIN.1), CW, CH, COLS, ROWS).expect("valid geometry")
}

/// **VALID.** The `index → cell_center (world) → world_to_index` round-trip is the identity on the
/// entire lattice — every cell of a nonzero-origin anisotropic field recovers its own index. This is
/// the "ordinary mapped cell recovers the correct source index where the mapping promises it" law,
/// enumerated over all cells rather than sampled at one impulse.
#[test]
fn the_index_world_index_round_trip_is_the_identity_on_the_whole_lattice() {
    let ext = extent();
    let data = vec![0.0f64; ext.len()];
    let view = FieldView::new(ext, &data, None).expect("view");
    for iy in 0..ROWS {
        for ix in 0..COLS {
            let center = view.extent().cell_center(ix, iy).expect("in range");
            assert_eq!(
                view.extent().world_to_index(center),
                Some((ix, iy)),
                "cell ({ix},{iy}) must recover its own index through its world center"
            );
        }
    }
}

/// **VALID.** Cell geometry is half-open `[lo, hi)` on each axis, with no fabricated inverse past the
/// edges. The lower corner is inclusive, the upper corner belongs to the next cell, and anything
/// before the origin or at/after the far edge is off-field (`None`) — never clamped to an edge cell.
#[test]
fn cell_geometry_is_half_open_with_no_fake_inverse_at_the_edges() {
    let e = extent();
    let p = |x: f64, y: f64| Point::new(x, y);

    // Lower corner of the field is inclusive → cell (0,0).
    assert_eq!(e.world_to_index(p(ORIGIN.0, ORIGIN.1)), Some((0, 0)));
    // Just inside the first cell's far edge is still (0,0); exactly at it crosses to (1,0).
    assert_eq!(
        e.world_to_index(p(ORIGIN.0 + CW - 1e-9, ORIGIN.1)),
        Some((0, 0))
    );
    assert_eq!(e.world_to_index(p(ORIGIN.0 + CW, ORIGIN.1)), Some((1, 0)));
    // The y axis is half-open the same way (cell height is CH).
    assert_eq!(e.world_to_index(p(ORIGIN.0, ORIGIN.1 + CH)), Some((0, 1)));

    // Before the origin on either axis → off-field, not clamped to the edge cell.
    assert_eq!(e.world_to_index(p(ORIGIN.0 - 1e-9, ORIGIN.1)), None);
    assert_eq!(e.world_to_index(p(ORIGIN.0, ORIGIN.1 - 1e-9)), None);
    // At or past the far edge (origin + n·cell) is exclusive → off-field.
    assert_eq!(
        e.world_to_index(p(ORIGIN.0 + COLS as f64 * CW, ORIGIN.1)),
        None
    );
    assert_eq!(
        e.world_to_index(p(ORIGIN.0, ORIGIN.1 + ROWS as f64 * CH)),
        None
    );
}

/// **VALID.** The four result classes are held distinct and absence is never a fake zero: a cell
/// holding 0.0 reports `Value(0.0)`, a masked cell reports `Masked`, a NaN cell reports `NonFinite`,
/// and an off-field probe reports `index: None, sample: None` — four different outcomes for four
/// different situations.
#[test]
fn the_four_sample_classes_are_distinct_and_absence_is_never_a_fake_zero() {
    let ext = extent();
    // Row-major; idx = iy*COLS + ix.
    let data = vec![0.0, 1.0, 2.0, 3.0, f64::NAN, 5.0]; // (1,1) = idx 4 is NaN
    let mut mask = vec![true; ext.len()];
    mask[2] = false; // (2,0) = idx 2 is masked
    let view = FieldView::new(ext, &data, Some(&mask)).expect("view");

    assert_eq!(
        view.sample(0, 0),
        Some(Sample::Value(0.0)),
        "a genuine zero is a Value, not absence"
    );
    assert_eq!(view.sample(1, 0), Some(Sample::Value(1.0)));
    assert_eq!(
        view.sample(2, 0),
        Some(Sample::Masked),
        "a masked cell is Masked, not Value(0.0)"
    );
    assert_eq!(
        view.sample(1, 1),
        Some(Sample::NonFinite),
        "a NaN cell is NonFinite, not coerced"
    );

    // Off-field probe: a distinct class again — None index, None sample, no clamp to an edge cell.
    let off = view.probe(Point::new(ORIGIN.0 - 10.0, ORIGIN.1));
    assert_eq!(off.index, None, "off-field is off-field");
    assert_eq!(
        off.sample, None,
        "off-field discloses no sample, not a fabricated zero"
    );
}

/// **VALID.** A probe discloses its quantization granularity and the quantized target it resolved to
/// — the provenance that lets an inspector show how coarse the reading is. The probe echoes the
/// exact world point asked (never a silently adjusted one).
#[test]
fn a_probe_discloses_its_quantization_and_provenance() {
    let ext = extent();
    let data = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
    let view = FieldView::new(ext, &data, None).expect("view");

    let center = view.extent().cell_center(1, 0).expect("in range");
    let report = view.probe(center);
    assert_eq!(
        report.world, center,
        "the probe echoes exactly what was asked"
    );
    assert_eq!(report.index, Some((1, 0)));
    assert_eq!(report.sample, Some(Sample::Value(1.0)));
    assert_eq!(
        report.quantization,
        Some((CW, CH)),
        "the cell size is disclosed as the granularity"
    );
    assert_eq!(
        report.cell_center,
        Some(center),
        "and the quantized target it resolved to"
    );
}

/// **VALID.** A non-finite probe point is a clean off-field, not a panic or a wrapped index — the
/// hostile numerical limit at the boundary of the coordinate map.
#[test]
fn a_non_finite_probe_point_is_off_field_not_a_panic() {
    let ext = extent();
    let data = vec![0.0; ext.len()];
    let view = FieldView::new(ext, &data, None).expect("view");

    assert_eq!(
        view.extent().world_to_index(Point::new(f64::NAN, 0.0)),
        None
    );
    assert_eq!(
        view.extent().world_to_index(Point::new(0.0, f64::INFINITY)),
        None
    );
    let report = view.probe(Point::new(f64::NAN, f64::NAN));
    assert_eq!(report.index, None);
    assert_eq!(report.sample, None);
}
