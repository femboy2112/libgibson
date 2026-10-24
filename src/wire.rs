//! A narrow, versioned, **bounded** wire transport for an out-of-process scientific
//! producer (issue #125 — SmartChem-facing producer canary, Wave 5).
//!
//! The division of labour is deliberate and matches the snapshot bridge's Law IV:
//!
//! * **The producer owns its data and its model ticks.** A Python (or any) process
//!   computes a scalar field each logical tick and publishes it as a self-describing,
//!   identified `WireField` observation. It never links this library.
//! * **The presenter owns frame realization and interaction.** It decodes the stream,
//!   stores the latest observation per source in a bounded `FieldInbox`, renders it
//!   through [`crate::field_view::FieldView`], and sends *edits* back as ordered
//!   `WireCommand`s carried by the bounded [`crate::snapshot::SnapshotBridge`].
//!
//! This module is **I/O-free and substrate-agnostic**: it operates on byte buffers, not
//! sockets or pipes. The caller chooses the transport (stdio, a Unix socket, a TCP
//! stream) and feeds bytes to a `FrameDecoder`; the decoder yields typed messages. The
//! reference producer at `bindings/python/producer.py` writes exactly this format with
//! nothing but the Python standard library.
//!
//! # The bounded-memory discipline (why this is safe against a diseased producer)
//!
//! A length-prefixed stream is the classic out-of-memory vector: a frame that *claims*
//! four gigabytes makes a naive decoder pre-allocate four gigabytes. This decoder never
//! does. It **rejects the declared length against a hard cap before allocating anything**
//! (`WireError::FrameTooLarge`), and its accumulation buffer is itself bounded to one
//! maximum frame (`WireError::BufferOverflow`). The only bytes it ever holds are bytes
//! the caller already handed it, plus at most one bounded field. There is no
//! amplification. The `FieldInbox` then caps *retained* observations the same way the
//! [`crate::snapshot::SnapshotBridge`] caps frames: latest-wins per source, a hard source
//! ceiling, and typed refusal at the ceiling — never silent growth, never silent eviction.
//!
//! # Chronology — epoch and sequence (Law L6, and "old frame after a newer one")
//!
//! Every message carries an `(epoch, sequence)` cursor for its `source`. `sequence` is
//! monotonic within an epoch; `epoch` bumps when a producer **restarts** (and resets its
//! sequence). Ordering is lexicographic on `(epoch, sequence)`: a strictly greater cursor
//! supersedes, anything not strictly greater is **stale** and dropped with an honest
//! count. A higher epoch always wins — that is precisely how a restarted producer, whose
//! sequence has fallen back to low numbers, is accepted rather than rejected as stale.
//!
//! # Boundary — what this does not promise
//!
//! This is an **in-memory, in-process** transport contract plus a byte format. It makes no
//! durability or exactly-once claim: a process death loses the buffer. A truncated final
//! frame (a producer that crashed mid-write) is reported by `FrameDecoder::finish` as
//! `WireError::Truncated`, not silently accepted. Non-finite data (`NaN`, `±∞`) is a
//! legal payload that is transmitted **faithfully** (its exact IEEE-754 bit pattern) and
//! disclosed downstream as [`crate::field_view::Sample::NonFinite`]; the wire never lies
//! about the data by coercing it.

use crate::field_view::FieldExtent;
use crate::shape::Point;
use std::collections::HashMap;

/// The four-byte frame magic — `b"GWIR"` (GibsonWIRe). Present at the head of **every**
/// frame so a desynchronized or corrupt stream is detected immediately rather than
/// silently misframed.
pub const MAGIC: [u8; 4] = *b"GWIR";

/// The wire protocol version this build speaks. A frame with any other version is refused
/// ([`WireError::UnsupportedVersion`]) — the contract is explicit, never best-effort.
pub const VERSION: u16 = 1;

/// The fixed header size in bytes: magic(4) + version(2) + msg_type(2) + epoch(4) +
/// sequence(8) + source(8) + payload_len(4).
pub const HEADER_BYTES: usize = 32;

const MSG_FIELD: u16 = 1;
const MSG_COMMAND: u16 = 2;
const MSG_ACK: u16 = 3;
const MSG_RETIRE: u16 = 4;

/// Fixed prefix of a field payload before its data array: cols(4) + rows(4) + origin_x(8)
/// + origin_y(8) + cell_w(8) + cell_h(8) + model_tick(8) + flags(4).
const FIELD_HEAD_BYTES: usize = 52;
/// Fixed size of a command payload before its argument bytes: command_id(8) + kind(4) +
/// arg_len(4).
const COMMAND_HEAD_BYTES: usize = 16;
/// Fixed size of an ack payload: command_id(8) + status(4).
const ACK_BYTES: usize = 12;

/// Flag bit: the field carries a per-cell mask after its data array. No other bit is
/// defined; an unknown bit is a malformed frame ([`WireError::BadFlags`]).
const FLAG_HAS_MASK: u32 = 1;

/// Explicit ceilings for a [`FrameDecoder`]. There is no "unbounded"; you name the budget
/// and the decoder enforces it with typed refusals before it allocates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireLimits {
    /// Hard cap on a frame's declared `payload_len`. A header claiming more is refused
    /// ([`WireError::FrameTooLarge`]) *before* any payload buffer is touched. This also
    /// bounds the decoder's accumulation buffer to `HEADER_BYTES + max_payload_bytes`.
    pub max_payload_bytes: usize,
    /// Hard cap on a field's `cols * rows`. A field claiming more is refused
    /// ([`WireError::FieldTooLarge`]). Bounds a single observation's cell count.
    pub max_field_cells: usize,
}

impl WireLimits {
    /// Default field-cell ceiling: 2^20 = 1,048,576 cells (e.g. 1024×1024).
    pub const DEFAULT_MAX_FIELD_CELLS: usize = 1 << 20;
    /// Default payload ceiling: enough for a default-max field as `f64` data plus a
    /// one-byte-per-cell mask plus the fixed field head, with headroom.
    pub const DEFAULT_MAX_PAYLOAD_BYTES: usize =
        Self::DEFAULT_MAX_FIELD_CELLS * (8 + 1) + FIELD_HEAD_BYTES + 64;

    /// Limits with the given explicit ceilings.
    pub fn new(max_payload_bytes: usize, max_field_cells: usize) -> Self {
        Self {
            max_payload_bytes,
            max_field_cells,
        }
    }

    /// The largest command `arg` that fits under
    /// [`max_payload_bytes`](WireLimits::max_payload_bytes) once the fixed command header is
    /// accounted for. A command whose `arg` exceeds this would encode to a frame the matching
    /// decoder must reject, so [`encode_command_checked`] and [`issue_command`] refuse it first.
    pub fn max_command_arg_bytes(&self) -> usize {
        self.max_payload_bytes.saturating_sub(COMMAND_HEAD_BYTES)
    }
}

impl Default for WireLimits {
    fn default() -> Self {
        Self::new(
            Self::DEFAULT_MAX_PAYLOAD_BYTES,
            Self::DEFAULT_MAX_FIELD_CELLS,
        )
    }
}

/// Acknowledgement status for a command on the return channel. Mirrors the five-state
/// command lifecycle of [`crate::snapshot::SnapshotBridge`] so a producer can branch on
/// the exact fate of each edit it was sent, never guessing at a swallowed command (L4/L7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckStatus {
    /// Queued by the presenter; not yet delivered.
    Accepted,
    /// Handed out of the presenter's bridge toward the producer.
    Delivered,
    /// The producer confirms it received (and will act on) the command.
    Acknowledged,
    /// Declined as typed backpressure; the command was not retained — re-offer it.
    Refused,
    /// Delivered but the producer could not apply it. Not a success (L7: never erased).
    Failed,
}

impl AckStatus {
    fn to_u32(self) -> u32 {
        match self {
            AckStatus::Accepted => 0,
            AckStatus::Delivered => 1,
            AckStatus::Acknowledged => 2,
            AckStatus::Refused => 3,
            AckStatus::Failed => 4,
        }
    }

    fn from_u32(v: u32) -> Option<AckStatus> {
        Some(match v {
            0 => AckStatus::Accepted,
            1 => AckStatus::Delivered,
            2 => AckStatus::Acknowledged,
            3 => AckStatus::Refused,
            4 => AckStatus::Failed,
            _ => return None,
        })
    }
}

/// A scalar-field observation decoded from the wire. Geometry is carried as a validated
/// [`FieldExtent`] — the *same* type pointer selection uses — so a decoded field's
/// world↔index mapping is identical to the editor's, by construction (Law L3). `data` is
/// row-major (`index = iy * cols + ix`); `mask`, when present, uses the field-view
/// convention where `true` means **present/valid** and `false` means **masked**.
#[derive(Debug, Clone, PartialEq)]
pub struct WireField {
    /// Validated geometry (cols, rows, origin, cell sizes).
    pub extent: FieldExtent,
    /// The producer's authoritative logical model tick for this observation (L6 — a frame
    /// carries the tick it depicts, independent of wall-clock or render time).
    pub model_tick: u64,
    /// Row-major scalar data, `len == extent.len()`. Non-finite values are preserved.
    pub data: Vec<f64>,
    /// Optional per-cell mask, `len == extent.len()` when present; `true` = present/valid.
    pub mask: Option<Vec<bool>>,
}

/// An edit sent back to the producer on the return channel. The library owns the ordered,
/// bounded delivery discipline (via [`crate::snapshot::SnapshotBridge`]); the producer
/// owns what `kind` and `arg` *mean*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireCommand {
    /// Ordered identity, monotonic per presenter session, so the producer can detect loss
    /// or reordering and validate before applying.
    pub command_id: u64,
    /// Application-defined command kind.
    pub kind: u32,
    /// Opaque application payload.
    pub arg: Vec<u8>,
}

/// An acknowledgement for a previously-sent command (either direction may carry it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireAck {
    /// The `command_id` being acknowledged.
    pub command_id: u64,
    /// Its fate.
    pub status: AckStatus,
}

/// The `(epoch, sequence)` chronology cursor for a source. Ordered lexicographically: a
/// higher epoch wins outright (a producer restart), and within an epoch a higher sequence
/// wins. This is the ordering the [`FieldInbox`] uses to drop stale observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceCursor {
    /// Producer run/restart generation. Bumps on restart; resets `sequence`.
    pub epoch: u32,
    /// Monotonic observation sequence within the epoch.
    pub sequence: u64,
}

impl SourceCursor {
    /// A cursor.
    pub fn new(epoch: u32, sequence: u64) -> Self {
        Self { epoch, sequence }
    }
}

/// The body of a decoded message.
#[derive(Debug, Clone, PartialEq)]
pub enum WireBody {
    /// A scalar-field observation (producer → presenter).
    Field(WireField),
    /// An edit (presenter → producer).
    Command(WireCommand),
    /// An acknowledgement.
    Ack(WireAck),
    /// A source announcing it is retiring (no payload).
    Retire,
}

/// A fully decoded message: the source/chronology header plus a typed body.
#[derive(Debug, Clone, PartialEq)]
pub struct WireMessage {
    /// Source identity (aligns with [`crate::frame::FrameSource`]'s `u64`).
    pub source: u64,
    /// The chronology cursor for this message.
    pub cursor: SourceCursor,
    /// The typed payload.
    pub body: WireBody,
}

/// A typed decode/encode failure. Every malformed condition is named, never a silent drop
/// and never a panic (L7). See [`WireError::is_fatal`] for whether the stream can continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// The frame did not begin with [`MAGIC`] — the stream is desynchronized or corrupt.
    /// Fatal: without a frame boundary the decoder cannot resynchronize.
    BadMagic { found: [u8; 4] },
    /// The frame's version is not [`VERSION`]. Fatal: the layout is not understood.
    UnsupportedVersion { found: u16, supported: u16 },
    /// The message type byte is unknown. Recoverable: the frame length is known, so it is
    /// skipped and decoding continues.
    UnknownMessageType { found: u16 },
    /// The declared `payload_len` exceeds [`WireLimits::max_payload_bytes`]. Fatal: the
    /// decoder refuses to buffer toward a frame it would reject, and will not blindly skip
    /// a length it does not trust.
    FrameTooLarge { payload_len: usize, cap: usize },
    /// A field's `cols * rows` exceeds [`WireLimits::max_field_cells`]. Recoverable: the
    /// frame length is consistent, so it is skipped.
    FieldTooLarge { cells: usize, cap: usize },
    /// `cols * rows` (or its byte size) overflowed `usize`. Recoverable: the frame length
    /// is known, so it is skipped.
    DimensionOverflow,
    /// The declared `payload_len` did not equal the size computed from the payload's own
    /// fields. Recoverable: the declared length is skipped and decoding resynchronizes at
    /// the next frame.
    LengthMismatch { declared: usize, computed: usize },
    /// The field geometry was rejected by [`FieldExtent::new`] (non-finite or non-positive
    /// cell size, zero dimension, or an extent that overflows). Recoverable.
    BadExtent,
    /// A field's `flags` had a bit set that this version does not define. Recoverable.
    BadFlags { found: u32 },
    /// A mask byte was neither 0 nor 1. Recoverable.
    BadMaskByte { found: u8 },
    /// An ack status value was out of range. Recoverable.
    BadAckStatus { found: u32 },
    /// A `push` would grow the accumulation buffer past its bound
    /// (`HEADER_BYTES + max_payload_bytes`). Fatal: the stream is not being drained, or a
    /// frame larger than the cap is being dribbled in. The bytes were not appended.
    BufferOverflow { ceiling: usize },
    /// [`FrameDecoder::finish`] found buffered bytes that do not form a complete frame —
    /// the producer disconnected mid-frame.
    Truncated { buffered: usize },
    /// An encode was asked to produce a frame that would violate the limits (field too
    /// large or a dimension overflow). Encoding refuses to emit what a decoder must reject.
    EncodeTooLarge,
}

impl WireError {
    /// Whether this error desynchronizes the stream. A fatal error poisons the
    /// [`FrameDecoder`]: the caller should retire the source. A non-fatal error was
    /// confined to one skipped frame and decoding can continue.
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            WireError::BadMagic { .. }
                | WireError::UnsupportedVersion { .. }
                | WireError::FrameTooLarge { .. }
                | WireError::BufferOverflow { .. }
                | WireError::Truncated { .. }
        )
    }
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::BadMagic { found } => write!(f, "bad frame magic: {found:?}"),
            WireError::UnsupportedVersion { found, supported } => {
                write!(
                    f,
                    "unsupported wire version {found} (this build speaks {supported})"
                )
            }
            WireError::UnknownMessageType { found } => write!(f, "unknown message type {found}"),
            WireError::FrameTooLarge { payload_len, cap } => {
                write!(f, "frame payload {payload_len} exceeds cap {cap}")
            }
            WireError::FieldTooLarge { cells, cap } => {
                write!(f, "field {cells} cells exceeds cap {cap}")
            }
            WireError::DimensionOverflow => write!(f, "field dimensions overflow usize"),
            WireError::LengthMismatch { declared, computed } => {
                write!(f, "declared payload_len {declared} != computed {computed}")
            }
            WireError::BadExtent => write!(f, "field geometry rejected by FieldExtent"),
            WireError::BadFlags { found } => write!(f, "unknown field flag bits: {found:#x}"),
            WireError::BadMaskByte { found } => write!(f, "mask byte {found} is neither 0 nor 1"),
            WireError::BadAckStatus { found } => write!(f, "ack status {found} out of range"),
            WireError::BufferOverflow { ceiling } => {
                write!(f, "decoder buffer would exceed {ceiling} bytes")
            }
            WireError::Truncated { buffered } => {
                write!(
                    f,
                    "stream ended with {buffered} buffered bytes (truncated frame)"
                )
            }
            WireError::EncodeTooLarge => write!(f, "refusing to encode a frame beyond the limits"),
        }
    }
}

impl std::error::Error for WireError {}

// ---- little-endian readers (callers guarantee the slice is long enough) --------------

#[inline]
fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
#[inline]
fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
#[inline]
fn rd_u64(b: &[u8], o: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(a)
}
#[inline]
fn rd_f64(b: &[u8], o: usize) -> f64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    f64::from_le_bytes(a)
}

// ---- encoders ------------------------------------------------------------------------

fn push_header(
    out: &mut Vec<u8>,
    msg_type: u16,
    source: u64,
    cursor: SourceCursor,
    payload_len: u32,
) {
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&msg_type.to_le_bytes());
    out.extend_from_slice(&cursor.epoch.to_le_bytes());
    out.extend_from_slice(&cursor.sequence.to_le_bytes());
    out.extend_from_slice(&source.to_le_bytes());
    out.extend_from_slice(&payload_len.to_le_bytes());
}

/// Encode a field observation to bytes. Refuses ([`WireError::EncodeTooLarge`]) rather than
/// emit a field whose cell count or byte size would exceed `limits` — it never produces
/// what a matching decoder must reject. Returns the exact frame bytes (header + payload).
pub fn encode_field(
    source: u64,
    cursor: SourceCursor,
    field: &WireField,
    limits: WireLimits,
) -> Result<Vec<u8>, WireError> {
    let cells = field.extent.len();
    if cells > limits.max_field_cells {
        return Err(WireError::EncodeTooLarge);
    }
    if field.data.len() != cells {
        return Err(WireError::LengthMismatch {
            declared: cells,
            computed: field.data.len(),
        });
    }
    let has_mask = field.mask.is_some();
    if let Some(m) = &field.mask {
        if m.len() != cells {
            return Err(WireError::LengthMismatch {
                declared: cells,
                computed: m.len(),
            });
        }
    }
    let data_bytes = cells.checked_mul(8).ok_or(WireError::DimensionOverflow)?;
    let mask_bytes = if has_mask { cells } else { 0 };
    let payload = FIELD_HEAD_BYTES
        .checked_add(data_bytes)
        .and_then(|v| v.checked_add(mask_bytes))
        .ok_or(WireError::DimensionOverflow)?;
    if payload > limits.max_payload_bytes {
        return Err(WireError::EncodeTooLarge);
    }
    let payload_u32 = u32::try_from(payload).map_err(|_| WireError::EncodeTooLarge)?;

    let mut out = Vec::with_capacity(HEADER_BYTES + payload);
    push_header(&mut out, MSG_FIELD, source, cursor, payload_u32);
    let cols = u32::try_from(field.extent.cols()).map_err(|_| WireError::EncodeTooLarge)?;
    let rows = u32::try_from(field.extent.rows()).map_err(|_| WireError::EncodeTooLarge)?;
    out.extend_from_slice(&cols.to_le_bytes());
    out.extend_from_slice(&rows.to_le_bytes());
    // Origin and cell sizes come straight off the extent's public accessors, so the
    // geometry round-trips bit-exactly for every input (the decoder reconstructs the
    // extent from exactly these four f64s).
    let origin = field.extent.origin();
    out.extend_from_slice(&origin.x.to_le_bytes());
    out.extend_from_slice(&origin.y.to_le_bytes());
    out.extend_from_slice(&field.extent.cell_w().to_le_bytes());
    out.extend_from_slice(&field.extent.cell_h().to_le_bytes());
    out.extend_from_slice(&field.model_tick.to_le_bytes());
    let flags = if has_mask { FLAG_HAS_MASK } else { 0 };
    out.extend_from_slice(&flags.to_le_bytes());
    for v in &field.data {
        out.extend_from_slice(&v.to_le_bytes());
    }
    if let Some(m) = &field.mask {
        for &present in m {
            out.push(u8::from(present));
        }
    }
    debug_assert_eq!(out.len(), HEADER_BYTES + payload);
    Ok(out)
}

/// Encode a command (presenter → producer).
pub fn encode_command(source: u64, cursor: SourceCursor, command: &WireCommand) -> Vec<u8> {
    let arg_len = command.arg.len();
    let payload = COMMAND_HEAD_BYTES + arg_len;
    let mut out = Vec::with_capacity(HEADER_BYTES + payload);
    // arg_len is bounded by the caller; a command with a 4 GiB arg is a caller bug, and
    // the decoder's payload cap will reject it on the far side regardless.
    push_header(&mut out, MSG_COMMAND, source, cursor, payload as u32);
    out.extend_from_slice(&command.command_id.to_le_bytes());
    out.extend_from_slice(&command.kind.to_le_bytes());
    out.extend_from_slice(&(arg_len as u32).to_le_bytes());
    out.extend_from_slice(&command.arg);
    out
}

/// Encode a command, refusing ([`WireError::EncodeTooLarge`]) rather than emit a frame whose
/// payload would exceed `limits` — the checked sibling of [`encode_command`], which trusts the
/// caller to keep `arg` in bounds. Mirrors [`encode_field`]'s refuse-don't-emit contract, so a
/// locally-built command cannot bypass the byte cap the decoder enforces on the far side.
pub fn encode_command_checked(
    source: u64,
    cursor: SourceCursor,
    command: &WireCommand,
    limits: WireLimits,
) -> Result<Vec<u8>, WireError> {
    if command.arg.len() > limits.max_command_arg_bytes() {
        return Err(WireError::EncodeTooLarge);
    }
    Ok(encode_command(source, cursor, command))
}

/// Offer a command to a return-channel bridge, refusing an oversized `arg`
/// ([`WireError::EncodeTooLarge`]) *before* it is admitted — so a locally-constructed command
/// that bypasses the decoder still cannot pin more than
/// [`max_command_arg_bytes`](WireLimits::max_command_arg_bytes) of queued bytes per entry. On an
/// in-bounds command this is exactly
/// [`SnapshotBridge::issue`](crate::snapshot::SnapshotBridge::issue), so queue ordering and the
/// conservation law are untouched; the byte refusal happens ahead of the bridge and is never
/// counted as a queue admission.
pub fn issue_command(
    bridge: &mut crate::snapshot::SnapshotBridge<WireCommand>,
    command: WireCommand,
    limits: WireLimits,
) -> Result<crate::snapshot::Admission, WireError> {
    if command.arg.len() > limits.max_command_arg_bytes() {
        return Err(WireError::EncodeTooLarge);
    }
    Ok(bridge.issue(command))
}

/// Encode an acknowledgement.
pub fn encode_ack(source: u64, cursor: SourceCursor, ack: WireAck) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_BYTES + ACK_BYTES);
    push_header(&mut out, MSG_ACK, source, cursor, ACK_BYTES as u32);
    out.extend_from_slice(&ack.command_id.to_le_bytes());
    out.extend_from_slice(&ack.status.to_u32().to_le_bytes());
    out
}

/// Encode a source retirement (no payload).
pub fn encode_retire(source: u64, cursor: SourceCursor) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_BYTES);
    push_header(&mut out, MSG_RETIRE, source, cursor, 0);
    out
}

// ---- streaming decoder ---------------------------------------------------------------

/// A streaming, bounded decoder. Feed it bytes with [`push`](FrameDecoder::push) and pull
/// decoded messages with [`next_message`](FrameDecoder::next_message). It never allocates
/// toward a frame whose declared length exceeds the limits, and its own buffer is bounded
/// to one maximum frame. A fatal error (see [`WireError::is_fatal`]) poisons it: every
/// subsequent `next_message` returns that same error until the source is abandoned.
#[derive(Debug)]
pub struct FrameDecoder {
    buf: Vec<u8>,
    limits: WireLimits,
    poisoned: Option<WireError>,
    messages_decoded: u64,
    frames_skipped: u64,
    bytes_consumed: u64,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder {
    /// A decoder with [`WireLimits::default`] ceilings.
    pub fn new() -> Self {
        Self::with_limits(WireLimits::default())
    }

    /// A decoder with explicit limits.
    pub fn with_limits(limits: WireLimits) -> Self {
        Self {
            buf: Vec::new(),
            limits,
            poisoned: None,
            messages_decoded: 0,
            frames_skipped: 0,
            bytes_consumed: 0,
        }
    }

    /// The ceiling on the internal accumulation buffer: one maximum frame.
    fn buffer_ceiling(&self) -> usize {
        HEADER_BYTES + self.limits.max_payload_bytes
    }

    /// Append received bytes. Returns [`WireError::BufferOverflow`] (and appends nothing)
    /// if the buffer would grow past the buffer ceiling (one maximum frame) — the stream
    /// is not being drained, or a frame larger than the cap is being dribbled in. This is
    /// the bound that makes the decoder safe against a producer that sends a huge length
    /// and then trickles bytes forever.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        if let Some(e) = self.poisoned {
            return Err(e);
        }
        if self.buf.len() + bytes.len() > self.buffer_ceiling() {
            let e = WireError::BufferOverflow {
                ceiling: self.buffer_ceiling(),
            };
            self.poisoned = Some(e);
            return Err(e);
        }
        self.buf.extend_from_slice(bytes);
        Ok(())
    }

    /// Bytes currently buffered and not yet consumed.
    pub fn buffered_bytes(&self) -> usize {
        self.buf.len()
    }

    /// Total fully-decoded messages.
    pub fn messages_decoded(&self) -> u64 {
        self.messages_decoded
    }

    /// Total frames skipped due to a recoverable error.
    pub fn frames_skipped(&self) -> u64 {
        self.frames_skipped
    }

    /// Total frame bytes consumed from the stream.
    pub fn bytes_consumed(&self) -> u64 {
        self.bytes_consumed
    }

    /// Pull the next message.
    ///
    /// * `None` — not enough bytes buffered yet for a complete frame; push more.
    /// * `Some(Ok(msg))` — a decoded message; its frame was consumed.
    /// * `Some(Err(e))` — a decode error. If [`e.is_fatal()`](WireError::is_fatal) the
    ///   decoder is now poisoned and every further call returns `e`; otherwise exactly one
    ///   malformed frame was skipped and the next call resumes decoding.
    pub fn next_message(&mut self) -> Option<Result<WireMessage, WireError>> {
        if let Some(e) = self.poisoned {
            return Some(Err(e));
        }
        if self.buf.len() < HEADER_BYTES {
            return None;
        }
        // Header.
        let magic = [self.buf[0], self.buf[1], self.buf[2], self.buf[3]];
        if magic != MAGIC {
            return Some(Err(self.poison(WireError::BadMagic { found: magic })));
        }
        let version = rd_u16(&self.buf, 4);
        if version != VERSION {
            return Some(Err(self.poison(WireError::UnsupportedVersion {
                found: version,
                supported: VERSION,
            })));
        }
        let msg_type = rd_u16(&self.buf, 6);
        let epoch = rd_u32(&self.buf, 8);
        let sequence = rd_u64(&self.buf, 12);
        let source = rd_u64(&self.buf, 20);
        let payload_len = rd_u32(&self.buf, 28) as usize;

        if payload_len > self.limits.max_payload_bytes {
            return Some(Err(self.poison(WireError::FrameTooLarge {
                payload_len,
                cap: self.limits.max_payload_bytes,
            })));
        }
        let frame_len = HEADER_BYTES + payload_len;
        if self.buf.len() < frame_len {
            return None; // need the rest of this frame
        }

        // We now hold exactly one complete frame at the front; parse its payload from a
        // borrowed view, decide, then consume `frame_len` bytes regardless of outcome for
        // every *recoverable* case so the stream resynchronizes at the next frame.
        let cursor = SourceCursor { epoch, sequence };
        let result = self.decode_body(msg_type, payload_len);
        // Consume the frame. (A fatal error returned above via `poison` never reaches
        // here; recoverable errors and successes both consume exactly this frame.)
        self.buf.drain(..frame_len);
        self.bytes_consumed += frame_len as u64;
        match result {
            Ok(body) => {
                self.messages_decoded += 1;
                Some(Ok(WireMessage {
                    source,
                    cursor,
                    body,
                }))
            }
            Err(e) => {
                self.frames_skipped += 1;
                Some(Err(e))
            }
        }
    }

    /// Decode a payload body from `self.buf[HEADER_BYTES .. HEADER_BYTES + payload_len]`.
    /// Returns a recoverable error on malformed content; the caller consumes the frame
    /// either way.
    fn decode_body(&self, msg_type: u16, payload_len: usize) -> Result<WireBody, WireError> {
        let p = &self.buf[HEADER_BYTES..HEADER_BYTES + payload_len];
        match msg_type {
            MSG_FIELD => decode_field(p, self.limits).map(WireBody::Field),
            MSG_COMMAND => decode_command(p).map(WireBody::Command),
            MSG_ACK => decode_ack(p).map(WireBody::Ack),
            MSG_RETIRE => {
                if payload_len != 0 {
                    Err(WireError::LengthMismatch {
                        declared: payload_len,
                        computed: 0,
                    })
                } else {
                    Ok(WireBody::Retire)
                }
            }
            other => Err(WireError::UnknownMessageType { found: other }),
        }
    }

    /// Mark the decoder poisoned with a fatal error and return it.
    fn poison(&mut self, e: WireError) -> WireError {
        self.poisoned = Some(e);
        e
    }

    /// Assert the stream ended cleanly. Returns [`WireError::Truncated`] if bytes remain
    /// buffered that do not form a complete, consumed frame — i.e. the producer
    /// disconnected mid-frame.
    pub fn finish(&self) -> Result<(), WireError> {
        if let Some(e) = self.poisoned {
            return Err(e);
        }
        if self.buf.is_empty() {
            Ok(())
        } else {
            Err(WireError::Truncated {
                buffered: self.buf.len(),
            })
        }
    }
}

fn decode_field(p: &[u8], limits: WireLimits) -> Result<WireField, WireError> {
    if p.len() < FIELD_HEAD_BYTES {
        return Err(WireError::LengthMismatch {
            declared: p.len(),
            computed: FIELD_HEAD_BYTES,
        });
    }
    let cols = rd_u32(p, 0) as usize;
    let rows = rd_u32(p, 4) as usize;
    let origin_x = rd_f64(p, 8);
    let origin_y = rd_f64(p, 16);
    let cell_w = rd_f64(p, 24);
    let cell_h = rd_f64(p, 32);
    let model_tick = rd_u64(p, 40);
    let flags = rd_u32(p, 48);
    if flags & !FLAG_HAS_MASK != 0 {
        return Err(WireError::BadFlags { found: flags });
    }
    let has_mask = flags & FLAG_HAS_MASK != 0;

    let cells = cols.checked_mul(rows).ok_or(WireError::DimensionOverflow)?;
    if cells > limits.max_field_cells {
        return Err(WireError::FieldTooLarge {
            cells,
            cap: limits.max_field_cells,
        });
    }
    let data_bytes = cells.checked_mul(8).ok_or(WireError::DimensionOverflow)?;
    let mask_bytes = if has_mask { cells } else { 0 };
    let computed = FIELD_HEAD_BYTES
        .checked_add(data_bytes)
        .and_then(|v| v.checked_add(mask_bytes))
        .ok_or(WireError::DimensionOverflow)?;
    if computed != p.len() {
        return Err(WireError::LengthMismatch {
            declared: p.len(),
            computed,
        });
    }

    // Geometry: build the same validated extent pointer selection uses. A rejected extent
    // (non-finite/zero cell size, zero dim, overflow) is a malformed frame, not a panic.
    let extent = FieldExtent::new(Point::new(origin_x, origin_y), cell_w, cell_h, cols, rows)
        .ok_or(WireError::BadExtent)?;
    // FieldExtent::new validates cols/rows > 0, so cells matches extent.len().
    debug_assert_eq!(extent.len(), cells);

    let mut data = Vec::with_capacity(cells);
    let mut off = FIELD_HEAD_BYTES;
    for _ in 0..cells {
        data.push(rd_f64(p, off));
        off += 8;
    }
    let mask = if has_mask {
        let mut m = Vec::with_capacity(cells);
        for _ in 0..cells {
            let byte = p[off];
            off += 1;
            match byte {
                0 => m.push(false),
                1 => m.push(true),
                other => return Err(WireError::BadMaskByte { found: other }),
            }
        }
        Some(m)
    } else {
        None
    };

    Ok(WireField {
        extent,
        model_tick,
        data,
        mask,
    })
}

fn decode_command(p: &[u8]) -> Result<WireCommand, WireError> {
    if p.len() < COMMAND_HEAD_BYTES {
        return Err(WireError::LengthMismatch {
            declared: p.len(),
            computed: COMMAND_HEAD_BYTES,
        });
    }
    let command_id = rd_u64(p, 0);
    let kind = rd_u32(p, 8);
    let arg_len = rd_u32(p, 12) as usize;
    let computed = COMMAND_HEAD_BYTES + arg_len;
    if computed != p.len() {
        return Err(WireError::LengthMismatch {
            declared: p.len(),
            computed,
        });
    }
    let arg = p[COMMAND_HEAD_BYTES..].to_vec();
    Ok(WireCommand {
        command_id,
        kind,
        arg,
    })
}

fn decode_ack(p: &[u8]) -> Result<WireAck, WireError> {
    if p.len() != ACK_BYTES {
        return Err(WireError::LengthMismatch {
            declared: p.len(),
            computed: ACK_BYTES,
        });
    }
    let command_id = rd_u64(p, 0);
    let raw = rd_u32(p, 8);
    let status = AckStatus::from_u32(raw).ok_or(WireError::BadAckStatus { found: raw })?;
    Ok(WireAck { command_id, status })
}

// ---- bounded field inbox -------------------------------------------------------------

/// Explicit ceiling for a [`FieldInbox`] — the source count (an *entry* count, not a byte
/// total). The per-field cell count is bounded by the [`FrameDecoder`]'s [`WireLimits`] on the
/// decode path, so for decoder-admitted fields the retained bytes are capped at
/// `max_sources * max_field_cells * (size_of::<f64>() + 1)` — the `+ 1` is the optional mask
/// byte per cell, which is real and was previously understated. A field handed to
/// [`FieldInbox::admit`] directly (in-process, bypassing the decoder) is trusted to respect
/// those limits; the decoder is where an untrusted field's size is enforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InboxCapacity {
    /// Maximum number of distinct sources with a live latest observation. A new source
    /// beyond this is refused ([`FieldAdmission::SourceCapReached`]); none is evicted.
    pub max_sources: usize,
}

impl InboxCapacity {
    /// Default source ceiling, matching [`crate::snapshot::Capacity::DEFAULT_MAX_SOURCES`].
    pub const DEFAULT_MAX_SOURCES: usize = 64;

    /// A capacity with the given source ceiling.
    pub fn new(max_sources: usize) -> Self {
        Self { max_sources }
    }
}

impl Default for InboxCapacity {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_SOURCES)
    }
}

/// The fate of an observation offered to a [`FieldInbox`]. Never a silent drop (L7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldAdmission {
    /// Stored as the new latest for its source (first observation, a strictly-newer
    /// cursor, or the first after a restart to a higher epoch).
    Stored,
    /// Dropped as stale: its cursor was not strictly newer than the stored one. Lossy by
    /// design (latest-wins), counted by [`FieldInbox::superseded`].
    Stale {
        /// The cursor already stored for this source.
        stored: SourceCursor,
        /// The cursor of the rejected incoming observation.
        incoming: SourceCursor,
    },
    /// Refused: a new source when the inbox is at [`InboxCapacity::max_sources`]. No live
    /// source is evicted — admission, not eviction.
    SourceCapReached {
        /// The ceiling that was reached.
        capacity: usize,
    },
}

impl FieldAdmission {
    /// Whether the observation became the stored latest.
    pub fn is_stored(&self) -> bool {
        matches!(self, FieldAdmission::Stored)
    }
}

/// A bounded, latest-wins store of the most recent scalar-field observation per source —
/// the scalar-field analogue of [`crate::snapshot::SnapshotBridge`]'s frame side, with
/// `(epoch, sequence)` chronology so a restarted producer (higher epoch) is accepted and a
/// late-arriving stale observation (lower cursor) is dropped.
///
/// A presenter renders [`latest`](FieldInbox::latest) through
/// [`crate::field_view::FieldView`]. Because the stored geometry is a [`FieldExtent`],
/// probing the stored field uses the identical world↔index mapping as pointer selection.
#[derive(Debug, Clone)]
pub struct FieldInbox {
    latest: HashMap<u64, (SourceCursor, WireField)>,
    capacity: InboxCapacity,
    offered: u64,
    // Each offer lands in exactly one of these four terminal outcomes. Counting them
    // independently (rather than deriving one from the others) is what makes
    // `conserves` a real cross-check instead of a tautology.
    stored_new: u64,     // first frame for a source
    stored_replace: u64, // a strictly-newer frame displaced the stored latest
    stale: u64,          // an incoming frame not newer than the stored latest — dropped
    refused: u64,        // a new source at the ceiling — refused, nothing evicted
    retired: u64,
}

impl Default for FieldInbox {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldInbox {
    /// An empty inbox with [`InboxCapacity::default`].
    pub fn new() -> Self {
        Self::with_capacity(InboxCapacity::default())
    }

    /// An empty inbox with an explicit capacity.
    pub fn with_capacity(capacity: InboxCapacity) -> Self {
        Self {
            latest: HashMap::new(),
            capacity,
            offered: 0,
            stored_new: 0,
            stored_replace: 0,
            stale: 0,
            refused: 0,
            retired: 0,
        }
    }

    /// The ceiling this inbox enforces.
    pub fn capacity(&self) -> InboxCapacity {
        self.capacity
    }

    /// Offer an observation. Latest-wins by `(epoch, sequence)`: a strictly-greater cursor
    /// is [`Stored`](FieldAdmission::Stored) (any displaced field superseded); anything not
    /// strictly greater is [`Stale`](FieldAdmission::Stale). A new source beyond the
    /// ceiling is [`SourceCapReached`](FieldAdmission::SourceCapReached) — refused, nothing
    /// evicted.
    pub fn admit(&mut self, source: u64, cursor: SourceCursor, field: WireField) -> FieldAdmission {
        self.offered += 1;
        if let Some((stored_cursor, stored_field)) = self.latest.get_mut(&source) {
            if cursor > *stored_cursor {
                *stored_cursor = cursor;
                *stored_field = field;
                self.stored_replace += 1; // the displaced field is coalesced away
                FieldAdmission::Stored
            } else {
                self.stale += 1;
                FieldAdmission::Stale {
                    stored: *stored_cursor,
                    incoming: cursor,
                }
            }
        } else if self.latest.len() >= self.capacity.max_sources {
            self.refused += 1;
            FieldAdmission::SourceCapReached {
                capacity: self.capacity.max_sources,
            }
        } else {
            self.latest.insert(source, (cursor, field));
            self.stored_new += 1;
            FieldAdmission::Stored
        }
    }

    /// The latest stored field for `source`, if any.
    pub fn latest(&self, source: u64) -> Option<&WireField> {
        self.latest.get(&source).map(|(_, f)| f)
    }

    /// The cursor of the latest stored field for `source`, if any.
    pub fn latest_cursor(&self, source: u64) -> Option<SourceCursor> {
        self.latest.get(&source).map(|(c, _)| *c)
    }

    /// How many sources currently hold a live latest observation.
    pub fn live_sources(&self) -> usize {
        self.latest.len()
    }

    /// Retire a source, freeing its retained field. Returns whether one was present.
    pub fn retire(&mut self, source: u64) -> bool {
        let had = self.latest.remove(&source).is_some();
        if had {
            self.retired += 1;
        }
        had
    }

    /// Total retained cells across all live sources — the bounded-RSS witness.
    pub fn retained_cells(&self) -> usize {
        self.latest.values().map(|(_, f)| f.data.len()).sum()
    }

    /// Observations offered to [`admit`](Self::admit).
    pub fn offered(&self) -> u64 {
        self.offered
    }
    /// Observations that became the stored latest (first-for-a-source plus replacements).
    pub fn stored(&self) -> u64 {
        self.stored_new + self.stored_replace
    }
    /// First observations stored for a previously-unseen source.
    pub fn stored_new(&self) -> u64 {
        self.stored_new
    }
    /// Observations that displaced an existing latest for their source.
    pub fn stored_replace(&self) -> u64 {
        self.stored_replace
    }
    /// Observations dropped as stale (not newer than the stored latest).
    pub fn stale(&self) -> u64 {
        self.stale
    }
    /// Fields coalesced away: a displaced latest (one per replacement) plus every dropped
    /// stale incoming.
    pub fn superseded(&self) -> u64 {
        self.stored_replace + self.stale
    }
    /// Observations refused at the source ceiling.
    pub fn refused(&self) -> u64 {
        self.refused
    }
    /// Sources retired.
    pub fn retired(&self) -> u64 {
        self.retired
    }

    /// The conservation receipt (L7 — nothing offered vanishes silently). Two independent
    /// identities must hold:
    ///
    /// * **Every offer has exactly one terminal outcome:**
    ///   `offered == stored_new + stored_replace + stale + refused`.
    ///   If any `admit` path forgot to count its outcome, this breaks.
    /// * **Coalescing is consistent:** `superseded == stored_replace + stale` — a frame is
    ///   coalesced away exactly when it is a displaced latest or a dropped stale incoming.
    ///   (`superseded()` is defined from the same two figures, so this arm is a guard
    ///   against a future refactor decoupling them.)
    pub fn conserves(&self) -> bool {
        self.offered == self.stored_new + self.stored_replace + self.stale + self.refused
            && self.superseded() == self.stored_replace + self.stale
    }
}

// ---- bidirectional command ledger (the return channel's per-id fate) -----------------

/// Explicit ceiling for a [`CommandLedger`] — the maximum number of **outstanding**
/// (issued but not yet resolved) commands tracked at once. A resolved command frees its
/// slot, so this bounds the ledger's live memory *regardless* of how many commands flow
/// over its lifetime: a presenter that issues edits forever never grows the ledger past
/// this, because every edit is eventually acknowledged, refused or failed and pruned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerCapacity {
    /// Maximum simultaneously-outstanding commands. [`CommandLedger::note_issued`] beyond
    /// this is refused ([`LedgerReject::OutstandingCapReached`]) — backpressure, not growth.
    pub max_outstanding: usize,
}

impl LedgerCapacity {
    /// Default outstanding ceiling, matching [`crate::snapshot::Capacity`]'s default pending
    /// command budget so the ledger and the bridge it shadows agree on in-flight headroom.
    pub const DEFAULT_MAX_OUTSTANDING: usize = 256;

    /// A capacity with the given outstanding ceiling.
    pub fn new(max_outstanding: usize) -> Self {
        Self { max_outstanding }
    }
}

impl Default for LedgerCapacity {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_OUTSTANDING)
    }
}

/// The lifecycle state of a command the presenter issued on the return channel. Mirrors the
/// five-state [`AckStatus`] vocabulary, but as a *tracked local state* rather than a wire
/// value. `Accepted` and `Delivered` are **live** (a slot in the ledger); `Acknowledged`,
/// `Refused` and `Failed` are **terminal** and pruned to counters, so they are never a live
/// state (see [`CommandLedger`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandState {
    /// Issued and queued locally; not yet delivered over the wire.
    Accepted,
    /// Handed toward the producer; awaiting a verdict ack.
    Delivered,
    /// Terminal: the producer confirmed it (and the authoritative model moved).
    Acknowledged,
    /// Terminal: the producer declined it as typed backpressure — re-offer it.
    Refused,
    /// Terminal: the producer tried and could not apply it. Not a success (L7).
    Failed,
}

impl CommandState {
    /// Whether this state is terminal (no further transition is legal).
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            CommandState::Acknowledged | CommandState::Refused | CommandState::Failed
        )
    }
}

/// A typed refusal from a [`CommandLedger`]. Every illegal correlation is named — a
/// mis-correlated, duplicated, or out-of-contract ack is reported, never a silent drop and
/// never a panic (L7). The ledger's own invariants hold across every one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerReject {
    /// [`note_issued`](CommandLedger::note_issued) was called with a `command_id` that is not
    /// strictly greater than the last one issued. This *enforces* the
    /// [`WireCommand::command_id`] "monotonic per presenter session" contract at the ledger
    /// boundary rather than merely trusting it: a still-outstanding id (a duplicate) and a
    /// once-resolved-then-reused id are both refused, so a stale or duplicate ack can never be
    /// mis-bound to a different command that happened to reuse the number.
    NonMonotonicId {
        /// The offending id.
        command_id: u64,
        /// The highest id issued so far, or `None` if none has been.
        last_issued: Option<u64>,
    },
    /// [`note_issued`](CommandLedger::note_issued) at [`LedgerCapacity::max_outstanding`].
    /// The command was not tracked; drain acks or retire it before issuing more.
    OutstandingCapReached { capacity: usize },
    /// A delivery or ack referenced a `command_id` that is not outstanding — never issued,
    /// or already resolved and pruned. A post-resolution duplicate ack lands here (the
    /// resolved entry no longer exists), reported rather than silently swallowed.
    UnknownCommand { command_id: u64 },
    /// [`note_delivered`](CommandLedger::note_delivered) for a command not in
    /// [`CommandState::Accepted`] — it was already delivered. Delivery happens once.
    NotAwaitingDelivery {
        /// The offending command.
        command_id: u64,
        /// Its current live state.
        state: CommandState,
    },
    /// An ack arrived for a command still only [`CommandState::Accepted`] — you cannot get a
    /// verdict on an edit that was never delivered. A producer acking an undelivered command
    /// is a protocol fault, surfaced rather than trusted.
    AckBeforeDelivery { command_id: u64 },
    /// An ack carried a non-terminal status ([`AckStatus::Accepted`] or
    /// [`AckStatus::Delivered`]) on the return channel, which only carries verdicts.
    NonTerminalAck {
        /// The command the ack named.
        command_id: u64,
        /// The non-terminal status it carried.
        status: AckStatus,
    },
}

impl std::fmt::Display for LedgerReject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerReject::NonMonotonicId {
                command_id,
                last_issued,
            } => {
                write!(
                    f,
                    "command id {command_id} is not strictly greater than last issued {last_issued:?}"
                )
            }
            LedgerReject::OutstandingCapReached { capacity } => {
                write!(f, "outstanding-command ceiling {capacity} reached")
            }
            LedgerReject::UnknownCommand { command_id } => {
                write!(f, "command id {command_id} is not outstanding")
            }
            LedgerReject::NotAwaitingDelivery { command_id, state } => {
                write!(
                    f,
                    "command id {command_id} is {state:?}, not awaiting delivery"
                )
            }
            LedgerReject::AckBeforeDelivery { command_id } => {
                write!(f, "command id {command_id} was acked before delivery")
            }
            LedgerReject::NonTerminalAck { command_id, status } => {
                write!(
                    f,
                    "ack for command id {command_id} carried non-terminal status {status:?}"
                )
            }
        }
    }
}

impl std::error::Error for LedgerReject {}

/// The presenter-side fate tracker for commands issued on the return channel.
///
/// [`crate::snapshot::SnapshotBridge`] owns the ordered, bounded *queuing and delivery* of
/// commands, with **aggregate** counts. What it cannot do is correlate a returned
/// [`WireAck`] back to the specific command it answers: `acknowledge_delivered(n)` is a blind
/// count bump with no identity and no verdict. The ledger closes that gap — it maps each
/// `command_id` through [`Accepted`](CommandState::Accepted) →
/// [`Delivered`](CommandState::Delivered) → one of the three terminal verdicts, so the
/// presenter knows the exact fate of every edit and a producer cannot silently swallow one
/// (Law L4 — every accepted command is accounted for; Law L7 — no condition is erased).
///
/// It is **bounded**: only outstanding (non-terminal) commands occupy a live slot, capped at
/// [`LedgerCapacity::max_outstanding`]; a resolved command is pruned to a counter. It is
/// **pure**: no I/O and no clock, so the whole correlation is deterministic and testable
/// without a subprocess or a timing race.
///
/// # Boundary
///
/// Because a resolved command is pruned, a *second* ack for an already-resolved id cannot be
/// told apart from an ack for an id that was never issued — both are
/// [`LedgerReject::UnknownCommand`]. That is a deliberate memory/fidelity trade: the ledger
/// is a live-fate tracker plus terminal tallies, not an unbounded audit log. The rejection is
/// always typed; nothing is swallowed.
#[derive(Debug, Clone)]
pub struct CommandLedger {
    live: HashMap<u64, CommandState>,
    capacity: LedgerCapacity,
    /// The highest `command_id` ever issued; `note_issued` requires strict increase, which
    /// is what closes the id-reuse mis-correlation across the resolution boundary.
    last_issued: Option<u64>,
    issued: u64,
    delivered_ever: u64,
    acknowledged: u64,
    refused: u64,
    failed: u64,
}

impl Default for CommandLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandLedger {
    /// A ledger with [`LedgerCapacity::default`].
    pub fn new() -> Self {
        Self::with_capacity(LedgerCapacity::default())
    }

    /// A ledger with an explicit capacity.
    pub fn with_capacity(capacity: LedgerCapacity) -> Self {
        Self {
            live: HashMap::new(),
            capacity,
            last_issued: None,
            issued: 0,
            delivered_ever: 0,
            acknowledged: 0,
            refused: 0,
            failed: 0,
        }
    }

    /// The ceiling this ledger enforces.
    pub fn capacity(&self) -> LedgerCapacity {
        self.capacity
    }

    /// Begin tracking a command the presenter has just issued (accepted by the bridge). The
    /// `command_id` must be **strictly greater than every id issued before** (enforcing the
    /// [`WireCommand::command_id`] monotonicity contract), and the ledger must be below its
    /// outstanding ceiling.
    ///
    /// Returns [`LedgerReject::NonMonotonicId`] for an id that is not strictly increasing — a
    /// duplicate of a still-outstanding id *and* a reuse of a once-resolved id are both caught,
    /// so a stale or duplicate ack can never be mis-bound to a reused number. Returns
    /// [`LedgerReject::OutstandingCapReached`] at the ceiling. The command is not tracked in
    /// either case (and `last_issued` is unchanged on refusal).
    pub fn note_issued(&mut self, command_id: u64) -> Result<(), LedgerReject> {
        if let Some(last) = self.last_issued {
            if command_id <= last {
                return Err(LedgerReject::NonMonotonicId {
                    command_id,
                    last_issued: self.last_issued,
                });
            }
        }
        if self.live.len() >= self.capacity.max_outstanding {
            return Err(LedgerReject::OutstandingCapReached {
                capacity: self.capacity.max_outstanding,
            });
        }
        self.live.insert(command_id, CommandState::Accepted);
        self.last_issued = Some(command_id);
        self.issued += 1;
        Ok(())
    }

    /// Record that an accepted command has been delivered toward the producer (handed out of
    /// the bridge and written to the wire). Legal exactly once, from
    /// [`Accepted`](CommandState::Accepted).
    ///
    /// Returns [`LedgerReject::UnknownCommand`] if the id is not outstanding, or
    /// [`LedgerReject::NotAwaitingDelivery`] if it was already delivered.
    pub fn note_delivered(&mut self, command_id: u64) -> Result<(), LedgerReject> {
        match self.live.get_mut(&command_id) {
            None => Err(LedgerReject::UnknownCommand { command_id }),
            Some(state @ CommandState::Accepted) => {
                *state = CommandState::Delivered;
                self.delivered_ever += 1;
                Ok(())
            }
            Some(&mut state) => Err(LedgerReject::NotAwaitingDelivery { command_id, state }),
        }
    }

    /// Apply a verdict ack returned by the producer, resolving the command to its terminal
    /// fate and pruning it from the live set. On success returns the terminal
    /// [`CommandState`] ([`Acknowledged`](CommandState::Acknowledged),
    /// [`Refused`](CommandState::Refused) or [`Failed`](CommandState::Failed)).
    ///
    /// Rejections (all typed, nothing swallowed), checked in this order — identity first, so a
    /// caller branching on the reject type always learns "was this command ever mine?" before
    /// anything about the ack's contents:
    /// * [`LedgerReject::UnknownCommand`] — no outstanding command with that id.
    /// * [`LedgerReject::AckBeforeDelivery`] — the command was issued but never delivered.
    /// * [`LedgerReject::NonTerminalAck`] — the ack status was `Accepted`/`Delivered`.
    pub fn apply_ack(&mut self, ack: WireAck) -> Result<CommandState, LedgerReject> {
        match self.live.get(&ack.command_id) {
            None => Err(LedgerReject::UnknownCommand {
                command_id: ack.command_id,
            }),
            Some(CommandState::Accepted) => Err(LedgerReject::AckBeforeDelivery {
                command_id: ack.command_id,
            }),
            Some(CommandState::Delivered) => {
                let terminal = match ack.status {
                    AckStatus::Acknowledged => CommandState::Acknowledged,
                    AckStatus::Refused => CommandState::Refused,
                    AckStatus::Failed => CommandState::Failed,
                    AckStatus::Accepted | AckStatus::Delivered => {
                        return Err(LedgerReject::NonTerminalAck {
                            command_id: ack.command_id,
                            status: ack.status,
                        });
                    }
                };
                self.live.remove(&ack.command_id);
                match terminal {
                    CommandState::Acknowledged => self.acknowledged += 1,
                    CommandState::Refused => self.refused += 1,
                    CommandState::Failed => self.failed += 1,
                    // Unreachable: `terminal` is one of the three above by construction.
                    CommandState::Accepted | CommandState::Delivered => unreachable!(),
                }
                Ok(terminal)
            }
            // Terminal states are pruned, so a live entry is only ever Accepted/Delivered.
            Some(&other) => Err(LedgerReject::NotAwaitingDelivery {
                command_id: ack.command_id,
                state: other,
            }),
        }
    }

    /// The live state of a command, or `None` if it was never issued or has been resolved
    /// (terminal states are pruned). A returned state is always `Accepted` or `Delivered`.
    pub fn state(&self, command_id: u64) -> Option<CommandState> {
        self.live.get(&command_id).copied()
    }

    /// How many commands are currently outstanding (live — `Accepted` or `Delivered`).
    pub fn outstanding(&self) -> usize {
        self.live.len()
    }

    /// Live commands still only `Accepted` (issued, not yet delivered).
    pub fn accepted_now(&self) -> u64 {
        self.live
            .values()
            .filter(|&&s| s == CommandState::Accepted)
            .count() as u64
    }

    /// Live commands `Delivered` and awaiting a verdict.
    pub fn delivered_now(&self) -> u64 {
        self.live
            .values()
            .filter(|&&s| s == CommandState::Delivered)
            .count() as u64
    }

    /// Commands ever issued (tracked).
    pub fn issued(&self) -> u64 {
        self.issued
    }
    /// Commands ever delivered (the `Accepted` → `Delivered` transition count).
    pub fn delivered_ever(&self) -> u64 {
        self.delivered_ever
    }
    /// Commands resolved as [`Acknowledged`](CommandState::Acknowledged).
    pub fn acknowledged(&self) -> u64 {
        self.acknowledged
    }
    /// Commands resolved as [`Refused`](CommandState::Refused).
    pub fn refused(&self) -> u64 {
        self.refused
    }
    /// Commands resolved as [`Failed`](CommandState::Failed).
    pub fn failed(&self) -> u64 {
        self.failed
    }
    /// Commands resolved to any terminal fate.
    pub fn resolved(&self) -> u64 {
        self.acknowledged + self.refused + self.failed
    }

    /// The command-fate conservation receipt (Law L4/L7). Three independent identities, each of
    /// which breaks if a transition forgot to move its bookkeeping. Crucially,
    /// [`accepted_now`](Self::accepted_now) and [`delivered_now`](Self::delivered_now) scan the
    /// live map *independently* of the counters, so a map/counter desync is actually caught
    /// rather than being hidden by a definitional restatement:
    ///
    /// * **Every issued command is in exactly one place:**
    ///   `issued == accepted_now + delivered_ever` (live `Accepted` scan vs the delivered
    ///   counter).
    /// * **Everything delivered is either awaiting or resolved:**
    ///   `delivered_ever == delivered_now + resolved` (live `Delivered` scan vs the terminal
    ///   counters).
    /// * **No terminal state ever leaks into the live set:**
    ///   `outstanding == accepted_now + delivered_now` — every live entry is `Accepted` or
    ///   `Delivered`, nothing else.
    ///
    /// Together these force `issued == accepted_now + delivered_now + resolved`: nothing the
    /// presenter issued is unaccounted for and nothing was double-counted.
    pub fn conserves(&self) -> bool {
        self.issued == self.accepted_now() + self.delivered_ever
            && self.delivered_ever == self.delivered_now() + self.resolved()
            && self.outstanding() as u64 == self.accepted_now() + self.delivered_now()
    }
}
