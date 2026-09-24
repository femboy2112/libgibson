# Final pre-main hardening validation

Classification: **READY FOR MAIN** on the locally tested Linux x86_64 scope.
Remote CI remains **BLOCKED / ENVIRONMENTAL**. No merge was performed.

## Repository and scope

- Branch: `codex/acid-vs-crash-cinematic`.
- Verified starting HEAD: `08c6e3be98fc4d7ad1d77f83b7ec29eecec67a22`.
- Clean starting worktree; 29 commits ahead of main, zero behind.
- One final hardening commit; its resulting SHA is recorded in the delivery report.
- No cinematic, story, planner, action, outcome, or core architecture changes.

Exact changed files:

- `src/raster_fx.rs`: clamp resize dimensions before equality comparison; document
  effective-size identity and maximum feedback allocation.
- `tests/raster_fx.rs`: one low-memory regression covering both axes.
- `src/raster.rs`: documentation only, allocation ceiling and blend terminology.
- `src/raster3d.rs`: documentation only, continuous projection coordinates.
- `DESIGN.md`: current test count, precise resize semantics, memory ceiling.
- `README.md`: current test count and unit/integration split.
- `docs/acid-vs-crash-hardening-validation.md`: this validation record.

## Fix and decisive regression

`FeedbackBuffer::resize` now applies the same public `MAX_RASTER_DIMENSION`
constant and `min` rule as `RgbRaster::new` before comparing sizes. The limit
remains 2048 per axis. Equivalent effective-size requests return without
allocation or resetting history; actual size changes retain existing reset behavior.

`feedback_resize_preserves_history_when_clamped_dimensions_match` exercises
`(3000, 1)` and `(1, 3000)` independently. Each case starts small, resizes,
emits light, and decays for an irregular positive step. It then repeats the
oversized request twice and supplies the equivalent clamped size. Full
`FeedbackBuffer` equality checks output RGB, floating energy, half-life, and
update counter. Each live buffer is only 2,048 pixels; no 2048² copies are needed.

The exact command was run before and after the production fix:

```sh
cargo test --test raster_fx feedback_resize_preserves_history_when_clamped_dimensions_match
```

- Before: exit 101, failed because resize erased nonblack history and reset updates.
- After: exit 0, one passed; full buffer state preserved.

The new test changes the final total from the verified baseline **535
(226 unit + 309 integration)** to **536 (226 unit + 310 integration)**.
DESIGN's stale current 515 claim and README's current 535 claims now use 536.
Historical checkpoint counts remain unchanged.

`RgbRaster::blend` is documented as per-channel interpolation of stored 8-bit
RGB, without implying linear-light interpolation. `Camera::project` documents
continuous framebuffer-edge coordinates: exact right/bottom edges may map to
width/height; discrete clipping/indexing remains rasterization's responsibility.
Neither behavior changed.

The allocation ceiling is explicitly not a recommended operating size.
2048² feedback pixels require 96 MiB of floating energy plus 12 MiB of RGB
output, about 108 MiB excluding allocator overhead. Normal terminal rasters
are orders of magnitude smaller.

## Narrow core review

Reviewed FeedbackBuffer, RgbRaster, Camera, Rasterizer, RasterFxWorkspace,
the line-clipping helper, and their existing tests. A separate read-only agent
review covered the same narrow correctness scope; this is source review, not
an independent implementation or proof of absence of bugs.

No additional concrete blocker was found. Dimensions bound indexing/products;
clipping bounds pixel loops; glow has a maximum 7×7 kernel; invalid floating
inputs are rejected or ignored and hostile finite math uses f64; depth storage
tracks dimensions; counters saturate. Timeline mutation stays in explicit update
methods, not accessors/projection. No speculative changes were made.

## Commands and results

Environment: Linux x86_64; rustc `1.97.0-nightly (a5c825cd8 2026-04-14)`;
cargo `1.97.0-nightly (eb94155a9 2026-04-09)`.

Before editing, ran `cargo fmt --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and
`cargo build --release`, followed by every individual targeted test command
below from `raster3d` through `safety_api`. All 20 commands exited 0. Baseline
full suite: 535 passed, zero failures, zero ignored.

Final commands (all completed; no inferred results):

| Command | Result |
| --- | --- |
| `cargo fmt --check` | PASS, exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS, exit 0 |
| `cargo test` | PASS, exit 0; 536 passed |
| `cargo build --release` | PASS, exit 0 |
| `cargo build --examples` | PASS, exit 0 |
| `cargo test --test raster3d` | PASS, exit 0; 15 passed |
| `cargo test --test raster_fx` | PASS, exit 0; 18 passed |
| `cargo test --test acid_graphics` | PASS, exit 0; 10 passed |
| `cargo test --test acid_architecture` | PASS, exit 0; 3 passed |
| `cargo test --test acid_presentation` | PASS, exit 0; 12 passed |
| `cargo test --test acid_battle` | PASS, exit 0; 18 passed |
| `cargo test --test acid_story` | PASS, exit 0; 14 passed |
| `cargo test --test acid_render` | PASS, exit 0; 8 passed |
| `cargo test --test acid_battlefield` | PASS, exit 0; 4 passed |
| `cargo test --test acid_battlefield_goldens` | PASS, exit 0; 1 passed |
| `cargo test --test scene_cinematic` | PASS, exit 0; 14 passed |
| `cargo test --test story_reactions` | PASS, exit 0; 15 passed |
| `cargo test --test surface_fx` | PASS, exit 0; 21 passed |
| `cargo test --test whole_renderer_vt100` | PASS, exit 0; 21 passed |
| `cargo test --test pty_resize_torture` | PASS, exit 0; 3 passed |
| `cargo test --test safety_api` | PASS, exit 0; 8 passed |
| `cargo test --test pty_demos acid_ -- --test-threads=1` | PASS, exit 0; 16 passed |
| `cargo test --test pty_resize_torture -- --test-threads=1` | PASS, exit 0; 3 passed |
| `bash scripts/dev/bindings_smoke.sh --asan` | PASS, exit 0 |
| `cargo build --release --example acid_vs_crash` | PASS, exit 0 |

Final full suite: **536 passed, zero failures, zero ignored**. Targeted runs
repeat tests from that total; they do not increase the count. `git diff --check`
also passed.

Bindings: C, C++, Python smoke paths passed. C/C++ ASan and UBSan passed;
LeakSanitizer is disabled by the existing script. Go compiler unavailable:
**Go UNVERIFIED**. Bindings were unchanged.

## Replay, static frames, resize

Existing passing tests reconfirmed:

- Full encounter replay compares BattleGraph/planner, exact input trace,
  StoryDirector trace/current beat, facts, mounted bundles, presentation, and
  VisualHistory (including ShotHistory). Irregular-input presentation tests
  additionally compare each shot/camera and the final realized frame.
- Frozen cinematic frame: exact changed cells = 0, affected cells = 0,
  emitted bytes = 0. Paint and resize do not advance visual history.
- Resize reprojects history across dimensions and matches fresh replay output.
  Real PTY resizing retains typed input and restores terminal state, including
  cinematic, feedback, and takeover scenes.

## Release cinematic smoke

Actual PTY command at 120×32, with TrueColor environment:

```sh
cargo run --release --example acid_vs_crash -- --auto --deterministic
```

Completed without input or speed override: exit 0, normal ending, terminal
settings restored. Host elapsed time was about 105 seconds, including cargo
startup/build-lock overhead; this is not a frame-rate claim.

The following six commands also exited 0 and restored terminal state. Each
rendered the command prompt; PTY dimensions are listed in matching order:
56×24, 120×32, 160×40, 56×24, 120×32, 160×40.

```sh
target/release/examples/acid_vs_crash --seconds=0.6 --deterministic --color=truecolor --manual
target/release/examples/acid_vs_crash --seconds=0.6 --deterministic --color=truecolor --debug-shot
target/release/examples/acid_vs_crash --seconds=0.6 --deterministic --color=truecolor --debug-raster
target/release/examples/acid_vs_crash --seconds=0.6 --deterministic --color=truecolor --presentation=legacy
target/release/examples/acid_vs_crash --seconds=0.6 --deterministic --color=truecolor --visual=flat
target/release/examples/acid_vs_crash --seconds=0.6 --deterministic --color=truecolor --visual=cyber
```

Additional PTY frames were captured with:

```sh
target/release/examples/acid_vs_crash --deterministic --stage=first-breach --freeze-at=24 --color=truecolor --manual
target/release/examples/acid_vs_crash --deterministic --stage=display-intrusion --freeze-at=24 --color=truecolor --manual
target/release/examples/acid_vs_crash --deterministic --stage=climax --freeze-at=24 --color=truecolor --manual
```

Dimensions respectively 56×24, 120×32, 160×40. All three exited 0 after PTY
Ctrl-C and restored terminal settings. Agent inspection of reconstructed
TrueColor frames found no visual regression: geometry/portal remained visible
and the bottom command island was readable. This is a narrow regression check,
not a new independent human aesthetic acceptance claim.

Run-local logs, PTY bytes, screenshots, command receipts and process audit are
under `/tmp/libgibson-hardening/`; generated images are not committed.

## Remote CI and remaining boundaries

Freshly inspected [run 35938473507](https://github.com/femboy2112/libgibson/actions/runs/35938473507),
which matches starting HEAD. Rust failed with zero executed steps; all dependent
jobs were skipped. The annotation says the job was not started because recent
account payments failed or the spending limit needs increasing.

**REMOTE CI: BLOCKED / ENVIRONMENTAL.** This is neither green CI nor a code
failure. Local gates supply the validation in this report. Cross-platform and
cross-terminal limits from earlier reports remain; experimental APIs remain
experimental. No new blocker was found, and no automatic merge is authorized.
