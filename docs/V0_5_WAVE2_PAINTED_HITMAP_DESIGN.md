# v0.5 Wave 2 — Painted-Frame / Hit-Map Unification (design handoff)

**Status:** DESIGN ONLY (Conjectured). Nothing here is built or compiled. Derived from a
read-only source audit of the `integrate/v0.5-instrumental-convergence-2026-10-22` tree
(Operation Instrumental Convergence, 2026-10-22). Every load-bearing claim carries a
`file:line` citation and an epistemic label; a claim without a citation is a judgement, not a
reading.

This is the round's most important *architectural* seam and the one deliberately **not** built
this round (one serial compiler lane; Wave 0 integration + Wave 1 correctness came first). It is
written so the next round starts at a sprint.

## The problem (Finding E / Categorical Law L2 — target soundness)

LibGibson has semantic elements+keys, layout, painting, pointer events, world-space geometry,
layered hit testing, and edit/gesture transactions — but **they are separate subsystems that do
not share code or data.** The existing spatial-edit slice builds hit rectangles from *application
model* coordinates, which proves nothing about whether the **actually painted** UI (its visible
positions, layering, scrolling, clipping, modal occlusion) agrees with pointer targeting.

**L2 demands:** interaction cannot target anything not currently eligible under the authoritative
*painted* layout. Today that is unenforced — and unenforceable — because the hit map is hand-fed.

## Current-state map (all `Verified` by source read unless noted)

- **Layout resolves once** at `src/layout.rs:473` `compute_layout(&mut Node, w, h)` → writes
  `node.computed_rect` (field `src/node.rs:215`), root-absolute cells, negatives clamped to 0
  (`src/layout.rs:455-458`). This is the single source of truth for position.
- **Painter reads that same field** at `src/painter.rs:123`, then applies accumulated offsets and
  the clip chain to get the *real visible rect* (`src/painter.rs:124-126`). **`computed_rect`
  alone is NOT where a node is painted** — absolute offsets (`src/painter.rs:54-63`), the viewport
  camera (`src/painter.rs:380-391`), and bordered-container clip shrink (`src/painter.rs:359-365`)
  all move/clip it. *Reading `computed_rect` raw as a hit rect is the bug the demos commit.*
- **`hit_test` shares none of it.** `src/hit_test.rs:91` `add_target(id, rect, z)` — the **caller**
  supplies all three; the module imports only `world_view` + `Rect`. There is **no pointer code in
  the UI layer at all** (`src/ui/runtime.rs` routes only Key/Paste). `Verified`.
- **No identity survives paint → hit → selection.** `Element.key: Option<Key>` exists
  (`src/ui/element.rs`), and lowering computes `let id = key(el, path)` at `src/ui/compile.rs:409`
  — but **never stamps it on the `Node`** (`Node` has no id field, `src/node.rs:210-219`). So
  `Interaction` carries a key but no rect; `Node` carries a rect but no key; **they never meet.**
  `Verified`.
- **Z-order can disagree.** `hit_test` order is caller-chosen (highest z wins, ties = last
  inserted, `src/hit_test.rs:55-61`; modal ends the walk, `:137-147`). Paint order is implicit
  tree pre-order (`src/painter.rs:385-392`). **No function relates the two** → they *can* diverge.
  This is the concrete L2 violation. `Verified` (by absence of shared code).
- **Clipping/disabled holes.** `HitLayers` has no clip concept, only `Rect::contains`
  (`src/surface.rs:33-38`); a clipped/scrolled child handed its natural rect is hit outside its
  visible area. `Interaction.disabled` gates only *keyboard focus* (`src/ui/runtime.rs:290`);
  pointer has no disabled concept. `Verified`.
- **`world_view` is already honest** — degenerate/non-invertible zoom returns `Clipped(NonInvertible)`
  (`src/world_view.rs:380-425`), `hit_test_world` maps `Clipped → None` (`src/hit_test.rs:154-159`).
  No bogus in-bounds coordinate can come out of it. The hole is that cell-space `hit_test` and
  `gesture` capture are tied to no anchor/clip. `Verified`.

## The narrowest interface (three thin pieces — Conjectured, unbuilt)

Reuse unchanged: `Node.computed_rect`, `Rect`, `HitLayers`, `TargetId`, `PointerCapture`,
`DragGesture` (already takes `&HitLayers`), `Key`, `Interaction`, `ModalScope`, `Compiled`.

**A. Identity on the node** (`src/node.rs`, `src/ui/compile.rs`)
- Add one field `Node.tag: Option<u32>` (index into a new `Compiled::targets: Vec<TargetSpec>`),
  stamped in `lower_role` before the overlay wrapper. A tag survives clone/transform/wrap, which a
  child-index path does not. `TargetSpec { key, disabled, modal, role }`.
- ⚠️ Touching `Node` (a core type) has unaudited ABI/golden-test blast radius — see Open Decisions.

**B. Painter-owned geometry walk** (`src/painter.rs`)
- Factor the *pure geometry* out of `paint_node_contents` into shared helpers
  (`node_frame(node, clip, ox, oy)`, `child_origin(parent, child, ox, oy)`) that **both** paint and
  the new walk call — so they cannot drift.
- `fn painted_regions(root: &Node, surface_area: Rect) -> Vec<PaintedRegion>` where
  `PaintedRegion { order, subtree_end, tag, natural, visible: Rect, fx: bool }`. Reads
  `computed_rect` only; runs **no second layout pass**. `order` = pre-order index = paint order
  (later = above).

**C. Assembly** (`src/ui/hit.rs`, new)
- `impl<A> Compiled<A> { fn hit_map(&self, surface_area: Rect) -> HitMap }` — pure, precondition
  `compute_layout` already ran. `HitMap { layers: HitLayers, targets: Vec<ResolvedTarget> }`,
  `ResolvedTarget { id, key, rect: Rect /*visible*/, order /*z*/, clip, eligibility, modal, role }`.
- Assembly rule: walk regions in `order`; `z = order`; a modal backdrop opens a layer, its
  `subtree_end` closes it (later siblings sit above but see-through — matches
  `src/hit_test.rs` `modal_only_blocks_layers_beneath`); fully-clipped (`visible.is_empty()`) kept
  in `targets` but not in `layers`; partial clips use `visible`, never `natural`; disabled targets
  stay as occluders (`eligibility = Disabled`) so they **swallow** the click, not fall through.
- `HitMap::layers()` feeds `PointerCapture`/`DragGesture` unchanged → the whole input path lights up.

## The discriminating fixture (Conjectured — `tests/ui_hit_paint_unification.rs`)

**Oracle must be independent of the hit map: read the painted `Surface`.** Make each keyed target a
`raw` node with a unique `background(Color::Rgb(sentinel))`; expected key at a cell = topmost
sentinel color found there (via `surface.get(x,y).style.bg`). Assert for every `(x,y)`:
`hit_map.hit(x,y).map(|t| &t.key) == expected(x,y)`. Return a `Vec<(x,y)>` of disagreements —
genuine pipeline gives `[]`; every mutant gives non-empty.

Scene (one axis each): nested panels; a **clipped scrolled viewport** with items straddling its
edges; **overlapping** targets via `Stack` order; an **absolute-offset** node (`computed_rect` ≠
painted); a **disabled occluder**; a **modal overlay**; plus **resize** (`compute_layout` at two
sizes) and **animated** frames (viewport scroll + `.grow()` between t0/t1).

Mutation arms (tamper a public `PaintedRegion` field, assert the oracle bleeds):

| # | Tamper | Cell that must disagree |
|---|---|---|
| M1 | `visible.x += 1` | edge of A |
| M2 | `visible.y += 1` | edge of A |
| M3 | reverse `order`/z | P/Q overlap cell |
| M4 | use `natural` not `visible` | viewport item at the clip edge |
| M5 | ignore modal layer (one flat layer) | base-target cell under the modal backdrop |
| M6 | raw `computed_rect`, ignore `(ox,oy)` | the offset node C |
| M7 | stale map (size 1) queried after resize 2 | many cells |
| M8 | skip disabled targets | B (must be swallowed, not fall through) |

Plus a **control** (unmutated → zero disagreements, so a broken oracle can't pass vacuously) and a
**sentinel self-test** (each key's box painted alone carries its sentinel over every `visible` cell
— settles that `painted_regions` matches paint even through the scratch/fx path).

## Open decisions / nags (the owner or the next round must resolve)

1. **Modal locality vs global capture.** Paint dims only the modal's containing block
   (`src/ui/compile.rs:678-689`); the runtime/`HitLayers` model a global block
   (`src/ui/runtime.rs:330,416`). Decide: backdrop target spans the whole surface (consistent with
   keyboard) or only the painted rect (consistent with pixels). Decides what M5 asserts.
2. **Painter viewport quirk** (`src/painter.rs:386-390`): a nonzero camera offset drops a child's
   absolute `offset_x/y`. Bug or intended? A separate painter defect; the shared walk inherits it.
3. **Layout clamps negative locations to 0** (`src/layout.rs:455-456`): an oversized centered modal
   reads `x=0` where Taffy placed it negative. Paint and hit agree (both clamped), but "where Taffy
   put it" is lost — neither side can detect it.
4. **`SurfaceFx`** (`RowShift`/`Tear`/`Dissolve`) moves/erases pixels inside a region; the hit rect
   is the pre-effect extent. Decide whether `fx: true` regions are accepted as-is.
5. **Inert opaque occluders:** a non-actionable panel bg painted over a button is no `Interaction`,
   so it would not occlude in the map. Add an `Occluder` role for opaque Box/Border/Raster, or rule
   "only controls matter." The oracle's non-sentinel cells expose this immediately.
6. **No "not laid out" signal:** `Rect::default()` is all-zero, so a node that never went through
   `compute_layout` yields silently-empty regions. Cheap guard: assert root `visible` non-empty, or
   make `hit_map` return a `Result`.

## ABI / blast-radius note (UNVERIFIED)

Adding `Node.tag` touches a core type. `ui` is excluded from the C ABI per `src/ui/mod.rs:9`, but
the `src/ffi.rs` and painter-golden impact of a new `Node` field was **not** checked. Audit before
building B.

## Epistemic ledger

- **Verified (read):** the current-state map (hit_test↔painter disjoint, identity dies at
  `compile.rs:409`, z-order can disagree, clip/disabled holes, `world_view` soundness).
- **Conjectured:** that the shared `node_frame` is behavior-identical to the current inline painter
  code (the painter goldens + the sentinel self-test must prove it); that `Raw` leaves carry the
  sentinel through `compile` untouched; the exact modal-in-local-panel geometry.
- **UNVERIFIED:** nothing here was compiled or run; ABI/golden impact of `Node.tag`; `scene.rs`,
  `ui/skin.rs`, `ui/motion.rs`, `input.rs` mouse decoding not read.
