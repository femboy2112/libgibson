# Doom Terminal Platform — deferred implementation handoff and validation gates

> **RESEARCH/HOLD ONLY.** This plan may be discovered by future agents while C137, MeatsackProjection and GSPU-FM are still moving; do not execute merely because it exists. Do not merge this research branch or modify main/current feature branches without the maintainer's explicit approval. The game port and language-parity work have **not been implemented** here.
>
> Read these source-of-truth research documents in order:
> 1. [Terminal Doom v0.6–0.7 plan](DOOM_TERMINAL_0_6_0_7_PLAN.md)
> 2. [1.0 language ABI parity plan](LANGUAGE_ABI_PARITY_1_0.md)
>
> Provenance: \`research/doom-terminal-abi-parity-2026-10-09\` rooted at \`main@80706c91b88e59495b131bfd8ab23aa3ed013414\`; branch is documentation only. Both milestone numbers and new requested features are **living placeholders**. The stable feature set and acceptance law should be refreshed with the maintainer before implementation/release.

## A. On future authorization: cold-start reproducibility

1. Fetch repository instructions, current default branch, active PRs and accepted work, exact commit SHAs, tests and release status. Treat design-parent source paths/versions as **historical**. Record source/branch/commit and actual toolchain.
2. Inspect actual Rust \`RgbRaster\`, \`Node::raster\`, \`Surface\`, \`Context\`, diff/renderer, \`input.rs\`, \`TerminalSession\`, \`temporal.rs\`, C header, C++ wrapper, Rust FFI and all binding consumer tests. Read active \`ROADMAP.md\`, \`RELEASE_CONTRACT.md\`, \`STATE_OF_LIBGIBSON.md\`, ABI baseline, CI and GSPU plan.
3. Fetch **pinned upstream Doomgeneric commit** and verify \`doomgeneric.h\`, platform callback set, real framebuffer packing, build options, WAD loading and GPL license. Fetch legal Freedoom assets/license only as needed and pin checksums. Do not embed copyrighted id assets in repository.
4. Create a **new feature branch based on the accepted current LibGibson integration head**, not this stale research branch; preserve the research branch and FM design PR as unmodified provenance. Use a separately licensed Doom integration project/fixture where feasible, not GPL content in core.
5. Freeze baseline: C/C++/Python/Go installed-SDK samples, ABI-v1 exported symbol set, ownership/error/resize PTY tests, representative Rust \`Node::raster\` output, performance baseline. Do **not** claim Doom or new ABI tests until run.

## B. Execute discriminating rounds, not feature-dump commits

**R0 — Establish minimal C raster cut**

- Predeclare ownership semantics for a synchronous RGB submission. Inputs include exact pixel format, row stride, width/height, byte length, foreground/background alpha behavior. Reject invalid/malformed inputs before allocating.
- Add test fixtures: 2×2 24-bit red/green/blue/checkerboard and a deliberately unequal/padded stride; BMP-style BGR, swapped-channel negative control; zero dimensions and \`SIZE_MAX\` overflow cases; mutation of source bytes after upload to detect illegal borrowing.
- Compare the C consumer's resulting full \`Surface\` (or VT trace) with Rust's own raster path on the SAME fixture. Assert old v1 text/UI API still emits byte-identical output.
- Only then add a narrow \`RasterHandle\` / \`Node::raster\` bridge; prefer one batch upload over millions of \`set_cell\` calls. Add RAII C++ wrapper once C ownership proven.

**R1 — Input truthfulness**

- Characterize current event behavior: crossterm source may provide event phase, but existing \`input.rs\` mapping discards key \`kind\`; the v1 ABI event has no press/release field.
- Implement a separately versioned event2 or equivalent opaque queue, preserving legacy \`gibson_poll_event\` layout/behavior. Preserve keys, modifiers, resize and paste with explicit unsupported/degraded capability codes.
- Test genuine press/release with a protocol and terminal that supports it, repeat/toggle fallback on a basic PTY, simultaneous forward+turn+fire, holding a key over UI redraw, focus/resize resets, stuck-key prevention, Esc, Ctrl-C/process termination and shell restoration. A synthetic release timeout must be provenance-tagged, not asserted to be hardware truth.

**R2 — Independent C raster consumer**

- Write a separate C program using installed headers and \`-lgibson\` only: synthetic RGB frames, moving stripes, gradients, text overlay via ordinary node composition, resize and a bounded 5–10 minute stress run.
- Profile allocated bytes, framebuffer copies, dirty cell fraction, terminal wire bytes, generation/write durations, host CPU and peak RSS. Reproduce with slow/blocked PTY and compare to a headless VT baseline.
- Distinguish target dimensions, displayable image samples and cell aspect. For halfblocks, pixel grid is \(cols \times 2*rows\); render into effective viewport, not full Doom native resolution by fiction. Test monotone scaling, crop/letterbox/resize, palette fallback and black edges.

**R3 — Doomgeneric C adapter**

- Implement only the upstream platform callbacks; no LibGibson-specific Doom engine fork.
- First demonstrate game startup/headless tick and a *real Doom-generated* frame on a controlled WAD. Map byte order/palette explicitly, with tests; never reinterpret \`uint32_t*\` as an unspecified RGB layout.
- Feed frames through the public **C SDK raster route**. Do not print raw ANSI or use Rust private calls as a workaround. Keep game tick 35 Hz-ish native behavior (as actually measured on chosen engine/version) **independent** of slower terminal presentation. Frame dropping may affect display but not game simulation.
- Inject/replay controlled key sequences; then test actual human interaction in a real terminal: move, strafe/turn, fire, use/open, menu, quit, terminal restoration. Report any required enhanced keyboard protocol.
- Build and run with a lawful WAD; record exact WAD provenance, engine commit, terminal emulator/size, SDK commit, frame hashes, input-event trace and performance.

**R4 — Performance and gameplay polish (only after R3 success)**

- Bench \`80×24\`, \`120×40\`, \`160×40\` *if supported by actual environment*, truecolor/ANSI256/mono, static halfblock and optional Braille/graphics variants. Real refresh bandwidth can dominate game rendering. No untested 30/60 FPS guarantee.
- Support reliable resizing and HUD overlays without extra terminal ownership; test alternative/fullscreen screen and clean exit.
- Test game-control latency under terminal saturation, packet throttling, and a slow consumer. Present dropped-frame and latency counters distinct from game time.

**R5 — Music/audio (decoupled)**

- Keep initial game playable without FM. Only when independent GSPU OPL implementation is accepted, map MUS/GENMIDI instrument semantics to OPL2/OPL3 note/channel/control playback and compare against a lawful OPL reference (e.g. Chocolate Doom behavior) with independently derived fixtures.
- Doom effects are WAD PCM assets: decode and play through an appropriately licensed/optional sample playback path if desired, with no general sample-library requirement. Do not mistake Genesis OPN2 for Sound Blaster OPL.
- No audio callback locks, allocations, bad reentrancy or mixed clocks; terminal write speed must never become sound sample scheduling.

**R6 — Generalize from Doom to the language parity plan**

- Build a non-Doom C++ raster/video app and at least a Python and Go clean-room raster/input consumer as the v0.8–0.9 bridge, then Scene/Story/temporal/audio as they are stabilized.
- Maintain [the capability matrix](LANGUAGE_ABI_PARITY_1_0.md) with per-language method name, source SHA, test name, platform/ABI restrictions, actual result and residual. Every public **stable 1.0** Rust feature has equivalent C, C++, Python and Go surface/observables.
- Review any newly introduced Rust-only capability for future parity immediately. No giant monolithic last-minute binding rewrite.

## C. Minimal predeclared hostile matrix

| Case | Expected result | Falsified by |
|---|---|---|
| B01: 2×2 RGB frame / known half-block cells | C display equals Rust/reference cells | channel swap / wrong top-bottom ordering |
| B02: padded stride + shorter byte length | safe display / typed failure | out-of-bounds reads or silent reinterpretation |
| B03: oversize/overflow dimensions + nulls | refused without allocation/write | wraparound, panic or UB |
| B04: reuse/free source pixels after upload | clear copied/borrowed semantics | use-after-free hidden by a passing visual frame |
| B05: update raster while node visible | next frame correct, old handles safe | stale surface or hidden second renderer |
| K01: real phase-aware keyboard protocol | press AND release detected | press-only event mislabeled release |
| K02: basic terminal lacking release | explicitly degraded fallback, usable as declared | claimed true release or permanent stuck key |
| K03: concurrent controls, resize and shutdown | reproducible input, terminal restored | modifier/press loss, terminal ownership leak |
| T01: same game-state/input replay under slow terminal | same simulation progression within declared deterministic scope | presentation gating changes game logic |
| T02: partial/unchanged/dynamic frames | measured changed-cells and bytes truthful | fake diff wins on static tests only |
| D01: engine boot from WAD, real frame | Doomgeneric frame through C ABI | pre-rendered image or Rust-only bypass |
| D02: move/turn/shoot/use/menu/quit | playable controls on declared terminal/input mode | unplayable held keys or no clean exit |
| D03: GPL adapter packaged with permissive SDK | clear license/notice/dependency chain | GPL engine silently included in core |
| F01: old ABI-v1 consumer on new SDK | original exports/layout/semantics honored | old sample breaks merely from additive changes |
| P01: one stable feature C/Rust/C++/Python/Go | equivalent observable contract on supported platform | wrapper compiles but does less |
| A01: delayed FM music integration | same gameplay/audio-independent tick | audio absent blocks display acceptance |

All probes report PASS/FAIL/AMBIGUOUS with raw output and exact scope. Mutation controls include swapped RGB channels, dropped key-up, illegal stride, premature raster free, duplicated main-loop tick, skipped game ticks under frame drop, and raw ANSI side-channel injection. Don't silently fit a test fixture and call it a holdout.

## D. Operational gates and release discipline

- Never work directly on \`main\` or active C137/Meatsack branches; no autopublishing/merge. Create scoped commits, exact-head hosted CI, and a **draft PR** once implementation is legitimately authorized.
- Maintain pre-change v1 ABI symbols (run \`scripts/release/check-abi.sh\`), wrapper smoke tests, C++ sanitizers and optional Rust sanitizer coverage. Run \`cargo fmt --check\`, strict clippy, unit/integration tests, release build, FFI regression and full PTY suite as relevant. No suite result may be invented.
- Test external installed SDK in an independent build directory, not with a repository-relative source include that masks a distribution failure. Keep linker SONAME and shared/static expectations honest.
- Preserve terminal restoration on error, panic, signals where supported; never let two independent terminal sessions own the same TTY.
- No universal FPS/perceptual/headless claims. State source game tick, displayed cadence, image dimensions, emitted wire bytes and system test conditions.
- The game and synthetic C consumer are two independently useful deliverables. A Doom build failure must not erase a correctly designed cross-language raster ABI improvement.
- Move/cross-link reviewed docs into live roadmap only when the maintainer requests integration after the accepted source advances; reconcile \`docs/RELEASE_CONTRACT.md\` explicitly before v1.0.

## E. Future-agent launch operator — paste after explicit go

> Inspect \`femboy2112/libgibson\` and first read \`docs/DOOM_TERMINAL_0_6_0_7_PLAN.md\`, \`docs/LANGUAGE_ABI_PARITY_1_0.md\`, and \`docs/DOOM_TERMINAL_PLATFORM_HANDOFF.md\` from \`research/doom-terminal-abi-parity-2026-10-09\`. These documents are research-only and historical: fetch current branch/PR heads, repository instructions, accepted work, C ABI and wrapper surfaces. The target is a **REAL terminal-playable GPL Doom-compatible C app using only LibGibson's public SDK** as its graphics/input platform, with v0.6 bulk raster+structured key-state ABI work and v0.7 Doomgeneric integration, and a continuing v1.0 goal of **supported Rust functionality accessible with equivalent observable contracts through C, C++, Python and Go**. Freeze old ABI/output controls; use a new non-main feature branch from the accepted current head. Red-first implement safe bulk pixel upload, ownership/stride/format checks, raster-node composition and honest press/repeat/release/unknown events without breaking existing C ABI v1. Build a clean-room external C raster stress client before attaching Doomgeneric callbacks; keep GPL engine/adapter code separately licensed from permissive LibGibson core. Verify game-origin framebuffer, accurate clocks, keyboard fallback, playable movement/fire/menu/quit, resizing, terminal cleanup and real throughput. Doom's native world renderer remains upstream; LibGibson translates its pixels to terminal cells. Optional FM music via GSPU-OPL waits until that separately tested engine exists, and SFX may use lawful WAD PCM. Extend the parity matrix with independent C/C++/Python/Go consumer tests, not just signatures. Do not merge, tag, bump version or publish without maintainer approval. Record exact SHA, measured tests/frames/latencies, red controls, licensing, unsupported platforms and next verdict-changing probe.

**Owner's durable direction:** Versions and features are negotiable as the engine evolves; meaningful 1.0 foreign-language parity and the integrity of the renderer/ABI contracts are not cosmetic.
