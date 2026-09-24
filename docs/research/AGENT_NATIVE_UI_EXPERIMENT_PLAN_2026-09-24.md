# Agent-Native UI Experiments and Acceptance Plan

**Status:** NON-BINDING PREREGISTRATION / PRESSURE TEST ONLY / NO NEW CORE COMMITMENT

**Initial substrate:** `e5ede0a4ab8ff52caa567ee10d45824e42c1ebc6` (merged PR #14; pin exactly)

**Purpose:** turn the manga, meme, and generated-instrument ideas into discriminating tests of LibGibson's public API.

The merged baseline has **574 tests (226 unit + 348 integration)** and all five
public CI jobs passed [run 35960000454](https://github.com/femboy2112/libgibson/actions/runs/35960000454).
The introductory film's mounted Node micro-UIs are **CORROBORATING EVIDENCE** for
rendering composition, not for dynamic interaction identity. G1 remains an
unresolved prediction to test, not an instruction to add a core API.

## 1. Rules of the campaign

1. **LibGibson main is the substrate, not the app repo.**
   Implement the harnesses in a dedicated external repository, preferably
   `femboy2112/libgibson-agent-native-ui-lab` after checking for an existing lab.
   No experiment implementation belongs in LibGibson examples.

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

7. **Create the friction ledger before implementation.**
   Each entry records ID, experiment, desired behavior, public route attempted,
   result, workaround, useful complexity/LOC evidence, and one classification:
   **ERGONOMIC INCONVENIENCE**, **HARNESS-SPECIFIC**, **DELIBERATE SAFETY BOUNDARY**,
   **GENERIC PRIMITIVE GAP**, or **EXPRESSIVE WALL**. Record first-contact friction
   before any promotion; do not retroactively call a patched experiment a pass
   against its original substrate.

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

Include one user goal, parallel workers, a tool failure and recovery, a genuine
permission interaction, artifact production, and synthesis. Freeze its finite
ordered events before the first presentation. Only explicit user semantic actions
may change outcomes; presentation mode alone may not.

It belongs to the external harness and makes it possible to test:

**same semantics → different presentations**

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
- a permission choice that emits a semantic action;
- small marginal telemetry panels and animated transitions;
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

Use original terminal-native jokes/cards/procedural graphics, not a copyrighted
meme image collection in either repository. Run the same fixture in **NONE**,
**RESTRAINED**, and **REACTION-HEAVY** modes.

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

Candidate composition vocabulary (not a required kitchen-sink schema):

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

1. dependency explorer;
2. timeline;
3. source/provenance map;
4. decision inspector.

Begin with a deterministic representation planner over session state. The
lab-owned SessionPort validates mount/update/unmount, bounded subscriptions,
focus requests, structured actions, and finalized output. It never hands a
generated instrument a raw Context, renderer, filesystem/network authority,
or native code execution. A model adapter is optional and not a test dependency.

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
- excessive animation rate or retained history;
- unsupported required capability;
- unrestricted terminal escape text;
- subscriptions outside the declared event set;
- non-deterministic initial state when replay is requested.

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

After the public substrate and lab helper/IR surface are frozen, record
`FROZEN_LIBGIBSON_SHA` and `FROZEN_LAB_HELPER_IR_SHA` before candidate generation.
No core change, new widget, or expressive IR/schema extension may rescue a holdout.

The original dossier contained these suggestions, now **CONTAMINATED** design
examples, retained for provenance and excluded from fresh selection:

- compiler CFG + live value-flow explorer;
- theorem/proof obligation lattice;
- Git history/repository topology map;
- chemistry route comparison;
- audio structure/rhythm visualization;
- simulation phase-space inspector.

Use a fresh reasoning context supplied with frozen API docs, not this suggested
list, to invent five candidates. Select three using diversity, architectural
stress, usefulness, and visual ambition. Screen candidates against all research
documents and record when/how selection occurred. Do not claim strict process
isolation merely because a subagent has a new task/context.

Where practical, fresh implementers build each selection **without editing
LibGibson or the frozen helper/IR contract**. Use only public APIs, deterministic
seeded replay, bounded resources, TrueColor and Mono, usable resize, and relevant
interaction. Record failed cases without repair until the verdict is complete.

### Verdict classes

**PUBLIC API SUFFICIENT**
- public API sufficient;
- cleanly buildable with the frozen helpers or public lower layers.

**ERGONOMIC GAP**
- possible with public API, but repetitive enough to justify a generic helper candidate.

**GENERIC PRIMITIVE GAP**
- two or more unrelated holdouts require the same missing operation.

**DELIBERATE SAFETY BOUNDARY**
- requested behavior conflicts with a deliberate safety/resource/terminal constraint.

**EXPRESSIVE WALL**
- cannot be represented without private renderer changes and no lower public layer can express it.

No verdict creates core work during the holdout. Future proposals require the
promotion rule and must preserve the recorded failed result.

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
- exact semantic cell delta and affected footprint separately;
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

This refreshed dossier records merged-main evidence. It is non-binding and does
not begin a public API freeze: that happens only after first-contact experiments
and any earned promotion.

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

Stop adding experiment features and write `docs/CROSS_EXPERIMENT_FRICTION.md`
in the lab before considering a core change.

Propose the **smallest** generic LibGibson API changes.

### Phase 5 — core promotion, if earned

No promotion is a valid result. If earned, use a fresh branch from green main,
require full public CI, merge, then repin the lab to the exact new main revision.
Remove only workarounds superseded by the primitive and compare semantic behavior
and complexity against the original implementation.

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

The external experiments provide evidence for or against these predictions.

## 12. Stop condition

This research program is complete when:

1. all three concept families have recorded external implementation outcomes;
2. at least three fresh holdout interfaces have recorded verdicts against the frozen public surface;
3. remaining core gaps are classified;
4. any promoted core primitives are generic and independently motivated;
5. the final report states whether the thesis survived, failed, or remains unverified,
   without hiding a failed case or weakening the frozen contract.

Successful construction is not a prerequisite for an honest research conclusion.
Record the final evidence in the external lab and update this dossier afterward;
do not confuse these design probes with deployed production harnesses.
