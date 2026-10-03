//! `gibson::plot` — Observable Geometry (v0.5, EXPERIMENTAL, Rust-only).
//!
//! A **semantic plotting layer**: it turns an indexed set of scientific values
//! into terminal geometry and reports exactly what it did with every sample. It
//! owns *realization*, never *analysis* — FFT, regression, statistics and
//! simulation belong to the application. See
//! [`docs/PLOT_OBSERVABLE_GEOMETRY.md`](../../../docs/PLOT_OBSERVABLE_GEOMETRY.md)
//! for the full contract and the algebraic laws the tests enforce.
//!
//! Pipeline:
//! ```text
//! PlotSpec + PlotView + Rect  --compile-->  PlotLayout  --render-->  Surface
//!                                  (+ PlotReport receipt)   (capability-aware)
//! ```
//!
//! Names and signatures are experimental and may change before v0.5.0.

pub mod data;
pub mod layout;
pub mod render;
pub mod scale;
pub mod ticks;

pub use data::{
    reduce_extrema, Annotation, AxisSpec, PlotSpec, PlotView, Reduce, Series, SeriesKind,
};
pub use layout::{compile, PlotLayout, PlotReport, ProjectedTick};
pub use render::{render, render_themed, PlotTheme};
pub use scale::{AxisScale, AxisTransform, FiniteRange, PlotTransform2D, Viewport};
pub use ticks::{log10_major_ticks, log10_minor_ticks, major_ticks, Tick};

use crate::{Rect, SubcellGlyphMode, Surface};

/// One-call convenience: compile a spec + view into `area` and realize it under
/// `mode`, returning the drawn `Surface` and the execution [`PlotReport`].
pub fn plot(
    spec: &PlotSpec,
    view: &PlotView,
    area: Rect,
    mode: SubcellGlyphMode,
) -> (Surface, PlotReport) {
    let (layout, report) = compile(spec, view, area);
    (render(&layout, mode), report)
}
