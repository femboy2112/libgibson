//! Kitty graphics protocol, Unicode-placeholder route (v0.9/v0.10 design).
//!
//! The image bytes travel as APC sequences that the application writes itself; the picture
//! then appears wherever ordinary text cells hold U+10EEEE with the right colour and
//! diacritics. Those cells live in a [`Surface`], so the existing diff gives occlusion,
//! clipping and restore with no image-specific code.
//!
//! Bytes and `Surface`s only: nothing here writes to a terminal. The renderer hook, the
//! terminal-adapter frame plan, the panic hook and the session registry are later slices. See
//! `docs/design/v0.9-kitty-graphics-route.md`.
//!
//! Integration seams (own stub types; no dependency on unmerged work):
//! * `CellEncoder` (merged in #436) is the fallback: [`PixelRoute::Fallback`]
//!   means "draw the same box with the cell encoder the application chose".
//! * The parallel probe slice's tri-state `kitty_graphics` verdict maps onto
//!   [`crate::capability::Capability`] (`Supported` / `Unsupported` / `Unknown`), which is what
//!   [`pixel_route`] takes today.

use super::kitty_diacritics::ROWCOLUMN_DIACRITICS;
use crate::capability::{Capability, ColorDepth};
use crate::cell::{Cell, Color, Glyph, Style};
use crate::raster::RgbaRaster;
use crate::surface::Surface;
use std::collections::{BTreeMap, BTreeSet};

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

/// RFC 4648 base64 with padding (hand-written: no dependency).
fn base64_encode(data: &[u8]) -> Vec<u8> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b0 = u32::from(c[0]);
        let b1 = u32::from(c.get(1).copied().unwrap_or(0));
        let b2 = u32::from(c.get(2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize]);
        out.push(T[((n >> 12) & 63) as usize]);
        out.push(if c.len() > 1 {
            T[((n >> 6) & 63) as usize]
        } else {
            b'='
        });
        out.push(if c.len() > 2 {
            T[(n & 63) as usize]
        } else {
            b'='
        });
    }
    out
}

/// Transmit (not display) an image: chunked base64 RGBA over APC, quiet (`q=2`).
///
/// Shape of a single-chunk result: `ESC _ G a=t,f=32,s=<w>,v=<h>,i=<id>,q=2 ; <base64> ESC \`.
/// With more than one chunk the first chunk adds `,m=1`; the rest are `ESC _ G q=2,m=1 ; ...`
/// and the last is `ESC _ G q=2,m=0 ; ...`. No chunk's payload exceeds
/// [`MAX_CHUNK_BASE64`] and every chunk but the last is a multiple of 4.
///
/// Returns an empty `Vec` for input it must refuse: a buffer that is not `w * h * 4` bytes,
/// a zero size, id 0 or an id above [`MAX_IMAGE_ID`].
pub fn kitty_transmit_rgba(id: u32, w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    if id == 0 || id > MAX_IMAGE_ID || w == 0 || h == 0 {
        return Vec::new();
    }
    let need = u64::from(w) * u64::from(h) * 4;
    if need != rgba.len() as u64 {
        return Vec::new();
    }
    let payload = base64_encode(rgba);
    let chunks: Vec<&[u8]> = payload.chunks(MAX_CHUNK_BASE64).collect();
    let mut out = Vec::with_capacity(payload.len() + chunks.len() * 40);
    let last = chunks.len() - 1;
    for (i, chunk) in chunks.iter().enumerate() {
        let keys = if i == 0 {
            let more = if last > 0 { ",m=1" } else { "" };
            format!("a=t,f=32,s={w},v={h},i={id},q=2{more}")
        } else if i < last {
            "q=2,m=1".to_string()
        } else {
            "q=2,m=0".to_string()
        };
        out.extend_from_slice(b"\x1b_G");
        out.extend_from_slice(keys.as_bytes());
        out.push(b';');
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
    out
}

/// Create the virtual placement of image `id` over `cols x rows` cells:
/// `ESC _ G a=p,U=1,i=<id>,c=<cols>,r=<rows>,q=2 ESC \`.
pub fn kitty_place_virtual(id: u32, cols: u16, rows: u16) -> Vec<u8> {
    format!("\x1b_Ga=p,U=1,i={id},c={cols},r={rows},q=2\x1b\\").into_bytes()
}

/// Delete image `id` by id: `ESC _ G a=d,d=i,i=<id>,q=2 ESC \`. With `free` the data is freed
/// too (`d=I`); without it the data stays so the image can be shown again with no re-upload.
pub fn kitty_delete(id: u32, free: bool) -> Vec<u8> {
    let d = if free { 'I' } else { 'i' };
    format!("\x1b_Ga=d,d={d},i={id},q=2\x1b\\").into_bytes()
}

/// Free every id in `ids` (sorted ascending, duplicates dropped) with `kitty_delete(id, true)`.
/// This is the teardown and panic-hook payload. It deletes by id because `a=d,d=a` / `d=A`
/// "never affect virtual placements" (protocol page).
pub fn kitty_delete_all(ids: &[u32]) -> Vec<u8> {
    let sorted: BTreeSet<u32> = ids.iter().copied().collect();
    let mut out = Vec::new();
    for id in sorted {
        out.extend(kitty_delete(id, true));
    }
    out
}

/// The diacritic that encodes row or column number `n` (0-based), from the protocol's
/// `rowcolumn-diacritics.txt` (297 entries; `0` is U+0305, `1` is U+030D). `None` when
/// `n >= MAX_PLACEHOLDER_DIM`.
pub fn rowcolumn_diacritic(n: u16) -> Option<char> {
    ROWCOLUMN_DIACRITICS.get(usize::from(n)).copied()
}

/// A `cols x rows` surface of placeholder cells for image `id`. The cell at `(c, r)` holds
/// U+10EEEE + the row-`r` diacritic + the column-`c` diacritic, with foreground
/// `Color::Rgb(id >> 16, id >> 8, id)` and no other style. Both diacritics are always written
/// (never inferred), so a cell is correct on its own wherever the diff or a clip puts it.
/// `cols` and `rows` are clamped to [`MAX_PLACEHOLDER_DIM`].
pub fn placeholder_cells(id: u32, cols: u16, rows: u16) -> Surface {
    let cols = cols.min(MAX_PLACEHOLDER_DIM);
    let rows = rows.min(MAX_PLACEHOLDER_DIM);
    let style = Style::new().fg(Color::Rgb((id >> 16) as u8, (id >> 8) as u8, id as u8));
    let mut surface = Surface::new(cols, rows);
    for r in 0..rows {
        for c in 0..cols {
            let (Some(rd), Some(cd)) = (rowcolumn_diacritic(r), rowcolumn_diacritic(c)) else {
                continue;
            };
            let mut text = String::new();
            text.push(PLACEHOLDER);
            text.push(rd);
            text.push(cd);
            surface.set_cell(c, r, Cell::new(Glyph::new(&text), style));
        }
    }
    surface
}

/// Byte-budget knobs of the [`ImageCache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheLimits {
    /// An image whose raw RGBA size exceeds this is never uploaded (fallback + report).
    pub max_upload_bytes: usize,
    /// Base64 payload bytes allowed per frame; the rest is deferred to later frames. Read by
    /// the later frame-plan slice; [`ImageCache`] does not enforce it.
    pub max_frame_bytes: usize,
    /// Images kept resident on the terminal; the least recently shown is evicted beyond this
    /// (lowercase `d=i`: placements go, data stays). Values below 1 behave as 1.
    pub max_resident_images: usize,
    /// Optional application salt mixed into the content hash. `None` (the default) is the pure
    /// content hash; a salt changes every id deterministically, as insurance against another
    /// library's images in the terminal's global id namespace.
    pub salt: Option<u64>,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            max_upload_bytes: 4 * 1024 * 1024,
            max_frame_bytes: 256 * 1024,
            max_resident_images: 16,
            salt: None,
        }
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// FNV-1a 64 over `(width, height, pixels)`: a fixed, dependency-free function, so ids agree
/// across caches, processes and builds (std's hashers are randomized or unspecified).
fn content_hash(raster: &RgbaRaster) -> u64 {
    let mut h = fnv1a(FNV_OFFSET, &raster.width().to_le_bytes());
    h = fnv1a(h, &raster.height().to_le_bytes());
    for px in raster.pixels() {
        h = fnv1a(h, px);
    }
    h
}

/// Folds 64 bits to a 24-bit id by xor of three slices; 0 maps to 1.
fn fold_to_id(h: u64) -> u32 {
    let m = u64::from(MAX_IMAGE_ID);
    let id = ((h & m) ^ ((h >> 24) & m) ^ ((h >> 48) & m)) as u32;
    if id == 0 {
        1
    } else {
        id
    }
}

#[derive(Debug)]
struct Entry {
    /// The full 64-bit (salted) hash; the id is only a 24-bit handle derived from it.
    hash: u64,
    /// Kept for the byte compare that makes content, not the hash, the identity.
    raster: RgbaRaster,
    last_used: u64,
}

/// Content-identity id cache: the same pixels always map to the same id and are uploaded once.
///
/// Identity is the full content (64-bit hash, size, then a byte compare of the pixels); the
/// 24-bit id is only an allocated handle. If the folded id of new content is already held by
/// different content, the next free id is taken (linear probe, wrapping from
/// [`MAX_IMAGE_ID`] to 1), so a collision can never show the wrong image.
#[derive(Debug)]
#[non_exhaustive]
pub struct ImageCache {
    limits: CacheLimits,
    hasher: fn(&RgbaRaster) -> u64,
    entries: BTreeMap<u32, Entry>,
    /// Every id this cache ever transmitted (including evicted ones: `d=i` keeps their data),
    /// so teardown frees them all.
    transmitted: BTreeSet<u32>,
    tick: u64,
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageCache {
    pub fn new() -> Self {
        Self::with_limits(CacheLimits::default())
    }

    pub fn with_limits(limits: CacheLimits) -> Self {
        Self {
            limits,
            hasher: content_hash,
            entries: BTreeMap::new(),
            transmitted: BTreeSet::new(),
            tick: 0,
        }
    }

    /// Test seam: a cache whose content hash is `hasher`, so a test can force collisions.
    /// Not part of the supported API.
    #[doc(hidden)]
    pub fn with_hasher_for_tests(limits: CacheLimits, hasher: fn(&RgbaRaster) -> u64) -> Self {
        let mut cache = Self::with_limits(limits);
        cache.hasher = hasher;
        cache
    }

    fn keyed_hash(&self, raster: &RgbaRaster) -> u64 {
        let h = (self.hasher)(raster);
        match self.limits.salt {
            None => h,
            Some(salt) => fnv1a(fnv1a(FNV_OFFSET, &salt.to_le_bytes()), &h.to_le_bytes()),
        }
    }

    /// Returns the id for `raster` (in `1..=MAX_IMAGE_ID`; without a collision a pure function
    /// of its size, pixels and the optional salt) and the bytes to write before any frame that
    /// shows it. A repeat returns `(same id, None)`.
    ///
    /// The bytes of a first sight are the eviction deletes (lowercase `d=i`, one per image
    /// pushed out by `max_resident_images`, least recently shown first) followed by the
    /// transmit. Returning them in the one `Option<Vec<u8>>` keeps the signature and makes the
    /// order (delete, then upload) impossible to get wrong. A hit never evicts. An image that
    /// is empty or over `max_upload_bytes` is refused: `(0, None)`.
    pub fn ensure(&mut self, raster: &RgbaRaster) -> (u32, Option<Vec<u8>>) {
        let bytes = usize::from(raster.width())
            .saturating_mul(usize::from(raster.height()))
            .saturating_mul(4);
        if bytes == 0 || bytes > self.limits.max_upload_bytes {
            return (0, None);
        }
        let hash = self.keyed_hash(raster);
        self.tick += 1;
        let tick = self.tick;
        let found = self
            .entries
            .iter_mut()
            .find(|(_, e)| e.hash == hash && e.raster == *raster)
            .map(|(&id, e)| {
                e.last_used = tick;
                id
            });
        if let Some(id) = found {
            return (id, None);
        }
        let mut out = Vec::new();
        let cap = self.limits.max_resident_images.max(1);
        while self.entries.len() >= cap {
            let victim = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(&id, _)| id);
            let Some(victim) = victim else { break };
            self.entries.remove(&victim);
            out.extend(kitty_delete(victim, false));
        }
        let mut id = fold_to_id(hash);
        while self.entries.contains_key(&id) {
            id = if id >= MAX_IMAGE_ID { 1 } else { id + 1 };
        }
        let rgba: Vec<u8> = raster.pixels().iter().flatten().copied().collect();
        out.extend(kitty_transmit_rgba(
            id,
            u32::from(raster.width()),
            u32::from(raster.height()),
            &rgba,
        ));
        self.entries.insert(
            id,
            Entry {
                hash,
                raster: raster.clone(),
                last_used: tick,
            },
        );
        self.transmitted.insert(id);
        (id, Some(out))
    }

    /// Bytes that free (`d=I`) every image this cache transmitted, evicted ones included (see
    /// [`kitty_delete_all`]).
    pub fn teardown_bytes(&self) -> Vec<u8> {
        let ids: Vec<u32> = self.transmitted.iter().copied().collect();
        kitty_delete_all(&ids)
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
pub fn pixel_route(
    cap: Capability,
    depth: ColorDepth,
    force: bool,
    get_env: impl Fn(&str) -> Option<String>,
) -> PixelRoute {
    match cap {
        Capability::Unknown => return PixelRoute::Fallback(FallbackReason::NotOptedIn),
        Capability::Unsupported => return PixelRoute::Fallback(FallbackReason::Unsupported),
        Capability::Supported => {}
    }
    if depth != ColorDepth::TrueColor {
        return PixelRoute::Fallback(FallbackReason::NotTrueColor);
    }
    if !force {
        let term_is_mux =
            get_env("TERM").is_some_and(|t| t.starts_with("screen") || t.starts_with("tmux"));
        if get_env("TMUX").is_some() || get_env("STY").is_some() || term_is_mux {
            return PixelRoute::Fallback(FallbackReason::Multiplexer);
        }
        if ["SSH_CONNECTION", "SSH_TTY", "SSH_CLIENT"]
            .iter()
            .any(|k| get_env(k).is_some())
        {
            return PixelRoute::Fallback(FallbackReason::Remote);
        }
    }
    PixelRoute::Pixels
}
