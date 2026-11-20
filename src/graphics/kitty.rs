//! Kitty graphics protocol, Unicode-placeholder route (v0.9/v0.10 design).
//!
//! The image bytes travel as APC sequences that the application writes itself; the picture
//! then appears wherever ordinary text cells hold U+10EEEE with the right colour and
//! diacritics. Those cells live in a [`Surface`], so the existing diff gives occlusion,
//! clipping and restore with no image-specific code.
//!
//! Every function and type below is a **RED stub**: v0.9.0 design, not implemented. Stubs
//! return empty or neutral values so that `tests/kitty_graphics.rs` compiles and fails on
//! assertions, not on missing symbols. See `docs/design/v0.9-kitty-graphics-route.md`.
//!
//! Integration seams (own stub types; no dependency on unmerged work):
//! * `CellEncoder` (branch `feat/v0.9-cell-encoder`) is the fallback: [`PixelRoute::Fallback`]
//!   means "draw the same box with the cell encoder the application chose".
//! * The parallel probe slice's tri-state `kitty_graphics` verdict maps onto
//!   [`crate::capability::Capability`] (`Supported` / `Unsupported` / `Unknown`), which is what
//!   [`pixel_route`] takes today.

use crate::capability::{Capability, ColorDepth};
use crate::raster::RgbaRaster;
use crate::surface::Surface;

/// The placeholder codepoint: a Private Use character the terminal replaces with image pixels.
pub const PLACEHOLDER: char = '\u{10EEEE}';

/// Largest base64 payload of one transmit chunk, in bytes (the protocol's documented maximum).
pub const MAX_CHUNK_BASE64: usize = 4096;

/// Largest image id this route uses. Ids are carried by the 24-bit foreground colour, so the
/// optional third (most-significant-byte) diacritic is never needed.
pub const MAX_IMAGE_ID: u32 = 0x00FF_FFFF;

/// Number of row/column diacritics in the protocol's `rowcolumn-diacritics.txt` table. A
/// placeholder box can be at most this many cells in either direction.
pub const MAX_PLACEHOLDER_DIM: u16 = 297;

/// Transmit (not display) an image: chunked base64 RGBA over APC, quiet (`q=2`).
///
/// Shape of a single-chunk result: `ESC _ G a=t,f=32,s=<w>,v=<h>,i=<id>,q=2 ; <base64> ESC \`.
/// With more than one chunk the first chunk adds `,m=1`; the rest are `ESC _ G q=2,m=1 ; ...`
/// and the last is `ESC _ G q=2,m=0 ; ...`. No chunk's payload exceeds
/// [`MAX_CHUNK_BASE64`] and every chunk but the last is a multiple of 4.
// RED stub: v0.9.0 design, not implemented
pub fn kitty_transmit_rgba(_id: u32, _w: u32, _h: u32, _rgba: &[u8]) -> Vec<u8> {
    Vec::new()
}

/// Create the virtual placement of image `id` over `cols x rows` cells:
/// `ESC _ G a=p,U=1,i=<id>,c=<cols>,r=<rows>,q=2 ESC \`.
// RED stub: v0.9.0 design, not implemented
pub fn kitty_place_virtual(_id: u32, _cols: u16, _rows: u16) -> Vec<u8> {
    Vec::new()
}

/// Delete image `id` by id: `ESC _ G a=d,d=i,i=<id>,q=2 ESC \`. With `free` the data is freed
/// too (`d=I`); without it the data stays so the image can be shown again with no re-upload.
// RED stub: v0.9.0 design, not implemented
pub fn kitty_delete(_id: u32, _free: bool) -> Vec<u8> {
    Vec::new()
}

/// Free every id in `ids` (sorted ascending, duplicates dropped) with `kitty_delete(id, true)`.
/// This is the teardown and panic-hook payload. It deletes by id because `a=d,d=a` / `d=A`
/// "never affect virtual placements" (protocol page).
// RED stub: v0.9.0 design, not implemented
pub fn kitty_delete_all(_ids: &[u32]) -> Vec<u8> {
    Vec::new()
}

/// The diacritic that encodes row or column number `n` (0-based), from the protocol's
/// `rowcolumn-diacritics.txt` (297 entries; `0` is U+0305, `1` is U+030D). `None` when
/// `n >= MAX_PLACEHOLDER_DIM`.
// RED stub: v0.9.0 design, not implemented
pub fn rowcolumn_diacritic(_n: u16) -> Option<char> {
    None
}

/// A `cols x rows` surface of placeholder cells for image `id`. The cell at `(c, r)` holds
/// U+10EEEE + the row-`r` diacritic + the column-`c` diacritic, with foreground
/// `Color::Rgb(id >> 16, id >> 8, id)` and no other style. Both diacritics are always written
/// (never inferred), so a cell is correct on its own wherever the diff or a clip puts it.
/// `cols` and `rows` are clamped to [`MAX_PLACEHOLDER_DIM`].
// RED stub: v0.9.0 design, not implemented
pub fn placeholder_cells(_id: u32, cols: u16, rows: u16) -> Surface {
    Surface::new(cols, rows)
}

/// Byte-budget knobs of the [`ImageCache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheLimits {
    /// An image whose raw RGBA size exceeds this is never uploaded (fallback + report).
    pub max_upload_bytes: usize,
    /// Base64 payload bytes allowed per frame; the rest is deferred to later frames.
    pub max_frame_bytes: usize,
    /// Images kept resident on the terminal; the least recently shown is freed beyond this.
    pub max_resident_images: usize,
}

impl Default for CacheLimits {
    // RED stub: v0.9.0 design, not implemented (values are the proposal, nothing enforces them)
    fn default() -> Self {
        Self {
            max_upload_bytes: 4 * 1024 * 1024,
            max_frame_bytes: 256 * 1024,
            max_resident_images: 16,
        }
    }
}

/// Content-hash id cache: the same pixels always map to the same id and are uploaded once.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ImageCache {}

impl ImageCache {
    // RED stub: v0.9.0 design, not implemented
    pub fn new() -> Self {
        Self {}
    }

    // RED stub: v0.9.0 design, not implemented
    pub fn with_limits(_limits: CacheLimits) -> Self {
        Self {}
    }

    /// Returns the id for `raster` (in `1..=MAX_IMAGE_ID`, a pure function of its size and
    /// pixels) and, the first time only, the transmit bytes to write before any frame that
    /// shows it. A repeat returns `(same id, None)`.
    // RED stub: v0.9.0 design, not implemented
    pub fn ensure(&mut self, _raster: &RgbaRaster) -> (u32, Option<Vec<u8>>) {
        (0, None)
    }

    /// Bytes that free every image this cache transmitted (see [`kitty_delete_all`]).
    // RED stub: v0.9.0 design, not implemented
    pub fn teardown_bytes(&self) -> Vec<u8> {
        Vec::new()
    }
}

/// Why the pixel route is not used for a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FallbackReason {
    /// The application did not set the capability itself (`Unknown`), so nothing is assumed.
    NotOptedIn,
    /// The application says the terminal does not support it.
    Unsupported,
    /// Inside tmux or screen: placeholders and APC need passthrough that we do not assume.
    Multiplexer,
    /// Over ssh: latency and passthrough make a silent failure likely.
    Remote,
    /// The colour depth is below 24-bit, which would corrupt the id carried in the colour.
    NotTrueColor,
}

/// The route decision for an image box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PixelRoute {
    /// Use kitty placeholders for the box.
    Pixels,
    /// Use the cell encoder the application chose, in the same box.
    Fallback(FallbackReason),
}

/// Pure route gate. `cap` is the application's own capability value; `force` overrides only the
/// multiplexer and ssh checks (never `Unsupported`, `Unknown` or a sub-24-bit depth). The
/// environment is read only through `get_env` and only to switch the route **off**.
// RED stub: v0.9.0 design, not implemented
pub fn pixel_route(
    _cap: Capability,
    _depth: ColorDepth,
    _force: bool,
    _get_env: impl Fn(&str) -> Option<String>,
) -> PixelRoute {
    PixelRoute::Fallback(FallbackReason::NotOptedIn)
}
