//! The semantic plot description — axes, view, series, annotations — plus the
//! one explicit, honest reducer (`ExtremaPerColumn`).
//!
//! Axis *semantics* (scale, label, unit, tick policy) are kept separate from the
//! *view* (the currently visible ranges). Pan/zoom changes [`PlotView`]; it never
//! rewrites the data. See `docs/PLOT_OBSERVABLE_GEOMETRY.md` §3, §6.

use super::scale::{AxisScale, FiniteRange};

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

/// Extrema-preserving reducer for a **monotone-X** line series.
///
/// Partitions the x-range into `num_cols` device columns and, per column, keeps
/// the `first`, `min-y`, `max-y`, and `last` samples (x-ordered, de-duplicated).
/// A one-sample spike inside a dense column survives because its value becomes
/// that column's `max` (or `min`). A non-finite sample is a **gap**: it flushes
/// the current column and emits a `(NaN, NaN)` sentinel so the path breaks and is
/// never bridged. Degenerate inputs (`num_cols == 0` or a collapsed x-range) pass
/// through unchanged. Non-monotone input never panics (behaviour simply
/// unspecified beyond "no crash, no NaN into downstream math").
pub fn reduce_extrema(
    points: &[(f64, f64)],
    num_cols: usize,
    x_lo: f64,
    x_hi: f64,
) -> Vec<(f64, f64)> {
    if num_cols == 0 || x_hi <= x_lo || !x_lo.is_finite() || !x_hi.is_finite() {
        return points.to_vec();
    }
    let span = x_hi - x_lo;
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut cur_col: Option<i64> = None;
    let (mut first, mut miny, mut maxy, mut last) = (None, None, None, None);

    for &(x, y) in points {
        if !x.is_finite() || !y.is_finite() {
            flush_col(&mut out, &mut first, &mut miny, &mut maxy, &mut last);
            cur_col = None;
            if out.last().map(|p| !p.0.is_nan()).unwrap_or(true) {
                out.push((f64::NAN, f64::NAN));
            }
            continue;
        }
        let c = (((x - x_lo) / span) * num_cols as f64).floor() as i64;
        let c = c.clamp(0, num_cols as i64 - 1);
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

    #[test]
    fn extrema_preserves_single_spike_law_j() {
        // 100 samples in a single column, all y=0 except one spike at y=100.
        let mut pts: Vec<(f64, f64)> = (0..100).map(|i| (i as f64 / 1000.0, 0.0)).collect();
        pts[50] = (0.050, 100.0);
        let reduced = reduce_extrema(&pts, 4, 0.0, 1.0);
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
        let reduced = reduce_extrema(&pts, 1, 0.0, 0.04);
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
        let reduced = reduce_extrema(&pts, 8, 0.0, 0.4);
        assert!(
            reduced.iter().any(|&(x, y)| x.is_nan() && y.is_nan()),
            "gap sentinel must survive so the path breaks: {reduced:?}"
        );
    }

    #[test]
    fn reduce_degenerate_passthrough() {
        let pts = vec![(0.0, 1.0), (1.0, 2.0)];
        assert_eq!(reduce_extrema(&pts, 0, 0.0, 1.0), pts);
        assert_eq!(reduce_extrema(&pts, 10, 1.0, 1.0), pts); // collapsed x-range
    }

    #[test]
    fn reduce_bounds_output_count() {
        let pts: Vec<(f64, f64)> = (0..10_000).map(|i| (i as f64, (i as f64).sin())).collect();
        let reduced = reduce_extrema(&pts, 100, 0.0, 10_000.0);
        // at most ~4 points per column
        assert!(reduced.len() <= 100 * 4 + 4, "bounded: {}", reduced.len());
        assert!(reduced.len() >= 100, "but not collapsed");
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
}
