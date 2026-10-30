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
    encode_command, CommandLedger, CommandState, LedgerCapacity, LedgerReject, SourceCursor,
    WireAck, WireCommand,
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
}

/// Why resolving a returned ack failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The ack arrived on a frame from a different source than this channel addresses.
    WrongSource { expected: u64, got: u64 },
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
}

impl CommandChannel {
    /// A channel bound to one producer `source` / `epoch`.
    pub fn new(source: u64, epoch: u32) -> Self {
        Self {
            // 16 source slots is irrelevant for a command bridge; only the pending cap matters.
            bridge: SnapshotBridge::with_capacity(Capacity::new(16, COMMAND_CAP)),
            ledger: CommandLedger::with_capacity(LedgerCapacity::new(COMMAND_CAP)),
            source,
            epoch,
            next_id: 1,
            out_seq: 0,
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
        if self.ledger.outstanding() >= self.ledger.capacity().max_outstanding {
            return Err(LocalRefusal::LedgerFull);
        }
        if self.bridge.pending_commands() >= COMMAND_CAP {
            return Err(LocalRefusal::BridgeFull);
        }
        let id = self.next_id;
        let admission = self.bridge.issue(WireCommand {
            command_id: id,
            kind,
            arg,
        });
        assert!(
            admission.is_accepted(),
            "bridge refused a command below its pre-checked pending cap"
        );
        self.ledger
            .note_issued(id)
            .expect("ledger refused a monotonic id below its pre-checked outstanding cap");
        self.next_id += 1;
        Ok(id)
    }

    /// Drain the next pending command, serialize it to `w`, and mark it delivered — in lockstep.
    /// Returns the delivered id, or `None` if nothing is pending.
    ///
    /// Delivery is marked only *after* a successful write/flush. If the write errors (a broken
    /// pipe, say), the command has already left the bridge but remains `Accepted` (not
    /// `Delivered`) in the ledger — reported honestly as outstanding-and-undelivered, never
    /// faked as delivered. Exactly-once is not claimed: the bytes may or may not have reached
    /// the producer, and the channel says so by leaving the id un-advanced past `Accepted`.
    pub fn deliver_next<W: Write>(&mut self, w: &mut W) -> io::Result<Option<u64>> {
        let Some(cmd) = self.bridge.next_command() else {
            return Ok(None);
        };
        let id = cmd.command_id;
        self.out_seq += 1;
        let bytes = encode_command(
            self.source,
            SourceCursor::new(self.epoch, self.out_seq),
            &cmd,
        );
        w.write_all(&bytes)?;
        w.flush()?;
        self.ledger
            .note_delivered(id)
            .expect("delivered a command the ledger is tracking");
        Ok(Some(id))
    }

    /// Resolve a returned ack, binding it to this channel's source *first*. An ack on a frame
    /// from another source cannot resolve a command addressed here, even if the id collides.
    pub fn resolve(&mut self, source: u64, ack: WireAck) -> Result<CommandState, ResolveError> {
        if source != self.source {
            return Err(ResolveError::WrongSource {
                expected: self.source,
                got: source,
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
    /// Both stores' conservation laws hold.
    pub fn conserves(&self) -> bool {
        self.ledger.conserves() && self.bridge.commands_conserved()
    }
}
