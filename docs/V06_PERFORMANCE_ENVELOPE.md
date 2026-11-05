# v0.6 Performance & Resource Envelope (RXIV Gate G2)

A **measured** envelope for the live-film / instrument stack — not a benchmark claim, not a universal
frame rate. Every number below was taken on one host, at one exact SHA, from a monotonic clock or
`/proc`, by `field_cinema --envelope` (`examples/field_cinema/envelope.rs`). Nothing is extrapolated.

## Provenance

| | |
|---|---|
| Code measured | `9ca3ed431783fa4b0f4c0c86f9abacb92e7495bc` (clean tree) |
| Command | `cargo build --release --example field_cinema` then `target/release/examples/field_cinema --envelope --frames 2000 --stream-secs 10` (run twice) |
| Toolchain | `rustc 1.97.0-nightly (a5c825cd8 2026-04-14)`, release profile |
| Host | AMD Ryzen 5 (8 logical CPUs), 7.0 GiB RAM, Linux `7.0.0-38-generic`, load ≈1.4–1.8 (an interactive workstation, not an isolated runner) |
| Producer | `bindings/python/producer.py`, Python 3.12.3, 48×24 field |
| Wall time | 17.2 s per run; `/usr/bin/time` max RSS 45 476 KiB |

## Results (run 1 / run 2)

**A. Lockstep film — 2000 frames, an edit every 25 frames (80 edits, 80 evidenced transforms).**

| Measure | n | p50 | p99 | max |
|---|---|---|---|---|
| Observation round trip (advance → admitted field) | 2000 | 1059 / 1058 µs | 1079 / 2127 µs | 2588 / 2197 µs |
| Command → correlated verdict | 80 | 1062 / 1061 µs | 1074 / 1075 µs | 1088 / 1154 µs |
| Input → dispatch (key → command written + flushed) | 80 | 3.1 / 2.8 µs | 5.8 / 6.3 µs | 8.1 / 7.3 µs |
| `Film::on_field` (stage + arc + record) | 2000 | 14.6 / 14.4 µs | 29.9 / 42.4 µs | 85.5 / 83.0 µs |
| Scene build + effect eval + lowering (`Film::view`, 100×32) | 2000 | 94 / 95 µs | 271 / 295 µs | 315 / 507 µs |
| Renderer generation (layout + paint + diff + compile, headless 100×32) | 2000 | 882 / 901 µs | 1671 / 1883 µs | 1863 / 2288 µs |

Retention after 2000 frames: trace 2161 events (not truncated), capture 2001 frames (not
truncated, ≈18 MiB of samples — inside the 256 MiB capture budget). Transport counters all zero
(no foreign/stale/dropped/skipped/coalesced in lockstep). Spawn → first admitted field 27 / 47 ms.
Supervised cleanup (stdin EOF → producer exits → reaped): 6.4 ms, graceful.

**Calibration note (instrument rule).** Both round trips sit at ≈1.06 ms because `LiveSession`
polls its non-blocking reader with a 1 ms sleep when nothing is ready. They are therefore UPPER
bounds quantized by that poll interval — not the producer's turnaround — and they bound the
lockstep observation age from above. Streaming-mode observation age is **not measured**: the wire
carries no producer clock.

**B. Terminal write path — 400 film frames committed into an OS pipe drained concurrently
(`wc -c`).**

| Measure | n | p50 | p99 | max |
|---|---|---|---|---|
| Renderer `write + flush` | 400 | 5.8 / 5.9 µs | 14.2 / 16.4 µs | 90.1 / 28.1 µs |

Bytes per frame: mean 11 909, p50 11 964, max 13 420 (4 763 717 bytes drained, matching what the
renderer reported emitting). An OS pipe is not a physical terminal: this is the cost of handing a
frame to the kernel, **not** a display frame rate, and a slow terminal would block longer.

**C. Streaming — the free-running producer (`--hz 0`) consumed at ≈60 Hz for 10 s.**

| Measure | Run 1 | Run 2 |
|---|---|---|
| Producer model cadence | 3270 ticks/s | 3465 ticks/s |
| Consumer observation rate | 62.0 /s (620 frames) | 61.9 /s (620 frames) |
| Ticks never observed (lawfully coalesced) | 32 109 | 34 069 |
| Reader coalesced / dropped / stale / decode skips | 32 129 / 0 / 0 / 0 | — / 0 / 0 / 0 |
| Retained FIELD bytes high-water | 9 KiB (one field) | 9 KiB |

The bounded coalescing reader held **one** field's payload while discarding everything a 60 Hz
consumer could not show — memory independent of producer rate. Cleanup: **2.0 s, not graceful** —
the streaming producer does not stop on stdin EOF, so the bounded kill backstop
(`SHUTDOWN_GRACE`) reaps it. (The lockstep producer does exit on EOF — run A.)

**Whole-process RSS.** Presenter peak 45.5 MiB (VmHWM; current 23 MiB) after all three workloads,
dominated by the retained capture; producer peak ≈13 MiB.

## What this does not establish

- One host, one Python, one terminal size; no cross-machine or CI-runner envelope.
- No physical display latency or FPS (nothing here touches a real terminal emulator).
- No long-duration soak beyond the 2000-frame / 10-second workloads.
- Percentiles at n = 80 (edits) are coarse; p99 there is effectively the second-largest sample.
