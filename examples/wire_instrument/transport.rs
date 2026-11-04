// Shared by the example and by `tests/wire_instrument_transport.rs` via `#[path]`; not every
// consumer exercises every accessor, so follow the channel/workbench convention and quiet
// dead_code for the whole transport module rather than scattering per-method allows.
#![allow(dead_code)]
//! Bounded, deadlock-free inbound transport for `wire_instrument` (Renaissance VII, Gate B).
//!
//! # The knot this module unties (2A + 2C are the *same* defect)
//!
//! A full-duplex consumer over two pipes — reading the producer's FIELD stream on its stdout
//! while writing COMMAND frames to its stdin — has a classic deadlock. The reference producer's
//! `serve` loop writes each field with a **blocking** `flush()` and only *then* drains its stdin
//! non-blockingly; so if the consumer ever stops draining stdout, the OS pipe fills, the producer
//! blocks in `flush()`, and while blocked it never reaches its stdin drain. If the consumer is at
//! that moment blocked writing a command to the now-unread stdin, both sides wait forever.
//!
//! The first cut of this example dodged that deadlock with an **unbounded** `mpsc` channel: a
//! reader thread drained the producer's stdout into the heap as fast as it arrived, so the
//! producer never blocked on stdout — at the cost of unbounded memory under any UI stall (an L7
//! violation: an undisclosed queue that grows without limit). Bounding that channel with a
//! *blocking* `sync_channel` would have handed the deadlock right back: a full channel blocks the
//! reader, the reader stops draining stdout, the producer blocks in `flush()`, and a subsequent
//! command write deadlocks.
//!
//! This module keeps the reader **non-blocking at the bound** and so bounds memory *without*
//! reintroducing the deadlock. The reader never waits on a consumer: when its buffer is full it
//! **coalesces** (collapses a run of consecutive same-source FIELD frames to the latest — lawful
//! under the declared snapshot-coalescing policy, since [`FieldInbox`](gibson::wire::FieldInbox)
//! already keeps only the latest per source) rather than blocking. The producer's stdout is
//! therefore always drained, the producer never blocks in `flush()`, and a command write can
//! never deadlock. Memory is bounded to one pending field per source plus a bounded control
//! queue.
//!
//! # 2B — decode is interleaved with accumulation, incrementally
//!
//! [`FrameDecoder`] self-bounds its accumulation buffer to one maximum frame and returns a
//! **fatal** `BufferOverflow` past it, and its [`push`](FrameDecoder::push) enforces that ceiling
//! *atomically* — it rejects a chunk whole when `buffered + incoming` would exceed one frame,
//! because it cannot foresee that draining the frame the chunk completes is about to free that
//! space. Two input shapes therefore defeat a naive "push the whole chunk, then drain" loop on
//! perfectly legal data: a backlog of chunks pushed before any drain (Renaissance VII), and — the
//! one that survived it — a single read that straddles a frame boundary at the ceiling (the tail
//! of a near-maximum frame plus the head of the next), whose combined length exceeds one frame
//! even though each frame is legal (Renaissance VIII, Gate B). [`FramePump::pump`] closes both by
//! feeding at most [`remaining_capacity`](FrameDecoder::remaining_capacity) bytes at a time and
//! draining between, so the decoder never holds more than one frame and the overflow can fire only
//! on a genuinely oversized *declared* frame — which is what it is for. Legal streams stay legal
//! regardless of where the OS chopped the read.
//!
//! # What is never dropped (L5) — two lanes, not one cap
//!
//! Fields and control do not share a budget, because they are not equally expendable. A FIELD is
//! a *snapshot*: lawful to collapse (consecutive same-source frames coalesce to their newest) and
//! lawful to drop under flood (a dropped snapshot is superseded by the next one). Control — ACK /
//! RETIRE / COMMAND — is a *verdict*: an edit's acknowledged fate, a source's retirement. It is
//! never coalesced, never reordered, and **never silently dropped**.
//!
//! So the buffer reserves a separate control lane. Under the declared bounded-producer contract a
//! conformant producer's control volume has a real ceiling (one ack per outstanding edit, and
//! outstanding edits are themselves capped by the [`CommandChannel`](crate::channel::CommandChannel)),
//! so control is admitted in full and the field lane — flooded or not — can never starve it.
//!
//! The feasibility boundary is honest: finite memory, an unbounded/out-of-contract producer,
//! indefinitely-preserved control, and never-blocking ingestion cannot all hold at once. When an
//! out-of-contract flood exhausts the reserved control lane, the buffer does **not** keep running
//! while counting lost verdicts behind a misleading `dropped=N` — that would let the consumer go
//! on advertising command-lifecycle correctness after losing an authoritative ACK. Instead it
//! latches a typed, terminal [`ControlOverflow`] (fail-closed): the consumer surfaces it like a
//! fatal transport error and stops. Expendable field drops are still *counted*
//! ([`CoalescingBuffer::fields_dropped`], truthful L7 telemetry); a lost control verdict is a
//! *terminal state*, not a counter.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read};
use std::process::{Child, ChildStdin};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use gibson::wire::{FrameDecoder, WireBody, WireError, WireMessage};

/// The reader's stdout read granularity. Real reads are far under the decoder's one-frame
/// ceiling, so a single [`FramePump::pump`] of one chunk never trips `BufferOverflow`.
const READ_CHUNK: usize = 65536;

/// Default reserved budget for **non-coalesceable** control messages (ACK/RETIRE/COMMAND).
/// Generous: a conformant producer's acks are one-per-outstanding-edit and bounded well under
/// this, so control is admitted in full; only an out-of-contract flood exhausts it, and that
/// latches a terminal [`ControlOverflow`] rather than silently dropping a verdict.
pub const DEFAULT_CONTROL_CAP: usize = 4096;

/// Default cap on retained **un-coalesced** FIELD entries. Coalescing already collapses a run of
/// consecutive same-source snapshots to one, so this is only reached by many *distinct* sources
/// (or a pathological source-alternating flood); past it, the oldest-lane admission is refused and
/// the expendable snapshot is counted as a lawful drop — fields never consume the control lane.
pub const DEFAULT_FIELD_CAP: usize = 1024;

/// Default byte budget for retained **FIELD** payload (f64 data + optional mask) — the
/// dominant, variable memory the transport holds. A message-count cap alone does **not**
/// bound memory: one legal field carries up to `2^20` cells × 8 bytes (+ a 1-byte-per-cell
/// mask) ≈ 9 MiB, so 1024 count-bounded entries is a multi-GiB retention envelope. This
/// byte cap bounds that regardless of count. 64 MiB comfortably holds several maximum-size
/// fields across distinct sources (so a single legal field is always admissible) while
/// giving a documented worst-case envelope; past it the oldest expendable snapshots are
/// evicted (latest-wins), never a control verdict. Tune via [`CoalescingBuffer::with_byte_budget`].
pub const DEFAULT_FIELD_BYTE_CAP: usize = 64 * 1024 * 1024;

/// Which non-expendable control body overflowed the reserved control lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    /// An edit acknowledgment (`ACK`).
    Ack,
    /// A source retirement (`RETIRE`).
    Retire,
    /// A command (`COMMAND`).
    Command,
}

/// The control lane overflowed its reserved budget: an authoritative `ACK`/`RETIRE`/`COMMAND`
/// could not be admitted. Unlike an expendable field snapshot, a control verdict must never be
/// silently discarded, so the transport latches this terminal, fail-closed state rather than
/// continuing to advertise command-lifecycle correctness behind a misleading drop counter. The
/// consumer surfaces it like a fatal transport error: the stream is dead and every outstanding
/// command's fate is unresolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlOverflow {
    /// The reserved control-lane capacity that was exhausted.
    pub control_cap: usize,
    /// The kind of control message that could not be admitted.
    pub kind: ControlKind,
}

/// Pushes raw byte chunks into a [`FrameDecoder`] one at a time and drains the frames each chunk
/// completes, so the decoder's accumulation buffer never grows past one chunk plus one partial
/// frame. This is the interleaved parse/drain discipline that makes a spurious fatal
/// `BufferOverflow` on legal fast input impossible (Gate B, 2B).
pub struct FramePump {
    dec: FrameDecoder,
}

impl Default for FramePump {
    fn default() -> Self {
        Self::new()
    }
}

impl FramePump {
    /// A pump over a default-limits decoder.
    pub fn new() -> Self {
        Self {
            dec: FrameDecoder::new(),
        }
    }

    /// A pump over a caller-configured decoder (e.g. tightened [`WireLimits`](gibson::wire::WireLimits)).
    pub fn with_decoder(dec: FrameDecoder) -> Self {
        Self { dec }
    }

    /// Feed `chunk` into the decoder **incrementally**, draining every complete frame as room is
    /// freed, and hand each decoded message to `sink` in stream order.
    ///
    /// The subtlety this exists for (Gate B straddle): [`FrameDecoder::push`] refuses a chunk
    /// *atomically* when `buffered + incoming` would exceed the one-frame ceiling — it cannot know
    /// that draining the frame the chunk *completes* is about to free that very space. So a single
    /// legal read that carries the tail of a near-ceiling frame plus the head of the next would
    /// trip a spurious fatal `BufferOverflow` if pushed whole. Instead this feeds at most
    /// [`remaining_capacity`](FrameDecoder::remaining_capacity) bytes at a time and drains between,
    /// so the decoder never holds more than one frame and a legal stream stays legal regardless of
    /// where the OS chopped the read.
    ///
    /// Returns `Err` only on a **fatal** decoder condition (bad magic, unsupported version, a
    /// declared frame larger than the cap, or a genuine buffer overflow from an undrainable frame);
    /// the decoder is poisoned and the caller should treat the stream as dead. A *recoverable*
    /// per-frame decode error is swallowed here — the decoder skips exactly that frame and
    /// resynchronizes — because a single malformed frame must not tear down a live instrument.
    pub fn pump(
        &mut self,
        chunk: &[u8],
        sink: &mut impl FnMut(WireMessage),
    ) -> Result<(), WireError> {
        let mut rest = chunk;
        loop {
            // Drain whatever is already complete first, freeing buffer headroom.
            loop {
                match self.dec.next_message() {
                    None => break,
                    Some(Ok(msg)) => sink(msg),
                    Some(Err(e)) if e.is_fatal() => return Err(e),
                    Some(Err(_)) => { /* recoverable skip; the decoder resynchronized */ }
                }
            }
            if rest.is_empty() {
                return Ok(());
            }
            let headroom = self.dec.remaining_capacity();
            if headroom == 0 {
                // The buffer is full (== one frame) yet nothing drained: the front is an
                // undrainable frame — a genuinely oversized declared payload. Force the decoder
                // to surface its fatal (`FrameTooLarge`/`BufferOverflow`); this never pushes on
                // success because `headroom == 0` means the push must overflow.
                self.dec.push(&rest[..1])?;
                rest = &rest[1..]; // defensive: unreachable on a well-formed decoder
                continue;
            }
            let take = rest.len().min(headroom);
            // Bounded to headroom, so this push never overflows; the only Err it can return is a
            // pre-existing poisoning, which is correctly fatal.
            self.dec.push(&rest[..take])?;
            rest = &rest[take..];
        }
    }

    /// Bytes currently held in the decoder's accumulation buffer (one partial frame at most, after
    /// a `pump` returns).
    pub fn buffered_bytes(&self) -> usize {
        self.dec.buffered_bytes()
    }

    /// Total frames the decoder skipped due to a **recoverable** error (unknown message type,
    /// malformed field content, an undefined flag bit, …) and resynchronized past. This is the
    /// countable receipt the pump's `Err`-free `Ok(())` would otherwise hide: a single malformed
    /// frame must not tear down a live instrument (so `pump` swallows it), but it must still be
    /// *observable* rather than vanishing without a trace (Gate C4). Fatal errors never count here
    /// — they poison the decoder and surface through [`pump`](FramePump::pump)'s `Err`.
    pub fn frames_skipped(&self) -> u64 {
        self.dec.frames_skipped()
    }
}

/// Classify a body: `None` for an expendable FIELD snapshot, `Some(kind)` for a non-expendable
/// control verdict.
fn control_kind(body: &WireBody) -> Option<ControlKind> {
    match body {
        WireBody::Field(_) => None,
        WireBody::Ack(_) => Some(ControlKind::Ack),
        WireBody::Retire => Some(ControlKind::Retire),
        WireBody::Command(_) => Some(ControlKind::Command),
    }
}

/// Heap bytes a decoded message retains while queued — the dominant, variable cost the byte
/// budget must bound. A FIELD owns its `f64` data array (8 bytes/cell) plus an optional
/// one-byte-per-cell mask; a COMMAND owns its argument bytes; ACK/RETIRE are unit-sized. The
/// fixed per-message overhead (header, enum tag, deque slot) is small and already bounded by
/// the count caps, so it is intentionally excluded from this variable-payload accounting.
fn field_payload_bytes(body: &WireBody) -> usize {
    match body {
        WireBody::Field(f) => {
            f.data.len() * std::mem::size_of::<f64>() + f.mask.as_ref().map_or(0, |m| m.len())
        }
        WireBody::Command(c) => c.arg.len(),
        WireBody::Ack(_) | WireBody::Retire => 0,
    }
}

/// An order-preserving inbound queue over two lanes that share one FIFO (so cross-kind order is
/// preserved) but **not** one budget. A FIELD run from one source coalesces to its newest member
/// and is bounded by an expendable field cap (over-cap snapshots are dropped and counted). Control
/// (ACK/RETIRE/COMMAND) is never coalesced, never reordered, and bounded by a reserved control
/// lane; exhausting that lane latches a terminal [`ControlOverflow`] rather than dropping a
/// verdict. See the module docs for why this is lawful, deadlock-safe, and honest under flood.
pub struct CoalescingBuffer {
    queue: VecDeque<WireMessage>,
    field_cap: usize,
    control_cap: usize,
    field_byte_cap: usize,
    field_count: usize,
    control_count: usize,
    field_bytes: usize,
    bytes_high_water: usize,
    coalesced: u64,
    fields_dropped: u64,
    out_of_contract: u64,
    overflow: Option<ControlOverflow>,
}

impl Default for CoalescingBuffer {
    fn default() -> Self {
        Self::with_caps(DEFAULT_FIELD_CAP, DEFAULT_CONTROL_CAP)
    }
}

impl CoalescingBuffer {
    /// A buffer with a reserved control lane of `control_cap` and the default field cap. Kept for
    /// callers that only tune the control budget; equivalent to
    /// `with_caps(DEFAULT_FIELD_CAP, control_cap)`.
    pub fn with_control_cap(control_cap: usize) -> Self {
        Self::with_caps(DEFAULT_FIELD_CAP, control_cap)
    }

    /// A buffer with explicit count lane budgets and the default FIELD byte budget
    /// ([`DEFAULT_FIELD_BYTE_CAP`]). Equivalent to
    /// `with_byte_budget(field_cap, control_cap, DEFAULT_FIELD_BYTE_CAP)`.
    pub fn with_caps(field_cap: usize, control_cap: usize) -> Self {
        Self::with_byte_budget(field_cap, control_cap, DEFAULT_FIELD_BYTE_CAP)
    }

    /// A buffer with explicit lane budgets: at most `field_cap` un-coalesced FIELD entries, at
    /// most `field_byte_cap` bytes of retained FIELD payload, and a reserved control lane of
    /// `control_cap`. Each is floored at 1. The byte budget bounds memory independently of the
    /// count: at saturation the oldest expendable snapshots are evicted to admit the freshest
    /// (latest-wins), and a control verdict is never evicted.
    pub fn with_byte_budget(field_cap: usize, control_cap: usize, field_byte_cap: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            field_cap: field_cap.max(1),
            control_cap: control_cap.max(1),
            field_byte_cap: field_byte_cap.max(1),
            field_count: 0,
            control_count: 0,
            field_bytes: 0,
            bytes_high_water: 0,
            coalesced: 0,
            fields_dropped: 0,
            out_of_contract: 0,
            overflow: None,
        }
    }

    /// Offer one decoded message.
    ///
    /// * FIELD — coalesces onto a same-source FIELD at the queue back (keeping the newer
    ///   `(epoch, sequence)` cursor); otherwise admitted until the field lane is full, then
    ///   dropped and counted ([`fields_dropped`](CoalescingBuffer::fields_dropped)). A field never
    ///   consumes control-lane headroom, so a field flood can never cost a verdict.
    /// * CONTROL (ACK/RETIRE/COMMAND) — never coalesced; admitted within the reserved control
    ///   lane. Past it, the buffer latches a terminal [`ControlOverflow`] and admits nothing
    ///   further — it never silently drops a verdict and never blocks the reader.
    pub fn offer(&mut self, msg: WireMessage) {
        // Once the control lane has failed closed the stream is terminal: admit nothing more, so
        // no post-overflow message can masquerade as part of a healthy stream.
        if self.overflow.is_some() {
            return;
        }
        match control_kind(&msg.body) {
            None => {
                if let Some(back) = self.queue.back_mut() {
                    if back.source == msg.source && matches!(back.body, WireBody::Field(_)) {
                        // Consecutive same-source snapshots: keep the newest, drop the stale one,
                        // reclaiming the stale payload's bytes and charging the newcomer's.
                        if msg.cursor >= back.cursor {
                            let new = field_payload_bytes(&msg.body);
                            if new > self.field_byte_cap {
                                // The fresher snapshot alone exceeds the ENTIRE field byte envelope.
                                // No eviction of older expendable snapshots can make room (emptied
                                // to just this one it still breaches), so retaining it would break
                                // the hard byte budget. Mirror the append path's oversized-field
                                // refusal below: keep the prior valid (staler) snapshot and count
                                // the drop, rather than silently breach the cap. FIELD snapshots are
                                // expendable; a staler valid observation is the honest bounded choice.
                                self.fields_dropped += 1;
                                return;
                            }
                            let old = field_payload_bytes(&back.body);
                            *back = msg;
                            self.field_bytes = self.field_bytes.saturating_sub(old) + new;
                            // A larger (within-cap) replacement may still exceed the budget together
                            // with older snapshots: evict older expendable snapshots (never the
                            // just-coalesced newest) to restore it.
                            self.enforce_field_byte_cap();
                            self.note_high_water();
                        }
                        self.coalesced += 1;
                        return;
                    }
                }
                let incoming = field_payload_bytes(&msg.body);
                if incoming > self.field_byte_cap {
                    // Larger than the ENTIRE envelope — it cannot be retained without breaking the
                    // budget, so refuse it (counted) without disturbing the snapshots already held.
                    // (The default envelope exceeds one maximum legal field, so this only bites a
                    // pathologically small configured cap.)
                    self.fields_dropped += 1;
                    return;
                }
                // Make room for the freshest observation by evicting the OLDEST expendable
                // snapshots under count OR byte pressure — at saturation we drop a stale snapshot,
                // never refuse the newest one (latest-wins), and never touch a control verdict.
                while (self.field_count >= self.field_cap
                    || self.field_bytes + incoming > self.field_byte_cap)
                    && self.evict_oldest_field().is_some()
                {}
                // field_cap >= 1 and incoming <= field_byte_cap, so room now exists.
                self.field_count += 1;
                self.field_bytes += incoming;
                self.queue.push_back(msg);
                self.note_high_water();
            }
            Some(ControlKind::Command) => {
                // COMMAND is presenter -> producer only (docs/WIRE_PROTOCOL.md §COMMAND payload).
                // An inbound COMMAND on the presenter's drain path is a DIRECTION violation from a
                // diseased producer: reject it explicitly and count it, rather than admit it to the
                // ACK/RETIRE verdict lane. Admitting it would (a) charge a verdict slot to a message
                // that is not a verdict, and (b) leak unbounded `arg` bytes, since the control lane
                // is count-bounded, not byte-bounded. The presenter's loop ignores it anyway
                // (main.rs: `WireBody::Command(_) => {}`), so admitting it is exactly the "buffer
                // then silently ignore" anti-pattern. With COMMAND rejected here, the only control
                // bodies ever admitted are ACK and RETIRE — both ZERO-payload — so the inbound
                // control lane is byte-bounded by construction (count_cap x 0 variable bytes).
                self.out_of_contract += 1;
            }
            Some(kind) => {
                if self.control_count >= self.control_cap {
                    // Non-expendable: fail closed rather than discard an authoritative verdict.
                    self.overflow = Some(ControlOverflow {
                        control_cap: self.control_cap,
                        kind,
                    });
                    return;
                }
                self.control_count += 1;
                self.queue.push_back(msg);
            }
        }
    }

    /// Hand every queued message to `sink` in order, emptying the queue. A latched
    /// [`ControlOverflow`] is **sticky** — draining does not clear it, so a consumer that read a
    /// terminal control overflow cannot resume treating the stream as healthy.
    pub fn drain_into(&mut self, sink: &mut impl FnMut(WireMessage)) {
        for msg in self.queue.drain(..) {
            sink(msg);
        }
        self.field_count = 0;
        self.control_count = 0;
        self.field_bytes = 0;
    }

    /// Evict the oldest queued FIELD entry — the front-most FIELD, skipping any control so no
    /// verdict is reordered or lost — counting it as an expendable drop and reclaiming its
    /// bytes. Returns the reclaimed bytes, or `None` if the queue holds no FIELD to evict.
    fn evict_oldest_field(&mut self) -> Option<usize> {
        let idx = self
            .queue
            .iter()
            .position(|m| matches!(m.body, WireBody::Field(_)))?;
        let msg = self.queue.remove(idx).expect("index just located");
        let bytes = field_payload_bytes(&msg.body);
        self.field_count -= 1;
        self.field_bytes = self.field_bytes.saturating_sub(bytes);
        self.fields_dropped += 1;
        Some(bytes)
    }

    /// Evict oldest expendable snapshots until retained FIELD bytes are within the byte budget,
    /// but never evict the sole remaining snapshot: a field larger than the whole envelope is
    /// retained rather than lost (the documented soft-cap exception, unreachable at the default
    /// cap since it exceeds one maximum legal field).
    fn enforce_field_byte_cap(&mut self) {
        while self.field_bytes > self.field_byte_cap
            && self.field_count > 1
            && self.evict_oldest_field().is_some()
        {}
    }

    /// Record a new retained-bytes high-water mark.
    fn note_high_water(&mut self) {
        if self.field_bytes > self.bytes_high_water {
            self.bytes_high_water = self.field_bytes;
        }
    }

    /// Messages currently queued (fields + control).
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Un-coalesced FIELD entries currently queued.
    pub fn field_len(&self) -> usize {
        self.field_count
    }

    /// Control messages currently queued.
    pub fn control_len(&self) -> usize {
        self.control_count
    }

    /// Retained FIELD payload bytes currently queued (f64 data + optional mask). This is the
    /// dominant, variable memory the buffer holds; it is bounded by [`field_byte_cap`] regardless
    /// of how many distinct sources are queued.
    pub fn field_bytes(&self) -> usize {
        self.field_bytes
    }

    /// The configured retained-FIELD byte budget (bytes).
    pub fn field_byte_cap(&self) -> usize {
        self.field_byte_cap
    }

    /// High-water mark of retained FIELD payload bytes over this buffer's life.
    pub fn bytes_high_water(&self) -> usize {
        self.bytes_high_water
    }

    /// Total FIELD frames collapsed by coalescing over this buffer's life (lawful snapshot drops).
    pub fn coalesced(&self) -> u64 {
        self.coalesced
    }

    /// Total **expendable** FIELD snapshots dropped at the field cap (0 against a conformant
    /// producer; nonzero discloses a flood rather than hiding it — L7). This is never used for
    /// control: a lost verdict is a [`ControlOverflow`], not a counter.
    pub fn fields_dropped(&self) -> u64 {
        self.fields_dropped
    }

    /// Total messages rejected as OUT OF CONTRACT for this (producer -> presenter) direction — an
    /// inbound COMMAND, which the protocol only defines presenter -> producer. Nonzero discloses a
    /// diseased/misdirected producer; such messages are never admitted, so they consume neither a
    /// verdict slot nor any retained bytes.
    pub fn out_of_contract(&self) -> u64 {
        self.out_of_contract
    }

    /// The terminal control-lane overflow, if one has been latched. `Some` means the stream has
    /// failed closed and command fate is unresolved; the consumer must stop.
    pub fn control_overflow(&self) -> Option<ControlOverflow> {
        self.overflow
    }
}

/// State shared between the reader thread and the UI thread.
struct Shared {
    buf: CoalescingBuffer,
    /// The read loop ended: either a clean `Ok(0)` EOF (graceful disconnect) **or** a read fault.
    /// Either way no further frames arrive; `read_error` disambiguates the two (Gate C4).
    eof: bool,
    /// `Some(kind)` iff the producer's stdout read itself FAULTED (an `Err` from `read`), as
    /// opposed to a clean `Ok(0)` end-of-stream. The I/O [`ErrorKind`] is retained so a severed
    /// pipe or reset is distinguishable from the producer closing cleanly — the old code collapsed
    /// both to `eof = true`, destroying the provenance a disconnect handler needs (Gate C4).
    read_error: Option<ErrorKind>,
    /// A fatal decode poisoned the stream; surfaced to the UI verbatim.
    fatal: Option<WireError>,
    /// Running count of recoverable frames the decoder skipped and resynchronized past — the
    /// observable receipt for a malformed-but-nonfatal frame (Gate C4). Monotonic.
    decode_skips: u64,
}

/// Outcome of a UI-side drain: whether the producer has disconnected and any fatal decode.
#[derive(Debug, Clone, Copy, Default)]
pub struct DrainOutcome {
    /// The producer closed its stream (or the read faulted): no more frames will arrive. See
    /// [`read_error`](DrainOutcome::read_error) to tell a clean close from a fault.
    pub disconnected: bool,
    /// `Some(kind)` iff the stream ended because the stdout read FAULTED, not because the producer
    /// closed cleanly. Present only alongside `disconnected`; a graceful EOF leaves it `None`. This
    /// is the provenance Gate C4 requires — a disconnect handler can report "producer stream
    /// faulted (<kind>)" distinctly from an orderly shutdown instead of guessing.
    pub read_error: Option<ErrorKind>,
    /// A fatal decode error, if the stream poisoned.
    pub fatal: Option<WireError>,
    /// The control lane failed closed (see [`ControlOverflow`]). Treat it like a fatal transport
    /// error: an authoritative verdict was lost, outstanding command fate is unresolved, and the
    /// consumer must stop rather than keep claiming command-lifecycle correctness.
    pub control_overflow: Option<ControlOverflow>,
    /// Running count of recoverable frames the decoder has skipped and resynchronized past over
    /// the reader's life (Gate C4). Nonzero means the producer emitted malformed-but-nonfatal
    /// frames; a healthy stream holds it at 0. A countable receipt, never a silent loss.
    pub decode_skips: u64,
}

/// A background reader that decodes the producer's stdout into a bounded, coalescing buffer and
/// never blocks waiting on the UI — so the producer's stdout is always drained and a command
/// write can never deadlock against a stdout-blocked producer (Gate B, 2C).
pub struct Reader {
    shared: Arc<Mutex<Shared>>,
    handle: Option<JoinHandle<()>>,
}

impl Reader {
    /// Spawn a reader over `stdout`, retaining up to `control_cap` un-collapsible messages.
    pub fn spawn(mut stdout: impl Read + Send + 'static, control_cap: usize) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            buf: CoalescingBuffer::with_control_cap(control_cap),
            eof: false,
            read_error: None,
            fatal: None,
            decode_skips: 0,
        }));
        let worker = Arc::clone(&shared);
        let handle = thread::spawn(move || {
            let mut pump = FramePump::new();
            let mut chunk = [0u8; READ_CHUNK];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) => {
                        // Clean end-of-stream: the producer closed its stdout. `read_error` stays
                        // `None`, marking this a graceful disconnect rather than a fault.
                        worker.lock().unwrap().eof = true;
                        return;
                    }
                    Ok(n) => {
                        // Decode this chunk's frames into a local batch, then publish under one
                        // lock. The pump interleaves parse with accumulation (2B).
                        let mut batch: Vec<WireMessage> = Vec::new();
                        let res = pump.pump(&chunk[..n], &mut |m| batch.push(m));
                        let mut guard = worker.lock().unwrap();
                        for m in batch {
                            guard.buf.offer(m);
                        }
                        // Publish the decoder's recoverable-skip receipt every cycle so a malformed
                        // frame the pump swallowed is observable, not lost (Gate C4).
                        guard.decode_skips = pump.frames_skipped();
                        if let Err(e) = res {
                            guard.fatal = Some(e);
                            return;
                        }
                    }
                    Err(e) => {
                        // The read FAULTED (broken pipe, reset, interrupted I/O, …). This is NOT a
                        // clean EOF: retain the error kind so the disconnect is reported as a fault
                        // with provenance, not mistaken for an orderly shutdown (Gate C4). `eof` is
                        // still set — no more frames arrive either way — and `read_error` is what
                        // disambiguates the two.
                        let mut guard = worker.lock().unwrap();
                        guard.read_error = Some(e.kind());
                        guard.eof = true;
                        return;
                    }
                }
            }
        });
        Self {
            shared,
            handle: Some(handle),
        }
    }

    /// Drain every currently-buffered message in order into `sink`, reporting disconnect/fatal.
    ///
    /// The sink is invoked **after** the lock is released (Gate C2): a sink is caller code — it may
    /// be slow, or reentrant (calling back into a [`Reader`] accessor). Holding the mutex across it
    /// would let a slow sink block the reader thread, which needs this very lock to publish, so the
    /// producer's stdout would stop draining and the full-duplex deadlock this module exists to
    /// prevent would return; a reentrant sink would deadlock outright on the non-reentrant mutex.
    /// The already-bounded queue is moved into a local batch under the lock, preserving ordering
    /// and the retention bound, and the sink runs lock-free.
    pub fn drain(&self, sink: &mut impl FnMut(WireMessage)) -> DrainOutcome {
        let (batch, outcome) = {
            let mut guard = self.shared.lock().unwrap();
            let mut batch: Vec<WireMessage> = Vec::new();
            guard.buf.drain_into(&mut |m| batch.push(m));
            let outcome = DrainOutcome {
                disconnected: guard.eof,
                read_error: guard.read_error,
                fatal: guard.fatal,
                control_overflow: guard.buf.control_overflow(),
                decode_skips: guard.decode_skips,
            };
            (batch, outcome)
        };
        for msg in batch {
            sink(msg);
        }
        outcome
    }

    /// Messages currently buffered (test/telemetry; proves the bound holds without a UI drain).
    pub fn buffered_len(&self) -> usize {
        self.shared.lock().unwrap().buf.len()
    }

    /// FIELD frames the reader has coalesced away (lawful snapshot drops).
    pub fn coalesced(&self) -> u64 {
        self.shared.lock().unwrap().buf.coalesced()
    }

    /// Expendable FIELD snapshots dropped at the field cap (0 against a conformant producer;
    /// disclosed, L7). Control is never counted here — a lost verdict is a terminal overflow.
    pub fn fields_dropped(&self) -> u64 {
        self.shared.lock().unwrap().buf.fields_dropped()
    }

    /// Retained FIELD payload bytes currently queued — the dominant, variable memory the reader
    /// holds, bounded by the byte budget independently of the message count (Gate C1).
    pub fn field_bytes(&self) -> usize {
        self.shared.lock().unwrap().buf.field_bytes()
    }

    /// High-water mark of retained FIELD payload bytes over the reader's life (telemetry).
    pub fn bytes_high_water(&self) -> usize {
        self.shared.lock().unwrap().buf.bytes_high_water()
    }

    /// The configured retained-FIELD byte budget (bytes).
    pub fn field_byte_cap(&self) -> usize {
        self.shared.lock().unwrap().buf.field_byte_cap()
    }

    /// The terminal control-lane overflow, if one has been latched (the stream failed closed).
    pub fn control_overflow(&self) -> Option<ControlOverflow> {
        self.shared.lock().unwrap().buf.control_overflow()
    }

    /// `Some(kind)` iff the reader's stdout read FAULTED (vs a clean EOF). Distinguishes a severed
    /// pipe from a graceful producer shutdown — both set [`disconnected`](DrainOutcome), only a
    /// fault sets this (Gate C4).
    pub fn read_error(&self) -> Option<ErrorKind> {
        self.shared.lock().unwrap().read_error
    }

    /// Recoverable frames the decoder has skipped and resynchronized past (the observable receipt
    /// for malformed-but-nonfatal frames; 0 on a healthy stream — Gate C4).
    pub fn decode_skips(&self) -> u64 {
        self.shared.lock().unwrap().decode_skips
    }

    /// Wrong-direction control frames rejected as out of contract (Gate B2). COMMAND is
    /// presenter → producer only (`docs/WIRE_PROTOCOL.md`), so an inbound COMMAND on the presenter's
    /// read path is a direction violation: it is never admitted to the control lane and never
    /// resolves or creates an edit, and its payload is dropped (not retained). This surfaces the
    /// `CoalescingBuffer` counter through the production reader so a hostile producer's violation is
    /// visible to the operator instead of being counted invisibly. 0 on a conformant producer.
    pub fn out_of_contract(&self) -> u64 {
        self.shared.lock().unwrap().buf.out_of_contract()
    }

    /// Join the reader thread. The producer's stdout closing (e.g. after `child.kill()`) ends the
    /// read loop, so this returns promptly.
    pub fn join(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// A supervised owner for the producer subprocess and everything whose lifetime is tied to it:
/// the [`Child`], its stdin, and the background [`Reader`] thread (Gate E).
///
/// # The lifecycle hole it closes
///
/// `run_interactive` spawns the producer and the reader thread, *then* builds the terminal
/// [`Context`](gibson::context::Context) and enters a render loop — both fallible. Before this
/// owner, the only teardown (EOF stdin → `kill` → `wait` → reader `join` → terminal restore) sat
/// at the *bottom* of the function, reachable only on the normal `break` paths. A `?` error from
/// `Context::fullscreen()` or any in-loop `ctx.run_once()` returned early, **orphaning the Python
/// child, detaching the reader thread** (neither [`Reader`] nor `Context` restores on `Drop`), and
/// potentially leaving the terminal raw.
///
/// This owner moves child-ownership into a value whose [`Drop`] runs on **every** exit — normal
/// return, a propagated `?` error, or a panic unwind — so the child is always reaped through the
/// OS `wait` handle (not inferred from an escape-stream heuristic) and the reader is always joined.
/// Terminal restoration is handled separately by the caller, since the `Context` must outlive a
/// failed terminal init (there is nothing to restore if `fullscreen()` never succeeded).
///
/// Reaping order: drop stdin first (EOF — lets a healthy producer exit gracefully), then `kill`
/// as a hard backstop (idempotent if the child already exited), then `wait` to reap the process
/// (preventing a zombie) and close its stdout, which ends the reader's read loop so `join` returns
/// promptly.
pub struct ProducerOwner {
    child: Child,
    stdin: Option<ChildStdin>,
    reader: Reader,
}

impl ProducerOwner {
    /// Take ownership of the spawned `child`, its `stdin` (if piped), and the `reader` thread.
    pub fn new(child: Child, stdin: Option<ChildStdin>, reader: Reader) -> Self {
        Self {
            child,
            stdin,
            reader,
        }
    }

    /// The outbound command sink — the producer's stdin — while it is open.
    pub fn stdin_mut(&mut self) -> Option<&mut ChildStdin> {
        self.stdin.as_mut()
    }

    /// The inbound reader, for draining decoded messages (`&self` — drain does not consume it).
    pub fn reader(&self) -> &Reader {
        &self.reader
    }

    /// The OS process id of the owned child — the handle a test measures reaping against (process
    /// death observed through the OS, never an escape-stream assertion).
    pub fn child_id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for ProducerOwner {
    fn drop(&mut self) {
        // EOF to the producer's stdin for a graceful exit…
        drop(self.stdin.take());
        // …with a hard backstop so no child outlives us (idempotent if it already exited).
        let _ = self.child.kill();
        // Reap through the OS wait handle — prevents a zombie and closes the child's stdout…
        let _ = self.child.wait();
        // …so the reader's read loop hits EOF and this join returns promptly, never detached.
        self.reader.join();
    }
}
