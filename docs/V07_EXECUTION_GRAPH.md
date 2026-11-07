# v0.7 execution graph — a playable terminal Doom through public interfaces

**Status: PLAN, in execution.** Nothing here is part of v0.6.0, and no v0.7 ABI is added by it. The
approved v0.6 integration merged as `f6cd8c7` (#253); v0.7 work branches from the **actual** `main`
that contains it — never from the integration branch itself.

## What v0.6 already proves (preserved evidence, not re-litigated)

- An installed-SDK forcing consumer (`bindings/sdk_forcing/doom_forcing_consumer.c`, staged prefix,
  `pkg-config` only) drives 240 headless frames at 64×8 and reads back 122,880 cells through the
  59-symbol ABI v1, in CI and in the release cleanroom ([`SDK_DOOM_HORIZON.md`](SDK_DOOM_HORIZON.md)).
- A real-TTY input → state → render loop runs entirely through the ABI
  (`ffi_realtty_probe abi-loop`, `tests/ffi_realtty_pty.rs`).
- The two named boundaries: **a headless context takes no input**, and **the cell grid is the finest
  readback unit** (no pixel raster).
- In Rust, a raster already reaches the one renderer: `RgbRaster` (`src/frame.rs`), sub-cell glyph
  modes (`SubcellGlyphMode`, `src/glyph.rs`) and `Node::raster` (`src/node.rs`). The C ABI has no
  raster node and no mouse/focus events.
- Terminal key input is **press-only** (no key-release events are decoded), and #15 (a key withheld
  behind a coincident resize on stock crossterm 0.29.0) stays open upstream.

## Principles

1. **Domain-neutral library.** No public symbol names a game. Capabilities are named for what they
   are — raster ingest, event injection, frame clock — and each is gated on the forcing consumer
   actually needing it.
2. **One renderer.** A raster frame is lowered to cells and diffed like any other node; no second
   output path.
3. **GPL separation.** LibGibson stays `MIT OR Apache-2.0` and never links GPL code. The playable
   adapter that embeds a GPL engine lives in a **separate repository** and consumes only the
   installed SDK; LibGibson's CI runs only the in-tree, non-GPL synthetic forcing example (node E).
4. **Additive ABI only, reviewed before it lands.** ABI v1 grows by additive symbols with
   `abi/gibson-abi-v1.symbols`, a layout-freeze test and the installed-SDK consumer updated in the
   same change; `GIBSON_ABI_VERSION` stays 1 unless an existing contract must break.

## The graph

Each node lists its deliverable, what it depends on, the oracle that closes it, and the result that
would kill or reshape it.

| Node | Deliverable | Needs | Closing oracle | Kill / reshape criterion |
|---|---|---|---|---|
| **A. Baseline** | Successor branch from the merged `main`; forcing consumer + envelope re-run at that SHA | merge | Same consumer output; envelope deltas recorded | — |
| **B. Raster ingest** | Measure `RgbRaster` → sub-cell lowering for a moving 320×200 source at representative grids (80×25 … 200×60); then, only if the C consumer needs it, one additive `gibson_node_raster`-class entry point | A | Per-frame lowering cost and bytes/frame at an exact SHA; cell readback of a known test pattern is exact | Lowering cost leaves no budget at 35 Hz on the reference host → change the lowering, not the ABI |
| **C. Event injection** | Headless event injection (`Context` in Rust; one additive C symbol, e.g. `gibson_push_event`) so a game loop is deterministic and testable without a TTY | A | Injected key sequence → identical frame digests across runs; a headless `poll_event` returns injected events in order | — |
| **D. Held-key model** | Normalized keyboard for continuous movement on press-only terminals: repeat-driven hold with an explicit release timeout; opt-in key-release reporting where the terminal supports it | C | Scripted press/repeat/stop traces produce the declared hold intervals; real-PTY check | Release timeout cannot be tuned below perceptible lag → document as a terminal limit |
| **E. Synthetic forcing example** | In-repo, non-GPL: a procedural raycast maze framebuffer + instrument/status overlay + real keyboard + animated scene transitions + capability-aware monochrome/ASCII fallback + built-in performance and replay receipts | B, C, D | Headless replay equality; real-PTY play session at 80×24 and 40×15 mono; envelope published | — |
| **F. Pacing & backpressure** | Fixed simulation tic decoupled from presentation; bounded output (skip presentation, never simulation, when the terminal lags); measured write time per frame | B | Under an artificially slow PTY reader the simulation tic count stays exact and presented frames drop measurably | — |
| **G. Resize** | Raster scale and HUD re-layout on resize, including a resize mid-frame | B, F | Real-PTY resize sweep; no torn/stale rows (vt100); #15 behaviour recorded, not hidden | — |
| **H. Lifecycle** | Terminal restoration on quit, panic and signal for the game loop; bounded shutdown | E | Existing hangup/restore PTY harnesses extended to the example | — |
| **I. Cross-language leg** | The C forcing consumer extended to drive B + C from an installed prefix | B, C | Bindings-job run of the installed-SDK consumer | — |
| **J. GPL adapter (separate repo)** | A playable adapter embedding an existing GPL engine, built only against the installed SDK; game data files are user-supplied, never vendored | E, I | Its own CI; LibGibson CI unaffected | Any need for a private or game-specific LibGibson API → stop and file it as a domain-neutral capability first |

Order: **A → {B, C} → D; B → F → G; {B, C, D} → E → H; {B, C} → I → J.**

## Progress ledger (updated 2026-11-06)

States: **merged** (on `main`), **partial** (some of the node's oracle met), **not started**. Each
node closes only on its own oracle, not because this table looks complete.

The intermittent hangup-gate failures (#269) are root-caused, not fixed: a check-then-act race in
the library's hangup-aware poll lets crossterm's read loop spin forever on a terminal that hangs up
mid-call. It affects applications that ignore or handle SIGHUP, and the CI PTY job's bounded retry
can turn it green. The candidate fixes are owner decisions (dependency-level).

| Node | State | Where | Evidence and limits |
|---|---|---|---|
| A. Baseline | **merged** | #262 (`fb9b6f5`) | [`V07_BASELINE.md`](V07_BASELINE.md): forcing consumer identical at `f6cd8c7` and `7f57047`; ABI 59 = 59 |
| B. Raster ingest | **measured, merged**; quantizer fix **merged** | #262 · #263 (`7888d4a`, Tier B) | Bench + 8-test oracle (all 21 configurations parsed back). Found #260: 256-colour render 4.7–7.2× truecolor; #263 makes it O(1), proven equal over all 2²⁴ inputs, 17–22× faster (8217→372, 14706→847, 35097→1962 µs p50), byte-identical. No fit/ingest API: the consumer-side fit is exact and cheap, and node I shows a C consumer can lower its own raster through the existing ABI. |
| C. Event injection | **merged** (Rust and C ABI) | #265 (`a172723`, Tier B) · #273 (`67c43bd`, Tier B) | `InjectedQueue` (bounded FIFO, refusal not drop, receipts); identical headless digests across runs, batching and the `ScriptedSource` route. Found and fixed **#259**: a headless `Context` read the real terminal. C ABI: `gibson_push_event` (59 → 60 symbols, additive) with `gibson_event_t`'s layout frozen by test, `GIBSON_ERR_QUEUE_FULL`, and the installed-SDK forcing consumer driven only by pushed-and-polled input (frames and pinned checksum unchanged). Found **#274**: v1 key codes collide with U+1001–U+100D. |
| D. Held keys | **merged** (incl. focus-loss release) | #265 (`a172723`, Tier B) · #276 (`72845b3`) | `HoldTracker`: explicit grace/repeat model on simulation tics, never a synthetic release; matches an independent per-tic simulator; cadence-invariant; a distinct-model review found an end-of-time invariant breach, fixed red-first. Focus loss on a real PTY: DEEP SIGNAL enables focus reporting through `ctx.session` (restore-tracked; #267 was filed in error and closed), and a recorded focus loss changes the replayed trajectory. Observed desktop repeat *settings* (X11/MATE, `xset`/`gsettings`: delay 500 ms, 30 Hz), not terminal-delivered timing: by the hold rule, the 114 ms repeat timeout covers a 30 Hz rate, and DEEP SIGNAL's old 257 ms grace did not bridge it. **Observed through a real terminal** since (#275): xterm under X/XTEST with the game recording, the first autorepeat arrives 18 tics after the press at 500 ms / 30 Hz and 23 tics at X's default 660 ms / 25 Hz, so the old grace stalled a hold for 9 and 14 tics. The default grace is now 25 tics (714 ms), which holds both measured timelines without interruption (`tests/deep_signal_hold_timeline.rs`). The stated cost is that a tap now moves for 25 tics; `--hold-grace-ms` chooses otherwise. Real release events (the kitty keyboard protocol) would remove the trade, but no available emulator here offers them. |
| E. Synthetic forcing example | **merged** | #266 (`af1498c`) | DEEP SIGNAL: original raycaster on the public API; headless replay equality at 4 cadences + 40×15 ASCII; every level solvable; real-PTY play at 80×24 colour and 40×15 ASCII. |
| F. Pacing & backpressure | **merged** (game-level) | #266 (`af1498c`) | Simulation keeps wall-clock pace, presentation drops frames: slow PTY reader → 172–175 tics (≥ 94% of those owed) with ~15 frames presented; red against a 4-tic catch-up cap (45 of 178 and 49 of 185 owed, in two runs). A terminal that stops reading stalls the synchronous loop. |
| G. Resize | **merged** (game level and library level) | #266 (`af1498c`) · #279 (`47a216a`) | Real-PTY resize sweep mid-play (80×24 → 120×40 → 40×15 → 100×30 → 80×24, terminal model resized at the exact byte offset): every view row repainted, HUD on the last rows in the right form. Found and fixed a HUD that lagged a late `Resize` event. Library level (#279): a full-size `Node::canvas` on a headless `Context`, resized through an injected `Resize` event (grow, shrink, down to 1×1), is handed exactly the new rect (origin included) on the next due render, and the frame takes exactly the new geometry, every cell's glyph read back from the current paint (a full-size canvas: this checks the readback of a fully repainted frame; physical clearing of uncovered areas is in `whole_renderer_vt100`). A queued but undelivered resize neither renders nor calls the canvas, checked once the frame budget allows a frame. Mutants (resize not marking the scheduler dirty; renderer keeping the old size) fail. |
| H. Lifecycle | **merged** (game level) | #266 (`af1498c`) · #271 (`82aaae7`) | PTY-tested, with raw mode / alternate screen / hidden cursor checked first: quitting with `q`, Esc or Ctrl-C (a key in raw mode); delivered SIGTERM/SIGINT (restore, trace written, signal re-raised); a delivered signal whose orderly stop cannot finish within a 3 s grace (a terminal that stopped reading blocks every write) still ends the process by that signal, after a watchdog restores cooked mode — the alternate screen, hidden cursor and focus reporting (`?1004`) need writes and stay; if the watchdog cannot be started, the signals are not caught at all; a panic in the loop (library hook); unwritable diagnostics, including argument errors (no panic). SIGHUP keeps its default action — the kernel ends the game within a bound and the trace is lost — because a handled or ignored hangup can make `poll_event` busy-spin forever (**#269**, root-caused, library-level, not fixed). Not covered: job-control stop (`kill -TSTP`), SIGQUIT. |
| I. Cross-language leg | **merged** | #278 (`667cef5`) | The installed-SDK forcing consumer (staged prefix, pkg-config only; CI Bindings job and the release cleanroom) drives C (input only through `gibson_push_event` / `gibson_poll_event`) and B: a C-side 48×16-pixel RGB raster, moved only by injected keys, lowered **by the consumer** to half-block cells through the existing styled-span ABI (`gibson_line_add_span` with fg = top pixel, bg = bottom pixel) and read back exactly — every cell of 60 frames (23,040 cells) at TRUECOLOR, with each frame's single injected key checked as delivered. This verifies the cell readback, not the emitted ANSI. 223–336 µs per frame across three runs for the whole loop (input, painting, span construction, render, readback, comparison), release library, development box; load-dependent, not a benchmark. This consumer needs no raster entry point; whether a dedicated one pays for larger rasters is unmeasured. |
| J. GPL adapter | not started | separate repository | Never in this repository. |

## Not in v0.7

Pixel-accurate rendering (the cell grid stays the finest unit), audio, networked play, non-Linux
qualification (v0.9), and any promotion of `gibson::media` without a named consumer.
