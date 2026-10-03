//! Axis transforms as validated partial morphisms, and their 2-D product.
//!
//! `σ : Domain ⇀ [0,1]` for one axis (`AxisTransform`), composed with a
//! `Viewport` (`[0,1]² → device pixels`) into a `PlotTransform2D`. Every map is
//! partial: a sample outside the scale domain (non-finite, or `≤ 0` under
//! `Log10`) has **no image** — `project` returns `None`, and no `NaN` ever
//! reaches quantization. See `docs/PLOT_OBSERVABLE_GEOMETRY.md` §4, §8.

/// A validated, finite, non-degenerate closed interval `[min, max]`.
///
/// Construction is the only gate: both ends finite and `min < max`. A reversed
/// or collapsed or non-finite range yields `None`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteRange {
    min: f64,
    max: f64,
}

impl FiniteRange {
    /// `Some` iff both ends are finite, `min < max`, **and the span
    /// `max - min` is itself finite**; `None` otherwise (NaN, ±∞, `min == max`,
    /// `min > max`, or a span that overflows `f64` — e.g. `-1e308..1e308`, whose
    /// width exceeds `f64::MAX`). The span gate is load-bearing: without it
    /// `span()` can be `+∞`, and a Linear projection then produces `NaN`/`∞` that
    /// reaches raster math (§4).
    pub fn new(min: f64, max: f64) -> Option<FiniteRange> {
        if min.is_finite() && max.is_finite() && min < max && (max - min).is_finite() {
            Some(FiniteRange { min, max })
        } else {
            None
        }
    }

    #[inline]
    pub fn min(&self) -> f64 {
        self.min
    }
    #[inline]
    pub fn max(&self) -> f64 {
        self.max
    }
    /// `max - min`, always finite and strictly positive by construction.
    #[inline]
    pub fn span(&self) -> f64 {
        self.max - self.min
    }
}

/// The scale a semantic axis is drawn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisScale {
    Linear,
    Log10,
}

/// A validated partial map `σ : Domain ⇀ [0,1]` over a visible `FiniteRange`.
///
/// `project` normalizes a data value to `[0,1]` within the range (values outside
/// the range project outside `[0,1]`, which is legal — clipping handles it). It
/// returns `None` for a non-finite value, and for a non-positive value under
/// `Log10`. `unproject` is the inverse for any finite normalized coordinate.
#[derive(Clone, Copy, Debug)]
pub struct AxisTransform {
    scale: AxisScale,
    range: FiniteRange,
}

impl AxisTransform {
    /// `None` if `scale == Log10` and `range.min() <= 0` (log domain is
    /// positive reals); otherwise `Some`.
    pub fn new(scale: AxisScale, range: FiniteRange) -> Option<AxisTransform> {
        if scale == AxisScale::Log10 && range.min() <= 0.0 {
            return None;
        }
        Some(AxisTransform { scale, range })
    }

    #[inline]
    pub fn scale(&self) -> AxisScale {
        self.scale
    }
    #[inline]
    pub fn range(&self) -> FiniteRange {
        self.range
    }

    /// Normalized coordinate in `[0,1]` (or outside it, for out-of-range data).
    /// `None` for a non-finite value, a non-positive value under `Log10`, or any
    /// value whose projection is not finite (e.g. a far out-of-range datum whose
    /// numerator overflows). The output is **guaranteed finite** when `Some` — no
    /// `NaN`/`∞` ever reaches quantization (§4).
    pub fn project(&self, v: f64) -> Option<f64> {
        if !v.is_finite() {
            return None;
        }
        let u = match self.scale {
            AxisScale::Linear => (v - self.range.min()) / self.range.span(),
            AxisScale::Log10 => {
                if v <= 0.0 {
                    return None;
                }
                let lo = self.range.min().log10();
                let hi = self.range.max().log10();
                (v.log10() - lo) / (hi - lo)
            }
        };
        u.is_finite().then_some(u)
    }

    /// Inverse of [`project`](Self::project) for a finite normalized coordinate.
    pub fn unproject(&self, u: f64) -> f64 {
        match self.scale {
            AxisScale::Linear => self.range.min() + u * self.range.span(),
            AxisScale::Log10 => {
                let lo = self.range.min().log10();
                let hi = self.range.max().log10();
                10.0_f64.powf(lo + u * (hi - lo))
            }
        }
    }
}

/// Affine map `[0,1]² → device pixels`, with the `y` axis flipped (data-up maps
/// to screen-down). Pixel units, so it serves both cell and Braille subpixel
/// grids by choosing the scale.
#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    /// Device x of normalized `u = 0`.
    pub ox: f64,
    /// Device y of normalized `v = 1` (the TOP edge; y is flipped).
    pub oy: f64,
    /// Device x-span that `u = 1` maps to (`px → ox + w`). For a discrete grid of
    /// `n` pixels indexed `0..=n-1` where both endpoints are drawable, this is
    /// `n - 1`, so `u = 1` lands on the last valid index (not one past it).
    pub w: f64,
    /// Device y-span that `v = 0` maps to; `n - 1` for an `n`-pixel grid (see `w`).
    pub h: f64,
}

impl Viewport {
    /// `(u, v) ∈ [0,1]² → (px, py)` device pixels, flipping `y`.
    pub fn map(&self, u: f64, v: f64) -> (f64, f64) {
        (self.ox + u * self.w, self.oy + (1.0 - v) * self.h)
    }

    /// Inverse of [`map`](Self::map). Degenerate (`w` or `h` == 0) axes map back
    /// to `0`.
    pub fn unmap(&self, px: f64, py: f64) -> (f64, f64) {
        let u = if self.w != 0.0 {
            (px - self.ox) / self.w
        } else {
            0.0
        };
        let v = if self.h != 0.0 {
            1.0 - (py - self.oy) / self.h
        } else {
            0.0
        };
        (u, v)
    }
}

/// The product `σx × σy` composed with a [`Viewport`]: data `(x, y)` → device
/// pixel `(px, py)`. `project` is `None` whenever either axis rejects its input.
#[derive(Clone, Copy, Debug)]
pub struct PlotTransform2D {
    pub x: AxisTransform,
    pub y: AxisTransform,
    pub viewport: Viewport,
}

impl PlotTransform2D {
    pub fn new(x: AxisTransform, y: AxisTransform, viewport: Viewport) -> PlotTransform2D {
        PlotTransform2D { x, y, viewport }
    }

    /// Project a data point to device pixels, or `None` if either axis rejects
    /// it. Equivalent to `viewport.map(x.project(x)?, y.project(y)?)` (law C).
    pub fn project(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let u = self.x.project(x)?;
        let v = self.y.project(y)?;
        Some(self.viewport.map(u, v))
    }

    /// Inverse of [`project`](Self::project) for a device pixel inside finite
    /// axes. Used for hit-testing / readouts.
    pub fn unproject(&self, px: f64, py: f64) -> (f64, f64) {
        let (u, v) = self.viewport.unmap(px, py);
        (self.x.unproject(u), self.y.unproject(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(min: f64, max: f64) -> FiniteRange {
        FiniteRange::new(min, max).expect("valid range")
    }
    fn lin(min: f64, max: f64) -> AxisTransform {
        AxisTransform::new(AxisScale::Linear, r(min, max)).expect("linear ok")
    }
    fn log(min: f64, max: f64) -> AxisTransform {
        AxisTransform::new(AxisScale::Log10, r(min, max)).expect("log ok")
    }

    // ---- validation / hostile construction (doc §4) ----------------------

    #[test]
    fn range_rejects_nonfinite_and_degenerate() {
        assert!(FiniteRange::new(f64::NAN, 1.0).is_none());
        assert!(FiniteRange::new(0.0, f64::INFINITY).is_none());
        assert!(FiniteRange::new(f64::NEG_INFINITY, 0.0).is_none());
        assert!(FiniteRange::new(1.0, 1.0).is_none(), "collapsed");
        assert!(FiniteRange::new(2.0, 1.0).is_none(), "reversed");
        let ok = FiniteRange::new(-1.0, 3.0).unwrap();
        assert_eq!(ok.span(), 4.0);
    }

    #[test]
    fn log_transform_rejects_nonpositive_range() {
        assert!(AxisTransform::new(AxisScale::Log10, r(-1.0, 10.0)).is_none());
        assert!(AxisTransform::new(AxisScale::Log10, r(0.0, 10.0)).is_none());
        assert!(AxisTransform::new(AxisScale::Log10, r(1.0, 1000.0)).is_some());
        // Linear is fine across zero / negatives.
        assert!(AxisTransform::new(AxisScale::Linear, r(-5.0, 5.0)).is_some());
    }

    // ---- adversary regressions (dalembert audit 2026-10-03) --------------

    #[test]
    fn finite_range_rejects_overflowing_span_break1() {
        // min very negative AND max very positive: max - min overflows f64 to
        // +inf. Such a range must NOT construct — otherwise span() is inf, and a
        // Linear project/unproject produces NaN that reaches raster math (break
        // #1, silent corruption; violates §4 and the span() "always finite"
        // contract).
        assert!(
            FiniteRange::new(-1e308, 1e308).is_none(),
            "overflowing-span range must not construct"
        );
        // And the span of every range that DOES construct is finite & positive.
        for (lo, hi) in [(-1e308, 0.0), (0.0, 1e308), (-1e300, 1e300), (-3.0, 7.0)] {
            if let Some(fr) = FiniteRange::new(lo, hi) {
                assert!(fr.span().is_finite() && fr.span() > 0.0, "span {lo}..{hi}");
            }
        }
    }

    #[test]
    fn project_output_is_finite_or_none_break3() {
        // Finite span (1e308, so the range constructs), but a finite OUT-OF-RANGE
        // datum whose numerator (v - min) overflows. §4 postcondition: project
        // yields Some(finite) or None — never Some(inf/NaN) into quantization
        // (break #3).
        let t =
            AxisTransform::new(AxisScale::Linear, FiniteRange::new(-1e308, 0.0).unwrap()).unwrap();
        match t.project(1e308) {
            None => {}
            Some(u) => assert!(u.is_finite(), "project returned Some({u}), not finite"),
        }
        // In-range values still project to a finite coordinate.
        assert!(t.project(-5e307).map(|u| u.is_finite()).unwrap_or(false));
    }

    // ---- law A: inverse (doc §8.A) ---------------------------------------

    fn assert_close(a: f64, b: f64, tol: f64, what: &str) {
        let err = (a - b).abs();
        let scale = 1.0_f64.max(a.abs()).max(b.abs());
        assert!(err <= tol * scale, "{what}: {a} vs {b} (err {err})");
    }

    #[test]
    fn law_a_linear_inverse() {
        let t = lin(-3.0, 7.0);
        for &x in &[-3.0, -1.5, 0.0, 2.3, 7.0, 100.0, -50.0] {
            let u = t.project(x).unwrap();
            assert_close(t.unproject(u), x, 1e-9, "linear inverse");
        }
    }

    #[test]
    fn law_a_log_inverse() {
        let t = log(1e-3, 1e5);
        for &x in &[1e-3, 1e-2, 0.5, 1.0, 42.0, 1e5, 1e7] {
            let u = t.project(x).unwrap();
            assert_close(t.unproject(u), x, 1e-9, "log inverse");
        }
    }

    // ---- law B: order preservation (doc §8.B) ----------------------------

    #[test]
    fn law_b_order_preservation() {
        for t in [lin(-3.0, 7.0), log(1e-3, 1e5)] {
            let xs = [1e-3, 1e-2, 0.1, 1.0, 2.0, 10.0, 1000.0];
            let mut prev = f64::NEG_INFINITY;
            for &x in &xs {
                if let Some(u) = t.project(x) {
                    assert!(u >= prev, "monotonic: {x} -> {u} < {prev}");
                    prev = u;
                }
            }
        }
    }

    // ---- domain rejection (doc §4) ---------------------------------------

    #[test]
    fn project_rejects_nonfinite_sample() {
        let t = lin(0.0, 1.0);
        assert!(t.project(f64::NAN).is_none());
        assert!(t.project(f64::INFINITY).is_none());
        assert!(t.project(f64::NEG_INFINITY).is_none());
        // ±0 and tiny/huge finite values are accepted (no panic, finite image).
        assert!(t.project(0.0).unwrap().is_finite());
        assert!(t.project(-0.0).unwrap().is_finite());
        assert!(t.project(1e-300).unwrap().is_finite());
        assert!(t.project(1e300).unwrap().is_finite());
    }

    #[test]
    fn log_project_rejects_nonpositive_sample() {
        let t = log(1.0, 1000.0);
        assert!(t.project(0.0).is_none());
        assert!(t.project(-0.0).is_none());
        assert!(t.project(-5.0).is_none());
        assert!(t.project(f64::NAN).is_none());
        assert!(t.project(10.0).unwrap().is_finite());
    }

    // ---- law C: composition (doc §8.C) -----------------------------------

    #[test]
    fn law_c_composition() {
        let x = lin(0.0, 10.0);
        let y = log(1.0, 1000.0);
        let vp = Viewport {
            ox: 4.0,
            oy: 2.0,
            w: 200.0,
            h: 80.0,
        };
        let t = PlotTransform2D::new(x, y, vp);
        for &(dx, dy) in &[(0.0, 1.0), (5.0, 100.0), (10.0, 1000.0), (3.3, 7.0)] {
            let composed = t.project(dx, dy).unwrap();
            let manual = vp.map(x.project(dx).unwrap(), y.project(dy).unwrap());
            assert_close(composed.0, manual.0, 1e-12, "compose px");
            assert_close(composed.1, manual.1, 1e-12, "compose py");
        }
        // either axis rejecting ⇒ whole projection rejected
        assert!(t.project(5.0, 0.0).is_none(), "log y rejects 0");
        assert!(t.project(f64::NAN, 10.0).is_none());
    }

    // ---- viewport y-flip -------------------------------------------------

    #[test]
    fn viewport_y_flip() {
        let vp = Viewport {
            ox: 0.0,
            oy: 10.0,
            w: 100.0,
            h: 50.0,
        };
        let (_, top) = vp.map(0.0, 1.0); // v=1 is the top edge
        let (_, bot) = vp.map(0.0, 0.0); // v=0 is the bottom edge
        assert_eq!(top, 10.0);
        assert_eq!(bot, 60.0);
        assert!(
            top < bot,
            "y flipped: higher data value is higher on screen"
        );
    }

    // ---- law D: quantization error bound (doc §8.D) ----------------------

    #[test]
    fn law_d_quantization_bound() {
        // 100 px over data range [0,10]: one px = 0.1 data units, so a
        // project→round→unproject round trip must stay within half a px = 0.05.
        let x = lin(0.0, 10.0);
        let vp = Viewport {
            ox: 0.0,
            oy: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let t = PlotTransform2D::new(x, lin(0.0, 1.0), vp);
        let half_subcell = 0.5 * x.range().span() / vp.w;
        for &dx in &[0.0, 1.0, 3.3333, 7.77, 9.999] {
            let (px, py) = t.project(dx, 0.5).unwrap();
            let (rx, _) = (px.round(), py.round());
            let (ux, _) = t.unproject(rx, py);
            assert!(
                (ux - dx).abs() <= half_subcell + 1e-12,
                "quantization: {dx} -> px {px} -> {ux} (bound {half_subcell})"
            );
        }
    }
}
