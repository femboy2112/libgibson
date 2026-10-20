# LibGibson v0.5.0 — Interactive Observable Instruments

**Roadmap reorientation: 2026-10-19. Status: proposed implementation contract, not an implemented feature or release.**
**Authoritative release baseline:** `main@80706c91b88e59495b131bfd8ab23aa3ed013414`, published `v0.4.0` (engineering alpha), Rust MSRV 1.85, C ABI v1, best-tested Linux x86_64.
**Companion:** [bounded Claude execution handoff](V0_5_EXECUTION_HANDOFF.md).
**Forcing consumer:** [SmartChem integration epic #125](https://github.com/femboy2112/libgibson/issues/125) and [child tickets #126–#146](https://github.com/femboy2112/libgibson/issues?q=is%3Aissue+is%3Aopen+%22SmartChem%22).

> **Decision:** Stop feature-expanding HumanMusic during this release. Preserve tested and ear-accepted music work as an explicitly characterized WIP. Turn v0.5.0 into a usable, domain-neutral interactive instrument platform: accurate input, coordinates, edited geometry, clocks, snapshots, fields/plots, controls, capture, and real-terminal testing.
>
> **Stopping rule:** a fresh author can build, edit, inspect, replay, and capture a nontrivial public-API instrument in a headless and real-PTY environment. Do not require all imaginable features or universal cross-language parity to ship.

This document changes **proposed v0.5 scope and sequencing**, not the historical claims in `ROADMAP.md`. It does not authorize merges, tagging, publishing, or overwriting any currently active worktree. Existing release/finale owner directives remain visible; where scope changes are proposed below, reconcile the original draft PR #92 with maintainer approval rather than silently deleting it.

## 1. Why the course correction is necessary

The v0.4.0 foundation already provides one cell-`Surface` rendering pipeline, a diff/atomic terminal writer, scrollback vs live-region semantics, Taffy layout, glyph/color degradation, headless capture, scene/temporal effects, RGB/Braille/HalfBlock realization, semantic UI, and an experimental audio/HumanMusic engine. The absent interface is *authoritative interactive manipulation*: mouse/focus events, rendered-layout hit targets, world-coordinate conversion, reversible tool gestures, bounded producer snapshots, and linked scientific views.

SmartChem 3.0 intends a falling-sand/reactive-transport **application**: walls, heater regions, valve-like controls, parameters, and observed chemical states. LibGibson supplies the **instruments**, never the chemical laws, solver, safety decisions or source truth. SmartChem is Python/NumPy/SciPy based; a Rust-only UI is not yet a direct Python consumer SDK.

### Library/application authority split

```text
Application (SmartChem or another consumer):
  model state, units, evolution, validation, provenance, authoritative edit commands
          │ immutable snapshot(tick, sequence, source identity)
          ▼
LibGibson domain-neutral adapter:
  bounded frame transport + observable fields/plots + geometry/view transform
  semantic UI controls + inspected action/capture receipts
          │ same source identity / explicit quantization
          ▼
ONE Surface → existing diff renderer → terminal
          ▲
real terminal events / synthetic replay → normalized semantic intent
          │
          └────────────── application-owned command queue (ordered/acknowledged)
```

**Invariant A — non-destructive observation:** changing zoom, terminal size, palette, motion preference, frame rate, or the selected presentation grammar cannot rewrite authoritative source state.

**Invariant B — command/frame separation:** visual frames may be coalesced/dropped with disclosed counters; press/release, edits and simulation commands must not silently disappear, multiply, reorder, or be pretended successful. The consumer decides whether an edit is valid.

**Invariant C — clock separation:** logical model ticks, source publication times, renderer generation, terminal write, input receipt, and edit sequence have different identities. No implicit model update is caused by an extra frame or resize. A terminal write is not proof of optical presentation.

**Invariant D — one geometry:** drawing, clipping, hit-testing, selection and probe inversion reuse the *same* declared transforms and coordinate conventions; terminal mouse cells do not grant subcell precision. Report uncertainty/quantization.

**Invariant E — fallback and accessibility:** keyboard-only, monochrome, reduced-motion, no-pointer, narrow-screen modes retain the core editing/inspection tasks and truthful data labels.

**Invariant F — public API and single renderer:** resist demonstration-only private hooks, a second ANSI renderer, domain-specific widgets or an application-owned duplicate input parser.

## 2. Current branch topology — do not misrepresent branches as releases

| Candidate | Seen at review | Disposition |
|---|---|---|
| main/released v0.4.0 | `80706c9` | Reference and regression baseline; do not rewrite |
| [PR #81](https://github.com/femboy2112/libgibson/pull/81), plot | `06147d5`, draft | Reuse plot axes/data/receipts; do not create competing plot engine |
| [PR #83](https://github.com/femboy2112/libgibson/pull/83), spatial UI | `fcd5bbc`, draft | Reuse tested modal fix #82, textured quad, Album Flow; evaluate portability of public primitives |
| [PR #85](https://github.com/femboy2112/libgibson/pull/85), experience grammars | merged **into #83's branch**, not main | Respect ancestry; retain bounded identity-preserving features |
| [PR #86](https://github.com/femboy2112/libgibson/pull/86), cinematic | `5f927ea`, draft | Descends from #81/#83 line; likely visual integration starting point; verify live SHA/ancestry |
| [PR #84](https://github.com/femboy2112/libgibson/pull/84), auditory observer | `f94f8fa`, scaffold | Do not mistake for complete implementation; defer standalone scope |
| [PR #87](https://github.com/femboy2112/libgibson/pull/87), C137 | `fbdbffc`, draft | Preserve, no independent music escalation |
| [PR #116](https://github.com/femboy2112/libgibson/pull/116), acoustic lock | `f3725fd`, stacked draft | Preserve the accepted 51-artifact reference; verify actual lineage before promotion |
| [PR #123](https://github.com/femboy2112/libgibson/pull/123), rehearsal | `e6fbaff`, stacked draft | Freeze opt-in experiment, music status = WIP; ear verdict pending |
| [PR #124](https://github.com/femboy2112/libgibson/pull/124), pocket audit | `ac63e6b`, stacked draft | Preserve read-only timing/causality findings; no audition verdict |

GitHub commit comparisons at review showed `feat/v0.5-cinematic-showcase` **ahead of**, not disjoint from, the plotting/spatial heads. Do not blindly cherry-pick or merge #81, #83 and #86 on top of each other. Audit `merge-base`, changed-file overlaps, branch status and every PR base again at actual execution. CI at an individual head is historical evidence, **not** combined-head acceptance.

## 3. Graceful HumanMusic closeout — freeze, not discard

**Freeze conditions:**
1. Maintain released v0.4.0 behavior and public contracts. No default sound/composer change by accident.
2. Preserve ear-accepted C137 material plus the `C137_ACCEPTED_V1` immutable reference from PR #116 (51 declared artifacts, manifests, render reproducibility and deliberately calibrated mutation check). Carry it as a separately validated opt-in artifact, not a false claim that its branch is released.
3. Capture exact SHA, ancestry, test/CI receipts and sample-command references for the Homosapians rehearsal composer (#123) and read-only pocket naturality research (#124). Their successful historical CI runs do not certify the musical outcome.
4. Mark *explicitly unresolved*: subjective ear acceptance, 17.31 ms Keys-comp lead at 104 BPM and related potential self-flam, an untyped near-barline chord anticipation, discretionary drummer response computed but not directly reflected in Score, the limited harmonic-camp diversity, true polyphonic Keys comp and non-vacuous section form.
5. Record the future HumanMusic/FM/Meatsack, melodic semantics, auditory observer, quality improvements and musical pop production in a hold ledger. Audio tickets #74 (offline PCM memory), #75 (reference axes), #78 (noncontiguous seek) remain discoverable correctness/contract debts; no new music feature train.
6. Permit **only** essential regression/security/correctness fixes after the freeze, with source-level evidence and no unapproved acoustic changes. Do not undertake another subjective 'one more listen/tweak' round.

**Closeout artifact:** one verified music status ledger (released, ear-accepted on a specified branch, machine-tested, observed once, proposed/deferred, unverified). Reference manifest pointers, not copyrighted clips or large generated WAV additions. Stop when the ledger and lock survive the chosen no-change regression path.

## 4. Scope ledger: SmartChem #125 and children

The issue labels P0/P1/P2 express *consumer urgency*, not a commitment to finish every issue's fullest implementation in 0.5. Minimum useful slices are differentiated below.

| Ticket(s) | v0.5 obligation | Minimum falsifiable outcome |
|---|---|---|
| **#126** | P0: opt-in mouse/focus + lifecycle | Correct normalized down/up/drag/wheel/unknown/focus/resize, with PTY and synthetic receipt, capture restores terminal |
| **#127** | P0: layout-derived hit test and capture | Exactly one intended target; occluded/hidden/clipped nodes do not receive clicks; drag cancel/release exact |
| **#128** | P0: 2D world viewport | Checked invertible transforms, pan/zoom/resize invariants and cell quantization receipts |
| **#129** | P0: bounded Rust bulk frame path | Valid stride/length, copy/lifetime and dirty-region contracts; foreign ABI extension explicitly staged |
| **#130** | P0: correct clocks | Red-first test for `FixedStepClock::now()` truncating `u64` to `u32`; no model tick change under variable FPS |
| **#131** | P0: event injection/replay | Same action/model digest on replay; mutation catches dropped press, changed coordinate or reordered edit |
| **#132** | P1: minimal line/rect/brush/select | One preview-to-command per gesture; gaps interpolated within bounded work; keyboard equivalent |
| **#133** | Conditional P1 | First fix #77 entity retirement; demand benchmarks before introducing full tiled compositor |
| **#134** | P1: numeric inspector | Invalid/nonfinite/out-of-range edits refused, preview is not model commit, keyboard/pointer same typed action |
| **#135** | P1: scalar field/heatmap first | Known zero/negative/NaN/masked/spike values accounted; physical values untouched by palette |
| **#136** | P1: probe + linked time series | Plot hit inversion via #81 transform, stable sample ID, no manufactured missing sample |
| **#137** | P1: bounded undo/redo transactions | Stroke/property edits invert exactly; cancel and branching-redo cases specified |
| **#138** | P1: deterministic capture | Capture authoritative frame once; portable PPM/text plus manifest/digest; bounded output |
| **#139** | P1: truthful capabilities | Supported/unsupported/unknown/user-forced distinct; keyboard/mono fallback always reachable |
| **#140** | P1: keyboard/a11y essentials | Full basic draw/select/parameter/probe/undo path in 40×15 mono, textual readouts, reduced motion |
| **#141** | P1: bounded snapshot bridge | Replaceable visuals separate from ordered acknowledged commands; Python producer Rust frontend canary |
| **#142** | Defer broad parity | 0.5 records Rust/Python bridge limitations; true C/C++/Python/Go ABI parity goes to 0.6+ / 1.0 goal |
| **#143** | 0.5 minimum telemetry | Input-to-dispatch, generation/write, frame coalescing, queue age/memory separately; full observatory later |
| **#144** | **Mandatory flagship** | Fresh author builds general instrument using only public interface; PTY/headless + keyboard-only canary |
| **#145** | Defer optional sonification | Keep acoustic/WIP boundary; no simulated 'real chemistry sound' |
| **#146** | Defer optional pixel protocols | ANSI HalfBlock/Braille/mono sufficient unless measured evidence forces alternative |

**Other pre-existing gates:** #15 queued input on resize under PTY backpressure must be explicitly reproduced/resolved/characterized, never silently waived; #79 post-hangup poll busy-loop must not trap long-lived users; #77 retained hidden/retired scene entities needs a bounded life cycle; #82 modal sizing fix is already staged in #83 and must not be rewritten. Evaluate #73 unknown Story start and #76 Shake/Jitter lifetime as small truthful correctness contracts if in the touched paths.

## 5. Dependency-ordered release train

- **R0 — freeze and reconcile.** Preserve music ledger. Re-audit current main/PR heads, branch ancestry, non-default experiments, tests and ABI. Create an integration branch with isolated worktrees; no history rewriting.
- **R1 — make time/input failure observable.** Correct #130 with a boundary test; address #79 and probe #15 without broad parser replacement; land the minimal injectable event stream/trace (#131).
- **R2 — spatial interaction kernel.** Pure #128 geometry and #126 input can progress independently; #127 hit maps/capture depends on them. Integrate basic #139 capability semantics and test clipped/modal/resize cases.
- **R3 — frame/model transport.** #129 bounded Rust frame ingestion + minimum #141 immutable snapshot/command queues. Profile existing renderer first; fix #77, then determine if #133 tile invalidation is justified.
- **R4 — usable authoring.** #132 geometric gestures, #134 validated controls, #137 undo/redo. No domain-aware 'heater' type. Tools produce reversible intents consumed by the app.
- **R5 — scientific observation.** Integrate PR #81 from the actual visual ancestry, then #135 scalar heatmaps, #136 point probe/linked plot and #138 capture. Preserve missing-data identities.
- **R6 — certify a library, not a demo.** #144 synthetic instrument with field, 2-chamber geometric sketch, numeric source, pause/step, view change, probe/history, undo, export/replay; a separate minimal Python producer. Add #140 fallback and #143 calibrated metrics; fresh SDK/document-only consumer canary.
- **R7 — v0.5 artistic and release closure.** Reuse PR #86's one-edit cinematic authoring work; produce upgraded intro + keyed Filthy Frank reaction and frozen, clearly identified soundtrack. Verify under headless and real terminal with local media gate; obtain owner watch/listen approval. Test integrated exact SHA, MSRV/ABI/SDK and documentation. Only then consider a merge, `Cargo.toml` bump, tag and GitHub release with maintainer authorization.

Parallelization is **dependency-aware**: pure geometry, clock and music-document closeout can run concurrently; input normalization and hit routing must integrate sequentially; raster/snapshot branch must not silently mutate UI or model authority. See [execution handoff](V0_5_EXECUTION_HANDOFF.md) for low-memory worker lanes.

## 6. Release flagship contract

`examples/instrument_workbench.rs` (name flexible) must be a **domain-neutral, explicitly SYNTHETIC** source. It is not a chemical simulator or evidence of scientifically correct reaction kinetics.

Minimum interactive walkthrough:
1. Open on 40×15 mono keyboard-only or standard truecolor with mouse reporting permitted; choose a tool.
2. Draw/select/move a rectangle or polyline representing a compartment; enter a bounded generic numerical source parameter (e.g. display label '200 W' is *caller-supplied*, not measured).
3. Render an app-owned scalar field with declared unknown/masked samples; zoom around a position and probe the same sample in a linked plot and inspector.
4. Pause/step a **consumer-owned** fixed-tick synthetic model; keyboard and pointer paths emit equivalent typed commands.
5. Undo/redo, capture authoritative frame and export/replay a bounded trace. Resize, change glyph/color capability and vary presentation frame rate without changing the canonical world/command digest.
6. Through a separate Python producer, deliver identified frame snapshots with bounded lifetime and a non-lossy command channel; do not promise a production Python SDK before such a client is genuinely tested.

A second *cold-start* developer receives only installed/documented public APIs and independently builds a different geometric instrument. Record real friction, no private flag knowledge or false promise of language parity.

## 7. Validation and evidence ledger

Every task must declare: `source SHA/worktree`, `hypothesis`, `positive test`, `negative test`, `mutation control`, `actual command/run result`, `remaining uncertainty`.

- **Correctness:** boundary cases include unknown mouse buttons, missing up, stale captured widget, clipping/overlay, exact world inverses and finite-range failure; duration beyond `u32` frame indices; invalid/stride/overflow buffers; snapshot disconnect and stale IDs.
- **Determinism:** identical authoritative command/model digest at 15/30/60/unlimited renderer cadence and slow terminal. Screen content may vary with capability; source identity cannot.
- **PTY:** real SGR mouse, press/release ordering, focus, simultaneous resize, output backpressure, EOF/SIGHUP; separate parsed, queued, delivered, dispatched and presented levels. PTY failures reap every child and restore lease.
- **Field honesty:** masked, unknown, invalid-log, NaN/Inf, clipping/decimation/exact extrema paths reported. No 'missing means 0' display.
- **Resource bounds:** measure generated frames, frames written, bytes emitted, terminal blocking time, input-to-action latency, dropped/coalesced visual snapshots, queue high-water and RSS/allocations. Distinguish hardware/terminal conditions.
- **Portability/a11y:** 40×15/80×24/160×50, mono/16/256/truecolor, keyboard, reduced-motion, absence of mouse, no-tty and capabilities unknown/forced states. Claim only tested platforms.
- **Compatibility:** maintain `GIBSON_ABI_VERSION = 1` unless explicitly reviewed versioned break; validate published C/C++/Python/Go SDK clients where their existing capabilities are claimed; Rust MSRV 1.85; `cargo fmt`, Clippy, full tests and examples on the **actual integration head**.
- **Musical boundary:** no automatic art/PCM acceptance; retained reference-golden checks run when music code/build path is affected; author ear gate remains separate.
- **Cinema:** one edit clock; real new features in the film, no prerecord-faked controls, local Filthy Frank source needed for full keyed review. CI synthetic/no-reaction success is **not** full finale acceptance.

Observed branch CI is not independent provenance for an integrated program; tests must execute after integration. A compiled demo is not a verified visual interaction. Static test reports are evidence only for their exact head, environment and fixture.

## 8. The previous v0.5 finale directive and the change request

PR #92 currently carries the owner requirement for a **new** terminal-native, keyed Filthy Frank reaction film, showing real v0.5 capabilities, with one authoritative edit clock and maintainer visual/listening signoff. **Keep that requirement**. The roadmap now proposes retargeting the story from 'larger C137 music/auditory observer integration' to **interactive geometry → editable instruments → observed field → worked scientific interface**. Use a frozen verified v0.4.0 or separately pinned accepted C137 soundtrack with truthfully named provenance. Do not make a new auditory observer or music composition iteration necessary for release.

The old plan is a *draft in PR #92*. Reconcile its explicit scope/shot list with this new owner-requested direction before marking either contract final. Missing local Frank footage makes the full media gate UNVERIFIED; do not invent its presence, publish copyrighted clips or reduce the gate to a replay of the old intro.

## 9. Sequenced future horizon (informational)

| Milestone | Identity |
|---|---|
| 0.5.0 | Interactive Observable Instruments; Rust-first usable spatial/scientific workbench |
| 0.5.x | Measured stabilization, regression/terminal portability and consumer ergonomics |
| 0.6.0 | High-throughput native C ABI + input/frame parity for C/C++/Python/Go where verified; incorporate #142 and PR #91 design |
| 0.7.0 | Public-SDK terminal Doom integration from PR #91 research, real gameplay gate |
| 0.8–0.9 | Broader functional parity, compositional authoring and sustained platform evidence |
| 1.0.0 | Reviewed CORE stability and **supported**, independently consumer-tested Rust-equivalent language capabilities |

Cross-language equivalence is a **goal with a measurable verification debt**, not an assertion about today's SDK.

## 10. Governance and update cadence

- One bounded, independently reviewable PR per seam; central input/UI/raster/ABI modifications require one integrator with exclusive file ownership at a time.
- Keep main protected; all implementation starts from a fresh, verified ref on reviewable branches; no force push, tag, release, or PR merge without owner consent.
- At every rung, update a concise progress table: source, shipped-on-branch, CI, user-visible demonstration, still blocked, next discriminating probe, and issue links. Only mark a requirement complete once the direct test passes; do not close parent tickets for partial slices.
- Prefer correctness/usable consumer value over breadth. If time, disk or memory is scarce, reduce concurrency and stop after a complete certified rung; never trade source correctness for an invented success report.
