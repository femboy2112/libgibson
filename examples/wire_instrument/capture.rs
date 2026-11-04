// Shared by the example (`main.rs`) and by `tests/wire_instrument_capture.rs` via `#[path]`; like
// the sibling `viewport`/`layout` modules it depends only on `gibson::` and `std`, so it stays
// independently includable. Quiet dead_code for the whole module.
#![allow(dead_code)]
//! Capture / replay of the observed field stream, with a content digest that is reproducible
//! ACROSS COLOR DEPTH (RXII Checkpoint 2 / Gate B, C4).
//!
//! A session capture records the SEMANTIC content of each observed field — source identity, cursor,
//! model tick, geometry (origin, cell sizes, dimensions), the scalar data, and the mask — so a
//! `--replay` reproduces exactly what the instrument saw, independent of the terminal it is replayed
//! on. The reproducibility certificate is [`content_digest`].
//!
//! The load-bearing property: the digest is taken over the field CONTENT, never over the rendered
//! ANSI bytes. A terminal's color depth changes the emitted bytes for an identical field (mono emits
//! no color SGR; truecolor emits 24-bit RGB), so a digest of the rendered output would disagree
//! across depth and be useless as a reproducibility certificate. Digesting the content instead makes
//! the certificate depth-invariant by construction — proven in `tests/wire_instrument_capture.rs`,
//! where a digest of the rendered bytes DOES differ across depth while this one does not. `f64`
//! values are hashed by their bit pattern (`to_bits`), so non-finite samples (NaN/±inf) contribute
//! deterministically and are never silently normalized.

use std::io::{self, Read, Write};

use gibson::field_view::FieldExtent;
use gibson::wire::{SourceCursor, WireField};
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
    /// valid [`FieldExtent`] (it always is for a frame captured from a valid field).
    pub fn extent(&self) -> Option<FieldExtent> {
        FieldExtent::new(
            Point::new(self.origin_x, self.origin_y),
            self.cell_w,
            self.cell_h,
            self.cols as usize,
            self.rows as usize,
        )
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(FNV_PRIME);
    }
}

/// A 64-bit FNV-1a digest over the SEMANTIC content of a capture — identity, geometry, data and
/// mask, in a fixed canonical order. Independent of color depth, terminal size and render mode by
/// construction (no rendered bytes enter it); `f64`s enter by their bit pattern so non-finite
/// samples are preserved. The capture/replay reproducibility certificate.
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

const MAGIC: &[u8; 8] = b"WIRECAP1";

/// Serialize a capture to a self-describing little-endian binary stream. `read_capture` is its exact
/// inverse, so `content_digest` round-trips byte-for-byte through a file (proven in the tests).
pub fn write_capture<W: Write>(w: &mut W, frames: &[CapturedFrame]) -> io::Result<()> {
    w.write_all(MAGIC)?;
    w.write_all(&(frames.len() as u32).to_le_bytes())?;
    for f in frames {
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
        w.write_all(&(f.data.len() as u32).to_le_bytes())?;
        for &v in &f.data {
            w.write_all(&v.to_bits().to_le_bytes())?;
        }
        match &f.mask {
            None => w.write_all(&[0u8])?,
            Some(m) => {
                w.write_all(&[1u8])?;
                w.write_all(&(m.len() as u32).to_le_bytes())?;
                let bytes: Vec<u8> = m.iter().map(|&b| b as u8).collect();
                w.write_all(&bytes)?;
            }
        }
    }
    Ok(())
}

fn read_n<R: Read, const N: usize>(r: &mut R) -> io::Result<[u8; N]> {
    let mut b = [0u8; N];
    r.read_exact(&mut b)?;
    Ok(b)
}

/// Deserialize a capture written by [`write_capture`]. Rejects a stream whose magic does not match.
pub fn read_capture<R: Read>(r: &mut R) -> io::Result<Vec<CapturedFrame>> {
    let magic = read_n::<_, 8>(r)?;
    if &magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a wire_instrument capture (bad magic)",
        ));
    }
    let n = u32::from_le_bytes(read_n::<_, 4>(r)?) as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let source = u64::from_le_bytes(read_n::<_, 8>(r)?);
        let epoch = u32::from_le_bytes(read_n::<_, 4>(r)?);
        let sequence = u64::from_le_bytes(read_n::<_, 8>(r)?);
        let model_tick = u64::from_le_bytes(read_n::<_, 8>(r)?);
        let cols = u32::from_le_bytes(read_n::<_, 4>(r)?);
        let rows = u32::from_le_bytes(read_n::<_, 4>(r)?);
        let origin_x = f64::from_bits(u64::from_le_bytes(read_n::<_, 8>(r)?));
        let origin_y = f64::from_bits(u64::from_le_bytes(read_n::<_, 8>(r)?));
        let cell_w = f64::from_bits(u64::from_le_bytes(read_n::<_, 8>(r)?));
        let cell_h = f64::from_bits(u64::from_le_bytes(read_n::<_, 8>(r)?));
        let dlen = u32::from_le_bytes(read_n::<_, 4>(r)?) as usize;
        let mut data = Vec::with_capacity(dlen);
        for _ in 0..dlen {
            data.push(f64::from_bits(u64::from_le_bytes(read_n::<_, 8>(r)?)));
        }
        let mask = match read_n::<_, 1>(r)?[0] {
            0 => None,
            _ => {
                let mlen = u32::from_le_bytes(read_n::<_, 4>(r)?) as usize;
                let mut m = Vec::with_capacity(mlen);
                for _ in 0..mlen {
                    m.push(read_n::<_, 1>(r)?[0] != 0);
                }
                Some(m)
            }
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
