//! RXIII Checkpoint B / Gate C — the non-Gaussian HOLDOUT.
//!
//! Every live frame so far is a single moving Gaussian (the reference producer's only field). A
//! cinema grammar secretly overfit to "one compact bump" would pass every Gaussian test and still
//! be wrong. This holdout feeds the grammar fields it has NEVER seen — two competing peaks, a
//! moving wavefront ridge, and a sparsely-masked layout — and proves the grammar frames the true
//! observed structure, not an assumed shape.
//!
//! The grammar only ever sees a `FieldView`, so no producer is needed: these fields are built
//! directly and driven through the UNMODIFIED `observe → compose → render_shot` pipeline. If the
//! camera tracked a hard-coded center or a single assumed lobe, the two-peak switch and the
//! wavefront advance would not move the focus — those are the teeth.

#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;

use cinema::{
    compose, field_center, observe, render_shot, Emphasis, FieldObservation, SourceId,
    DROPOUT_DENSITY_THRESHOLD,
};
use gibson::field_view::{Colormap, FieldExtent, FieldView, Sample};
use gibson::shape::Point;
use gibson::Rect;

const COLS: usize = 32;
const ROWS: usize = 16;
const ANCHOR: Rect = Rect {
    x: 0,
    y: 0,
    width: 40,
    height: 16,
};

fn extent() -> FieldExtent {
    FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, COLS, ROWS).expect("holdout geometry")
}

fn gaussian_at(data: &mut [f64], cx: f64, cy: f64, amp: f64, sigma: f64) {
    for iy in 0..ROWS {
        for ix in 0..COLS {
            let x = ix as f64 + 0.5;
            let y = iy as f64 + 0.5;
            let d2 = (x - cx).powi(2) + (y - cy).powi(2);
            data[iy * COLS + ix] += amp * (-d2 / (2.0 * sigma * sigma)).exp();
        }
    }
}

fn observe_field<'a>(
    data: &'a [f64],
    mask: Option<&'a [bool]>,
    tick: u64,
) -> (FieldObservation, FieldView<'a>) {
    let view = FieldView::new(extent(), data, mask).expect("holdout field geometry");
    // observe borrows the view and returns an owned (Copy) observation; return the view too so the
    // caller can probe/render against the same buffers.
    let obs = observe(&view, SourceId(7), 1, tick);
    (obs, view)
}

/// A field with TWO separated peaks. The camera must frame the dominant one, and when dominance
/// switches lobes the focus must JUMP across the field — a single-bump assumption can't do this.
#[test]
fn the_camera_tracks_the_dominant_of_two_peaks_and_follows_the_switch() {
    // Peak A at (8,8), peak B at (24,8). First A dominates, then B.
    let mut a_dom = vec![0.0f64; COLS * ROWS];
    gaussian_at(&mut a_dom, 8.0, 8.0, 2.0, 2.0);
    gaussian_at(&mut a_dom, 24.0, 8.0, 1.0, 2.0);

    let mut b_dom = vec![0.0f64; COLS * ROWS];
    gaussian_at(&mut b_dom, 8.0, 8.0, 1.0, 2.0);
    gaussian_at(&mut b_dom, 24.0, 8.0, 2.0, 2.0);

    let (oa, va) = observe_field(&a_dom, None, 0);
    let (ob, vb) = observe_field(&b_dom, None, 1);

    let ca = oa.peak_center.expect("peak A dominant");
    let cb = ob.peak_center.expect("peak B dominant");
    assert!(
        ca.x < 12.0,
        "when A dominates, the camera frames the LEFT lobe (~x=8), got x={}",
        ca.x
    );
    assert!(
        cb.x > 20.0,
        "when B dominates, the focus JUMPS to the RIGHT lobe (~x=24), got x={}",
        cb.x
    );

    // The composed shot's focus follows the dominant peak, and the rendered film differs.
    let sa = compose(&oa, &va.probe(field_center(extent())), extent());
    let sb = compose(&ob, &vb.probe(field_center(extent())), extent());
    assert_ne!(
        sa.focus, sb.focus,
        "the shot focus must switch lobes with the dominant peak"
    );
    let fa = render_shot(&va, &sa, ANCHOR);
    let fb = render_shot(&vb, &sb, ANCHOR);
    assert_ne!(
        fa.surface, fb.surface,
        "the framed film visibly differs when dominance switches lobes"
    );

    // And the field really is multi-modal (not a single bump that happens to move): both lobes
    // carry a substantial, comparable value in the A-dominant frame.
    let left = va.sample(8, 8);
    let right = va.sample(24, 8);
    assert!(
        matches!(left, Some(Sample::Value(v)) if v > 1.5)
            && matches!(right, Some(Sample::Value(v)) if v > 0.8),
        "the holdout is genuinely two-peaked, not one Gaussian"
    );
}

/// A moving WAVEFRONT (a diagonal ridge, not a point source). As the front advances the framing
/// must move — the grammar frames observed structure, it does not assume a compact lobe.
#[test]
fn a_moving_wavefront_is_framed_without_assuming_a_point_source() {
    fn wavefront(front: f64) -> Vec<f64> {
        let mut data = vec![0.0f64; COLS * ROWS];
        for iy in 0..ROWS {
            for ix in 0..COLS {
                let x = ix as f64 + 0.5;
                let y = iy as f64 + 0.5;
                // A bright ridge along the line x + y == front, falling off to either side.
                let d = (x + y) - front;
                data[iy * COLS + ix] = (-(d * d) / (2.0 * 1.2 * 1.2)).exp();
            }
        }
        data
    }

    let early = wavefront(10.0);
    let late = wavefront(28.0);
    let (oe, ve) = observe_field(&early, None, 0);
    let (ol, vl) = observe_field(&late, None, 1);

    let ce = oe.peak_center.expect("early ridge");
    let cl = ol.peak_center.expect("late ridge");
    // The ridge's framed point advances outward along the diagonal.
    assert!(
        (cl.x + cl.y) > (ce.x + ce.y) + 6.0,
        "the framed ridge point advances with the wavefront: early {:.1} vs late {:.1}",
        ce.x + ce.y,
        cl.x + cl.y
    );

    let se = compose(&oe, &ve.probe(field_center(extent())), extent());
    let sl = compose(&ol, &vl.probe(field_center(extent())), extent());
    let fe = render_shot(&ve, &se, ANCHOR);
    let fl = render_shot(&vl, &sl, ANCHOR);
    assert_ne!(
        fe.surface, fl.surface,
        "the wavefront film changes as the ridge sweeps"
    );
}

/// A SPARSELY-MASKED non-Gaussian field: scattered dropout over half the grid, one honest unmasked
/// peak, and a masked cell hiding a huge value. The grammar must (a) never read the hidden value
/// (no fake zero, no fake peak), and (b) drop to the colour-free density ramp under heavy dropout.
#[test]
fn a_sparsely_masked_field_never_fabricates_a_value_and_presents_honestly() {
    let mut data = vec![0.3f64; COLS * ROWS];
    // One honest, moderate, UNMASKED peak at (20,6).
    gaussian_at(&mut data, 20.0, 6.0, 1.5, 2.0);

    // Scatter a mask over a checkerboard of the left half — heavy, non-rectangular dropout.
    let mut mask = vec![true; COLS * ROWS];
    let mut masked = 0usize;
    for iy in 0..ROWS {
        for ix in 0..COLS {
            if ix < COLS / 2 && (ix + iy) % 2 == 0 {
                mask[iy * COLS + ix] = false;
                masked += 1;
            }
        }
    }
    // Bury a huge value under a masked cell — it must never surface as the peak.
    let decoy = 4 * COLS + 2; // (ix=2, iy=4): masked (2+4 even, ix<16)
    assert!(!mask[decoy], "the decoy cell is masked");
    data[decoy] = 1.0e9;

    let (obs, view) = observe_field(&data, Some(&mask), 0);

    // The peak is the honest unmasked lobe near (20,6), NOT the 1e9 hidden under the mask.
    let peak = obs.peak_center.expect("an unmasked peak exists");
    let pv = obs.peak_value.expect("peak value");
    assert!(
        pv < 100.0 && peak.x > 16.0,
        "the hidden 1e9 under a masked cell must never become the peak (got v={pv} at {peak:?})"
    );
    assert!(
        obs.masked_fraction > DROPOUT_DENSITY_THRESHOLD,
        "the dropout is heavy ({:.1}%), above the honest-presentation threshold",
        obs.masked_fraction * 100.0
    );

    // Under heavy dropout the grammar chooses the colour-free density ramp (intensity survives with
    // no colour at all) — an honest presentation of a gappy field.
    let shot = compose(&obs, &view.probe(peak), extent());
    assert_eq!(
        shot.colormap,
        Colormap::Density,
        "heavy dropout ⇒ the colour-free density ramp"
    );
    // A probe on the honest peak still punches in — the grammar is live on a non-Gaussian field.
    assert_eq!(shot.emphasis, Emphasis::PunchIn);

    // The reported masked fraction matches the dropout actually built (no miscount).
    let expected = masked as f64 / (COLS * ROWS) as f64;
    assert!(
        (obs.masked_fraction - expected).abs() < 1e-9,
        "masked fraction {} should equal the built dropout {expected}",
        obs.masked_fraction
    );
}
