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
    /// Filled rounded rectangle, composited at `alpha`, with **anti-aliased
    /// corners** — a one-pixel analytic coverage band at the edge, so a pill reads
    /// as a smooth "shrinkwrapped" shape rather than a blocky cell rectangle even
    /// at half-block terminal scale. `radius` is clamped to half the shorter side
    /// (a radius ≥ min(w,h)/2 gives a full lozenge/stadium). The rectangle covers
    /// pixels `[x, x+w) × [y, y+h)`. Non-positive sizes or non-finite inputs do
    /// nothing. This is the raster primitive beneath baked pill buttons/chips.
    #[allow(clippy::too_many_arguments)] // a rounded-rect fill: geometry + colour + alpha
    pub fn round_rect(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        radius: f32,
        color: Rgb,
        alpha: f32,
    ) {
        if w <= 0 || h <= 0 || !radius.is_finite() || !alpha.is_finite() || alpha <= 0.0 {
            return;
        }
        let (fw, fh) = (w as f32, h as f32);
        let r = radius.max(0.0).min(fw / 2.0).min(fh / 2.0);
        let (cx, cy) = (x as f32 + fw / 2.0, y as f32 + fh / 2.0);
        let (bx, by) = (fw / 2.0, fh / 2.0);
        let px0 = x.max(0);
        let px1 = (x + w - 1).min(self.width as i32 - 1);
        let py0 = y.max(0);
        let py1 = (y + h - 1).min(self.height as i32 - 1);
        for py in py0..=py1 {
            for px in px0..=px1 {
                // Signed distance to a rounded box (IQ's sdRoundBox), sampled at the
                // pixel centre; coverage is the 1px band straddling the zero contour.
                let dx = (px as f32 + 0.5 - cx).abs() - bx + r;
                let dy = (py as f32 + 0.5 - cy).abs() - by + r;
                let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
                let d = outside + dx.max(dy).min(0.0) - r;
                let coverage = (0.5 - d).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.blend(px, py, color, alpha * coverage);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_rect_zero_radius_fills_every_pixel_opaque() {
        let mut r = RgbRaster::new(6, 4);
        r.round_rect(0, 0, 6, 4, 0.0, (200, 100, 50), 1.0);
        for y in 0..4 {
            for x in 0..6 {
                assert_eq!(r.get(x, y), Some((200, 100, 50)), "pixel ({x},{y})");
            }
        }
    }

    #[test]
    fn round_rect_rounds_the_corners_but_fills_the_centre() {
        let mut r = RgbRaster::new(12, 12);
        // Max radius → a full rounded shape: the extreme corner is (near) untouched,
        // the centre is fully filled.
        r.round_rect(0, 0, 12, 12, 6.0, (255, 255, 255), 1.0);
        assert_eq!(r.get(0, 0), Some((0, 0, 0)), "corner should be cut away");
        assert_eq!(
            r.get(6, 6),
            Some((255, 255, 255)),
            "centre should be filled"
        );
        // A mid-edge pixel sits on the rounded contour: brightly filled but
        // anti-aliased (not necessarily a solid 255) — the smoothing that makes the
        // edge read as curved rather than blocky.
        let (tr, _, _) = r.get(6, 0).unwrap();
        assert!(
            tr > 200,
            "top-centre edge should be brightly filled, got {tr}"
        );
    }

    #[test]
    fn round_rect_is_bounds_safe_and_ignores_degenerate_input() {
        let mut r = RgbRaster::new(4, 4);
        // Partly off-canvas (negative origin) must not panic and must clip.
        r.round_rect(-2, -2, 5, 5, 1.0, (10, 20, 30), 1.0);
        // Degenerate / non-finite inputs are no-ops.
        r.round_rect(0, 0, 0, 4, 1.0, (1, 2, 3), 1.0);
        r.round_rect(0, 0, 4, 4, f32::NAN, (1, 2, 3), 1.0);
        r.round_rect(0, 0, 4, 4, 1.0, (1, 2, 3), f32::INFINITY);
        // Something inside the clipped region did get painted.
        assert_ne!(r.get(0, 0), Some((0, 0, 0)));
    }
}
