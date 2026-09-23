//! Experimental opaque RGB subcell framebuffer. Software blending happens here,
//! before conversion to ordinary Unicode cells; it is not terminal alpha.
use crate::canvas::clip_line_to_bounds;
use crate::cell::{Cell, Color, Glyph, Style};
use crate::surface::Surface;
use std::io::{self, Write};

pub type Rgb = (u8, u8, u8);
/// Explicit allocation bound for tiny software graphics, per pixel axis.
pub const MAX_RASTER_DIMENSION: u16 = 2048;

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
    /// Linear RGB interpolation. Nonfinite amounts are ignored; finite amounts clamp.
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
                surface.set_cell(
                    x,
                    y,
                    Cell::new(Glyph::new(&glyph.to_string()), Style::new()),
                );
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
