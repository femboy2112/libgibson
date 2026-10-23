//! Wave 5 — the SmartChem-facing producer transport, driven as hostile consumer code.
//!
//! The producer is out-of-process and presumed diseased until proven otherwise. These
//! tests feed the decoder the whole failure matrix the plan demands — producer faster than
//! presenter, command-queue saturation, crash/disconnect, epoch restart, malformed
//! dimensions/lengths/metadata, an old frame after a newer one, reconnect/retirement, and
//! the bounded-RSS ceilings — and assert the transport answers each with a *typed* outcome,
//! never a panic, a silent drop, or an unbounded allocation.
//!
//! The decisive reconciliation (Law L3): a field decoded off the wire renders through the
//! SAME `FieldExtent` pointer selection uses, so a producer's geometry and the editor's
//! geometry cannot disagree — proven here by probing a decoded field and matching a
//! `FieldView` sample.

use gibson::field_view::{FieldExtent, FieldView, Sample};
use gibson::shape::Point;
use gibson::snapshot::{Admission, Capacity, RefuseReason, SnapshotBridge};
use gibson::wire::{
    encode_ack, encode_command, encode_field, AckStatus, FieldAdmission, FieldInbox, FrameDecoder,
    InboxCapacity, SourceCursor, WireAck, WireBody, WireCommand, WireError, WireField, WireLimits,
    HEADER_BYTES, MAGIC, VERSION,
};

// ---- builders -------------------------------------------------------------------------

fn extent(cols: usize, rows: usize) -> FieldExtent {
    FieldExtent::new(Point::new(-1.0, 2.0), 0.5, 0.25, cols, rows).expect("valid extent")
}

/// A field with a simple linear-gradient payload so a probe has a known answer.
fn gradient_field(cols: usize, rows: usize, tick: u64) -> WireField {
    let ext = extent(cols, rows);
    let data: Vec<f64> = (0..cols * rows).map(|i| i as f64).collect();
    WireField {
        extent: ext,
        model_tick: tick,
        data,
        mask: None,
    }
}

/// Hand-assemble a FIELD frame with explicit control over every field, so the malformed
/// cases can corrupt exactly one thing. `MSG_FIELD` is wire type `1`.
#[allow(clippy::too_many_arguments)]
fn raw_field_frame(
    epoch: u32,
    seq: u64,
    source: u64,
    cols: u32,
    rows: u32,
    cell_w: f64,
    cell_h: f64,
    tick: u64,
    flags: u32,
    data: &[f64],
    mask: Option<&[u8]>,
    payload_len_override: Option<u32>,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&cols.to_le_bytes());
    payload.extend_from_slice(&rows.to_le_bytes());
    payload.extend_from_slice(&(-1.0_f64).to_le_bytes()); // origin_x
    payload.extend_from_slice(&2.0_f64.to_le_bytes()); // origin_y
    payload.extend_from_slice(&cell_w.to_le_bytes());
    payload.extend_from_slice(&cell_h.to_le_bytes());
    payload.extend_from_slice(&tick.to_le_bytes());
    payload.extend_from_slice(&flags.to_le_bytes());
    for v in data {
        payload.extend_from_slice(&v.to_le_bytes());
    }
    if let Some(m) = mask {
        payload.extend_from_slice(m);
    }
    let plen = payload_len_override.unwrap_or(payload.len() as u32);

    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // MSG_FIELD
    out.extend_from_slice(&epoch.to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&source.to_le_bytes());
    out.extend_from_slice(&plen.to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

/// Drain one message, asserting there is one ready and it decoded cleanly.
fn decode_one(dec: &mut FrameDecoder) -> WireBody {
    match dec.next_message() {
        Some(Ok(m)) => m.body,
        other => panic!("expected one decoded message, got {other:?}"),
    }
}

// ---- round-trip ----------------------------------------------------------------------

#[test]
fn field_round_trip_with_and_without_mask() {
    let limits = WireLimits::default();

    let plain = gradient_field(3, 4, 77);
    let bytes = encode_field(9, SourceCursor::new(1, 5), &plain, limits).unwrap();
    let mut dec = FrameDecoder::new();
    dec.push(&bytes).unwrap();
    let msg = dec.next_message().unwrap().unwrap();
    assert_eq!(msg.source, 9);
    assert_eq!(msg.cursor, SourceCursor::new(1, 5));
    match msg.body {
        WireBody::Field(f) => assert_eq!(f, plain),
        other => panic!("not a field: {other:?}"),
    }

    // With a mask: true = present, false = masked.
    let masked = WireField {
        extent: extent(2, 2),
        model_tick: 1,
        data: vec![10.0, 20.0, 30.0, 40.0],
        mask: Some(vec![true, false, true, true]),
    };
    let bytes = encode_field(1, SourceCursor::new(0, 0), &masked, limits).unwrap();
    let mut dec = FrameDecoder::new();
    dec.push(&bytes).unwrap();
    assert_eq!(decode_one(&mut dec), WireBody::Field(masked));
}

#[test]
fn non_finite_values_survive_bit_exact() {
    // The wire must transmit NaN/±∞ faithfully, never coerce the data (it is disclosed
    // downstream as Sample::NonFinite, not laundered into a number here).
    let field = WireField {
        extent: extent(2, 2),
        model_tick: 0,
        data: vec![1.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY],
        mask: None,
    };
    let bytes = encode_field(3, SourceCursor::new(2, 9), &field, WireLimits::default()).unwrap();
    let mut dec = FrameDecoder::new();
    dec.push(&bytes).unwrap();
    let WireBody::Field(decoded) = decode_one(&mut dec) else {
        panic!("not a field");
    };
    for (a, b) in decoded.data.iter().zip(field.data.iter()) {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "non-finite datum not bit-preserved"
        );
    }
}

#[test]
fn command_and_ack_round_trip() {
    let cmd = WireCommand {
        command_id: 42,
        kind: 7,
        arg: b"move 3 4".to_vec(),
    };
    let bytes = encode_command(5, SourceCursor::new(0, 1), &cmd);
    let mut dec = FrameDecoder::new();
    dec.push(&bytes).unwrap();
    assert_eq!(decode_one(&mut dec), WireBody::Command(cmd));

    let ack = WireAck {
        command_id: 42,
        status: AckStatus::Acknowledged,
    };
    let bytes = encode_ack(5, SourceCursor::new(0, 2), ack);
    let mut dec = FrameDecoder::new();
    dec.push(&bytes).unwrap();
    assert_eq!(decode_one(&mut dec), WireBody::Ack(ack));
}

#[test]
fn retire_round_trip() {
    let bytes = gibson::wire::encode_retire(8, SourceCursor::new(1, 1));
    let mut dec = FrameDecoder::new();
    dec.push(&bytes).unwrap();
    let msg = dec.next_message().unwrap().unwrap();
    assert_eq!(msg.source, 8);
    assert_eq!(msg.body, WireBody::Retire);
}

// ---- chronology: epoch restart, old-after-new (L6) -----------------------------------

#[test]
fn field_inbox_latest_wins_and_drops_stale() {
    let mut inbox = FieldInbox::new();
    // Producer much faster than presenter: 100 frames, one survives.
    for seq in 0..100u64 {
        let adm = inbox.admit(7, SourceCursor::new(0, seq), gradient_field(2, 2, seq));
        assert!(adm.is_stored());
    }
    assert_eq!(inbox.live_sources(), 1);
    assert_eq!(inbox.latest_cursor(7), Some(SourceCursor::new(0, 99)));
    assert_eq!(inbox.latest(7).unwrap().model_tick, 99);
    assert_eq!(inbox.stored_new(), 1);
    assert_eq!(inbox.stored_replace(), 99);
    assert_eq!(inbox.superseded(), 99);
    assert!(inbox.conserves());

    // An OLD frame arriving after a newer one is stale, and does not disturb the latest.
    let adm = inbox.admit(7, SourceCursor::new(0, 50), gradient_field(2, 2, 50));
    assert!(matches!(adm, FieldAdmission::Stale { .. }));
    assert_eq!(inbox.latest_cursor(7), Some(SourceCursor::new(0, 99)));
    assert_eq!(inbox.stale(), 1);
    assert!(inbox.conserves());
}

#[test]
fn epoch_restart_beats_a_fallen_back_sequence() {
    let mut inbox = FieldInbox::new();
    inbox.admit(7, SourceCursor::new(0, 90), gradient_field(2, 2, 90));
    // Producer restarts: epoch bumps, sequence resets to 0. Strict latest-wins by seq would
    // wrongly reject this as stale; lexicographic (epoch, seq) accepts it.
    let adm = inbox.admit(7, SourceCursor::new(1, 0), gradient_field(2, 2, 1000));
    assert!(
        adm.is_stored(),
        "higher epoch must win over a higher stale sequence"
    );
    assert_eq!(inbox.latest_cursor(7), Some(SourceCursor::new(1, 0)));
    assert_eq!(inbox.latest(7).unwrap().model_tick, 1000);

    // A straggler from the OLD epoch is now stale.
    let adm = inbox.admit(7, SourceCursor::new(0, 99), gradient_field(2, 2, 99));
    assert!(matches!(adm, FieldAdmission::Stale { .. }));
    assert_eq!(inbox.latest_cursor(7), Some(SourceCursor::new(1, 0)));
    assert!(inbox.conserves());
}

// ---- reconnect / retirement ----------------------------------------------------------

#[test]
fn retire_frees_the_source_and_reconnect_restores_it() {
    let mut inbox = FieldInbox::new();
    inbox.admit(7, SourceCursor::new(0, 1), gradient_field(4, 4, 1));
    assert_eq!(inbox.live_sources(), 1);
    assert_eq!(inbox.retained_cells(), 16);

    assert!(inbox.retire(7));
    assert_eq!(inbox.live_sources(), 0);
    assert_eq!(inbox.retained_cells(), 0);
    assert!(!inbox.retire(7), "retiring an absent source reports false");

    // Reconnect under a fresh epoch.
    let adm = inbox.admit(7, SourceCursor::new(1, 0), gradient_field(2, 2, 2));
    assert!(adm.is_stored());
    assert_eq!(inbox.live_sources(), 1);
    assert!(inbox.conserves());
}

// ---- bounded RSS: source ceiling, frame cap, buffer cap ------------------------------

#[test]
fn field_inbox_refuses_new_source_at_ceiling_without_evicting() {
    let mut inbox = FieldInbox::with_capacity(InboxCapacity::new(3));
    for s in 0..3u64 {
        assert!(inbox
            .admit(s, SourceCursor::new(0, 0), gradient_field(2, 2, 0))
            .is_stored());
    }
    assert_eq!(inbox.live_sources(), 3);
    // A fourth, NEW source is refused — admission, not eviction.
    let adm = inbox.admit(99, SourceCursor::new(0, 0), gradient_field(2, 2, 0));
    assert!(matches!(
        adm,
        FieldAdmission::SourceCapReached { capacity: 3 }
    ));
    assert_eq!(inbox.live_sources(), 3);
    assert!(inbox.latest(0).is_some(), "no live source was evicted");
    assert_eq!(inbox.refused(), 1);
    // But an existing source can still update.
    assert!(inbox
        .admit(1, SourceCursor::new(0, 1), gradient_field(2, 2, 1))
        .is_stored());
    assert!(inbox.conserves());
}

#[test]
fn oversized_declared_length_is_refused_before_allocating() {
    // The classic length-prefix OOM: a header that CLAIMS a huge payload. The decoder must
    // reject it on the length field, before buffering or allocating toward it.
    let limits = WireLimits::new(10_000, 1_000);
    let mut dec = FrameDecoder::with_limits(limits);
    let mut header = Vec::new();
    header.extend_from_slice(&MAGIC);
    header.extend_from_slice(&VERSION.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes()); // epoch
    header.extend_from_slice(&0u64.to_le_bytes()); // seq
    header.extend_from_slice(&0u64.to_le_bytes()); // source
    header.extend_from_slice(&50_000u32.to_le_bytes()); // payload_len >> cap
    assert_eq!(header.len(), HEADER_BYTES);
    dec.push(&header).unwrap();
    let err = dec.next_message().unwrap().unwrap_err();
    assert!(matches!(err, WireError::FrameTooLarge { cap: 10_000, .. }));
    assert!(err.is_fatal());
    // Poisoned: further reads return the same fatal error.
    assert_eq!(dec.next_message().unwrap().unwrap_err(), err);
}

#[test]
fn accumulation_buffer_is_bounded() {
    let limits = WireLimits::new(1_000, 100);
    let mut dec = FrameDecoder::with_limits(limits);
    let ceiling = HEADER_BYTES + 1_000;
    let overshoot = vec![0u8; ceiling + 1];
    let err = dec.push(&overshoot).unwrap_err();
    assert!(matches!(err, WireError::BufferOverflow { .. }));
    assert!(err.is_fatal());
    assert_eq!(dec.buffered_bytes(), 0, "overflowing push appends nothing");
}

#[test]
fn field_larger_than_cell_cap_is_refused() {
    let limits = WireLimits::new(100_000, 100);
    let mut dec = FrameDecoder::with_limits(limits);
    // 20x20 = 400 cells > cap 100, but the byte size (3252) is under the payload cap, so it
    // buffers fine and is rejected on the cell count — recoverable, not fatal.
    let data = vec![0.0_f64; 400];
    let frame = raw_field_frame(0, 0, 0, 20, 20, 0.5, 0.25, 0, 0, &data, None, None);
    dec.push(&frame).unwrap();
    let err = dec.next_message().unwrap().unwrap_err();
    assert!(matches!(
        err,
        WireError::FieldTooLarge {
            cells: 400,
            cap: 100
        }
    ));
    assert!(!err.is_fatal());
}

// ---- malformed dimensions / lengths / metadata --------------------------------------

#[test]
fn declared_length_inconsistent_with_dimensions_is_rejected_and_resyncs() {
    // cols*rows = 16 cells implies 52 + 128 = 180 payload bytes, but we send data for only
    // 10 cells (52 + 80 = 132), with an honest payload_len of 132. The body size computed
    // from the dimensions disagrees with the declared length -> LengthMismatch, recoverable.
    let bad = raw_field_frame(0, 0, 0, 4, 4, 0.5, 0.25, 0, 0, &[0.0f64; 10], None, None);
    // Sandwich it between two good frames to prove the stream resynchronizes.
    let good_a = encode_field(
        1,
        SourceCursor::new(0, 1),
        &gradient_field(2, 2, 1),
        WireLimits::default(),
    )
    .unwrap();
    let good_b = encode_field(
        1,
        SourceCursor::new(0, 2),
        &gradient_field(2, 2, 2),
        WireLimits::default(),
    )
    .unwrap();

    let mut dec = FrameDecoder::new();
    dec.push(&good_a).unwrap();
    dec.push(&bad).unwrap();
    dec.push(&good_b).unwrap();

    assert!(matches!(decode_one(&mut dec), WireBody::Field(_)));
    let err = dec.next_message().unwrap().unwrap_err();
    assert!(matches!(
        err,
        WireError::LengthMismatch {
            declared: 132,
            computed: 180
        }
    ));
    assert!(
        !err.is_fatal(),
        "a length mismatch is per-frame, not stream-fatal"
    );
    // The next good frame still decodes — the decoder resynchronized on the frame boundary.
    match decode_one(&mut dec) {
        WireBody::Field(f) => assert_eq!(f.model_tick, 2),
        other => panic!("expected the trailing good frame, got {other:?}"),
    }
}

#[test]
fn degenerate_geometry_is_rejected() {
    // cell_w = 0 -> FieldExtent::new refuses -> BadExtent (recoverable).
    let frame = raw_field_frame(
        0,
        0,
        0,
        2,
        2,
        0.0,
        0.25,
        0,
        0,
        &[1.0, 2.0, 3.0, 4.0],
        None,
        None,
    );
    let mut dec = FrameDecoder::new();
    dec.push(&frame).unwrap();
    assert_eq!(
        dec.next_message().unwrap().unwrap_err(),
        WireError::BadExtent
    );

    // A NaN cell size is just as dead.
    let frame = raw_field_frame(
        0,
        0,
        0,
        2,
        2,
        0.5,
        f64::NAN,
        0,
        0,
        &[1.0, 2.0, 3.0, 4.0],
        None,
        None,
    );
    let mut dec = FrameDecoder::new();
    dec.push(&frame).unwrap();
    assert_eq!(
        dec.next_message().unwrap().unwrap_err(),
        WireError::BadExtent
    );
}

#[test]
fn unknown_flag_bits_and_bad_mask_bytes_are_rejected() {
    // Flag bit 1 is undefined.
    let frame = raw_field_frame(
        0,
        0,
        0,
        2,
        2,
        0.5,
        0.25,
        0,
        0b10,
        &[1.0, 2.0, 3.0, 4.0],
        None,
        None,
    );
    let mut dec = FrameDecoder::new();
    dec.push(&frame).unwrap();
    assert_eq!(
        dec.next_message().unwrap().unwrap_err(),
        WireError::BadFlags { found: 0b10 }
    );

    // has_mask set, but a mask byte is neither 0 nor 1.
    let frame = raw_field_frame(
        0,
        0,
        0,
        2,
        2,
        0.5,
        0.25,
        0,
        1,
        &[1.0, 2.0, 3.0, 4.0],
        Some(&[1, 0, 1, 2]),
        None,
    );
    let mut dec = FrameDecoder::new();
    dec.push(&frame).unwrap();
    assert_eq!(
        dec.next_message().unwrap().unwrap_err(),
        WireError::BadMaskByte { found: 2 }
    );
}

#[test]
fn bad_ack_status_is_rejected() {
    // Hand-build an ack frame (MSG_ACK = 3) with an out-of-range status.
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&12u32.to_le_bytes());
    out.extend_from_slice(&1u64.to_le_bytes()); // command_id
    out.extend_from_slice(&99u32.to_le_bytes()); // status out of range
    let mut dec = FrameDecoder::new();
    dec.push(&out).unwrap();
    assert_eq!(
        dec.next_message().unwrap().unwrap_err(),
        WireError::BadAckStatus { found: 99 }
    );
}

// ---- corrupt framing: magic, version, unknown type ----------------------------------

#[test]
fn bad_magic_is_fatal() {
    let mut dec = FrameDecoder::new();
    let mut junk = vec![b'X', b'X', b'X', b'X'];
    junk.extend_from_slice(&[0u8; HEADER_BYTES - 4]);
    dec.push(&junk).unwrap();
    let err = dec.next_message().unwrap().unwrap_err();
    assert!(matches!(err, WireError::BadMagic { .. }));
    assert!(err.is_fatal());
}

#[test]
fn unsupported_version_is_fatal() {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&999u16.to_le_bytes());
    out.extend_from_slice(&[0u8; HEADER_BYTES - 6]);
    let mut dec = FrameDecoder::new();
    dec.push(&out).unwrap();
    let err = dec.next_message().unwrap().unwrap_err();
    assert!(
        matches!(err, WireError::UnsupportedVersion { found: 999, supported } if supported == VERSION)
    );
    assert!(err.is_fatal());
}

#[test]
fn unknown_message_type_is_skipped_not_fatal() {
    // MSG type 77, empty payload -> skipped; a following good frame still decodes.
    let mut unknown = Vec::new();
    unknown.extend_from_slice(&MAGIC);
    unknown.extend_from_slice(&VERSION.to_le_bytes());
    unknown.extend_from_slice(&77u16.to_le_bytes());
    unknown.extend_from_slice(&[0u8; HEADER_BYTES - 8]); // epoch..payload_len all zero
    let good = encode_field(
        1,
        SourceCursor::new(0, 1),
        &gradient_field(2, 2, 5),
        WireLimits::default(),
    )
    .unwrap();

    let mut dec = FrameDecoder::new();
    dec.push(&unknown).unwrap();
    dec.push(&good).unwrap();
    let err = dec.next_message().unwrap().unwrap_err();
    assert!(matches!(err, WireError::UnknownMessageType { found: 77 }));
    assert!(!err.is_fatal());
    match decode_one(&mut dec) {
        WireBody::Field(f) => assert_eq!(f.model_tick, 5),
        other => panic!("expected the good frame after the unknown type, got {other:?}"),
    }
}

// ---- producer crash / disconnect -----------------------------------------------------

#[test]
fn truncated_final_frame_is_reported_by_finish() {
    let good = encode_field(
        1,
        SourceCursor::new(0, 1),
        &gradient_field(2, 2, 1),
        WireLimits::default(),
    )
    .unwrap();
    let mut dec = FrameDecoder::new();
    // One clean frame, then a producer that died mid-write: header + half a payload.
    dec.push(&good).unwrap();
    assert!(matches!(decode_one(&mut dec), WireBody::Field(_)));

    let truncated = &good[..good.len() - 5];
    dec.push(truncated).unwrap();
    assert!(
        dec.next_message().is_none(),
        "a partial frame yields no message, just waits"
    );
    let err = dec.finish().unwrap_err();
    assert!(matches!(err, WireError::Truncated { .. }));
    assert!(err.is_fatal());
}

#[test]
fn clean_stream_finishes_ok() {
    let good = encode_field(
        1,
        SourceCursor::new(0, 1),
        &gradient_field(2, 2, 1),
        WireLimits::default(),
    )
    .unwrap();
    let mut dec = FrameDecoder::new();
    dec.push(&good).unwrap();
    let _ = decode_one(&mut dec);
    assert!(dec.finish().is_ok());
}

// ---- streaming: bytes split arbitrarily across pushes --------------------------------

#[test]
fn a_frame_split_across_pushes_reassembles() {
    let field = gradient_field(5, 5, 123);
    let bytes = encode_field(2, SourceCursor::new(3, 4), &field, WireLimits::default()).unwrap();
    let mut dec = FrameDecoder::new();
    // Feed it one byte at a time — the nastiest fragmentation a stream can hand you.
    for b in &bytes {
        assert!(dec.next_message().is_none() || dec.messages_decoded() == 1);
        dec.push(std::slice::from_ref(b)).unwrap();
    }
    match decode_one(&mut dec) {
        WireBody::Field(f) => assert_eq!(f, field),
        other => panic!("reassembly failed: {other:?}"),
    }
}

// ---- command return channel: ordering, saturation, five-state lifecycle --------------

#[test]
fn command_channel_is_ordered_bounded_and_conserved() {
    // The presenter's bounded command queue is the Finding-A SnapshotBridge, carrying our
    // WireCommand as the payload. Saturate it; the overflow is a typed refusal, not growth.
    let mut bridge: SnapshotBridge<WireCommand> =
        SnapshotBridge::with_capacity(Capacity::new(8, 2));
    let c0 = WireCommand {
        command_id: 0,
        kind: 1,
        arg: vec![],
    };
    let c1 = WireCommand {
        command_id: 1,
        kind: 1,
        arg: vec![],
    };
    let c2 = WireCommand {
        command_id: 2,
        kind: 1,
        arg: vec![],
    };

    assert_eq!(bridge.issue(c0.clone()), Admission::Accepted);
    assert_eq!(bridge.issue(c1.clone()), Admission::Accepted);
    // Queue full (cap 2) -> refused, counted, not retained.
    assert_eq!(
        bridge.issue(c2.clone()),
        Admission::Refused(RefuseReason::CommandQueueFull { capacity: 2 })
    );

    // Delivery is exact FIFO order, and each drained command can be encoded for the
    // producer and decoded back identically.
    let drained = bridge.drain_commands();
    assert_eq!(drained, vec![c0.clone(), c1.clone()]);
    let mut dec = FrameDecoder::new();
    for (i, c) in drained.iter().enumerate() {
        let bytes = encode_command(5, SourceCursor::new(0, i as u64), c);
        dec.push(&bytes).unwrap();
    }
    assert_eq!(decode_one(&mut dec), WireBody::Command(c0));
    assert_eq!(decode_one(&mut dec), WireBody::Command(c1));

    // After re-offering the refused command (backpressure handled) it is accepted.
    assert_eq!(bridge.issue(c2), Admission::Accepted);
    bridge.acknowledge_delivered(2);
    assert!(bridge.commands_conserved());
    assert_eq!(bridge.commands_refused(), 1);
    assert_eq!(bridge.commands_delivered(), 2);
}

// ---- L3: a decoded field renders through the SAME extent pointer selection uses -------

#[test]
fn decoded_field_reconciles_with_field_view_sampling() {
    // A 3x4 gradient where data[iy*3 + ix] = iy*3 + ix.
    let field = gradient_field(3, 4, 0);
    let bytes = encode_field(1, SourceCursor::new(0, 0), &field, WireLimits::default()).unwrap();
    let mut dec = FrameDecoder::new();
    dec.push(&bytes).unwrap();
    let WireBody::Field(decoded) = decode_one(&mut dec) else {
        panic!("not a field");
    };

    // Geometry survived the wire exactly.
    assert_eq!(decoded.extent, field.extent);

    // The decoded extent resolves a world point to the SAME index the editor would, and the
    // FieldView sample at that index is the gradient value we encoded. extent = origin
    // (-1,2), cell 0.5 x 0.25. A point inside cell (ix=2, iy=1): x in [0, 0.5), y in [2.25,
    // 2.5): world (0.1, 2.3).
    let p = Point::new(0.1, 2.3);
    let (ix, iy) = decoded
        .extent
        .world_to_index(p)
        .expect("point is inside the field");
    assert_eq!((ix, iy), (2, 1));
    let view = FieldView::new(decoded.extent, &decoded.data, decoded.mask.as_deref()).unwrap();
    assert_eq!(
        view.sample(ix, iy),
        Some(Sample::Value((iy * 3 + ix) as f64))
    );
}

#[test]
fn model_tick_travels_independently_of_sequence() {
    // Law L6: the logical model tick a frame depicts is its own chronology, distinct from
    // the transport sequence. A frame can be sequence 3 yet depict tick 9000.
    let field = WireField {
        extent: extent(1, 1),
        model_tick: 9000,
        data: vec![2.5],
        mask: None,
    };
    let bytes = encode_field(0, SourceCursor::new(0, 3), &field, WireLimits::default()).unwrap();
    let mut dec = FrameDecoder::new();
    dec.push(&bytes).unwrap();
    let msg = dec.next_message().unwrap().unwrap();
    assert_eq!(msg.cursor.sequence, 3);
    if let WireBody::Field(f) = msg.body {
        assert_eq!(f.model_tick, 9000);
    } else {
        panic!("not a field");
    }
}
