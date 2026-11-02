//! Hostile command-path battery for the wire instrument (Renaissance VII, Gate C).
//!
//! The inbound field path has its own battery (`wire_instrument_transport.rs`); this one is the
//! *return* channel — the presenter→producer command/ack lifecycle owned by `CommandChannel`.
//! Every case here is a hostility the delivery or acknowledgement path must survive *truthfully*:
//! a command is never reported delivered unless its bytes landed, never acknowledged unless the
//! producer said so, never silently dropped, and the two stores the channel composes
//! (`SnapshotBridge` delivery + `CommandLedger` fate) never disagree — not even on a write error.
//!
//! Pure in-process logic: no subprocess, no clock. Delivery failures are injected with
//! controlled `Write` implementations (3A); the id-exhaustion latch is unit-tested inside
//! `channel.rs` because it needs private access.

#[path = "../examples/wire_instrument/channel.rs"]
mod channel;

use std::io::{self, Write};

use channel::{CommandChannel, LocalRefusal, ResolveError};
use gibson::wire::{
    encode_command_checked, AckStatus, CommandState, FrameDecoder, LedgerReject, SourceCursor,
    WireAck, WireBody, WireCommand, WireError, WireLimits,
};

const KIND: u32 = 1;

// ---- controlled writers (3A) ---------------------------------------------------------------

/// Errors on the very first write; nothing reaches the stream.
struct FailImmediately;
impl Write for FailImmediately {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Accepts up to `ok_bytes` total, then errors — a partial write that tears a frame mid-stream.
/// Records exactly what landed so a test can confirm a torn prefix is on the wire.
struct FailAfter {
    ok_bytes: usize,
    written: Vec<u8>,
}
impl FailAfter {
    fn new(ok_bytes: usize) -> Self {
        Self {
            ok_bytes,
            written: Vec::new(),
        }
    }
}
impl Write for FailAfter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written.len() >= self.ok_bytes {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "pipe died mid-frame",
            ));
        }
        let room = self.ok_bytes - self.written.len();
        let take = room.min(buf.len());
        self.written.extend_from_slice(&buf[..take]);
        Ok(take)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Always reports `Ok(0)` — writes nothing but never errors. `write_all` must turn this into a
/// `WriteZero` error rather than loop forever, which the channel then treats as a fault.
struct WriteZero;
impl Write for WriteZero {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Ok(0)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `write_all` succeeds in full; `flush` fails — the bytes reached our buffer but we cannot
/// confirm they were pushed to the producer.
struct FailOnFlush {
    written: Vec<u8>,
}
impl FailOnFlush {
    fn new() -> Self {
        Self {
            written: Vec::new(),
        }
    }
}
impl Write for FailOnFlush {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        // `io::Error::other` sets `ErrorKind::Other`, which c03 asserts on.
        Err(io::Error::other("flush failed"))
    }
}

// ---- helpers ---------------------------------------------------------------------------------

fn ack(id: u64, status: AckStatus) -> WireAck {
    WireAck {
        command_id: id,
        status,
    }
}

/// Decode every *complete* command frame in `bytes`, returning each frame's sequence number in
/// order. A trailing partial frame is simply not yielded (the decoder buffers it).
fn command_seqs(bytes: &[u8]) -> Vec<u64> {
    let mut dec = FrameDecoder::new();
    dec.push(bytes).expect("in-bounds frames push cleanly");
    let mut out = Vec::new();
    while let Some(res) = dec.next_message() {
        let msg = res.expect("a complete frame decodes");
        if let WireBody::Command(_) = msg.body {
            // The cursor carries the delivery sequence the channel assigned.
            out.push(msg.cursor.sequence);
        }
    }
    out
}

// ========== Group A — delivery / transport truth (3A / 3B) ==================================

/// 01 — an immediate write error faults the channel and never fakes a delivery.
#[test]
fn c01_immediate_write_error_faults_without_false_delivery() {
    let mut ch = CommandChannel::new(1, 1);
    let id = ch
        .issue_tracked(KIND, 1.0f64.to_le_bytes().to_vec())
        .unwrap();

    let err = ch.deliver_next(&mut FailImmediately).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);

    assert!(ch.faulted(), "a write error must fault the channel");
    assert_eq!(ch.transport_failed(), 1);
    assert_eq!(
        ch.delivered_frames(),
        0,
        "no frame landed, so out_seq stays 0"
    );
    assert_eq!(
        ch.outstanding(),
        1,
        "the command stays outstanding, not resolved"
    );
    assert_eq!(ch.acknowledged(), 0);
    let fault = ch.fault().expect("fault recorded");
    assert_eq!(fault.command_id, id);
    assert_eq!(fault.seq, 1);
    assert!(
        ch.conserves(),
        "cross-store tallies stay reconciled through the fault"
    );
}

/// 02 — a partial write leaves a torn prefix on the stream and faults; out_seq is not burned.
#[test]
fn c02_partial_write_leaves_a_torn_frame_and_faults() {
    let mut ch = CommandChannel::new(1, 1);
    ch.issue_tracked(KIND, 2.0f64.to_le_bytes().to_vec())
        .unwrap();

    // 10 bytes is inside the 32-byte header, so the frame is guaranteed torn.
    let mut sink = FailAfter::new(10);
    let err = ch.deliver_next(&mut sink).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);

    assert_eq!(
        sink.written.len(),
        10,
        "a partial frame leaked onto the stream"
    );
    assert!(ch.faulted());
    assert_eq!(ch.transport_failed(), 1);
    assert_eq!(
        ch.delivered_frames(),
        0,
        "out_seq not advanced for a torn frame"
    );
    assert_eq!(ch.outstanding(), 1);
    assert!(ch.conserves());
}

/// 03 — write_all fully succeeds but flush fails: *transport-uncertain* (Gate D), not known-torn.
/// The complete frame reached the transport, so the producer may hold and apply it: the channel
/// still faults conservatively (a generic writer may have spilled a torn prefix) and does NOT
/// confirm delivery, but the command is classified `transport_uncertain` — NOT `transport_failed`
/// — and a later genuine, source-matched ack RECONCILES it to its terminal fate rather than
/// dying as `AckBeforeDelivery`. (This case previously conflated the uncertain outcome with a
/// known-torn write, counting it as `transport_failed` and leaving the entry unreconcilable.)
#[test]
fn c03_flush_error_after_full_write_is_transport_uncertain_and_reconcilable() {
    let mut ch = CommandChannel::new(1, 1);
    let id = ch.issue_tracked(KIND, vec![]).unwrap();

    let mut sink = FailOnFlush::new();
    let err = ch.deliver_next(&mut sink).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::Other);

    assert!(
        !sink.written.is_empty(),
        "the full command frame reached the transport before flush failed"
    );
    assert!(
        ch.faulted(),
        "an unflushed write is not a confirmed delivery; the channel faults conservatively"
    );
    assert_eq!(
        ch.transport_failed(),
        0,
        "a handed-off-but-unflushed frame is NOT a known-torn write"
    );
    assert_eq!(
        ch.transport_uncertain(),
        1,
        "it is counted as transport-uncertain (Gate D)"
    );
    assert_eq!(
        ch.delivered_frames(),
        0,
        "delivery is not confirmed, so no frame is committed"
    );
    assert_eq!(ch.outstanding(), 1, "it stays live, awaiting a verdict");
    assert!(ch.conserves());

    // The producer did receive and apply the complete frame, and returns a genuine verdict: the
    // ledger reconciles it instead of rejecting it for an uncertain local flush outcome.
    assert_eq!(
        ch.resolve(1, ack(id, AckStatus::Acknowledged)),
        Ok(CommandState::Acknowledged)
    );
    assert_eq!(ch.acknowledged(), 1);
    assert_eq!(ch.outstanding(), 0, "reconciled and pruned");
    assert!(ch.conserves(), "books stay balanced after reconciliation");
}

/// 04 — once faulted, further delivery is refused and writes nothing onto the torn stream.
#[test]
fn c04_delivery_after_a_fault_refuses_and_writes_nothing() {
    let mut ch = CommandChannel::new(1, 1);
    ch.issue_tracked(KIND, vec![]).unwrap();
    ch.deliver_next(&mut FailImmediately).unwrap_err(); // fault it

    // A second edit can still be issued (it just queues); delivery must refuse.
    ch.issue_tracked(KIND, vec![]).unwrap();
    assert_eq!(ch.pending(), 1, "the new edit is queued");

    let mut sink: Vec<u8> = Vec::new();
    let err = ch.deliver_next(&mut sink).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    assert!(sink.is_empty(), "nothing is appended to a faulted stream");
    assert_eq!(ch.pending(), 1, "the queued edit was NOT popped");
    assert_eq!(
        ch.transport_failed(),
        1,
        "the refusal is not a new transport failure"
    );
    assert!(ch.conserves());
}

/// 05 — out_seq advances by exactly one per *successful* delivery, and the frames carry 1,2,…
#[test]
fn c05_out_seq_advances_only_on_successful_delivery() {
    let mut ch = CommandChannel::new(1, 7);
    let mut good: Vec<u8> = Vec::new();

    for _ in 0..2 {
        ch.issue_tracked(KIND, vec![]).unwrap();
        ch.deliver_next(&mut good).unwrap().unwrap();
    }
    assert_eq!(ch.delivered_frames(), 2);
    assert_eq!(
        command_seqs(&good),
        vec![1, 2],
        "sequence is contiguous from 1"
    );

    // A third delivery tears: out_seq must stay at 2, not jump to 3.
    ch.issue_tracked(KIND, vec![]).unwrap();
    ch.deliver_next(&mut FailAfter::new(10)).unwrap_err();
    assert_eq!(
        ch.delivered_frames(),
        2,
        "a failed delivery does not burn a sequence number"
    );
    assert_eq!(ch.transport_failed(), 1);
    assert!(ch.conserves());
}

// ========== Group B — encode bounds (3D) ====================================================

/// 06 — an oversize arg is refused at issue, before either store is touched.
#[test]
fn c06_oversize_arg_is_refused_at_issue_touching_neither_store() {
    // Tight limits: a 64-byte payload ceiling → max arg = 64 - COMMAND_HEAD(16) = 48 bytes.
    let limits = WireLimits::new(64, 16);
    let max = limits.max_command_arg_bytes();
    let mut ch = CommandChannel::with_limits(9, 1, limits);

    let refusal = ch.issue_tracked(KIND, vec![0u8; max + 1]);
    assert_eq!(
        refusal,
        Err(LocalRefusal::TooLarge {
            arg_len: max + 1,
            max
        })
    );
    assert_eq!(ch.issued(), 0, "nothing issued");
    assert_eq!(ch.pending(), 0, "bridge untouched");
    assert_eq!(ch.outstanding(), 0, "ledger untouched");
    assert!(ch.conserves());
}

/// 07 — an arg exactly at the ceiling is accepted; one byte over is refused.
#[test]
fn c07_arg_at_the_cap_is_accepted_one_over_is_refused() {
    let limits = WireLimits::new(64, 16);
    let max = limits.max_command_arg_bytes();
    let mut ch = CommandChannel::with_limits(1, 1, limits);
    let mut sink: Vec<u8> = Vec::new();

    let id = ch
        .issue_tracked(KIND, vec![7u8; max])
        .expect("exactly at cap is fine");
    assert_eq!(ch.deliver_next(&mut sink).unwrap(), Some(id));

    assert_eq!(
        ch.issue_tracked(KIND, vec![7u8; max + 1]),
        Err(LocalRefusal::TooLarge {
            arg_len: max + 1,
            max
        })
    );
    assert!(ch.conserves());
}

// ========== Group C — acknowledgement lifecycle hostilities =================================

/// 08 — a duplicate ack for an already-resolved id binds to nothing (it was pruned).
#[test]
fn c08_duplicate_ack_after_resolution_is_unknown() {
    let mut ch = CommandChannel::new(1, 1);
    let mut sink: Vec<u8> = Vec::new();
    let id = ch.issue_tracked(KIND, vec![]).unwrap();
    ch.deliver_next(&mut sink).unwrap().unwrap();
    assert!(matches!(
        ch.resolve(1, ack(id, AckStatus::Acknowledged)),
        Ok(CommandState::Acknowledged)
    ));

    let dup = ch.resolve(1, ack(id, AckStatus::Acknowledged));
    assert!(matches!(
        dup,
        Err(ResolveError::Ledger(LedgerReject::UnknownCommand { command_id })) if command_id == id
    ));
    assert_eq!(ch.outstanding(), 0);
}

/// 09 — an ack for an id that was never issued is UnknownCommand, not a silent drop.
#[test]
fn c09_ack_for_an_unissued_id_is_unknown() {
    let mut ch = CommandChannel::new(1, 1);
    let bogus = ch.resolve(1, ack(999, AckStatus::Acknowledged));
    assert!(matches!(
        bogus,
        Err(ResolveError::Ledger(LedgerReject::UnknownCommand {
            command_id: 999
        }))
    ));
    assert!(ch.conserves());
}

/// 10 — an ack for an issued-but-undelivered command is refused (you cannot verdict the unsent).
#[test]
fn c10_ack_before_delivery_is_refused() {
    let mut ch = CommandChannel::new(1, 1);
    let id = ch.issue_tracked(KIND, vec![]).unwrap(); // issued, NOT delivered
    let early = ch.resolve(1, ack(id, AckStatus::Acknowledged));
    assert!(matches!(
        early,
        Err(ResolveError::Ledger(LedgerReject::AckBeforeDelivery { command_id })) if command_id == id
    ));
    assert_eq!(
        ch.outstanding(),
        1,
        "still outstanding, not falsely resolved"
    );
}

/// 11 — a non-terminal ack status on the return channel is refused.
#[test]
fn c11_non_terminal_ack_status_is_refused() {
    let mut ch = CommandChannel::new(1, 1);
    let mut sink: Vec<u8> = Vec::new();
    let id = ch.issue_tracked(KIND, vec![]).unwrap();
    ch.deliver_next(&mut sink).unwrap().unwrap();

    for bad in [AckStatus::Accepted, AckStatus::Delivered] {
        let r = ch.resolve(1, ack(id, bad));
        assert!(
            matches!(
                r,
                Err(ResolveError::Ledger(LedgerReject::NonTerminalAck { .. }))
            ),
            "status {bad:?} must be refused as non-terminal, got {r:?}"
        );
    }
    assert_eq!(
        ch.outstanding(),
        1,
        "the command is unresolved, still awaiting a real verdict"
    );
}

/// 12 — an ack arriving on a frame from the wrong source cannot resolve the command.
#[test]
fn c12_wrong_source_ack_is_refused_then_the_right_one_resolves() {
    let mut ch = CommandChannel::new(7, 1);
    let mut sink: Vec<u8> = Vec::new();
    let id = ch.issue_tracked(KIND, vec![]).unwrap();
    ch.deliver_next(&mut sink).unwrap().unwrap();

    match ch.resolve(9, ack(id, AckStatus::Acknowledged)) {
        Err(ResolveError::WrongSource {
            expected: 7,
            got: 9,
        }) => {}
        other => panic!("wrong-source ack must be refused, got {other:?}"),
    }
    assert_eq!(ch.outstanding(), 1);
    assert!(matches!(
        ch.resolve(7, ack(id, AckStatus::Acknowledged)),
        Ok(CommandState::Acknowledged)
    ));
    assert_eq!(ch.outstanding(), 0);
}

/// 13 — a Refused verdict is terminal and frees the slot; the retry mints a fresh monotonic id.
#[test]
fn c13_refused_verdict_frees_the_slot_and_is_reissuable() {
    let mut ch = CommandChannel::new(1, 1);
    let mut sink: Vec<u8> = Vec::new();
    let id = ch.issue_tracked(KIND, vec![]).unwrap();
    ch.deliver_next(&mut sink).unwrap().unwrap();

    assert!(matches!(
        ch.resolve(1, ack(id, AckStatus::Refused)),
        Ok(CommandState::Refused)
    ));
    assert_eq!(ch.refused(), 1);
    assert_eq!(ch.acknowledged(), 0, "a refusal is not an acknowledgement");
    assert_eq!(ch.outstanding(), 0, "the slot is freed");

    let id2 = ch.issue_tracked(KIND, vec![]).unwrap();
    assert!(id2 > id, "retry uses a fresh monotonic id");
    assert!(ch.conserves());
}

/// 14 — a Failed verdict is terminal and is never counted as a success.
#[test]
fn c14_failed_verdict_is_terminal_and_not_a_success() {
    let mut ch = CommandChannel::new(1, 1);
    let mut sink: Vec<u8> = Vec::new();
    let id = ch.issue_tracked(KIND, vec![]).unwrap();
    ch.deliver_next(&mut sink).unwrap().unwrap();

    assert!(matches!(
        ch.resolve(1, ack(id, AckStatus::Failed)),
        Ok(CommandState::Failed)
    ));
    assert_eq!(ch.failed(), 1);
    assert_eq!(ch.acknowledged(), 0);
    assert_eq!(ch.refused(), 0);
    assert_eq!(ch.outstanding(), 0);
    assert!(ch.conserves());
}

// ========== Group D — saturation (bounded resources, L7) ====================================

/// 15 — filling the bridge's pending queue (issue without delivering) refuses, never grows.
#[test]
fn c15_bridge_pending_saturation_refuses_without_growth() {
    let mut ch = CommandChannel::new(1, 1);
    // Issue (but never deliver) up to the shared cap: bridge pending climbs to COMMAND_CAP.
    for _ in 0..channel::COMMAND_CAP {
        ch.issue_tracked(KIND, vec![]).unwrap();
    }
    assert_eq!(ch.pending(), channel::COMMAND_CAP);

    // The next issue is refused as BridgeFull (the bridge cap is hit before the ledger's here,
    // because nothing has been delivered yet so outstanding == pending == cap simultaneously;
    // the ledger check runs first, so it surfaces as LedgerFull — either way it is refused and
    // nothing grows). Assert the refusal is one of the two bounded refusals, and nothing grew.
    let refusal = ch.issue_tracked(KIND, vec![]);
    assert!(
        matches!(
            refusal,
            Err(LocalRefusal::LedgerFull | LocalRefusal::BridgeFull)
        ),
        "saturation must refuse, got {refusal:?}"
    );
    assert_eq!(
        ch.pending(),
        channel::COMMAND_CAP,
        "queue did not grow past the cap"
    );
    assert!(ch.conserves());
}

/// 16 — deliver-without-ack to the cap fills the ledger's outstanding set and then refuses.
#[test]
fn c16_ledger_outstanding_saturation_refuses_without_desync() {
    let mut ch = CommandChannel::new(1, 1);
    let mut sink: Vec<u8> = Vec::new();
    for _ in 0..channel::COMMAND_CAP {
        let id = ch.issue_tracked(KIND, vec![]).unwrap();
        assert_eq!(ch.deliver_next(&mut sink).unwrap(), Some(id));
    }
    assert_eq!(ch.outstanding(), channel::COMMAND_CAP);
    assert_eq!(ch.pending(), 0, "all delivered, nothing queued");

    assert_eq!(
        ch.issue_tracked(KIND, vec![]),
        Err(LocalRefusal::LedgerFull)
    );
    assert_eq!(ch.outstanding(), channel::COMMAND_CAP);
    assert!(ch.conserves(), "stores still agree at the ceiling");
}

// ========== Group E — cross-store conservation under hostility (3C) =========================

/// 17 — conservation holds after *every* step of a hostile interleaving that mixes issue,
/// deliver, acknowledge, refuse, fail, and a terminal transport fault.
#[test]
fn c17_conservation_holds_across_a_hostile_interleaving() {
    let mut ch = CommandChannel::new(3, 2);
    let mut sink: Vec<u8> = Vec::new();
    assert!(ch.conserves());

    let a = ch.issue_tracked(KIND, vec![1]).unwrap();
    assert!(ch.conserves());
    let b = ch.issue_tracked(KIND, vec![2]).unwrap();
    assert!(ch.conserves());

    ch.deliver_next(&mut sink).unwrap().unwrap(); // a delivered
    assert!(ch.conserves());
    ch.resolve(3, ack(a, AckStatus::Acknowledged)).unwrap();
    assert!(ch.conserves());

    ch.deliver_next(&mut sink).unwrap().unwrap(); // b delivered
    assert!(ch.conserves());
    ch.resolve(3, ack(b, AckStatus::Refused)).unwrap();
    assert!(ch.conserves());

    let c = ch.issue_tracked(KIND, vec![3]).unwrap();
    ch.deliver_next(&mut sink).unwrap().unwrap(); // c delivered
    assert!(ch.conserves());
    ch.resolve(3, ack(c, AckStatus::Failed)).unwrap();
    assert!(ch.conserves());

    // Now a terminal transport fault mid-stream.
    ch.issue_tracked(KIND, vec![4]).unwrap();
    ch.deliver_next(&mut FailAfter::new(10)).unwrap_err();
    assert!(ch.conserves(), "conservation survives the fault");

    assert_eq!(ch.acknowledged(), 1);
    assert_eq!(ch.refused(), 1);
    assert_eq!(ch.failed(), 1);
    assert_eq!(ch.transport_failed(), 1);
    assert!(ch.faulted());
}

/// 18 — after a fault the public tallies tell the exact truth: the three delivered frames are
/// committed (out_seq == 3), the torn one is counted as a transport failure, and the channel is
/// faulted — and `conserves()` (which checks the cross-store delivered identity internally)
/// is true the whole way.
#[test]
fn c18_post_fault_tallies_are_exact_and_reconciled() {
    let mut ch = CommandChannel::new(1, 1);
    let mut sink: Vec<u8> = Vec::new();

    for _ in 0..3 {
        ch.issue_tracked(KIND, vec![]).unwrap();
        ch.deliver_next(&mut sink).unwrap().unwrap();
    }
    // Resolve one so delivered_ever (3) and resolved (1) differ from outstanding.
    ch.resolve(1, ack(1, AckStatus::Acknowledged)).unwrap();
    assert_eq!(ch.delivered_frames(), 3);
    assert_eq!(ch.transport_failed(), 0);
    assert!(ch.conserves());

    // Fault on the fourth.
    ch.issue_tracked(KIND, vec![]).unwrap();
    ch.deliver_next(&mut FailImmediately).unwrap_err();

    assert_eq!(
        ch.delivered_frames(),
        3,
        "only the three that landed are committed"
    );
    assert_eq!(
        ch.transport_failed(),
        1,
        "the torn fourth is counted, not hidden"
    );
    assert!(ch.faulted());
    assert_eq!(ch.acknowledged(), 1);
    assert!(
        ch.conserves(),
        "bridge.delivered == ledger.delivered_ever + transport_failed holds after the fault"
    );
    assert!(
        ch.transport_failed() <= 1,
        "the fault latch pins transport_failed at most 1 (no second pop after a fault)"
    );
}

// ========== Group F — regressions from the adversarial pass =================================

/// 19 — REGRESSION (adversary, encode-bounds lens). A payload budget smaller than a single
/// command header (`max_payload_bytes < COMMAND_HEAD_BYTES = 16`) must refuse *even the empty
/// command* at issue, and must never be reported delivered. The old `arg.len() > max_command_arg_
/// bytes()` gate saturated `max` to 0 and admitted the empty arg, emitting a 16-byte-payload frame
/// over the channel's own cap; the total-payload guard refuses it.
#[test]
fn c19_subheader_payload_budget_refuses_even_the_empty_command() {
    let limits = WireLimits::new(8, 0); // 8-byte payload budget — smaller than the 16-byte head
    assert_eq!(
        limits.max_command_arg_bytes(),
        0,
        "saturating_sub floors to 0"
    );

    // Library layer: the checked encoder refuses the empty command (its documented contract).
    let empty = WireCommand {
        command_id: 1,
        kind: KIND,
        arg: vec![],
    };
    assert_eq!(
        encode_command_checked(1, SourceCursor::new(1, 1), &empty, limits),
        Err(WireError::EncodeTooLarge),
        "no command fits under a sub-header payload budget"
    );

    // Channel layer inherits the fix: issue is refused, and nothing is ever delivered.
    let mut ch = CommandChannel::with_limits(1, 1, limits);
    let refusal = ch.issue_tracked(KIND, vec![]);
    assert!(
        matches!(refusal, Err(LocalRefusal::TooLarge { .. })),
        "the empty command must be refused at issue, got {refusal:?}"
    );
    assert_eq!(ch.issued(), 0);
    assert_eq!(ch.pending(), 0);
    assert_eq!(ch.delivered_frames(), 0);
    assert!(ch.conserves());

    // Any realistic budget (>= the 16-byte head) still accepts an empty command.
    let mut ok = CommandChannel::with_limits(1, 1, WireLimits::new(16, 0));
    let mut sink: Vec<u8> = Vec::new();
    let id = ok
        .issue_tracked(KIND, vec![])
        .expect("an exact-header budget fits the empty command");
    assert_eq!(ok.deliver_next(&mut sink).unwrap(), Some(id));
}

/// 20 — a writer that always reports `Ok(0)` must fault (via `write_all`'s `WriteZero`), not hang
/// and not falsely deliver. Covers the std write-contract path the honest writers never hit.
#[test]
fn c20_a_write_that_makes_no_progress_faults() {
    let mut ch = CommandChannel::new(1, 1);
    ch.issue_tracked(KIND, vec![]).unwrap();

    let err = ch.deliver_next(&mut WriteZero).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::WriteZero);
    assert!(ch.faulted());
    assert_eq!(ch.transport_failed(), 1);
    assert_eq!(ch.delivered_frames(), 0);
    assert_eq!(ch.outstanding(), 1);
    assert!(ch.conserves());
}
