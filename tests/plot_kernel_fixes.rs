//! POST-CANARY regression tests for the plotting-kernel breaks the Project
//! Pulsar integration canary (and the adversarial fork) surfaced against the
//! frozen candidate `068807d`.
//!
//! The canary first-contact SHA stays immutable evidence; these tests pin the
//! CORRECTED behaviour on the fix commit that follows it. One root defect — the
//! `[0,1]² → device` map answered "last drawable index" inconsistently
//! (`u=1 → px_w`, one subpixel past the last valid index `px_w-1`, while ticks
//! used `width-1`) — produced three symptoms, all exercised here:
//!
//! - NEW-1 (HIGH): scatter points on the view extent silently dropped, with a
//!   receipt that reported zero rejections (the honesty violation).
//! - F2: edge annotations (`VLine` at `x.max`, `HLine` at `y.min`) not drawn.
//! - F3: a datum exactly on a tick value realized one cell off its tick mark.
//!
//! Plus the receipt-completeness gap (no scatter-clip counter) and F1 (axis
//! captions carried but never rendered).

use gibson::plot::{
    self, Annotation, AxisScale, AxisSpec, FiniteRange, PlotSpec, PlotView, Series,
};
use gibson::{Rect, SubcellGlyphMode};

fn fr(a: f64, b: f64) -> FiniteRange {
    FiniteRange::new(a, b).expect("range")
}

fn lin_spec() -> PlotSpec {
    PlotSpec::new(
        AxisSpec::new(AxisScale::Linear, "x"),
        AxisSpec::new(AxisScale::Linear, "y"),
    )
}

fn braille_cells_in(s: &gibson::Surface, rect: Rect) -> usize {
    let mut n = 0;
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            if let Some(c) = s.get(x, y) {
                if c.glyph
                    .grapheme
                    .chars()
                    .any(|ch| ('\u{2800}'..='\u{28FF}').contains(&ch))
                {
                    n += 1;
                }
            }
        }
    }
    n
}

/// NEW-1 (HIGH). The fit-to-extent pattern: the view is fit EXACTLY to the data,
/// so points sit on `x.min/x.max/y.min/y.max`. Every one is finite and in the
/// linear domain, therefore every one must be realized — none may silently
/// vanish, and the receipt must account for all of them.
#[test]
fn scatter_on_view_extent_is_drawn_not_silently_lost() {
    let spec = lin_spec().series(Series::scatter(vec![
        (0.0, 0.0),   // x.min, y.min
        (10.0, 0.0),  // x.max, y.min
        (0.0, 10.0),  // x.min, y.max
        (10.0, 10.0), // x.max, y.max
        (5.0, 5.0),   // interior
    ]));
    let view = PlotView::new(fr(0.0, 10.0), fr(0.0, 10.0));
    let (_, rep) = plot::compile(&spec, &view, Rect::new(0, 0, 80, 24));

    assert_eq!(rep.finite_samples, 5);
    assert_eq!(rep.scale_domain_rejected, 0);
    // No fit-to-extent point is off the viewport, so none is clipped...
    assert_eq!(
        rep.points_clipped, 0,
        "points on the view extent are inside the viewport"
    );
    // ...and all five are drawn.
    assert_eq!(
        rep.primitives_emitted, 5,
        "every corner + interior point must be realized"
    );
    // Receipt completeness for a scatter series: every finite in-domain sample
    // is either emitted or clipped — never silently gone.
    assert_eq!(
        rep.primitives_emitted + rep.points_clipped + rep.scale_domain_rejected,
        rep.finite_samples
    );
}

/// The receipt must distinguish "outside the viewport" from "invalid for the
/// scale". An off-VIEW (but finite, in-domain) point is counted as clipped, not
/// as a domain rejection, and not silently dropped.
#[test]
fn off_view_scatter_point_is_counted_as_clipped() {
    let spec = lin_spec().series(Series::scatter(vec![(100.0, 100.0), (0.5, 0.5)]));
    let view = PlotView::new(fr(0.0, 1.0), fr(0.0, 1.0));
    let (_, rep) = plot::compile(&spec, &view, Rect::new(0, 0, 80, 24));

    assert_eq!(rep.finite_samples, 2);
    assert_eq!(
        rep.scale_domain_rejected, 0,
        "off-view is not domain-invalid under Linear"
    );
    assert_eq!(rep.primitives_emitted, 1, "the in-view point draws");
    assert_eq!(
        rep.points_clipped, 1,
        "the off-view point is COUNTED, not vanished"
    );
}

/// F2. Edge annotations at the view extent must draw at BOTH bounds — the fix
/// removes the asymmetry where `VLine@x.max` / `HLine@y.min` landed one subpixel
/// off the canvas.
#[test]
fn edge_annotations_draw_at_both_bounds() {
    let view = PlotView::new(fr(0.0, 10.0), fr(0.0, 10.0));
    let c = (255, 255, 255);
    let drew = |ann: Annotation| {
        let spec = lin_spec().annotate(ann);
        let (layout, _) = plot::compile(&spec, &view, Rect::new(0, 0, 60, 20));
        let s = plot::render(&layout, SubcellGlyphMode::Braille2x4);
        braille_cells_in(&s, layout.plot_rect) > 0
    };
    assert!(drew(Annotation::VLine { x: 0.0, color: c }), "VLine x.min");
    assert!(drew(Annotation::VLine { x: 10.0, color: c }), "VLine x.max");
    assert!(drew(Annotation::HLine { y: 0.0, color: c }), "HLine y.min");
    assert!(drew(Annotation::HLine { y: 10.0, color: c }), "HLine y.max");
}

/// F3. A datum exactly on a tick value must realize into the SAME cell column as
/// the tick mark — data, annotations and ticks share one last-index convention.
#[test]
fn datum_on_tick_value_realizes_in_the_tick_cell() {
    let view = PlotView::new(fr(0.0, 1.0), fr(0.0, 1.0));
    for width in [80u16, 90, 100, 111, 120, 130] {
        // First compile with no series to read the tick columns.
        let (base, _) = plot::compile(&lin_spec(), &view, Rect::new(0, 0, width, 30));
        let tick_vals: Vec<f64> = base.x_ticks.iter().map(|t| t.value).collect();
        let tick_cells: Vec<u16> = base.x_ticks.iter().map(|t| t.cell).collect();

        // A scatter point at each tick value (y at mid-view).
        let pts: Vec<(f64, f64)> = tick_vals.iter().map(|&v| (v, 0.5)).collect();
        let spec = lin_spec().series(Series::scatter(pts));
        let (layout, _) = plot::compile(&spec, &view, Rect::new(0, 0, width, 30));
        let surf = plot::render(&layout, SubcellGlyphMode::Braille2x4);

        for (i, &cell) in tick_cells.iter().enumerate() {
            // The point for tick i must have drawn a braille dot somewhere in the
            // tick's own cell column, inside the plot rows.
            let mut found = false;
            for y in layout.plot_rect.y..layout.plot_rect.y + layout.plot_rect.height {
                if let Some(c) = surf.get(cell, y) {
                    if c.glyph
                        .grapheme
                        .chars()
                        .any(|ch| ('\u{2800}'..='\u{28FF}').contains(&ch))
                    {
                        found = true;
                        break;
                    }
                }
            }
            assert!(
                found,
                "width {width}: datum on tick {i} (value {}) must land in tick cell column {cell}",
                tick_vals[i]
            );
        }
    }
}

/// F1. Axis captions (label + unit) were carried in the layout but never drawn.
/// At a comfortable size they must now render for both axes.
#[test]
fn axis_captions_are_rendered() {
    let spec = PlotSpec::new(
        AxisSpec::new(AxisScale::Linear, "frequency").unit("Hz"),
        AxisSpec::new(AxisScale::Linear, "power").unit("dB"),
    )
    .title("Spectrum")
    .series(Series::line(vec![(0.0, 0.0), (1.0, 1.0)]));
    let view = PlotView::new(fr(0.0, 1.0), fr(0.0, 1.0));
    let (surf, _) = plot::plot(
        &spec,
        &view,
        Rect::new(0, 0, 120, 40),
        SubcellGlyphMode::Braille2x4,
    );
    let text = surf.to_visible_lines().join("\n");
    assert!(text.contains("frequency"), "x-axis label rendered");
    assert!(text.contains("Hz"), "x-axis unit rendered");
    assert!(text.contains("power"), "y-axis label rendered");
    assert!(text.contains("dB"), "y-axis unit rendered");
}
