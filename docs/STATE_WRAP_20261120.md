# LibGibson state at wrap-up, 2026-11-20

A docs-only snapshot for picking the work back up. Nothing here changes code, CI or the release contract.

## main

- `main` = `27a958f` (merge of #446). All v0.9.0 slices are merged:

| PR | Slice | Merge |
|---|---|---|
| #436 | opt-in cell encoders | `9e3f9cf` |
| #437 | terminal capability probe | `6377c07` |
| #434 | SurfaceFx | `5854068` |
| #443 | toast lane | `9202e88` |
| #438 | kitty route, including #442 frame plan `5370628` | `f6c525b` |
| #448 | `layout::measure_wrapped` | `8a657fa` |
| #446 | F1–F12 key events | `27a958f` |

## v0.9.0 "clear SIGKILL 0": release PR #449 (draft)

- **H1** = `fe7239f`: the refolded `[Unreleased]` changelog, ROADMAP completion table, and version bumps.
  - Hosted CI 9/9: run 44753834393, plus Release preflight run 44753834385.
- **Waiting on the consumer gate.** MINERVA's TRAIN-2 suite is pinned to H1: `cargo check --all-targets`, `--lib`, and its render suites.
- **After the gate:**
  1. A docs-only H2 fills the ROADMAP "Release gates" and "Consumer gate" rows with the observed ids ("observed at H1; H2 differs only in this row").
  2. A tree check shows H1→H2 touches only `ROADMAP.md`.
  3. CI runs at H2.
  4. Merge on the release coordinator's GO at the exact sha.
  5. Tags follow, each with its own GO.
- **The one default change in 0.9.0 (#446).**
  - F1–F12 now reach Rust consumers as `Event::Key`. `KeyCode` was already `#[non_exhaustive]`.
- **F-key no-op guards (#446).**
  - **C ABI 1 is unchanged:** `poll_event` skips F-key events and keeps polling to the caller's deadline. It is bounded past the deadline and never blocks a headless poll.
  - **CPR guard test:** `CSI 1;<mod>R` is always parsed as a cursor-position reply and never as F3. So F3 with modifiers is decoded only from `CSI 13;<mod>~`, as documented.
- **Release wording:** "No breaking changes to stable-tier APIs; C ABI 1 and MSRV 1.85 unchanged." Experimental `ui::Key` is now `#[non_exhaustive]` (adds `Key::Scoped`).

## Parked

- **#444 text area (EXPERIMENTAL), 0.9.1:**
  - Held until after the v0.9.0 cut. Last green: `a4df296` (run 44748785329).
  - Includes a direct wide-grapheme continuation-cell test plus a `should_panic` mutant.
  - After the cut: merge main, green CI, then it's the first 0.9.1 merge request.
- **#433 Settings form primitives** (toggle, form_row, `Element::error`):
  - The RED slice is parked to the next candidate train, together with #444.
  - The narrow-stacking point from the consumer's needs list is folded into it.
- **F13+ keys:** issue #447.

## #450 review: host-owned ephemeral creative contributions (R0 research, not merged)

- **Verdict:** sound as R0. **Zero new libgibson types are needed.** Every presentation invariant maps to primitives already on `main`:
  - **Containment:** keyed pane with painter clip, `overlay`/`toast_lane` containment, and `RasterAnchor::Key`.
  - **Precedence:** host chrome (STOP, consent, egress label) is mounted last, outside the pane.
  - **Dismissal:** `modal(..).on_dismiss`, and removal is immediate.
  - **Geometry:** 80×24 via `measure_wrapped`/`responsive`/the `Fallback` pattern.
  - **Colour:** Mono/`NO_COLOR`.
  - **Motion:** `MotionPreference::{Reduced,None}`.
- **Gap noted, not filed:** there is no semantic screen-reader/alt-text export, only `Surface`/`Cell::plain_text`. The host already owns the typed proposal and its text summary. A library export would be filed only with a demonstrated consumer blocker.
- **"Verified" is never a renderer concept.** A render is a presentation receipt only.
- **Doc nits** were sent to the PR author.

## Embodiment / puppet runtime

- #439 (Gibson World 2.0 contract) and #445 (generic character rig, morphology sliders, animation DSL, props) are **R0 docs drafts**.
- **No implementation exists, and none starts without the owner's go.**

## Evaluation holdouts

- Withheld evaluation holdouts exist. They are kept privately, outside this repository, and are deliberately not described here.

## Next steps

1. Consumer gate result at H1, then H2 (gates rows), then CI, then merge GO for #449.
2. Tag `v0.9.0` and `bindings/go/v0.9.0` on separate GOs, following the release procedure.
3. Post-tag docs PR.
4. #444: merge main, green CI, merge GO as the first 0.9.1 item.
5. Next train: #444 + #433 (with narrow stacking) plus the consumer's upstream needs list.

## Pause addendum, 2026-11-21

- Nothing in this repository changed since the snapshot above:
  - `main` = `27a958f`;
  - #449 head `fe7239f` (CI 9/9) still waits on the consumer gate;
  - #444 and #433 are parked;
  - every branch is clean and pushed.
- The maintainer session spent the interval on downstream build/test-speed infrastructure for a consumer project. None of it touches this library.
- Resume order is unchanged: consumer gate → H2 gates row → CI → merge GO for #449 → tags on separate GOs.
