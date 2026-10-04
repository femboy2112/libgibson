//! The semantic plot description — axes, view, series, annotations — plus the
//! one explicit, honest reducer (`ExtremaPerColumn`).
//!
//! Axis *semantics* (scale, label, unit, tick policy) are kept separate from the
//! *view* (the currently visible ranges). Pan/zoom changes [`PlotView`]; it never
//! rewrites the data. See `docs/PLOT_OBSERVABLE_GEOMETRY.md` §3, §6.

use super::scale::{AxisScale, AxisTransform, FiniteRange};

/// Semantic axis: how it is scaled and labelled. Units are a free string; there
/// is **no** dimensional analysis and **no** silent conversion (doc §12).
#[derive(Clone, Debug)]
pub struct AxisSpec {
    pub scale: AxisScale,
    pub label: String,
    pub unit: Option<String>,
}

impl AxisSpec {
    pub fn new(scale: AxisScale, label: impl Into<String>) -> AxisSpec {
        AxisSpec {
            scale,
            label: label.into(),
            unit: None,
        }
    }
    pub fn unit(mut self, unit: impl Into<String>) -> AxisSpec {
        self.unit = Some(unit.into());
        self
    }
}

/// The currently visible window. Pan/zoom = replace this, never the data.
#[derive(Clone, Copy, Debug)]
pub struct PlotView {
    pub x: FiniteRange,
    pub y: FiniteRange,
}

impl PlotView {
    pub fn new(x: FiniteRange, y: FiniteRange) -> PlotView {
        PlotView { x, y }
    }
}

/// The topology of an indexed sample set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesKind {
    /// Discrete points; no adjacency.
    Scatter,
    /// Ordered samples with adjacency between consecutive **valid** samples.
    Line,
}

/// Downsampling policy. Default is `None` (render every segment). Reduction is
/// never silent; the caller opts in per series.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reduce {
    /// No reduction: every input sample/segment is projected (O(N)).
    None,
    /// Monotone-X line envelope: per device column keep `first/min/max/last`, so
    /// a narrow spike cannot vanish. Only valid for monotone-X line series.
    ExtremaPerColumn,
}

/// One series: its topology, its samples, how it is coloured, and whether it is
/// reduced. A non-finite coordinate is a **gap** (breaks a line; dropped from a
/// scatter) — see the renderer.
#[derive(Clone, Debug)]
pub struct Series {
    pub kind: SeriesKind,
    pub points: Vec<(f64, f64)>,
    pub color: (u8, u8, u8),
    pub reduce: Reduce,
    pub label: String,
}

impl Series {
    pub fn line(points: Vec<(f64, f64)>) -> Series {
        Series {
            kind: SeriesKind::Line,
            points,
            color: (120, 200, 255),
            reduce: Reduce::None,
            label: String::new(),
        }
    }
    pub fn scatter(points: Vec<(f64, f64)>) -> Series {
        Series {
            kind: SeriesKind::Scatter,
            points,
            color: (255, 200, 120),
            reduce: Reduce::None,
            label: String::new(),
        }
    }
    pub fn color(mut self, rgb: (u8, u8, u8)) -> Series {
        self.color = rgb;
        self
    }
    pub fn reduce(mut self, reduce: Reduce) -> Series {
        self.reduce = reduce;
        self
    }
    pub fn label(mut self, label: impl Into<String>) -> Series {
        self.label = label.into();
        self
    }
}

/// An annotation in **data** coordinates — it rides the same axis/view transform
/// as the data, so pan/zoom moves data and annotations coherently.
#[derive(Clone, Debug)]
pub enum Annotation {
    VLine {
        x: f64,
        color: (u8, u8, u8),
    },
    HLine {
        y: f64,
        color: (u8, u8, u8),
    },
    Point {
        x: f64,
        y: f64,
        label: String,
        color: (u8, u8, u8),
    },
}

/// The whole semantic plot.
#[derive(Clone, Debug)]
pub struct PlotSpec {
    pub x: AxisSpec,
    pub y: AxisSpec,
    pub series: Vec<Series>,
    pub annotations: Vec<Annotation>,
    pub title: String,
}

impl PlotSpec {
    pub fn new(x: AxisSpec, y: AxisSpec) -> PlotSpec {
        PlotSpec {
            x,
            y,
            series: Vec::new(),
            annotations: Vec::new(),
            title: String::new(),
        }
    }
    pub fn title(mut self, title: impl Into<String>) -> PlotSpec {
        self.title = title.into();
        self
    }
    pub fn series(mut self, s: Series) -> PlotSpec {
        self.series.push(s);
        self
    }
    pub fn annotate(mut self, a: Annotation) -> PlotSpec {
        self.annotations.push(a);
        self
    }
}

/// Flush one column's `first/min/max/last` envelope into `out`, in x-order,
/// de-duplicating coincident points.
fn flush_col(
    out: &mut Vec<(f64, f64)>,
    first: &mut Option<(f64, f64)>,
    miny: &mut Option<(f64, f64)>,
    maxy: &mut Option<(f64, f64)>,
    last: &mut Option<(f64, f64)>,
) {
    let mut bucket: Vec<(f64, f64)> = [*first, *miny, *maxy, *last]
        .into_iter()
        .flatten()
        .collect();
    *first = None;
    *miny = None;
    *maxy = None;
    *last = None;
    if bucket.is_empty() {
        return;
    }
    bucket.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    for p in bucket {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
}

/// Is the series' X coordinate nondecreasing over its **finite** samples?
///
/// The precondition for [`reduce_extrema`]. Non-finite (gap) samples are
/// transparent to the check — skipped, not compared and not resetting the
/// running bound — so a gap can neither make a genuinely monotone series look
/// non-monotone nor hide a descending step across itself (two finite values on
/// opposite sides of a gap are still compared to each other). Empty / all-gap /
/// single-finite inputs are vacuously nondecreasing.
pub(crate) fn is_nondecreasing_x(points: &[(f64, f64)]) -> bool {
    let mut last = f64::NEG_INFINITY;
    for &(x, _) in points {
        if !x.is_finite() {
            continue;
        }
        if x < last {
            return false;
        }
        last = x;
    }
    true
}

/// Extrema-preserving reducer for a **monotone-X** line series.
///
/// Internal: the public surface is `Series::reduce(Reduce::ExtremaPerColumn)`,
/// which `compile` applies only after [`is_nondecreasing_x`] confirms the
/// precondition (so this is never called on data it would mangle).
///
/// Columns are taken in the axis's **projected** space — the sample's device
/// column is `floor(xt.project(x) · num_cols)`, so a `Log10` x axis buckets in
/// log space, one device column per on-screen column (POST-CANARY fix: the old
/// reducer bucketed linearly in raw x and so mangled the envelope on a log axis).
/// Per column it keeps the `first`, `min-y`, `max-y`, and `last` samples
/// (x-ordered, de-duplicated), so a one-sample spike inside a dense column
/// survives as that column's `max`/`min`.
///
/// Samples **outside the view** (`u < 0` or `u > 1`) occupy their own edge
/// buckets (`-1` on the left, `num_cols` on the right) and so can **never evict**
/// an in-view column's envelope (POST-CANARY fix: the old reducer *clamped*
/// out-of-view columns into `0`/`num_cols-1`, letting off-view data silently
/// steal the first/min/max/last slots of a real in-view column — a one-sample
/// in-view spike vanished under a zoomed view, violating "never silently eat
/// data" and law J). The edge buckets keep the path entering/leaving the view
/// (clipping draws the crossing); segments entirely outside the view clip away.
///
/// A non-finite sample, or one with no image under the scale (`x ≤ 0` on
/// `Log10`), is a **gap**: it flushes the current column and emits a `(NaN, NaN)`
/// sentinel so the path breaks and is never bridged. `num_cols == 0` passes
/// through unchanged.
pub(crate) fn reduce_extrema(
    points: &[(f64, f64)],
    xt: &AxisTransform,
    num_cols: usize,
) -> Vec<(f64, f64)> {
    if num_cols == 0 {
        return points.to_vec();
    }
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut cur_col: Option<i64> = None;
    let (mut first, mut miny, mut maxy, mut last) = (None, None, None, None);
    let gap = |out: &mut Vec<(f64, f64)>,
               first: &mut Option<(f64, f64)>,
               miny: &mut Option<(f64, f64)>,
               maxy: &mut Option<(f64, f64)>,
               last: &mut Option<(f64, f64)>,
               cur_col: &mut Option<i64>| {
        flush_col(out, first, miny, maxy, last);
        *cur_col = None;
        if out.last().map(|p| !p.0.is_nan()).unwrap_or(true) {
            out.push((f64::NAN, f64::NAN));
        }
    };

    for &(x, y) in points {
        // A non-finite sample, or one with no image under the scale, breaks the path.
        let u = if x.is_finite() && y.is_finite() {
            xt.project(x)
        } else {
            None
        };
        let Some(u) = u else {
            gap(
                &mut out,
                &mut first,
                &mut miny,
                &mut maxy,
                &mut last,
                &mut cur_col,
            );
            continue;
        };
        // In-view u ∈ [0,1] maps onto columns [0, num_cols-1] (u=1 caps to the
        // last column, preserving the fit-to-extent convention); genuinely
        // out-of-view samples get their own edge buckets and never touch an
        // in-view column.
        let c = if u < 0.0 {
            -1
        } else if u > 1.0 {
            num_cols as i64
        } else {
            ((u * num_cols as f64).floor() as i64).min(num_cols as i64 - 1)
        };
        if cur_col != Some(c) {
            flush_col(&mut out, &mut first, &mut miny, &mut maxy, &mut last);
            cur_col = Some(c);
        }
        if first.is_none() {
            first = Some((x, y));
        }
        last = Some((x, y));
        if miny.is_none_or(|(_, my)| y < my) {
            miny = Some((x, y));
        }
        if maxy.is_none_or(|(_, my)| y > my) {
            maxy = Some((x, y));
        }
    }
    flush_col(&mut out, &mut first, &mut miny, &mut maxy, &mut last);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin_xt(lo: f64, hi: f64) -> AxisTransform {
        AxisTransform::new(AxisScale::Linear, FiniteRange::new(lo, hi).unwrap()).unwrap()
    }
    fn log_xt(lo: f64, hi: f64) -> AxisTransform {
        AxisTransform::new(AxisScale::Log10, FiniteRange::new(lo, hi).unwrap()).unwrap()
    }

    #[test]
    fn extrema_preserves_single_spike_law_j() {
        // 100 samples in a single column, all y=0 except one spike at y=100.
        let mut pts: Vec<(f64, f64)> = (0..100).map(|i| (i as f64 / 1000.0, 0.0)).collect();
        pts[50] = (0.050, 100.0);
        let reduced = reduce_extrema(&pts, &lin_xt(0.0, 1.0), 4);
        assert!(
            reduced.iter().any(|&(_, y)| (y - 100.0).abs() < 1e-9),
            "the spike must survive reduction: {reduced:?}"
        );
        // and it genuinely reduced the count
        assert!(reduced.len() < pts.len());
    }

    #[test]
    fn extrema_preserves_min_and_max_both() {
        let pts = vec![(0.0, 5.0), (0.01, -9.0), (0.02, 9.0), (0.03, 5.0)];
        let reduced = reduce_extrema(&pts, &lin_xt(0.0, 0.04), 1);
        assert!(
            reduced.iter().any(|&(_, y)| (y + 9.0).abs() < 1e-9),
            "min kept"
        );
        assert!(
            reduced.iter().any(|&(_, y)| (y - 9.0).abs() < 1e-9),
            "max kept"
        );
    }

    #[test]
    fn gap_sentinel_is_preserved_law_i() {
        let pts = vec![
            (0.0, 1.0),
            (0.1, 2.0),
            (f64::NAN, f64::NAN),
            (0.2, 3.0),
            (0.3, 4.0),
        ];
        let reduced = reduce_extrema(&pts, &lin_xt(0.0, 0.4), 8);
        assert!(
            reduced.iter().any(|&(x, y)| x.is_nan() && y.is_nan()),
            "gap sentinel must survive so the path breaks: {reduced:?}"
        );
    }

    #[test]
    fn reduce_zero_cols_passthrough() {
        let pts = vec![(0.0, 1.0), (1.0, 2.0)];
        assert_eq!(reduce_extrema(&pts, &lin_xt(0.0, 1.0), 0), pts);
    }

    #[test]
    fn reduce_bounds_output_count() {
        let pts: Vec<(f64, f64)> = (0..10_000).map(|i| (i as f64, (i as f64).sin())).collect();
        let reduced = reduce_extrema(&pts, &lin_xt(0.0, 10_000.0), 100);
        // at most ~4 points per column, plus the two out-of-view edge buckets
        assert!(
            reduced.len() <= (100 + 2) * 4 + 4,
            "bounded: {}",
            reduced.len()
        );
        assert!(reduced.len() >= 100, "but not collapsed");
    }

    // ---- POST-CANARY (PULSAR-2 defects 1 & 2) --------------------------------

    #[test]
    fn reduce_out_of_view_samples_do_not_evict_in_view_spike() {
        // Defect 1: a zoomed view [500, 2000]. Everything left of the view has
        // huge alternating ±100 values; a lone in-view spike sits at x=505. The
        // old reducer clamped the off-view columns into column 0 and the ±100
        // values evicted the spike. It must now survive (edge bucket -1 holds the
        // off-view data, which clips away and never touches column 0).
        let mut pts: Vec<(f64, f64)> = (0..2000).map(|i| (i as f64, 0.0)).collect();
        for (i, p) in pts.iter_mut().enumerate().take(500) {
            p.1 = if i % 2 == 0 { 100.0 } else { -100.0 };
        }
        pts[505].1 = 1.0;
        let reduced = reduce_extrema(&pts, &lin_xt(500.0, 2000.0), 120);
        assert!(
            reduced
                .iter()
                .any(|&(x, y)| (x - 505.0).abs() < 1e-9 && (y - 1.0).abs() < 1e-9),
            "the in-view spike must survive a zoomed view"
        );
    }

    #[test]
    fn reduce_log_x_buckets_in_log_space() {
        // Defect 2: on a Log10 x axis the device columns are log-spaced. Bucketing
        // in log space means the low-x decades keep their own envelopes instead of
        // collapsing into one wide linear bucket. The first point of each decade
        // boundary must land in a distinct column, so e.g. x=1,10,100,1000 are not
        // folded together.
        let pts: Vec<(f64, f64)> = (0..20_000)
            .map(|i| {
                let x = 1.0 + i as f64 * 0.05;
                (x, (x * 6.0).sin())
            })
            .collect();
        let xt = log_xt(1.0, 1000.0);
        let reduced = reduce_extrema(&pts, &xt, 90);
        // Count how many of the 90 device columns are represented among the
        // reduced points. Linear bucketing on a log axis crams almost everything
        // into the high-x columns and leaves the low-x decades nearly empty; log
        // bucketing spreads the envelope across the columns.
        let mut cols = std::collections::BTreeSet::new();
        for &(x, _) in reduced.iter().filter(|(x, _)| x.is_finite()) {
            if let Some(u) = xt.project(x) {
                if (0.0..=1.0).contains(&u) {
                    cols.insert((u * 90.0).floor() as i64);
                }
            }
        }
        assert!(
            cols.len() > 60,
            "log bucketing should populate most columns, got {}",
            cols.len()
        );
    }

    #[test]
    fn builders_work() {
        let spec = PlotSpec::new(
            AxisSpec::new(AxisScale::Linear, "t").unit("s"),
            AxisSpec::new(AxisScale::Log10, "power").unit("dB"),
        )
        .title("demo")
        .series(
            Series::line(vec![(0.0, 1.0)])
                .color((1, 2, 3))
                .reduce(Reduce::ExtremaPerColumn),
        )
        .annotate(Annotation::VLine {
            x: 1.0,
            color: (9, 9, 9),
        });
        assert_eq!(spec.series.len(), 1);
        assert_eq!(spec.series[0].kind, SeriesKind::Line);
        assert_eq!(spec.series[0].reduce, Reduce::ExtremaPerColumn);
        assert_eq!(spec.x.unit.as_deref(), Some("s"));
        assert_eq!(spec.annotations.len(), 1);
    }

    #[test]
    fn nondecreasing_x_predicate() {
        assert!(is_nondecreasing_x(&[(0.0, 9.0), (1.0, 9.0), (2.0, 9.0)]));
        assert!(
            is_nondecreasing_x(&[(0.0, 9.0), (1.0, 9.0), (1.0, 9.0)]),
            "dup X ok"
        );
        assert!(!is_nondecreasing_x(&[(2.0, 9.0), (1.0, 9.0)]), "descending");
        // Gaps are transparent: a monotone series with a gap still passes…
        assert!(is_nondecreasing_x(&[
            (0.0, 0.0),
            (f64::NAN, f64::NAN),
            (1.0, 0.0)
        ]));
        // …but a descending step hidden across a gap is still caught.
        assert!(!is_nondecreasing_x(&[
            (5.0, 0.0),
            (f64::NAN, f64::NAN),
            (1.0, 0.0)
        ]));
        // Degenerate inputs are vacuously nondecreasing.
        assert!(is_nondecreasing_x(&[]));
        assert!(is_nondecreasing_x(&[(3.0, 0.0)]));
    }
}
