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
//! * **Commands are ordered edits.** Edits are a FIFO queue that never drops and never
//!   reorders: an edit is delivered exactly once, in the order issued, no matter how many
//!   frames were coalesced beside it. Losing or reordering an edit would corrupt the
//!   consumer's authoritative state, so the bridge refuses to.
//!
//! The load-bearing guarantee this type exists to make true: **frame coalescing can never
//! perturb the command stream.** [`SnapshotBridge::commands_conserved`] is the receipt
//! (issued == delivered + pending, always), and [`SnapshotBridge::frames_superseded`]
//! counts the lossy-by-design frame coalescing with an honest number.

use crate::frame::{Frame, FrameSource};
use std::collections::HashMap;
use std::collections::VecDeque;

/// A two-channel bridge: latest-wins [`Frame`]s per source, and an ordered,
/// never-dropped queue of consumer-defined commands `C`.
///
/// The library is generic over `C`: it owns the ordering and delivery discipline; the
/// consumer owns what a command *means*.
#[derive(Debug, Clone)]
pub struct SnapshotBridge<C> {
    latest: HashMap<FrameSource, Frame>,
    commands: VecDeque<C>,
    frames_superseded: u64,
    commands_issued: u64,
    commands_drained: u64,
}

impl<C> Default for SnapshotBridge<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> SnapshotBridge<C> {
    /// An empty bridge: no frames, no pending commands.
    pub fn new() -> Self {
        Self {
            latest: HashMap::new(),
            commands: VecDeque::new(),
            frames_superseded: 0,
            commands_issued: 0,
            commands_drained: 0,
        }
    }

    /// Publishes a frame as the latest observation of its source.
    ///
    /// Latest-wins by [`seq`](crate::frame::Frame::seq): a frame with a `seq` strictly
    /// greater than the stored one replaces it (the displaced frame is *superseded*); a
    /// frame whose `seq` is not newer (less than *or equal*) is itself stale and dropped.
    /// Either way exactly one frame per source survives, and every coalesced frame — the
    /// displaced old one or the dropped stale one — is counted by
    /// [`frames_superseded`](SnapshotBridge::frames_superseded). Returns `true` if this
    /// frame is now the stored latest, `false` if it was dropped as stale.
    pub fn publish_frame(&mut self, frame: Frame) -> bool {
        match self.latest.get(&frame.source()) {
            Some(stored) if frame.seq() <= stored.seq() => {
                // Incoming is not newer: it is the stale one, coalesced away.
                self.frames_superseded += 1;
                false
            }
            Some(_) => {
                // Incoming is newer: it displaces the stored latest, which is coalesced.
                self.frames_superseded += 1;
                self.latest.insert(frame.source(), frame);
                true
            }
            None => {
                // First frame for this source: nothing superseded.
                self.latest.insert(frame.source(), frame);
                true
            }
        }
    }

    /// The current latest frame for `source`, if any has been published.
    pub fn latest_frame(&self, source: FrameSource) -> Option<&Frame> {
        self.latest.get(&source)
    }

    /// How many distinct sources currently have a live latest frame.
    pub fn live_sources(&self) -> usize {
        self.latest.len()
    }

    /// Enqueues a command as an ordered edit. Never dropped, never reordered.
    pub fn issue(&mut self, command: C) {
        self.commands.push_back(command);
        self.commands_issued += 1;
    }

    /// Drains every pending command in exact issue order (FIFO), leaving the queue empty.
    pub fn drain_commands(&mut self) -> Vec<C> {
        let drained: Vec<C> = self.commands.drain(..).collect();
        self.commands_drained += drained.len() as u64;
        drained
    }

    /// Delivers the next pending command in issue order, or `None` if the queue is empty.
    pub fn next_command(&mut self) -> Option<C> {
        let next = self.commands.pop_front();
        if next.is_some() {
            self.commands_drained += 1;
        }
        next
    }

    /// How many commands are waiting to be delivered.
    pub fn pending_commands(&self) -> usize {
        self.commands.len()
    }

    /// How many frames were coalesced away (a displaced old frame, or a stale incoming
    /// one) over this bridge's life — the receipt that frame churn is lossy *by design*,
    /// with a count rather than a silent discard.
    pub fn frames_superseded(&self) -> u64 {
        self.frames_superseded
    }

    /// Total commands ever issued.
    pub fn commands_issued(&self) -> u64 {
        self.commands_issued
    }

    /// Total commands ever delivered (drained).
    pub fn commands_drained(&self) -> u64 {
        self.commands_drained
    }

    /// The command conservation law: every issued command is either already delivered or
    /// still pending — none lost, none duplicated — regardless of frame coalescing.
    pub fn commands_conserved(&self) -> bool {
        self.commands_issued == self.commands_drained + self.pending_commands() as u64
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
        bridge.issue(Cmd::A);
        assert!(bridge.publish_frame(frame(0, 1))); // first src0
        bridge.issue(Cmd::B);
        assert!(bridge.publish_frame(frame(0, 2))); // supersedes src0 seq1
        assert!(!bridge.publish_frame(frame(0, 1))); // stale: older than stored seq2
        bridge.issue(Cmd::Move(7));
        assert!(bridge.publish_frame(frame(1, 5))); // first src1

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
        assert_eq!(bridge.commands_drained(), 3);
        assert_eq!(bridge.pending_commands(), 0);
    }

    #[test]
    fn heavy_frame_churn_does_not_touch_the_command_stream() {
        // MUTATION CONTROL: a storm of frames (99 superseded, 1 survivor) around a tiny
        // command stream must leave the commands exactly [A, B]. If coalescing leaked into
        // the command channel this would lose or reorder one. Predicted: [A, B],
        // superseded == 99, latest seq 100.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        bridge.issue(Cmd::A);
        for seq in 1..=100 {
            bridge.publish_frame(frame(0, seq));
        }
        bridge.issue(Cmd::B);
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
        // seq 3; the seq-1 publish returns false; superseded == 1.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        assert!(bridge.publish_frame(frame(0, 3)));
        assert!(!bridge.publish_frame(frame(0, 1)));
        assert_eq!(bridge.latest_frame(FrameSource(0)).map(Frame::seq), Some(3));
        assert_eq!(bridge.frames_superseded(), 1);
    }

    #[test]
    fn equal_seq_is_stale_and_sources_are_isolated() {
        // Equal seq does not replace (strict latest-wins); and two sources never affect
        // each other's latest. Predicted: src0 keeps its first seq-4 frame (a re-publish
        // of seq 4 is dropped); src1 is independent at seq 4; superseded == 1.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        assert!(bridge.publish_frame(frame(0, 4)));
        assert!(!bridge.publish_frame(frame(0, 4))); // equal seq: stale
        assert!(bridge.publish_frame(frame(1, 4))); // different source: independent
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
        bridge.issue(Cmd::A);
        bridge.issue(Cmd::A);
        assert_eq!(bridge.pending_commands(), 2);
        assert_eq!(bridge.drain_commands(), vec![Cmd::A, Cmd::A]);
    }

    #[test]
    fn next_command_delivers_fifo_and_counts() {
        // next_command pops in issue order and advances the drained receipt; empty yields
        // None without miscounting. Predicted: A, B, then None; drained == 2, conserved.
        let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
        bridge.issue(Cmd::A);
        bridge.issue(Cmd::B);
        assert_eq!(bridge.next_command(), Some(Cmd::A));
        assert_eq!(bridge.next_command(), Some(Cmd::B));
        assert_eq!(bridge.next_command(), None);
        assert_eq!(bridge.commands_drained(), 2);
        assert!(bridge.commands_conserved());
    }
}
