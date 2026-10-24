# LibGibson Architectural Roadmap

This document outlines completed work and the planned future milestones for LibGibson.


## Current direction (2026-10-19): v0.5.0 — Interactive Observable Instruments

**Status (2026-10-19 snapshot): ROADMAP/PLANNING ONLY; no v0.5.0 release, merge, or new API accepted on main.**
*Update 2026-10-24: the v0.5.0 interactive-instruments milestone is delivered and
**released as v0.5.0** (tag `v0.5.0` → `d8c7b99`, GitHub prerelease; see the 2026-10-24
reconciliation below). The instrument-driven cinematic film moved to 0.6.0.*
The current published release is v0.5.0 (v0.4.0 is the prior release). This section
describes the *v0.5 program as originally proposed*; the historical progress and earlier studies below remain evidence,
not a competing set of current execution priorities.

**New canonical plan:** [docs/V0_5_INTERACTIVE_INSTRUMENTS_ROADMAP.md](docs/V0_5_INTERACTIVE_INSTRUMENTS_ROADMAP.md).  
**Memory-bounded multi-agent handoff:** [docs/V0_5_EXECUTION_HANDOFF.md](docs/V0_5_EXECUTION_HANDOFF.md).  
**External forcing consumer:** [SmartChem 3.0 #125](https://github.com/femboy2112/libgibson/issues/125) and children #126–#146.

The release's organizing purpose shifts from continued **HumanMusic expansion** to a
**domain-neutral, compositional interactive-instrument platform**. Gracefully freeze and
document the ear-accepted/experimental C137 and Homosapians work (including PRs #116,
#123 and #124); no new subjective song/instrument/arrangement refinement is a v0.5
gate. Preserve acoustic reference locks and unresolved evidence rather than
merging unaccepted music.

**Dependency-ordered train:**

1. **R0:** audit accepted main and overlapping draft PR ancestry; preserve music
   results and establish a non-destructive integration branch.
2. **R1–R2:** fix/document clock and terminal-event correctness (#130, #15, #79);
   typed inject/replay (#131), opt-in mouse/focus (#126), world transforms (#128),
   layout-derived hit testing, drag capture and modal occlusion (#127).
3. **R3–R5:** validated high-rate Rust visual frames (#129), bounded snapshots
   separate from reliable commands (#141), minimal geometry editing (#132),
   numeric inspectors (#134), edit transactions (#137), scalar plots/fields/probes
   (#135/#136; reuse PR #81), capture/export (#138). Profile #77/#133 before
   architectural optimization; require #139/#140 truthful fallbacks.
4. **R6:** prove the public interface with a runnable **synthetic** instrument
   workbench (#144), a small Python-producer/Rust-frontend consumer canary and
   observability (#143). No chemistry-specific solver enters LibGibson.
5. **R7:** upgrade the **new** terminal meme intro with the Filthy Frank keyed
   reaction, now showcasing genuine interactive instruments and a frozen,
   provenance-identified soundtrack; test and obtain owner art acceptance.

**Scope boundary:** broad high-throughput C/C++/Python/Go functional parity (#142),
advanced pixel protocols (#146) and optional sonification (#145) remain future
work. The narrow producer bridge does **not** constitute a supported full Python
UI SDK. Preserve v0.4.0 ABI v1 and Rust MSRV 1.85 unless explicitly reviewed.

**Existing draft-history warning:** PR #86's cinematic branch descends from
the scientific plot/spatial-UI work in PRs #81/#83 (which also contain merged-into-
feature-branch PR #85). Integrate by **actual git ancestry**, never blind
double-cherry-picking. The pre-existing owner directive in draft PR #92 for an
upgraded Frank reaction film remains; its old music/auditory shot scope should be
reconciled to the instrument-first plan, not silently overwritten. No version
bump, branch merge, tag or release without explicit maintainer authorization.

**Release stopping rule:** a fresh consumer can draw/edit, inspect, pause/step,
replay and capture a nontrivial domain-neutral instrument using LibGibson's public
APIs with truthful keyboard/mono/real-PTY behavior, bounded resources and
declared remaining limitations. Unit success on disjoint feature branches
does not certify the integrated exact SHA.

### Reconciliation (2026-10-23): plan → implemented-on-branch

The plan above is the **2026-10-19 snapshot** (its train and any SHAs it cites are
that snapshot; they are not re-verified here). As of 2026-10-23 the v0.5 program is
**implemented on a branch and CI-green, but unreleased and unmerged** — `main` is
still frozen at `80706c9` (v0.4.0). The single stabilization candidate is
**[PR #181](https://github.com/femboy2112/libgibson/pull/181)** (branch
`integrate/v0.5-release-merge-gate-2026-10-23`). The states are distinct:

- **Released (on `main`):** v0.4.0 only (`Cargo.toml` is still `0.4.0`; no v0.5 tag).
- **Accepted-on-branch v0.5 (in PR #181, CI-green):** world transforms (#128),
  layout-derived painted hit-map + drag capture + modal occlusion (#127), pointer
  eligibility / inert targets, session-owned mouse/focus lifecycle (#126, #79),
  bounded snapshot/command bridge (#141/#161), immutable shapes + narrow-phase
  `Shape::hit` (#132/#165), numeric inspectors with schema validation (#134),
  reversible edits / compound undo with honest `RollbackFailed` (#137/#166),
  scalar fields/plots/probes (#135/#136, reusing #81), the flagship instrument
  workbench (#144), the `gibson::wire` bounded transport + Python conformance
  (#141), and an independent cold-start consumer (`scope_editor`).
- **Integrated-but-unreleased:** everything in the bullet above lives only on PR #181;
  none of it is on `main` and none of it is in a tagged release.
- **Known bugs / open items:** the `pty_hangup` test's between-polls flake — **fixed
  in this candidate** (terminal identity latched across the liveness transition,
  `src/input.rs`); a deferred bidirectional producer-ack canary (one-way canary +
  conformance shipped); a latent stale-`TargetId` epoch (documented, not yet a
  built witness). `Node` gained a public `tag` field — a source-breaking CORE change
  for raw struct-literal users (see `CHANGELOG.md`); C ABI v1 and MSRV 1.85 unchanged.
- **Outstanding v0.5 release gates:** main branch-protection + required review (the
  owner's merge button); version bump + changelog; a clean release preflight on the
  exact RC; SDK/wrapper artifact review; the upgraded cinematic finale (PR #92) and
  the owner's visual/listening acceptance; explicit release authorization. None are
  satisfied by the development CI matrix alone.

### Reconciliation (2026-10-24): v0.5.0 released

The v0.5 interactive-instruments milestone is **delivered and released as v0.5.0**
on 2026-10-24 (GitHub prerelease, Engineering Alpha, Linux x86_64; tag `v0.5.0` → main
commit `d8c7b99`; `Cargo.toml` and the Python `pyproject.toml` are 0.5.0; CHANGELOG `0.5.0`
entry dated 2026-10-24; `main` is now 0.5.0). Release assets: the native SDK archive
`libgibson-0.5.0-linux-x86_64.tar.gz` (+ `.sha256`), the Python wheel and sdist. v0.4.0 is
the prior release; the 2026-10-23 "outstanding gates" list above is a dated snapshot of
what was open then, superseded by the release. All new surface is experimental
and Rust-only (not exposed through the C, C++, Python or Go wrappers); C ABI stays
version 1; MSRV stays 1.85. The cinematic finale ships as the example
`examples/libgibson_intro_reaction` (no third-party media; original synthesized performer;
real footage only via a local `--clip=PATH`). The **instrument-driven film**
(Workbench/FieldView/plot/producer causal chain in the picture) is **deferred to 0.6.0**;
for 0.5 the instruments ship as separate executable demos.

Music is **frozen** and documented — see
[docs/V0_5_MUSIC_FREEZE_LEDGER.md](docs/V0_5_MUSIC_FREEZE_LEDGER.md); the C137 /
Homosapians research branches are preserved, not merged.

---


> **Current status (2026-10-02):** **v0.4.0** is released on GitHub (Engineering Alpha,
> Linux x86_64; tag → `v0.4.0` at `c2f6483`, prerelease-flagged) — **HumanMusic & Audio**:
> a deliberate additive minor that adds the experimental, Rust-only `gibson::audio` axis
> (deterministic offline synthesis, DSP, WAV, optional `audio-cpal` device output) and
> HumanMusic, a procedural composer and checked band driven by semantic style/story state,
> with Cover Mode and three sonic worlds (see
> [docs/HUMAN_MUSIC_ARCHITECTURE.md](docs/HUMAN_MUSIC_ARCHITECTURE.md)). C ABI unchanged at
> v1; MSRV 1.85; 1,522 tests (18 ignored). It builds on **v0.3.1** (tag → `v0.3.1` at
> `1010209`) — **Temporal hardening + keyed compositing**: incremental region accounting
> (fixes #68), keyed greenscreen compositing and a surface-local reaction-cut flagship that
> composites a keyed subject into the existing intro film, all **additive** to the
> experimental Rust-only `gibson::temporal` axis (850 tests) — and on
> **v0.3.0** (tag → `v0.3.0` at `e57cdb2`), the **Temporal
> Rendering Milestone**: high-cadence phase-locked frame pacing (`FramePacing`) and
> split generation/write per-frame diagnostics (`FrameReport`) as CORE infrastructure,
> plus an experimental, Rust-only temporal cell realization axis (`gibson::temporal`)
> that is **static by default** and emits ordinary `Surface`s through the one existing
> renderer. The temporal feature's perceptual benefit is setup-dependent and **unproven**
> (see [docs/TEMPORAL_DISPLAY_VALIDATION.md](docs/TEMPORAL_DISPLAY_VALIDATION.md)); GPU
> acceleration was probed but left unmeasured. It follows the Europa v0.2.x patch train —
> v0.2.5 (tag → `6b7096a`), v0.2.4 (`d991c93`), v0.2.3 (`e219841`), v0.2.2 (`0052dbe`),
> v0.2.1 (`219a592`) — over the experimental semantic UI milestone v0.2.0 (2026-09-26;
> tag → `7a10e60`), itself a deliberate additive minor over v0.1.1 (tag → `3789332`) and
> v0.1.0 (2026-09-25). The C ABI is unchanged (`GIBSON_ABI_VERSION` 1); `gibson::audio`,
> `gibson::temporal` and `gibson::ui` are EXPERIMENTAL, not CORE. The v0.3.0 suite was
> **792 tests (296 unit + 494 integration + 2 doctests)**.
> Ecosystem-registry publication (crates.io / PyPI / Go module proxy) remains deferred.
> The checkpoint counts and CI run ids below are historical development evidence,
> preserved as measured.

## Post-merge priorities

The approved cinematic branch is merged at `0b673cc`. That checkpoint validated
**536 tests (226 unit + 310 integration)**. Current intro/consolidation evidence is
[recorded separately](docs/INTRODUCTORY_CINEMA.md). See the canonical
[State of LibGibson](docs/STATE_OF_LIBGIBSON.md) for the ranked P0–P3 roadmap,
known correctness gaps and release boundaries, and the
[validation index](docs/VALIDATION_INDEX.md) for historical checkpoints.

Public-readiness preparation now supplies full license files, read-only SHA-pinned
CI, corrected native loader paths, and a Go module/example that passes local Linux
smoke. The owner authorized public visibility. Exact-main public CI now passes all five
jobs: [run 35946157443](https://github.com/femboy2112/libgibson/actions/runs/35946157443)
on `aa60036` (Rust 1.98.1 and Go 1.27.1), the first public checkpoint. This does not establish an MSRV.

PR #14 is merged at `e5ede0a4ab8ff52caa567ee10d45824e42c1ebc6`, containing
flagship tip `11d2cca12430f68eb8fa3de3231827e1176e6c27`. The merged baseline passes
**574 tests (226 unit + 348 integration)** locally on Rust 1.98.1 and all five
exact-main public jobs in [run 35960000454](https://github.com/femboy2112/libgibson/actions/runs/35960000454).
It fixes rectangle arithmetic and exact changed-coordinate reporting, consolidates
bounded Unix PTY harnesses, and adds the canonical introductory short film.
Next: maintain useful public CI and owner-approved protection;
establish API/MSRV/package contracts; profile sustained graphics and build a platform matrix.
Avoid expanding graphics features before those remaining contracts improve. The phase sections below
preserve development history rather than promise an execution order. Their older
CI/Go statuses are historical and superseded by the public run linked above.

## Event delivery investigation

[Event Pressure Lab](docs/EVENT_PRESSURE_LAB.md), PR #18, isolates issue #15 at
Crossterm's consumption of a simultaneous signal/TTY readiness batch. Raw input,
standalone Crossterm, Context, syscall observation and a scratch token-retention
intervention separate this from graphical output delays and screen reconstruction.
[Upstream #1126](https://github.com/crossterm-rs/crossterm/issues/1126) carries the
minimal independent reproducer. **No production fix is shipped; #15 remains open.**
Next: evaluate a supported upstream repair, including zero-timeout behavior,
large reads, EOF/error handling and bounded fairness, then enable the currently
red collision-delivery acceptance. Local branch suite: 591 passed (226 unit + 365
integration), one known-red acceptance ignored. D6/D7/D8 remain separate programs. The lab's
finite output/latency observations are diagnostic evidence, not a long-session policy.

## Runtime architecture megaround

Branch `claude/runtime-architecture-megaround` / PR #19 turns three previously
implicit runtime contracts into explicit, tested, and *observable* ones:

- **#11 terminal ownership** — a process-global lease (Available/Owned/Restoring);
  a second owner errors with `AlreadyExists`; `restore()` is one-shot, reports its
  first error with best-effort completion, and releases the lease even on failure;
  Drop stays best-effort. Round II: panic restoration is owner-thread-scoped, so a
  recoverable non-owner worker panic no longer tears down the owner's terminal (a
  real bug, proven on a PTY then fixed); the restore-failure path (fd → /dev/full)
  and host-hook chaining are now tested. IMPLEMENTED + TESTED (7 PTY tests).
  **Not closed**: awaits PR #19 merge + final audit.
- **#10 trace retention** — measured (~40 B/tick, unbounded) then bounded:
  `TraceRetention {All (default), Bounded(cap), Disabled}` + `drain_trace` with
  honest `is_complete()`/`dropped_steps()`. Round II: the *beat* log is bounded
  too (a ping-pong story leaked ~44 MiB of beats at 1M ticks, invisible to the
  steps-only fix), `dropped_beats()` added, `Bounded(usize::MAX)` overflow fixed,
  and the resource inventory is documented
  ([`RESOURCE_OWNERSHIP.md`](docs/RESOURCE_OWNERSHIP.md)): Facts/Scene are
  caller-owned domains, not engine leaks. Default `All` = no behavior change.
  IMPLEMENTED + TESTED. **Not closed**: awaits PR #19 merge + final audit.
- **#15 crossterm input starvation** — CORROBORATED, and the naive drain fix is
  *proven to hang* on the blocking `VMIN=1` fd. Round II: the minimal Shape A fix
  is implemented and independently verified in an isolated crossterm 0.29.0 clone
  (stock reproduces the stall, the fix delivers both events with no hang), then
  rebased onto crossterm `master` and **filed upstream as
  [crossterm#1128](https://github.com/crossterm-rs/crossterm/pull/1128)** — not yet
  merged, **NOT vendored** ([`CROSSTERM_1126_FIX_ANALYSIS.md`](docs/CROSSTERM_1126_FIX_ANALYSIS.md)).
  **Still open** — LibGibson still builds on unpatched crossterm 0.29.0.

The [Runtime Observatory](docs/RUNTIME_OBSERVATORY.md) is now a live interactive
instrument (Million-Tick, Ownership-Duel, Restore-Failure, Endurance) plus the
deterministic `--dump` frames, rendering the contracts from real state. Full local
suite: **612 passed (236 unit + 376 integration)**, 0 failed, 1 known-red ignored,
on Rust 1.98.1; PR #19 public CI green. D6 (MSRV / API-stability / installable-package
contract) is now RESOLVED via the pre-1.0 release contract (PR #21) — declared+tested
MSRV 1.85, C ABI v1 policy, clean-room external consumers, and a publication-free
release preflight. **`v0.1.0` is now released on GitHub** (2026-09-25; tag →
`a3f1e29`, prerelease-flagged, Linux x86_64 SDK attached); ecosystem-registry
publication (crates.io / PyPI / Go module tag) stays deliberately deferred. Next:
track crossterm#1128 upstream (filed; awaiting maintainer review) toward a fixed
release LibGibson can adopt.

## Verification labels

- **IMPLEMENTED + TESTED** — the code exists and is covered by automated tests in `cargo test`.
- **PARTIALLY TESTED** — implemented, with a named facet that is not automatically verified.
- **UNVERIFIED** — present in source or assumed, but never compiled/executed in an attested environment.
- **PLANNED** — not implemented.

> Historical note: earlier revisions of this roadmap marked phases "Completed" and listed Go bindings as verified. Go was **UNVERIFIED** at those checkpoints. The public-readiness round now supplies local build/example evidence; historical entries below retain their original boundaries.

---

## Cinematic machine presentation — IMPLEMENTED + TESTED, EXPERIMENTAL demo-local

- [x] Default shot-directed machine from frame one; legacy flat/cyber retained.
- [x] Deterministic camera and exposure transitions, responsive focal framing,
  distinct subsystem architecture and depth-checked Braille structural rails.
- [x] Planner-positioned Acid actor, trace chase, feint paths, isolation rupture,
  mirror materialization, DISPLAY UI echo, distinct endings and scarred aftermath.
- [x] Three-row stable controls, uppercase shortcuts and numeric final actions;
  ordinary typed input and autonomous Crash preserved.
- [x] Shot/camera/raster replay, frozen 0/0/0, geometry and damage probes,
  real PTY shortcuts/full auto and seven-shot resize exercise.
- [x] Production-design research, release PTY frame inspection and stage matrix.
- [x] At the cinematic presentation checkpoint: **535 tests** (226 unit + 309 integration); strict local gates and binding
  smoke tests pass. Remote CI remains billing-blocked.
- [ ] Independent human aesthetic acceptance remains **UNVERIFIED**; screenshots
  and test metrics are not a substitute for the user's visual judgment.

See [research](docs/acid-vs-crash-cinematic-research.md) and
[validation/commands](docs/acid-vs-crash-cinematic-validation.md).

## Terminal software graphics — IMPLEMENTED + TESTED, EXPERIMENTAL Rust-only

- [x] Opaque RGB raster, software blending, half-block realization, Mono density,
  bounded shapes and dependency-free PPM inspection.
- [x] Filled triangle meshes, camera/frustum clipping, reciprocal-depth Z buffer,
  Lambert shading, fog, emissive terms and depth-tested paths.
- [x] Ordered bounded RasterFx, explicit feedback history and procedural fields.
- [x] Seven borderless FX Lab scenes, deterministic input/resize/freeze checks.
- [x] BattleGraph → shaded topology + influence field + trails; automatic dive,
  DISPLAY lattice, existing UI fragment, stable controls and ending reassembly.
- [x] Full semantic/visual replay; RGB counterfactuals; frozen 0/0/0 law;
  graphical counterplay and resize in real PTYs. Original flat goldens retained.
- [x] At the RGB substrate checkpoint: **515 tests** (226 unit + 289 integration), plus three explicit FX Lab
  generator tests. All required local gates and native binding smokes pass.
- [ ] Subjective “mistaken for an image protocol” acceptance is **UNVERIFIED**;
  visual quality has direct screenshot inspection, not an independent human panel.
- [ ] Sustained performance across terminal emulators, SSH and multiplexers is
  **PARTIALLY TESTED**. Optional SDF raymarching and general mesh/font loaders
  are not implemented; the triangle path is the hero renderer.

See [graphics evidence and exact commands](docs/acid-vs-crash-rgb-validation.md).

## Window into Crash's machine — IMPLEMENTED + TESTED

- [x] Crash fights by default; typed interventions remain available. `--manual`
  selects human defense; `--auto` retains automatic exit after resolution.
- [x] Deterministic 1.2s operator pauses after accepted actions, visible command
  receipts, persistent aftermath and exact replay of autonomous counterplay.
- [x] Foreground-only route strokes, thin Braille influence fronts and packet
  trails; presentation advances between 50ms world steps at a 60 FPS ceiling.
  Whole-panel jitter is reserved for near-complete display possession.
- [x] Five additional tests: default operator PTYs, aftermath replay, operator
  pacing/intervention, sub-quantum motion/replay, foreground route rendering.
  At that checkpoint: **469 tests** (226 unit + 243 integration).

See [spectator refinement evidence](docs/acid-vs-crash-spectator-validation.md).

## Living battlefield round — IMPLEMENTED + TESTED, EXPERIMENTAL Rust-only

- [x] Scoped SurfaceFx: rect, normalized wipes/radial/band, seeded noise; wide
  glyph, transparency, outside-cell preservation and local-damage tests.
- [x] Demo-local topology reducer with fixed-point influence, separate integrity,
  legal path traversal, telemetry loss, resource costs and cooldowns.
- [x] Deterministic scored Acid planner with personality, trace/isolation/decoy
  memory, legal pivots and repeat-decoy feints. State-based auto defender.
- [x] Broad Story acts consume semantic milestones; in-beat battles continue
  without tactical micro-beats or timer-authored ownership.
- [x] Full EncounterTrace replay includes graph, planner, resources, history,
  StoryDirector, mounts and realized scene. Shared-prefix counterfactuals change
  targets, legal routes, facts and screens.
- [x] Directed influence fronts, actual graph traffic and ghost trajectory,
  full-width climax composition, stable command island, aftermath quality/scars.
- [x] Eight new glyph/style goldens; 13 retained/refreshed Acid PTY cases; all 30
  other demo snapshots unchanged. Eight Acid PTY tests cover genuine adaptation,
  telemetry trade-offs, replay, auto completion and terminal restoration.
- [x] Local fmt, all-target/all-feature strict Clippy, **464 tests** (226 unit +
  238 integration), release, C/C++/Python and C/C++ ASan/UBSan smoke.
- [x] DESIGN §33 distinguishes exact semantic delta, affected footprint and wire
  cost without renaming existing APIs.
- [ ] Extended human playtesting and subjective cinematic impact remain
  **PARTIALLY TESTED**. Linux terminal/VT100 evidence is not universal portability.
- [ ] Go, Windows and broader emulator/multiplexer/SSH behavior are **UNVERIFIED**.
- [ ] No stable serialized encounter replay format; traces are in-memory.
- [ ] Remote Actions remain **BLOCKED / ENVIRONMENTAL** (billing, zero steps).

See [round II architecture, commands and evidence](docs/acid-vs-crash-round2-validation.md).

## Cinematic frontier — IMPLEMENTED + TESTED, EXPERIMENTAL Rust-only

- [x] Ordered entity `SurfaceFx` on arbitrary ordinary nodes; transparent scratch
  only when active. Glyph-safe style/scan/tear/scramble/dissolve and progressive
  StyleMask; removal restores the underlying node.
- [x] Additive displacement/jitter independent of legacy placement; persistent
  finite-period loops and overflow-safe timing.
- [x] In-beat Reactions with explicit event/action order, one transition per
  update, validation, exact replay and bundle clocks.
- [x] `acid_vs_crash`: coherent fictional machine, semantic ownership, deterministic
  adversary/auto defender, interactive counters, local branches/rejoin, display
  possession, stable command island and three endings. No real attack capability.
- [x] 13 new semantic PTY goldens; seven PTY interaction/capability cases;
  whole-renderer frames at 56x24, 80x24, 120x32, 160x40; live resize and input
  preservation; stage/branch replay and bounded damage checks.
- [x] FX Lab scene 20 demonstrates generic post-processing, additive motion and
  reactions without transitions.
- [x] Fixed natural clipping of positioned panels and stale blank cells on
  fullscreen resize. Existing 30 visual goldens remain unchanged.
- [x] Round I verification: fmt, all-target/all-feature clippy, **430 tests**
  (226 unit + 204 integration), release build, C/C++/Python smoke and ASan/UBSan.
- [ ] Subjective cinematic taste, extended human playtesting and terminals beyond
  the local Linux/VT100 evidence remain **PARTIALLY TESTED / UNVERIFIED**.
- [ ] Go remains **UNVERIFIED**: no toolchain installed.
- [ ] Remote CI remains **BLOCKED / ENVIRONMENTAL**: live check confirms billing
  rejection with zero executed steps. No automatic merge.

See [the evidence and command record](docs/acid-vs-crash-validation.md).

## Scene Algebra & Story Director Round — IMPLEMENTED (EXPERIMENTAL) + TESTED

This round stops hand-wiring visual effects to application state by introducing a
small deterministic Scene Algebra and Story Director.

- [x] **Bounded 2D line clipping** (Liang–Barsky) before Bresenham, in both
      canvases: huge finite / near-`i32`-extreme endpoints can no longer cause
      magnitude-proportional walks or overflow. Hostile clipping regression tests.
- [x] **Honest damage metrics**: `exact_changed_cell_count` (true semantic delta)
      alongside `affected_cell_count` (cells *addressed* by update semantics, may
      exceed the live area when rows are removed). Docs corrected.
- [x] **Projector parameter validation**: `Projector::new`/`is_valid` reject
      pathological parameters; no NaN reaches raster math.
- [x] **Scene Algebra** (`src/scene.rs`): `Scene`/`SceneEntity`/`SceneId`/`TagId`,
      `Effect` with `identity`, `sequence` (composition) and `parallel` (monoidal
      product), `Presentation`, `EffectBundle`, and the `Render : SCENE → UI`
      functor (`Scene::to_node`). Effects write presentation channels; they never
      mutate `Node`s.
- [x] **Story Director** (`src/story.rs`): `Facts`, `StoryEvent`, `Beat`,
      `Condition`, `StoryAction`, `StoryDirector`, `StoryTrace` + deterministic
      `Story::replay`. Branch/rejoin, mounted bundles, semantic (persistent) facts.
      Hardened in the merge-gate pass: **at most one transition per `update`**,
      **exact `(dt, events)` step replay**, duplicate beat/bundle rejection, and
      `Story::validate` reference checking. Scene entity labels are unique
      (`Scene::add` panics / `Scene::try_add` errors).
- [x] **Real replication effect** (`src/replication.rs`): bounded deterministic
      branching graph with freeze/neutralize; rabbit/cookie in the demo are a real
      entity, not a generic particle burst.
- [x] **`polished_agent` migration**: story-directed beats, permission *before*
      mutation, truthful approve/reject/cancel outcomes, focus actually routes
      input, overlays animated through Scene effects, `--stage=` fast starts.
- [x] **`hack_the_gibson` partial migration**: tactical branch via `StoryDirector`,
      Plague as persistent semantic state, rabbit/cookie real effect, shell
      commands mutate semantic scene state. The act timeline remains `Phase`-based.
- [x] **Docs truth pass** incl. a new DESIGN §37.

Integration tests: `tests/scene_algebra.rs` (functor through the renderer, bounded
damage, associativity, branch/rejoin, replay). Merge-gate hardening included the
one-arrow law, exact step replay and semantic-identity validation. Total:
**362 tests**.

---

## Phase 1: Core Engine & Minimal Vertical Slice — IMPLEMENTED + TESTED

- [x] Clean-room cell framebuffer architecture
- [x] Compact, stack-allocated grapheme cluster model (`CompactString`)
- [x] Unicode width measurement and wide glyph overwrite protection
- [x] Taffy Flexbox integration with custom text measurement
- [x] Declarative UI node tree (Box, Row, Column, Text, RichText, Border, Spinner, TextInput)
- [x] Differential diff engine with run coalescing and erase-to-end-of-line (`CSI K`)
- [x] Stateful minimal ANSI escape sequence compiler
- [x] Flagship Inline Mode with immutable-scrollback commit semantics
- [x] Fullscreen alternate-buffer mode reusing the same rendering pipeline
- [x] Frame scheduler with frame-budget throttling and telemetry metrics
- [x] Grapheme-aware text input component with navigation and bracketed paste
- [x] RAII terminal lifecycle guard and global panic hook restoration
- [x] Non-interactive / CI redirection detection and plain text degradation
- [x] Stable `extern "C"` ABI with opaque handles and panic containment
- [x] Real PTY integration testing with `portable-pty` (`pty_integration`, `pty_resize_torture`)

---

## Phase 1.5: Agent-Class Typography & Interactive Primitives — IMPLEMENTED + TESTED

- [x] **Chrome Primitives**: `Node::rule` (horizontal divider with optional title) and `Node::rail` (left-border callout).
- [x] **Typography & Hierarchy System**: `Span`, `Line`, `RichText`, and semantic `Theme::styles()` roles with zero raw SGR escape codes in components.
- [x] **Asynchronous Scrollback Insertion (`insert_before_live`)**: two named strategies — `InsertLineFastPath` (uses `CSI L`; no live repaint) and `RepaintFallback` (always correct; repaints and restores cursor). Proven directly in `whole_renderer_vt100`.
- [x] **Display-Width-Aware TextInput**: horizontal scrolling, wide CJK, and emoji display-width calculations.
- [x] **Right-Margin Autowrap Protection**: DECAWM `\x1b[?7l` disabling and right-boundary wide glyph clipping.
- [x] **Responsive Layout Sizing**: percentage width (`percent_width`), min/max dimensions, and per-side padding.
- [x] **Virtual Terminal Screen State Verification**: `vt100` parser automated tests verifying exact screen grids and cursor positions (`screen_state_vt100`, `whole_renderer_vt100`).
- [x] **Rapid Resize & Narrow Terminal Torture Tests**: bounded inline heights and valid cursor coordinates across cyclic resizing down to narrow widths. The **re-anchor path is PARTIALLY TESTED** — erase-from-cursor-down recovery is exercised, but exact absolute recovery is not (DSR unimplemented).
- [x] **Flagship `polished_agent` CLI Demo**: restrained, typography-driven agent interface with rule header, rail callouts, stable-height live plan, permission selector, persistent Unicode input, and zero-escape non-TTY redirection. `hack_the_gibson` is the maximalist twin with zero raw ANSI literals.

---

## Phase 1.75: Documentation Truth & Engine Hardening Round — IMPLEMENTED + TESTED

This round converted earlier over-strong claims into verified behavior and explicit gaps.

- [x] **Atomic `TerminalTransaction`**: one `write_all` + `flush` batching synchronized-update begin/end (`CSI ?2026 h/l`), private modes, cursor motion, diff bytes, final cursor placement, and visibility. Sync-update ownership removed from `AnsiCompiler`.
- [x] **Explicit Physical Anchor**: `AnchorState::{Invalid, Stable{cols,rows,live_height}}` plus last cursor position/visibility. Empty cell diffs still emit when cursor state changes. Geometry changes invalidate the anchor, discard the diff baseline, re-anchor, and increment `anchor_resyncs`. Tested in `whole_renderer_vt100` and the PTY resize probe.
- [x] **Two Insertion Strategies**: `InsertLineFastPath` and `RepaintFallback`, reported through `fast_insertions` / `insertion_repaints`. Universal zero-repaint is explicitly **not** claimed.
- [x] **Unicode Mutation Invariant**: `TextInputState` uses byte-range edits and re-derives the cursor grapheme from the complete resulting string. `cursor_grapheme <= grapheme_count` holds across combining accents, ZWJ emoji, skin-tone modifiers, and flags. Deterministic randomized edit fuzzing included. Single-line paste normalizes `\r\n`, `\r`, `\n` to a space.
- [x] **Structured Static Output**: `commit_text` / `commit_rich_text` / `commit_node` share the width-aware layout engine; control characters are neutralized at the cell model boundary; `commit_raw_ansi_unchecked` is the escape hatch; the ANSI stripper is documented as **not** a sanitizer.
- [x] **Language-Neutral Rich Text ABI**: opaque `gibson_line_t` / `gibson_rich_text_t` with span/align builders. **C, C++, Python: IMPLEMENTED + TESTED. Go: UNVERIFIED.**
- [x] **Metrics / Byte Accounting**: `frame_bytes`, `commit_bytes`, `insertion_bytes`, `control_bytes`, `total_terminal_bytes()`, plus `fast_insertions` / `insertion_repaints` / `anchor_resyncs`. `bytes_emitted()` deprecated in favor of `frame_bytes`.
- [x] **Scheduler / Runtime Model**: `DEFAULT_ANIMATION_INTERVAL = 80ms` and `Context::run_once(max_wait)` with input priority. Demos use `run_once` instead of `render() + sleep()`.
- [x] **Versioned C ABI**: `GIBSON_ABI_VERSION = 1`, `gibson_abi_version()`, `gibson_stats_init()`, dedicated `#[repr(C)]` `gibson_stats_t` with `struct_size` + `abi_version` + 14 `u64` fields. `gibson_get_stats` validates the version and refuses an undersized buffer (fixing a real 32-byte overflow). Enum-like inputs cross as validated raw `int32`.
- [x] **Hostile-Input Test Coverage**: invalid mode/border/color/wrap, null pointers, malformed UTF-8, wrong ABI version, undersized stats buffer, and non-finite layout floats.
- [x] **Verification Toolchain**: 100 tests (49 unit + 51 integration), clean `clippy -D warnings`, clean `fmt --check`, successful release build, and C/C++ examples clean under ASan + UBSan (LeakSanitizer disabled). Python `ctypes` example runs.

### Known gaps carried out of this round

- Go bindings **UNVERIFIED** (no toolchain).
- Windows / ConPTY **UNVERIFIED**; only Linux x86_64 exercised.
- tmux / screen / SSH matrix **UNVERIFIED**.
- Terminal capability negotiation **NOT implemented**; the `CSI L` fast-insertion assumption is not probed.
- Absolute cursor query (DSR) **NOT implemented**.
- Hard `SIGKILL` cannot be intercepted.
- Ctrl-C is a raw-mode key event in demos; no general signal-handling guarantee.
- No `cargo-fuzz` / AFL target yet.

---

## Phase 1.85: Compositor, Sub-cell Canvas & Determinism — IMPLEMENTED + TESTED

- **Safety split**: safe `insert_text_before_live` / `insert_rich_text_before_live` / `insert_node_before_live`; raw is explicitly `insert_raw_lines_before_live_unchecked`. `Renderer::commit` is now safe text (`commit_text`), fixing the old name-inverts-safety bug. (`tests/safety_api.rs`)
- **Exact wire metrics**: `TerminalTransaction::commit` returns the exact byte count including the synchronized-update terminator; renderer metrics no longer undercount. (`src/transaction.rs`)
- **Layer compositor**: `Node::stack()` + explicit `Cell::transparent` + `Node::dim()` style-only veil; overlay removal leaves no ghosts; floating modals do not reflow. (`tests/compositor.rs`, `whole_renderer_vt100`)
- **Sub-cell canvases**: `BrailleCanvas`, `HalfBlockCanvas`, `braille_oscilloscope`; exact glyph/colour tests; bounded update cost. (`src/canvas.rs`, `tests/effects_perf.rs`)
- **Deterministic clock**: `TimeSource`/`FixedStepClock` + motion helpers; scripted frames are reproducible. (`src/clock.rs`)
- **Capability + color ladder**: `TerminalCapabilities`, tri-state `Capability`, `ColorDepth`, central quantization; the `CSI L` fast path requires explicit support. (`src/capability.rs`, `tests/capability_fallback.rs`)
- **Visual goldens**: deterministic PTY→vt100 screen snapshots under `tests/goldens/` with explicit regeneration. (`tests/visual_goldens.rs`)
- **Demos**: `polished_agent` gains a floating permission modal, dim veil, toast overlay and a Braille telemetry scope; `hack_the_gibson` gains a Braille virus-scan instrument and a CRT scanline overlay.
- **CI status**: the workflow now also runs the visual goldens and capability tests. Remote GitHub Actions is currently **blocked by account billing** (the job is refused before any step runs: "recent account payments have failed or your spending limit needs to be increased"). This is environmental, not a code failure; local verification is green.

## Phase 1.9: Scene/Effects Substrate — IMPLEMENTED + TESTED

- **Positioned layers** (`Node::offset`) with clip-aware leading-edge rendering. (`tests/scene.rs`)
- **Camera viewports** (`Node::viewport`, `ViewportState`). (`tests/scene.rs`)
- **Raster embedding** (`Node::raster`/`Node::surface`, `blit_transparent_clipped`). (Rust-only, no C ABI yet.)
- **Vector primitives** on `BrailleCanvas` (rect/circle/ellipse/polygon).
- **3D wireframe projector** (`geom`: Vec3/Transform3/Mesh/Projector, near-plane clip). (`src/geom.rs`)
- **Particles** (seeded deterministic system). (`src/particles.rs`)
- **Procedural fields** (plasma/interference/heat). (`src/field.rs`)
- **Text transitions + safe glitch** (grapheme-safe; content-only). (`src/transition.rs`, `src/glitch.rs`)
- **Damage inspection** (`SurfaceDiff::dirty_cells`, `Context::last_dirty_cells`) powering live damage maps.
- **Demos**: `hack_the_gibson` rotating wireframe/plasma/particles/glitch/damage map; `polished_agent` modal shadow + transcript camera; **new `examples/fx_lab.rs`** gallery.
- **Tests**: bounded effect damage, resize-during-wireframe, deterministic goldens incl. `fx_lab`. 233 tests total.
- **Remote CI remains BLOCKED / ENVIRONMENTAL** (GitHub account billing); local verification green.

## Phase 1.10: Correctness Hardening + Interactive Demos — IMPLEMENTED + TESTED

- **Near-plane clipping made consistent**: inclusive `depth >= near` predicate shared by `project`/`clip_near`; crossings no longer disappear. Decisive tests. (`src/geom.rs`)
- **Wide-glyph clip containment**: clipping wins; no continuation leaks past any clip rectangle. Regression tests for raster nodes, viewports and positioned layers. (`src/surface.rs`, `tests/scene.rs`)
- **Logical damage model**: runs ∪ erase-to-EOL ∪ cleared rows, separate from wire cost. (`src/diff.rs`, `src/renderer.rs`, `tests/effects_perf.rs`)
- **TextInput cursor policy**: the placeholder stays intact; the software cursor highlights an existing glyph rather than blanking it. (`src/painter.rs`)
- **Particle API truth**: radial vs life-variance vs directional bursts. (`src/particles.rs`)
- **Mono field ordered dithering** (4×4 Bayer). (`src/field.rs`)
- **Minimal focus ring** + `PageUp`/`PageDown` keys. (`src/focus.rs`, `src/input.rs`)
- **Demos**: `polished_agent` is **inline by default** (real scrollback + mutable live foreground), with a richer plan (failure/recovery), scrollable code viewport and a completion summary; `hack_the_gibson` is an act-based *Hackers* (1995) homage (Gibson data city, Plague, Da Vinci, broadcast, Grand Central attack, rooftop pool, CRASH AND BURN) with a real root shell. `fx_lab` adds regression scenes and `--debug-damage`.
- **Tests**: 291 total (was 233), incl. interactive PTY tests (`tests/pty_demos.rs`) and expanded visual goldens.
- **Remote CI remains BLOCKED / ENVIRONMENTAL** (GitHub account billing); local verification green.

## Phase 2: Input Protocols & Interaction Enhancements — PARTIAL

- [x] **Minimal focus ring** (`FocusId`/`FocusRing`, Tab/Shift-Tab, modal capture/restore) — implemented; a full hierarchical focus tree and event bubbling remain planned.

- [ ] **Kitty Keyboard Protocol**: progressive enhancement for disambiguated escape keys, key release events, and modifier combinations.
- [ ] **Focus Management Tree**: hierarchical focus tree with Tab / Shift-Tab cycling and focus restoration.
- [ ] **Event Bubbling and Capture**: structured event dispatch pipeline allowing parent containers to intercept or bubble user events.
- [ ] **Mouse Tracking**: optional SGR mouse reporting (`CSI ? 1006 h`) for click-to-focus, scroll wheel handling, and selection.
- [ ] **OSC 8 Terminal Hyperlinks**: embedded clickable URLs in Text nodes with fallback plain-text formatting.

---

## Phase 3: Advanced Layout & Rich Components — PLANNED

- [ ] **Incremental Layout Caching**: cache Taffy layout subtrees across frames when node contents are unmodified.
- [ ] **Viewport Scrollbar Widgets**: `Node::viewport` and `ViewportState` already provide tested clipping/pan/page state; integrated scrollbar widgets and virtualized containers remain planned.
- [ ] **Virtualization Engine**: virtual list and table rendering supporting very large datasets without memory pressure.
- [ ] **Rich Component Library**:
  - Tables with auto-sizing columns and alignment.
  - Progress bars with smooth Unicode fraction characters (`▏▎▍▌▋▊▉█`).
  - Tree views with expandable/collapsible nodes.
  - Tab headers and segmented controls.
- [ ] **Markdown and Syntax Highlighting**: streaming Markdown parser with Syntect or Tree-sitter token colorization.

---

## Phase 4: Styling, Themes, and Accessibility — PLANNED

- [ ] **Extensible Theme Definitions**: `Theme` and TrueColor/ANSI256/ANSI16/Mono fallback already exist and are tested; a configurable theme schema remains planned. No CSS system is required.
- [ ] **Terminal Capability Negotiation**: automatic detection via Primary and Secondary Device Attributes (`CSI c`, `CSI > c`) for synchronized output, color depth, and graphics protocols. This also lets the engine choose the insertion strategy from measured capability rather than assumption.
- [ ] **DSR Absolute Anchoring**: query the cursor position (`CSI 6 n`) to re-anchor exactly after resize/reflow instead of the current best-effort relative rebuild.
- [ ] **Accessibility (A11y)**: screen reader annotations, semantic headings, and ARIA-like terminal roles for assistive technology.

---

## Phase 5: Protocol Extensions & Hardening — PLANNED

- [ ] **Terminal Graphics Protocols**: inline image rendering via Kitty graphics protocol, iTerm2 inline images, and Sixel graphics.
- [ ] **Windows ConPTY Torture Testing**: extended automated testing under the Windows Console API and ConPTY.
- [ ] **Multiplexer & Remote Shell Hardening**: specialized test matrix for tmux, screen, and SSH connections over high-latency networks.
- [ ] **Property & Fuzz Testing**: `cargo-fuzz` / AFL suite exercising arbitrary Unicode sequences, arbitrary terminal byte streams, invalid ANSI input, and rapid terminal resizes.
- [x] **Executable Foreign Binding CI**: C/C++/Python/ASan and Go jobs are prepared with corrected native paths, fresh native builds, least privilege and timeouts. Local Go smoke passes. All five public-main jobs now pass; see the state audit for the exact run and remaining coverage limits.


---

## Research pressure test — Agent-native dynamic interfaces (NON-BINDING)

This docs-only, **NON-BINDING / PRESSURE TEST ONLY / NO NEW CORE COMMITMENT**
dossier records three external pressure tests for the public API. It preserves
original research tip `bc4d1ce748ffa67121104ac39d960ba4a82736d3` and refreshes its
baseline to the green PR #14 merge, `e5ede0a4ab8ff52caa567ee10d45824e42c1ebc6`:

- manga/comic sequential-art presentation grammar;
- semantic meme reaction layer;
- agent-generated temporary interactive instruments.

These are **not planned LibGibson products** and should not be implemented as core widget families. They exist to test a broader architectural claim: common professional dynamic graphics should be easy, while unanticipated visual ideas should remain possible through safe public lower layers (`Node`, `Scene`, `Surface`, `RgbRaster`, generic geometry/effects) rather than renderer forks or raw ANSI.

Core promotion is earned only by repeated generic pressure, a correctness boundary, or a missing minimal primitive. Harness-specific vocabulary stays external.

The flagship's ordinary Node mini-UIs mounted on projected city facades are
**CORROBORATING EVIDENCE** for rendering composition without building-specific
core widgets. They do not solve the predicted **G1 stable interaction identity /
event routing** pressure for arbitrary runtime-created objects. Pin the initial
external lab to the merged baseline, record first-contact friction, reconcile it,
and promote nothing by default. The final API/helper freeze precedes fresh
holdout selection; examples already named in the dossier are ineligible holdouts.

Research dossier:

- [Agent-native dynamic UI research program](docs/research/AGENT_NATIVE_DYNAMIC_UI_RESEARCH_PROGRAM_2026-09-24.md)
- [LibGibson expressivity/API audit](docs/research/LIBGIBSON_EXPRESSIVITY_API_AUDIT_2026-09-24.md)
- [Experiment and acceptance plan](docs/research/AGENT_NATIVE_UI_EXPERIMENT_PLAN_2026-09-24.md)
- [Zero-context handoff](docs/research/AGENT_NATIVE_UI_ZERO_CONTEXT_HANDOFF_2026-09-24.md)

This first campaign is now **COMPLETE within its recorded scope**: the three
planned consumers and Foldroom/Cuebox/Weavebench holdouts used an unchanged frozen
public API. No core promotion was justified. See the [results and claim limits](docs/research/AGENT_NATIVE_UI_CAMPAIGN_RESULTS_2026-09-24.md).
The original plan above remains provenance, not an unstarted feature promise.
Failed consumer attempts and selection exclusions remain visible; broader ease,
human usability and universality remain unverified.
D6/D7/D8 and issue #15 remain separate engineering debt, not scope for this campaign.
