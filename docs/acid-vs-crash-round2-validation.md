# Acid vs Crash, round II: evidence record

Starting branch: `codex/acid-vs-crash-cinematic`.
Starting local and remote HEAD: `b6a353e1c76fd0b851846087c7630b2e5a6c263f`.
Working tree clean. Remote main remains
`1b4358e91df108cdf0e0dbb92e886f010a8b7eb9`. No merge authorized.

## Baseline (2026-09-23)

All commands exited 0 before modifications:

```
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
cargo test --test surface_fx
cargo test --test scene_cinematic
cargo test --test story_reactions
cargo test --test acid_story
cargo test --test acid_render
cargo test --test pty_demos acid_ -- --test-threads=1
cargo test --test visual_goldens
```

Full baseline: 430 tests, comprising 226 library unit + 204 integration.
Local command receipts: `/tmp/libgibson-acid-round2/baseline-0.log` through
`baseline-10.log` (run-local, not portable artifacts).

## Acceptance probes

The rival explanations are an authored beat sequence versus a topology-driven
opponent. Shared-prefix counterfactual inputs must produce different legal
routes, influence, facts and realized screens. Cutting every route must prevent
new influence at the unreachable target. Full encounter replay must reproduce
continuous graph state, planner memory, StoryDirector, bundles and presentation.
Core mask tests independently check cell/glyph boundaries; whole-renderer VT100
and actual PTY inputs check the externally consumed terminal result.

Remote Actions were reported billing-blocked at intake; no remote green claim.
New cinematic APIs remain experimental and Rust-only. All battle semantics
remain local fiction, with no networking or host command execution.

## Spatial substrate checkpoint

`SurfaceFx::Scoped` and `FxMask` add rectangle, horizontal/vertical wipe,
normalized radial, moving band and seeded-noise selection. They retain ordered
composition and operate after ordinary node painting. Partial scopes isolate
input and clip complete output glyphs to the selection. No new core module or
ABI was needed. No bounds channel was added: layout/reflow remains ordinary
Node geometry upstream of painting.

`cargo test --test surface_fx --test scene_cinematic`: exit 0 (21 + 14 tests).
`cargo clippy --lib --test surface_fx -- -D warnings`: exit 0.
A 120x32 ordinary-panel frontier increment measured 8 exact changed cells,
8 affected cells, 108 wire bytes. The next frozen frame measured 0/0/0.
A mask test exposed a radial reflection boundary rounding asymmetry; normalized
radial distance now subtracts the center before division in f64. No renderer
change was required for this mask correction.

## Remote CI boundary

Branch run [35821399803](https://github.com/femboy2112/libgibson/actions/runs/35821399803)
at `d41a8f6` was checked via `gh run list`, jobs API and check-run annotations.
The Rust job has `steps: []`; dependent jobs were skipped. Its annotation says
account payments failed or the spending limit needs increasing. This is
**BLOCKED / ENVIRONMENTAL**, not an executed code failure. Local evidence below
must not be represented as remote CI success.

## Battlefield architecture and causality

The demo-local reducer is `examples/acid_vs_crash/battle.rs`. Seven nodes include
MODEM, ROUTE, AUTH, SHELL, FILES, DISPLAY and a dormant mirror. Edges own
connectivity, cost and pressure. Shortest-hop routing with stable node order
supplies legal paths; scoring penalizes their cost. There is no network model
beyond this small fictional graph.

Influence uses integers [-1000,1000]; ownership thresholds are ±350. Integrity
uses [0,1000] independently. A fortified intermediate node resists forward
movement until its influence yields. The exact 50ms reducer retains fractional
nanoseconds. A finite 90-second world horizon bounds extreme dt; outcome timeout
is 80 seconds. This deliberately models a short encounter, not arbitrary uptime.

The planner scores reachable candidates using objective value, novelty,
vulnerability, footholds, fixed aggression/curiosity/pride/evasiveness/playfulness,
trace exposure, route cost and isolation memory. The seed provides stable tie
variation. Goals include exploration, foothold, display, evasion, retaliation,
decoy curiosity, showing off and escape. Tactics include probing, pressure,
pivot, route splitting, occupation, display interference, retreat, decoy attack
and a real feint. Repeated recognized decoys can induce an approach and withdrawal
before influence enters the mirror. The debug inspector exposes decisions and
scores; the default UI shows geography and consequences.

TRACE spends reserves and increases awareness; ISOLATE removes edges and local
telemetry; HARD ISOLATE disconnects four systems and blocks decoy capability;
DECOY consumes reserves and becomes recognizable; KILL restores one lease's
influence while others survive. Cooldowns prevent free repeated progress.
TURN TRACE needs earned confidence; SPRING DECOY needs an occupied mirror.
LET HER IN explicitly reopens a MODEM→SHELL→FILES→DISPLAY corridor and sacrifices
it. The player can create that opening; a story timeout cannot.

Broad Story acts retain deterministic stage aliases, with several former tactical
stages now entering counterplay/escalation. Milestones drive progression after
breathing room, with fallback timers that never manufacture influence. Finite
semantic projection events run through Story Reactions to maintain ownership,
isolation, milestones and mounted bundles. Continuous data is not copied into
Facts. Terminal state freezes only after the model's release projection agrees.
Core Story/Scene behavior and the one-arrow law were preserved without changes.

## Replay and counterfactual evidence

`EncounterTrace` records initial stage/seed and exact dt plus ordered semantic
input batches. Replay reconstructs the whole model and recomputes derived events.
Tests compare every model field (including hidden reducer remainder, planner
memory, cooldowns and receipts), StoryTrace, facts, mounts, Presentation and
realized frames at all four sizes. StoryTrace separately reproduces its projected
facts. Editor text and inspector selection remain deliberately outside the world.
There is no disk format compatibility claim.

Shared-prefix encounters receive ISOLATE versus DECOY, then the same updates.
Tests require different targets, legal paths, graph state, facts and screens;
both stay in valid macro stories and replay exactly. The old round-I test that
expected hard isolation to be overridden by later authored DISPLAY pressure was
replaced by the opposite requirement: no reachable corridor means no renewed
possession, even after a dramatic timeout.

With irregular [83,137,31,249]ms cadence, passive play yielded Acid/display prank
(3 systems lost, 0 recovered); auto yielded Crash/traced contain (4 losses,
1 recovery, 1 successful decoy, 40 target adaptations, trace 100%). These are
observations for that seed/cadence, not claims that every input yields those counts.

## Visual changes

The map draws actual graph edges, visible breaks, decoy geometry, directed Acid
traffic and reverse trace pulses. The remote cursor follows active path segments;
remote typing follows sparse action-responsive lines. Influence realizes scoped
FX on existing ordinary panels. Color uses magenta foreground invasion with
localized reverse accents; Mono uses reverse and line grammar. Escalation gives
the existing map the full-width center, collapsing session and remote entities
to witnesses and preserving a clean command island. Resolution restores the
composition, retaining damaged integrity, altered files and isolation costs.

Eight new headless semantic goldens include glyphs and ownership-style grammar:
route pivot, AUTH frontier, isolation break, decoy split, reverse trace, directed
takeover, costly containment and a Mono pivot. Their setup asserts the semantic
cause before accepting a snapshot. All 13 earlier Acid PTY golden cases remain,
with intentionally refreshed layouts. The other 30 demo goldens are unchanged.

## Bugs exposed during this round

- Spatial radial boundaries were asymmetric under floating-point reflection;
  centered f64 geometry restored the tested symmetry.
- First reducer draft advanced through fortified intermediate nodes; traversal
  now waits for influence resistance to yield. Committed regression coverage.
- Acid release initially left edge pressure/activity indefinitely; both clear.
- The director could finish before model recovery, freezing stale ownership
  facts; release completion now follows the actual recovered classification.
- Immediate HARD ISOLATE → CUT LINK froze a finite recoil offscreen when the
  terminal act stopped its clock. Resolution now unmounts that transient bundle.
- A mutual-route-hold ending incorrectly severed every edge. It now retains
  the shared topology with contested ownership, while mutual disconnect severs
  it. Closed remote leases are explicitly labeled disconnected. Regression tested.
- Hero composition hid the map's telemetry-loss notice. The retained session
  chip now also displays the actual local telemetry loss.

No new core renderer bug was found or renderer modification needed. The bugs
above belong to the mask implementation or demo's new reducer/presentation
coupling. Independent scratch finite sweeps found no invalid non-neighbor motion
in 90,000 action steps or fortified-source bypass in 300,000 counterfactuals;
these supplement the committed regression tests, not a universal proof.

## Damage and wire observations (120x32)

| Change | Exact semantic delta | Affected footprint | Wire bytes |
| --- | ---: | ---: | ---: |
| Ordinary-panel scoped frontier increment | 8 | 8 | 108 |
| Actual AUTH entity advancing 100ms (frontier plus its ordinary telemetry) | 82 | 82 | 490 |
| Ghost entity moved one cell with world held fixed | 5 | 5 | 83 |
| Quiet graph packet update, map entity isolated, 100ms | 15 | 15 | 337 |
| Whole DISPLAY counter/cleanup | 2804 | 2804 | 5773 |
| Identical frozen battlefield frame | 0 | 0 | 0 |

The first four are bounded local updates. Cleanup is intentionally broad. Exact
state delta, addressed footprint and emitted bytes are measured separately;
equality of the first two in these cases is not an API identity. No wall-clock
performance threshold is asserted by a unit test.

## Final verification

Code/test tip: `1d6a60b73196998a73f39f2bb158dabb32082313`; the following
completion documentation commit changes no code. All commands below exited 0:

```
cargo build --examples
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
cargo test --test surface_fx --test scene_cinematic --test story_reactions --test acid_battle --test acid_story --test acid_battlefield --test acid_render --test acid_battlefield_goldens --test scene_algebra --test effects_perf --test whole_renderer_vt100 --test resize_torture --test pty_resize_torture --test safety_api
cargo test --test pty_demos acid_ -- --test-threads=1
cargo test --test visual_goldens
cargo test --test acid_render -- --nocapture
scripts/dev/bindings_smoke.sh --asan
```

The full suite passes **464 tests: 226 library unit + 238 integration**, zero
failures or ignored tests. Requested targeted groups pass 141 tests; the Acid
PTY filter passes 8; existing visual harness passes its 2 tests/43 snapshots.
The new battlefield golden test separately verifies 8 glyph/style snapshots.
The other 30 demo snapshots are byte-for-byte unchanged from the starting SHA.
Existing `polished_agent`, `hack_the_gibson` and `fx_lab` paths remain green.

Intentional golden generation commands, followed by ordinary non-update checks:

```
UPDATE_GOLDENS=1 cargo test --test acid_battlefield_goldens
UPDATE_GOLDENS=1 cargo test --test visual_goldens acid_cinematic_goldens
```

C/C++/Python smoke passes, including C/C++ AddressSanitizer and
UndefinedBehaviorSanitizer. The script reports no Go toolchain; Go was skipped,
not verified. No binding ABI changed.

23 direct `cargo run` PTY startup smoke invocations cover interactive, auto,
deterministic, both debug flags and all 18 stage aliases while cycling 56x24,
80x24, 120x32, 160x40 and all four color capabilities. Every invocation exited 0
and restored kernel termios. These bounded startup smokes send Ctrl-C after the
first visible frame; they do not claim full playthrough. Full auto completion
and voluntary exit are separately exercised by the committed real PTY test.
The exact invocation table follows below.

Local receipts: `/tmp/libgibson-acid-round2/final-*.log`,
`bindings.log`, `cargo-smoke.json`, `cargo-smoke-*.ansi`,
`damage-and-replay.log`; run-local helpers `cargo_smoke.py` and `inspect_frame.py`.
The latter captured default-color quiet/breach/takeover frames for visual review
using the actual PTY stream, with software glyph rendering; this supplements,
not replaces, VT100 and human-terminal evidence.

Remote code-tip run
[35931112407](https://github.com/femboy2112/libgibson/actions/runs/35931112407)
again reports zero Rust steps, skipped dependents and the billing rejection.
Remote status remains **BLOCKED / ENVIRONMENTAL**.

## Claim boundaries and review classification

| Claim | Status / boundary |
| --- | --- |
| Legal paths, costed actions, memory and deterministic world replay | IMPLEMENTED + TESTED within committed cases and finite audit probes |
| Macro story/continuous world coupling and semantic bundle removal | IMPLEMENTED + TESTED; no core Story rewrite |
| Wide-glyph-safe scoped effects and differential locality | IMPLEMENTED + TESTED on Linux, existing renderer |
| Cinematic APIs | EXPERIMENTAL, Rust-only; no new C ABI |
| Visual taste, pacing and extended player behavior | PARTIALLY TESTED; frame inspection and scripted input are not broad human playtesting |
| Go, Windows, broader emulators/multiplexers/SSH | UNVERIFIED |
| Stable on-disk encounter serialization | Not implemented or claimed |
| Remote CI execution | BLOCKED / ENVIRONMENTAL, billing |

**READY FOR REVIEW**, not certified READY FOR MAIN. All work stays on
`codex/acid-vs-crash-cinematic`, pushed in coherent increments. Main was not
modified or merged. No PR or merge was automatically performed.

## Direct PTY invocation receipt

| Exact command | PTY size | Exit / restored |
| --- | --- | --- |
| `cargo run --example acid_vs_crash` | 56x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --auto` | 80x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --deterministic` | 120x32 | 0 / True |
| `cargo run --example acid_vs_crash -- --debug-battle` | 160x40 | 0 / True |
| `cargo run --example acid_vs_crash -- --debug-ai` | 56x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=quiet --color=mono` | 80x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=knock --color=ansi16` | 120x32 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=signature --color=ansi256` | 160x40 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=route-contested --color=truecolor` | 56x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=first-breach --color=mono` | 80x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=adapt --color=ansi16` | 120x32 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=sidepath --color=ansi256` | 160x40 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=decoy --color=truecolor` | 56x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=trace --color=mono` | 80x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=pressure --color=ansi16` | 120x32 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=display-intrusion --color=ansi256` | 160x40 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=ghost --color=truecolor` | 56x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=trap --color=mono` | 80x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=climax --color=ansi16` | 120x32 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=takeover --color=ansi256` | 160x40 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=crash-win --color=truecolor` | 56x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=acid-win --color=mono` | 80x24 | 0 / True |
| `cargo run --example acid_vs_crash -- --stage=stalemate --color=ansi16` | 120x32 | 0 / True |
