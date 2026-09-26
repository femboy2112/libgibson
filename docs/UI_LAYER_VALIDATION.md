# Experimental UI layer — validation record

This records the experimental UI implementation branch, not a release or a
merge decision. Architecture and usage are in [UI_LAYER.md](UI_LAYER.md).

## Provenance and baseline

- Starting main: `936c164e0bfcd9614e9a167330032c7c8233931d`.
- Baseline run: 2026-09-26, isolated detached worktree at that SHA, before UI
  edits; Linux x86_64, kernel `6.18.44`.
- Stable compiler: `rustc 1.98.1 (48a229cea 2026-09-01)`;
  Cargo `1.98.1 (797e8a9bc 2026-08-05)`.
- Declared library MSRV: 1.85; verified with
  `rustc 1.85.1 (4eb161250 2025-03-15)`.
- Package remains 0.1.1; ABI remains 1. No release, wrapper expansion or new
  dependency is part of this branch.

| Baseline command | Observed outcome |
| --- | --- |
| `cargo fmt --check` | Exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0 |
| `cargo build --examples` | Exit 0 |
| `cargo test` | Exit 0; 661 passed, 0 failed, 1 ignored across 46 result groups |
| `cargo test --example fx_lab` | Exit 0; 3 passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | Exit 0 |
| `cargo build --release` | Exit 0 |
| `cargo +1.85.1 check --locked --lib` | Exit 0 |
| `cargo +1.85.1 test --locked --lib` | Exit 0; 247 passed |
| `MSRV_TOOLCHAIN=+1.85.1 scripts/release/msrv-consumer.sh` | Exit 0; fallback and default fresh resolutions build |
| `scripts/release/check-versions.sh` | Exit 0; package/ABI/license payloads agree |
| `scripts/release/check-abi.sh` | Exit 0; 52 baseline and 52 built symbols |
| `cargo package --list --locked` | Exit 0 |

The baseline 661 is 247 library units, 413 integration tests and one doctest;
the three `fx_lab` tests are separate. The ignored
`context_coincident_lone_key_delivery_acceptance` explicitly targets known
upstream crossterm issue #15. Its presence predates this branch; normal suite
success does not claim the collision has been fixed. The ignored test was not
re-run in this baseline pass.

Run-local logs were preserved during execution under `logs/baseline-*.log` in
the task workspace, outside the repository. They are not committed assets or
dependencies of a clean checkout. No source was changed to obtain the baseline.

## Branch validation

Final gates were executed on Linux x86_64 with Rust 1.98.1. The executable
source, tests and examples are checkpointed at
`a2d58ebf3e550d52d5b64fdd4c175c71650bcacf`; the final documentation commit also
repairs four rustdoc links without changing executable behavior. No remote CI
result is claimed for this branch.

| Command / probe | Observed branch result |
| --- | --- |
| `cargo fmt --check` | Exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0 |
| `cargo build --examples` | Exit 0; all existing and four new examples build |
| `cargo test` | Exit 0; **697 passed, 0 failed, 1 known ignored** |
| `cargo test --test ui_layer --test ui_goldens` | Exit 0; **26 passed** (22 behavior/escape tests + 4 visual suites) |
| `cargo test --example fx_lab --example skin_gallery --example polished_agent_ui` | Exit 0; **9 passed** (3 existing + 6 new) |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | Exit 0 after repairing broken/ambiguous intra-doc links |
| `cargo build --release` | Exit 0 |
| `cargo +1.85.1 check --locked --lib` | Exit 0 |
| `cargo +1.85.1 test --locked --lib` | Exit 0; **257 passed** |
| `MSRV_TOOLCHAIN=+1.85.1 scripts/release/msrv-consumer.sh` | Exit 0; both fallback and default fresh resolutions build |
| `scripts/release/check-versions.sh` | Exit 0; package 0.1.1, ABI 1, license payloads agree |
| `scripts/release/check-abi.sh` | Exit 0; **52 baseline / 52 built symbols** |
| `cargo package --list --locked --allow-dirty` | Exit 0; all nine UI source files included |
| `cargo package --locked --allow-dirty` | Exit 0; packaged crate builds |
| `polished_agent_ui --auto --no-motion` | Exit 0; approved simulated session reaches 2 changes and 1 Unicode follow-up, with no real file changes |
| `git diff --check` | Exit 0 |

The main suite is **257 library units + 439 integration tests + 1 doctest**.
This adds 10 library units and 26 integration tests to the 661 baseline. Six new
example tests are counted separately. No baseline test was weakened, removed,
newly ignored, or reclassified. The known collision acceptance remains ignored.

The visual suite covers **48 combinations** of 3 skins, 4 terminal sizes and 4
color depths, plus tiny/zero bounds and ASCII realization. Three small checked-in
Mono text goldens distinguish structure without relying on color. The gallery
example test checks 27 visible-control cases across skins, pages and geometries.
An additional final capture sweep checked all three skins at 80×24, 120×40 and
32×16 in ANSI16/Mono (18 captures); Send and skin controls remained visible.

TrueColor and Mono 80×24 captures of all three skins were rasterized from the
actual headless ANSI stream and visually inspected. The permission view was
also inspected across the three skins. These are deterministic virtual-terminal
captures, not claims about a particular real terminal font or non-Linux system.
Temporary raster images are not committed; `--dump-ansi` reproduces the source
frames. The original substrate PTY suites pass in the full test run.

Architecture review confirmed that only `src/lib.rs` and new `src/ui/` code
change the library; renderer, layout, NodeKind, Theme, Context/session, C ABI,
wrappers, dependencies and the original polished-agent example are unchanged.
Review used parallel same-model agents plus direct root checks, not independent
human witnesses. The branch is ready for independent review, not certified for
production or automatically approved for merge.

## Defects found during implementation review

The review produced source corrections before the final validation gate:

- Leaf semantic children could have contributed actions while never rendering;
  compilation now rejects them, including children appended to a raw element.
  Viewports reject multiple direct children rather than silently using only one.
- Overlay placement initially used terminal coordinates and an all-absolute
  stack, losing a nested base's intrinsic size. An in-flow base and absolute
  overlay children now use the containing box; modal constraints are resolved
  by existing Taffy layout. Screen-level overlays explicitly get screen height.
- Density overrides initially bypassed custom spacing tokens. Lowering now
  resolves the skin's actual density tokens, with explicit builder overrides
  taking precedence.
- Explicit semantic tone now affects heading/code styling and container chrome.
- Running motion now honors changes to the user's motion preference; disabling
  motion cannot leave an old active plan behind.
- Unchanged focus order reuses its FocusRing; controlled editor boundary keys
  do not fall through to background shortcuts. Borrowed headless App execution
  settles once and leaves the caller's Context usable.
- Compact Vapor95 omits the extra shadow row to preserve narrow control space.
- Relative row growth now uses equal weighted bases unless width is explicit;
  flexible root rows receive a definite available width, avoiding zero-width
  collapse. Responsive columns retain intrinsic widths.
- Nested and sibling modal focus memories are bounded by live scopes and survive
  covering, removal and reordering. Focused placeholder styles are skin-resolved.
- A local-modal test initially mistook the letter I in NEIGHBOR for the modal's
  Inside label. The oracle now locates the complete label and measures its cell
  position; it does not weaken the host-containment requirement.

These are finite implementation findings, not a claim that review found every
defect. The final command outcomes above and named regression tests must be
consulted together; source inspection alone is not execution evidence.

## Reproduce the visual proving grounds

```sh
cargo run --example ui_quickstart
cargo run --example ui_gallery
cargo run --example skin_gallery -- --skin=vapor95 --dump --width=80 --height=24
cargo run --example skin_gallery -- --skin=black-ice --dump --ansi16 --width=120 --height=40
cargo run --example skin_gallery -- --skin=swiss-signal --dump --mono --width=36 --height=18
cargo run --example skin_gallery -- --skin=vapor95 --dump-ansi --truecolor
cargo run --example polished_agent_ui -- --stage=permission --dump --skin=swiss-signal
cargo run --example polished_agent_ui -- --auto --no-motion
```

The two galleries share one semantic view. `--page=0|1|2` selects component
groups. `--dump` reconstructs the headless terminal byte stream as text;
`--dump-ansi` preserves its color/style bytes for visual rendering. A text
snapshot alone cannot certify color contrast or motion quality. Review both
structural assertions and representative rendered surfaces.

## Acceptance boundaries

The intended discriminators are unchanged-tree skin switching; structure in
Mono; fixed-time determinism and exact settled motion; focus retention through
keyed reflow/replacement; modal capture/restoration; controlled Unicode editing;
disabled routing; ordinary Node/Surface/custom-canvas/Scene escape paths; and
bounded retention after churn. Representative sizes include 80×24, 120×40 and a
narrow constrained viewport. Passing one large TrueColor screenshot is not
equivalent to those checks.

Removal is deliberately immediate: Exit is observable metadata, not a retained
ghost implementation. Toast lifetime, selection and editor contents are owned
by the application. Existing renderer/session guarantees and the known resize
collision are inherited unchanged.

The original `polished_agent` remains intact. Any comparison with
`polished_agent_ui` must distinguish preserved interaction outcomes from the
original's story/art direction. Source line counts alone do not establish human
usability. No independent user study, new platform certification or sustained
performance certification is claimed.
