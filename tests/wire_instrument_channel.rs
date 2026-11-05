//! Coordinator tests for the wire instrument (Renaissance V, Milestone B).
//!
//! Two things proven here, both pure in-process logic (no subprocess, no clock):
//!   1. The composition hazard is REAL — the canary's discard of `note_issued`'s `Result`
//!      lets the bridge deliver a command the ledger never tracked, whose ack is then
//!      `UnknownCommand`, while each store's own `conserves()` still passes.
//!   2. The application-owned `CommandChannel` closes it: a pre-admission capacity check refuses
//!      locally instead of desyncing, and an ack is bound to its source before it can resolve.

#[path = "../examples/wire_instrument/channel.rs"]
mod channel;

use std::io::{self, Write};

use channel::{CommandChannel, LocalRefusal, ResolveError};
use gibson::snapshot::{Capacity, SnapshotBridge};
use gibson::wire::{
    AckStatus, CommandLedger, CommandState, LedgerCapacity, LedgerReject, WireAck, WireCommand,
};

/// A writer that accepts every byte of `write_all` (the full frame reaches the writer) but fails
/// on `flush`. This is the *transport-uncertain* outcome: the complete, intact frame was handed
/// off; only the flush that would confirm it reached the far side failed — so the producer may
/// well have received and applied the command, and a genuine verdict ack may follow.
struct FlushFailsAfterWrite {
    buf: Vec<u8>,
}
impl FlushFailsAfterWrite {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }
}
impl Write for FlushFailsAfterWrite {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        // `io::Error::other` sets `ErrorKind::Other` (idiomatic; matches the battery's FailOnFlush).
        Err(io::Error::other("flush failed"))
    }
}

/// A writer whose very first `write` fails, so `write_all` returns `Err` with at most a prefix
/// delivered. This is the *known-torn / not-delivered* outcome: the producer cannot have decoded
/// a whole frame, so no genuine ack for this command is possible.
struct WriteFailsImmediately;
impl Write for WriteFailsImmediately {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "write failed"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// GATE D — the conservation defect, end to end: a command whose delivery `write_all` fully
/// succeeded but whose `flush` failed is *transport-uncertain* — the producer may hold and apply
/// it. A genuine, source-matched verdict ack must RECONCILE it to its terminal fate, not be
/// rejected merely because the local flush outcome was uncertain.
///
/// RED on `16a7ab2`: the channel parks the entry at `Accepted` (never delivered), so the honest
/// producer ack dies as `AckBeforeDelivery` and the command's fate is lost.
#[test]
fn a_flush_uncertain_command_is_reconciled_by_a_genuine_ack() {
    let mut ch = CommandChannel::new(7, 1);
    let mut w = FlushFailsAfterWrite::new();

    let id = ch.issue_tracked(1, 2.0f64.to_le_bytes().to_vec()).unwrap();

    // Delivery: the full frame bytes are accepted by the writer, but flush fails.
    let delivered = ch.deliver_next(&mut w);
    assert!(
        delivered.is_err(),
        "a failed flush must surface as a delivery error"
    );
    assert!(
        !w.buf.is_empty(),
        "the full command frame reached the transport before flush failed"
    );

    // The producer received and applied the complete frame, and returns a genuine ACK on a frame
    // from the channel's own source.
    let reconciled = ch.resolve(
        7,
        WireAck {
            command_id: id,
            status: AckStatus::Acknowledged,
        },
    );
    assert!(
        matches!(reconciled, Ok(CommandState::Acknowledged)),
        "a genuine ack must reconcile a transport-uncertain command, got {reconciled:?}"
    );
    assert_eq!(ch.acknowledged(), 1);
    assert_eq!(ch.outstanding(), 0, "the reconciled command is resolved");
    assert!(ch.conserves(), "stores still conserve after reconciliation");
}

/// GATE D negative control / distinction: a command whose delivery `write_all` FAILED is genuinely
/// not-delivered (the producer saw at most an undecodable prefix), so an "ack" for it is a protocol
/// fault and must still be refused as `AckBeforeDelivery`. This is the boundary the fix must NOT
/// cross: transport-uncertain reconciles; known-torn does not. GREEN before and after the fix.
#[test]
fn a_write_failed_command_still_refuses_an_ack_as_before_delivery() {
    let mut ch = CommandChannel::new(7, 1);
    let mut w = WriteFailsImmediately;

    let id = ch.issue_tracked(1, 2.0f64.to_le_bytes().to_vec()).unwrap();
    assert!(
        ch.deliver_next(&mut w).is_err(),
        "a failed write must surface as a delivery error"
    );

    let rejected = ch.resolve(
        7,
        WireAck {
            command_id: id,
            status: AckStatus::Acknowledged,
        },
    );
    match rejected {
        Err(ResolveError::Ledger(LedgerReject::AckBeforeDelivery { command_id })) => {
            assert_eq!(command_id, id)
        }
        other => panic!("ack for a never-delivered (torn) command must be refused, got {other:?}"),
    }
    assert_eq!(ch.acknowledged(), 0);
    assert_eq!(ch.transport_failed(), 1, "the torn write is counted");
    assert!(ch.conserves());
}

/// The defect, demonstrated: drain-without-ack fills the ledger while the bridge keeps
/// accepting; discarding `note_issued`'s `Result` (exactly as `smartchem_canary.rs` does) lets
/// an untracked command be delivered, and its ack correlates to nothing.
#[test]
fn discarded_note_issued_result_desyncs_bridge_from_ledger() {
    let mut bridge: SnapshotBridge<WireCommand> =
        SnapshotBridge::with_capacity(Capacity::new(16, 256));
    let mut ledger = CommandLedger::with_capacity(LedgerCapacity::new(256));

    // Issue + deliver 256 edits, never acking: the bridge's PENDING count is drained to 0 each
    // iteration (so it never hits its own cap), while the ledger's OUTSTANDING count climbs to
    // its cap — delivery does not free a ledger slot, only a terminal ack does.
    for id in 1..=256u64 {
        assert!(bridge
            .issue(WireCommand {
                command_id: id,
                kind: 1,
                arg: vec![],
            })
            .is_accepted());
        let _ = ledger.note_issued(id); // Result discarded — the canary pattern
        let got = bridge.next_command().expect("one pending to drain");
        let _ = ledger.note_delivered(got.command_id); // Result discarded
    }
    assert_eq!(ledger.outstanding(), 256);
    assert_eq!(bridge.pending_commands(), 0);

    // The 257th: the bridge accepts and delivers it; the ledger SILENTLY refuses to track it,
    // and the discard throws that refusal away.
    assert!(bridge
        .issue(WireCommand {
            command_id: 257,
            kind: 1,
            arg: vec![],
        })
        .is_accepted());
    let refusal = ledger.note_issued(257);
    assert!(
        matches!(refusal, Err(LedgerReject::OutstandingCapReached { .. })),
        "ledger should refuse the 257th, got {refusal:?}"
    );
    let delivered = bridge
        .next_command()
        .expect("bridge delivered the untracked 257");
    assert_eq!(delivered.command_id, 257);

    // So the producer's eventual ack for 257 correlates to nothing in the ledger.
    let orphan = ledger.apply_ack(WireAck {
        command_id: 257,
        status: AckStatus::Acknowledged,
    });
    assert!(
        matches!(orphan, Err(LedgerReject::UnknownCommand { .. })),
        "ack for the untracked command must be UnknownCommand, got {orphan:?}"
    );

    // And yet each component's OWN conservation law still passes — the break is cross-component,
    // which is exactly why the discard is dangerous: nothing local looks wrong.
    assert!(bridge.commands_conserved(), "bridge conserves on its own");
    assert!(ledger.conserves(), "ledger conserves on its own");
}

/// The fix: at the cap the coordinator refuses *locally* and mutates neither store, so no
/// untracked command is ever delivered.
#[test]
fn coordinator_refuses_at_cap_without_desync() {
    let mut ch = CommandChannel::new(1, 1);
    let mut sink: Vec<u8> = Vec::new();

    for _ in 0..256 {
        let id = ch.issue_tracked(1, vec![]).expect("below cap");
        assert_eq!(ch.deliver_next(&mut sink).unwrap(), Some(id));
    }
    assert_eq!(ch.outstanding(), 256);
    assert_eq!(ch.pending(), 0);

    // The 257th is refused before either store is touched.
    assert_eq!(ch.issue_tracked(1, vec![]), Err(LocalRefusal::LedgerFull));
    assert_eq!(ch.outstanding(), 256);
    assert_eq!(ch.pending(), 0);
    assert!(ch.conserves(), "both stores still conserve and still agree");
}

/// GATE D at the ledger boundary: `TransportUncertain` is a live, non-terminal state reachable
/// only from `Accepted`, it does NOT advance `delivered_ever`, it is fully accounted by
/// `conserves()`, and a genuine verdict ack resolves it to its terminal fate.
#[test]
fn transport_uncertain_is_a_live_nonterminal_state_that_an_ack_resolves() {
    let mut ledger = CommandLedger::with_capacity(LedgerCapacity::new(8));
    ledger.note_issued(1).unwrap();

    // Accepted -> TransportUncertain: a live slot, but not counted as a confirmed delivery.
    ledger.note_transport_uncertain(1).unwrap();
    assert_eq!(ledger.state(1), Some(CommandState::TransportUncertain));
    assert_eq!(ledger.uncertain_ever(), 1);
    assert_eq!(ledger.uncertain_now(), 1);
    assert_eq!(
        ledger.delivered_ever(),
        0,
        "uncertain is not a confirmed delivery"
    );
    assert_eq!(
        ledger.outstanding(),
        1,
        "it stays live, awaiting reconciliation"
    );
    assert!(ledger.conserves(), "uncertain is fully accounted");

    // Delivering an already-uncertain command is refused (it left `Accepted`): no silent retry.
    assert!(matches!(
        ledger.note_delivered(1),
        Err(LedgerReject::NotAwaitingDelivery {
            command_id: 1,
            state: CommandState::TransportUncertain
        })
    ));

    // A genuine ack reconciles it to Acknowledged, pruning it from the live set.
    assert_eq!(
        ledger.apply_ack(WireAck {
            command_id: 1,
            status: AckStatus::Acknowledged,
        }),
        Ok(CommandState::Acknowledged)
    );
    assert_eq!(ledger.acknowledged(), 1);
    assert_eq!(ledger.outstanding(), 0);
    assert_eq!(ledger.uncertain_now(), 0);
    assert!(ledger.conserves(), "resolution keeps the books balanced");
}

/// Source binding: an ack for a live command, but arriving on a frame from a *different*
/// producer, must not resolve it — even though the ledger alone indexes by `command_id`.
#[test]
fn ack_from_the_wrong_source_is_refused_not_mis_correlated() {
    let mut ch = CommandChannel::new(7, 1); // bound to source 7
    let mut sink: Vec<u8> = Vec::new();

    let id = ch.issue_tracked(1, 2.0f64.to_le_bytes().to_vec()).unwrap();
    assert_eq!(ch.deliver_next(&mut sink).unwrap(), Some(id));

    let foreign = ch.resolve(
        9, // a frame from source 9
        WireAck {
            command_id: id,
            status: AckStatus::Acknowledged,
        },
    );
    match foreign {
        Err(ResolveError::WrongSource { expected, got }) => {
            assert_eq!(expected, 7);
            assert_eq!(got, 9);
        }
        other => panic!("wrong-source ack must be refused, got {other:?}"),
    }
    assert_eq!(
        ch.outstanding(),
        1,
        "command stays outstanding, not falsely resolved"
    );

    // The correct source resolves it.
    let own = ch.resolve(
        7,
        WireAck {
            command_id: id,
            status: AckStatus::Acknowledged,
        },
    );
    assert!(matches!(own, Ok(CommandState::Acknowledged)));
    assert_eq!(ch.outstanding(), 0);
}

/// Retry identity: a delayed/duplicate ack for a resolved id cannot bind to anything, and the
/// next edit gets a fresh monotonic id rather than reusing a resolved one.
#[test]
fn a_delayed_ack_cannot_bind_to_a_reused_id() {
    let mut ch = CommandChannel::new(1, 1);
    let mut sink: Vec<u8> = Vec::new();

    let id1 = ch.issue_tracked(1, vec![]).unwrap();
    ch.deliver_next(&mut sink).unwrap();
    assert!(matches!(
        ch.resolve(
            1,
            WireAck {
                command_id: id1,
                status: AckStatus::Acknowledged,
            },
        ),
        Ok(CommandState::Acknowledged)
    ));

    // A second ack for id1 — it was pruned on resolution, so it is simply unknown now.
    let dup = ch.resolve(
        1,
        WireAck {
            command_id: id1,
            status: AckStatus::Acknowledged,
        },
    );
    match dup {
        Err(ResolveError::Ledger(LedgerReject::UnknownCommand { command_id })) => {
            assert_eq!(command_id, id1)
        }
        other => panic!("duplicate ack must be UnknownCommand, got {other:?}"),
    }

    // A retry uses a new, strictly greater id — never the resolved one.
    let id2 = ch.issue_tracked(1, vec![]).unwrap();
    assert!(
        id2 > id1,
        "retry must mint a fresh monotonic id (got {id2} after {id1})"
    );
}
