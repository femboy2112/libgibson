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
| Post-merge / 536 | [State of LibGibson](STATE_OF_LIBGIBSON.md) | Fresh gates, architecture/API inventory, source review, decisive debt probes and roadmap | Original baseline; current debt ledger is updated explicitly by later sections. |
| Introductory cinema / consolidation | [Introduction](INTRODUCTORY_CINEMA.md) | Dependency PR integration, D1/D2/D5 fixes, cubic routes, finite cinematic intro, art-direction/continuity pass, Surface and PTY witnesses | Merged through PR #14 at e5ede0a; exact-main CI 35960000454 green. Not an API stability or aesthetic certification. |
| External agent-native UI / frozen API | [Campaign results](research/AGENT_NATIVE_UI_CAMPAIGN_RESULTS_2026-09-24.md) | Shared fixture, three planned consumers, no core promotions, three post-freeze holdouts, 61 lab tests, freeze and public CI | Bounded expressivity corroboration; independent lab count, no universal ease or platform claim. |

| Event pressure / upstream isolation | [Event Pressure Lab](EVENT_PRESSURE_LAB.md) | Raw/Crossterm/Context controls, actual epoll witness, scratch causal contrast, bounded visual replay and repeated matrix | Upstream cause isolated; no production fix; #15 acceptance remains red. |

| Crossterm #1126 fix analysis | [Fix analysis](CROSSTERM_1126_FIX_ANALYSIS.md) | Corroborated defect (early-return over a shared mio readiness batch), the drain-fix's blocking-fd hang reproduced by strace, two proposed fix shapes (A: single-read discipline, B: non-blocking fd) | Shape A fix implemented and independently verified in an isolated crossterm 0.29.0 clone (stock reproduces the stall, the fix delivers both events with no hang); staged with a ready upstream PR body but NOT filed, NOT vendored, no `[patch.crates-io]`; #15 remains open. |

| Runtime Observatory / live + `--dump` | [Runtime Observatory](RUNTIME_OBSERVATORY.md) | Live interactive loop over real session machinery: Million-Tick (All-vs-Bounded `StoryDirector`, #10), Ownership-Duel and Restore-Failure (run a real `terminal_ownership_probe` child under a PTY, #11), Endurance (bounded soak); plus deterministic `--dump` frames (Million-Tick/Ghost-Key/Ownership-Duel replaying real fixtures, #10/#15/#11); shared bounded diagnostic log and `?`-for-unobserved epistemic rule; PTY smoke tests | Both live and `--dump` exist; a live Ghost-Key and Slow-Terminal are not built (Event Pressure Lab provides the latter); does not close #10, #11 or #15; VmRSS/VmHWM are Linux `/proc`-specific; #11's `restore()` error path is now covered by a failing-write test. |

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
