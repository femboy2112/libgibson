# Cover identity and constrained generation

A cover is a performance in a declared invariant fiber. For a reference performance `P`, selection `I`, and projection `κ_I`, the map is `C = κ_I(P)`. A lawful generated cover `P'` must satisfy `κ_I(P') = C` **and** the ordinary performance laws. There is no weighted similarity score. Neither equality nor a successful render establishes human recognition.

`CoverMap` contains selected relative musical coordinates. It does not retain the source `Composition`, `SongMap`, seed, semantic trace, instruments, or a source lookup. The generator accepts only the map and a target world, language, grammar, seed, and realization profile. Constraints enter form, harmony, stage, statement/material and source-event choices before rendering. No finished Score is repaired to force cover equality.

This document describes the implemented experimental API and its boundaries. Source-bound receipts that now exist: the [listening corpus](fixtures/humanmusic-cover/listening/README.md) (machine checks and hashes, bound to source `04e81fc`, before the groove-debt repair `1a06575`; see [Listening artifacts](#listening-artifacts)) and the cover-law tests listed under [Noninterference law](#noninterference-law) and [Freedom law](#freedom-law) (Observed at the commits named there). The post-repair corpus (16 WAVs byte-identical to `04e81fc`; see [Post-repair listening corpus](#post-repair-listening-corpus)) and the one-contact fresh holdout (7/36; see [Holdout v1](#holdout-v1-cover-rows)) are now recorded; final-source CI and Release Preflight are recorded in the [consolidation report](HUMAN_MUSIC_CONSOLIDATION.md). The retained source-import observations below have their own explicit evidence; they do not establish musical acceptance.

## Three routes

- Jam: existing immediate composition APIs.
- Song: build a `SongMap`, then perform its song fiber with an explicit profile.
- Cover: extract a `CoverMap`, then generate its remaining performance fiber.

```rust
use gibson::audio::human_music::{
    contract::CompositionGrammar,
    cover::{cover, CoverError, CoverMap, CoverSpec, CoverTarget},
    functor::Composition,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    MusicWorld,
};

fn reinterpret(
    reference: Composition,
    reference_world: &MusicWorld,
    target_world: &MusicWorld,
    seed: u64,
) -> Result<Composition, CoverError> {
    let spec = CoverSpec::from_contract(&reference.song.plan.contract);
    let map = CoverMap::extract(&reference, reference_world, spec)?;
    drop(reference);
    cover(&map, CoverTarget {
        world: target_world,
        seed,
        grammar: CompositionGrammar::HookArc,
        options: PerformanceOptions::default(),
        profile: PerformanceProfile::POCKET,
    })
}
```

`cover` is the checked boundary: it rejects failed cover conformance or pipeline receipts. `cover_candidate` retains a candidate for diagnosis; it makes no admission claim. `CoverConformance` independently reports each selected axis, while `CoverPipelineReceipt` checks song projection, Score structural validation, pitch claims, heard identity, causal hearing, voice edges, authored occupancy evidence, action witnesses and stage boundaries. These are enumerated checks, not a claim that every possible musical property was checked. Rejected candidates remain reproducible. Ordinary known-red cases are not renamed lawful covers.

| Entry point | Meaning of success |
| --- | --- |
| `perform` and historical `perform_*` | A result from that preserved experimental path; no new general acceptance claim |
| `perform_with_profile` / `realize_with_profile` | Validated profile/coupling and generated result; these do not run the cover admission checks |
| `cover_candidate` | Planned and realized candidate; conformance and pipeline may still fail |
| `cover` | Candidate passes the declared cover projection and current pipeline receipt |
| `cover_skeleton` | Partial candidate under an explicit target schedule; no checked full-cover claim |

`CoverError::Rejected` contains a `CoverAdmission` with conformance and pipeline receipts, not the rejected `Composition`. To retain a red object for inspection or rendering, call `cover_candidate`, then measure `CoverConformance` and `CoverPipelineReceipt` on that result. This avoids generating the same plan twice. The [dedicated lab](../examples/cover_music_lab.rs) follows this candidate route. The older `perform` default is unchanged; choosing `PerformanceProfile::POCKET` is explicit.

## Exact relations and remaining freedom

Generated-reference extraction requires every requested axis to be available. `ReferenceSong::extract` distinguishes the **requested** spec from the **effective observed** pins: a HookArc request from the current symbolic adapter pins Motif only; `all()` pins Motif and Riff from the same selected voice. Unsupported requested axes remain Unknown. Callers must inspect `map.spec` and `map.knowledge(axis)`; callers requiring every requested pin must reject any requested axis that is not Invariant. This is not an eight-axis freeze from one voice.

The default derives the selected axes from `CoherenceContract::anchors`, adding the phrase/family Form scaffold. Callers can explicitly pin/free axes. Unknown axes are never silently asserted. For a generated song the contract supplies the recognition declaration; an external source can only supply observations actually present.

| Source grammar | Default requested axes |
| --- | --- |
| HookArc | Motif, HarmonicContour, Groove, Form |
| LoopEvolution | Groove, HarmonicLoop, Orchestration, Form |
| RiffDrive | Riff, Groove, BassFigure, Form |
| WorldSwitch | Motif, BassFigure, Form |
| DeflectedLift | Motif, HarmonicLoop, Groove, Form |
| PropulsiveReturn | Motif, HarmonicLoop, Groove, Form |

These are the declarations in [`CoherenceContract::for_grammar`](../src/audio/human_music/contract.rs), with Form added by [`CoverSpec::from_contract`](../src/audio/human_music/cover.rs). They do not establish that every short source exposes every requested axis, or that a listener recognizes that selection. StablePropulsion chooses PropulsiveReturn, so its default follows that actual song contract.

| Axis | Pinned relation | Examples of excluded coordinates |
| --- | --- | --- |
| Motif | Whole actual lead line's tonic-relative chromatic pitches, normalized by one global octave, and canonical attack sequence; explicit source rest boundaries when supplied | Global octave, key, dynamics, gates within reservations |
| Riff | The selected primary lead line, or bass when no lead exists, under the same exact relation | Timbre, target rendering, legal articulation |
| Groove | Canonical kick/snare ordered multiset before target transport, including simultaneous-stroke multiplicity | Hats, velocities, timbre; kick/snare fill attacks are pinned |
| HarmonicContour | Exact relative root/quality sequence and metric landmarks | Voicing/register/production; extensions are currently part of the pinned quality |
| HarmonicLoop | Same explicit chart relation; loop structure is represented by that sequence | Same as harmonic contour |
| Form | Metric maps: exact phrase bar spans and family values. Ordered charts: ordered family equivalence, with source durations unknown | Target section energy and free arrangement details |
| Orchestration | Exact per-bar ArrangementRole vector in Lead/Keys/Pad/Bass/Drums order | Seat gain, patch, pan and voicing |
| BassFigure | Whole actual bass line's tonic-relative chromatic pitches, normalized by its own global octave, and canonical attacks | Global register, articulation, patch |

These are conservative exact relations. The current implementation does not infer a smaller latent riff, distinguish identity passing notes, or invent a missing harmonic analysis. The entire selected line/pattern is promoted. `max_transform` does not silently turn exact equality into approximate similarity.

Pitch units are explicit: historical generated motifs use `PitchBasis::ScaleSteps`; imported/pinned heard contours use `Semitones`. Transformations preserve the unit and interaction material retains the caller's exact contour. `typed_identity` adds the unit to the historical `MotifIdentity` contour/proportional-rhythm descriptor; it still identifies augmentation/diminution and is not full metric event identity. Canonical Motif fingerprints retain full stored rhythm and pitch basis. Exact cover rhythm uses rational `MetricPosition`.

Metric positions are reduced rational values, with float conversion at realization. Groove transport preserves the canonical source coordinate. Targets that merge distinct canonical positions or reverse pinned attacks/rest endpoints are rejected before source generation; repeated strokes at the same canonical position retain their multiplicity. Legacy drum extraction admits a unique canonical quarter-beat (sixteenth-note) grid event only within the historical producer's declared ±0.008_f32 beat jitter domain, considering the historical straight and swung source paths; ambiguity is an error. It is not a generic audio/MIDI quantizer or a global swing inverse.

Known source note durations reserve rests: generated articulation may shorten a gate, but may not extend the note-off beyond the supplied nominal endpoint. Instrument release tails remain governed by normal render physics; the reservation is not a promise of sample-perfect acoustic silence. Generated HumanMusic extraction currently pins canonical attacks, with articulation free when no nominal source-rest observation exists.

Free harmony is selected against pinned simultaneous melody/bass pitches before harmonic contexts, agency and notes. Candidate chords come from the target vocabulary. An infeasible chord domain is a typed error. Chord function labels are derived with the ordinary contextual classifier, not purported source facts. Source gates release at a foreign harmonic boundary unless an explicit suspension, retardation, anticipation or pedal law permits a carry; no finished-score clipping is used.

A selected fiber can be empty under a target. The current lift requires a supported 4/4 domain and monophonic pinned lines, bounded MIDI pitches, mutually compatible pins and target chord vocabulary, and transport that preserves order and piece extent. Modern canonical observation requires `EnsembleCoupling::Independent`; expression, authored occupancy and continuation physics must satisfy [`PerformanceProfile::validate`](../src/audio/human_music/policy.rs). Historical coupled/mass/tension paths are compatibility experiments, not automatically supported modern cover combinations. An excluded coordinate is free of the identity constraint; that does not guarantee every excluded coordinate varies in every generated pair.

## Source observation is not a freedom choice

`CoverKnowledge` distinguishes **Invariant**, **Free**, and **Unknown**. Unknown means not observed; it does not mean the source had no melody or that an invented melody was preserved. The same `CoverMap` supports a metric symbolic melody, complete generated reference, or partial ordered chart. Enrichment must explicitly supply newly observed fields; metric lift never guesses unknown source durations. There is no automatic chart/melody merge or enrichment method in this version: callers construct and validate the enriched map explicitly.

The generic `ReferenceSong::from_tsv(text, selected_voice)` boundary accepts explicit symbolic records:

```text
meter 4 4
key 7 major
tempo 100
length 64/1
note sop 0/1 1/1 71
```

Onset and duration are exact rationals. The caller selects a named monophonic voice. The parser validates every record, MIDI range, key, 4/4 meter, ordered selected notes, extent, overflow and duplicate metadata; unsupported records fail closed. Key and metric length are required; source tempo may be absent for extraction but is required for isolated source audition. It does not guess the melody from the highest notes. Source mode remains in `ReferenceSong` even when the chromatic quotient frees target mode.

The adapter retains every named voice as an observation but promotes only the selected voice into Lead-role Motif/Riff identity. Selecting a voice named `bass` does not create a BassFigure pin. Harmony, BassFigure, Groove, Form and Orchestration have no TSV ingestion records here and remain Unknown. A generated reference or an explicitly constructed validated CoverMap can provide those axes. Source provenance and license receipts remain beside the fixture, outside the generator.

The [symbolic importer](../scripts/audio/import_symbolic.py) converts a documented strict LilyPond subset and standard MIDI into normalized observations. It is source-agnostic. The [retained source provenance](fixtures/humanmusic-cover/sources/mutopia-528/provenance.json) records Mutopia catalog 528's Public Domain declaration, exact edition, source URLs and hashes. The explicitly selected `sop` voice has 62 notes, G major, 4/4, 100 BPM and 64 beats; all 245 named-voice events remain in the observations. The [import manifest](fixtures/humanmusic-cover/ode-import/manifest.json) separates these facts from derived choices and unknown harmony.

The [raw cross-check](fixtures/humanmusic-cover/ode-import/first-cross-check.json) differs: 245 Lily voice events versus 238 MIDI events. Under the declared within-staff projection, exactly two upper-staff and five lower-staff simultaneous unisons coalesce, and the [projected records agree](fixtures/humanmusic-cover/ode-import/cross-check.json). Both encodings share the same Mutopia edition. This corroborates the conversion within that provenance boundary, not independent historical correctness or an audio transcription. The generated accompaniment does not claim to reproduce the observed SATB arrangement or source chord labels.

## Partial Swing & A Miss experiment

The [partial reference provenance](fixtures/humanmusic-cover/swing-partial/provenance.json) preserves the maintainer's latest provisional ordered chord/section skeleton and the earlier conflicting chart. No lyrics, downloaded audio, inferred exact melody, or artist-style preset are included. The latest assertion is A Ionian, approximately 95 BPM/179 seconds, 4/4, with straight timing an assumption. The [ordered chart](fixtures/humanmusic-cover/swing-partial/ordered-chart.tsv) is verse/pre E–Bm–F#m–A, chorus E–Bm–D–A, post Bm–D–A–E, bridge F#m–D–A. Intro harmony, note-level identity, exact section/chord lengths and internal repetition counts remain Unknown.

`cover_skeleton(map, target, SkeletonSchedule { bars_per_chord: 1 })` produces a **partial-cover skeleton candidate** through `cover_candidate`. One target bar per chord and a generated tonic intro are explicit target decisions, not source observations; they do not reproduce the asserted 179-second duration. Family equivalence and ordered observed chords have separate conformance checks. These outputs cannot establish recognition of Swing & A Miss. Authoritative symbolic melody and timing would allow an explicitly enriched map without song-specific generator code; that reference and enrichment remain outstanding.

## Falsifiers and epistemic boundaries

Pinned lines are structural source events already in the declared metric/transport frame. They do not pass through optional ornament search; their phrase-expression plan and continuation graph are empty unless another source supplies links. This supports exact pinned attacks, generated bounded articulation, explicit-envelope lifetime and semantic reservations. It does **not** establish expressive phrase-treatment coverage by an empty plan. Ordinary free voices continue through their selected realization profile.

The [cover controls](../src/audio/human_music/cover_tests.rs) and [directed review controls](../tests/audio_cover_review.rs) specify actual-output mutation, free-voicing invariance, key/tempo changes, exact chromatic material, rest preservation, transport collision refusal, orchestration conflict refusal, source-gate harmonic crossings, deterministic generation, noninterference and freedom. The noninterference test changes reference seed/trace, non-lead pitches/dynamics and tempo, extracts equal maps, drops both references, then compares generated song/plan/score evidence and canonical plan/event fingerprints. A deliberate forbidden-source dynamics mutation must fail full-output equality even when the cover quotient passes. These are finite falsifiers with shared implementation provenance; their existence is not an execution receipt.

For fully observed maps, `CoverSpec::all()` freezes all eight **supported identity axes**, not every event attribute or PCM sample: articulation, dynamics, global register, hats and rendering still have freedom. Equal map and equal full target reproduce the same result. `CoverSpec::none()` retains only the metric domain; regenerated melody differs and original identity checks fail. That vacuous fiber is not a useful claim to be a cover.

**Disclosed within source inspection:** the extractor/generator data boundary and the relations implemented above. **Observed:** the linked retained source-import facts, raw mismatch and declared projected agreement. Development counterexamples are retained in the [review ledger](fixtures/humanmusic-cover/development-review/README.md); its receipts name their evolving-source boundary. **Observed after this paragraph was first written:** the retained listening corpus (lab renders and WAV hashes at source `04e81fc`, pre-repair) and the law tests named in the sections below. **UNVERIFIED at this documentation checkpoint:** post-repair lab renders/hashes, fresh holdout, final CI/preflight, listener recognition and interpretation quality. The provisional Swing chart's relationship to the real recording remains unverified. A generated test is not independent listening evidence. Final results belong in the consolidation report and must not be fitted after holdout contact.

## Noninterference law

A cover generator may read only the map and the target. The law is: two references whose selected coordinates are equal produce equal maps, and equal maps with an identical complete target produce identical covers, whatever else differed between the references. This is a finite-falsifier claim with shared implementation provenance, not a proof over all inputs.

- `equal_quotients_cannot_leak_free_reference_data` (`src/audio/human_music/cover_tests.rs`): changes reference seed/trace, non-lead pitches/dynamics and tempo, extracts equal maps, drops both references, then compares generated song/plan/score evidence and canonical plan/event fingerprints.
- `hostile_source_leak_would_fail_the_output_equality_witness` (same file): a deliberate forbidden-source dynamics mutation must fail full-output equality even when the cover quotient passes, so the witness can detect a leak.
- `contract_default_noninterference` (`tests/audio_cover_laws.rs`, commit `3216021`, Observed): perturbs only unpinned score data (Pad/Keys/Bass pitch and velocity, Lead velocity, hat/open-hat/clap velocity). It requires equal maps by canonical fingerprint and by Debug, identical covers over 2 seeds x {BLACK_ICE, SWISS_SIGNAL}, and identical refusals. Three positive controls confirm the perturbation harness is not vacuous: a changed lead pitch, a kick-to-snare change and a changed chord root each move the map.

## Freedom law

An excluded coordinate is free of the identity constraint. That does not guarantee that every excluded coordinate varies in every generated pair; the tested claim is weaker and explicit.

- `CoverFreedom` has four groups: `support_voicing`, `dynamics`, `percussion_detail`, `interactions`.
- `external_melody_survives_three_worlds_key_tempo_and_seed` and `all_axes_freeze_identity_but_not_articulation_and_none_is_not_a_cover` (`cover_tests.rs`) assert that `CoverFreedom::compare` reports some freedom while pinned coordinates hold.
- `every_free_group_varies_somewhere_in_a_small_sweep` (`tests/audio_cover_laws.rs`, commit `3216021`, Observed): Ode motif-only map, seeds 901-903 across three worlds; each `CoverFreedom` field is true in at least one pair. Each group is checked separately, not only their OR.
- `tempo_only_change_preserves_metric_cover_identity`, `transposition_only_preserves_relative_harmony_and_motif` and `language_only_change_is_either_conformant_or_an_explicit_refusal` (same file, `3216021`, Observed) change one variable at a time.

## Pinned groove and discourse obligations

Defect (fixed in `1a06575`): with Groove pinned, the kit plays its canonical pattern regardless of phrase role. `CoverConstraints::drums` never reads discourse and `apply_stage` keeps the drum seat on, so the kit is never stripped and no return can happen. Yet `resolve_obligations` opened a `GrooveDestabilization` obligation from the phrase role alone (`strips_groove` in `discourse.rs`), and `plan_settlements` skipped the `ReEntry` for a pinned Groove. Every generated contract-default cover therefore failed SongMap admission with exactly `unwitnessed_song_obligations: 1` (receipts in [diagnosis/](fixtures/humanmusic-cover/listening/diagnosis/README.md)).

Repair: a typed planning input `KitMotion { RoleDriven, Pinned }`, decided from the quotient (`map.groove.is_some()`) before discourse planning, threaded through `resolve_obligations_for`, `DiscoursePlan::build_for_kit` and `CompositionPlan::from_form_for_kit`. Existing entry points are exact `RoleDriven` wrappers. No `DiscoursePlan` field was added, so legacy Debug fingerprints and non-cover paths are unchanged. No gate, witness or threshold moved and no witness is manufactured.

Alternatives rejected: the diagnosis-time conjecture (reserve a silence and stamp a designated hat arrival) would fabricate a return from a departure that never happened, and the `ReEntry` witness also has a staged alternative, so the proposed silence reservation was self-imposed. A post-hoc ledger filter was rejected because `answer_referent` reads the ledger during `DiscoursePlan::build`.

Falsifiers in `cover_tests.rs` (Observed at `1a06575`): `pinned_groove_plans_no_groove_debt_and_the_generated_cover_is_admitted` (BLACK_ICE seeds 901/902 at 95 BPM now admitted; no Drums `ReEntry`; no action stamped on any drum stroke), `freed_groove_keeps_its_debt_and_a_real_reentry_witnesses_it`, `an_unwitnessed_groove_settlement_still_fails_admission`, `pinned_kit_ledger_is_the_role_driven_ledger_without_groove_debts`, `ordinary_song_planning_is_unchanged_by_the_kit_seam`. Mutation check: forcing `RoleDriven` turns the repro test red (it trips the new `debug_assert` in `plan_settlements`). The retained corpus below predates this repair and still shows the failure.

## Known limits and refusals

- **SWISS_SIGNAL pinned-harmony refusal (Observed, `3216021`).** The generated HookArc contract-default map is refused by SWISS_SIGNAL with `Invalid("pinned harmony outside target vocabulary")`, identically for both references. Cover generation is therefore not total across worlds when harmony is pinned. This is a lawful, explicit refusal. Whether `HarmonicContour`'s exact chord-quality relation should admit a quality-family relation is a maintainer decision and is not made here. The listening corpus shows the same refusal for the two INFEASIBLE generated targets.
- **Ode `transposed_faster` (Observed at `04e81fc`).** The preserved candidate has three temporal-function claims and is not counted as admitted.
- **Generated VAPOR95 (Observed at `04e81fc`).** The candidate has one temporal-function claim in addition to the `song: false` failure; the `1a06575` falsifier covers BLACK_ICE seeds 901/902 only, so the VAPOR95 song failure has not been re-measured.
- **Partial Swing has no melody.** Only ordered relative harmony and section-family topology are pinned; no recognition claim of any kind follows.
- The pinned-harmony refusal and temporal claims have not been re-measured after `1a06575`; the corpus is pre-repair.

## Listening artifacts

Corpus: [listening/README.md](fixtures/humanmusic-cover/listening/README.md), [receipt.json](fixtures/humanmusic-cover/listening/receipt.json), [WAV_SHA256SUMS](fixtures/humanmusic-cover/listening/WAV_SHA256SUMS), with per-run reports under [ode/](fixtures/humanmusic-cover/listening/ode/report.txt), [swing-partial/](fixtures/humanmusic-cover/listening/swing-partial/report.txt), [generated/](fixtures/humanmusic-cover/listening/generated/report.txt) and [diagnosis/](fixtures/humanmusic-cover/listening/diagnosis/README.md). WAVs stay local under `target/humanmusic-cover/`; only hashes and metadata are committed. The corpus is source-bound to `04e81fc`, before the groove-debt repair.

| Run | Admitted (declared machine checks pass) | Preserved failed | Infeasible |
| --- | ---: | ---: | ---: |
| ode | 5 | 1 (`transposed_faster`, 3 temporal-function claims) | 0 |
| swing-partial | 5 | 0 | 0 |
| generated | 0 | 3 (`song: false`; VAPOR95 also 1 temporal claim) | 2 (pinned harmony outside target vocabulary) |

Human recognition and musical quality are **UNVERIFIED**; a successful render command does not mean a candidate passed its checks.

<!-- POST-REPAIR-LISTENING -->
## Post-repair listening corpus

**Observed** at `ced9833` ([receipt](fixtures/humanmusic-cover/listening-post-repair/README.md)): the three
cover lab runs regenerated with identical inputs produce **16 WAVs byte-identical** to the `04e81fc`
corpus; Ode and Swing reports are byte-identical; generated candidates changed only their `perf`
fingerprints and moved from `song: false` to `song: true`, admitting both BLACK_ICE covers (VAPOR95
stays red on one temporal claim; two vocabulary refusals unchanged). Human recognition remains
UNVERIFIED.
<!-- /POST-REPAIR-LISTENING -->

<!-- HOLDOUT-V1-RESULTS -->
## Holdout v1 (cover rows)

**Observed, first and only contact** at `6ac812f` (architecture frozen at `3216021`; release profile;
executed configuration byte-identical to the declared SHA256 `c45fa6de…`). Raw receipts:
[`fresh/results/`](fixtures/humanmusic-consolidation/fresh/results/) (committed `8a0b0cd`); post-contact
analysis: [`fresh/CLASSIFICATION.md`](fixtures/humanmusic-consolidation/fresh/CLASSIFICATION.md).

- **7/36 cases pass**: all six external Ode covers and G29. 29/30 generated cases fail; 67 of 970
  receipts fail; exit 101 preserved.
- Largest families are source-side ordinary POCKET generation on fresh short, partial-bar and
  Simple-language inputs: a partial-final-bar bass reservation (`invalid reservation for Bass`, 16
  sources + 2 covers; 0/9 bar-aligned vs 16/21 partial-bar sources), absolute action-witness gaps (15;
  Simple language fragment 0/12, hit 6/24), and unwitnessed song obligations/theme sites (7).
- 11 extractions refuse `MissingAxis(Groove)` because those sources are literally drumless while the
  contract default pins Groove; 4 cover lifts are explicit vocabulary refusals; 7 of the 11 generated
  covers that exist fail at least one cover-side receipt.
- Held identity failed 0 times (30 sources, 17 covers).
- Nothing was repaired after contact. Any repair must first write its counterexample and be judged on a
  second untouched holdout. The finite, deliberately chosen matrix is not a population sample; generator
  and observers share provenance.
<!-- /HOLDOUT-V1-RESULTS -->

## Reproduction and listening

The committed symbolic fixture can be used directly. To reproduce ingestion from the retained local sources, use a separate output directory and the [pinned parser requirements](../scripts/audio/requirements-symbolic.txt):

```sh
python3 -m venv target/humanmusic-cover-parser-venv
target/humanmusic-cover-parser-venv/bin/python -m pip install -r scripts/audio/requirements-symbolic.txt
target/humanmusic-cover-parser-venv/bin/python -m unittest discover -s scripts/audio -p 'test_import_symbolic.py'
target/humanmusic-cover-parser-venv/bin/python scripts/audio/import_symbolic.py \
  --lily docs/fixtures/humanmusic-cover/sources/mutopia-528/ode.ly \
  --midi docs/fixtures/humanmusic-cover/sources/mutopia-528/ode.mid \
  --voice sop --voice alto --voice tenor --voice bass \
  --midi-group upper=sop,alto --midi-group lower=tenor,bass \
  --out target/humanmusic-cover-import-reproduction
```

The following are reproduction commands, not a claim that final-source results are already green:

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 test --lib audio::human_music::cover::tests
CARGO_BUILD_JOBS=1 cargo +1.98.1 run --release --example cover_music_lab -- --reference=docs/fixtures/humanmusic-cover/ode-import/reference.tsv --voice=sop --out=target/humanmusic-cover/ode --render
CARGO_BUILD_JOBS=1 cargo +1.98.1 run --release --example cover_music_lab -- --ordered-chart=docs/fixtures/humanmusic-cover/swing-partial/ordered-chart.tsv --out=target/humanmusic-cover/swing-partial --render
CARGO_BUILD_JOBS=1 cargo +1.98.1 run --release --example cover_music_lab -- --out=target/humanmusic-cover/generated --render
```

The dedicated lab renders the observed source melody where available, a same-source-key/mode/tempo baseline, alternate seed, BLACK_ICE/SWISS_SIGNAL/VAPOR95 and transposed/faster variants. It writes per-axis conformance, free-coordinate comparisons, ordinary pipeline receipts, notes, temporal/identity/action/hearing ledgers, and optional WAVs. Red candidates remain labeled. No human-feel transport is added: the established pocket remains the timing authority.

Listen without the title if possible: is it recognizably the same song played by different bands? Do accompaniment, articulation and texture interpret the piece? Does the accepted R17 BLACK_ICE pocket remain intact? Those answers belong to the maintainer's ear.

## Rust source migration

Existing entry functions remain, and historical Debug/fingerprint formulas are explicitly preserved; final artifact equivalence has a separate gate. New public struct fields (`Motif.pitch_basis`, `InteractionMaterial.pitch_basis`, `PerformancePlan.cover_constraints`, `Score.observed_lifetime`) and enum cases (`SectionFamily::Named`, `ActionCause::Discourse`) require updates to external Rust struct literals/exhaustive matches.

Historical motifs/materials use `PitchBasis::ScaleSteps`; unconstrained performance plans use `cover_constraints: None`. `observed_lifetime: None` retains archived Score observation semantics; modern profile entry points declare their observation policy. Prefer the SongMap/PerformancePlan constructors and `Score::new`. Do not silently migrate all legacy Score literals to modern observation. This is not blanket Rust source compatibility. These experimental Rust-only objects are outside the C ABI; the final-source ABI/consumer gate is still required.
