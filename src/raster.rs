//! Experimental opaque RGB subcell framebuffer. Software blending happens here,
//! before conversion to ordinary Unicode cells; it is not terminal alpha.
use crate::canvas::clip_line_to_bounds;
use crate::cell::{Cell, Color, Glyph, Style};
use crate::surface::Surface;
use std::io::{self, Write};

pub type Rgb = (u8, u8, u8);
/// Hard allocation bound per pixel axis, not a recommended operating size.
/// Normal terminal rasters are orders of magnitude smaller than this ceiling.
pub const MAX_RASTER_DIMENSION: u16 = 2048;

/// Why a caller-supplied byte buffer could not be ingested as RGB pixels (#129).
///
/// Ingest **rejects** bad input rather than clamping or truncating it: silently
/// clamping a caller's declared dimensions (the way [`RgbRaster::new`] does for
/// convenience) would be a lie about what the caller actually handed over. Every
/// variant carries the numbers needed to see exactly what was wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameError {
    /// A dimension was zero: an empty frame has no pixels to ingest.
    ZeroDimension { width: u16, height: u16 },
    /// A dimension exceeded [`MAX_RASTER_DIMENSION`] (rejected, not clamped).
    TooLarge { width: u16, height: u16, max: u16 },
    /// The row stride (bytes per row) is smaller than the `width * 3` a packed RGB
    /// row needs.
    StrideTooSmall { stride: usize, min_row_bytes: usize },
    /// `stride * height` overflowed `usize`: the declared frame cannot be addressed.
    DimensionsOverflow { stride: usize, height: u16 },
    /// The buffer is shorter than `stride * height`: it cannot hold the declared frame.
    ShortBuffer { have: usize, need: usize },
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDimension { width, height } => {
                write!(f, "frame has a zero dimension: {width}x{height}")
            }
            Self::TooLarge { width, height, max } => {
                write!(
                    f,
                    "frame {width}x{height} exceeds the {max} pixel-per-axis limit"
                )
            }
            Self::StrideTooSmall {
                stride,
                min_row_bytes,
            } => write!(
                f,
                "row stride {stride} is smaller than the {min_row_bytes} bytes a packed row needs"
            ),
            Self::DimensionsOverflow { stride, height } => {
                write!(f, "stride {stride} * height {height} overflows usize")
            }
            Self::ShortBuffer { have, need } => {
                write!(f, "buffer holds {have} bytes but the frame needs {need}")
            }
        }
    }
}

impl std::error::Error for FrameError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbRaster {
    width: u16,
    height: u16,
    pixels: Vec<Rgb>,
}
impl RgbRaster {
    /// Pixel dimensions, clamped to [`MAX_RASTER_DIMENSION`]. A terminal of
    /// `w × h` cells normally uses `new(w, 2*h)`.
    pub fn new(width: u16, height: u16) -> Self {
        let width = width.min(MAX_RASTER_DIMENSION);
        let height = height.min(MAX_RASTER_DIMENSION);
        Self {
            width,
            height,
            pixels: vec![(0, 0, 0); width as usize * height as usize],
        }
    }
    pub fn width(&self) -> u16 {
        self.width
    }
    pub fn height(&self) -> u16 {
        self.height
    }
    pub fn pixels(&self) -> &[Rgb] {
        &self.pixels
    }
    pub fn pixels_mut(&mut self) -> &mut [Rgb] {
        &mut self.pixels
    }
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < i32::from(self.width) && y < i32::from(self.height))
            .then(|| y as usize * self.width as usize + x as usize)
    }
    pub fn get(&self, x: i32, y: i32) -> Option<Rgb> {
        self.index(x, y).map(|i| self.pixels[i])
    }
    pub fn set(&mut self, x: i32, y: i32, color: Rgb) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = color;
        }
    }
    pub fn clear(&mut self, color: Rgb) {
        self.pixels.fill(color);
    }
    /// Linear per-channel interpolation of stored 8-bit RGB values, not linear-light
    /// color-space interpolation. Nonfinite amounts are ignored; finite amounts clamp.
    pub fn blend(&mut self, x: i32, y: i32, color: Rgb, amount: f32) {
        if !amount.is_finite() {
            return;
        }
        if let Some(old) = self.get(x, y) {
            let t = amount.clamp(0., 1.);
            let c = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
            self.set(
                x,
                y,
                (c(old.0, color.0), c(old.1, color.1), c(old.2, color.2)),
            );
        }
    }
    /// Clipped Bresenham: work is bounded by raster dimensions, even for i32 extremes.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgb) {
        let Some(((mut x, mut y), (x1, y1))) = clip_line_to_bounds(
            x0,
            y0,
            x1,
            y1,
            self.width as i32 - 1,
            self.height as i32 - 1,
        ) else {
            return;
        };
        let dx = (x1 - x).abs();
        let sx = if x < x1 { 1 } else { -1 };
        let dy = -(y1 - y).abs();
        let sy = if y < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.set(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e = 2 * error;
            if e >= dy {
                error += dy;
                x += sx;
            }
            if e <= dx {
                error += dx;
                y += sy;
            }
        }
    }
    /// Filled disc, clipped before iterating. Invalid/negative radii do nothing.
    pub fn disc(&mut self, cx: f32, cy: f32, radius: f32, color: Rgb) {
        if !cx.is_finite() || !cy.is_finite() || !radius.is_finite() || radius < 0. {
            return;
        }
        let (cx, cy, r) = (cx as f64, cy as f64, radius as f64);
        let x0 = (cx - r).floor().max(0.) as i32;
        let x1 = (cx + r).ceil().min(self.width as f64 - 1.) as i32;
        let y0 = (cy - r).floor().max(0.) as i32;
        let y1 = (cy + r).ceil().min(self.height as f64 - 1.) as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                if (x as f64 - cx).powi(2) + (y as f64 - cy).powi(2) <= r * r {
                    self.set(x, y, color);
                }
            }
        }
    }
    /// Opaque half-block realization. An unmatched bottom pixel is black.
    /// Color capability quantization remains the ordinary renderer's responsibility.
    pub fn to_surface(&self) -> Surface {
        let mut surface = Surface::new(self.width, self.height.div_ceil(2));
        for y in 0..surface.height {
            for x in 0..surface.width {
                let top = self.get(x as i32, y as i32 * 2).unwrap_or_default();
                let bottom = self.get(x as i32, y as i32 * 2 + 1).unwrap_or_default();
                surface.set_cell(
                    x,
                    y,
                    Cell::new(
                        Glyph::new("▀"),
                        Style::new()
                            .fg(Color::Rgb(top.0, top.1, top.2))
                            .bg(Color::Rgb(bottom.0, bottom.1, bottom.2)),
                    ),
                );
            }
        }
        surface
    }
    /// Same cell bounds as half-block output, but luminance becomes ordered
    /// Braille dot density. Each RGB sample covers 2×2 candidate dots. No RGB
    /// colors are emitted, so silhouettes survive true monochrome terminals.
    pub fn to_mono_surface(&self) -> Surface {
        let mut surface = Surface::new(self.width, self.height.div_ceil(2));
        const BAYER: [[u32; 2]; 4] = [[0, 4], [6, 2], [1, 5], [7, 3]];
        const BITS: [[u8; 2]; 4] = [[1, 8], [2, 16], [4, 32], [64, 128]];
        for y in 0..surface.height {
            for x in 0..surface.width {
                let mut bits = 0;
                for dy in 0..4 {
                    let c = self
                        .get(x as i32, y as i32 * 2 + (dy / 2) as i32)
                        .unwrap_or_default();
                    let luminance = (54 * c.0 as u32 + 183 * c.1 as u32 + 19 * c.2 as u32) / 256;
                    for dx in 0..2 {
                        if luminance * 8 > BAYER[dy][dx] * 255 + 127 {
                            bits |= BITS[dy][dx];
                        }
                    }
                }
                let glyph = char::from_u32(0x2800 + bits as u32).unwrap_or(' ');
                surface.set_cell(x, y, Cell::new(Glyph::from_char(glyph), Style::new()));
            }
        }
        surface
    }
    /// Optional development dump; does not participate in terminal rendering.
    pub fn write_ppm(&self, mut writer: impl Write) -> io::Result<()> {
        write!(writer, "P6\n{} {}\n255\n", self.width, self.height)?;
        for &(r, g, b) in &self.pixels {
            writer.write_all(&[r, g, b])?;
        }
        Ok(())
    }
}

impl RgbRaster {
    /// Ingests a bounded buffer of packed RGB bytes into a new owned raster (#129).
    ///
    /// `stride` is the number of bytes per row in `bytes` (>= `width * 3`; rows may be
    /// padded). Pixels are COPIED (never borrowed) into the owned `Vec<Rgb>`. Bad input
    /// is REJECTED with a [`FrameError`], never clamped: an oversize dimension, a stride
    /// too small for a packed row, an arithmetic overflow, or a buffer too short all
    /// return an error rather than a surprising partial frame.
    pub fn from_rgb_bytes(
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
        // width <= 2048 here, so this cannot overflow usize.
        let min_row_bytes = width as usize * 3;
        if stride < min_row_bytes {
            return Err(FrameError::StrideTooSmall {
                stride,
                min_row_bytes,
            });
        }
        let need = stride
            .checked_mul(height as usize)
            .ok_or(FrameError::DimensionsOverflow { stride, height })?;
        if bytes.len() < need {
            return Err(FrameError::ShortBuffer {
                have: bytes.len(),
                need,
            });
        }
        let mut pixels = Vec::with_capacity(width as usize * height as usize);
        for row in 0..height as usize {
            let row_start = row * stride;
            for col in 0..width as usize {
                let p = row_start + col * 3;
                pixels.push((bytes[p], bytes[p + 1], bytes[p + 2]));
            }
        }
        Ok(RgbRaster {
            width,
            height,
            pixels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_rgb_bytes_rejects_zero_dimension() {
        // Predicted: an empty frame has nothing to ingest -> ZeroDimension echoing inputs.
        assert_eq!(
            RgbRaster::from_rgb_bytes(0, 2, 6, &[0; 12]),
            Err(FrameError::ZeroDimension {
                width: 0,
                height: 2
            })
        );
    }

    #[test]
    fn from_rgb_bytes_rejects_oversize_not_clamps() {
        // Predicted: ingest REJECTS an oversize axis with TooLarge...
        let w = MAX_RASTER_DIMENSION + 1;
        assert_eq!(
            RgbRaster::from_rgb_bytes(w, 1, w as usize * 3, &[]),
            Err(FrameError::TooLarge {
                width: w,
                height: 1,
                max: MAX_RASTER_DIMENSION
            })
        );
        // ...whereas `new` CLAMPS (convenience). Ingest must not do that: it would lie
        // about the bytes the caller actually handed over.
        assert_eq!(RgbRaster::new(w, 1).width(), MAX_RASTER_DIMENSION);
    }

    #[test]
    fn from_rgb_bytes_rejects_stride_smaller_than_row() {
        // Predicted: width 2 needs 6 bytes/row; stride 5 -> StrideTooSmall.
        assert_eq!(
            RgbRaster::from_rgb_bytes(2, 2, 5, &[0; 64]),
            Err(FrameError::StrideTooSmall {
                stride: 5,
                min_row_bytes: 6
            })
        );
    }

    #[test]
    fn from_rgb_bytes_rejects_short_buffer() {
        // Predicted: 2x2 at stride 6 needs 12 bytes; 11 given -> ShortBuffer{11,12}.
        assert_eq!(
            RgbRaster::from_rgb_bytes(2, 2, 6, &[0; 11]),
            Err(FrameError::ShortBuffer { have: 11, need: 12 })
        );
    }

    #[test]
    fn from_rgb_bytes_round_trips_packed_pixels() {
        // Predicted: row-major, R,G,B order preserved; every pixel distinct so a
        // channel swap or row/column transpose would fail.
        let buf = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let r = RgbRaster::from_rgb_bytes(2, 2, 6, &buf).unwrap();
        assert_eq!((r.width(), r.height()), (2, 2));
        assert_eq!(r.get(0, 0), Some((1, 2, 3)));
        assert_eq!(r.get(1, 0), Some((4, 5, 6)));
        assert_eq!(r.get(0, 1), Some((7, 8, 9)));
        assert_eq!(r.get(1, 1), Some((10, 11, 12)));
    }

    #[test]
    fn from_rgb_bytes_skips_row_padding() {
        // Predicted: 2x1 at stride 8 -> the 2 trailing 0xFF pad bytes are skipped,
        // so only the six real bytes become pixels.
        let buf = [10, 20, 30, 40, 50, 60, 0xFF, 0xFF];
        let r = RgbRaster::from_rgb_bytes(2, 1, 8, &buf).unwrap();
        assert_eq!(r.pixels(), &[(10, 20, 30), (40, 50, 60)]);
        // Padding also sits between rows: 1x2 at stride 5 -> row 1 starts at byte 5.
        let buf = [1, 2, 3, 0xFF, 0xFF, 4, 5, 6, 0xFF, 0xFF];
        let r = RgbRaster::from_rgb_bytes(1, 2, 5, &buf).unwrap();
        assert_eq!(r.pixels(), &[(1, 2, 3), (4, 5, 6)]);
    }

    #[test]
    fn from_rgb_bytes_rejects_dimensions_overflow() {
        // Predicted: width 1 passes the stride check (usize::MAX >= 3), then
        // usize::MAX * 2 overflows -> DimensionsOverflow, before bytes are touched.
        assert_eq!(
            RgbRaster::from_rgb_bytes(1, 2, usize::MAX, &[0; 3]),
            Err(FrameError::DimensionsOverflow {
                stride: usize::MAX,
                height: 2
            })
        );
    }
}
