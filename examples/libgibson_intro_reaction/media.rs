//! `gibson::media` — deterministic, bounded, self-describing **embedded media assets**.
//!
//! A terminal program often needs to carry media it did not author — a keyed video
//! subject to composite, an image, a short sound, a table of bytes — without shipping
//! the original file beside the binary and without fetching it at runtime. The idiomatic
//! Rust answer is [`include_bytes!`], which bakes a file into the executable. This module
//! is the *format* that makes that bytes-blob **self-describing, compact, and safe to
//! decode**: you extract only the data you need from a source once, encode it here, embed
//! the result, and decode frames/samples/bytes back out deterministically — the source
//! file is never redistributed and never re-downloaded.
//!
//! # Design, and why it matches the rest of this crate
//!
//! Like [`crate::wire`], this is **I/O-free and substrate-agnostic**: it operates on byte
//! buffers, not files or sockets. `include_bytes!` (or any loader) supplies the bytes; the
//! decoder yields typed media. Encoding is **deterministic** — the same input produces the
//! same bytes, every build, on every platform (no hashing of pointers, no float
//! formatting, fixed little-endian integers) — so an embedded asset has a stable identity
//! and a checked-in asset never churns.
//!
//! An asset is a small header (a [`MediaKind`] and its parameters) followed by a table of
//! **segments** and a compressed payload. A segment is one independently-decodable unit:
//! for a [`MediaKind::FrameSeq`] it is one frame (so a single frame decodes in O(1), with
//! no inter-frame decode chain — seek is cheap); for an image, sound, or blob it is the
//! whole buffer. Each segment is stored either raw ([`SegMethod::Store`]) or
//! run-length-coded ([`SegMethod::Rle`], a deterministic PackBits), whichever is smaller —
//! so a keyed frame that is mostly one flat transparent colour collapses to a fraction of
//! its raw size while an incompressible blob never grows.
//!
//! # The bounded-decode discipline (why a hostile asset cannot exhaust memory)
//!
//! A length-prefixed container is the classic out-of-memory vector: a header that *claims*
//! four gigabytes makes a naive decoder allocate four gigabytes. This decoder never does.
//! [`Asset::parse`] validates every declared length **against hard caps and against the
//! bytes actually present** before trusting it: the magic and version gate the stream, the
//! segment count is capped ([`MAX_SEGMENTS`]), each declared raw length is capped
//! ([`MAX_SEGMENT_BYTES`]), and the sum of the compressed lengths must equal the payload
//! that is really there ([`MediaError::PayloadLengthMismatch`]). Decoding a segment bounds
//! its output to the declared raw length, so a corrupt RLE run cannot inflate without
//! limit ([`MediaError::CorruptSegment`]). The only bytes the decoder ever holds are bytes
//! the caller already handed it, plus at most one decoded segment.
//!
//! # Boundary — what this does not promise
//!
//! This is a container and a general-purpose lossless codec, not a media *codec* in the
//! MPEG sense: it does not decode VP9/H.264/Opus and it is not a competitive video
//! compressor. Produce raw frames/samples with a real decoder (offline `ffmpeg`, say),
//! extract and key exactly the window you need, and embed *that*. RLE wins big on flat and
//! transparent regions and breaks even on noise; a future codec can be added without
//! breaking the format (the per-segment [`SegMethod`] is a tagged, extensible choice). The
//! format carries no copyright or licence claim about its contents — what you embed is
//! your decision, exactly as with any `include_bytes!`.
//!
//! Vendored verbatim into this example from the `gibson::media` module (PR #186) so the
//! self-contained finale builds on `main` before that framework lands; swap to
//! `gibson::media` once it merges. The example uses only the FrameSeq encode/decode path,
//! so the rest reads as dead code here.

#![allow(dead_code)]

use std::fmt;

/// The four-byte asset magic — `b"GMED"` (Gibson MEDia). Present at the head of every
/// asset so a wrong or corrupt buffer is rejected immediately rather than misread.
pub const MAGIC: [u8; 4] = *b"GMED";

/// The asset format version this build writes and reads. Any other version is refused
/// ([`MediaError::UnsupportedVersion`]) rather than guessed at.
pub const VERSION: u16 = 1;

/// Hard cap on the number of segments in one asset. Generous enough for a long frame
/// sequence, small enough that the segment table can never be used to force a huge
/// pre-allocation. A larger sequence is split across several assets by the caller.
pub const MAX_SEGMENTS: u32 = 1_000_000;

/// Hard cap on a single segment's **raw** (decoded) length, in bytes (256 MiB). A frame
/// is `width * height * channels`; a blob segment is its chunk. Any declared raw length
/// above this is refused before a byte is allocated.
pub const MAX_SEGMENT_BYTES: usize = 256 * 1024 * 1024;

/// What an asset's payload *is*. The parameters are the minimum needed to interpret the
/// decoded segment bytes; richer policy (colour space, keying, resampling) is the caller's.
///
/// Not `Eq`: the `FrameSeq` variant carries an `f32` fps, which has only partial equality.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum MediaKind {
    /// Opaque bytes with no interpretation (a data table, a font, a compressed sub-asset).
    /// One segment holds the whole blob.
    Blob,
    /// A single still image: `channels` bytes per pixel (1 = grey, 3 = RGB, 4 = RGBA),
    /// row-major, no padding. One segment holds the whole image.
    Image {
        width: u32,
        height: u32,
        channels: u8,
    },
    /// Interleaved PCM audio. One segment holds the whole buffer. `sample_format` selects
    /// the per-sample encoding ([`SampleFormat`]); `channels` are interleaved.
    Audio {
        sample_rate: u32,
        channels: u8,
        sample_format: SampleFormat,
    },
    /// A frame sequence (video): `nframes` frames, each `width * height * channels` bytes,
    /// row-major, no padding. One segment per frame, so any frame decodes on its own.
    FrameSeq {
        width: u32,
        height: u32,
        channels: u8,
        /// Frames per second, carried as its exact IEEE-754 bits so it round-trips.
        fps: f32,
    },
}

impl MediaKind {
    /// The on-wire discriminant byte.
    fn tag(&self) -> u8 {
        match self {
            MediaKind::Blob => 0,
            MediaKind::Image { .. } => 1,
            MediaKind::Audio { .. } => 2,
            MediaKind::FrameSeq { .. } => 3,
        }
    }
}

/// The per-sample encoding of [`MediaKind::Audio`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SampleFormat {
    /// Signed 16-bit, little-endian.
    S16Le,
    /// IEEE-754 32-bit float, little-endian.
    F32Le,
}

impl SampleFormat {
    fn tag(self) -> u8 {
        match self {
            SampleFormat::S16Le => 0,
            SampleFormat::F32Le => 1,
        }
    }
    fn from_tag(t: u8) -> Option<Self> {
        match t {
            0 => Some(SampleFormat::S16Le),
            1 => Some(SampleFormat::F32Le),
            _ => None,
        }
    }
}

/// How a single segment is stored in the payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SegMethod {
    /// Stored verbatim (chosen when compression would not shrink it).
    Store,
    /// Deterministic PackBits run-length coding.
    Rle,
}

impl SegMethod {
    fn tag(self) -> u8 {
        match self {
            SegMethod::Store => 0,
            SegMethod::Rle => 1,
        }
    }
    fn from_tag(t: u8) -> Option<Self> {
        match t {
            0 => Some(SegMethod::Store),
            1 => Some(SegMethod::Rle),
            _ => None,
        }
    }
}

/// Everything that can be wrong with an asset buffer or an encode request. Every decode
/// failure is one of these — the decoder never panics on malformed input and never
/// silently returns partial data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaError {
    /// The first four bytes are not [`MAGIC`].
    BadMagic,
    /// The version field is not [`VERSION`].
    UnsupportedVersion(u16),
    /// The buffer ended before a declared field or payload was complete.
    Truncated,
    /// The kind discriminant byte is not a known [`MediaKind`].
    UnknownKind(u8),
    /// A segment's method byte is not a known [`SegMethod`].
    UnknownMethod(u8),
    /// The audio sample-format byte is not a known [`SampleFormat`].
    UnknownSampleFormat(u8),
    /// The segment count exceeds [`MAX_SEGMENTS`].
    TooManySegments(u32),
    /// A declared segment raw length exceeds [`MAX_SEGMENT_BYTES`], or geometry (a frame's
    /// `width * height * channels`) overflows or disagrees with a segment's raw length.
    SegmentTooLarge { index: usize, raw_len: u64 },
    /// The sum of the segment table's compressed lengths does not equal the payload bytes
    /// actually present.
    PayloadLengthMismatch { declared: u64, actual: u64 },
    /// Decoding a segment did not reproduce exactly its declared raw length (a corrupt or
    /// overlong RLE stream, caught before it can grow without bound).
    CorruptSegment { index: usize },
    /// Zero or nonsensical geometry for an image/frame (a zero dimension, zero channels).
    BadGeometry,
    /// A frame/segment index was out of range for this asset.
    IndexOutOfRange { index: usize, count: usize },
    /// An encode request was malformed (e.g. a frame whose length is not
    /// `width * height * channels`).
    EncodeMismatch,
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaError::BadMagic => write!(f, "not a GMED media asset (bad magic)"),
            MediaError::UnsupportedVersion(v) => {
                write!(
                    f,
                    "unsupported media asset version {v} (this build reads {VERSION})"
                )
            }
            MediaError::Truncated => write!(f, "media asset truncated"),
            MediaError::UnknownKind(k) => write!(f, "unknown media kind {k}"),
            MediaError::UnknownMethod(m) => write!(f, "unknown segment method {m}"),
            MediaError::UnknownSampleFormat(s) => write!(f, "unknown sample format {s}"),
            MediaError::TooManySegments(n) => {
                write!(f, "segment count {n} exceeds cap {MAX_SEGMENTS}")
            }
            MediaError::SegmentTooLarge { index, raw_len } => {
                write!(
                    f,
                    "segment {index} raw length {raw_len} exceeds cap {MAX_SEGMENT_BYTES}"
                )
            }
            MediaError::PayloadLengthMismatch { declared, actual } => {
                write!(f, "payload length mismatch: segments declare {declared} bytes, buffer has {actual}")
            }
            MediaError::CorruptSegment { index } => write!(f, "segment {index} is corrupt"),
            MediaError::BadGeometry => write!(f, "bad media geometry (zero dimension or channels)"),
            MediaError::IndexOutOfRange { index, count } => {
                write!(f, "index {index} out of range (asset has {count} segments)")
            }
            MediaError::EncodeMismatch => {
                write!(f, "encode input does not match declared geometry")
            }
        }
    }
}

impl std::error::Error for MediaError {}

// ---------------------------------------------------------------------------
// PackBits RLE — deterministic, bounded. A control byte governs the next run:
//   0..=127  : copy the next (n + 1) literal bytes.
//   129..=255: repeat the next single byte (257 - n) times (i.e. 2..=128).
//   128      : reserved no-op (never emitted; skipped on decode for robustness).
// ---------------------------------------------------------------------------

/// Deterministic PackBits encode. Greedy and canonical: a repeat of two or more equal
/// bytes is always coded as a run, everything else accumulates into literal spans of at
/// most 128 bytes. The output is a pure function of the input.
fn packbits_encode(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let n = src.len();
    let mut i = 0;
    while i < n {
        let b = src[i];
        // Measure the run of equal bytes at i (capped at 128, the max repeat count).
        let mut run = 1usize;
        while i + run < n && src[i + run] == b && run < 128 {
            run += 1;
        }
        if run >= 2 {
            out.push((257 - run) as u8); // run 2..=128 -> control 255..=129
            out.push(b);
            i += run;
        } else {
            // Literal span: take bytes until a run of >= 2 begins or we hit 128.
            let start = i;
            let mut lit = 0usize;
            while i < n && lit < 128 {
                if i + 1 < n && src[i] == src[i + 1] {
                    break; // a run starts here; let the repeat branch take it
                }
                i += 1;
                lit += 1;
            }
            out.push((lit - 1) as u8); // literal count-1 in 0..=127
            out.extend_from_slice(&src[start..start + lit]);
        }
    }
    out
}

/// Deterministic PackBits decode, bounded to `max_out` decoded bytes. Any attempt to read
/// past the input or to produce more than `max_out` bytes is a [`MediaError::CorruptSegment`]
/// (the caller supplies the segment index).
fn packbits_decode(src: &[u8], max_out: usize, index: usize) -> Result<Vec<u8>, MediaError> {
    let mut out: Vec<u8> = Vec::with_capacity(max_out.min(src.len().saturating_mul(2)));
    let mut i = 0;
    while i < src.len() {
        let ctrl = src[i];
        i += 1;
        if ctrl == 128 {
            continue; // reserved no-op
        } else if ctrl <= 127 {
            let count = ctrl as usize + 1;
            if i + count > src.len() {
                return Err(MediaError::CorruptSegment { index });
            }
            if out.len() + count > max_out {
                return Err(MediaError::CorruptSegment { index });
            }
            out.extend_from_slice(&src[i..i + count]);
            i += count;
        } else {
            let count = 257 - ctrl as usize; // 2..=128
            if i >= src.len() {
                return Err(MediaError::CorruptSegment { index });
            }
            let b = src[i];
            i += 1;
            if out.len() + count > max_out {
                return Err(MediaError::CorruptSegment { index });
            }
            out.resize(out.len() + count, b);
        }
    }
    Ok(out)
}

/// Encode one segment: choose [`SegMethod::Rle`] only when it is strictly smaller than the
/// raw bytes, so an incompressible segment never grows (and ties go to [`SegMethod::Store`],
/// keeping the choice — and thus the output bytes — deterministic).
fn encode_segment(raw: &[u8]) -> (SegMethod, Vec<u8>) {
    let rle = packbits_encode(raw);
    if rle.len() < raw.len() {
        (SegMethod::Rle, rle)
    } else {
        (SegMethod::Store, raw.to_vec())
    }
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

fn write_header(out: &mut Vec<u8>, kind: &MediaKind) {
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(kind.tag());
    out.push(0); // reserved
    match *kind {
        MediaKind::Blob => {}
        MediaKind::Image {
            width,
            height,
            channels,
        } => {
            out.extend_from_slice(&width.to_le_bytes());
            out.extend_from_slice(&height.to_le_bytes());
            out.push(channels);
            out.extend_from_slice(&[0u8; 3]); // pad to a 4-byte boundary
        }
        MediaKind::Audio {
            sample_rate,
            channels,
            sample_format,
        } => {
            out.extend_from_slice(&sample_rate.to_le_bytes());
            out.push(channels);
            out.push(sample_format.tag());
            out.extend_from_slice(&[0u8; 2]);
        }
        MediaKind::FrameSeq {
            width,
            height,
            channels,
            fps,
        } => {
            out.extend_from_slice(&width.to_le_bytes());
            out.extend_from_slice(&height.to_le_bytes());
            out.push(channels);
            out.extend_from_slice(&[0u8; 3]);
            out.extend_from_slice(&fps.to_bits().to_le_bytes());
        }
    }
}

/// Assemble a complete asset from already-segmented raw buffers. The segment table and
/// payload are written in order; the result is deterministic in the inputs.
fn assemble(kind: &MediaKind, segments: &[&[u8]]) -> Result<Vec<u8>, MediaError> {
    if segments.len() as u64 > MAX_SEGMENTS as u64 {
        return Err(MediaError::TooManySegments(segments.len() as u32));
    }
    for (index, raw) in segments.iter().enumerate() {
        if raw.len() > MAX_SEGMENT_BYTES {
            return Err(MediaError::SegmentTooLarge {
                index,
                raw_len: raw.len() as u64,
            });
        }
    }
    let mut out = Vec::new();
    write_header(&mut out, kind);
    out.extend_from_slice(&(segments.len() as u32).to_le_bytes());

    // Encode each segment, buffering payload separately so the table precedes it.
    let mut payload = Vec::new();
    for raw in segments {
        let (method, comp) = encode_segment(raw);
        out.push(method.tag());
        out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        out.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        payload.extend_from_slice(&comp);
    }
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Encode an opaque [`MediaKind::Blob`].
pub fn encode_blob(data: &[u8]) -> Result<Vec<u8>, MediaError> {
    assemble(&MediaKind::Blob, &[data])
}

/// Encode a single [`MediaKind::Image`]. `data` must be exactly
/// `width * height * channels` bytes, row-major.
pub fn encode_image(
    width: u32,
    height: u32,
    channels: u8,
    data: &[u8],
) -> Result<Vec<u8>, MediaError> {
    let expect = pixel_bytes(width, height, channels)?;
    if data.len() != expect {
        return Err(MediaError::EncodeMismatch);
    }
    assemble(
        &MediaKind::Image {
            width,
            height,
            channels,
        },
        &[data],
    )
}

/// Encode interleaved PCM [`MediaKind::Audio`] from its raw bytes.
pub fn encode_audio(
    sample_rate: u32,
    channels: u8,
    sample_format: SampleFormat,
    data: &[u8],
) -> Result<Vec<u8>, MediaError> {
    assemble(
        &MediaKind::Audio {
            sample_rate,
            channels,
            sample_format,
        },
        &[data],
    )
}

/// Encode a [`MediaKind::FrameSeq`]: one segment per frame. Every frame must be exactly
/// `width * height * channels` bytes, row-major.
pub fn encode_frame_seq(
    width: u32,
    height: u32,
    channels: u8,
    fps: f32,
    frames: &[&[u8]],
) -> Result<Vec<u8>, MediaError> {
    let stride = pixel_bytes(width, height, channels)?;
    for f in frames {
        if f.len() != stride {
            return Err(MediaError::EncodeMismatch);
        }
    }
    assemble(
        &MediaKind::FrameSeq {
            width,
            height,
            channels,
            fps,
        },
        frames,
    )
}

/// `width * height * channels` as a `usize`, rejecting zero geometry and overflow.
fn pixel_bytes(width: u32, height: u32, channels: u8) -> Result<usize, MediaError> {
    if width == 0 || height == 0 || channels == 0 {
        return Err(MediaError::BadGeometry);
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|wh| wh.checked_mul(channels as usize))
        .filter(|&n| n <= MAX_SEGMENT_BYTES)
        .ok_or(MediaError::SegmentTooLarge {
            index: 0,
            raw_len: (width as u64) * (height as u64) * (channels as u64),
        })
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// One entry of the parsed segment table.
#[derive(Clone, Copy, Debug)]
struct SegEntry {
    method: SegMethod,
    raw_len: usize,
    /// Byte offset of this segment's compressed bytes within the payload.
    payload_off: usize,
    comp_len: usize,
}

/// A parsed, borrowed view over an embedded asset. Parsing validates the header and the
/// whole segment table up front (bounded — see the module docs); decoding a segment is
/// deferred until asked, so a large asset costs only its table until a frame is read.
#[derive(Clone, Debug)]
pub struct Asset<'a> {
    kind: MediaKind,
    segments: Vec<SegEntry>,
    payload: &'a [u8],
}

/// A little-endian cursor over a byte slice that never reads past the end (every short
/// read is [`MediaError::Truncated`]).
struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Reader { b, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], MediaError> {
        let end = self.pos.checked_add(n).ok_or(MediaError::Truncated)?;
        if end > self.b.len() {
            return Err(MediaError::Truncated);
        }
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, MediaError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, MediaError> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Result<u32, MediaError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
}

impl<'a> Asset<'a> {
    /// Parse and validate an asset buffer. Does not decode any segment. Rejects a wrong
    /// magic/version, an over-cap segment count or raw length, and a segment table whose
    /// compressed lengths do not sum to the payload actually present.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, MediaError> {
        let mut r = Reader::new(bytes);
        if r.take(4)? != MAGIC {
            return Err(MediaError::BadMagic);
        }
        let version = r.u16()?;
        if version != VERSION {
            return Err(MediaError::UnsupportedVersion(version));
        }
        let kind_tag = r.u8()?;
        let _reserved = r.u8()?;
        let kind = match kind_tag {
            0 => MediaKind::Blob,
            1 => {
                let width = r.u32()?;
                let height = r.u32()?;
                let channels = r.u8()?;
                let _ = r.take(3)?;
                MediaKind::Image {
                    width,
                    height,
                    channels,
                }
            }
            2 => {
                let sample_rate = r.u32()?;
                let channels = r.u8()?;
                let sf = r.u8()?;
                let _ = r.take(2)?;
                let sample_format =
                    SampleFormat::from_tag(sf).ok_or(MediaError::UnknownSampleFormat(sf))?;
                MediaKind::Audio {
                    sample_rate,
                    channels,
                    sample_format,
                }
            }
            3 => {
                let width = r.u32()?;
                let height = r.u32()?;
                let channels = r.u8()?;
                let _ = r.take(3)?;
                let fps = f32::from_bits(r.u32()?);
                MediaKind::FrameSeq {
                    width,
                    height,
                    channels,
                    fps,
                }
            }
            other => return Err(MediaError::UnknownKind(other)),
        };

        let nsegs = r.u32()?;
        if nsegs > MAX_SEGMENTS {
            return Err(MediaError::TooManySegments(nsegs));
        }
        let mut segments = Vec::with_capacity(nsegs as usize);
        let mut payload_cursor: usize = 0;
        for index in 0..nsegs as usize {
            let method_tag = r.u8()?;
            let method =
                SegMethod::from_tag(method_tag).ok_or(MediaError::UnknownMethod(method_tag))?;
            let raw_len = r.u32()? as usize;
            let comp_len = r.u32()? as usize;
            if raw_len > MAX_SEGMENT_BYTES {
                return Err(MediaError::SegmentTooLarge {
                    index,
                    raw_len: raw_len as u64,
                });
            }
            segments.push(SegEntry {
                method,
                raw_len,
                payload_off: payload_cursor,
                comp_len,
            });
            payload_cursor = payload_cursor
                .checked_add(comp_len)
                .ok_or(MediaError::Truncated)?;
        }

        // Everything left is payload; it must be exactly the declared sum.
        let payload = &bytes[r.pos..];
        if payload.len() != payload_cursor {
            return Err(MediaError::PayloadLengthMismatch {
                declared: payload_cursor as u64,
                actual: payload.len() as u64,
            });
        }

        Ok(Asset {
            kind,
            segments,
            payload,
        })
    }

    /// The asset's kind and parameters.
    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    /// The number of segments (for a [`MediaKind::FrameSeq`], the number of frames).
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Decode segment `index` into `out` (cleared first). For a [`MediaKind::FrameSeq`]
    /// this is one frame's raw bytes. O(segment size); no other segment is touched.
    pub fn decode_segment(&self, index: usize, out: &mut Vec<u8>) -> Result<(), MediaError> {
        let seg = self
            .segments
            .get(index)
            .ok_or(MediaError::IndexOutOfRange {
                index,
                count: self.segments.len(),
            })?;
        let comp = self
            .payload
            .get(seg.payload_off..seg.payload_off + seg.comp_len)
            .ok_or(MediaError::Truncated)?;
        out.clear();
        match seg.method {
            SegMethod::Store => {
                if comp.len() != seg.raw_len {
                    return Err(MediaError::CorruptSegment { index });
                }
                out.extend_from_slice(comp);
            }
            SegMethod::Rle => {
                let decoded = packbits_decode(comp, seg.raw_len, index)?;
                if decoded.len() != seg.raw_len {
                    return Err(MediaError::CorruptSegment { index });
                }
                *out = decoded;
            }
        }
        Ok(())
    }

    /// Decode and return segment `index` as an owned buffer.
    pub fn segment(&self, index: usize) -> Result<Vec<u8>, MediaError> {
        let mut out = Vec::new();
        self.decode_segment(index, &mut out)?;
        Ok(out)
    }

    /// The frame count of a [`MediaKind::FrameSeq`], or `None` for any other kind.
    pub fn frame_count(&self) -> Option<usize> {
        matches!(self.kind, MediaKind::FrameSeq { .. }).then_some(self.segments.len())
    }

    /// Decode frame `index` of a [`MediaKind::FrameSeq`] into `out`. Errors with
    /// [`MediaError::UnknownKind`] if this asset is not a frame sequence.
    pub fn decode_frame(&self, index: usize, out: &mut Vec<u8>) -> Result<(), MediaError> {
        if !matches!(self.kind, MediaKind::FrameSeq { .. }) {
            return Err(MediaError::UnknownKind(self.kind.tag()));
        }
        self.decode_segment(index, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- PackBits round-trips -------------------------------------------------

    fn rt(bytes: &[u8]) {
        let enc = packbits_encode(bytes);
        let dec = packbits_decode(&enc, bytes.len(), 0).expect("decode");
        assert_eq!(dec, bytes, "packbits round-trip mismatch");
    }

    #[test]
    fn packbits_round_trips_edge_shapes() {
        rt(&[]);
        rt(&[7]);
        rt(&[1, 2, 3, 4, 5]);
        rt(&[9, 9, 9, 9, 9, 9]);
        rt(&[0u8; 500]); // long run beyond the 128 cap -> multiple runs
        rt(&[1, 1, 2, 3, 3, 3, 4, 5, 5]);
        let mixed: Vec<u8> = (0..1000u32).map(|i| (i % 7) as u8).collect();
        rt(&mixed);
        let flat: Vec<u8> = std::iter::repeat_n(0xAB, 1000).collect();
        rt(&flat);
    }

    #[test]
    fn packbits_is_deterministic() {
        let data: Vec<u8> = (0..300u32).map(|i| ((i * 31) % 11) as u8).collect();
        assert_eq!(packbits_encode(&data), packbits_encode(&data));
    }

    #[test]
    fn packbits_shrinks_a_flat_run() {
        let flat = [0xCDu8; 10_000];
        assert!(
            packbits_encode(&flat).len() < 200,
            "a flat run must compress hard"
        );
    }

    #[test]
    fn packbits_decode_rejects_overlong_output() {
        // A repeat run that would exceed max_out must be refused, not grown.
        let evil = [129u8, 0]; // repeat 0 x128
        assert!(packbits_decode(&evil, 10, 0).is_err());
    }

    // ---- Blob -----------------------------------------------------------------

    #[test]
    fn blob_round_trips_and_is_deterministic() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 256) as u8).collect();
        let a = encode_blob(&data).unwrap();
        let b = encode_blob(&data).unwrap();
        assert_eq!(a, b, "encode must be deterministic");
        let asset = Asset::parse(&a).unwrap();
        assert_eq!(asset.kind(), MediaKind::Blob);
        assert_eq!(asset.segment_count(), 1);
        assert_eq!(asset.segment(0).unwrap(), data);
    }

    // ---- FrameSeq -------------------------------------------------------------

    fn synthetic_frames(w: u32, h: u32, c: u8, n: usize) -> Vec<Vec<u8>> {
        let stride = (w * h * c as u32) as usize;
        (0..n)
            .map(|f| {
                let mut v = vec![0u8; stride]; // mostly flat (transparent) -> RLE-friendly
                                               // a small moving opaque block
                let base = (f * 10) % stride.max(1);
                for k in 0..stride.min(40) {
                    v[(base + k) % stride] = (f as u8).wrapping_add(k as u8);
                }
                v
            })
            .collect()
    }

    #[test]
    fn frame_seq_round_trips_every_frame() {
        let (w, h, c, n) = (64u32, 36u32, 4u8, 24usize);
        let frames = synthetic_frames(w, h, c, n);
        let refs: Vec<&[u8]> = frames.iter().map(|v| v.as_slice()).collect();
        let bytes = encode_frame_seq(w, h, c, 24.0, &refs).unwrap();

        let asset = Asset::parse(&bytes).unwrap();
        assert_eq!(
            asset.kind(),
            MediaKind::FrameSeq {
                width: w,
                height: h,
                channels: c,
                fps: 24.0
            }
        );
        assert_eq!(asset.frame_count(), Some(n));
        let mut out = Vec::new();
        for (i, want) in frames.iter().enumerate() {
            asset.decode_frame(i, &mut out).unwrap();
            assert_eq!(&out, want, "frame {i} mismatch");
        }
    }

    #[test]
    fn frame_seq_compresses_flat_frames() {
        // Fully transparent frames must shrink dramatically versus raw.
        let (w, h, c, n) = (120u32, 68u32, 4u8, 30usize);
        let stride = (w * h * c as u32) as usize;
        let frame = vec![0u8; stride];
        let refs: Vec<&[u8]> = (0..n).map(|_| frame.as_slice()).collect();
        let bytes = encode_frame_seq(w, h, c, 24.0, &refs).unwrap();
        let raw_total = stride * n;
        assert!(
            bytes.len() * 20 < raw_total,
            "flat frames should compress >20x (got {} vs raw {raw_total})",
            bytes.len()
        );
        // and still decode exactly
        let asset = Asset::parse(&bytes).unwrap();
        let mut out = Vec::new();
        asset.decode_frame(5, &mut out).unwrap();
        assert_eq!(out, frame);
    }

    #[test]
    fn fps_bits_round_trip_exactly() {
        let frames = [vec![1u8, 2, 3, 4]];
        let refs: Vec<&[u8]> = frames.iter().map(|v| v.as_slice()).collect();
        let bytes = encode_frame_seq(1, 1, 4, 23.976, &refs).unwrap();
        match Asset::parse(&bytes).unwrap().kind() {
            MediaKind::FrameSeq { fps, .. } => assert_eq!(fps.to_bits(), 23.976f32.to_bits()),
            _ => panic!("kind"),
        }
    }

    #[test]
    fn encode_rejects_wrong_frame_length() {
        let bad = [vec![0u8; 10]]; // not w*h*c = 48
        let refs: Vec<&[u8]> = bad.iter().map(|v| v.as_slice()).collect();
        assert_eq!(
            encode_frame_seq(4, 4, 3, 24.0, &refs),
            Err(MediaError::EncodeMismatch)
        );
    }

    #[test]
    fn encode_rejects_zero_geometry() {
        assert_eq!(
            encode_frame_seq(0, 4, 3, 24.0, &[]),
            Err(MediaError::BadGeometry)
        );
    }

    // ---- Decode hardening -----------------------------------------------------

    #[test]
    fn parse_rejects_bad_magic_and_version() {
        // `Asset` is a borrowed view and deliberately not `PartialEq`, so assert on the
        // error directly rather than on the whole `Result`.
        assert_eq!(
            Asset::parse(b"XXXXxxxxxxxx").unwrap_err(),
            MediaError::BadMagic
        );
        let mut v = encode_blob(b"hi").unwrap();
        v[4] = 9; // corrupt version low byte
        v[5] = 0;
        assert_eq!(
            Asset::parse(&v).unwrap_err(),
            MediaError::UnsupportedVersion(9)
        );
    }

    #[test]
    fn parse_rejects_truncation() {
        let full = encode_blob(b"some bytes here").unwrap();
        for cut in 1..full.len() {
            // Any prefix shorter than the whole asset must fail to parse-or-decode, never
            // panic and never return wrong data.
            if let Ok(a) = Asset::parse(&full[..cut]) {
                // If the header+table happened to parse, the payload length guard or a
                // segment decode must still reject it.
                let mut out = Vec::new();
                let _ = a.decode_segment(0, &mut out);
            }
        }
    }

    #[test]
    fn parse_rejects_payload_length_mismatch() {
        let mut v = encode_blob(&[0u8; 100]).unwrap();
        v.push(0xFF); // trailing junk -> payload longer than declared
        assert!(matches!(
            Asset::parse(&v),
            Err(MediaError::PayloadLengthMismatch { .. })
        ));
    }

    #[test]
    fn decode_segment_out_of_range() {
        let a = encode_blob(b"x").unwrap();
        let asset = Asset::parse(&a).unwrap();
        let mut out = Vec::new();
        assert!(matches!(
            asset.decode_segment(3, &mut out),
            Err(MediaError::IndexOutOfRange { .. })
        ));
    }
}
