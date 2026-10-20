//! Reversible 2D coordinate transport between a continuous data space and the
//! integer terminal cell grid.
//!
//! Interactive instruments pan and zoom over continuous data, then realize that
//! data on a coarse, non-square cell grid. Each realization quantizes (continuous
//! → integer) or clips (outside the viewport). This module makes every such step
//! **explicit, invertible and honest**: a quantizing or clipping transport never
//! silently returns `0` or clamps — it returns a `Transport` that either carries
//! the mapped value *and* a `Residual` receipt describing what was discarded, or
//! reports a `Clip` with no value at all.
//!
//! ```text
//! WORLD  <->  VIEW  <->  RASTER  <->  CELL
//! (f64)      (f64)       (i32 px)    (u16 Rect)
//! continuous pan/zoom    integer     terminal
//! data       applied     pixels      cells
//! ```
//!
//! * **WORLD** — continuous `f64` data space. Units are caller-defined "world units".
//! * **VIEW** — WORLD after pan and zoom, measured in raster pixels with the origin
//!   at the top-left of the cell anchor (see `WorldView::anchor`). Still continuous;
//!   `view = (world - pan) * zoom`.
//! * **RASTER** — integer sub-cell pixels, `[0, raster_w) x [0, raster_h)`. This is
//!   where continuous VIEW is quantized (floor) and clipped.
//! * **CELL** — terminal cells inside the [`Rect`] anchor, `u16`, matching the
//!   half-open `[x, x + width)` convention of [`Rect`]. Pixel-aspect (a terminal
//!   cell is roughly twice as tall as wide) is carried by
//!   `WorldView::pixels_per_cell`: a cell groups `ppc_x` x
//!   `ppc_y` raster pixels, so the pixel aspect is `ppc_y / ppc_x`.
//!
//! Every function here is a pure function of the `WorldView` and its arguments:
//! no clock, no mutable global state, reproducible. No input panics — non-finite
//! coordinates, zero or enormous zoom, and out-of-range indices each produce a
//! defined `Transport` result.

use crate::Rect;

/// What a quantizing transport discarded, as a receipt.
///
/// Quantization snaps a continuous source point to a discrete quantum (a raster
/// pixel, or a cell). An inverse map returns the **center** of that quantum. The
/// receipt records the quantum size and where the source sat relative to the
/// center, both in the *source* space's units:
///
/// * `interval_{x,y}` — the width of the quantum, i.e. the span of source values
///   that collapse to one quantized result. A forward round-trip
///   (`forward` then its inverse) recovers the input to within `interval / 2`.
/// * `offset_{x,y}` — the signed distance from the source point to the center of
///   the quantum it snapped to, in `(-interval/2, +interval/2]`. This is exactly
///   the error a forward-then-inverse round-trip will exhibit: the inverse returns
///   the center, so `recovered == source - offset`.
///
/// Units follow the quantizing step: [`WorldView::view_to_raster`] reports view
/// pixels, [`WorldView::raster_to_cell`] reports raster pixels, and the composed
/// [`WorldView::world_to_cell`] reports world units. Inverse maps report
/// `offset == 0` (a cell center is exact within its own quantum) and the quantum's
/// world footprint as `interval`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Residual {
    /// Width of one quantum on the x axis, in the source space's units.
    pub interval_x: f64,
    /// Width of one quantum on the y axis, in the source space's units.
    pub interval_y: f64,
    /// Signed source-to-quantum-center offset on the x axis; the round-trip error.
    pub offset_x: f64,
    /// Signed source-to-quantum-center offset on the y axis; the round-trip error.
    pub offset_y: f64,
}

impl Residual {
    /// An exact, lossless receipt: a continuous quantum that discarded nothing.
    pub const EXACT: Residual = Residual {
        interval_x: 0.0,
        interval_y: 0.0,
        offset_x: 0.0,
        offset_y: 0.0,
    };

    /// The worst-case round-trip error this quantization admits, `interval / 2`
    /// per axis. A forward-then-inverse round-trip always lands within this bound.
    pub fn max_round_trip_error(&self) -> (f64, f64) {
        (self.interval_x.abs() / 2.0, self.interval_y.abs() / 2.0)
    }
}

/// Why a transport produced no valid value.
///
/// A clipped transport deliberately carries **no** coordinate: an out-of-view or
/// non-finite source has no honest in-bounds answer, so callers cannot mistake a
/// clamped `0` for a real cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clip {
    /// The source coordinate was `NaN` or infinite (or became so under the
    /// transform, e.g. a finite world point times an infinite zoom).
    NonFinite,
    /// The source mapped outside the raster/cell viewport on at least one axis.
    OutOfView,
    /// The quantized index exceeded the destination integer range (`i32` raster
    /// pixel or `u16` cell). Guarded rather than truncated.
    OutOfRange,
}

/// The result of a quantizing or clipping transport: either a mapped value with a
/// [`Residual`] receipt, or a [`Clip`] with no value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transport<T> {
    /// Landed inside the viewport. `residual` records what quantization discarded.
    Mapped {
        /// The mapped, quantized coordinate.
        value: T,
        /// Receipt describing the quantization loss (see [`Residual`]).
        residual: Residual,
    },
    /// Fell outside the viewport, or was non-finite / out of range.
    Clipped(Clip),
}

impl<T> Transport<T> {
    /// `true` if the transport was clipped (carries no value).
    pub fn is_clipped(&self) -> bool {
        matches!(self, Transport::Clipped(_))
    }

    /// `true` if the transport produced a value.
    pub fn is_mapped(&self) -> bool {
        matches!(self, Transport::Mapped { .. })
    }

    /// Borrows the mapped value, or `None` if clipped.
    pub fn value(&self) -> Option<&T> {
        match self {
            Transport::Mapped { value, .. } => Some(value),
            Transport::Clipped(_) => None,
        }
    }

    /// The quantization receipt, or `None` if clipped.
    pub fn residual(&self) -> Option<Residual> {
        match self {
            Transport::Mapped { residual, .. } => Some(*residual),
            Transport::Clipped(_) => None,
        }
    }

    /// The clip reason, or `None` if mapped.
    pub fn clip(&self) -> Option<Clip> {
        match self {
            Transport::Clipped(c) => Some(*c),
            Transport::Mapped { .. } => None,
        }
    }
}

/// An invertible 2D transport from continuous world data to terminal cells.
///
/// Construct with [`WorldView::new`] (unit zoom, no pan, a default 1x2
/// pixels-per-cell aspect) and refine with the `with_*` builders. The transform is
/// `view = (world - pan) * zoom`: **pan is subtracted in world units first, then
/// zoom scales to pixels.** Because of that order, zooming while holding a world
/// point fixed on screen must compensate the pan; [`WorldView::zoom_about_world`]
/// does exactly that, giving a stable fixed point for round-trips.
///
/// All state is small and `Copy`; the builders return a new value rather than
/// mutating in place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldView {
    pan_x: f64,
    pan_y: f64,
    zoom_x: f64,
    zoom_y: f64,
    anchor: Rect,
    ppc_x: u16,
    ppc_y: u16,
}

impl WorldView {
    /// A transport anchored to `anchor` (the terminal cell rectangle this view
    /// paints into), with no pan, unit zoom (1 pixel per world unit), and a 1x2
    /// pixels-per-cell grouping — the common terminal aspect where a cell is one
    /// pixel wide and two pixels tall (pixel aspect 2.0).
    pub fn new(anchor: Rect) -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom_x: 1.0,
            zoom_y: 1.0,
            anchor,
            ppc_x: 1,
            ppc_y: 2,
        }
    }

    /// Sets the pan origin: the world coordinate that maps to the top-left of the
    /// anchor (view origin). Units: world units.
    pub fn with_pan(mut self, pan_x: f64, pan_y: f64) -> Self {
        self.pan_x = pan_x;
        self.pan_y = pan_y;
        self
    }

    /// Sets the per-axis zoom in raster pixels per world unit. Independent axes
    /// permit correcting for pixel aspect. Values are stored verbatim; a zero or
    /// non-finite zoom yields a defined (not panicking) degenerate transport —
    /// see the type-level and method docs.
    pub fn with_zoom(mut self, zoom_x: f64, zoom_y: f64) -> Self {
        self.zoom_x = zoom_x;
        self.zoom_y = zoom_y;
        self
    }

    /// Sets how many raster pixels compose one cell per axis; the pixel aspect is
    /// `ppc_y / ppc_x`. Each value is clamped to at least 1 (a cell always covers
    /// at least one pixel), so division by zero is impossible downstream.
    pub fn with_pixels_per_cell(mut self, ppc_x: u16, ppc_y: u16) -> Self {
        self.ppc_x = ppc_x.max(1);
        self.ppc_y = ppc_y.max(1);
        self
    }

    /// The pan origin in world units: `(pan_x, pan_y)`.
    pub fn pan(&self) -> (f64, f64) {
        (self.pan_x, self.pan_y)
    }

    /// The per-axis zoom in pixels per world unit.
    pub fn zoom(&self) -> (f64, f64) {
        (self.zoom_x, self.zoom_y)
    }

    /// The terminal cell rectangle this view paints into.
    pub fn anchor(&self) -> Rect {
        self.anchor
    }

    /// Raster pixels per cell, per axis (always at least 1).
    pub fn pixels_per_cell(&self) -> (u16, u16) {
        (self.ppc_x, self.ppc_y)
    }

    /// The pixel aspect ratio `ppc_y / ppc_x` (height per unit width). A standard
    /// terminal cell reads near 2.0.
    pub fn pixel_aspect(&self) -> f64 {
        f64::from(self.ppc_y) / f64::from(self.ppc_x)
    }

    /// The raster extent in pixels, `(width, height)`, as `anchor dimension *
    /// pixels_per_cell`. Saturated at [`i32::MAX`] so an enormous grouping cannot
    /// overflow the raster integer range; the valid raster region is
    /// `[0, width) x [0, height)`.
    pub fn raster_extent(&self) -> (i32, i32) {
        let w = i64::from(self.anchor.width) * i64::from(self.ppc_x);
        let h = i64::from(self.anchor.height) * i64::from(self.ppc_y);
        (
            w.min(i64::from(i32::MAX)) as i32,
            h.min(i64::from(i32::MAX)) as i32,
        )
    }

    // -- Forward: continuous (exact) --------------------------------------

    /// Maps a world point to continuous VIEW coordinates (raster pixels, origin at
    /// the anchor top-left): `view = (world - pan) * zoom`. Exact and unbounded —
    /// no quantization or clipping — so it returns the pair directly. Non-finite
    /// inputs (or overflow under extreme zoom) propagate as non-finite outputs,
    /// which the downstream quantizing maps report as [`Clip::NonFinite`].
    pub fn world_to_view(&self, wx: f64, wy: f64) -> (f64, f64) {
        (
            (wx - self.pan_x) * self.zoom_x,
            (wy - self.pan_y) * self.zoom_y,
        )
    }

    // -- Forward: quantizing / clipping -----------------------------------

    /// Quantizes continuous VIEW coordinates to an integer RASTER pixel.
    ///
    /// Snaps each axis by `floor` and clips against the raster extent
    /// (`[0, w) x [0, h)`, half-open like [`Rect`]). The [`Residual`] reports a
    /// quantum of 1.0 view pixel and the signed offset of the source from the pixel
    /// center. Non-finite input is [`Clip::NonFinite`]; out-of-extent is
    /// [`Clip::OutOfView`].
    pub fn view_to_raster(&self, vx: f64, vy: f64) -> Transport<(i32, i32)> {
        if !vx.is_finite() || !vy.is_finite() {
            return Transport::Clipped(Clip::NonFinite);
        }
        let (rw, rh) = self.raster_extent();
        // Compare in f64 so an out-of-range value never reaches a narrowing cast.
        if vx < 0.0 || vy < 0.0 || vx >= f64::from(rw) || vy >= f64::from(rh) {
            return Transport::Clipped(Clip::OutOfView);
        }
        let fx = vx.floor();
        let fy = vy.floor();
        // fx, fy are now within [0, rw) / [0, rh), each <= i32::MAX, so the cast
        // cannot truncate.
        let rx = fx as i32;
        let ry = fy as i32;
        Transport::Mapped {
            value: (rx, ry),
            residual: Residual {
                interval_x: 1.0,
                interval_y: 1.0,
                offset_x: vx - (fx + 0.5),
                offset_y: vy - (fy + 0.5),
            },
        }
    }

    /// Groups an integer RASTER pixel into its CELL.
    ///
    /// Integer-floors by [`pixels_per_cell`](WorldView::pixels_per_cell) and offsets
    /// by the anchor origin. The [`Residual`] reports a quantum of `ppc` raster
    /// pixels and the signed offset of the pixel from the cell center, in raster
    /// pixels. Pixels outside the raster extent are [`Clip::OutOfView`]; a cell
    /// index that would exceed `u16` is [`Clip::OutOfRange`] rather than truncated.
    pub fn raster_to_cell(&self, rx: i32, ry: i32) -> Transport<(u16, u16)> {
        let (rw, rh) = self.raster_extent();
        if rx < 0 || ry < 0 || rx >= rw || ry >= rh {
            return Transport::Clipped(Clip::OutOfView);
        }
        let ppc_x = i32::from(self.ppc_x);
        let ppc_y = i32::from(self.ppc_y);
        let ox = rx / ppc_x; // rx >= 0, ppc >= 1: a true floor.
        let oy = ry / ppc_y;
        let cx = i32::from(self.anchor.x) + ox;
        let cy = i32::from(self.anchor.y) + oy;
        if cx > i32::from(u16::MAX) || cy > i32::from(u16::MAX) {
            return Transport::Clipped(Clip::OutOfRange);
        }
        let cell_center_x = (ox * ppc_x) as f64 + f64::from(self.ppc_x) / 2.0;
        let cell_center_y = (oy * ppc_y) as f64 + f64::from(self.ppc_y) / 2.0;
        Transport::Mapped {
            value: (cx as u16, cy as u16),
            residual: Residual {
                interval_x: f64::from(self.ppc_x),
                interval_y: f64::from(self.ppc_y),
                offset_x: f64::from(rx) - cell_center_x,
                offset_y: f64::from(ry) - cell_center_y,
            },
        }
    }

    /// Maps a world point directly to its CELL, composing
    /// [`world_to_view`](WorldView::world_to_view),
    /// [`view_to_raster`](WorldView::view_to_raster) and
    /// [`raster_to_cell`](WorldView::raster_to_cell).
    ///
    /// The [`Residual`] is reported in **world units**: `interval` is the world
    /// footprint of one cell (`ppc / zoom` per axis) and `offset` is the signed
    /// distance from the world point to the world-space center of its cell. Because
    /// [`cell_to_world`](WorldView::cell_to_world) returns that center,
    /// `offset` is exactly the error of a world → cell → world round-trip, bounded
    /// by `interval / 2`.
    ///
    /// Non-finite world (or non-finite view under extreme zoom) is
    /// [`Clip::NonFinite`]; out-of-view is [`Clip::OutOfView`]; an impossible cell
    /// index is [`Clip::OutOfRange`]. A zero zoom collapses all world onto the
    /// anchor's first cell and reports infinite residual intervals — degenerate but
    /// defined and non-panicking.
    pub fn world_to_cell(&self, wx: f64, wy: f64) -> Transport<(u16, u16)> {
        if !wx.is_finite() || !wy.is_finite() {
            return Transport::Clipped(Clip::NonFinite);
        }
        let (vx, vy) = self.world_to_view(wx, wy);
        if !vx.is_finite() || !vy.is_finite() {
            return Transport::Clipped(Clip::NonFinite);
        }
        let (rw, rh) = self.raster_extent();
        if vx < 0.0 || vy < 0.0 || vx >= f64::from(rw) || vy >= f64::from(rh) {
            return Transport::Clipped(Clip::OutOfView);
        }
        let rx = vx.floor() as i32; // within [0, rw) <= i32::MAX.
        let ry = vy.floor() as i32;
        let ppc_x = i32::from(self.ppc_x);
        let ppc_y = i32::from(self.ppc_y);
        let ox = rx / ppc_x;
        let oy = ry / ppc_y;
        let cx = i32::from(self.anchor.x) + ox;
        let cy = i32::from(self.anchor.y) + oy;
        if cx > i32::from(u16::MAX) || cy > i32::from(u16::MAX) {
            return Transport::Clipped(Clip::OutOfRange);
        }
        // Cell center in view pixels, then back to world for an honest world-space
        // receipt whose offset is the exact round-trip error.
        let center_view_x = (ox * ppc_x) as f64 + f64::from(self.ppc_x) / 2.0;
        let center_view_y = (oy * ppc_y) as f64 + f64::from(self.ppc_y) / 2.0;
        let center_world_x = self.pan_x + center_view_x / self.zoom_x;
        let center_world_y = self.pan_y + center_view_y / self.zoom_y;
        Transport::Mapped {
            value: (cx as u16, cy as u16),
            residual: Residual {
                interval_x: f64::from(self.ppc_x) / self.zoom_x,
                interval_y: f64::from(self.ppc_y) / self.zoom_y,
                offset_x: wx - center_world_x,
                offset_y: wy - center_world_y,
            },
        }
    }

    // -- Inverse ----------------------------------------------------------

    /// Maps continuous VIEW coordinates back to WORLD: `world = pan + view / zoom`.
    /// Exact inverse of [`world_to_view`](WorldView::world_to_view). A zero zoom
    /// produces non-finite output, which [`cell_to_world`](WorldView::cell_to_world)
    /// reports as [`Clip::NonFinite`].
    pub fn view_to_world(&self, vx: f64, vy: f64) -> (f64, f64) {
        (self.pan_x + vx / self.zoom_x, self.pan_y + vy / self.zoom_y)
    }

    /// Maps a CELL back to the WORLD point at the **center** of the world region it
    /// covers.
    ///
    /// The center is the canonical representative of a cell's world footprint: it
    /// minimizes the worst-case distance to any point the cell could have come
    /// from, so a world → cell → world round-trip is bounded by half the cell's
    /// world footprint. That footprint is reported as the [`Residual`] `interval`
    /// (`ppc / zoom` per axis); `offset` is `0` because the center is exact within
    /// its own quantum.
    ///
    /// A cell outside the anchor is [`Clip::OutOfView`]; a center that maps to a
    /// non-finite world point (zero zoom) is [`Clip::NonFinite`].
    pub fn cell_to_world(&self, cx: u16, cy: u16) -> Transport<(f64, f64)> {
        if !self.anchor.contains(cx, cy) {
            return Transport::Clipped(Clip::OutOfView);
        }
        let ox = f64::from(cx - self.anchor.x);
        let oy = f64::from(cy - self.anchor.y);
        let center_view_x = (ox + 0.5) * f64::from(self.ppc_x);
        let center_view_y = (oy + 0.5) * f64::from(self.ppc_y);
        let (wx, wy) = self.view_to_world(center_view_x, center_view_y);
        if !wx.is_finite() || !wy.is_finite() {
            return Transport::Clipped(Clip::NonFinite);
        }
        Transport::Mapped {
            value: (wx, wy),
            residual: Residual {
                interval_x: f64::from(self.ppc_x) / self.zoom_x,
                interval_y: f64::from(self.ppc_y) / self.zoom_y,
                offset_x: 0.0,
                offset_y: 0.0,
            },
        }
    }

    // -- Pan / zoom operations --------------------------------------------

    /// Shifts the pan origin by a world-space delta, returning the updated view.
    /// Since `view = (world - pan) * zoom`, a positive delta increases `pan` and
    /// thus scrolls world content toward the origin (left/up on screen).
    pub fn pan_by(self, dx_world: f64, dy_world: f64) -> Self {
        self.with_pan(self.pan_x + dx_world, self.pan_y + dy_world)
    }

    /// Scales the per-axis zoom by `factor`, holding the world point
    /// `(wx, wy)` fixed at its current VIEW (and therefore cell) position.
    ///
    /// Zoom alone would pivot about the view origin; this compensates the pan so
    /// the chosen world point does not drift, which is what keeps repeated
    /// zoom operations and their round-trips stable. New zoom is `zoom * factor`;
    /// new pan is `w - (w - pan) / factor` per axis. A non-finite or zero factor is
    /// applied verbatim, yielding a defined degenerate view rather than a panic.
    pub fn zoom_about_world(self, wx: f64, wy: f64, factor_x: f64, factor_y: f64) -> Self {
        let pan_x = wx - (wx - self.pan_x) / factor_x;
        let pan_y = wy - (wy - self.pan_y) / factor_y;
        self.with_zoom(self.zoom_x * factor_x, self.zoom_y * factor_y)
            .with_pan(pan_x, pan_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A worked reference configuration. anchor 10x10 cells, zoom 2 px/world,
    // pixels-per-cell 1x2 -> raster 10x20, pixel aspect 2.0.
    fn reference() -> WorldView {
        WorldView::new(Rect::new(0, 0, 10, 10))
            .with_zoom(2.0, 2.0)
            .with_pixels_per_cell(1, 2)
    }

    #[test]
    fn raster_extent_and_aspect_are_exact() {
        let v = reference();
        assert_eq!(v.raster_extent(), (10, 20));
        assert_eq!(v.pixel_aspect(), 2.0);
        // Default pixels-per-cell is 1x2.
        assert_eq!(
            WorldView::new(Rect::new(0, 0, 4, 4)).pixels_per_cell(),
            (1, 2)
        );
        // pixels-per-cell is clamped to at least 1 so downstream division is safe.
        assert_eq!(v.with_pixels_per_cell(0, 0).pixels_per_cell(), (1, 1));
    }

    #[test]
    fn world_to_cell_and_back_lands_within_one_cell() {
        // Predicted by hand: world (1.3, 1.3), zoom 2 -> view (2.6, 2.6).
        // floor -> raster (2, 2). ppc (1,2) -> cell (2, 1).
        // cell center view ((2.5)*1, (1.5)*2) = (2.5, 3.0) -> world (1.25, 1.5).
        let v = reference();
        let fwd = v.world_to_cell(1.3, 1.3);
        let (cell, res) = match fwd {
            Transport::Mapped { value, residual } => (value, residual),
            Transport::Clipped(c) => panic!("expected mapped, got {c:?}"),
        };
        assert_eq!(cell, (2, 1));
        assert_eq!(res.interval_x, 0.5); // ppc_x/zoom_x = 1/2
        assert_eq!(res.interval_y, 1.0); // ppc_y/zoom_y = 2/2
        assert!((res.offset_x - 0.05).abs() < 1e-12); // 1.3 - 1.25
        assert!((res.offset_y - (-0.2)).abs() < 1e-12); // 1.3 - 1.5

        let back = v.cell_to_world(cell.0, cell.1);
        let (w2, bres) = match back {
            Transport::Mapped { value, residual } => (value, residual),
            Transport::Clipped(c) => panic!("expected mapped, got {c:?}"),
        };
        // Inverse returns the cell center, offset 0.
        assert_eq!(bres.offset_x, 0.0);
        assert_eq!(bres.offset_y, 0.0);
        assert!((w2.0 - 1.25).abs() < 1e-12);
        assert!((w2.1 - 1.5).abs() < 1e-12);
        // recovered == source - offset, and the error is bounded by interval/2.
        assert!((w2.0 - (1.3 - res.offset_x)).abs() < 1e-12);
        assert!((w2.1 - (1.3 - res.offset_y)).abs() < 1e-12);
        let (ex, ey) = res.max_round_trip_error();
        assert!(res.offset_x.abs() <= ex + 1e-12);
        assert!(res.offset_y.abs() <= ey + 1e-12);
    }

    #[test]
    fn round_trip_bounded_across_many_pan_zoom_settings() {
        let configs = [
            reference(),
            reference().with_pan(-10.0, 5.0),
            reference().with_zoom(0.25, 7.0),
            reference().with_pixels_per_cell(2, 3).with_zoom(1.5, 1.5),
            WorldView::new(Rect::new(3, 4, 20, 8))
                .with_pan(100.0, -100.0)
                .with_zoom(0.5, 0.5),
        ];
        // Sample world points that stay in view for the reference-ish configs via
        // their own forward map; only assert the bound when mapped.
        for v in configs {
            for i in 0..7 {
                for j in 0..7 {
                    let (px, py) = v.pan();
                    let (zx, zy) = v.zoom();
                    // Build a world point known to sit inside the raster extent.
                    let (rw, rh) = v.raster_extent();
                    let wx = px + (f64::from(rw) * (i as f64 + 0.37) / 7.0) / zx;
                    let wy = py + (f64::from(rh) * (j as f64 + 0.61) / 7.0) / zy;
                    if let Transport::Mapped { value, residual } = v.world_to_cell(wx, wy) {
                        let back = v.cell_to_world(value.0, value.1);
                        let (w2, _) = match back {
                            Transport::Mapped { value, residual } => (value, residual),
                            Transport::Clipped(c) => panic!("inverse clipped {c:?}"),
                        };
                        let (ex, ey) = residual.max_round_trip_error();
                        assert!(
                            (wx - w2.0).abs() <= ex + 1e-9,
                            "x err exceeds cell interval"
                        );
                        assert!(
                            (wy - w2.1).abs() <= ey + 1e-9,
                            "y err exceeds cell interval"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn pixel_aspect_maps_square_world_to_nonsquare_cells() {
        // Square world region [0, 8) x [0, 8), zoom 1, ppc 1x2 (aspect 2:1).
        // Width: 8 world units -> 8 px -> 8 cells. Height: 8 -> 8 px -> 4 cells.
        let v = WorldView::new(Rect::new(0, 0, 8, 4))
            .with_zoom(1.0, 1.0)
            .with_pixels_per_cell(1, 2);
        let lo = v.world_to_cell(0.0, 0.0);
        let hi = v.world_to_cell(7.999, 7.999);
        assert_eq!(lo.value().copied(), Some((0, 0)));
        assert_eq!(hi.value().copied(), Some((7, 3)));
        // 8 cells wide, 4 cells tall for the same square world extent.
        let (clo, chi) = (lo.value().unwrap(), hi.value().unwrap());
        assert_eq!(chi.0 - clo.0 + 1, 8);
        assert_eq!(chi.1 - clo.1 + 1, 4);
    }

    #[test]
    fn out_of_view_world_is_clipped_not_a_bogus_cell() {
        let v = reference();
        // Left/above of the view.
        assert_eq!(
            v.world_to_cell(-1.0, -1.0),
            Transport::Clipped(Clip::OutOfView)
        );
        // Right/below: view x = 100*2 = 200 >= raster width 10.
        assert_eq!(
            v.world_to_cell(100.0, 100.0),
            Transport::Clipped(Clip::OutOfView)
        );
        // A cell outside the anchor inverts to Clipped, not world (0,0).
        assert_eq!(v.cell_to_world(99, 99), Transport::Clipped(Clip::OutOfView));
    }

    #[test]
    fn nonfinite_and_degenerate_zoom_are_defined() {
        let v = reference();
        assert_eq!(
            v.world_to_cell(f64::NAN, 0.0),
            Transport::Clipped(Clip::NonFinite)
        );
        assert_eq!(
            v.world_to_cell(f64::INFINITY, 0.0),
            Transport::Clipped(Clip::NonFinite)
        );
        // Enormous (finite) zoom pushes any nonzero offset out of view, never panics.
        let huge = reference().with_zoom(1e300, 1e300);
        assert_eq!(
            huge.world_to_cell(1.0, 1.0),
            Transport::Clipped(Clip::OutOfView)
        );
        // Zero zoom collapses all world onto the anchor's first cell with an
        // infinite reported interval: degenerate but defined.
        let flat = reference().with_zoom(0.0, 0.0);
        match flat.world_to_cell(42.0, -42.0) {
            Transport::Mapped { value, residual } => {
                assert_eq!(value, (0, 0));
                assert!(residual.interval_x.is_infinite());
                assert!(residual.interval_y.is_infinite());
            }
            Transport::Clipped(c) => panic!("zero zoom should map, got {c:?}"),
        }
        // Zero zoom makes the inverse non-finite: reported, not panicked.
        assert_eq!(
            flat.cell_to_world(0, 0),
            Transport::Clipped(Clip::NonFinite)
        );
    }

    #[test]
    fn boundary_and_negative_world_are_defined() {
        let v = reference();
        // World 1.0 -> view 2.0 exactly: the lower (inclusive) edge of raster px 2,
        // so it belongs to cell x = 2, offset = -interval/2 exactly.
        let b = v.world_to_cell(1.0, 1.0);
        match b {
            Transport::Mapped { value, residual } => {
                assert_eq!(value.0, 2);
                assert!((residual.offset_x - (-0.25)).abs() < 1e-12); // -interval_x/2
                let (ex, _) = residual.max_round_trip_error();
                assert!(residual.offset_x.abs() <= ex + 1e-12);
            }
            Transport::Clipped(c) => panic!("boundary should map, got {c:?}"),
        }
        // Negative world coordinates map fine when the pan brings them into view.
        let shifted = reference().with_pan(-10.0, -10.0);
        // world (-9, -9) -> view ((-9+10)*2, ...) = (2, 2) -> cell (2, 1).
        assert_eq!(
            shifted.world_to_cell(-9.0, -9.0).value().copied(),
            Some((2, 1))
        );
    }

    #[test]
    fn pan_by_shifts_cells_as_predicted() {
        let v = reference();
        // world (1,1) -> view (2,2) -> cell x 2.
        assert_eq!(v.world_to_cell(1.0, 1.0).value().unwrap().0, 2);
        // Shift pan origin to (1,1): world (1,1) is now the view origin -> cell x 0.
        let p = v.pan_by(1.0, 1.0);
        assert_eq!(p.pan(), (1.0, 1.0));
        assert_eq!(p.world_to_cell(1.0, 1.0).value().unwrap().0, 0);
    }

    #[test]
    fn zoom_about_world_holds_the_fixed_point() {
        let v = reference();
        let fixed = (1.3_f64, 1.3_f64);
        let before = v.world_to_cell(fixed.0, fixed.1).value().copied();
        // Zoom 2x about the fixed world point; it must stay in the same cell.
        let z = v.zoom_about_world(fixed.0, fixed.1, 2.0, 2.0);
        assert_eq!(z.zoom(), (4.0, 4.0));
        // new pan_x = 1.3 - (1.3 - 0)/2 = 0.65; view of 1.3 = (1.3-0.65)*4 = 2.6, unchanged.
        assert!((z.pan().0 - 0.65).abs() < 1e-12);
        let after = z.world_to_cell(fixed.0, fixed.1).value().copied();
        assert_eq!(before, after);
    }

    // MUTATION-style negative control: if cell_to_world returned the cell's LOWER
    // CORNER instead of its CENTER, this test would pass with the wrong 1.0/1.0.
    // It asserts the center (1.25, 1.5), so a corner implementation FAILS here.
    #[test]
    fn cell_to_world_returns_center_not_corner() {
        let v = reference();
        let w = v.cell_to_world(2, 1);
        let (wx, wy) = match w {
            Transport::Mapped { value, .. } => value,
            Transport::Clipped(c) => panic!("expected mapped, got {c:?}"),
        };
        // Center, NOT the corner (which would be 1.0, 1.0).
        assert!(
            (wx - 1.25).abs() < 1e-12,
            "x should be cell center 1.25, not corner 1.0"
        );
        assert!(
            (wy - 1.5).abs() < 1e-12,
            "y should be cell center 1.5, not corner 1.0"
        );
        assert_ne!(wx, 1.0);
        assert_ne!(wy, 1.0);
    }

    // Second negative control: distinct world points inside one cell collapse to
    // the same cell (quantization is real and lossy), and the inverse yields the
    // single shared center. A transport that secretly preserved sub-cell position
    // would fail the equality of cells.
    #[test]
    fn quantization_is_lossy_within_a_cell() {
        let v = reference();
        let a = v.world_to_cell(1.26, 1.26).value().copied();
        let b = v.world_to_cell(1.30, 1.30).value().copied();
        assert_eq!(
            a, b,
            "two points in the same cell must quantize identically"
        );
        assert_eq!(a, Some((2, 1)));
    }
}
