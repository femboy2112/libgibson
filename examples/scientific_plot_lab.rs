//! scientific_plot_lab — a concise validation harness for `gibson::plot`.
//!
//! NOT the integration app — a library exerciser. One dominant graph area, a set
//! of deterministic synthetic cases, dumped as a glyph frame so a human can read
//! geometry, ticks, gaps and the extrema reducer without a live terminal.
//!
//! Usage:
//!   cargo run --example scientific_plot_lab -- [--case=NAME] [--size=WxH] [--glyphs=MODE]
//!   NAME  : line | scatter | logspec | gap | spike | annotated | zoom   (default line)
//!   WxH   : e.g. 120x40 (default 100x30)
//!   MODE  : braille | halfblock | block | ascii                        (default braille)
//!
//! The application owns the observables; `gibson::plot` only displays them.

use gibson::plot::{
    self, Annotation, AxisScale, AxisSpec, FiniteRange, PlotSpec, PlotView, Reduce, Series,
};
use gibson::{Rect, SubcellGlyphMode};

fn fr(a: f64, b: f64) -> FiniteRange {
    FiniteRange::new(a, b).expect("valid range")
}

/// A tiny deterministic LCG so the scatter cloud is reproducible.
struct Lcg(u64);
impl Lcg {
    fn next_f64(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn case(name: &str) -> (PlotSpec, PlotView, &'static str) {
    match name {
        "scatter" => {
            let mut rng = Lcg(0xC0FFEE);
            let pts: Vec<(f64, f64)> = (0..500)
                .map(|i| {
                    let x = i as f64 / 50.0;
                    let noise = (rng.next_f64() - 0.5) * 3.0;
                    (x, 0.5 * x + noise)
                })
                .collect();
            let spec = PlotSpec::new(
                AxisSpec::new(AxisScale::Linear, "drive").unit("V"),
                AxisSpec::new(AxisScale::Linear, "response").unit("mA"),
            )
            .title("SCATTER — response vs drive (noisy linear trend)")
            .series(Series::scatter(pts).color((255, 200, 120)));
            (
                spec,
                PlotView::new(fr(0.0, 10.0), fr(-3.0, 8.0)),
                "discrete samples, no adjacency",
            )
        }
        "logspec" => {
            // A synthetic power spectrum: three Lorentzian peaks on a decaying floor.
            let peaks = [(3.0, 50.0), (12.0, 200.0), (40.0, 30.0)];
            let pts: Vec<(f64, f64)> = (1..=400)
                .map(|i| {
                    let f = i as f64 * 0.25;
                    let floor = 0.05 / f;
                    let p: f64 = peaks
                        .iter()
                        .map(|&(f0, amp)| amp / (1.0 + ((f - f0) / 0.4).powi(2)))
                        .sum();
                    (f, floor + p + 0.02)
                })
                .collect();
            let spec = PlotSpec::new(
                AxisSpec::new(AxisScale::Linear, "frequency").unit("Hz"),
                AxisSpec::new(AxisScale::Log10, "power").unit("dB"),
            )
            .title("LOGSPEC — power spectrum (log y)")
            .series(Series::line(pts).color((120, 230, 255)));
            (
                spec,
                PlotView::new(fr(0.0, 100.0), fr(1e-2, 1e3)),
                "log-y spectrum with sharp peaks",
            )
        }
        "gap" => {
            // A receiver trace with a dropout: NaN in the middle must NOT bridge.
            let pts: Vec<(f64, f64)> = (0..200)
                .map(|i| {
                    let x = i as f64 / 20.0;
                    if (80..120).contains(&i) {
                        (x, f64::NAN) // sensor dropout
                    } else {
                        (x, (x * 1.3).sin())
                    }
                })
                .collect();
            let spec = PlotSpec::new(
                AxisSpec::new(AxisScale::Linear, "time").unit("s"),
                AxisSpec::new(AxisScale::Linear, "signal"),
            )
            .title("GAP — a dropout must break the line, never bridge it")
            .series(Series::line(pts).color((255, 150, 150)));
            (
                spec,
                PlotView::new(fr(0.0, 10.0), fr(-1.3, 1.3)),
                "explicit NaN gap",
            )
        }
        "spike" => {
            // 8000 samples near zero with ONE one-sample spike; the extrema
            // reducer must keep it (nearest-neighbour resampling would lose it).
            let mut pts: Vec<(f64, f64)> = (0..8000).map(|i| (i as f64, 0.0)).collect();
            pts[4000] = (4000.0, 1.0);
            let spec = PlotSpec::new(
                AxisSpec::new(AxisScale::Linear, "sample"),
                AxisSpec::new(AxisScale::Linear, "amplitude"),
            )
            .title("SPIKE — 8000 flat samples, ONE spike, extrema reducer ON")
            .series(
                Series::line(pts)
                    .color((180, 255, 160))
                    .reduce(Reduce::ExtremaPerColumn),
            );
            (
                spec,
                PlotView::new(fr(0.0, 8000.0), fr(-0.2, 1.2)),
                "narrow spike must survive reduction",
            )
        }
        "annotated" => {
            let pts: Vec<(f64, f64)> = (0..400)
                .map(|i| {
                    let x = i as f64 / 25.0;
                    (x, (x * 0.7).sin() * (-(x * 0.08)).exp())
                })
                .collect();
            let spec = PlotSpec::new(
                AxisSpec::new(AxisScale::Linear, "time").unit("s"),
                AxisSpec::new(AxisScale::Linear, "amplitude").unit("mV"),
            )
            .title("ANNOTATED — damped oscillation with references")
            .series(Series::line(pts).color((140, 210, 255)))
            .annotate(Annotation::HLine {
                y: 0.0,
                color: (90, 100, 120),
            })
            .annotate(Annotation::VLine {
                x: 4.5,
                color: (230, 120, 80),
            })
            .annotate(Annotation::Point {
                x: 2.24,
                y: 0.79,
                label: "first peak".into(),
                color: (120, 240, 120),
            });
            (
                spec,
                PlotView::new(fr(0.0, 16.0), fr(-1.0, 1.0)),
                "reference lines + point label in DATA coords",
            )
        }
        "zoom" => {
            // Same observable as `line`, a zoomed-in VIEW — data is not rewritten.
            let pts: Vec<(f64, f64)> = (0..800)
                .map(|i| {
                    let x = i as f64 / 50.0;
                    (x, (x).sin())
                })
                .collect();
            let spec = PlotSpec::new(
                AxisSpec::new(AxisScale::Linear, "t").unit("s"),
                AxisSpec::new(AxisScale::Linear, "sin t"),
            )
            .title("ZOOM — same data, a narrowed PlotView (no data mutated)")
            .series(Series::line(pts).color((120, 220, 255)));
            (
                spec,
                PlotView::new(fr(6.0, 9.0), fr(-1.1, 1.1)),
                "pan/zoom = change the view only",
            )
        }
        _ => {
            let pts: Vec<(f64, f64)> = (0..600)
                .map(|i| {
                    let x = i as f64 / 24.0;
                    (x, (x).sin())
                })
                .collect();
            let spec = PlotSpec::new(
                AxisSpec::new(AxisScale::Linear, "t").unit("s"),
                AxisSpec::new(AxisScale::Linear, "sin t"),
            )
            .title("LINE — a signed waveform")
            .series(Series::line(pts).color((120, 220, 255)));
            (
                spec,
                PlotView::new(fr(0.0, 25.0), fr(-1.2, 1.2)),
                "signed line series",
            )
        }
    }
}

fn arg(flag: &str) -> Option<String> {
    std::env::args()
        .skip(1)
        .find_map(|a| a.strip_prefix(flag).map(|s| s.to_string()))
}

fn main() {
    let name = arg("--case=").unwrap_or_else(|| "line".into());
    let (w, h) = arg("--size=")
        .and_then(|s| {
            let (a, b) = s.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((100u16, 30u16));
    let mode = match arg("--glyphs=").as_deref() {
        Some("halfblock") => SubcellGlyphMode::HalfBlock1x2,
        Some("block") => SubcellGlyphMode::Block,
        Some("ascii") => SubcellGlyphMode::Ascii,
        _ => SubcellGlyphMode::Braille2x4,
    };

    let (spec, view, desc) = case(&name);
    // A malformed CONFIGURATION (e.g. a Log10 axis over a non-positive view) is a
    // distinct outcome from a valid plot that rejected some samples — surface it
    // honestly rather than drawing an empty frame and pretending all was well.
    let (surface, report) = match plot::plot(&spec, &view, Rect::new(0, 0, w, h), mode) {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("# case={name} ({desc})  size={w}x{h}  CONFIGURATION ERROR: {e:?}");
            std::process::exit(2);
        }
    };

    for line in surface.to_visible_lines() {
        println!("{}", line.trim_end());
    }
    eprintln!("# case={name} ({desc})  size={w}x{h}  glyphs={mode:?}");
    eprintln!(
        "# report: seen={} finite={} nonfinite_rej={} domain_rej={} segs_drawn/considered={}/{} segs_clipped={} pts_drawn={} pts_clipped={} reduced={}->{} reducers={}req/{}declined",
        report.samples_seen,
        report.finite_samples,
        report.nonfinite_rejected,
        report.scale_domain_rejected,
        report.segments_emitted,
        report.segments_considered,
        report.segments_clipped,
        report.points_emitted,
        report.points_clipped,
        report.reduced_from,
        report.reduced_to,
        report.reducers_requested,
        report.reducers_declined,
    );
}
