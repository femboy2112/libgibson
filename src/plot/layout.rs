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

/// A **configuration** fault: the plot spec + view do not describe a valid
/// drawing at all, independent of any sample. This is ontologically distinct
/// from a *sample* that is merely out of a valid scale's domain
/// (`PlotReport::scale_domain_rejected`) — a malformed request versus a datum
/// with no image under a well-formed request. `compile` returns `Err(..)` for a
/// configuration fault and never fabricates per-sample rejections from it (doc
/// §4; SAI crossover law A — "unknown ≠ free", a bad request is its own state).
///
/// A zero-area plot rectangle is **not** a configuration fault: a valid spec can
/// legitimately compile to an empty realization (transform `None`, nothing
/// drawn, nothing rejected), so it stays an `Ok` with empty geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotError {
    /// The X view is not a valid domain for the X scale — e.g. `Log10` over a
    /// range whose `min ≤ 0`. The X axis transform has no definition, so no
    /// sample could be placed regardless of its value. Checked before Y.
    InvalidXAxisDomain,
    /// The Y view is not a valid domain for the Y scale (checked after X is
    /// found valid).
    InvalidYAxisDomain,
}

/// Execution receipt. Not a quality score — a record of what happened to the
/// data, so "off-viewport" is distinguishable from "all log-invalid" (doc §5).
///
/// The counters divide into two **gates** that never cross (SAI crossover law D):
/// *acceptance* counts (`finite_samples`, `nonfinite_rejected`,
/// `scale_domain_rejected`, and the reducer *request* accounting) are a property
/// of (scale, view, spec) alone and are independent of terminal size, capability,
/// and the device rectangle; *realization* counts (`segments_*`, `points_clipped`,
/// `points_emitted`, `segments_emitted`, `reduced_*`) depend on the device
/// rectangle. Terminal size can never move an acceptance counter (law E).
///
/// Realized output is split by **object kind** — points and segments are not the
/// same object (doc §5), so they are never summed into one counter. A mixed
/// Line+Scatter plot therefore stays auditable per kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlotReport {
    pub samples_seen: usize,
    pub finite_samples: usize,
    pub nonfinite_rejected: usize,
    pub scale_domain_rejected: usize,
    pub segments_considered: usize,
    pub segments_clipped: usize,
    /// Scatter points that were finite AND inside the scale domain, but projected
    /// to a pixel outside the plot viewport and so were not drawn. The scatter
    /// analogue of `segments_clipped`: without it a fit-to-extent scatter could
    /// lose points with every rejection count reading zero (doc §5 — plotting
    /// must never silently eat data).
    pub points_clipped: usize,
    /// Scatter **points** drawn (scatter series only). Kept distinct from
    /// `segments_emitted` so a mixed plot never conflates the two realization
    /// kinds into one number (the conflation that broke the §5 conservation law).
    pub points_emitted: usize,
    /// Line **segments** drawn (line series only).
    pub segments_emitted: usize,
    /// Original sample count that entered a reducer (0 if none applied). Includes
    /// any gap samples in the input, matching `reduced_to`.
    pub reduced_from: usize,
    /// Output length a reducer produced (0 if none applied). Includes the
    /// `(NaN,NaN)` gap sentinels the reducer injects to break the path, matching
    /// `reduced_from`; it is in no conservation equation.
    pub reduced_to: usize,
    /// Series carrying a reduce policy (`reduce != Reduce::None`). An **acceptance**
    /// count: it is a property of the spec, independent of terminal size — a
    /// zero-area plot still reports the request (doc §5).
    pub reducers_requested: usize,
    /// Requested reducers that were **declined** and rendered unreduced because
    /// they are not applicable: a scatter series (cannot reduce) or a line whose X
    /// is not nondecreasing (`ExtremaPerColumn`'s proven domain). Also size-
    /// independent. `reducers_requested − reducers_declined` were applied. The
    /// scientific reducer is never run outside its domain; the plot stays correct,
    /// just not downsampled (doc §6; SAI crossover law C — requested vs effective).
    pub reducers_declined: usize,
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
    /// Data→subpixel transform (for hit-testing / readouts). `None` **only** when
    /// the plot rectangle has zero area (nothing to realize) — an invalid scale
    /// *view* is a configuration fault that `compile` rejects with `Err` and so
    /// never reaches a `PlotLayout` (doc §4).
    pub transform: Option<PlotTransform2D>,
    pub title: String,
    pub x_axis: AxisSpec,
    pub y_axis: AxisSpec,
    /// Whether the renderer has room to draw labels/title (responsive).
    pub show_y_labels: bool,
    pub show_x_labels: bool,
    pub show_title: bool,
    /// Whether there is reserved room for the axis caption (label + unit).
    pub show_x_title: bool,
    pub show_y_title: bool,
}

/// Reserve chrome margins. Responsive: at small sizes labels are dropped and the
/// plot keeps the space (doc §7 — data outranks chrome). Tick *values* are
/// unaffected.
struct Chrome {
    rect: Rect,
    show_y_labels: bool,
    show_x_labels: bool,
    show_title: bool,
    show_x_title: bool,
    show_y_title: bool,
}

fn reserve(
    area: Rect,
    y_ticks: &[Tick],
    has_title: bool,
    has_x_label: bool,
    has_y_label: bool,
) -> Chrome {
    let max_ylab = y_ticks
        .iter()
        .map(|t| t.label.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let show_y = area.width >= 24 && max_ylab > 0;
    let show_x = area.height >= 8;
    let show_title = has_title && area.height >= 6;
    // Axis captions cost a whole margin row each; only reserve them once the plot
    // has height to spare, so at tiny sizes the data keeps the space (doc §7).
    let show_x_title = has_x_label && show_x && area.height >= 12;
    let show_y_title = has_y_label && show_y && area.width >= 30 && area.height >= 12;

    let left = if show_y {
        (max_ylab + 1).min(area.width / 3)
    } else {
        0
    };
    let right = if area.width >= 24 { 1 } else { 0 };
    // bottom: axis row + tick-label row (+ x-caption row); top: title (+ y-caption).
    let bottom = (if show_x { 2 } else { 0 }) + u16::from(show_x_title);
    let top = u16::from(show_title) + u16::from(show_y_title);

    let w = area.width.saturating_sub(left + right);
    let h = area.height.saturating_sub(top + bottom);
    let rect = Rect::new(area.x + left, area.y + top, w, h);
    Chrome {
        rect,
        show_y_labels: show_y && w > 0,
        show_x_labels: show_x && h > 0,
        show_title,
        show_x_title: show_x_title && h > 0,
        show_y_title: show_y_title && h > 0,
    }
}

/// Compile a spec + view into device geometry and a receipt.
///
/// Two failure ontologies are kept strictly apart (doc §4; SAI crossover law A):
/// a malformed **configuration** — a scale view that is not a valid domain, e.g.
/// `Log10` over a non-positive range — returns `Err(PlotError)` and reads no
/// samples; a well-formed configuration always returns `Ok`, and individual
/// samples with no image under a *valid* scale are counted in
/// `PlotReport::scale_domain_rejected`. A zero-area rectangle is a valid
/// configuration that realizes nothing — `Ok` with `transform == None`.
pub fn compile(
    spec: &PlotSpec,
    view: &PlotView,
    area: Rect,
) -> Result<(PlotLayout, PlotReport), PlotError> {
    let mut report = PlotReport::default();

    // CONFIGURATION GATE (first, before any sample or tick is read; law A). The
    // axis transforms are a property of (scale, view) ALONE — independent of
    // terminal size and of the data. A scale whose view is outside its domain
    // (e.g. Log10 with min ≤ 0) has no transform at all: that is a malformed
    // request, NOT a basis for rejecting otherwise-valid samples. X before Y.
    let xt = AxisTransform::new(spec.x.scale, view.x).ok_or(PlotError::InvalidXAxisDomain)?;
    let yt = AxisTransform::new(spec.y.scale, view.y).ok_or(PlotError::InvalidYAxisDomain)?;

    // Tick VALUES — size-independent.
    let x_tick_vals = major_ticks(spec.x.scale, view.x, TARGET_X_TICKS);
    let y_tick_vals = major_ticks(spec.y.scale, view.y, TARGET_Y_TICKS);

    let chrome = reserve(
        area,
        &y_tick_vals,
        !spec.title.is_empty(),
        !spec.x.label.is_empty(),
        !spec.y.label.is_empty(),
    );
    let plot_rect = chrome.rect;

    let px_w = plot_rect.width.saturating_mul(2);
    let px_h = plot_rect.height.saturating_mul(4);

    // The full device transform additionally requires a non-degenerate plot
    // rectangle. A zero-area rect leaves `transform == None` with the axes still
    // valid: nothing is *realized*, but nothing is *rejected* either (law E) —
    // the realization gate is independent of the (already-passed) config gate.
    let transform = if px_w > 0 && px_h > 0 {
        Some(PlotTransform2D::new(
            xt,
            yt,
            // The drawable subpixel grid is indexed `0..=px-1`, so normalized
            // `u/v ∈ [0,1]` must map onto the span `[0, px-1]`: `u=1` lands on the
            // LAST valid index, not one subpixel past it. Passing `px` here was the
            // single off-by-one behind NEW-1/F2/F3 — a datum on the view extent
            // rounded to an out-of-bounds pixel and was silently dropped.
            Viewport {
                ox: 0.0,
                oy: 0.0,
                w: (px_w - 1) as f64,
                h: (px_h - 1) as f64,
            },
        ))
    } else {
        None
    };

    let mut proj_series = Vec::new();
    let max_x = px_w.saturating_sub(1) as i32;
    let max_y = px_h.saturating_sub(1) as i32;

    for s in &spec.series {
        report.samples_seen += s.points.len();
        // Acceptance counts (size- AND reduction-independent, law E): a finite
        // sample is domain-rejected iff an axis transform has no image for it.
        for &(x, y) in &s.points {
            if x.is_finite() && y.is_finite() {
                report.finite_samples += 1;
                // Under a VALID scale (config gate already passed), a finite
                // sample is domain-rejected iff it has no image — e.g. y ≤ 0 on a
                // valid Log10 axis. This is a per-sample fact, never a stand-in
                // for a malformed view (that became an `Err` above; law A).
                let in_domain = xt.project(x).is_some() && yt.project(y).is_some();
                if !in_domain {
                    report.scale_domain_rejected += 1;
                }
            } else {
                report.nonfinite_rejected += 1;
            }
        }

        // Reducer REQUEST accounting — an acceptance-gate fact, computed here so it
        // is independent of terminal size (a zero-area plot still records that a
        // reducer was requested; law E / SAI law C). `requested` matches the doc
        // definition (any non-`None` policy); a request is *applicable* only on a
        // monotone-X line, and a non-applicable request (scatter, or non-monotone
        // line) is DECLINED, not silently swallowed. The actual reduction (and its
        // `reduced_*` counts) is a realization step gated on the viewport below.
        let requested_reduce = s.reduce != Reduce::None;
        let can_reduce = s.kind == SeriesKind::Line
            && s.reduce == Reduce::ExtremaPerColumn
            && super::data::is_nondecreasing_x(&s.points);
        if requested_reduce {
            report.reducers_requested += 1;
            if !can_reduce {
                report.reducers_declined += 1;
            }
        }

        // Realization requires a viewport. Without one the acceptance counts
        // above stand and no geometry is emitted (an empty layer, not a reject).
        let Some(t) = transform else {
            proj_series.push(ProjectedSeries {
                color: s.color,
                prims: match s.kind {
                    SeriesKind::Scatter => Prims::Scatter(Vec::new()),
                    SeriesKind::Line => Prims::Segments(Vec::new()),
                },
            });
            continue;
        };

        // Apply the reduction only when it was found applicable above (monotone-X
        // line). This is the realization half: it needs the viewport's column
        // count, and `reduced_*` are therefore device-dependent.
        let pts: Vec<(f64, f64)> = if can_reduce {
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
                        None => {} // domain-rejected: already counted (first pass)
                        Some((px, py)) => {
                            let (ix, iy) = (px.round() as i32, py.round() as i32);
                            if ix >= 0 && ix <= max_x && iy >= 0 && iy <= max_y {
                                visible.push((ix, iy));
                                report.points_emitted += 1;
                            } else {
                                // Finite and in-domain, but outside the viewport:
                                // counted, never silently eaten (doc §5).
                                report.points_clipped += 1;
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
                            // domain-rejected (already counted); breaks the path
                            prev = None;
                        }
                        Some((px, py)) => {
                            let cur = (px.round() as i32, py.round() as i32);
                            if let Some(p0) = prev {
                                report.segments_considered += 1;
                                match clip_line_to_bounds(p0.0, p0.1, cur.0, cur.1, max_x, max_y) {
                                    Some((a, b)) => {
                                        segs.push((a, b));
                                        report.segments_emitted += 1;
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
        // Project ticks through the SAME device transform + rounding + subcell
        // reduction the data uses, so a datum sitting on a tick value realizes in
        // the tick's own cell (one shared last-index convention; fixes F3).
        let xs = x_tick_vals
            .into_iter()
            .filter_map(|tk| {
                let u = t.x.project(tk.value)?;
                if !(0.0..=1.0).contains(&u) {
                    return None;
                }
                let sx = t.viewport.map(u, 0.0).0.round().clamp(0.0, max_x as f64) as u16;
                Some(ProjectedTick {
                    value: tk.value,
                    label: tk.label,
                    cell: plot_rect.x + sx / 2,
                })
            })
            .collect();
        let ys = y_tick_vals
            .into_iter()
            .filter_map(|tk| {
                let v = t.y.project(tk.value)?;
                if !(0.0..=1.0).contains(&v) {
                    return None;
                }
                let sy = t.viewport.map(0.0, v).1.round().clamp(0.0, max_y as f64) as u16;
                Some(ProjectedTick {
                    value: tk.value,
                    label: tk.label,
                    cell: plot_rect.y + sy / 4,
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
                            // Same viewport map as the data: `u=1` lands on the
                            // last drawable column, not one subpixel past it (F2).
                            annotations.push(ProjAnnotation::VLine {
                                col_px: t.viewport.map(u, 0.0).0.round() as i32,
                                color: *color,
                            });
                        }
                    }
                }
                Annotation::HLine { y, color } => {
                    if let Some(v) = t.y.project(*y) {
                        if (0.0..=1.0).contains(&v) {
                            annotations.push(ProjAnnotation::HLine {
                                row_px: t.viewport.map(0.0, v).1.round() as i32,
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
        show_y_labels: chrome.show_y_labels,
        show_x_labels: chrome.show_x_labels,
        show_title: chrome.show_title,
        show_x_title: chrome.show_x_title,
        show_y_title: chrome.show_y_title,
    };
    Ok((layout, report))
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

    fn log_x_spec() -> PlotSpec {
        PlotSpec::new(
            AxisSpec::new(AxisScale::Log10, "x"),
            AxisSpec::new(AxisScale::Linear, "y"),
        )
    }
    fn log_y_spec() -> PlotSpec {
        PlotSpec::new(
            AxisSpec::new(AxisScale::Linear, "x"),
            AxisSpec::new(AxisScale::Log10, "y"),
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
        let (_, rep) = compile(&spec, &view(0.0, 3.0, 0.0, 3.0), Rect::new(0, 0, 80, 24)).unwrap();
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
        let (_, rep) = compile(&spec, &view(0.0, 3.0, 0.0, 3.0), Rect::new(0, 0, 80, 24)).unwrap();
        assert_eq!(rep.segments_considered, 1, "gap must break the path");
    }

    #[test]
    fn log_domain_rejection_counted_and_breaks_path() {
        // y log scale (VALID view); a zero/negative y SAMPLE is domain-rejected
        // and breaks the line — a per-sample fact under a well-formed config.
        let spec = log_y_spec().series(Series::line(vec![(0.0, 1.0), (1.0, 0.0), (2.0, 100.0)]));
        let (_, rep) = compile(&spec, &view(0.0, 3.0, 1e-1, 1e3), Rect::new(0, 0, 80, 24)).unwrap();
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
        let (la, ra) = compile(&spec, &v, Rect::new(0, 0, 120, 40)).unwrap();
        let (lb, rb) = compile(&spec, &v, Rect::new(0, 0, 60, 20)).unwrap();
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
        let (la, _) =
            compile(&spec, &view(0.0, 10.0, 0.0, 10.0), Rect::new(0, 0, 100, 30)).unwrap();
        assert!(!la.x_ticks.is_empty() && !la.y_ticks.is_empty());
        for t in &la.x_ticks {
            assert!(t.cell >= la.plot_rect.x && t.cell < la.plot_rect.x + la.plot_rect.width);
        }
        for t in &la.y_ticks {
            assert!(t.cell >= la.plot_rect.y && t.cell < la.plot_rect.y + la.plot_rect.height);
        }
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
            // Valid linear config at hostile sizes: always Ok, never a panic.
            let _ = compile(&spec, &view(0.0, 1.0, 0.0, 1.0), rect).unwrap();
        }
    }

    // ---- SAI law A: invalid CONFIGURATION is a different thing than an invalid
    // SAMPLE (doc §4; Round-II seam #1) --------------------------------------

    #[test]
    fn valid_log_axis_rejects_nonpositive_sample_as_sample() {
        // VALID Log10 x view (1..1000). A sample x≤0 has no image under a
        // well-formed log axis → it is a SAMPLE rejection (compile still Ok),
        // while x=10 (perfectly valid here) is NOT rejected.
        let spec = log_x_spec().series(Series::scatter(vec![(0.0, 1.0), (10.0, 1.0), (-5.0, 1.0)]));
        let (_, rep) =
            compile(&spec, &view(1.0, 1000.0, 0.0, 2.0), Rect::new(0, 0, 80, 24)).unwrap();
        assert_eq!(rep.finite_samples, 3);
        assert_eq!(
            rep.scale_domain_rejected, 2,
            "x=0 and x=-5 rejected under a VALID log axis; x=10 accepted"
        );
    }

    #[test]
    fn invalid_log_x_view_is_config_error_not_sample_reject() {
        // INVALID Log10 x view (min ≤ 0). The sample x=10 WOULD be valid under a
        // proper log view, so it must NOT be laundered into scale_domain_rejected:
        // the VIEW is malformed. Err, and no PlotLayout/receipt at all.
        let spec = log_x_spec().series(Series::line(vec![(10.0, 1.0), (100.0, 2.0)]));
        let err = compile(&spec, &view(-5.0, 5.0, 0.0, 3.0), Rect::new(0, 0, 80, 24)).unwrap_err();
        assert_eq!(err, PlotError::InvalidXAxisDomain);
    }

    #[test]
    fn invalid_log_y_view_is_config_error() {
        let spec = log_y_spec().series(Series::line(vec![(1.0, 10.0), (2.0, 100.0)]));
        let err =
            compile(&spec, &view(0.0, 3.0, -1.0, 100.0), Rect::new(0, 0, 80, 24)).unwrap_err();
        assert_eq!(err, PlotError::InvalidYAxisDomain);
    }

    #[test]
    fn config_gate_checks_x_before_y() {
        // Both axes' views invalid under Log10: X is reported first (deterministic).
        let spec = PlotSpec::new(
            AxisSpec::new(AxisScale::Log10, "x"),
            AxisSpec::new(AxisScale::Log10, "y"),
        )
        .series(Series::line(vec![(10.0, 10.0)]));
        let err = compile(&spec, &view(-1.0, 5.0, -1.0, 5.0), Rect::new(0, 0, 80, 24)).unwrap_err();
        assert_eq!(err, PlotError::InvalidXAxisDomain);
    }

    #[test]
    fn linear_valid_view_compiles_ok() {
        let spec = lin_spec().series(Series::line(vec![(-5.0, -5.0), (5.0, 5.0)]));
        assert!(compile(&spec, &view(-5.0, 5.0, -5.0, 5.0), Rect::new(0, 0, 80, 24)).is_ok());
    }

    #[test]
    fn zero_area_is_ok_not_config_error_break2() {
        // Valid Linear scale, valid in-domain data. A ZERO-AREA plot rect is a
        // valid-but-empty REALIZATION, ontologically distinct from an invalid
        // CONFIG: it stays Ok (transform None, nothing realized) and must NOT
        // reclassify finite in-domain samples as scale_domain_rejected — that
        // count is an axis property, size-independent (law E; dalembert break #2).
        let spec = lin_spec().series(Series::scatter(vec![(0.0, 0.0), (0.5, 0.5), (1.0, 1.0)]));
        let v = view(0.0, 1.0, 0.0, 1.0);
        let (big_l, big) = compile(&spec, &v, Rect::new(0, 0, 80, 24)).unwrap();
        let (zw_l, zero_w) = compile(&spec, &v, Rect::new(0, 0, 0, 24)).unwrap();
        let (_, zero_h) = compile(&spec, &v, Rect::new(0, 0, 80, 0)).unwrap();
        assert!(big_l.transform.is_some());
        assert!(zw_l.transform.is_none(), "zero width ⇒ nothing realized");
        for (name, rep) in [("big", big), ("zero_w", zero_w), ("zero_h", zero_h)] {
            assert_eq!(
                rep.finite_samples, 3,
                "{name}: finite count size-independent"
            );
            assert_eq!(
                rep.scale_domain_rejected, 0,
                "{name}: zero-area must not fabricate domain rejections"
            );
        }
    }

    // ---- SAI law C: reducer requested vs effective (doc §6; Round-II seam #2) -

    #[test]
    fn reduce_records_receipt_on_increasing_x() {
        let pts: Vec<(f64, f64)> = (0..5000)
            .map(|i| (i as f64, (i as f64 * 0.01).sin()))
            .collect();
        let spec = lin_spec().series(Series::line(pts).reduce(Reduce::ExtremaPerColumn));
        let (_, rep) = compile(
            &spec,
            &view(0.0, 5000.0, -1.0, 1.0),
            Rect::new(0, 0, 100, 30),
        )
        .unwrap();
        assert_eq!(rep.reducers_requested, 1);
        assert_eq!(rep.reducers_declined, 0, "strictly increasing X ⇒ applied");
        assert_eq!(rep.reduced_from, 5000);
        assert!(rep.reduced_to > 0 && rep.reduced_to < 5000);
    }

    #[test]
    fn reduce_applies_on_nondecreasing_x_with_duplicates() {
        // Duplicate X is still nondecreasing; the column envelope stays faithful.
        let mut pts: Vec<(f64, f64)> = (0..400).map(|i| (i as f64 / 100.0, 0.0)).collect();
        pts.insert(200, (2.0, 7.0)); // duplicate x=2.0
        let spec = lin_spec().series(Series::line(pts).reduce(Reduce::ExtremaPerColumn));
        let (_, rep) =
            compile(&spec, &view(0.0, 4.0, -1.0, 8.0), Rect::new(0, 0, 100, 30)).unwrap();
        assert_eq!(rep.reducers_requested, 1);
        assert_eq!(rep.reducers_declined, 0, "nondecreasing (dup X) ⇒ applied");
        assert!(rep.reduced_to < rep.reduced_from);
    }

    #[test]
    fn reduce_declines_on_descending_x_and_renders_unreduced() {
        // Descending X is outside the reducer's proven domain → declined, NOT run.
        let pts: Vec<(f64, f64)> = (0..500)
            .rev()
            .map(|i| (i as f64, (i as f64).sin()))
            .collect();
        let v = view(0.0, 500.0, -1.0, 1.0);
        let area = Rect::new(0, 0, 100, 30);
        let spec = lin_spec().series(Series::line(pts.clone()).reduce(Reduce::ExtremaPerColumn));
        let (lr, rep) = compile(&spec, &v, area).unwrap();
        assert_eq!(rep.reducers_requested, 1);
        assert_eq!(rep.reducers_declined, 1, "descending X ⇒ declined");
        assert_eq!(rep.reduced_from, 0, "reducer never ran");
        assert_eq!(rep.reduced_to, 0);
        // Declined ⇒ identical geometry to an explicit Reduce::None.
        let none_spec = lin_spec().series(Series::line(pts).reduce(Reduce::None));
        let (ln, _) = compile(&none_spec, &v, area).unwrap();
        let (Prims::Segments(a), Prims::Segments(b)) = (&lr.series[0].prims, &ln.series[0].prims)
        else {
            panic!("line series ⇒ segments");
        };
        assert_eq!(
            a, b,
            "declined reduction renders exactly the unreduced path"
        );
    }

    #[test]
    fn reduce_declines_on_nonmonotone_x() {
        // Self-intersecting X (up then down) → no silent reduction.
        let mut pts: Vec<(f64, f64)> = (0..100).map(|i| (i as f64, 0.0)).collect();
        pts.extend((0..100).map(|i| (100.0 - i as f64, 1.0)));
        let spec = lin_spec().series(Series::line(pts).reduce(Reduce::ExtremaPerColumn));
        let (_, rep) = compile(
            &spec,
            &view(0.0, 100.0, -1.0, 2.0),
            Rect::new(0, 0, 100, 30),
        )
        .unwrap();
        assert_eq!(rep.reducers_declined, 1);
        assert_eq!(rep.reduced_from, 0);
    }

    #[test]
    fn reduce_gap_does_not_defeat_monotonicity() {
        // A NaN gap is transparent to the monotonicity check: a globally
        // nondecreasing series WITH a gap still applies…
        let mono = vec![
            (0.0, 0.0),
            (1.0, 1.0),
            (f64::NAN, f64::NAN),
            (2.0, 2.0),
            (3.0, 3.0),
        ];
        let spec = lin_spec().series(Series::line(mono).reduce(Reduce::ExtremaPerColumn));
        let (_, rep) = compile(&spec, &view(0.0, 3.0, 0.0, 3.0), Rect::new(0, 0, 100, 30)).unwrap();
        assert_eq!(
            rep.reducers_declined, 0,
            "gap must not block a monotone series"
        );

        // …but a descending step HIDDEN across a gap is still caught → declined.
        let hidden = vec![(0.0, 0.0), (5.0, 1.0), (f64::NAN, f64::NAN), (1.0, 2.0)];
        let spec2 = lin_spec().series(Series::line(hidden).reduce(Reduce::ExtremaPerColumn));
        let (_, rep2) =
            compile(&spec2, &view(0.0, 5.0, 0.0, 3.0), Rect::new(0, 0, 100, 30)).unwrap();
        assert_eq!(
            rep2.reducers_declined, 1,
            "a gap must not hide a descending jump (5 → 1)"
        );
    }

    #[test]
    fn declined_reducer_does_not_mutate_source_spec() {
        let pts: Vec<(f64, f64)> = (0..50).rev().map(|i| (i as f64, 0.0)).collect();
        let spec = lin_spec().series(Series::line(pts.clone()).reduce(Reduce::ExtremaPerColumn));
        let _ = compile(&spec, &view(0.0, 50.0, -1.0, 1.0), Rect::new(0, 0, 80, 24)).unwrap();
        assert_eq!(
            spec.series[0].points, pts,
            "compile never mutates source points"
        );
    }

    #[test]
    fn reducer_request_accounting_is_size_independent_break_evil_morty_f2() {
        // Evil-Morty Finding 2: `reducers_requested` is a spec property, not a
        // device property — it must not drop to 0 at zero area (where the series
        // short-circuits before realization). Same monotone-reduce spec, three
        // sizes: request/decline counts identical; only the realized reduced_*
        // move (0 when there is no viewport to reduce into).
        let pts: Vec<(f64, f64)> = (0..400).map(|i| (i as f64, 0.0)).collect();
        let spec = lin_spec().series(Series::line(pts).reduce(Reduce::ExtremaPerColumn));
        let v = view(0.0, 400.0, -1.0, 1.0);
        let (_, big) = compile(&spec, &v, Rect::new(0, 0, 100, 30)).unwrap();
        let (_, zero) = compile(&spec, &v, Rect::new(0, 0, 0, 30)).unwrap();
        assert_eq!(big.reducers_requested, 1);
        assert_eq!(zero.reducers_requested, 1, "request survives zero area");
        assert_eq!(big.reducers_declined, 0);
        assert_eq!(
            zero.reducers_declined, 0,
            "monotone ⇒ applicable, not declined"
        );
        assert!(big.reduced_to > 0, "applied where there is a viewport");
        assert_eq!(zero.reduced_from, 0, "nothing realized at zero area");
        assert_eq!(zero.reduced_to, 0);
    }

    #[test]
    fn scatter_with_reduce_is_requested_and_declined_break_evil_morty_f3() {
        // Evil-Morty Finding 3: a scatter series carrying a reduce policy is a
        // request the kernel cannot honour (scatter cannot reduce). Per the §5
        // definition it must count as requested AND declined — never silently
        // dropped from the accounting.
        let spec = lin_spec()
            .series(Series::scatter(vec![(0.0, 0.0), (1.0, 1.0)]).reduce(Reduce::ExtremaPerColumn));
        let (_, r) = compile(&spec, &view(0.0, 1.0, 0.0, 1.0), Rect::new(0, 0, 80, 24)).unwrap();
        assert_eq!(r.reducers_requested, 1, "a reduce policy was carried");
        assert_eq!(r.reducers_declined, 1, "scatter cannot reduce ⇒ declined");
        assert_eq!(r.reduced_from, 0, "reducer never ran");
    }

    // ---- SAI law D: conservation / separate gates (doc §5; Round-II seam #3) -

    #[test]
    fn scatter_conservation_law() {
        // All-scatter plot, valid config + non-empty area, no reduction:
        // finite_samples == scale_domain_rejected + points_emitted + points_clipped.
        // One accepted & drawn, one in-domain but far off-view (clipped), one y≤0
        // under a valid log-y (domain rejected), one NaN.
        let spec = log_y_spec().series(Series::scatter(vec![
            (1.0, 10.0),      // in view → emitted
            (1.5, 1e9),       // in-domain (y>0) but way above view → clipped
            (2.0, 0.0),       // y=0 under valid Log10 → domain-rejected
            (f64::NAN, 10.0), // non-finite → nonfinite_rejected, NOT in the law
        ]));
        let (_, r) = compile(&spec, &view(0.0, 3.0, 1.0, 100.0), Rect::new(0, 0, 80, 24)).unwrap();
        assert_eq!(r.finite_samples, 3);
        assert_eq!(r.nonfinite_rejected, 1);
        assert_eq!(r.scale_domain_rejected, 1);
        assert_eq!(r.points_clipped, 1);
        assert_eq!(r.points_emitted, 1);
        assert_eq!(r.segments_emitted, 0, "no line series ⇒ no segments");
        assert_eq!(
            r.finite_samples,
            r.scale_domain_rejected + r.points_emitted + r.points_clipped,
            "scatter conservation under valid config + viewport"
        );
    }

    #[test]
    fn line_segment_accounting_is_separate_from_points() {
        // Line emits SEGMENTS, not points — a different gate. For a line series:
        // segments_considered == segments_emitted + segments_clipped. Two points
        // sit far outside the view in the SAME direction, so the connecting
        // segment is fully off-canvas and clipped away.
        let spec = lin_spec().series(Series::line(vec![
            (0.3, 0.3),
            (0.6, 0.6),     // one visible segment (0.3→0.6)
            (100.0, 100.0), // leaves the view
            (200.0, 200.0), // the 100→200 segment is entirely off-canvas
        ]));
        let (_, r) = compile(&spec, &view(0.0, 1.0, 0.0, 1.0), Rect::new(0, 0, 80, 24)).unwrap();
        assert_eq!(r.points_emitted, 0, "no scatter series ⇒ no points");
        assert_eq!(
            r.segments_considered,
            r.segments_emitted + r.segments_clipped,
            "line segment conservation (segments are not points)"
        );
        assert!(r.segments_clipped >= 1, "the 100→200 segment is off-canvas");
    }

    #[test]
    fn mixed_line_scatter_conservation_holds_per_kind_break_evil_morty_f1() {
        // Evil-Morty Finding 1: with a SHARED emitted counter the §5 laws collapsed
        // on the canonical "scatter data + fit line" overlay, because a line's
        // points inflated finite_samples while its segments inflated the same
        // counter the scatter law read. With points/segments split, each kind's
        // accounting stays honest in a mixed plot.
        let spec = lin_spec()
            .series(Series::scatter(vec![(0.5, 0.5)]))
            .series(Series::line(vec![(0.1, 0.1), (0.2, 0.2), (0.3, 0.3)]));
        let (_, r) = compile(&spec, &view(0.0, 1.0, 0.0, 1.0), Rect::new(0, 0, 80, 24)).unwrap();
        // Counters are NOT conflated: 1 scatter point, 2 line segments, cleanly apart.
        assert_eq!(r.points_emitted, 1, "only the scatter point");
        assert_eq!(r.segments_emitted, 2, "only the line segments");
        // The LINE conservation law holds universally — all line-only quantities.
        assert_eq!(
            r.segments_considered,
            r.segments_emitted + r.segments_clipped,
            "line law holds even in a mixed plot"
        );
        // The SCATTER points are fully accounted among the scatter-kind counters
        // (the one scatter point was finite, in-domain, in-view → emitted).
        assert_eq!(r.points_emitted + r.points_clipped, 1);
        // Guard against the regression: the OLD naive whole-plot scatter equation
        // (finite == domain_rej + <all emitted> + points_clipped) was false here —
        // finite_samples(4) ≠ 0 + (1 point + 2 segs) + 0. We no longer sum kinds.
        assert_eq!(r.finite_samples, 4, "1 scatter + 3 line points");
    }

    #[test]
    fn capability_cannot_mutate_acceptance_counters() {
        // The receipt is produced by compile, which takes no capability/mode —
        // so realization capability is structurally unable to touch acceptance
        // counts. Rendering the SAME layout under different modes leaves the
        // compile-time report untouched (it is the same value).
        let spec = log_y_spec().series(Series::line(vec![(0.0, 1.0), (1.0, 0.0), (2.0, 50.0)]));
        let (layout, rep) =
            compile(&spec, &view(0.0, 3.0, 1e-1, 1e3), Rect::new(0, 0, 80, 24)).unwrap();
        let _ascii = crate::plot::render(&layout, crate::SubcellGlyphMode::Ascii);
        let _braille = crate::plot::render(&layout, crate::SubcellGlyphMode::Braille2x4);
        // rep is unchanged by either render (it predates and is independent of them)
        assert_eq!(rep.scale_domain_rejected, 1);
        assert_eq!(rep.finite_samples, 3);
    }

    #[test]
    fn layer_monoid_law_h() {
        // Empty layer list = identity (no projected geometry).
        let (empty, _) = compile(
            &lin_spec(),
            &view(0.0, 1.0, 0.0, 1.0),
            Rect::new(0, 0, 80, 24),
        )
        .unwrap();
        assert!(empty.series.is_empty());

        // Layers are ORDERED overlays; z-order is load-bearing (NOT commutative).
        let a = Series::line(vec![(0.0, 0.0), (1.0, 1.0)]).color((1, 0, 0));
        let b = Series::scatter(vec![(0.5, 0.5)]).color((0, 0, 2));
        let v = view(0.0, 1.0, 0.0, 1.0);
        let (ab, _) = compile(
            &lin_spec().series(a.clone()).series(b.clone()),
            &v,
            Rect::new(0, 0, 80, 24),
        )
        .unwrap();
        let (ba, _) =
            compile(&lin_spec().series(b).series(a), &v, Rect::new(0, 0, 80, 24)).unwrap();
        assert_eq!(ab.series.len(), 2);
        assert_eq!(ab.series[0].color, (1, 0, 0));
        assert_eq!(
            ba.series[0].color,
            (0, 0, 2),
            "order preserved in the layer list"
        );
    }
}
