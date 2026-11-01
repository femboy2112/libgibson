//! Isolates compose **Rule 2** — dynamic range frames the shot (`cinema.rs`): a wider
//! range between a field's floor and its peak zooms the camera tighter.
//!
//! The existing #191 mutation battery (`tests/field_cinema_mutation.rs`) covers Rules 1
//! (focus), 3 (dropout→colormap) and 4 (probe→punch) with positive dependency tests, but
//! Rule 2 has **no** positive test: the model's hotspot is a constant unit-amplitude
//! Gaussian, so `range_span` is effectively flat across every tick the battery exercises,
//! and the only zoom coverage comes from Rule 4's punch multiplier. This file adds the
//! missing dimension — CONTRAST — with the battery's own discipline: deterministic,
//! pixel-checked, with teeth.
//!
//! The isolation that makes it a Rule-2 test and nothing else: two (or three) fields that
//! share the SAME peak cell (Rule 1 focus held), are probed COLD (Rule 4 stays Wide, no
//! punch multiplier on the zoom), and are lightly masked (Rule 3 stays Heat), so the only
//! observable that differs is `range_span` and the only shot field that moves is `zoom`.
//!
//! The one truthfulness trap, respected explicitly: `span_norm` clamps at 1.0
//! (`SPAN_REFERENCE = 1.2`), so the monotonic points all sit strictly below that ceiling
//! (spans 0.3 / 0.6 / 0.9), and a separate test exercises the clamp above it so the linear
//! points are proven to be on the linear part, not accidentally saturated.

#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;

use cinema::{
    compose, compose_blind, observe, render_shot, Emphasis, Model, SourceId, SPAN_REFERENCE,
    SPAN_ZOOM_GAIN, WIDE_ZOOM,
};
use gibson::field_view::{Colormap, FieldView};
use gibson::shape::Point;
use gibson::Rect;

const ANCHOR: Rect = Rect {
    x: 0,
    y: 0,
    width: 56,
    height: 18,
};

const TICK: u64 = 0;

fn model() -> Model {
    Model::new(SourceId(7), 1)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// A flat field at 0.0 everywhere except one peak cell set to `span`, so the finite
/// extremes are exactly `(0.0, span)` and `range_span == span`. The peak cell is fixed, so
/// `peak_center` (Rule 1's focus) is identical across every span. All cells present (mask
/// all-true), so `masked_fraction == 0` keeps Rule 3 on Heat.
fn ramp_field(span: f64) -> (Vec<f64>, Vec<bool>, (usize, usize)) {
    let cols = cinema::COLS;
    let rows = cinema::ROWS;
    let peak = (cols / 2, rows / 2);
    let mut data = vec![0.0f64; cols * rows];
    data[peak.1 * cols + peak.0] = span;
    let mask = vec![true; cols * rows];
    (data, mask, peak)
}

/// Build the shot for one span through the real pipeline, with a cold probe so Rule 4
/// holds Wide. Returns the observation and the composed shot.
fn shot_for_span(m: &Model, span: f64) -> (cinema::FieldObservation, cinema::Shot) {
    let (data, mask, _) = ramp_field(span);
    let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let obs = observe(&view, m.source(), m.epoch(), TICK);
    let cold = Point::new(1.0, 1.0); // off the central peak ⇒ Wide
    let shot = compose(&obs, &view.probe(cold), m.extent());
    (obs, shot)
}

/// THE HEADLINE: a wider dynamic range zooms the camera tighter, monotonically, with the
/// focus / colormap / emphasis all held constant — so the effect is Rule 2 alone.
#[test]
fn dynamic_range_zooms_tighter_monotonically() {
    let m = model();
    let spans = [0.3_f64, 0.6, 0.9];

    let mut shots = Vec::new();
    for &span in &spans {
        // Precondition (red-first discipline): each span is below the clamp ceiling, so it
        // lands on the linear part of Rule 2 rather than saturating.
        assert!(
            span < SPAN_REFERENCE,
            "span {span} must sit below SPAN_REFERENCE so the point is not clamped"
        );
        let (obs, shot) = shot_for_span(&m, span);
        assert!(
            close(obs.range_span, span),
            "observation must derive the intended range: {} != {span}",
            obs.range_span
        );
        assert!(
            obs.masked_fraction < cinema::DROPOUT_DENSITY_THRESHOLD,
            "light mask keeps Rule 3 on Heat"
        );
        assert_eq!(
            shot.emphasis,
            Emphasis::Wide,
            "cold probe holds Rule 4 Wide"
        );
        assert_eq!(shot.colormap, Colormap::Heat, "Rule 3 stays Heat");
        shots.push(shot);
    }

    // Rule 1 is held: every shot focuses on the same (fixed) peak cell.
    assert_eq!(shots[0].focus, shots[1].focus);
    assert_eq!(shots[1].focus, shots[2].focus);

    // Rule 2: zoom rises strictly with range, and every point is strictly inside the open
    // interval (WIDE_ZOOM, WIDE_ZOOM + SPAN_ZOOM_GAIN) — neither floored nor clamped.
    for s in &shots {
        assert!(
            s.zoom > WIDE_ZOOM && s.zoom < WIDE_ZOOM + SPAN_ZOOM_GAIN,
            "zoom {} must sit inside the open linear interval",
            s.zoom
        );
    }
    assert!(
        shots[0].zoom < shots[1].zoom && shots[1].zoom < shots[2].zoom,
        "more dynamic range must zoom tighter, monotonically: {} < {} < {}",
        shots[0].zoom,
        shots[1].zoom,
        shots[2].zoom
    );
}

/// The range-driven zoom actually reaches the PIXELS, not just the `Shot` struct. The two
/// shots differ in nothing BUT zoom (asserted field by field), so rendering them against
/// the SAME view isolates the framing change to Rule 2.
#[test]
fn the_range_driven_zoom_reaches_the_pixels() {
    let m = model();
    let (_, lo) = shot_for_span(&m, 0.3);
    let (_, hi) = shot_for_span(&m, 0.9);

    // The two shots differ ONLY in zoom.
    assert_eq!(lo.focus, hi.focus);
    assert_eq!(lo.colormap, hi.colormap);
    assert_eq!(lo.emphasis, hi.emphasis);
    assert_eq!(lo.probe_reading, hi.probe_reading);
    assert_eq!(
        (lo.source, lo.epoch, lo.tick),
        (hi.source, hi.epoch, hi.tick)
    );
    assert!(
        hi.zoom > lo.zoom,
        "precondition: the high-range shot zooms tighter"
    );

    // Render the SAME field with each shot: only the zoom differs, so a different frame
    // proves the range-driven zoom reaches the painted surface.
    let (data, mask, _) = ramp_field(0.9);
    let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let f_lo = render_shot(&view, &lo, ANCHOR);
    let f_hi = render_shot(&view, &hi, ANCHOR);
    assert_ne!(
        f_lo.surface, f_hi.surface,
        "a range-driven zoom change must change the painted frame"
    );
}

/// TEETH: a contrast-blind grammar freezes zoom at WIDE_ZOOM regardless of range, so the
/// two ranges the real grammar separates become indistinguishable in zoom. If Rule 2's
/// wiring were ever severed, the headline test would catch exactly this shape.
#[test]
fn contrast_blind_grammar_freezes_zoom_so_the_test_has_teeth() {
    let m = model();
    let (obs_lo, real_lo) = shot_for_span(&m, 0.3);
    let (obs_hi, real_hi) = shot_for_span(&m, 0.9);
    let cold = Point::new(1.0, 1.0);
    let probe = {
        // A probe report is needed for compose_blind's signature; its value is unused by
        // the blind grammar, so any view's cold probe is fine.
        let (data, mask, _) = ramp_field(0.9);
        let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
        view.probe(cold)
    };

    // The real grammar separates the two ranges...
    assert!(
        real_hi.zoom > real_lo.zoom,
        "the real grammar must zoom on range"
    );

    // ...a contrast-blind grammar does not: zoom is frozen for both.
    let blind_lo = compose_blind(&obs_lo, &probe, m.extent());
    let blind_hi = compose_blind(&obs_hi, &probe, m.extent());
    assert!(close(blind_lo.zoom, WIDE_ZOOM));
    assert!(close(blind_hi.zoom, WIDE_ZOOM));
    assert!(
        close(blind_lo.zoom, blind_hi.zoom),
        "a contrast-blind grammar must freeze zoom (the failure the headline guards)"
    );
}

/// The clamp is real, which is what proves the monotonic points above genuinely sit on the
/// linear part: a range at or beyond SPAN_REFERENCE saturates zoom at the ceiling
/// WIDE_ZOOM + SPAN_ZOOM_GAIN, and two such ranges are indistinguishable.
#[test]
fn range_zoom_clamps_at_the_reference_ceiling() {
    let m = model();
    let ceiling = WIDE_ZOOM + SPAN_ZOOM_GAIN;

    let (obs_ref, at_ref) = shot_for_span(&m, SPAN_REFERENCE);
    let (_, past_ref) = shot_for_span(&m, SPAN_REFERENCE * 2.0);

    assert!(close(obs_ref.range_span, SPAN_REFERENCE));
    assert!(
        close(at_ref.zoom, ceiling),
        "a range at the reference saturates zoom at the ceiling: {} != {ceiling}",
        at_ref.zoom
    );
    assert!(
        close(past_ref.zoom, ceiling),
        "a range beyond the reference is clamped to the same ceiling, not higher"
    );
}
