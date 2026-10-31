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

use channel::{CommandChannel, LocalRefusal, ResolveError};
use gibson::snapshot::{Capacity, SnapshotBridge};
use gibson::wire::{
    AckStatus, CommandLedger, CommandState, LedgerCapacity, LedgerReject, WireAck, WireCommand,
};

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
