# HumanMusic Round XII — temporal pitch evidence

## Scope and authority

Started at `9ccb84b4462607eb04e10f0f98ecc6f9e5585c03` on
`feat/v0.4-humanmusic-audio`, main/base `7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`.
PR #70 remains draft. No package/ABI/MSRV/release change is authorized.

The maintainer heard bad wrong notes in Round XI despite zero notes lacking a
`PitchFunction`. That falsifies label coverage as sufficient musical justification.
It does not identify every offending note. Round XI's phenomenal regimes and
SongMap architecture are preserved; enjoyment and affect are outside this round.

## Three different questions

1. **Local syntax:** what can this note be called over its harmony and neighbors?
2. **Ensemble syntax:** what happens when all simultaneous pitches sound together?
3. **Temporal syntax:** what source, continuation and destination explain this note?

`PitchFunction` remains local syntax. Existing sonority diagnostics remain the
ensemble layer. `TemporalPitchDiagnostics` reconstructs evidence from notes and
actual harmony instead of trusting their function stamps. No quality scalar is used.

An available extension can be a legal local pitch and still have no observed
ownership in the bounded phrase. Orphan means a listening suspect, not a banned
pitch. The audit cannot infer an unrecorded composer's intention.

## Theory and limits

A suspension requires preparation, the displaced harmony, and resolution.
This experiment uses the user's stricter **actual held event** contract; the broader
textbook vocabulary can also include a restruck preparation. See Puget Sound's
[Music Theory for the 21st-Century Classroom: Suspension](https://musictheory.pugetsound.edu/mt21c/Suspension.html)
and [Non-Chord Tones](https://musictheory.pugetsound.edu/mt21c/NonChordTonesIntroduction.html).

The bounded harmonic window includes the next structural change and a destination
beyond it, sufficient to inspect I–IV–I and ii–V–I. The latter is a relational
harmonic object, not three unrelated chord scales; see
[Open Music Theory: ii–V–I](https://viva.pressbooks.pub/openmusictheory/chapter/ii-v-i/).
These pedagogical sources support the mechanisms, not a claim about what this
particular audio sounds like.

## Evidence discipline

The initial three Astra/High read-only scouts covered classifier semantics,
lead/material generation, and diagnostic design. Their shared repository provenance
was factored; their agreement is not independent listening evidence. Parent reconciled
one contract before implementation. The diagnostic is a separate predicate implementation
from generation. It still shares score/theory data and requires calibration.

Pre-intervention failing witnesses and source evidence are in
[fixtures/humanmusic-r12](fixtures/humanmusic-r12). A diagnostic-only checkpoint
reconstructs unchanged Round XI scores before any pitch-selection changes.

## Claim ledger

| Claim | Status | Evidence | Boundary |
|---|---|---|---|
| Zero absent labels implies no bad wrong notes | Refuted | Maintainer audition against R11 zero receipts | No note-level human timestamps supplied |
| Historical chord membership proves suspension | Refuted | New onset after change still stamped Suspension | Physical carry is the declared contract |
| Final material position proves arrival | Refuted | Closed final licensed color stamped Arrival | Open selected colors remain possible |
| Next targets guide current generation | Refuted at R11 | Only declaration/construction/test; intervention unchanged | New behavior tested separately |
| A licensed extension is necessarily an owned destination | Refuted | Orphan/common-tone controls have identical old local labels | Orphans remain soft suspects |
| Changes sound better | Human-unverified | Matched listening A/B required | No machine proxy settles audition |

## False-negative diagnosis

The old receipt counted `function.is_none()`. A name supplied by generation was
accepted as evidence by measurement. Several independent defects hid beneath that:

- `next_targets` was computed but never read by pitch selection.
- Lead structural candidates and material projection treated every available
  extension as a resting target. The lead DP priced motif contour and local
  color, without a future guide/common-tone term; material projection was greedy.
- Historical chord membership could label a newly attacked note `Suspension`.
- Anticipation checked future chord membership and proximity, without an actual
  connection to its arrival.
- Final material position conferred `Arrival` before any pitch check.
- A literal chord-tone landing used the color-inclusive `nearest_stable` helper.
- Bass approaches selected the next root near the **current** root's octave;
  the arriving bass root used its home register. Some alleged semitone approaches
  therefore resolved by an octave-displaced leap.
- A short keys stab could carry into foreign harmony within the existing
  half-beat overhang allowance while retaining its onset's chord-tone stamp.

The original generator/classifier remains an explicit R11 control. Public current
classification requires physical carry; the new realization calls that corrected
classifier. The baseline's false labels are not silently relabelled to improve its
receipts.

## Temporal model and invariants

Each note has a `PitchTrajectoryProof`: source/current/next/destination context
indices, immediate ordered voice neighbors, independently supported local functions,
path status, extension relationship, reasons and a complete contextual receipt.
All pitched Score notes are audited. SFX retain their separate explicit action
ownership audit. Written duration is used; acoustic release tails remain the
existing audible sonority diagnostic's responsibility.

Extension relationships are `RestingColor`, `CommonTone`, `ForwardLeading`,
`OwnedTendency`, and `OrphanColor`. An authored open color must be explicitly
selected by material and pitch class; an `open` string or a label alone cannot own
it. Written chord extensions remain authored harmony. Natural colors need not
resolve universally. Orphan color is a soft unexplained-path finding.

The structural horizon follows root and triad identity through two changes;
C6/Cmaj7 color changes do not consume the I–IV–I window. Every immediate chord
change still participates in sustain/anticipation checks. Structural-note detection
is a documented proxy: integer beat, duration >=1 beat, or material endpoint.
It does not claim to recover every composer's intended accent.

Lead and bass are monophonic timelines, including entry into a new material.
Polyphonic support pairs equal-sized onset bundles in register order, one-to-one.
Changing bundle cardinality is explicitly ambiguous; no convenient later pitch
is borrowed as proof. This is a conservative reconstruction, not stored voice identity.
The report's vertical field is the existing written ensemble-slice verdict using
reconstructed functions and a world-free lenient color budget; it is not a claim
that the entire mix is perceptually clean.

One audit correction was necessary after the diagnostic checkpoint: material IDs
cannot divide a monophonic bass into separate physical voices. A control now proves
that an approach can resolve into the immediate next quoted phrase, and an octave
jump still fails. A second control refuses to equate repeated attacks with an actual
held suspension. Original pre-generation reports remain under `baseline/`; final
A and B reports are measured by the same revised observer.
The held-resolution check also requires the old event to end by its resolving
successor: sounding a second pitch while the first continues is not a resolved
hold. Together these revisions change the old deflected false-claim counts from
7/9 (Swiss/BLACK_ICE) to 9/10; the score fingerprints are unchanged.

## Generation

`perform_temporal` is opt-in. `perform` keeps R11; neither SongMap nor
PerformanceOptions changes. Lead search retains the motif contour/fidelity costs,
adds soft ownership and near-boundary next-guide costs, and only admits candidates
with the original gates. Legacy entry/register decisions prevent a changed exit
pitch from transposing every later phrase.

Material starts from the same rhythmic/contour proposal, jointly chooses its
structural targets in a small candidate window, then fits connectors. Notes that
cross a harmonic boundary also participate in that solve. Closed arrivals and
actual carry are hard contracts; desirable guide/common-tone paths are soft costs.
The lead and material search share a small extension-edge predicate, not an entire
second melody engine.

Support changes are restricted to evidence from the baseline: choose a common
pitch for the full written keys-stab gate, and aim bass approaches at the actual
root register. Existing voicing machinery is unchanged. There is no temporal
post-hoc repair pass and no change to Surgical. The comparison uses Independent
coupling; CoupledR8 and Surgical remain prior controls, not certified temporal arms.

## Matched structural receipts

All old and new local label-coverage counts remain zero. All new false function,
false suspension, broken anticipation, bad closed arrival and unresolved mandatory
claim counts are zero in the four acceptance scores.

| Witness | Swiss stable A → B | BLACK_ICE stable A → B | Swiss deflected A → B | BLACK_ICE deflected A → B |
|---|---:|---:|---:|---:|
| False function claims | 8 → 0 | 5 → 0 | 9 → 0 | 10 → 0 |
| False suspensions | 6 → 0 | 0 → 0 | 1 → 0 | 0 → 0 |
| Broken anticipations | 0 → 0 | 1 → 0 | 4 → 0 | 1 → 0 |
| Orphan extensions | 42 → 38 | 42 → 39 | 46 → 35 | 55 → 46 |
| Structural orphan extensions | 9 → 6 | 17 → 14 | 29 → 19 | 31 → 23 |
| Unresolved mandatory claims | 6 → 0 | 4 → 0 | 6 → 0 | 5 → 0 |
| Bad closed arrivals | 0 → 0 | 1 → 0 | 0 → 0 | 0 → 0 |
| Guide-path misses | 3 → 0 | 3 → 1 | 5 → 3 | 4 → 1 |
| Harmony-boundary path breaks | 3 → 1 | 4 → 0 | 4 → 0 | 11 → 2 |

False-claim types in A (every corresponding B count is zero):

| Score | False declared functions |
|---|---|
| Swiss stable | 2 ChordTone, 6 Suspension |
| BLACK_ICE stable | 1 ChordTone, 3 ChromaticApproach, 1 Anticipation |
| Swiss deflected | 3 ChordTone, 1 ChromaticApproach, 4 Anticipation, 1 Suspension |
| BLACK_ICE deflected | 3 LicensedExtension, 2 ChordTone, 4 ChromaticApproach, 1 Anticipation |

| Perturbation | Swiss stable | BLACK_ICE stable | Swiss deflected | BLACK_ICE deflected |
|---|---:|---:|---:|---:|
| Pitched notes | 449 | 449 | 403 | 401 |
| Changed pitches | 6 | 8 | 17 | 19 |
| Edit fraction | 1.34% | 1.78% | 4.22% | 4.74% |
| Changed durations/onsets | 0 | 0 | 0 | 0 |
| Removed/added notes | 0 | 0 | 0 | 0 |

Every changed pitch, old/new function, current/next chord and reconstructed reasons
is recorded in the final `*.changes.tsv`. Label-only corrections are counted
separately; e.g. a false suspension that is a real neighbor can retain its pitch.
No claim is made that the remaining orphan counts correspond one-to-one to audible
mistakes. Polyphonic identity ambiguity remains visible.

Residual boundary suspects include Swiss stable pad D6 at 20 moving to G5 at 24,
and BLACK_ICE deflected keys E5 at 63.5/111.5 moving to B5 at 64/112. These are
soft path findings, not false function claims. B still has 38/41/36/46 locally
legal but unexplained notes and 149/149/186/188 ambiguous voice assignments in
the table's column order. Global support-voice identity is deliberately parked.

SongMap fingerprints remain `0xaf5360442b9b6729` (StablePropulsion) and
`0x8d901e3de29190a2` (MeaningDirected/DeflectedLift). PerformancePlan fingerprints
remain identical in each A/B. The frozen four R11 score hashes are checked in a
regression test. Every new score passes the Round IX song projection check with
six thematic identity sites and fifteen harmonic landmarks. Round XI phenomenal
classification is unchanged because its source SongMap is identical.

## Preserved good wrong notes

Concrete acceptance-score examples (full contexts are in `*.good-paths.txt`):

- Swiss stable keys C#5 at 84.5 moves to C5 at 85 over Fmaj9: a real semitone approach.
- Swiss stable keys D5 at 115.5 lies between C5 at 115 and E5 at 116: a passing
  relationship as well as an available ninth; it reaches the next harmony's guide.
- BLACK_ICE stable pad B4 at 24 over Am6 is retained at 28 over Am9: an owned
  common-tone color. Its independent vertical report still exposes other ensemble
  contacts; temporal validity is not blanket vertical approval.
- BLACK_ICE deflected bass E2 at 3.5 belongs to the imminent Fmaj7 at 4 and steps
  to F2 there. The audit supports anticipation and chromatic approach predicates.
- Swiss stable lead E4 at 19.5/83.5 and BLACK_ICE stable lead G5 at 35.5/43.5
  anticipate the actual harmony, with gates ending exactly at its arrival.
  An intermediate implementation incorrectly required a strict overhang. The
  independent audit exposed that over-filter; positive and early-release negative
  controls now preserve all four notes without changing cost coefficients.

The physical D suspension across G7→C, selected open A13 arrival, E–F–G passing
line, and ii–V–I guide motion also have positive synthetic controls. No authentic
held suspension or explicitly selected open-color arrival is claimed to occur in
the acceptance audio merely because the engine supports those contracts.

## Listening and reproduction

```sh
cargo +1.98.1 run --release --example temporal_pitch_lab -- \
  --compare --render --out=target/humanmusic-r12/final
```

Seed 2112, 120 semantic beats, FusionConversation, default PerformanceOptions,
normal production. SWISS_SIGNAL renders first; BLACK_ICE follows. Each song is
composed once and reused by its two realizations. The harness checks unchanged
plan, event geometry, velocities, provenance, drums, SFX and harmony, and records
all changed pitches before rendering. It saves both regimes' scores/diagnostics;
the four primary WAVs are StablePropulsion A/B in the two worlds.

Primary listening files in `target/humanmusic-r12/final/`:

- `swiss_r11.wav`
- `swiss_temporal.wav`
- `black_ice_r11.wav`
- `black_ice_temporal.wav`

Notes use `{swiss,black_ice}_stable_{r11,temporal}.notes.tsv`; suspicious-note
reports use the same stems with `.audit.txt`. Deflected control artifacts replace
`stable` with `deflected`. WAV hashes and source hashes bind the final receipts.

Both R11 WAVs are byte-for-byte identical to the preserved Round XI files.
All four are 48 kHz stereo PCM16; Swiss has 3,048,814 frames per arm and BLACK_ICE
has 4,047,273 frames per arm. Rendering found no nonfinite samples.
The [final manifest](fixtures/humanmusic-r12/final-manifest.json) binds source,
score, plan, note, audit and audio hashes. Portable text receipts are committed;
the WAVs and full score dumps stay in the local artifact folder.

## Engineering

- Final `scripts/release/preflight.sh`: **PASS**, using stable 1.98.1, MSRV 1.85
  and serial tests. This includes fmt, strict all-target/all-feature clippy, full
  default tests, rustdoc with warnings denied, fresh MSRV consumer resolution,
  packaging, ABI v1, clean-room C/C++/Python/Go/Rust consumers and license notices.
- `RUST_TEST_THREADS=1 cargo +1.98.1 test --all-features`: **1,242 passed**, zero
  failures across 60 suites, including Round IX/X/XI and the new temporal controls.
- `cargo +1.85 check --locked --lib --all-features`: passed, including audio-cpal.
- Focused temporal controls: 31 passed; pitch classifier controls: 18 passed.
- Five example suites: 19 passed. Both prior release-mode HumanMusic fuzz sweeps
  passed, each covering 1,152 compositions: Coupled lost zero control receipts;
  Surgical lost zero silently and explicitly deferred nine. These remain prior
  control receipts, not evidence that every temporal realization has been audited.
- Protected source hashes verify unchanged world, language, SongMap, performance,
  interaction, voicing, synth, groove, phenomenal model, Cargo version and lockfile.

The first final preflight failed an obsolete anticipation negative fixture whose
gate ended exactly at the arrival. That failure is retained. The fixture now
releases early, while a separate exact-boundary positive control preserves the
good anticipations. Production behavior and rendered artifacts did not change
after this test-only correction.

The real output-device test and known upstream issue #15 key-delivery acceptance
remain ignored and unverified. Machine validation does not substitute for audition.

## PARKED

Global voicing redesign; removing residual orphan color; changing MusicalLanguage,
worlds, synthesis, mix or production; writing a psychology/quality score; default
promotion; formal stored voice identity for polyphonic support; general full-song
DP; acoustic-tail temporal semantics; additional post-hoc repair layers; Round XIII;
merges, tags and releases.

## Human-unverified

Whether the changed notes are perceptually better remains a human listening judgment.

**Did the bad wrong notes disappear without killing the good wrong notes?**
