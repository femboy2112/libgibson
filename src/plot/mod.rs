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

pub use data::{Annotation, AxisSpec, PlotSpec, PlotView, Reduce, Series, SeriesKind};
pub use layout::{
    compile, PlotError, PlotLayout, PlotReport, Prims, ProjAnnotation, ProjectedSeries,
    ProjectedTick,
};
pub use render::{render, render_themed, PlotTheme};
pub use scale::{AxisScale, AxisTransform, FiniteRange, PlotTransform2D, Viewport};
pub use ticks::{log10_major_ticks, log10_minor_ticks, log10_ticks, major_ticks, Tick};

use crate::{Rect, SubcellGlyphMode, Surface};

/// One-call convenience: compile a spec + view into `area` and realize it under
/// `mode`, returning the drawn `Surface` and the execution [`PlotReport`] — or a
/// [`PlotError`] if the spec + view is not a valid configuration (e.g. a `Log10`
/// axis over a non-positive view). A valid-but-empty area is `Ok` with a blank
/// surface, not an error.
pub fn plot(
    spec: &PlotSpec,
    view: &PlotView,
    area: Rect,
    mode: SubcellGlyphMode,
) -> Result<(Surface, PlotReport), PlotError> {
    let (layout, report) = compile(spec, view, area)?;
    Ok((render(&layout, mode), report))
}
