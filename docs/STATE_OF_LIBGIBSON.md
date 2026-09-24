# State of LibGibson

## Executive summary

This is the canonical bird's-eye assessment of the first post-merge baseline,
not a replacement for [DESIGN](../DESIGN.md). Historical evidence is catalogued
in the [validation index](VALIDATION_INDEX.md).

**Classification: ENGINEERING ALPHA, with a well-tested Linux rendering core
and experimental Rust composition/graphics layers.** “Core” describes architectural
responsibility and relative maturity; it does not promise a frozen Rust API or
universal terminal compatibility. There is no basis yet for “release-candidate
platform,” universal 60 FPS, or modern Rust/C feature parity.

The approved tip `3b67ca23ee6988cc450845977fe2daf017ae6747` was merged through
[PR #1](https://github.com/femboy2112/libgibson/pull/1). Main at this audit is
`0b673ccfa50492111352c969d58ebe3cfadfcac1`. Ancestry verification succeeded.
The audit branch, `codex/post-merge-state-audit`, began at that exact main SHA
with a clean worktree. It contains documentation/comment changes only and is
not automatically merged. Findings below describe the merged implementation.

Both baseline and final local gates passed: **536 tests = 226 unit + 310
integration**, no failures or ignored tests. Three additional FX Lab example
tests pass separately. This inventory is scope, not a quality score.

Important discoveries remain open: hostile `Rect` arithmetic, inaccurate exact
changed-coordinate enumeration, a native CI loader path defect, incomplete Go
build setup, and PTY harness cleanup/deadline weaknesses. Passing the existing
suite does not erase these findings. See the reproducible probes below.

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
| Differential rendering | IMPLEMENTED + TESTED, known coordinate-report defect | Count, affected footprint and bytes are distinct; see debt D2. |
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

API documentation still needs a support/stability policy, crate-level entrypoint,
FFI ownership contracts and a review of public mutable implementation fields.
Privatization, renaming and feature flags are recommendations, not this audit's edits.

## Demo-local technology and demo strategy

BattleGraph, continuous influence, AcidPlanner memory/scoring, CrashController,
EncounterTrace, VisualHistory/ShotHistory, VisualDirector, camera choreography,
subsystem meshes and final formations belong in `examples/acid_vs_crash/`.
They are a domain model and its projections, not core library concepts.

| Demo | Job | Positioning |
| --- | --- | --- |
| polished_agent | Practical inline UI, native history, mutable foreground, focus/viewport | Product-oriented example; good next step after Quickstart. |
| hack_the_gibson | Broad act/effect showcase and fictional Hackers homage | Showcase, not minimal API tutorial; shell is simulated. |
| fx_lab | Isolated primitive gallery and regression microscope | Engineering lab; three example tests require explicit invocation. |
| acid_vs_crash | Integrated deterministic model → shots → graphics/UI → terminal | Cinematic flagship; default Crash fights, manual interventions remain. |
| agent_chat | Earlier lifecycle/chat example | Legacy example/fixture; manual timing and raw output make it a weaker starting point. |
| perf_probe / resize_test_app | Damage/byte and resize exercise | Probe/fixture, not universal performance certification. |

Legacy flat/cyber projections intentionally preserve comparison and goldens.
Do not delete them as dead code or spend another round polishing the flagship.

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
source is **UNVERIFIED** and has build-system gaps, not merely missing local execution.

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
of a minimum stable Rust version. The workflow requests stable, but did not execute.

Executed before edits and again after housekeeping:

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
Go executable absent; overall smoke success does not certify Go. The script even
allows a Go build failure to remain best-effort when Go is installed.

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
| 32 integration targets / 310 tests | Composition, diff, ABI, replay, graphics, PTY and fixtures | Some targets import demo modules, so this is also demo maintenance coverage. |
| whole_renderer_vt100 / screen_state_vt100 | Full renderer or diff/compiler bytes reconstruct expected screen | Shared vt100 parser family, not independent emulator consensus. |
| scene/story/acid model tests | One-arrow law, reactions, irregular dt replay, counterfactual choices, visual history/camera equality | Exact recorded input updates; no durable portable replay format. |
| raster3d / raster_fx / surface_fx | Occlusion, clipping, hostile floats, masks, wide cells, deterministic feedback | Newer raster safety does not cover every older helper. |
| PTY integration/demos/resize | Actual input, lifecycle, restoration, typed-input preservation and resize | Unix termios checks are conditional; executed only on Linux here. |
| 51 text goldens + raster probes | Terminal reconstruction plus semantic RGB/depth/diversity checks | Acid text goldens retain legacy flat projections; they do not certify cinematic art. |
| effects_perf / perf_probe | Damage locality, exact/affected/wire separation | Not sustained throughput benchmarks. |

The baseline `cargo test` command took about 92 seconds on this host; visual_goldens
reported about 37 seconds. These are observations, not timing assertions. Golden
captures use fixed wall-clock waits; semantic/raster tests use deterministic time.
Three fx_lab example tests sit outside the 536 default count. Repeated checks at
model/presentation/terminal layers guard distinct boundaries; CI's repeated goldens
and PTY commands also duplicate some full-suite execution and can later be tuned.

Highest-value missing coverage is generative cell/diff and geometry invariants,
bounded fresh-build PTY harnesses, actual platform/mux runs, sustained graphics
memory/transport, and foreign binding parity. Use property testing first for small
in-process invariants; add cargo-fuzz for parser/FFI-adjacent valid harnesses and
clipping after a corpus and input contracts exist. Do not feed invalid foreign
pointers and call resulting UB a fuzzable safe API contract.

Existing tests reconfirm frozen-frame exact delta/affected/wire **0/0/0**, full
encounter/shot/camera/final-frame replay, and resize re-projection without semantic
advance. Frozen *cells alone* do not imply zero wire if physical cursor or anchor
state changed. This audit did not repeat aesthetic acceptance or a full manual
playthrough; previous frame inspections are historical evidence, not a new human vote.

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
| Go | UNVERIFIED; compiler absent and module/native-job setup incomplete. |
| Active negotiation / DSR | PLANNED; color inference and relative anchors exist instead. |

Fresh merged-main [Actions run 35941989705](https://github.com/femboy2112/libgibson/actions/runs/35941989705)
failed with **zero Rust steps**; dependent jobs were skipped. Its annotation states
that recent payments failed or the spending limit needs increasing.
**REMOTE CI: BLOCKED / ENVIRONMENTAL**, neither green nor a demonstrated source
failure. This was verified for the main SHA above, not copied from a prior dossier.
The separate workflow defects below will still need attention once jobs can run.

## Known blockers and technical debt

These findings are intentionally recorded rather than bundled into a behavior-changing
“cleanup.” No new feature or core fix is hidden in this consolidation branch.

| ID | Finding | Evidence and required next check |
| --- | --- | --- |
| D1 | Public Rect overflow | Debug probe confirms intersection at x=65535,width=1 and shrink(32768) panic. Endpoint/multiplication arithmetic in surface.rs is unchecked; release wrapping must also receive a defined contract and regression. Ordinary terminal geometry is not a reproducer. |
| D2 | Exact coordinate enumeration is incomplete | Erasing `hello` from an 8×1 Surface yields exact count 5, but exact_changed_cells() returns []. It enumerates explicit runs, omitting erase/removal deltas. Repair storage/contract with blank-padding, erasure and removed-row regressions; leave compatibility metrics distinct. |
| D3 | Native CI runtime path is wrong | Workflow single-quotes `$PWD/target/release`; readelf confirms literal `$PWD` RUNPATH and local exact-command executable fails loading libgibson.so, exit 127. Local smoke script uses a correct expanded path. |
| D4 | Go is not installation-ready | No go.mod; ignored example imports old owner path; isolated Go job never builds/downloads native library; best-effort failure is masked. Compiler availability alone will not close this. |
| D5 | Test harness lifecycle/deadlines | Older pty_integration blocking read_line defeats its apparent timeout; visual_goldens kill without wait/join; several harnesses build only when binary absent. Fresh build ordering and bounded reaping need consolidation. |
| D6 | Packaging/API contract | No declared/tested MSRV, changelog, crate overview, installable foreign packages or explicit API stability policy. LICENSE points to missing split files and includes Apache notice/link rather than full Apache text. |
| D7 | Long-session/resource policy | Story trace grows indefinitely until completion; public Surface storage and caller-sized helpers lack uniform hostile-resource bounds. No sustained memory/backpressure study. |
| D8 | Embedded lifecycle ownership | Global panic hook/stdout and best-effort restore suit a single terminal owner; concurrent contexts, failed writes and host panic-hook integration lack a product contract. |

Small reproducible Rust probe, linked against the existing debug rlib:

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

D3 was reproduced with these commands from the repository root:

```sh
gcc -Iinclude bindings/c/example.c -Ltarget/release -lgibson \
  -Wl,-rpath,'$PWD/target/release' -o /tmp/libgibson-post-merge/ci-rpath-probe
readelf -d /tmp/libgibson-post-merge/ci-rpath-probe
env -u LD_LIBRARY_PATH /tmp/libgibson-post-merge/ci-rpath-probe
```

The literal RUNPATH and exit 127 were observed. No CI permission or billing bypass was
attempted. General Rust warnings and the existing suite remain green despite D1–D3.

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

## Housekeeping delivered in this branch

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

No implementation, public signature, dependency, workflow, demo, test or ABI behavior
changed. No branches, history, legacy modes or validation evidence were deleted.
The discovered behavioral and workflow issues are follow-up work, not silently
fixed or represented as passing this audit.

## Release readiness and onboarding

An external Rust developer can follow the README's minimal Node/Context example,
then polished_agent or fx_lab. C headers and C/C++/Python examples work from a
checkout. Installation and stability expectations are less clear: crate lib name
is `gibson`, package is `libgibson`; no tested MSRV; Python searches repository
.so locations; C++ includes a repository-relative header; Go has no module;
full modern API parity is absent. A clean-machine packaging exercise, not another
graphics demo, is the next evidence needed for release confidence.

A serious release should close D1–D6 or explicitly narrow its supported contract,
restore useful CI, define Linux-first support rather than imply platform parity,
and document stable versus experimental APIs. “Ready to merge the approved
feature work” was not a claim that every public API was release-candidate quality.

## Strategic opportunities and near-term roadmap

Ranked by evidence, not spectacle:

| Priority | Work / why | Dependency | Acceptance criterion | Architectural risk |
| --- | --- | --- | --- | --- |
| P0 | Restore Actions account execution; correct native CI paths and independent job inputs | Account owner resolves billing | Exact-head clean runner executes useful Rust/native jobs; zero-step failures no longer mistaken for test results | LOW |
| P1 | Fix D1/D2, add bounded property tests for Rect/Surface/diff reconstruction | Explicit endpoint/delta contracts | Debug/release hostile cases defined; erased/removed coordinates agree with exact count; seeded generative corpus passes | MEDIUM |
| P1 | Consolidate fresh-build, bounded/reaped PTY harnesses | Existing stronger demo harness patterns | No stale executable path, zombie child or uninterruptible timeout; resize/input checks retained | LOW |
| P1 | Establish release/API/MSRV/package contract | Useful CI and license review | Minimal Rust/C/Python apps build from documented clean install; full license texts, ownership docs and tested compiler floor | LOW–MEDIUM |
| P1 | Profile sustained graphics and trace retention | Representative deterministic workloads | Separate generation/allocation/wire/consumer metrics, memory trend and slow-reader behavior; no universal FPS claim | MEDIUM |
| P2 | Verify Go; establish macOS/Windows then tmux/screen/SSH matrix | Toolchains/hosts/native artifacts | Each claimed environment executes input/lifecycle/resize/binding tests; unsupported cells remain explicit | MEDIUM |
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

## Current claim ledger

| Claim | Status | Boundary / next evidence |
| --- | --- | --- |
| Approved cinematic work is on main | VERIFIED | Ancestor of 0b673cc; PR #1 merged. |
| Linux core pipeline works | IMPLEMENTED + TESTED | 536 tests and native smokes; D1/D2 remain outside previous coverage. |
| C/C++/Python binding smoke works | IMPLEMENTED + TESTED | Local checkout; sanitizer boundary stated above. |
| Go bindings work | UNVERIFIED | No compiler; source/build gaps D4. |
| Windows/macOS/mux/SSH work | UNVERIFIED | Need actual host/terminal matrix. |
| Scene/Story semantics are exercised | EXPERIMENTAL + TESTED | No API freeze; recorded updates only, no portable serialization. |
| RGB/3D/effects work through Unicode cells | EXPERIMENTAL + TESTED | No image protocol; normal Surface/diff output. |
| Replay and frozen 0/0/0 hold | IMPLEMENTED + TESTED | Recorded world/visual history and unchanged physical terminal state. |
| Renderer is uniformly hostile-input hardened | FALSE as a broad claim | Rect counterexample, mutable storage and resource policies. |
| Exact changed-coordinate enumeration is complete | FALSE | D2 erase probe. Exact count is a separate API. |
| 60 FPS everywhere | DO NOT CLAIM | Cadence ceiling; sustained transport and slow hardware unverified. |
| Remote CI is green | FALSE / BLOCKED ENVIRONMENTALLY | Exact-main zero-step billing annotation; D3/D4 are separate latent job defects. |
| Full modern API is language-neutral | FALSE | Core UI ABI exists; modern composition/graphics remain Rust-only. |
| New aesthetic acceptance was performed | UNVERIFIED THIS ROUND | No new cinematic content or human visual review. |
| Project is a release-candidate platform | NOT YET | Engineering alpha; operational, core-contract and distribution work outrank new features. |
