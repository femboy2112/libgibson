//! The terminal adapter's per-frame graphics plan for the kitty placeholder route (v0.9 design,
//! `docs/design/v0.9-kitty-graphics-route.md` section 18).
//!
//! GREEN: the behaviour is pinned by `tests/kitty_frame_plan.rs`.
//!
//! Bytes and `Surface`s only: nothing here writes to a terminal, and nothing is wired into the
//! renderer, `Context` or the session.

use super::kitty::{
    kitty_delete, kitty_place_virtual, placeholder_cells, ImageCache, MAX_PLACEHOLDER_DIM,
};
use crate::raster::RgbaRaster;
use crate::surface::Surface;
use std::collections::{BTreeMap, BTreeSet};
use std::mem;

/// Why a box was not drawn with placeholders. The caller draws the cell-encoder form instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BoxFallback {
    /// The image is refused: `ImageCache::ensure` refused it (empty, or over `max_upload_bytes`),
    /// or showing it would need a second eviction while one eviction delete is still unconfirmed
    /// (design section 18, rule 10: residency never exceeds the cap plus one).
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

/// Where the in-flight frame is in its life.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// No frame to confirm: fresh, committed or reset.
    #[default]
    Idle,
    /// `begin_frame` ran; `end_frame` has not handed the bytes over yet.
    Open,
    /// `end_frame` handed the bytes over; `commit` has not confirmed the write yet.
    Ended,
}

/// The start of an upload APC (`ESC _ G a=t`). `ImageCache::ensure` returns "eviction deletes,
/// then the transmit", and a base64 payload never contains ESC, so the first occurrence splits
/// the two exactly.
const UPLOAD_APC: &[u8] = b"\x1b_Ga=t";

/// **EXPERIMENTAL** per-frame graphics plan, owned by the terminal adapter (design section 16,
/// ruling 5). Not part of `Renderer` or `Context`.
///
/// Per frame: [`begin_frame`](Self::begin_frame), one [`place`](Self::place) per image box,
/// [`end_frame`](Self::end_frame) (the bytes to write before the frame's cell bytes, in the same
/// transaction), then [`commit`](Self::commit) once the write succeeded.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct KittyFramePlan {
    fallbacks: Vec<BoxFallback>,
    phase: Phase,
    /// Committed placement records: image id to the box size it was last placed at.
    placed: BTreeMap<u32, (u16, u16)>,
    /// Ids uploaded by an abandoned frame: the cache thinks they are resident, the terminal may
    /// not have them.
    suspect: BTreeSet<u32>,
    /// Eviction-delete bytes carried by the current frame and not yet confirmed written.
    unconfirmed_evictions: Vec<u8>,
    /// The in-flight frame: uploads (with their eviction deletes), resize deletes, placements.
    uploads: Vec<u8>,
    deletes: Vec<u8>,
    places: Vec<u8>,
    /// Ids the in-flight frame uploaded (suspect if the frame is abandoned).
    frame_uploaded: Vec<u32>,
    /// The first box size each id got in the in-flight frame (recorded on commit).
    frame_sizes: BTreeMap<u32, (u16, u16)>,
}

impl KittyFramePlan {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a frame. Anything the previous frame emitted but never committed is treated as not
    /// resident.
    pub fn begin_frame(&mut self) {
        if self.phase != Phase::Idle {
            // Ended-not-committed or never ended: the write is not confirmed, so the ids it
            // uploaded are suspect and its queued bytes are dropped. Unconfirmed evictions stay.
            self.suspect.extend(mem::take(&mut self.frame_uploaded));
        }
        self.uploads.clear();
        self.deletes.clear();
        self.places.clear();
        self.frame_uploaded.clear();
        self.frame_sizes.clear();
        self.fallbacks.clear();
        // An eviction delete the cache already paid for is re-sent, first, ahead of any upload.
        self.uploads.extend_from_slice(&self.unconfirmed_evictions);
        self.phase = Phase::Open;
    }

    /// Plans one image box of `cols x rows` cells for `raster`. The caller has already passed
    /// `pixel_route`; this decides residency, queues the graphics bytes and returns the cells.
    /// A box that would need an eviction while an earlier eviction delete is unconfirmed (in this
    /// frame or an abandoned one) returns `Fallback(Refused)`, is reported and queues nothing.
    /// Calling it with no open frame starts one.
    pub fn place(
        &mut self,
        cache: &mut ImageCache,
        raster: &RgbaRaster,
        cols: u16,
        rows: u16,
    ) -> BoxPlacement {
        if self.phase != Phase::Open {
            self.begin_frame();
        }
        if cols == 0 || rows == 0 {
            return self.fall_back(BoxFallback::EmptyBox);
        }
        let size = (cols.min(MAX_PLACEHOLDER_DIM), rows.min(MAX_PLACEHOLDER_DIM));

        // Rule 10: peek before `ensure`, so a refused box leaves the cache untouched.
        if cache.would_evict(raster) && !self.unconfirmed_evictions.is_empty() {
            return self.fall_back(BoxFallback::Refused);
        }
        let (mut id, mut upload) = cache.ensure(raster);
        if id == 0 {
            return self.fall_back(BoxFallback::Refused);
        }
        if upload.is_none() && self.suspect.contains(&id) {
            // Resident in the cache, but its upload was never confirmed written: upload again.
            cache.forget(id);
            (id, upload) = cache.ensure(raster);
            if id == 0 {
                return self.fall_back(BoxFallback::Refused);
            }
        }
        let fresh = upload.is_some();
        if let Some(bytes) = upload {
            let split = bytes
                .windows(UPLOAD_APC.len())
                .position(|w| w == UPLOAD_APC)
                .unwrap_or(bytes.len());
            self.unconfirmed_evictions
                .extend_from_slice(&bytes[..split]);
            self.uploads.extend_from_slice(&bytes);
            self.frame_uploaded.push(id);
            self.suspect.remove(&id);
        }

        if self.frame_sizes.get(&id).is_some_and(|&s| s != size) {
            return self.fall_back(BoxFallback::SizeConflict);
        }
        if fresh {
            // An upload implies a fresh placement; a recorded size may be stale (rule 5).
            self.places.extend(kitty_place_virtual(id, size.0, size.1));
        } else if !self.frame_sizes.contains_key(&id) {
            match self.placed.get(&id).copied() {
                Some(recorded) if recorded == size => {}
                Some(_) => {
                    self.deletes.extend(kitty_delete(id, false));
                    self.places.extend(kitty_place_virtual(id, size.0, size.1));
                }
                None => self.places.extend(kitty_place_virtual(id, size.0, size.1)),
            }
        }
        self.frame_sizes.insert(id, size);
        BoxPlacement::Pixels {
            id,
            cells: placeholder_cells(id, size.0, size.1),
        }
    }

    fn fall_back(&mut self, why: BoxFallback) -> BoxPlacement {
        self.fallbacks.push(why);
        BoxPlacement::Fallback(why)
    }

    /// Ends the frame and returns all of its graphics bytes: uploads (with any eviction deletes
    /// `ImageCache::ensure` put in front of them), then deletes, then placements. Empty when
    /// nothing changed. Write them before the frame's cell bytes, then call
    /// [`commit`](Self::commit).
    pub fn end_frame(&mut self) -> Vec<u8> {
        if self.phase != Phase::Open {
            return Vec::new();
        }
        self.phase = Phase::Ended;
        let mut out = mem::take(&mut self.uploads);
        out.append(&mut self.deletes);
        out.append(&mut self.places);
        out
    }

    /// Confirms that the bytes of the last [`end_frame`](Self::end_frame) were written
    /// successfully: only then are its uploads resident and its placements recorded.
    pub fn commit(&mut self) {
        if self.phase != Phase::Ended {
            return;
        }
        self.placed.append(&mut self.frame_sizes);
        self.frame_uploaded.clear();
        self.unconfirmed_evictions.clear();
        self.phase = Phase::Idle;
    }

    /// Forgets all residency and placement records and any unconfirmed eviction delete. For
    /// teardown and torn-write recovery, after the caller has written the delete-all bytes and
    /// cleared its `ImageCache`.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The fallbacks `place` reported since the last [`begin_frame`](Self::begin_frame), in order.
    pub fn fallbacks(&self) -> &[BoxFallback] {
        &self.fallbacks
    }
}
