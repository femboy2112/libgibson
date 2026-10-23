#!/usr/bin/env python3
"""Reference SmartChem-facing producer for LibGibson Wave 5.

A scientific producer *owns its data and its model ticks*. This one computes a synthetic
scalar field each tick — a moving 2-D Gaussian bump on a fixed grid — and publishes it as
a ``gibson::wire`` FIELD frame to stdout. It links nothing: pure Python standard library.
The Rust presenter decodes the stream into a bounded ``FieldInbox`` and renders the latest
observation through ``FieldView``; edits come back as ordered COMMAND frames on stdin,
which this producer validates and ACKs.

Usage:
    producer.py                 # stream frames to stdout at --hz for --frames ticks
    producer.py --golden        # emit the fixed cross-language conformance vector
    producer.py --frames 20 --hz 30 --cols 48 --rows 24 --source 1

The presenter example (`examples/smartchem_canary.rs`) pipes this producer's stdout into
itself. The byte format lives in `wire.py` and `docs/WIRE_PROTOCOL.md`.
"""

import argparse
import math
import select
import sys

import wire


def gaussian_field(cols, rows, t):
    """A moving Gaussian bump, value in [0, 1], finite everywhere. `t` advances the center
    on a circle so successive frames differ visibly."""
    cx = (cols - 1) / 2.0 + (cols / 4.0) * math.cos(t * 0.3)
    cy = (rows - 1) / 2.0 + (rows / 4.0) * math.sin(t * 0.3)
    sigma = max(cols, rows) / 6.0
    data = []
    for iy in range(rows):
        for ix in range(cols):
            d2 = (ix - cx) ** 2 + (iy - cy) ** 2
            data.append(math.exp(-d2 / (2.0 * sigma * sigma)))
    return data


def drain_and_ack_commands(decoder_buf, source):
    """Best-effort, non-blocking: read any pending COMMAND frames from stdin and ACK them.
    Returns updated buffer. Ordered identity is preserved; an out-of-contract frame is
    refused with a FAILED ack rather than silently dropped."""
    # Poll stdin without blocking the producer's tick loop.
    while select.select([sys.stdin.buffer], [], [], 0)[0]:
        chunk = sys.stdin.buffer.read1(65536)
        if not chunk:
            break
        decoder_buf += chunk
    ack_seq = 0
    while True:
        try:
            decoded = wire.decode_message(decoder_buf)
        except wire.WireError:
            # Desync/corruption: stop draining this round, report nothing false.
            break
        if decoded is None:
            break
        msg, consumed = decoded
        decoder_buf = decoder_buf[consumed:]
        if msg.kind == wire.MSG_COMMAND:
            status = wire.ACK_ACKNOWLEDGED  # a real model would validate msg.body here
            out = wire.encode_ack(source, 0, ack_seq, msg.body["command_id"], status)
            sys.stdout.buffer.write(out)
            sys.stdout.buffer.flush()
            ack_seq += 1
    return decoder_buf


def serve(args):
    """Stream `args.frames` field observations, then retire the source."""
    out = sys.stdout.buffer
    stdin_buf = bytearray()
    # Pace with a plain sleep; the presenter's bounded inbox coalesces anything it misses.
    import time

    for seq in range(1, args.frames + 1):
        data = gaussian_field(args.cols, args.rows, float(seq))
        frame = wire.encode_field(
            source=args.source,
            epoch=args.epoch,
            sequence=seq,
            cols=args.cols,
            rows=args.rows,
            origin=(args.origin_x, args.origin_y),
            cell_w=args.cell_w,
            cell_h=args.cell_h,
            model_tick=seq,  # this producer's logical tick == its sequence here
            data=data,
        )
        out.write(frame)
        out.flush()
        stdin_buf = drain_and_ack_commands(stdin_buf, args.source)
        if args.hz > 0:
            time.sleep(1.0 / args.hz)

    out.write(wire.encode_retire(args.source, args.epoch, args.frames + 1))
    out.flush()


def golden(_args):
    """Emit the fixed cross-language conformance vector (finite + ±inf only, so the bytes
    are deterministic on every platform; NaN bit-preservation is tested per-language)."""
    out = sys.stdout.buffer
    out.write(wire.encode_field(1, 1, 1, 3, 2, (-1.0, 2.0), 0.5, 0.25, 100,
                                [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]))
    out.write(wire.encode_field(1, 1, 2, 2, 2, (0.0, 0.0), 1.0, 1.0, 101,
                                [42.0, float("inf"), float("-inf"), 7.5],
                                mask=[1, 0, 1, 1]))
    out.write(wire.encode_command(1, 0, 1, 7, 2, b"hi"))
    out.write(wire.encode_ack(1, 0, 2, 7, wire.ACK_ACKNOWLEDGED))
    out.write(wire.encode_retire(1, 1, 3))
    out.flush()


def main():
    ap = argparse.ArgumentParser(description="LibGibson Wave 5 reference producer")
    ap.add_argument("--golden", action="store_true",
                    help="emit the fixed conformance vector and exit")
    ap.add_argument("--frames", type=int, default=30)
    ap.add_argument("--hz", type=float, default=20.0, help="frames per second (0 = as fast as possible)")
    ap.add_argument("--cols", type=int, default=48)
    ap.add_argument("--rows", type=int, default=24)
    ap.add_argument("--source", type=int, default=1)
    ap.add_argument("--epoch", type=int, default=1)
    ap.add_argument("--origin-x", type=float, default=0.0)
    ap.add_argument("--origin-y", type=float, default=0.0)
    ap.add_argument("--cell-w", type=float, default=1.0)
    ap.add_argument("--cell-h", type=float, default=1.0)
    args = ap.parse_args()

    if args.golden:
        golden(args)
    else:
        serve(args)


if __name__ == "__main__":
    main()
