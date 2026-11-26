# M5-A wrong-value mutation controls

Each patch replaces one `stage::motion` function with a plausible but **wrong** value. The
`tests/stage_motion.rs` suite must then fail on **assertions** (panics in `tests/`), not on stub
panics. This is what shows that the tests discriminate. A `todo!()` stub shows only that the tests
compile and exercise the API.

| patch | what it does | expected to be caught by |
|---|---|---|
| `sample_frame0.patch` | `FrameSequence::sample` always returns frame 0 with no deadline | G1 sampling tests, G4 |
| `hash_zero.patch` | the G3 `mix` returns 0, so jitter is 0 | G3 golden and seed-0 tests |
| `nth_origin.patch` | `CueSchedule::nth` always returns the origin | all G3 tests |
| `planes_flat.patch` | `LayerSlots::planes` ignores slot offsets and gives every fill `z = 0` | G2 tests |

The patches are evidence only. They are never applied to `src` in a commit. To reproduce them,
use `run.sh`. Recorded results are in `gate3-results.txt` (the first three patches; G2 is run
separately).

Accepted reading: the five G3 structural tests (slot and gap bounds, `nth`/`next_after`
consistency, overflow, O(1)) still pass under `hash_zero`. A zero-jitter schedule really does
satisfy those laws. The hash value is pinned by the golden-table, reference and seed-0 tests.
