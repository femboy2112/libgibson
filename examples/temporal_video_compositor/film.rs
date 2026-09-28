//! Raw-RGB film store, subpixel sampling, keyed compositing, and foreground
//! bounding-box detection. Depends only on [`super::key`] — no `gibson`, no I/O —
//! so the sampling/bbox/mapping math is unit-testable against synthetic film.
//!
//! A "film" is a decoded, downscaled frame sequence produced offline by ffmpeg
//! (see `bake.rs`): `nframes` frames of `film_w x film_h` packed RGB24. The
//! compositor never touches the source codec; it reads bytes.
//!
//! The projector samples an image at *subpixel* resolution — `2*cols` by `4*rows`
//! for Braille 2x4 — so compositing happens per subpixel: map the subpixel to a
//! film pixel (nearest), key it, and alpha-over the LibGibson background at that
//! same subpixel. The result is an ordinary RGB field the existing projector turns
//! into cells. One renderer.

#![allow(dead_code)]

use super::key::{composite_over, key_pixel, Bbox, KeyParams};

/// A decoded frame sequence in packed RGB24. Frames are concatenated in `data`;
/// frame `i` is the slice `[i * stride .. (i + 1) * stride]` with
/// `stride = film_w * film_h * 3`.
#[derive(Debug, Clone)]
pub struct KeyedFilm {
    pub film_w: u32,
    pub film_h: u32,
    pub fps: f32,
    pub nframes: usize,
    data: Vec<u8>,
}

impl KeyedFilm {
    /// Wrap a raw RGB24 buffer. Returns `None` if the length is not a whole number
    /// of `film_w * film_h * 3`-byte frames (a truncated bake is a bug, not silent
    /// garbage). Empty input is rejected too: a 0-frame film is a corpse that only
    /// panics later in `frame(0)`, so success here guarantees `nframes >= 1`.
    pub fn from_raw(data: Vec<u8>, film_w: u32, film_h: u32, fps: f32) -> Option<Self> {
        let stride = (film_w as usize) * (film_h as usize) * 3;
        // Empty data slips past the modulo check (0 % stride == 0); refuse it up
        // front so no caller ever holds a film with nothing to show.
        if stride == 0 || data.is_empty() || data.len() % stride != 0 {
            return None;
        }
        let nframes = data.len() / stride;
        Some(Self {
            film_w,
            film_h,
            fps,
            nframes,
            data,
        })
    }

    #[inline]
    fn stride(&self) -> usize {
        (self.film_w as usize) * (self.film_h as usize) * 3
    }

    /// The packed RGB24 bytes of frame `i` (clamped to the last frame).
    #[inline]
    pub fn frame(&self, i: usize) -> &[u8] {
        let i = i.min(self.nframes.saturating_sub(1));
        let s = self.stride();
        &self.data[i * s..(i + 1) * s]
    }

    /// Nearest-neighbour sample of a film frame at integer film coordinates
    /// (clamped into range). Nearest is deliberate: at terminal-cell resolution it
    /// is sharp and, more importantly, deterministic and allocation-free.
    #[inline]
    pub fn sample(&self, frame: &[u8], fx: u32, fy: u32) -> [u8; 3] {
        let fx = fx.min(self.film_w - 1) as usize;
        let fy = fy.min(self.film_h - 1) as usize;
        let o = (fy * self.film_w as usize + fx) * 3;
        [frame[o], frame[o + 1], frame[o + 2]]
    }

    /// Map a subpixel coordinate in a `subw x subh` grid to a film pixel (nearest).
    #[inline]
    pub fn map_subpixel(&self, subw: u32, subh: u32, lx: u32, ly: u32) -> (u32, u32) {
        // Center-of-cell mapping keeps the image from biasing toward the top-left.
        let fx = ((lx as u64 * 2 + 1) * self.film_w as u64) / (2 * subw as u64);
        let fy = ((ly as u64 * 2 + 1) * self.film_h as u64) / (2 * subh as u64);
        (fx as u32, fy as u32)
    }

    /// Composite one subpixel: key the film sample and alpha-over `bg`. This is the
    /// exact value fed to the projector for subpixel `(lx, ly)`.
    #[inline]
    #[allow(clippy::too_many_arguments)] // grid dims + coord + bg + params are all needed inputs
    pub fn composite_subpixel(
        &self,
        frame: &[u8],
        subw: u32,
        subh: u32,
        lx: u32,
        ly: u32,
        bg: [u8; 3],
        p: &KeyParams,
    ) -> [u8; 3] {
        let (fx, fy) = self.map_subpixel(subw, subh, lx, ly);
        let (fg, alpha) = key_pixel(self.sample(frame, fx, fy), p);
        composite_over(fg, alpha, bg)
    }

    /// The tight foreground bounding box (opaque pixels, alpha > 0) of a frame, in
    /// FILM pixels, together with the opaque pixel count. Empty box if the frame is
    /// pure key. This is the dirty region: only cells overlapping it need
    /// reprojection each source frame, and the count gives honest key-coverage.
    pub fn foreground_stats(&self, frame: &[u8], p: &KeyParams) -> (Bbox, usize) {
        let (w, h) = (self.film_w, self.film_h);
        let mut x0 = w;
        let mut y0 = h;
        let mut x1 = 0u32;
        let mut y1 = 0u32;
        let mut opaque = 0usize;
        for y in 0..h {
            let row = (y * w) as usize * 3;
            for x in 0..w {
                let o = row + x as usize * 3;
                let px = [frame[o], frame[o + 1], frame[o + 2]];
                if super::key::alpha_of(px, p) > 0.0 {
                    opaque += 1;
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                    y1 = y1.max(y + 1);
                }
            }
        }
        let bbox = if x1 <= x0 || y1 <= y0 {
            Bbox::EMPTY
        } else {
            Bbox {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            }
        };
        (bbox, opaque)
    }

    /// The tight foreground bounding box (see [`Self::foreground_stats`]).
    pub fn foreground_bbox(&self, frame: &[u8], p: &KeyParams) -> Bbox {
        self.foreground_stats(frame, p).0
    }

    /// Convert a FILM-pixel bbox to a CELL rectangle `(x, y, w, h)` covering every
    /// cell the region touches (ceil on the far edge, clamped to the grid). This is
    /// the argument to `TemporalDisplayProcessor::set_target_region`.
    pub fn bbox_to_cells(&self, fb: &Bbox, cells_w: u16, cells_h: u16) -> (u16, u16, u16, u16) {
        if fb.is_empty() || cells_w == 0 || cells_h == 0 {
            return (0, 0, 0, 0);
        }
        let cw = cells_w as u64;
        let ch = cells_h as u64;
        let cx0 = (fb.x as u64 * cw) / self.film_w as u64;
        let cy0 = (fb.y as u64 * ch) / self.film_h as u64;
        // Ceil the exclusive far edge so a partially-covered cell is included.
        let cx1 = (fb.right() as u64 * cw).div_ceil(self.film_w as u64);
        let cy1 = (fb.bottom() as u64 * ch).div_ceil(self.film_h as u64);
        let cx0 = cx0.min(cw) as u16;
        let cy0 = cy0.min(ch) as u16;
        let cx1 = cx1.min(cw) as u16;
        let cy1 = cy1.min(ch) as u16;
        (cx0, cy0, cx1.saturating_sub(cx0), cy1.saturating_sub(cy0))
    }

    /// Convert a FILM-pixel bbox to a SUBPIXEL bbox in a `subw x subh` grid (the
    /// reticle tracks the subject in subpixel space). Ceil on the far edge.
    pub fn bbox_to_sub(&self, fb: &Bbox, subw: u32, subh: u32) -> Bbox {
        if fb.is_empty() {
            return Bbox::EMPTY;
        }
        let sx0 = (fb.x as u64 * subw as u64) / self.film_w as u64;
        let sy0 = (fb.y as u64 * subh as u64) / self.film_h as u64;
        let sx1 = (fb.right() as u64 * subw as u64).div_ceil(self.film_w as u64);
        let sy1 = (fb.bottom() as u64 * subh as u64).div_ceil(self.film_h as u64);
        Bbox {
            x: sx0.min(subw as u64) as u32,
            y: sy0.min(subh as u64) as u32,
            w: (sx1.min(subw as u64) as u32).saturating_sub(sx0 as u32),
            h: (sy1.min(subh as u64) as u32).saturating_sub(sy0 as u32),
        }
    }

    /// Build a synthetic film for tests: a pure-key green field with one opaque
    /// rectangle of `fill` colour, moving `dx`/`dy` pixels per frame. Deterministic,
    /// no ffmpeg. `rect` is the frame-0 rectangle in film pixels.
    pub fn synthetic(
        film_w: u32,
        film_h: u32,
        nframes: usize,
        rect: Bbox,
        dx: i32,
        dy: i32,
        fill: [u8; 3],
    ) -> Self {
        let key = [0u8, 254, 2]; // measured clean-key green
        let stride = (film_w * film_h * 3) as usize;
        let mut data = vec![0u8; stride * nframes];
        for f in 0..nframes {
            let base = f * stride;
            // Fill with key green.
            for p in data[base..base + stride].chunks_exact_mut(3) {
                p[0] = key[0];
                p[1] = key[1];
                p[2] = key[2];
            }
            // Place the (translated) opaque rectangle.
            let rx = rect.x as i64 + dx as i64 * f as i64;
            let ry = rect.y as i64 + dy as i64 * f as i64;
            for yy in 0..rect.h as i64 {
                let y = ry + yy;
                if y < 0 || y >= film_h as i64 {
                    continue;
                }
                for xx in 0..rect.w as i64 {
                    let x = rx + xx;
                    if x < 0 || x >= film_w as i64 {
                        continue;
                    }
                    let o = base + (y as usize * film_w as usize + x as usize) * 3;
                    data[o] = fill[0];
                    data[o + 1] = fill[1];
                    data[o + 2] = fill[2];
                }
            }
        }
        Self {
            film_w,
            film_h,
            fps: 29.0,
            nframes,
            data,
        }
    }
}
