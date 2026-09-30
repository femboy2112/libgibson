# Cover identity and constrained generation

A cover is a performance in a declared invariant fiber. For a reference performance `P`, selection `I`, and projection `κ_I`, the map is `C = κ_I(P)`. A lawful generated cover `P'` must satisfy `κ_I(P') = C` **and** the ordinary performance laws. There is no weighted similarity score. Neither equality nor a successful render establishes human recognition.

`CoverMap` contains selected relative musical coordinates. It does not retain the source `Composition`, `SongMap`, seed, semantic trace, instruments, or a source lookup. The generator accepts only the map and a target world, language, grammar, seed, and realization profile. Constraints enter form, harmony, stage, statement/material and source-event choices before rendering. No finished Score is repaired to force cover equality.

This document describes the implemented experimental API and its boundaries. Final-source cover controls, listening renders, fresh holdout, CI and Release Preflight remain **UNVERIFIED** until their source-bound receipts are recorded in the [consolidation report](HUMAN_MUSIC_CONSOLIDATION.md). The retained source-import observations below have their own explicit evidence; they do not establish musical acceptance.

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

**Disclosed within source inspection:** the extractor/generator data boundary and the relations implemented above. **Observed:** the linked retained source-import facts, raw mismatch and declared projected agreement. Development counterexamples are retained in the [review ledger](fixtures/humanmusic-cover/development-review/README.md); its receipts name their evolving-source boundary. **UNVERIFIED at this documentation checkpoint:** final-source controls, final lab renders/hashes, fresh holdout, final CI/preflight, listener recognition and interpretation quality. The provisional Swing chart's relationship to the real recording remains unverified. A generated test is not independent listening evidence. Final results belong in the consolidation report and must not be fitted after holdout contact.

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
