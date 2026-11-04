// Shared by the example and by `tests/wire_instrument_channel.rs` via `#[path]`; not every
// consumer exercises every accessor, so follow the workbench/scope convention and quiet
// dead_code for the whole coordinator module rather than scattering per-method allows.
#![allow(dead_code)]
//! Application-owned command coordinator for the wire instrument (Renaissance V, Candidate A).
//!
//! The wire library ships two independent stores: `SnapshotBridge<WireCommand>` owns the
//! ordered, bounded *delivery* of edits; `CommandLedger` owns their per-id *fate*. Composing
//! them naively is a real hazard — `examples/smartchem_canary.rs` does
//! `bridge.issue(cmd); let _ = ledger.note_issued(id);`, throwing the ledger's `Result` away.
//! The two caps are *different quantities*: the bridge's `max_pending_commands` counts edits
//! queued-but-not-drained (freed by `next_command`), while the ledger's `max_outstanding`
//! counts edits issued-but-not-resolved (freed only by a terminal ack — `note_delivered` does
//! NOT free a ledger slot). So a run of deliver-without-ack edits drains the bridge while the
//! ledger fills; once `note_issued` starts returning `OutstandingCapReached`, the discarded
//! `Result` lets the bridge deliver a command the ledger never tracked. Its eventual ack is
//! then an `UnknownCommand`, and cross-component accounting breaks *silently* — each store's
//! own `conserves()` still passes. `tests/wire_instrument_channel.rs` reproduces exactly that.
//!
//! `CommandChannel` closes the gap: it owns both stores and never lets them disagree, and it
//! binds an ack to a source before resolving it (the ledger indexes by `command_id` alone, so
//! a stray ack from a *different* producer could otherwise resolve a command addressed
//! elsewhere — source-checking is correlation, not authentication, but it stops the mix-up).
//!
//! It lives in the example, not `src/`, on purpose: Renaissance V's directive is to prove the
//! single-source instrument with the existing building blocks first (Candidate A), and promote
//! a library façade (Candidate B) only once an application demonstrates the need.

use std::io::{self, Write};

use gibson::snapshot::{Capacity, SnapshotBridge};
use gibson::wire::{
    encode_command_checked, issue_command, CommandLedger, CommandState, LedgerCapacity,
    LedgerReject, SourceCursor, WireAck, WireCommand, WireError, WireLimits,
};

/// The pending/outstanding ceiling both stores share (matches the canary's bridge config).
pub const COMMAND_CAP: usize = 256;

/// Why a local `issue_tracked` was refused *before* either store was mutated. Distinct from a
/// producer-issued verdict (`Refused`/`Failed` ack): this is local backpressure, nothing left
/// the presenter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalRefusal {
    /// The ledger is at its outstanding ceiling — too many edits still await a verdict.
    LedgerFull,
    /// The bridge is at its pending ceiling — too many edits still await delivery.
    BridgeFull,
    /// The edit's `arg` exceeds [`WireLimits::max_command_arg_bytes`], so it would encode to a
    /// frame the matching decoder must reject on the far side. Refused here, locally, before
    /// either store is touched — a locally-built command cannot bypass the wire's byte cap (3D).
    TooLarge {
        /// The offending `arg` length.
        arg_len: usize,
        /// The ceiling it exceeded.
        max: usize,
    },
    /// The per-session command-id space is exhausted: the last usable id (`u64::MAX`) has already
    /// been issued, and ids must stay strictly monotonic, so no further edit can be minted. A
    /// theoretical ceiling (2^64 edits in one session), surfaced rather than wrapped into a
    /// colliding id (3D).
    IdsExhausted,
}

/// A record of the write/flush error that faulted a [`CommandChannel`]. Once set, the channel's
/// byte stream may carry a torn frame, so the channel refuses all further delivery rather than
/// append a fresh frame onto a corrupt stream (which the producer would decode as garbage, and
/// which the channel would otherwise falsely mark `Delivered` — L5/L10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportFault {
    /// The command whose delivery write failed. It remains outstanding-and-undelivered in the
    /// ledger (never faked as delivered) and is counted in [`CommandChannel::transport_failed`].
    pub command_id: u64,
    /// The sequence number the faulted frame was encoded with. It is NOT committed to the
    /// channel's `out_seq`, so no later frame reuses or skips past it.
    pub seq: u64,
}

/// Why resolving a returned ack failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The ack arrived on a frame from a different source than this channel addresses.
    WrongSource { expected: u64, got: u64 },
    /// The ack arrived on a frame from a different EPOCH — a producer restart (A1). A restarted
    /// producer resets its command ids, so an ack from the new session could otherwise collide with
    /// and falsely resolve a command issued in the prior session. The command stays unresolved.
    WrongEpoch { expected: u32, got: u32 },
    /// The ledger rejected the ack (unknown id, ack-before-delivery, or non-terminal status).
    Ledger(LedgerReject),
}

/// Owns the bridge + ledger for one producer source and keeps them in lockstep.
pub struct CommandChannel {
    bridge: SnapshotBridge<WireCommand>,
    ledger: CommandLedger,
    source: u64,
    epoch: u32,
    next_id: u64,
    out_seq: u64,
    /// The byte ceilings a locally-built command must respect so it cannot bypass the cap the
    /// decoder enforces on the far side (3D).
    limits: WireLimits,
    /// Set once a delivery write/flush fails; the channel then refuses all further delivery (the
    /// stream may be torn). `None` while healthy.
    fault: Option<TransportFault>,
    /// Commands popped from the bridge toward the wire whose delivery `write_all` failed (known
    /// not delivered / torn) — counted so the bridge's "delivered" and the ledger's "delivered"
    /// can be reconciled rather than silently disagreeing (L7, and the cross-store half of
    /// [`Self::conserves`]). Distinct from [`transport_uncertain`](Self::transport_uncertain),
    /// which counts a *handed-off-but-unconfirmed* frame (Gate D).
    transport_failed: u64,
    /// Commands popped toward the wire whose full frame was written (`write_all` ok) but whose
    /// `flush` failed — *transport-uncertain* deliveries (Gate D). Tracked separately from
    /// [`transport_failed`](Self::transport_failed) because the producer may have received and
    /// applied them, so a later genuine ack reconciles them. Mirrors the ledger's
    /// [`CommandLedger::uncertain_ever`](gibson::wire::CommandLedger::uncertain_ever).
    transport_uncertain: u64,
    /// Set once `next_id` has handed out `u64::MAX`; further issues are refused as
    /// [`LocalRefusal::IdsExhausted`] rather than wrapping.
    ids_exhausted: bool,
}

impl CommandChannel {
    /// A channel bound to one producer `source` / `epoch`, with the default wire limits.
    pub fn new(source: u64, epoch: u32) -> Self {
        Self::with_limits(source, epoch, WireLimits::default())
    }

    /// A channel bound to one producer `source` / `epoch` with explicit `limits`. The limits bound
    /// the `arg` of every issued command (3D); a command whose `arg` exceeds
    /// [`WireLimits::max_command_arg_bytes`] is refused at [`issue_tracked`](Self::issue_tracked).
    pub fn with_limits(source: u64, epoch: u32, limits: WireLimits) -> Self {
        Self {
            // 16 source slots is irrelevant for a command bridge; only the pending cap matters.
            bridge: SnapshotBridge::with_capacity(Capacity::new(16, COMMAND_CAP)),
            ledger: CommandLedger::with_capacity(LedgerCapacity::new(COMMAND_CAP)),
            source,
            epoch,
            next_id: 1,
            out_seq: 0,
            limits,
            fault: None,
            transport_failed: 0,
            transport_uncertain: 0,
            ids_exhausted: false,
        }
    }

    /// Admit an edit into *both stores atomically, or neither*.
    ///
    /// The trap: `bridge.issue` and `ledger.note_issued` can each reject independently, leaving
    /// one mutated and the other not. We avoid it without a rollback (the ledger exposes no
    /// "un-issue") via a capacity pre-check. Id monotonicity holds by construction — `next_id`
    /// only advances on success — and this is single-threaded, so if *both* stores sit below
    /// their ceilings now, neither call can reject. If either is full we refuse locally and
    /// touch nothing. After the pre-check the two mutations cannot fail; the asserts guard that
    /// invariant loudly rather than let a silent desync through.
    pub fn issue_tracked(&mut self, kind: u32, arg: Vec<u8>) -> Result<u64, LocalRefusal> {
        // 3D — the id space is finite; refuse rather than wrap into a colliding id.
        if self.ids_exhausted {
            return Err(LocalRefusal::IdsExhausted);
        }
        // Pre-check BOTH ceilings before touching either store, so the two mutations below cannot
        // reject (atomic admit-or-neither; the ledger exposes no "un-issue" to roll back).
        if self.ledger.outstanding() >= self.ledger.capacity().max_outstanding {
            return Err(LocalRefusal::LedgerFull);
        }
        if self.bridge.pending_commands() >= COMMAND_CAP {
            return Err(LocalRefusal::BridgeFull);
        }
        let id = self.next_id;
        let arg_len = arg.len();
        // 3D — admit through the library's byte-bounded `issue_command`, which refuses a command
        // whose TOTAL payload (head + arg) exceeds the wire limits *before* it is queued. Routing
        // the bound through the library (rather than a local `arg.len()` test) keeps one source of
        // truth and closes the sub-header-budget gap where an arg-only test wrongly admits the
        // empty command. On refusal nothing was mutated; on success the bridge holds it.
        match issue_command(
            &mut self.bridge,
            WireCommand {
                command_id: id,
                kind,
                arg,
            },
            self.limits,
        ) {
            Ok(admission) => assert!(
                admission.is_accepted(),
                "bridge refused a command below its pre-checked pending cap"
            ),
            Err(WireError::EncodeTooLarge) => {
                return Err(LocalRefusal::TooLarge {
                    arg_len,
                    max: self.limits.max_command_arg_bytes(),
                })
            }
            Err(other) => {
                unreachable!("issue_command only refuses with EncodeTooLarge, got {other:?}")
            }
        }
        self.ledger
            .note_issued(id)
            .expect("ledger refused a monotonic id below its pre-checked outstanding cap");
        // Advance monotonically; `u64::MAX` was the last usable id, so latch exhaustion instead
        // of overflowing.
        match self.next_id.checked_add(1) {
            Some(n) => self.next_id = n,
            None => self.ids_exhausted = true,
        }
        Ok(id)
    }

    /// Drain the next pending command, serialize it to `w`, and mark it delivered — in lockstep.
    /// Returns the delivered id, or `None` if nothing is pending.
    ///
    /// Ordering is the whole point. The sequence number and the ledger's `Delivered` transition
    /// are committed *only after* a fully successful `write_all` + `flush`. The bridge's own
    /// `next_command` pop is irreversible and counts the command as delivered the moment it
    /// leaves the queue, so a write error would otherwise leave the bridge saying "delivered"
    /// while the ledger still says `Accepted` — the two stores silently disagreeing, with the
    /// command orphaned and un-redeliverable. Instead, a write/flush error:
    ///
    /// * leaves the ledger entry `Accepted` (never faked as `Delivered` — L5/L10),
    /// * does NOT advance `out_seq` (no sequence number is burned for a frame that did not land),
    /// * counts the loss in [`transport_failed`](Self::transport_failed) so the bridge's and the
    ///   ledger's "delivered" tallies stay reconciled (L7, [`conserves`](Self::conserves)), and
    /// * **faults the channel**: the stream may now carry a torn frame, so every subsequent call
    ///   refuses (returns `Err`) rather than append a fresh frame onto a corrupt stream — which
    ///   the producer would decode as garbage and which the channel would otherwise falsely mark
    ///   `Delivered`.
    ///
    /// Exactly-once is not claimed even on success: `Ok(id)` means the bytes were handed to the
    /// transport (`Delivered`), not that the producer applied them (`Acknowledged`).
    pub fn deliver_next<W: Write>(&mut self, w: &mut W) -> io::Result<Option<u64>> {
        if let Some(fault) = self.fault {
            // The stream may be torn; refuse to write another frame onto it.
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                format!(
                    "command channel faulted on command {} (seq {}); stream may be torn",
                    fault.command_id, fault.seq
                ),
            ));
        }
        let Some(cmd) = self.bridge.next_command() else {
            return Ok(None);
        };
        let id = cmd.command_id;
        // Candidate sequence — committed to `out_seq` only if the write fully lands.
        let seq = self
            .out_seq
            .checked_add(1)
            .expect("out_seq exhausted: more deliveries than the u64 id space allows");
        // `arg` was bounded against `self.limits` at issue, so the checked encode cannot refuse;
        // the `expect` documents that invariant rather than silently trusting it.
        let bytes = encode_command_checked(
            self.source,
            SourceCursor::new(self.epoch, seq),
            &cmd,
            self.limits,
        )
        .expect("command arg bounded at issue stays within limits at delivery");
        // Two physically distinct failure outcomes must NOT be conflated (Gate D). `write_all`
        // returning `Err` means not all bytes were written: the producer saw at most an
        // undecodable prefix, so the command is *known not delivered* and no genuine ack is
        // possible. `write_all` succeeding then `flush` failing means the *complete* frame reached
        // the writer but the confirming flush failed: the producer may hold and apply it, so
        // delivery is *transport-uncertain* and a later genuine ack must be able to reconcile it.
        match w.write_all(&bytes) {
            Err(e) => {
                // Known not delivered (torn/short write). Ledger entry stays `Accepted`; a later
                // "ack" for it is correctly a protocol fault (`AckBeforeDelivery`). Count the loss
                // and fault (the stream may carry a torn prefix).
                self.transport_failed += 1;
                self.fault = Some(TransportFault {
                    command_id: id,
                    seq,
                });
                Err(e)
            }
            Ok(()) => match w.flush() {
                Ok(()) => {
                    self.out_seq = seq;
                    self.ledger
                        .note_delivered(id)
                        .expect("delivered a command the ledger is tracking");
                    Ok(Some(id))
                }
                Err(e) => {
                    // Transport-uncertain: the whole frame was handed off, flush was not confirmed.
                    // Mark the ledger entry `TransportUncertain` (NOT `Delivered`, NOT left
                    // `Accepted`) so a genuine verdict ack can reconcile it; count it; and still
                    // fault, because a generic `W` (e.g. a `BufWriter`) may have spilled a torn
                    // prefix — the channel conservatively refuses further delivery. `out_seq` is
                    // not advanced: the frame is not *confirmed* flushed, and we never emit another
                    // frame (faulted), so the sequence number is not reused.
                    self.transport_uncertain += 1;
                    self.ledger
                        .note_transport_uncertain(id)
                        .expect("uncertain delivery of a command the ledger is tracking");
                    self.fault = Some(TransportFault {
                        command_id: id,
                        seq,
                    });
                    Err(e)
                }
            },
        }
    }

    /// Resolve a returned ack in this channel's command session. Binds the ack to the channel's
    /// source first (an ack on a frame from another source cannot resolve a command addressed here,
    /// even if the id collides) and assumes the channel's bound epoch. When the ack frame's epoch is
    /// available — the interactive loop — prefer [`CommandChannel::resolve_in_session`], which also
    /// refuses an ack from a restarted producer (a different epoch).
    pub fn resolve(&mut self, source: u64, ack: WireAck) -> Result<CommandState, ResolveError> {
        let epoch = self.epoch;
        self.resolve_in_session(source, epoch, ack)
    }

    /// Resolve a returned ack against the FULL command-session identity `(source, epoch)` (A1). An
    /// ack from another source (`WrongSource`) or another epoch (`WrongEpoch` — a producer restart,
    /// a new session whose reset command ids must not resolve this session's commands) cannot
    /// resolve a command this channel issued, even if the `command_id` collides.
    pub fn resolve_in_session(
        &mut self,
        source: u64,
        epoch: u32,
        ack: WireAck,
    ) -> Result<CommandState, ResolveError> {
        if source != self.source {
            return Err(ResolveError::WrongSource {
                expected: self.source,
                got: source,
            });
        }
        if epoch != self.epoch {
            return Err(ResolveError::WrongEpoch {
                expected: self.epoch,
                got: epoch,
            });
        }
        self.ledger.apply_ack(ack).map_err(ResolveError::Ledger)
    }

    // --- read-only telemetry for the HUD and conservation receipts ---

    /// The producer source this channel is bound to.
    pub fn source(&self) -> u64 {
        self.source
    }
    /// Edits issued but not yet resolved (the ledger's live set).
    pub fn outstanding(&self) -> usize {
        self.ledger.outstanding()
    }
    /// Edits queued in the bridge but not yet delivered.
    pub fn pending(&self) -> usize {
        self.bridge.pending_commands()
    }
    /// Total edits ever issued.
    pub fn issued(&self) -> u64 {
        self.ledger.issued()
    }
    /// Edits the producer acknowledged (applied).
    pub fn acknowledged(&self) -> u64 {
        self.ledger.acknowledged()
    }
    /// Edits the producer refused (valid frame, rejected value — model untouched).
    pub fn refused(&self) -> u64 {
        self.ledger.refused()
    }
    /// Edits the producer failed (could not apply — never a success).
    pub fn failed(&self) -> u64 {
        self.ledger.failed()
    }
    /// Frames **confirmed** written-and-flushed onto the wire (the committed `out_seq`). Advances
    /// only on a fully successful `write_all` + `flush`, exactly in lockstep with the ledger's
    /// `Accepted` → `Delivered` transition, so `delivered_frames() == ledger.delivered_ever()`
    /// always. A torn write or a transport-uncertain (flush-failed) delivery does NOT advance it —
    /// an unconfirmed frame is never counted as confirmed.
    pub fn delivered_frames(&self) -> u64 {
        self.out_seq
    }
    /// Commands popped toward the wire whose `write_all` failed — known not delivered / torn (L7 —
    /// counted, never hidden).
    pub fn transport_failed(&self) -> u64 {
        self.transport_failed
    }
    /// Commands popped toward the wire whose full frame was written but whose `flush` failed —
    /// *transport-uncertain* (Gate D). A later genuine, source-matched ack reconciles these to
    /// their terminal fate; they are neither counted as delivered nor lost.
    pub fn transport_uncertain(&self) -> u64 {
        self.transport_uncertain
    }
    /// Whether a delivery write/flush has faulted the channel. A faulted channel refuses all
    /// further delivery; the fault names the command and sequence number that broke.
    pub fn fault(&self) -> Option<TransportFault> {
        self.fault
    }
    /// Convenience predicate: the channel has faulted and will deliver no more.
    pub fn faulted(&self) -> bool {
        self.fault.is_some()
    }

    /// Conservation across *both* stores — each store's own law, plus the cross-store identity
    /// that makes a delivery desync detectable.
    ///
    /// The bridge counts a command "delivered" the instant `next_command` pops it; the ledger
    /// counts it "delivered" only when [`deliver_next`](Self::deliver_next) confirms the full
    /// write *and* flush landed. Every popped command is therefore in exactly one of three
    /// buckets: confirmed-delivered (`ledger.delivered_ever`), transport-uncertain
    /// ([`transport_uncertain`](Self::transport_uncertain), mirrored by `ledger.uncertain_ever`),
    /// or known-torn ([`transport_failed`](Self::transport_failed)). So:
    ///
    /// ```text
    ///   bridge.commands_delivered == ledger.delivered_ever + ledger.uncertain_ever + transport_failed
    /// ```
    ///
    /// The channel's own `transport_uncertain` counter must also agree with the ledger's
    /// `uncertain_ever`, catching a channel/ledger desync on the uncertain path exactly as the
    /// identity above catches one on the delivered path. On a healthy channel both failure
    /// counters are 0 and the bridge's and ledger's delivered tallies agree exactly. The naive
    /// ordering (pop, then write, then mark delivered on success) breaks this identity silently on
    /// any write error — which is precisely the lie this channel exists to prevent.
    pub fn conserves(&self) -> bool {
        self.ledger.conserves()
            && self.bridge.commands_conserved()
            && self.transport_uncertain == self.ledger.uncertain_ever()
            && self.bridge.commands_delivered()
                == self.ledger.delivered_ever()
                    + self.ledger.uncertain_ever()
                    + self.transport_failed
    }
}

// The id-exhaustion latch is testable only with private access (the public path cannot reach
// `u64::MAX` issues in finite time), so it lives here rather than in the `#[path]`-included
// hostile battery. `#[cfg(test)]` keeps it out of the `wire_instrument` example build entirely.
#[cfg(test)]
mod coordinator_unit {
    use super::*;

    #[test]
    fn id_exhaustion_is_latched_not_wrapped() {
        let mut ch = CommandChannel::new(1, 1);
        // Seed the counter to the last usable id (private field — same module).
        ch.next_id = u64::MAX;

        // The final id issues cleanly, and latches exhaustion as it advances.
        let last = ch
            .issue_tracked(1, vec![])
            .expect("u64::MAX is a usable id");
        assert_eq!(last, u64::MAX);
        assert!(ch.ids_exhausted, "issuing the last id latches exhaustion");

        // The next issue is refused rather than wrapping to a colliding id.
        assert_eq!(ch.issue_tracked(1, vec![]), Err(LocalRefusal::IdsExhausted));
        // And again — the latch is sticky, and nothing was mutated.
        assert_eq!(ch.issue_tracked(1, vec![]), Err(LocalRefusal::IdsExhausted));
        assert_eq!(ch.issued(), 1, "only the one real id was ever issued");
        assert!(ch.conserves());
    }
}
