# Terminal software graphics — validation record

2026-09-23. **READY FOR REVIEW**, not an automatic main merge.

Starting SHA: `d91747fce3f0b40488172597dea4c778adc45765`.
Branch: `codex/acid-vs-crash-cinematic`.
Implementation/test checkpoint: `bf14c43` (the final documentation commit follows).
Main remains `1b4358e91df108cdf0e0dbb92e886f010a8b7eb9`.
Work was committed and pushed in separate baseline, substrate, lab, cinematic,
PTY and damage-evidence changes. The final response supplies the ending SHA.

## Baseline and provenance

The starting branch, SHA, clean worktree and tracked origin matched the request.
All four baseline gates passed: fmt, all-target/all-feature strict Clippy,
`cargo test`, release build. Baseline was **469 = 226 unit + 243 integration**.
The requested Acid, SurfaceFx, scene, reaction, golden and whole-renderer targets
also passed before edits. Logs are run-local in `/tmp/libgibson-rgb/`.

The evidence has three instruments with different failure modes: independent
ray/plane depth probes against the barycentric implementation, deterministic
RGB/world/frame tests, and real PTY output interpreted by VT100/visual capture.
They share this source tree and execution environment; they are not independent
human aesthetic judgments. A numerical color-diversity test is not proof that
an audience will mistake the result for an image protocol.

## New visual primitives

`src/raster.rs` is an opaque RGB framebuffer with set/get/clear/blend, clipped
line/disc drawing, half-block Surface conversion, Mono ordered Braille density,
and optional PPM output. Width/height mean **pixels**. W×H terminal cells use
W×2H samples, foreground above/background below. Software color mixing is done
before cell realization; terminal cells gain no alpha. Allocation is explicitly
bounded at 2048 per axis, and raster walks are bounded by dimensions.

`src/raster3d.rs` provides indexed filled meshes (cube, prism, octahedron), a
look-at camera, six-plane frustum clipping, pixel-center barycentric filling
and an actual Z buffer. Interpolation uses reciprocal camera depth; triangles
and emissive lines share the buffer. Exact coplanar ties have a stable color
tie-break. Flat Lambert lighting adds ambient/diffuse/emissive contributions;
linear camera-depth fog supplies distance cues. Invalid/nonfinite and degenerate
inputs are rejected. Submitted/drawn triangles and Z tests are exposed.

`src/raster_fx.rs` provides an ordered chain of glow, chromatic split, sine warp,
vignette and scanline operations. Empty is identity; A then B need not equal
B then A. A workspace reuses one scratch source across the chain. Glow caps at
radius three. Radial glow, metaballs, vortex, ring and palette functions supply
finite-safe fields without a shader language.

`FeedbackBuffer` owns explicit floating RGB history: decay by supplied dt and
half-life, then add input emission and saturate when materializing bytes. Zero
dt is identity; reset/resize clear it. Equal ordered update inputs replay exactly;
arbitrary timestep repartitioning is not claimed equivalent. These stateful
buffers are distinct from pure Scene presentation effects.

These additions are generic and Rust-only. No FFI or cell renderer API changed.
No image protocol, networking, shell execution or security tooling was added.

## Battle realization and choreography

The world reducer, Acid planner, tactical rules and Story graph are unchanged.
The new demo-local `cyber.rs` projects their existing state:

| Semantic source | Graphical consequence |
| --- | --- |
| Node integrity | Height of shaded stepped structures |
| Signed node influence | Cyan/magenta material mixture and spatial field sources |
| Connected topology and Acid path | Luminous depth-tested routes and forward magenta packets |
| Earned trace | Cyan feedback pulses moving backward on the legal path |
| Isolation | Detached structures, severed path gaps, boundary sparks, removed shared-field source |
| Visible decoy | Mirrored alternate structure and its actual attached edges |
| DISPLAY possession/takeover | Growing orbital lattice, field contours, intermittent RGB distortion |
| Existing entity SurfaceFx | Corrupted header and an ordinary session fragment over the graphical world |
| Outcome | RGB fragment formation or reassembly into the scarred ordinary UI |

The quiet machine keeps its ordinary layout with restrained local RGB light.
A genuine foothold starts a two-second dive. Recorded entry time avoids jumping
back to the flat view at each new broad beat. A deterministic cell dissolve
reveals the 3D projection. This is cinematic choreography, not a general widget
morph or a second simulation. The command island remains ordinary crisp input.
Narrow mode uses one focal graphic, concise HUD and two action rows; larger
terminals dedicate almost the whole screen to graphics.

`--visual=auto` is default; `flat` retains the old realization and `cyber` forces
the graphical one. `--debug-raster`/`--debug-renderer` expose raster work counters.
Crash still fights automatically; `--manual` and interactive interventions keep
their prior semantics. `--auto` waits for ending reconstruction before exit.
The 60 FPS ceiling is a scheduler setting, not a universal throughput claim.

Three inspected hero targets are (1) the borderless shaded core in FX Lab,
(2) crossing cyan/magenta feedback beams, and (3) the full-screen battle's
DISPLAY lattice/influence field. The Acid win adds scattered bitmap fragments
assembling into ACID BURN over the still-present geometry. The formation is
RGB fragments, not a generic font engine or perspective mesh-text system.

Run-local TrueColor PTY images were inspected at 160×40; the RGB PPM path also
allows inspection before terminal realization. Captures are not committed image
corpora. “Terminal developers mistake this for Sixel” is **UNVERIFIED** pending
independent human viewing. Optional SDF raymarching was deliberately not added.

## Replay, goldens and capability behavior

VisualHistory keeps at most 48 world-space light samples, captured at semantic
update boundaries. Each paint reconstructs bounded feedback using those inputs
and the current camera. Thus frozen paint is pure and resize reprojects the
same history. Replay reconstructs history plus BattleGraph, planner, facts,
beat sequence, bundles and final frame. Recorded aftermath time continues
reassembly after terminal beats without changing the resolved semantic world.
Tests include irregular dt, multiple commands, isolation/decoy counterfactuals,
and default auto entering cyberspace and returning without input.

Original text/style goldens are preserved without snapshot changes, explicitly
selecting the flat view. New RGB probes check color diversity, highlights,
actual geometry/Z work, signed-control response and graphical counterfactuals.
At 120×32, the measured hero buffers contained 1231 / 1240 / 1816 distinct RGB
colors (breach / trace / takeover); this proves nontrivial shading, not taste.

TrueColor is the intended spectacle. ANSI256/ANSI16 use the existing central
compiler quantization. Mono uses luminance dithered Braille and explicit Acid,
contested and isolated glyph grammar. All capability/size combinations were
smoked; universal emulator/font equivalence is not claimed.

## Bugs and corrections

No core cell renderer bug was found or changed. New graphics code received
finite-coordinate, perspective depth, clipping and public-raster-resize probes.
Integration exposed three presentation/lifecycle defects which were fixed:

- Repeated bright emissions washed the battle trails white; timestep-scaled
  emission retains the cyan/magenta distinction.
- Terminal story beats stopped presentation time, preventing reassembly;
  recorded visual aftermath now continues while final world state stays fixed.
- The compact cyber header omitted the final outcome; it now names the result.

FX Lab also had an existing input defect: deterministic/auto mode ignored its
events, including Ctrl-C. Both paths now process input, with a real PTY test.
One run-local screenshot helper initially waited for child exit without draining
stdout and hit PTY backpressure. The helper was corrected; the product lifecycle
checks use readers that drain continuously. Final hero captures all exited zero
and restored termios.

## Damage and performance observations

160×40, one 17ms recorded update, TrueColor renderer:

| Frame | Exact delta | Affected footprint | Wire bytes | Generation, observed |
| --- | ---: | ---: | ---: | ---: |
| First breach | 124 | 124 | 4423 | 10350 μs |
| Trace | 122 | 122 | 4425 | 4771 μs |
| Takeover | 412 | 412 | 13492 | 6180 μs |
| Repeated frozen frame | 0 | 0 | 0 | — |

These are single release-test observations on this host, with other work running,
not timing assertions or a sustained-FPS benchmark. Generation includes Node
construction and feedback reconstruction; terminal write latency is separate.
Broad raster change is expected and permitted. The code does not pretend this
is sparse panel-local animation. The existing flat-view locality tests remain.

## Exact validation commands and results

All commands below exited zero on the final implementation unless explicitly
marked environmental/unavailable. `cargo test` reports **515 tests: 226 unit +
289 integration**, all passing. Forty-six tests were added (15 depth/raster,
17 raster-FX, 10 Acid graphics, four PTY/resize). Three FX Lab generator tests
run separately and also pass.

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
cargo build --examples
cargo test --example fx_lab
cargo test --test pty_demos acid_ -- --test-threads=1
cargo test --test pty_demos fx_lab_deterministic -- --test-threads=1
cargo test --test pty_resize_torture -- --test-threads=1
cargo test --test raster3d --test raster_fx --test surface_fx \
  --test scene_cinematic --test story_reactions --test acid_battle \
  --test acid_story --test acid_battlefield --test acid_render \
  --test acid_graphics --test acid_battlefield_goldens --test visual_goldens \
  --test whole_renderer_vt100 --test resize_torture \
  --test pty_resize_torture --test safety_api
DUMP_ACID_RGB=/tmp/libgibson-rgb/dumps \
  cargo test --release --test acid_graphics -- --nocapture
bash scripts/dev/bindings_smoke.sh --asan
python3 /tmp/libgibson-rgb/matrix.py
```

The 12 Acid PTY tests include manual counterplay, default autonomous play,
full auto completion, replay, final CUT LINK and Ctrl-C. New graphical cases
exercise TRACE/DECOY/replay and takeover agency. The new resize test drives
56×24, 80×24, 120×32 and 160×40 through breach, takeover, ending reconstruction
and FX Lab feedback, including real input after resizing and terminal restoration.
The filled-3D lab Ctrl-C regression passes. Existing demos, whole-renderer VT100,
safety and resize suites remain green. Existing snapshots were not regenerated.

The run-local matrix executed 96 actual `cargo run --quiet --example
acid_vs_crash -- ... --seconds=0.22` PTY starts/exits: default/manual/auto/
deterministic/flat/cyber × four sizes × mono/ansi16/ansi256/truecolor. Each exited
zero, showed the command prompt and restored termios. These are startup/exit
smokes; they are not 96 complete playthroughs. Full auto completion is separately
covered by the PTY suite. Additional TrueColor lab and battle captures were
inspected directly. C, C++, Python, and C/C++ ASan/UBSan smoke passed; Go was
skipped because the toolchain is absent.

GitHub run [35935164638](https://github.com/femboy2112/libgibson/actions/runs/35935164638)
at `6d8430e` has zero Rust job steps. Its check annotation says account payments
failed or spending limits must increase. Remote CI is **BLOCKED / ENVIRONMENTAL**,
not a code-test failure and not green.

## Claim limits

**EXPERIMENTAL:** RGB/raster3d/RasterFx and Scene cinematic APIs are Rust-only;
no immature C ABI was added. Exact visual replay is tested within the same
binary/platform, not promised as cross-platform floating-point serialization.

**PARTIALLY TESTED:** aesthetics, extended manual play, sustained frame delivery
on real terminal emulators, SSH/multiplexer behavior. Actual screenshot inspection
and PTYs support the local result; they cannot certify universal visual quality.

**UNVERIFIED:** independent audience “image protocol” impression, Windows/macOS,
Go bindings, remote CI execution. No optional raymarcher was implemented.

**READY FOR REVIEW.** Main was not merged or modified.
