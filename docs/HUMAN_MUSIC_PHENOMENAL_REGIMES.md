# HumanMusic Round XI: two phenomenal regimes

This is an architectural falsification experiment, not a pleasantness model. Start:
`5ac53ad365efce762cc6b6bc55952d30a10e227d`, branch `feat/v0.4-humanmusic-audio`,
base `7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`, draft PR #70. Remote and local
matched at the start; the worktree was clean. No version, ABI, MSRV or release change.

## Diagnosis and scope

The long-lived grammar really did require expectation and denial. Every Lift ends on
V7, every Deflect substitutes for home, and the legacy target demands `Prepare(High)`
even for `calm_loop`. `Arrive` exists in Round X but is an observed divergence from the
requested Miss. The chart candidate space excludes fulfilled arrival at that slot.
HookyFusion further prefers home duration over miss duration, strong reset and few
pedals. Strong expectation is a hard grammar/target condition, not a tunable prior weight.

Those source findings establish a narrow ontology. They do **not** establish which
layer caused the reported anxious/neurotic/dungeon perception. Semantic pressure also
feeds rhythmic instability; FusionConversation retains syncopation 0.6, interaction
0.85, subdivision 4, complexity budget 9 and chromatic connectives 0.55. BLACK_ICE
retains its A Aeolian, dark, saturated sound. Nothing in those downstream mechanisms
was changed to get the new composition to pass.

The experiment preserves `StructuralR9`, `MeaningDirected` (R10), the listener-model
concept and the one-song-upstream architecture. `StablePropulsion` is opt-in.
The first commit's two tests genuinely failed on the baseline; their log is retained.
The final tests retain the old behavior as a control and require the contrasting
explicit target to escape it. Low semantic pressure does not silently rewrite R10.

## Maps and witnesses

```text
SemanticTrace + explicit regime ──F──▶ PhenomenalTarget
                                      (vector + timed harmonic contour)
                                                   ▲
                                                   │ compared with μ
Performance ──π──▶ SongMap ──μ──▶ PhenomenalTrajectory
```

`PhenomenalTarget::from_trace` precedes composition: the explicit requested regime
selects the grammar, the trace supplies duration, and expectation determines preparation.
This is a family F_regime, not a claim that one trace has only one musical interpretation.
`PhenomenalTrajectory::observe` reads the actual canonical lead sheet and audible
written theme sites. It never reads the target label or grammar name. R10 MeaningPlan
is preserved separately; an absent new target does not change its frozen fingerprint.

| Coordinate | Musical witness and categorical boundaries |
|---|---|
| Stability | Tonic duration / chart duration; Mid ≥0.25, High ≥0.60 |
| Propulsion | Attacks / beat of written audible thematic statements; Mid ≥0.5, High ≥1.0 |
| Expectation | Peak concrete dominant pull toward home; Mid ≥0.5, High ≥0.9; pointer duration also reported |
| Surprise | Denied predicted arrivals plus unfamiliar harmonic moves; 0 Low, 1 Mid, ≥2 High |
| Openness | Written melodic register span in scale degrees; Mid ≥4, High ≥7 |
| Familiarity | Literal thesis statements heard so far; 0 Low, 1 Mid, ≥2 High |
| Closure/debt | Denied home arrivals since actual home, plus a currently sounding dominant; home clears debt |

These thresholds are declared musical diagnostics, not calibrated emotion scales.
No weighted total, enjoyment score, or optimizer exists. Propulsion is a written
rhythmic affordance; the observer does not certify rendered groove continuity or
perceived momentum. Openness is written register reach, not measured spatial texture.
Rows without a new thematic statement report zero new attacks; whole-song propulsion
summarizes the statements, not a demand that every player fill every beat.

`Arrival::Confirm` is positive meaning: a dominant arrives at its actual target, or a
previously heard non-home→home relation recurs successfully. The first plagal return
teaches the pattern; later ones confirm it. `Withheld` is its content-dependent rival.
The observer tracks these from preceding chords and learned returns, never from a
slot called Confirm. This small learned-transition model is deliberately inspectable;
it is not a general theory of human prediction.

The commutation check compares every requested vector coordinate, duration, debt,
and each timed harmonic state/confirmation. Short fragments can fail recognition or
confirmation; a missing chart cannot pass vacuously. The separate SuspendedDeflection
synthetic control has a global target with no prescribed timed contour.

## One contrasting grammar

`PropulsiveReturn` uses the cell `I – I – IV – I`, two bars per phase, closing an
incomplete final cell at home. It transports to `i – i – iv – i` in Aeolian.
IV is a tonic-prolonging neighbor, not a compulsory preparation for an unfulfilled V.
This is ordinary plagal prolongation; see John Peterson,
[Open Music Theory: Plagal Motion as a Form of Prolongation](https://viva.pressbooks.pub/openmusictheory/chapter/plagal-motion/).
That source supports the harmonic mechanism, not an affect or enjoyment claim.

The implementation uses the existing physical gestures `[Reset, Reset, Open, Reset]`
and existing chord realization. No Deflect was renamed; there is no Lift to force a
pointer. The legacy ChartCell carrier retains unused Lift/Deflect fields for compatibility.
Only the new timeline's Open/Reset coordinates are scheduled. Root-preserving satellites
leave rhythmic motion to the theme and the existing band. The chart remains degree-based;
worlds color/re-mode it without inventing another song.

The new thesis is `[0,2,4,2,7,4,2,0]` over `[.5,.5,1,.5,.5,1,.5,1.5]` beats: a short
rhythmic identity, octave expansion and tonic landing. It is literal from the first
lead-seated site and recurs before any performance development. The discourse uses
Establish/Restate/Return, with no compulsory Question, Miss or suspended-cadence debt.
It retains the existing arrangement planner, including the opening instrumental tease;
"immediate" here means first thematic statement, not a new lead solo on beat zero.
The one new bounded cell intentionally does not explore a family of theme candidates.

All changes are upstream song/meaning or diagnostics/harness code. The explicit
StablePropulsion composer preserves the R10 control's exact orchestration envelope and
schedules its new thesis in those same seats; a test checks equality. Its discourse and
harmonic gestures differ. No orchestration algorithm or patch was changed. The fixed-band
claim includes the same seats, engine and options; it does not require identical realized
notes or identical interaction decisions on two different songs.

## Discriminating controls and provenance

The investigation began with three read-only lanes, reconciled centrally. They share
repository/model provenance and are not three independent empirical confirmations.
The controls provide different probes of the one implementation:

- Actual chart and target witnesses disclose the restricted R10 ontology.
- Same tonic chart, same harmonic center, active versus held motifs: propulsion rises
  while stability stays High, expectation Low and debt zero.
- Repeated V→vi with no home: Low stability, High expectation, repeated withheld
  closure, outstanding debt; classified `SuspendedDeflection`, without a quality label.
- Change a learned scheduled I landing to vi: surprise/withheld closure rises and the
  stable target fails. Remove home, recurrence or rhythmic motion: the appropriate
  independent coordinate fails. Short valid fragments lose literal familiarity without
  panicking in a head-comparison helper. Changing metadata alone leaves μ unchanged.
- Fresh lengths 64, 97.5 and 160 beats at seeds 7 and 991 preserve the target; a
  9-beat fragment explicitly reports its recognition/confirmation limitations.
- Both actual songs pass nonempty π checks under SWISS_SIGNAL, VAPOR95 and BLACK_ICE.
  All prior R9/R10 controls remain required, including their known limits.

κ/contact is source, executable mutations and actual offline rendering. φ/survival
is bounded by these controls. σ/source is one local Rust implementation plus one music
education source family. ρ/commitment risk is guarded by preserved failing baseline
witnesses and mutations of the preferred song. Human affect is UNVERIFIED.

The render design is song × world × production (2×2×2). No numerical affect interaction
or phasor is computed: no lawful affect measurement exists here. A world swap changes
mode, tempo and performance constraints as well as timbre. It tests robustness across
worlds; only the same-score production control isolates production factors. A listener
finding B still anxious would keep surface/world/performance explanations alive; a
less anxious B supports an upstream contribution, without uniquely proving a single cause.

## A/B structural receipt (120 beats, seed 2112)

| Witness | A: MeaningDirected / DeflectedLift | B: StablePropulsion / PropulsiveReturn |
|---|---|---|
| SongMap | `0x8d901e3de29190a2` (unchanged R10) | `0xaf5360442b9b6729` |
| Stability / tonic occupancy | Mid / 28.3% | High / 80.0% |
| Propulsion / attacks per theme beat | High / 1.167 | High / 1.333 |
| Peak expectation / dominant exposure | High / 11.7% | Low / 0% |
| Surprise / withheld arrivals | High / 3 | Low / 0 |
| Openness / written degree span | Mid / 4 | High / 7 |
| Familiarity / literal thesis statements | High / 4 of 6 | High / 6 of 6 |
| Learned confirmations | 3 | 2 |
| Peak / final harmonic debt | 1 / 0 | 0 / 0 |
| Harmonic identity | I→V7→vi→IV→I, with diatonic satellites | I→I→IV→I, with held-root colors |

A also contains confirmations: the test does not equate meaning with a binary absence
of arrival. Its distinguishing feature is recurring preparation and denial before
return. It is not classified as the extreme SuspendedDeflection control because it
does return home and pays its debt. No failed classifier was relabeled to force a result.

Both songs pass π in both rendered worlds (six identity sites, fifteen harmonic
landmarks each; 31 chord changes for A, 30 for B). All four performances have zero
melody repairs, zero rejudged notes and zero unjustified notes by pitched role.
All eight WAVs are stereo 48 kHz PCM16, finite. SWISS_SIGNAL: 3,048,814 frames each;
BLACK_ICE: 4,047,273. The four A files match the earlier control renders byte for byte.
These are structural/rendering checks; no one has certified their affect by listening.

## Reproduction and listening

```bash
cargo +1.98.1 run --release --example human_music_lab -- \
  --phenomenal --beats=120 --seed=2112 --language=fusion --out=target/humanmusic-r11
```

A is preserved R10 MeaningDirected/DeflectedLift; B is StablePropulsion/PropulsiveReturn.
Both use the exact same semantic trace, seed and captured PerformanceOptions. Both
SongMaps are built before choosing a world. SWISS_SIGNAL is rendered first, then BLACK_ICE.
Normal A/B uses unchanged production. Harmonic references render the same scores with
the existing neutral production and harmonic stem mask (drums/SFX excluded), and record
individual peak-normalization gain. Compare A/B within a production, accounting for
that gain; peak normalization is not perceptual loudness matching.

Listen first to `swiss_signal/tension_deflection.wav` then
`swiss_signal/stable_propulsion.wav`: **Does B still sound like anxious dungeon music?**
Then compare the two BLACK_ICE songs and the paired harmonic references. None has been
claimed to sound better based on numerical proxies.

Exact fingerprints, chart/theme pages, trajectory rows, score diagnostics, WAV hashes,
commands and engineering receipts are in [fixtures/humanmusic-r11](fixtures/humanmusic-r11).
Audio is local under `target/humanmusic-r11`; the harness regenerates it without assets
or a device. The receipts retain source file hashes rather than pretending a commit can
contain its own hash. Full gates are recorded after validation.

## Engineering gate

Final-source publication-free release preflight passed: format, strict all-targets /
all-features clippy, denied-warning rustdoc, full default tests, locked and fresh
MSRV 1.85 consumers, package verification, ABI v1, external C/C++/Python/Go/Rust
consumers, notices and licenses. The final serial all-features run passed 1,208 tests;
the five example-local groups passed 19. Both explicitly invoked release sweeps
passed 1,152 compositions each, with zero coupled receipts lost and zero surgical
receipts lost silently. The all-features MSRV check passed separately.

One concurrent all-features attempt failed two FX Lab 1.6-second PTY header captures.
The earlier full run passed, all seven isolated PTY cases passed, and the final full
serial run passed. This is an observed intermittent gate failure; load sensitivity is
an inference. No UI repair is claimed. Both failed and passing receipts are retained.
The real-device playback test and known upstream terminal-input acceptance #15 remain
outside the executed tests. Four tests are marked ignored in the full log, including
the two HumanMusic sweeps that were separately executed above.

See [engineering.txt](fixtures/humanmusic-r11/engineering.txt) for exact commands and
[preflight.txt](fixtures/humanmusic-r11/preflight.txt) for the final gate output.

## PARKED

MusicalLanguage retuning; world/patch/mix changes; other composition grammars; a new
performance-gesture vocabulary; general chart redesign; psychology/enjoyment scores;
seed diversity beyond this bounded cell; automatic default promotion; voice/inversion
orchestration redesign; new synthesis; actual-performance phenomenal inference;
Round XII; merge, tag, package-version change or release.
The unrelated FX Lab capture sensitivity was recorded, not repaired.

**Human-unverified:** The engine can now deliberately express at least two distinct
musical phenomenal regimes. Which one is enjoyable remains a listening judgment.
