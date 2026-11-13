//! Pixel fitting for [`super::element::image`]: nearest-pixel resampling into a
//! bounded box, aspect preserved, never enlarged. Work and storage are bounded by
//! the destination box, not by the source.
use crate::raster::{Rgb, RgbRaster};

/// Pixels already fitted to a box; at most `box_w × box_h` of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Fitted {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<Rgb>,
}

/// The largest `(w, h)` with the source aspect ratio that fits `box_w × box_h`
/// without enlarging (scale ≤ 1). Zero when either side is empty.
pub(crate) fn fit_dims(src_w: u16, src_h: u16, box_w: u16, box_h: u16) -> (u16, u16) {
    if src_w == 0 || src_h == 0 || box_w == 0 || box_h == 0 {
        return (0, 0);
    }
    let (sw, sh, bw, bh) = (
        u64::from(src_w),
        u64::from(src_h),
        u64::from(box_w),
        u64::from(box_h),
    );
    if sw <= bw && sh <= bh {
        return (src_w, src_h);
    }
    // Scale = min(bw/sw, bh/sh), decided exactly by cross-multiplication.
    let (w, h) = if bw * sh <= bh * sw {
        (bw, (sh * bw / sw).max(1))
    } else {
        ((sw * bh / sh).max(1), bh)
    };
    (w.min(bw) as u16, h.min(bh) as u16)
}

impl Fitted {
    /// Resamples `width × height` pixels read through `get` into the fitted size.
    /// Calls `get` exactly once per output pixel.
    fn resample(
        width: u16,
        height: u16,
        box_w: u16,
        box_h: u16,
        get: impl Fn(u16, u16) -> Rgb,
    ) -> Self {
        let (w, h) = fit_dims(width, height, box_w, box_h);
        let mut pixels = Vec::with_capacity(usize::from(w) * usize::from(h));
        for y in 0..h {
            // Centre of output pixel y, mapped into the source: (2y + 1)·H / 2h.
            let sy = ((2 * u64::from(y) + 1) * u64::from(height) / (2 * u64::from(h))) as u16;
            for x in 0..w {
                let sx = ((2 * u64::from(x) + 1) * u64::from(width) / (2 * u64::from(w))) as u16;
                pixels.push(get(sx.min(width - 1), sy.min(height - 1)));
            }
        }
        Self {
            width: w,
            height: h,
            pixels,
        }
    }

    pub fn from_raster(raster: &RgbRaster, box_w: u16, box_h: u16) -> Self {
        let (width, height) = (raster.width(), raster.height());
        let src = raster.pixels();
        Self::resample(width, height, box_w, box_h, |x, y| {
            src[usize::from(y) * usize::from(width) + usize::from(x)]
        })
    }

    /// Like [`Self::from_raster`] but with an exact area (box) filter: every
    /// source pixel contributes, so painted or photographic art does not alias.
    /// Reads every source pixel (work `O(source + box)`); stores only the fit.
    pub fn from_raster_area(raster: &RgbRaster, box_w: u16, box_h: u16) -> Self {
        let (w, h) = fit_dims(raster.width(), raster.height(), box_w, box_h);
        if (w, h) == (raster.width(), raster.height()) {
            return Self::from_raster(raster, box_w, box_h);
        }
        let src = raster.pixels();
        let out = crate::raster::downscale_area_by(
            raster.width(),
            raster.height(),
            |i| {
                let (r, g, b) = src[i];
                [r, g, b, 255]
            },
            w,
            h,
        );
        Self {
            width: out.width(),
            height: out.height(),
            pixels: out.pixels().iter().map(|&[r, g, b, _]| (r, g, b)).collect(),
        }
    }

    /// Fits these pixels into a (possibly smaller) box. Never enlarges.
    pub fn refit(&self, box_w: u16, box_h: u16) -> Self {
        if self.width <= box_w && self.height <= box_h {
            return self.clone();
        }
        let w = self.width;
        Self::resample(self.width, self.height, box_w, box_h, |x, y| {
            self.pixels[usize::from(y) * usize::from(w) + usize::from(x)]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_preserves_aspect_and_never_enlarges() {
        assert_eq!(fit_dims(640, 480, 40, 24), (32, 24));
        assert_eq!(fit_dims(10, 10, 40, 24), (10, 10));
        assert_eq!(fit_dims(1000, 10, 40, 24), (40, 1));
        assert_eq!(fit_dims(10, 1000, 40, 24), (1, 24));
        assert_eq!(fit_dims(0, 5, 40, 24), (0, 0));
        assert_eq!(fit_dims(5, 5, 0, 24), (0, 0));
    }

    #[test]
    fn resample_reads_each_output_pixel_once() {
        use std::cell::Cell;
        let reads = Cell::new(0usize);
        let f = Fitted::resample(2048, 2048, 40, 24, |_, _| {
            reads.set(reads.get() + 1);
            (0, 0, 0)
        });
        assert_eq!((f.width, f.height), (24, 24));
        assert_eq!(reads.get(), 24 * 24);
        assert_eq!(f.pixels.capacity(), 24 * 24);
    }

    #[test]
    fn area_fit_averages_where_nearest_aliases() {
        // 1-pixel black/white stripes: nearest picks one colour, area gives grey.
        let mut r = RgbRaster::new(64, 2);
        for x in 0..64 {
            let v = if x % 2 == 0 { 0 } else { 255 };
            r.set(x, 0, (v, v, v));
            r.set(x, 1, (v, v, v));
        }
        let near = Fitted::from_raster(&r, 8, 24);
        let area = Fitted::from_raster_area(&r, 8, 24);
        assert_eq!((area.width, area.height), (near.width, near.height));
        assert!(near.pixels.iter().all(|p| p.0 == 0 || p.0 == 255));
        assert!(area.pixels.iter().all(|p| p.0 == 128), "{:?}", area.pixels);
    }

    #[test]
    fn identity_fit_copies_pixels() {
        let mut r = RgbRaster::new(3, 2);
        r.set(2, 1, (9, 8, 7));
        let f = Fitted::from_raster(&r, 40, 24);
        assert_eq!((f.width, f.height), (3, 2));
        assert_eq!(f.pixels[5], (9, 8, 7));
    }
}
