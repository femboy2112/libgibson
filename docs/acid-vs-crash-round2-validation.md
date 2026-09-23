# Acid vs Crash, round II: evidence record

Starting branch: `codex/acid-vs-crash-cinematic`.
Starting local and remote HEAD: `b6a353e1c76fd0b851846087c7630b2e5a6c263f`.
Working tree clean. Remote main remains
`1b4358e91df108cdf0e0dbb92e886f010a8b7eb9`. No merge authorized.

## Baseline (2026-09-23)

All commands exited 0 before modifications:

```
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
cargo test --test surface_fx
cargo test --test scene_cinematic
cargo test --test story_reactions
cargo test --test acid_story
cargo test --test acid_render
cargo test --test pty_demos acid_ -- --test-threads=1
cargo test --test visual_goldens
```

Full baseline: 430 tests, comprising 226 library unit + 204 integration.
Local command receipts: `/tmp/libgibson-acid-round2/baseline-0.log` through
`baseline-10.log` (run-local, not portable artifacts).

## Acceptance probes

The rival explanations are an authored beat sequence versus a topology-driven
opponent. Shared-prefix counterfactual inputs must produce different legal
routes, influence, facts and realized screens. Cutting every route must prevent
new influence at the unreachable target. Full encounter replay must reproduce
continuous graph state, planner memory, StoryDirector, bundles and presentation.
Core mask tests independently check cell/glyph boundaries; whole-renderer VT100
and actual PTY inputs check the externally consumed terminal result.

Remote Actions were reported billing-blocked at intake; no remote green claim.
New cinematic APIs remain experimental and Rust-only. All battle semantics
remain local fiction, with no networking or host command execution.
