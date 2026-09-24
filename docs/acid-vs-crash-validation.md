# Acid vs Crash: validation record

Starting authority: local and `git ls-remote origin refs/heads/main` both returned
`1b4358e91df108cdf0e0dbb92e886f010a8b7eb9`. Working tree was clean.
Development branch: `codex/acid-vs-crash-cinematic`; no merge authorized.

## Baseline, 2026-09-23

| Command | Result |
| --- | --- |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| `cargo test` | exit 0; 226 unit + 136 integration = 362 tests |
| `cargo build --release` | exit 0 |

Local raw logs for this run: `/tmp/libgibson-acid-audit/baseline-*.log`.
These paths are run-local receipts, not portable repository artifacts.
Remote Actions were reported billing-blocked at task intake; no remote success
is inferred from these local checks.

## Evidence boundaries

The encounter is entirely fictional and offline. Scene and story APIs remain
experimental and Rust-only. Automated cell/PTY evidence does not certify
subjective cinematic impact or every terminal emulator.

Remote main CI checked live with `gh run list --branch main --limit 3 --json
...`, jobs API and check-run annotations. Run
[35817809009](https://github.com/femboy2112/libgibson/actions/runs/35817809009)
has zero Rust job steps and skipped dependent jobs. Annotation: "The job was
not started because recent account payments have failed or your spending limit
needs to be increased." Status remains **BLOCKED / ENVIRONMENTAL**; this is not
an executed code failure.

## Implementation and necessity

Code/test tip before documentation: `581bc9f` (final branch tip is reported in the
completion message). The original `main` was never modified.

| Primitive | Why it was needed | Boundary |
| --- | --- | --- |
| Node/Scene entity post-process chain | Acid must affect an existing panel after ordinary painting | No widget story knowledge; no layout wrapper; allocation only for active chains |
| SurfaceFx | Deterministic ordered cell transformations | Identity is empty; concatenation is associative, not commutative; no alpha |
| StyleMask and Dissolve | Progressive terminal-native invasion and reveal | Stable seed/local coordinates; whole wide glyphs selected together |
| Additive Displace/Jitter | Placement, hostile perturbation and counter recoil coexist | Legacy Translate/Shake unchanged; sum first, clamp final position |
| Loop | Persistent mounted scan/jitter/cursor motion | Exact modulo; zero-duration no-op; remove bundle to stop |
| Story Reaction | User counters change the world without thousands of micro-beats | Original-beat event/declaration/action order; at most one transition |

SurfaceFx includes StyleOverlay, StyleMask, Dim, Reverse (reverse attribute),
RowShift, Tear, Scramble, Dissolve and Scanline. Vacated shifted cells become
transparent. Style-only cells never become opaque glyphs. Scramble preserves
styles and skips wide glyphs. Sequence retains earlier contributions; effects
are not undo commands. Unmounting provides restoration.

The Scene → Node → layout → paint → Surface → diff → ANSI pipeline remains intact.
No ECS, renderer replacement, networking, shell execution, VM, persistence, or
cinematic C ABI was added. `surface_fx.rs` is the only new core module.

## Bugs exposed and fixed

1. **Positioned trailing-edge clipping.** Identity post-processing disagreed with
   ordinary panel rendering at `(15, 0)` in a 30×10 destination: the ordinary
   border moved inward when clipped. Positioned entities now realize their
   natural geometry before clipping on every edge. Flow widgets retain their
   established visible-width/input-scrolling contract. All old goldens pass
   unchanged.
2. **Fullscreen resize ghosts.** Live takeover resize 120×32 → 56×24 left old map
   fragments in blank rows and after short rows. Geometry invalidation discarded
   the previous surface, and fresh diffs omitted default blanks. The renderer
   now clears the fullscreen canvas inside its normal atomic re-anchor
   transaction; its affected-footprint capture includes that clear. The real
   encounter resize regression and an independent minimal stale-row regression
   both pass. No demo emits terminal protocol bytes.
3. **Scene time arithmetic.** Duration sums/delays could overflow; Repeat narrowed
   `usize` counts and large nanosecond remainders. Exact remainder evaluation and
   saturated public durations now handle extreme values, including MAX boundary
   cases. Legacy finite behavior remains tested.

## Encounter architecture and consequences

The example declares stable Scene entities/tags and an immutable Story graph.
Facts contain all consequential world state: six subsystem owners/integrities,
route/auth isolation, trace confidence, pressure, decoy occupancy, remote identity,
file scars and outcome. Nodes rebuild normal data; mounted bundles own cinematic
presentation. The command island has a separate tag and is excluded from Acid's
presentation targets. The remote cursor never edits Crash's TextInput.

The acts proceed from quiet → handshake → signature → route contest → first
breach → adaptation → display intrusion → ghost → trap → clash → takeover.
Local adaptation branches are decoy, modem/display bypass, false trace paths,
and file pressure; they rejoin. Acid uses fixed-seed explicit rules and responds
to prior defenses. Auto Crash uses the same commands and graph, preserving an
infection interval before reacting. Human input does not stop story time.

Counters include TRACE, route/auth ISOLATE, KILL SESSION, DECOY, HARD ISOLATE and
TRACE TOKEN. They alter facts and mount/unmount bundles through Reactions.
A prepared decoy or trace is required for the corresponding final strategy.
CUT LINK contains Acid, LET HER IN grants temporary display ownership followed
by voluntary release, and an earned TURN TRACE produces mutual respect.
Scars/integrity and trace history survive resolution. The after-credits inspector
supports replay verification, facts, trace, damage, scene, Acid, Crash, reset and
exit. No real offensive commands exist.

## Replay proof

`tests/acid_story.rs`: 14 tests cover every inspection stage, irregular timesteps,
ordered multi-command inputs, four choices and rejoin, all endings/prerequisites,
auto/idle completion, and ownership/bundle consistency under interruption.
The replay comparison includes exact steps, beat sequence/time, Facts, outcome,
all subsystem owners/integrities, mounts, and Presentation (including mount age).
Inspection setup lives in the Story's declared start, not `jump_to`/`facts_mut`.
UI editing/inspector selection is outside world replay; recorded submitted
commands carry its consequential input. Replay is in-memory against the same
Story definition, not a versioned on-disk trace format.

## Visual and PTY evidence

13 new PTY snapshots: quiet, signature, contested route, infected ordinary panel,
display intrusion, decoy, trace, climax, Crash win, Acid win, stalemate, narrow
56×24, and Mono takeover 80×24. Standard snapshots are 120×32. Existing 30
polished-agent, Hackers and FX Lab snapshots remain unchanged.
Plain-screen snapshots establish glyph/layout stability; they do not certify
color taste. Separate PTY capability checks cover TrueColor/ANSI256/ANSI16/Mono.
Whole-renderer tests compare every declared stage to actual VT100 reconstruction
at 56×24, 80×24, 120×32 and 160×40. Live resize also preserves unfinished Unicode
input while the ghost and takeover continue.

Seven new PTY tests send real keystrokes and verify TRACE, isolate→bypass,
decoy→target change, final CUT LINK + replay inspector, complete auto without
input, Ctrl-C, and four responsive/capability combinations. Exit checks require
successful voluntary exit, restored kernel termios, alternate-screen exit,
cursor/style/paste/sync/autowrap restoration. Tests use stage/freeze/speed hooks;
the seven-case targeted run completed in 7.44 seconds before final refinements,
and the final full suite reruns them.

## Damage observations

Deterministic 120×32 measurements from `final-render-evidence.log`:

| Change | Exact semantic delta | Affected footprint | Wire bytes |
| --- | ---: | ---: | ---: |
| Ordinary panel style infection | 80 | 80 | 299 |
| Actual ghost moves one cell, world held fixed | 5 | 5 | 80 |
| Actual quiet route advances 100ms, map isolated | 16 | 16 | 310 |
| Full display corruption removed by hard isolation | 2530 | 2530 | 7559 |
| Identical frozen frame | 0 | 0 | 0 |

These are bounded correctness observations, not throughput benchmarks. Map and
cursor measurements isolate the specified entity from the independent clock
label and other animations. Broad takeover/counter damage is intentional.
Fullscreen resize addresses the entire canvas; that physical footprint is not
misreported as a minimal semantic delta.

## Final commands and results

All commands ran locally on Linux x86_64. Final `cargo test` counted **430 tests:
226 library unit + 204 integration**, zero failures, zero ignored. Documentation
contains no doctests. New checks are ordinary integration tests run by cargo test.

| Exact command | Result |
| --- | --- |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| `cargo build --examples` | exit 0; refreshes binaries used by PTY harnesses |
| `cargo test` | exit 0; 430 passed |
| `cargo build --release` | exit 0 |
| `scripts/dev/bindings_smoke.sh --asan` | exit 0; C/C++/Python ran; C/C++ ASan+UBSan ran; Go skipped (toolchain absent) |
| `cargo test --test scene_cinematic --test story_reactions` | exit 0 (early integrated check; expanded cases included in final suite) |
| `cargo test --test acid_story` | exit 0; 14 semantic/replay cases |
| `cargo test --test pty_demos acid_ -- --test-threads=1 --nocapture` | exit 0; seven real PTY cases |
| `cargo test --test whole_renderer_vt100 --test acid_render --test surface_fx -- --nocapture` | exit 0; 21 + 4 + 12 tests; damage evidence above |
| `UPDATE_GOLDENS=1 cargo test --test visual_goldens acid_cinematic_goldens` | exit 0; regenerated only 13 new snapshots, reviewed |
| `cargo test --test visual_goldens visual_goldens -- --exact` | exit 0; existing 30 snapshots unchanged |
| `git diff --check` | exit 0 |

The final full suite includes scene unit tests, story unit tests, scene_algebra,
effects_perf, visual_goldens, pty_demos, whole_renderer_vt100, resize_torture,
pty_resize_torture, safety_api, FFI lifecycle and the new cinematic tests.
Earlier red probes were not waived: clipped identity mismatch, fullscreen resize
residue, and semantic ownership/prerequisite contradictions were fixed before
final validation. The first attempted broad clipping fix also broke input resize
and old bottom borders; it was narrowed to the positioned-entity contract, and
both the old tests and new identity tests now pass.

Direct cargo-run PTY startup/exit smoke commands (all exit 0 and exact termios
restoration; each deliberately ended with Ctrl-C after its first visible frame):

```text
cargo run --example acid_vs_crash
cargo run --example acid_vs_crash -- --auto
cargo run --example acid_vs_crash -- --deterministic
cargo run --example acid_vs_crash -- --stage=signature --color=ansi16
cargo run --example acid_vs_crash -- --stage=first-breach --color=ansi256
cargo run --example acid_vs_crash -- --stage=display-intrusion --color=truecolor
cargo run --example acid_vs_crash -- --stage=trap --color=mono
cargo run --example acid_vs_crash -- --stage=climax
cargo run --example acid_vs_crash -- --stage=acid-win
cargo run --example acid_vs_crash -- --stage=crash-win
```

Full no-input completion is separately tested through the same compiled example
with `--auto --speed=20`, without a forced termination. Existing demos run through
their unchanged goldens and PTY cases; FX Lab scene 20 additionally received a
real `r/r/q` interaction smoke with successful exit and restored termios.

Run-local receipts reside in `/tmp/libgibson-acid-audit/`: baseline/final command
logs, targeted engine/story/PTY/render logs, `cargo-smoke.json` and raw `.ansi`
startup transcripts. These are local audit aids; reproducible source/tests and
reviewed snapshots are committed artifacts.

## Claim status and review classification

- **TESTED:** local engine semantics, replay, deterministic stage surfaces, four
  capability modes/sizes, terminal restoration, resize cleanup, binding smoke,
  bounded affected footprints and zero-byte frozen frames.
- **EXPERIMENTAL:** Scene, Story/Reaction and SurfaceFx APIs, all Rust-only.
- **PARTIALLY TESTED:** human pacing, playability and subjective visual impact;
  automated snapshots and byte-stream equivalence cannot certify these.
- **UNVERIFIED:** Go, Windows and a broader real terminal/multiplexer/SSH matrix.
  No versioned on-disk story trace format is claimed.
- **BLOCKED / ENVIRONMENTAL:** remote GitHub Actions billing; zero executed steps.

**READY FOR REVIEW.** Not classified READY FOR MAIN: retain experimental API and
human cinematic review boundaries. No automatic merge or push was performed.
