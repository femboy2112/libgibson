//! Domain-neutral scalar-field observation (Wave 4 — "Scientific Observation").
//!
//! A [`FieldView`] is a read-only window onto a caller-owned, row-major grid of
//! `f64` samples laid out in WORLD space. It renders that field to a [`Surface`]
//! as a heatmap (or a monochrome / glyph-density alternative), probes a world
//! point for the sample under it, and hands back a conservation-law
//! [`FieldReport`] receipt — and it does all of this without a PDE solver, a
//! chemistry model, or any opinion about what the numbers *mean*. The producer
//! owns the data and the model; this module only observes it honestly.
//!
//! The load-bearing law (Law L3, identity transport) is geometric: a field cell
//! resolved under the pointer must be the *same* cell the renderer painted there.
//! This module guarantees that by construction — both the renderer and the probe
//! go `terminal-cell → world` through the **same** [`WorldView`] the rest of the
//! interactive stack uses for pointer selection, and then `world → field-index`
//! through one [`FieldExtent`] mapping. There is deliberately no second
//! projection: the plot module's own `Viewport` quantizes differently (it rounds,
//! and flips y), so a `FieldView` must never borrow it, or a probed cell and a
//! clicked cell could disagree at the edges.
//!
//! Honesty rules, mirrored from the rest of v0.5:
//!
//! * **Observation never mutates** the borrowed data (Law L1): every method takes
//!   `&self` and the data by shared reference.
//! * **Missing data is disclosed, not invented.** A masked cell renders as a
//!   distinct glyph and is *counted* as masked; a non-finite datum is reported as
//!   [`Sample::NonFinite`], never coerced to a plausible value.
//! * **The scale is disclosed.** [`RangePolicy::Auto`] records the range it chose
//!   (and whether it had to widen a degenerate all-equal field) in the receipt;
//!   [`RangePolicy::Fixed`] uses exactly the range it was given.
//! * **The receipt conserves cells.** Every cell in the rendered area is accounted
//!   for as a value, masked, non-finite, off-field, or clipped cell.

use crate::cell::Glyph;
use crate::plot::scale::FiniteRange;
use crate::shape::{Bounds, Point};
use crate::world_view::{Transport, WorldView};
use crate::{Cell, Color, Rect, Style, Surface};

/// A rectangular scalar field laid out in WORLD space.
///
/// The field is `cols × rows` cells; the cell at index `(ix, iy)` spans the
/// half-open world rectangle
/// `[origin.x + ix·cell_w, origin.x + (ix+1)·cell_w) × [origin.y + iy·cell_h, …)`.
/// The half-open convention on the max edge matches [`WorldView`]'s raster, so a
/// world point on a shared boundary resolves to exactly one cell under both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldExtent {
    origin: Point,
    cell_w: f64,
    cell_h: f64,
    cols: usize,
    rows: usize,
}

impl FieldExtent {
    /// `Some` iff the geometry is usable: finite origin, strictly positive finite
    /// cell sizes, and at least one cell on each axis. A degenerate extent is
    /// refused rather than silently repaired.
    pub fn new(origin: Point, cell_w: f64, cell_h: f64, cols: usize, rows: usize) -> Option<Self> {
        if origin.is_finite()
            && cell_w.is_finite()
            && cell_h.is_finite()
            && cell_w > 0.0
            && cell_h > 0.0
            && cols > 0
            && rows > 0
        {
            Some(Self {
                origin,
                cell_w,
                cell_h,
                cols,
                rows,
            })
        } else {
            None
        }
    }

    /// Column count.
    pub fn cols(&self) -> usize {
        self.cols
    }
    /// Row count.
    pub fn rows(&self) -> usize {
        self.rows
    }
    /// Total cell count.
    pub fn len(&self) -> usize {
        self.cols * self.rows
    }
    /// Always `false` — a constructed extent has at least one cell.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The field index a world point falls in, or `None` if the point is outside
    /// the field (or non-finite). Floor + half-open, matching pointer selection.
    pub fn world_to_index(&self, p: Point) -> Option<(usize, usize)> {
        if !p.is_finite() {
            return None;
        }
        let fx = (p.x - self.origin.x) / self.cell_w;
        let fy = (p.y - self.origin.y) / self.cell_h;
        if fx < 0.0 || fy < 0.0 {
            return None;
        }
        // fx,fy are finite and >= 0 here; floor-to-usize cannot wrap, and the
        // bound check rejects anything at or past the far edge.
        let ix = fx.floor() as usize;
        let iy = fy.floor() as usize;
        if ix >= self.cols || iy >= self.rows {
            return None;
        }
        Some((ix, iy))
    }

    /// The world-space center of a field cell (panics-free: out-of-range indices
    /// return `None`).
    pub fn cell_center(&self, ix: usize, iy: usize) -> Option<Point> {
        if ix >= self.cols || iy >= self.rows {
            return None;
        }
        Some(Point::new(
            self.origin.x + (ix as f64 + 0.5) * self.cell_w,
            self.origin.y + (iy as f64 + 0.5) * self.cell_h,
        ))
    }

    /// The world-space bounds of a field cell, as the same [`Bounds`] type pointer
    /// selection tests against (`Bounds::contains`). `None` if out of range.
    pub fn cell_bounds(&self, ix: usize, iy: usize) -> Option<Bounds> {
        if ix >= self.cols || iy >= self.rows {
            return None;
        }
        let min = Point::new(
            self.origin.x + ix as f64 * self.cell_w,
            self.origin.y + iy as f64 * self.cell_h,
        );
        let max = Point::new(min.x + self.cell_w, min.y + self.cell_h);
        Some(Bounds { min, max })
    }
}

/// The state of a single field sample.
///
/// A value is reported as-is; absence and non-finiteness are first-class, never
/// smuggled in as a number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sample {
    /// A finite observed value.
    Value(f64),
    /// The caller marked this cell absent via the presence mask.
    Masked,
    /// The underlying datum is `NaN` or infinite.
    NonFinite,
}

/// How the colour/intensity scale's range is chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RangePolicy {
    /// Use exactly this range; values outside it saturate at the ends.
    Fixed(FiniteRange),
    /// Derive the range from the finite samples in view, disclosed in the receipt.
    Auto,
}

/// How a sample's normalized intensity becomes a painted cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colormap {
    /// Full-colour blue→cyan→green→yellow→red ramp (needs truecolor/256).
    Heat,
    /// Monochrome luminance ramp — the reduced-colour alternative.
    Grayscale,
    /// Colour-free glyph-density ramp — the mono-terminal-safe alternative, where
    /// intensity survives even with no colour at all.
    Density,
}

/// A conservation-law receipt for one [`render`] call.
///
/// Every cell in the rendered `area` is accounted for exactly once:
/// `cells_in_area == value_cells + masked_cells + nonfinite_cells
///  + off_field_cells + clipped_cells`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldReport {
    /// The terminal area that was rendered.
    pub area: Rect,
    /// `true` if the range was derived ([`RangePolicy::Auto`]) rather than fixed.
    pub policy_was_auto: bool,
    /// The range actually used to scale values — `None` only when `Auto` found no
    /// finite samples to scale (then there are no value cells either).
    pub resolved_range: Option<FiniteRange>,
    /// `true` if an `Auto` range was widened because every finite sample was equal
    /// (a constant field has no natural span).
    pub range_degenerate_widened: bool,
    /// Terminal cells in the rendered area.
    pub cells_in_area: u32,
    /// Cells painted from a finite value.
    pub value_cells: u32,
    /// Cells whose sample was masked.
    pub masked_cells: u32,
    /// Cells whose datum was non-finite.
    pub nonfinite_cells: u32,
    /// In-view cells that fell outside the field extent.
    pub off_field_cells: u32,
    /// Cells the [`WorldView`] could not map (non-invertible / out of view).
    pub clipped_cells: u32,
}

impl FieldReport {
    /// Checks the conservation law. A rendered frame that fails this is a bug in
    /// this module, not in the caller.
    pub fn conserves(&self) -> bool {
        self.value_cells
            + self.masked_cells
            + self.nonfinite_cells
            + self.off_field_cells
            + self.clipped_cells
            == self.cells_in_area
    }
}

/// A rendered field plus its receipt.
pub struct FieldFrame {
    /// The painted surface, sized to the rendered area.
    pub surface: Surface,
    /// The conservation-law receipt.
    pub report: FieldReport,
}

/// The result of probing a world point against a field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeReport {
    /// The world point that was probed (provenance: exactly what was asked).
    pub world: Point,
    /// The field index the point fell in, or `None` if off-field.
    pub index: Option<(usize, usize)>,
    /// The sample at that index, or `None` if off-field.
    pub sample: Option<Sample>,
    /// The world-space center of the resolved cell (the quantized target).
    pub cell_center: Option<Point>,
    /// The cell's world size `(cell_w, cell_h)` — the quantization granularity the
    /// probe discloses, so the inspector can show how coarse the reading is.
    pub quantization: Option<(f64, f64)>,
}

/// A read-only view over a caller-owned, row-major scalar grid.
///
/// Cloning is cheap (it copies two references and the extent); the data is never
/// copied or mutated.
#[derive(Clone, Copy, Debug)]
pub struct FieldView<'a> {
    extent: FieldExtent,
    data: &'a [f64],
    mask: Option<&'a [bool]>,
}

impl<'a> FieldView<'a> {
    /// `Some` iff the data (and the mask, if present) length exactly matches the
    /// extent's cell count. A length mismatch is refused — it is a caller bug, not
    /// something to paper over with a partial render.
    pub fn new(extent: FieldExtent, data: &'a [f64], mask: Option<&'a [bool]>) -> Option<Self> {
        if data.len() != extent.len() {
            return None;
        }
        if let Some(m) = mask {
            if m.len() != extent.len() {
                return None;
            }
        }
        Some(Self { extent, data, mask })
    }

    /// The field's geometry.
    pub fn extent(&self) -> FieldExtent {
        self.extent
    }

    #[inline]
    fn flat(&self, ix: usize, iy: usize) -> usize {
        iy * self.extent.cols + ix
    }

    /// The sample at a field index, or `None` if out of range. A masked cell wins
    /// over its stored value; a non-finite datum is reported as such.
    pub fn sample(&self, ix: usize, iy: usize) -> Option<Sample> {
        if ix >= self.extent.cols || iy >= self.extent.rows {
            return None;
        }
        let i = self.flat(ix, iy);
        if let Some(m) = self.mask {
            if !m[i] {
                return Some(Sample::Masked);
            }
        }
        let v = self.data[i];
        Some(if v.is_finite() {
            Sample::Value(v)
        } else {
            Sample::NonFinite
        })
    }

    /// The `(min, max)` of every finite, unmasked sample, or `None` if there are
    /// none. Returns the raw extremes; callers decide how to handle `min == max`.
    pub fn finite_extremes(&self) -> Option<(f64, f64)> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for iy in 0..self.extent.rows {
            for ix in 0..self.extent.cols {
                if let Some(Sample::Value(v)) = self.sample(ix, iy) {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
        }
        if lo <= hi {
            Some((lo, hi))
        } else {
            None
        }
    }

    /// Probe a WORLD point: the field cell it lands in, the sample there, the
    /// resolved cell center, and the quantization granularity. This is the
    /// inspector's reading, with full provenance (it echoes the probed point).
    pub fn probe(&self, world: Point) -> ProbeReport {
        match self.extent.world_to_index(world) {
            Some((ix, iy)) => ProbeReport {
                world,
                index: Some((ix, iy)),
                sample: self.sample(ix, iy),
                cell_center: self.extent.cell_center(ix, iy),
                quantization: Some((self.extent.cell_w, self.extent.cell_h)),
            },
            None => ProbeReport {
                world,
                index: None,
                sample: None,
                cell_center: None,
                quantization: None,
            },
        }
    }

    /// Probe through a [`WorldView`] from a terminal cell — the pointer path. This
    /// is the SAME `terminal-cell → world` mapping the renderer uses, so a probe
    /// here and a painted cell agree. `None` if the cell is clipped by the view.
    pub fn probe_cell(&self, view: &WorldView, col: u16, row: u16) -> Option<ProbeReport> {
        match view.cell_to_world(col, row) {
            Transport::Mapped {
                value: (wx, wy), ..
            } => Some(self.probe(Point::new(wx, wy))),
            Transport::Clipped(_) => None,
        }
    }

    /// Renders the field into a [`Surface`] sized to `area`, mapping each terminal
    /// cell through `view` to a world point, then to a field sample. Returns the
    /// surface and a conservation-law [`FieldReport`].
    pub fn render(
        &self,
        view: &WorldView,
        area: Rect,
        colormap: Colormap,
        policy: RangePolicy,
    ) -> FieldFrame {
        // Resolve the scale range and disclose how.
        let (resolved, policy_was_auto, range_degenerate_widened) = match policy {
            RangePolicy::Fixed(r) => (Some(r), false, false),
            RangePolicy::Auto => match self.finite_extremes() {
                Some((lo, hi)) if lo < hi => (FiniteRange::new(lo, hi), true, false),
                // All finite samples equal: no natural span, so widen symmetrically
                // and disclose it rather than refuse to colour a constant field.
                Some((lo, _)) => (FiniteRange::new(lo - 0.5, lo + 0.5), true, true),
                None => (None, true, false),
            },
        };

        let mut surface = Surface::new(area.width.max(1), area.height.max(1));
        let mut report = FieldReport {
            area,
            policy_was_auto,
            resolved_range: resolved,
            range_degenerate_widened,
            cells_in_area: 0,
            value_cells: 0,
            masked_cells: 0,
            nonfinite_cells: 0,
            off_field_cells: 0,
            clipped_cells: 0,
        };

        for ry in 0..area.height {
            for rx in 0..area.width {
                report.cells_in_area += 1;
                let (col, row) = (area.x + rx, area.y + ry);
                let world = match view.cell_to_world(col, row) {
                    Transport::Mapped {
                        value: (wx, wy), ..
                    } => Point::new(wx, wy),
                    Transport::Clipped(_) => {
                        report.clipped_cells += 1;
                        continue;
                    }
                };
                let idx = match self.extent.world_to_index(world) {
                    Some(i) => i,
                    None => {
                        report.off_field_cells += 1;
                        continue;
                    }
                };
                // `world_to_index` already bounds-checked, so `sample` is `Some`.
                let cell = match self.sample(idx.0, idx.1) {
                    Some(Sample::Value(v)) => {
                        report.value_cells += 1;
                        let t = resolved
                            .map(|r| ((v - r.min()) / r.span()).clamp(0.0, 1.0))
                            .unwrap_or(0.5);
                        paint_value(colormap, t)
                    }
                    Some(Sample::Masked) => {
                        report.masked_cells += 1;
                        Cell::new(Glyph::from_char('·'), Style::new().dim())
                    }
                    Some(Sample::NonFinite) => {
                        report.nonfinite_cells += 1;
                        Cell::new(
                            Glyph::from_char('?'),
                            Style::new().fg(Color::Rgb(255, 80, 80)),
                        )
                    }
                    None => {
                        // Unreachable given the bounds check above, but accounted
                        // honestly rather than silently dropped.
                        report.off_field_cells += 1;
                        continue;
                    }
                };
                surface.set_cell(rx, ry, cell);
            }
        }

        FieldFrame { surface, report }
    }
}

/// Maps a normalized intensity `t ∈ [0,1]` to a painted cell under a colormap.
fn paint_value(colormap: Colormap, t: f64) -> Cell {
    let t = t.clamp(0.0, 1.0);
    match colormap {
        Colormap::Heat => {
            let (r, g, b) = crate::field::heat_rgb(t as f32);
            Cell::new(Glyph::from_char(' '), Style::new().bg(Color::Rgb(r, g, b)))
        }
        Colormap::Grayscale => {
            let g = (t * 255.0).round().clamp(0.0, 255.0) as u8;
            Cell::new(Glyph::from_char(' '), Style::new().bg(Color::Rgb(g, g, g)))
        }
        Colormap::Density => {
            const RAMP: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
            let i = ((t * (RAMP.len() - 1) as f64).round() as usize).min(RAMP.len() - 1);
            Cell::new(Glyph::from_char(RAMP[i]), Style::new())
        }
    }
}

/// Probes one world point across a sequence of field-data frames sharing `extent`,
/// yielding `(time, value)` points for a **linked time-series** plot. Only finite
/// samples contribute a point; masked / non-finite / off-field frames are skipped,
/// so the series never invents a reading. Feed the result straight to
/// [`crate::plot`] as a `Series`.
///
/// Each frame is a row-major slice the same length as `extent.len()`; a frame of
/// the wrong length contributes no point.
pub fn probe_series(
    frames: &[&[f64]],
    extent: FieldExtent,
    world: Point,
    dt: f64,
) -> Vec<(f64, f64)> {
    let idx = match extent.world_to_index(world) {
        Some(i) => i,
        None => return Vec::new(),
    };
    let flat = idx.1 * extent.cols + idx.0;
    let mut out = Vec::new();
    for (frame_i, &data) in frames.iter().enumerate() {
        if data.len() != extent.len() {
            continue;
        }
        let v = data[flat];
        if v.is_finite() {
            out.push((frame_i as f64 * dt, v));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent() -> FieldExtent {
        // 4x4 field, one world unit per cell, origin at (0,0).
        FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 4, 4).unwrap()
    }

    #[test]
    fn extent_refuses_degenerate_geometry() {
        assert!(FieldExtent::new(Point::new(0.0, 0.0), 0.0, 1.0, 4, 4).is_none());
        assert!(FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 0, 4).is_none());
        assert!(FieldExtent::new(Point::new(f64::NAN, 0.0), 1.0, 1.0, 4, 4).is_none());
    }

    #[test]
    fn world_to_index_is_floor_half_open() {
        let e = extent();
        assert_eq!(e.world_to_index(Point::new(0.0, 0.0)), Some((0, 0)));
        assert_eq!(e.world_to_index(Point::new(0.9, 0.9)), Some((0, 0)));
        assert_eq!(e.world_to_index(Point::new(1.0, 0.0)), Some((1, 0))); // boundary -> next
        assert_eq!(e.world_to_index(Point::new(3.5, 3.5)), Some((3, 3)));
        assert_eq!(e.world_to_index(Point::new(4.0, 0.0)), None); // half-open far edge
        assert_eq!(e.world_to_index(Point::new(-0.1, 0.0)), None);
    }

    #[test]
    fn sample_distinguishes_value_masked_nonfinite() {
        let e = extent();
        let mut data = vec![1.0; e.len()];
        data[e.cols() + 2] = f64::NAN; // (2,1) non-finite
        let mut mask = vec![true; e.len()];
        mask[3] = false; // (3,0) masked
        let fv = FieldView::new(e, &data, Some(&mask)).unwrap();
        assert_eq!(fv.sample(0, 0), Some(Sample::Value(1.0)));
        assert_eq!(fv.sample(3, 0), Some(Sample::Masked));
        assert_eq!(fv.sample(2, 1), Some(Sample::NonFinite));
        assert_eq!(fv.sample(4, 0), None);
    }

    #[test]
    fn new_refuses_length_mismatch() {
        let e = extent();
        let data = vec![0.0; e.len() - 1];
        assert!(FieldView::new(e, &data, None).is_none());
    }

    #[test]
    fn probe_reports_quantization_and_provenance() {
        let e = extent();
        let data: Vec<f64> = (0..e.len()).map(|i| i as f64).collect();
        let fv = FieldView::new(e, &data, None).unwrap();
        let p = fv.probe(Point::new(2.3, 1.7));
        assert_eq!(p.world, Point::new(2.3, 1.7)); // provenance: echoes the ask
        assert_eq!(p.index, Some((2, 1)));
        assert_eq!(p.sample, Some(Sample::Value(6.0))); // data[(2,1)] = 1*4+2
        assert_eq!(p.quantization, Some((1.0, 1.0)));
        assert_eq!(p.cell_center, Some(Point::new(2.5, 1.5)));
        // Off-field probe discloses nothing invented.
        let off = fv.probe(Point::new(9.0, 9.0));
        assert_eq!(off.index, None);
        assert_eq!(off.sample, None);
    }

    #[test]
    fn render_conserves_cells_and_discloses_range() {
        let e = extent();
        let data: Vec<f64> = (0..e.len()).map(|i| i as f64).collect();
        let fv = FieldView::new(e, &data, None).unwrap();
        // Anchor the view over the whole 6x6 render area so every rendered cell
        // maps to a world point; the 20 cells beyond the 4x4 field are off-field.
        let view = WorldView::new(Rect::new(0, 0, 6, 6))
            .with_zoom(1.0, 1.0)
            .with_pixels_per_cell(1, 1);
        let frame = fv.render(
            &view,
            Rect::new(0, 0, 6, 6),
            Colormap::Heat,
            RangePolicy::Auto,
        );
        assert!(frame.report.conserves(), "cells must be conserved");
        assert_eq!(frame.report.cells_in_area, 36);
        assert_eq!(frame.report.value_cells, 16); // the 4x4 field
        assert_eq!(frame.report.off_field_cells, 20);
        assert_eq!(frame.report.clipped_cells, 0);
        assert!(frame.report.policy_was_auto);
        assert_eq!(frame.report.resolved_range, FiniteRange::new(0.0, 15.0));
    }

    #[test]
    fn auto_range_widens_a_constant_field_and_flags_it() {
        let e = extent();
        let data = vec![7.0; e.len()];
        let fv = FieldView::new(e, &data, None).unwrap();
        let view = WorldView::new(Rect::new(0, 0, 4, 4))
            .with_zoom(1.0, 1.0)
            .with_pixels_per_cell(1, 1);
        let frame = fv.render(
            &view,
            Rect::new(0, 0, 4, 4),
            Colormap::Grayscale,
            RangePolicy::Auto,
        );
        assert!(frame.report.range_degenerate_widened);
        assert_eq!(frame.report.resolved_range, FiniteRange::new(6.5, 7.5));
        assert_eq!(frame.report.value_cells, 16);
    }

    #[test]
    fn probe_series_skips_nonfinite_frames_only() {
        let e = extent();
        let flat = e.cols() + 2; // cell (2,1)
        let mut f0 = vec![0.0; e.len()];
        f0[flat] = 3.0;
        let mut f1 = vec![0.0; e.len()];
        f1[flat] = f64::NAN; // skipped
        let mut f2 = vec![0.0; e.len()];
        f2[flat] = 5.0;
        let frames: Vec<&[f64]> = vec![&f0, &f1, &f2];
        let series = probe_series(&frames, e, Point::new(2.5, 1.5), 0.1);
        assert_eq!(series, vec![(0.0, 3.0), (0.2, 5.0)]);
    }
}
