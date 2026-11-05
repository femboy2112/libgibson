// Shared by the example (`main.rs`) and by `tests/wire_instrument_capture.rs` via `#[path]`; like
// the sibling `viewport`/`layout` modules it depends only on `gibson::` and `std`, so it stays
// independently includable. Quiet dead_code for the whole module.
#![allow(dead_code)]
//! Capture / replay of the observed field stream, with a content CHECKSUM that is reproducible
//! ACROSS COLOR DEPTH (RXII Checkpoint 2 / Gate B C4; hardened in RXIII Checkpoint A / Gate B1/B2).
//!
//! A session capture records the SEMANTIC content of each observed field — source identity, cursor,
//! model tick, geometry (origin, cell sizes, dimensions), the scalar data, and the mask — so a
//! `--replay` reproduces exactly what the instrument saw, independent of the terminal it is replayed
//! on. The fingerprint of that content is [`content_digest`].
//!
//! The load-bearing property: the digest is taken over the field CONTENT, never over the rendered
//! ANSI bytes. A terminal's color depth changes the emitted bytes for an identical field (mono emits
//! no color SGR; truecolor emits 24-bit RGB), so a digest of the rendered output would disagree
//! across depth and be useless as a cross-depth checksum. Digesting the content instead makes it
//! depth-invariant by construction — proven in `tests/wire_instrument_capture.rs`, where a digest of
//! the rendered bytes DOES differ across depth while this one does not. `f64` values are hashed by
//! their bit pattern (`to_bits`), so non-finite samples (NaN/±inf) contribute deterministically and
//! are never silently normalized.
//!
//! ## Reading the file is adversarial (Gate B1)
//!
//! [`read_capture`] treats its input as hostile: every declared length is checked against a
//! [`CaptureLimits`] budget and against the geometry *before* a byte of payload is allocated, so a
//! lying header cannot force an unbounded reservation. Geometry is validated through
//! [`gibson::field_view::FieldExtent::new`] (the same degenerate/non-finite refusal the live wire
//! uses), the data count must equal `cols × rows`, and a present mask must match. A malformed stream
//! is rejected with a typed [`CaptureError`], never accepted or silently narrowed. [`write_capture`]
//! likewise refuses a length that would not survive the `u32` on-disk field rather than truncating
//! it. See `tests/wire_instrument_capture_hostile.rs`.

use std::io::{self, Read, Write};

use gibson::field_view::FieldExtent;
use gibson::wire::{SourceCursor, WireField, WireLimits};
use gibson::Point;

/// One captured field observation: its identity, geometry and data — everything the display and the
/// probe are derived from, and nothing about the terminal it was (or will be) rendered on.
#[derive(Debug, Clone, PartialEq)]
pub struct CapturedFrame {
    pub source: u64,
    pub epoch: u32,
    pub sequence: u64,
    pub model_tick: u64,
    pub cols: u32,
    pub rows: u32,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_w: f64,
    pub cell_h: f64,
    pub data: Vec<f64>,
    pub mask: Option<Vec<bool>>,
}

impl CapturedFrame {
    /// Capture a live observation: the source, its cursor, and the field geometry/data/mask.
    pub fn from_field(source: u64, cursor: SourceCursor, field: &WireField) -> Self {
        let ext = field.extent;
        let o = ext.origin();
        CapturedFrame {
            source,
            epoch: cursor.epoch,
            sequence: cursor.sequence,
            model_tick: field.model_tick,
            cols: ext.cols() as u32,
            rows: ext.rows() as u32,
            origin_x: o.x,
            origin_y: o.y,
            cell_w: ext.cell_w(),
            cell_h: ext.cell_h(),
            data: field.data.clone(),
            mask: field.mask.clone(),
        }
    }

    /// Rebuild the gibson geometry for rendering on replay. `None` if the stored geometry is not a
    /// valid [`FieldExtent`] (it always is for a frame captured from a valid field, and
    /// [`read_capture`] refuses to produce one where it is not).
    pub fn extent(&self) -> Option<FieldExtent> {
        FieldExtent::new(
            Point::new(self.origin_x, self.origin_y),
            self.cell_w,
            self.cell_h,
            self.cols as usize,
            self.rows as usize,
        )
    }

    /// `cols × rows` as a `usize`, or `None` if the product overflows (a hostile header can declare
    /// dimensions whose product does not fit). Used to bound allocation and validate the data count.
    fn cell_count(&self) -> Option<usize> {
        (self.cols as usize).checked_mul(self.rows as usize)
    }
}

// ---------------------------------------------------------------------------------------------
// Bounds and typed errors (Gate B1)
// ---------------------------------------------------------------------------------------------

/// Resource budget a [`read_capture`] / [`write_capture`] will admit. Defaults mirror the live wire
/// ([`WireLimits`]): at most `1<<20` cells per field, and a generous but finite frame count. A
/// stream declaring more than this is rejected before allocation; adversarial tests pass a tiny
/// budget to prove the refusal cheaply, never by actually allocating gigabytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureLimits {
    /// Hard cap on the number of frames a capture may declare.
    pub max_frames: usize,
    /// Hard cap on a single field's `cols × rows`.
    pub max_field_cells: usize,
}

impl CaptureLimits {
    /// A generous frame ceiling: a live capture records a handful, so this is pure headroom while
    /// still bounding the `with_capacity` a hostile frame count could request.
    pub const DEFAULT_MAX_FRAMES: usize = 1 << 16;
}

impl Default for CaptureLimits {
    fn default() -> Self {
        CaptureLimits {
            max_frames: Self::DEFAULT_MAX_FRAMES,
            max_field_cells: WireLimits::DEFAULT_MAX_FIELD_CELLS,
        }
    }
}

/// Why a capture stream (or an attempt to write one) was rejected. Every variant is a *typed*
/// refusal: the reader never accepts a malformed stream, and the writer never narrows a length it
/// cannot represent faithfully.
#[derive(Debug)]
pub enum CaptureError {
    /// The leading magic did not match — not a wire_instrument capture, or a different format
    /// version (the magic's trailing digit is the version, so `WIRECAP2` lands here).
    BadMagic,
    /// A declared frame count exceeds the configured budget.
    FramesExceedBudget { declared: usize, cap: usize },
    /// A declared field `cols × rows` exceeds the configured budget.
    CellsExceedBudget { declared: usize, cap: usize },
    /// `cols × rows` overflowed `usize`.
    DimensionOverflow { cols: u32, rows: u32 },
    /// The stored geometry is not a usable [`FieldExtent`] (non-finite/degenerate).
    DegenerateGeometry,
    /// The data count disagrees with `cols × rows`.
    DataLengthMismatch { data: usize, cells: usize },
    /// A present mask's length disagrees with the data count.
    MaskLengthMismatch { mask: usize, data: usize },
    /// The mask-presence tag was neither 0 nor 1.
    InvalidMaskTag(u8),
    /// A mask byte was neither 0 nor 1.
    InvalidBooleanByte(u8),
    /// The stream ended inside a header or payload that the preceding length promised.
    Truncated,
    /// A value could not be represented in the `u32` on-disk length field (writer side).
    LengthNotRepresentable(usize),
    /// An underlying I/O error that is not a clean truncation.
    Io(io::Error),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::BadMagic => {
                write!(f, "not a wire_instrument capture (bad magic / version)")
            }
            CaptureError::FramesExceedBudget { declared, cap } => {
                write!(f, "declared {declared} frames exceeds budget {cap}")
            }
            CaptureError::CellsExceedBudget { declared, cap } => {
                write!(f, "declared {declared} cells exceeds budget {cap}")
            }
            CaptureError::DimensionOverflow { cols, rows } => {
                write!(f, "cols {cols} × rows {rows} overflows usize")
            }
            CaptureError::DegenerateGeometry => {
                write!(f, "degenerate or non-finite field geometry")
            }
            CaptureError::DataLengthMismatch { data, cells } => {
                write!(f, "data count {data} ≠ cols×rows {cells}")
            }
            CaptureError::MaskLengthMismatch { mask, data } => {
                write!(f, "mask length {mask} ≠ data count {data}")
            }
            CaptureError::InvalidMaskTag(b) => {
                write!(f, "invalid mask-presence tag {b} (want 0/1)")
            }
            CaptureError::InvalidBooleanByte(b) => write!(f, "invalid boolean byte {b} (want 0/1)"),
            CaptureError::Truncated => write!(f, "capture stream truncated"),
            CaptureError::LengthNotRepresentable(n) => {
                write!(f, "length {n} does not fit the u32 on-disk field")
            }
            CaptureError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<io::Error> for CaptureError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            CaptureError::Truncated
        } else {
            CaptureError::Io(e)
        }
    }
}

impl From<CaptureError> for io::Error {
    fn from(e: CaptureError) -> Self {
        match e {
            CaptureError::Io(io) => io,
            other => io::Error::new(io::ErrorKind::InvalidData, other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Content checksum
// ---------------------------------------------------------------------------------------------

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(FNV_PRIME);
    }
}

/// A 64-bit FNV-1a **content checksum** over the semantic content of a capture — identity, geometry,
/// data and mask, in a fixed canonical order. Independent of color depth, terminal size and render
/// mode by construction (no rendered bytes enter it); `f64`s enter by their bit pattern so non-finite
/// samples are preserved.
///
/// This is a deterministic regression receipt: equal content ⇒ equal checksum, and it reproduces
/// across color depth. It is **not** a cryptographic authenticator, **not** a collision-free
/// identifier, and **not** proof that a producer's model history was reproduced — it fingerprints the
/// bytes of the recorded observations, nothing more. Two different interaction histories that happen
/// to emit the same field sequence share this checksum (that is precisely why Gate B3 adds a
/// separate causal trace); do not read reproduced-checksum as reproduced-causality.
pub fn content_digest(frames: &[CapturedFrame]) -> u64 {
    let mut h = FNV_OFFSET;
    fnv(&mut h, &(frames.len() as u64).to_le_bytes());
    for f in frames {
        fnv(&mut h, &f.source.to_le_bytes());
        fnv(&mut h, &f.epoch.to_le_bytes());
        fnv(&mut h, &f.sequence.to_le_bytes());
        fnv(&mut h, &f.model_tick.to_le_bytes());
        fnv(&mut h, &f.cols.to_le_bytes());
        fnv(&mut h, &f.rows.to_le_bytes());
        fnv(&mut h, &f.origin_x.to_bits().to_le_bytes());
        fnv(&mut h, &f.origin_y.to_bits().to_le_bytes());
        fnv(&mut h, &f.cell_w.to_bits().to_le_bytes());
        fnv(&mut h, &f.cell_h.to_bits().to_le_bytes());
        fnv(&mut h, &(f.data.len() as u64).to_le_bytes());
        for &v in &f.data {
            fnv(&mut h, &v.to_bits().to_le_bytes());
        }
        match &f.mask {
            None => fnv(&mut h, &[0u8]),
            Some(m) => {
                fnv(&mut h, &[1u8]);
                fnv(&mut h, &(m.len() as u64).to_le_bytes());
                for &b in m {
                    fnv(&mut h, &[b as u8]);
                }
            }
        }
    }
    h
}

// ---------------------------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------------------------

const MAGIC: &[u8; 8] = b"WIRECAP1";

/// On-disk `u32` length, refusing a value that does not fit rather than silently narrowing it
/// through `as u32` (Gate B1: the writer must not narrow a length it cannot represent faithfully).
fn len_u32(n: usize) -> Result<u32, CaptureError> {
    u32::try_from(n).map_err(|_| CaptureError::LengthNotRepresentable(n))
}

/// Serialize a capture to a self-describing little-endian binary stream. Refuses a frame whose data
/// count disagrees with its geometry, or any length that would not survive the `u32` on-disk field —
/// so a capture that `write_capture` accepts is one `read_capture` will accept back. `read_capture`
/// is its exact inverse, so `content_digest` round-trips byte-for-byte through a file.
pub fn write_capture<W: Write>(w: &mut W, frames: &[CapturedFrame]) -> Result<(), CaptureError> {
    w.write_all(MAGIC)?;
    w.write_all(&len_u32(frames.len())?.to_le_bytes())?;
    for f in frames {
        // Refuse to emit a frame we would reject on read: data must match cols×rows, mask must match
        // data. Catches a corrupt in-memory frame at the boundary instead of writing a bad capture.
        let cells = f.cell_count().ok_or(CaptureError::DimensionOverflow {
            cols: f.cols,
            rows: f.rows,
        })?;
        if f.data.len() != cells {
            return Err(CaptureError::DataLengthMismatch {
                data: f.data.len(),
                cells,
            });
        }
        if let Some(m) = &f.mask {
            if m.len() != f.data.len() {
                return Err(CaptureError::MaskLengthMismatch {
                    mask: m.len(),
                    data: f.data.len(),
                });
            }
        }
        w.write_all(&f.source.to_le_bytes())?;
        w.write_all(&f.epoch.to_le_bytes())?;
        w.write_all(&f.sequence.to_le_bytes())?;
        w.write_all(&f.model_tick.to_le_bytes())?;
        w.write_all(&f.cols.to_le_bytes())?;
        w.write_all(&f.rows.to_le_bytes())?;
        w.write_all(&f.origin_x.to_bits().to_le_bytes())?;
        w.write_all(&f.origin_y.to_bits().to_le_bytes())?;
        w.write_all(&f.cell_w.to_bits().to_le_bytes())?;
        w.write_all(&f.cell_h.to_bits().to_le_bytes())?;
        w.write_all(&len_u32(f.data.len())?.to_le_bytes())?;
        for &v in &f.data {
            w.write_all(&v.to_bits().to_le_bytes())?;
        }
        match &f.mask {
            None => w.write_all(&[0u8])?,
            Some(m) => {
                w.write_all(&[1u8])?;
                w.write_all(&len_u32(m.len())?.to_le_bytes())?;
                let bytes: Vec<u8> = m.iter().map(|&b| b as u8).collect();
                w.write_all(&bytes)?;
            }
        }
    }
    Ok(())
}

/// Read exactly `N` bytes, mapping a clean end-of-stream to [`CaptureError::Truncated`].
fn read_n<R: Read, const N: usize>(r: &mut R) -> Result<[u8; N], CaptureError> {
    let mut b = [0u8; N];
    r.read_exact(&mut b)?;
    Ok(b)
}

fn read_u32<R: Read>(r: &mut R) -> Result<u32, CaptureError> {
    Ok(u32::from_le_bytes(read_n::<_, 4>(r)?))
}
fn read_u64<R: Read>(r: &mut R) -> Result<u64, CaptureError> {
    Ok(u64::from_le_bytes(read_n::<_, 8>(r)?))
}
fn read_f64<R: Read>(r: &mut R) -> Result<f64, CaptureError> {
    Ok(f64::from_bits(u64::from_le_bytes(read_n::<_, 8>(r)?)))
}

/// Deserialize a capture written by [`write_capture`], with the default [`CaptureLimits`].
pub fn read_capture<R: Read>(r: &mut R) -> Result<Vec<CapturedFrame>, CaptureError> {
    read_capture_with(r, &CaptureLimits::default())
}

/// Deserialize a capture under an explicit resource budget, treating the stream as hostile.
///
/// Every declared length is validated against `limits` and against the geometry *before* allocation,
/// so a lying header cannot force an unbounded reservation: a declared count beyond budget is a typed
/// refusal, and a plausible-but-false count simply hits [`CaptureError::Truncated`] when the payload
/// runs out — `with_capacity` is bounded by the (already budget-checked) declared size either way.
pub fn read_capture_with<R: Read>(
    r: &mut R,
    limits: &CaptureLimits,
) -> Result<Vec<CapturedFrame>, CaptureError> {
    let magic = read_n::<_, 8>(r)?;
    if &magic != MAGIC {
        return Err(CaptureError::BadMagic);
    }
    let n = read_u32(r)? as usize;
    if n > limits.max_frames {
        return Err(CaptureError::FramesExceedBudget {
            declared: n,
            cap: limits.max_frames,
        });
    }
    // Bounded: n ≤ max_frames was just enforced, so this reservation is capped. A false n that
    // survives that check still cannot over-read — the per-frame loop hits EOF → Truncated.
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let source = read_u64(r)?;
        let epoch = read_u32(r)?;
        let sequence = read_u64(r)?;
        let model_tick = read_u64(r)?;
        let cols = read_u32(r)?;
        let rows = read_u32(r)?;

        // Validate geometry BEFORE reading any payload, so a hostile dimension cannot drive a huge
        // allocation. cols×rows must not overflow and must fit the cell budget.
        let cells = (cols as usize)
            .checked_mul(rows as usize)
            .ok_or(CaptureError::DimensionOverflow { cols, rows })?;
        if cells > limits.max_field_cells {
            return Err(CaptureError::CellsExceedBudget {
                declared: cells,
                cap: limits.max_field_cells,
            });
        }

        let origin_x = read_f64(r)?;
        let origin_y = read_f64(r)?;
        let cell_w = read_f64(r)?;
        let cell_h = read_f64(r)?;
        // The same degenerate/non-finite refusal the live wire applies.
        if FieldExtent::new(
            Point::new(origin_x, origin_y),
            cell_w,
            cell_h,
            cols as usize,
            rows as usize,
        )
        .is_none()
        {
            return Err(CaptureError::DegenerateGeometry);
        }

        let dlen = read_u32(r)? as usize;
        if dlen != cells {
            return Err(CaptureError::DataLengthMismatch { data: dlen, cells });
        }
        // Bounded: dlen == cells ≤ max_field_cells.
        let mut data = Vec::with_capacity(dlen);
        for _ in 0..dlen {
            data.push(read_f64(r)?);
        }

        let mask = match read_n::<_, 1>(r)?[0] {
            0 => None,
            1 => {
                let mlen = read_u32(r)? as usize;
                if mlen != dlen {
                    return Err(CaptureError::MaskLengthMismatch {
                        mask: mlen,
                        data: dlen,
                    });
                }
                let mut m = Vec::with_capacity(mlen);
                for _ in 0..mlen {
                    m.push(match read_n::<_, 1>(r)?[0] {
                        0 => false,
                        1 => true,
                        other => return Err(CaptureError::InvalidBooleanByte(other)),
                    });
                }
                Some(m)
            }
            other => return Err(CaptureError::InvalidMaskTag(other)),
        };

        out.push(CapturedFrame {
            source,
            epoch,
            sequence,
            model_tick,
            cols,
            rows,
            origin_x,
            origin_y,
            cell_w,
            cell_h,
            data,
            mask,
        });
    }
    Ok(out)
}
