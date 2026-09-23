# Terminal software graphics — work record

Starting commit: `d91747fce3f0b40488172597dea4c778adc45765`.
Branch: `codex/acid-vs-crash-cinematic`. Main is not a development target.

Baseline (2026-09-23): clean worktree; `cargo fmt --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and
`cargo build --release` all passed. 469 tests: 226 unit, 243 integration.
The requested acid_battle, acid_story, acid_battlefield, acid_render,
acid_battlefield_goldens, surface_fx, scene_cinematic, story_reactions,
visual_goldens and whole_renderer_vt100 targeted suites also passed.
Run-local command logs: `/tmp/libgibson-rgb/baseline-*.log`.

## Scope and acceptance

The remaining gap is visual realization, not adversary behavior. Build opaque
RGB software graphics before ordinary half-block Surface realization. Keep
cell SurfaceFx separate from RGB processing. Preserve the world reducer,
StoryDirector, exact replay, and terminal lifecycle. No image protocol or
network behavior belongs in this implementation.

Correctness requires depth-order probes, clipped hostile geometry, reproducible
feedback, full encounter replay and frozen zero-byte frames. Visual quality
requires inspecting RGB artifacts and terminal captures; passing numerical
tests does not establish the subjective “looks impossible” acceptance bar.

Status: implementation in progress; no final readiness claim yet.
