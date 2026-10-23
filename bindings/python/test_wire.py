#!/usr/bin/env python3
"""Local conformance checks for the Wave 5 producer codec (`wire.py`).

Pure standard library; run directly:  python3 bindings/python/test_wire.py

The Rust side gates the *same* golden vector in CI (`tests/wire_conformance.rs`): Rust
decodes these Python-produced bytes and re-encodes them byte-for-byte. This file is the
Python half — it is not run by CI, so run it locally when you touch the format.
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


def main():
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for t in tests:
        t()
        print(f"ok  {t.__name__}")
    print(f"\nall {len(tests)} python wire conformance checks passed")


if __name__ == "__main__":
    main()
