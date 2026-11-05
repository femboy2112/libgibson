//! Isolates the COMPOSITION of compose Rules 2 and 4 — the one interaction the
//! #191 batteries structurally cannot catch.
//!
//! `cinema.rs` builds the zoom in two steps: Rule 2 sets `zoom = WIDE_ZOOM +
//! SPAN_ZOOM_GAIN * span_norm` from the dynamic range, and Rule 4, when the probe
//! lands on a strong sample, does `zoom *= PUNCH_ZOOM_MUL`. The final zoom is a
//! **product** of a range term and a punch term.
//!
//! Every existing test pins one of those two contributors OFF:
//!   - the Gate D battery (`field_cinema_mutation.rs`) punches on a flat-range
//!     field, so its range term is ~WIDE_ZOOM and the punch multiplier is all
//!     that moves the zoom;
//!   - the Gate G contrast battery (`field_cinema_contrast.rs`) probes COLD, so
//!     Rule 4 never fires and only the range term moves.
//!
//! So a defect that made the punch *replace* the range zoom (`zoom =
//! PUNCH_ZOOM_MUL`) or *add* to it (`zoom += PUNCH_ZOOM_MUL`) instead of
//! multiplying would pass BOTH batteries and be caught by neither. This file adds
//! the missing dimension — COMPOSITION — with the battery's own discipline:
//! deterministic, pixel-checked, with teeth.
//!
//! The isolation that makes it a Rule-2×Rule-4 test and nothing else: the field is
//! fixed (same peak cell ⇒ Rule 1 focus held; all cells present ⇒ Rule 3 Heat),
//! and the ONLY thing toggled is whether the probe lands on the peak (Rule 4) at a
//! fixed, nontrivial dynamic range (Rule 2). The claim under test is that the
//! punch factor and the range factor are *separable* — the camera zoom is their
//! product, and the punch factor is constant across different ranges.

#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;

use cinema::{
    compose, compose_blind, observe, render_shot, Emphasis, Model, SourceId, PUNCH_ZOOM_MUL,
    SPAN_REFERENCE, SPAN_ZOOM_GAIN, WIDE_ZOOM,
};
use gibson::field_view::FieldView;
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

/// The pure Rule-2 range term at `span` (on the linear part, below the clamp).
fn range_zoom(span: f64) -> f64 {
    let span_norm = (span / SPAN_REFERENCE).clamp(0.0, 1.0);
    WIDE_ZOOM + SPAN_ZOOM_GAIN * span_norm
}

/// A flat field at 0.0 with one peak cell set to `span`, so the finite extremes
/// are exactly `(0.0, span)`: `range_span == span` AND `peak_value == span`. The
/// peak cell is fixed, so Rule 1's focus is identical regardless of `span`; all
/// cells present, so Rule 3 stays on Heat.
fn ramp_field(span: f64) -> (Vec<f64>, Vec<bool>) {
    let cols = cinema::COLS;
    let rows = cinema::ROWS;
    let mut data = vec![0.0f64; cols * rows];
    data[(rows / 2) * cols + (cols / 2)] = span;
    (data, vec![true; cols * rows])
}

/// Compose a shot through the real observe→probe→compose pipeline at dynamic
/// range `span`, probing either the peak cell (Rule 4 punches in) or a cold
/// off-peak point (Rule 4 holds Wide).
fn shot(m: &Model, span: f64, punch: bool) -> cinema::Shot {
    let (data, mask) = ramp_field(span);
    let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let obs = observe(&view, m.source(), m.epoch(), TICK);
    let probe_pt = if punch {
        // The world center of the peak cell reads `span == peak_value`, so
        // `pv >= PUNCH_PEAK_FRACTION * peak` holds and Rule 4 fires.
        obs.peak_center.expect("ramp field has a finite peak")
    } else {
        Point::new(1.0, 1.0) // off the central peak ⇒ reads 0.0 ⇒ Wide
    };
    compose(&obs, &view.probe(probe_pt), m.extent())
}

/// THE HEADLINE: the punch MULTIPLIES the range zoom; it does not replace or add
/// to it. At a fixed, nontrivial dynamic range, turning the probe from cold to
/// on-peak scales the zoom by exactly `PUNCH_ZOOM_MUL` — and the result is
/// provably distinct from the replace and the additive mutants.
#[test]
fn punch_multiplies_the_range_zoom_it_does_not_replace_it() {
    let m = model();
    let span = 0.9_f64;
    // Red-first precondition: the span is below the clamp ceiling (linear part of
    // Rule 2) AND strictly lifts the range term above WIDE_ZOOM, so the product
    // genuinely differs from a bare punch multiplier.
    assert!(
        span < SPAN_REFERENCE,
        "span must be on the linear part of Rule 2"
    );
    let range = range_zoom(span);
    assert!(
        range > WIDE_ZOOM + 1e-6,
        "the range term must be nontrivial, else replace and multiply coincide"
    );

    let wide = shot(&m, span, false);
    let punch = shot(&m, span, true);

    // Rule 4 fired in exactly one of them.
    assert_eq!(wide.emphasis, Emphasis::Wide, "cold probe must hold Wide");
    assert_eq!(
        punch.emphasis,
        Emphasis::PunchIn,
        "peak probe must punch in"
    );

    // The wide shot is the pure Rule-2 range term (punch absent).
    assert!(
        close(wide.zoom, range),
        "wide zoom {} must equal the pure range term {range}",
        wide.zoom
    );

    // The composed zoom is the range term times the punch multiplier — a product.
    assert!(
        close(punch.zoom, range * PUNCH_ZOOM_MUL),
        "composed zoom {} must be range*punch = {}",
        punch.zoom,
        range * PUNCH_ZOOM_MUL
    );
    assert!(
        close(punch.zoom, wide.zoom * PUNCH_ZOOM_MUL),
        "the punch must scale the range zoom, not stand alone"
    );

    // Anti-conflation: the composed zoom is NOT the replace mutant (zoom =
    // PUNCH_ZOOM_MUL), NOT the additive-constant mutant (zoom += PUNCH_ZOOM_MUL),
    // and NOT the additive-gain mutant (zoom += PUNCH_ZOOM_MUL - 1). With a
    // nontrivial range term these three values are all distinct from the product.
    assert!(
        !close(punch.zoom, PUNCH_ZOOM_MUL),
        "must not be the replace mutant"
    );
    assert!(
        !close(punch.zoom, wide.zoom + PUNCH_ZOOM_MUL),
        "must not be the additive-constant mutant"
    );
    assert!(
        !close(punch.zoom, wide.zoom + (PUNCH_ZOOM_MUL - 1.0)),
        "must not be the additive-gain mutant"
    );

    // Composition isolated to the zoom/emphasis axis: Rules 1 and 3 held fixed.
    assert_eq!(wide.focus, punch.focus, "Rule 1 focus must be held fixed");
    assert_eq!(
        wide.colormap, punch.colormap,
        "Rule 3 colormap must be held fixed"
    );
}

/// SEPARABILITY: the punch factor is CONSTANT across different dynamic ranges, so
/// the two axes compose multiplicatively rather than being entangled in one
/// `f(span, punch)`. The range axis is live (the wide zoom differs between spans),
/// and the product carries that difference through (the punch zoom differs too),
/// yet the ratio punch/wide is `PUNCH_ZOOM_MUL` at every span.
#[test]
fn the_punch_factor_is_constant_across_range_so_the_axes_are_separable() {
    let m = model();
    let spans = [0.6_f64, 0.9];
    for &s in &spans {
        assert!(s < SPAN_REFERENCE, "span {s} must stay on the linear part");
    }

    let wides: Vec<f64> = spans.iter().map(|&s| shot(&m, s, false).zoom).collect();
    let punches: Vec<f64> = spans.iter().map(|&s| shot(&m, s, true).zoom).collect();

    // Range axis is live: the wide (pure-range) zoom genuinely moves with span.
    assert!(
        !close(wides[0], wides[1]),
        "the range axis must be live across the two spans"
    );
    // The product carries the range difference through the punch.
    assert!(
        !close(punches[0], punches[1]),
        "the composed zoom must carry the range difference under punch"
    );

    // The punch factor is the SAME constant at both ranges ⇒ separable product.
    for i in 0..spans.len() {
        assert!(
            close(punches[i] / wides[i], PUNCH_ZOOM_MUL),
            "punch factor at span {} is {} (expected {PUNCH_ZOOM_MUL})",
            spans[i],
            punches[i] / wides[i]
        );
    }
}

/// TEETH: under the field-blind grammar the zoom is frozen at `WIDE_ZOOM` and the
/// emphasis never punches, so the punch factor is 1.0 — the separability assertion
/// above is non-vacuous only because the blind grammar fails it.
#[test]
fn blind_grammar_has_no_punch_factor_so_the_composition_test_has_teeth() {
    let m = model();
    let span = 0.9_f64;
    let (data, mask) = ramp_field(span);
    let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let obs = observe(&view, m.source(), m.epoch(), TICK);
    let peak = obs.peak_center.expect("ramp field has a finite peak");

    let blind_wide = compose_blind(&obs, &view.probe(Point::new(1.0, 1.0)), m.extent());
    let blind_punch = compose_blind(&obs, &view.probe(peak), m.extent());

    assert!(
        close(blind_wide.zoom, WIDE_ZOOM),
        "blind freezes zoom at WIDE_ZOOM"
    );
    assert!(
        close(blind_punch.zoom, WIDE_ZOOM),
        "blind ignores the peak probe"
    );
    assert_eq!(
        blind_punch.emphasis,
        Emphasis::Wide,
        "blind never punches in"
    );
    assert!(
        !close(blind_punch.zoom / blind_wide.zoom, PUNCH_ZOOM_MUL),
        "blind punch factor must be 1.0, not {PUNCH_ZOOM_MUL} — this is the teeth"
    );
}

/// The composed zoom reaches the PIXELS, not just the `Shot` struct: the wide and
/// punch shots share focus and colormap and differ only in zoom (and emphasis), so
/// rendering both against the same anchor and getting different surfaces proves the
/// product zoom reaches the painted frame.
#[test]
fn the_composed_zoom_reaches_the_pixels() {
    let m = model();
    let span = 0.9_f64;
    let (data, mask) = ramp_field(span);
    let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let obs = observe(&view, m.source(), m.epoch(), TICK);
    let peak = obs.peak_center.expect("ramp field has a finite peak");

    let wide = compose(&obs, &view.probe(Point::new(1.0, 1.0)), m.extent());
    let punch = compose(&obs, &view.probe(peak), m.extent());

    // Differ in nothing but zoom + emphasis.
    assert_eq!(wide.focus, punch.focus);
    assert_eq!(wide.colormap, punch.colormap);
    assert!(
        !close(wide.zoom, punch.zoom),
        "the composed zoom must differ"
    );

    let surf_wide = render_shot(&view, &wide, ANCHOR).surface;
    let surf_punch = render_shot(&view, &punch, ANCHOR).surface;
    assert_ne!(
        surf_wide, surf_punch,
        "the product zoom must reach the painted pixels"
    );
}
