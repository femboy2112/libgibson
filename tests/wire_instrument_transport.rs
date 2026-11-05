//! Bounded-transport proofs for `wire_instrument` (Renaissance VII, Gate B).
//!
//! These drive the application-owned transport module directly — no subprocess, no clock — and
//! prove the three Gate B obligations against the REAL [`FrameDecoder`]:
//!
//!   * **2B** — the original drain idiom (push every queued chunk into the decoder *before*
//!     draining a message) trips a spurious **fatal** `BufferOverflow` on perfectly legal input
//!     once a backlog sums past the decoder's one-frame ceiling; [`FramePump`] interleaves
//!     parse with accumulation and cannot (`naive_push_all_*` RED, `frame_pump_*` GREEN).
//!   * **2A** — a flood of field snapshots is bounded to one pending frame per source by lawful
//!     coalescing, and a conformant producer's control messages are never dropped
//!     (`coalescing_*`).
//!   * **2C** — the reader drains its source to completion WITHOUT a consumer, so it never blocks
//!     the producer's stdout (the property that makes the full-duplex command-write deadlock
//!     impossible), while memory stays bounded (`reader_drains_without_a_consumer_*`).

#[path = "../examples/wire_instrument/transport.rs"]
mod transport;

use gibson::field_view::FieldExtent;
use gibson::shape::Point;
use gibson::wire::{
    encode_field, AckStatus, FieldAdmission, FieldInbox, FrameDecoder, SourceCursor, WireAck,
    WireBody, WireError, WireField, WireLimits, WireMessage, HEADER_BYTES,
};

use transport::{
    CoalescingBuffer, ControlKind, FramePump, ProducerOwner, Reader, DEFAULT_CONTROL_CAP,
};

/// A synthetic RETIRE message (unit body; never coalesceable).
fn retire_msg(source: u64, epoch: u32, seq: u64) -> WireMessage {
    WireMessage {
        source,
        cursor: SourceCursor::new(epoch, seq),
        body: WireBody::Retire,
    }
}

/// A synthetic COMMAND message (never coalesceable).
fn command_msg(source: u64, epoch: u32, seq: u64, command_id: u64) -> WireMessage {
    use gibson::wire::WireCommand;
    WireMessage {
        source,
        cursor: SourceCursor::new(epoch, seq),
        body: WireBody::Command(WireCommand {
            command_id,
            kind: 1,
            arg: vec![],
        }),
    }
}

// ---- builders -------------------------------------------------------------------------

fn gradient_field(cols: usize, rows: usize) -> WireField {
    let extent = FieldExtent::new(Point::new(-1.0, 2.0), 0.5, 0.25, cols, rows).expect("extent");
    let data: Vec<f64> = (0..cols * rows).map(|i| i as f64).collect();
    WireField {
        extent,
        model_tick: 0,
        data,
        mask: None,
    }
}

/// One legal FIELD frame for `source` at `(epoch, seq)` under `limits`.
fn field_frame(source: u64, epoch: u32, seq: u64, limits: WireLimits) -> Vec<u8> {
    encode_field(
        source,
        SourceCursor::new(epoch, seq),
        &gradient_field(4, 4),
        limits,
    )
    .expect("encode")
}

/// A synthetic ACK message to offer directly (no wire round-trip needed to test ordering).
fn ack_msg(source: u64, epoch: u32, seq: u64, command_id: u64) -> WireMessage {
    WireMessage {
        source,
        cursor: SourceCursor::new(epoch, seq),
        body: WireBody::Ack(WireAck {
            command_id,
            status: AckStatus::Acknowledged,
        }),
    }
}

fn field_msg(source: u64, epoch: u32, seq: u64) -> WireMessage {
    WireMessage {
        source,
        cursor: SourceCursor::new(epoch, seq),
        body: WireBody::Field(gradient_field(4, 4)),
    }
}

/// Tight limits whose decoder buffer ceiling is exactly one `field_frame`, so a backlog of two
/// frames pushed before a drain must overflow.
fn one_frame_limits() -> (WireLimits, Vec<u8>) {
    let probe = field_frame(7, 0, 1, WireLimits::default());
    let payload = probe.len() - HEADER_BYTES;
    // ceiling = HEADER_BYTES + max_payload_bytes == probe.len(); 16 cells fits a 4x4 field.
    (WireLimits::new(payload, 16), probe)
}

// ---- 2B: the drain-idiom bug, and the pump that fixes it ------------------------------

/// RED — documents the original defect. Pushing a two-frame backlog into the decoder *before*
/// draining (exactly what `run_interactive`'s old loop did across a `try_recv` burst) trips a
/// fatal `BufferOverflow`, even though every frame is individually legal and small.
#[test]
fn naive_push_all_before_drain_trips_spurious_overflow_on_legal_input() {
    let (tight, _) = one_frame_limits();
    let f1 = field_frame(7, 0, 1, tight);
    let f2 = field_frame(7, 0, 2, tight);

    let mut dec = FrameDecoder::with_limits(tight);
    // First chunk fits (empty buffer + one frame == ceiling).
    assert!(dec.push(&f1).is_ok());
    // Second chunk, pushed before any drain, sums to two frames > ceiling → fatal overflow.
    let err = dec
        .push(&f2)
        .expect_err("backlog must overflow the one-frame ceiling");
    assert!(matches!(err, WireError::BufferOverflow { .. }));
    assert!(err.is_fatal(), "a BufferOverflow poisons the decoder");
}

/// GREEN — the same two legal frames through the pump: one chunk pushed, drained, then the next.
/// The decoder never holds more than one frame, so the overflow cannot fire.
#[test]
fn frame_pump_interleaves_parse_and_drain_so_a_backlog_never_overflows() {
    let (tight, _) = one_frame_limits();
    let f1 = field_frame(7, 0, 1, tight);
    let f2 = field_frame(7, 0, 2, tight);

    let mut pump = FramePump::with_decoder(FrameDecoder::with_limits(tight));
    let mut got: Vec<WireMessage> = Vec::new();

    pump.pump(&f1, &mut |m| got.push(m)).expect("f1 decodes");
    assert_eq!(
        pump.buffered_bytes(),
        0,
        "drained to empty after a whole frame"
    );
    pump.pump(&f2, &mut |m| got.push(m)).expect("f2 decodes");
    assert_eq!(pump.buffered_bytes(), 0);

    assert_eq!(got.len(), 2);
    assert_eq!(got[0].cursor.sequence, 1);
    assert_eq!(got[1].cursor.sequence, 2);
}

/// The pump handles arbitrary chunk boundaries (a read can split a frame anywhere) on a long
/// legal stream, decoding every frame in order while the decoder buffer stays under one frame.
#[test]
fn frame_pump_handles_arbitrary_chunk_boundaries_with_a_bounded_buffer() {
    const N: u64 = 200;
    let limits = WireLimits::default();
    let one = field_frame(7, 0, 1, limits).len();

    // Concatenate N frames, then re-chop at a size that splits frames at arbitrary points.
    let mut stream = Vec::new();
    for seq in 1..=N {
        stream.extend_from_slice(&field_frame(7, 0, seq, limits));
    }

    let mut pump = FramePump::new();
    let mut seqs: Vec<u64> = Vec::new();
    for chunk in stream.chunks(97) {
        pump.pump(chunk, &mut |m| seqs.push(m.cursor.sequence))
            .expect("legal stream never errors");
        assert!(
            pump.buffered_bytes() < one,
            "buffer holds at most one partial frame ({} < {})",
            pump.buffered_bytes(),
            one
        );
    }
    assert_eq!(seqs.len(), N as usize);
    assert!(seqs.iter().copied().eq(1..=N), "every frame, in order");
}

/// Renaissance VIII, Gate B. A single legal read that straddles a frame boundary *at the one-frame
/// ceiling* — the tail of a maximum-size frame plus the head of the next — must not trip a fatal
/// overflow. RED on the pre-fix pump (it pushed the whole chunk, so `buffered + chunk` exceeded the
/// ceiling before frame 1 could be drained to free room); GREEN once the pump feeds incrementally
/// bounded by the decoder's `remaining_capacity`, draining between.
#[test]
fn frame_pump_survives_a_straddle_at_the_one_frame_ceiling() {
    let (tight, _) = one_frame_limits();
    let f1 = field_frame(7, 0, 1, tight);
    let f2 = field_frame(7, 0, 2, tight);
    let f = f1.len();

    // chunk 1: all of f1 except its final byte. chunk 2: f1's final byte + all of f2 — a legal
    // read whose (buffered f-1) + (1 + f) exceeds the one-frame ceiling of f.
    let c1 = f1[..f - 1].to_vec();
    let mut c2 = vec![f1[f - 1]];
    c2.extend_from_slice(&f2);

    let mut pump = FramePump::with_decoder(FrameDecoder::with_limits(tight));
    let mut got: Vec<WireMessage> = Vec::new();
    pump.pump(&c1, &mut |m| got.push(m))
        .expect("the partial first frame just buffers");
    pump.pump(&c2, &mut |m| got.push(m))
        .expect("a legal straddle at the ceiling must decode, not overflow");

    assert_eq!(got.len(), 2, "both legal frames decoded");
    assert_eq!(got[0].cursor.sequence, 1);
    assert_eq!(got[1].cursor.sequence, 2);
    assert_eq!(
        pump.buffered_bytes(),
        0,
        "drained to empty after the second frame"
    );
}

/// A genuinely oversized DECLARED frame still poisons fatally through the incremental pump — the
/// straddle fix must not blunt the real overflow guard. A frame declaring a payload past the tight
/// limit is fatal `FrameTooLarge` (the decoder refuses to buffer toward a frame it would reject).
#[test]
fn frame_pump_still_rejects_a_genuinely_oversized_declared_frame() {
    // Tight limits sized to a 4x4 field; an 8x8 field's declared payload (64 cells) far exceeds
    // that cap, so the decoder must poison fatally even though the bytes are well-formed.
    let (tight, _) = one_frame_limits();
    let oversized = encode_field(
        7,
        SourceCursor::new(0, 1),
        &gradient_field(8, 8),
        WireLimits::default(),
    )
    .expect("encode a large but legal field under default limits");
    assert!(
        oversized.len() - HEADER_BYTES > tight.max_payload_bytes,
        "the 8x8 payload must exceed the tight cap for this test to mean anything"
    );

    let mut pump = FramePump::with_decoder(FrameDecoder::with_limits(tight));
    let err = pump
        .pump(&oversized, &mut |_| {})
        .expect_err("a frame declaring more than the cap is fatal");
    assert!(
        err.is_fatal(),
        "oversized declared payload poisons the decoder"
    );
    assert!(matches!(err, WireError::FrameTooLarge { .. }));
}

// ---- 2A: coalescing bounds a snapshot flood; control is never silently lost ----------

#[test]
fn coalescing_collapses_a_same_source_field_flood_to_one() {
    let mut buf = CoalescingBuffer::default();
    for seq in 1..=10_000u64 {
        buf.offer(field_msg(7, 0, seq));
    }
    assert_eq!(
        buf.len(),
        1,
        "a run of same-source snapshots is one pending frame"
    );
    assert_eq!(buf.coalesced(), 9_999);
    assert_eq!(
        buf.fields_dropped(),
        0,
        "coalescing is not dropping — nothing was lost at a cap"
    );

    let mut drained: Vec<WireMessage> = Vec::new();
    buf.drain_into(&mut |m| drained.push(m));
    assert_eq!(drained.len(), 1);
    assert_eq!(
        drained[0].cursor.sequence, 10_000,
        "the newest snapshot survives"
    );
}

#[test]
fn coalescing_never_reorders_or_collapses_fields_across_a_control_message() {
    let mut buf = CoalescingBuffer::default();
    buf.offer(field_msg(7, 0, 1));
    buf.offer(field_msg(7, 0, 2)); // coalesces onto seq 1 -> newest (seq 2)
    buf.offer(ack_msg(7, 0, 3, 42)); // control: breaks the run
    buf.offer(field_msg(7, 0, 4)); // a new field run after the ack
    buf.offer(field_msg(7, 0, 5)); // coalesces onto seq 4 -> newest (seq 5)

    assert_eq!(
        buf.len(),
        3,
        "field / ack / field — the ack is never collapsed away"
    );
    assert_eq!(buf.coalesced(), 2);

    let mut kinds: Vec<&'static str> = Vec::new();
    let mut seqs: Vec<u64> = Vec::new();
    buf.drain_into(&mut |m| {
        seqs.push(m.cursor.sequence);
        kinds.push(match m.body {
            WireBody::Field(_) => "field",
            WireBody::Ack(_) => "ack",
            WireBody::Retire => "retire",
            WireBody::Command(_) => "command",
        });
    });
    assert_eq!(
        kinds,
        ["field", "ack", "field"],
        "order preserved across the control message"
    );
    assert_eq!(
        seqs,
        [2, 3, 5],
        "each surviving field is the newest of its run"
    );
}

/// A snapshot of one source never collapses onto another's: each source keeps its own latest.
#[test]
fn distinct_sources_each_keep_their_latest_snapshot() {
    let mut buf = CoalescingBuffer::default();
    // Source 1 sends a run (collapses to its newest), then source 2 does the same.
    buf.offer(field_msg(1, 0, 1));
    buf.offer(field_msg(1, 0, 2));
    buf.offer(field_msg(1, 0, 3)); // -> one pending s1 @ seq 3
    buf.offer(field_msg(2, 0, 7)); // different source: a new entry, not a collapse
    buf.offer(field_msg(2, 0, 8)); // -> one pending s2 @ seq 8

    assert_eq!(buf.len(), 2, "one pending snapshot per source");
    assert_eq!(buf.fields_dropped(), 0);

    let mut out: Vec<(u64, u64)> = Vec::new();
    buf.drain_into(&mut |m| out.push((m.source, m.cursor.sequence)));
    assert_eq!(
        out,
        [(1, 3), (2, 8)],
        "each source's latest, in arrival order"
    );
}

/// Renaissance VIII, Gate A. A control flood past the reserved control lane must NOT silently
/// continue behind a drop counter — counting a lost ACK is not preserving command reliability.
/// It fails CLOSED: the buffer latches a typed terminal [`ControlOverflow`], and that state is
/// sticky across a drain so the consumer cannot resume treating the stream as healthy.
///
/// (This replaces the Renaissance VII test that *accepted* the defect by asserting the dropped
/// ACKs were merely counted — a safeguard that certified the bug.)
#[test]
fn control_flood_past_the_lane_fails_closed_not_silently_dropped() {
    let cap = 8;
    let mut buf = CoalescingBuffer::with_control_cap(cap);
    for id in 0..(cap as u64) * 2 {
        buf.offer(ack_msg(7, 0, id, id)); // acks are never coalesceable
    }
    assert_eq!(buf.coalesced(), 0, "control messages are never collapsed");
    assert_eq!(
        buf.control_len(),
        cap,
        "admitted exactly the reserved control lane"
    );
    let ov = buf
        .control_overflow()
        .expect("a control flood past the lane latches a terminal overflow");
    assert_eq!(ov.control_cap, cap);
    assert_eq!(
        ov.kind,
        ControlKind::Ack,
        "the overflowing kind is disclosed"
    );

    // Sticky: draining does not clear the terminal state.
    buf.drain_into(&mut |_| {});
    assert!(
        buf.control_overflow().is_some(),
        "a terminal control overflow survives a drain — the stream stays failed-closed"
    );
    // And post-overflow offers are refused, never masquerading as part of a healthy stream.
    buf.offer(ack_msg(7, 0, 999, 999));
    assert!(
        buf.is_empty(),
        "nothing is admitted after the stream failed closed"
    );
}

/// Gate A minimal counterexample, repaired. Two distinct-source fields fill the field lane; an
/// authoritative ACK that arrives next must still be admitted — a field flood can never cost a
/// verdict. (RED on the pre-fix single-cap buffer: the ACK was dropped at the cap.)
#[test]
fn an_ack_is_admitted_even_when_the_field_lane_is_full() {
    let mut buf = CoalescingBuffer::with_caps(2, 4); // field_cap 2, control_cap 4
    buf.offer(field_msg(1, 0, 1));
    buf.offer(field_msg(2, 0, 1)); // field lane now full (2 distinct sources)
    buf.offer(field_msg(3, 0, 1)); // over the field cap: dropped as an expendable snapshot
    buf.offer(ack_msg(1, 0, 2, 7)); // the control lane has its own reserved room

    assert_eq!(buf.field_len(), 2, "field lane bounded at its own cap");
    assert_eq!(
        buf.fields_dropped(),
        1,
        "the over-cap snapshot is a counted, lawful drop"
    );
    assert_eq!(
        buf.control_len(),
        1,
        "the ACK is admitted, not starved by the field flood"
    );
    assert!(
        buf.control_overflow().is_none(),
        "no terminal state — this is in-contract"
    );

    let mut acks = 0u32;
    buf.drain_into(&mut |m| {
        if matches!(m.body, WireBody::Ack(_)) {
            acks += 1;
        }
    });
    assert_eq!(acks, 1, "the authoritative ACK survived to the consumer");
}

/// Conservation over a mixed multi-source stream: every authoritative VERDICT (ACK/RETIRE) is
/// preserved and in order around coalesced field runs. An inbound COMMAND is rejected as
/// out-of-contract (COMMAND is presenter -> producer only; see WIRE_PROTOCOL.md) rather than
/// conserved as a verdict — the bound it enforces is proven in
/// `an_inbound_command_flood_is_rejected_and_byte_bounded`.
#[test]
fn acks_and_retires_survive_a_mixed_stream_while_inbound_commands_are_rejected() {
    let mut buf = CoalescingBuffer::with_caps(64, 64);
    // A run from source 1, an ack for it, a retire of source 2, an (out-of-contract) inbound
    // command, a fresh field run.
    buf.offer(field_msg(1, 0, 1));
    buf.offer(field_msg(1, 0, 2)); // coalesces onto the previous s1 field
    buf.offer(ack_msg(1, 0, 3, 100)); // verdict: breaks the run
    buf.offer(retire_msg(2, 0, 4)); // verdict
    buf.offer(command_msg(1, 0, 5, 101)); // OUT OF CONTRACT inbound (presenter -> producer only)
    buf.offer(field_msg(3, 0, 6));
    buf.offer(field_msg(3, 0, 7)); // coalesces onto the previous s3 field

    assert_eq!(buf.coalesced(), 2, "only same-source field runs collapsed");
    assert_eq!(
        buf.out_of_contract(),
        1,
        "the inbound COMMAND is rejected as out-of-contract, not conserved"
    );
    assert!(buf.control_overflow().is_none());

    let mut kinds: Vec<&'static str> = Vec::new();
    buf.drain_into(&mut |m| {
        kinds.push(match m.body {
            WireBody::Field(_) => "field",
            WireBody::Ack(_) => "ack",
            WireBody::Retire => "retire",
            WireBody::Command(_) => "command",
        });
    });
    assert_eq!(
        kinds,
        ["field", "ack", "retire", "field"],
        "ACK/RETIRE verdicts survive in order; the out-of-contract COMMAND is never admitted"
    );
}

/// Gate B/B2 — an inbound COMMAND flood is rejected and byte-bounded. COMMAND is presenter ->
/// producer only, so a producer flooding large-`arg` COMMAND frames back at the presenter is sending
/// them the WRONG way. Each must be rejected out-of-contract: it consumes neither a verdict slot in
/// the count-bounded control lane nor any retained bytes. This closes the hole where a count-only
/// control cap left variable COMMAND payloads unbounded (control_cap x max_arg_bytes of memory).
#[test]
fn an_inbound_command_flood_is_rejected_and_byte_bounded() {
    use gibson::wire::WireCommand;
    let mut buf = CoalescingBuffer::with_caps(4, 4); // tiny lanes — any byte leak would show
    let big_arg = vec![0u8; 1_000_000]; // 1 MB arg per command
    for id in 0..100u64 {
        buf.offer(WireMessage {
            source: 1,
            cursor: SourceCursor::new(0, id + 1),
            body: WireBody::Command(WireCommand {
                command_id: id,
                kind: 1,
                arg: big_arg.clone(),
            }),
        });
    }
    assert_eq!(
        buf.out_of_contract(),
        100,
        "every inbound COMMAND is rejected and counted"
    );
    assert_eq!(
        buf.control_len(),
        0,
        "no inbound COMMAND consumes the ACK/RETIRE verdict lane"
    );
    assert_eq!(
        buf.field_bytes(),
        0,
        "an inbound COMMAND flood retains ZERO bytes — the lane holds only zero-payload verdicts"
    );
    assert!(
        buf.control_overflow().is_none(),
        "out-of-contract rejection is not a fail-closed verdict overflow"
    );
    assert_eq!(buf.len(), 0, "nothing is queued");
}

/// Mutation guard: control admitted up to EXACTLY the lane cap is never lost and never latches a
/// premature overflow. A mutation that dropped a control message while headroom remained would
/// make this red.
#[test]
fn control_is_never_lost_while_the_lane_has_headroom() {
    let cap = 16;
    let mut buf = CoalescingBuffer::with_control_cap(cap);
    for id in 0..cap as u64 {
        buf.offer(ack_msg(7, 0, id, id));
    }
    assert_eq!(
        buf.control_len(),
        cap,
        "every in-lane control message retained"
    );
    assert!(
        buf.control_overflow().is_none(),
        "no overflow while the lane is exactly full but not exceeded"
    );
    let mut got = 0u32;
    buf.drain_into(&mut |_| got += 1);
    assert_eq!(got as usize, cap, "all admitted control messages delivered");
}

// ---- 2C: the reader never blocks on a consumer (deadlock-freedom) ---------------------

/// The decisive 2C property. A reader fed a large single-source field flood with **no** UI
/// draining it runs the source to EOF and returns — it does not block waiting for a consumer.
/// (A blocking-bounded design would hang here forever.) Memory stays at one pending frame.
#[test]
fn reader_drains_without_a_consumer_and_stays_bounded() {
    const N: u64 = 5_000;
    let limits = WireLimits::default();
    let mut stream = Vec::new();
    for seq in 1..=N {
        stream.extend_from_slice(&field_frame(7, 0, seq, limits));
    }

    let mut reader = Reader::spawn(std::io::Cursor::new(stream), DEFAULT_CONTROL_CAP);
    reader.join(); // if the reader blocked on a bound, this would never return

    assert_eq!(
        reader.buffered_len(),
        1,
        "bounded: one coalesced pending snapshot"
    );
    assert_eq!(reader.coalesced(), N - 1);

    let mut drained: Vec<WireMessage> = Vec::new();
    let outcome = reader.drain(&mut |m| drained.push(m));
    assert!(
        outcome.disconnected,
        "EOF on the source is a clean disconnect"
    );
    assert!(outcome.fatal.is_none());
    assert_eq!(drained.len(), 1);
    assert_eq!(
        drained[0].cursor.sequence, N,
        "the latest snapshot is delivered"
    );
}

/// Distinct sources are each preserved (no false coalescing) and all delivered after EOF.
#[test]
fn reader_preserves_distinct_sources_through_eof() {
    let limits = WireLimits::default();
    let mut stream = field_frame(1, 0, 10, limits);
    stream.extend_from_slice(&field_frame(2, 0, 20, limits));

    let mut reader = Reader::spawn(std::io::Cursor::new(stream), DEFAULT_CONTROL_CAP);
    reader.join();

    let mut sources: Vec<u64> = Vec::new();
    let outcome = reader.drain(&mut |m| sources.push(m.source));
    assert!(outcome.disconnected);
    sources.sort_unstable();
    assert_eq!(sources, [1, 2]);
}

/// Renaissance VIII, Gate C2. The reader must run the caller's sink WITHOUT holding the shared
/// mutex, so a reentrant sink (one that calls back into a `Reader` accessor) cannot deadlock on
/// the non-reentrant `std::sync::Mutex`. The drain runs on a worker thread with a wall-clock
/// deadline — if the lock were held across the sink, the reentrant `buffered_len()` would deadlock
/// and the deadline would elapse. (Pre-fix: `drain_into(sink)` ran under the lock → hang.)
#[test]
fn drain_invokes_a_reentrant_sink_without_deadlocking() {
    use std::sync::mpsc;
    use std::time::Duration;

    let limits = WireLimits::default();
    let mut stream = field_frame(1, 0, 1, limits);
    stream.extend_from_slice(&field_frame(2, 0, 1, limits));
    let mut reader = Reader::spawn(std::io::Cursor::new(stream), DEFAULT_CONTROL_CAP);
    reader.join();

    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut delivered = 0u32;
        reader.drain(&mut |_m| {
            // Reentrant accessor from inside the sink: re-locks the same mutex. Safe only because
            // the sink now runs after the lock is released.
            let _ = reader.buffered_len();
            delivered += 1;
        });
        tx.send(delivered).unwrap();
    });

    let delivered = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("a reentrant sink must complete, not deadlock under the reader's mutex");
    worker.join().unwrap();
    assert_eq!(
        delivered, 2,
        "both coalesced snapshots were delivered to the sink"
    );
}

/// A corrupt stream poisons the decoder; the reader surfaces the fatal error rather than hiding
/// it or panicking.
#[test]
fn reader_surfaces_a_fatal_decode() {
    // 64 bytes of non-magic garbage: enough for a header attempt, wrong magic → fatal BadMagic.
    let mut reader = Reader::spawn(std::io::Cursor::new(vec![0xABu8; 64]), DEFAULT_CONTROL_CAP);
    reader.join();

    let outcome = reader.drain(&mut |_| {});
    match outcome.fatal {
        Some(WireError::BadMagic { .. }) => {}
        other => panic!("expected a fatal BadMagic, got {other:?}"),
    }
}

// ---- Gate E: supervised process ownership -------------------------------------------------

/// GATE E — a [`ProducerOwner`]'s `Drop` reaps the child through the OS `wait` handle and joins the
/// reader on **every** exit, closing the lifecycle hole where `run_interactive`'s teardown sat
/// lexically after the loop and a `?` early return (terminal init or an in-loop render) skipped it,
/// orphaning the Python child and detaching the reader thread. Process death is observed via the
/// OS, not an escape-stream heuristic.
///
/// Self-contained red→green: first the bare pattern (Child + Reader dropped with no reaping, as the
/// early return did) is shown to leave the child ALIVE; then the owner is shown to reap it — and to
/// do so *promptly* (kill + wait, not waiting out the long-lived child).
#[test]
fn producer_owner_reaps_the_child_and_joins_the_reader_on_drop() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn spawn_sleeper() -> std::process::Child {
        Command::new("sleep")
            .arg("30")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn `sleep` as a stand-in long-lived producer")
    }
    // `kill -0 <pid>` exits 0 iff a live process with that id exists — an OS-level liveness probe.
    fn alive(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    // --- RED baseline: the bare pattern leaks. `std::process::Child` has no reaping `Drop`, so a
    // scope that spawns the child + reader and then returns without kill/wait/join (exactly what a
    // `?` early return did) leaves the child running and the reader thread detached. ---
    let mut leaked = spawn_sleeper();
    let leaked_pid = leaked.id();
    let leaked_stdout = leaked.stdout.take().expect("stdout piped");
    let leaked_reader = Reader::spawn(leaked_stdout, DEFAULT_CONTROL_CAP);
    drop(leaked_reader); // Reader has no Drop that joins — the thread is merely detached.
    assert!(
        alive(leaked_pid),
        "the bare pattern orphans the child (red baseline — Child does not reap on drop)"
    );
    // Reap the orphan ourselves so the test leaves nothing behind (and closing its stdout lets the
    // detached reader thread read EOF and exit).
    let _ = leaked.kill();
    let _ = leaked.wait();

    // --- GREEN: the owner reaps on Drop. ---
    let mut child = spawn_sleeper();
    let child_pid = child.id();
    let stdin = child.stdin.take();
    let stdout = child.stdout.take().expect("stdout piped");
    let reader = Reader::spawn(stdout, DEFAULT_CONTROL_CAP);
    let owner = ProducerOwner::new(child, stdin, reader);
    assert_eq!(
        owner.child_id(),
        child_pid,
        "owner reports the OS pid it owns"
    );

    let t0 = Instant::now();
    drop(owner);
    let elapsed = t0.elapsed();

    // Prompt: if Drop called `wait` WITHOUT `kill` first, it would block ~30s on the live sleeper;
    // returning fast proves kill-then-wait both ran (and the reader join returned on stdout EOF).
    assert!(
        elapsed < Duration::from_secs(5),
        "teardown must kill+wait+join promptly, not wait out the long-lived child (took {elapsed:?})"
    );
    // Reaped: observed through the OS, not inferred from the terminal escape stream.
    assert!(
        !alive(child_pid),
        "the owner must reap the child on drop; pid {child_pid} is still alive"
    );
}

// ---- Gate C1/C2: byte-denominated retention + latest-wins freshness -------------------

/// A FIELD message of a chosen size: `cells` f64 samples = `cells * 8` payload bytes.
fn sized_field_msg(source: u64, seq: u64, cells: usize) -> WireMessage {
    WireMessage {
        source,
        cursor: SourceCursor::new(0, seq),
        body: WireBody::Field(gradient_field(cells, 1)),
    }
}

#[test]
fn a_distinct_source_field_flood_is_bounded_in_bytes_not_just_count() {
    // 256 KiB per field (32768 f64), a 1 MiB byte budget, a generous COUNT cap. Distinct
    // sources never coalesce, so only the byte budget can bound retention — the exact gap a
    // count-only cap leaves (RED: 40 such fields retained 10 MiB behind a count of 40).
    let cells = 32_768; // 256 KiB
    let cap = 1024 * 1024; // 1 MiB
    let mut buf = CoalescingBuffer::with_byte_budget(4096, 16, cap);
    for s in 0..40u64 {
        buf.offer(sized_field_msg(s, 1, cells));
    }
    assert!(
        buf.field_bytes() <= cap,
        "retained {} bytes exceeds the {cap}-byte budget",
        buf.field_bytes()
    );
    assert!(
        buf.bytes_high_water() <= cap,
        "high-water {} exceeded the budget",
        buf.bytes_high_water()
    );
    // The flood is disclosed as expendable drops, never hidden behind a bounded count.
    assert!(
        buf.fields_dropped() > 0,
        "over-budget snapshots must be counted, not hidden"
    );
    // Latest-wins: the most recent arrivals survive; the oldest sources were evicted.
    let mut retained = std::collections::BTreeSet::new();
    buf.drain_into(&mut |m| {
        retained.insert(m.source);
    });
    assert!(
        retained.contains(&39),
        "the newest arrival (source 39) must survive"
    );
    assert!(
        !retained.contains(&0),
        "the oldest arrival (source 0) must have been evicted"
    );
}

#[test]
fn at_the_field_cap_the_newest_snapshot_survives_not_the_stalest() {
    // field_cap = 2 (byte budget generous). A@1, B@1, A@2 — the current back is B when A@2
    // arrives, so it cannot coalesce; the lane is full. The STALE A@1 must be evicted and the
    // FRESH A@2 admitted (RED: the old policy dropped A@2 and kept the stale A@1).
    let mut buf = CoalescingBuffer::with_caps(2, 16);
    buf.offer(field_msg(0xA, 0, 1));
    buf.offer(field_msg(0xB, 0, 1));
    buf.offer(field_msg(0xA, 0, 2));
    let mut seen: Vec<(u64, u64)> = Vec::new();
    buf.drain_into(&mut |m| seen.push((m.source, m.cursor.sequence)));
    let a_seq = seen.iter().find(|(s, _)| *s == 0xA).map(|(_, q)| *q);
    assert_eq!(
        a_seq,
        Some(2),
        "source A's retained snapshot must be the newest (seq 2)"
    );
    assert!(
        seen.iter().any(|(s, _)| *s == 0xB),
        "source B's snapshot must also be retained"
    );
}

#[test]
fn coalescing_reclaims_the_stale_snapshot_bytes() {
    // A run of same-source snapshots coalesces to ONE; retained bytes reflect only the latest,
    // not the sum of the run — otherwise coalescing would leak the very bytes it claims to free.
    let cells = 16_384; // 128 KiB
    let mut buf = CoalescingBuffer::default();
    for seq in 1..=20u64 {
        buf.offer(sized_field_msg(7, seq, cells));
    }
    assert_eq!(
        buf.field_len(),
        1,
        "a same-source run must collapse to one entry"
    );
    assert_eq!(
        buf.field_bytes(),
        cells * 8,
        "retained bytes must be ONE snapshot's worth, not the whole run's"
    );
    assert_eq!(buf.coalesced(), 19, "19 of 20 snapshots coalesced away");
}

#[test]
fn a_field_larger_than_the_whole_budget_is_refused_without_evicting_others() {
    // A byte budget below a single incoming field: the giant cannot be retained without breaking
    // the budget, so it is refused (counted) — and the snapshots already held are NOT disturbed.
    let small = 1_024; // 8 KiB field
    let giant = 100_000; // 800 KiB field
    let cap = 64 * 1024; // 64 KiB budget
    let mut buf = CoalescingBuffer::with_byte_budget(4096, 16, cap);
    buf.offer(sized_field_msg(1, 1, small));
    let before = buf.field_bytes();
    buf.offer(sized_field_msg(2, 1, giant));
    assert_eq!(
        buf.field_bytes(),
        before,
        "the oversized field must not disturb retained bytes"
    );
    assert!(buf.field_bytes() <= cap, "budget must hold");
    assert_eq!(buf.fields_dropped(), 1, "the refused giant must be counted");
}

#[test]
fn an_oversized_same_source_replacement_is_refused_not_retained() {
    // Gate B/B1 — the replacement (coalescing) path must honour the SAME hard byte cap the append
    // path does. The plan's exact counterexample: field_byte_cap = 8 bytes, A@seq1 = 1 f64 (8 B),
    // then A@seq2 = 2 f64 (16 B). A@seq2 is the same source at the queue back, so it takes the
    // coalescing path — which, before the fix, swapped it in with no size guard, leaving
    // field_bytes = 16 > 8 with field_count = 1, so enforce_field_byte_cap (which never evicts the
    // sole snapshot) could not restore the budget. RED before the fix: the cap is breached.
    let cap = 8; // one f64
    let mut buf = CoalescingBuffer::with_byte_budget(1, 16, cap);
    buf.offer(sized_field_msg(0xA, 1, 1)); // 8 B — admitted
    assert_eq!(buf.field_bytes(), 8);
    buf.offer(sized_field_msg(0xA, 2, 2)); // 16 B — same source, newer: the replacement path

    assert!(
        buf.field_bytes() <= cap,
        "an oversized same-source replacement must not breach the byte cap: {} > {cap}",
        buf.field_bytes()
    );
    assert!(
        buf.bytes_high_water() <= cap,
        "high-water must also stay within the cap"
    );
    // The refusal preserves the prior VALID (staler) snapshot rather than dropping to nothing —
    // FIELD snapshots are expendable, and a staler valid observation beats a budget breach.
    let mut seen: Vec<(u64, u64)> = Vec::new();
    buf.drain_into(&mut |m| seen.push((m.source, m.cursor.sequence)));
    assert_eq!(
        seen,
        vec![(0xA, 1)],
        "the prior valid snapshot (seq 1) is kept; the oversized seq 2 is refused"
    );
    assert_eq!(
        buf.fields_dropped(),
        1,
        "the refused oversized replacement is counted as a drop, not a coalesce"
    );
    assert_eq!(
        buf.coalesced(),
        0,
        "a refused replacement is NOT a coalesce"
    );
}

#[test]
fn a_within_cap_replacement_grows_then_shrinks_retained_bytes_exactly() {
    // Control for B1: a replacement that stays within the byte cap still coalesces correctly —
    // growing to the newcomer's bytes and shrinking back, never retaining the stale snapshot's
    // memory. This is the behaviour the oversized-refusal guard must NOT regress.
    let cap = 64 * 1024; // 64 KiB
    let mut buf = CoalescingBuffer::with_byte_budget(1, 16, cap);
    buf.offer(sized_field_msg(7, 1, 1_024)); // 8 KiB
    assert_eq!(buf.field_bytes(), 1_024 * 8);
    buf.offer(sized_field_msg(7, 2, 4_096)); // 32 KiB — grows, within cap
    assert_eq!(
        buf.field_bytes(),
        4_096 * 8,
        "a within-cap replacement retains ONLY the newcomer's bytes (grow)"
    );
    buf.offer(sized_field_msg(7, 3, 512)); // 4 KiB — shrinks, no stale memory retained
    assert_eq!(
        buf.field_bytes(),
        512 * 8,
        "a smaller replacement reclaims the stale bytes (shrink), retaining no stale memory"
    );
    assert_eq!(
        buf.field_len(),
        1,
        "the run stays collapsed to one snapshot"
    );
    assert!(buf.field_bytes() <= cap);
    assert_eq!(
        buf.fields_dropped(),
        0,
        "within-cap replacements drop nothing"
    );
}

#[test]
fn byte_pressure_evicts_fields_but_never_a_control_verdict() {
    // Under byte pressure the buffer evicts expendable FIELD snapshots, but an interleaved ACK
    // (a non-expendable verdict) must survive the eviction — control is never dropped to free bytes.
    let cells = 32_768; // 256 KiB
    let cap = 512 * 1024; // room for ~2 fields
    let mut buf = CoalescingBuffer::with_byte_budget(4096, 16, cap);
    buf.offer(sized_field_msg(1, 1, cells));
    buf.offer(ack_msg(2, 0, 1, 7)); // a verdict interleaved between fields
    buf.offer(sized_field_msg(3, 1, cells));
    buf.offer(sized_field_msg(4, 1, cells)); // forces eviction of an older field
    assert!(
        buf.field_bytes() <= cap,
        "byte budget must hold under the flood"
    );
    let mut acks = 0usize;
    buf.drain_into(&mut |m| {
        if matches!(m.body, WireBody::Ack(_)) {
            acks += 1;
        }
    });
    assert_eq!(
        acks, 1,
        "the interleaved ACK verdict must survive field eviction"
    );
}

// ---- Gate B/B3: hostile per-source chronology — where stale rejection belongs ----------
//
// Decision (evidenced by the two tests below): per-source stale rejection is the INBOX's job,
// not the queue's. The queue is a bounded, latest-wins COALESCING buffer whose contract is to
// preserve message ORDER across control messages (see
// `coalescing_never_reorders_or_collapses_fields_across_a_control_message`); making it reject or
// globally-coalesce per source would both violate that ordering contract and require unbounded
// per-source cursor memory (it discards evicted cursors). `FieldInbox` is the per-source
// authority: it keeps the latest `(epoch, sequence)` per source durably and drops anything not
// strictly newer. So chronology is enforced at the inbox; the queue only bounds delivery.

/// For a CONFORMANT (per-source monotonic) interleaved stream under capacity pressure, the queue
/// never regresses per-source chronology: front-first eviction removes the oldest-INSERTED field
/// first, which for a monotonic source is also its lowest sequence, so the snapshot retained for
/// each source is always its NEWEST — never a staler one resurrected over a fresher one.
#[test]
fn the_queue_keeps_each_sources_newest_under_pressure_for_a_conformant_stream() {
    let mut buf = CoalescingBuffer::with_caps(2, 16); // only two field slots: eviction is forced
    buf.offer(field_msg(0xA, 0, 1));
    buf.offer(field_msg(0xB, 0, 1));
    buf.offer(field_msg(0xA, 0, 2)); // back is B: no coalesce; evicts oldest field (A@1), keeps A@2
    buf.offer(field_msg(0xB, 0, 2)); // back is A: no coalesce; evicts oldest field (B@1), keeps B@2

    let mut latest: std::collections::BTreeMap<u64, u64> = std::collections::BTreeMap::new();
    buf.drain_into(&mut |m| {
        // A conformant producer must never see an OLDER sequence overwrite a newer one here.
        let e = latest.entry(m.source).or_insert(0);
        assert!(
            m.cursor.sequence >= *e,
            "per-source chronology regressed in the queue: {} after {}",
            m.cursor.sequence,
            *e
        );
        *e = m.cursor.sequence;
    });
    assert_eq!(
        latest.get(&0xA),
        Some(&2),
        "source A's retained snapshot is its newest (seq 2)"
    );
    assert_eq!(
        latest.get(&0xB),
        Some(&2),
        "source B's retained snapshot is its newest (seq 2)"
    );
    assert_eq!(
        buf.fields_dropped(),
        2,
        "two stale snapshots evicted under the 2-slot cap"
    );
}

/// The plan's hostile trace `A@seq10, B@seq1, A@seq5` — a per-source chronology VIOLATION (source
/// A's sequence went backwards within one epoch). With a generous queue (no eviction) both A
/// snapshots reach the inbox; the inbox, the authoritative per-source gate, keeps A@10 and drops
/// A@5 as Stale. This is why chronology rejection lives at the inbox: the queue forwarded an
/// out-of-order same-source pair (A@5 could not coalesce — B broke the run), and the inbox caught it.
#[test]
fn per_source_stale_rejection_is_enforced_at_the_inbox_not_the_queue() {
    let mut buf = CoalescingBuffer::with_caps(16, 16); // generous: no eviction, both A frames survive
    buf.offer(field_msg(0xA, 0, 10));
    buf.offer(field_msg(0xB, 0, 1));
    buf.offer(field_msg(0xA, 0, 5)); // back is B: cannot coalesce onto A@10 — forwarded as-is

    // The queue preserves ORDER (its only chronology duty): A@10, B@1, A@5.
    let mut drained: Vec<(u64, u64)> = Vec::new();
    buf.drain_into(&mut |m| drained.push((m.source, m.cursor.sequence)));
    assert_eq!(
        drained,
        [(0xA, 10), (0xB, 1), (0xA, 5)],
        "queue preserves arrival order"
    );

    // Replay the drained stream into the inbox — the per-source authority.
    let mut inbox = FieldInbox::new();
    let outcomes: Vec<FieldAdmission> = drained
        .iter()
        .map(|&(s, q)| inbox.admit(s, SourceCursor::new(0, q), gradient_field(2, 1)))
        .collect();

    assert!(matches!(outcomes[0], FieldAdmission::Stored), "A@10 stored");
    assert!(matches!(outcomes[1], FieldAdmission::Stored), "B@1 stored");
    assert!(
        matches!(
            outcomes[2],
            FieldAdmission::Stale {
                stored: SourceCursor { sequence: 10, .. },
                incoming: SourceCursor { sequence: 5, .. }
            }
        ),
        "the out-of-order A@5 is rejected as Stale against the stored A@10: {:?}",
        outcomes[2]
    );
    assert_eq!(
        inbox.latest_cursor(0xA),
        Some(SourceCursor::new(0, 10)),
        "source A's authoritative latest stays at seq 10 — never regressed to the stale seq 5"
    );
    assert_eq!(
        inbox.superseded(),
        1,
        "exactly one stale observation was dropped, and counted"
    );
}

// ---- Gate B/B4: the retained-field budget is NOT total process memory --------------------
//
// The queue's `field_byte_cap` bounds ONE quantity: retained FIELD payload inside the shared
// buffer. That is deliberately not equated with total process high-water, which additionally
// holds (not counted here): the decoder's partial-frame buffer, the reader's per-read decoded
// batch, the UI drain batch, the inbox's retained per-source observations, rendered surfaces, and
// the Python process's own memory. This test proves the queue budget holds under a mixed flood
// AND that the inbox is a SEPARATE, independently-bounded owner — so neither budget hides the
// other. (Process RSS is reported as a reasoned envelope in the PR, not asserted here: it is
// runner-dependent and not reliably measurable in a unit test.)

#[test]
fn the_field_byte_budget_bounds_retention_and_the_inbox_is_a_separate_owner() {
    let cap = 512 * 1024; // 512 KiB retained-field budget
    let mut buf = CoalescingBuffer::with_byte_budget(4096, 16, cap);
    // A mixed flood: many large distinct-source fields, interleaved verdicts, and out-of-contract
    // inbound commands (rejected, so they add nothing to either budget).
    for s in 0..50u64 {
        buf.offer(sized_field_msg(s, 1, 16_384)); // 128 KiB each
        if s % 5 == 0 {
            buf.offer(ack_msg(s, 0, 1, s)); // a verdict interleaved
        }
    }
    assert!(
        buf.field_bytes() <= cap,
        "retained field bytes exceed the budget"
    );
    assert!(
        buf.bytes_high_water() <= cap,
        "field high-water exceeds the budget"
    );

    // Drain into the inbox: its retention is bounded by SOURCE COUNT, a different budget entirely.
    let mut inbox = FieldInbox::with_capacity(gibson::wire::InboxCapacity::new(64));
    let mut acks = 0u64;
    buf.drain_into(&mut |m| match m.body {
        WireBody::Field(f) => {
            inbox.admit(m.source, m.cursor, f);
        }
        WireBody::Ack(_) => acks += 1,
        _ => {}
    });
    // The two budgets are distinct and both held: the queue bounded bytes; the inbox bounds
    // sources (<= its capacity), independent of how many bytes the queue ever retained.
    let distinct_sources = (0..50u64)
        .filter(|s| inbox.latest_cursor(*s).is_some())
        .count();
    assert!(
        distinct_sources <= 64,
        "inbox retention is bounded by its own source cap, not the byte budget"
    );
    assert!(
        acks > 0,
        "interleaved verdicts survived the field flood (control is never byte-evicted)"
    );
}

// ---- Gate C4: error provenance — read fault vs clean EOF, recoverable-skip receipts ---

use std::io::{self, Read};

/// A [`Read`] that yields `data` once, then returns an `io::Error` forever — a stand-in for a
/// producer stdout that FAULTS (broken pipe / reset) rather than closing cleanly.
struct FaultAfter {
    data: Vec<u8>,
    pos: usize,
    kind: io::ErrorKind,
}
impl Read for FaultAfter {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.data.len() {
            return Err(io::Error::new(self.kind, "simulated producer stdout fault"));
        }
        let n = (self.data.len() - self.pos).min(out.len());
        out[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// A frame that is structurally legal (magic, version, known length) but carries an unknown message
/// type — a RECOVERABLE decode error: the decoder skips exactly this frame (its length is known)
/// and resynchronizes at the next, rather than poisoning the stream.
fn unknown_type_frame(source: u64, epoch: u32, seq: u64) -> Vec<u8> {
    let mut f = field_frame(source, epoch, seq, WireLimits::default());
    // msg_type is the u16 at header offset 6; 0xFFFF is not FIELD/COMMAND/ACK/RETIRE.
    f[6] = 0xFF;
    f[7] = 0xFF;
    f
}

/// GATE C4 — a hard stdout read FAULT is distinguished from a clean EOF. Both end the stream
/// (`disconnected`), but only a fault carries a `read_error`; the old reader collapsed `Err(_)` to
/// the same `eof = true` a clean `Ok(0)` sets, destroying the provenance a disconnect handler needs.
#[test]
fn a_stdout_read_fault_is_distinguished_from_a_clean_eof() {
    // Fault path: one legal field, then the read errors.
    let stream = field_frame(1, 0, 1, WireLimits::default());
    let mut reader = Reader::spawn(
        FaultAfter {
            data: stream,
            pos: 0,
            kind: io::ErrorKind::BrokenPipe,
        },
        DEFAULT_CONTROL_CAP,
    );
    reader.join();
    let outcome = reader.drain(&mut |_| {});
    assert!(outcome.disconnected, "a fault ends the stream");
    assert_eq!(
        outcome.read_error,
        Some(io::ErrorKind::BrokenPipe),
        "a read fault must be recorded with its kind, not mistaken for a clean EOF"
    );
    assert!(
        outcome.fatal.is_none(),
        "a read fault is not a decode-fatal"
    );
    assert_eq!(
        reader.read_error(),
        Some(io::ErrorKind::BrokenPipe),
        "the fault provenance is also readable via the Reader accessor"
    );

    // Negative control: a clean EOF (Cursor returns Ok(0) at the end) disconnects with NO read_error.
    let stream = field_frame(2, 0, 1, WireLimits::default());
    let mut clean = Reader::spawn(std::io::Cursor::new(stream), DEFAULT_CONTROL_CAP);
    clean.join();
    let outcome = clean.drain(&mut |_| {});
    assert!(outcome.disconnected, "a clean close ends the stream too");
    assert!(
        outcome.read_error.is_none(),
        "a graceful EOF must NOT masquerade as a fault"
    );
}

/// GATE C4 — a recoverable decode error (one malformed frame) is skipped AND counted: the stream
/// stays live, the frames on either side decode, and the skip is surfaced as a countable receipt
/// (`decode_skips`) rather than vanishing silently.
#[test]
fn a_recoverable_decode_skip_is_counted_and_surfaced() {
    let mut stream = Vec::new();
    stream.extend_from_slice(&field_frame(1, 0, 1, WireLimits::default()));
    stream.extend_from_slice(&unknown_type_frame(9, 0, 1)); // the poison pill: skipped, not fatal
    stream.extend_from_slice(&field_frame(2, 0, 1, WireLimits::default()));

    let mut reader = Reader::spawn(std::io::Cursor::new(stream), DEFAULT_CONTROL_CAP);
    reader.join();
    let mut got = 0usize;
    let outcome = reader.drain(&mut |_| got += 1);

    // Two DISTINCT sources straddle the pill → both snapshots survive (no coalescing between them).
    assert_eq!(
        got, 2,
        "the frames on either side of a skip must still decode"
    );
    assert!(
        outcome.fatal.is_none(),
        "a recoverable skip must not poison the stream"
    );
    assert_eq!(
        outcome.decode_skips, 1,
        "the single skipped frame must be counted (observable receipt), not lost"
    );
    assert_eq!(
        reader.decode_skips(),
        1,
        "the skip receipt is also readable via the Reader accessor"
    );
    assert!(
        outcome.disconnected,
        "the Cursor reaches a clean EOF after the frames"
    );
    assert!(outcome.read_error.is_none(), "a clean EOF is not a fault");
}

/// GATE C4 — a decode-FATAL (poisoning) error is not a read fault and vice versa: the two
/// provenance channels stay separate. A poisoned decoder surfaces `fatal` with NO `read_error`.
#[test]
fn a_decode_fatal_is_not_recorded_as_a_read_fault() {
    let mut reader = Reader::spawn(std::io::Cursor::new(vec![0xABu8; 64]), DEFAULT_CONTROL_CAP);
    reader.join();
    let outcome = reader.drain(&mut |_| {});
    assert!(
        matches!(outcome.fatal, Some(WireError::BadMagic { .. })),
        "garbage poisons the decoder"
    );
    assert!(
        outcome.read_error.is_none(),
        "a decode-fatal is distinct from a read fault — read_error stays None"
    );
}

// ---- Gate E: the reader's non-blocking drain makes a wall-clock deadline enforceable ----

/// A [`Read`] that BLOCKS on every read until its paired sender is dropped — a producer that
/// connects but never writes a byte. A direct blocking `read()` on this hangs forever; the point
/// of the reader thread is that the UI side can still bound its wait.
struct NeverWrites {
    release: std::sync::mpsc::Receiver<()>,
}
impl Read for NeverWrites {
    fn read(&mut self, _out: &mut [u8]) -> io::Result<usize> {
        // Park until the test drops the sender; models a producer stuck with nothing to say.
        let _ = self.release.recv();
        Ok(0)
    }
}

/// GATE E — a producer that never writes must not hang the consumer. The reader thread blocks in
/// `read()` indefinitely, but [`Reader::drain`] returns promptly every call, so a caller (the
/// headless selfcheck) can bound its wait by a WALL-CLOCK deadline and fail fast — exactly what a
/// direct blocking `stdout.read()` guarded only by a read COUNT could never do (the count never
/// advances while the first read is parked, so it is no deadline at all).
#[test]
fn a_never_writing_producer_is_bounded_by_a_wall_clock_deadline_not_a_hang() {
    use std::time::{Duration, Instant};

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let reader = Reader::spawn(NeverWrites { release: rx }, DEFAULT_CONTROL_CAP);

    let budget = Duration::from_millis(300);
    let start = Instant::now();
    let deadline = start + budget;
    let mut got_any = false;
    let mut timed_out = false;
    loop {
        let outcome = reader.drain(&mut |_| got_any = true);
        // A parked read is none of these: not a disconnect, not a fault, not a fatal.
        assert!(!outcome.disconnected, "a blocked read is not a disconnect");
        assert!(
            outcome.read_error.is_none(),
            "a blocked read is not a fault"
        );
        assert!(outcome.fatal.is_none());
        if got_any {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let elapsed = start.elapsed();
    assert!(!got_any, "a producer that never writes yields no message");
    assert!(
        timed_out,
        "the wall-clock deadline must fire — drain never blocks on the parked read"
    );
    // The wait was bounded within a small margin, NOT an unbounded hang.
    assert!(
        elapsed < budget + Duration::from_secs(2),
        "the deadline must bound the wait (took {elapsed:?})"
    );
    // Release the blocked reader thread so the test leaves nothing parked.
    drop(tx);
}

/// Gate B2 (Renaissance XI): a wrong-direction inbound COMMAND fed through the REAL production
/// reader is rejected as out of contract — never admitted to the control lane, never drained as a
/// deliverable message, its (large) payload never retained — and the violation is VISIBLE through
/// the new `Reader::out_of_contract()` surface rather than counted invisibly inside the buffer.
/// COMMAND is presenter → producer only (`docs/WIRE_PROTOCOL.md`); a conformant ACK in the same
/// stream is still delivered, so the rejection is direction-specific, not a blanket drop, and a
/// recoverable direction violation does not fail the lane closed.
#[test]
fn an_inbound_command_through_the_real_reader_is_rejected_counted_and_visible() {
    use gibson::wire::{encode_ack, encode_command, WireCommand};

    let limits = WireLimits::default();
    let source = 7u64;

    let mut stream = Vec::new();
    // A legitimate FIELD and ACK: the stream is otherwise well-formed and must be processed.
    stream.extend_from_slice(&field_frame(source, 0, 1, limits));
    stream.extend_from_slice(&encode_ack(
        source,
        SourceCursor::new(0, 2),
        WireAck {
            command_id: 1,
            status: AckStatus::Acknowledged,
        },
    ));
    // A flood of wrong-direction COMMANDs, each with a 1 KiB argument — the payload a hostile
    // producer would try to make the presenter retain.
    const HOSTILE: u64 = 50;
    for i in 0..HOSTILE {
        stream.extend_from_slice(&encode_command(
            source,
            SourceCursor::new(0, 100 + i),
            &WireCommand {
                command_id: 1000 + i,
                kind: 1,
                arg: vec![0xCDu8; 1024],
            },
        ));
    }

    let mut reader = Reader::spawn(std::io::Cursor::new(stream), DEFAULT_CONTROL_CAP);
    reader.join(); // decode to EOF

    // VISIBLE through the production reader (the Gate B2 surface), one per hostile COMMAND.
    assert_eq!(
        reader.out_of_contract(),
        HOSTILE,
        "every inbound COMMAND must be counted as a direction violation, visible on the Reader"
    );
    // A recoverable direction violation (here 50) must NOT fail the control lane closed.
    assert!(
        reader.control_overflow().is_none(),
        "a recoverable direction violation must not fail the lane closed"
    );

    let mut drained: Vec<WireMessage> = Vec::new();
    let outcome = reader.drain(&mut |m| drained.push(m));
    assert!(outcome.disconnected, "EOF is a clean disconnect");

    // The hostile COMMAND never becomes a deliverable message: it cannot enter the control lane
    // and cannot resolve or create an edit.
    assert!(
        !drained
            .iter()
            .any(|m| matches!(m.body, WireBody::Command(_))),
        "no inbound COMMAND may be delivered to the application"
    );
    // The conformant ACK IS delivered — the rejection is direction-specific, not a blanket drop.
    assert!(
        drained.iter().any(|m| matches!(m.body, WireBody::Ack(_))),
        "a conformant ACK in the same stream must still be delivered"
    );
    // No unbounded payload retention: nothing is left buffered and the retained FIELD budget never
    // absorbed the 50 KiB of hostile COMMAND arguments (a COMMAND is not a field, so it is dropped
    // at offer, not stored).
    assert_eq!(reader.buffered_len(), 0, "nothing retained after drain");
    assert_eq!(
        reader.field_bytes(),
        0,
        "the hostile COMMAND payload never entered FIELD retention"
    );
}
