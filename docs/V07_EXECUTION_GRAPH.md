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

## Not in v0.7

Pixel-accurate rendering (the cell grid stays the finest unit), audio, networked play, non-Linux
qualification (v0.9), and any promotion of `gibson::media` without a named consumer.
