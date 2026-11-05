# Issue #15 — stock vs patched crossterm on a real PTY (RXIV Gate G4)

An isolated, real-backend A/B for issue #15 (a key that shares one readiness batch with a resize is
stranded until more input arrives). It depends only on `crossterm` and `portable-pty` — no
LibGibson code — so the only variable between the arms is crossterm itself.

- `child`: a quiet crossterm program — raw mode, prime the event source, announce `READY`, then
  deliberately do not poll for 300 ms; afterwards poll/read for 1.5 s and print `EV key r` or
  `STALLED`.
- `parent`: per trial, spawn `child` on a fresh 80×24 PTY, wait for `READY`, then stage the
  collision inside the child's non-polling window and classify the outcome.
  `ORDER=resize-key` (the calibrated failing order, same as `event_pressure_lab`'s `coincident`):
  resize the PTY (SIGWINCH), 10 ms later write `r`. `ORDER=key-resize` is the negative control.

```sh
cargo +1.98.1 build --bins                       # stock arm (crossterm =0.29.0)
ORDER=resize-key target/debug/parent "$PWD/target/debug/child" 30
ORDER=key-resize target/debug/parent "$PWD/target/debug/child" 30
# patched arm: swap the crossterm line in Cargo.toml for the fork (see the comment), rebuild, rerun.
```

## Result (2026-11-05, measured on the RXIV branch head; Linux 7.0.0-38, rustc 1.98.1)

| crossterm | `resize-key` (one batch, resize first) | `key-resize` (control) |
|---|---|---|
| stock `0.29.0` (crates.io, checksum `d8b9f2e4…`) | **0 / 30 delivered — 30 stalled** (an interleaved re-run: 0 / 30) | 30 / 30 delivered |
| fork `femboy2112/crossterm@2a195fb4` (upstream PR #1128) | **30 / 30 delivered** | 30 / 30 delivered |

**Reading.** OBSERVED, decisively: on this host the stall is in stock crossterm's Unix Mio source,
and the #1128 change delivers the stranded key without any further input. CONJECTURED, not shown
here: that the result transfers to every kernel/terminal (only one host was measured).

**Status of #15 — unchanged: blocked upstream.** #1128 is still open and unmerged (checked
2026-11-05); crates.io's newest crossterm is still `0.29.0`. LibGibson does not adopt the fork: a
`[patch.crates-io]` on an unmerged, force-pushable branch would silently change every downstream
build. #15 remains an explicitly scoped Engineering Alpha limitation (see the release contract's
limitations list); `tests/event_pressure_pty.rs` keeps pinning the stock stall and its `#[ignore]`d
delivery acceptance flips on when a fixed crossterm ships.
