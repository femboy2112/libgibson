//! The compilation seam: `(PlotSpec, PlotView, Rect) -> (PlotLayout, PlotReport)`.
//!
//! Semantic compilation, separate from glyph realization. `PlotLayout` holds
//! device geometry in **Braille subpixels** relative to the plot rectangle
//! (`2×` wide, `4×` tall per cell) — a fixed high-resolution grid the renderer
//! realizes under whatever capability is in force (so capability changes
//! realization, not this layer; law F). Tick *values* are size-independent; only
//! label drawing is responsive. The receipt counts exactly what happened to
//! every sample. See `docs/PLOT_OBSERVABLE_GEOMETRY.md` §2, §5, §8.

use crate::canvas::clip_line_to_bounds;
use crate::Rect;

use super::data::{Annotation, AxisSpec, PlotSpec, PlotView, Reduce, SeriesKind};
use super::scale::{AxisTransform, PlotTransform2D, Viewport};
use super::ticks::{major_ticks, Tick};

/// Fixed tick targets keep tick *values* independent of terminal size (law E).
const TARGET_X_TICKS: usize = 6;
const TARGET_Y_TICKS: usize = 5;

/// Execution receipt. Not a quality score — a record of what happened to the
/// data, so "off-viewport" is distinguishable from "all log-invalid" (doc §5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlotReport {
    pub samples_seen: usize,
    pub finite_samples: usize,
    pub nonfinite_rejected: usize,
    pub scale_domain_rejected: usize,
    pub segments_considered: usize,
    pub segments_clipped: usize,
    pub primitives_emitted: usize,
    /// Original sample count that entered a reducer (0 if no reduction ran).
    pub reduced_from: usize,
    /// Sample count a reducer produced (0 if no reduction ran).
    pub reduced_to: usize,
}

/// A tick resolved to an absolute cell coordinate along its axis.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedTick {
    pub value: f64,
    pub label: String,
    /// Absolute cell column (x axis) or row (y axis).
    pub cell: u16,
}

/// Device-space primitives for one series (Braille subpixel coordinates).
#[derive(Clone, Debug)]
pub enum Prims {
    Scatter(Vec<(i32, i32)>),
    Segments(Vec<((i32, i32), (i32, i32))>),
}

#[derive(Clone, Debug)]
pub struct ProjectedSeries {
    pub color: (u8, u8, u8),
    pub prims: Prims,
}

#[derive(Clone, Debug)]
pub enum ProjAnnotation {
    VLine {
        col_px: i32,
        color: (u8, u8, u8),
    },
    HLine {
        row_px: i32,
        color: (u8, u8, u8),
    },
    Point {
        x_px: i32,
        y_px: i32,
        label: String,
        color: (u8, u8, u8),
    },
}

/// Compiled, realization-agnostic plot geometry.
#[derive(Clone, Debug)]
pub struct PlotLayout {
    /// The full region compiled into, in CELLS (plot area plus margins).
    pub area: Rect,
    /// Drawable plot area in CELLS (inside the reserved axis/label margins).
    pub plot_rect: Rect,
    /// Braille subpixel extent of the plot area (`plot_rect.width*2 × *4`).
    pub px_w: u16,
    pub px_h: u16,
    pub x_ticks: Vec<ProjectedTick>,
    pub y_ticks: Vec<ProjectedTick>,
    pub series: Vec<ProjectedSeries>,
    pub annotations: Vec<ProjAnnotation>,
    /// Data→subpixel transform (for hit-testing / readouts). `None` if the view
    /// was not a valid domain for a scale (e.g. non-positive under `Log10`).
    pub transform: Option<PlotTransform2D>,
    pub title: String,
    pub x_axis: AxisSpec,
    pub y_axis: AxisSpec,
    /// Whether the renderer has room to draw labels/title (responsive).
    pub show_y_labels: bool,
    pub show_x_labels: bool,
    pub show_title: bool,
}

/// Reserve chrome margins. Responsive: at small sizes labels are dropped and the
/// plot keeps the space (doc §7 — data outranks chrome). Tick *values* are
/// unaffected.
fn reserve(area: Rect, y_ticks: &[Tick], has_title: bool) -> (Rect, bool, bool, bool) {
    let max_ylab = y_ticks
        .iter()
        .map(|t| t.label.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let show_y = area.width >= 24 && max_ylab > 0;
    let show_x = area.height >= 8;
    let show_title = has_title && area.height >= 6;

    let left = if show_y {
        (max_ylab + 1).min(area.width / 3)
    } else {
        0
    };
    let right = if area.width >= 24 { 1 } else { 0 };
    let bottom = if show_x { 2 } else { 0 }; // axis row + label row
    let top = if show_title { 1 } else { 0 };

    let w = area.width.saturating_sub(left + right);
    let h = area.height.saturating_sub(top + bottom);
    let rect = Rect::new(area.x + left, area.y + top, w, h);
    (rect, show_y && w > 0, show_x && h > 0, show_title)
}

/// Compile a spec + view into device geometry and a receipt.
pub fn compile(spec: &PlotSpec, view: &PlotView, area: Rect) -> (PlotLayout, PlotReport) {
    let mut report = PlotReport::default();

    // Tick VALUES — size-independent.
    let x_tick_vals = major_ticks(spec.x.scale, view.x, TARGET_X_TICKS);
    let y_tick_vals = major_ticks(spec.y.scale, view.y, TARGET_Y_TICKS);

    let (plot_rect, show_y_labels, show_x_labels, show_title) =
        reserve(area, &y_tick_vals, !spec.title.is_empty());

    let px_w = plot_rect.width.saturating_mul(2);
    let px_h = plot_rect.height.saturating_mul(4);

    // Build the transform (None if the view is not a valid domain for a scale).
    let transform = match (
        AxisTransform::new(spec.x.scale, view.x),
        AxisTransform::new(spec.y.scale, view.y),
    ) {
        (Some(xt), Some(yt)) if px_w > 0 && px_h > 0 => Some(PlotTransform2D::new(
            xt,
            yt,
            Viewport {
                ox: 0.0,
                oy: 0.0,
                w: px_w as f64,
                h: px_h as f64,
            },
        )),
        _ => None,
    };

    let mut proj_series = Vec::new();
    let max_x = px_w.saturating_sub(1) as i32;
    let max_y = px_h.saturating_sub(1) as i32;

    for s in &spec.series {
        report.samples_seen += s.points.len();
        for &(x, y) in &s.points {
            if x.is_finite() && y.is_finite() {
                report.finite_samples += 1;
            } else {
                report.nonfinite_rejected += 1;
            }
        }

        let Some(t) = transform else {
            // No valid transform: every finite sample is domain-rejected.
            report.scale_domain_rejected += s.points.len()
                - s.points
                    .iter()
                    .filter(|(x, y)| !x.is_finite() || !y.is_finite())
                    .count();
            continue;
        };

        // Explicit, opt-in reduction (monotone-X line only).
        let pts: Vec<(f64, f64)> = if s.kind == SeriesKind::Line
            && s.reduce == Reduce::ExtremaPerColumn
        {
            let r =
                super::data::reduce_extrema(&s.points, px_w as usize, view.x.min(), view.x.max());
            report.reduced_from += s.points.len();
            report.reduced_to += r.len();
            r
        } else {
            s.points.clone()
        };

        match s.kind {
            SeriesKind::Scatter => {
                let mut visible = Vec::new();
                for &(x, y) in &pts {
                    if !x.is_finite() || !y.is_finite() {
                        continue; // already counted; scatter drops gaps
                    }
                    match t.project(x, y) {
                        None => report.scale_domain_rejected += 1,
                        Some((px, py)) => {
                            let (ix, iy) = (px.round() as i32, py.round() as i32);
                            if ix >= 0 && ix <= max_x && iy >= 0 && iy <= max_y {
                                visible.push((ix, iy));
                                report.primitives_emitted += 1;
                            }
                        }
                    }
                }
                proj_series.push(ProjectedSeries {
                    color: s.color,
                    prims: Prims::Scatter(visible),
                });
            }
            SeriesKind::Line => {
                let mut segs = Vec::new();
                let mut prev: Option<(i32, i32)> = None;
                for &(x, y) in &pts {
                    if !x.is_finite() || !y.is_finite() {
                        prev = None; // gap: break the path, never bridge (law I)
                        continue;
                    }
                    match t.project(x, y) {
                        None => {
                            report.scale_domain_rejected += 1;
                            prev = None; // log-invalid also breaks the path
                        }
                        Some((px, py)) => {
                            let cur = (px.round() as i32, py.round() as i32);
                            if let Some(p0) = prev {
                                report.segments_considered += 1;
                                match clip_line_to_bounds(p0.0, p0.1, cur.0, cur.1, max_x, max_y) {
                                    Some((a, b)) => {
                                        segs.push((a, b));
                                        report.primitives_emitted += 1;
                                    }
                                    None => report.segments_clipped += 1,
                                }
                            }
                            prev = Some(cur);
                        }
                    }
                }
                proj_series.push(ProjectedSeries {
                    color: s.color,
                    prims: Prims::Segments(segs),
                });
            }
        }
    }

    // Project ticks to absolute cell coordinates (only those inside the view).
    let (x_ticks, y_ticks) = if let Some(t) = transform {
        let xt = t.x;
        let yt = t.y;
        let xs = x_tick_vals
            .into_iter()
            .filter_map(|tk| {
                let u = xt.project(tk.value)?;
                if !(0.0..=1.0).contains(&u) {
                    return None;
                }
                let cell =
                    plot_rect.x + (u * (plot_rect.width.saturating_sub(1)) as f64).round() as u16;
                Some(ProjectedTick {
                    value: tk.value,
                    label: tk.label,
                    cell,
                })
            })
            .collect();
        let ys = y_tick_vals
            .into_iter()
            .filter_map(|tk| {
                let v = yt.project(tk.value)?;
                if !(0.0..=1.0).contains(&v) {
                    return None;
                }
                let row = plot_rect.y
                    + ((1.0 - v) * (plot_rect.height.saturating_sub(1)) as f64).round() as u16;
                Some(ProjectedTick {
                    value: tk.value,
                    label: tk.label,
                    cell: row,
                })
            })
            .collect();
        (xs, ys)
    } else {
        (Vec::new(), Vec::new())
    };

    // Project annotations into subpixel space (same transform as the data).
    let mut annotations = Vec::new();
    if let Some(t) = transform {
        for a in &spec.annotations {
            match a {
                Annotation::VLine { x, color } => {
                    if let Some(u) = t.x.project(*x) {
                        if (0.0..=1.0).contains(&u) {
                            annotations.push(ProjAnnotation::VLine {
                                col_px: (u * px_w as f64).round() as i32,
                                color: *color,
                            });
                        }
                    }
                }
                Annotation::HLine { y, color } => {
                    if let Some(v) = t.y.project(*y) {
                        if (0.0..=1.0).contains(&v) {
                            annotations.push(ProjAnnotation::HLine {
                                row_px: ((1.0 - v) * px_h as f64).round() as i32,
                                color: *color,
                            });
                        }
                    }
                }
                Annotation::Point { x, y, label, color } => {
                    if let Some((px, py)) = t.project(*x, *y) {
                        let (ix, iy) = (px.round() as i32, py.round() as i32);
                        if ix >= 0 && ix <= max_x && iy >= 0 && iy <= max_y {
                            annotations.push(ProjAnnotation::Point {
                                x_px: ix,
                                y_px: iy,
                                label: label.clone(),
                                color: *color,
                            });
                        }
                    }
                }
            }
        }
    }

    let layout = PlotLayout {
        area,
        plot_rect,
        px_w,
        px_h,
        x_ticks,
        y_ticks,
        series: proj_series,
        annotations,
        transform,
        title: spec.title.clone(),
        x_axis: spec.x.clone(),
        y_axis: spec.y.clone(),
        show_y_labels,
        show_x_labels,
        show_title,
    };
    (layout, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::data::{Annotation, AxisSpec, PlotSpec, Reduce, Series};
    use crate::plot::scale::{AxisScale, FiniteRange};

    fn view(x0: f64, x1: f64, y0: f64, y1: f64) -> PlotView {
        PlotView::new(
            FiniteRange::new(x0, x1).unwrap(),
            FiniteRange::new(y0, y1).unwrap(),
        )
    }
    fn lin_spec() -> PlotSpec {
        PlotSpec::new(
            AxisSpec::new(AxisScale::Linear, "x"),
            AxisSpec::new(AxisScale::Linear, "y"),
        )
    }

    #[test]
    fn receipt_counts_nonfinite() {
        let spec = lin_spec().series(Series::line(vec![
            (0.0, 0.0),
            (1.0, 1.0),
            (f64::NAN, 5.0),
            (2.0, 2.0),
        ]));
        let (_, rep) = compile(&spec, &view(0.0, 3.0, 0.0, 3.0), Rect::new(0, 0, 80, 24));
        assert_eq!(rep.samples_seen, 4);
        assert_eq!(rep.finite_samples, 3);
        assert_eq!(rep.nonfinite_rejected, 1);
    }

    #[test]
    fn gap_never_bridged_law_i() {
        // (0,0)-(1,1) is one segment; the NaN breaks the path so (1,1)-(2,2) is
        // NOT formed across the gap. Exactly ONE segment considered.
        let spec = lin_spec().series(Series::line(vec![
            (0.0, 0.0),
            (1.0, 1.0),
            (f64::NAN, f64::NAN),
            (2.0, 2.0),
        ]));
        let (_, rep) = compile(&spec, &view(0.0, 3.0, 0.0, 3.0), Rect::new(0, 0, 80, 24));
        assert_eq!(rep.segments_considered, 1, "gap must break the path");
    }

    #[test]
    fn log_domain_rejection_counted_and_breaks_path() {
        // y log scale; a zero/negative y is domain-rejected and breaks the line.
        let spec = PlotSpec::new(
            AxisSpec::new(AxisScale::Linear, "x"),
            AxisSpec::new(AxisScale::Log10, "y"),
        )
        .series(Series::line(vec![(0.0, 1.0), (1.0, 0.0), (2.0, 100.0)]));
        let (_, rep) = compile(&spec, &view(0.0, 3.0, 1e-1, 1e3), Rect::new(0, 0, 80, 24));
        assert_eq!(rep.scale_domain_rejected, 1, "y=0 rejected under log");
        // the invalid middle point breaks the path: no segment spans it
        assert_eq!(rep.segments_considered, 0);
    }

    #[test]
    fn view_naturality_law_e() {
        // Same spec + view, two sizes: reject/finite counts are identical;
        // only geometry (plot_rect / px) differs.
        let spec = lin_spec().series(Series::scatter(vec![
            (0.0, 0.0),
            (1.0, 1.0),
            (f64::INFINITY, 2.0),
            (5.0, 5.0),
        ]));
        let v = view(0.0, 3.0, 0.0, 3.0);
        let (la, ra) = compile(&spec, &v, Rect::new(0, 0, 120, 40));
        let (lb, rb) = compile(&spec, &v, Rect::new(0, 0, 60, 20));
        assert_eq!(ra.samples_seen, rb.samples_seen);
        assert_eq!(ra.finite_samples, rb.finite_samples);
        assert_eq!(ra.nonfinite_rejected, rb.nonfinite_rejected);
        assert_eq!(ra.scale_domain_rejected, rb.scale_domain_rejected);
        assert_ne!((la.px_w, la.px_h), (lb.px_w, lb.px_h));
    }

    #[test]
    fn ticks_land_inside_plot_rect() {
        let spec = lin_spec()
            .title("t")
            .series(Series::line(vec![(0.0, 0.0), (10.0, 10.0)]));
        let (la, _) = compile(&spec, &view(0.0, 10.0, 0.0, 10.0), Rect::new(0, 0, 100, 30));
        assert!(!la.x_ticks.is_empty() && !la.y_ticks.is_empty());
        for t in &la.x_ticks {
            assert!(t.cell >= la.plot_rect.x && t.cell < la.plot_rect.x + la.plot_rect.width);
        }
        for t in &la.y_ticks {
            assert!(t.cell >= la.plot_rect.y && t.cell < la.plot_rect.y + la.plot_rect.height);
        }
    }

    #[test]
    fn reduce_records_receipt() {
        let pts: Vec<(f64, f64)> = (0..5000)
            .map(|i| (i as f64, (i as f64 * 0.01).sin()))
            .collect();
        let spec = lin_spec().series(Series::line(pts).reduce(Reduce::ExtremaPerColumn));
        let (_, rep) = compile(
            &spec,
            &view(0.0, 5000.0, -1.0, 1.0),
            Rect::new(0, 0, 100, 30),
        );
        assert_eq!(rep.reduced_from, 5000);
        assert!(rep.reduced_to > 0 && rep.reduced_to < 5000);
    }

    #[test]
    fn tiny_and_degenerate_no_panic() {
        let spec = lin_spec()
            .series(Series::line(vec![(0.0, 0.0), (1.0, 1.0)]))
            .annotate(Annotation::VLine {
                x: 0.5,
                color: (9, 9, 9),
            });
        for rect in [
            Rect::new(0, 0, 1, 1),
            Rect::new(0, 0, 3, 2),
            Rect::new(0, 0, 0, 0),
        ] {
            let (_la, _rep) = compile(&spec, &view(0.0, 1.0, 0.0, 1.0), rect);
        }
        // invalid log view: everything domain-rejected, no panic
        let logspec = PlotSpec::new(
            AxisSpec::new(AxisScale::Log10, "x"),
            AxisSpec::new(AxisScale::Linear, "y"),
        )
        .series(Series::line(vec![(1.0, 1.0), (10.0, 2.0)]));
        let (la, rep) = compile(
            &logspec,
            &view(-5.0, 5.0, 0.0, 3.0),
            Rect::new(0, 0, 80, 24),
        );
        assert!(la.transform.is_none());
        assert_eq!(rep.scale_domain_rejected, 2);
    }

    #[test]
    fn layer_monoid_law_h() {
        // Empty layer list = identity (no projected geometry).
        let (empty, _) = compile(
            &lin_spec(),
            &view(0.0, 1.0, 0.0, 1.0),
            Rect::new(0, 0, 80, 24),
        );
        assert!(empty.series.is_empty());

        // Layers are ORDERED overlays; z-order is load-bearing (NOT commutative).
        let a = Series::line(vec![(0.0, 0.0), (1.0, 1.0)]).color((1, 0, 0));
        let b = Series::scatter(vec![(0.5, 0.5)]).color((0, 0, 2));
        let v = view(0.0, 1.0, 0.0, 1.0);
        let (ab, _) = compile(
            &lin_spec().series(a.clone()).series(b.clone()),
            &v,
            Rect::new(0, 0, 80, 24),
        );
        let (ba, _) = compile(&lin_spec().series(b).series(a), &v, Rect::new(0, 0, 80, 24));
        assert_eq!(ab.series.len(), 2);
        assert_eq!(ab.series[0].color, (1, 0, 0));
        assert_eq!(
            ba.series[0].color,
            (0, 0, 2),
            "order preserved in the layer list"
        );
    }
}
