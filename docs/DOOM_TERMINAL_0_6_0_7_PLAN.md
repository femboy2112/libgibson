# Project DOOM — Terminal Game Platform (v0.6–v0.7)

> **Status: FUTURE RESEARCH / DESIGN SKELETON — NOT IMPLEMENTED, NOT MERGED, NOT A RELEASE PROMISE.**
> Owner direction (2026-10-09): a real, playable open Doom-compatible engine rendered *through LibGibson*, with a subsequent OPL/Sound Blaster FM path, is an intended **0.6–0.7-class feature**. This is a **living plan**: features, version placement and acceptance criteria may be amended as the project evolves.
>
> Design provenance: \`research/doom-terminal-abi-parity-2026-10-09\` forked from \`main@80706c91b88e59495b131bfd8ab23aa3ed013414\` (released v0.4.0 baseline). At the research snapshot active HumanMusic C137 work is in draft PR #87; GSPU-FM design is in draft PR #90; the MeatsackProjection research branch is separate. **Refresh all heads before building.**
>
> See companion plans: [language ABI parity to v1.0](LANGUAGE_ABI_PARITY_1_0.md) and [deferred implementation + hostile validation](DOOM_TERMINAL_PLATFORM_HANDOFF.md). This research branch makes NO source, ABI, binding or runtime modifications.

## 0. Product outcome and boundaries

The target is **actual interactive Doom-compatible gameplay** inside a terminal, not a pre-rendered video, recreated scene, fake demo, or LibGibson-authored 3D raycaster. A mature upstream C game engine does its gameplay, world rasterization, tick schedule and WAD logic; LibGibson is its **display, input and host-graphics platform**.

**First candidate:** [ozkl/doomgeneric](https://github.com/ozkl/doomgeneric) (GPL-2.0-based Doom C port) has a narrow set of platform callbacks:

- \`doomgeneric_Create(argc, argv)\`, \`doomgeneric_Tick()\`.
- \`DG_Init\`, \`DG_DrawFrame\`, \`DG_SleepMs\`, \`DG_GetTicksMs\`, \`DG_GetKey\`, \`DG_SetWindowTitle\`.
- \`DG_ScreenBuffer\` is game-owned pixel storage. \`pixel_t\` is 32-bit by default or 8-bit with \`CMAP256\`; the provided public header defaults to \`DOOMGENERIC_RESX=640\`, \`RESY=400\` at this research snapshot. **Never assume actual build size, byte order, channel packing, palette, or buffer stride** without checking the selected build and a pixel-format calibration.
- Game content comes from a WAD. Use owner-supplied legally obtained WADs, or the independently licensed [Freedoom](https://github.com/freedoom/freedoom) content. The renderer/ABI should not bundle id Software proprietary game assets.

Doomgeneric is a **candidate, not an irrevocable dependency**. Test its build/ownership/lifecycle and compare alternatives if it proves unsuitable. Keep the adapter separate from the LibGibson core: GPL engine code and the MIT/Apache-2.0 library must not silently share incompatible distribution assumptions. A standalone external application using LibGibson's public C SDK is the preferred first integration artifact. Distributing that application may require GPL obligations even while LibGibson itself remains permissively licensed; verify packaging and notices before publishing.

### The authority split

~~~text
            DOOM ENGINE (C, upstream-owned)
 game tick / 2.5D world render / map / WAD / gameplay
                         |
            DG_ScreenBuffer (pixels)
                         |
            C ABI: validated bulk frame
                         v
            LibGibson RGB frame importer
            / resampler / color conversion
                         |
            Raster -> Surface -> Node::raster
            layout/overlays -> diff -> ANSI terminal
                         |
                REAL terminal output

  Keyboard: terminal input -> LibGibson event2
            -> adapter mapping -> DG_GetKey

  Audio (later): Doom MUS/GENMIDI/PCM intent
            -> separate compliant sound backend
            -> GSPU-OPL if/when earned
~~~

LibGibson renders the **terminal representation** of Doom's framebuffer; Doom retains its own world rasterizer. \`src/raster3d.rs\` need not recreate Doom walls/sprites. The renderer's scene/UI compositing remains reusable for other host applications.

## 1. Existing substrate: use it, do not fork it

Verified at the v0.4.0 main source snapshot:

- Rust \`RgbRaster\` (\`src/raster.rs\`) stores opaque RGB, produces half-block color \`Surface\` via \`to_surface\`, and mono fallback. Existing \`HalfBlockCanvas\` is also present.
- \`Node::raster\` / \`Node::surface\` in \`src/node.rs\`, transparent compositing and \`Surface\` layers in \`src/surface.rs\`, and core terminal diff/transaction/alternate-screen/lifecycle exist.
- \`src/temporal.rs\` offers image-to-Braille subcell mapping and controlled temporal experiments. Its static path is useful as a diagnostic/optional visual mode; **temporal modulation at Doom-like frame rates must not be assumed safe or superior**, since it has explicit presentation safety gates.
- \`src/input.rs\` and \`include/gibson.h\` expose basic keys, resize and paste. The existing Rust crossterm mapping constructs \`KeyEvent {code, modifiers}\` **without retaining press/repeat/release kind**. Doomgeneric explicitly wants both pressed and released states; this is a load-bearing missing capability.
- \`include/gibson.h\` / \`include/gibson.hpp\` and \`src/ffi.rs\` provide a C ABI v1 and an RAII C++ wrapper. **There is no direct bulk RGB-frame upload and raster-node interface in the current C API**; the C API is predominantly structured text/UI nodes, rendering, history and input.
- \`docs/RELEASE_CONTRACT.md\` explicitly allows **additive C ABI functions with ABI version 1** when existing exports/layout/semantics remain intact. Do not bump \`GIBSON_ABI_VERSION\` just for a new function. Existing \`abi/gibson-abi-v1.symbols\` and \`scripts/release/check-abi.sh\` are guards.

The first actual task is not "port Doom" but **make a game-friendly binary framebuffer and event interface available to all foreign-language consumers without breaking the old ones**. Doom is an adversarial external consumer that verifies the interface is not a cosmetic wrapper.

## 2. Proposed v0.6 milestone — graphics + structured input ABI

**Goal:** an external C program can present sustained RGB frames through LibGibson with normal layout/terminal lifecycle, inspect what was actually emitted, and receive honest actionable input.

### (A) Validated bulk raster ABI

Create a minimal, stable-intent, additive family of functions **after testing the necessary ownership pattern**, not a speculative clone of every Rust drawing API. Conceptual shape (names illustrative only):

~~~c
// DESIGN SKETCH ONLY — not a current export.
typedef struct gibson_raster gibson_raster_t;
typedef enum { GIBSON_PIX_RGB24, GIBSON_PIX_RGBA32, GIBSON_PIX_BGRA32 } gibson_pixel_format_t;

gibson_status_t gibson_raster_new(uint32_t width, uint32_t height, gibson_raster_t **out);
gibson_status_t gibson_raster_upload(
    gibson_raster_t *dst, const uint8_t *bytes, size_t byte_len,
    uint32_t width, uint32_t height, uint32_t stride_bytes,
    int32_t pixel_format);
gibson_status_t gibson_node_raster(const gibson_raster_t *src, gibson_node_t **out);
void gibson_raster_free(gibson_raster_t *dst);
~~~

Requirements to earn concrete names:

- **One bulk call per source frame**, never one FFI invocation per pixel/cell. Existing Rust \`Node::raster\` and renderer compose the imported frame as an ordinary \`Surface\`, not raw private ANSI.
- Explicit width/height, byte length, **row stride**, pixel channel order, alpha semantics, resize/crop policy and hard memory/dimension budget; checked multiplication and pointer validity. Validate Doom's actual 32-bit packing rather than guessing host endianness. Test padded stride and underlength buffers.
- Ownership/lifetime and frame visibility are unambiguous: caller may reuse/free source bytes after synchronous copy, or an explicit constrained borrowed-frame lifetime if a zero-copy variant is *actually* earned. A live node must not dereference freed C memory. Define cross-thread/callback limitations.
- Separate **frame input format**, **color depth capability**, **subcell output strategy**, and **terminal dimensions**. Expose an explicit resize/present request rather than stealing the terminal session from a hidden adapter.
- Avoid permanently allocating full original-size RGB and \`Surface\` copies per stage if a bounded reuse strategy can preserve semantics. But don't claim zero-copy without measuring the full chain.
- A C++ RAII wrapper around the same handles/calls should follow in v0.6. It must not become a second implementation. Python/Go parity work can build on the exact C core.

### (B) Structured key-state ABI without rewriting v1

Do **not** change the existing \`gibson_event_t\` layout in place. Design \`gibson_event_v2_t\` (or a new opaque event API), carrying:

- key code (with exact physical/logical semantics documented), modifiers, **Pressed/Repeated/Released/Unknown** phase where actually observable, capability/origin, sequence/time data if earned;
- resize, focus, quit/interrupt handling, optional mouse input later;
- explicit terminal keyboard-protocol detection/negotiation where available (e.g. modern CSI-u/Kitty enhancements) and a compatibility fallback.

**Terminal reality:** many common terminal keyboard modes do not transmit genuine key releases. An adapter may synthesize a bounded release timeout from repeats, or offer tap/toggle/continuous-turn control modes, but must label the events \`Synthesized\` / \`Unknown\`, not pretend the terminal sent a release. Measure held-forward, held-turn, simultaneous controls, menu navigation and stuck-key recovery. Plain terminals still get an honest degraded mode; an enhanced terminal may offer better semantics.

Restore terminal raw mode, cursor, alternate screen and shell state on normal exit, game exit, resize, signal and best-effort panic paths. LibGibson owns terminal I/O; Doomgeneric must not independently initialize an SDL/ANSI terminal alongside it.

### (C) Present path and observability

Provide full-frame and dirty-region semantics only when earned; pipeline:

\`Doom frame -> verified RGB/ARGB conversion -> scale/letterbox -> half-block (or supported static alternative) -> Surface -> Node::raster -> renderer diff -> terminal\`.

- Default **static 1×2 half-block color** for truecolor terminals. A \`C×R\` terminal can address roughly \`C×2R\` half-block pixels, minus UI. At \`160×40\` cells that's \`160×80\` image samples — Doom's source image must be downsampled. An \`80×24\` terminal gives only \`80×48\` samples before UI; it will be very low-detail. **Do not claim pixel-perfect native Doom resolution in ordinary terminals.**
- Aspect correction and crop/letterbox logic must account for **terminal cell physical aspect**, which varies; provide a measured/configured ratio, no universal guess.
- Provide monochrome and ANSI256 fallback, but keep the actual image-vs-cells semantics explicit. Braille's 2×4 geometry is not eight independently RGB-colored dots.
- Report source dimensions, output sampling dimensions, terminal dimensions/capabilities, changed cells, emitted bytes, generation vs write time, full repaint reason, dropped presentation frames and sim-tick count. Distinguish terminal write success from *actual terminal display completion*.
- Optional future Kitty graphics/Sixel support is a separately gated capability, **not required** for v0.6/0.7. A fast terminal bitmap protocol may ultimately outperform half-block ANSI, but don't build it just to make a benchmark look better.

### v0.6 graduation

An **external clean-room C consumer**, compiled and linked solely against the published SDK, continuously submits synthetic moving RGB frames + transforms/resizes without memory errors, preserves shell ownership and reports performance honestly. A standalone C++ consumer uses the identical C core with RAII safety. Reject red controls (wrong stride/format, buffer underflow, null, overflow, resize, unsupported keyboard phases).

## 3. Proposed v0.7 milestone — real Doom via LibGibson

**Adapter should stay outside the permissively licensed core**, ideally an independently packaged \`doomgeneric-gibson\` C application/repo or separately licensed integration artifact. No LibGibson code should import GPL Doom engine implementation to obtain a renderer. The adapter uses **only the public C SDK** so it stress-tests actual foreign consumers.

Minimal implementation:

1. \`DG_Init\`: create/own a fullscreen LibGibson context, configure capability/fallback, dimension/scaler buffers and input behavior, set cleanup.
2. \`DG_DrawFrame\`: validate/convert \`DG_ScreenBuffer\` pixel format and source extent, resample/letterbox to actual available terminal subpixels, submit frame once, render as a raster node with optional ordinary UI HUD/debug overlay. Never write ANSI directly from the adapter.
3. \`DG_GetKey\`: drain a finite event queue, translate keys into Doom's key constants and explicit press/release/tap fallback, refuse/latch invalid events carefully; do not synthesize impossible terminal evidence.
4. \`DG_GetTicksMs\`/\`DG_SleepMs\`: monotonic engine time and bounded wait; **game simulation cadence is distinct from presentation cadence**. Dropped terminal frames must not speed up/slow down Doom logic. Preserve intended game tick pacing, process input without starving the game.
5. \`DG_SetWindowTitle\`: best-effort capability-aware title, no forced terminal escape emission.
6. Startup/end: WAD path verification and error output **outside** the live screen, clean exit, restore terminal, and report accepted input mode/capabilities.

**Test data**: use a lawful user-supplied WAD or Freedoom content; use synthetic RGB fixtures/headless captured frames for CI without distributing game data. Doomgeneric's GPL license and Freedoom's separately licensed art require correct notices if assets/adapter are packaged. No copyrighted id Software assets in the library.

**Primary acceptance:** actual game logic runs; a user can load a map, move, turn, interact, shoot, open a menu, quit and recover their terminal; live frames originate from the real Doom engine and traverse LibGibson raster Surface/render path; reproducible logs identify the Doomgeneric commit/WAD hash, branch, terminal size and keyboard protocol. Distinguish "headless render worked" from "playable on a real terminal".

**Performance target is empirical, not a blanket 30/60 FPS promise.** Record e.g. 80×24, 120×40 and 160×40 terminal profiles, multiple color depths and true/mux/SSH environments *where actually accessible*. Synthetic stress scenes and real Doom shots can have very different dirty-cell fractions. Profile diff generation, bytes/frame, PTY pipe back-pressure, terminal consumption, CPU, memory, simulation tick drift, input delay, resize behavior. If low-bandwidth terminals are slow, use explicit decimation/downsampling and presentation-frame dropping without changing the simulation, not imaginary high refresh.

### Audio, intentionally decoupled

**Playable Doom does not depend on GSPU-FM being finished.** v0.7 can be no-audio, or use a lawful separately implemented/optional sound backend if it fits.

Later, when the independent [GSPU-FM design](https://github.com/femboy2112/libgibson/blob/research/humanmusic-gspu-fm-2026-10-09/docs/HUMAN_MUSIC_GSPU_FM.md) becomes code, route Doom's MUS/GENMIDI-related music intents through an **OPL2/OPL3-compatible** program/channel allocation layer. Generic 4-op Genesis/OPN2 synthesis is *not* the same as authentic Doom Sound Blaster music. Audibly recognizable OPL emulation must use chip-appropriate algorithms, voices, envelopes, rhythm/percussion behavior and instrumentation, with reference tests; do not "convert MUS to MIDI" and declare exact AdLib playback.

Doom sound effects are sourced from game WAD sound assets, *not* procedurally recreated by FM by default. A simple decoded-PCM SFX route can coexist with GSPU FM music without needing a general commercial sample library for HumanMusic. Source assets remain owned/licensed by the game/adapter, not bundled into LibGibson core.

## 4. Beyond v0.7: cross-language pressure becomes a 1.0 requirement

Doom is the first major hard case, **not** the only C++ parity feature and not a shortcut that special-cases foreign consumers. The 1.0 target is the user-established contract: **all supported stable Rust capabilities have equivalent usable C, C++, Python and Go routes**. That is defined in [LANGUAGE_ABI_PARITY_1_0.md](LANGUAGE_ABI_PARITY_1_0.md), tracked continuously rather than postponed to the release deadline.

Initial proposal, subject to feature additions:

| Proposed milestone | Purpose | Non-negotiable exit |
|---|---|---|
| 0.5 | finish current C137/Meatsack and active graphic/temporal work on their own branches | accepted upstream heads, no retroactive promises |
| **0.6** | reusable bulk raster + structured input C API, SDK/C++ wrapper | independent external C/C++ consumer passes |
| **0.7** | GPL-contained Doomgeneric adapter powered by public C SDK | *real playable game* through LibGibson in a terminal; honest terminal constraints |
| **0.8** | deliberate feature-family bridge (Scene, Story, temporal, graphics, audio) | foreign-language representation/ownership/controls defined; no decorative parity claims |
| **0.9** | full cross-language contract tests, lifecycle/platform/resource hardening | language capability matrix no unexamined holes in v1.0 freeze scope |
| **1.0** | stable-intent public CORE + supported-language **functional parity** | Rust/C/C++/Python/Go external consumer tests demonstrate equivalent behaviors on supported configurations |

**Versions are provisional planning buckets.** The maintainer may add, reprioritize, split or remove features. A milestone that gains too much scope should move rather than dropping hard safety/accuracy gates. No new version is promised by this document.

## 5. What could falsify or reframe the plan?

- Doomgeneric's framebuffer/input/timing interface proves incompatible with clean separate C SDK integration; try another open engine or narrow the portability contract, document actual mismatch.
- C bulk-raster API cannot support refresh/resize safely at reasonable resource cost; benchmark and design bounded retained surfaces or better terminal bitmap transports instead of manually emitting ANSI.
- Terminal keyboard protocols cannot support acceptably responsive move/turn/fire on the targeted terminals; gate "playable" by capability, provide an honest degraded fallback, or use a different explicit control mode.
- Full-screen dynamic ANSI has unavoidable bandwidth/terminal latency limits in the tested matrix; distinguish contextual limit from universal impossibility.
- License isolation cannot be established for the selected distribution layout; keep the GPL adapter separate and re-evaluate packaging. Do not contaminate the library's dual license.
- Cross-language parity reveals unexportable Rust-owned behavior; refactor the **shared substrate**, build safe callback/handle semantics, or accurately narrow which capabilities are stable and supported. Do not build shadow engines.
- Doom becomes expensive relative to actual SDK value; keep the general-purpose ABI improvements and treat the game as a consumer demonstration instead of a core feature.

## 6. Source ledger and future handoff

**Observed (source inspection, not build result):** Rust half-block raster/surface/node path; C ABI v1 basics but no bulk raster node; input phase not preserved; Doomgeneric narrow C callback contract. No Doom port, benchmark, or terminal playback was executed during planning.

**Design / UNVERIFIED:** correct C frame import under Doom source layout; high-refresh terminal performance; real key-release path on selected terminals; actual playable Doom with LibGibson; GSPU-OPL music integration; v1.0 parity across languages. Required new evidence = native C and C++ consumers, PTY/terminal runs, hostile inputs, recorded game controls, and independent ABI/parity tests.

Primary paths and external references (re-check on implementation):

- \`src/raster.rs\`, \`src/node.rs\`, \`src/surface.rs\`, \`src/renderer.rs\`, \`src/temporal.rs\`, \`src/input.rs\`, \`src/session.rs\`, \`include/gibson.h\`, \`include/gibson.hpp\`, \`src/ffi.rs\`, \`abi/gibson-abi-v1.symbols\`, \`docs/RELEASE_CONTRACT.md\`.
- [Doomgeneric README](https://github.com/ozkl/doomgeneric/blob/master/README.md), [Doomgeneric C API](https://github.com/ozkl/doomgeneric/blob/master/doomgeneric/doomgeneric.h), [Doomgeneric SDL reference](https://github.com/ozkl/doomgeneric/blob/master/doomgeneric/doomgeneric_sdl.c) (press + release key events), [Freedoom repository](https://github.com/freedoom/freedoom), [Freedoom license](https://github.com/freedoom/freedoom/blob/master/COPYING.adoc).
- [GSPU-FM research PR #90](https://github.com/femboy2112/libgibson/pull/90); C137 active PR #87 at design time: https://github.com/femboy2112/libgibson/pull/87.

**Closing invariant:** genuine Doom produces the pixels; **LibGibson owns the terminal**. C/C++ Doom consumers exercise the same public API future Python/Go/Rust consumers rely on. The game is a pressure test of an honest general platform, not a special workaround.
