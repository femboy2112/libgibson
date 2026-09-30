# HumanMusic consolidation and cover mission

## Mission state

Start: `207f0ab092e3d0a274998d84a245e7b78d096f97`, branch
`feat/v0.4-humanmusic-audio`, origin `git@github.com:femboy2112/libgibson.git`.
The initial worktree was clean; fetch found no newer branch work. PR #70 is draft.
This is an in-progress architectural audit, not a completion claim.

**Observed, human:** the maintainer accepts the Round XVII BLACK_ICE flagship
pocket: “The musician is in the pocket. It sounds good.” Residual robotic feel
is an aesthetic limitation. This does not accept R17 generally. The known sweep
remains **120 executed / 61 complete passes / 59 first-assertion failures**.
No old holdout fitting, humanization, default-arm promotion, package version,
ABI, tag, release, merge, or gate weakening is authorized by this mission.

The audit below was written before production edits. Its line references refer
to the start SHA. Later sections will record implementation and exact evidence.

## Current implementation: authority and law boundaries

This section describes the integrated objects and call boundaries. It is a source
map, not final-source acceptance: cover controls, final listening artifacts, fresh
holdout and final engineering gates remain **UNVERIFIED** until their receipts are
recorded (later cover-law and pre-repair listening receipts now exist; see the
[claim ledger](#current-claim-ledger)). The before-state audits and historical controls below remain evidence.
The [cover guide](HUMAN_MUSIC_COVER.md) gives exact defaults, relations, API use and
reference-ingestion limits.

```text
SemanticTrace
  -> SongMap::build / compose                   song identity and composition
  -> PerformancePlan::from_song                world/language performance choices
  -> realize_with_profile / historical adapter
       lead -> keys -> bass -> explicit links -> pad -> drums
  -> Score                                     events plus source/observer evidence
  -> HumanMusicSynth + world/production/rate    scheduling and PCM

Composition + source world + CoverSpec -> CoverMap::extract
ReferenceSong + selected observed axes -> ReferenceSong::extract -> CoverMap
CoverMap + CoverTarget
  -> fresh constrained SongMap
  -> PerformancePlan::from_song_constrained
  -> source realization -> candidate Composition
  -> CoverConformance + CoverPipelineReceipt -> checked cover or rejection

OrderedChart -> partial CoverMap + explicit target SkeletonSchedule
  -> cover_skeleton -> candidate only
```

The graph's player order is the modern independent source path; historical
coupled/repair adapters keep their own archived causality. `perform` and immediate
composition keep the historical default. `perform_with_profile` validates the
selected laws but does not itself certify every diagnostic. `cover_candidate`
preserves a result for inspection; `cover` additionally checks its selected
projection and enumerated pipeline receipts. A rejected `cover` returns admission
receipts, not the Composition; diagnostic callers can retain a candidate directly.

| Concern | Current authority and boundary | Executable law / falsifier |
| --- | --- | --- |
| Song form, themes and declared harmonic landmarks | `SongMap`; the performance derives its plan from it | `SongMapConformance`; source/plan/actual-note mutations remain distinct |
| Cover identity | `CoverSpec` and `CoverMap`, with no retained source Composition or lookup | `CoverConformance` reads actual events; pinned pitch/landmark mutations fail; excluded voicing mutation does not change the quotient |
| Cover generation | Fresh constrained song and `PerformancePlan::from_song_constrained`; constraints precede harmony, stage, material and note choices | Equal extracted maps plus identical complete targets produce equal generated evidence; a deliberately injected excluded dynamics field breaks that witness |
| Pitch units and chord membership | `PitchBasis`; `PitchClassSet` and the shared source stable-function relation | Scale-step/semitone distinction; exhaustive chord masks; independent temporal sustain-crossing falsifier |
| Canonical rhythm | `MetricPosition`, `GrooveTransport`, carried `PerformedPosition`; one lattice enumerator | Transported position projects to its exact carried metric source; distinct-position collisions are rejected in constrained generation |
| Authored floor ownership | `AuthoredOccupancy`, separate from acoustic notes | Empty acoustic input does not release a reserved attack or an owned rest; cover occupancy checks use canonical source reservations |
| Direct audibility | `voice` envelope/continuation law and borrowed `HeardWindows` | Only matching continuation edges can shorten direct envelopes; unrelated same-role notes and empty modern graphs cannot imply choke |
| Causal hearing and agency | Final upstream notes consumed before each dependent source; Score hearing ledger | Missing/stale source observations and source/score mutations are inspectable; retained hearings must agree with final source notes |
| Orchestration | Stage seats and admitted actions before source generation | `orchestration_violations`; incompatible cover pins fail before realization; partial Form has its own family-equivalence relation |
| Realization policy | Orthogonal `PerformanceProfile` plus one validation boundary | Source expression and render lifetime can vary independently; unavailable support treatment exists only in the historical factorial adapter |
| Rendering | Existing synth scheduling consumes events and explicit continuation edges | Short PCM controls plus separate full/stem SHA256 comparison; event fingerprints alone cannot certify PCM |
| Historical experiments | Thin public wrappers, explicit archived policy/repair choices and frozen receipts | Accepted event/evidence characterization, historical hashes and exact known-case replay |

The core no longer selects its musical laws by a chronological Contract arm.
Round names remain receipt provenance and public compatibility names. Score remains
the compatibility container; moving its evidence into another object or adding a
RenderContract wrapper did not establish an additional authority and was deferred.
Its explicit observation policy is distinct from the continuity graph consumed by
synthesis. The source-level expression admission policy is also distinct from
render lifetime; historical mono factors deliberately map to both for reproduction.

Canonical schemas bind declared fields, enum tags, coordinate units and sequence
order without using Debug. Core transitive schemas use exhaustive destructuring;
the newer cover-value encoders in `cover.rs` have explicit field lists and require
manual schema review when a field is added. They do not inherit an automatic promise
that every future field enters identity. Legacy Debug encodings remain separately
named historical receipts. `typed_identity` tags the old proportional motif shape
with its pitch basis; it still quotients rhythmic augmentation. Exact cover rhythm
uses rational positions, and canonical Motif fingerprints bind stored rhythm.

### Reference authority, supported fibers and compatibility

The [import manifest](fixtures/humanmusic-cover/ode-import/manifest.json) retains
245 named Ode voice events and an explicitly selected 62-note soprano. The raw
Lily/MIDI comparison is 245 versus 238, not equal. The [declared staff projection](fixtures/humanmusic-cover/ode-import/cross-check.json)
coalesces exactly seven simultaneous unisons and agrees. Both encodings share one
Mutopia edition; this corroborates conversion within that provenance boundary,
not an independent historical source or a listener's recognition. The exact source
edition, stated Public Domain status, URLs and hashes are in the [source custody record](fixtures/humanmusic-cover/sources/mutopia-528/provenance.json).

`ReferenceSong` retains all named observations but its current 4/4 TSV adapter
promotes only the selected monophonic voice to Motif/Riff. A HookArc request becomes
an effective Motif-only map; `all()` becomes Motif plus Riff, with unavailable axes
Unknown. Generated-reference extraction instead requires its requested axes.
Callers requiring every requested pin must check the returned effective spec and
knowledge states. No current TSV record imports source chords, BassFigure, drums,
orchestration or phrase families. No automatic melody/chart enrichment is promised.

For a fully observed map, all eight identity axes still leave articulation,
dynamics, global register, hats and rendering outside the quotient; there is no
all-event/PCM freeze profile. Conversely, the empty selection retains only the
metric domain and has no useful song-identity claim. Different worlds/languages
can make a selected fiber infeasible under pitch vocabulary, stage, range or groove
transport restrictions. Pinned structural lines retain exact transported attacks
and generated bounded gates but bypass optional ornament search; empty phrase plans
are not evidence of expressive treatment. These are current feature limits.

The [provisional Swing reference](fixtures/humanmusic-cover/swing-partial/provenance.json)
contains maintainer assertions and an ordered chord/section skeleton, not melody,
characteristic bass/drums or exact source timing. `cover_skeleton` adds a declared
target schedule and tonic intro; it produces a partial candidate, not a checked
recognizable cover of the recording. A full reference and blind human recognition
remain outstanding. No artist preset, transcription from memory or feel noise is
introduced by this consolidation.

Historical public functions remain, but experimental Rust source compatibility is
narrower than native ABI preservation. Exhaustive struct literals need new
`Motif.pitch_basis`, `InteractionMaterial.pitch_basis`,
`PerformancePlan.cover_constraints` and `Score.observed_lifetime` fields; exhaustive
enum matches need `SectionFamily::Named` and `ActionCause::Discourse` cases. The
[migration note](HUMAN_MUSIC_COVER.md#rust-source-migration) gives historical values
and preferred constructors. These Rust-only audio objects are outside the C ABI;
final-source ABI and clean-room consumer checks remain mandatory.

### Current claim ledger

| Claim | Status and boundary |
| --- | --- |
| Accepted R17 BLACK_ICE pocket sounds good | **Observed**, maintainer audition of the flagship only |
| Start-source accepted controls reproduce | **Observed**, 48 WAVs and 757 text receipts at the frozen start source; manifest below |
| Historical known sweep remains 61/59 | **Observed**, exact replay source and first-failure comparison recorded below; known data |
| New objects express the stated source/lifetime/cover relations | **Disclosed within code inspection and the individually cited law-test boundaries**; not universal musical acceptance |
| Ode source encodings agree under declared staff projection | **Corroborated within a shared edition**, with the raw mismatch retained |
| Cover laws (tempo-only, transposition-only, language-only, contract-default noninterference, per-group freedom) | **Observed**, `tests/audio_cover_laws.rs` 5/5 at `3216021`; the generated contract-default map's SWISS_SIGNAL refusal is an Observed lawful limit, see [Known unresolved defects](#known-unresolved-defects) |
| Pinned-groove groove-debt repair | **Observed** at `1a06575`: lib cover tests 19/19, discourse/song/plan lib tests 31/31, `audio_consolidation_characterization` 1/1, `audio_realization_policy` 8/8, `audio_canonical_fingerprint` 5/5, strict clippy and fmt clean; mutation check red as recorded |
| Cover lab renders and WAV hashes | **Observed** at pre-repair source `04e81fc` ([corpus](fixtures/humanmusic-cover/listening/README.md): ode 5 admitted/1 failed, swing-partial 5 admitted, generated 0 admitted/3 failed/2 infeasible); post-repair regeneration **Observed** at `ced9833` ([receipt](fixtures/humanmusic-cover/listening-post-repair/README.md)): 16 WAVs byte-identical, generated 2 admitted/1 failed/2 infeasible |
| PCM preservation and final engineering gates at the final SHA | **Observed**: R17 pocket 48/48 WAVs + 757/757 receipts byte-identical and known sweep 61/59 unchanged at `8a0b0cd`; local release preflight 13/13 and GitHub CI + Release Preflight success at `91c3820` (see Final gate below) |
| Fresh 36-case holdout | **Observed** once at `6ac812f` (committed `8a0b0cd`): 7/36 pass (6/6 external Ode covers, 1/30 generated); failures preserved and classified, none repaired after contact; see holdout results below |
| Cover recognition, interpretation quality and post-consolidation listening acceptance | **UNVERIFIED**, requires the maintainer's ear |

## Phase 0: accepted artifact freeze

**Observed:** regeneration at the start source with Rust 1.98.1 matched all
48 required R14/R16/R17 full/stem WAVs and all 757 emitted text receipts byte
for byte. Historical factorial diagnostic receipts are included; the optional
34 factorial/stable-precursor WAVs were not rerendered in this freeze (their
committed hashes are preserved).

- [Freeze manifest](fixtures/humanmusic-consolidation/baseline/manifest.json)
- [WAV and receipt comparison](fixtures/humanmusic-consolidation/baseline/equivalence.json)
- [Exact start CI](fixtures/humanmusic-consolidation/baseline/ci.json)
- [Exact start Release Preflight](fixtures/humanmusic-consolidation/baseline/release-preflight.json)
- `tests/audio_consolidation_characterization.rs`: cheap event/performance/song,
  continuation, occupancy, action, hearing, phrase and pocket receipt protection.
- `scripts/verify-pocket-freeze.py`: separate full/stem PCM byte-equivalence gate.

The BLACK full mix hash is
`6fe3abf55cee22b69e6d3c06684574770eea40445de4183588cfd12707382f4d`.
No production source had changed when these artifacts were regenerated.

## Evidence and independence

The three source audits split call-graph/policy, rhythm/physics, and cover-source
questions. They share repository and model provenance and are not independent
human bearings. Structural equivalence, PCM byte equivalence, and listening
acceptance remain separate claims. No phasor model is asserted. Configuration
contrasts preserve their source path; a nonzero interaction is non-additivity,
not a mechanism by itself.

κ: direct source/receipt contact and one maintainer audition. φ: limited to the
executed invariants. σ: receipt encodings and source-coordinate ownership are
under audit. ρ: protecting the accepted flagship requires mutation falsifiers,
not assuming an architectural rewrite is an improvement.


## Before: authority graph


Read-only source audit. Paths below are relative to `src/audio/human_music/`.
This is code-path evidence, not new musical acceptance or a completed consolidation.
The R17 BLACK_ICE pocket acceptance is the maintainer's observation; the general
120-case result remains 61 passes / 59 first-assertion failures.

### Actual dependency and authority graph

```text
SemanticTrace
  -> IntentTimeline::walk                         timeline.rs
  -> CompositionPlan::build*                      plan.rs
       CoherenceContract + FormGraph + discourse + arrangement + backbone
  -> SongMap::build / SongMap::compose             song.rs:164, song.rs:227
       ThematicMap, optional HarmonicMap, meaning/phenomenal target
  -> PerformancePlan::from_song                   performance.rs:257
       harmony -> actions -> Stage/admissions -> harmonic actions/regions
       -> AccentGrid -> interactions/materials -> ensemble -> obligations -> budget
  -> functor::realize_arm                         functor.rs:334
       lead -> keys -> bass -> voice links -> pad, then drums
       (historical coupled/surgical/mass/tension variants have different order)
  -> Score                                        score.rs:357
       musical events + physics flags/edges + diagnostics/evidence
  -> HumanMusicSynth::with_production             synth.rs:584
       validate continuity -> sample schedule -> voices -> production -> PCM
```

The production-facing default is still historical `perform` (functor.rs:86),
which builds a plan and calls `realize(..., false)`. `perform_pocketed` is an
explicit opt-in and must remain so. `compose_full` delegates to `perform`, not
to R17 (functor.rs:73-82).

| Object / concern | Actual owner and consumers | Classification |
|---|---|---|
| Song identity | `SongMap` (`song.rs:133`), built independently of world; conformance `song.rs:404` | single authority with declared gap |
| Composition | `SongMap::build/compose`; meaning-directed search `composer::compose_meaning` (`composer.rs:567`) | single entry boundary; alternate composers |
| Form | `CompositionPlan::form: FormGraph` (`plan.rs:142,633`); score sections derived by `functor::sections_from_plan` (`functor.rs:746`) | single authority; projection |
| Motif identity | `ThematicMap`/`ThemeSite` (`song.rs:51,84`); `MotifBank` (`motif.rs:355`); `plan_interactions` consumes it (`performance.rs:396-414`) | single song authority |
| Harmonic identity | optional `HarmonicMap` (`song.rs:123,152`), `backbone::realize`; non-backbone grammars use `HarmonyEngine::generate` (`performance.rs:272-282`) | unclear/incomplete ownership: some harmony remains performance-owned |
| Coherence anchors | `CoherenceContract::for_grammar` (`contract.rs:85-196`); checked by `CoherenceDiagnostics` (`diagnostics.rs:27`) | single declaration; diagnostics do not make all axes hard constraints |
| Performance planning | `PerformancePlan::from_song` (`performance.rs:257-489`) | single authority; large but causally coherent |
| Stage/orchestration | `Stage::from_arrangement`, `admit_actions` before realization (`performance.rs:311-312,814`); `stamp_arrangement` is provenance only (`functor.rs:767`) | single authority; do not revive old post-hoc arrangement deletion |
| Pulse/accent coordinates | `AccentGrid` fixed 16 steps (`performance.rs:54-120`); `pocket::LatticeSlot` (`pocket.rs:51-95`) | duplicate coordinate representations with different domains |
| Swing | `GrooveEngine` historical rule (`groove.rs:169`), current drum local closure (`groove.rs:277`), `phrase_expression::groove_position` (`phrase_expression.rs:777`) | duplicate authority; first rule is historically different |
| Phrase expression | `phrase_expression::realize_impl` (`phrase_expression.rs:173`), pocket candidates (`pocket.rs:126`) | single source planner with historical strategies |
| Physical connective viability | `expression::ConnectiveViability`, observer using `voice::effective_audible_end` (`expression.rs:287-299`) | single predicate; phrase generator chooses candidates before veto |
| Semantic occupancy | `AuthoredOccupancy`, authored lead/bass reservation stream committed by `functor` (`functor.rs:365,440-469`) | source authority, distinct from acoustic hearing |
| Downstream agency | keys/drums owned variants consume authored occupancy (`functor.rs:386,549`) | source intent authority |
| Voice continuity | `VoiceEventId`, `VoiceContinuation`, effective endpoint (`voice.rs:18-145`); source construction currently in `pocket::continuations` (`pocket.rs:260`) | core physics exists but source constructor lives under chronology |
| Heard lifetime | raw envelope in `sonority::audible_end_at` (`sonority.rs:956`); link truncation in `voice.rs:78`; `audible_voices` reconstructs note correspondence (`sonority.rs:837`) | two-layer law, leaky placement/dependency |
| Support voicing | `voicing::voice_path` (`voicing.rs:1100`); `comp::heard_pad_path_impl` (`comp.rs:1434`); `support_voicing::select*` | distinct valid responsibilities; source choice before hearing |
| Score events | `Score.notes/drums/sfx/chords/sections` (`score.rs:357-408`) | single mutable event collection mixed with evidence and physics |
| Synthesis/render | `HumanMusicSynth::with_production` (`synth.rs:584`) and render loop (`synth.rs:783`) | single PCM authority |
| Diagnostics | `diagnostics`, `temporal`, `sonority`, `identity`, `mass`, `tension`, phrase/pocket diagnostics | diagnostic-only observers; intentional observer/generator independence |
| Fingerprints | debug-FNV in `song.rs:291-335`, `performance.rs:493`, `score.rs:453` | duplicate noncanonical encoding authority |
| Historical experiments | public `perform_*` wrappers + private `Contract` (`functor.rs:98-315`), `EnsembleCoupling` (`performance.rs:167`), several `*_r11` branches | legacy adapters plus historical negative controls; currently leak into core orchestration |

SongMap conformance explicitly does not audit realized note pitch content
(`song.rs:398-403`). The pitch equality of an R16 and R17 score is therefore
not interchangeable with the existing song law. A cover checker must inspect
realized pinned note material as well as generator claims.

### Executable objects and policy decomposition

The existing chronological ladder combines orthogonal decisions. A replacement
should keep public wrappers and map them to these executable policies:

1. Pitch path: historical written classifier versus temporal path.
2. Source expression: unchanged, local connective expression, phrase expression,
   or pulse-constrained phrase expression. Pocket internals supply their explicit
   independent source factors, not a new round enum.
3. Occupancy: acoustic-only historical behavior versus authored semantic ownership.
4. Support: independent path, heard identity/spacing, heard path plus source
   support selection. Physics is an explicit input to the heard observer.
5. Lifetime: release envelope or explicit continuation graph.
6. Historical repair: none, support-mass gate, or mass plus sounding-tension gate.
7. Ensemble coupling: independent, coupled R8, surgical legacy.

Require a single validation boundary: source-expression/final-hearing policy
cannot combine with post-hoc surgical/tension repair or coupled R8 order. This
is already enforced only by assertions in expressive/phrase/pocket wrappers
(`functor.rs:174,228,267`). The policy constructor should carry this invariant.

The table must preserve the exact call order and dispatch, not merely names:

| Historical wrapper | Pitch | Expression | Occupancy | Support | Lifetime | Repair |
|---|---|---|---|---|---|---|
| perform | written | none | acoustic | independent | envelope | none |
| perform_temporal | temporal | none | acoustic | independent | envelope | none |
| perform_mass | temporal | none | acoustic | independent | envelope | support mass |
| perform_tension | temporal | none | acoustic | independent | envelope | mass then tension |
| perform_coherent | temporal | none | acoustic | heard identity | envelope | none |
| perform_expressive | temporal | local | acoustic | heard identity | envelope | none |
| perform_phrase_experiment | temporal | local/phrase by factor | authored by factor | heard/path by factor | envelope | none |
| perform_pocket_experiment | temporal | pocket source factors | authored | heard/path | explicit links by factor | none |

The R17 `mono_voice` factor currently also chooses support entry point
(`functor.rs:510-523`). That choice is physically justified only as selecting
which lifetime law the support observer consumes. It should not independently
enable an unrelated treatment.

Compatibility requires careful handling of `PocketOptions::NONE`: it reproduces
R16 by taking the phrase planner fallback, not merely by disabling all policy
fields. This deserves an adapter-equivalence property before reshaping dispatch.

### Concrete duplicate / misleading authorities

* `comp::function_over` (`comp.rs:25`) and `bass::stable_function`
  (`bass.rs:943`) implement exactly the same chord-tone/licensed-extension
  relation. Move this relation to the harmonic context/pitch relation layer.
* `Chord::contains_pc` allocates `pitch_classes()` (`theory.rs:230`), whereas
  `pitch::in_chord` manually loops intervals allocation-free (`pitch.rs:15`).
  `motif::chord_mask` (`motif.rs:792`) builds another allocation. One explicit
  chord pitch mask can serve all three, with exhaustive chord-quality/12-PC
  equivalence tests. Do not merge semantically different guide-tone/color sets.
* `pitch::in_mask` and `motif::has_pc` repeat membership. A tiny pitch-class set
  type earns its place through both duplicate removal and allocation removal.
* `AccentGrid::step_of` says floor, but rounds (`performance.rs:91-100`).
  Correct the contract or use an explicit nearest-step conversion. Do not
  silently change results.
* R17 pocket slots already have exact `(index, subdivision)` before float
  conversion; they delegate transport to a phrase-expression function. Promote
  that primitive out of the expression module, retaining the old wrapper.
* Current drum transport delays odd eighth positions from a sixteenth grid
  (`groove.rs:277-287`); phrase transport recognizes odd exact eighth beats
  (`phrase_expression.rs:777-783`). These can share an exact coordinate transport
  without touching structural straight positions. Historical `GrooveEngine`
  delays odd subdivisions of its selected grid and is not the same law.
* `PocketOptions.support_top_voice` is never read by production realization.
  It is included in options/default/factorial diagnostics but no treatment
  exists. It must be explicitly unavailable in the new profile, while legacy
  option bytes remain an archived experiment adapter.
* `PocketOptions::changes_phrase()` (`pocket.rs:31`) includes `mono_voice`, and
  functor uses it to switch authored bass reservation recovery (`functor.rs:440`).
  This is a genuine source/render coupling candidate, not yet a proven bug:
  isolate physics-only toggling with identical lead/bass source notes and
  occupancy, while auditing heard lifetime differences separately.
* `Score.mono_voice` is redundant as a renderer permission: synthesis validates
  and consumes explicit `voice_continuity` regardless of the bool (`synth.rs:590-623`).
  Sonority accepts either bool or nonempty edges (`sonority.rs:875`). The bool
  still controls source construction and support path dispatch in functor.
  A render contract should use the graph, and preserve the bool only as an
  archived generation option/accessor if needed.
* Raw lifetime physics lives under `sonority`; `voice` imports it, while
  `sonority::audible_voices` imports `voice` again. Move envelope ruler to the
  voice/audibility layer and re-export legacy sonority names to remove this
  ownership cycle. Preserve exact arithmetic and tests.
* Role-to-patch matches repeat across expression, sonority and other observers.
  A `MusicWorld::patch(Role)`-style helper is justified. SFX envelopes must remain
  their own law; they cannot be classified as generic Lead patches.

### Things that are not duplicates to collapse

* `pitch::classify` is generation/local syntax. `TemporalPitchDiagnostics`
  independently measures paths and harmony crossings. Sharing elementary chord
  membership is reasonable; sharing the whole verdict would erase an observer.
* `voice` explicit link truncation and `sonority` envelope tail are two factors
  of one law, not rival estimates. Old raw-note diagnostics need legacy envelope
  entry points because they do not receive the graph.
* `AuthoredOccupancy` and `Hearing` deliberately answer different questions.
  Acoustic vacancy is not semantic relinquishment.
* `motif_similarity` is a diagnostic resemblance measure, not an admissible
  cover identity equivalence relation. Cover needs exact selected coordinates.
* `probes.rs` is already `#[cfg(test)]` at `mod.rs:73-74`, as are multiple other
  probe modules. Moving it out of a production compilation path gains nothing.
* `Score::fingerprint` intentionally omits render lifetime/tempo fields because
  it is the historical event receipt. It cannot be relabeled as full PCM identity;
  introduce canonical event and render-contract identities separately.

### Large module boundaries that earn a split

* `functor.rs` contains 824-1251 SFX realization, register choice, voicing, ownership
  and SfxAudit. This is a genuine independent subsystem with a narrow function
  `add_sfx_and_provenance(score, trace, plan, perf, world)`. Move to `sfx` with
  public compatibility re-exports of SfxAudit/SfxVerdict. The main realizer then
  owns causal scheduling only.
* `diagnostics.rs` contains coherence (27), discourse (211), local realization
  (407), lead outline (670), action complexity/conversation (995), rigidity
  (1688), and harmonic context (1824), then tests from 2036. Distinct observers
  justify submodules with old re-exports, but this is lower priority than causal
  policy and cover reuse; file size alone is insufficient reason.
* `sonority.rs` owns elementary harmony relations, color policy, voice projection,
  envelope physics, vertical evaluation and diagnostic report. Extracting lifetime
  physics removes a real cycle. Separating report formatting alone is low value.
* `comp.rs` contains keys generation (125-1200), support-mass historical gate
  (208-338), and pad support (1203-1647). Keys and pad have different incoming
  hearing edges, so submodules can clarify causal boundaries after wrappers are
  frozen. Keep event/source relation helpers in one shared layer.
* `voicing.rs` separates candidate generation, path optimization (1100), role
  adaptation (1214), and diagnostics (1465 onwards). They form a sensible solver
  stack; do not rewrite solver because the file is large.
* `motif.rs` mixes motif algebra/identity (17-559) and melodic path realization
  (561-1574), an authentic source-object versus realization-morphism boundary.
  Cover can reuse motif algebra without importing line-search concerns.
* `performance.rs` has a single causally ordered planner; stage functions can
  encode inputs after constraints, but indiscriminate splitting obscures order.
* `phrase_expression` generates candidates and commits source choices;
  `expression` supplies physical viability/local fallback. Keep this directional
  distinction, remove only the rhythmic coordinate placement and policy coupling.
* `synth.rs` owns rendering plus explicit production ablations. Its schedule
  construction is the natural RenderContract validation boundary; tests from
  1080 onward need not be compiled for normal library code.

### Score / evidence / render boundary

Score currently holds event data (notes/drums/sfx/chords/sections/time), render
physics (mono flag and continuity graph), and performance evidence (repair logs,
hearings, occupancy, phrase plans, support reports). A compatibility-preserving
first step is borrowed `EventView`, `PerformanceEvidence` and `RenderContract`
views rather than immediately moving public fields and breaking consumers.
Canonical identity can traverse those views with distinct schemas. Future code
should consume the narrowest view it needs. A validated lifetime view also
provides one cached source-note mapping rather than reconstructing it repeatedly.

### Cover implications

1. Coherence anchors exist, but not every anchor already has canonical hard
   coordinates. Groove and BassFigure in particular live partly in performance.
2. Non-backbone harmony is selected in `PerformancePlan`, so extracting only
   `SongMap.harmonic` silently loses the tune's realized harmonic progression.
3. `PerformancePlan::from_song` uses `song.seed` for every performance random
   choice (`performance.rs:262`); there is no independent performance seed in
   `PerformanceOptions`. Add a seed-aware planner entry preserving the old one
   before claiming a same-song alternate-seed fiber.
4. Source `SemanticTrace` also generates SFX (`functor.rs:824`). Retaining it in
   a CoverMap would leak excluded free information. A cover must construct its
   own target-only performance context from retained invariants.
5. Source `SongMap` contains full arrangement, discourse, timeline and optional
   meaning state. Storing the whole SongMap in CoverMap is not a safe shortcut
   when profiles can free any of those axes.
6. Pinning a plan claim is not enough: existing SongMap conformance accepts
   material-stamped notes without rechecking their realized pitches. Cover
   conformance must reproject source notes under explicit relative relations.

### Performance / test work found

Observed avoidable allocation: Chord membership constructs vectors in hot paths.
Observed repeated work: heard pad path trials repeatedly clone all band notes,
re-realize the pad, and measure full IdentityDiagnostics (`comp.rs:1451-1510`).
Optimize only after preserving selected path equivalence; cache immutable band
projection and recompute trial pad contribution, not the final verdict.
Observed O(V*N) source matching in `sonority::audible_voices` (837-908), repeated
by multiple diagnostic passes. A projection retaining source event indices would
eliminate this reconstruction and ambiguity.
Observed repeated full-score cloning in historical tension repair witness closure
(`functor.rs:574-589`); quarantine as expensive historical arm instead of changing
accepted current physics.

Test classes should distinguish cheap algebra/policy/property checks, frozen event
characterization, PCM/listening renders, historical factorials, known regression
sweeps, and fresh declared holdout. `cfg(test)` is already used, so the issue is
which tests run by default and whether repeated fixtures can share immutable data.

### Claim ledger

* Observed: the authority graph and duplicate implementations listed above, by
  direct source inspection at mission-writing baseline.
* Disclosed: `support_top_voice` has no production read; the alleged enabled
  treatment is semantically empty at this source.
* Conjectured: source/lifetime coupling can alter occupancy under physics-only
  toggles; a mutation control must determine whether the suspect path matters.
* Conjectured: typed metric transport, validated realization policy and canonical
  lifetime views reduce genuine authorities while preserving accepted behavior.
* UNVERIFIED: behavior equivalence of any proposed extraction, new CoverMap law,
  fresh holdout, PCM equality after changes. No such tests were run in this audit.

## Before: coordinate and lifetime evidence


Read-only audit at `207f0ab092e3d0a274998d84a245e7b78d096f97`, branch `feat/v0.4-humanmusic-audio`, origin `git@github.com:femboy2112/libgibson.git`; clean when checked. No generation, rendering, fresh holdout, production edit, or tests executed. Existing committed receipts were inspected and mechanically compared. Source/receipt observations share production provenance; they are not new human evidence.

### Authority graph and concrete duplicate authorities

| Responsibility | Current authority / route | Classification |
|---|---|---|
| Metric accent field | `performance::AccentGrid`, `STEPS=16`, `STEP_BEATS=.25`; `beat_of` / `step_of` | Single field authority; legacy float conversion boundary |
| World subdivision | `MusicWorld.subdiv`, documented **hat subdivisions per beat** | Not a universal legal event grid |
| Language subdivision | `MusicalLanguage.surface_subdivision`; `performance::build_accent` filters cells by >=4 | Surface choice, not song coordinate identity |
| Historical drum swing | `GrooveBuilder::swung` in `groove.rs:172` | Legacy subdivision-relative swing law |
| Current drum swing | local `swung` closure in `groove.rs:277` | Duplicate eighth-only transport authority |
| Phrase swing | `phrase_expression::groove_position` at 777 | Duplicate eighth-only transport authority |
| Pulse candidates | `pocket::slots` at 67 | Single R17 candidate enumerator, but combines metric identity and performed coordinate in one record |
| Source phrase intent | `AuthoredOccupancy::from_lead/from_bass` | Semantic authority distinct from acoustic vacancy |
| Source expression | `phrase_expression::realize_impl`, `expression` admission, `pocket` candidate/fallback | Chronological branch presence still changes laws |
| Explicit continuity | `voice::VoiceEventId`, `VoiceContinuation` | Core source-owned identity/edge authority |
| Envelope endpoint | `sonority::audible_end_at/release_tail_secs` | Core physics currently housed in observer module |
| Linked endpoint | `voice::effective_audible_end_at` | Canonical minimum of envelope end and linked destination + choke |
| Historical heard windows | `tension::heard_windows`, `mass::TemporalMass::of_notes` | Duplicate same-role masking implementation |
| Current heard windows | `tension::heard_windows_score`, `mass::TemporalMass::of_score` | Duplicate iteration/dispatch over canonical endpoint |
| Identity | `IdentityDiagnostics::measure_score` -> `heard_windows_score` | Uses current physics correctly |
| Tension | score path -> `heard_windows_score` and explicit links | Uses current physics correctly; historical naked-note API retained |
| Sonority | `audible_voices` projects then O(n²) rematches source notes | Correct current endpoint but avoidable identity reconstruction |
| Phrase observations | `expression::observe_with_voice_contract`, `phrase_diagnostics::audible_end_fn` | Current endpoint consumers |
| Support voicing | trial Score -> voicing/identity observers | Current endpoint consumers; pad tail helper legitimately envelope-only since links forbid Pad |
| PCM | `HumanMusicSynth::with_production`, NoteEvent IDs, continuation destination schedules predecessor choke | Explicit-edge authority; ignores `mono_voice` flag itself |

`AccentGrid::step_of` is documented “floor” but implemented with nearest-step `.round()`. Preserve actual behavior in a compatibility accessor; a new typed coordinate API should name projection explicitly rather than perpetuate the false doc.

### Rhythm finding and safe design

**Observed:** VAPOR95 has `subdiv=2`, swing=.16; AccentGrid remains sixteenths. `pocket::slots` starts with world subdivision and only adds triplets; surface subdivision 4 does not add quarter-beat candidates. The old all-onset witness consequently conflates three things: authored metric events, groove-transported events, and one candidate grid.

From frozen `fresh-failures.json`: all 38 off-lattice first failures are Keys. Their onsets are .5-based (32 cases: 16 at .5, 12 at 16.5, 4 at 1.5) or .25-based (6 cases: 4 at 16.25, 2 at 19.25). Eighth authored onsets are unswung; quarter onsets additionally lie outside the world hat grid. These are distinct coordinate defects; neither licenses moving structural events merely to pass the witness.

Minimal compatibility-safe extraction:

1. `MetricPosition { ticks: i64, subdivision: NonZeroU32 }`, canonicalized for semantic equality; `MetricGrid` enumerates rational positions.
2. `GrooveTransport` declares an actual transport law: historical subdivision swing versus current eighth swing. Existing current drum and phrase helpers delegate to one law, preserving operation order and float output bits.
3. `PerformedPosition` retains its exact metric source and transport identity, converting to f64 only at legacy/render boundaries.
4. Projection back to metric is exact when provenance is carried. A nearest-float lookup is an observer with residual, never the identity inverse. Current eighth-only perturbation on arbitrary real beats is not an automatically lawful global inverse.
5. Event-grid membership and world hat-grid preference remain separate. Keep historical R17 candidate-domain construction exactly in its adapter; explicit modern grid policy may admit sixteenths without silently changing the frozen arm.

Falsifiers: rational equivalent positions compare equal; metric -> transport -> metric commutes on positive/negative/bar-boundary ticks; swing affects only declared positions; straight transport is exact identity; preserve current float bits over the existing input domain; reject undefined/inexact inverse instead of rounding it into success. A mixed authored Keys + swung drums fixture must report their declared coordinate roles without moving either.

### Voice physics finding and safe design

The key law already exists in `voice.rs:78`: minimum ordinary envelope endpoint and every explicit outgoing destination onset + 15 ms. `VoiceContinuation::new` restricts to forward same-role Lead/Bass; `continuity_violations` rejects absent/ambiguous endpoints and duplicate predecessors. This is suitable core structure, not Round-specific mathematics.

`mono_voice` is a misleading compound flag. With no links it does not alter PCM (existing `mono_factor_without_explicit_links_does_not_choke_by_role` test). It does select *unmasked* observation, replacing historical inferred same-role tail clipping in mass/tension/identity. It also affects source phrase and pad generation through dispatch. These are three separate concepts.

Promote `LifetimePolicy::{LegacyRoleMasking, ExplicitContinuity}` or equivalent and a `HeardWindows` immutable intermediate in `voice`/audibility core. Preserve compatibility wrappers:

- naked-note `heard_windows` and historical `TemporalMass::of_notes` select the legacy rule;
- score-aware methods select the current declared render/observation policy;
- both current observers consume the same endpoints;
- keep envelope approximation explicitly approximate (not sample-exact DSP proof);
- retain source identity through `Voice` projection rather than searching again by onset/pitch/gate/function/tag.

The mass legacy masking loop is O(n²); tension already sorts role onset arrays and uses partition points. They use the same EPS=1e-6 and same selection predicates, so a shared historical implementation can remove duplication with exact endpoint equality tests on unsorted/duplicate/simultaneous data.

Do not blindly move all Score fields now. Add borrowed views (`events`, evidence, render contract) first if ownership gains are real. The continuity graph is a load-bearing render input; expression decisions, hearings, occupancy and repair counts are evidence. Current Score fingerprint deliberately excludes mono flag and continuity, so render identity must bind them elsewhere.

Potential false alarm eliminated: `functor::sfx_octave` uses raw envelope endpoints but only executes under CoupledR8; R17 requires Independent coupling. This is a legacy boundary, not an observed R17 lifetime bypass. `voicing_diagnostics` direct envelope calls measure Pad only, which cannot have current continuity links; also not a current mismatch.

Falsifiers: unrelated same-role onset does not affect direct lifetime; one explicit edge affects only its predecessor; gate clipping/arrangement stamps do not change event IDs; ambiguous duplicate source IDs fail validation; score with identical events and no links has identical PCM regardless legacy flag; canonical observers use same endpoint array; historical endpoints remain exact.

### Option bugs, with receipt evidence

### Phantom support

**Disclosed by complete symbol-use search:** `support_top_voice` is never read in production. It appears only in PocketOptions construction/default and lab/test factorial labels. `Default=true` claims a treatment that does not exist. Keep the field only in the historical experiment adapter, document it as a no-op factor, and omit it from a new production profile. Do not change historical factor serialization. A named experimental capability may return unavailable instead of pretending enabled.

### Render/source coupling

`PocketOptions::changes_phrase` includes `mono_voice`. It gates `realize_pocket -> realize_impl(Some(...))` and authored bass-intent recovery in `functor.rs:432`.

Presence of `Some(pocket)` does more than supply lifetime physics:

- `phrase_expression.rs:394` disables broad R16 stable precursor capture unless stable_precursors=true;
- :504 changes stable movement admissibility;
- :607 dispatches to the pocket fallback instead of historical `expression::realize` (which may substitute pitch);
- candidate admission observes explicit links;
- bass occupancy uses a regenerated authored source rather than reverse mapping result decisions.

**Observed from committed factorial receipts, no rerun:** BLACK `f0000 -> f0100` changes only mono_voice but changes 8 Lead records, removes 9/adds 12 Bass records, and changes occupancy, phrase plans and expression decisions. Pad/Keys notes stay equal. SWISS notes/occupancy happen to stay equal; continuity differs. So a predicate rename alone cannot establish source independence.

Small sound-preserving design: extract source-expression policy, lifetime admission policy and render continuity policy as separate typed axes; the R17 adapter explicitly supplies all currently bundled choices. In the new public profile, changing render lifetime alone leaves source policy and occupancy untouched. If physical admission is intentionally coupled, it must be an explicit separate source policy, with its causal dependence documented/tested. The accepted all-on BLACK arm must still map to exactly its old tuple.

### Known R17 failure classification (counts preserved)

Frozen result: 120 executed, 61 complete invariant passes, 59 first-assertion failures. Later assertions are unestablished for failed cases.

- **14 accepted-output pitch differences:** 8 Keys, 6 Lead; 10 SWISS, 4 BLACK. Six lead records at 35.75 preserve authored F4 where R15/R16 substituted G4 at 35.5. Existing bounded source replay identifies Keys change at 67.25 as changed eligible comp slots -> changed stab ordinal -> inclusion of an existing third voice, not repitching the underlying voicing. Other Keys mechanisms are not individually proved by this audit. The old comparator remains failed. New identity law must distinguish authored song contour, structural obligations, legitimate free support voicing, and historical repair output.
- **38 VAPOR lattice failures:** exact domain breakdown above. Preserve as failed historical witness; introduce a new explicitly typed witness separately. Do not rename the old result a pass.
- **5 temporal claims:** all BLACK demo MeaningDirected, tempo76, seeds 3/7/19/43/101. Seed3 is already source-traced: structural D2@62 gate1.35 crosses G->F#7 at63, authored C2@63.5 fails its held-resolution relation whereas historical C#2 substitution supplied support. This is an authored-path defect masked by old repair. Do not reclassify away. General source repair requires a constrained earlier choice or leave red; source identity cannot be silently changed to match old repair.
- **2 held identity flips:** same seed43 deflected StablePropulsion VAPOR case at tempo71/59. Fmaj6 loses root and A-minor rival holds 114.25–115.415640562 (0.985s) / 114.25–115.370743562 (1.140s). Threshold .5s remains fixed. Root cause not yet isolated. A bounded crossed probe should hold notes constant and switch legacy/explicit heard-window policy, then hold physics fixed and compare R16/R17 notes, identifying observation-model exposure versus genuine changed source. Either can be a real audible defect; neither permits threshold tuning.

### Suggested implementation dependency order

1. Freeze all accepted receipts and exact rendered PCM first.
2. Extract rhythm transport with bit-preserving adapters and indexed coordinate laws.
3. Extract shared lifetime windows with endpoint characterization and historical policy named.
4. Replace compound chronological/pocket option presence with explicit law tuple; map old wrappers to old tuple.
5. Test pure render-policy mutation source invariance on modern API; keep historical factorial behavior as characterization.
6. Re-run known failures only as preserved regression/classification evidence.
7. New CoverMap uses canonical metric axes and explicit continuation ownership, never the mixed legacy onset coordinate as identity.

No fresh holdout or broad tests were run here. No alleged universal musical fix is certified by this read-only audit.

## Before: cover boundary and proposed lift


Observed repository: feat/v0.4-humanmusic-audio, clean at 207f0ab092e3d0a274998d84a245e7b78d096f97; origin git@github.com:femboy2112/libgibson.git. No production edits, renders, tests or commits in this reconnaissance.

### The critical boundary

CoverMap must be a value containing selected canonical identities, not a wrapper around SongMap or Composition. Existing SongMap contains trace, timeline, seed, CompositionPlan (including energy/register/density intent), meaning and phenomenal targets. Copying those wholesale into CoverMap would retain coordinates that the requested cover profile claims are free. Source metadata belongs in a separate extraction receipt, never a generator input.

Existing perform(song, world, opts) is already a useful lift but song.seed controls both composition and performance. A cover lift should build a NEW SongMap from only CoverMap plus target inputs; set its seed from target seed. No borrow, closure, Arc or identifier-based lookup of the old Composition should cross that function boundary. New SongMap is legitimate constrained composition output; retained old SongMap is leakage.

### Current integration seams verified by code

- `song.rs:133`: SongMap: trace/timeline/plan/seed/frame/thematic/harmonic/meaning/phenomenal.
- `contract.rs:111`: grammar-derived anchors. HookArc = Motif/HarmonicContour/Groove; LoopEvolution = Groove/HarmonicLoop/Orchestration; RiffDrive = Riff/Groove/BassFigure; WorldSwitch = Motif/BassFigure; DeflectedLift and PropulsiveReturn = Motif/HarmonicLoop/Groove.
- `plan.rs:680`: CompositionPlan::build_with_contract_for_beats builds form -> discourse -> arrangement -> optional backbone. Introduce typed structural constraints BEFORE each derived stage, not an override of finished plan fields afterward. FormGraph/Phrase embed free intent magnitudes; define compact FormIdentity instead of retaining them wholesale.
- `performance.rs:259`: PerformancePlan::from_song owns staged generation: (1) harmony, (2) actions and stage admissions, (3) harmonic actions, (4) shared accent, (5) interactions/statements, then ensemble/complexity. This is where `PerformanceConstraints` should enter. Historical constructor calls it with no constraints and must remain exact.
- `performance.rs:978`: build_accent_grid chooses four gesture cells before applying per-bar variations/actions. Add optional canonical cell constraints here. Retaining an entire AccentGrid overpins energy/weight/action details and does not prove realized groove identity.
- `interaction.rs:385`: lead statements start from ThemeSite, then entry selection, possible Fragment, material creation, call opportunities and responses. A pinned motif/riff should enter at ThemeSite/material input; explicit pinned schedule enters before entry selection. Do not rewrite final statements/materials afterward because references and action admissions already depend on them.
- `motif.rs:1321`: line DP candidates and repair fallback own actual melody pitch. Add pin constraints BEFORE candidate enumeration, with a typed Infeasible result for an empty lawful domain; do not soften pin to cost and do not let nearest-chord fallback change it.
- `bass.rs:239`: source bass chooses its own foundation/pedal/walk/counter/quote events. BassFigure is not implemented by preserving MotifBank::bass_cell alone. Pinned bass figure must enter the source event planner before phrase expression and before downstream drums/pad hear it.
- `groove.rs:254`: actual drums own kick/backbeat/hat decisions, read authored bass reservations and final lead. Groove is not AccentGrid::cells alone: phrase-engine grammars often do not consume gesture cells. Add canonical mandatory strokes/pattern constraints at source selection, before swing/hit emission; keep free ghosts/fills/velocities/timbres downstream.
- `ensemble.rs:Stage`: stage is one orchestration authority, seeded from ArrangementPlan then actions admitted. Pin role topology before action admission, and reject/recast admissions that would violate it. Store role topology without gains/patches.

### Important falsifiers in current architecture

1. `SongMapConformance` at song.rs:455-477 verifies planned statement motif equality, then merely requires ANY lead note carrying its material. Mutating actual melody while retaining material can pass. CoverConformance must reconstruct selected identities from actual source/score events and canonical event links. Planned equality is a separate receipt, not cover sound proof.
2. `Motif::identity` at motif.rs:213 normalizes durations by total. This quotients augmentation as well as tempo; default cover metric identity should preserve rational beat durations, not accept arbitrary rhythmic stretching merely because a tempo change is allowed.
3. `realize_line` allows internal rests, pitches selected by weighted contour fidelity, and final nearest-chord snapping. Existing motifs are soft melodic anchors. External exact reference melody requires actual hard pitch-domain constraints or explicit failure.
4. `ChartRoot::Degree` changes interval/quality under target mode. That is lawful for existing SongMap re-moding, but is NOT automatically a strict transposed real-song cover. The CoverSpec must declare degree-relative versus semitone-relative relation. For an external sheet, explicit relative chromatic pitch/harmony is the least surprising safe default; key can move, mode must not silently alter the source tune.
5. Exact realized Groove or BassFigure cannot reliably be recovered by rounding performed beats: authored/transported coordinates must be explicit. Source-event IDs + canonical coordinates are the clean extraction seam. If an external fixture lacks required canonical data, fail extraction with MissingAxis rather than inventing it.

### Minimal objects proposed

- `CoverSpec`: set of CoherenceAnchor axes plus explicit relations/options. `from_contract` starts from declared anchors and includes form/closure scaffold as explicitly documented default pins. Caller overrides may free those. Canonicalize axis order/duplicates.
- `CoverMap`: schema ID, selected axis relation descriptors, selected axis values. No source seed, World, Language, Composition, complete Score, unselected motifs/chords/actions, semantic trace or fingerprint lookup. `total_beats`/meter must be declared as structural domain coordinates; do not smuggle them while claiming to freeze nothing.
- `FormIdentity`: phrase/family boundaries and exact length; reference/return/closure landmarks only if selected. Strip intent energy/register/density, arrangement gains and runtime obligation action IDs.
- `RelativeMotif`: canonical metric slots and relative pitch coordinates, including literal rests where pinned. Stable local material IDs are alpha-renamed, not source object identity. Separate motif rhythm from sounding gates.
- `HarmonicIdentity`: canonical root/quality or chart-relation landmarks/spans, relative to declared reference frame; no voicing/register. HarmonicLoop exact root loop + canonical change rhythm, HarmonicContour explicit landmark trajectory. Free extensions distinguished from pinned chord core.
- `GrooveIdentity`: canonical mandatory drum roles/strokes or explicit gesture-cell relation; must name what actual projection tests, cannot call cell-only preservation full realized groove preservation.
- `BassFigureIdentity`: canonical relative contour/rhythm for characteristic source material/sites, not the entire performance's bass MIDI by default.
- `OrchestrationIdentity`: structural role topology, not patch identity or numeric gain.
- `CoverTarget`: world, language/performance options, explicit realization profile, target seed, optional key/tempo override. This is the entire generator context.
- `CoverConformance`: one row per pinned axis: relation, checked objects, Pass/Fail/Missing/Infeasible, localized differences. No weighted score. Read generated actual events for audible axes, plus planned/reference relation checks separately.
- `CoverFreedom`: vector of changed unpinned coordinates (voicings/register/response material/fills/velocities/timbres etc), with before/after canonical values or hashes. Timbre alone is insufficient to reject replay.

### Lift algorithm

1. Validate finite canonical positions, complete required source axes, nonempty melodies, lengths, phrase references, harmonic coverage, and compatibility between pins. No silent default notes.
2. Generate free intent/discourse/envelope inputs from target seed/profile. Construct pinned form/closure values first, then derive the remaining plan. Any dependency on free source energy/timeline is forbidden.
3. Build the new thematic bank from pinned motif/riff coordinates and target-generated unpinned materials. Preserve selected sites, regenerate free response/countermaterial.
4. Choose harmony from candidate domains constrained by pinned harmonic coordinates. Admit harmonic actions only when constraints survive; do this before shared contexts and melody choices.
5. Choose pinned groove cells/strokes and source bass figures before their consumers. Generate unpinned details with existing planners. All expression/occupancy/hearing paths consume final constrained source events.
6. Run ordinary realization under explicit chosen profile. Pins restrict choices, not finished Score patches. Return `Result<Composition, CoverError>` so genuinely incompatible target language/pitch laws do not cause a false success.
7. Extract actual projected identity and run conformance. This is verification, not a retry/repair loop. Generation failure remains failure.

### Required laws and adversaries

- Extraction quotient/noninterference: produce two reference Compositions sharing every selected axis but differing in trace/seed/patch/voicing/velocities/response data/etc. Assert exact CoverMap equality. Destroy/drop both originals. Generate with same target and compare Score event fingerprint, PerformancePlan fingerprint and render hash if promised. A test helper intentionally appending a free source field to target seed must fail the witness.
- Cover law: extract same spec from generated output and compare exact CoverMaps; also check per-axis actual event projection (do not trust copied cover metadata).
- Pinned motif mutation changes actual source event pitch or metric slot while labels untouched -> Motif-specific failure.
- Pinned harmonic landmark changes actual score chord root while plan label intact -> localized harmonic failure.
- Free voicing octave/inversion mutation -> conformance remains passing, ordinary pitch/hearing laws separately checked.
- Legal whole-key transposition -> relative identity unchanged. Tempo -> beat identity unchanged, seconds changed. Swing -> canonical pattern unchanged after declared inverse.
- Freedom: use >=3 targets including same world/new seed, new world, new language/key/tempo. Require at least one non-timbre free coordinate differs, with pinned axes unchanged. Do not assert every axis always changes.
- Freeze all representable axes -> describe remaining dimensions (typically synthesis/sample rate/production unless explicitly pinned). No claim all audio collapses if only the eight structural axes are pinned.
- Freeze almost none -> generator may be lawful but `is_recognizable_cover_profile=false`; retain exact pin list so no implicit song claim.
- Empty/incompatible/missing reference axis -> typed error, never fabricated values.

### Smallest reference-song ingestion boundary

No repository-local `Swing and a Miss`, `Oliver Tree`, MIDI, MusicXML, ABC, lead-sheet or reference-song fixture found by filename and content scans (tracked and hidden excluding .git/target). This is a concrete external-source access gap, NOT grounds to synthesize an imagined transcription. Searches did not inspect arbitrary unrelated maintainer directories.

There is no current MIDI/MusicXML importer or serde dependency. The smallest clean first boundary is `ReferenceSong` as a validated typed Rust representation/build API, plus a simple documented text/TSV format only if a file-based lab needs it. Fields: source provenance receipt outside musical input; explicit meter and beat length; tonic/frame; canonical phrases; melody events with rests/ties, relative or absolute pitch with declared normalization; harmony spans; optional bass/groove/role topology. Required selected fields cannot be omitted. `ReferenceSong::extract(spec) -> Result<CoverMap,...>` shares exactly the same canonical axis constructors as generated Composition extraction. A generic synthetic fixture can test it, explicitly labelled synthetic and never called Oliver Tree.

Do not implement a full MIDI parser only to satisfy this request. An adapter can be added later from a legitimately supplied MIDI or sheet into ReferenceSong. No audio downloading or transcription occurred. Serious Swing and a Miss listening artifacts remain UNVERIFIED until an actual structured source is supplied or identified.

### Claim ledger

Observed: current seams, seed coupling, planned-vs-heard conformance gap, no local structured external reference found.
Disclosed inside type boundary if implemented: a pure CoverMap-only generator cannot read excluded original Composition fields; runtime equality tests still required for accidental serialization/seed leakage.
Conjectured: proposed axis default is sufficient for recognizability; only human blind audition can establish that bearing.
UNVERIFIED: all proposed implementation/property results and real-song listening target; no CoverMap implementation yet exists at inspected HEAD.

## Implemented: canonical identity schema v2

`fingerprint::CanonicalFingerprint` writes a streaming, explicitly tagged binary
encoding through `FingerprintWriter`. `fingerprint/schema.rs` declares field order
and enum discriminants for the transitive SongMap, PerformancePlan and Score event
graph. Exhaustive struct destructuring and enum matching require review when a
field or variant is added. This is not a serializer generated from Debug at runtime.

The stream begins `humanmusic-canonical/v2`. Tags/strings carry u64 byte lengths;
integers are little endian; usize is u64; sequences carry lengths; enums carry
explicit UTF-8 discriminant tags; floats are IEEE bits with signed zero and NaN
payloads normalized. No blanket unordered-map implementation exists. FNV-1a is
still the digest algorithm, not a cryptographic collision-proof identity.

Existing `fingerprint()` formulas remain unchanged; explicit `legacy_fingerprint()`
aliases name historical receipts. Song v2 includes song-defining consequences,
not raw input seed/trace. Performance v2 includes the decisions, not its redundant
legacy song-hash claim. Score v2 is **event identity**, including tempo, meter and
length; render continuity and evidence are separate concerns. Never use an event
digest alone to claim equal PCM. Canonical equality is schema-relative, not musical
quality or recognition.

`audio_canonical_fingerprint` tests Debug presentation independence, primitive
boundaries, float normalization, song-coordinate mutation, free seed invariance,
performance-grid mutation and note/time/provenance mutation. Historical baseline
characterization remains a separate test and WAV comparisons remain mandatory.

## Implemented: realization laws and historical adapters

The central `Contract` chronology is removed. `PerformanceProfile` selects pitch,
source expression, occupancy, support, source evidence and direct lifetime; one
validation boundary rejects incompatible final-hearing/coupling combinations.
`perform_with_profile` and `realize_with_profile` expose it. Existing `perform`
remains the written default; historical functions construct their original exact
law tuples, including mass/tension negative controls.

`PulsePolicy::continuation_admission` is a source candidate law. The independent
`VoiceLifetimePolicy` chooses emitted continuation physics. The historical mono
factor explicitly maps to both. The new profile omits the unavailable support
switch; `PocketOptions` retains the archived no-op for factorial reproduction.
A pure lifetime-toggle test preserves lead/bass source events, phrase plans,
expression decisions and authored occupancy while the edge graph changes.

SFX realization/ownership/audit moved unchanged into `sfx.rs`, with old public
names re-exported by `functor`. This removes a distinct subsystem from the causal
band realizer. The planner/realizer construction boilerplate is shared.

**Observed:** an isolated staged-source checkout passed 12 targeted tests (four
policy, three pocket phase, four voice including small PCM controls, one complete
flagship event/evidence characterization). This does not substitute for the final
full/stem PCM rerender. Logs are under `fixtures/humanmusic-consolidation/verification`.

## Implemented: metric coordinates and audibility ownership

`MetricPosition` is a reduced rational quarter-note coordinate. `GrooveTransport`
produces a `PerformedPosition` carrying its exact source; projecting that source
commutes exactly. This is not an invented inverse of arbitrary floating beats.
The historical sparse eighth-delay law is not globally monotone/bijective over
arbitrary dense grids; new constrained generators must reject collisions/order
reversals. A typed float import admits only exactly representable rational values.

`rhythm::lattice_slots` owns enumeration. `pocket::LatticeSlot/slots` are compatible
adapters; historical world/hat candidate selection remains unchanged. Current
drum and phrase swing delegate to one transport boundary; historical subdivision
swing remains explicitly separate. No structural Keys event moved to improve the
known VAPOR result.

Envelope endpoints, role-to-patch selection and continuation endpoints now live
in `voice`; old sonority/expression/mass entry points re-export that authority.
`HeardWindows` borrows immutable source notes and owns one reusable endpoint vector.
Mass, tension and identity share its lifetime rules. The historical same-role
masking path builds one sorted index instead of a quadratic scan. The canonical
explicit path changes lifetime only through source-owned continuation edges.

Sonority retains original note/SFX indices through projection and sorting instead
of reconstructing identity by floating onset/pitch/gate matches. This exposed and
fixed a universal diagnostic bug: coincident same-pitch SFX of different kinds
could use the first event's envelope. The new falsifier preserves each source's
own envelope. This correction changes a diagnostic on that counterexample; it
changes no accepted flagship event/evidence or synthesis output.

**Observed:** isolated staged-source verification passed 12 integration tests and
three exact-coordinate unit laws. The tests include historical endpoint equality
against the old quadratic definition, immutable-window reuse, explicit-edge
mutation, coincident-SFX identity, and the frozen accepted event/evidence receipts.


## Shared pitch relations and allocation ownership

`PitchClassSet` is the sole normalized 12-bit membership object. Ordered chord
voices remain separate: order and octave matter for voicing but not membership.
`Chord`, motif masks and source membership consume this object. The comp/bass
source classifier now shares the chord-first, literal licensed-palette relation;
temporal classification remains a separate observer with its boundary evidence.
Nearest chord tone selection retains the historical downward tie choice.

**Disclosed:** exhaustive controls cover 18 qualities, all 12 roots/classes and
13 octaves; shared source labels also cover malformed palette values -1 through13.
A sustain-crossing falsifier shows that static membership cannot certify temporal
function. The isolated staged source passed five pitch/coordinate laws and the
accepted R17 event/evidence characterization (six tests). No whole-program speedup
is claimed; removed vector allocations and duplicate scans are structural facts.
`AccentGrid::metric_of` exposes exact metric identity; `step_of` documentation now
correctly says nearest-step projection, including its existing tie/rollover behavior.

Exact downloaded external-source bytes are archived unchanged, including upstream
whitespace in `ode.ly` and the license page; their recorded SHA256 identities take
precedence over normalizing archival text. No production formatting rule changed.


## Future feel seam and decisions awaiting human evidence

The future morphism has source `PocketedPerformance` and target performed timing:
`FeelTransport` must retain canonical metric identity while correlating timing
across a phrase and interacting roles. Its identity element changes nothing.
A future implementation must supply a projection law, bounded groove-domain
checks and falsifiers for structural displacement; independent jitter is not an
implementation of this object. This consolidation adds no feel behavior, random
jitter, arbitrary drift or aesthetic timing thresholds.

The maintainer is away and has authorized reasonable decisions without further
questions. These unresolved decisions are recorded rather than silently answered:

* R17 preservation and cover recognition/interpretation require actual listening.
  No generated test substitutes for those observations.
* The later provisional A-major Swing chart is the working declaration. The earlier
  pasted D/Am/Em/G chart differs in transposition and post-chorus order. Neither
  capo nor missing notes are inferred. Confirmation awaits richer lawful evidence.
* Swing melody/riff, characteristic bass/drums, chord durations, section lengths,
  rests and exact tempo remain unobserved or provisional. A later local MIDI or
  melody-plus-rhythm lead sheet would increase identity information most directly.
* The imported Ode soprano is the explicitly selected melody; the other three
  named source voices remain observations. Generated accompaniment is not claimed
  to reconstruct that SATB arrangement or a historically authoritative harmony.
* The first cover relation pins whole selected role lines, rather than discovering
  a listener's minimal recognizable hook. Whether this is musically too rigid
  requires the paired auditions; changing the quotient later requires new tests.
* Public API promotion and default changes remain outside this mission. The draft
  PR and experimental opt-in boundary remain in force.


## Test architecture and constrained-machine gate

The normal developer loop uses changed-module unit/property tests, the relevant
integration binary, and `audio_consolidation_characterization` when source events
may change. `audio_pocket_voice` adds short PCM controls for continuation changes.
The complete release gate is retained. Never blanket-run ignored tests: they
include consumed holdouts, historical red probes and hardware-dependent tests.

| Class | Examples | Evidence boundary |
|---|---|---|
| Unit / property | rhythm, pitch relations, canonical encoding, cover laws | named executable invariant |
| Characterization | accepted R17 event/evidence corpus | exact events/receipts, not PCM |
| Short render | continuation and block-size controls | bounded direct synthesis |
| Expensive integration | multiworld and paired full PCM tests | full local/CI gate |
| Listening | pocket/cover lab WAVs | bytes are machine evidence; perception awaits ear |
| Historical archival | ignored old round sweeps | known data, not fresh validation |
| Release | package, ABI, cleanroom, MSRV, notices | artifact/consumer boundary |

The normal editing loop selects the affected law, then adds characterization when
ownership can affect the accepted score. Example commands below are prescriptions,
not additional execution receipts:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test --lib audio::human_music::rhythm::tests
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test --test audio_consolidation_characterization --test audio_pocket_voice
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test --lib audio::human_music::cover::tests
```

Choose the relevant commands rather than rerunning all three for every edit.
Event/evidence characterization does not render PCM; the short voice controls
test scheduling and block-size equivalence. Long multiworld render tests and the
48-WAV accepted-corpus comparison remain required at their full-gate boundaries.
Once a final source is frozen, preflight itself runs the ordinary suite; an extra
identical full local rerun is unnecessary unless source changed or a failure needs
discrimination. The dedicated CI PTY and consumer boundaries remain intact.

Never blanket-run `cargo test -- --ignored`. Ignored tests include consumed R16/R17
sweeps, known-red upstream PTY/device probes, expensive archives and the new holdout.
The [fresh configuration and execution protocol](fixtures/humanmusic-consolidation/fresh/README.md)
declare 36 cases: 30 generated sources and six Ode combinations. Its own executable
is ignored and requires a new receipt directory; compilation alone is not contact.
Commit the configuration after architecture freeze, then preserve the first run's
complete per-case failures and panics. New target seeds do not make the already
inspected Ode fixture, shared generator or shared diagnostics independent sources.

Use both `CARGO_BUILD_JOBS=1` and `RUST_TEST_THREADS=1` on the constrained
maintainer machine. This limits concurrency without changing assertions, timeouts
or coverage. Existing line-table-only debug settings already reduce disk cost.
A 120-beat, 71-BPM stereo f32 buffer at 48kHz is approximately 39 MB before
other allocations; paired renders and concurrent tests multiply that cost. This
is an allocation estimate, not a measured speedup claim.

Source changes in this consolidation remove quadratic historical endpoint scans,
fragile rescan/rematch in sonority, and repeated chord-membership allocations.
`HeardWindows` permits several observers to borrow one immutable reconstruction.
We do not introduce a global mutable cache or weaken final-hearing freshness.
Repeated clean-room/package/MSRV checks establish different boundaries and stay.

The actual scripts, rather than a generic command list, define the release gate.
`scripts/release/check-abi.sh` can select an existing release library; explicitly
build release at final source before preflight to prevent a stale ABI receipt.
Nested scripts invoke plain cargo, so set `RUSTUP_TOOLCHAIN=1.98.1` as well as
`STABLE_TOOLCHAIN=+1.98.1`; the active local default is a different nightly.
The installed `+1.85` alias resolves to 1.85.1. An audit version query additionally
installed exact 1.85.0; no project dependency or source changed from that query.

Final local commands include the script's fmt, strict all-target/all-feature
clippy, normal tests, warnings-denied docs, locked MSRV, fresh-resolution MSRV
consumer, package verification, ABI, C/C++/Python/Go/Rust cleanroom, notices and
licenses. CI additionally covers explicit FX/UI example tests and isolated serial
PTY suites. The importer has its own seven Python parser laws and pinned tool
requirements. All-feature/MSRV extras and final rerender receipts are reported
separately; prescribed commands are not claimed as executed.


## Repaired: canonical empty-edge observation

A red-first mutation exposed a real architectural bug in the new public profile:
selecting release envelopes with no continuation graph fell through to historical
same-role masking. An unrelated Pad attack at1.25 shortened an earlier voice from
1.545759609205760 to1.25 beats although synthesis did not choke that voice.
`modern-lifetime-counterexample.json` and the failing test output preserve contact.

Modern profiles now select observation explicitly before support chooses notes.
Only explicit edges shorten canonical direct envelopes; an empty graph means no
choke. Historical wrappers select the archived interpretation. One validation
boundary rejects contradictory modern observation/render combinations. The Score
accessor feeds mass, identity, tension, sonority and source support consistently.
A long-release source-support falsifier distinguishes a real planner repair from
merely changing the final diagnostic: support choices change while lead/keys/bass
remain fixed, and final hearings stay fresh. No synth code changed.

Score is retained as the compatibility container. The explicit observation selector
is evidence policy, not render physics: synthesis consumes notes and continuation
edges, never the old mono flag. Event and graph canonical identities remain separate.
A new RenderContract wrapper would add no authority here and was not introduced.
Modern Debug appends the selected observer; legacy None preserves exact old Debug.
The modern BLACK pocket matches the archived full Score receipt after normalizing
only that new metadata. The isolated snapshot passed21 tests, including short PCM
controls and complete accepted event/evidence characterization; final full/stem WAV
equivalence remains a separately required receipt.


## Known R17 replay after core consolidation

At exact source `44e2d7efabc28e684701240b5466d06a91d541dd`, the unchanged
known120-case sweep executed in18.377 seconds (release, one test thread) and
returned exit101: **61 complete passes,59 first-assertion failures**. Every
passing case label and all59 first-failure reason strings match the original.
The four counts remain **14 pitch,38 lattice,5 temporal,2 held identity**.
[Full classification and raw failures](fixtures/humanmusic-consolidation/known-r17-replay/README.md)
preserve the distinction between authored identity and historical pitch repair,
canonical structural versus performed coordinates, the directly diagnosed
source-function crossing, and the unresolved held F-major6/A-minor object.
The source/binary hashes are recorded; evolving cover work was excluded. This is
known-data regression, not a new holdout or general acceptance claim.


## Removed or unified duplicate authorities

This section consolidates what the "Implemented" sections above record, by commit. Each entry says what that commit's message and the sections above state it did; none is a claim of musical acceptance.

- `21430e1` (`refactor(audio): realize musical laws through explicit profiles`): the central chronological `Contract` dispatch is replaced by validated orthogonal policies in `PerformanceProfile`, with one validation boundary; historical wrappers construct their original law tuples. Source admission (`PulsePolicy::continuation_admission`) is split from render lifetime (`VoiceLifetimePolicy`). SFX realization, ownership and audit move unchanged into `sfx.rs` with `functor` re-exports.
- `055f9ae` (`refactor(audio): centralize metric transport and heard lifetime physics`): `MetricPosition`/`GrooveTransport`/`PerformedPosition` replace duplicate eighth-only swing helpers (current drum and phrase swing delegate to one transport; historical subdivision swing stays separate). Envelope endpoints, role-to-patch selection and continuation endpoints live in `voice`; `HeardWindows` is shared by mass, tension and identity, and the historical same-role masking path uses one sorted index instead of a quadratic scan. Sonority keeps source indices instead of rematching by float, which fixed a coincident-SFX envelope misidentification (falsifier added).
- `e277d27` (`refactor(audio): share exact pitch membership and source relations`): duplicate chord-mask, nearest-tone and source-function logic (comp/bass) are unified on `PitchClassSet` and one chord-first licensed-palette relation. Static membership is deliberately not merged with the temporal classifier.
- `3188c69` (`refactor(audio): preserve explicit pitch units through musical identity`): not a removal of a duplicate; it separates two conflated units by adding `PitchBasis` (scale steps versus semitones) to motif and interaction material, with archived Debug/fingerprint encodings preserved for scale-step objects.
- `1b920f6` (`feat(audio): add explicit canonical v2 identity schemas`): adds tagged canonical encodings beside the legacy Debug/FNV formulas rather than removing them; the legacy formulas remain named `legacy_fingerprint()` receipts. This separates two identity authorities rather than collapsing them.
- Canonical empty-edge observation (see "Repaired" above): the historical same-role masking fallback for an empty continuation graph is no longer selected by modern profiles.
- `1a06575` (`fix(audio): plan no groove debt a pinned cover kit cannot discharge`): removes a planner/realizer split-brain. Discourse planning opened a `GrooveDestabilization` debt from phrase role alone, while a pinned Groove realizer never strips or returns the kit, so the debt could not be discharged and every generated contract-default cover failed admission with `unwitnessed_song_obligations: 1`. There is now one typed authority, `KitMotion { RoleDriven, Pinned }`, decided from the quotient before discourse planning. Old entry points are exact `RoleDriven` wrappers; no `DiscoursePlan` field was added.

## Known unresolved defects

- **Known R17 sweep: 59 first-assertion failures remain red**, classified as 14 pitch, 38 lattice, 5 temporal, 2 held identity (120 executed, 61 complete passes). Counts unchanged; see the [replay classification](fixtures/humanmusic-consolidation/known-r17-replay/README.md).
- **`SongMap::composer()` self-report ambiguity.** It infers `StablePropulsion` from the PropulsiveReturn phenomenal target, so a StructuralR9 or MeaningDirected song on the PropulsiveReturn grammar reports the wrong composer. Recorded in the [holdout pre-freeze review](fixtures/humanmusic-consolidation/fresh/README.md); the holdout harness treats the declared composer as authority. Not fixed in the library.
- **Generated contract-default pinned-harmony refusal in SWISS_SIGNAL** (`Invalid("pinned harmony outside target vocabulary")`, Observed at `3216021`). Lawful and explicit; whether the chord-quality relation should admit a quality-family relation is a maintainer decision.
- **Ode `transposed_faster`**: 3 temporal-function claims (Observed at `04e81fc`, preserved failure).
- **Generated VAPOR95**: 1 temporal-function claim (Observed at `04e81fc`).
- **`PocketOptions::default()`** still spells archived `support_top_voice: true`. It is inert (never read by production realization) and documented as such.
- **f64 beat migration is incremental**: exact rational coordinates exist for canonical identity and transport, but legacy float beats remain at many boundaries.

## Session continuation (post-Codex)

Facts recorded after Codex's last commit (`cee745b`):

- `064de33` `test(audio): retain generated-cover SongMap admission diagnosis`: reporting-only sidecar; all three generated admission failures are exactly `unwitnessed_song_obligations: 1`; main report byte-identical to the `04e81fc` corpus; no WAV replaced. Its mechanism was Conjectured there.
- `1a06575` `fix(audio): plan no groove debt a pinned cover kit cannot discharge`: the `KitMotion` repair described above and in the [cover guide](HUMAN_MUSIC_COVER.md#pinned-groove-and-discourse-obligations). The conjectured hat-arrival repair from the diagnosis was rejected (it would fabricate a return from a departure that never happened) as was a post-hoc ledger filter. Observed after repair: lib cover tests 19/19, discourse/song/plan lib tests 31/31, `audio_consolidation_characterization` 1/1, `audio_realization_policy` 8/8, `audio_canonical_fingerprint` 5/5, strict clippy `--all-targets --all-features` exit 0, fmt clean. Mutation check: forcing `RoleDriven` turns the repro red.
- `3216021` `test(audio): isolate cover laws one variable at a time`: `tests/audio_cover_laws.rs`, 5/5 Observed (tempo-only, transposition-only, language-only, contract-default noninterference with three positive controls, per-group freedom sweep). Observed limit: SWISS_SIGNAL refuses the generated HookArc contract-default map.
- `6ac812f` `test(audio): declare consolidated holdout v1 before first contact`: configuration and harness committed before any case executed; see the [fresh README](fixtures/humanmusic-consolidation/fresh/README.md), including its "Pre-freeze harness review".

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
## Consolidated holdout v1 results

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

<!-- FINAL-GATE -->
## Final gate

**Observed at `91c38206a38bd49cfb1669f15ac97515de393874`** (clean tree), after the rustdoc fix `91c3820`:

- Local `scripts/release/preflight.sh` (after `cargo +1.98.1 build --release`; `STABLE_TOOLCHAIN=+1.98.1`,
  `MSRV_TOOLCHAIN=+1.85`, `CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=2`): **13/13 PASS** — whitespace,
  version/ABI consistency, fmt, clippy `--all-targets --all-features -D warnings`, `cargo test`, rustdoc
  `-D warnings`, MSRV 1.85 locked lib check, MSRV declared-range consumer, `cargo package`, ABI v1 symbol
  baseline, clean-room C/C++/Python/Go/Rust consumers, third-party notices, license files.
- GitHub **CI: success** and **Release Preflight: success** at the same SHA.
- The previous local run at `61ba2aa` failed only rustdoc (`KitMotion`'s public docs linked the private
  `strips_groove`); that is the defect `91c3820` fixed. It is preserved here rather than hidden.
- PCM: the accepted R17 pocket is byte-identical at `8a0b0cd` ([receipt](fixtures/humanmusic-consolidation/pocket-freeze-final/README.md));
  the known R17 sweep is unchanged at `8a0b0cd` ([receipt](fixtures/humanmusic-consolidation/known-r17-replay-final/README.md)).
  No source changed between those receipts and `91c3820` except the comment-only rustdoc fix.

Later commits touching only Markdown do not enter the build.
<!-- /FINAL-GATE -->

## Hardening round: holdout-v1 dispositions

Holdout v1 stays immutable (`fresh/`); these repairs were each preceded by a committed falsifier on
fresh seeds outside its rows (`tests/audio_source_contracts.rs`, `tests/audio_cover_contracts.rs`) and are
judged by the separately declared holdout v2.

| Family | Mechanism (read in code, reproduced red) | Disposition |
| --- | --- | --- |
| U1 partial final bar | `bass::realize` gated its last onset to the bar line, past the requested end; the reservation read from that line outlived the clipped note | **Repaired** at the one authority (the bass planner bounds its line to the piece). `occupancy::violations` unchanged; a mutation guard proves it still rejects `total + 1e-3`. |
| U2 Groove never sounded | every Intro/Coda-only form seated the kit Silent; the coverage guard covered pitched voices only | **Repaired**: a Groove-declaring contract seats the kit in its highest-energy phrase holding a full bar. `AnchorReport` separates `StructurallyInapplicable` (no full bar) from `DeclaredButMissing` (a named violation); `CoverSpec::established` pins what a source really establishes. 8 of the 11 v1 `MissingAxis(Groove)` rows were explicit Groove requests (spec `all`/`riff-bass`) on drumless sources: lawful refusals. |
| U2 theme site never stated | theme sites were planned in phrases shorter than the statement | **Repaired**: `ThemeSite::statable`; unstatable sites are never planned (all three site builders). |
| U3 action receipts | the planner admits verbs no realizer performs (initiator silent in its window, one player on an ensemble hit, windows at the end); POCKET and WRITTEN identical | **Repaired for the new law, characterized for the historical arm**: `ActionAdmission::Rehearsed` strikes unperformed verbs before the take (recorded, never forged); historical `Planned` profiles stay byte-exact and keep the defect. |
| U3 song obligations | debts settled by discourse role alone, before any action exists | **Repaired for groove debts** (the ordinary settlement planner schedules a real drum re-entry, admitted onto a Silent seat for its window, as the cover path already did). **Remaining, named**: cadence/departure debts settled where no home chord arrives, planned Resolves no bass/keys attack performs, Simple-language motif questions with no answer. |
| G02/G22 temporal | a keys hold (a voicing chosen for its harmony) rang up to half a beat into a nonmember harmony | **Repaired**: nonmember hold voices lift off at the change (the coupled path's rule); member voices sustain the hold. |
| G17 / G23 / G14,G24 | riff lane re-derived; pinned strokes dropped under a Silent pinned seat; a constrained cover claimed an unplayed chart | **Repaired** (see `HUMAN_MUSIC_COVER.md`). |
| G18, G28 | extraction timing / seat-quotient refusals | **Not repaired**: explicit extraction refusals (G28 is the G23 quotient limit). |

The accepted R17 pocket (48/48 WAV, 757/757 receipts) and all 16 cover listening WAVs stayed
byte-identical after every source commit of this round (verify-pocket-freeze pass at each gate).

## Hardening round: one percussion surface

The drummer's producers (pocket, accents, ghosts, hats, figures, unison, answers) now offer typed
candidates to one arbitration boundary (`percussion.rs`, `groove::realize_drums_arbitrated`) under
`PercussionPolicy::Arbitrated(DrumRestraint)`: required strokes (pocket anchors, time-line hats, stamped
action witnesses, a fill's first stroke and landing, pinned cover strokes) always sound; optional ones
compete per bar for an allowance set by an ordered decision (the floor > a phrase-end moment > the band
already speaking > spoke last bar > support), shifted by the restraint; a rate guard keeps pull-back /
acceleration witnesses. `PerformanceProfile::BAND` (POCKET + rehearsed admission + Balanced) is the new
general profile; POCKET keeps the historical drummer byte for byte. The feel seam above is now a typed
identity (`rhythm::FeelTransport`), wired nowhere.
