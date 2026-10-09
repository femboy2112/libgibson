# HumanMusic — the MeatsackProjection boundary (C137 → acoustic)

**Status: DESIGN / HANDOFF CONTRACT. Nothing under this name is implemented in `src/`.**
This document defines the boundary between the Rick-C137 symbolic/performed layer and the
physical/acoustic realization layer, from the C137 side. It specifies the *handoff* — what C137
owns, what it hands across, and what the projection may never alter — so that a physical layer can
be built against a fixed, testable contract. It does **not** design the physical layer's internals.

## 0. Relationship to the research branch (read this first)

The name **MeatsackProjection** already exists as a design on the branch
`origin/research/humanmusic-meatsack-projection` (docs-only, marked *"RESEARCH DESIGN ONLY. Not
implemented, not merged."*). That design scopes the projection **narrower** than the whole span to
PCM: it defines `P_K : G ⇀ E`, a partial map from intended *gestures* to physical *excitation and
control events*, parameterized by a declared `PerformerRig` / `InstrumentRig` (effectors, strings,
limbs, pedals) — with the acoustic synth treated as a downstream, out-of-first-round concern. Its
standing law: *"The musical plan says why; the performer rig determines how."*

This document is deliberately scoped as the **outer handoff contract**: the cut between an
accepted C137 performance and *any* acoustic realization, whether that realization is the current
synth or the research branch's gesture/rig layer followed by a synth. **The research branch's
`P_K` is the natural first sub-stage inside this boundary, not a competitor to it.**

**Open scope decision for the maintainer** (do not resolve unilaterally): whether this document
(a) stands as the umbrella boundary with `P_K` as its first internal stage, (b) is folded into the
research branch as its outer contract, or (c) the acoustic-only half is given a distinct name so
"MeatsackProjection" stays the gesture/rig layer. Until decided, this file uses
**MeatsackProjection** for the whole symbolic→PCM span and flags every place the two scopes meet.

> Terminology hazard: the word *projection* is overloaded in this repo. `src/audio/human_music/`
> `projection.rs` is **"Identity projections"** — a symbolic predicate (`IdentityMaterial::carries`)
> over `Score` events, unrelated to acoustics. Every realizer is also called "a projection of the
> PerformancePlan." Always write **MeatsackProjection** in full; never bare "projection" for this
> boundary.

## 1. The pipeline and the cut

```text
  SemanticTrace ──F──▶ MeaningPlan ◀──μ── SongMap ──π──▶ PerformancePlan ──realize──▶ Score
                                                                                        │
                                                        ┌───────────────────────────────┘
                                                        ▼
                                              [ perform_checked gate ]
                                                        │  (PerformanceReceipt.passes())
                                                        ▼
                                             ★ AcceptedPerformance ★   ← THE CUT
                                                        │
                                                        ▼
                                              MeatsackProjection  M : AcceptedPerformance → AcousticRealization
                                                        │   (optionally: P_K gesture/rig stage, then synth)
                                                        ▼
                                                AcousticRealization (PCM + receipts)
```

Everything left of the cut is C137 and already exists. The cut itself exists today only as an
unnamed seam: `HumanMusicSynth::new(&Score, &MusicWorld, SampleRate)` followed by
`OfflineRenderer::render(&mut synth, frames)`. MeatsackProjection is the act of **naming,
constraining, and witnessing** that seam — not of inventing a new renderer.

## 2. `AcceptedPerformance` — the input (what crosses the cut)

The only genuinely *accepted* object in the tree is what `perform_checked` returns:
`Composition { score: Score, song: SongMap, perf: PerformancePlan }` (`functor.rs:41`, produced at
`functor.rs:349`), gated by `PerformanceReceipt::measure_under(...).passes()` (`receipt.rs:52`).
A `Composition` can also come from the *unchecked* `perform`, so **`AcceptedPerformance` should be a
newtype (or a documented constructor) wrapping a `Composition` whose `PerformanceReceipt.passes()`**
— the projection must never run on an unaccepted performance.

- The synth today consumes **only `Score`** (`synth.rs:600`): `notes`, `drums`, `sfx`,
  `voice_continuity`, `tempo_bpm`, `beats_per_bar`, `total_beats`, `stroke_origins`. `Note`
  (`score.rs:274`) carries `start_beat`, `dur_beats`, `pitch`, `velocity`, `role`, `prov`,
  `function`.
- The **narrative is NOT in `Score`** — it lives at `PerformancePlan.narrative`
  (`performance.rs:335`), i.e. only in `perf`. A projection that wants narrative-aware instrument
  handoff (e.g. to honor a withheld lead or a named carrier at the acoustic stage) must take the
  whole `Composition`, not just `&Score`. A projection that needs only timing and dynamics can take
  `&Score` + `&MusicWorld` + `SampleRate` — which is exactly the current synth signature.

## 3. `AcousticRealization` — the output

**No such type exists.** The nearest is `RenderResult` (`render.rs:145`: `audio: StereoBlock`, `sr`,
`reports`, `peak`, `rms`, `max_active_voices`, `had_nonfinite`). A proper `AcousticRealization`
would wrap:

- the `RenderResult` (the PCM),
- the per-role `BusLevels` (`synth.rs:356`, a pre-master RMS/peak meter — observer-independent),
- the `ProductionControl` / `StemMask` actually used (`synth.rs:412`, `:230`),
- a **realization fingerprint** of the world + patches + control path (nothing fingerprints PCM
  realization today — this is new and needed for byte-exact regression of the acoustic layer).

## 4. The nondestructive contract (§16) — what MeatsackProjection may NEVER alter

The projection may freely change *timbre, envelope, oscillator choice, stereo placement, room, and
the resulting PCM*. It may **not** alter, or allow its artifacts to silently rewrite, any of the
following C137-owned facts. Each has an existing symbolic guard; the PCM-side guard is marked NEW.

| Invariant | Owned by (C137) | Guard |
|---|---|---|
| Thematic identity | `Provenance.motif_id` / `material` / `role_note`; `Note.pitch` | `projection::IdentityMaterial::carries` before/after |
| Assigned narrative carrier | `Note.role`; `NarrativePlan.carriers` | `narrative_receipt::NarrativeReceipt` (symbolic) |
| Harmonic identity | the `ChordSpan` chart; `Note.pitch` / `function` | chart equality |
| Metric coordinates | `StrokeOrigin.metric`, `MetricPosition`, `GrooveTransport` result | must not be recovered by subtraction |
| Promised timing | `Note.start_beat` | apply `FeelTransport` + any gesture offset **exactly once**; never re-key `VoiceEventId` by a shifted time (`synth.rs` hashes the original onset) |
| Intended withholding | `LeadRole::Withheld`; silent roles (`Provenance.role_kind == "silent"`) | NEW — no PCM-side guard exists; a projection must not synthesize sound for a withheld lead or an off-stage player |

The projection must **not** write back into `Score` or `SongMap`. This is the research branch's
**anti-self-healing law**: no iterative rewrite of the symbolic sources in response to an acoustic
quality scalar. Physical artifacts (masking, a buried carrier) are *observable degradations to
report*, never a license to edit the source. Related research-branch laws to carry over verbatim:
*"No phantom resources"*, *"Unknown is not free"*, and *Rig ≠ Patch ≠ Role*.

## 5. The two-way coupling leak (honest — the boundary is not cleanly one-way today)

The symbolic side **already reads acoustic patch internals**: `voice::audible_end_at` (`voice.rs:27`)
approximates the synth's ADSR audible endpoint from `Patch.adsr`, feeding
`effective_audible_end_at`, the `VoiceContinuation` / observed-lifetime logic, and
`sonority::audible_voices` (`sonority.rs`). `expression.rs`, `pocket.rs` and `surgical.rs` also read
`Patch`/ADSR for gate and attack minimums. So C137 depends on `MusicWorld`/`Patch` for lifetime and
masking judgments **before** the cut. Consequence: a projection that changes envelopes invalidates
those upstream symbolic judgments unless it **declares which acoustic facts it holds fixed**. The
boundary contract must therefore name a fixed `AcousticAssumptions` set (at minimum: the ADSR
audible-endpoint model) that both sides agree on; a projection that changes them must re-run the
affected symbolic judgments, not silently diverge.

## 6. Two witnesses, kept separate (§17)

A `Score` can satisfy the symbolic witness while the PCM obscures a carrier (masking, low register,
velocity imbalance, decay, room). So the boundary has **two independent witnesses**, and they must
never be conflated:

- **Symbolic enactment** — *does the Score carry the narrative?* This EXISTS:
  `narrative_receipt::NarrativeReceipt` (the comparative STORY-vs-control witness),
  `PerformanceReceipt`, `SongMapConformance`, `projection::IdentityMaterial`.
- **Audible exposure** — *is the intended carrier plausibly audible in the PCM?* This is PARTIAL.
  `src/audio/perception/` (`AuditoryTrace::observe`) is a real, deterministic, read-only PCM witness
  reporting per-ERB-band excitation, envelope modulation, zero-crossing TFS, and onset flux — but it
  has **no masking model, no loudness model, and no auditory-scene grouping**. It cannot, as-is, say
  "the keys are the carrier here." A defensible carrier-exposure witness must be built NEW: slice the
  carrier's phrase window, render the carrier bus (`StemMask::solo`/`only`, `synth.rs:311`) and the
  rest-of-mix, and compare their `AuditoryTrace` band-energy / onset contrast. The honest claim is
  *"band-energy / onset contrast between the carrier bus and the rest of the mix,"* never a listener
  model.

**Rule:** if the symbolic witness passes but audible exposure fails, classify it a **projection/mix
issue** — tune the projection, never "repair" the `SongMap`. And neither machine witness certifies
that it *sounds like a story*; only the maintainer's ear does (it already has, for the symbolic
layer, on the generated withhold fixture).

## 7. Reuse vs new

**Reusable as-is:** the DSP primitives (`src/audio/dsp`: `Osc`/`FmOsc`/`Noise`, `Adsr`/`ExpDecay`,
`Svf`, `Chorus`/`Reverb`/`Compressor`/`Limiter`/`Delay`, the drum voices); `HumanMusicSynth` as the
control path, with `ProductionControl`/`StemMask`/`BusLevels` as the ablation & metering surface;
`OfflineRenderer`/`wav`/`SampleRate`/`TempoMap`; the whole `perception` module as the downstream
witness; on the symbolic side `PerformanceReceipt`/`perform_checked`/`projection::IdentityMaterial`/
`narrative_receipt`.

**Genuinely new:** the `AcceptedPerformance` newtype and the `AcousticRealization` wrapper; any
event input richer than `Note.velocity` + `start_beat` (the synth's private `NoteEvent` has no
attack-shape, brightness, or per-event pan — velocity is a pure linear amplitude scalar at
`synth.rs:181`); the per-source audible-exposure glue (window slicing + carrier-bus-vs-rest
comparison, the proposed-only `human_music::audition`); a PCM realization fingerprint; and, **if**
the research branch's gesture/rig stage is in scope, the grouped gesture intent that does not exist
today (`Note`/`DrumHit` carry no string/limb/group id) plus the rig types and typed infeasibility.

## 8. Explicitly OUT of v0.5

Physically modeled strings / soundboard / brass / lungs; keyboard & wind rigs (build only after
drums and fretted strings prove a shared contract — research branch's own order); any
perceptual-planning feedback loop (observer "Phase 1–4": mix/timbre assistance, a `PerceptualContract`
that constrains generation); a "human quality" scalar or aesthetic optimizer; independent per-event
jitter (allowed only as a negative control — structured/relational feel only); a non-identity
`FeelTransport` (that is a C137 concern, not the projection's); and any change to default audio
paths, v0.4 byte-exact PCM, canonical fingerprints, R17, or the cover corpus — everything here
defaults OFF.

## 9. Open decisions (maintainer)

1. **Scope vs the research branch** (§0): umbrella / fold-in / rename.
2. **`AcceptedPerformance` surface:** newtype over a receipt-passing `Composition` (narrative-aware)
   vs the thinner `&Score` + `&MusicWorld` + `SampleRate` (timing/dynamics only).
3. **The fixed `AcousticAssumptions` set** (§5): which acoustic facts both sides hold constant so the
   two-way ADSR/lifetime coupling stays sound.
4. **Whether to implement any of it in v0.5** — this document deliberately stops at the contract; the
   mission's own guidance is not to build the physical layer unless its contract is unusually small
   and fully earned.
