# Zero-Context Handoff — Agent-Native Dynamic UI Pressure Tests

**Branch:** `research/agent-native-dynamic-ui-2026-09-24`  
**Base:** LibGibson public main `3438c89645c760201be4b2598cf12161efada782`

## Mission

Do **not** build the manga harness, meme harness, or generated-widget agent inside LibGibson.

Use these ideas as external stress tests to answer:

> Is the LibGibson public API expressive and ergonomic enough for professional, highly dynamic, currently-unanticipated interfaces?

The engine should make common impressive work easy while retaining public low-level escape hatches for concepts nobody anticipated.

## Read first

1. `docs/research/AGENT_NATIVE_DYNAMIC_UI_RESEARCH_PROGRAM_2026-09-24.md`
2. `docs/research/LIBGIBSON_EXPRESSIVITY_API_AUDIT_2026-09-24.md`
3. `docs/research/AGENT_NATIVE_UI_EXPERIMENT_PLAN_2026-09-24.md`
4. `docs/STATE_OF_LIBGIBSON.md`
5. `DESIGN.md`

Also inspect the current cinematic work before execution. At dossier creation, open PR #14 contained a generic `CubicPath3` proof-of-shape and an introductory film, but this research branch intentionally starts from public main and does not depend on that PR.

## Architecture to preserve

```text
Semantic session
  + Presentation grammar
  + Reaction policy
  + Generated instruments
          |
          v
   external validated layer
          |
     public LibGibson
Node / Scene / Surface / Raster
          |
       terminal
```

## Hard constraints

- no raw ANSI as the extensibility story;
- no model-generated Rust/native code as the first generated-widget architecture;
- no manga/meme/LLM concepts in LibGibson core;
- no core feature promotion from one consumer;
- preserve deterministic replay and resource bounds;
- classify friction before fixing it;
- fresh holdouts after API freeze;
- semantic behavior and presentation behavior stay separable.

## First execution step

When ready to continue, create an **external scratch/prototype repository or worktree** with one deterministic synthetic agent-session trace.

Implement the manga presentation using **public LibGibson only**.

Do not patch LibGibson on first contact with friction.

Record every friction point in a ledger with:

```text
location
desired behavior
public route attempted
why route was awkward/blocked
classification:
  ergonomic_gap
  generic_primitive_gap
  deliberate_boundary
  harness_specific
  expressive_wall
candidate minimal fix
second independent consumer? yes/no
```

Then repeat with the meme reaction layer and generated-instrument layer.

## The verdict-changing probe

After any API improvements, freeze the public surface and hand three previously unspecified visual interactions to a fresh session.

If they can be built professionally using public APIs and the low-level escape hatch without core changes, the "unknown dope shit" objective has survived its first real holdout.

If not, record the exact wall.

Do not rescue the thesis by adding the missing widget during the holdout.
