//! Source-identified immutable frames of ingested RGB pixels (issue #129).
//!
//! A [`Frame`] is a *replaceable observation*, not an editable document: it is built
//! once by ingesting a bounded byte buffer ([`Frame::from_rgb_bytes`]) and is then
//! immutable. Its [`FrameSource`] says whose pixels these are (a consumer ingesting
//! from several producers can tell them apart) and its sequence number orders frames
//! from one source so a later bridge (issue #141) can keep the latest and drop the
//! stale. Ingest copies the caller's bytes; nothing is borrowed and no C ABI or
//! zero-copy is involved.
//!
//! Quantization receipt: a frame holds truecolor pixels; realizing them as
//! [`crate::raster::RgbRaster::to_surface`] emits truecolor cells, and any
//! per-terminal colour downsampling is the central quantizer's job
//! ([`crate::capability`]) — the loss, if any, is located there, never hidden here.

use crate::raster::{FrameError, RgbRaster};
use crate::surface::Surface;

/// Opaque identity of a frame producer. The library assigns no meaning; the consumer
/// owns the namespace. Two frames with the same `FrameSource` are from the same stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameSource(pub u64);

/// An immutable, source-identified frame of RGB pixels (#129). Build with
/// [`Frame::from_rgb_bytes`]; once built it exposes only read accessors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    source: FrameSource,
    seq: u64,
    raster: RgbRaster,
}

impl Frame {
    /// Ingests a bounded packed-RGB buffer into an immutable frame tagged with its
    /// `source` and `seq`. Delegates the bounds/stride/overflow checks to
    /// [`RgbRaster::from_rgb_bytes`], so the same rejection rules apply.
    pub fn from_rgb_bytes(
        source: FrameSource,
        seq: u64,
        width: u16,
        height: u16,
        stride: usize,
        bytes: &[u8],
    ) -> Result<Self, FrameError> {
        let raster = RgbRaster::from_rgb_bytes(width, height, stride, bytes)?;
        Ok(Self {
            source,
            seq,
            raster,
        })
    }

    /// The producer identity this frame came from.
    pub fn source(&self) -> FrameSource {
        self.source
    }

    /// The sequence number ordering this frame within its source's stream.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// The immutable underlying raster.
    pub fn raster(&self) -> &RgbRaster {
        &self.raster
    }

    /// Pixel width.
    pub fn width(&self) -> u16 {
        self.raster.width()
    }

    /// Pixel height.
    pub fn height(&self) -> u16 {
        self.raster.height()
    }

    /// Realizes the frame as a truecolor half-block [`Surface`] for presentation
    /// (delegates to the existing renderer path; no second engine).
    pub fn to_surface(&self) -> Surface {
        self.raster.to_surface()
    }

    /// Consumes the frame, returning the owned raster.
    pub fn into_raster(self) -> RgbRaster {
        self.raster
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Immutability is a compile-time property: Frame has no `&mut self` method and no
    // public field, so it is enforced by the API shape, not by a runtime test.

    fn buf() -> [u8; 12] {
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
    }

    #[test]
    fn frame_carries_source_and_seq() {
        // Predicted: metadata and dimensions come back exactly as supplied.
        let f = Frame::from_rgb_bytes(FrameSource(7), 42, 2, 2, 6, &buf()).unwrap();
        assert_eq!(f.source(), FrameSource(7));
        assert_eq!(f.seq(), 42);
        assert_eq!(f.width(), 2);
        assert_eq!(f.height(), 2);
    }

    #[test]
    fn frame_delegates_ingest_rejection() {
        // Predicted: a short buffer yields the SAME error as raster ingest, proving
        // Frame cannot bypass the checks.
        let short = &buf()[..11];
        let via_frame = Frame::from_rgb_bytes(FrameSource(1), 0, 2, 2, 6, short);
        let via_raster = RgbRaster::from_rgb_bytes(2, 2, 6, short);
        assert_eq!(via_frame.unwrap_err(), via_raster.unwrap_err());
        assert_eq!(
            Frame::from_rgb_bytes(FrameSource(1), 0, 2, 2, 6, short).unwrap_err(),
            FrameError::ShortBuffer { have: 11, need: 12 }
        );
    }

    #[test]
    fn frame_to_surface_realizes_pixels() {
        // Predicted (from raster.rs to_surface): Surface::new(width, height.div_ceil(2)),
        // so a 2x3 pixel frame -> 2 cells wide, ceil(3/2)=2 cells tall.
        let bytes = [0u8; 18];
        let f = Frame::from_rgb_bytes(FrameSource(0), 0, 2, 3, 6, &bytes).unwrap();
        let s = f.to_surface();
        assert_eq!(s.width, 2);
        assert_eq!(s.height, 2);
    }
}
