//! The terminal adapter's per-frame graphics plan for the kitty placeholder route (v0.9 design,
//! `docs/design/v0.9-kitty-graphics-route.md` section 18).
//!
//! RED stub: v0.9.0 design, not implemented. The types and signatures are the contract; every
//! method is deliberately wrong (`place` always falls back, `end_frame` is always empty) so the
//! `red_*` tests in `tests/kitty_frame_plan.rs` fail on assertions until GREEN lands.
//!
//! Bytes and `Surface`s only: nothing here writes to a terminal, and nothing is wired into the
//! renderer, `Context` or the session.

use super::kitty::ImageCache;
use crate::raster::RgbaRaster;
use crate::surface::Surface;

/// Why a box was not drawn with placeholders. The caller draws the cell-encoder form instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BoxFallback {
    /// `ImageCache::ensure` refused the image (empty, or over `max_upload_bytes`).
    Refused,
    /// The same image id already has a box of a different size in this frame (design section 7:
    /// one box size per id).
    SizeConflict,
    /// The box has zero columns or zero rows.
    EmptyBox,
}

/// What `KittyFramePlan::place` decided for one image box.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BoxPlacement {
    /// Paint `cells` into the box. Its graphics bytes are queued in the plan.
    Pixels {
        /// The image id the cells carry.
        id: u32,
        /// The `cols x rows` placeholder cells (see `placeholder_cells`).
        cells: Surface,
    },
    /// Paint the cell-encoder form of the same box; nothing was queued for it.
    Fallback(BoxFallback),
}

/// Per-frame graphics plan, owned by the terminal adapter (design section 16, ruling 5). Not
/// part of `Renderer` or `Context`.
///
/// Per frame: [`begin_frame`](Self::begin_frame), one [`place`](Self::place) per image box,
/// [`end_frame`](Self::end_frame) (the bytes to write before the frame's cell bytes, in the same
/// transaction), then [`commit`](Self::commit) once the write succeeded.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct KittyFramePlan {
    fallbacks: Vec<BoxFallback>,
}

impl KittyFramePlan {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a frame. Anything the previous frame emitted but never committed is treated as not
    /// resident.
    // RED stub: v0.9.0 design, not implemented
    pub fn begin_frame(&mut self) {}

    /// Plans one image box of `cols x rows` cells for `raster`. The caller has already passed
    /// `pixel_route`; this decides residency, queues the graphics bytes and returns the cells.
    // RED stub: v0.9.0 design, not implemented
    pub fn place(
        &mut self,
        _cache: &mut ImageCache,
        _raster: &RgbaRaster,
        _cols: u16,
        _rows: u16,
    ) -> BoxPlacement {
        BoxPlacement::Fallback(BoxFallback::Refused)
    }

    /// Ends the frame and returns all of its graphics bytes: uploads (with any eviction deletes
    /// `ImageCache::ensure` put in front of them), then deletes, then placements. Empty when
    /// nothing changed. Write them before the frame's cell bytes, then call
    /// [`commit`](Self::commit).
    // RED stub: v0.9.0 design, not implemented
    pub fn end_frame(&mut self) -> Vec<u8> {
        Vec::new()
    }

    /// Confirms that the bytes of the last [`end_frame`](Self::end_frame) were written
    /// successfully: only then are its uploads resident and its placements recorded.
    // RED stub: v0.9.0 design, not implemented
    pub fn commit(&mut self) {}

    /// The fallbacks `place` reported since the last [`begin_frame`](Self::begin_frame), in order.
    // RED stub: v0.9.0 design, not implemented
    pub fn fallbacks(&self) -> &[BoxFallback] {
        &self.fallbacks
    }
}
