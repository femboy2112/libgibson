# Experimental UI layer — validation record

This records the experimental UI implementation branch, not a release or a
merge decision. Architecture and usage are in [UI_LAYER.md](UI_LAYER.md).

## Original main provenance and baseline

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

## Round I implementation validation

The Round I gates were executed on Linux x86_64 with Rust 1.98.1. The executable
source, tests and examples are checkpointed at
`a2d58ebf3e550d52d5b64fdd4c175c71650bcacf`; documentation checkpoint
`0bcc2f8d48b5e6b1ce18af4efc2f09187117ee97` also repairs four rustdoc links
without changing executable behavior. This is historical evidence for Round I,
not the outcome of later edits. No remote CI result was claimed for this checkpoint.

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

The Round I main suite was **257 library units + 439 integration tests + 1 doctest**.
This adds 10 library units and 26 integration tests to the 661 baseline. Six new
example tests are counted separately. No baseline test was weakened, removed,
newly ignored, or reclassified. The known collision acceptance remains ignored.

The Round I visual suite covered **36 combinations** of 3 skins, 3 terminal
sizes (80×24, 120×40, 32×16) and 4 color depths, plus tiny/zero bounds and ASCII
realization. The earlier claim of 48 combinations was an arithmetic/source-audit
error: there were only three sizes in the committed test. Three small checked-in
Mono text goldens distinguish structure without relying on color. The gallery
example test checks 27 visible-control cases across skins, pages and geometries.
An additional final capture sweep checked all three skins at 80×24, 120×40 and
32×16 in ANSI16/Mono (18 captures); Send and skin controls remained visible.

TrueColor and Mono 80×24 captures of all three skins were rasterized from the
actual headless ANSI stream and visually inspected. The permission view was
also inspected across the three skins. These are deterministic virtual-terminal
captures, not claims about a particular real terminal font or non-Linux system.
Temporary raster images are not committed; `--dump-ansi` reproduces the source
frames. The original substrate PTY suites passed in that recorded full test run;
the fresh Round II baseline below also records intermittent failures.

Architecture review confirmed that only `src/lib.rs` and new `src/ui/` code
change the library; renderer, layout, NodeKind, Theme, Context/session, C ABI,
wrappers, dependencies and the original polished-agent example are unchanged.
Review used parallel same-model agents plus direct root checks, not independent
human witnesses. That checkpoint was submitted for independent review, not
certified for production or automatically approved for merge.

## Round I defects found during implementation review

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

## Round II baseline and CI discovery

Round II began from clean branch `feat/high-level-ui-layer` at
`0bcc2f8d48b5e6b1ce18af4efc2f09187117ee97`. A separate detached worktree froze
that source while implementation continued elsewhere. Toolchain and host were
the Linux x86_64 / Rust 1.98.1 environment recorded above. No source edits,
golden regeneration, ignored-test changes or assertion weakening were used for
these baseline runs.

| Fresh baseline command | Observed outcome |
| --- | --- |
| `cargo fmt --check` | Exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0 |
| `cargo build --examples` | Exit 0 |
| `cargo test` | Exit 101; stopped after 30 result groups: 509 passed, 2 failed, 1 known ignored |
| `cargo test -- --test-threads=1` | Exit 101; stopped after 30 result groups: 510 passed, 1 failed, 1 known ignored |
| `cargo test --no-fail-fast` | Exit 0; all 48 result groups completed: **697 passed, 0 failed, 1 known ignored** |
| `cargo test --test ui_layer --test ui_goldens` | Exit 0; **26 passed** |
| `cargo test --example fx_lab --example skin_gallery --example polished_agent_ui` | Exit 0; **9 passed** |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | Exit 0 |

`--no-fail-fast` continues to later targets if a target fails; it does not turn a
failed test into a pass. That complete run happened to pass. It verifies the
documented 697 count and exercises suites skipped by the earlier stops, while
the earlier failures remain part of the evidence. Neither the 697 nor the
original-main 661 is a claim about the final Round II test count.

### Intermittent nonignored resize failures

The first fresh full run failed these existing tests in `pty_resize_torture`:

- `acid_graphical_and_feedback_resize_torture_restores_terminal`: input absent
  after a 160×40 resize (`crash > rr`, expected one additional `r`).
- `acid_cinematic_shots_resize_without_losing_input_or_terminal_state`: input
  absent in the trace stage at 120×32 (`crash > rrr`, expected one more `r`).

The full serial run also failed the cinematic test, in the stalemate stage at
120×32. A targeted serial repeat passed all three tests. Of four targeted default
parallel runs, the first failed the cinematic test at 80×24 and the next three
passed. Four default parallel runs and one serial run against isolated original
main `936c164` all passed. No Round II source was present in either control.

The relevant resize test/harness, Acid demo, Context/input/renderer/session and
Cargo manifest/lock sources have no differences between original main and the
Round I checkpoint. These observations establish intermittence in an unchanged
code path, **not its precise mechanism**. The symptoms resemble delayed input
around resize, but the known crossterm issue #15 is not proven to explain these
specific failures. The existing explicitly ignored collision-acceptance test
is separate. Serialization is not a demonstrated repair. A green repeat does
not erase the red runs, and no timing assertion was relaxed to obtain it.

Raw command logs remain outside the repository under the task workspace's
`logs/round2-baseline-*.log` and `logs/round2-original-main-pty-resize-*.log`.
These are run provenance, not files required to reproduce the tests.

### Hosted CI scope

`ci.yml` triggers on pushes to main, pull requests and manual workflow dispatch.
A feature-branch push alone does not trigger it. The publication-free Release
Preflight runs manually or on PRs changing its listed release-shaping paths;
this branch's release-contract edit is one such path. Both workflows keep
read-only contents permission. No hosted result is inferred from YAML alone.
Draft [PR #32](https://github.com/femboy2112/libgibson/pull/32) provides the hosted
check surface. Its description records final run IDs/outcomes after observation;
this local validation document does not substitute for those checks.

The baseline CI compiled all examples but explicitly executed only `fx_lab`'s
example-local tests. Default `cargo test` does not execute those tests. Round II
adds a separate normal-CI command for `skin_gallery`, `polished_agent_ui` and
`ui_showcase`, retaining the existing fresh-example-build step. The new command's
final local outcome is recorded below; hosted execution is reported separately.

## Round II implementation validation

Final local gates ran on 2026-09-26 against executable source, tests and examples
at local checkpoint `8c8aac67dcd0e5d6ec6b1fdf970906ba0dacaf5e`, using the Linux
x86_64 / Rust 1.98.1 host above. Its canonical GitHub source checkpoint is
`4ab5c0e308ca3c6cade12e6ebf216608320a7158`: both commits have the verified same
complete Git tree, `0f1129b71805d49fc8bcf6eb4d68a1bbc631f093`. GitHub object
publication used the connector because local Git HTTPS credentials were absent;
commit metadata differs, while the validated files are identical. Documentation
and captured-image commits follow that source checkpoint. Each command below
completed on the frozen final source; earlier passing runs before the last
visual fixes are separate interim evidence.

| Command / probe | Observed final local result |
| --- | --- |
| `cargo fmt --check` | Exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0 |
| `cargo build --examples` | Exit 0; all existing and five new UI examples build |
| `cargo build --release` | Exit 0 |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | Exit 0 |
| `cargo +1.85.1 check --locked --lib` | Exit 0 |
| `cargo +1.85.1 test --locked --lib` | Exit 0; **263 passed**, 0 failed, 0 ignored |
| `MSRV_TOOLCHAIN=+1.85.1 scripts/release/msrv-consumer.sh` | Exit 0; both fallback and default fresh resolutions build |
| `scripts/release/check-versions.sh` | Exit 0; package 0.1.1, ABI 1, license payloads agree |
| `scripts/release/check-abi.sh` | Exit 0; **52 baseline / 52 built symbols** |
| `cargo package --list --locked --allow-dirty` | Exit 0 |
| `cargo package --locked --allow-dirty` | Exit 0; packaged crate builds |
| `cargo test --test ui_layer --test ui_goldens --test ui_controls` | Exit 0; **39 passed**, 0 failed (22 + 4 + 13) |
| `cargo test --example fx_lab --example skin_gallery --example polished_agent_ui --example ui_showcase` | Exit 0; **14 passed**, 0 failed (3 + 1 + 5 + 5) |
| `cargo test` | Exit 0; **716 passed, 0 failed, 1 known ignored**, across 49 result groups |
| Hosted CI on the final review revision | Reported separately in [draft PR #32](https://github.com/femboy2112/libgibson/pull/32), with exact head SHA, run IDs and final outcomes |

The final main suite is **263 library units + 452 integration tests + 1 doctest**.
That is 55 additional tests over original main's 661: 16 library units and
39 UI integrations. The 14 example-local tests are separate, including three
pre-existing `fx_lab` tests. The MSRV unit run and explicit UI run repeat subsets
of the main suite; they are not additional unique tests. No baseline test was
weakened, removed, newly ignored, or reclassified. The single ignored collision
acceptance remains unchanged. The previously intermittent nonignored resize
target passed in this final full run; its earlier failures and unresolved
mechanism remain recorded above.

The MSRV commands used an external `CARGO_TARGET_DIR` to separate toolchain
artifacts. Packaging emitted the existing manifest-policy warnings that excluded
integration-test files are not in the published crate; package verification
itself succeeded. Run-local evidence is in `logs/round2-final-*.log`, including
`round2-final-gates-summary.log`, outside the repository. Earlier passing gates
before the final visual corrections are retained separately under
`logs/round2-interim-before-visual-fixes/` and are not substituted for these runs.

### Rendered evidence and coverage

The four `ui_goldens` tests now execute and explicitly count **48 combinations**:
3 skins × 4 sizes (80×24, 120×40, 36×18, 32×16) × 4 color depths. Tiny/zero bounds
and ASCII fallback are additional checks. The three Mono text goldens were
updated only after inspection of the intended structural changes; the final
suite passes those exact expectations.

The 13 `ui_controls` regressions cover phase-correct custom presentation,
current-frame focus, controlled input ownership, local modal/toast effects,
Unicode escape content, authored section ordinals, persistent control-state
contrast, selected-control activation and bounded skin switching. Raw and
presented overlays assert exact authored cell offsets. Modal/toast locality
assertions also require visible dialog/notification content, so an empty panel
cannot satisfy the locality test accidentally.

The five `ui_showcase` example tests include **96 focused editor/control/cursor
cases** (8 sizes × 3 skins × 4 depths), cache reuse and invalidation, deterministic
captured cells and exact Full-to-None settled equivalence for seven event kinds
across all three skins, **18 success/error consistency cases**, and **9 toast
visibility cases**. The separate gallery test covers 27 visible-control cases.
These case counts describe loops inside tests, not additional Rust test targets.

Root review inspected 30 whole-frame resting/focus captures (3 skins × 5
environments × 2 states), plus 75 deterministic motion samples (5 roles × 3
skins × 5 event-relative times). A separate skin-agent pass inspected 75
modal/input/success/error/toast captures, refreshing 60 affected cases after the
last fixes; these reviews overlap and are not counted as unique extra coverage.
The captures come from actual headless output, not hand-drawn mockups. The
reproducible skin comparison image and its renderer are documented in
[assets/README.md](assets/README.md).

### Informational transport and retention probes

Twelve final-source profiles produced **612 TSV data rows**: each skin at
80×24 Full, 120×40 Full, 80×24 Reduced and 80×24 None, with 51 rows per run.
At 80×24 Full, the observed transport was:

| Probe | Black Ice | Vapor95 | Swiss Signal |
| --- | --- | --- | --- |
| Settled frame, bytes / changed cells | 0 / 0 | 0 / 0 | 0 / 0 |
| Focus maximum per-frame bytes / changed cells | 186 / 16 | 343 / 83 | 210 / 49 |
| Activation initial bytes / changed cells | 233 / 16 | 246 / 16 | 178 / 14 |
| Modal initial bytes / changed cells | 4022 / 1738 | 6469 / 1728 | 3932 / 1723 |
| Later modal motion maximum bytes / maximum changed cells | 1804 / 359 | 1012 / 360 | 860 / 364 |

The last row's two maxima may come from different samples. Opening a modal
changes the surrounding veil once; its later samples are local to the dialog,
and the next settled sample returns to zero bytes. Success/error initially
changed 16–18 cells using 115–170 bytes; the Black Ice error additionally changed
two cells using 74 bytes. These are observed output deltas, not universal budgets.

Canvas construction remained at one build per run. None retained zero active
motions. At 80×24, retained keys were 30 ordinarily and 35 with a modal; the
maximum observed at 120×40 was 59. Source/test lifecycle assertions establish
bounded removal independently of these sample counts. Logs are
`logs/round2-profile-*.tsv` in the task workspace. Reproduce exact full row budgets
with the built binary and `--fullscreen`, for example:

```sh
cargo build --example ui_showcase
target/debug/examples/ui_showcase --profile --skin=black-ice --width=80 --height=24 --truecolor --fullscreen
```

View/lowering/render microsecond columns are informational host measurements,
not stable benchmark thresholds. The displayed workload is synthetic; renderer
bytes and cell differences come from the actual headless stream.

This is machine-test and rendered-review evidence from the implementation team,
not an independent human usability study. It establishes no new platform,
universal performance or production-stability guarantee.

### Round II fixes with targeted regression evidence

- Semantic construction and reconciled presentation now have distinct phases:
  `BuildCx` carries environment/skin/time, while `PresentationCx` exposes current
  focus and motion after reconciliation. `presented(...)` callbacks observe
  initial, traversed, modal, restored and replacement focus in the frame being
  rendered, plus current skin/geometry and clamped time. They return ordinary
  Nodes and cannot change semantic metadata. The lowering applies their semantic
  motion exactly as for raw content; callers do not apply it a second time.
- Every enabled text input owns editing events, including a handlerless or
  submit-only controlled input. Without `on_edit`, those edits are consumed
  without mutating application state or leaking through `.on_event`/background
  shortcuts. Up/Down are consumed no-ops for the single-line editor; an unsupported
  shortcut can still use the explicit fallback route. Bound Enter submits a
  typed action.
- Modal and toast effects now act on their local panel instead of the enclosing
  placement surface. Tests require the modal's surrounding veil to remain stable
  and a toast to leave the rest of its parent untouched during motion.
- Idle, focused, selected, selected-and-focused, and disabled controls remain
  pairwise distinguishable under Mono and ANSI16 across all three skins.
  Activation is visible and finite under Full/Reduced and reaches the exact
  settled state; repeated skin switching clears old feedback while preserving
  application-owned selection and typed actions. A selected Black Ice control
  initially had inert Mono activation because its settled style was already
  bold; the regression now covers selected and unselected activation.
- Raw and presented overlays preserve authored Node offsets. Their tests check
  exact cell positions rather than only finding their text somewhere in a frame.
- Vapor95 bevels keep corners local, fill stretched windows, and preserve the
  editor cursor after content insets. Swiss Signal's elevated containers now use
  a restrained frame so a narrow Mono dialog title cannot visually join an
  underlay's section ordinal; ordinary flat editorial panels remain open.
- Showcase success/error status, table content and compact summaries now agree.
  Its toast is hosted on a noninteractive mission panel so it cannot cover the
  primary editor or Send control. These defects were found in rendered review,
  corrected in source, and covered by the final example tests above.

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
cargo run --example ui_showcase -- --dump-ansi --skin=black-ice --width=80 --height=24 --fullscreen
cargo run --example ui_showcase -- --dump --transition=modal --at-ms=80 --mono
cargo run --example ui_showcase -- --profile
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
