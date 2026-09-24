# LibGibson Expressivity & API Audit for Agent-Native Dynamic UI

> **Campaign update:** the external program has now completed. See the
> [recorded results](AGENT_NATIVE_UI_CAMPAIGN_RESULTS_2026-09-24.md) for frozen revisions,
> three post-freeze holdout verdicts and limits. This proposal/audit below is
> preserved as preregistration; it creates no core commitment.

**Date:** 2026-09-24

**Audited baseline:** public `main` at `e5ede0a4ab8ff52caa567ee10d45824e42c1ebc6`

**Status:** NON-BINDING RESEARCH / PRESSURE TEST ONLY / NO NEW CORE COMMITMENT

**Provenance:** the original dossier at research commit `bc4d1ce748ffa67121104ac39d960ba4a82736d3` audited `3438c89645c760201be4b2598cf12161efada782`. This refresh follows the merge of [PR #14](https://github.com/femboy2112/libgibson/pull/14), preserving the original proposal while updating its evidence and holdout protocol.

### Baseline evidence

The merge commit contains flagship tip `11d2cca12430f68eb8fa3de3231827e1176e6c27`.
Exact-main public [CI run 35960000454](https://github.com/femboy2112/libgibson/actions/runs/35960000454)
completed all five jobs successfully: Rust, C/C++/Python bindings, ASan/UBSan, Go,
and PTY. This is evidence from the merged main, not substitution of the earlier PR run.

Local merge-baseline validation on Rust 1.98.1 passed: `cargo +1.98.1 fmt --check`,
`cargo +1.98.1 clippy --all-targets --all-features -- -D warnings`,
`cargo +1.98.1 test`, `cargo +1.98.1 build --release`,
`cargo +1.98.1 build --examples`, `cargo +1.98.1 build --release --examples`,
`RUSTDOCFLAGS="-D warnings" cargo +1.98.1 doc --no-deps`,
`cargo +1.98.1 test --example fx_lab`,
`RUSTUP_TOOLCHAIN=1.98.1 bash scripts/dev/bindings_smoke.sh --asan`, and
`git diff --check`. The authoritative suite is **574 tests: 226 unit + 348 integration**;
the explicit FX Lab command adds three example tests outside that total.

### Merged flagship: corroborating composition evidence

The introductory film realizes ordinary `Node` mini-applications into local
`Surface`s, mounts their cells inside projected physical facade regions, and
combines them with world-space vector geometry in a moving wireframe city.
The implementation is demo-local ([facade](../../examples/libgibson_intro/facade.rs),
[world projection](../../examples/libgibson_intro/world.rs)); it did not add
`BuildingWidget`, `WorldSpacePanel`, `FacadeUI`, `TextureEngine`, or `AgentBuilding`
to core. Native text is mounted during readable near-frontal holds; this does not
claim arbitrary perspective texture mapping.

**CORROBORATING EVIDENCE:** high-level `Node` plus public `Surface`, raster, and
geometry escape routes support surprising composition without renderer changes.
This is authored, deterministic, presentation-driven evidence from the same
development lineage, not an independent external-consumer holdout and not proof
of arbitrary runtime-generated UI. It leaves G1 below unresolved: persistent
semantic identity ↔ interaction target ↔ user action ↔ changing representation.

## 0. Claim discipline

This is an API pressure audit, not a claim that the three proposed harnesses already work.

Labels:

- **PRESENT** — public API exists on audited main.
- **PRESENT / LOW-LEVEL** — possible today but requires manual composition.
- **PARTIAL** — substrate exists but a generic interaction/ergonomic piece is missing.
- **GAP** — no clean public route identified.
- **DEFER** — desirable only after external experiments demonstrate repeated need.

## 1. Current capability ladder

### 1.1 Structured UI — strong

Public Node surface already includes:

- row / column / stack;
- border/panel/rule/rail;
- text / rich text;
- text input;
- viewport;
- signed offset positioning;
- raster/surface embedding;
- percentage/min/max sizing;
- padding/gap/alignment/justification;
- ordered `SurfaceFx`.

**Assessment:** PRESENT.

This is enough for professional conventional layouts and many stylized compositions.

### 1.2 Stable semantic scene identities — strong

`Scene` provides:

- stable `SceneId`;
- labels and tags;
- z ordering;
- offsets/visibility;
- node replacement;
- scene evaluation;
- `Effect` sequence/parallel composition;
- displacement, reveal, camera, shake, post-process, custom channels.

**Assessment:** PRESENT, with mutation ergonomics gaps.

Important current limitation: creation and node replacement exist, but the public API is not yet a general retained dynamic scene-store interface. Removal/unmount and stable application-level interaction binding deserve pressure testing before promotion.

### 1.3 Story/event sequencing — strong but specialized

`Story` / `StoryDirector` provide deterministic beats, facts, actions, reactions, trace/replay, and exact update steps.

**Assessment:** PRESENT for narrative/state-machine flows.

**Warning:** do not force arbitrary agent sessions into Story. A tool-call stream is not automatically a screenplay. The harness should own its semantic event log; Story is optional when its graph semantics are useful.

### 1.4 Surface and compositing escape hatch — strong

`Surface` is public and supports transparent compositing and clipping.

**Assessment:** PRESENT.

This is the most important anti-canned-code fact in the current design. A caller can create arbitrary cell-level output without touching ANSI.

### 1.5 RGB software raster — strong

`RgbRaster` exposes bounded dimensions, pixel access, blending, lines, discs, half-block realization, and Mono realization.

**Assessment:** PRESENT.

This already permits a large class of novel effects. The 2048-axis hard bound is a useful safety property for generated interfaces.

### 1.6 Sub-cell / vector-ish primitives — useful but incomplete

`BrailleCanvas` and `HalfBlockCanvas` provide lines and basic shapes. `RgbRaster` has lines/discs. `geom` has meshes/projectors. `raster3d` has filled triangles.

Merged PR #14 adds public `geom::CubicPath3::sample` and `tangent`, motivated by
cameras and semantic couriers. It is now part of the pinned substrate, not a
pending research promotion.

**Assessment:** PRESENT / LOW-LEVEL; generic path vocabulary is a plausible promotion candidate.

Repeated pressure likely wants:

- sampled polyline/path abstraction;
- cubic/quadratic curves;
- normalized coordinate helpers;
- strokes;
- possibly fill/mask construction.

Do **not** immediately build SVG.

The correct next step is to see which tiny path operations recur across two external harnesses.

### 1.7 Procedural graphics — strong

Current public modules include:

- fields;
- particles;
- transitions;
- glitch primitives;
- `RasterFx`;
- `SurfaceFx`;
- feedback buffers;
- wireframe and filled 3D.

**Assessment:** PRESENT.

The main ergonomic issue is discoverability/composition, not raw expressivity.

### 1.8 Deterministic time/replay — strong

Fixed clocks, Story replay, deterministic seeded systems, and ordinary frame scheduling exist.

**Assessment:** PRESENT.

Generated instruments should use these instead of inventing their own timers.

### 1.9 Capability fallback — good but not complete

Color depth and capability representation exist; Mono/ANSI/TrueColor fallback is already real.

Active negotiation and broader terminal protocols are still roadmap work.

**Assessment:** PARTIAL, adequate for the first experiments.

Generated visual specs should declare required/optional capabilities and have a deterministic fallback.

## 2. Primary gaps for the three experiments

## G1 — Stable interaction identity and event routing

Current input event surface is:

- Key;
- Paste;
- Resize;
- Tick.

Focus is a small flat `FocusRing`.

There is no general:

- hierarchical hit-test tree;
- event bubbling/capture;
- mouse;
- semantic interaction ID attached directly to arbitrary nodes/scenes;
- generated-widget event-binding layer.

This is the largest **predicted** shared pressure point. `FocusId`, `FocusRing`,
and application-owned reducers already offer a public keyboard route; their
adequacy and repeated mapping cost must be measured before calling G1 an earned
core-promotion requirement. The cinema's persistent visual identities do not
test runtime-created interactive objects or close this gap.

### Proposed direction

Do not immediately turn Node into a browser DOM.

First external experiments should define a harness-side:

```text
InteractionId
InteractionRegion
Action
EventBinding
```

and use keyboard focus initially.

Promote a LibGibson primitive only when two different experiments need the same stable interaction mapping.

Likely eventual generic primitive:

```text
InteractiveRegion {
    id,
    rect/hit policy,
    focusable,
    event mask
}
```

The renderer should report semantic events; application/harness owns meaning.

## G2 — Agent-safe declarative instrument representation

LibGibson exposes Rust objects, not a bounded serializable representation suitable for arbitrary model output.

**Assessment:** GAP by design.

### Proposed direction

Prototype **outside LibGibson**.

The external IR should compile into ordinary Node/Scene/Surface operations.

Only after the schema survives holdouts should any stable subset be considered for a dedicated integration crate.

Do not put an LLM-specific schema in the renderer crate.

## G3 — Capability-scoped session authority

`Context` exposes a practical application API, but giving an agent direct mutable access to it is too much authority and too little semantic structure.

**Assessment:** GAP at integration layer, not core renderer.

### Proposed direction

Define a harness-owned `SessionPort` / `PresentationPort`.

The port may:

- snapshot allowed semantic state;
- mount/update/unmount an instrument;
- request focus;
- subscribe to a bounded event set;
- emit structured application actions;
- commit finalized output.

The port may not expose raw ANSI or arbitrary renderer mutation.

## G4 — Dynamic scene lifecycle ergonomics

`Scene::try_add`, `entity_mut`, and `set_node` exist.

Pressure tests will likely require:

- remove entity;
- replace entity while preserving identity where legal;
- temporary mount scopes;
- bulk transactional scene patch;
- tag/query helpers;
- validation before apply.

**Assessment:** PARTIAL.

Do not add all of these speculatively. The generated-instrument experiment should identify which are actually load-bearing.

## G5 — Irregular 2D composition without heroic raster code

Manga panels, speech balloons, arrows, diagrams, and custom callouts can be drawn today through Surface/Raster/Canvas. That is expressive but can be verbose.

**Assessment:** PRESENT / LOW-LEVEL.

Candidate ergonomic layer:

- generic 2D path;
- stroke;
- fill;
- clip/mask;
- transforms;
- normalized coordinates.

Promotion requires repeated demand. A `MangaPanel` is forbidden as a core abstraction.

## G6 — Retained widget-local state

LibGibson intentionally treats Node largely as declarative presentation. Generated instruments need state such as selection, expansion, viewport, drag/hover later, etc.

**Assessment:** PARTIAL at application level.

The likely architecture is not stateful Nodes. It is:

```text
InstrumentState -> render(state) -> Node/Scene
event -> reducer(state,event) -> new state
```

This keeps renderer semantics clean and supports replay.

LibGibson may only need better generic interaction IDs and viewport/focus helpers.

## G7 — Resource budgets for untrusted/generated presentation

Existing pieces have individual limits, e.g. `RgbRaster::MAX_RASTER_DIMENSION`, but there is no one budget object for a generated UI.

**Assessment:** GAP at integration layer.

Prototype a harness-owned budget:

```text
PresentationBudget
  max_nodes
  max_depth
  max_scene_entities
  max_raster_pixels
  max_effects
  max_particles
  max_animation_hz
  max_trace_steps
  max_retained_bytes
```

The validator must reject, not silently truncate semantic structure, unless truncation is explicitly part of the contract.

If multiple external consumers need the same resource contract, consider a LibGibson utility type later.

## G8 — Introspection and explainability

LibGibson has render stats, damage inspection, scene debug lines, and deterministic structures.

Generated interfaces additionally need:

- spec validation errors with paths;
- explainable fallback decisions;
- mounted instrument inventory;
- event/subscription inventory;
- resource-budget accounting;
- "why this node exists" metadata at the harness layer.

**Assessment:** PARTIAL.

Keep semantic provenance outside the cell renderer. The engine can expose structural metrics.

## 3. API friction matrix

| Requirement | Current route | Status | Promotion pressure |
|---|---|---:|---|
| Professional text/layout | Node/Taffy/RichText | PRESENT | low |
| Overlay/modal | Stack/Dim/offset | PRESENT | low |
| Scroll/camera | Viewport/ViewportState | PRESENT | low |
| Custom cell art | Surface | PRESENT | none |
| Custom RGB art | RgbRaster | PRESENT | none |
| Sub-cell line art | Braille/HalfBlock | PRESENT | low |
| 3D visuals | geom/raster3d | PRESENT | low |
| Procedural visuals | field/particles/fx | PRESENT | low |
| Scene identity | SceneId/tags | PRESENT | medium |
| Dynamic scene add/replace | Scene | PARTIAL | medium |
| Dynamic remove/mount patch | no clean generic contract | PARTIAL/GAP | medium |
| Sequential presentation | Story/Effect | PRESENT | low |
| General agent event model | application-owned | CORRECTLY EXTERNAL | none |
| Hierarchical interaction | not present | GAP | high |
| Mouse hit testing | roadmap only | GAP | medium |
| Agent-safe generated UI IR | none | CORRECTLY EXTERNAL | high at harness layer |
| Session capability handle | none | CORRECTLY EXTERNAL | high at harness layer |
| Unified presentation budget | none | GAP | medium |
| Generic path/curve | low-level lines; merged `geom::CubicPath3` | PRESENT / LOW-LEVEL | measure remaining 2D ergonomics |
| Irregular masks/shapes | effects/raster/manual | PARTIAL | medium |
| Theme schema | semantic Theme exists | PARTIAL | medium |
| Raw unknown visual escape | Surface/Raster | PRESENT | critical strength |

## 4. Proposed API philosophy

### 4.1 Public primitive completeness over widget catalog size

The test is not "how many widgets ship?"

The test is:

> Can a caller build a new widget with public, safe, bounded primitives?

### 4.2 Convenience APIs should desugar

A high-level helper should ideally compile down to public lower layers rather than unlock hidden renderer magic.

This preserves learnability and prevents the "canned code" trap.

### 4.3 Keep semantic state outside presentation nodes

Prefer:

```text
state -> render -> Node
```

to embedding arbitrary application state machines inside Node.

Scene identity is useful for presentation continuity. It should not become a universal application object model.

### 4.4 Make costs inspectable

For generated interfaces, expose enough metrics to answer:

- how many nodes/entities?
- how many pixels?
- how much damage?
- how many wire bytes?
- how much retained history?
- how long did generation/paint/diff take?

A polished dynamic UI that silently produces unbounded work is not acceptable.

### 4.5 Every advanced layer needs a lower escape route

Examples:

- panel helper -> Node/Surface;
- chart helper -> Canvas/Raster;
- Scene effect -> SurfaceFx/RasterFx;
- generated widget -> Node/Scene/Surface;
- cinematic path helper -> generic path sampling.

No feature should require private renderer surgery solely because the library authors did not imagine it.

## 5. Candidate small generic improvements

These are **candidates**, not a build list.

1. **Generic path sampling**
   - Merged `CubicPath3` is available now; use it before proposing more types.
   - Consider a tiny `Path2/Path3` family only if another consumer needs it.

2. **Scene patch/lifecycle utility**
   - validated add/remove/replace/update operations;
   - stable identity semantics explicit.

3. **Interaction IDs**
   - stable semantic identity for focus/action routing;
   - decoupled from visual style.

4. **Public hit regions**
   - keyboard first; mouse later.

5. **2D normalized geometry helpers**
   - map normalized coordinates into Rect/Raster;
   - reduces resize boilerplate.

6. **Generic masks / clip paths**
   - only if manga + generated instrumentation both need them.

7. **Budget/limits helper**
   - likely integration crate first.

8. **Theme token extension**
   - configurable semantic tokens, not CSS.

## 6. Things we should resist

- baking LLM concepts into `NodeKind`;
- adding one Node variant for every fancy visual;
- model-generated native code as the default extension path;
- raw ANSI as "advanced mode";
- hard-coded anime/meme/cinematic concepts in core;
- stateful implicit clocks in widgets;
- hidden global session mutation;
- unbounded generated surfaces;
- adding a general plugin system before a declarative IR is proven insufficient.

## 7. Verdict

### Current substrate

**CORROBORATED as unusually expressive for the tested terminal domain.**

Why:

- high-level declarative UI exists;
- semantic scene identity exists;
- deterministic effects/story exist;
- low-level cell and RGB escape hatches exist;
- custom graphics do not require ANSI;
- current cinematic work has repeatedly built genuinely new visuals on the existing pipeline.

### Largest unresolved question at preregistration

**Probe-gap:** we have not yet frozen the public API and asked a fresh implementer to build ambitious, previously unspecified interfaces without modifying LibGibson.

That is the lamp.

Rendering expressivity is increasingly corroborated within the authored examples;
dynamic semantic interaction is now the principal pressure frontier. The wider
external-consumer claim remains **UNVERIFIED** until the planned experiments and
post-freeze holdouts run. Project maturity remains **ENGINEERING ALPHA**;
Scene/Story/software graphics remain experimental Rust APIs.

The next step is not adding more core features.

That planned program has since completed; the [campaign results](AGENT_NATIVE_UI_CAMPAIGN_RESULTS_2026-09-24.md) supersede this historical probe-gap. G1 was observed as ergonomic pressure with a working public route; no core promotion was justified.
