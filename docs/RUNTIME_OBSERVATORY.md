# Runtime Observatory

A restrained, mission-control-style diagnostic instrument for watching three
specific LibGibson runtime contracts happen, instead of reading about them in
a probe's stdout dump: issue #10's trace-retention bound, the crossterm #1126
input-starvation collision (issue #15), and issue #11's process-global
terminal-lease contract. It lives at `examples/runtime_observatory.rs` plus
`examples/runtime_observatory/{diag,visual,million_tick,ghost_key,
ownership_duel}.rs`. It is a diagnostic instrument with a `--dump` path, not a
finished live TUI — see [Honest boundaries](#honest-boundaries) before relying
on anything here as a stability or completeness claim.

**Status: three grounded modes built, `--dump` only. Issues #10, #11 and #15
remain open** (#10 and #11 have merged fixes, exercised here; #15's upstream
input-starvation defect has no production fix — see
[Crossterm #1126 fix analysis](CROSSTERM_1126_FIX_ANALYSIS.md) and the
[Event Pressure Lab](EVENT_PRESSURE_LAB.md)). Built 2026-09-24 on
`claude/runtime-architecture-megaround`, Rust/Cargo `1.98.1`, Linux
`7.0.0-28-generic` x86_64.

## Try it

```sh
cargo run --release --example runtime_observatory -- --help
```

Every mode requires `--dump` today; there is no live/interactive loop (see
[Honest boundaries](#honest-boundaries)). `--dump` renders exactly one
deterministic frame to stdout and exits.

## The shared spine

All three modes render through the same scaffold in
`examples/runtime_observatory/visual.rs`, so the discipline below applies to
all of them, not just the mode that happens to be driving a real process.

- **Bounded diagnostic log.** `examples/runtime_observatory/diag.rs` defines
  `DiagRecord` (a monotonic timestamp, a sequence number, a `Category`, a
  `kind`, an explicit `source` string, and a value) and `DiagLog`, a ring
  buffer with a fixed capacity. Once it starts evicting, it counts every
  eviction (`dropped()`) right next to what it kept (`retained()`) — the tool
  practices issue #10's own lesson on itself, on its own bounded history, not
  just on the `StoryTrace` it's showing you.
- **Five categories, not all populated by every mode:** `RESOURCE`, `RENDER`,
  `OUTPUT`, `INPUT`, `SESSION`. A mode that observes nothing in a category
  renders `?` for it in the SUBSYSTEM STATUS panel — never a fabricated
  reading.
- **The epistemic rule, stated plainly because it is non-negotiable:**
  **an unobserved internal layer renders `?`. A later receipt existing is
  never treated as proof an earlier, unprobed layer fired.** This shows up
  concretely in Ghost-Key (the `APP` stage stays `?` for the crossterm gate,
  never inferred from `CHILD`/`Restored` receipts) and in Ownership-Duel (the
  transient `Restoring` lease state is real but shown as `? not caught by
  poller`, not synthesized because we know it must have happened).
- **Pure render.** `visual::frame(view, width, height, depth) -> Surface`
  reads a `View` and draws it; it never mutates the log or state it's handed
  and never advances a clock. A frozen `--dump` frame is a photograph, not a
  simulation step — same discipline `examples/event_pressure_lab` already
  established for its own oscilloscope.
- **The `--dump` path.** Every mode's `dump(...)` function builds a `View`
  once, calls `visual::frame` once, and prints the resulting `Surface`
  glyph-by-glyph to stdout. `--width`/`--height` (clamped 1..240 / 1..80) and
  `--color mono|ansi16|ansi256|truecolor` are shared across all three modes.

## Mode: Million-Tick

```
--mode million-tick --dump [--ticks N] [--retention all|bounded:CAP|disabled]
```

Drives a real, self-looping `StoryDirector` (`Story::new("loop").beat(Beat::new
("loop"))`, mirroring `examples/long_session_probe.rs`) for `N` ticks under a
chosen `TraceRetention` policy (`StoryDirector::set_trace_retention`), sampling
`trace().steps.len()`, `trace().dropped_steps()`, `trace().is_complete()`, and
`/proc/self/status` VmRSS/VmHWM at bounded intervals (~300 samples across the
run, itself capped into a 256-record `DiagLog`). The header hero state is the
whole point: switching `--retention` visibly changes the curve.

- `--retention all` → header `[TRACE GROWTH]`. Retained steps climb linearly
  with ticks, unbounded; `dropped_steps()` stays 0; `is_complete()` stays true.
- `--retention bounded:CAP` → header `[BOUNDED RETENTION (cap CAP)]`. Retained
  steps sawtooth inside `[cap, 2*cap]` forever (the documented slack-drain in
  `src/story.rs`: push to `2*cap`, evict the oldest half back down to `cap`),
  `dropped_steps()` climbs, `is_complete()` goes false — and VmRSS holds flat
  instead of climbing with the tick count. That flat-vs-climbing contrast,
  same two `--dump` commands, same `--ticks`, is the visual proof issue #10's
  fix actually bounds memory.

Money-shot commands (paste both, compare the sparklines and the header):

```sh
cargo run --release --example runtime_observatory -- \
  --mode million-tick --dump --retention all --ticks 100000 \
  --color mono --width 100 --height 32

cargo run --release --example runtime_observatory -- \
  --mode million-tick --dump --retention bounded:2000 --ticks 100000 \
  --color mono --width 100 --height 32
```

Only `RESOURCE` has real data in this mode; `RENDER`/`OUTPUT`/`INPUT`/`SESSION`
render `?` — this mode never paints a frame, drains output, takes input, or
touches a session lease, so there is genuinely nothing to report there.

## Mode: Ghost-Key

```
--mode ghost-key --dump [--gate crossterm|raw] [--freeze-at US]
```

Replays the REAL crossterm #1126 evidence captured for
[the Event Pressure Lab / PR #18](EVENT_PRESSURE_LAB.md) —
`docs/fixtures/event-pressure/crossterm-gate.tsv` and `raw-gate.tsv`, real
release-lab receipts from a controlled resize-then-key PTY collision, embedded
via `include_str!` — through an EVENT PIPELINE panel. This mode drives
nothing; there is no live process inside `--dump`, only a filter over
already-recorded records (`us <= freeze_at`) and a projection into the shared
`View`.

The bug: a resize (`SIGWINCH`) and a key byte share one mio readiness batch.
`epoll_wait` hands back both the SIGNAL and TTY tokens in one call; crossterm
returns `Event::Resize` on the SIGNAL token and — because the fds are
edge-triggered (`EPOLLET`) — never goes back for the TTY token in that same
wake. `FIONREAD` says `1` before and after; nobody reads it.

- `--gate crossterm` never reaches an `APP Key` receipt in this fixture, at
  any freeze point up to the recording's own end. Before the byte becomes
  readable: header `[AWAITING COLLISION]`. Once it's readable and the backend
  has already returned `Resize` without reading it: header `[INPUT STARVED]`,
  `APP` stays `? UNOBSERVED — never inferred from a later receipt`.
- `--gate raw` reaches a real `APP Key` receipt (`t=23171us 114`): header
  `[KEY DELIVERED]`. Same physical byte, same ordering, different backend.

The independent syscall witness (four named excerpts from
`docs/fixtures/event-pressure/readiness-witness.json`, verbatim, with the
file's own already-redacted PIDs/fds) is shown as a **static reference block**
for the crossterm gate only — it has no timestamp on the tsv's clock (the
JSON's provenance note says epoch timestamps were removed), so it is never
time-correlated with `--freeze-at`. There is no comparably-named witness
excerpt for the raw control in that JSON, so the raw gate's witness row
honestly reads `? no strace excerpt keyed to the raw control`.

Money-shot commands:

```sh
# Before the collision
cargo run --release --example runtime_observatory -- \
  --mode ghost-key --dump --gate crossterm --freeze-at 20000 \
  --color mono --width 100 --height 32

# The stranded moment (deadline reached, byte never delivered)
cargo run --release --example runtime_observatory -- \
  --mode ghost-key --dump --gate crossterm --freeze-at 724703 \
  --color mono --width 100 --height 32

# The raw control actually delivering the same byte
cargo run --release --example runtime_observatory -- \
  --mode ghost-key --dump --gate raw --freeze-at 23594 \
  --color mono --width 100 --height 32
```

`RESOURCE`/`RENDER`/`OUTPUT` render `?` in this mode; `INPUT` and `SESSION`
carry the real PTY/TTY/backend/APP and CHILD lifecycle receipts respectively.

## Mode: Ownership-Duel

```
--mode ownership-duel --dump [--freeze-at US]
```

Replays a real captured lease-transition trace,
`docs/fixtures/ownership/duel.tsv`, through a TERMINAL OWNERSHIP panel. The
fixture was produced by adding a `duel-trace` scenario to
`examples/terminal_ownership_probe.rs` and capturing it under a REAL PTY:

```sh
cargo build --release --example terminal_ownership_probe
script -qec "./target/release/examples/terminal_ownership_probe duel-trace" \
  /tmp/duel-capture.typescript
```

`script` allocates a real PTY for the child, so `stdout().is_terminal()` is
true and `TerminalSession::new()` genuinely takes the process-global CAS lease
(`src/session.rs`) — confirmed by the captured trace actually flipping between
`Owned`/`Available`, which a non-TTY run could never do. The raw capture was
then stripped of `script`'s own header/footer lines, `\r` (PTY line-ending
translation), and the raw ANSI reset bytes `TerminalSession::restore()`
legitimately writes to the same fd mid-stream — normalization of terminal
noise around the values, not a change to any value, the same kind of cleanup
the Event Pressure Lab fixtures document for themselves. Two contexts fight
over the one lease for real: ctx#1 acquires, ctx#2's `TerminalSession::new()`
is rejected with `AlreadyExists` while ctx#1 still holds it, ctx#1 calls
`restore()`, ctx#2 reacquires, ctx#2 tears down.

- At the rejection (`t=113us`, lease still `Owned` by ctx#1): header
  `[OWNERSHIP CONFLICT]`.
- Mid-hold after ctx#2 reacquires but before teardown: header
  `[LEASE HELD (ctx2)]`.
- After teardown (lease back to `Available`): header `[LEASE RELEASED]` — the
  header reflects the *current* lease state at the freeze point
  (`TerminalSession::lease_state()`'s last recorded value at or before
  `--freeze-at`), not which context most recently acquired; an earlier
  version of this mode latched on the last acquirer and kept reading
  `[LEASE HELD (ctx2)]` after teardown, which was wrong and has been fixed.

Money-shot commands:

```sh
# The rejection moment
cargo run --release --example runtime_observatory -- \
  --mode ownership-duel --dump --freeze-at 115 \
  --color mono --width 100 --height 32

# After teardown, lease back to Available
cargo run --release --example runtime_observatory -- \
  --mode ownership-duel --dump --freeze-at 192 \
  --color mono --width 100 --height 32
```

A background thread in the probe's `duel-trace` scenario busy-polls
`TerminalSession::lease_state()` for the whole run, trying to catch the
transient `Restoring` state between the `Owned` and `Available` stores inside
`restore()`. In the captured run it caught it zero times —
`restore()`'s real work (a few crossterm `execute!` calls, one write+flush,
`disable_raw_mode()`) apparently completes faster than the poller thread gets
scheduled even once. The panel says so honestly (`Restoring seen: ? not
caught by poller`) rather than synthesizing a state that was never actually
witnessed, even though we know structurally it must occur.

`RESOURCE`/`RENDER`/`OUTPUT`/`INPUT` render `?` in this mode — this replay
only carries `SESSION`-lane lease transitions, nothing else.

## Honest boundaries

- **Only `--dump` is wired today.** Every mode errors if you omit it. There is
  no live/interactive loop; the original vision's heavier modes — Slow-
  Terminal, Restore-Failure, Endurance — are **not built**. Nothing in this
  document should be read as a claim they exist.
- **VmRSS/VmHWM are Linux `/proc/self/status`-specific**, same convention as
  `examples/long_session_probe.rs`. If `/proc` is unavailable, those fields
  are never populated and render `?`, not a fake number; this path has not
  been exercised on a non-Linux host.
- **The transient `Restoring` lease state is real but currently
  unwitnessed** in the captured fixture — see Ownership-Duel above. This is a
  measured absence, not a missing feature.
- **The crossterm #1126 fix is analysis-only, not shipped.** See
  [`CROSSTERM_1126_FIX_ANALYSIS.md`](CROSSTERM_1126_FIX_ANALYSIS.md): the
  diagnosis is corroborated and a fix shape is proposed, but no patched
  crossterm ships in LibGibson and issue #15's collision-delivery acceptance
  remains red. Ghost-Key visualizes the *evidence* for that open defect, not a
  fix.
- **Issues #10, #11 and #15 are not closed.** #10 (unbounded `StoryTrace`) and
  #11 (terminal-ownership lease) both have merged runtime contracts that this
  instrument exercises and visualizes — that is not the same claim as the
  GitHub issues being closed, and this document does not assert that. #11's
  `restore()` best-effort error path (`src/session.rs`: attempts every
  cleanup op, reports the first error) is implemented but is **not covered by
  a test that forces a failing terminal write** — that path is exercised only
  by the happy-path restoration tests in `tests/terminal_ownership.rs`.
- **`docs/fixtures/ownership/duel.tsv` is a captured artifact**, produced by
  the exact `script`-over-PTY command documented above, then mechanically
  normalized (control-byte/line-ending stripping only, no value changes) —
  it is not hand-authored and was not regenerated by re-running until a
  more dramatic trace appeared.
- **This document does not update or supersede
  [`STATE_OF_LIBGIBSON.md`](STATE_OF_LIBGIBSON.md) or
  [`../ROADMAP.md`](../ROADMAP.md)** — the claim ledger for issues #10/#11/#15
  lives there, maintained separately.

## Source map

| Path | Role |
| --- | --- |
| `examples/runtime_observatory.rs` | CLI entry point, mode dispatch, `--dump`-only gate |
| `examples/runtime_observatory/diag.rs` | `Category`, `DiagRecord`, bounded `DiagLog` |
| `examples/runtime_observatory/visual.rs` | Shared scaffold: header/panels/events rail, pure `frame()` |
| `examples/runtime_observatory/million_tick.rs` | Million-Tick mode: drives a real `StoryDirector` |
| `examples/runtime_observatory/ghost_key.rs` | Ghost-Key mode: replays `docs/fixtures/event-pressure/*.tsv` + witness JSON |
| `examples/runtime_observatory/ownership_duel.rs` | Ownership-Duel mode: replays `docs/fixtures/ownership/duel.tsv` |
| `examples/terminal_ownership_probe.rs` | `duel-trace` scenario: the fixture's real capture source |
| `docs/fixtures/ownership/duel.tsv` | Captured lease-transition fixture (this round) |
| `docs/fixtures/event-pressure/*` | Captured #1126 evidence (from PR #18, reused here unchanged) |
