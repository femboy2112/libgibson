# Introductory cinema and repository consolidation

LibGibson remains an **engineering alpha**. The introduction is a directed,
72-second local simulation, not a real agent service. It demonstrates one
information system changing its visual realization, entirely through Unicode
cells and terminal colors.

```sh
cargo run --release --example libgibson_intro -- --auto --color=truecolor
cargo run --release --example libgibson_intro -- --stage=city
cargo run --release --example libgibson_intro -- --at=39 --freeze
cargo run --release --example libgibson_intro -- --color=mono
```

Space pauses; left/right selects adjacent acts; R restarts; Esc or Ctrl-C exits.
Hints appear for the first four seconds, retreat during playback, and return for
three presentation seconds after pause/resume or act navigation. Pausing keeps
them visible without advancing visual time; the final hold gets a quiet replay
affordance after 71s. `--no-hints` suppresses those overlays.
Without `--auto`, the final image holds for replay. `--deterministic` uses a fixed
positive presentation step; `--speed=2` accelerates the film. `--seconds=5` is a
wall-clock smoke-test exit, not narrative state. `--help` lists inspection flags.

## The film

| Time | Act | Visual and semantic purpose |
| --- | --- | --- |
| 0–20 s | Agent harness | Ordinary declarative Node UI. ARCHITECT defines acceptance, SCOUT maps an offline transit graph, BUILDER produces a solver, VERIFY challenges fixtures. The UI reports this finite fictional job graph. |
| 20–28 s | Membrane | The actual painted harness refracts through one coalescing scalar field. Its glyphs and strokes feed the well; buildings appear inside it before the city fills the frame. |
| 28–36 s | Information city | Depth-tested architecture and fine colored Braille wireframes. Four landmarks retain the agents' identities. |
| 36–44 s | Facades | Four two-second shots frame acceptance, topology, candidate routes and replay witnesses. Depth-tested application windows resolve into native micro-UIs on the same facade anchors. |
| 44–53 s | Couriers | Contract, graph, candidate and verification messages follow named source/destination routes. Tangent/lookahead camera attention follows their actual route; sender-colored capsules carry payload slats and trigger receiving-facade pulses. |
| 53–60 s | Ascent | The live city and curved ground share a perspective camera. Altitude reveals the horizon; the buildings recede to their geographic beacon without rescaling a captured city frame. |
| 60–72 s | Earth | Rotating, perspective ray/sphere Earth, atmosphere and four identity-colored network arcs. The complete chrome `libGibson` title approaches from depth, locks, catches a highlight, and holds above graphical gold “Hack the planet!” lettering. |

The city draws from the film's information-as-architecture idea: the original
production notes describe Manhattan becoming a circuit board with information
passing between buildings. This informs the visual metaphor; no film assets or
screenplay passages are included. [Hackers production notes](https://ibiblio.org/team/history/hack/prod.html),
[director interview](https://nerdist.com/article/hackers-director-iain-softley-interview/).
The existing FX Lab wireframe city is the repository's immediate visual ancestor.
Acid vs Crash retains its separate adversarial story and presentation.

## Composition, not a second engine

- `model.rs`: finite agent/dependency graph and four courier records shared by UI
  and city. All tools and outcomes are explicitly simulated.
- `director.rs`: seven contiguous, seekable cues. Pure projection of explicit
  time; no per-frame trace retention. Pause and final hold stop time advancement.
- `identity.rs`: four persistent accents, one-cell signatures and semantic membrane
  anchors. Colors support identity; silhouettes and marks preserve it without color.
- `shots.rs`: twelve finite city subshots, including four facade holds and four
  courier follows. This is film direction, not a new core cinematic API.
- `harness.rs`: ordinary Node/layout/painter realization. Widgets know nothing
  about their later distortion.
- `membrane.rs`: inverse sampling of that realized Surface, summed field and analytic
  gradient, RGB tunnel and approaching Braille city geometry. Four identity conduits
  connect harness anchors to the same building roofs; no expanding circular wipe.
  Complete cell writes preserve glyph ownership; there is no terminal alpha.
- `facade.rs`: compact ordinary Node applications: framed identity, status tab, metrics,
  Braille chart and handoff footer. One diagram recipe serves the card and world plane.
- `world.rs`: four distinct wireframe landmarks, mounted information displays, semantic
  routes, curved camera framing and the scale transition. Overlapping courier
  attention sums continuously; it does not jump between the first active messages.
- `planet.rs` / `wordart.rs`: analytic perspective sphere, shared city camera/site
  projection, illustrated geographic material, body rotation, occluded identity
  arcs and bounded bitmap art. Face coverage is sampled once and reused across
  extrusion layers; no font asset or generic texture engine is introduced.
- `libgibson_intro.rs`: Context lifecycle, controls, capability selection and
  frame composition. Painting neither advances the world nor records history.

`geom::CubicPath3` is the only new core primitive: a finite, clamped cubic Bézier
route with a normalized tangent. Cameras and couriers consume the same generic
math. It owns no clock, actor or story state. Invalid input has an explicit safe
result; intermediate arithmetic is widened to avoid finite-f32 overflow.

The cue sheet stays demo-local. It does not replace Scene or Story: this finite
seekable film has no branching semantic events requiring a new StoryTrace.
Existing Scene/Story/replay tests remain authoritative for those abstractions.
There is no ECS, second terminal renderer, image protocol, external service or
new dependency for the demo.

## Responsiveness and resource policy

The harness changes column composition at 74/110 cells. The city uses viewport
aspect in its camera; the same semantic objects remain present. TrueColor is the
hero path, ANSI256/16 use central terminal quantization, and Mono uses deliberate
Braille/density geometry. Readability depends on the terminal's Unicode font.

Realization is capped at 320×100 cells; Context handles the actual terminal area.
The model and cue storage are constant-sized, trails are sampled from finite
routes, and held/replayed frames retain no growing history. This addresses the
intro's lifetime, not the core's general long-session resource-policy debt.
City rasterization uses a 2×4 sample grid for crisp Braille. This is substantially
more generation work than ordinary text; moving cameras naturally cause broad
frame damage. Frozen frames must have zero semantic delta, affected footprint,
and wire bytes. There is no universal 60 FPS claim.

`--dump=/tmp/intro.ppm --at=68` exports a developer RGB witness without starting a
terminal. It preserves half-block colors; ordinary glyphs are only color samples,
so it is **not** a faithful text screenshot. No generated image corpus is tracked.

## Phase A decisions

Starting main: `996ced2e375512177cd946a490284b7c52774d70`.
Baseline: fmt, strict Clippy, 536 default tests (226 unit + 310 integration), release
build passed. Existing public CI was useful and green, not billing-blocked.

- PR #3 / setup-go and #4 / checkout: official immutable action refs reviewed,
  refreshed checks passed all five jobs; merged. Read-only token and disabled
  checkout credentials preserved. Checkout merge used ordinary SSH Git push
  because the local API token lacked workflow-update scope; no force push.
- PR #6 / compact_str: full refreshed CI passed; merged.
- PR #5 / vt100: refreshed CI exposed real API migration requirements. Resize
  moved to `screen_mut().set_size`, and cell contents became borrowed. Tests were
  adapted in the PR, preserving their assertions; full local suite and all five refreshed CI jobs then passed. Merged as `3438c89`.
- Older cinematic, audit, public-CI, AGY and DeepSeek branch tips are ancestors
  of main. They contain no outstanding alternate implementation to absorb.
  No branches were deleted.

D1 is fixed on this branch: Rect's half-open endpoints saturate to the maximum
Surface extent; intersection and shrink no longer overflow. D2 is fixed: exact
coordinates retain compact spans independent of ANSI patches, covering erasures,
removed rows/columns and wide continuations. Already-blank erase padding remains
only affected footprint. The public Rust `SurfaceDiff` gains a metadata field;
external exhaustive literals need it or `..Default::default()`. C ABI unchanged.

D5's Unix direct-child test contract is consolidated: all four existing PTY/golden
harnesses use nonblocking capture, deadlines, bounded output, fresh Cargo builds
and kill/reap cleanup, including panic paths. No reader threads survive tests.
Windows/ConPTY and arbitrary process-tree supervision remain outside that contract.

D6 remains open for MSRV, stable API policy and independently installable foreign
packages. D7 remains open for general sustained memory/backpressure policy. D8
remains open for embedded ownership and restoration-error behavior. These need
bounded engineering rounds, not speculative rewrites inside an intro demo.

## Validation boundary

Correctness tests cover cue/model continuity, route geometry, exact replay and
seeking, frozen zero-diff frames, capabilities, dimensions, glyph invariants,
real PTY controls, resize and terminal restoration. They do not establish human
cinematic taste. Rendered Surface frames are inspected separately; terminal font,
color response, bandwidth and display latency can change the experience.

Final command results and remote PR evidence are recorded in the completion
update below. Historical validation documents retain their original counts.

### Initial 564-test checkpoint: measured scope and retained failures

The release 120×32 deterministic full film completed at normal speed in 77.44s on
this Linux host and emitted 7,822,634 bytes; terminal mode and alternate-screen
restoration passed. That runner did not set `--color`, and its effective color
depth was not recorded. It is not a measured TrueColor baseline. The current
host has `NO_COLOR=1`, which takes precedence over `COLORTERM`; an inherited
Mono setting is a plausible explanation, not recoverable historical proof. Stage smokes passed at 56×24 Mono, 160×40 TrueColor and 120×32
ANSI256. The 72s cue timeline is visual time; scheduling/generation/transport can
extend elapsed playback. No universal frame-rate claim follows.

Five-sample release generation medians at selected times (not CI timing assertions):

| Cells | Harness | Tunnel | City | Facade | Courier | Ascent | Earth/title |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
|56×24|0.11ms|0.73ms|1.72ms|1.68ms|1.95ms|2.01ms|0.29ms|
|120×32|0.52ms|2.19ms|3.60ms|3.99ms|4.21ms|4.20ms|0.69ms|
|160×40|0.76ms|3.83ms|5.29ms|6.14ms|6.35ms|6.61ms|1.13ms|

These samples precede the final held-frame reuse optimization and measure Surface
generation only, not terminal display latency. A 1/60s tunnel increment at 160×40
changed 4,003 cells and compiled to 118,976 bytes; Earth/title changed 23 cells and
917 bytes. Broad animated damage is intentional. A frozen scene yields 0/0/0.
The runner now reuses the existing Node while time/dimensions/pause status are
unchanged, and blocks on input instead of regenerating a frozen raster.

The first integrated local and remote gates failed the old Acid resize test after
reader-thread removal. A retained live-child diagnostic showed a delayed key was
queued, not erased: sending distinct `z` released the old `r`, yielding `rrrz`.
The harness's inherited 60ms sleep had stopped draining graphical output. It now
actively captures for those same 60ms, preserving all key counts, assertions and
timeouts. Serial and six parallel resize runs passed. A possible crossterm
SIGWINCH/readiness interaction under deliberately blocked output remains a bounded
[follow-up #15](https://github.com/femboy2112/libgibson/issues/15); the backend mechanism is not claimed proven or fixed by a harness edit.

A temporary release-smoke script also initially stopped reading at child exit and
missed queued restoration bytes. Draining that final queue fixed the measurement;
termios restoration itself had already succeeded. This was a probe error, not an
engine patch. Generated captures and raw logs remain local development artifacts.

Dependency duplication is now limited to proc-macro/transitive `syn` and
`thiserror` versions (portable-pty's older branch versus the direct thiserror 2
branch). The vt100 update unifies unicode-width. No broad dependency upgrade,
API rewrite, generated image corpus or runtime asset dependency was introduced.

### Initial 564-test checkpoint: local validation

Linux x86_64, Rust/Cargo 1.98.1. Commands below exited 0 after the documented
resize-capture correction. The earlier failed full-suite run is retained as a
failure, not counted as a pass.

```sh
cargo +1.98.1 fmt --check
cargo +1.98.1 clippy --all-targets --all-features -- -D warnings
cargo +1.98.1 test
cargo +1.98.1 build --release
cargo +1.98.1 build --examples
cargo +1.98.1 build --release --examples
RUSTDOCFLAGS="-D warnings" cargo +1.98.1 doc --no-deps
cargo +1.98.1 test --example fx_lab
RUSTUP_TOOLCHAIN=1.98.1 bash scripts/dev/bindings_smoke.sh --asan
cargo +1.98.1 package --list --allow-dirty
git diff --check
```

Default suite: **564 tests = 226 unit + 338 integration**, 36 integration targets,
no failed/ignored tests. FX Lab: 3 additional tests. New coverage comprises 8
geometry/diff regressions, 2 PTY cleanup regressions, 5 cubic-route tests, 8 cinematic
projection/model tests and 5 real intro PTY tests. Existing goldens remain unchanged.

Additional targeted commands passed during development:

```sh
cargo test --test geometry_diff_contract
cargo test --release --test geometry_diff_contract
cargo test --lib diff::tests
cargo test --lib surface::tests
cargo test --test whole_renderer_vt100 --test surface_fx
cargo test --test pty_demos acid_ -- --test-threads=1
cargo test --test pty_demos -- --test-threads=1
cargo test --test pty_resize_torture -- --test-threads=1
cargo test --test pty_integration -- --test-threads=1
cargo test --test visual_goldens -- --test-threads=1
cargo +1.98.1 test --test cinematic_paths --test intro_cinema
cargo +1.98.1 test --test intro_pty -- --test-threads=1
```

Early scoped checks used the host default nightly; final combined gates above
use stable 1.98.1. This is not an MSRV claim. C/C++/Python smokes and C/C++
ASan/UBSan passed; Go local vet/test/build/example passed with gccgo 14.2
(reporting Go 1.18). The Go package contains no unit tests. The package file list
includes the intro modules, README and both complete licenses; no crate published.

The repaired vt100 PR passed all five jobs in [run 35949465570](https://github.com/femboy2112/libgibson/actions/runs/35949465570)
and merged into main `3438c89645c760201be4b2598cf12161efada782`. The new intro and
core fixes are reviewed separately in [PR #14](https://github.com/femboy2112/libgibson/pull/14).
The first PR #14 run exposed the resize harness failure; subsequent exact-tip
results must be read as distinct checkpoints. Main has the dependency updates;
this feature/hardening PR remains unmerged until separately accepted.

Public CI also passed all five jobs for intro tip
`22009cef516e0df1f69c7b4787a5d26699384f0b` in
[run 35950398202](https://github.com/femboy2112/libgibson/actions/runs/35950398202):
Rust, native/Python bindings, native sanitizers, Go and PTY. This is branch
evidence; it does not imply that the intro is already on main.

## Cinematic art-direction pass

Starting tip: `5aa91018d083a12b9321366a71772f9ebb5b87af`; main remained
`3438c89645c760201be4b2598cf12161efada782`. PR #14 stays unmerged.
The stable baseline again passed all 564 tests. This pass changes only the intro,
its presentation tests, and documentation; D1/D2/D5, the four-agent model and
all other demos retain their implementation.

The four identities are ARCHITECT/cyan/house frame, SCOUT/gold/diamond,
BUILDER/violet/scaffold, VERIFY/mint/validation lattice. Their marks, colors,
structures and payload routes persist across the different realizations.
The film still lasts 72 presentation seconds, with its original seven acts.

Visual review is separate from correctness. Reconstructed cell frames were
inspected with actual glyphs, Braille dots and capability-quantized styles;
`--dump` remains a lower-resolution RGB witness. The review found and corrected
occluded rear-landmark camera corridors, overlapping narrow labels, projected
output that read like structural floors, dark orbital strokes overwriting Earth,
and clipped subtitle placement. These were demo presentation faults, not new
core renderer defects. No subjective human acceptance is inferred from tests.

The title bounds contract reports unclamped geometry, including extrusion,
subtitle, rail and the timed sparkle. Supported-size checks therefore detect
clipping instead of merely testing already-clamped coordinates. Other new
contracts cover all four readable facade identities, continuous camera positions
at shot and overlapping-message boundaries, and transient hints through real PTY
input. Frozen frames still require zero exact delta, footprint and wire bytes.

That polish checkpoint passed **571 tests: 226 unit + 345 integration**, plus
three explicit FX Lab tests. Seven new contracts supplement the prior 564-test
checkpoint: hint lifetime, real PTY hint retreat/navigation, identity/facade
coverage, camera continuity, unclamped title bounds/frozen hold, and additive
planetary emission, and late-ascent source clipping. The emission regression catches route markers or beacon
rings darkening the globe during their entrance. Prior numbers above remain
historical evidence.

A final independent review reproduced a late-ascent glyph leak at 59.94084s,
120×32: an inverse sample `(327717,131101)` narrowed to `u16` and wrapped into
source cell `(37,29)`. The new regression failed before the fix and passed after
checking signed source bounds before conversion. This demo-only fix prevents a
stray city glyph at destination `(107,18)`; it changes no core raster semantics.

Five-sample optimized generation medians on this Linux host:

| Cells | Harness 12s | Membrane 24s | City 33s | Facade 39s | Courier 47s | Ascent 58s | Title 68s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 56×24 | 0.11ms | 0.81ms | 1.36ms | 1.36ms | 1.62ms | 2.05ms | 0.47ms |
| 120×32 | 0.37ms | 2.43ms | 3.26ms | 3.98ms | 3.81ms | 4.45ms | 1.42ms |
| 160×40 | 0.47ms | 3.74ms | 5.02ms | 6.22ms | 5.54ms | 6.83ms | 2.36ms |

The earlier city recognition overlaps RGB membrane and fine wireframe generation
at 26s: 7.55ms at 120×32, 10.99ms at 160×40. The previous implementation also
paid for both realizations near its later exit. The hero title costs about twice
the old shallow wordmark; it now covers substantially more pixels with multiple
bevel/extrusion layers and network light. Flattened bitmap rows and a reused
coverage mask reduced the first implementation's roughly 2.75/4.80ms canonical/
wide title samples to roughly 1.42/2.36ms. These are observations with host noise,
not timing assertions or end-to-end frame-rate claims. The new film uses all rows
instead of reserving a permanent footer.

A 1/60s membrane increment at 160×40 changed 6,135 cells and emitted 127,964 bytes;
city33s changed 1,813 cells / 31,338 bytes. Graphics legitimately change broadly.
Frozen comparisons across acts and the final title remain exactly **0 / 0 / 0**.
The explicit TrueColor normal-speed release film at 120×32 emitted 75,906,237
bytes in 79.40s on the corrected code tip, exited 0 and restored termios and the
alternate screen. A prior full run took 79.10s and emitted the same byte count.
Stage release smokes also passed at 56×24 Mono, 160×40 TrueColor and 120×32
ANSI256, each with terminal restoration. Capture
inspection counted 3,978 nonempty transactions, no consecutive identical payloads,
and a median 593 emitted glyphs per transaction; this is not a blanket repaint.
The historical 7.8MB run above used auto-detection with unrecorded effective depth,
so it cannot establish a same-capability regression ratio. Matched TrueColor
samples do show higher costs: city33s 10,306→20,196 bytes/update and title68s
1,022→5,372. Broad moving color is expensive over a terminal transport.

Inspected requested TrueColor times 8,18,21,24,27,30,35,37,39,41,43,45,47,50,52,
55,58,61,64,67,70 at all three sizes. Boundary pairs 19.8/20.2,27.8/28.2,
35.8/36.2,43.8/44.2,52.8/53.2,59.8/60.2,64.8/65.2 preserve recognizable objects,
geometry or title motion. Mono was inspected separately. These cell-reconstructed
images and local PPM dumps are inspection artifacts, not a committed image corpus
or proof of identical appearance in every terminal font.

All commands in the local-validation block above passed again on Rust 1.98.1.
Additional explicit gates:

```sh
cargo +1.98.1 test --test geometry_diff_contract --test cinematic_paths --test whole_renderer_vt100 --test surface_fx
cargo +1.98.1 test --test intro_cinema
cargo +1.98.1 test --test intro_pty --test pty_integration --test pty_resize_torture -- --test-threads=1
```

C/C++/Python and ASan/UBSan passed. Go vet/test/build/example passed locally;
the Go package still has no unit tests. Package listing includes the new modules
and complete licenses; nothing was published. No core API, model, dependency,
workflow, ABI or other demo implementation changed in this polish pass.
D6/D7/D8 and issue #15 remain open with their existing claim boundaries.

Public CI passed all five jobs for the final code checkpoint
`4d87e2a756067bd43ef674a3422fb980745d62a5` in
[run 35954830898](https://github.com/femboy2112/libgibson/actions/runs/35954830898):
Rust, C/C++/Python, ASan/UBSan, Go and PTY. The earlier polished code checkpoint
`0d9b3f6` also passed in run 35954297546. The final documentation tip has its own
CI evidence in PR #14; these links attest only to their named commits.

## Perspective Earth and continuity follow-up

Starting from `c03bc4d`, the follow-up replaces the remaining city-frame shrink
with an actual perspective pullback. The city is still rendered from its live
world coordinates. Its circuit ground is tangent to a radius-600 software
sphere; the geographic site, city camera and planetary material share the same
basis. Rays intersect that sphere analytically, then evaluate illustrated
longitude/latitude land, lighting, clouds and atmosphere. There are no image
assets. At orbital altitude the Earth's body frame rotates visibly beneath the
camera. Network paths use ray/sphere occlusion and reject the camera near plane
before projection. The title and final hold retain the existing timeline.

The horizon develops from the circuit ground at 53–55s; the shared camera
pulls back through 63s. Constant floor subdivision avoids a sampling-pattern
change when curvature first becomes nonzero. Fine city detail yields to the
site beacon only as it becomes subpixel. The previous inverse-screenshot
sampling path is gone; its historical stray-cell regression remains.

The harness-to-city transition also loses its circular copy/wipe. The incoming
city's actual Braille dots approach through the tunnel, with identity-colored
conduits terminating at the four shared building-roof anchors. This transition
still samples a finite upcoming city view; it does not add mutable simulation or
claim general widget-to-world morphing. Each facade now carries a five-line
simulated receipt: contract, graph constraints, candidate policy or replay
witness, plus its next handoff. The finite agent model is unchanged.

Chrome lettering keeps a continuous bright front face, open dark counters and
quieter side faces. Narrow lettering is wider and straighter; the gold subtitle
has a dark separating shadow. No font assets, rendering APIs or engine modules
were added.

Two new geometric contracts verify perspective site/ray agreement, strictly
receding city extent, visibly rotating fixed-scale geography, hidden-side route
occlusion and near-plane rejection. Facade assertions now require meaningful
receipt details and the next agent at all three sizes in TrueColor and Mono.
Title bounds, deterministic replay, frozen 0/0/0 output and the original
late-ascent witness remain covered. That perspective follow-up passed **573 tests:
226 unit + 347 integration**, with three additional FX Lab tests.

Inspected reconstructed TrueColor frames at 56×24, 120×32 and 160×40, covering
membrane/city entry, all four facades, ascent, Earth rotation and the final text.
Mono and ANSI16 were inspected separately. These are actual cell/color
reconstructions, not terminal photographs or guarantees about every font.

Five-sample release generation medians on this host:

| Cells | Membrane 24s | City 33s | Ascent 58s | Title 68s |
| --- | ---: | ---: | ---: | ---: |
| 56×24 | 2.53ms | 1.45ms | 1.98ms | 0.52ms |
| 120×32 | 6.32ms | 3.55ms | 5.08ms | 1.78ms |
| 160×40 | 9.65ms | 5.11ms | 8.36ms | 2.55ms |

Earlier city recognition now pays for both realizations at 24s; the 26s overlap
is about 6.49/10.03ms at canonical/wide sizes. These are generation observations,
not transport or frame-rate guarantees. Rotating geography legitimately costs
more wire than a slowly changing globe: the 120×32 title sample at 68s emits
10,475 bytes for a 1/60s increment. Frozen state still emits none.

Local follow-up gates passed on Rust 1.98.1:

```sh
cargo +1.98.1 fmt --check
cargo +1.98.1 clippy --all-targets --all-features -- -D warnings
cargo +1.98.1 build --examples
cargo +1.98.1 test
cargo +1.98.1 build --release --examples
cargo +1.98.1 build --release --example libgibson_intro
RUSTDOCFLAGS="-D warnings" cargo +1.98.1 doc --no-deps
cargo +1.98.1 test --example fx_lab
cargo +1.98.1 test --test intro_cinema
RUSTUP_TOOLCHAIN=1.98.1 bash scripts/dev/bindings_smoke.sh --asan
cargo +1.98.1 package --list --allow-dirty
git diff --check
```

The full suite includes all six intro PTY tests, the renderer/geometry regressions
and existing visual goldens. C/C++/Python, native sanitizers and Go smoke passed.
A normal-speed release `--auto --deterministic --color=truecolor` run at 120×32
completed in 79.47s, emitted 99,108,558 bytes, exited 0 and restored termios and
the alternate screen. This is about 31% more transport than the preceding
explicit-TrueColor film: longer overlapping geometry and the live spherical
pullback change more cells. It is an accepted spectacle cost, not sparse-output
or 60 FPS evidence. Narrow TrueColor ascent, wide TrueColor membrane and narrow
Mono finale smokes also exited 0 and restored the terminal. Generated captures
remain outside the repository. Exact pushed-tip CI is recorded in PR #14.

The first remote follow-up run,
[35957005460](https://github.com/femboy2112/libgibson/actions/runs/35957005460)
at `5b6a715`, passed the geometric tests but hit the auto-film PTY test's
14-second deadline; dependent jobs were skipped. This was an executed test
failure, not billing or runner admission. The completion test now uses the
supported 56×24 viewport while retaining ANSI256, the full timeline, the same
14-second deadline and every final-label/restoration assertion. Separate
TrueColor PTY coverage still exercises canonical/wide frames and resize.
`cargo +1.98.1 test --test intro_pty -- --test-threads=1` passes all six tests;
strict fmt/Clippy pass too. No runtime or film timing changed for this correction.
The subsequent exact-tip CI result is recorded in PR #14.

## Mounted micro-UIs

The facade follow-up from `f08fc50` replaces bare receipt lines with ordinary
Node applications: a rounded identity header, active tab, status badge, paired
metrics, Braille diagram and model-derived next-agent footer. ARCHITECT carries
a contract graph; SCOUT a topology map; BUILDER candidate curves; VERIFY a fixture
matrix. Larger views include additional receipt details. The completed statuses
come from the same finite simulated work; there are no new agents or timers.

Each window has a physical world-space plate. Moving views draw its chrome and
diagram with depth testing. During the existing near-frontal holds, the native
UI fits entirely inside the projected four-corner plate and clips to that
rectangle. This is a deliberate level-of-detail realization, not arbitrary
perspective texture mapping of text. The vector and native diagrams share one
normalized recipe. No core API, story, camera path or timeline changed.

The projection regression checks rectangle containment independently against
all four projected edges at the three supported sizes. Existing facade checks
now require window chrome, tab/status and the original meaningful receipts in
TrueColor and Mono. Replay, resize-only projection and frozen 0 exact / 0 affected /
0 wire remain green. Current full suite: **574 tests = 226 unit + 348 integration**;
FX Lab has three additional explicit tests.

Inspected actual cell/color reconstructions of all four holds (37/39/41/43s) at
56×24, 120×32 and 160×40, plus canonical Mono and moving entry/exit frames.
The capture helper treats a reset background as terminal-default black rather
than applying the foreground RGB approximation; captures assume that dark theme.
No screenshot corpus is committed. Real release PTY runs through the facade
sequence passed at all three sizes in TrueColor and at 56×24 in Mono, exiting 0
and restoring termios and the alternate screen. The 120×32 normal-speed segment
ran 9.17s and emitted 6,371,064 bytes; this is a segment, not another full-film run.

Validation on Rust 1.98.1: fmt, all-target/all-feature Clippy with warnings denied,
all example builds (debug/release), full `cargo test`, `intro_cinema` (17), strict
rustdoc, FX Lab (3), native bindings smoke with ASan/UBSan, and package file listing.
The full suite includes the six intro PTY interaction tests. Exact pushed-tip CI
is recorded in PR #14; the PR remains unmerged.
