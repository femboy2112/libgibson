# Visual Intent — API Design Proposal (NOT an implementation)

> **Status:** DESIGN PROPOSAL ONLY. No code is proposed for `src/` this round.
> This records ONE candidate the evidence genuinely earns, the candidates it does
> **not**, and the gate each must pass before any implementation. The charter
> rule stands: *"no core feature promotion from one consumer"* — so nothing here
> ships until the [holdout](AI_VISUAL_AUTHORING_HOLDOUT.md) and the recipes show
> repeated, redundant manual glue across independent consumers.
>
> **Date:** 2026-10-02.

## Why a proposal exists at all

The audit's verdict is a **documentation gap, not an API gap**, and Phase 1 is
docs-only. But one piece of manual glue recurs across **every** world-first
realization inspected or built this round, which is exactly the signal that earns
a *proposal* (not an implementation):

| Consumer / artifact | Manually writes the full-frame-hero glue? |
|---|---|
| Chronoscope (external, Sonnet) | yes — `screen().height(lay.h)` + `raw(Node::surface(world))` + `.overlay(modal)` (`ui.rs:509, 411, 566`) |
| `libgibson_intro` (in-repo) | yes — builds a full-frame `Surface` world, `ctx.set_root(Node::raster(..))` |
| `recipe_hero_with_hud` (this round) | yes — `screen().height(env.height)` + `raw(Node::canvas(..)).grow(1.0)` |
| the other four recipes | yes — all repeat the same pin + hero mount |

And a real footgun sits inside it: **`screen()` is natural-height.** Omit
`.height(cx.environment.height)` and `.grow()` has nothing to expand into, so a
full-frame world silently collapses to a **single row** (reproduced this round;
see the audit §E). Every author must learn this the hard way or copy it.

## The one candidate the evidence earns

**A thin "full-frame hero" convenience** over the existing primitives — a helper
that mounts a size-aware world node as the frame-filling subject with the height
pinned, leaving the author to add chrome and overlays. Illustrative shape only
(names are placeholders; this is not a signature to implement):

```rust
// Today (every world-first app writes this):
screen()
    .height(cx.environment.height)                 // <-- the footgun if omitted
    .child(status_line)
    .child(raw(Node::canvas(move |rect| paint_world(rect, &snap))).grow(1.0))
    .child(hint_line)

// Proposed convenience (desugars to EXACTLY the above):
stage(cx)                                           // pins height to env, owns the frame
    .status(status_line)                            // optional sparse chrome
    .world(|rect| paint_world(rect, &snap))         // the dominant object, grows
    .hint(hint_line)
    .overlay_when(inspecting, || inspector_modal())
```

It must **desugar to the public primitives already shown** and add no new
rendering path.

### Gate (the §13 questions; this is why it is a *proposal*, not a commit)

1. **Is it repeated?** Yes — table above: 4+ independent realizations write the
   same pin + hero-mount + overlay glue.
2. **Is app-owned implementation materially redundant?** Partly. The glue is
   ~4 lines, but the `screen().height(env.height)` pin is a **silent-collapse
   footgun** every author hits once. The redundancy is small; the footgun is the
   stronger argument.
3. **Can LibGibson implement it generically, with no domain assumptions?** Yes —
   it is pure layout sugar over `screen`/`raw`/`grow`/`overlay`; it never touches
   domain semantics. (This is what keeps it in scope; a role *taxonomy* would
   not — see below.)
4. **Does it preserve escape hatches?** Required: it must be a thin convenience
   that desugars to the public primitives, which remain fully usable. If it
   cannot be expressed as desugaring, it is rejected.
5. **Does it make correct composition easier than bad composition?** Partially.
   It makes *mounting a hero* easier; it does **not** make an author *choose* a
   hero — that is the doctrine's job, not the type system's. This is the honest
   limit of the candidate and the reason docs come first.

**Verdict on this candidate:** earns a *proposal*, not an implementation. The
redundancy it removes is thin (~4 lines), so the whole case rests on the
footgun + repetition. If the holdout shows agents still collapse the world or
still fail to mount a hero *even with the guide*, this becomes worth building. If
the guide alone fixes behavior, a documented 4-line idiom is cheaper than new
public surface. **Decide after the holdout.**

A strictly smaller alternative worth weighing first: a single
`screen().fill_height(cx)` (or making the fullscreen root default to filling)
that only removes the collapse footgun, with no hero/chrome vocabulary at all.
Smaller surface, kills the footgun, leaves composition to the author + docs.
Likely the right first step if *anything* ships.

## Candidates explicitly NOT promoted (and why)

- **A composition-role taxonomy** (`Hero`, `World`, `Lens`, `HUD`, `Rail`,
  `Timeline`, `Annotation`, `Overlay` as public element kinds).
  **Rejected.** The audit (§D) shows the problem is not a missing vocabulary:
  **Cathedral built a panel farm using zero `gibson::ui` nouns**, and Theseus
  hand-tiled boxes with none of these roles. Renaming widgets to intent-nouns
  would add a large taxonomy that a weak composer would populate into a farm
  anyway (`Hero` containing six `Panel`s). It fails gate #5 hardest: it does not
  make correct composition easier than bad, it just relabels the bad. This is a
  *doctrine* concern, not a type.

- **Responsive semantic-priority classes** (`Essential` / `Secondary` /
  `Tertiary` as a public API). **Rejected for now.** The existing
  `cx.environment.width`/`height` branching already expresses this (see
  `recipe_semantic_zoom.rs`), and the right *policy* is domain-specific. Teach it
  (doctrine §6); do not encode a priority enum until the holdout shows authors
  can't express it with what exists.

- **A built-in camera/projection API for the common 2D/3D world.**
  **Rejected (separate track).** `Camera` + `Rasterizer` already exist
  (`raster3d.rs`). The only friction found is `Camera::project`'s frustum
  rejection, which forced Chronoscope to hand-roll near-plane clipping
  (Chronoscope `FRICTION` E5). That is a precise, narrow ergonomic bug for the
  renderer track — **not** a composition API, and not this round's problem.

- **`Node::canvas` promotion into `gibson::ui`.** **Deferred.** `Node::canvas`
  (`src/node.rs:321`) is the clean full-frame-hero primitive and is merely
  *undocumented* in `UI_LAYER.md`. The cheapest fix is documentation (done —
  it is now the star of the guide), not a new wrapper. Revisit only if the
  `stage`/`fill_height` candidate above is built, since it would use `canvas`
  internally.

## Preconditions before ANY of this is implemented

1. The [holdout](AI_VISUAL_AUTHORING_HOLDOUT.md) runs and shows that **docs alone
   do not** fix the behavior the candidate targets (world collapse / failure to
   mount a hero).
2. The change is confirmed to **desugar to existing public primitives** with no
   new rendering path and full escape hatches.
3. It passes the project's normal bar: CORE-tier review, green CI across the
   4-lens gate, deterministic replay and resource bounds preserved, and
   `semantic behavior / presentation behavior stay separable` (charter).
4. It is introduced as a *convenience*, documented as desugaring, with the manual
   idiom kept valid and shown side-by-side.

Until then: **documented, not built.**
