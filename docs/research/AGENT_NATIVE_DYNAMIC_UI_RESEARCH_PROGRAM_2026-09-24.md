# Agent-Native Dynamic Interfaces: LibGibson Research Program

**Status:** NON-BINDING RESEARCH / PRESSURE TEST ONLY / NO NEW CORE COMMITMENT

**Date:** 2026-09-24

**Initial experimental substrate:** merged public `main` at `e5ede0a4ab8ff52caa567ee10d45824e42c1ebc6`

**Dossier provenance:** original research tip `bc4d1ce748ffa67121104ac39d960ba4a82736d3`, refreshed after PR #14 merged. The original research branch is preserved.

**Normative effect on LibGibson main:** NONE. This dossier proposes stress tests and promotion criteria. It does **not** make manga, memes, an agent harness, or an agent-generated widget language part of LibGibson core.

The merged baseline passes **574 tests (226 unit + 348 integration)** and all five
public jobs in [run 35960000454](https://github.com/femboy2112/libgibson/actions/runs/35960000454).
See the [audit's baseline evidence](LIBGIBSON_EXPRESSIVITY_API_AUDIT_2026-09-24.md#baseline-evidence)
for exact local commands and provenance. The external lab must pin this revision;
moving main is not an experimental substrate.

The merged introductory cinema is **CORROBORATING EVIDENCE** that ordinary Node
mini-UIs, public Surface/raster/geometry, and demo-local projection can compose
into an unforeseen-looking world without adding building/facade widgets to core.
It is authored presentation, not evidence that arbitrary runtime-created objects
bind stable identity to focus, actions, and changing representations. **G1 remains
unresolved**; dynamic semantic interaction is the next pressure frontier.

## 0. Why this exists

LibGibson has crossed the point where the interesting question is no longer merely:

> Can a terminal application render a polished dynamic interface?

The next question is:

> Can an AI-native application choose or invent a useful visual/interactive representation at runtime without the engine needing to know that representation in advance?

Three concrete harness ideas expose three different axes of that question:

1. **Manga / comic-book harness** — conversation and agent orchestration rendered through a dynamic sequential-art grammar: panels, gutters, splash frames, interruptions, parallel panels, overlays, motion grammar, and compositional transitions.
2. **Meme-reactive harness** — semantic session events may trigger carefully selected visual meme reactions that are funny because they match the event structure, not because a random image was inserted. The meme layer can also become interaction: a joke can be a warning, explanation, or branch selector.
3. **Agent-generated instruments** — the agent can inspect the current task/session, decide that prose is the wrong representation, and dynamically instantiate a bounded custom interface: graph, dependency explorer, timeline, proof-state viewer, source-provenance map, selector, simulation surface, etc.

These should **not** become three LibGibson products. They are pressure tests for one deeper property:

**information structure → interaction geometry**

The research goal is to make **known dope shit easy** while preserving a route for **currently unknown dope shit** through composable lower-level APIs.

## 1. The non-negotiable architectural boundary

LibGibson should remain a presentation and interaction substrate.

It should not learn what:

- manga is;
- a meme is;
- an LLM is;
- a prompt is;
- a tool call is;
- a compiler dependency graph is;
- a proof obligation is.

Those belong above the engine.

The desired architecture is:

```text
Application / Agent Semantics
        |
        +--> Semantic event stream
        |
        +--> Presentation grammar
        |       - restrained/professional
        |       - manga/comic
        |       - cinematic
        |       - meme-reactive
        |
        +--> Instrument planner
                - graph
                - timeline
                - inspector
                - custom visualization
                        |
                        v
              validated declarative spec
                        |
                        v
                 Session presentation
                        |
       +----------------+----------------+
       |                |                |
      Node             Scene          Surface/Raster
       |                |                |
       +----------------+----------------+
                        |
                     Painter
                        |
                     Surface
                        |
                      Diff
                        |
                      ANSI
                        |
                    Terminal
```

A style-specific harness may use every public layer. It must not force its vocabulary downward into the renderer.

## 2. The four-layer rule: convenience without imprisonment

The core API should have a **ladder**, not one blessed abstraction.

### Layer A — boring high-level composition

For common UI:

- `Node`
- rows / columns / stack / panels / text / rich text / input
- standard focus and viewport behavior
- semantic themes

This layer should let normal agent CLIs remain concise.

### Layer B — semantic scene composition

For dynamic identity-preserving presentation:

- `Scene`, `SceneEntity`, `SceneId`, tags
- `Presentation`
- `Effect`, `EffectBundle`
- `Story`, `StoryDirector` when a story graph is actually appropriate

This is where "same semantic object, different visual realization" should live.

### Layer C — graphics construction

For custom graphics that should still be easy:

- `Surface`
- `RgbRaster`
- Braille / half-block canvases
- wireframe and filled 3D
- particles
- procedural fields
- raster/surface effects
- generic geometry/path primitives

This layer should make custom visuals possible without touching ANSI or renderer internals.

### Layer D — primitive escape hatch

For unknown future work:

- direct, safe cell/surface operations;
- explicit clipped raster access;
- compositing;
- public coordinate/geometry math;
- deterministic clocks;
- capability information.

**Success criterion:** an unknown visualization may be awkward at Layer D, but it should not require modifying LibGibson internals just to become possible.

The engine must not solve flexibility by exposing unsafe terminal protocol everywhere. The escape hatch is **Surface/Raster/Scene composition**, not raw ANSI.

## 3. Experiment A — manga / comic-book flow

### 3.1 Thesis

Sequential art is an excellent model for agent systems because layout itself can encode concurrency, interruption, importance, causality, pacing, and synthesis.

A conventional chat renders:

```text
user
assistant
tool
assistant
subagent
subagent
assistant
```

A comic grammar can render the same semantic trace as:

- a wide establishing panel for the user goal;
- narrow parallel panels for independent agent workers;
- a panel breaking the gutter for an unexpected tool failure;
- a splash panel when a major result lands;
- a reconvergence panel when the coordinator synthesizes;
- tiny marginal panels for low-importance telemetry;
- gutters that encode temporal separation;
- overlapping panels that encode interruption or concurrency.

### 3.2 Required engine affordances

The harness should be implementable without manga-specific core features. It needs generic support for:

- responsive nested layout;
- overlapping/offset nodes;
- clipping;
- irregular graphics via Surface/Raster;
- animated transforms and visibility;
- semantic identity across relayout;
- text wrapping and rich typography;
- stable focus/input during presentation changes;
- deterministic animation;
- transition/effect composition;
- a way to render decorative gutters, speed lines, impact fields, captions, balloons, and callouts from generic primitives;
- monotone fallback to ANSI16/Mono without destroying information hierarchy.

### 3.3 What must remain outside LibGibson

- panel-selection policy;
- manga vocabulary;
- "splash page" semantics;
- narrative importance inference;
- speech/thought/caption classification;
- art-direction rules.

The harness owns those.

### 3.4 Hard acceptance probe

Freeze the public LibGibson API, then implement a manga harness externally.

Pass only if:

1. ordinary conversation, tool calls, parallel workers, interruptions, and synthesis all render through one semantic event model;
2. resizing does not require rebuilding renderer internals;
3. a mono terminal preserves the semantic panel order;
4. input/focus remain usable while panels animate;
5. the harness needs **zero renderer/private-module patches**;
6. at least one visual element not anticipated in this document can be implemented using public lower-level APIs.

That final condition tests "unknown dope shit," not merely the planned aesthetic.

## 4. Experiment B — semantic meme reaction layer

### 4.1 Thesis

A useful meme layer is not:

```text
if error:
    show_meme()
```

It is a typed reaction policy over semantic events.

Example event vocabulary, owned by the harness:

```text
AbsurdRequest
RepeatedFailure
Overengineering
UnexpectedSuccess
ScopeExplosion
PermissionRequest
Contradiction
UserOwnGoalConflict
AgentOverconfidence
BoringSuccess
```

A reaction asset declares where it fits and where it is forbidden.

Conceptually:

```text
ReactionAsset {
    semantic_roles: [ScopeExplosion, AbsurdRequest],
    exclusions: [SeriousDistress, SafetyCritical],
    intensity_range: ...,
    cooldown: ...,
    presentation: ...
}
```

The funny case is when the meme becomes an instrument.

Example:

```text
fix parsing bug
      |
rewrite parser
      |
design language
      |
replace compiler backend

[ local repair ]   [ architecture expedition ]
```

The meme layer may frame the escalation, but the actual decision is a structured interaction event.

### 4.2 Required engine affordances

- transient overlay mounting/unmounting;
- z-order and dimming;
- effect timing;
- image-like rendering using terminal-native surfaces or authorized external assets;
- animation without corrupting scrollback;
- interaction regions/focus;
- event emission back to the harness;
- per-reaction resource limits;
- capability fallback;
- semantic accessibility/fallback text.

### 4.3 Asset boundary

LibGibson core should bundle **no copyrighted meme catalog** and no hard-coded public-figure reactions.

The reaction layer may reference user-supplied, licensed, public-domain, or otherwise authorized assets. A terminal-native recreation can also be generated from primitives. This is a harness/content-policy concern, not renderer semantics.

### 4.4 Hard acceptance probe

Run the same semantic event trace through:

- no reaction layer;
- restrained reaction layer;
- meme layer.

The underlying agent decisions and event trace must be invariant except where the user explicitly chooses an interaction exposed by the reaction.

If changing presentation changes hidden semantics, the layering failed.

## 5. Experiment C — agent-generated instruments

### 5.1 Thesis

This is the highest-value experiment.

The agent encounters information and chooses a representation:

**task state → representation choice → temporary interactive instrument**

Examples:

- dependency graph;
- causal graph;
- hypothesis board;
- proof-state explorer;
- timeline;
- source-provenance map;
- route/path explorer;
- side-by-side semantic diff;
- decision matrix;
- live simulation control surface.

The instrument may exist for thirty seconds and disappear after the task resolves.

### 5.2 Do not hand the model arbitrary Rust

The first architecture should **not** be:

> generate Rust, compile it, dlopen it into the running terminal process.

That destroys the main properties LibGibson has earned: boundedness, deterministic replay, inspection, and meaningful validation.

The first architecture should be a small declarative **Widget / Instrument IR** owned by an external harness or integration crate.

Example shape:

```text
InstrumentSpec
  id
  title
  root:
    Split(horizontal)
      left: Graph(...)
      right: Inspector(selection = graph.selected)

  bindings:
    graph.select(node) -> emit Inspect(node)
```

The exact syntax is deliberately not frozen here.

### 5.3 Session access should be capability-scoped

The user concept "agent has a pointer to the session" is right at the semantic level but should not become a raw process pointer.

Prefer a capability surface such as:

```text
SessionPort
  read_semantic_snapshot()
  mount_instrument(spec)
  update_instrument(id, patch)
  unmount_instrument(id)
  subscribe_events(id, ...)
  request_focus(id)
  emit_semantic_action(...)
  commit_output(...)
```

The agent does not receive:

- arbitrary renderer mutation;
- arbitrary terminal writes;
- raw memory;
- unrestricted event interception.

The port is a deliberately narrow authority boundary.

### 5.4 Validation before mounting

Every generated instrument should be validated for:

- schema/type correctness;
- unique stable IDs;
- maximum node count/depth;
- maximum raster/canvas dimensions;
- maximum effects;
- maximum animation rate;
- bounded retained history;
- allowed input subscriptions;
- supported capability fallbacks;
- no raw ANSI;
- no unknown action target;
- deterministic initial state when replay is requested.

Refusal is a valid result.

### 5.5 The crucial generality test

Do **not** grade the generated-interface system only on widgets used to design it.

The original dossier named the following design examples:

- a finite-state-machine debugger;
- a SAT/SMT counterexample explorer;
- a music structure viewer;
- a Git branch-topology instrument;
- a resource-flow simulation.

These examples are now **CONTAMINATED** and cannot count as fresh holdouts.
After both substrate and lab helper/IR SHAs are frozen, a fresh reasoning context
receives only frozen API documentation and invents five candidates. Select three
for diversity, usefulness, visual ambition, and architectural stress, screening
against this entire dossier. Record selection provenance before implementation.
Do not give the selector this design-example list or change the substrate to
accommodate a chosen holdout.

If every holdout demands a new core widget type, the abstraction is canned.

If the holdouts compose existing primitives, the abstraction is earning generality.

## 6. Presentation grammar, reaction layer, and generated instruments are orthogonal

Define the harness conceptually as:

```text
Harness = Semantic Session + Presentation Grammar
        + Reaction Policy + Generated Instruments
```

These axes should compose.

Examples:

- a generated dependency graph inside manga panels;
- a professional theme with generated proof widgets and no memes;
- a meme reaction that opens a real scope-control selector;
- the same instrument rendered under restrained, manga, and cinematic grammars.

This provides a strong architectural probe: if the three axes cannot vary independently, the harness has entangled semantics and presentation.

## 7. The "make dope shit easy" rule

A good API has two properties at once.

### 7.1 Low activation energy

A competent application author should be able to produce polished dynamic output without:

- hand-writing ANSI;
- manually managing wide-glyph continuation cells;
- rebuilding clipping;
- rebuilding color fallback;
- rebuilding frame scheduling;
- rebuilding terminal restoration;
- writing bespoke PTY logic.

### 7.2 No expressive ceiling disguised as convenience

The convenience API must not trap advanced authors inside a catalog of canned widgets.

Whenever an abstraction is introduced, ask:

> What can the caller do when this abstraction is almost right but not quite?

Desired answers:

- compose lower-level primitives;
- provide a Surface;
- draw into a bounded Raster;
- build a Scene;
- use generic paths/effects;
- retain semantic event handling above the engine.

Undesired answer:

> fork the renderer.

## 8. Promotion rule for LibGibson core

Nothing in these experiments enters LibGibson core merely because one harness needs it.

A new core primitive should satisfy at least one of:

1. **Two orthogonal consumers** independently need the same mechanism.
2. The primitive closes a generic correctness/ergonomics gap already visible in the engine.
3. The primitive is a minimal lower-level operation from which multiple higher-level constructs compose.
4. Lack of the primitive forces callers into unsafe/raw terminal behavior.

Examples of plausible promotions:

- generic path sampling;
- reusable clipping/mask primitives;
- stable interaction IDs and event routing;
- bounded scene mutation operations;
- resource-budget types;
- generic gradients/strokes;
- layout-independent semantic hit regions.

Examples that should stay external:

- MangaPanel;
- MemeReaction;
- FilthyFrankWidget;
- AgentToolCallCard;
- CompilerDependencyViewer.

## 9. "Unknown dope shit" holdout

This is the most important acceptance criterion in the program.

After a candidate API is frozen, ask a fresh model/person to invent an ambitious visual interaction **not represented in the design examples**.

Constraints:

- public API only;
- no LibGibson source edits;
- no raw ANSI;
- deterministic replay when seeded;
- bounded memory/work;
- works at TrueColor and Mono;
- usable after resize;
- basic interaction possible if relevant.

If the fresh concept can be built cleanly by dropping to public lower layers, mark the expressivity claim **CORROBORATED within the tested domain**.

If it appears to need a core patch, leave the holdout failed and classify why:

- missing primitive;
- ergonomics only;
- actual expressive wall;
- deliberate safety boundary.

Do not add a feature during the frozen campaign. First determine which category
it is. Helper/IR schema changes are also forbidden, except a recorded bug that
prevents the harness itself from running; such an exception must not add the
missing expressive capability. Failed holdouts remain in the final verdict.

## 10. Non-goals

This research program does not currently propose:

- a browser/DOM clone;
- CSS;
- a game engine;
- an unrestricted plugin ABI;
- model-generated native code;
- a general multimedia framework;
- manga/meme assets in the LibGibson crate;
- a stable serialized Widget IR in LibGibson core;
- replacement of Node/Scene/Story.

The point is to discover what **minimal generic substrate** lets external systems build those experiences.

## 11. Thesis under test and completion condition

The program succeeds if we can truthfully say:

> LibGibson makes common professional dynamic terminal graphics easy, lets ambitious applications compose sophisticated experiences from generic public primitives, and still exposes a safe lower-level route when the desired visualization was not anticipated by the library author.

That is a stronger claim than "LibGibson has many widgets."

It is also falsifiable.

Completing the research does not require this thesis to survive. Complete the
campaign when all planned experiments and three post-freeze holdouts have recorded
outcomes, friction has been reconciled, and any promotion decision has evidence.
Use **OBSERVED**, **CORROBORATED**, **EXPERIMENTAL**, **UNVERIFIED**, and **REFUTED**
with an explicit tested domain. Three successful holdouts would corroborate a
bounded domain, not prove universal UI expressivity.

See:

- [Expressivity/API audit](LIBGIBSON_EXPRESSIVITY_API_AUDIT_2026-09-24.md)
- [Experiment and acceptance plan](AGENT_NATIVE_UI_EXPERIMENT_PLAN_2026-09-24.md)
- [Zero-context handoff](AGENT_NATIVE_UI_ZERO_CONTEXT_HANDOFF_2026-09-24.md)
