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
