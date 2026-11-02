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
use std::io::Read;
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
    field_count: usize,
    control_count: usize,
    coalesced: u64,
    fields_dropped: u64,
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

    /// A buffer with explicit lane budgets: at most `field_cap` un-coalesced FIELD entries and a
    /// reserved control lane of `control_cap`. Each is floored at 1.
    pub fn with_caps(field_cap: usize, control_cap: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            field_cap: field_cap.max(1),
            control_cap: control_cap.max(1),
            field_count: 0,
            control_count: 0,
            coalesced: 0,
            fields_dropped: 0,
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
                        // Consecutive same-source snapshots: keep the newest, drop the stale one.
                        if msg.cursor >= back.cursor {
                            *back = msg;
                        }
                        self.coalesced += 1;
                        return;
                    }
                }
                if self.field_count >= self.field_cap {
                    // Expendable: a dropped snapshot is superseded by the next. Counted, not fatal.
                    self.fields_dropped += 1;
                    return;
                }
                self.field_count += 1;
                self.queue.push_back(msg);
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

    /// The terminal control-lane overflow, if one has been latched. `Some` means the stream has
    /// failed closed and command fate is unresolved; the consumer must stop.
    pub fn control_overflow(&self) -> Option<ControlOverflow> {
        self.overflow
    }
}

/// State shared between the reader thread and the UI thread.
struct Shared {
    buf: CoalescingBuffer,
    /// EOF reached on the producer's stdout (graceful disconnect).
    eof: bool,
    /// A fatal decode poisoned the stream; surfaced to the UI verbatim.
    fatal: Option<WireError>,
}

/// Outcome of a UI-side drain: whether the producer has disconnected and any fatal decode.
#[derive(Debug, Clone, Copy, Default)]
pub struct DrainOutcome {
    /// The producer closed its stream (or the read failed): no more frames will arrive.
    pub disconnected: bool,
    /// A fatal decode error, if the stream poisoned.
    pub fatal: Option<WireError>,
    /// The control lane failed closed (see [`ControlOverflow`]). Treat it like a fatal transport
    /// error: an authoritative verdict was lost, outstanding command fate is unresolved, and the
    /// consumer must stop rather than keep claiming command-lifecycle correctness.
    pub control_overflow: Option<ControlOverflow>,
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
            fatal: None,
        }));
        let worker = Arc::clone(&shared);
        let handle = thread::spawn(move || {
            let mut pump = FramePump::new();
            let mut chunk = [0u8; READ_CHUNK];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) => {
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
                        if let Err(e) = res {
                            guard.fatal = Some(e);
                            return;
                        }
                    }
                    Err(_) => {
                        worker.lock().unwrap().eof = true;
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
                fatal: guard.fatal,
                control_overflow: guard.buf.control_overflow(),
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

    /// The terminal control-lane overflow, if one has been latched (the stream failed closed).
    pub fn control_overflow(&self) -> Option<ControlOverflow> {
        self.shared.lock().unwrap().buf.control_overflow()
    }

    /// Join the reader thread. The producer's stdout closing (e.g. after `child.kill()`) ends the
    /// read loop, so this returns promptly.
    pub fn join(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
