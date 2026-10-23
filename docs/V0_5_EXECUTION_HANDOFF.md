# OPERATION INSTRUMENTAL RENAISSANCE — Claude Mega-Round Execution Handoff

**Maintainer intent (2026-10-19):** turn LibGibson from a rapidly expanding HumanMusic experiment into a generally useful interactive instrument framework without dumping valuable musical work. Work aggressively, but do **not** OOM the low-memory local machine by letting agent swarms compile Rust.

**READ FIRST:** [v0.5 Interactive Observable Instruments roadmap](V0_5_INTERACTIVE_INSTRUMENTS_ROADMAP.md), `ROADMAP.md`, `docs/RELEASE_CONTRACT.md`, `docs/STATE_OF_LIBGIBSON.md`, [SmartChem epic #125](https://github.com/femboy2112/libgibson/issues/125), tickets #126–#146, and PR #81/#83/#86/#92/#123/#124 metadata. The roadmap is direction; branch facts must be re-observed at runtime.

**Requested outcome:** execute real source/tests/docs work and push inspectable, reviewable **draft** PRs. Not another plan-only research session. Do the largest *coherent and validated* subset; leave explicit scoped blockers instead of expanding forever. **Do not merge, tag, release or mutate main without authorization.**

## 0. First contact — actual repository and box state

Collect and record (not invent): `pwd`, `git status --short --branch`, `git rev-parse HEAD`, `git branch -vv`, `git remote -v`, linked worktrees, current main/PR heads, any maintainer instructions/CI settings, `rustc --version`, `free -h`, `df -h`, `nproc`, actual running agent/build processes and target-dir size. Do not wipe any target directories without examining disk state and consulting the owner if those artifacts are needed. Respect concurrent local music sessions and worktrees.

Capture the baseline `main@80706c91...` only as the **last observed** main SHA; fetch and verify fresh. Released v0.4.0's ABI v1, MSRV 1.85, published interface and audio goldens are protected. PR #86 is likely the visual integration ancestry (already includes PR #81 and #83) but `merge-base` and exact heads are the authority.

## 1. Hard local resource constitution — NO OOM

There is **ONE and ONLY ONE local Rust compiler/test lane** under the principal integrator. This is a mutex, not an aspiration.

- **Workers/children MUST NOT run:** `cargo check`, `cargo build`, `cargo test`, `cargo clippy`, `cargo doc`, `cargo run`, `cargo bench`, `cargo build --all-features`, `cargo test --all`, or any test helper that secretly launches Cargo/Rust compilation. Do not start rust-analyzer cargo auto-check/build sessions. Never spawn background validation daemons from workers.
- Workers **MAY** inspect source, reason, design APIs, write disjoint source/tests/docs, `git diff --check`, inspect with `rg`/Python standard library small scripts, and run focused *standalone* `rustfmt --check <owned-file>` where available. Static lint is not compiling. Mark uncompiled changes **UNVERIFIED**.
- Default **max two active children** plus principal on an 8-GiB-class machine; reduce to one if RAM or process state warrants. Many specialist agents can participate in **sequential waves**, not by keeping 6–12 live processes on a low-memory computer. Skip/kill no process belonging to another session without owner consent.
- When running local Rust, principal only: **one Cargo invocation at a time**, `CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0` (existing line-tables-only debug profile remains), `RUST_TEST_THREADS=1`; use `cargo test -j 1 ... -- --test-threads=1` for selected tests. Avoid simultaneous `--release`, `--all-features`, examples, docs, and Clippy. Do not repeatedly recompile equivalent graphs per child/worktree.
- Check `MemAvailable`, swap pressure and free disk **before** each heavyweight command. If memory headroom is insufficient for the planned command (or actual RSS climbs dangerously), do not brute-force it: stop local builds and use the existing remote GitHub CI on a pushed draft branch. Record *CI exact commit status*. No arbitrary universal throughput/memory claim.
- Keep a single validated integration worktree for compilation, with a single target cache. Agent working trees are source-only. Prefer bounded unit/integration tests early; one full serial gate at an integrated checkpoint; no uncalibrated concurrent sweeps.
- Do not launch an extra ffmpeg/transcoder, heavy WAV render, DSP benchmark, rustdoc, language-binding rebuild or PTY torture harness while the integration compile lane is occupied. Any heavy media/PTY gate happens later in an exclusive slot or CI.
- Pause expansion on OOM suspicion rather than depleting swap/disk. Report `UNVERIFIED: deferred by resource gate` honestly.

These restrictions override the temptation to run local validation in every agent response. **Source fan-out is broad; Rust toolchain fan-out is zero.**

## 2. Process/ownership architecture — one integrator, many independent specialists

The principal is the **sole kernel integrator**. Each child gets: one scoped issue, named owned paths (or a new disjoint file), protected paths it must not touch, expected public contract, falsifying tests to *write*, and a return envelope: SHA, changed paths, thought-to-be-pass, actual runs (often none), risks and follow-up.

**NEVER have two agents write `src/input.rs`, `src/context.rs`, `src/ui/runtime.rs`, `src/ui/compile.rs`, `src/raster.rs`, `src/lib.rs`, `Cargo.toml`, ABI headers, lockfile or the same test file concurrently.** Prefer separate worktrees/branches and cherry-picks after owner verification; no overwriting user changes. Early abstractions may be prepared as independent new modules (e.g. `src/world_view.rs` candidate and tests) without wiring them until the integrator owns the central imports.

Suggested **rotating specialties** (do not run all simultaneously):
- **A / archaeology and contract scout:** source/PR ancestry, released vs draft APIs, recent issue overlap, event/terminal failure root-cause, safe integration path. READ-ONLY, except an audit note in a disjoint doc.
- **B / pure math and clock:** `FixedStepClock` u32 truncation boundary tests + checked duration strategy #130; `WorldViewport` transform math #128, proofs/property tests. Coordinate any `src/clock.rs` edit with principal.
- **C / event & PTY witness:** #126 event normalization/capture restoration contract and #131 event injection/typed trace tests; #15/#79 independent reproducer harness, with no concurrent input-kernel edit.
- **D / graphics & snapshots:** #129 validated bounded image frame/dirty rect API and #141 immutable frame envelope vs semantic commands. Design small producer canary; no new rendering engine/FFI ABI by default.
- **E / UI interaction:** #127 hit-map and capture/bubble/modal semantics, then #132/#134/#137 after input/geometry land. Do not hardwire SmartChem domain symbols.
- **F / scientific viewer & cold-start author:** map PR #81 interfaces to #135/#136, minimal synthetic scalar field/probe, #138 receipts, #144 public API demo and 40×15 keyboard fallback, not a screenshot-only mock.
- **G / music preservation + cinematic:** read-only preserve PR #116/#123/#124 and PR #92 art requirements; produce truthful WIP ledger and later updated cinematic shot map. Absolutely no music-composer/synth changes.

Run independent scouts first while principal lays out the event/clock integration path. Allocate at most two live children. Reassign roles across waves as earlier outputs land.

## 3. High-value delivery train — implement, don't just design

**Wave 0: R0 freeze and baseline**
- Create a clean feature/integration branch from a verified source ref, not an existing live music worktree.
- Record current ancestry among #81/#83/#86; no duplicate merges. Inventory #92 before proposing changes to finale gate.
- Commit bounded HumanMusic WIP/freeze ledger with exact heads, known-good reference and pending ear gates; do not merge music branches by default.
- Establish red-first tests / test plans for #130, #79/#15, #126/#131, #128.

**Wave 1: R1 correctness and instrumented event seam**
- Principal lands a direct >u32-index fixed-clock counterexample, fixes overflow consistently with API/Duration overflow policy and demonstrates unchanged normal-times behavior.
- Produce a bounded EOF/SIGHUP observation/refusal for #79; isolate upstream Crossterm vs local caller responsibility and avoid indefinite poll. For #15 measure parser arrival vs dispatched input and record if upstream issue remains.
- Add event injection/trace structure #131 before claiming mouse delivery; time/sequence/observed-vs-synthetic fields, bounded retention and coalescing policy.

**Wave 2: R2 spatial interaction**
- Pure `WorldViewport` (#128): checked pan/zoom, world↔view cell/subcell, center-vs-edge, aspect ratio, finite inverses, clipping and quantization uncertainties.
- Opt-in terminal pointer/focus events (#126), SGR/PTY coverage, unknown identity, RAII terminal mode ownership, no stealing mouse selection by default, no bogus clicks.
- Build `HitMap`/capture (#127) only from actual resolved paint geometry; test overlaid modals, hidden/retired nodes, drag outside/resize/focus loss and keyboard equivalents. Avoid double dispatch.
- If end-to-end geometry/input not accepted, **stop before dependent editor features** and report next exact probe.

**Wave 3: R3–R5 conditional expansion after above pass**
- Rust-first bounded bulk RGB frame/dirty rect (#129) and visual snapshot channel with separate command queue (#141), with ownership and memory bound controls. No immediate full C/Python/Go parity.
- Minimal reusable geometry editor (#132), validated numeric inspector (#134), reversible command grouping (#137).
- Integrate authoritative plot line from PR #86/#81; add small accurate scalar/missing-data view (#135), linked probe (#136), deterministic capture (#138).
- Profile #77 retired Scene entities and relevant #133 dirty/render path *before* designing a new tile compositor.
- Do not broaden any seam just to be clever. Produce the smallest general primitive that enables the next practical consumer.

**Wave 4: R6–R7 end-to-end certification**
- Build `examples/instrument_workbench.rs` (#144) through public API: synthetic chambers/field/source with mouse and keyboard draw/select, numerical parameter, pause/step app-owned tick, field/plot/probe, undo, trace/export.
- Separate minimal Python/NumPy producer → bounded snapshot bridge → Rust UI consumer, with precise versioned wire, lifetime, backpressure and command acknowledgement. The library remains chemistry-neutral.
- Fallback and telemetry: #139/#140/#143 (40×15 mono keyboard-only, optional mouse, distinction between receipt/write/presentation, bounded resource profiling).
- Cinema release finale: adapt PR #92 to the new *instrument* story, retain upgraded new intro and keyed Filthy Frank + actual accepted audio with no new music development; edit-clock and local-media proofs. Only after owner approval/acceptance.
- Do not simulate a release: tag/version/publish/main merge remain outside this task.

## 4. Independent witnesses and test gates

Before coding each contract, write its **failure condition**. Minimum:
- 2^32−1/2^32/2^32+1 `FixedStepClock` boundary without looping; Duration overflow vs finite behavior.
- Injected press-drag-release sequence and one missing-release mutation; unknown button/unsupported focus; click behind modal must FAIL.
- Same canonical world point after inverse pan/zoom/resize to declared precision, without lying about terminal cell resolution.
- Input-to-action order invariant under 15/30/60 FPS, blocked output and resize; model digest unaffected by extra renders.
- Scalar grid `0/-1/NaN/masked/spike` receipts; max-preserving vs lossy downsample disclosure.
- Rust-owned and foreign-source malformed frame metadata (stride/length/bounds/lifetime) typed refusal; no segfault/UB.
- Snapshot backpressure drops only visuals, not unacknowledged edit commands; stale snapshot detected.
- Capture/replay same identified frame and command trace; mutate one pixel/edit and verify receipt changes.
- 40×15 mono keyboard-only command parity and explicit missing-data labels.
- Fresh consumer *installed/public SDK* test: don't compile an example via privileged in-repo internals and call it generic.

Treat passing unit tests as scoped, not proof of arbitrary terminal behavior. Re-run exact-head integration CI after merging/stacking; read actual run results. Report confidence in two lanes: code/protocol transport and user-observed functionality.

## 5. Mandatory progress and stopping discipline

Every completed wave yields:
```text
BASE: <sha> / BRANCH: <name>
WORK: issue numbers, exact files/commits
SOURCE CHANGES: concrete callable public APIs and tests
CLAIMS: observed / corroborated / conjectured / UNVERIFIED / refuted
VERIFICATION: actual local commands or exact-head CI run URLs
RESSOURCES: peak relevant headroom, compile lane used yes/no, cache/disk status
KNOWN GAPS: next falsifying probe, not a speculative endless research tree
GATE: PASS / FAIL / BLOCKED with reason
NEXT: highest-leverage safe rung
```

**Progress reporting:** keep user updated as coherent checkpoints arrive, not every worker thought. Push commit history and draft PRs; include exact SHA/source/test statuses. If OOM risk, competing local session, PTY deadlock, ambiguous ABI change or incomplete tests arises, stop that *seam*, isolate it and continue a truly independent scope. Never rewrite established evidence to make a pass.

**Definition of done for this mega-round:** materially move v0.5 toward a usable interactive instrument: at minimum attempt R0 and highest-risk R1/R2 contract work, with actual code/tests where source access permits; ideally integrate a coherent P0 slice and leave R3+ ready. Favor two finished, independently testable primitives over twelve half-written subsystems. Full v0.5 **release** is not authorized and is not promised by this execution prompt.

## 6. Working handoff note

Do not mistake the music freeze for rejection of that research: preserve C137/Homosapians and return to it as a separate, deliberately funded program. Do not use HumanMusic sound quality as a blocking acceptance criterion for input/geometry/scientific instrument code.

Do not substitute a hypothetical chemistry demo for actual SmartChem source/evidence; all generic flagship field physics is clearly **SYNTHETIC**. LibGibson owns transforms/control/observation but never claims scientific validity.

**Be ambitious in mathematical/architectural coherence, conservative in irreversible operations, and ruthless about compiler memory.**
