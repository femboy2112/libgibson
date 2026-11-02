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
//! # 2B — decode is interleaved with accumulation, by construction
//!
//! [`FrameDecoder`] already self-bounds its accumulation buffer to one maximum frame and returns
//! a **fatal** `BufferOverflow` past it. The original drain loop pushed *every* queued chunk into
//! the decoder before draining a single message, so on fast legal input a backlog of chunks could
//! sum past that one-frame ceiling and trip a spurious fatal overflow on perfectly good data.
//! [`FramePump::pump`] pushes exactly one chunk and then drains every frame it completes, so the
//! decoder never holds more than one chunk plus one partial frame — the overflow can only fire on
//! a genuinely oversized declared frame, which is what it is for.
//!
//! # What is never dropped (L5)
//!
//! Only consecutive same-source FIELD frames are ever collapsed, and only to their newest member;
//! a snapshot is coalesceable by design. Control messages (ACK / RETIRE / COMMAND) are never
//! coalesced and never reordered across one another or across a field, so an edit's acknowledged
//! fate is never silently lost. The control queue is bounded by a generous hard cap; a producer
//! flooding past it has its overflow **counted** ([`CoalescingBuffer::dropped`]) rather than
//! hidden — truthful telemetry, L7 — and a well-behaved producer's acks (one per outstanding
//! edit, itself capped by the [`CommandChannel`](crate::channel::CommandChannel)) never approach
//! the cap.

use std::collections::VecDeque;
use std::io::Read;
use std::process::{Child, ChildStdin};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use gibson::wire::{FrameDecoder, WireBody, WireError, WireMessage};

/// The reader's stdout read granularity. Real reads are far under the decoder's one-frame
/// ceiling, so a single [`FramePump::pump`] of one chunk never trips `BufferOverflow`.
const READ_CHUNK: usize = 65536;

/// Default hard cap on retained **non-coalesceable** messages (ACK/RETIRE/COMMAND) plus any
/// uncoalesced fields. Generous: a conformant producer's acks are one-per-outstanding-edit and
/// bounded well under this; only a misbehaving flood reaches it, and then the overflow is counted.
pub const DEFAULT_CONTROL_CAP: usize = 4096;

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

    /// Push exactly one `chunk`, then drain every complete frame it finishes, handing each decoded
    /// message to `sink` in stream order.
    ///
    /// Returns `Err` only on a **fatal** decoder condition (bad magic, unsupported version, a
    /// declared frame larger than the cap, or a genuine buffer overflow); the decoder is poisoned
    /// and the caller should treat the stream as dead. A *recoverable* per-frame decode error is
    /// swallowed here — the decoder skips exactly that frame and resynchronizes — because a single
    /// malformed frame must not tear down a live instrument.
    pub fn pump(
        &mut self,
        chunk: &[u8],
        sink: &mut impl FnMut(WireMessage),
    ) -> Result<(), WireError> {
        // `push` fails only when poisoned or when the (one chunk + partial) buffer would exceed
        // one maximum frame — both fatal. One real read is orders of magnitude under that ceiling.
        self.dec.push(chunk)?;
        loop {
            match self.dec.next_message() {
                None => return Ok(()),
                Some(Ok(msg)) => sink(msg),
                Some(Err(e)) if e.is_fatal() => return Err(e),
                Some(Err(_)) => { /* recoverable skip; the decoder resynchronized */ }
            }
        }
    }

    /// Bytes currently held in the decoder's accumulation buffer (one partial frame at most, after
    /// a `pump` returns).
    pub fn buffered_bytes(&self) -> usize {
        self.dec.buffered_bytes()
    }
}

/// An order-preserving inbound queue that bounds memory by **coalescing** a run of consecutive
/// same-source FIELD frames down to its newest member, while never coalescing or reordering a
/// control message (ACK/RETIRE/COMMAND). See the module docs for why this is both lawful and
/// deadlock-safe.
pub struct CoalescingBuffer {
    queue: VecDeque<WireMessage>,
    hard_cap: usize,
    coalesced: u64,
    dropped: u64,
}

impl Default for CoalescingBuffer {
    fn default() -> Self {
        Self::with_control_cap(DEFAULT_CONTROL_CAP)
    }
}

impl CoalescingBuffer {
    /// A buffer that retains at most `hard_cap` queued messages before counting overflow drops.
    pub fn with_control_cap(hard_cap: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            hard_cap: hard_cap.max(1),
            coalesced: 0,
            dropped: 0,
        }
    }

    /// Offer one decoded message. A FIELD whose immediate predecessor in the queue is a FIELD from
    /// the same source collapses onto it (keeping the newer `(epoch, sequence)` cursor); anything
    /// else is appended in order. At the hard cap an un-collapsible message is dropped and counted
    /// rather than blocking the reader — the property that keeps the producer's stdout draining.
    pub fn offer(&mut self, msg: WireMessage) {
        if matches!(msg.body, WireBody::Field(_)) {
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
        }
        if self.queue.len() >= self.hard_cap {
            self.dropped += 1;
            return;
        }
        self.queue.push_back(msg);
    }

    /// Hand every queued message to `sink` in order, emptying the buffer.
    pub fn drain_into(&mut self, sink: &mut impl FnMut(WireMessage)) {
        for msg in self.queue.drain(..) {
            sink(msg);
        }
    }

    /// Messages currently queued.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Total FIELD frames collapsed by coalescing over this buffer's life (lawful snapshot drops).
    pub fn coalesced(&self) -> u64 {
        self.coalesced
    }

    /// Total messages dropped at the hard cap (should be 0 against a conformant producer; nonzero
    /// discloses a flood rather than hiding it — L7).
    pub fn dropped(&self) -> u64 {
        self.dropped
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
    pub fn drain(&self, sink: &mut impl FnMut(WireMessage)) -> DrainOutcome {
        let mut guard = self.shared.lock().unwrap();
        guard.buf.drain_into(sink);
        DrainOutcome {
            disconnected: guard.eof,
            fatal: guard.fatal,
        }
    }

    /// Messages currently buffered (test/telemetry; proves the bound holds without a UI drain).
    pub fn buffered_len(&self) -> usize {
        self.shared.lock().unwrap().buf.len()
    }

    /// FIELD frames the reader has coalesced away (lawful snapshot drops).
    pub fn coalesced(&self) -> u64 {
        self.shared.lock().unwrap().buf.coalesced()
    }

    /// Messages dropped at the control-queue cap (0 against a conformant producer; disclosed).
    pub fn dropped(&self) -> u64 {
        self.shared.lock().unwrap().buf.dropped()
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
