"""LibGibson Wave 5 producer wire codec — pure Python standard library.

This is the *producer* half of ``gibson::wire``: a SmartChem-style forcing model owns its
data and its model ticks, computes a scalar field each tick, and publishes it as a
self-describing, identified frame the Rust presenter renders. The producer never links
``libgibson.so`` — it only needs ``struct`` — so it runs in any Python 3.

The byte format is the contract (little-endian throughout):

    Header (32 bytes):
        magic[4]="GWIR" | version u16=1 | msg_type u16| epoch u32 | sequence u64
        | source u64 | payload_len u32

    FIELD payload (msg_type=1):
        cols u32 | rows u32 | origin_x f64 | origin_y f64 | cell_w f64 | cell_h f64
        | model_tick u64 | flags u32 | data[cols*rows] f64 (row-major) | mask[cols*rows] u8?
        flags bit0 = has_mask; mask byte 1 = present/valid, 0 = masked.

    COMMAND payload (msg_type=2):  command_id u64 | kind u32 | arg_len u32 | arg[arg_len]
    ACK     payload (msg_type=3):  command_id u64 | status u32
    RETIRE  payload (msg_type=4):  (empty)

Non-finite data (NaN, ±inf) is carried faithfully as its IEEE-754 bit pattern and is
disclosed, never coerced, on the far side.
"""

import struct

MAGIC = b"GWIR"
VERSION = 1
HEADER_BYTES = 32

MSG_FIELD = 1
MSG_COMMAND = 2
MSG_ACK = 3
MSG_RETIRE = 4

FLAG_HAS_MASK = 1

# Ack status codes — must match gibson::wire::AckStatus.
ACK_ACCEPTED = 0
ACK_DELIVERED = 1
ACK_ACKNOWLEDGED = 2
ACK_REFUSED = 3
ACK_FAILED = 4

# Default ceilings mirroring gibson::wire::WireLimits.
DEFAULT_MAX_FIELD_CELLS = 1 << 20
DEFAULT_MAX_PAYLOAD_BYTES = DEFAULT_MAX_FIELD_CELLS * (8 + 1) + 52 + 64

_HEADER = struct.Struct("<4sHHIQQI")
_FIELD_HEAD = struct.Struct("<IIddddQI")
_COMMAND_HEAD = struct.Struct("<QII")
_ACK = struct.Struct("<QI")


class WireError(Exception):
    """A malformed or out-of-contract frame."""


def _header(msg_type, source, epoch, sequence, payload_len):
    return _HEADER.pack(MAGIC, VERSION, msg_type, epoch, sequence, source, payload_len)


def encode_field(source, epoch, sequence, cols, rows, origin, cell_w, cell_h,
                 model_tick, data, mask=None):
    """Encode a scalar-field observation.

    ``data`` is a flat row-major sequence of ``cols*rows`` floats (index =
    ``iy*cols + ix``). ``mask``, if given, is a parallel sequence of truthy/falsey values
    where truthy means present/valid. Returns the complete frame bytes.
    """
    cells = cols * rows
    if len(data) != cells:
        raise WireError(f"data has {len(data)} values, expected {cells}")
    has_mask = mask is not None
    if has_mask and len(mask) != cells:
        raise WireError(f"mask has {len(mask)} values, expected {cells}")
    flags = FLAG_HAS_MASK if has_mask else 0
    payload = bytearray()
    payload += _FIELD_HEAD.pack(cols, rows, origin[0], origin[1], cell_w, cell_h,
                                model_tick, flags)
    payload += struct.pack(f"<{cells}d", *data)
    if has_mask:
        payload += bytes(1 if m else 0 for m in mask)
    return _header(MSG_FIELD, source, epoch, sequence, len(payload)) + bytes(payload)


def encode_command(source, epoch, sequence, command_id, kind, arg=b""):
    """Encode an edit for the producer."""
    payload = _COMMAND_HEAD.pack(command_id, kind, len(arg)) + bytes(arg)
    return _header(MSG_COMMAND, source, epoch, sequence, len(payload)) + payload


def encode_ack(source, epoch, sequence, command_id, status):
    """Encode an acknowledgement."""
    payload = _ACK.pack(command_id, status)
    return _header(MSG_ACK, source, epoch, sequence, len(payload)) + payload


def encode_retire(source, epoch, sequence):
    """Encode a source retirement (no payload)."""
    return _header(MSG_RETIRE, source, epoch, sequence, 0)


class Message:
    """A decoded message: source/chronology header plus a typed body.

    ``kind`` is one of the ``MSG_*`` constants; ``body`` is a dict whose shape depends on
    the kind (field/command/ack/retire).
    """

    __slots__ = ("source", "epoch", "sequence", "kind", "body")

    def __init__(self, source, epoch, sequence, kind, body):
        self.source = source
        self.epoch = epoch
        self.sequence = sequence
        self.kind = kind
        self.body = body

    def __repr__(self):
        return (f"Message(source={self.source}, epoch={self.epoch}, "
                f"sequence={self.sequence}, kind={self.kind}, body={self.body})")


def decode_message(buf, limits_max_payload=DEFAULT_MAX_PAYLOAD_BYTES,
                   limits_max_cells=DEFAULT_MAX_FIELD_CELLS):
    """Decode one frame from the front of ``buf``.

    Returns ``(Message, consumed_bytes)`` when a whole frame is present, or ``None`` when
    more bytes are needed. Raises :class:`WireError` on a malformed frame. This mirrors the
    Rust ``FrameDecoder`` closely enough for conformance, including the pre-allocation
    length cap.
    """
    if len(buf) < HEADER_BYTES:
        return None
    magic, version, msg_type, epoch, sequence, source, payload_len = \
        _HEADER.unpack_from(buf, 0)
    if magic != MAGIC:
        raise WireError(f"bad magic {magic!r}")
    if version != VERSION:
        raise WireError(f"unsupported version {version}")
    if payload_len > limits_max_payload:
        raise WireError(f"payload {payload_len} exceeds cap {limits_max_payload}")
    frame_len = HEADER_BYTES + payload_len
    if len(buf) < frame_len:
        return None
    p = bytes(buf[HEADER_BYTES:frame_len])

    if msg_type == MSG_FIELD:
        body = _decode_field(p, limits_max_cells)
    elif msg_type == MSG_COMMAND:
        body = _decode_command(p)
    elif msg_type == MSG_ACK:
        body = _decode_ack(p)
    elif msg_type == MSG_RETIRE:
        if payload_len != 0:
            raise WireError("retire frame must have an empty payload")
        body = {}
    else:
        raise WireError(f"unknown message type {msg_type}")

    return Message(source, epoch, sequence, msg_type, body), frame_len


def _decode_field(p, max_cells):
    if len(p) < _FIELD_HEAD.size:
        raise WireError("field payload shorter than its head")
    cols, rows, ox, oy, cw, ch, model_tick, flags = _FIELD_HEAD.unpack_from(p, 0)
    if flags & ~FLAG_HAS_MASK:
        raise WireError(f"unknown field flags {flags:#x}")
    has_mask = bool(flags & FLAG_HAS_MASK)
    cells = cols * rows
    if cells > max_cells:
        raise WireError(f"field {cells} cells exceeds cap {max_cells}")
    mask_bytes = cells if has_mask else 0
    computed = _FIELD_HEAD.size + cells * 8 + mask_bytes
    if computed != len(p):
        raise WireError(f"declared payload {len(p)} != computed {computed}")
    if cw <= 0.0 or ch <= 0.0 or cols == 0 or rows == 0:
        raise WireError("degenerate field geometry")
    off = _FIELD_HEAD.size
    data = list(struct.unpack_from(f"<{cells}d", p, off))
    off += cells * 8
    mask = None
    if has_mask:
        raw = p[off:off + cells]
        for b in raw:
            if b not in (0, 1):
                raise WireError(f"mask byte {b} is neither 0 nor 1")
        mask = [b == 1 for b in raw]
    return {
        "cols": cols, "rows": rows, "origin": (ox, oy),
        "cell_w": cw, "cell_h": ch, "model_tick": model_tick,
        "data": data, "mask": mask,
    }


def _decode_command(p):
    if len(p) < _COMMAND_HEAD.size:
        raise WireError("command payload shorter than its head")
    command_id, kind, arg_len = _COMMAND_HEAD.unpack_from(p, 0)
    computed = _COMMAND_HEAD.size + arg_len
    if computed != len(p):
        raise WireError(f"declared payload {len(p)} != computed {computed}")
    return {"command_id": command_id, "kind": kind,
            "arg": bytes(p[_COMMAND_HEAD.size:])}


def _decode_ack(p):
    if len(p) != _ACK.size:
        raise WireError("ack payload wrong size")
    command_id, status = _ACK.unpack_from(p, 0)
    if status not in (ACK_ACCEPTED, ACK_DELIVERED, ACK_ACKNOWLEDGED,
                      ACK_REFUSED, ACK_FAILED):
        raise WireError(f"ack status {status} out of range")
    return {"command_id": command_id, "status": status}
