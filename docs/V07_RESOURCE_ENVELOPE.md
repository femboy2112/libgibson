# v0.7.0 resource and latency envelope

This page restates the resource and latency evidence for 0.7.0 (qualification gate G14). It makes
**no new measurement**. Every number is quoted from the source it cites, with that source's host
and conditions. A number holds for those conditions only; none is a universal FPS or latency
promise. The v0.6 envelope, [`V06_PERFORMANCE_ENVELOPE.md`](V06_PERFORMANCE_ENVELOPE.md), still
covers everything listed under "Not re-measured" below.

The **Gate** column says whether a test or self-check enforces the number, or whether it was
recorded once.

## Idle and wake (library, enforced)

| Quantity | Bound | Gate |
|---|---|---|
| Idle `run_once` (#264) | over 1 s with `max_wait` 100 ms and nothing pending: at most 40 returns, and `cpu_ms * 4 < wall_ms` (under a quarter of a core); PTY at 1/60/1000 fps, headless, and rootless | `tests/run_once_idle_pty.rs` (`idle_tty_loop_sleeps_*`, `idle_headless_loop_sleeps_off_tty`, `rootless_tty_loop_sleeps`) |
| Input during an idle wait | the wait ends in under 4 s against an 8 s `max_wait` | `arriving_input_ends_an_idle_wait_promptly` |
| Pending frame | renders near its 100 ms budget at 10 fps, not at a 5 s `max_wait` | `a_pending_frame_renders_at_its_budget_not_at_max_wait` |
| Exit after a termination signal | up to `max_wait` plus one frame (an interrupted poll resumes for the time left) | contract text, [`TERMINAL_LEASE_CONTRACT.md`](TERMINAL_LEASE_CONTRACT.md) (#268). One consumer measured 33–45 ms with a 50–100 ms wait; that measurement is not in this repository |
| Poll on a hung-up terminal | returns `BrokenPipe` promptly, with SIGHUP default, ignored or handled | `tests/pty_hangup_matrix.rs` (single attempt in CI) |

## Retained memory (enforced)

| Quantity | Bound | Gate |
|---|---|---|
| Headless C capture with a drain each frame (#297/#306) | VmRSS growth below 2048 KiB + 4 × the largest frame, at 40×15, 80×24 and 240×80 over 3000 frames; the undrained control grows ≥ 19 MiB | `bindings/sdk_forcing/headless_capture_drain.c`, installed-SDK consumer in CI |
| Undrained headless capture (why the drain exists) | a frame whose every cell changes adds about 657 B at 40×15, 1,995 B at 80×24 and 19,387 B at 240×80; an unchanged frame adds nothing | measured on the v0.6.0 candidate, [`RELEASE_CONTRACT.md`](RELEASE_CONTRACT.md) §9 |
| Streaming viewport example (#346) | logical budgets of 32 KiB of source, 4,096 cached rows and 8 KiB per block, with eviction receipts. These are logical budgets, not process RSS. | `streaming_viewport` `budget_tests` |

## Frame cost (recorded; one host)

**Framebuffer adapter.** From [`bindings/framebuffer_adapter/README.md`](../bindings/framebuffer_adapter/README.md), P7. SDK from `main` `5790f1a`; 2000 frames after a 200-frame warm-up; real PTY through `pty_harness.py`.

| Grid | bytes / frame | total mean / p99 µs | 175 tics at 35 Hz |
|---|---|---|---|
| 80×24 | 39 KB | 903.9 / 1,545.8 | 35.0 fps, 0 dropped |
| 160×50 | 128 KB | 3,066.1 / 3,528.9 | 34.9 fps, 0 dropped |
| 200×60 | 169 KB | 4,149.6 / 4,998.9 | 34.9 fps, 0 dropped |
| 320×100 | 355 KB | 10,088.1 / 12,754.5 | 34.9 fps, 0 dropped (one re-run: 22.1 fps, 29 dropped) |

- **Gated by the adapter self-check:**
  - at least 34.0 fps with 0 dropped tics at 80×24, 160×50 and 200×60;
  - headless p99 of lower + render (frames 200–1999) at most 28,571 µs, one 35 Hz tic;
  - VmHWM growth from frame 1100 to 2000 at most 1024 KiB.
- **320×100 is measured but not gated:** it depends on load.
- **Render share:** about 90% of the frame cost is in `gibson_render`.

**Other frame-cost figures:**
- **256-colour render after #263:** p50 went from 8217 to 372 µs at 80×25, from 14706 to 847 µs at 120×40, and from 35097 to 1962 µs at 200×60. Equality with the old output is proven by the exhaustive test `nearest256_exhaustive`, which is `#[ignore]` and run on demand, not in CI. The timing was recorded once.
- **Node I, C forcing loop:** 223–336 µs per frame across three runs (48×16 raster, release library, development box; "not a benchmark").
- **Node F, pacing:** a slow PTY reader still gets 172–175 simulation tics (at least 94% of those owed) while about 15 frames are presented (`tests/deep_signal_pacing_pty.rs`).
- **Raster bench baseline:** per-stage cost, bytes and allocations for 21 configurations, peak RSS 6.7 MB ([`V07_BASELINE.md`](V07_BASELINE.md)). It was measured at the **v0.6 integration merge `f6cd8c7`**, so it is a baseline, not a 0.7 measurement.

## Exit and restore (adapter P6, real PTY 80×24)

| Action | Recorded result |
|---|---|
| `q` | exit 0 in 0.01 s |
| SIGTERM | death by signal 15 (re-raised after restore) in 0.04 s |
| Hangup with SIGHUP ignored | exit 0 in 0.01 s, child CPU 0.09 s in total (no spin) |

In every row, termios after the action equals termios before it. The self-check bound is under 2 s
wall time and under 3 s CPU.

## Not re-measured since v0.6 (the v0.6 envelope stands)

- Observation round trip and command→verdict latency.
- Input→dispatch latency (v0.6: 3.1/2.8 µs).
- Scene evaluation, and renderer generation at headless 100×32.
- Write+flush into a pipe.
- Producer cadence and coalescing.
- Spawn→first field and supervised cleanup.
- Presenter and producer process RSS.

Never measured, then or now:
- the observation age in streaming mode;
- physical display latency or FPS (the PTY harness reader discards bytes).

There is no measurement of per-poll allocation either. The input source is reused across polls
by design (`src/session.rs`), and nothing measures it.
