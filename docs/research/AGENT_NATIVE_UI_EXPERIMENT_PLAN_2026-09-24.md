# Agent-Native UI Experiments and Acceptance Plan

**Status:** PREREGISTRATION / NO IMPLEMENTATION ON THIS BRANCH  
**Purpose:** turn the manga, meme, and generated-instrument ideas into discriminating tests of LibGibson's public API.

## 1. Rules of the campaign

1. **LibGibson main is the substrate, not the app repo.**
   Actual harnesses should eventually live in separate repositories or a dedicated external test workspace.

2. **No core patch during the first implementation pass.**
   When friction occurs, record it. Do not immediately "fix" LibGibson.

3. **Classify every friction point.**
   - missing generic primitive;
   - ergonomic inconvenience;
   - deliberate safety boundary;
   - harness-specific concern;
   - genuine expressive wall.

4. **Promote only repeated generic pressure.**
   One weird harness does not get to redesign core.

5. **Keep semantic traces presentation-independent.**
   A presentation grammar may alter what is shown and how. It must not silently alter agent/session decisions.

6. **Preserve low-level public escape routes.**
   The point is not to force every visual through a high-level widget schema.

## 2. Common semantic fixture

All three experiments should initially consume the same deterministic fake agent-session trace.

Suggested events:

```text
SessionStarted
UserMessage
AgentStarted(id)
PlanUpdated
SubagentSpawned(parent,id)
ToolStarted(id,name)
ToolProgress(id,pct)
ToolFinished(id,result_summary)
AgentMessageChunk(id,text)
Warning(kind)
PermissionRequested(id,choices)
PermissionResolved(id,choice)
ArtifactProduced(id,kind)
AgentFinished(id,outcome)
SessionFinished
```

This fixture is deliberately not a LibGibson type.

It belongs to the external harness and makes it possible to test:

[
	ext{same semantics} ightarrow 	ext{different presentations}
]

## 3. Experiment A: Manga grammar

### Build

Render the common fixture using:

- responsive panel composition;
- parallel subagent panels;
- gutters as time/concurrency separators;
- one interruption that visually crosses a gutter;
- one full-width synthesis/splash moment;
- rich captions/dialog;
- deterministic speed/impact graphics;
- keyboard selection of an active panel;
- resize at 56x24, 80x24, 120x32, 160x40;
- TrueColor, ANSI16, Mono.

### Pass

- no LibGibson source modification;
- no raw ANSI;
- semantic event order preserved;
- one visual state can be replayed exactly from event trace + time;
- keyboard interaction remains usable;
- no unbounded retained frame history;
- minimum readable fallback in Mono;
- implement at least one unplanned panel effect after the initial grammar is "done".

### Record

- public APIs touched;
- custom helper LOC;
- places where raw Surface/Raster was necessary;
- places where engine internals were desired;
- damage/wire metrics on representative frames.

## 4. Experiment B: Meme reaction layer

### Build

Create an external semantic reaction registry with:

- at least eight event/reaction roles;
- exclusions;
- cooldown;
- intensity;
- optional user action.

Do not bundle copyrighted assets in LibGibson.

At least one reaction must be more than decoration: e.g. "scope explosion" reaction opens an explicit choice between local repair and architecture expedition.

### Pass

- presentation can be disabled without changing the underlying semantic trace;
- reactions cannot capture arbitrary input;
- malformed reaction specs fail closed;
- overlays cleanly mount/unmount;
- Mono fallback remains intelligible;
- repeated events respect cooldown/budget;
- one meme reaction is replaced with a completely different visual metaphor using the same reaction API.

The last item tests whether the mechanism is semantic rather than meme-specific.

## 5. Experiment C: Generated instrument

### Build

Prototype an **external** bounded InstrumentSpec.

Minimum primitive set should be intentionally small:

```text
Text
RichText
Row
Column
Stack
Panel
Viewport
Table
Graph
Canvas/Raster
Selector
Inspector
```

Do not add another primitive merely because one example is awkward. Use lower-level composition first.

Implement:

1. dependency graph + detail inspector;
2. timeline;
3. hypothesis/provenance board.

The agent/model may output only the declarative spec and structured actions.

### Validator tests

Reject:

- duplicate IDs;
- unknown action targets;
- cycles where forbidden by the schema;
- excessive depth;
- excessive node count;
- raster dimensions above budget;
- too many effects;
- unsupported required capability;
- unrestricted terminal escape text;
- subscriptions outside the declared event set.

### Interaction tests

- select graph node;
- move focus;
- invoke action;
- resize;
- update mounted spec while preserving selection when identity remains valid;
- unmount and restore previous focus.

### Replay

A recorded sequence of:

```text
semantic snapshot
instrument spec
instrument patches
user actions
clock
```

must reproduce the same realized screen sequence under a fixed capability profile.

## 6. Experiment D: unknown-dope-shit holdouts

This is the decisive experiment.

After the public helper/IR surface is frozen, choose at least three previously unplanned interfaces.

Examples are intentionally suggestions, not commitments:

- compiler CFG + live value-flow explorer;
- theorem/proof obligation lattice;
- Git history/repository topology map;
- chemistry route comparison;
- audio structure/rhythm visualization;
- simulation phase-space inspector.

Have a fresh session implement them **without editing LibGibson**.

### Verdict classes

**PASS**
- public API sufficient;
- helper additions are external and compositional.

**ERGONOMIC GAP**
- possible with public API, but repetitive enough to justify a generic helper candidate.

**GENERIC PRIMITIVE GAP**
- two or more unrelated holdouts require the same missing operation.

**BOUNDARY**
- requested behavior conflicts with a deliberate safety/resource/terminal constraint.

**EXPRESSIVE WALL**
- cannot be represented without private renderer changes and no lower public layer can express it.

Only GENERIC PRIMITIVE GAP should normally create LibGibson core work.

## 7. Experiment E: style/semantics separation

Render one identical semantic session through:

- restrained professional grammar;
- manga grammar;
- meme-reactive grammar.

Compare:

- semantic event trace hash;
- application state transitions;
- emitted structured actions;
- terminal interaction events.

Presentation changes may affect explicit user choices only where the same choice is intentionally offered.

Otherwise semantics should remain invariant.

## 8. Experiment F: low-level escape hatch

Deliberately invent one visual that the high-level helper layer cannot express elegantly.

Build it directly with public:

- `RgbRaster` / `Surface`;
- geometry/path primitives;
- Scene embedding.

Pass if this works without:

- renderer modification;
- unsafe terminal writes;
- broken resize behavior;
- bypassing color capability handling.

This is the anti-canned-code control.

## 9. Metrics

Do not reduce the result to a single "API quality score."

Track:

### Ergonomics
- user/harness LOC;
- number of custom helper types;
- amount of boilerplate;
- number of internal APIs desired.

### Expressivity
- holdouts completed with public APIs;
- number and type of core gaps;
- lowest layer needed for each visual.

### Runtime
- frame generation time;
- paint/layout/diff time when available;
- logical damage;
- wire bytes;
- retained memory/high-water mark;
- max node/entity/raster counts.

### Robustness
- resize;
- Mono/ANSI fallback;
- malformed generated specs;
- event/focus restoration;
- terminal restoration;
- deterministic replay.

### Epistemic
- which examples shaped the API;
- which holdouts were genuinely fresh;
- whether a promoted primitive had two independent consumers.

## 10. Phases

### Phase 0 — documentation freeze

This branch.

No core code.

### Phase 1 — external manga prototype

Use current public APIs first.

Produce an API-friction ledger.

### Phase 2 — external meme reaction prototype

Reuse the same semantic fixture.

Do not add meme concepts to core.

### Phase 3 — external generated-instrument prototype

Build a tiny validated IR and SessionPort.

### Phase 4 — cross-harness reconciliation

Normalize repeated friction.

Propose the **smallest** generic LibGibson API changes.

### Phase 5 — core promotion, if earned

For each proposed core change:

- name two consumers or one correctness boundary;
- show old workaround;
- define the new primitive;
- provide mutation/negative test;
- demonstrate that the primitive does not depend on manga/meme/agent concepts.

### Phase 6 — frozen API holdout

Run unknown-dope-shit holdouts.

This is where the expressivity thesis can lose.

## 11. Suggested promotion queue — not authorized work

The audit suggests these as likely candidates, in descending confidence:

1. stable interaction/action identity;
2. minimal scene lifecycle patch operations;
3. generic path sampling;
4. normalized 2D coordinate helpers;
5. bounded presentation budget utilities;
6. clip/mask ergonomics;
7. extensible semantic theme tokens.

Do not implement them just because they are listed.

The first external prototype decides whether they are real.

## 12. Stop condition

This research program is complete when:

1. all three concept families can be implemented externally;
2. at least three fresh holdout interfaces can be built with the frozen public surface;
3. remaining core gaps are classified;
4. any promoted core primitives are generic and independently motivated;
5. no experiment requires a manga/meme/LLM-specific concept in the LibGibson renderer.

At that point, build the actual harnesses in their own homes.
