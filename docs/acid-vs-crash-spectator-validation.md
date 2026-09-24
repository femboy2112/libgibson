# Acid vs Crash: watch Crash work

Starting commit: `b82d8b7967506de0db9f5859fe2cabe3c807e210`.
Branch: `codex/acid-vs-crash-cinematic`. Main is not modified or merged.

The default demo now runs Crash's state-based defender. The terminal stays open
in the aftermath, and typed interventions remain available throughout. `--manual`
selects human-only defense; `--auto` retains the existing automatic exit at the
ending. If both flags are supplied, manual control takes precedence.

The command island shows the latest recorded command without modifying the real
TextInput buffer. Accepted defenses introduce a deterministic 1.2-second pause
before Crash's next autonomous decision, including after a human intervention.
This gives actions room to read without suspending Acid or the story. The pause
is world state reconstructed from the existing ordered EncounterTrace inputs.
View configuration survives reset and the instance replay helper.

## Finer motion and style

Acid's route palette previously used reverse video on every Braille cell. That
turned thin vector strokes into filled terminal rectangles. Routes now use
foreground color and distinct glyph grammar; ownership tint remains a separate
SurfaceFx projection. Solid influence bars become thin Braille frontiers. Square
packets become small heads/tails along real graph edges. Routine whole-panel
jitter is removed; near-complete possession retains a slower motion accent.
Mono retains its ownership labels, broken carriers and scoped reverse infection.

The presentation ceiling is 60 FPS. Packet motion and scan position consume the
recorded world clock plus its unconsumed fractional quantum. The authoritative
reducer still advances in exact 50 ms steps. No wall clock enters deterministic
presentation, and no influence is extrapolated between reducer steps. This is
terminal sub-cell motion, not pixel interpolation or a guaranteed display rate.

No core API, renderer, ABI or binding code changed. The blocky route correction
was a demo style misuse, not a renderer invariant failure.

## Verification

All commands below passed locally on Linux:

```sh
cargo build --examples
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
cargo test --test acid_battle --test acid_story --test acid_battlefield --test acid_render
cargo test --test acid_render --test acid_battlefield -- --nocapture
cargo test --test pty_demos acid_ -- --test-threads=1
UPDATE_GOLDENS=1 cargo test --test acid_battlefield_goldens --test visual_goldens
```

Final suite: **469 tests = 226 library unit + 243 integration**, zero failures.
The full suite verifies refreshed goldens without the update environment variable.
Five added tests cover default autonomous defense, persistent aftermath and
replay, paced decisions plus human intervention, motion between world quanta,
and foreground-only Acid Braille strokes.

Ten Acid PTY tests pass. The default operator test sends no input until it sees
an actual trace, its awareness cost and the recorded command. The default ending
test waits beyond the explicit-auto exit hold, then verifies full-world replay
through the real input line before exiting. Existing manual counterplay tests
now explicitly use `--manual`. Terminal protocol and kernel termios restoration
remain checked.

Thirteen Acid PTY snapshots and eight battlefield glyph/style snapshots were
refreshed. The thirty other demo snapshots are unchanged. Stage fixtures use
manual mode to inspect a fixed world without autonomous intervention. At the new
cadence, `--freeze-at=12` represents twelve 60 Hz updates rather than twelve 30 Hz
updates; both timing and appearance changes are intentional.

Additional run-local PTY checks:

```sh
python3 /tmp/libgibson-acid-round2/spectator_smoke.py
python3 /tmp/libgibson-acid-round2/inspect_frame.py quiet first-breach takeover
```

The smoke script exercised 24 `cargo run --example acid_vs_crash` invocations:
default, auto, manual, deterministic, both debug flags and all 18 inspection
stages. Cases rotate through 56x24, 80x24, 120x32, 160x40 and Mono, ANSI16,
ANSI256, TrueColor. These are startup/Ctrl-C smokes, not 24 complete playthroughs;
all exited successfully and restored termios. Actual quiet, breach and takeover
PTY captures were inspected for the foreground route correction. Full automatic
completion is covered separately by the PTY tests.

Measured differential updates (exact delta / affected footprint / wire bytes):

| Isolated observation | Exact cells | Affected cells | Bytes |
|---|---:|---:|---:|
| Route motion across 33 ms, with no simulation step, 160x40 | 10 | 10 | 212 |
| Graph packet motion across 100 ms, 120x32 | 9 | 9 | 197 |
| Ghost cursor one cell | 5 | 5 | 79 |
| AUTH infection frontier across 100 ms | 36 | 36 | 398 |
| Broad display counter/removal | 2803 | 2803 | 5755 |
| Identical frozen battlefield | 0 | 0 | 0 |

These are frame-pair measurements, not frame-rate benchmarks. Exact replay
compares the graph, planner memory, timing remainder, facts, StoryDirector,
mounted bundles and rendered surfaces; watching configuration is also retained
by instance replay.

## Claim limits

**READY FOR REVIEW.** The cinematic APIs remain **EXPERIMENTAL, Rust-only**.
Subjective smoothness across actual terminal fonts, emulators and hardware is
**PARTIALLY TESTED**. Non-Linux platforms remain **UNVERIFIED**. Binding code is
unchanged; the earlier round's C/C++/Python smoke evidence is not a new binding
run in this refinement. Remote CI is not part of this local result; its previously
reported billing block remains an environmental limitation, not a code failure.
