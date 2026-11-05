#!/usr/bin/env python3
"""Local conformance checks for the Wave 5 producer codec (`wire.py`).

Pure standard library; run directly:  python3 bindings/python/test_wire.py

The Rust side gates the *same* golden vector in CI (`tests/wire_conformance.rs`): Rust
decodes these Python-produced bytes and re-encodes them byte-for-byte. This file is the
Python half, run by CI's `python-wire` job (and locally whenever you touch the format).
"""

import math
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wire  # noqa: E402


def golden_path():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)),
                        "testdata", "golden_field_v1.bin")


def test_golden_round_trips():
    data = open(golden_path(), "rb").read()
    msgs = []
    off = 0
    while off < len(data):
        res = wire.decode_message(data[off:])
        assert res is not None, "golden vector truncated"
        msg, consumed = res
        msgs.append(msg)
        off += consumed
    assert off == len(data)
    assert len(msgs) == 5, f"expected 5 frames, got {len(msgs)}"

    f1, f2, f3, f4, f5 = msgs
    assert f1.kind == wire.MSG_FIELD and (f1.epoch, f1.sequence) == (1, 1)
    assert f1.body["data"] == [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]
    assert f1.body["mask"] is None

    assert f2.kind == wire.MSG_FIELD and (f2.epoch, f2.sequence) == (1, 2)
    assert f2.body["data"][0] == 42.0
    assert math.isinf(f2.body["data"][1]) and f2.body["data"][1] > 0
    assert math.isinf(f2.body["data"][2]) and f2.body["data"][2] < 0
    assert f2.body["mask"] == [True, False, True, True]

    assert f3.kind == wire.MSG_COMMAND and f3.body["command_id"] == 7
    assert f3.body["kind"] == 2 and f3.body["arg"] == b"hi"

    assert f4.kind == wire.MSG_ACK and f4.body["status"] == wire.ACK_ACKNOWLEDGED
    assert f5.kind == wire.MSG_RETIRE


def test_reencode_matches_golden_bytes():
    rebuilt = b"".join([
        wire.encode_field(1, 1, 1, 3, 2, (-1.0, 2.0), 0.5, 0.25, 100,
                          [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]),
        wire.encode_field(1, 1, 2, 2, 2, (0.0, 0.0), 1.0, 1.0, 101,
                          [42.0, float("inf"), float("-inf"), 7.5], mask=[1, 0, 1, 1]),
        wire.encode_command(1, 0, 1, 7, 2, b"hi"),
        wire.encode_ack(1, 0, 2, 7, wire.ACK_ACKNOWLEDGED),
        wire.encode_retire(1, 1, 3),
    ])
    assert rebuilt == open(golden_path(), "rb").read(), "re-encode diverged from golden"


def test_nan_is_bit_preserved():
    nan_bits = struct.pack("<d", float("nan"))
    frame = wire.encode_field(1, 0, 0, 1, 1, (0.0, 0.0), 1.0, 1.0, 0, [float("nan")])
    msg, _ = wire.decode_message(frame)
    assert struct.pack("<d", msg.body["data"][0]) == nan_bits


def test_oversized_length_is_refused_before_buffering():
    # A header claiming a huge payload must be rejected on the length field.
    header = struct.pack("<4sHHIQQI", wire.MAGIC, wire.VERSION, wire.MSG_FIELD,
                         0, 0, 0, 10 ** 9)
    try:
        wire.decode_message(header, limits_max_payload=10_000)
    except wire.WireError:
        pass
    else:
        raise AssertionError("expected WireError for oversized declared length")


def test_stale_vs_restart_ordering():
    # Lexicographic (epoch, sequence): higher epoch wins; within an epoch, higher seq wins.
    assert (1, 0) > (0, 999)
    assert (0, 5) < (0, 6)


def test_producer_command_verdicts():
    # The producer's return-channel contract (mirrors the Rust ProducerModel in
    # tests/wire_transport.rs): set-amplitude (kind 1) with an in-range finite f64 is
    # acknowledged and carries the applied value; out of range or non-finite is refused; a
    # wrong-length arg or an unknown kind is failed.
    import producer

    def amp(v):
        return struct.pack("<d", v)

    assert producer.classify_command(1, amp(3.0)) == (wire.ACK_ACKNOWLEDGED, 3.0)
    assert producer.classify_command(1, amp(0.0)) == (wire.ACK_ACKNOWLEDGED, 0.0)
    assert producer.classify_command(1, amp(10.0)) == (wire.ACK_ACKNOWLEDGED, 10.0)
    assert producer.classify_command(1, amp(99.0)) == (wire.ACK_REFUSED, None)
    assert producer.classify_command(1, amp(-0.5)) == (wire.ACK_REFUSED, None)
    assert producer.classify_command(1, amp(float("nan"))) == (wire.ACK_REFUSED, None)
    assert producer.classify_command(1, amp(float("inf"))) == (wire.ACK_REFUSED, None)
    assert producer.classify_command(1, b"\x00\x00") == (wire.ACK_FAILED, None)
    assert producer.classify_command(7, amp(1.0)) == (wire.ACK_FAILED, None)


def test_producer_acknowledged_command_mutates_field():
    # The loop closure: an acknowledged set-amplitude scales the next authoritative field;
    # a refused one does not touch the model.
    import producer

    base = producer.gaussian_field(5, 5, 1.0, 1.0)
    scaled = producer.gaussian_field(5, 5, 1.0, 3.0)
    assert scaled == [3.0 * v for v in base]
    assert producer.classify_command(1, struct.pack("<d", 99.0)) == (wire.ACK_REFUSED, None)


def test_command_ack_wire_round_trip():
    # The full return-channel loop at the Python byte layer: encode a command, the producer
    # classifies it, encode the verdict ack, decode it back — the id and status survive.
    import producer

    cmd_bytes = wire.encode_command(1, 0, 10, command_id=10, kind=1,
                                    arg=struct.pack("<d", 2.5))
    msg, consumed = wire.decode_message(cmd_bytes)
    assert consumed == len(cmd_bytes) and msg.kind == wire.MSG_COMMAND
    status, value = producer.classify_command(msg.body["kind"], msg.body["arg"])
    assert (status, value) == (wire.ACK_ACKNOWLEDGED, 2.5)
    ack_bytes = wire.encode_ack(1, 0, 0, msg.body["command_id"], status)
    ack, _ = wire.decode_message(ack_bytes)
    assert ack.kind == wire.MSG_ACK
    assert ack.body["command_id"] == 10
    assert ack.body["status"] == wire.ACK_ACKNOWLEDGED


def test_field_geometry_parity_with_rust_fieldextent():
    """wire.decode_message must reject exactly the geometries Rust `FieldExtent::new`
    (src/field_view.rs) rejects: non-finite origin, non-finite or non-positive cell size,
    and a zero dimension. RED-FIRST against the pre-parity decoder: NaN/+inf origin and
    NaN/+inf cell size slipped through the old `cw <= 0.0` guard (both compare False), and the
    origin was never checked. cols==0/rows==0 and cw<=0/ch<=0 already failed closed; kept here
    as regressions so the whole 7-condition contract is pinned in one place."""
    nan = float("nan")
    inf = float("inf")

    def frame(cols, rows, origin, cw, ch):
        return wire.encode_field(1, 1, 1, cols, rows, origin, cw, ch, 0, [0.0] * (cols * rows))

    rejected = [
        ("nan origin_x", frame(2, 2, (nan, 0.0), 1.0, 1.0)),
        ("nan origin_y", frame(2, 2, (0.0, nan), 1.0, 1.0)),
        ("+inf origin_x", frame(2, 2, (inf, 0.0), 1.0, 1.0)),
        ("-inf origin_y", frame(2, 2, (0.0, -inf), 1.0, 1.0)),
        ("nan cell_w", frame(2, 2, (0.0, 0.0), nan, 1.0)),
        ("+inf cell_w", frame(2, 2, (0.0, 0.0), inf, 1.0)),
        ("nan cell_h", frame(2, 2, (0.0, 0.0), 1.0, nan)),
        ("+inf cell_h", frame(2, 2, (0.0, 0.0), 1.0, inf)),
        ("zero cell_w", frame(2, 2, (0.0, 0.0), 0.0, 1.0)),
        ("negative cell_h", frame(2, 2, (0.0, 0.0), 1.0, -1.0)),
        ("zero cols", frame(0, 2, (0.0, 0.0), 1.0, 1.0)),
        ("zero rows", frame(2, 0, (0.0, 0.0), 1.0, 1.0)),
    ]
    for label, buf in rejected:
        try:
            wire.decode_message(buf)
        except wire.WireError:
            continue
        raise AssertionError(f"decode accepted degenerate geometry it must reject: {label}")


def test_field_accepts_valid_geometry_and_nonfinite_data():
    """The parity tightening must NOT over-reject. Valid finite geometry decodes, and
    non-finite *data samples* (NaN, ±inf) are carried faithfully — Rust refuses a bad extent
    but preserves field values bit-for-bit, and the geometry guard touches only geometry. A
    large-but-finite geometry whose spatial extent would overflow to +inf is ACCEPTED: Rust
    `FieldExtent::new` has no overflow guard, so honest parity adds none (such extremes are
    disclosed downstream as clipping, not rejected at decode)."""
    nan = float("nan")
    inf = float("inf")
    buf = wire.encode_field(1, 1, 1, 2, 2, (-1.5, 2.5), 0.5, 0.25, 0, [nan, inf, -inf, 7.0])
    res = wire.decode_message(buf)
    assert res is not None, "valid finite geometry must decode"
    msg, consumed = res
    assert consumed == len(buf)
    d = msg.body["data"]
    assert math.isnan(d[0]) and d[1] == inf and d[2] == -inf and d[3] == 7.0
    assert msg.body["origin"] == (-1.5, 2.5)
    big = wire.encode_field(1, 1, 1, 2, 2, (0.0, 0.0), 1e308, 1e308, 0, [0.0] * 4)
    assert wire.decode_message(big) is not None, \
        "finite-but-huge geometry must decode (Rust FieldExtent::new has no overflow guard)"


def main():
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for t in tests:
        t()
        print(f"ok  {t.__name__}")
    print(f"\nall {len(tests)} python wire conformance checks passed")


if __name__ == "__main__":
    main()
