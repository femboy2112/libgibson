# Carried-forward PR review (v0.6 Gate G)

Reviewed 2026-11-03 against `main` @ `2ffdd92`. Per the v0.6 roadmap ([#191](https://github.com/femboy2112/libgibson/issues/191)) and ROADMAP.md, the deferred Wave-3 editor PRs (**#176 / #177** → R6.2) and media PRs (**#186 / #188** → R6.4) are reviewed **only for surviving unique functionality** — what is genuinely not on `main`, what is worth landing in v0.6, and what must not be double-merged or scope-crept in.

This is an assessment, **not** a merge or a promotion. Nothing here is landed; the owner holds every merge/tag/release.

## Method

For each PR: its changed files, size, base branch, and whether its functionality already exists on `main` or in the v0.6 draft stack (`#241 ← #242 ← #243 ← #244`). "Surviving unique" = present nowhere merged and carrying work worth keeping. "Double-merge risk" = primitives the PR's base chain also contains that are *already* on `main`.

## R6.2 — Wave-3 scene editor

The hit-test / painted-hitmap primitives the editor is built on (`src/hit_test.rs`, `src/ui/hit.rs`, `src/node.rs` semantic keys, `src/gesture.rs`) **are already on `main`**. The editor *example and its tests* are **not** — `examples/` has only `scope_editor` (a different, scope-focused tool), no `scene_editor`.

### #176 — `feat(editor): Wave 3 — scene editor` (test slice)
- **Changes:** `tests/scene_editor_slice.rs` (+618, 1 commit). **Base:** `wave2/painted-hitmap` (stale, not `main`).
- **Verdict:** SURVIVING / UNIQUE. A substantial test slice proving a scene editor composes from public primitives — exactly the R6.2 "prove the public interface" goal. No equivalent on `main`.
- **Caveat:** its base branch is stale and the slice depends on the `scene_editor` example in #177; rebase the pair onto current `main` before landing. The underlying hit-test primitives are already on `main` — **do not** re-introduce them from the `wave2` base (double-merge risk).

### #177 — `feat(editor): Wave 3 PTY walkthrough`
- **Changes:** `examples/scene_editor.rs` + `tests/scene_editor_pty.rs` (+795, 3 commits). **Base:** `wave3/editor` (the #176 chain).
- **Verdict:** SURVIVING / UNIQUE and the higher-value of the pair — a real scene-editor example plus a true-terminal walkthrough, the kind of independent-consumer ergonomics R6.2 is about. Complementary to, **not** duplicated by, the v0.6 `wire_instrument` (that is a scientific-field instrument; this is a geometry/shape editor).
- **Recommendation:** keep as an R6.2 candidate; rebase #176→#177 onto current `main` as one reviewable slice. Confirm it does not re-vendor on-`main` primitives. Not superseded by any merged work.

## R6.4 — Embedded media

`gibson::media` does **not** exist on `main` (`src/media.rs` absent, no `mod media` in `src/lib.rs`), so there is **no double-merge risk** for the media work. Per #191 R6.4, media is promoted **only when a concrete consumer justifies it** — so these stay deferred drafts, surviving-but-held, not landed speculatively.

### #186 — `feat(media): deterministic embedded media assets (gibson::media)`
- **Changes:** `src/media.rs` + `src/lib.rs` (+939, 1 commit). **Base:** `main`.
- **Verdict:** SURVIVING / UNIQUE, **HOLD (consumer-gated)**. A real embedded-asset module (deterministic, rights-safe). Adds public API surface, so it is an ABI/surface commitment once landed — promote only behind a named consumer (e.g. the R6.3 Doom-horizon raster needs or higher-resolution graphics), not on its own.

### #188 — `feat(media): optional DEFLATE segment codec (media-deflate)`
- **Changes:** `src/media.rs` + `Cargo.toml`/`Cargo.lock` (+174 / −8, 1 commit). **Base:** `feat/v0.5-media-embed-2026-10-24` (stacked on #186) — a −13.6% embed-size win.
- **Verdict:** SURVIVING / UNIQUE, **HOLD (consumer-gated), stacked**. Only meaningful atop #186; must land after it, never before. Same R6.4 gate: promote with the consumer that needs the size reduction, not standalone.

## Summary

| PR | Maps to | Unique vs `main`? | Double-merge risk | Verdict |
|---|---|---|---|---|
| #176 | R6.2 | Yes (test slice) | hit-test primitives (already on `main`) — rebase, don't re-vendor | Keep; rebase with #177 |
| #177 | R6.2 | Yes (editor + PTY) | none beyond #176's base | Keep; rebase onto `main` as one slice |
| #186 | R6.4 | Yes (`gibson::media`) | none | Hold — promote only behind a named consumer |
| #188 | R6.4 | Yes (deflate codec) | none; stacked on #186 | Hold — lands after #186, consumer-gated |

**No PR is superseded by merged work; none should be double-merged.** The editor pair (#176/#177) is the actionable R6.2 item (needs a rebase onto current `main`); the media pair (#186/#188) stays deferred under R6.4 until a concrete consumer justifies the added public surface. Existing checkouts: `../LibGibson.media` (#186 branch) and `../LibGibson.deflate` (#188 branch).

---

## RXIII addendum (2026-11-05) — #176 / #177 decision, revised: superseded by `instrument_workbench`

**This revises the R6.2 conclusion above.** The 2026-11-03 review concluded #176/#177 were "surviving / unique, not superseded by any merged work." That conclusion rested on line 13's claim that "`examples/` has only `scope_editor` … no `scene_editor`" — which searched for an example *named* `scene_editor` and so missed the **functional** equivalent that is already merged: **`examples/instrument_workbench`** (merged to `main` via #140/#199). The workbench *is* a scene editor composed from nothing but LibGibson's public surface, with a headless slice **and** a real-PTY walkthrough — i.e. the #176/#177 thesis, shipped.

### Oracle-by-oracle evidence (merged on `main`)

| #176 / #177 deliverable | Delivered by (merged) |
|---|---|
| scene editor from public primitives | `examples/instrument_workbench/{main,workbench}.rs` |
| headless composition slice | `tests/instrument_workbench_slice.rs` (`s1`–`s14`, +more) |
| pointer → painted canvas (L2); canvas cell → shape (L3) | `s1_pointer_targets_the_painted_canvas_not_an_app_rect`, `s2_world_cell_reconciles_pointer_probe_and_index` |
| pointer pick resolves the **topmost** shape; off-canvas picks nothing | `s12_pick_prefers_the_topmost_overlapping_shape`, `s5_pointer_outside_viewport_has_no_in_field_coordinate` |
| drag → exactly one undo transaction; cancel on Esc / focus / resize / modal (`DragGesture`) | `s6_dropped_release_cancels_the_drag` (Esc + Resize), `workbench.rs:926` "self-cancels on Esc/focus/resize/modal" |
| pointer and keyboard reach the **same canonical model state** | `s7_keyboard_and_pointer_moves_reach_the_same_state`, `s13_tab_select_then_move_equals_pointer_select_then_move` |
| bounded numeric **refused** out of range (no clamp — Finding D) | `s3_numeric_inspector_refuses_out_of_range` |
| undo/redo round-trip (log + keymap) | `s9_undo_redo_round_trip_and_atomic_compound_rollback`, `clean_undo_and_redo_report_applied`, `empty_undo_and_redo_are_noops_not_silent_success` |
| honest pan/zoom world coords; pick after pan | `Intent::Pan`/`Intent::Zoom` fold into the structural digest (`workbench.rs:479`,`:564`,`:566`); pick uses the same `view` transport |
| deterministic replay | `trace_replays_deterministically` |
| real SGR mouse click selects the shape under the pointer (live terminal) | `tests/instrument_workbench_pty.rs::workbench_pty_mouse_click_drives_the_model`, `pty_click_selects_the_specific_shape_under_the_pointer` |
| session-owned restoration (alt-screen, raw mode, bracketed paste, mouse + focus reporting) | `workbench_pty_unwinds_reporting_on_early_failure`, `workbench_pty_walkthrough` |
| real PTY walkthrough; 40×15 compact keyboard-only path | `workbench_pty_walkthrough`, `instrument_workbench_kbd_pty.rs::keyboard_only_workflow_at_40x15_mono` |

### The sole surviving-unique functionality

`instrument_workbench`'s model has `Intent::DrawRect` and `Intent::DrawLine` — **rect and line, not the freehand `stroke`** kind #176 lists as its third shape. That is the one deliverable with no merged equivalent. It carries **no current consumer demand**, so it is held under the same consumer-gate as the media PRs (#186/#188): add the freehand stroke only when a named consumer needs it, not speculatively.

### Decision (recommendation — owner holds the close button)

**Close #176 and #177 as SUPERSEDED by the merged `instrument_workbench` + its three test files.** Re-basing the stale `wave2/wave3` editor branches onto `main` would re-land a second scene editor that duplicates the merged one and risks re-vendoring the hit-test primitives already on `main` (the double-merge hazard the original review flagged). The narrow freehand-`stroke` residual is recorded here as deferred-until-a-consumer-needs-it. This changes the summary verdicts for #176/#177 from "Keep; rebase" to "Close; superseded (narrow stroke residual deferred)"; the #186/#188 media verdicts are unchanged.

## RXIV addendum (2026-11-05) — final disposition for the v0.6 integration candidate

| PR | Disposition | Owner action |
|---|---|---|
| #176 / #177 (Wave-3 scene editor + PTY walkthrough) | **Superseded** by the merged `instrument_workbench` (mapping above); sole residual is freehand `stroke`, not required by any v0.6 consumer | close as superseded (not done by the agent) |
| #186 / #188 (`gibson::media`, `media-deflate`) | **Deferred by design** (R6.4) — no concrete consumer yet | leave open / revisit at v0.7 (Doom horizon) |
| #195 / #196 / #197 (dependabot) | Outside the v0.6 integration scope; independent of this candidate | owner's call |
| #241 … #251 (Renaissance stack) | Contained in full, with ancestry preserved, in the consolidated integration PR | close or leave after that PR merges |

No PR was closed, merged or edited for state by the agent.
