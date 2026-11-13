# AI Visual Authoring — Root-Cause Audit

> **Status:** research / non-binding. No core commitment. This audit explains a
> *human aesthetic verdict* on three external consumers and proposes a
> documentation-first intervention to test. It does **not** claim tests can
> measure beauty, and it does **not** earn a new public API on its own.
>
> **Date:** 2026-10-02 · **Substrate:** released `v0.4.0`
> (`c2f6483d92fe2b351e6cd50936a97d8cdf73cb79`).
> **Evidence repo:** `femboy2112/libgibson-agent-native-ui-lab`, consumer PRs
> #5 (Theseus), #6 (Cathedral), #7 (Chronoscope).

## The question

Three external models were given similarly ambitious "build a spatial/cinematic
LibGibson app" prompts against the **same public substrate**. One (Sonnet /
**Chronoscope**) produced a coherent, continuous, cinematic application. Two
(DeepSeek + agy / **Theseus**, **Cathedral**) produced technically interesting
apps that the maintainer judged to land in a *1980s-DOS-UI / 1990s-homemade-webpage
/ panel-farm* attractor.

> **Why?** And: what is the **smallest** intervention that makes the visual
> intent legible enough that weaker agents stop composing rectangles and start
> using LibGibson as a spatial medium?

The maintainer's verdict is treated as a **human acceptance datum**, not a
measurable. The three cinematic Chronoscope frames in its `docs/img/` were
inspected directly for this audit (`05-the-fork.png`, `07-compare.png`,
`42x15-truecolor-ghost.png`, `120x40-inspect-modal.png`).

---

## A. External evidence

Four independent read-only bearings dissected the three consumers and the
upstream authoring surface. Every claim below is cited to consumer or upstream
source. Package paths are relative to each consumer's `hyperion/project-*/`.

### A dangerous near-miss up front: the obvious theory is **wrong**

The tempting explanation — *"the weak models reached for `gibson::ui` panels;
the strong one didn't"* — is **refuted by the evidence**:

- **Cathedral uses `gibson::ui` zero times** (README: "does not use `gibson::ui`";
  `grep ui::` over its `src` is empty). It builds a genuine world-coordinate
  space with a real `Camera` (`visual.rs:151-238`) and a log-space zoom ease.
  It is **not** a widget tree. The maintainer still read it as a panel farm.
- **Theseus** builds its whole body as one hand-painted `Surface`
  (`visual.rs:770-933`) carrying a single `Node::raster` — but *inside* that
  raster it hand-tiles **four numbered rounded-border boxes**
  (`visual.rs:818,831,855,895`: `1 · … 2 · … 3 · … 4 · …`). A panel farm drawn by
  hand, with no panel widgets involved.

So "panel farm" is **not** caused by touching the `panel()` component. Both weak
consumers reproduced the attractor *without* it. The cause is deeper, and lives
in composition method — see §B.

### Chronoscope (cinematic — the positive control)

A time-travel debugger: execution history is a **tunnel** you move through; time
is the depth axis; forks peel off as continuous geometry; dead futures persist as
stippled ghosts.

- **One world, one camera, one coordinate space.** A hand-rolled perspective
  projector with a per-dot z-buffer, near-plane clip, and atmospheric depth
  fade (`view3d.rs:341-435`, `:202-217`). Forks are a continuous smoothstep
  peel-away in the same space (`center()`, `view3d.rs:508-524`); the camera and
  renderer share the *same* geometry function (`lane_xy` app.rs:1074 vs
  `center` view3d.rs:508). Rewind literally turns the camera to face the past
  (`app.rs:357, 816-821`).
- **The world owns the frame.** The viewport is ~58% at 120×40, ~67% at 160×50,
  73–75% at small sizes (computed from `layout()`, `ui.rs:20-43`), and it is the
  only large element. The hero is a single pure function
  `render_navigate(&Snapshot) -> Frame` (`view3d.rs:765`) composited into one
  `Surface` and handed to LibGibson as a `raw(Node::surface(...))`
  (`ui.rs:411`, `director.rs:376`).
- **Information lives IN the world.** Task/lane labels are *projected from world
  coordinates* onto the geometry (`view3d.rs:1184-1224`); causal ancestry on
  inspect is drawn *as world segments* (`view3d.rs:1128-1156`); the semantic
  regime colors the whole field (STABLE teal / DEADLOCK red / UNCERTAIN olive —
  verified in the PNGs). Chrome is almost entirely one-line text.
- **Responsive = LOD, not reflow.** `lod_for` (`view3d.rs:131-166`) keeps the
  hero and sheds density, labels, ghosts, and the side panel (only shown at
  w≥100) by viewport area. Confirmed in `42x15-truecolor-ghost.png`: the tunnel
  survives, the rail is gone.
- **Overlays float.** The inspect modal is `.overlay(modal(...))` over a fixed
  world (`ui.rs:566`); the world renders unchanged beneath it
  (`120x40-inspect-modal.png`).
- **Visual acceptance was load-bearing.** Chronoscope is the **only** consumer
  that committed rendered PNG screenshots. Its author treated *looking* as part
  of done.

### Theseus (panel farm)

A music-identity "quotient reactor". Built a real RGB raster reactor
(`identity_raster`, `visual.rs:251-413`) — then starved and boxed it.

- **Hero starvation.** At 120×40 the reactor *panel* is ~17% of the frame and
  the actual ring disc ~6%; the **text console (1320 cells) is larger than the
  hero (810)** and the vertical layout feeds the console first
  (`visual.rs:786-787`). No camera exists — a fixed polar map with no pan/zoom
  state (`visual.rs:283-286`).
- **N independent regions, not one space.** Reference roll, reactor, cover roll,
  spectrum ribbon, console — each its own mapping; toggling an axis regenerates
  whole regions (`main.rs:377`).
- **Information lives in chrome.** The same facts (the quotient %, the axis
  state) are duplicated across the header, the reactor HUD, the console, and the
  status strip (3–4×).
- **No visual-acceptance loop.** No committed captures; tests assert *strings*
  ("QUOTIENT", "PRESERVED", "PURGED"). Git history shows the panels came
  **first** (`f705c00` "Use semantic panel and card layout") and the raster was
  retrofitted after (`55a702d`).

### Cathedral (panel farm)

A distributed-incident "architecture". A real world-coordinate space and camera
— but the world is small, textureless, and decorative.

- **A big viewport, a tiny subject.** The viewport is ~76% of the frame, but the
  actual structure is a ~21% *portrait strip of 1-column bars* floating in a
  large empty gradient sky (arithmetic from the layout constants; `tower_w =
  zoom.clamp(1,6).round()` with default zoom ≈0.41 → 1-cell towers,
  `visual.rs:554`).
- **All detail is zoom-gated OFF at the default view** (`show_detail = zoom >=
  2.2`, `visual.rs:555`); the resting frame has no texture but color.
- **Information lives beside the world.** Everything readable — names, numbers,
  breaker state, hotspots — is in the right panel + header + footer *text*
  (`visual.rs:909-1062, 813-892`), rendered in DOS chrome vocabulary: `key:
  value` status bar, `[F]inject-fault` bracket hotkeys, `█/·` gauge bars, a
  double-line `╔═╗` report box.
- **The one real deformation is dead code.** The "severed wing displaced across
  a gap" (`ISLAND_OFFSET`) is only applied in a layout rebuild that the
  interactive path never calls (`visual.rs:108-113`; layout built once at
  `app.rs:77`).
- **No visual-acceptance loop.** No captures; `EXPERIMENT_REPORT.md:415` states
  outright: *"No visual-quality claim; the capability check is semantic (labels
  present), not an aesthetic judgment."*

---

## B. Capability vs composition

**This is a composition/authoring gap, not a capability gap.** The evidence is
decisive:

- Chronoscope built a full perspective-3D pipeline (z-buffer, near clip,
  atmospheric fade, shared camera/renderer geometry, overlay chrome, LOD,
  post-process FX) **entirely on the public `v0.4.0` primitives** — `Surface`,
  `RgbRaster`, `Camera` (`src/raster3d.rs:38`), `Rasterizer`, `Scene`, `Story`,
  `SurfaceFx`, `Node::surface`.
- Cathedral built a camera over a coordinate space with **zero `gibson::ui`**.
- The in-repo **introductory film already demonstrates the pattern** — "ordinary
  Node mini-UIs mounted inside projected physical facade regions, alongside
  world-space vectors," explicitly logged by the campaign charter as
  *"CORROBORATING EVIDENCE for public rendering composition without
  building-specific core types"* (`AGENT_NATIVE_UI_ZERO_CONTEXT_HANDOFF`).
- The prior expressivity audit already concluded the issue is
  *"discoverability/composition, not raw expressivity"*
  (`LIBGIBSON_EXPRESSIVITY_API_AUDIT_2026-09-24.md:162`) and recommended **"do
  not add widgets."**

The three distinguishing composition decisions — the smallest set that tracks
the human verdict — are:

| # | The decision that separates cinematic from panel-farm | Chronoscope | Theseus | Cathedral |
|---|---|---|---|---|
| **D1** | The world has perceptual **primacy** (big *and* dense *and* the subject), not just a big `Rect` | ~65% & dense & sole subject | hero ~6%, console larger | 76% Rect, ~21% 1-col-bar subject, detail gated off |
| **D2** | The **information lives in the world** (labels/ancestry/regime are geometry/color), not beside it in panels/text | in-world | duplicated in chrome 3–4× | all readable state in side panel + header |
| **D3** | **One coordinate space + a moving camera**; state change *deforms* the persistent object, it doesn't refill panels or swap views | yes | no camera; N regions regenerated | camera-zoom only; deformation is dead code |

A fourth, meta-level cause sits under all three:

- **D4 — Visual acceptance was never in the weak loop.** Both panel-farm
  consumers asserted *strings* and explicitly disclaimed visual quality. You
  cannot tune a frame you never render and look at. An *untuned* world collapses
  toward the nearest legible fallback — text in boxes — regardless of how much
  raster machinery sits behind it.

---

## C. Current authoring attractors

The one document a UI-task agent actually reads — `docs/UI_LAYER.md` (626 lines)
— steers hard toward widget arrangement:

- **It leads with a panel farm.** The first code (`:41-70`) is
  `screen().child(heading(..)).child(panel("WORKSPACE").child(status(..))…child(button(..)))`,
  presented as the canonical "small application," immediately followed by
  `:72-74` "Run `cargo run --example ui_quickstart`."
- **The world pattern is one subordinate clause** (`:37-39`): "reach for
  `gibson::ui` for semantic chrome and interaction, and for raw `Node` /
  `Surface` where you need dense custom graphics."
- **Custom drawing is framed as an "escape hatch"** (`:407`) that you *wrap in a
  panel*. The `Component` example (`:424-436`) is literally
  `panel("INSTRUMENT").child(raw(...))` — a canvas inside a box.
- **Responsive is taught for widget trees only** (`:163-203`).
- **No example mixes a world with sparse chrome.** All five `gibson::ui`
  examples (`ui_quickstart`, `ui_showcase`, `ui_gallery`, `skin_gallery`,
  `polished_agent_ui`) are widget farms whose only canvas is a 3–11 row Braille
  instrument *inside a panel* (`ui_showcase.rs:360-364`). The world-first
  exemplar (`libgibson_intro`, 4636 lines, `gibson::ui` = 0) is **not linked from
  `UI_LAYER.md`.**

Cold-start reading budget toward the panel-farm model: README pointer →
`UI_LAYER.md` (626) → `ui_quickstart` (34) ≈ **~1,017 lines, all widget-first.**
The world-first path is 4,636 unlinked lines.

---

## D. Which docs/API nouns steer model reasoning

The public `gibson::ui` vocabulary (`src/ui/element.rs`) is entirely
**arrangement** nouns: `screen, row, column, stack, spacer, panel, card,
section, divider, text, label, heading, status, badge, button, progress,
sparkline, table, list, modal, toast`. There is **no** intent vocabulary —
nothing named `hero`, `world`, `camera`, `lens`, `rail`, `hud`, `timeline`.

**But** — per §A, Cathedral built a panel farm using *none* of these nouns. So
the vocabulary is a **secondary** nudge, not the root cause. A model with strong
composition priors (Sonnet) transcended it; a model with weak priors rebuilt the
attractor by hand even while avoiding the nouns. The lever is a taught
**discipline**, not renamed widgets (see §G, §13-review in the API proposal).

---

## E. What Chronoscope did manually that the API does not encode

Repeated manual glue, worth noting (but not yet worth an API — see the proposal):

1. **Full-frame hero mount.** `screen().height(env.height)` is *mandatory* or the
   world collapses — `screen()` is natural-height, so `.grow()` has nothing to
   expand into (Chronoscope: `ui.rs:509`; independently reproduced while building
   this round's recipes — a hero with no height pin renders as a **single row**).
   There is no one-call "mount this node as the full-frame hero" helper.
2. **World-owned pixel buffer → LibGibson.** Compositing an own `Surface` and
   handing it over as `raw(Node::surface(...))`/`Node::canvas(...)`. `Node::canvas`
   (a size-aware deferred paint callback, `src/node.rs:321`) is the clean
   primitive for this and is **undocumented in `UI_LAYER.md`**.
3. **A hand-rolled projector.** `Camera::project` rejects points outside the
   frustum, so Chronoscope wrote its own `Proj` to get near-plane clipping
   (`view3d.rs:375-377`; Chronoscope `FRICTION` E5). Minor ergonomic friction.

## F. What Theseus/Cathedral reveal as repeated composition concepts

Even the panel-farm consumers repeatedly, manually implement the same roles —
which is evidence the concepts are *real*, not that they need an API:

- a **dominant region** / viewport (Theseus `mid_inner`; Cathedral `vp`);
- a **status/HUD line** (all three);
- a **contextual rail/panel** (Cathedral right panel; Theseus HUD);
- **size-class** branching (`compact`, `wide`, zoom LOD);
- **overlay** vs reflow (all three put help/inspect as overlays).

The concepts recur; the *quality* differs by whether the world got primacy,
information, and a look. That points at documentation, not new types.

---

## G. Hypotheses ranked by evidence

1. **Docs deficiency (strongest).** The single doc a UI-task agent reads teaches
   widget arrangement as "the LibGibson way" and buries the world pattern in one
   clause + an unlinked 4,636-line film. §C.
2. **Example deficiency (strong — same root).** No example demonstrates
   *custom-world + sparse-`gibson::ui`-chrome together*. The cinematic examples
   use zero `gibson::ui`; the `gibson::ui` examples bury the canvas in a panel.
   The mixed pattern an app author actually needs is undemonstrated. §C.
3. **Missing composition discipline + visual-acceptance loop (deepest).** Nothing
   teaches "the world is the dominant object," "put the information *in* the
   world," or "render the frame and look at it." Cathedral and Theseus prove a
   world can be built and still fail for lack of these. §B (D1–D4).
4. **Model capability / taste difference (real, not the lever).** Sonnet brought
   priors the others lacked. We cannot change the model; we *can* supply the
   priors in docs and test whether behavior changes. That is the point of the
   holdout.
5. **API deficiency (weakest; essentially absent for composition).** No expressive
   wall. Only *ergonomic* frictions (§E): no full-frame-hero one-liner;
   `Node::canvas` undocumented; `Camera::project` frustum-rejection. Documented,
   not doctrine-blocking. The genuine product gaps the campaign tracks (G1
   interaction identity, scene lifecycle, etc.) are **orthogonal** to the visual
   verdict — see §I.

---

## H. Smallest next experiment

**Phase 1 — documentation only, no new public API.** Supply the missing priors
and the missing demonstrations, and make them reachable from the cold-start path:

1. `docs/AI_VISUAL_AUTHORING.md` — a compact (~250–500 line) doctrine optimized
   for a coding agent: the three-layer model (domain → visual law → realization),
   the Dominant Visual Object rule, the Panel-Farm anti-pattern, the
   custom-world + sparse-chrome pattern, a decision tree, and a contrastive
   before/after.
2. `examples/recipe_*.rs` — five tiny, legible recipes, each teaching **one**
   composition law, each with a deterministic `--capture WxH[:depth]` mode so
   visual acceptance is a first-class run mode. (Built and verified this round.)
3. `AGENTS.md` + a link from `UI_LAYER.md` — a short router so a fresh agent is
   steered to the doctrine *before* the panel-farm quickstart.
4. `AI_VISUAL_AUTHORING_HOLDOUT.md` — a preregistered blind test on fresh
   non-Sonnet model contexts: control (v0.4.0 docs) vs treatment (docs + guide +
   recipes), measuring whether design *behavior* changes.

**Phase 2 — only if the holdout + recipes show repeated, redundant manual glue:**
a *design proposal* (not code) for a thin full-frame-hero / composition-role
helper. See `VISUAL_INTENT_API_PROPOSAL.md`. The charter forbids promoting a core
feature from one consumer; the bar is repeated evidence and a tiny change.

---

## I. Explicitly NOT this problem (product bugs kept separate)

The v0.4.0 consumers filed genuine product findings (upstream issues
#73–#79): `Story::start()` unknown-beat semantics (#73), offline PCM
materialization (#74), `ReferenceSong` axis coverage (#75), `Effect::Shake/Jitter`
duration (#76), `Scene` entity removal (#77), non-contiguous `RenderCtx::start`
(#78), terminal-hangup busy-spin (#79). These are **real and orthogonal** to the
visual-composition verdict. Do not conflate "the renderer has a bug" with "the
model composed a panel farm." This round touches none of them.

---

## Boundary

- This audit explains a **human aesthetic verdict**; it does not and cannot prove
  visual quality by test.
- Consumer layout fractions are arithmetic from layout constants (the bearings
  did not run the panel-farm binaries); the Chronoscope percentages and the
  cinematic verdict are corroborated by its committed PNGs, inspected directly.
- The weak-agent *reasoning* ("they lacked priors") is **Conjectured** from
  behavior and artifacts; the holdout (§H.4) is designed to test it.
- The `screen().height()` collapse and the `Node::canvas` hero path were
  **reproduced** this round (compiled + captured); see the recipes.
