# State of LibGibson

## Executive summary

This is the canonical bird's-eye assessment, including the introductory-cinema consolidation,
not a replacement for [DESIGN](../DESIGN.md). Historical evidence is catalogued
in the [validation index](VALIDATION_INDEX.md).

**Classification: ENGINEERING ALPHA, with a well-tested Linux rendering core
and experimental Rust composition/graphics layers.** “Core” describes architectural
responsibility and relative maturity; it does not promise a frozen Rust API or
universal terminal compatibility. There is no basis yet for “release-candidate
platform,” universal 60 FPS, or modern Rust/C feature parity.

The approved tip `3b67ca23ee6988cc450845977fe2daf017ae6747` was merged through
[PR #1](https://github.com/femboy2112/libgibson/pull/1). Main at the initial audit was
`0b673ccfa50492111352c969d58ebe3cfadfcac1`. Ancestry verification succeeded.
The audit branch, `codex/post-merge-state-audit`, began at that exact main SHA
with a clean worktree. Its first commit (`7bc4f9c`) contains documentation/comment
changes only. A separately authorized public-readiness commit adds repository,
license, CI and Go build hygiene; no core/cinematic behavior is changed. The
publication evidence below supersedes initial D3/D4/license-packaging status.

The post-merge/public-readiness checkpoint passed **536 tests = 226 unit + 310
integration**. The merged introductory cinema passes **574 tests = 226 unit + 348 integration**
and adds Rect/diff, PTY lifecycle, route and cinematic regressions; see [its current validation](INTRODUCTORY_CINEMA.md).
Historical command tables below preserve the checkpoint they measured.

PR #14 merged at `e5ede0a4ab8ff52caa567ee10d45824e42c1ebc6`, with all five
exact-main jobs passing in [run 35960000454](https://github.com/femboy2112/libgibson/actions/runs/35960000454).
The merge fixes the discovered hostile `Rect` arithmetic,
exact changed-coordinate enumeration, and Unix PTY harness cleanup/deadline weaknesses.
D6 API/distribution, D7 general resources and D8 embedding contracts remain open.
The resize investigation also records a queued-input/backpressure follow-up in
[issue #15](https://github.com/femboy2112/libgibson/issues/15).
The native CI loader and Go setup defects were fixed and exercised locally during
public readiness; hosted-runner proof is recorded in the public CI evidence below. Passing the expanded
suite does not erase the remaining findings. See the reproducible probes below.

The subsequent [external agent-native UI campaign](research/AGENT_NATIVE_UI_CAMPAIGN_RESULTS_2026-09-24.md)
built three planned consumers and three post-freeze holdouts on an unchanged
public revision. The separate lab passes 61 tests. G1 interaction identity/routing
showed ergonomic friction with a working public route; no core promotion was
justified. This corroborates bounded expressivity, not arbitrary-UI universality,
comparative ease, or a stable API. Its schema and domain tools remain external.

## Event delivery evidence

The event-pressure branch passes **591 tests = 226 unit + 365 integration**,
with one explicitly ignored, demonstrably failing collision-delivery acceptance.
The [Event Pressure Lab](EVENT_PRESSURE_LAB.md) investigates #15 without changing
core scheduling or dependencies. Its separate child PTY, monotonic receipt channel,
raw/Crossterm/Context controls, bounded captures and visual replay locate a
readiness-batch consumption defect upstream. Backpressure is one way to create the
polling gap; the standalone quiet reproducer also fails. An additional Context
write delay is a separate synchronous-output contract, not evidence of byte loss.

The supported repair remains **OPEN**. Ordinary diagnostic tests can pass while
the explicitly ignored no-second-key collision acceptance fails; green CI is not
claimed as a fix. #10 long-session retention and #11 embedding ownership remain
open. This work adds example/test diagnostics, not a public tracing framework.

## What LibGibson is

**A cell-framebuffer terminal UI engine with native scrollback/live-region
semantics and experimental compositional animation and software graphics.**

That is sufficient; a new brand or a second engine is unnecessary. The distinctive
core is the boundary between committed terminal history and mutable live content,
with one layout/paint/diff/output model for text, UI and generated graphics.
Rust developers can build inline agent tools, fullscreen consoles, clipped
compositions, deterministic animation, and terminal-native graphical stories.
Foreign-language users have a useful UI/output subset, not the whole visual stack.

The optional frontier adds meaning and presentation above that core: Scene
identifies UI objects; Story sequences semantic events; software graphics produce
ordinary cell surfaces. Neither a narrative model nor a cinematic camera is
required for a minimal CLI.

## Architectural stack

```text
Application state / input dispatch
  ├─ optional StoryDirector: facts, beats, reactions, exact update trace
  │    └─ mounted EffectBundles → Presentation
  ├─ optional Scene: entities + tags + time/effects → ordinary Node tree
  └─ direct ordinary Node tree
                  ↓
        Taffy layout → computed cell rectangles
                  ↓
                Painter
  ordinary subtree → [optional local SurfaceFx] → compositing
                  ↓
             next Surface
                  ↓
       Diff(previous, next Surface)
                  ↓
       AnsiCompiler: runs, styles, cursor motion
                  ↓
       TerminalTransaction: modes + bytes + cursor, write_all + flush
                  ↓
               terminal
```

Parallel producers feed that same painter/compositor: BrailleCanvas,
HalfBlockCanvas, RGB raster, filled 3D, fields, particles and text transitions.
`RasterFx` runs on RGB before cell realization; `SurfaceFx` runs on cells after
subtree painting. They have different semantics and should remain distinct.

Runtime responsibilities, verified in source:

| Owner | Responsibility and boundary |
| --- | --- |
| [Context](../src/context.rs) | Root Node, scheduler, session, renderer; polls input and writes through stdout. Application owns semantic dispatch. |
| [TerminalSession](../src/session.rs) | TTY/headless geometry, passive capability snapshot, raw/alternate/paste modes, best-effort restoration and global panic hook. |
| [Renderer](../src/renderer.rs) | Layout/paint invocation, previous Surface, physical anchor/cursor, inline/fullscreen policy, commits and history insertion. |
| [Scheduler](../src/scheduler.rs) | Dirty scheduling, cadence ceiling and operational stats; not an animation clock owned by Nodes. |
| [Layout](../src/layout.rs) / [Painter](../src/painter.rs) | Taffy tree is rebuilt per layout; Node gets computed rectangles. Painter realizes cells and requested cursor. |
| [Surface](../src/surface.rs) / [Cell](../src/cell.rs) | Graphemes, wide continuations, style/transparent overlays, clipping and ordinary storage. |
| [Diff](../src/diff.rs) / [ANSI](../src/ansi.rs) | Semantic comparison and terminal patch compilation; compiler does not own synchronized-update lifecycle. |
| [Transaction](../src/transaction.rs) | One buffered write_all/flush per transaction, including synchronized-update markers. This is batching, not a kernel-level atomicity guarantee. |
| [Input](../src/input.rs), [Focus](../src/focus.rs), [Viewport](../src/viewport.rs) | Key/paste/resize, grapheme editing, focus identity and scroll offsets. No DOM or built-in bubbling. |

Load-bearing boundaries are clean: semantic updates stay upstream; Nodes do not
own story time; generated graphics become Surface content; neither Scene nor
raster3d emits terminal protocol. Removing entity post-processing reveals ordinary
underlying rendering. Post-processing deliberately suppresses that subtree's
hardware cursor when effects could move or erase its insertion point.

Abstraction leaks worth managing: public mutable Context/Renderer/Surface fields
allow callers to violate internal assumptions; Context and session lifecycle assume
process terminal/stdout ownership; the panic hook is global; Scene evaluation
clones Nodes rather than providing a retained incremental layout engine. These
are product-contract questions, not reasons to rewrite the pipeline.

## Core capability matrix

“IMPLEMENTED + TESTED” below means local Linux automated evidence for the named
contract. It does not include every hostile input or every emulator.

| Capability | Status | Evidence / limit |
| --- | --- | --- |
| Primary-screen inline mode; fullscreen | IMPLEMENTED + TESTED | Renderer, whole_renderer_vt100, commit_invariance, PTYs. |
| Lifecycle, cursor, resize/re-anchor | IMPLEMENTED + TESTED; PARTIALLY TESTED across environments | Drop/panic cleanup and PTY input/resize; relative inline anchor, no DSR. Restore errors are best effort. |
| Synchronized updates | IMPLEMENTED + PARTIALLY TESTED | Transaction markers tested; user toggle + TTY enables them, not negotiated support. |
| Differential rendering | IMPLEMENTED + TESTED | Count, exact coordinate spans, affected footprint and bytes are distinct; D2 regression now covered. |
| Scrollback insertion | IMPLEMENTED + TESTED | CSI L fast path or repaint fallback; not universally zero repaint. Passive capability assumptions remain. |
| Structured output / non-TTY | IMPLEMENTED + TESTED | Text/RichText/Node share layout; redirected output is plain text. |
| Raw ANSI escape hatch | IMPLEMENTED + TESTED, caller responsibility | Explicit unchecked paths bypass sanitization; legacy alias noted below. |
| Graphemes / wide glyphs / Unicode input | IMPLEMENTED + TESTED | Cell continuations, clipping, boundary-merging input, paste and width-aware cursor tests. Emulator width policies may differ. |
| RichText, wrapping, alignment, themes | IMPLEMENTED + TESTED | Cell/layout/structured_output tests. Not a general Markdown or CSS engine. |
| TrueColor / ANSI256 / ANSI16 / Mono | IMPLEMENTED + TESTED | Central quantization and injected capabilities; passive environment inference only. |
| Node tree, flex, stack/overlay, offsets | IMPLEMENTED + TESTED | Taffy, compositor and scene tests; not incremental layout or virtualized lists. |
| Viewport / focus / TextInput | IMPLEMENTED + TESTED | Clipping/pan state, small focus ring, single-line editing. Application dispatches events. |
| Mouse / hierarchical routing | PLANNED | Mouse/focus events are not exposed as a routing system; no capture/bubble tree. |
| Active capability negotiation / DSR anchoring | PLANNED | Capability fields are not implementations of the named protocols. |

## Experimental capabilities and public API

| Layer | Status | Actual contract |
| --- | --- | --- |
| SceneEntity, IDs/tags and targeting | EXPERIMENTAL + TESTED, Rust-only | Identity around ordinary Node, deterministic targeting/z-order. |
| Effects, sequence/parallel, displacement, loops | EXPERIMENTAL + TESTED, Rust-only | Placement writes ordered; displacements add; finite/loop time deterministic. Independent channels can commute, arbitrary FX chains do not. |
| Entity post-process / spatial masks | EXPERIMENTAL + TESTED, Rust-only | Transparent scratch only when active; ordered SurfaceFx, glyph-safe rect/wipe/radial/band/noise scopes. |
| StoryDirector / reactions / replay | EXPERIMENTAL + TESTED, Rust-only | In-beat actions plus at most one transition per update; exact ordered dt/event replay. Direct facts_mut/jump_to edits are outside trace replay. |
| Braille / HalfBlock canvases | IMPLEMENTED + TESTED, optional Rust graphics | 2×4 binary dots or 1×2 RGB samples per cell; ordinary Surface output. |
| RgbRaster | EXPERIMENTAL + TESTED, Rust-only | Opaque RGB, stored-channel blending, dimensions capped at 2048 per pixel axis; half-block or Mono density realization. |
| Filled triangles / camera / Z buffer | EXPERIMENTAL + TESTED, Rust-only | Six-plane clipping, reciprocal depth, deterministic overlap, Lambert + ambient/emissive + fog; no mesh loader or general renderer backend. |
| RasterFx | EXPERIMENTAL + TESTED, Rust-only | Ordered glow, split, warp, vignette, scanline; reusable workspace exists. |
| FeedbackBuffer | EXPERIMENTAL + TESTED, Rust-only | Explicit dt-driven energy history, reset/resize; equivalent clamped size preserves history. No terminal alpha. |
| Fields / particles / wireframes / glitch / text transitions | IMPLEMENTED + TESTED helpers; experimental scope | Deterministic utilities feeding cells; older helpers do not all share newer raster hostile-input bounds. |
| Replication | IMPLEMENTED + TESTED optional effect helper | Bounded caller-configured branching; public module still has demo-shaped rabbit/cookie vocabulary. |

All modules are publicly exported in [lib.rs](../src/lib.rs); many Scene/Story
symbols are re-exported at crate root. No feature gates separate maturity levels.
Keep Scene, Story, SurfaceFx and RGB/3D/RasterFx **public + experimental**. Tests
justify use and further iteration, not a stable ABI promise. Feature flags may
later separate support tiers or build cost after measurement; they do not solve
API stability by themselves, and these modules have no large graphics dependency.

Overlapping APIs mostly serve distinct levels:

- HalfBlockCanvas is a cell-sized convenience canvas; RgbRaster is bounded opaque
  pixel storage with blending. `Node::surface(Arc<Surface>)` shares storage;
  `Node::raster(Surface)` takes ownership. Neither performs graphical resampling.
- `geom::Projector` is a small wireframe projection helper; `raster3d::Camera`
  supports full frustum/depth geometry. Both reuse vector math.
- `glitch` exposes direct cell mutation; SurfaceFx adds ordered specs and scopes.
  Text dissolve transforms graphemes; Surface dissolve masks realized cells.
- `field` has older canvas-oriented functions; raster_fx contains RGB-oriented
  field helpers. Document their input domains before attempting consolidation.
- `commit`, old dirty-count names and `termframe` aliases preserve compatibility.
  Recommend modern explicit names in new code; do not remove them casually.

Crate documentation now supplies an entrypoint and alpha/ownership boundaries.
API documentation still needs a tested support/stability policy,
FFI ownership contracts and a review of public mutable implementation fields.
Privatization, renaming and feature flags are recommendations, not this audit's edits.

## Demo-local technology and demo strategy

BattleGraph, continuous influence, AcidPlanner memory/scoring, CrashController,
EncounterTrace, VisualHistory/ShotHistory, VisualDirector, camera choreography,
subsystem meshes and final formations belong in `examples/acid_vs_crash/`.
They are a domain model and its projections, not core library concepts.

| Demo | Job | Positioning |
| --- | --- | --- |
| libgibson_intro | Professional Node harness → refracted Surface → information city → Earth | Canonical introductory short film; finite simulated jobs and seekable pure presentation. |
| polished_agent | Practical inline UI, native history, mutable foreground, focus/viewport | Product-oriented example; good next step after Quickstart. |
| hack_the_gibson | Broad act/effect showcase and fictional Hackers homage | Showcase, not minimal API tutorial; shell is simulated. |
| fx_lab | Isolated primitive gallery and regression microscope | Engineering lab; three example tests require explicit invocation. |
| acid_vs_crash | Integrated deterministic model → shots → graphics/UI → terminal | Interactive cinematic battle; default Crash fights, manual interventions remain. |
| agent_chat | Earlier lifecycle/chat example | Legacy example/fixture; manual timing and raw output make it a weaker starting point. |
| perf_probe / resize_test_app | Damage/byte and resize exercise | Probe/fixture, not universal performance certification. |

Legacy flat/cyber projections intentionally preserve comparison and goldens.
Do not delete them as dead code. The intro demonstrates another realization grammar,
without replacing the existing demos or changing Acid battle semantics.

## Language-neutral ABI status

[include/gibson.h](../include/gibson.h) exposes ABI version 1: opaque context,
Node, Line and RichText handles; inline/fullscreen rendering and scheduling;
key/paste/resize input; styled row/column/stack/dim/text/spinner/input/border/rule/rail;
a subset of layout setters; safe commit/insertion; unchecked ANSI; errors/stats.
Stats require exact ABI-version equality and sufficient struct size; larger caller
buffers retain trailing bytes. This is not older-small-struct append compatibility.

C/C++/Python can build useful agent/chat/logging consoles now. They cannot directly
build the Rust cinematic flagship via this ABI: Scene/Story/replay, raster upload,
3D/RasterFx/feedback, viewport/focus state, positioned offsets and modern capability
setters are absent. C++ RAII and Python ctypes are locally smoke-tested; Go cgo
now passes local vet/build/example smoke. Its repo-local packaging is repaired;
there are no Go unit tests; public Go 1.27.1 build/example smoke now also passes.

FFI checks nulls, UTF-8 and raw enum-like integers and catches unwinding at
substantive entry points. It cannot validate arbitrary dangling pointers, lengths,
double frees or unterminated C strings. Ownership transfer (for example,
`gibson_set_root_node` consumes its Node) needs clearer header-wide documentation.
`tf_insert_before_live` is a legacy **raw** alias; its new comment clarifies this
without changing behavior. Do not confuse it with safe modern wrapper aliases.

Near-term priority is core parity, ownership and distributable bindings. Later,
a small versioned raster-upload API may be lower-risk than exporting the whole
cinematic graph. Full Scene ABI, command buffers and serialization each require
validation, lifetime, version and replay-format decisions; choose after Rust
contracts settle. No new ABI is implemented here.

## Test and validation status

Host: Linux x86_64, Ubuntu-based kernel `7.0.0-28-generic`.
Rust `1.97.0-nightly (a5c825cd8 2026-04-14)`; Cargo
`1.97.0-nightly (eb94155a9 2026-04-09)`. This is a local nightly result, not proof
of a minimum stable Rust version. The initial private workflow did not execute. Subsequent public stable-Rust evidence appears below.

At the initial state-audit checkpoint, executed before edits and again after housekeeping:

| Exact command | Baseline | Final |
| --- | --- | --- |
| `cargo fmt --check` | exit 0 | exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 | exit 0 |
| `cargo test` | exit 0; 536 passed | exit 0; 536 passed |
| `cargo build --release` | exit 0 | exit 0 |
| `cargo build --examples` | exit 0 | exit 0 |
| `bash scripts/dev/bindings_smoke.sh --asan` | exit 0 | exit 0 |

C, C++, Python and native ASan/UBSan smokes passed; existing script disables
LeakSanitizer. C/C++ sanitizer instrumentation is not whole-Rust ASan coverage.
At that checkpoint the Go executable was absent and the script allowed Go build
failures. Public readiness below adds local Go execution and makes a present but
failing Go toolchain fail the smoke script.

Additional commands: `cargo tree -d` completed; `cargo test --example fx_lab`
passed three tests. An initial `cargo doc --no-deps` succeeded with two broken
FocusRing method-link warnings; those comment links were corrected, and
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` then passed. `git diff --check` passed.
Command logs and probe artifacts are run-local under `/tmp/libgibson-post-merge/`;
that directory is not a portable repository dependency. Final branch SHA belongs
in the delivery report rather than self-referential document content.

Test taxonomy:

| Group | What it establishes | Boundary |
| --- | --- | --- |
| 226 source unit tests | Cells, layout, input, scheduling, geometry and semantics | Finite examples, not exhaustive public-input safety. |
| Original audit: 32 integration targets / 310 tests | Composition, diff, ABI, replay, graphics, PTY and fixtures | Some targets import demo modules, so this is also demo maintenance coverage. |
| whole_renderer_vt100 / screen_state_vt100 | Full renderer or diff/compiler bytes reconstruct expected screen | Shared vt100 parser family, not independent emulator consensus. |
| scene/story/acid model tests | One-arrow law, reactions, irregular dt replay, counterfactual choices, visual history/camera equality | Exact recorded input updates; no durable portable replay format. |
| raster3d / raster_fx / surface_fx | Occlusion, clipping, hostile floats, masks, wide cells, deterministic feedback | Newer raster safety does not cover every older helper. |
| PTY integration/demos/resize | Actual input, lifecycle, restoration, typed-input preservation and resize | Unix termios checks are conditional; executed only on Linux here. |
| 51 text goldens + raster probes | Terminal reconstruction plus semantic RGB/depth/diversity checks | Acid text goldens retain legacy flat projections; they do not certify cinematic art. |
| effects_perf / perf_probe | Damage locality, exact/affected/wire separation | Not sustained throughput benchmarks. |

The baseline `cargo test` command took about 92 seconds on this host; visual_goldens
reported about 37 seconds. These are observations, not timing assertions. Golden
captures use fixed wall-clock waits; semantic/raster tests use deterministic time.
Three fx_lab example tests sit outside the default count (536 at the initial audit,
574 on the introductory-cinema branch). Repeated checks at
model/presentation/terminal layers guard distinct boundaries; the former CI's repeated goldens
and PTY commands also duplicate some full-suite execution and can later be tuned.

Highest-value missing coverage is broader generative cell/diff and geometry invariants,
PTY coverage beyond Unix direct children, actual platform/mux runs, sustained graphics
memory/transport, and foreign binding parity. Use property testing first for small
in-process invariants; add cargo-fuzz for parser/FFI-adjacent valid harnesses and
clipping after a corpus and input contracts exist. Do not feed invalid foreign
pointers and call resulting UB a fuzzable safe API contract.

Existing tests reconfirm frozen-frame exact delta/affected/wire **0/0/0**, full
encounter/shot/camera/final-frame replay, and resize re-projection without semantic
advance. Frozen *cells alone* do not imply zero wire if physical cursor or anchor
state changed. The initial state audit did not repeat aesthetic acceptance or a full
manual playthrough. The [intro validation](INTRODUCTORY_CINEMA.md) adds a release PTY
playthrough and rendered-frame inspection; neither is independent human aesthetic acceptance.

## Performance model

Differential output saves terminal work; it does not mean incremental generation.
Layout rebuilds Taffy, painting creates a next Surface, and diff scans rows/cells.
Broad animated RGB scenes naturally change many cells and send many bytes.

| Path | Source-derived cost / allocation | Assessment |
| --- | --- | --- |
| Renderer/layout | New Taffy tree, next Surface, diff runs/byte buffers | Small terminal areas plausible; no retained-layout scaling claim. |
| Scene evaluate | Target vectors, baseline linear scans per entity, z sort, cloned Nodes including hidden | Worst-case baseline lookup O(E²); a concern for large scenes, not observed demo failure. |
| SurfaceFx scopes | Selection mask and partial-scope Surface copy | Local damage does not imply CPU proportional only to selected cells. |
| RGB field | O(pixel count × sources), seven-node demo world | Small fixed source count; straightforward bounded work. |
| raster3d | RGB + f64 depth allocation, triangle clipping vectors, pixel bounding boxes | Real depth; worst case includes overdraw. Not a GPU workload model. |
| RasterFx | Linear passes; glow radius ≤3, at most 49 kernel samples/pixel | Workspace can be reused; convenience apply_chain creates one. |
| Cinematic trails | Rebuild feedback/input and replay ≤48 history samples each paint | Strongest profiling candidate: O(history × pixels), chosen for deterministic reprojection. |
| Demo geometry | Rebuild Vec-backed meshes, rails and fragments each paint | Small bounded forms; allocation reuse merits measurement first. |
| Story trace | Every unfinished update retained, including empty events | Memory grows with duration; no recording opt-out/streaming retention policy. |
| Wire transport | Depends on changed glyphs/colors/cursor operations and terminal consumer | Remote/mux/backpressure may dominate generation. |

References: [Scene evaluation](../src/scene.rs), [graphics realization](../examples/acid_vs_crash/cyber.rs),
[world geometry](../examples/acid_vs_crash/world_geom.rs), [RasterFx](../src/raster_fx.rs).
At 160×40, a full 1×2 raster has 12,800 samples; 48 history passes address 614,400
sample-pixels before multiple per-pass operations. Normal demo dimensions are
far below the 2048² RGB ceiling. At that ceiling, feedback alone is about 108 MiB;
RGB plus f64 depth is about 44 MiB before extra scratch/Surface conversion. The
ceiling is a safety limit, not recommended operating size. Surface, meshes,
particles and application traces do not share a universal allocation cap.

Historical cinematic generator probes were single-digit milliseconds on the dev
host; those are not fresh sustained measurements, terminal FPS, or cheap-laptop
certification. Smaller viewports reduce pixel work without architectural change.
Running well on slow hardware is plausible but **UNVERIFIED**. Measure generation,
layout/paint, diff, compilation, allocation and wire/consumer latency separately
before reusing buffers or caching meshes. Preserve replay/resize purity when caching.

## Platform support and operational CI

| Environment | Status / evidence |
| --- | --- |
| Linux x86_64 | IMPLEMENTED + TESTED locally: Rust, C/C++/Python, PTY and vt100. |
| macOS | UNVERIFIED; no executed host matrix. Python loader currently searches .so paths. |
| Windows / ConPTY | UNVERIFIED; no job or host run. Key event-kind handling, lifecycle and loader paths need explicit tests. |
| tmux / screen / SSH | UNVERIFIED; passive environment assumptions and local PTY tests are insufficient. |
| Specific terminal emulator matrix | UNVERIFIED broadly; one parser and local PTY are not certification of every emulator/font. |
| Go | PARTIALLY TESTED: local gccgo 14.2 / Go 1.18 and public Go 1.27.1 vet/build/example smoke; no Go unit tests or platform matrix. |
| Active negotiation / DSR | PLANNED; color inference and relative anchors exist instead. |

Historical private merged-main [Actions run 35941989705](https://github.com/femboy2112/libgibson/actions/runs/35941989705)
failed with **zero Rust steps**; dependent jobs were skipped. Its annotation states
that recent payments failed or the spending limit needs increasing.
At that checkpoint **REMOTE CI: BLOCKED / ENVIRONMENTAL**, neither green nor a demonstrated source
failure. This was verified for the main SHA above, not copied from a prior dossier.
The later audit-tip [run 35942790824](https://github.com/femboy2112/libgibson/actions/runs/35942790824)
also executed zero steps. These billing records remain historical evidence. The
owner-authorized public launch and executed CI results below supersede that status.

## Known blockers and technical debt

D1/D2 and the bounded Unix direct-child portion of D5 are resolved on main
through PR #14, with regressions. Issues #7, #8 and #9 are closed. D3/D4 retain public-run proof; D6/D7/D8 remain open.

| ID | Finding | Evidence and required next check |
| --- | --- | --- |
| D1 | FIXED + LOCAL REGRESSIONS | Saturated half-open Rect endpoints, normalized intersection/shrink and overflow-safe amounts. Debug/release boundary oracle tests. Coordinate 65535 is outside the maximum Surface extent. |
| D2 | FIXED + LOCAL REGRESSIONS | Compact exact spans include erased/removed rows and columns plus wide continuations; an independent union-of-extents oracle checks count/enumeration agreement. Public Rust SurfaceDiff gains a field; exhaustive literals need updating. C ABI unchanged. |
| D3 | RESOLVED + PUBLIC CI VERIFIED | Former literal `$PWD` RUNPATH caused exit 127 (historical probe below). All four workflow C/C++/sanitizer commands now expand `$GITHUB_WORKSPACE`; readelf and execution without LD_LIBRARY_PATH pass; both public binding/sanitizer jobs also execute successfully. |
| D4 | Repository build RESOLVED + PUBLIC SMOKE; distribution remains limited | Correct module/import, normal command example, SRCDIR paths, per-job native build and hard-failing vet/test/build/example. Local gccgo succeeds. Hosted Go 1.27.1 vet/build/example also passes. No Go unit tests; standalone installation remains D6 debt. |
| D5 | FIXED for Unix direct-child harnesses | All four existing captures plus intro tests share nonblocking reads/writes, deadlines, bounded output and kill/reap cleanup on every exit. Fresh builds and assertion-unwind regression. Windows and arbitrary descendants remain outside this tested contract. |
| D6 | Packaging/API contract | No declared/tested MSRV, changelog, installable foreign packages or explicit API stability policy. Full MIT/Apache texts and Cargo package integrity are now verified; the remaining API/MSRV/installable-package contract is OPEN. |
| D7 | Long-session/resource policy | Story trace grows indefinitely until completion; public Surface storage and caller-sized helpers lack uniform hostile-resource bounds. No sustained memory/backpressure study. |
| D8 | Embedded lifecycle ownership | Global panic hook/stdout and best-effort restore suit a single terminal owner; concurrent contexts, failed writes and host panic-hook integration lack a product contract. |
| D9 | Queued input under resize/backpressure | [Issue #15](https://github.com/femboy2112/libgibson/issues/15): a delayed key is released by a later key. **REPRODUCED / ISOLATED UPSTREAM, NOT FIXED.** [Event Pressure Lab](EVENT_PRESSURE_LAB.md) supplies a bounded independent raw/Crossterm/Context reproducer. Syscalls show a signal-first batch with both tokens, followed by loss of the remaining TTY token; scratch retention restores delivery. [Upstream #1126](https://github.com/crossterm-rs/crossterm/issues/1126). Production dependencies and runtime remain unchanged; explicit collision acceptance stays red. |

Historical audit reproducer (now fixed by the consolidation regressions):

```rust
use gibson::{compute_diff, Rect, Style, Surface};
fn main() {
    assert!(std::panic::catch_unwind(||
        Rect::new(65535, 0, 1, 1).intersection(&Rect::new(0, 0, 1, 1))
    ).is_err());
    assert!(std::panic::catch_unwind(||
        Rect::new(0, 0, 1, 1).shrink(32768)
    ).is_err());
    let mut before = Surface::new(8, 1);
    before.print_str(0, 0, "hello", Style::default(), None);
    let diff = compute_diff(Some(&before), &Surface::new(8, 1));
    assert_eq!(diff.exact_changed_cell_count(), 5);
    assert!(diff.exact_changed_cells().is_empty()); // observed contract defect
}
```

Execution used `rustc --edition=2021 /tmp/libgibson-post-merge/probe.rs --extern
gibson=target/debug/deps/libgibson.rlib -L dependency=target/debug/deps -o
/tmp/libgibson-post-merge/probe`, then that binary (exit 0 with caught panics).
Initial scratch compilation had a wrong rlib glob and then a missing print_str
argument; these were probe setup errors, not library failures. The corrected live
probe produced `true`, `true`, and `erase_exact_count=5 exact_coords=[]`.

The original D3 defect was reproduced with these commands (historical failure, now fixed):

```sh
gcc -Iinclude bindings/c/example.c -Ltarget/release -lgibson \
  -Wl,-rpath,'$PWD/target/release' -o /tmp/libgibson-post-merge/ci-rpath-probe
readelf -d /tmp/libgibson-post-merge/ci-rpath-probe
env -u LD_LIBRARY_PATH /tmp/libgibson-post-merge/ci-rpath-probe
```

The literal RUNPATH and exit 127 were observed. No CI permission or billing bypass was
attempted. The earlier suite was green despite D1–D3; the public-readiness exact-command check now covers the D3 fix.

Robustness strengths remain substantial: structured text skips control-containing
graphemes, wide glyph invariants are tested, FFI enums are validated, clipping
bounds newer raster work, nonfinite camera inputs are rejected, and feedback has
explicit history semantics. Raw ANSI and directly mutated public cell storage
are not untrusted-input sandboxes. SIGKILL/abort, allocation failure and broken
terminal I/O are outside unconditional restoration guarantees.

## Repository hygiene and dependencies

At merged baseline: 155 tracked files, 1,883,817 working-tree bytes; largest file
about 80 KiB. No tracked binary, image capture corpus, log, editor swap or root
scratch script was found. Git object storage reported about 4.51 MiB loose objects;
the largest reachable historical blob is also about 80 KiB (checked with
rev-list --objects --all and cat-file). This is not a full forensic history audit. Seven docs
records are preserved and indexed. Their /tmp references are historical artifacts,
not assets needed by a clean checkout. No actionable TODO/FIXME marker backlog
was found; movie labels and test strings are not debt markers.

`cargo tree -d` reports thiserror/thiserror-impl 1.x vs 2.x, syn 2.x vs 3.x, and
unicode-width 0.1 vs 0.2. The older branches come through dev dependencies
portable-pty/vt100. Different parser/render width implementations deserve test
awareness, not forced upgrades. Direct libc and thiserror dependencies have no
source uses found by rg; review with a dependency tool and all targets before
removal. Nothing was upgraded or removed here.

Cargo metadata correctly identifies 0.1.0, edition 2021, repository, readme,
MIT OR Apache-2.0, and rlib/cdylib/staticlib outputs. It lacks rust-version,
keywords/categories and docs.rs configuration. Missing discoverability metadata
is lower priority than MSRV, license completeness, artifact install paths and
package verification. No package was published or history rewritten.

All four development remote tips are ancestors of merged main: cinematic,
agy/next-level-terminal-ui, deepseek/next-level-terminal-ui and
deepseek/visual-fx-frontier. Keep main and the active audit branch. The four
merged branches can be archived or deleted after confirmation; none was deleted.

## Initial audit housekeeping (`7bc4f9c`)

- Added this state assessment and VALIDATION_INDEX; retained all seven historical
  records, their counts and run-local provenance.
- Linked current state/index from README and DESIGN; added ranked post-merge
  priorities to ROADMAP and clarified completed primitives versus planned widgets.
- Corrected README raster constructor signature, the documented /tmp ASan binary's
  runtime library path, and the overclaim that both full license texts are included.
  Licensing terms themselves were not changed; packaging debt remains explicit.
- Marked Scene/Story module APIs experimental, corrected pipeline order and Story
  trace/event ownership wording, and repaired FocusRing rustdoc method links.
- Added a warning comment to the legacy raw insertion alias in termframe.h.

In that initial commit, no implementation, public signature, dependency, workflow, demo, test or ABI behavior
changed. No branches, history, legacy modes or validation evidence were deleted.
The discovered behavioral and workflow issues are follow-up work, not silently
fixed or represented as passing this audit.

## Release readiness and onboarding

An external Rust developer can follow the README's minimal Node/Context example,
then polished_agent or fx_lab. C headers and C/C++/Python examples work from a
checkout. Installation and stability expectations are less clear: crate lib name
is `gibson`, package is `libgibson`; no tested MSRV; Python searches repository
.so locations; C++ includes a repository-relative header; Go requires the native checkout;
full modern API parity is absent. A clean-machine packaging exercise, not another
graphics demo, is the next evidence needed for release confidence.

A serious release should close D1–D6 or explicitly narrow its supported contract,
maintain useful CI, define Linux-first support rather than imply platform parity,
and document stable versus experimental APIs. “Ready to merge the approved
feature work” was not a claim that every public API was release-candidate quality.

## Strategic opportunities and near-term roadmap

Ranked by evidence, not spectacle:

| Priority | Work / why | Dependency | Acceptance criterion | Architectural risk |
| --- | --- | --- | --- | --- |
| P0 | Keep public CI useful; enable owner-selected security settings and main protection | First exact-main public CI is now green | Preserve five executed jobs; require meaningful checks through owner-approved protection; no unreviewed bot merges | LOW |
| P1 | Build on fixed D1/D2 with broader property tests for Rect/Surface/diff reconstruction | Implemented endpoint/delta contracts | Debug/release hostile cases defined; erased/removed coordinates agree with exact count; seeded generative corpus passes | MEDIUM |
| P1 | Extend consolidated Unix PTY harnesses to a platform matrix | Existing stronger demo harness patterns | No stale executable path, zombie child or uninterruptible timeout; resize/input checks retained | LOW |
| P1 | Establish release/API/MSRV/package contract | Useful CI and license review | Minimal Rust/C/Python apps build from documented clean install; full license texts, ownership docs and tested compiler floor | LOW–MEDIUM |
| P1 | Profile sustained graphics and trace retention | Representative deterministic workloads | Separate generation/allocation/wire/consumer metrics, memory trend and slow-reader behavior; no universal FPS claim | MEDIUM |
| P2 | Expand Go wrapper coverage; establish macOS/Windows then tmux/screen/SSH matrix | Toolchains/hosts/native artifacts | Each claimed environment executes input/lifecycle/resize/binding tests; unsupported cells remain explicit | MEDIUM |
| P2 | Evaluate active capabilities/DSR and core ABI parity | Actual terminal failure evidence; stable input/query routing | Query timeout/fallback tests and no input theft; parity requirements agreed before API expansion | MEDIUM–HIGH |
| P2 | Evaluate minimal raster upload versus Scene command ABI | Rust API stabilization and consumer need | Versioned ownership/resource design with two foreign-language consumers and adversarial validation | HIGH |
| P3 | Semantic terminal application runtime with replay/inspection and accessibility | Portable core and mature cross-language contracts | Ordinary CLI and graphical applications share one tested realization/output model, with useful non-graphical fallbacks | HIGH |

The next five moves are P0 operations, core invariant fixes, trustworthy harnesses,
release contracts, then sustained performance evidence. Go/platform work can
proceed when hosts are available. Active negotiation matters, but should not jump
ahead of already-demonstrated defects or become a new protocol feature spree.

## Explicit non-goals and what to stop doing

Do not spend the next round on more Acid shots, meshes, glow, dialogue, widgets,
an ECS, shader language, raymarcher or image protocols. Do not freeze experimental
graphics into C for parity optics, merge overlapping helpers without a user need,
chase test count, or add unindexed validation dossiers. Do not delete historical
proof or legacy projections merely to make the tree look simpler.

The coherent moonshot is **a language-neutral semantic terminal runtime for
ordinary CLI UX and terminal-native graphical applications under one compositional
rendering model**. That requires portable terminal ownership, observable costs,
bounded resource/replay policies, and a usable API across languages. More graphics
alone does not advance that objective now.

## Public repository readiness

**Historical preparation checkpoint: ENGINEERING ALPHA / PUBLIC-READY.**
The repository was still **PRIVATE** when PR #2 merged. The owner subsequently
authorized the visibility switch; the current public CI evidence is recorded below.
No branch deletion, history rewrite or crate publication was performed.

### Exposure gate and accepted disclosure

Before editing, fetched all remotes and verified `7bc4f9c` was exactly one commit
ahead of `0b673cc`, zero behind, with that main commit as its parent and a clean
tree. The initial audit's nine-file diff was documentation/comments only.

Gitleaks **v8.30.1**, downloaded from its official release with checksum verification,
ran with redaction and `--log-opts=--all`. The initial inventory was **57 reachable
commits, 816 objects, 540 unique blobs**. The patch scan covered 56 patch-bearing
commits; a separate `gitleaks dir` scan over every exported reachable blob also
covered merge content. Both scans found no credentials. Current tracked-tree
scanning also found none. No suppressions were added. Supplemental historical
patterns covered private keys, provider tokens, credentials, private hosts,
database URLs, cookies, environment files and personal paths.

That supplemental check found one personal home-directory path in three old
`polished_agent.rs` blobs. It is absent from the current tree. Publication was
paused; the owner explicitly accepted disclosure of their already-public name
and authorized continuation. No other private personal information was identified.
This is an **accepted historical disclosure**, not a claim that history contained
no personal data. Author/committer identities are the expected GitHub noreply and
project identities. Scanner results are bounded evidence, not a proof that no
undiscovered secret exists.

Initial tree: 157 tracked files, 1,927,196 bytes; largest reachable historical
blob 80,044 bytes. No suspicious binary/dump/archive/log corpus was found. No tags
were advertised; the one PR head was already in scanned main ancestry. New
publication changes are scanned again before delivery.

GitHub exposure review enumerated all pages: PR #1, zero ordinary issues/comments/
reviews, 35 Actions runs, 175 jobs, zero executed steps and zero artifacts. All 35
downloaded log ZIPs were empty. The 70 annotations contained billing blocks and
runner-migration notices; no sensitive material was found. New PR text contains
only the public audit and validation summary. Logs and scan receipts are run-local
under `/tmp/libgibson-public-review/`, not repository assets.

### Licensing and workflow preparation

- Complete `LICENSE-MIT` and canonical `LICENSE-APACHE` now accompany the
  `MIT OR Apache-2.0` declaration; terms are unchanged. The Apache text was fetched
  from [Apache's official license](https://www.apache.org/licenses/LICENSE-2.0.txt).
- `LICENSES-THIRD-PARTY.md` is explicitly a direct/dev-dependency inventory,
  cross-checked with locked Cargo metadata/tree. It adds vt100, corrects
  compact_str to MIT, and removes claims about every source line's provenance.
- `SECURITY.md` states alpha support limits and directs sensitive reports to
  GitHub private reporting once the owner enables it; no personal email is added.
- CI uses `contents: read`, no write escalation or repository secrets, ordinary
  `pull_request` (never `pull_request_target`), and credential-free checkouts.
  No publish/deploy path or PR-metadata shell interpolation exists.
- Triggers: main pushes, PRs and manual dispatch. Timeouts: Rust 25 minutes,
  bindings/ASan/Go 15 each, PTY 20. These bound jobs; D5 harness internals remain open.
- Every external action is pinned to an official upstream commit, resolved with
  git refs and GitHub commit/action definitions (including annotated-tag peeling):

| Action | Verified upstream reference | Immutable commit |
| --- | --- | --- |
| actions/checkout | v4.4.0 | `11d5960a326750d5838078e36cf38b85af677262` |
| dtolnay/rust-toolchain | stable | `6bed0761d98439e5a578e2877258200ad565ba87` |
| Swatinem/rust-cache | v2.9.2 | `6323deb102c322ba6fcbdcafc7e3dddab59af2b6` |
| actions/setup-go | v5.6.0 | `40f1582b2485089dde7abd97c1529aa768e1baff` |

Rust-cache saves only on main pushes. Weekly Dependabot updates cover Actions and
Cargo, with no auto-merge. Go caching is disabled because the module has no external
Go dependencies/go.sum. Each native job builds its own release library. Rust and
PTY jobs build fresh examples before tests. Redundant standalone goldens/capability/
effects reruns were removed from Rust CI; all remain in `cargo test`. Separate
FX Lab example tests and strict rustdoc were retained/added.

### Local execution and limits

All of these passed in the public-readiness working tree:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --examples
cargo test
cargo build --release
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo test --example fx_lab
bash scripts/dev/bindings_smoke.sh --asan
cargo test --test pty_integration --test pty_resize_torture -- --test-threads=1
git diff --check
cargo package --list --allow-dirty
cargo package --allow-dirty
```

`cargo test`: **536 passed (226 unit + 310 integration)**. FX Lab: **3 passed**.
Explicit PTY integration/resize: **5 passed**. C/C++/Python and native ASan/UBSan
smokes passed (LeakSanitizer remains disabled). Local Rust is still the nightly
recorded above, not evidence for stable Rust/MSRV.

Actionlint **v1.7.12**, YAML invariant checks and an independent source review pass.
Each of the four exact C/C++/ASan workflow command blocks was run with
`GITHUB_WORKSPACE` set to the checkout and `LD_LIBRARY_PATH` removed. `readelf -d`
confirmed an expanded RUNPATH; `env -u LD_LIBRARY_PATH <binary>` passed again for
all four binaries. This closed D3 locally at preparation; the public evidence below now also covers
the hosted runner.

Go now has module `github.com/femboy2112/libgibson/bindings/go`, a normal
`cmd/example` package, corrected imports and `${SRCDIR}` cgo paths. Local
`go vet ./...`, `go test ./...`, `go build ./...`, and `go run ./cmd/example`
(with the explicit native-library path) all exit 0 on Go 1.18 / gccgo 14.2.
There are **no Go test files**; this is compile/example smoke, not exhaustive API
coverage. The smoke script and Go CI no longer mask failures. Hosted stable-Go
verification was pending at preparation and is recorded below; standalone Go
distribution remains open.

The first plain `cargo package --list` refused the uncommitted tree (exit 101);
this was an integrity guard, not a build failure. With explicit `--allow-dirty`,
listing and package build verification passed. The archive contains Cargo.toml,
source, README and both complete licenses. Nothing was published.

### Owner launch sequence

Visibility, dispatch, evidence recording and issue creation are now complete.
The security-settings and branch-protection choices remain with the owner.

1. Settings → General → Danger Zone → Change repository visibility → Public;
   confirm `femboy2112/libgibson`.
2. Immediately run Actions → CI → Run workflow → **main**. An old blocked private
   run is not public CI evidence. Watch Rust, bindings, sanitizers, Go and PTY.
3. Record the exact public-main SHA, run ID and every job result in a small docs
   follow-up. Only successful executed jobs justify **REMOTE CI GREEN**.
4. Enable/check dependency graph, Dependabot alerts/security updates, secret
   scanning/push protection where available, and private vulnerability reporting.
   Keep Actions default token read-only and conservative fork-PR approval. Enable
   full-SHA action policy if offered for this repository/account.
5. After the first green run establishes real check names, protect main with PRs
   and meaningful CI checks. Convert D1/D2/D5/D7/D8 into reproduction/acceptance
   issues; do not delete merged development branches without owner instruction.

GitHub currently documents free **standard GitHub-hosted runners in public
repositories** ([billing policy](https://docs.github.com/en/billing/concepts/product-billing/github-actions)).
That is the expected route past the private-minutes block, not a guarantee that
an account-level issue cannot remain. Diagnose the actual first public run.

## Executed public CI evidence

The owner explicitly authorized publication after accepting the historical personal
path disclosure. The repository is now **PUBLIC**. PR #2 merged readiness work at
`9e8602acca353e4ce3409a792f0135cb6796b9bf`; a manual dispatch immediately followed.

The first public [run 35945512611](https://github.com/femboy2112/libgibson/actions/runs/35945512611)
executed on stable Rust 1.98.1 and failed strict Clippy. That was a real lint failure,
not billing. Two manual canvas-clear loops and one redundant formatting borrow
were replaced with equivalent syntax in [PR #12](https://github.com/femboy2112/libgibson/pull/12).
No rendering/story behavior, public signature, dependency or lint policy changed.
Exact-toolchain local fmt/Clippy and all 536 tests passed before submission; all
five public PR jobs passed in run 35945896281.

**First green exact-public-main run:**
[35946157443](https://github.com/femboy2112/libgibson/actions/runs/35946157443),
commit `aa60036a66413925309f0f6e9d99830a967cf7ff`.

| Job | Executed result |
| --- | --- |
| Rust (fmt, clippy, test, build) | PASS: Rust 1.98.1; fmt, strict all-target Clippy, examples, 536 tests, 3 FX Lab tests, strict rustdoc, release build |
| Bindings (C, C++, Python) | PASS: fresh native release and all three example runs |
| AddressSanitizer (C, C++) | PASS: both ASan/UBSan examples; LeakSanitizer disabled |
| Go bindings | PASS: Go 1.27.1; fresh native build, vet/test/build/example; no Go unit test files |
| PTY integration tests | PASS: fresh examples and five integration/resize tests |

This closes the billing execution gap and D3/D4 hosted-runner uncertainty for this
Linux snapshot. It does not establish an MSRV, exhaustive Go wrapper coverage,
other-platform support, universal performance or release-candidate maturity.
**REMOTE CI GREEN** refers to this exact executed snapshot, not future commits.

Known debt now has public reproductions/contracts and acceptance criteria:
[D1 #7](https://github.com/femboy2112/libgibson/issues/7),
[D2 #8](https://github.com/femboy2112/libgibson/issues/8),
[D5 #9](https://github.com/femboy2112/libgibson/issues/9),
[D7 #10](https://github.com/femboy2112/libgibson/issues/10),
[D8 #11](https://github.com/femboy2112/libgibson/issues/11).
D6's API/MSRV/distribution work remains open. Four initial Dependabot update PRs
were unmerged at this `aa60036` checkpoint. All four subsequently passed public CI
and merged during the [intro consolidation](INTRODUCTORY_CINEMA.md); they are not
part of the older green-main result above.

Settings observed at launch: default Actions token read-only, Actions cannot
approve PR reviews, first-time fork-contributor approval required. Private
vulnerability reporting, secret scanning/push protection and Dependabot security
updates were disabled. The owner should review/enable these and main protection
now that actual check names/results exist. No such settings or branches were
changed automatically.

## Current claim ledger

| Claim | Status | Boundary / next evidence |
| --- | --- | --- |
| Approved cinematic work is on main | VERIFIED | Acid-vs-Crash through PR #1; introductory flagship through PR #14 at e5ede0a. |
| Linux core pipeline works | IMPLEMENTED + TESTED | Merged regressions now cover D1/D2; historical full-run evidence and current intro validation are separately identified. |
| C/C++/Python binding smoke works | IMPLEMENTED + TESTED | Local checkout; sanitizer boundary stated above. |
| Go bindings build and example runs | PARTIALLY TESTED | Local gccgo plus public Go 1.27.1 smoke; no Go unit tests or wider API/platform certification. |
| Windows/macOS/mux/SSH work | UNVERIFIED | Need actual host/terminal matrix. |
| Scene/Story semantics are exercised | EXPERIMENTAL + TESTED | No API freeze; recorded updates only, no portable serialization. |
| RGB/3D/effects work through Unicode cells | EXPERIMENTAL + TESTED | No image protocol; normal Surface/diff output. |
| Replay and frozen 0/0/0 hold | IMPLEMENTED + TESTED | Recorded world/visual history and unchanged physical terminal state. |
| Renderer is uniformly hostile-input hardened | FALSE as a broad claim | Mutable storage and resource policies. |
| Exact changed-coordinate enumeration is complete | IMPLEMENTED + TESTED on main | Erase/removal/wide-glyph oracle; forged public mutable metadata remains caller responsibility. |
| 60 FPS everywhere | DO NOT CLAIM | Cadence ceiling; sustained transport and slow hardware unverified. |
| Remote CI is green | VERIFIED for the recorded public-main snapshot | Run 35960000454 at e5ede0a; five executed jobs passed. Documentation-only research merge 3d9117d also passed run 35961022598. Historical private failures remain preserved. |
| Full modern API is language-neutral | FALSE | Core UI ABI exists; modern composition/graphics remain Rust-only. |
| New aesthetic acceptance was performed | PARTIALLY TESTED | Intro Surface frames inspected and release/PTY paths exercised; independent human taste acceptance remains unverified. |
| Reachable-history credential scan | PASS, accepted privacy disclosure | Gitleaks history + all blobs + supplemental patterns; owner accepted old personal path; bounded detection. |
| Own license packaging | VERIFIED | Complete MIT/Apache texts; Cargo archive verification. |
| Public workflow least privilege | VERIFIED FROM YAML + ACTIONLINT | SHA-pinned, read-only, no persisted checkout credentials/secrets/publish paths; five public CI jobs now pass at the recorded SHA. |
| Native CI loader paths | FIXED + LOCAL EXACT-COMMAND VERIFIED | Four normal/sanitized binaries run without LD_LIBRARY_PATH. |
| Public GitHub CI | IMPLEMENTED + TESTED | Public, exact-main run 35960000454; stable Rust 1.98.1 and Go 1.27.1 evidence. |
| External dynamic-UI expressivity | CORROBORATED in tested domain | Three planned consumers and three post-freeze holdouts; shared model/fixture limits, no core promotion, no universal ease claim. |
| Terminal ownership lease + checked restore (#11) | IMPLEMENTED + TESTED | Megaround branch `claude/runtime-architecture-megaround` / PR #19: process-global lease (Available/Owned/Restoring), a second owner errors with `AlreadyExists`, `restore()` reports the first error with best-effort completion, panic hook gated on the lease, Drop best-effort; 4 PTY ownership tests. **#11 not closed**: the restore-error path is not covered by a failing-write test (needs writer injection). |
| Long-session Story trace retention (#10) | IMPLEMENTED + TESTED | Megaround: measured ~40 B/tick, ~40 MiB at 1M ticks, linear/unbounded; `TraceRetention {All (default), Bounded(cap), Disabled}` + `drain_trace` + honest `is_complete()`/`dropped_steps()`. Default `All` preserves prior behavior. **#10 not closed**: secondary Scene/Facts append-only gaps remain. |
| Crossterm #1126 input starvation (#15) | DIAGNOSED / UPSTREAM, NOT FIXED | Megaround + PR #18: CORROBORATED across source, mio `EPOLLET`, runtime delivery counts and strace; upstream A5 (0.29.0 still latest, no fix); the naive drain-to-WouldBlock fix is **proven to hang** on the blocking `VMIN=1` tty fd. Analysis staged (`docs/CROSSTERM_1126_FIX_ANALYSIS.md`); no crossterm fork vendored. **#15 open.** |
| Runtime Observatory diagnostic instrument | IMPLEMENTED + TESTED | Megaround: `--dump`-only instrument; three modes (Million-Tick/Ghost-Key/Ownership-Duel) render real state with unobserved layers shown as `?`; heavier modes (Slow-Terminal/Restore-Failure/Endurance) and a live loop are not built. See `docs/RUNTIME_OBSERVATORY.md`. |
| Project is a release-candidate platform | NOT YET | Engineering alpha; operational, core-contract and distribution work outrank new features. |
