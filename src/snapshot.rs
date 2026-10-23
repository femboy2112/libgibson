//! An immutable snapshot + command bridge between a producer and a presenter (issue #141).
//!
//! A consumer owns its authoritative model; the library owns the transport that carries
//! that model's *observations* out to a presenter and the *edits* back in. The two
//! directions have deliberately different disciplines (Law IV — state evolves by
//! commands, not by observation):
//!
//! * **Frames are replaceable observations.** A [`Frame`] is the
//!   latest picture of a source; a newer one (higher [`seq`](crate::frame::Frame::seq))
//!   supersedes and *coalesces away* the stale one, because a presenter only ever needs
//!   the latest. Dropping a superseded frame loses nothing a consumer could still act on.
//! * **Commands are ordered edits.** Edits are a FIFO queue that never reorders and never
//!   *silently* drops: an accepted edit is delivered exactly once, in the order issued, no
//!   matter how many frames were coalesced beside it. Losing or reordering an *accepted*
//!   edit would corrupt the consumer's authoritative state, so the bridge refuses to.
//!
//! The load-bearing guarantee this type exists to make true: **frame coalescing can never
//! perturb the command stream.** [`SnapshotBridge::commands_conserved`] is the receipt,
//! and [`SnapshotBridge::frames_superseded`] counts the lossy-by-design frame coalescing
//! with an honest number.
//!
//! # Why this type is *bounded* — the impossibility that forces the design
//!
//! A finite-capacity in-memory queue cannot simultaneously (a) accept an unbounded stream
//! of distinct commands, (b) preserve every one, (c) never block, and (d) never refuse —
//! not when the consumer may stall indefinitely. Those four are mutually exclusive; memory
//! is finite and a stalled consumer never drains. So we drop (d): the only *honest* design
//! is **typed admission / backpressure / refusal**. The bridge is built with an explicit
//! [`Capacity`]; at the ceiling it returns a *typed* [`Admission::Refused`] or
//! [`FramePublish::SourceCapReached`] instead of growing without bound or discarding in
//! silence. A diseased producer can be *told no*; it cannot make this map eat the box.
//!
//! Two concrete leaks this closes (both were unbounded before #141's hardening):
//!
//! * `latest` had no cap on distinct sources and no retirement — a producer minting fresh
//!   [`FrameSource`]s grew the map forever, each entry pinning a whole [`Frame`]'s raster
//!   bytes. Now [`Capacity::max_sources`] caps it and [`SnapshotBridge::retire_source`]
//!   frees them.
//! * `issue()` returned `()` — it could never refuse and was never bounded; a stalled
//!   consumer plus a fast producer was unbounded memory. Now it returns an [`Admission`]
//!   and is capped at [`Capacity::max_pending_commands`].
//!
//! # Command lifecycle — five states, one conservation law (Laws L4, L7)
//!
//! Every command passes through accountable states: **offered** (handed to
//! [`issue`](SnapshotBridge::issue)),
//! then either **accepted** (queued) or **refused** (typed backpressure, L4); an accepted
//! command is **pending** until **delivered** (drained). A minimal **acknowledged** notion
//! sits downstream of delivered, for the Python-producer contract that needs
//! "delivered vs acknowledged-or-explicitly-refused." The conservation law, asserted by
//! [`SnapshotBridge::commands_conserved`], is:
//!
//! ```text
//!   offered   == accepted  + refused     (nothing offered vanishes — no silent erasure, L7)
//!   accepted  == delivered + pending      (every accepted edit is delivered or still waiting)
//!   acked     <= delivered                (you cannot acknowledge what was never delivered)
//! ```
//!
//! # Boundary — what this does NOT promise
//!
//! This is an **in-memory** transport. It makes no durability or exactly-once
//! crash-recovery claim: a process death loses the queue and the frames. "Delivered" means
//! handed out of this bridge in-process, not persisted and not confirmed applied. A
//! *refused* command is not retained — refusal is explicit backpressure the producer must
//! handle by re-offering; it is counted, never silently swallowed.

use crate::frame::{Frame, FrameSource};
use crate::raster::{Rgb, MAX_RASTER_DIMENSION};
use std::collections::HashMap;
use std::collections::VecDeque;

/// The worst-case byte cost of a single frame's raster: a full
/// [`MAX_RASTER_DIMENSION`]-square truecolor buffer. The whole point of bounding the
/// source count is that retained bytes are then capped at `max_sources * MAX_FRAME_BYTES`
/// — a finite number you can put on a spec sheet.
pub const MAX_FRAME_BYTES: usize =
    (MAX_RASTER_DIMENSION as usize) * (MAX_RASTER_DIMENSION as usize) * std::mem::size_of::<Rgb>();

/// Explicit ceilings for a [`SnapshotBridge`]. There is no "unbounded" — that was the
/// disease. You state the budget up front; the bridge enforces it with typed refusals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capacity {
    /// Maximum number of distinct sources with a live latest frame. A new source beyond
    /// this is refused ([`FramePublish::SourceCapReached`]); no live source is evicted.
    pub max_sources: usize,
    /// Maximum number of commands pending (accepted but not yet delivered). An `issue`
    /// beyond this is refused ([`Admission::Refused`]); the accepted prefix is untouched.
    pub max_pending_commands: usize,
}

impl Capacity {
    /// Default source ceiling — enough distinct producers for a real panel, small enough
    /// that `max_sources * MAX_FRAME_BYTES` stays a sane worst case.
    pub const DEFAULT_MAX_SOURCES: usize = 64;
    /// Default pending-command ceiling — a deep-enough edit buffer that a healthy consumer
    /// never sees backpressure, shallow enough that a stalled one is told no quickly.
    pub const DEFAULT_MAX_PENDING_COMMANDS: usize = 1024;

    /// A capacity with the given ceilings. Both are hard caps, enforced by refusal.
    pub fn new(max_sources: usize, max_pending_commands: usize) -> Self {
        Self {
            max_sources,
            max_pending_commands,
        }
    }
}

impl Default for Capacity {
    fn default() -> Self {
        Self::new(
            Self::DEFAULT_MAX_SOURCES,
            Self::DEFAULT_MAX_PENDING_COMMANDS,
        )
    }
}

/// Why a reason an [`issue`](SnapshotBridge::issue) could not be accepted. Typed, so the
/// producer branches on the diagnosis instead of guessing at a swallowed command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefuseReason {
    /// The pending-command queue is at [`Capacity::max_pending_commands`]. The consumer is
    /// behind; drain before offering more. The refused command was not retained.
    CommandQueueFull { capacity: usize },
}

/// The outcome of offering a command to the bridge. Never a silent drop (Law L7): a
/// refusal is a *counted*, typed event the producer must handle as backpressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Queued as an ordered edit; it will be delivered exactly once, in issue order.
    Accepted,
    /// Declined — the command was not queued and not retained. Re-offer after draining.
    Refused(RefuseReason),
}

impl Admission {
    /// `true` if the command was queued.
    pub fn is_accepted(&self) -> bool {
        matches!(self, Admission::Accepted)
    }
}

/// The outcome of publishing a frame. Distinguishes the three fates a frame can meet so
/// none of them is a silent mutation of the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePublish {
    /// Stored as the new latest for its source (a first frame, a newer-seq replacement, or
    /// the first frame after a [`restart_source`](SnapshotBridge::restart_source)).
    Stored,
    /// Coalesced away: the incoming frame was not newer than the stored one, so it is the
    /// stale party and is dropped. Lossy *by design*, counted by `frames_superseded`.
    Superseded,
    /// Refused: this is a *new* source and `latest` is already at
    /// [`Capacity::max_sources`]. No live source was evicted — admission, not eviction.
    SourceCapReached { capacity: usize },
}

impl FramePublish {
    /// `true` if this publish became the stored latest.
    pub fn is_stored(&self) -> bool {
        matches!(self, FramePublish::Stored)
    }
}

/// Receipt for [`SnapshotBridge::retire_source`] — a declared, non-silent reclamation.
/// The byte count is the raster that was pinned by the retired frame and is now freed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetirementReceipt {
    /// The source that was retired.
    pub source: FrameSource,
    /// Raster bytes released (`0` if the source held no live frame).
    pub freed_bytes: usize,
    /// Whether a live frame was actually present and removed.
    pub had_frame: bool,
}

/// A stored latest frame plus the one bit of bookkeeping the bare [`Frame`] can't carry:
/// whether a producer restart has armed it to be superseded regardless of seq. Kept
/// private to the module — the epoch is the bridge's concern, not the frame's.
#[derive(Debug, Clone)]
struct StoredFrame {
    frame: Frame,
    // A restarted producer resets its seq to low numbers; strict latest-wins would wrongly
    // reject that fresh frame as stale. This flag, set by `restart_source`, tells the next
    // publish to accept unconditionally and then disarms — the new low seq becomes the new
    // baseline. Keeping the frame visible meanwhile means a restart never blanks the panel.
    accept_next_regardless: bool,
}

/// Exact raster byte cost of a frame: the size of its `[Rgb]` pixel buffer, no
/// hidden copies. `size_of_val` over the slice is `len * size_of::<Rgb>()` — same
/// number, stated the way clippy insists (`manual_slice_size_calculation`).
fn frame_bytes(frame: &Frame) -> usize {
    std::mem::size_of_val(frame.raster().pixels())
}

/// A two-channel bridge: latest-wins [`Frame`]s per source, and an ordered,
/// typed-admission queue of consumer-defined commands `C`.
///
/// The library is generic over `C`: it owns the ordering, bounding, and delivery
/// discipline; the consumer owns what a command *means*. Every ceiling is explicit
/// ([`Capacity`]); every over-ceiling event is a typed refusal, never silent growth.
#[derive(Debug, Clone)]
pub struct SnapshotBridge<C> {
    latest: HashMap<FrameSource, StoredFrame>,
    commands: VecDeque<C>,
    capacity: Capacity,
    frames_superseded: u64,
    // Full command lifecycle: offered == accepted + refused; accepted == delivered +
    // pending; acked <= delivered. Five states, one conservation law.
    commands_offered: u64,
    commands_accepted: u64,
    commands_refused: u64,
    commands_delivered: u64,
    commands_acked: u64,
}

impl<C> Default for SnapshotBridge<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> SnapshotBridge<C> {
    /// An empty bridge with [`Capacity::default`] ceilings. Sane documented defaults for a
    /// typical panel; reach for [`with_capacity`](SnapshotBridge::with_capacity) when you
    /// want to state the budget yourself.
    pub fn new() -> Self {
        Self::with_capacity(Capacity::default())
    }

    /// An empty bridge with the given explicit [`Capacity`]. This is the honest
    /// constructor: you name the ceilings, the bridge enforces them with typed refusals.
    pub fn with_capacity(capacity: Capacity) -> Self {
        Self {
            latest: HashMap::new(),
            commands: VecDeque::new(),
            capacity,
            frames_superseded: 0,
            commands_offered: 0,
            commands_accepted: 0,
            commands_refused: 0,
            commands_delivered: 0,
            commands_acked: 0,
        }
    }

    /// The ceilings this bridge was built with.
    pub fn capacity(&self) -> Capacity {
        self.capacity
    }

    /// Publishes a frame as the latest observation of its source.
    ///
    /// Latest-wins by [`seq`](crate::frame::Frame::seq): a frame with a `seq` strictly
    /// greater than the stored one replaces it ([`FramePublish::Stored`], the displaced
    /// frame *superseded*); a frame whose `seq` is not newer is itself stale and dropped
    /// ([`FramePublish::Superseded`]). Either way exactly one frame per source survives and
    /// every coalesced frame is counted by
    /// [`frames_superseded`](SnapshotBridge::frames_superseded).
    ///
    /// Two bounded departures from bare latest-wins:
    /// * A **new** source when `latest` is already at [`Capacity::max_sources`] is refused
    ///   ([`FramePublish::SourceCapReached`]); no live source is evicted. Coalescing of an
    ///   *existing* source is never affected — Law IV holds, frame churn never touches the
    ///   command stream.
    /// * A source armed by [`restart_source`](SnapshotBridge::restart_source) accepts its
    ///   next frame unconditionally (seq reset tolerated), then disarms.
    pub fn publish_frame(&mut self, frame: Frame) -> FramePublish {
        let source = frame.source();
        // Existing source first. We return an owned `FramePublish` from every arm, so the
        // `get_mut` borrow does not escape this block — which is what lets us re-borrow
        // `self.latest` for the new-source insert below (sidestepping the NLL
        // conditional-control-flow limitation; no `insert` inside a `None` arm).
        if let Some(stored) = self.latest.get_mut(&source) {
            if stored.accept_next_regardless {
                // Producer restart: accept regardless of seq, disarm, count the displaced
                // frame as superseded. The reset stream's low seq is now the baseline.
                stored.frame = frame;
                stored.accept_next_regardless = false;
                self.frames_superseded += 1;
                return FramePublish::Stored;
            }
            if frame.seq() <= stored.frame.seq() {
                // Incoming is not newer: it is the stale one, coalesced away.
                self.frames_superseded += 1;
                return FramePublish::Superseded;
            }
            // Incoming is newer: it displaces the stored latest, which is coalesced.
            stored.frame = frame;
            self.frames_superseded += 1;
            return FramePublish::Stored;
        }

        // A source we have never seen. Admission, not eviction: if the map is full we
        // refuse the newcomer rather than quietly murder a live source.
        if self.latest.len() >= self.capacity.max_sources {
            return FramePublish::SourceCapReached {
                capacity: self.capacity.max_sources,
            };
        }
        self.latest.insert(
            source,
            StoredFrame {
                frame,
                accept_next_regardless: false,
            },
        );
        FramePublish::Stored
    }

    /// The current latest frame for `source`, if any has been published.
    pub fn latest_frame(&self, source: FrameSource) -> Option<&Frame> {
        self.latest.get(&source).map(|s| &s.frame)
    }

    /// How many distinct sources currently have a live latest frame.
    pub fn live_sources(&self) -> usize {
        self.latest.len()
    }

    /// Retires a source: drops its stored frame (freeing the pinned raster bytes) and
    /// returns a [`RetirementReceipt`]. This is how retained memory comes back down — the
    /// cure for the old never-retire leak. After retirement the source may be re-published
    /// fresh (its next frame is a first frame, accepted at any seq).
    pub fn retire_source(&mut self, source: FrameSource) -> RetirementReceipt {
        match self.latest.remove(&source) {
            Some(stored) => RetirementReceipt {
                source,
                freed_bytes: frame_bytes(&stored.frame),
                had_frame: true,
            },
            None => RetirementReceipt {
                source,
                freed_bytes: 0,
                had_frame: false,
            },
        }
    }

    /// Arms a source so its next published frame is accepted regardless of seq — the
    /// explicit epoch/restart mechanism for a producer that reset its sequence to low
    /// numbers. The currently stored frame stays visible until that next frame arrives
    /// (a restart never blanks the panel). Returns `true` if the source was present and
    /// armed; a restart on an unknown source is a no-op (`false`) because its next frame
    /// is already a first frame and accepted anyway.
    pub fn restart_source(&mut self, source: FrameSource) -> bool {
        match self.latest.get_mut(&source) {
            Some(stored) => {
                stored.accept_next_regardless = true;
                true
            }
            None => false,
        }
    }

    /// Total raster bytes currently pinned by all live frames. Because the source count is
    /// capped, this is bounded above by
    /// [`worst_case_retained_bytes`](SnapshotBridge::worst_case_retained_bytes) — the
    /// bound that makes "bounded" mean something.
    pub fn retained_frame_bytes(&self) -> usize {
        self.latest.values().map(|s| frame_bytes(&s.frame)).sum()
    }

    /// The provable ceiling on [`retained_frame_bytes`](SnapshotBridge::retained_frame_bytes):
    /// `max_sources * MAX_FRAME_BYTES`. A finite number, by construction.
    pub fn worst_case_retained_bytes(&self) -> usize {
        self.capacity.max_sources.saturating_mul(MAX_FRAME_BYTES)
    }

    /// Offers a command as an ordered edit. Returns a typed [`Admission`]: queued in issue
    /// order if there is room, or [`Admission::Refused`] at
    /// [`Capacity::max_pending_commands`]. A refused command is *not* retained and *not*
    /// coalesced — refusal is backpressure, not a silent drop (Law L7). Accepted commands
    /// are never reordered and never deduped.
    pub fn issue(&mut self, command: C) -> Admission {
        self.commands_offered += 1;
        if self.commands.len() >= self.capacity.max_pending_commands {
            // The consumer is behind. We say no — out loud, counted — rather than grow.
            self.commands_refused += 1;
            return Admission::Refused(RefuseReason::CommandQueueFull {
                capacity: self.capacity.max_pending_commands,
            });
        }
        self.commands.push_back(command);
        self.commands_accepted += 1;
        Admission::Accepted
    }

    /// Drains every pending command in exact issue order (FIFO), leaving the queue empty.
    /// Each drained command counts as delivered.
    pub fn drain_commands(&mut self) -> Vec<C> {
        let drained: Vec<C> = self.commands.drain(..).collect();
        self.commands_delivered += drained.len() as u64;
        drained
    }

    /// Delivers the next pending command in issue order, or `None` if the queue is empty.
    pub fn next_command(&mut self) -> Option<C> {
        let next = self.commands.pop_front();
        if next.is_some() {
            self.commands_delivered += 1;
        }
        next
    }

    /// Acknowledges up to `n` delivered commands — the minimal "acknowledged" step the
    /// downstream contract needs, distinct from *delivered*. Clamps so `acked` can never
    /// exceed `delivered` (you cannot ack what was never handed out). Returns how many were
    /// actually acknowledged.
    pub fn acknowledge_delivered(&mut self, n: u64) -> u64 {
        let room = self.commands_delivered - self.commands_acked;
        let take = n.min(room);
        self.commands_acked += take;
        take
    }

    /// How many commands are waiting to be delivered (accepted but not yet drained).
    pub fn pending_commands(&self) -> usize {
        self.commands.len()
    }

    /// How many frames were coalesced away (a displaced old frame, or a stale incoming
    /// one) over this bridge's life — the receipt that frame churn is lossy *by design*,
    /// with a count rather than a silent discard.
    pub fn frames_superseded(&self) -> u64 {
        self.frames_superseded
    }

    /// Commands ever **offered** to [`issue`](SnapshotBridge::issue).
    pub fn commands_offered(&self) -> u64 {
        self.commands_offered
    }

    /// Commands ever **accepted** (queued).
    pub fn commands_accepted(&self) -> u64 {
        self.commands_accepted
    }

    /// Commands ever **refused** (typed backpressure).
    pub fn commands_refused(&self) -> u64 {
        self.commands_refused
    }

    /// Commands ever **delivered** (drained via
    /// [`drain_commands`](SnapshotBridge::drain_commands) or
    /// [`next_command`](SnapshotBridge::next_command)).
    pub fn commands_delivered(&self) -> u64 {
        self.commands_delivered
    }

    /// Commands **acknowledged** so far (`<= delivered`).
    pub fn commands_acked(&self) -> u64 {
        self.commands_acked
    }

    /// The command conservation law — the whole receipt in one boolean:
    ///
    /// ```text
    ///   offered  == accepted  + refused     (no silent erasure — L7)
    ///   accepted == delivered + pending      (every accepted edit accounted for)
    ///   acked    <= delivered                (cannot ack the undelivered)
    /// ```
    ///
    /// Holds regardless of frame coalescing (Law IV) and regardless of any refusals.
    pub fn commands_conserved(&self) -> bool {
        self.commands_offered == self.commands_accepted + self.commands_refused
            && self.commands_accepted == self.commands_delivered + self.pending_commands() as u64
            && self.commands_acked <= self.commands_delivered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal 1x1 frame from `source` at `seq` (pixel value encodes the seq).
    fn frame(source: u64, seq: u64) -> Frame {
        let v = seq as u8;
        Frame::from_rgb_bytes(FrameSource(source), seq, 1, 1, 3, &[v, v, v])
            .expect("1x1 frame ingests")
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Cmd {
        A,
        B,
        Move(i32),
    }

    #[test]
    fn commands_keep_exact_order_through_frame_coalescing() {
        // THE load-bearing falsifier: interleave ordered edits with a churn of frames
        // (one superseding, one stale) and prove the command stream is delivered in the
        // exact issue order, complete, untouched by any frame coalescing. Predicted:
        // [A, B, Move(7)]; latest src0 seq 2, src1 seq 5; superseded == 2.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        assert!(bridge.issue(Cmd::A).is_accepted());
        assert!(bridge.publish_frame(frame(0, 1)).is_stored()); // first src0
        assert!(bridge.issue(Cmd::B).is_accepted());
        assert!(bridge.publish_frame(frame(0, 2)).is_stored()); // supersedes src0 seq1
        assert_eq!(bridge.publish_frame(frame(0, 1)), FramePublish::Superseded); // stale
        assert!(bridge.issue(Cmd::Move(7)).is_accepted());
        assert!(bridge.publish_frame(frame(1, 5)).is_stored()); // first src1

        assert_eq!(bridge.latest_frame(FrameSource(0)).map(Frame::seq), Some(2));
        assert_eq!(bridge.latest_frame(FrameSource(1)).map(Frame::seq), Some(5));
        assert_eq!(bridge.live_sources(), 2);
        assert_eq!(bridge.frames_superseded(), 2);

        // Conservation holds before the drain.
        assert!(bridge.commands_conserved());
        assert_eq!(bridge.pending_commands(), 3);

        let drained = bridge.drain_commands();
        assert_eq!(drained, vec![Cmd::A, Cmd::B, Cmd::Move(7)]);
        assert!(bridge.commands_conserved());
        assert_eq!(bridge.commands_delivered(), 3);
        assert_eq!(bridge.pending_commands(), 0);
    }

    #[test]
    fn heavy_frame_churn_does_not_touch_the_command_stream() {
        // MUTATION CONTROL: a storm of frames (99 superseded, 1 survivor) around a tiny
        // command stream must leave the commands exactly [A, B]. If coalescing leaked into
        // the command channel this would lose or reorder one. Predicted: [A, B],
        // superseded == 99, latest seq 100.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        assert!(bridge.issue(Cmd::A).is_accepted());
        for seq in 1..=100 {
            bridge.publish_frame(frame(0, seq));
        }
        assert!(bridge.issue(Cmd::B).is_accepted());
        assert_eq!(
            bridge.latest_frame(FrameSource(0)).map(Frame::seq),
            Some(100)
        );
        assert_eq!(bridge.frames_superseded(), 99);
        assert_eq!(bridge.drain_commands(), vec![Cmd::A, Cmd::B]);
    }

    #[test]
    fn out_of_order_frames_keep_the_newest_by_seq_not_by_arrival() {
        // A higher seq arriving first, then a lower one: latest-wins is by seq, so the
        // lower (late-arriving) frame is dropped as stale, not stored. Predicted: latest
        // seq 3; the seq-1 publish is Superseded; superseded == 1.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        assert!(bridge.publish_frame(frame(0, 3)).is_stored());
        assert_eq!(bridge.publish_frame(frame(0, 1)), FramePublish::Superseded);
        assert_eq!(bridge.latest_frame(FrameSource(0)).map(Frame::seq), Some(3));
        assert_eq!(bridge.frames_superseded(), 1);
    }

    #[test]
    fn equal_seq_is_stale_and_sources_are_isolated() {
        // Equal seq does not replace (strict latest-wins); and two sources never affect
        // each other's latest. Predicted: src0 keeps its first seq-4 frame (a re-publish
        // of seq 4 is dropped); src1 is independent at seq 4; superseded == 1.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        assert!(bridge.publish_frame(frame(0, 4)).is_stored());
        assert_eq!(bridge.publish_frame(frame(0, 4)), FramePublish::Superseded); // equal
        assert!(bridge.publish_frame(frame(1, 4)).is_stored()); // different source
        assert_eq!(bridge.live_sources(), 2);
        assert_eq!(bridge.latest_frame(FrameSource(0)).map(Frame::seq), Some(4));
        assert_eq!(bridge.latest_frame(FrameSource(1)).map(Frame::seq), Some(4));
        assert_eq!(bridge.frames_superseded(), 1);
    }

    #[test]
    fn commands_are_not_coalesced_unlike_frames() {
        // CONTRAST with frames: two identical commands are BOTH kept in order (commands
        // are not deduped the way stale frames are). Predicted: [A, A], both delivered.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        assert!(bridge.issue(Cmd::A).is_accepted());
        assert!(bridge.issue(Cmd::A).is_accepted());
        assert_eq!(bridge.pending_commands(), 2);
        assert_eq!(bridge.drain_commands(), vec![Cmd::A, Cmd::A]);
    }

    #[test]
    fn next_command_delivers_fifo_and_counts() {
        // next_command pops in issue order and advances the delivered receipt; empty yields
        // None without miscounting. Predicted: A, B, then None; delivered == 2, conserved.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        assert!(bridge.issue(Cmd::A).is_accepted());
        assert!(bridge.issue(Cmd::B).is_accepted());
        assert_eq!(bridge.next_command(), Some(Cmd::A));
        assert_eq!(bridge.next_command(), Some(Cmd::B));
        assert_eq!(bridge.next_command(), None);
        assert_eq!(bridge.commands_delivered(), 2);
        assert!(bridge.commands_conserved());
    }
}
