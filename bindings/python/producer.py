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
import struct
import sys

import wire

# The producer's one mutable model parameter and its command contract. `set-amplitude`
# (kind 1) carries an 8-byte little-endian f64 in [AMPLITUDE_MIN, AMPLITUDE_MAX]; an
# acknowledged one scales the published field, so an accepted edit is visible in the next
# authoritative frame. Mirrors `ProducerModel` in `tests/wire_transport.rs`.
KIND_SET_AMPLITUDE = 1
# Lockstep control (test-only, --lockstep): advance the model one tick and emit the next
# frame. It is a control signal, not a model edit, so it is never acked. Distinct from
# KIND_SET_AMPLITUDE; the normal serve() path never sees it.
KIND_ADVANCE = 2
AMPLITUDE_MIN = 0.0
AMPLITUDE_MAX = 10.0


def gaussian_field(cols, rows, t, amplitude=1.0):
    """A moving Gaussian bump scaled by `amplitude`, finite everywhere. `t` advances the
    center on a circle so successive frames differ visibly; `amplitude` is set by an
    acknowledged `set-amplitude` command, so an accepted edit changes the next field."""
    cx = (cols - 1) / 2.0 + (cols / 4.0) * math.cos(t * 0.3)
    cy = (rows - 1) / 2.0 + (rows / 4.0) * math.sin(t * 0.3)
    sigma = max(cols, rows) / 6.0
    data = []
    for iy in range(rows):
        for ix in range(cols):
            d2 = (ix - cx) ** 2 + (iy - cy) ** 2
            data.append(amplitude * math.exp(-d2 / (2.0 * sigma * sigma)))
    return data


def classify_command(kind, arg):
    """Decide a return-channel command's fate against this producer's contract. Returns
    ``(status, value)``: the matching ``ACK_*`` status and, for an acknowledged
    ``set-amplitude``, the new amplitude (otherwise ``None``). Pure, so the conformance suite
    drives it directly; mirrors the Rust ``ProducerModel::classify_and_apply`` exactly.

    * ``kind == KIND_SET_AMPLITUDE`` with an 8-byte LE ``f64`` finite and in range ->
      ``ACK_ACKNOWLEDGED`` (the caller applies ``value``).
    * the same kind, value out of range or non-finite -> ``ACK_REFUSED`` (typed backpressure;
      the model is untouched).
    * a wrong-length arg or an unknown ``kind`` -> ``ACK_FAILED``.
    """
    if kind != KIND_SET_AMPLITUDE:
        return wire.ACK_FAILED, None
    if len(arg) != 8:
        return wire.ACK_FAILED, None
    (value,) = struct.unpack("<d", arg)
    if not math.isfinite(value) or not (AMPLITUDE_MIN <= value <= AMPLITUDE_MAX):
        return wire.ACK_REFUSED, None
    return wire.ACK_ACKNOWLEDGED, value


def drain_and_ack_commands(decoder_buf, source, state):
    """Best-effort, non-blocking: read any pending COMMAND frames from stdin, decide each
    command's fate with :func:`classify_command`, apply an acknowledged one to ``state``
    (``state["amplitude"]``), and ACK every command with its TRUE verdict — acknowledged,
    refused, or failed — never a blanket success and never a silent drop. Returns the updated
    buffer. Ordered identity is preserved; the ack's `command_id` echoes the command's."""
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
            status, value = classify_command(msg.body["kind"], msg.body["arg"])
            if status == wire.ACK_ACKNOWLEDGED:
                state["amplitude"] = value  # the accepted edit mutates the model
            out = wire.encode_ack(source, 0, ack_seq, msg.body["command_id"], status)
            sys.stdout.buffer.write(out)
            sys.stdout.buffer.flush()
            ack_seq += 1
    return decoder_buf


def _emit_field(out, args, seq, amplitude):
    """Write one FIELD frame for `seq` at `amplitude`. Byte-for-byte what serve() emits per
    tick, so the lockstep driver and the streaming server produce identical wire bytes."""
    data = gaussian_field(args.cols, args.rows, float(seq), amplitude)
    out.write(wire.encode_field(
        source=args.source, epoch=args.epoch, sequence=seq,
        cols=args.cols, rows=args.rows, origin=(args.origin_x, args.origin_y),
        cell_w=args.cell_w, cell_h=args.cell_h, model_tick=seq, data=data))
    out.flush()


def _read_one_message(buf):
    """Block until one whole wire message decodes from `buf`, reading stdin as needed. Returns
    ``(Message, remaining_bytearray)`` or ``None`` at EOF/corruption. This blocking read is the
    lockstep driver's synchronization receipt -- there are no sleeps anywhere in that path."""
    while True:
        try:
            decoded = wire.decode_message(buf)
        except wire.WireError:
            return None
        if decoded is not None:
            msg, consumed = decoded
            return msg, buf[consumed:]
        chunk = sys.stdin.buffer.read1(65536)
        if not chunk:
            return None
        buf += chunk


def serve_lockstep(args):
    """Deterministic, sleep-free cross-process driver (test-only, --lockstep).

    Emit frame seq=1, then block reading inbound messages: a ``KIND_ADVANCE`` command advances
    to the next seq and emits its frame (a control signal -- never acked); any other command is
    classified, acked with its TRUE verdict, and applied unless ``--freeze-amplitude``. The
    presenter/test drives the model one frame at a time, using the ack as the only
    synchronization receipt. Ends with a RETIRE when stdin closes or ``--frames`` is reached.

    ``--freeze-amplitude`` is the causal mutation control: it acks a valid set-amplitude
    ``Acknowledged`` but suppresses the model update, so the causal test's "the next frame
    reflects the accepted edit" assertion MUST fail -- proving the test detects the causal link,
    not merely the ack."""
    out = sys.stdout.buffer
    state = {"amplitude": 1.0}
    seq = 1
    buf = bytearray()
    _emit_field(out, args, seq, state["amplitude"])
    while seq < args.frames:
        decoded = _read_one_message(buf)
        if decoded is None:
            break
        msg, buf = decoded
        if msg.kind != wire.MSG_COMMAND:
            continue
        kind = msg.body["kind"]
        if kind == KIND_ADVANCE:
            seq += 1
            _emit_field(out, args, seq, state["amplitude"])
            continue
        status, value = classify_command(kind, msg.body["arg"])
        if status == wire.ACK_ACKNOWLEDGED and not args.freeze_amplitude:
            state["amplitude"] = value
        out.write(wire.encode_ack(args.source, args.epoch, seq, msg.body["command_id"], status))
        out.flush()
    out.write(wire.encode_retire(args.source, args.epoch, seq + 1))
    out.flush()


def serve(args):
    """Stream `args.frames` field observations, then retire the source."""
    out = sys.stdout.buffer
    stdin_buf = bytearray()
    # The producer's authoritative, mutable model state. An acknowledged set-amplitude command
    # moves it, so the field published on the next tick reflects the accepted edit.
    state = {"amplitude": 1.0}
    # Pace with a plain sleep; the presenter's bounded inbox coalesces anything it misses.
    import time

    for seq in range(1, args.frames + 1):
        data = gaussian_field(args.cols, args.rows, float(seq), state["amplitude"])
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
        stdin_buf = drain_and_ack_commands(stdin_buf, args.source, state)
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
    ap.add_argument("--lockstep", action="store_true",
                    help="deterministic frame-at-a-time driver for the causal test (test-only)")
    ap.add_argument("--freeze-amplitude", action="store_true",
                    help="ack a valid set-amplitude Acknowledged but do NOT apply it (causal mutation control)")
    args = ap.parse_args()

    if args.golden:
        golden(args)
    elif args.lockstep:
        serve_lockstep(args)
    else:
        serve(args)


if __name__ == "__main__":
    main()
