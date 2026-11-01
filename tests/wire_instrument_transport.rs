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
    encode_field, AckStatus, FrameDecoder, SourceCursor, WireAck, WireBody, WireError, WireField,
    WireLimits, WireMessage, HEADER_BYTES,
};

use transport::{CoalescingBuffer, FramePump, Reader, DEFAULT_CONTROL_CAP};

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
        buf.dropped(),
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
    assert_eq!(buf.dropped(), 0);

    let mut out: Vec<(u64, u64)> = Vec::new();
    buf.drain_into(&mut |m| out.push((m.source, m.cursor.sequence)));
    assert_eq!(
        out,
        [(1, 3), (2, 8)],
        "each source's latest, in arrival order"
    );
}

/// Control messages never coalesce, so a flood of them past the hard cap proves the bound:
/// the queue stays at the cap and every dropped message is *counted*, never silently lost (L7).
#[test]
fn control_flood_past_the_cap_is_bounded_and_disclosed() {
    let cap = 8;
    let mut buf = CoalescingBuffer::with_control_cap(cap);
    for id in 0..(cap as u64) * 2 {
        buf.offer(ack_msg(7, 0, id, id)); // acks are never coalesceable
    }
    assert_eq!(buf.coalesced(), 0, "control messages are never collapsed");
    assert_eq!(buf.len(), cap, "hard-bounded at the cap");
    assert_eq!(
        buf.dropped(),
        cap as u64,
        "every message past the cap is disclosed"
    );
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
