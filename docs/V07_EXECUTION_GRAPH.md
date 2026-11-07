# v0.7 execution graph — a playable terminal Doom through public interfaces

**Status: PLAN, in execution.** Nothing here is part of v0.6.0, and no v0.7 ABI is added by it. The
approved v0.6 integration merged as `f6cd8c7` (#253); v0.7 work branches from the **actual** `main`
that contains it — never from the integration branch itself.

*Update 2026-11-07:* v0.7 work under this plan has since added three additive ABI v1 symbols on
`main` (59 → 60: `gibson_push_event`, #273; → 62: `gibson_poll_event_kind` /
`gibson_push_event_kind`, #283). `GIBSON_ABI_VERSION` stays 1; v0.6.0 keeps 59.

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
  behind a coincident resize on stock crossterm 0.29.0) stays open upstream. *Update 2026-11-07:*
  on v0.7 `main`, #15 is fixed for LibGibson's own Unix terminal source only (#282, `04c0ad2`);
  crossterm#1126 / crossterm#1128 are still open. A kitty-protocol key release is delivered as a
  press (#285, open).

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

## Progress ledger (updated 2026-11-07)

States: **merged** (on `main`), **partial** (some of the node's oracle met), **not started**. Each
node closes only on its own oracle, not because this table looks complete.

**Renaissance XIX (2026-11-07): status reconciliation.** Checked against `main` `5790f1a` and
GitHub on 2026-11-07. Where RXVIII's wording below differs, this governs.

- **#269 fixed** by #282 (`04c0ad2`) for LibGibson's owned Unix input path (`src/tty_source.rs`):
  a poll on a hung-up terminal returns `BrokenPipe`, also with SIGHUP ignored or handled
  (`tests/pty_hangup_matrix.rs`).
- **#15 fixed on that owned path only** (#282; `context_coincident_lone_key_delivery_acceptance`
  enabled). crossterm's own reader still strands the key: crossterm#1126 / crossterm#1128 are
  open and the newest crossterm on crates.io is 0.29.0. Off Unix, or on Unix when no controlling
  terminal was latched (stdin not a terminal and no interactive session), `poll_event` still reads
  through crossterm.
- **CI hangup gate is single-attempt.** #282 removed the 3-attempt retry: the `ci.yml` step "PTY
  hangup tests (serial, single attempt)" and `scripts/release/preflight.sh` run `pty_hangup`,
  `pty_hangup_restore` and `pty_hangup_matrix` once, serially. CI at `5790f1a`: run 42337843945,
  8/8.
- **#264 fixed** by #281 (`1a9c0f5`).
- **#274 addressed, not fixed**, by #283 (`da2be76`): additive `gibson_poll_event_kind` /
  `gibson_push_event_kind` carry the key kind. Legacy `gibson_poll_event` still reports a typed
  U+1001–U+100D and the `GIBSON_KEY_*` of equal value identically, and `gibson_push_event` reads
  those codes as special keys: a documented compatibility property (`include/gibson.h`).
- **#275: measured compromise**, by #284 (`d526d7c`): default grace 25 tics, from xterm autorepeat
  timelines recorded under Xvfb/XTEST. Not key-release knowledge: a press-only terminal cannot
  tell a tap from the start of a hold, so a tap moves for 25 tics. Other emulators, Wayland and a
  physical keyboard are unmeasured.
- **#285 fixed on Unix** by #289 (`f365239`; exact-main CI 42421266844, 8/8): a kitty-protocol
  key release is not delivered (press and repeat still are), and a dropped event no longer ends a
  poll before its timeout. Evidence is PTY injection; no emulator here implements the protocol.
  Windows key-kind decoding is unchanged: crossterm 0.29 reports every Windows key-up as a release,
  which LibGibson still delivers as a second press (#292, open; no Windows runner here).
- **C ABI:** 62 symbols on `main` (59 → 60 by #273 `67c43bd`, → 62 by #283); 59 at `9a899e1` and
  on the v0.6.0 candidate `572e551`. `GIBSON_ABI_VERSION` stays 1.
- **Node H:** its earlier statement that a handled or ignored SIGHUP can spin in #269 is
  superseded by #282; the row now separates what is tested from what is not.
- **v0.6.0: not tagged, not released.** HOLD on owner decision #287. The candidate is now
  `release/v0.6.0-rc` @ `572e551` = `1b90b48` + the #259 backport (#288, `e6b9c42`) + the #285
  backport (#294, `572e551`), requalified at that exact head: CI 42421627573 (8/8), Release
  Preflight 42421629285 (PASS), `check-maintenance-candidate.sh` PASS (59 symbols, built library
  included), 140/140 counted stress runs; record on #287. The release-line policy is proposed in
  draft #291 (Tier C, owner only). Latest published release: v0.5.0.
- **v0.8 first parity slice merged** (#295, `ccbac09`): one script, five installed-interface
  consumers; C++, Python and Go transcripts byte-identical to C; Rust differs on 28 documented
  records. Found #293 (Rust `poll_from` does not resize a headless context; the C ABI does).
- **Node J:** a clean-room software-framebuffer adapter over the installed SDK, with seven
  properties and 19 negative controls, is in review (#299). A GPL engine port exists only as a
  local, unpublished prototype; no external repository has been created. Found #297 (headless
  ANSI capture grows without bound in C) and #298 (no C terminal-size query, no C focus events).

**Renaissance XVIII (2026-11-06).** *Superseded in part 2026-11-07 (see RXIX): #15 is fixed on
the owned Unix path only; #274 is addressed, not fixed; #275 is a measured compromise; #285 is
fixed on Unix (#289; Windows #292).*

- **#269 is fixed** by #282 (`04c0ad2`). The intermittent hangup-gate failures were a
  check-then-act race: crossterm's read loop spun forever on a terminal that hung up mid-call.
  LibGibson now reads a Unix controlling terminal through its own source, using crossterm
  0.29.0's ported decoder, where the hangup is a read result. That source also fixes **#15**
  (the delivery acceptance is enabled). Stress, old vs new: 12/260 vs 0/380+ spins.
- The CI hangup step is single-attempt, with no retry.
- **#264 is fixed** by #281 (`1a9c0f5`): an idle `run_once` sleeps instead of spinning.
- **#274** is fixed by #283 (`da2be76`): an additive kind-aware event pair, ABI 60 → 62.
- **#275** is settled by #284 (`d526d7c`): measured key timelines, grace 25 tics.
- **#285** is filed: a kitty key release is read as a press.

| Node | State | Where | Evidence and limits |
|---|---|---|---|
| A. Baseline | **merged** | #262 (`fb9b6f5`) | [`V07_BASELINE.md`](V07_BASELINE.md): forcing consumer identical at `f6cd8c7` and `7f57047`; ABI 59 = 59 |
| B. Raster ingest | **measured, merged**; quantizer fix **merged** | #262 · #263 (`7888d4a`, Tier B) | Bench + 8-test oracle (all 21 configurations parsed back). Found #260: 256-colour render 4.7–7.2× truecolor; #263 makes it O(1), proven equal over all 2²⁴ inputs, 17–22× faster (8217→372, 14706→847, 35097→1962 µs p50), byte-identical. No fit/ingest API: the consumer-side fit is exact and cheap, and node I shows a C consumer can lower its own raster through the existing ABI. |
| C. Event injection | **merged** (Rust and C ABI), key kinds added | #265 (`a172723`, Tier B) · #273 (`67c43bd`, Tier B) · #283 (`da2be76`, Tier B: `gibson_poll_event_kind` / `gibson_push_event_kind`, #274) | `InjectedQueue` (bounded FIFO, refusal not drop, receipts); identical headless digests across runs, batching and the `ScriptedSource` route. Found and fixed **#259**: a headless `Context` read the real terminal. C ABI: `gibson_push_event` (59 → 60 symbols, additive) with `gibson_event_t`'s layout frozen by test, `GIBSON_ERR_QUEUE_FULL`, and the installed-SDK forcing consumer driven only by pushed-and-polled input (frames and pinned checksum unchanged). Found **#274**: v1 key codes collide with U+1001–U+100D. #283 adds `gibson_poll_event_kind` / `gibson_push_event_kind` (60 → 62, additive), which carry the key kind; legacy `gibson_poll_event` keeps the collision as a documented compatibility property (#274 addressed, not fixed). |
| D. Held keys | **merged** (incl. focus-loss release; grace measured) | #265 (`a172723`, Tier B) · #276 (`72845b3`) · #284 (`d526d7c`, #275) | `HoldTracker`: explicit grace/repeat model on simulation tics, never a synthetic release; matches an independent per-tic simulator; cadence-invariant; a distinct-model review found an end-of-time invariant breach, fixed red-first. Focus loss on a real PTY: DEEP SIGNAL enables focus reporting through `ctx.session` (restore-tracked; #267 was filed in error and closed), and a recorded focus loss changes the replayed trajectory. Observed desktop repeat *settings* (X11/MATE, `xset`/`gsettings`: delay 500 ms, 30 Hz), not terminal-delivered timing: by the hold rule, the 114 ms repeat timeout covers a 30 Hz rate, and DEEP SIGNAL's old 257 ms grace did not bridge it. **Observed through a real terminal** since (#275): xterm under X/XTEST with the game recording, the first autorepeat arrives 18 tics after the press at 500 ms / 30 Hz and 23 tics at X's default 660 ms / 25 Hz, so the old grace stalled a hold for 9 and 14 tics. The default grace is now 25 tics (714 ms), which holds both measured timelines without interruption (`tests/deep_signal_hold_timeline.rs`). The stated cost is that a tap now moves for 25 tics; `--hold-grace-ms` chooses otherwise. Real release events (the kitty keyboard protocol) would remove the trade, but no available emulator here offers them. The 25 tics are a compromise measured on xterm under Xvfb/XTEST only, not key-release knowledge. A kitty-protocol release that reaches the decoder was delivered as a press; on Unix it is no longer delivered (#285, fixed by #289; Windows #292). |
| E. Synthetic forcing example | **merged** | #266 (`af1498c`) | DEEP SIGNAL: original raycaster on the public API; headless replay equality at 4 cadences + 40×15 ASCII; every level solvable; real-PTY play at 80×24 colour and 40×15 ASCII. |
| F. Pacing & backpressure | **merged** (game-level) | #266 (`af1498c`) | Simulation keeps wall-clock pace, presentation drops frames: slow PTY reader → 172–175 tics (≥ 94% of those owed) with ~15 frames presented; red against a 4-tic catch-up cap (45 of 178 and 49 of 185 owed, in two runs). A terminal that stops reading stalls the synchronous loop. |
| G. Resize | **merged** (game level and library level) | #266 (`af1498c`) · #279 (`47a216a`) | Real-PTY resize sweep mid-play (80×24 → 120×40 → 40×15 → 100×30 → 80×24, terminal model resized at the exact byte offset): every view row repainted, HUD on the last rows in the right form. Found and fixed a HUD that lagged a late `Resize` event. Library level (#279): a full-size `Node::canvas` on a headless `Context`, resized through an injected `Resize` event (grow, shrink, down to 1×1), is handed exactly the new rect (origin included) on the next due render, and the frame takes exactly the new geometry, every cell's glyph read back from the current paint (a full-size canvas: this checks the readback of a fully repainted frame; physical clearing of uncovered areas is in `whole_renderer_vt100`). A queued but undelivered resize neither renders nor calls the canvas, checked once the frame budget allows a frame. Mutants (resize not marking the scheduler dirty; renderer keeping the old size) fail. |
| H. Lifecycle | **merged** (game level) | #266 (`af1498c`) · #271 (`82aaae7`) | PTY-tested, with raw mode / alternate screen / hidden cursor checked first: quitting with `q`, Esc or Ctrl-C (a key in raw mode); delivered SIGTERM/SIGINT (restore, trace written, signal re-raised); a delivered signal whose orderly stop cannot finish within a 3 s grace (a terminal that stopped reading blocks every write) still ends the process by that signal, after a watchdog restores cooked mode from the saved termios (`tcsetattr`; tested with stdin the terminal and with stdin redirected, the terminal then reached through `/dev/tty`) — the alternate screen, hidden cursor and focus reporting (`?1004`) need writes and stay, and the trace is not written; a stop signal pending through a failed startup still ends the process by that signal; if the watchdog cannot be started, the signals are not caught at all; a panic in the loop (library hook); unwritable diagnostics, including argument errors (no panic). SIGHUP is not caught (handlers are installed for SIGTERM/SIGINT only; checked from `/proc/<pid>/status` `SigCgt`): a hangup ends the game by SIGHUP within the test's 15 s bound and the `--record` trace is lost. *Updated 2026-11-07:* the stated reason, that a handled or ignored hangup could make `poll_event` busy-spin (**#269**), is superseded: since #282 (`04c0ad2`) a poll on a hung-up terminal returns `BrokenPipe`, also with SIGHUP ignored or handled (`pty_hangup_matrix`, library level), and the game loop ends on a poll error. Not tested at game level: a run with SIGHUP inherited as ignored (e.g. `nohup`); catching SIGHUP to restore and write the trace (not implemented). The rationale comments in `examples/deep_signal.rs` and `tests/deep_signal_lifecycle_pty.rs` predate #282. Not covered: job-control stop (`kill -TSTP`), SIGQUIT. |
| I. Cross-language leg | **merged** | #278 (`667cef5`) | The installed-SDK forcing consumer (staged prefix, pkg-config only; CI Bindings job and the release cleanroom) drives C (input only through `gibson_push_event` / `gibson_poll_event`) and B: a C-side 48×16-pixel RGB raster, moved only by injected keys, lowered **by the consumer** to half-block cells through the existing styled-span ABI (`gibson_line_add_span` with fg = top pixel, bg = bottom pixel) and read back exactly — every cell of 60 frames (23,040 cells) at TRUECOLOR, with each frame's single injected key checked as delivered. This verifies the cell readback, not the emitted ANSI. 223–336 µs per frame across three runs for the whole loop (input, painting, span construction, render, readback, comparison), release library, development box; load-dependent, not a benchmark. This consumer needs no raster entry point; whether a dedicated one pays for larger rasters is unmeasured. |
| J. GPL adapter | **partial**: in-repo clean-room adapter in review | #299 (in review) | The GPL engine port itself is never in this repository. In-repo, MIT OR Apache-2.0: `bindings/framebuffer_adapter/`, a generic framebuffer adapter over the installed SDK, with seven properties and 19 negative controls (#299). A doomgeneric port on top of it runs as a **local, unpublished** prototype; creating its repository needs the owner. Found #297, #298. |

## Not in v0.7

Pixel-accurate rendering (the cell grid stays the finest unit), audio, networked play, non-Linux
qualification (v0.9), and any promotion of `gibson::media` without a named consumer.
