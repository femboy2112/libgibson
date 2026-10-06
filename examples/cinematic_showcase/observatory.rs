//! The film's three **Observatory** scenes: scientific plots rendered with
//! `gibson::plot` and animated by a shot-local time.
//!
//! The application owns the observables; `gibson::plot` only displays them. Every
//! scene is a pure `f(scene, local, w, h, depth)` — no wall clock, and the only
//! randomness is a fixed-seed LCG — so a frame is reproducible and the film can
//! be re-rendered bit-for-bit.

#![allow(dead_code)]

use crate::shot::SceneId;
use gibson::capability::ColorDepth;
use gibson::plot::{
    self, Annotation, AxisScale, AxisSpec, FiniteRange, PlotSpec, PlotView, Reduce, Series,
};
use gibson::{Rect, SubcellGlyphMode, Surface};

/// Smallest plot the scenes will attempt; anything below is a blank surface.
const MIN_W: u16 = 6;
const MIN_H: u16 = 4;

/// A tiny deterministic LCG (same recipe as `scientific_plot_lab`).
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

fn fr(a: f64, b: f64) -> Option<FiniteRange> {
    FiniteRange::new(a, b)
}

/// Sanitise a hostile local time: NaN/inf become 0, and the value is clamped to
/// a generous range so downstream arithmetic stays finite.
fn clean_local(local: f32) -> f64 {
    if local.is_finite() {
        (local as f64).clamp(0.0, 1.0e4)
    } else {
        0.0
    }
}

/// Render one Observatory scene at shot-local time `local` (seconds).
///
/// Never panics: a plot error or a degenerate size yields a blank surface.
pub fn render(scene: SceneId, local: f32, w: u16, h: u16, depth: ColorDepth) -> Surface {
    if w < MIN_W || h < MIN_H {
        return Surface::new(w, h);
    }
    let t = clean_local(local);
    let Some((spec, view)) = build(scene, t) else {
        return Surface::new(w, h);
    };
    let mode = if matches!(depth, ColorDepth::Mono) {
        SubcellGlyphMode::Block
    } else {
        SubcellGlyphMode::Braille2x4
    };
    match plot::plot(&spec, &view, Rect::new(0, 0, w, h), mode) {
        Ok((surface, _report)) => surface,
        Err(_) => Surface::new(w, h),
    }
}

fn build(scene: SceneId, t: f64) -> Option<(PlotSpec, PlotView)> {
    match scene {
        SceneId::Scope => scope(t),
        SceneId::Spectrum => spectrum(t),
        SceneId::PhaseSpace => phase_space(t),
    }
}

/// A live oscilloscope: the x-window pans with `t`, so the trace scrolls past.
fn scope(t: f64) -> Option<(PlotSpec, PlotView)> {
    const RATE: f64 = 2.0; // window units per second
    const SPAN: f64 = 20.0;
    const SAMPLES: usize = 1200;
    let x0 = t * RATE;
    let pts: Vec<(f64, f64)> = (0..SAMPLES)
        .map(|i| {
            let x = x0 + SPAN * i as f64 / (SAMPLES - 1) as f64;
            // A carrier with a slow amplitude beat plus a faster harmonic.
            let env = 0.65 + 0.35 * (x * 0.21).sin();
            let y = env * (x * 2.4).sin() + 0.18 * (x * 9.1).sin();
            (x, y)
        })
        .collect();
    let spec = PlotSpec::new(
        AxisSpec::new(AxisScale::Linear, "time").unit("s"),
        AxisSpec::new(AxisScale::Linear, "signal").unit("V"),
    )
    .title("SCOPE - live trace")
    .series(
        Series::line(pts)
            .color((120, 220, 255))
            .reduce(Reduce::ExtremaPerColumn),
    )
    .annotate(Annotation::HLine {
        y: 0.0,
        color: (70, 90, 110),
    });
    let view = PlotView::new(fr(x0, x0 + SPAN)?, fr(-1.4, 1.4)?);
    Some((spec, view))
}

/// A log-frequency power spectrum with a cursor that sweeps across it.
fn spectrum(t: f64) -> Option<(PlotSpec, PlotView)> {
    const LO: f64 = 1.0;
    const HI: f64 = 1000.0;
    const SAMPLES: usize = 600;
    // (centre Hz, amplitude, half-width as a fraction of the centre)
    let peaks = [(6.0, 40.0, 0.06), (45.0, 100.0, 0.05), (310.0, 55.0, 0.04)];
    let pts: Vec<(f64, f64)> = (0..SAMPLES)
        .map(|i| {
            let f = LO * (HI / LO).powf(i as f64 / (SAMPLES - 1) as f64);
            let floor = 4.0 / f.sqrt();
            let p: f64 = peaks
                .iter()
                .map(|&(f0, amp, wid)| amp / (1.0 + ((f - f0) / (f0 * wid)).powi(2)))
                .sum();
            (f, floor + p)
        })
        .collect();
    // The cursor sweeps the whole band in ~6 s, then rests at the top edge.
    let phase = (t / 6.0).clamp(0.0, 1.0);
    let cursor = LO * (HI / LO).powf(0.02 + 0.96 * phase);
    let spec = PlotSpec::new(
        AxisSpec::new(AxisScale::Log10, "frequency").unit("Hz"),
        AxisSpec::new(AxisScale::Linear, "power"),
    )
    .title("SPECTRUM - resonances")
    .series(Series::line(pts).color((255, 180, 120)))
    .annotate(Annotation::VLine {
        x: cursor,
        color: (255, 240, 200),
    });
    let view = PlotView::new(fr(LO, HI)?, fr(0.0, 120.0)?);
    Some((spec, view))
}

/// A phase-space cloud that accretes and condenses: the point count grows with
/// `t` while the noise shrinks, so two-lobed structure precipitates from a fog.
fn phase_space(t: f64) -> Option<(PlotSpec, PlotView)> {
    const MAX_POINTS: usize = 600;
    let n = ((t * 120.0) as usize).min(MAX_POINTS);
    let progress = (t / 6.0).clamp(0.0, 1.0);
    let noise = 1.1 * (1.0 - progress) + 0.07 * progress;
    let mut rng = Lcg(0x5EED_CAFE);
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let lobe = if i % 2 == 0 { -1.3 } else { 1.3 };
            let a = rng.next_f64() * std::f64::consts::TAU;
            let r = 0.9 + 0.25 * (rng.next_f64() - 0.5);
            let (gx, gy) = (rng.next_f64() - 0.5, rng.next_f64() - 0.5);
            // The butterfly: a loop around each lobe, sheared into a figure-8.
            let x = lobe + r * a.cos() + noise * gx * 2.0;
            let y = 0.8 * r * a.sin() + 0.35 * lobe * a.cos() + noise * gy * 2.0;
            (x, y)
        })
        .collect();
    let spec = PlotSpec::new(
        AxisSpec::new(AxisScale::Linear, "x"),
        AxisSpec::new(AxisScale::Linear, "dx/dt"),
    )
    .title("PHASE SPACE - accretion")
    .series(Series::scatter(pts).color((200, 160, 255)));
    let view = PlotView::new(fr(-4.0, 4.0)?, fr(-3.0, 3.0)?);
    Some((spec, view))
}
