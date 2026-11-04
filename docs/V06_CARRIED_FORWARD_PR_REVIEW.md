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
