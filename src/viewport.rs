//! Scroll/pan state for a clipped viewport (camera) node.
//!
//! Integer cell offsets, deliberately small. The viewport node itself
//! ([`crate::Node::viewport`]) owns clipping and translation; this type owns the
//! camera position and clamping.

/// Camera offset in cell units. Negative values reveal content to the right/below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewportState {
    pub offset_x: i32,
    pub offset_y: i32,
}

impl ViewportState {
    pub const fn new() -> Self {
        Self {
            offset_x: 0,
            offset_y: 0,
        }
    }

    pub fn with_offset(x: i32, y: i32) -> Self {
        Self {
            offset_x: x,
            offset_y: y,
        }
    }

    pub fn scroll_by(&mut self, dx: i32, dy: i32) {
        self.offset_x += dx;
        self.offset_y += dy;
    }

    /// Page by one viewport height in `dir` (-1 up, +1 down).
    pub fn page(&mut self, view_height: u16, dir: i32) {
        self.offset_y = self
            .offset_y
            .saturating_add(dir * view_height.max(1) as i32);
    }

    pub fn home(&mut self) {
        self.offset_x = 0;
        self.offset_y = 0;
    }

    /// Jumps so the bottom of `content_height` aligns with the viewport bottom.
    pub fn end(&mut self, content_height: u16, view_height: u16) {
        self.offset_y = (content_height as i32 - view_height as i32).max(0);
    }

    /// Clamps the camera so the viewport never shows past the content edges.
    pub fn clamp(&mut self, content_width: u16, content_height: u16, view_w: u16, view_h: u16) {
        let max_x = (content_width as i32 - view_w as i32).max(0);
        let max_y = (content_height as i32 - view_h as i32).max(0);
        self.offset_x = self.offset_x.clamp(0, max_x);
        self.offset_y = self.offset_y.clamp(0, max_y);
    }

    /// Item indices (of `total_items`, each `item_height` cells tall) that are at
    /// least partially visible in a `view_height`-tall window at the current
    /// `offset_y`. Items partially clipped at the top or bottom edge are
    /// included. Returns an empty range when `item_height`, `total_items`, or
    /// `view_height` is zero. Argument order mirrors [`ViewportState::end`]
    /// (content first, then view).
    ///
    /// This is the windowing math a scrollable list would otherwise re-derive by
    /// hand; it does not consult `offset_x` (rows are laid out vertically).
    pub fn visible_range(
        &self,
        total_items: usize,
        item_height: u16,
        view_height: u16,
    ) -> std::ops::Range<usize> {
        if item_height == 0 || total_items == 0 || view_height == 0 {
            return 0..0;
        }
        let ih = item_height as i32;
        let top = self.offset_y.max(0);
        let bottom = top + view_height as i32 - 1; // inclusive last visible row
        let start = (top / ih) as usize;
        let end = ((bottom.max(0) / ih) as usize).saturating_add(1);
        let start = start.min(total_items);
        let end = end.min(total_items).max(start);
        start..end
    }

    /// Scrolls vertically only as far as needed so item `index` (each
    /// `item_height` cells tall) is fully within a `view_height`-tall window: up
    /// if the item is above the viewport, down if below, and untouched if it is
    /// already fully visible. No-op when `item_height` is zero.
    ///
    /// Like [`ViewportState::page`]/[`ViewportState::scroll_by`], this does not
    /// clamp against the content end; follow with [`ViewportState::clamp`] if the
    /// caller needs that invariant.
    pub fn ensure_visible(&mut self, index: usize, item_height: u16, view_height: u16) {
        if item_height == 0 {
            return;
        }
        let ih = item_height as i64;
        let top = (index as i64).saturating_mul(ih);
        let bottom = top + ih; // exclusive
        let view_h = view_height as i64;
        if top < self.offset_y as i64 {
            self.offset_y = top.clamp(0, i32::MAX as i64) as i32;
        } else if bottom > self.offset_y as i64 + view_h {
            self.offset_y = (bottom - view_h).clamp(0, i32::MAX as i64) as i32;
        }
        // Otherwise the item is already fully visible; leave the camera put.
    }

    /// Scrolls so item `index` (each `item_height` cells tall) sits at the top of
    /// the viewport. No-op when `item_height` is zero. Does not clamp against the
    /// content end; follow with [`ViewportState::clamp`] if needed.
    pub fn scroll_to_item(&mut self, index: usize, item_height: u16) {
        if item_height == 0 {
            return;
        }
        let top = (index as i64).saturating_mul(item_height as i64);
        self.offset_y = top.clamp(0, i32::MAX as i64) as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_camera_in_bounds() {
        let mut v = ViewportState::with_offset(-5, 999);
        v.clamp(100, 40, 20, 10);
        assert_eq!(v.offset_x, 0);
        assert_eq!(v.offset_y, 30);
    }

    #[test]
    fn end_then_clamp_bottoms_out() {
        let mut v = ViewportState::new();
        v.end(40, 10);
        assert_eq!(v.offset_y, 30);
        v.clamp(80, 40, 20, 10);
        assert_eq!(v.offset_y, 30);
    }

    #[test]
    fn page_and_home_behave() {
        let mut v = ViewportState::new();
        v.page(10, 1);
        assert_eq!(v.offset_y, 10);
        v.page(10, -1);
        assert_eq!(v.offset_y, 0);
        v.scroll_by(3, 4);
        v.home();
        assert_eq!(v, ViewportState::new());
    }

    #[test]
    fn content_smaller_than_viewport_clamps_to_zero() {
        let mut v = ViewportState::with_offset(50, 50);
        v.clamp(10, 5, 20, 10);
        assert_eq!(v.offset_x, 0);
        assert_eq!(v.offset_y, 0);
    }
}
