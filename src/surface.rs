use crate::cell::{Cell, Glyph, Style};
use unicode_segmentation::UnicodeSegmentation;

/// A 2D integer rectangle in cell coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub const fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x
            && x < self.x.saturating_add(self.width)
            && y >= self.y
            && y < self.y.saturating_add(self.height)
    }

    pub fn intersection(&self, other: &Rect) -> Rect {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        if x2 > x1 && y2 > y1 {
            Rect::new(x1, y1, x2 - x1, y2 - y1)
        } else {
            Rect::new(0, 0, 0, 0)
        }
    }

    /// Shrinks this rectangle inward on every side by `amount` (saturating).
    pub fn shrink(&self, amount: u16) -> Rect {
        if self.width <= amount * 2 || self.height <= amount * 2 {
            return Rect::new(self.x, self.y, 0, 0);
        }
        Rect::new(
            self.x + amount,
            self.y + amount,
            self.width - amount * 2,
            self.height - amount * 2,
        )
    }
}

/// Border styling variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderType {
    #[default]
    Single,
    Double,
    Rounded,
    Thick,
    Ascii,
}

impl BorderType {
    pub fn chars(
        &self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    ) {
        // (top_left, top_right, bottom_left, bottom_right, horizontal, vertical)
        match self {
            BorderType::Single => ("┌", "┐", "└", "┘", "─", "│"),
            BorderType::Double => ("╔", "╗", "╚", "╝", "═", "║"),
            BorderType::Rounded => ("╭", "╮", "╰", "╯", "─", "│"),
            BorderType::Thick => ("┏", "┓", "┗", "┛", "━", "┃"),
            BorderType::Ascii => ("+", "+", "+", "+", "-", "|"),
        }
    }
}

/// A rectangular 2D framebuffer of cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Cell>,
}

impl Surface {
    pub fn new(width: u16, height: u16) -> Self {
        let count = (width as usize) * (height as usize);
        Self {
            width,
            height,
            cells: vec![Cell::default(); count],
        }
    }

    /// Creates a fully transparent surface, used as a scratch layer for
    /// compositing overlays. Untouched cells contribute nothing.
    pub fn new_transparent(width: u16, height: u16) -> Self {
        let count = (width as usize) * (height as usize);
        Self {
            width,
            height,
            cells: vec![Cell::transparent(); count],
        }
    }

    /// Composites `src` onto this surface at the origin.
    ///
    /// Transparent source cells leave this surface untouched; style-only source
    /// cells merge their style onto the destination cell's style without
    /// replacing its glyph; opaque source cells replace the destination cell.
    pub fn blit_transparent_at(&mut self, src: &Surface, dx: u16, dy: u16) {
        self.blit_transparent_clipped(src, dx as i32, dy as i32, self.area());
    }

    /// Composites `src` onto this surface with a signed destination origin,
    /// writing only cells inside `clip`.
    ///
    /// This is the clipping-aware form used by the raster and camera nodes: a
    /// source may be partly (or fully) off the clip rectangle, including negative
    /// origins, and wide-glyph invariants are preserved by `set_cell`.
    pub fn blit_transparent_clipped(&mut self, src: &Surface, dx: i32, dy: i32, clip: Rect) {
        let clip = self.area().intersection(&clip);
        if clip.is_empty() {
            return;
        }
        for y in 0..src.height {
            for x in 0..src.width {
                let Some(c) = src.get(x, y) else { continue };
                if c.transparent {
                    continue;
                }
                let tx = dx + x as i32;
                let ty = dy + y as i32;
                if tx < clip.x as i32 || ty < clip.y as i32 {
                    continue;
                }
                let (tx, ty) = (tx as u16, ty as u16);
                if !clip.contains(tx, ty) {
                    continue;
                }
                if c.style_only {
                    if let Some(dst) = self.get_mut(tx, ty) {
                        dst.style = dst.style.overlay(c.style);
                    }
                } else {
                    let cell = c.clone();
                    self.set_cell(tx, ty, cell);
                }
            }
        }
    }

    /// Composites `src` onto this surface at the origin.
    pub fn blit_transparent(&mut self, src: &Surface) {
        self.blit_transparent_at(src, 0, 0);
    }

    /// Applies a dim veil to `rect`.
    ///
    /// Transparent cells in the region become style-only dim cells (so they can
    /// later be composited as a veil); opaque cells get the dim attribute merged
    /// in place.
    pub fn apply_dim_rect(&mut self, rect: Rect) {
        let inter = self.area().intersection(&rect);
        if inter.is_empty() {
            return;
        }
        for y in inter.y..(inter.y + inter.height) {
            for x in inter.x..(inter.x + inter.width) {
                if let Some(idx) = self.index(x, y) {
                    if self.cells[idx].transparent {
                        self.cells[idx] = Cell::style_overlay(Style::new().dim());
                    } else {
                        self.cells[idx].style = self.cells[idx].style.overlay(Style::new().dim());
                    }
                }
            }
        }
    }

    pub fn area(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    #[inline]
    pub fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y as usize) * (self.width as usize) + (x as usize))
        } else {
            None
        }
    }

    #[inline]
    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index(x, y).map(|i| &self.cells[i])
    }

    #[inline]
    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        if let Some(i) = self.index(x, y) {
            Some(&mut self.cells[i])
        } else {
            None
        }
    }

    /// Sets a cell with wide glyph overwrite protection.
    ///
    /// Invariants:
    /// - Overwriting a continuation cell clears its preceding wide lead glyph.
    /// - Overwriting a wide lead cell clears its trailing continuation cell.
    /// - Placing a wide glyph clears any overlapping wide glyph continuation.
    /// - Wide glyphs at the rightmost boundary are not split; space is emitted instead.
    pub fn set_cell(&mut self, x: u16, y: u16, cell: Cell) -> bool {
        let idx = match self.index(x, y) {
            Some(i) => i,
            None => return false,
        };

        // If current cell is a continuation, clear the lead cell to its left
        if self.cells[idx].is_continuation && x > 0 {
            let left_idx = idx - 1;
            let left_style = self.cells[left_idx].style;
            self.cells[left_idx] = Cell::space(left_style);
        }

        // If current cell is a wide lead, clear the continuation cell to its right
        if self.cells[idx].glyph.display_width == 2 && x + 1 < self.width {
            let right_idx = idx + 1;
            let right_style = self.cells[right_idx].style;
            self.cells[right_idx] = Cell::space(right_style);
        }

        if cell.glyph.display_width == 2 {
            // Cannot place wide character at the right edge
            if x + 1 >= self.width {
                self.cells[idx] = Cell::space(cell.style);
                return false;
            }

            let next_idx = idx + 1;
            // If the next cell was itself a wide lead, clear its continuation cell
            if self.cells[next_idx].glyph.display_width == 2 && x + 2 < self.width {
                let right2_idx = idx + 2;
                let right2_style = self.cells[right2_idx].style;
                self.cells[right2_idx] = Cell::space(right2_style);
            }

            let style = cell.style;
            self.cells[idx] = cell;
            self.cells[next_idx] = Cell::continuation(style);
        } else {
            self.cells[idx] = cell;
        }

        true
    }

    /// Prints a string at (x, y) respecting grapheme clusters, styles, and wide character boundaries.
    /// Returns the number of columns advanced.
    pub fn print_str(
        &mut self,
        x: u16,
        y: u16,
        text: &str,
        style: Style,
        max_width: Option<u16>,
    ) -> u16 {
        if y >= self.height || x >= self.width {
            return 0;
        }

        let max_adv = max_width.unwrap_or(self.width.saturating_sub(x));
        let end_x = (x + max_adv).min(self.width);

        let mut curr_x = x;
        for grapheme in text.graphemes(true) {
            if grapheme == "\r" || grapheme == "\n" {
                break;
            }
            // Never allow terminal control characters to reach the cell model:
            // untrusted text must not be able to inject ESC/OSC/CSI sequences.
            if grapheme.chars().any(|c| c.is_control()) {
                continue;
            }

            let glyph = Glyph::new(grapheme);
            let w = glyph.display_width as u16;

            if w == 0 {
                // Zero-width character: append to previous glyph if possible
                if curr_x > x {
                    let prev_idx = if curr_x > 1
                        && self.cells[self.index(curr_x - 1, y).unwrap()].is_continuation
                    {
                        self.index(curr_x - 2, y).unwrap()
                    } else {
                        self.index(curr_x - 1, y).unwrap()
                    };
                    self.cells[prev_idx].glyph.grapheme.push_str(grapheme);
                }
                continue;
            }

            if curr_x + w > end_x {
                break;
            }

            let cell = Cell::new(glyph, style);
            self.set_cell(curr_x, y, cell);
            curr_x += w;
        }

        curr_x - x
    }

    /// Fills a rectangular region with a given cell.
    pub fn fill_rect(&mut self, rect: Rect, cell: Cell) {
        let inter = self.area().intersection(&rect);
        if inter.is_empty() {
            return;
        }

        for y in inter.y..(inter.y + inter.height) {
            for x in inter.x..(inter.x + inter.width) {
                self.set_cell(x, y, cell.clone());
            }
        }
    }

    /// Draws a border within the given rectangle.
    pub fn draw_border(&mut self, rect: Rect, border_type: BorderType, style: Style) {
        if rect.width < 2 || rect.height < 2 {
            return;
        }

        let (tl, tr, bl, br, h, v) = border_type.chars();

        // Corners
        self.set_cell(rect.x, rect.y, Cell::new(Glyph::new(tl), style));
        self.set_cell(
            rect.x + rect.width - 1,
            rect.y,
            Cell::new(Glyph::new(tr), style),
        );
        self.set_cell(
            rect.x,
            rect.y + rect.height - 1,
            Cell::new(Glyph::new(bl), style),
        );
        self.set_cell(
            rect.x + rect.width - 1,
            rect.y + rect.height - 1,
            Cell::new(Glyph::new(br), style),
        );

        // Top and bottom edges
        for x in (rect.x + 1)..(rect.x + rect.width - 1) {
            self.set_cell(x, rect.y, Cell::new(Glyph::new(h), style));
            self.set_cell(x, rect.y + rect.height - 1, Cell::new(Glyph::new(h), style));
        }

        // Left and right edges
        for y in (rect.y + 1)..(rect.y + rect.height - 1) {
            self.set_cell(rect.x, y, Cell::new(Glyph::new(v), style));
            self.set_cell(rect.x + rect.width - 1, y, Cell::new(Glyph::new(v), style));
        }
    }

    /// Resizes the surface, preserving existing cell contents.
    pub fn resize(&mut self, new_width: u16, new_height: u16) {
        if self.width == new_width && self.height == new_height {
            return;
        }

        let mut new_cells = vec![Cell::default(); (new_width as usize) * (new_height as usize)];
        let copy_w = self.width.min(new_width) as usize;
        let copy_h = self.height.min(new_height) as usize;

        for y in 0..copy_h {
            let old_start = y * (self.width as usize);
            let new_start = y * (new_width as usize);
            new_cells[new_start..(new_start + copy_w)]
                .clone_from_slice(&self.cells[old_start..(old_start + copy_w)]);
        }

        self.width = new_width;
        self.height = new_height;
        self.cells = new_cells;
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_ascii() {
        let mut surface = Surface::new(10, 2);
        let adv = surface.print_str(1, 0, "hello", Style::default(), None);
        assert_eq!(adv, 5);
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), "h");
        assert_eq!(surface.get(5, 0).unwrap().glyph.grapheme.as_str(), "o");
        assert_eq!(surface.get(6, 0).unwrap().glyph.grapheme.as_str(), " ");
    }

    #[test]
    fn test_wide_cjk_placement() {
        let mut surface = Surface::new(10, 2);
        let adv = surface.print_str(0, 0, "你好", Style::default(), None);
        assert_eq!(adv, 4);

        // First character
        let c0 = surface.get(0, 0).unwrap();
        assert_eq!(c0.glyph.grapheme.as_str(), "你");
        assert_eq!(c0.glyph.display_width, 2);
        assert!(!c0.is_continuation);

        let c1 = surface.get(1, 0).unwrap();
        assert!(c1.is_continuation);

        // Second character
        let c2 = surface.get(2, 0).unwrap();
        assert_eq!(c2.glyph.grapheme.as_str(), "好");
        assert_eq!(c2.glyph.display_width, 2);
        assert!(!c2.is_continuation);

        let c3 = surface.get(3, 0).unwrap();
        assert!(c3.is_continuation);
    }

    #[test]
    fn test_wide_overwrite_continuation() {
        // If we write at the continuation cell of a wide character, the lead character MUST be cleared
        let mut surface = Surface::new(10, 2);
        surface.print_str(0, 0, "你", Style::default(), None);
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "你");
        assert!(surface.get(1, 0).unwrap().is_continuation);

        // Overwrite column 1 with 'x'
        surface.set_cell(1, 0, Cell::new(Glyph::new("x"), Style::default()));

        // Column 0 must now be space, NOT half of "你"
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), " ");
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), "x");
        assert!(!surface.get(1, 0).unwrap().is_continuation);
    }

    #[test]
    fn test_wide_overwrite_lead_with_single() {
        // If we overwrite a wide lead with a single-width character, the continuation MUST be cleared
        let mut surface = Surface::new(10, 2);
        surface.print_str(0, 0, "你", Style::default(), None);
        assert!(surface.get(1, 0).unwrap().is_continuation);

        // Overwrite column 0 with 'a'
        surface.set_cell(0, 0, Cell::new(Glyph::new("a"), Style::default()));

        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "a");
        // Column 1 must now be space, not continuation
        assert!(!surface.get(1, 0).unwrap().is_continuation);
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), " ");
    }

    #[test]
    fn test_wide_boundary_clipping() {
        // A wide character at the right edge must not overflow
        let mut surface = Surface::new(3, 1);
        // Col 2 cannot hold a 2-width character (would need col 2 and 3)
        let adv = surface.print_str(2, 0, "你", Style::default(), None);
        assert_eq!(adv, 0);
        // Cell 2 should remain space
        assert_eq!(surface.get(2, 0).unwrap().glyph.grapheme.as_str(), " ");
    }

    #[test]
    fn test_border_drawing() {
        let mut surface = Surface::new(5, 3);
        surface.draw_border(Rect::new(0, 0, 5, 3), BorderType::Rounded, Style::default());
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "╭");
        assert_eq!(surface.get(4, 0).unwrap().glyph.grapheme.as_str(), "╮");
        assert_eq!(surface.get(0, 2).unwrap().glyph.grapheme.as_str(), "╰");
        assert_eq!(surface.get(4, 2).unwrap().glyph.grapheme.as_str(), "╯");
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), "─");
        assert_eq!(surface.get(0, 1).unwrap().glyph.grapheme.as_str(), "│");
    }
}
