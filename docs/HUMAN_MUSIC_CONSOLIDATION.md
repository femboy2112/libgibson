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
