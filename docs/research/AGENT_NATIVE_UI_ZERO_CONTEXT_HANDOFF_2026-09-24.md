# Zero-Context Handoff — Agent-Native Dynamic UI Pressure Tests

**Status:** NON-BINDING RESEARCH / PRESSURE TEST ONLY / NO NEW CORE COMMITMENT

**Initial substrate:** LibGibson public main `e5ede0a4ab8ff52caa567ee10d45824e42c1ebc6`

**Provenance:** refreshed from original research tip `bc4d1ce748ffa67121104ac39d960ba4a82736d3`; the original branch is preserved.

PR #14 is merged. All five exact-main public CI jobs passed
[run 35960000454](https://github.com/femboy2112/libgibson/actions/runs/35960000454).
The local baseline is **574 tests (226 unit + 348 integration)** on Rust 1.98.1;
see the audit for the complete baseline commands. Reverify refs when resuming.

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

The introductory film and generic `CubicPath3` are now on the pinned main.
Its ordinary Node mini-UIs mounted inside projected physical facade regions,
alongside world-space vectors, are **CORROBORATING EVIDENCE** for public rendering
composition without building-specific core types. It remains authored and
presentation-driven. It does not solve **G1: stable semantic interaction identity
and routing for arbitrary runtime-created representations**.

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

Check for an existing **external** lab repository, preferably
`femboy2112/libgibson-agent-native-ui-lab`. Use or create it, pin LibGibson to the
exact merged baseline above, and create the friction ledger before experiment
code. If remote creation is unavailable, use a standalone sibling Git repository
and state that limitation. Do not put implementations in LibGibson examples.

Build one finite deterministic synthetic agent-session trace with parallel
workers, failure/recovery, permission interaction, artifacts, and synthesis.
Every presentation consumes that same trace; explicit user actions are the only
permitted semantic branch source.

Implement the manga presentation using **public LibGibson only**.

Do not patch LibGibson on first contact with friction.

Record every friction point in a ledger with:

```text
ID / experiment
desired behavior
public route attempted
result / workaround / complexity evidence
classification:
  ERGONOMIC INCONVENIENCE
  GENERIC PRIMITIVE GAP
  DELIBERATE SAFETY BOUNDARY
  HARNESS-SPECIFIC
  EXPRESSIVE WALL
candidate minimal fix
second independent consumer? yes/no
```

Then repeat with the meme reaction layer and generated-instrument layer.

## The verdict-changing probe

After the three planned experiments and cross-experiment reconciliation, promote
only earned generic primitives, if any. Require green core CI and a merged exact
revision before repinning the lab. Then record both `FROZEN_LIBGIBSON_SHA` and
`FROZEN_LAB_HELPER_IR_SHA`.

Only after freezing, give a fresh reasoning context the frozen API docs to invent
five candidates; do not supply the dossier's suggested holdout examples. Select
three and screen for contamination against the entire dossier. The examples
already printed in it do **not** count as fresh. Record the selection and actual
context/isolation limits, and use fresh implementers where practical.

If they can be built professionally using public APIs and the low-level escape hatch without core changes, the "unknown dope shit" objective has survived its first real holdout.

If not, record the exact wall.

Do not rescue the thesis by adding the missing widget during the holdout.

## Documentation hygiene

The original two proposal documents contained literal backspace/carriage-return
bytes and TAB-corrupted pseudo-LaTeX. This refresh uses Markdown/Unicode notation.
No new checker framework is required; run this small probe from the repository
root whenever editing the research dossier:

```sh
python3 - <<'PY'
from pathlib import Path
bad = []
for path in sorted(Path("docs/research").rglob("*.md")):
    for offset, byte in enumerate(path.read_bytes()):
        if (byte < 32 and byte not in (9, 10)) or byte == 127:
            bad.append((str(path), offset, hex(byte)))
assert not bad, bad
print("research control-byte check: PASS")
PY
```
