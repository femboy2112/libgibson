//! Straight-alpha RGBA rasters: ingest, exact compositing over an [`RgbRaster`],
//! and area-filtered downscaling that does not fringe at soft alpha edges.
//!
//! All arithmetic is integer and exact (no floating point), so results are
//! identical on every host. Work is bounded by the pixels involved: compositing
//! touches only the overlap; downscaling visits each source pixel a bounded
//! number of times.
use super::{FrameError, Rgb, RgbRaster, MAX_RASTER_DIMENSION};

/// One straight (non-premultiplied) RGBA pixel: `[r, g, b, a]`, `a = 0` fully
/// transparent, `a = 255` opaque.
pub type Rgba = [u8; 4];

/// An owned straight-alpha RGBA raster, at most [`MAX_RASTER_DIMENSION`] per side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaRaster {
    width: u16,
    height: u16,
    pixels: Vec<Rgba>,
}

impl RgbaRaster {
    /// A fully transparent raster; dimensions are clamped to [`MAX_RASTER_DIMENSION`].
    pub fn new(width: u16, height: u16) -> Self {
        let width = width.min(MAX_RASTER_DIMENSION);
        let height = height.min(MAX_RASTER_DIMENSION);
        Self {
            width,
            height,
            pixels: vec![[0; 4]; usize::from(width) * usize::from(height)],
        }
    }

    /// Ingests packed straight-alpha RGBA bytes (`stride` bytes per row, at
    /// least `width × 4`). Bad input is rejected, never clamped, exactly as
    /// [`RgbRaster::from_rgb_bytes`] rejects it.
    pub fn from_rgba_bytes(
        width: u16,
        height: u16,
        stride: usize,
        bytes: &[u8],
    ) -> Result<Self, FrameError> {
        if width == 0 || height == 0 {
            return Err(FrameError::ZeroDimension { width, height });
        }
        if width > MAX_RASTER_DIMENSION || height > MAX_RASTER_DIMENSION {
            return Err(FrameError::TooLarge {
                width,
                height,
                max: MAX_RASTER_DIMENSION,
            });
        }
        let min_row_bytes = usize::from(width) * 4;
        if stride < min_row_bytes {
            return Err(FrameError::StrideTooSmall {
                stride,
                min_row_bytes,
            });
        }
        let need = stride
            .checked_mul(usize::from(height))
            .ok_or(FrameError::DimensionsOverflow { stride, height })?;
        if bytes.len() < need {
            return Err(FrameError::ShortBuffer {
                have: bytes.len(),
                need,
            });
        }
        let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height));
        for row in 0..usize::from(height) {
            let start = row * stride;
            pixels.extend(
                bytes[start..start + min_row_bytes]
                    .chunks_exact(4)
                    .map(|p| [p[0], p[1], p[2], p[3]]),
            );
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Takes ownership of `width × height` straight-alpha pixels (row-major)
    /// without copying. Rejects a zero or oversize dimension, or a pixel count
    /// that does not match; the vector is returned untouched inside the error.
    pub fn from_pixels(
        width: u16,
        height: u16,
        pixels: Vec<Rgba>,
    ) -> Result<Self, (FrameError, Vec<Rgba>)> {
        if width == 0 || height == 0 {
            return Err((FrameError::ZeroDimension { width, height }, pixels));
        }
        if width > MAX_RASTER_DIMENSION || height > MAX_RASTER_DIMENSION {
            return Err((
                FrameError::TooLarge {
                    width,
                    height,
                    max: MAX_RASTER_DIMENSION,
                },
                pixels,
            ));
        }
        let need = usize::from(width) * usize::from(height);
        if pixels.len() != need {
            return Err((
                FrameError::ShortBuffer {
                    have: pixels.len(),
                    need,
                },
                pixels,
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// An opaque copy of `rgb` (alpha 255 everywhere).
    pub fn from_rgb(rgb: &RgbRaster) -> Self {
        Self {
            width: rgb.width(),
            height: rgb.height(),
            pixels: rgb
                .pixels()
                .iter()
                .map(|&(r, g, b)| [r, g, b, 255])
                .collect(),
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }
    pub fn height(&self) -> u16 {
        self.height
    }
    pub fn pixels(&self) -> &[Rgba] {
        &self.pixels
    }
    pub fn pixels_mut(&mut self) -> &mut [Rgba] {
        &mut self.pixels
    }
    pub fn get(&self, x: i32, y: i32) -> Option<Rgba> {
        self.index(x, y).map(|i| self.pixels[i])
    }
    /// Writes one pixel; out-of-bounds writes are ignored.
    pub fn set(&mut self, x: i32, y: i32, color: Rgba) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = color;
        }
    }
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < i32::from(self.width) && y < i32::from(self.height))
            .then(|| y as usize * usize::from(self.width) + x as usize)
    }
}

/// `(v + 127) / 255` for `v ≤ 255·255`: exact rounding of `v / 255`.
fn div255(v: u32) -> u8 {
    ((v + 127) / 255) as u8
}

/// Composites `src` over `dst` with its top-left at `(x, y)` (either may be
/// negative or past the edge; only the overlap is touched):
/// `out = round((src·a + dst·(255 − a)) / 255)` per channel, exact in integers.
/// Fully transparent pixels leave `dst` unchanged; opaque pixels replace it.
pub fn composite_over(dst: &mut RgbRaster, src: &RgbaRaster, x: i32, y: i32) {
    let (dw, dh) = (i64::from(dst.width()), i64::from(dst.height()));
    let (sw, sh) = (i64::from(src.width), i64::from(src.height));
    let (x, y) = (i64::from(x), i64::from(y));
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + sw).min(dw);
    let y1 = (y + sh).min(dh);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let dwu = dw as usize;
    let swu = sw as usize;
    let out = dst.pixels_mut();
    for dy in y0..y1 {
        let sy = (dy - y) as usize;
        for dx in x0..x1 {
            let sx = (dx - x) as usize;
            let [r, g, b, a] = src.pixels[sy * swu + sx];
            let d = &mut out[dy as usize * dwu + dx as usize];
            match a {
                0 => {}
                255 => *d = (r, g, b),
                a => {
                    let (a, na) = (u32::from(a), 255 - u32::from(a));
                    *d = (
                        div255(u32::from(r) * a + u32::from(d.0) * na),
                        div255(u32::from(g) * a + u32::from(d.1) * na),
                        div255(u32::from(b) * a + u32::from(d.2) * na),
                    );
                }
            }
        }
    }
}

/// Flattens `src` over a solid `background` into a new [`RgbRaster`].
pub fn flatten(src: &RgbaRaster, background: Rgb) -> RgbRaster {
    let mut out = RgbRaster::new(src.width, src.height);
    out.clear(background);
    composite_over(&mut out, src, 0, 0);
    out
}

/// Resamples `src` to `width × height` with an exact area (box) filter: each
/// output pixel averages the source area it covers, with fractional coverage at
/// its edges. Colour is averaged **premultiplied by alpha**, so transparent
/// pixels contribute no colour and soft alpha edges keep their true colour
/// instead of fringing toward whatever the transparent pixels held.
///
/// A fully transparent output pixel is `[0, 0, 0, 0]` (it has no colour).
/// Integer arithmetic throughout, rounded once at the end. Each source pixel
/// contributes to at most two output columns and two output rows when shrinking,
/// so work is `O(src.width × src.height + width × height)`. Intended for
/// shrinking; enlarging works but is plain box sampling. An empty source or
/// target yields an empty raster; the target is clamped to
/// [`MAX_RASTER_DIMENSION`].
pub fn downscale_area(src: &RgbaRaster, width: u16, height: u16) -> RgbaRaster {
    downscale_area_by(src.width, src.height, |i| src.pixels[i], width, height)
}

/// [`downscale_area`] over any row-major pixel source of `src_w × src_h`, read
/// through `px(index)`; lets an opaque [`RgbRaster`] be filtered without first
/// copying it into an [`RgbaRaster`].
pub(crate) fn downscale_area_by(
    src_w: u16,
    src_h: u16,
    px: impl Fn(usize) -> Rgba,
    width: u16,
    height: u16,
) -> RgbaRaster {
    let width = width.min(MAX_RASTER_DIMENSION);
    let height = height.min(MAX_RASTER_DIMENSION);
    if src_w == 0 || src_h == 0 || width == 0 || height == 0 {
        return RgbaRaster::new(width.min(src_w), height.min(src_h));
    }
    let (sw, sh) = (u64::from(src_w), u64::from(src_h));
    let (tw, th) = (u64::from(width), u64::from(height));
    // Spans along one axis: in units where a source pixel is `t` long and an
    // output pixel is `s` long, output `o` covers [o·s, (o+1)·s) and source `i`
    // covers [i·t, (i+1)·t). Weights are the integer overlap lengths; per output
    // pixel they sum to `s`.
    fn spans(s: u64, t: u64) -> Vec<Vec<(usize, u64)>> {
        (0..t)
            .map(|o| {
                let (lo, hi) = (o * s, (o + 1) * s);
                (lo / t..hi.div_ceil(t))
                    .filter_map(|i| {
                        let w = hi.min((i + 1) * t) - lo.max(i * t);
                        (w > 0).then_some((i as usize, w))
                    })
                    .collect()
            })
            .collect()
    }
    let xs = spans(sw, tw);
    let ys = spans(sh, th);
    // Horizontal pass: per (output x, source row) sums of [a·r, a·g, a·b, a],
    // weighted (total weight sw).
    let mut rows = vec![[0u64; 4]; (tw * sh) as usize];
    for sy in 0..sh as usize {
        let base = sy * sw as usize;
        for (ox, span) in xs.iter().enumerate() {
            let acc = &mut rows[sy * tw as usize + ox];
            for &(sx, w) in span {
                let [r, g, b, a] = px(base + sx);
                let a = u64::from(a);
                acc[0] += w * a * u64::from(r);
                acc[1] += w * a * u64::from(g);
                acc[2] += w * a * u64::from(b);
                acc[3] += w * a;
            }
        }
    }
    // Vertical pass (total weight sh), then un-premultiply with one rounding.
    let area = sw * sh;
    let mut out = RgbaRaster::new(width, height);
    for (oy, span) in ys.iter().enumerate() {
        for ox in 0..tw as usize {
            let mut acc = [0u64; 4];
            for &(sy, w) in span {
                let row = rows[sy * tw as usize + ox];
                for c in 0..4 {
                    acc[c] += w * row[c];
                }
            }
            let alpha_sum = acc[3];
            out.pixels[oy * tw as usize + ox] = if alpha_sum == 0 {
                [0, 0, 0, 0]
            } else {
                let unpremul = |v: u64| ((2 * v + alpha_sum) / (2 * alpha_sum)) as u8;
                [
                    unpremul(acc[0]),
                    unpremul(acc[1]),
                    unpremul(acc[2]),
                    ((2 * alpha_sum + area) / (2 * area)) as u8,
                ]
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic test card: horizontal colour gradient under a feathered alpha
    /// disc (opaque core, linear 8-pixel falloff, transparent outside).
    fn card(w: u16, h: u16) -> RgbaRaster {
        let mut r = RgbaRaster::new(w, h);
        let (cx, cy) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
        let radius = f64::from(w.min(h)) * 0.4;
        for y in 0..h {
            for x in 0..w {
                let d =
                    ((f64::from(x) + 0.5 - cx).powi(2) + (f64::from(y) + 0.5 - cy).powi(2)).sqrt();
                let a = ((radius - d) / 8.0).clamp(0.0, 1.0);
                let t = (u32::from(x) * 255 / u32::from(w.max(2) - 1)) as u8;
                r.set(
                    x.into(),
                    y.into(),
                    [t, 64, 255 - t, (a * 255.0).round() as u8],
                );
            }
        }
        r
    }

    #[test]
    fn ingest_matches_rgb_rules() {
        assert_eq!(
            RgbaRaster::from_rgba_bytes(0, 1, 4, &[0; 4]),
            Err(FrameError::ZeroDimension {
                width: 0,
                height: 1
            })
        );
        assert_eq!(
            RgbaRaster::from_rgba_bytes(2, 1, 7, &[0; 8]),
            Err(FrameError::StrideTooSmall {
                stride: 7,
                min_row_bytes: 8
            })
        );
        assert_eq!(
            RgbaRaster::from_rgba_bytes(1, 2, 5, &[0; 9]),
            Err(FrameError::ShortBuffer { have: 9, need: 10 })
        );
        let r = RgbaRaster::from_rgba_bytes(
            2,
            2,
            9,
            &[1, 2, 3, 4, 5, 6, 7, 8, 0, 9, 10, 11, 12, 13, 14, 15, 16, 0],
        )
        .unwrap();
        assert_eq!(r.get(1, 0), Some([5, 6, 7, 8]));
        assert_eq!(r.get(0, 1), Some([9, 10, 11, 12]));
    }

    #[test]
    fn composite_is_exact_and_clips_negative_offsets() {
        let mut dst = RgbRaster::new(4, 3);
        dst.clear((100, 100, 100));
        let mut src = RgbaRaster::new(3, 3);
        src.set(0, 0, [200, 0, 50, 255]); // opaque: replaces
        src.set(1, 0, [200, 0, 50, 0]); // transparent: unchanged
        src.set(2, 0, [200, 0, 50, 128]); // half
        src.set(2, 2, [0, 0, 0, 255]);
        composite_over(&mut dst, &src, -1, 0);
        // src x=1 lands on dst x=0, x=2 on dst x=1; src x=0 is clipped away.
        assert_eq!(dst.get(0, 0), Some((100, 100, 100)));
        let half = |s: u32, d: u32| ((s * 128 + d * 127 + 127) / 255) as u8;
        assert_eq!(
            dst.get(1, 0),
            Some((half(200, 100), half(0, 100), half(50, 100)))
        );
        assert_eq!(dst.get(1, 2), Some((0, 0, 0)));
        assert_eq!(dst.get(2, 0), Some((100, 100, 100)), "outside src");
        // Entirely off-surface in every direction: no change, no panic.
        let before = dst.clone();
        for (x, y) in [(-3, 0), (4, 0), (0, -3), (0, 3), (i32::MIN, i32::MAX)] {
            composite_over(&mut dst, &src, x, y);
        }
        assert_eq!(dst, before);
    }

    #[test]
    fn transparent_pixels_never_tint_a_downscale() {
        // Opaque red next to fully transparent GREEN: a straight-alpha average
        // would fringe toward green; premultiplied keeps pure red, half alpha.
        let mut src = RgbaRaster::new(2, 1);
        src.set(0, 0, [255, 0, 0, 255]);
        src.set(1, 0, [0, 255, 0, 0]);
        let out = downscale_area(&src, 1, 1);
        assert_eq!(out.get(0, 0), Some([255, 0, 0, 128]));
    }

    #[test]
    fn area_filter_handles_fractional_coverage_exactly() {
        // 3 → 2: output 0 covers source 0 fully and half of source 1.
        let mut src = RgbaRaster::new(3, 1);
        src.set(0, 0, [0, 0, 0, 255]);
        src.set(1, 0, [90, 90, 90, 255]);
        src.set(2, 0, [180, 180, 180, 255]);
        let out = downscale_area(&src, 2, 1);
        // (0·2 + 90·1) / 3 = 30; (90·1 + 180·2) / 3 = 150.
        assert_eq!(out.get(0, 0), Some([30, 30, 30, 255]));
        assert_eq!(out.get(1, 0), Some([150, 150, 150, 255]));
    }

    #[test]
    fn downscale_preserves_coverage_and_identity() {
        let c = card(384, 672);
        // 1:1 is the identity, except that a fully transparent pixel has no
        // colour once premultiplied and comes out as [0, 0, 0, 0].
        let mut normalised = c.clone();
        for p in normalised.pixels_mut() {
            if p[3] == 0 {
                *p = [0; 4];
            }
        }
        assert!(
            downscale_area(&c, 384, 672) == normalised,
            "1:1 is the identity"
        );
        let small = downscale_area(&c, 48, 84);
        let total = |r: &RgbaRaster, scale: u64| {
            r.pixels().iter().map(|p| u64::from(p[3])).sum::<u64>() * scale
        };
        // Mean alpha is preserved up to per-pixel rounding (≤ 0.5 per pixel).
        let (big, little) = (total(&c, 1), total(&small, 64));
        let tolerance = 64 * small.pixels().len() as u64 / 2;
        assert!(big.abs_diff(little) <= tolerance, "{big} vs {little}");
        // Every colour channel of a partly opaque output pixel stays inside the
        // range of the opaque-ish sources it came from: no green/black fringe.
        for p in small.pixels().iter().filter(|p| p[3] > 0) {
            assert_eq!(
                p[1], 64,
                "constant channel never drifts at soft edges: {p:?}"
            );
        }
    }

    #[test]
    fn empty_and_huge_requests_are_safe() {
        let c = card(16, 16);
        assert_eq!(downscale_area(&c, 0, 5).pixels().len(), 0);
        assert_eq!(
            downscale_area(&RgbaRaster::new(0, 0), 5, 5).pixels().len(),
            0
        );
        let big = downscale_area(&c, 4000, 1);
        assert_eq!(big.width(), MAX_RASTER_DIMENSION);
    }

    #[test]
    fn from_pixels_takes_ownership_and_validates() {
        let px = vec![[1, 2, 3, 4]; 6];
        let ptr = px.as_ptr();
        let r = RgbaRaster::from_pixels(3, 2, px).unwrap();
        assert_eq!(r.pixels().as_ptr(), ptr, "no copy");
        let (e, back) = RgbaRaster::from_pixels(4, 2, vec![[0; 4]; 6]).unwrap_err();
        assert_eq!(e, FrameError::ShortBuffer { have: 6, need: 8 });
        assert_eq!(back.len(), 6);
        assert!(matches!(
            RgbaRaster::from_pixels(4096, 1, vec![]).unwrap_err().0,
            FrameError::TooLarge { .. }
        ));
    }

    #[test]
    fn flatten_over_background() {
        let mut src = RgbaRaster::new(2, 1);
        src.set(0, 0, [10, 20, 30, 255]);
        let out = flatten(&src, (7, 8, 9));
        assert_eq!(out.get(0, 0), Some((10, 20, 30)));
        assert_eq!(out.get(1, 0), Some((7, 8, 9)));
    }
}
