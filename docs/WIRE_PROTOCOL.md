# The LibGibson producer wire protocol (v1)

Wave 5 of *Operation Instrumental Convergence* (issue #125). A narrow, versioned, **bounded**
byte contract by which an out-of-process scientific producer — e.g. the SmartChem Python
forcing model — publishes identified scalar-field observations the Rust presenter renders,
and receives ordered edits back.

- **Producer** owns its data and its model ticks. It computes a field each logical tick and
  writes frames. It links nothing: the reference producer (`bindings/python/producer.py`)
  needs only the Python standard library.
- **Presenter** owns frame realization and interaction. It decodes the stream
  (`gibson::wire::FrameDecoder`), stores the latest observation per source in a bounded
  `FieldInbox`, renders it through `FieldView`, and returns edits as ordered commands on the
  bounded `SnapshotBridge`.

The protocol is **transport-agnostic**: `gibson::wire` is I/O-free and works on byte
buffers. The canary pipes the producer's stdout into the presenter, but a socket serves
equally.

## Framing

Every frame is a 32-byte header followed by a typed payload. **All integers are
little-endian.** `f64` is IEEE-754 little-endian; non-finite values (`NaN`, `±∞`) are carried
faithfully by their exact bit pattern and disclosed downstream (`Sample::NonFinite`), never
coerced.

### Header (32 bytes)

| Offset | Size | Field         | Notes                                              |
|-------:|-----:|---------------|----------------------------------------------------|
| 0      | 4    | `magic`       | ASCII `GWIR`; present on every frame for resync.   |
| 4      | 2    | `version`     | `1`. Any other value is refused (fatal).           |
| 6      | 2    | `msg_type`    | 1=FIELD, 2=COMMAND, 3=ACK, 4=RETIRE.               |
| 8      | 4    | `epoch`       | Producer run/restart generation.                   |
| 12     | 8    | `sequence`    | Monotonic within an epoch.                          |
| 20     | 8    | `source`      | Source identity (`FrameSource` `u64`).             |
| 28     | 4    | `payload_len` | Payload byte count. **Hard-capped before alloc.**  |

### FIELD payload (`msg_type = 1`)

| Offset | Size | Field        | Notes                                    |
|-------:|-----:|--------------|------------------------------------------|
| 0      | 4    | `cols`       | > 0                                       |
| 4      | 4    | `rows`       | > 0                                       |
| 8      | 8    | `origin_x`   | f64, finite                               |
| 16     | 8    | `origin_y`   | f64, finite                               |
| 24     | 8    | `cell_w`     | f64, finite, **> 0**                      |
| 32     | 8    | `cell_h`     | f64, finite, **> 0**                      |
| 40     | 8    | `model_tick` | the producer's logical tick (Law L6)      |
| 48     | 4    | `flags`      | bit0 = has_mask; other bits refused       |
| 52     | 8·N  | `data`       | `N = cols*rows` f64, **row-major** (`iy*cols+ix`) |
| 52+8N  | N    | `mask`       | present iff has_mask; byte 1=present, 0=masked |

Geometry decodes into a validated `FieldExtent` — the *same* type pointer selection uses —
so a producer's world↔index mapping is identical to the editor's (Law L3). A geometry
`FieldExtent::new` rejects (non-finite/zero cell size, zero dimension) is a malformed frame.

### COMMAND payload (`msg_type = 2`) — presenter → producer

| Offset | Size | Field        |
|-------:|-----:|--------------|
| 0      | 8    | `command_id` (ordered identity) |
| 8      | 4    | `kind` (application-defined)     |
| 12     | 4    | `arg_len`                        |
| 16     | …    | `arg` (opaque)                   |

### ACK payload (`msg_type = 3`)

| Offset | Size | Field        |
|-------:|-----:|--------------|
| 0      | 8    | `command_id` |
| 8      | 4    | `status` (0=Accepted,1=Delivered,2=Acknowledged,3=Refused,4=Failed) |

### RETIRE payload (`msg_type = 4`)

Empty (`payload_len = 0`). The `source` in the header identifies the retiring source.

## Chronology (Law L6)

Ordering is lexicographic on `(epoch, sequence)`. A strictly-greater cursor supersedes;
anything not strictly greater is **stale** and dropped. A **higher epoch always wins** —
that is how a restarted producer (whose sequence reset to low numbers) is accepted rather
than rejected. `model_tick` is independent: a frame carries the logical tick it depicts,
distinct from its transport sequence.

## Bounds — safe against a diseased producer

- **Length-prefix OOM is closed.** `payload_len` is checked against
  `WireLimits::max_payload_bytes` *before* any payload is buffered or allocated. The decoder
  never allocates toward a frame it would reject, and its accumulation buffer is itself
  bounded to one maximum frame.
- **Field cell count** is bounded by `WireLimits::max_field_cells`.
- **Retained observations** are bounded by `InboxCapacity::max_sources` × `max_field_cells`:
  latest-wins per source, a new source beyond the ceiling is refused (not evicted).
- **Commands** are bounded by the `SnapshotBridge` capacity: an accepted edit is delivered
  exactly once in order; at the ceiling an `issue` is a typed `Refused`, never silent growth.

## Error discipline

Every malformed condition is a typed `WireError`, never a panic or a silent drop.

- **Fatal** (desynchronizes the stream → retire the source): bad magic, unsupported version,
  oversized declared length, buffer overflow, truncated final frame.
- **Recoverable** (one frame skipped; the stream resynchronizes on the next frame boundary):
  unknown message type, field-too-large, length/dimension mismatch, bad extent, unknown flag
  bits, bad mask byte, bad ack status.

## Conformance

`bindings/python/testdata/golden_field_v1.bin` is produced by `producer.py --golden`.
`tests/wire_conformance.rs` (CI-gated) decodes it in Rust and re-encodes it **byte-for-byte**;
`bindings/python/test_wire.py` does the Python half. The vector is finite + ±∞ only
(deterministic on every platform); NaN bit-preservation is checked per-language in the
round-trip tests.

## Boundary — what this does not promise

An **in-memory, in-process** transport plus a byte format. No durability and no exactly-once
crash recovery: a process death loses the buffer, and "delivered" means handed out of the
bridge in-process, not persisted or confirmed applied. A refused command is not retained —
refusal is explicit backpressure the producer re-offers.

Field geometry round-trips **bit-exactly for all inputs**: the encoder reads the origin and
cell sizes straight off `FieldExtent`'s `origin()`/`cell_w()`/`cell_h()` accessors and the
decoder reconstructs the extent from exactly those four `f64`s. (Earlier revisions reconstructed
the origin from a derived cell corner, which could drop low mantissa bits when the origin
magnitude dwarfed the cell size; the accessors close that boundary.)
