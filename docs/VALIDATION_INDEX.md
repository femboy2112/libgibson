# Validation evidence index

Use [State of LibGibson](STATE_OF_LIBGIBSON.md) for the current assessed baseline,
known defects, support boundaries and priorities. Use [DESIGN](../DESIGN.md) for
mechanics and [ROADMAP](../ROADMAP.md) for work history and planned directions.

Every record below is a **historical checkpoint**. Preserve its counts, hashes,
commands, failures and limits. Later checkpoints supersede current-state claims,
not the evidence that an earlier run occurred. Statements such as “main unchanged,”
“no merge,” branch names and CI status refer to that record's time. The approved
cinematic branch was subsequently merged through PR #1 at main `0b673cc`.

| Order / checkpoint | Record | Evidence retained | Current-state interpretation |
| --- | --- | --- | --- |
| Initial cinematic / 430 tests | [Acid vs Crash I](acid-vs-crash-validation.md) | Baseline 362; entity SurfaceFx, additive motion, reactions, story/replay, damage, goldens and PTY | Superseded inventory; original core regressions remain relevant. |
| Living battlefield / 464 | [Round II](acid-vs-crash-round2-validation.md) | BattleGraph, planner, spatial masks, action trade-offs, counterfactual and full-world replay | Superseded count/presentation; model provenance retained. |
| Autonomous Crash / 469 | [Spectator refinement](acid-vs-crash-spectator-validation.md) | Default defender, operator pacing, smoother presentation and aftercare | Historical evidence for interaction defaults. |
| RGB substrate / 515 | [Software graphics](acid-vs-crash-rgb-validation.md) | Filled depth-tested triangles, RGB fields/FX/feedback, graphics checks, transport costs and PTY | Superseded presentation; engine/performance observations retain their measured scope. |
| Cinematic research | [Hackers production research](acid-vs-crash-cinematic-research.md) | Source links, production influences and visual design constraints | Research provenance, not runtime verification or current API inventory. |
| Shot-directed machine / 535 | [Cinematic validation](acid-vs-crash-cinematic-validation.md) | Shots, distinct architecture, replay/camera sequence, raster probes, screenshots and resize | Historical visual acceptance scope; does not establish universal emulator performance. |
| Final merge gate / 536 | [Hardening validation](acid-vs-crash-hardening-validation.md) | Feedback resize counterexample/fix, docs contracts, exact gates, bindings and release PTY | Approved tip 3b67ca2; superseded by post-merge audit for current blockers and CI. |
| Post-merge / 536 | [State of LibGibson](STATE_OF_LIBGIBSON.md) | Fresh gates, architecture/API inventory, source review, decisive debt probes and roadmap | Current assessment at the explicitly recorded main SHA; refresh when implementation changes. |

Older visual FX, Scene Algebra and Story checkpoints live in [ROADMAP](../ROADMAP.md)
and [DESIGN](../DESIGN.md), with executable regressions under `tests/`; there is
no separate validation file to invent for them.

Text goldens are small committed terminal reconstruction fixtures. RGB probes,
semantic equality and real PTYs cover different claims. Historical PNG/PPM captures,
raw logs and helper scripts under `/tmp` are run-local observations, not files a
clean checkout is expected to contain. No image corpus is required to build or test.

A future validation record should identify its source SHA, exact commands/results,
platform/toolchain, failure evidence and claim limits, then link here. Do not update
old totals to make every historical record appear current.
