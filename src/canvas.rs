//! Sub-cell raster canvases built from ordinary Unicode cells.
//!
//! These are **not** graphics protocols (no Kitty/Sixel). They squeeze extra
//! resolution out of plain terminal cells:
//!
//! * [`BrailleCanvas`] — 2×4 binary dots per cell (`⠀`..`⣿`), 1 color per cell.
//! * [`HalfBlockCanvas`] — 1 horizontal × 2 vertical RGB samples per cell
//!   (addressable grid `width` × `2 * height`) via `▀`, foreground = top pixel,
//!   background = bottom pixel.
//!
//! Both produce a [`Surface`] (or styled text), so they flow through the normal
//! diff/ANSI pipeline and composite like any other node.

use crate::cell::{Color, Glyph, Line, RichText, Span, Style};
use crate::surface::{Rect, Surface};

// ---------------------------------------------------------------------------
// 2D segment clipping
// ---------------------------------------------------------------------------

/// Clips the segment `(x0,y0)-(x1,y1)` to the inclusive integer bounds
/// `[0, max_x] x [0, max_y]` using Liang–Barsky.
///
/// Returns the clipped endpoints, or `None` when the segment does not intersect
/// the rectangle at all (or the rectangle is empty). The returned coordinates
/// are guaranteed to lie inside the bounds.
///
/// This exists so **Bresenham walks are bounded by the canvas, not by the
/// coordinate magnitude**. A perspective point near the camera can produce
/// perfectly finite coordinates in the tens of millions; clipping first turns a
/// 20-million-step invisible walk into a ≤`max_x + max_y` step visible one. The
/// clip math is done in `f64`, which represents every `i32` exactly and cannot
/// overflow, so `i32::MIN..i32::MAX` endpoints are safe.
///
/// Pipeline: 3D near-plane clip → perspective projection → **this** → bounded
/// Bresenham.
pub fn clip_line_to_bounds(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    max_x: i32,
    max_y: i32,
) -> Option<((i32, i32), (i32, i32))> {
    if max_x < 0 || max_y < 0 {
        return None;
    }
    let (fx0, fy0) = (x0 as f64, y0 as f64);
    let (fx1, fy1) = (x1 as f64, y1 as f64);
    let dx = fx1 - fx0;
    let dy = fy1 - fy0;

    // p[i] * t <= q[i] for the four half-planes x>=0, x<=max_x, y>=0, y<=max_y.
    let p = [-dx, dx, -dy, dy];
    let q = [fx0, max_x as f64 - fx0, fy0, max_y as f64 - fy0];

    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for i in 0..4 {
        if p[i] == 0.0 {
            // Parallel to this boundary: keep the segment only if it is inside.
            if q[i] < 0.0 {
                return None;
            }
        } else {
            let r = q[i] / p[i];
            if p[i] < 0.0 {
                if r > t1 {
                    return None;
                }
                if r > t0 {
                    t0 = r;
                }
            } else {
                if r < t0 {
                    return None;
                }
                if r < t1 {
                    t1 = r;
                }
            }
        }
    }

    let cx0 = (fx0 + t0 * dx).round() as i32;
    let cy0 = (fy0 + t0 * dy).round() as i32;
    let cx1 = (fx0 + t1 * dx).round() as i32;
    let cy1 = (fy0 + t1 * dy).round() as i32;
    Some((
        (cx0.clamp(0, max_x), cy0.clamp(0, max_y)),
        (cx1.clamp(0, max_x), cy1.clamp(0, max_y)),
    ))
}

// ---------------------------------------------------------------------------
// Braille
// ---------------------------------------------------------------------------

/// Binary dot-matrix canvas: `2 * width` by `4 * height` addressable dots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrailleCanvas {
    pub width: u16,
    pub height: u16,
    cells: Vec<u8>,
}

impl BrailleCanvas {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![0u8; (width as usize) * (height as usize)],
        }
    }

    pub fn pixel_width(&self) -> u16 {
        self.width * 2
    }

    pub fn pixel_height(&self) -> u16 {
        self.height * 4
    }

    pub fn clear(&mut self) {
        self.cells.fill(0);
    }

    #[inline]
    fn dot_bit(dx: u16, dy: u16) -> u8 {
        match (dx, dy) {
            (0, 0) => 0x01,
            (0, 1) => 0x02,
            (0, 2) => 0x04,
            (1, 0) => 0x08,
            (1, 1) => 0x10,
            (1, 2) => 0x20,
            (0, 3) => 0x40,
            (1, 3) => 0x80,
            _ => 0,
        }
    }

    /// Sets a dot at pixel coordinates. Out-of-bounds coordinates are ignored.
    pub fn set(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as u16, y as u16);
        if x >= self.pixel_width() || y >= self.pixel_height() {
            return;
        }
        let cx = x / 2;
        let cy = y / 4;
        let bit = Self::dot_bit(x % 2, y % 4);
        let idx = (cy as usize) * (self.width as usize) + (cx as usize);
        self.cells[idx] |= bit;
    }

    pub fn unset(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as u16, y as u16);
        if x >= self.pixel_width() || y >= self.pixel_height() {
            return;
        }
        let cx = x / 2;
        let cy = y / 4;
        let bit = Self::dot_bit(x % 2, y % 4);
        let idx = (cy as usize) * (self.width as usize) + (cx as usize);
        self.cells[idx] &= !bit;
    }

    pub fn get(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (x, y) = (x as u16, y as u16);
        if x >= self.pixel_width() || y >= self.pixel_height() {
            return false;
        }
        let cx = x / 2;
        let cy = y / 4;
        let bit = Self::dot_bit(x % 2, y % 4);
        let idx = (cy as usize) * (self.width as usize) + (cx as usize);
        self.cells[idx] & bit != 0
    }

    /// The braille glyph for a cell, or `None` if all dots are clear.
    pub fn glyph_at(&self, cx: u16, cy: u16) -> Option<char> {
        if cx >= self.width || cy >= self.height {
            return None;
        }
        let bits = self.cells[(cy as usize) * (self.width as usize) + (cx as usize)];
        if bits == 0 {
            None
        } else {
            char::from_u32(0x2800 + bits as u32)
        }
    }

    /// Bresenham line between two dot coordinates, clipped to the canvas first.
    ///
    /// The segment is clipped to the canvas bounds before the walk, so the
    /// number of iterations is bounded by the clipped length even for enormous
    /// finite coordinates. See [`clip_line_to_bounds`].
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let max_x = self.pixel_width() as i32 - 1;
        let max_y = self.pixel_height() as i32 - 1;
        let Some(((x0, y0), (x1, y1))) = clip_line_to_bounds(x0, y0, x1, y1, max_x, max_y) else {
            return;
        };
        self.bresenham(x0, y0, x1, y1);
    }

    /// Unclipped Bresenham walk. Callers must pass in-bounds endpoints.
    fn bresenham(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (mut x0, mut y0) = (x0, y0);
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.set(x0, y0);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn polyline(&mut self, points: &[(i32, i32)]) {
        for w in points.windows(2) {
            self.line(w[0].0, w[0].1, w[1].0, w[1].1);
        }
    }

    /// Closes a polygon and draws its outline.
    pub fn polygon(&mut self, points: &[(i32, i32)]) {
        if points.len() < 2 {
            return;
        }
        self.polyline(points);
        let first = points[0];
        let last = points[points.len() - 1];
        self.line(last.0, last.1, first.0, first.1);
    }

    /// Rectangle outline.
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let (x1, y1) = (x.saturating_add(w - 1), y.saturating_add(h - 1));
        self.line(x, y, x1, y);
        self.line(x, y1, x1, y1);
        self.line(x, y, x, y1);
        self.line(x1, y, x1, y1);
    }

    /// Filled rectangle.
    pub fn filled_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for yy in y..(y + h) {
            for xx in x..(x + w) {
                self.set(xx, yy);
            }
        }
    }

    /// Midpoint circle outline.
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32) {
        if r < 0 {
            return;
        }
        let mut x = r;
        let mut y = 0;
        let mut err = 1 - r;
        while x >= y {
            for (px, py) in [
                (cx + x, cy + y),
                (cx + y, cy + x),
                (cx - y, cy + x),
                (cx - x, cy + y),
                (cx - x, cy - y),
                (cx - y, cy - x),
                (cx + y, cy - x),
                (cx + x, cy - y),
            ] {
                self.set(px, py);
            }
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// Filled circle via horizontal scanlines.
    pub fn filled_circle(&mut self, cx: i32, cy: i32, r: i32) {
        if r < 0 {
            return;
        }
        for dy in -r..=r {
            // `r * r` can overflow `i32` for large radii; do the math in `f64`.
            let rr = (r as f64) * (r as f64);
            let dd = (dy as f64) * (dy as f64);
            let dx = (rr - dd).max(0.0).sqrt().round() as i32;
            let (x0, x1) = (cx.saturating_sub(dx), cx.saturating_add(dx));
            self.line(x0, cy.saturating_add(dy), x1, cy.saturating_add(dy));
        }
    }

    /// Ellipse outline (axis-aligned).
    pub fn ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32) {
        if rx <= 0 || ry <= 0 {
            return;
        }
        let steps = ((rx + ry) as f32 * 1.5).max(12.0) as i32;
        let mut prev: Option<(i32, i32)> = None;
        for i in 0..=steps {
            let a = std::f32::consts::TAU * i as f32 / steps as f32;
            let p = (
                cx + (rx as f32 * a.cos()).round() as i32,
                cy + (ry as f32 * a.sin()).round() as i32,
            );
            if let Some(q) = prev {
                self.line(q.0, q.1, p.0, p.1);
            }
            prev = Some(p);
        }
    }

    /// True when every dot is clear.
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|c| *c == 0)
    }

    /// Paints the canvas into `surface` at `origin` with `style`.
    pub fn paint_into(&self, surface: &mut Surface, origin: (u16, u16), style: Style) {
        for cy in 0..self.height {
            for cx in 0..self.width {
                if let Some(ch) = self.glyph_at(cx, cy) {
                    let x = origin.0 + cx;
                    let y = origin.1 + cy;
                    surface.set_cell(
                        x,
                        y,
                        crate::cell::Cell::new(Glyph::new(&ch.to_string()), style),
                    );
                }
            }
        }
    }

    /// Converts to a standalone surface of exactly `width x height` cells.
    pub fn to_surface(&self, style: Style) -> Surface {
        let mut s = Surface::new(self.width, self.height);
        self.paint_into(&mut s, (0, 0), style);
        s
    }

    /// Plain text lines (blank cells become spaces).
    pub fn to_lines(&self) -> Vec<String> {
        (0..self.height)
            .map(|cy| {
                (0..self.width)
                    .map(|cx| self.glyph_at(cx, cy).unwrap_or(' '))
                    .collect()
            })
            .collect()
    }

    pub fn to_rich_text(&self, style: Style) -> RichText {
        let mut rt = RichText::new();
        for line in self.to_lines() {
            rt = rt.line(Line::styled(line, style));
        }
        rt
    }
}

// ---------------------------------------------------------------------------
// Half-block RGB
// ---------------------------------------------------------------------------

/// RGB raster canvas: `width` by `2 * height` addressable pixels, rendered as
/// `▀` cells (top pixel = foreground, bottom pixel = background).
///
/// The `▀` glyph's two colour attributes give **exactly one horizontal sample
/// and two vertical samples per terminal cell**, so the addressable grid is
/// `width` columns by `height * 2` rows — not `2 * width`. Each horizontal
/// pixel maps to its own cell; no column is discarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HalfBlockCanvas {
    pub width: u16,
    pub height: u16,
    pixels: Vec<Option<(u8, u8, u8)>>,
}

impl HalfBlockCanvas {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            pixels: vec![None; (width as usize) * (height as usize) * 2],
        }
    }

    /// Addressable pixel columns. One per terminal cell (the half-block glyph is
    /// only half-width in the vertical direction).
    pub fn pixel_width(&self) -> u16 {
        self.width
    }

    /// Addressable pixel rows. Two per terminal cell (top and bottom halves).
    pub fn pixel_height(&self) -> u16 {
        self.height * 2
    }

    pub fn clear(&mut self) {
        self.pixels.fill(None);
    }

    /// True when no pixel is set.
    pub fn is_empty(&self) -> bool {
        self.pixels.iter().all(|p| p.is_none())
    }

    #[inline]
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.pixel_width() as i32 || y >= self.pixel_height() as i32 {
            return None;
        }
        Some((y as usize) * (self.pixel_width() as usize) + (x as usize))
    }

    /// Sets a pixel. Out-of-bounds writes are ignored.
    pub fn set_pixel(&mut self, x: i32, y: i32, rgb: (u8, u8, u8)) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = Some(rgb);
        }
    }

    pub fn clear_pixel(&mut self, x: i32, y: i32) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = None;
        }
    }

    pub fn get_pixel(&self, x: i32, y: i32) -> Option<(u8, u8, u8)> {
        self.index(x, y).and_then(|i| self.pixels[i])
    }

    /// Bresenham line with a solid color, clipped to the canvas first.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, rgb: (u8, u8, u8)) {
        let max_x = self.pixel_width() as i32 - 1;
        let max_y = self.pixel_height() as i32 - 1;
        let Some(((x0, y0), (x1, y1))) = clip_line_to_bounds(x0, y0, x1, y1, max_x, max_y) else {
            return;
        };
        let (mut x0, mut y0) = (x0, y0);
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.set_pixel(x0, y0, rgb);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn polyline(&mut self, points: &[(i32, i32)], rgb: (u8, u8, u8)) {
        for w in points.windows(2) {
            self.line(w[0].0, w[0].1, w[1].0, w[1].1, rgb);
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: (u8, u8, u8)) {
        if w <= 0 || h <= 0 {
            return;
        }
        let (x1, y1) = (x.saturating_add(w - 1), y.saturating_add(h - 1));
        self.line(x, y, x1, y, rgb);
        self.line(x, y1, x1, y1, rgb);
        self.line(x, y, x, y1, rgb);
        self.line(x1, y, x1, y1, rgb);
    }

    /// Converts to a surface. Cells with no pixels are explicitly transparent so
    /// the canvas composes over other layers.
    pub fn to_surface(&self) -> Surface {
        let mut s = Surface::new_transparent(self.width, self.height);
        for cy in 0..self.height {
            for cx in 0..self.width {
                let top = self.get_pixel(cx as i32, cy as i32 * 2);
                let bottom = self.get_pixel(cx as i32, cy as i32 * 2 + 1);
                let (glyph, style) = match (top, bottom) {
                    (None, None) => continue,
                    (Some(t), None) => ("▀", Style::new().fg(Color::Rgb(t.0, t.1, t.2))),
                    (None, Some(b)) => ("▄", Style::new().fg(Color::Rgb(b.0, b.1, b.2))),
                    (Some(t), Some(b)) => (
                        "▀",
                        Style::new()
                            .fg(Color::Rgb(t.0, t.1, t.2))
                            .bg(Color::Rgb(b.0, b.1, b.2)),
                    ),
                };
                s.set_cell(cx, cy, crate::cell::Cell::new(Glyph::new(glyph), style));
            }
        }
        s
    }

    /// Composites the canvas onto `surface` at `origin`, respecting
    /// transparency.
    pub fn paint_into(&self, surface: &mut Surface, origin: (u16, u16)) {
        let layer = self.to_surface();
        surface.blit_transparent_at(&layer, origin.0, origin.1);
    }

    /// Builds a [`RichText`] view: each cell is a `▀` span with foreground = top
    /// pixel and background = bottom pixel. Cells with no pixels become spaces.
    pub fn to_rich_text(&self) -> RichText {
        let mut rt = RichText::new();
        for cy in 0..self.height {
            let mut line = Line::new();
            for cx in 0..self.width {
                let top = self.get_pixel(cx as i32, cy as i32 * 2);
                let bottom = self.get_pixel(cx as i32, cy as i32 * 2 + 1);
                match (top, bottom) {
                    (None, None) => line = line.span(Span::raw(" ")),
                    (Some(t), None) => {
                        line = line.span(Span::styled(
                            "▀",
                            Style::new().fg(Color::Rgb(t.0, t.1, t.2)),
                        ))
                    }
                    (None, Some(b)) => {
                        line = line.span(Span::styled(
                            "▄",
                            Style::new().fg(Color::Rgb(b.0, b.1, b.2)),
                        ))
                    }
                    (Some(t), Some(b)) => {
                        line = line.span(Span::styled(
                            "▀",
                            Style::new()
                                .fg(Color::Rgb(t.0, t.1, t.2))
                                .bg(Color::Rgb(b.0, b.1, b.2)),
                        ))
                    }
                }
            }
            rt = rt.line(line);
        }
        rt
    }
}

// ---------------------------------------------------------------------------
// Small chart helpers built on the canvases
// ---------------------------------------------------------------------------

/// Plots `values` (in `[-1, 1]` or any range; auto-scaled) as a braille
/// oscilloscope trace across `width x height` cells.
pub fn braille_oscilloscope(values: &[f32], width: u16, height: u16) -> BrailleCanvas {
    let mut c = BrailleCanvas::new(width, height);
    if values.is_empty() || width == 0 || height == 0 {
        return c;
    }
    let pw = c.pixel_width() as i32;
    let ph = c.pixel_height() as i32;
    let n = values.len();
    let mut prev: Option<(i32, i32)> = None;
    for x in 0..pw {
        let idx = ((x as usize) * n) / (pw as usize).max(1);
        let v = values[idx.min(n - 1)];
        let t = ((v + 1.0) * 0.5).clamp(0.0, 1.0);
        let y = ph - 1 - (t * (ph - 1) as f32).round() as i32;
        if let Some((px, py)) = prev {
            c.line(px, py, x, y);
        } else {
            c.set(x, y);
        }
        prev = Some((x, y));
    }
    c
}

/// Fills a rectangle in a braille canvas (dot resolution).
pub fn braille_fill_rect(c: &mut BrailleCanvas, x: i32, y: i32, w: i32, h: i32) {
    for yy in y..(y + h) {
        for xx in x..(x + w) {
            c.set(xx, yy);
        }
    }
    let _ = Rect::new(0, 0, 0, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_each_dot_position_is_exact() {
        // (column, row, expected braille bit)
        let dots = [
            (0u16, 0u16, 0x01u32), // dot 1
            (0, 1, 0x02),          // dot 2
            (0, 2, 0x04),          // dot 3
            (1, 0, 0x08),          // dot 4
            (1, 1, 0x10),          // dot 5
            (1, 2, 0x20),          // dot 6
            (0, 3, 0x40),          // dot 7
            (1, 3, 0x80),          // dot 8
        ];
        for (dx, dy, bit) in dots {
            let mut c = BrailleCanvas::new(1, 1);
            c.set(dx as i32, dy as i32);
            let expected = char::from_u32(0x2800 + bit).unwrap();
            assert_eq!(c.glyph_at(0, 0), Some(expected), "dot at ({dx},{dy})");
            assert!(c.get(dx as i32, dy as i32));
        }
    }

    #[test]
    fn braille_all_dots_is_full_block() {
        let mut c = BrailleCanvas::new(1, 1);
        for y in 0..4 {
            for x in 0..2 {
                c.set(x, y);
            }
        }
        assert_eq!(c.glyph_at(0, 0), Some('\u{28FF}'));
        assert_eq!(c.to_lines(), vec!["⣿".to_string()]);
    }

    #[test]
    fn braille_unset_and_clear() {
        let mut c = BrailleCanvas::new(1, 1);
        c.set(0, 0);
        assert_eq!(c.glyph_at(0, 0), Some('⠁'));
        c.unset(0, 0);
        assert_eq!(c.glyph_at(0, 0), None);
        c.set(1, 3);
        c.clear();
        assert_eq!(c.glyph_at(0, 0), None);
    }

    #[test]
    fn braille_clipping_and_out_of_bounds() {
        let mut c = BrailleCanvas::new(2, 1); // 4x4 dots
        c.set(-1, 0);
        c.set(0, -1);
        c.set(4, 0);
        c.set(0, 4);
        assert!(c.to_lines()[0].trim().is_empty());
        c.set(3, 3); // last valid dot -> dot 8
        assert_eq!(c.glyph_at(1, 0), Some('\u{2880}'));
    }

    #[test]
    fn braille_diagonal_line_covers_endpoints() {
        let mut c = BrailleCanvas::new(4, 2); // 8x8 dots
        c.line(0, 0, 7, 7);
        assert!(c.get(0, 0));
        assert!(c.get(7, 7));
        // A diagonal has one roughly-monotonic dot per column.
        let mut cols = 0;
        for x in 0..8 {
            if (0..8).any(|y| c.get(x, y)) {
                cols += 1;
            }
        }
        assert_eq!(cols, 8);
    }

    #[test]
    fn half_block_top_only_uses_upper_glyph() {
        let mut c = HalfBlockCanvas::new(1, 1);
        c.set_pixel(0, 0, (255, 0, 0));
        let s = c.to_surface();
        let cell = s.get(0, 0).unwrap();
        assert_eq!(cell.glyph.grapheme.as_str(), "▀");
        assert_eq!(cell.style.fg, Some(Color::Rgb(255, 0, 0)));
        assert_eq!(cell.style.bg, None);
    }

    #[test]
    fn half_block_bottom_only_uses_lower_glyph() {
        let mut c = HalfBlockCanvas::new(1, 1);
        c.set_pixel(0, 1, (0, 0, 255));
        let s = c.to_surface();
        let cell = s.get(0, 0).unwrap();
        assert_eq!(cell.glyph.grapheme.as_str(), "▄");
        assert_eq!(cell.style.fg, Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn half_block_both_uses_fg_top_bg_bottom() {
        let mut c = HalfBlockCanvas::new(1, 1);
        c.set_pixel(0, 0, (10, 20, 30));
        c.set_pixel(0, 1, (40, 50, 60));
        let s = c.to_surface();
        let cell = s.get(0, 0).unwrap();
        assert_eq!(cell.glyph.grapheme.as_str(), "▀");
        assert_eq!(cell.style.fg, Some(Color::Rgb(10, 20, 30)));
        assert_eq!(cell.style.bg, Some(Color::Rgb(40, 50, 60)));
    }

    #[test]
    fn half_block_empty_cell_is_transparent() {
        let c = HalfBlockCanvas::new(2, 1);
        let s = c.to_surface();
        assert!(s.get(0, 0).unwrap().transparent);
        assert!(s.get(1, 0).unwrap().transparent);
    }

    #[test]
    fn half_block_rect_and_clipping() {
        let mut c = HalfBlockCanvas::new(3, 3); // 3 columns x 6 rows of pixels
        c.rect(0, 0, 3, 6, (1, 2, 3));
        assert!(c.get_pixel(0, 0).is_some());
        assert!(c.get_pixel(2, 5).is_some());
        assert!(c.get_pixel(1, 3).is_none());
        c.set_pixel(99, 99, (9, 9, 9)); // ignored
    }

    #[test]
    fn half_block_geometry_is_one_pixel_wide_two_tall_per_cell() {
        let c = HalfBlockCanvas::new(2, 1);
        assert_eq!(c.pixel_width(), 2, "one addressable pixel column per cell");
        assert_eq!(c.pixel_height(), 2, "two addressable pixel rows per cell");

        // Both horizontal pixels are addressable; the odd column is not discarded.
        let mut c = HalfBlockCanvas::new(2, 1);
        c.set_pixel(0, 0, (255, 0, 0));
        c.set_pixel(1, 0, (0, 255, 0));
        let s = c.to_surface();
        assert_eq!(s.get(0, 0).unwrap().style.fg, Some(Color::Rgb(255, 0, 0)));
        assert_eq!(s.get(1, 0).unwrap().style.fg, Some(Color::Rgb(0, 255, 0)));

        // Changing x=0 vs x=1 must alter *different* terminal cells.
        let mut x0 = HalfBlockCanvas::new(2, 1);
        x0.set_pixel(0, 0, (1, 2, 3));
        let mut x1 = HalfBlockCanvas::new(2, 1);
        x1.set_pixel(1, 0, (1, 2, 3));
        let s0 = x0.to_surface();
        let s1 = x1.to_surface();
        assert!(s0.get(0, 0).unwrap().style.fg.is_some());
        assert!(s0.get(1, 0).unwrap().transparent);
        assert!(s1.get(0, 0).unwrap().transparent);
        assert!(s1.get(1, 0).unwrap().style.fg.is_some());
    }

    #[test]
    fn half_block_odd_column_pixel_is_rendered_in_rich_text() {
        let mut c = HalfBlockCanvas::new(2, 1);
        c.set_pixel(1, 1, (9, 9, 9)); // bottom half of the *second* cell
        let rt = c.to_rich_text();
        let line = &rt.lines[0];
        let rendered: String = line.spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(rendered, " ▄");
    }

    #[test]
    fn oscilloscope_produces_some_dots() {
        let vals: Vec<f32> = (0..64).map(|i| (i as f32 * 0.3).sin()).collect();
        let c = braille_oscilloscope(&vals, 8, 2);
        assert!(c.to_lines().iter().any(|l| !l.trim().is_empty()));
    }

    #[test]
    fn braille_rect_outline_and_fill() {
        let mut outline = BrailleCanvas::new(3, 2); // 6 x 8 dots
        outline.rect(0, 0, 6, 8);
        assert!(outline.get(0, 0));
        assert!(outline.get(5, 7));
        assert!(!outline.get(3, 3), "outline must be hollow");

        let mut filled = BrailleCanvas::new(3, 2);
        filled.filled_rect(0, 0, 6, 8);
        assert!(filled.get(3, 3), "filled rect must cover interior");
    }

    #[test]
    fn braille_circle_is_round_and_bounded() {
        let mut c = BrailleCanvas::new(8, 4); // 16 x 16 dots
        c.circle(8, 8, 6);
        assert!(c.get(8, 2), "top of circle");
        assert!(c.get(8, 14), "bottom of circle");
        assert!(c.get(2, 8), "left of circle");
        assert!(c.get(14, 8), "right of circle");
        assert!(!c.get(8, 8), "circle is hollow");
    }

    #[test]
    fn braille_filled_circle_covers_centre() {
        let mut c = BrailleCanvas::new(8, 4);
        c.filled_circle(8, 8, 5);
        assert!(c.get(8, 8));
        assert!(!c.get(0, 0));
    }

    #[test]
    fn braille_ellipse_and_polygon() {
        let mut c = BrailleCanvas::new(8, 4);
        c.ellipse(8, 8, 7, 3);
        assert!(c.get(8, 5) || c.get(7, 5) || c.get(9, 5));
        let mut p = BrailleCanvas::new(8, 4);
        p.polygon(&[(1, 1), (14, 1), (14, 14), (1, 14)]);
        assert!(p.get(1, 1) && p.get(14, 1) && p.get(14, 14) && p.get(1, 14));
        assert!(!p.get(8, 8), "polygon outline is hollow");
    }

    #[test]
    fn halfblock_is_empty_tracks_pixels() {
        let mut c = HalfBlockCanvas::new(2, 1);
        assert!(c.is_empty());
        c.set_pixel(0, 0, (1, 2, 3));
        assert!(!c.is_empty());
    }

    // -----------------------------------------------------------------------
    // 2D segment clipping (hostile coordinates).
    //
    // Regression: `line` used to run Bresenham directly over arbitrary i32
    // endpoints, so a finite near-camera projection like (-10_000_000, 5) →
    // (10_000_000, 5) performed ~20 million iterations to draw one visible row,
    // and `x1 - x0` could overflow i32 in debug builds.
    // -----------------------------------------------------------------------

    #[test]
    fn clip_rejects_empty_and_accepts_inside() {
        assert!(clip_line_to_bounds(0, 0, 5, 5, -1, 3).is_none());
        assert!(clip_line_to_bounds(0, 0, 5, 5, 3, -1).is_none());
        let (a, b) = clip_line_to_bounds(1, 1, 3, 2, 5, 5).unwrap();
        assert_eq!((a, b), ((1, 1), (3, 2)));
    }

    #[test]
    fn clip_truncates_huge_horizontal_line() {
        let (a, b) = clip_line_to_bounds(-10_000_000, 5, 10_000_000, 5, 79, 23).unwrap();
        assert_eq!(a, (0, 5));
        assert_eq!(b, (79, 5));
    }

    #[test]
    fn clip_truncates_huge_vertical_line() {
        let (a, b) = clip_line_to_bounds(5, -10_000_000, 5, 10_000_000, 79, 23).unwrap();
        assert_eq!(a, (5, 0));
        assert_eq!(b, (5, 23));
    }

    #[test]
    fn clip_truncates_huge_diagonal_line() {
        let (a, b) =
            clip_line_to_bounds(-9_000_000, -9_000_000, 9_000_000, 9_000_000, 79, 23).unwrap();
        // The diagonal crosses (0,0) and exits at the first bound it hits.
        assert_eq!(a, (0, 0));
        assert_eq!(b, (23, 23));
    }

    #[test]
    fn clip_drops_fully_outside_line() {
        assert!(clip_line_to_bounds(-5, 100, 200, 100, 79, 23).is_none());
        assert!(clip_line_to_bounds(100, 0, 100, 50, 79, 23).is_none());
        assert!(clip_line_to_bounds(-10, -10, -1, -1, 79, 23).is_none());
    }

    #[test]
    fn clip_handles_near_i32_extreme_coordinates() {
        // Differences near 2^32 are exact in f64; no overflow is possible.
        let (a, b) = clip_line_to_bounds(i32::MIN, 5, i32::MAX, 5, 79, 23).unwrap();
        assert_eq!((a, b), ((0, 5), (79, 5)));
        let (a, b) = clip_line_to_bounds(i32::MIN, i32::MIN, i32::MAX, i32::MAX, 79, 23).unwrap();
        assert_eq!(a, (0, 0));
        assert_eq!(b, (23, 23));
        // A degenerate point outside the canvas is rejected, inside is kept.
        assert!(clip_line_to_bounds(i32::MIN, i32::MIN, i32::MIN, i32::MIN, 79, 23).is_none());
        assert_eq!(
            clip_line_to_bounds(3, 4, 3, 4, 79, 23).unwrap(),
            ((3, 4), (3, 4))
        );
    }

    #[test]
    fn huge_lines_are_clipped_by_the_canvas_and_do_not_panic() {
        // Braille canvas: 4x2 cells = 8x8 dots. A line spanning tens of millions
        // of dots must draw only the visible diagonal and return promptly.
        let mut c = BrailleCanvas::new(4, 2);
        c.line(-10_000_000, 5, 10_000_000, 5);
        assert!(c.get(0, 5) && c.get(7, 5));
        assert!(!c.get(0, 4) && !c.get(0, 6));

        // Vertical and diagonal crossings.
        let mut v = BrailleCanvas::new(4, 2);
        v.line(3, -10_000_000, 3, 10_000_000);
        assert!(v.get(3, 0) && v.get(3, 7));

        let mut d = BrailleCanvas::new(4, 2);
        d.line(-10_000_000, -10_000_000, 10_000_000, 10_000_000);
        assert!(d.get(0, 0) && d.get(7, 7));

        // Fully outside: no dots, no panic.
        let mut o = BrailleCanvas::new(4, 2);
        o.line(-5, 100, 200, 100);
        assert!(o.is_empty());

        // Near-i32 extremes.
        let mut x = BrailleCanvas::new(4, 2);
        x.line(i32::MIN, 5, i32::MAX, 5);
        assert!(x.get(0, 5) && x.get(7, 5));

        // Half-block: 3 cells = 3 columns x 6 pixel rows.
        let mut hb = HalfBlockCanvas::new(3, 3);
        hb.line(-10_000_000, 2, 10_000_000, 2, (1, 2, 3));
        assert!(hb.get_pixel(0, 2).is_some() && hb.get_pixel(2, 2).is_some());
        hb.line(1, i32::MIN, 1, i32::MAX, (9, 9, 9));
        assert!(hb.get_pixel(1, 0).is_some() && hb.get_pixel(1, 5).is_some());
    }

    #[test]
    fn zero_and_single_pixel_canvases_are_safe() {
        let mut zero = BrailleCanvas::new(0, 0);
        zero.line(0, 0, 100, 100); // must not panic or hang
        assert!(zero.is_empty());

        let mut one = BrailleCanvas::new(1, 1); // 2x4 dots
        one.line(-10_000_000, -10_000_000, 10_000_000, 10_000_000);
        assert!(!one.is_empty());

        let mut zero_hb = HalfBlockCanvas::new(0, 0);
        zero_hb.line(0, 0, 100, 100, (1, 2, 3));
        assert!(zero_hb.is_empty());

        let mut one_hb = HalfBlockCanvas::new(1, 1); // 1x2 pixels
        one_hb.line(-10_000_000, 0, 10_000_000, 1, (4, 5, 6));
        assert!(!one_hb.is_empty());
    }

    #[test]
    fn clipping_is_idempotent_for_inside_lines() {
        // An already-in-bounds segment must be unchanged by the clip.
        let mut a = BrailleCanvas::new(8, 4);
        let mut b = BrailleCanvas::new(8, 4);
        a.line(1, 2, 14, 9);
        // Raw Bresenham equivalent (all coordinates in bounds for a 16x16 grid).
        let ((x0, y0), (x1, y1)) = clip_line_to_bounds(1, 2, 14, 9, 15, 15).unwrap();
        b.line(x0, y0, x1, y1);
        assert_eq!(a, b);
    }
}
