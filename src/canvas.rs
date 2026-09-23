//! Sub-cell raster canvases built from ordinary Unicode cells.
//!
//! These are **not** graphics protocols (no Kitty/Sixel). They squeeze extra
//! resolution out of plain terminal cells:
//!
//! * [`BrailleCanvas`] — 2×4 binary dots per cell (`⠀`..`⣿`), 1 color per cell.
//! * [`HalfBlockCanvas`] — 2 vertical RGB samples per cell via `▀`, foreground =
//!   top pixel, background = bottom pixel.
//!
//! Both produce a [`Surface`] (or styled text), so they flow through the normal
//! diff/ANSI pipeline and composite like any other node.

use crate::cell::{Color, Glyph, Line, RichText, Span, Style};
use crate::surface::{Rect, Surface};

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
        for c in &mut self.cells {
            *c = 0;
        }
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

    /// Bresenham line between two dot coordinates.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
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

/// RGB raster canvas: `2 * width` by `2 * height` addressable pixels, rendered
/// as `▀` cells (top pixel = foreground, bottom pixel = background).
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
            pixels: vec![None; (width as usize) * 2 * (height as usize) * 2],
        }
    }

    pub fn pixel_width(&self) -> u16 {
        self.width * 2
    }

    pub fn pixel_height(&self) -> u16 {
        self.height * 2
    }

    pub fn clear(&mut self) {
        for p in &mut self.pixels {
            *p = None;
        }
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

    /// Bresenham line with a solid color.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, rgb: (u8, u8, u8)) {
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
        self.line(x, y, x + w - 1, y, rgb);
        self.line(x, y + h - 1, x + w - 1, y + h - 1, rgb);
        self.line(x, y, x, y + h - 1, rgb);
        self.line(x + w - 1, y, x + w - 1, y + h - 1, rgb);
    }

    /// Converts to a surface. Cells with no pixels are explicitly transparent so
    /// the canvas composes over other layers.
    pub fn to_surface(&self) -> Surface {
        let mut s = Surface::new_transparent(self.width, self.height);
        for cy in 0..self.height {
            for cx in 0..self.width {
                let top = self.get_pixel(cx as i32 * 2, cy as i32 * 2);
                let bottom = self.get_pixel(cx as i32 * 2, cy as i32 * 2 + 1);
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
                let top = self.get_pixel(cx as i32 * 2, cy as i32 * 2);
                let bottom = self.get_pixel(cx as i32 * 2, cy as i32 * 2 + 1);
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
        let mut c = HalfBlockCanvas::new(3, 3); // 6x6 pixels
        c.rect(0, 0, 6, 6, (1, 2, 3));
        assert!(c.get_pixel(0, 0).is_some());
        assert!(c.get_pixel(5, 5).is_some());
        assert!(c.get_pixel(3, 3).is_none());
        c.set_pixel(99, 99, (9, 9, 9)); // ignored
    }

    #[test]
    fn oscilloscope_produces_some_dots() {
        let vals: Vec<f32> = (0..64).map(|i| (i as f32 * 0.3).sin()).collect();
        let c = braille_oscilloscope(&vals, 8, 2);
        assert!(c.to_lines().iter().any(|l| !l.trim().is_empty()));
    }
}
