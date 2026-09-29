# HumanMusic: the `gibson::audio` architecture (experimental)

Round XIII temporal mass (pitch ownership scaled by perceptual exposure): [HUMAN_MUSIC_TEMPORAL_MASS.md](HUMAN_MUSIC_TEMPORAL_MASS.md).

Round XII temporal pitch audit and matched realization: [HUMAN_MUSIC_TEMPORAL_PITCH.md](HUMAN_MUSIC_TEMPORAL_PITCH.md).

**Round XI experiment:** [Phenomenal regimes](HUMAN_MUSIC_PHENOMENAL_REGIMES.md) adds an
opt-in StablePropulsion target and one PropulsiveReturn grammar, preserving the Round IX/X
controls below. It records the matched A/B, independent musical witnesses and listening limits.

This documents LibGibson's **audio realization axis** — `gibson::audio` and, on top of
it, the **HumanMusic** procedural composition engine (`gibson::audio::human_music`). It
is a research/engineering-alpha subsystem, Rust-only, not yet part of the C ABI. Read
this alongside the module docs in `src/audio/mod.rs` and
`src/audio/human_music/mod.rs`, which this document expands on.

This is the **Round IX** revision of this document. The rounds are a layer progression,
each fixing what the previous one didn't reach. Rounds I–III are summarized here; Rounds IV–IX
in §3.12–§3.20, where **§3.20 is the current state** and §7 opens with the **current** limits.
Sections §3.1–§3.11 describe the Round II/III planning layers; where a later round replaced a
mechanism, the section is marked *historical* and points at its replacement.

- **Round I** shipped a vertical slice that got the categorical vocabulary right and the
  causal wiring wrong: a running `MusicIntent` was computed and then thrown away, and the
  four voice generators (comp/bass/melody/SFX) each made local decisions with private
  RNGs off a shared `Form` and `HarmonyEngine`. The result had **local musical
  plausibility without ensemble coherence** — "competent musicians noodling; music by
  coincidence," in the maintainer's own words.
- **Round II** inserted a real planning boundary between semantic meaning and note
  generation (`IntentTimeline -> CoherenceContract -> CompositionPlan -> realizers ->
  apply_arrangement`), so the plan, not the generators, decides what recurs, who plays,
  what the melody develops, and where the band shuts up. That gave the piece
  **ensemble/phrase grammar and identity coherence** — "one band playing one song," with
  recurring material, a real arrangement, and musical sentences that individually hold
  together.
- **Round III** — this document — adds a **discourse** layer on top: long-range goals,
  hierarchical expectation, cross-phrase obligations, directional transformation, and a
  thesis the piece departs from and returns to. The precise claim, and the one this
  document does not exceed: *HumanMusic now models an inspectable musical discourse —
  relations among recurring material, future goals, unresolved obligations, hierarchical
  closure, and directional transformations.* This is **not** a claim that philosophical
  "meaning" has been solved; it is a claim that the causal/structural relationships a
  listener tracks across a whole piece are now represented, computed, and testable, where
  Round II modeled only within-phrase and identity coherence.

Round II's own listening result was a concrete motivation for Round III: a piece could
satisfy every Round-II identity/coherence check — a motif recurs, cadences prepare,
silence is real — and still sound directionless, "a grammatically correct song whose
meaning has been scrambled." Round III's `discourse.rs` module and its adversarial
shuffle-the-sentences probe (§3.4) exist to make that failure a regression the machine
can catch.

## 1. Overview & the orthogonal-axis principle

Audio is a **second, orthogonal realization axis** alongside LibGibson's terminal visual
pipeline — not a feature bolted onto it. It is *not* owned by `Node`, `Surface`,
`Renderer` or `TerminalSession`: the terminal renderer has no audio callbacks, and the
audio graph has no terminal buffers. The two axes share only **semantic state** and
**clocks**, never ownership of buffers or the terminal itself:

```text
VISUAL:  semantic state -> Node/Surface -> terminal renderer
AUDIO:   semantic state -> Score/AudioEvents -> AudioGraph -> PCM -> device
```

A host application can drive both axes from the same underlying story/style state — the
same `Tone`/`Emphasis`/`Density`/`Elevation` that colors a `Surface` can also drive a
`SemanticTrace` that HumanMusic scores — but the two pipelines never reach into each
other's internals.

## 2. Layers

`gibson::audio` is organized in two tiers: a general-purpose **substrate**, and the DSP
layer it's built from. HumanMusic sits on top of both. (This section is unchanged since
Round I — the substrate and DSP layers were not touched by the Round II or Round III
rewrites.)

### Substrate

- **`time`** — sample-accurate clock/transport and beat↔sample tempo mapping. Audio time
  is an integer `SampleTime` counter; the running transport never accumulates an
  elapsed-seconds `f64`, so a long render carries no timing drift. `SampleRate::STUDIO`
  (48,000 Hz) is the canonical internal rate.
- **`buffer`** — the canonical internal representation: planar `f32` stereo blocks
  (`StereoBlock`, separate `left`/`right` vectors). Samples are nominally `[-1.0, 1.0]`
  but the type doesn't clamp — it only measures (peak/RMS/DC) — so mastering stays an
  explicit step.
- **`render`** — the deterministic, device-free `OfflineRenderer` and the `AudioSource`
  trait it drives. This is the load-bearing architectural rule: the *same* `AudioSource`
  is driven by this offline renderer and, under the `audio-cpal` feature, by a realtime
  device callback. There is no separate test synthesizer — correctness proven offline is
  correctness on the device. This is what CI, deterministic tests, profiling and
  listening review all use.
- **`wav`** — a minimal, dependency-free RIFF/WAVE writer for offline listening
  artifacts (16-bit PCM or 32-bit float, interleaved stereo).
- **`report`** — `AudioReport`, the audio analogue of `crate::FrameReport`: per-block
  peak/RMS/DC-offset/active-voice/underrun/render-duration diagnostics.
- **`graph`** — a bounded, inspectable processing graph: a fixed `Bus` taxonomy
  (`Dialogue`/`Music`/`Sfx`/`Master`) and a headroom-aware summing `Mixer`, prepared
  outside any realtime callback.

### DSP

Self-synthesized primitives — **no sampled or prerecorded assets** anywhere in this
layer:

- **`osc`** — band-limited oscillators: sine, polyBLEP saw/pulse, triangle, plus FM.
- **`env`** — ADSR and exponential-decay envelopes.
- **`filter`** — a stable TPT/Zavalishin state-variable filter (LP/HP/BP).
- **`drums`** — synthesized kick, snare, hat and clap voices.
- **`fx`** — equal-power pan, soft saturation, delay, chorus, an FDN reverb, a bus
  compressor and a peak limiter.

### HumanMusic

The composition engine: turns a semantic trace into a full multi-voice `Score` and
renders it through the DSP layer. This is the subject of the rest of this document.

## 3. What actually happens: the pipeline

**Current (Round VIIb)** — what `functor::compose_full` does today:

```text
SemanticTrace
  -> IntentTimeline      (causal walk; each transition carries prev/next SemanticState and an
                          EffectVector — the SIZE of the move, not just its kind)
  -> CompositionPlan     (contract, FormGraph with an exact requested length, discourse +
                          typed ObligationLedger, the coarse ArrangementPlan envelope, and —
                          for DeflectedLift — the world-independent BackboneTimeline: one clock)
  -> PerformancePlan     (the shared performance, built before anybody plays:
       ActionPlan          every morphism path-lifted (or deferred with a reason), gesture
                           verbs as varied manifestation families, sized by the EffectVector
       Stage               the SINGLE orchestration authority: seeded from the arrangement
                           envelope; every action admitted / recast / rejected BEFORE realization
       harmony             backbone or phrase engine + harmonic edits + HarmonicContext timeline
       AccentGrid          the shared 16th-note field
       interaction         InteractionOpportunity → Calls that OWN InteractionMaterial →
                           Responses deriving their material from the call's (InteractionMemory)
       stasis              declared stillness
       ensemble            EnsembleBar per bar (modes, foreground on stage, kinetic target)
       obligations         settled debts bound to the action that discharges them
       budget              one ComplexityAllocation per bar, lead + answers + figures reserved)
  -> realizers           (lead → keys → pad → bass → drums; each a projection of the same plan;
                          every event stamped with the exact ActionIds / interaction / material it
                          performs; the voice-path DP voices pad and keys)
  -> stamp_arrangement   (provenance only — no deletion; orchestration_violations() == [])
  -> Score IR            (exact total length; SFX pitched in the local harmony or owned)
  -> HumanMusicSynth / DSP
  witness::audit / interaction_receipts / diagnostics measure the realized Score, not the plan
```

**Historical (Round III)** — kept for the record; §3.8's post-hoc gate is gone (Round VIIb):

`gibson::audio::human_music` still uses categorical vocabulary in a few places
(`MusicIntent`, `IntentMorphism`, `MorphismCost` in `intent.rs`), and those types are
real and load-bearing — a `MorphismCost` vector genuinely drives the harmony engine's
chord choices (§3.5). But this document does not frame the module as a grand functor
construction: the causal pipeline below is what the code does, and it is a planning
pipeline with an explicit boundary between "what the piece is," "what it is arguing,"
and "which notes get played," not a natural-transformation proof.

The pipeline, end to end:

```text
SemanticTrace
  -> IntentTimeline        (causal walk; every transition inspectable; phrases now read
                             a full IntentSpan trajectory, not one start-of-phrase
                             snapshot)
  -> CoherenceContract     (declared identity axes + budgets + a grammar family)
  -> FormGraph             (phrase boundaries snapped to salient semantic events, so a
                             structural event starts its own phrase)
  -> MusicalThesis /
     DiscoursePlan          (the piece's home; a DiscourseRole + Closure + target
                             trajectory per phrase; an ObligationLedger of cross-phrase
                             debts opened and paid)
  -> CompositionPlan        (the FormGraph + ArrangementPlan + DiscoursePlan bundled;
                             CompositionPlan::targets() is the one PhraseTarget stream
                             every realizer reads)
  -> realizers               (harmony realizes each phrase's Closure under a
                              ResolutionPolicy; one motif threaded across the plan, each
                              statement a DiscourseRole-consequent transform of the
                              thesis germ; a deterministic groove cell that departs and
                              re-enters; a kick-locked bass figure)
  -> apply_arrangement       (gates/scales every voice, stamps real provenance;
                              Score::sections projected FROM the plan)
  -> Score IR
  -> HumanMusicSynth / DSP  (unchanged since Round I, see §2)
```

`CoherenceDiagnostics` (§3.9) measures whether the realized `Score` honored the identity
contract it was built from. `DiscourseDiagnostics` (§3.10) measures whether it honored
the *argument* — direction, not taste.

### 3.1 `IntentTimeline` — the causal spine, now a trajectory not a snapshot

`timeline.rs`'s `IntentTimeline::walk` walks a `SemanticTrace` **once** and materializes
a running `MusicIntent` over time as a sequence of `IntentTransition`s: the intent
before the event, the `IntentMorphism`s the event applied (via `event_to_morphisms`,
the same category-level event→morphism mapping as Round I), the intent after, the
step's `MorphismCost`, and the cost accumulated since the start of the trace. Every
downstream planning stage reads its intent from `IntentTimeline::intent_at(beat)`, so
the concrete score is derived from this spine instead of generators inventing structure
independently.

The walk also carries the semantic **elevation** axis into `MusicIntent::register`
(`SemanticState::register_bias()`), which Round I computed but never consulted. Register
tracks elevation downstream (in phrase-level octave choice, §3.6).

`MorphismCost::combine` sums two costs, and because `MusicIntent` accumulates
`expectation` and `motif.development` along the walk, a round trip through several
morphisms back to the same harmonic function is not the identity — a returning path
leaves a different accumulated state behind (`intent.rs::holonomy_returning_is_not_identity`
still tests this).

**Round III addition — `IntentSpan` and `IntentTimeline::span()`.** Round II represented
a phrase by a single `intent_at(start)` snapshot, so a phrase that began before a
structural event and ended after it silently carried its *pre*-event intent for the
whole phrase — the demo's `Confirmation@88` / `SectionResolved@104` used to land
mid-phrase and be missed. `IntentSpan` (`start`, `end`, `peak_energy`, `peak_tension`,
`events_inside`, `salient_inside`, `next_salient_beat`) sees the whole window a phrase
covers, not just its opening instant, and `EventKind::is_salient()` (`semantic.rs`)
marks which event kinds count as structural. `FormGraph::build` (§3.3) then snaps phrase
boundaries to salient events on a 2-bar sub-grid instead of letting a phrase swallow
one — the concrete fix for the one-snapshot defect.

### 3.2 `CoherenceContract` — the piece's declared identity

`contract.rs` states, before any note is generated, what kind of coherence the piece is
going for. A `CompositionGrammar` names the strategy:

- `HookArc` — a directed song: a recurring motif over a directed harmonic contour.
- `LoopEvolution` — production-driven: a short harmonic loop, interest from
  groove/orchestration evolving under bounded mutation.
- `RiffDrive` — rhythmically forceful, riff/bass/drum-locked, comparatively static
  harmony.
- `WorldSwitch` — two locally coherent regimes joined by an explicit transport that
  preserves at least one declared identity axis across the switch.

Each grammar has a canonical `CoherenceContract` (`CoherenceContract::for_grammar`):
declared `CoherenceAnchor`s (what the listener is meant to track — motif, riff, groove,
harmonic contour/loop, form, orchestration, bass figure), a recurrence period in bars, a
`max_transform` bound on how far a recurring anchor may drift before it stops being
itself, a preferred phrase length, a `ResolutionPolicy` (`Functional` / `Loop` /
`ModalPedal`), a `foreground_budget`, and a `novelty_budget`.

`CoherenceContract::infer` picks a grammar from the *shape* of a trace's
`IntentTimeline` (built-up-then-resolved → `HookArc`; built-up-and-stays-elevated →
`RiffDrive`; never builds → `LoopEvolution`) — a classification, not a quality claim.
**`infer` never selects `WorldSwitch`**; that grammar is reached only by the explicit
`CoherenceContract::for_grammar` / `compose_with_grammar` calibration path (§3.11), not
by the live `compose`/`compose_with_plan` inference — see the honest limits in §7.

### 3.3 `FormGraph` and `CompositionPlan` — the planning boundary and its sole authority

*Current authority (since Round VII/VIIb):* for DeflectedLift the harmony comes from the
`BackboneTimeline` (not from `targets()`), the realizers read the `PerformancePlan`, and the
`ensemble::Stage` inside it is the single orchestration authority (§3.16). `targets()` remains the
phrase-level goal stream the discourse, the lead's statement plan and the phrase harmony engine read.
*Round IX:* the plan is no longer "the song" on its own — it is wrapped, with the theme and the
chart that used to be discovered in performance, in the `SongMap` (§3.19), the one object every
performance is built from.

`plan.rs` is where the structural commitments live:

- **`FormGraph`** (`FormGraph::build`) — a hierarchy of `Phrase`s. Boundaries are the
  contract's phrase grid (`phrase_bars`, `2*phrase_bars`, …) **union** every salient
  event floored onto a 2-bar sub-grid, so a phrase never exceeds `phrase_bars` and a
  structural event (Impact / Confirmation / SectionResolved / …) always starts its own
  phrase instead of being swallowed mid-phrase — this is the concrete Round III fix
  §3.1 introduces. Each `Phrase` now carries its full `IntentSpan` (not just a start
  sample) alongside a `SectionFamily` (`Intro`, `A`, `APrime { base }`, `B`, `Break`,
  `Climax`, `Coda`) and a `PhraseObligation` (`Arrival`, `Continuation`, `Lift`,
  `Suspension`, `Breakdown`, `ReEntry`, `Release`). Recurrence is still explicit: interior
  odd phrases become the recurring `A` family, and every later occurrence is
  `SectionFamily::APrime { base }`, carrying the index of the specific `A` phrase it is a
  bounded transform of. Families are assigned positionally in `FormGraph::build` itself
  (intro first, coda last, the peak-energy interior phrase is `Climax`, odd/even interior
  phrases split `A`/`B`); the discourse layer (§3.4) layers rhetorical *roles* derived
  from the trajectory on top of this positional skeleton, it does not replace it.
- **`ArrangementPlan`** — unchanged in mechanism since Round II: a per-phrase assignment
  of every voice (pad/keys/bass/lead/drums) to an `ArrangementRole`
  (`Foreground`/`Support`/`Foundation`/`Pulse`/`Texture`/`Punctuation`/`Silent`), each
  with a fixed gain multiplier and `Silent` meaning literally zero, built per section
  family and then constrained by the contract's `foreground_budget` and a coverage guard
  that promotes any voice that would otherwise never sound anywhere.
- **`DiscoursePlan`** (§3.4, new in Round III) — the piece's thesis, a rhetorical goal
  per phrase, and the cross-phrase obligation ledger.
- **`CompositionPlan`** bundles all three (`contract`, `form`, `arrangement`,
  `discourse`). **Round III makes it the sole compositional authority.**
  `CompositionPlan::build` infers the grammar from the trace; `build_with_contract`
  takes an explicit contract (the calibration path, §3.11). `CompositionPlan::targets()`
  returns one `PhraseTarget` per phrase — the phrase's timing/family bundled with its
  discourse `PhraseGoal` — and this is the *single* object harmony, groove, bass and the
  lead all read. Round II's two-form split-brain (harmony reading one form's tension
  curve while the lead read a different form's phrase intent) is gone: `total_bars` is
  computed once as `((trace.total_beats / BEATS_PER_BAR).round() as u32).max(1)`
  (`functor.rs`), the legacy `Form::from_trace` is off the musical path entirely, and the
  Score IR's `sections` list is *projected from the plan* (`sections_from_plan`) rather
  than built independently — so `Score::summary` and each event's provenance finally
  describe the same decomposition. `Form`/`Section`/`SectionKind` remain as dumb data
  types the Score IR still carries (§4); they no longer carry independent structural
  authority.
- **`CompositionPlan::dump()`** renders a structural summary — contract, anchors,
  budgets, the discourse thesis/culmination/answer, one line per phrase (bars, family,
  discourse role, closure, tension/register targets, the obligation it opens/pays, and
  each voice's arrangement role), and the full obligation ledger with each debt's
  resolution status — for a cold reader (or the lab example) to answer "what recurs,
  what changed, why is this instrument playing, what is this phrase owed or owing."

### 3.4 `discourse.rs` — the discourse layer (new in Round III)

`discourse.rs` sits between the `IntentTimeline`/`FormGraph` and the realizers, and is
the module that answers the questions Round II's `CoherenceContract` could not: what did
this piece establish, what is currently unresolved, what is this phrase preparing, what
does the final return resolve that earlier arrivals did not.

- **`MusicalThesis`** — the piece-level home: `home_energy`/`home_tension`/
  `home_register`/`home_density` taken from the phrase's `IntentSpan` at the first
  `A`-family phrase (or the opening, if none), the index of the phrase that establishes
  it, and the contract's declared `CoherenceAnchor`s. It is an abstract home the planner
  reasons about, not a note-level object — the concrete motif identity still lives in
  `motif::MotifBank`.
- **`DiscourseRole`** — a phrase's rhetorical job, a *consequence* of the intent
  trajectory (not a parity/index label): `Establish`, `Restate`, `Depart`, `Intensify`,
  `Question`, `Withhold`, `Culminate`, `Answer`, `Return`, `Dissolve`. Each role carries a
  `tension_direction` (`+1`/`0`/`-1`) that diagnostics compare against the phrase's
  actual trajectory (§3.10), and `is_referential()` marks the roles (`Restate`, `Return`,
  `Answer`) that must carry a `refers_to` target.
- **`Closure`** — the permitted cadential strength for a phrase's close: `Open` (no
  cadence), `Half` (dominant, maximum pull), `Deferred` (an expected resolution evaded),
  `Deceptive` (V→vi), `Weak` (plagal/inversion arrival), `Strong` (a full prepared
  cadence). This is the mechanism that replaces "every phrase ends on a prepared tonic
  cadence" (Round II's over-cadencing) with a graded hierarchy; `closure_for(role)` maps
  each `DiscourseRole` to its permitted closure (only the discharge roles — `Answer`,
  `Return`, `Dissolve` — get `Strong`; `Culminate` is deliberately capped at `Half`).
- **`ObligationLedger`** — cross-phrase debts, the generalization of the local V/x
  obligation (§3.5) to the whole form. `ObligationKind` is `HarmonicDeparture`,
  `SuspendedCadence`, `MotifQuestion`, `RegisterAscent`, `OrchestrationSubtraction`, or
  `GrooveDestabilization`. Each `Obligation` records the phrase that opened it, an
  optional deadline, a strength, whether it is `deferrable`, and the phrase that settled
  it (`resolved_by: None` = still open). `is_abandoned()` — open and non-deferrable, or
  open past deadline — is exactly "the unresolved expectation a listener registers as
  scrambled direction," and `abandoned_count()` is the number diagnostics want at zero.
- **`DiscoursePlan::build(timeline, form, contract)`** derives everything from the
  trajectory, planning from both directions. Forward: the thesis is established from the
  first `A`-family phrase. Future-anchored: the **culmination** is the phrase of maximum
  commitment — `peak_energy.energy + peak_tension.tension` across each phrase's span,
  first-strictly-greater wins ties toward the earlier peak, clamped into the interior —
  and the **answer** is the first phrase *after* the culmination whose span actually
  lands a release (tension falls at least 0.2 below the culmination's peak and keeps
  falling). If no later phrase lands a release, `answer` is `None` — a legitimate
  rise-then-remain-open shape, not a bug papered over. `role_for` assigns every phrase's
  `DiscourseRole` from its position relative to the culmination/answer anchors (opening →
  `Establish`; the phrase immediately before the culmination → `Withhold`; between the
  culmination and the answer (or to the end, if unresolved) → `Withhold`; after the
  answer → `Return`; the tail → `Dissolve`; interior `A`-family phrases → `Restate`; the
  phrase two before the culmination poses the `Question` (an incomplete gesture the later
  `Answer` completes) when it is not a restatement; the rest of the run-in splits
  `Depart`/`Intensify` at its midpoint). `resolve_obligations`
  then opens and settles the ledger in role order: `Question`/`Withhold`/`Culminate`/
  `Depart`/`Intensify` open debts; `Answer`/`Return`/`Dissolve` pay the most recent
  matching one. Per-phrase `energy`/`tension`/`density`/`register` targets
  (`targets_for`) are read straight from each phrase's own `IntentSpan` — the culmination
  targets its *peak*, every other phrase targets the state it *concludes* in — so targets
  stay causal, not invented.
- **Culmination is explicitly separated from its discharge.** This is the fix for what
  the module doc calls Round II's "Climax≡Release conflation": the point of maximum
  pressure (`Culminate`) is not automatically the release. `Culminate`'s permitted
  closure is `Half`, not `Strong`, and its debt (a `SuspendedCadence` obligation,
  strength 1.0) stays open until a later `Answer` phrase pays it — proven by
  `culmination_precedes_the_answer_and_is_not_resolved` in `discourse.rs`'s own tests.
- **The adversarial "shuffle the sentences" probe** — `DiscoursePlan::scrambled()`
  reverses the rhetorical role order across phrases (each phrase keeps its own bar
  position and trajectory targets; only the assigned role, closure, and recomputed
  ledger are permuted) and recomputes the obligation ledger for that reversed order,
  sharing `resolve_obligations` with the real build so both use identical bookkeeping
  rules. This turns "a grammatically correct song whose meaning has been scrambled" into
  a regression `DiscourseDiagnostics::faults()` (§3.10) can assert scores strictly worse
  than the real ordering, without needing an aesthetic oracle.

### 3.5 Harmony (`harmony.rs`) — phrase-scoped, closure-realized, grammar-branched

`HarmonyEngine::generate` plans **per phrase**, not per scalar tension sample, and now
takes a `ResolutionPolicy` and reads each phrase's `Closure` from its `PhraseTarget`.
Each phrase is carved into contiguous chord slots (`carve_slots`, sized from the form's
local density). Interior diatonic degree choice (`choose_interior_degree`) minimizes a
weighted `MorphismCost` — voice-leading (nearest semitone root motion), tension error
against the form's target tension, and repetition — over the pool of degrees the current
harmonic function allows; the RNG only breaks an exact tie. Borrowed/mixture (`bVI mix`)
and chromatic-mediant colors can tint an already-chosen interior slot but are explicitly
guarded off cadences, cadence prep, and resolutions. Diatonic chord quality is still
classified from the actual stacked scale thirds (`classify`).

**Round III addition — the harmony engine now branches on `ResolutionPolicy` and on each
phrase's `Closure`, where Round II declared these fields but never read them.** Under
`ResolutionPolicy::Functional`, a `Closure::Strong` phrase gets the Round-II-style
prepared cadence (predominant → dominant → tonic); `Closure::Weak` lands a weaker tonic
arrival; `Closure::Half` stops on the dominant; `Closure::Deceptive` resolves V→vi rather
than V→I; `Closure::Deferred` sets up the expected resolution and then evades it;
`Closure::Open` closes on a plain continuation chord with no cadence at all. Under
`ResolutionPolicy::Loop`, a phrase's close is instead a cyclic return into the loop's own
harmony rather than a functional cadence. **Not every phrase cadences now** — the mix of
closures a real piece exercises (per the maintainer's own measurement on the canonical
demo) puts `cadence_preparation` (§3.9) at roughly 0.40 rather than Round II's 1.00 (every
multi-chord phrase closing on a prepared cadence); *that drop is the fix*, not a
regression, since not every phrase is entitled to a full stop. A secondary dominant
remains a real obligation, not decoration: emitting a `V/x` (`note: "V/of"`) sets a
`pending_resolve` that the *next* interior slot must pay off (tagged `note: "res"`),
reset at each phrase boundary so cadences don't inherit debts.

### 3.6 Motif (`motif.rs`) — one developing idea, now a discourse-consequent transform

*Historical (Round III).* `functor.rs::add_melody`/`motif_for_role` no longer exist: the lead is
planned in `interaction::plan_interactions` (the `ThematicTrajectory`) and realized by
`melody::realize_lead` → `motif::realize_line`, Round VII's targets-then-connectors engine (§3.15).

`motif::MotifBank::generate` grows a small, deterministic, related roster from a single
germ (an `identity` motif, a `hook` fragment, a `rhythmic_cell` diminution, a `bass_cell`
register-dropped opening, an optional inverted `countermotif`) — one idea in several
costumes, generated once per composition from `(scale, seed)`.

`MotifIdentity` is a transposition- and tempo-invariant fingerprint (interval contour,
normalized rhythm profile, direction signature), and `motif_similarity` scores how
related two statements are — tested to be ≥0.9 under transposition or tempo change and
<0.6 against an unrelated contour.

**Round III addition — `Motif::tail(skip)`, the withheld remainder, and every lead
statement is now chosen by the phrase's `DiscourseRole` rather than by drift/obligation
alone.** In `functor.rs::add_melody`, `motif_for_role` (called once per phrase, from
`plan.targets()`) is sign-consistent with the argument: `Establish`/`Restate`/`Return`
state the germ unchanged; `Depart` transposes it; `Intensify` scales its rhythm tighter;
`Culminate` states the bank's `hook`; `Dissolve` fragments it down to a short tail; and —
the concrete Question→Answer coupling — `Question` fragments the germ at a take point `k`
(`germ.fragment(k)`, tagged `"question"`) and records `question_take`, and a later
`Answer` phrase completes *exactly that remainder* via `germ.tail(question_take)`
(tagged `"answer"`), so the answer is provably the missing piece of the question that
posed it, not an unrelated new idea. `Motif::tail`/`fragment` partitioning each other is
directly unit-tested (`fragment_then_tail_reconstructs_the_motif`). Statements enter
grid-aligned; register is realized per role (`Culminate`/`Intensify` climb to octave 5,
`Dissolve` settles to octave 3, everything else follows the phrase's elevation-derived
`register_target`). The lead only sounds in phrases where the arrangement gives it an
audible role.

Each whole statement is still realized against the harmony **jointly**, not note by
note: `motif::realize_phrase` runs a bounded dynamic program over a small per-note
candidate set (chord tones on strong beats, scale tones on weak beats) that scores an
entire path at once — anchor-pitch fit, voice-leading motion between successive notes,
and (heavily weighted) agreement with the original motif's contour direction — unchanged
in mechanism since Round II.

### 3.7 Groove and bass (`groove.rs`, `functor.rs::add_bass`) — typed departure and re-entry

*Historical (Round III).* The live path is `groove::realize_drums` and `bass::realize_bass`
(§3.15/§3.16); `GrooveEngine::generate` remains only for its unit tests, and the kick now follows
the bass rather than the bass the kick.

`GrooveEngine::generate` realizes a deterministic 2-bar groove cell (bar 0 the plain
statement, bar 1 the bounded variation — syncopated kick anticipations, ghost snares, an
open-hat lift — gated by energy). Fills are driven by the plan: `is_fill_bar` checks
whether the *next* bar is a `phrase_end_bar`, so extra fill snares land because a phrase
is ending, not because a die rolled. Micro-timing humanization stays the one
deterministic-but-seeded stochastic element (±~6 ms). The engine exports `kick_beats` so
the bass can lock onto it.

**Round III addition.** A phrase whose `DiscourseRole` is `Withhold` or `Question`
strips the kit back to a stark kick+snare backbone — no hats, ghosts, or syncopated
pushes — and the full groove re-enters at the following `Answer`/`Return` phrase, so the
departure/re-entry is a typed consequence of the discourse role, not an independent coin
flip.

`add_bass` places a persistent, position-determined figure on every kick within a chord
span: the root on the first kick, a fifth on offbeat kicks at high energy, and a
chromatic/scale approach tone into the next chord's root on the last kick before a chord
change — locked to the groove's real kick placements, unchanged in mechanism since Round
II.

### 3.8 `apply_arrangement` and provenance

*Historical (Round II–VII).* Round VIIb removed the post-performance gate: it deleted notes of
phrase-silenced voices after the shared performance was realized, a second orchestration
authority. The arrangement is now the envelope the `ensemble::Stage` is seeded from; actions are
reconciled with it before realization and `functor::stamp_arrangement` only writes provenance.

`functor.rs::apply_arrangement` is the pass that turns "everyone plays all the time"
into a real arrangement: for every note and drum hit it looks up the phrase it falls in
(`FormGraph::phrase_at`) and that phrase's `ArrangementRole` for its voice; a `Silent`
role drops the event outright, an audible role scales its velocity by the role's fixed
gain, and the event's provenance is restamped with the real section family, phrase
index, family label, role label and obligation label from the plan. Unchanged in
mechanism since Round II.

### 3.9 `CoherenceDiagnostics` — structural measurement, not a quality score

`diagnostics.rs::CoherenceDiagnostics::measure(plan, score)` computes a vector of plain
counts and ratios: number of recurring section families, foreground-budget collisions
(should be 0), the fraction of phrases with at least one silent voice, the fraction of
multi-chord phrases that close on a prepared Dominant→Tonic cadence
(`cadence_preparation` — now deliberately *partial* under Round III's closure hierarchy,
§3.5), unresolved-secondary-dominant violations (should be 0) versus resolved ones,
motif restatement count, lead-note count, sounding register, and total note/drum counts.
The module's own tests include positive, null, and mutation controls. This is explicitly
**not** a taste metric — it measures whether the piece is recognizably *itself*, not
whether it's going anywhere (that is §3.10).

### 3.10 `DiscourseDiagnostics` — direction, not taste (new in Round III)

`diagnostics.rs::DiscourseDiagnostics::measure(plan, score)` measures whether the piece
knows where it came from and where it is going, as a vector of independent counts:
`orphan_phrases` (no discourse relation to past or future), `abandoned_obligations`
(`ObligationLedger::abandoned_count()`), `unearned_strong_closures` (a `Strong` terminal
closure that settles no obligation and isn't the final `Dissolve`),
`role_direction_contradictions` (a phrase's role claims one tension direction while its
realized trajectory target moves the other way, compared window-by-window),
`swallowed_salient_events` (phrases whose span still crosses a salient event — should be
0 given §3.1/§3.3's boundary snapping), `distinct_closures` (how many closure strengths
are actually in play — `>1` means the hierarchy is exercised, not degenerate),
`motif_questions`/`motif_answers` (counted from lead note provenance),
`culmination_before_answer` (the core teleology invariant; `true` when the piece is
intentionally unresolved and has no answer at all), and `thesis_return_strength` (mean
`1 - thematic_distance` over `Return`/`Answer` phrases, `[0,1]`).
`DiscourseDiagnostics::faults()` sums the defect counts *only* so the shuffle-the-sentences
probe (§3.4) can assert a scrambled ordering scores strictly worse — every component
stays separately inspectable and the module doc is explicit that this is not a quality
score. On the canonical demo (`demo_trace`, seed `2112`, `BLACK_ICE`):
`faults() == 0`, `distinct_closures == 4`, `abandoned_obligations == 0`,
`culmination_before_answer == true`, and `thesis_return_strength ≈ 0.82` — the numbers
the module's own `a_real_composition_is_directed` test and the shuffle-probe regression
pin.

### 3.11 Calibration probes and synthetic traces — anti-overfitting

`functor.rs::compose_with_grammar(trace, world, seed, grammar)` forces
`CompositionPlan::build_with_contract` to use `CoherenceContract::for_grammar(grammar)`
instead of inferring one from the trace's shape, so a piece can be constructed to
exercise a specific grammar's `ResolutionPolicy`/budgets/anchors regardless of what
`CoherenceContract::infer` would otherwise pick. `examples/human_music_lab.rs --calibrate`
renders four probes through this path — `hook_arc` (the resolved demo trace under
`HookArc`), `loop_evolution` (`calm_loop` under `LoopEvolution`), `riff_drive`
(`rise_unresolved` under `RiffDrive`), and `world_switch` — each writing a WAV, the
plan's structural dump, and both diagnostics reports. `semantic.rs` supplies synthetic
traces built specifically to stress the planner's response to *shape*, not to overfit to
the one canonical demo: `rise_unresolved` (a rising trace with no release, so
`DiscoursePlan::build` must leave `answer: None` and an abandoned debt, not manufacture a
false resolution), `false_climax` (an early smaller peak followed by a later larger one,
so the culmination must land on the true peak, not the first one), and `calm_loop` (a
trace that never builds significant tension, so no artificially high-pressure culmination
should be invented). Each is asserted directly in `discourse.rs`'s own tests
(`an_unresolved_arc_has_no_answer_and_leaves_a_debt_open`,
`false_climax_puts_the_culmination_on_the_true_peak`,
`a_calm_loop_invents_no_high_pressure_culmination`).

`world_switch` is the weakest of the four probes and is described precisely, not
oversold, in §7.

### 3.12 Round IV — pitch justification, phrase continuity, and honest measurement

Round IV addresses the two defects the Round III listen exposed: a "kaleidoscope of coherent
song-fragments spliced together" (motifs that do not flow into one another) and audible "wrong
notes". It works on the note and the phrase boundary, not the discourse skeleton, and adds an
independent measurement layer.

- **`PitchFunction` and the jazz principle (`pitch.rs`, `score.rs`).** Every realized lead note now
  carries a `PitchFunction` against the chord actually sounding beneath it: `ChordTone`, a licensed
  colour, or the justification a non-chord tone earns — `ChromaticApproach`, `DiatonicPassing`/
  `ChromaticPassing`, `Neighbor`, `Suspension`, `Anticipation`, `Appoggiatura`, `SlidePath`. A note
  with no such justification classifies to `None` — an unjustified "wrong note". `realize_phrase`
  repairs *only* those (snap to the nearest tone of the sounding chord); a note that already carries
  a reason is never touched. There are no forbidden pitches, only unjustified ones — remove
  unexplained tension, not tension itself. Measured on the demo, this took BLACK_ICE's lead line from
  7 unjustified notes to 0; all three worlds are now at 0, enforced by a test.
- **One definition of "the peak" (`discourse::culmination_index`).** Round III left a second
  split-brain: the form graph chose its `Climax` family from peak *energy* while the discourse chose
  `Culminate` from peak *energy+tension* — two different phrases, so the arrangement's "everyone in"
  landed on one phrase while the melody's hook landed on another. Both now call one
  `culmination_index`, so the peak is a single phrase and the voices agree on it.
- **Register continuity (`functor::add_melody`).** Each lead statement carries its exit pitch to the
  next; an ordinary phrase picks the octave (among its role's base ±1) that connects to the previous
  statement, while the culmination and licensed ruptures keep their dramatic leap — flow between
  phrases instead of teleportation.
- **Honest config (`world.rs`, `synth.rs`).** The dead `allow_extensions` gate was removed; the
  per-role mix knobs (`pad_mix`/`keys_mix`/`bass_mix`/`lead_mix`), previously declared per-world and
  ignored, now scale each role's contribution to the music bus.
- **`RealizationDiagnostics` (`diagnostics.rs`).** A target is not a receipt. Where the discourse
  diagnostics read the plan's declarations (Round III's `thesis_return_strength` averaged
  `1 - goal.thematic_distance`, auditing a declaration with itself), these read the notes the synth
  actually plays: `unjustified_nonchord_notes`, non-rupture `boundary_leap`, `neighbor_similarity`,
  and a `thesis_return_similarity` measured from realized pitches via `motif_similarity`. On the demo
  (seed 2112): unjustified 0 across all worlds; neighbour similarity 0.28–0.41; boundary leap max
  5–10 semitones — the objective picture of what the transport work has and has not yet achieved.

**Still open after Round IV (see §7):** deeper neighbour-relative motif *development* (the measured
neighbour similarity of ~0.3 shows consecutive statements are still only weakly related — the
remaining "kaleidoscope"); typed many-to-many obligation settlement; delta-aware semantic grounding;
a Prelude role so the first audible statement is not classified `Restate`; real `HarmonicLoop`/`Riff`
grammar objects; and the full adversarial-probe suite. The decisive human listen is still the gate.

### 3.13 Round V — the hanging voice, honest pitch across the whole mix, and a new backbone

Round V addresses a fresh listen: a background voice that "gets stuck on a chord and hangs", audible
"wrong notes" still present, and a suspicion that the cinematic backbone itself is the wrong song.

- **Bounded SFX lifecycle (`synth.rs`, `score.rs`) — the P0 bug.** `SfxVoice::trigger()` gated its
  amplitude envelope on and nothing ever gated it off, so a triggered SFX parked in the ADSR
  `Sustain` stage forever: a nonzero-sustain kind (Confirm/Warning/Transition) rang as a held tone to
  the end of the render, and every kind kept its voice slot occupied, dropping later events. The
  envelope and a per-kind hold are now declared properties of `SfxKind`; `SfxVoice` counts down
  through attack+decay+hold, gates off once, and lets the release reach silence — every gesture is a
  finite one-shot (the hanging Warning tritone becomes a short, bounded dissonance). Regressions prove
  every kind decays within its declared lifetime and four voices service ten sequential events.
- **Stem/bus isolation (`synth::StemMask`, lab `--stems`).** An experimental Rust-only debug surface
  renders any one bus (pad/keys/bass/lead/drums/sfx) alone, so a bad tone can be pinned to a voice
  family. A muted bus still advances its voices, so the full mask reproduces the normal mix exactly.
- **Time-carrying `PitchContext` (`pitch.rs`).** `classify` now takes the note's onset, duration and
  the real chord-change boundaries. Anticipation is accepted only within `ANTICIPATION_WINDOW` of the
  upcoming harmony (a distant future chord no longer justifies a note), and a chord tone whose
  sustained body crosses into dissonance (not a common-tone tie) is flagged — the onset-only blindness
  is closed.
- **Honest pitch across the whole mix (`functor.rs`, `diagnostics.rs`).** The jazz principle now
  covers *every* audible role, not just the lead. Bass carries a real `PitchFunction`: root/fifth are
  `ChordTone` (the fifth taken from the chord's actual tones — `root+7` was a wrong note on a
  diminished chord), the approach is a bounded `ChromaticApproach`, and with room the bass *walks* a
  stepwise `SlidePath` into the next root. Pad/keys are verified chord tones. `RealizationDiagnostics`
  reports `unjustified_nonchord_notes` across all roles with an `unjustified_by_role` breakdown and a
  `cross_boundary_dissonances` count — measured 0 everywhere on the demo.
- **`ThematicTrajectory` (`motif.rs`).** The kaleidoscope root cause was that `add_melody` re-read a
  fixed germ every phrase and applied an unrelated one-shot transform. The trajectory develops with
  MEMORY (`M_{n+1} = develop(M_n)`), returns explicitly to the thesis on Restate/Return, derives its
  hook from the call+response DNA, and reports a typed `Handoff` per statement. Register flow improved
  measurably (max inter-phrase leap fell to ~5 semitones across worlds).
- **`SongBackbone` + the `DeflectedLift` grammar (`contract.rs`, `discourse.rs`).** An additive
  compositional strategy: instead of one cinematic arc, a bittersweet BOUNCE that recurs — verse, then
  a cycle of lift → warm hook opening → soft landing → rounded reset, with the hook returning across
  cycles and a small cyclic harmonic identity (`ResolutionPolicy::Loop`) the ear can learn. Each cycle
  keeps the obligation ledger balanced. It reaches for the *relational contour* of a bittersweet-bounce
  pop song without reproducing any specific one. The canonical lab audition renders it by default
  (`--grammar=hookarc` A/Bs against the earlier song); the four existing grammars are untouched. It
  raised `thesis_return_similarity` sharply (0.38→0.75 on BLACK_ICE) — the song returns to itself.
- **Kaleidoscope adversarial probe (`diagnostics.rs`).** The R4-deferred test: permuting the composed
  lead statements across slots measures worse (a larger mean inter-phrase leap) than the composed
  order — proving the continuity metric is order-sensitive.

**Still open after Round V (see §7):** the `DeflectedLift` bounce imposed on the *cinematic* demo
trace leaves `role_direction_contradictions=2` (a bouncier trace, and a multi-peak arrangement that
tracks the recurring hook, are the follow-ups); moving pitch justification INTO the search (so
`repairs_performed` reaches 0 rather than repairing after the fact); SFX pitches relative to the local
harmonic field; and the R4-carried debts (typed obligations, delta-aware semantic grounding, a Prelude
role, real `HarmonicLoop`/`Riff` objects). The decisive human listen remains the gate.

### 3.14 Round VI — the space-jazz round: spine, intro, mix, melodic language, ensemble

Round V's listen: the music had reached "Jerry-level" — intros were a single repeated chord, the
lead sat far in front of the band, the melody read as beginner major-triad noodling, and the
"Swing & A Miss" spine was inaudible at the root. The target became "Rick-level": Casiopea-grade
fusion mechanics without transcribing any recording. Round VI is a *language* round, six commits,
built spine-to-surface. The measurement instrument was landed **first**, generation-neutral, so
before/after is read by the same tool.

- **Measurement instrument.** `semantic::deflected_lift_trace` is a bittersweet-bounce fixture: two
  lift→deflect→open→reset cycles cresting at Accent/Warning and releasing, never reaching
  `Danger`/`Overlay` (reserved for the cinematic `demo_trace`). It drops
  `role_direction_contradictions` 2→0 — the story stops fighting the grammar's up/up/down/down role
  cycle. `diagnostics::LeadOutlineDiagnostics` is the anti-triad-noodling meter: root/3rd/5th vs
  guide-tone-7th vs extension %, strong-beat extension rate, interval diversity, pitch-class entropy,
  exact-arpeggio recurrence, syncopation, internal rests, range, and a composite `triad_noodle` flag.
  An adversarial `1 3 5 3` fixture trips it; a guide-tone/color/rest line does not. The lab prints it;
  `--story={bounce|cinematic|calm|rise}` selects the story (flagship default = bounce).
- **A real harmonic backbone (`backbone.rs`).** `HarmonicGesture {Lift, Deflect, Open, Reset}` is an
  abstract harmonic axis ORTHOGONAL to the rhetorical `DiscourseRole` (Round V conflated the two, in
  the wrong order — the root cause of the missing spine). A `HarmonicCell` is generated once per
  composition by a bounded, deterministic constraint search over diatonic candidates (gesture-fit +
  distinctness + interior-tonic penalty + cyclic smoothness); no real progression is transcribed, only
  the relational geometry. It tiles bar-aligned as a small cyclic identity the ear can learn, and later
  cycles transform it (a warm color deepens). `HarmonyEngine::generate_with_backbone` tiles the cell for
  DeflectedLift; `generate` is a thin wrapper (other grammars unchanged). The Open/Reset windows finally
  use the extended `Quality` vocabulary (Add9/Maj9/Maj6/Min9/Min6/Maj7/Min7) that had sat dead in the
  type. The lab prints the spine per cycle so a listener can point to reach→miss→open→reset in the dump.
- **Intro composer + backbone-aware arrangement.** `IntroArchetype` (BassPickup/ThemeFragment/
  HarmonicTease/PedalWithUpperMotion) makes the intro a designed subtraction that foreshadows the next
  phrase and exposes ≥2 identity axes over the backbone's already-moving cell — never a lone pad
  (`ArrangementPlan::intro_axes`). `ArrangementPlan::build` now consumes the `DiscoursePlan` and routes
  **every** recurring `Culminate` hook to a Foreground lead, so the hook sings at every cycle, not just
  the one positional Climax. `deflected_lift_roles` fills a partial interior remainder with a
  ledger-balanced `Culminate`+`Answer` mini-cycle, so the hook recurs (≥2) while
  `abandoned_obligations` stays 0.
- **The mix — foreground ≠ louder.** The lead sat +11.5 dB over the harmony bed via three
  multiplicative gain stages; the keys bus was at 0.003 rms. The `ArrangementRole::gain()` spread is
  shrunk (Foreground-vs-Texture 7.5→2.9 dB at the role stage), the harmony bed lifted and the lead
  trimmed per world, and `synth.rs` gains a per-bus `BusLevels` meter tapped before the stem-mute gate
  (a pure observer — bit-exactness untouched). Realized BLACK_ICE lead-vs-keys cliff: 23.3→5.0 dB;
  full-mix peak 0.970→0.777.
- **Fusion melodic seeds + a visible repair counter.** The seed germs are no longer triad arpeggios:
  `seed_a` is a syncopated call leaping to the 5th and stepping back; `seed_b` climbs to the 7th and
  folds down. `realize_phrase_reporting` counts `repairs_performed` (notes the DP produced unjustified
  and the snap pass fixed), surfaced through `Score::melody_repairs` and the lead-outline diagnostic —
  the repair *rate* is now visible, not hidden behind a residual of 0 unjustified notes.
- **Keys as a conversing comp voice.** Keys play 2–3 note upper-structure shells (carrying the guide
  tones and licensed color the backbone supplies) on syncopated positions with an anticipation push,
  instead of a single-note arp — a comping voice under the lead, foregrounded in the B phrases where
  the lead rests.

Measured (bounce, seed 2112): the spine recurs and is distinct per world (BLACK_ICE
`Bm7b5→Cmaj7→Fadd9→Am6`, VAPOR `Gm7→Am7→Dm9→F6`, SWISS `Dm→Em→Am9→C6`); intro exposes ≥2 axes over the
moving cell; the hook sings at ≥2 Culminate phrases; `strong_beat_ext_rate` 0.00→0.35–0.50, extension
color 0.02→0.16 (BLACK_ICE), seventh 0.27 (VAPOR); keys 52→208 notes at rms just under the lead;
`unjustified=0`, `role_direction_contradictions=0`, `abandoned_obligations=0`, deterministic,
full-mix peak safe. Adversarial probes: triad-student, dead-intro, kaleidoscope (octave-invariant
contour similarity), and broken-DeflectedLift-permutation (the spine order beats a scrambled gesture
assignment).

**Honest, deferred at Round VI (historical — since delivered).** Round VII delivered
justification inside the search (targets-then-connect, repairs 0/0/0), `MelodicEvent` with
first-class rests, the `AccentGrid`/`EnsembleBar` ensemble and the bass engine; Round VIIb
delivered a shared complexity budget, SFX in the local harmony and global voice leading. The
Round VI text follows. `repairs_performed` is instrumented but not yet 0
(6/1/1 across the worlds) — moving justification INTO candidate generation (targets-then-connect, a
`MelodicEvent` representation with first-class rests, licensed strong-beat extensions/suspensions) is
the deeper melody rewrite for a later round. The full fusion **ensemble** — a shared `AccentGrid`, an
`EnsemblePlan` with call/response, unison figures and a complexity budget, and a bass melodic engine
beyond the R5 walk — is staged; Round VI delivers the keys step. SFX pitches relative to the local
harmony, and the R4/R5-carried debts, remain. The decisive human listen is still the gate.

### 3.15 Round VII — actions, one clock, contextual harmony, and players who listen

Round VI's listen: "hints of continuity, but the song's meaning remains ambiguous"; "moods and mood
shifts, but almost no ACTION"; instruments locked into characteristic forms whose placement stays
predictable; call-and-response locked into structural slots. The working hypothesis — the engine
generated musical *states* well but did not preserve or realize musical *actions* — was verified
in the code before any edit:

- `IntentTransition::applied` had **zero** readers outside `timeline.rs`. All seventeen morphisms
  died at the timeline boundary; composition read only the scalar energy/tension/density/register.
- Every part was realized blind from shared static inputs (keys at fixed +0.5/+2 offsets, bass on
  every kick, drums generated first, the lead deaf to everyone); call/response was
  `ThematicTrajectory::next_for`'s Question→Answer phrase slot.
- The DeflectedLift backbone was generated privately inside realization and tiled one gesture per
  bar (`bar % 4`) while the flagship story placed its semantic phases ~4 bars apart — two clocks.
  The story's second release (a `Confirmation`) sounded over a harmonic *deflect*.
- `degree_implied_tension` and `function_of_degree` were fixed Ionian tables (duplicated in two
  files). In A Aeolian every Functional cadence prepared with Em7 labelled `Dominant`.

**Contextual harmony (`context.rs`).** Function is a relation, not a degree. `PullEvidence`
derives pull toward a *specific* target from pitch content (leading tone, resolving tritone, root a
fifth/semitone above); a dominant claim needs evidence, so a natural-minor v and a modal ♭VII are
departures. `HarmonicRelation` (Arrival, Prolong, Depart, Prepare, DominantTo, Tonicize, Deflected,
ModalShift, Pedal, ChromaticConnector), a `TensionVector` (pull, distance, colour, strain, surprise,
openness) and a per-harmony `PitchPalette` (chord tones, guide tones, licensed tensions, colour,
expensive tones, next-context targets; the most continuous of several lawful chord-scales) make up
each `HarmonicContext`. The legacy tables survive only as preserved failing witnesses in the tests.
Functional cadences borrow the raised third where the diatonic v has no pull (`V(hm)`).

**One clock (`backbone.rs`).** `BackboneTimeline` is world-independent and lives in
`CompositionPlan`: per slot, the gesture, cycle, variation (thesis / statement / expanded /
compressed / transformed) and the `SemanticBinding` that opened it. `gesture_for_morphisms` binds by
*effect* (Resolve→Open, Relax/Cadence→Reset, Suspend/Modulate→Deflect, Intensify→Lift,
Prepare→home). `TimeScales`/`ClockBinding` declare the relation between the semantic, gesture,
phrase and bar timescales (`SemanticPhase`, or a declared `FixedTiling` fallback), and semantic
events are quantized onto musical time (`timeline::quantize_event_beat`). A home-establishing
opening states the cell in miniature (the thesis). World realization ends every Lift on a
**pointer** with concrete pull; the Deflect lands elsewhere while keeping common tones with the
expected arrival, recorded as a `DeflectWitness`; the Open is chosen as a consequence of the miss;
longer slots alternate their anchor with a pull-free common-tone satellite.

**Language (`language.rs`).** `MusicalLanguage {Simple, FusionConversation}` is how the band
speaks, orthogonal to the song (plan) and the timbre (world): harmonic rhythm inside a gesture,
colour depth, shell voicings, surface subdivision, syncopation weight, interaction readiness,
distributed agency, unison figures, a simultaneous-information budget, chromatic-connective weight
and internal-rest share.

**Actions (`action.rs`).** `MusicalAction {id, cause, initiator, window, kind, target, responders,
binding, pays}`. Every applied morphism is path-lifted (Prepare→Pickup, Intensify→Push,
Suspend→Hold, Syncopate→Displace, Reharmonize, Modulate→Tonicize *(Round VII realized Modulate
as a tonicization; see §3.16)*, Thicken/Thin, Fragment/Sequence,
Resolve paid by a Cadence Hit, Relax→Pullback, …) or recorded as a `Deferral` with a reason. The
backbone adds its own verbs each cycle (Lift: pickup + push into the miss; Deflect: ensemble hit +
break; Open: re-entry + unison figure; Reset: fill into the next attempt); `ActionFamilies` choose
once per piece who initiates each, so a recurring gesture has a recognizable consequence.

**The performance (`performance.rs`).** `CompositionPlan → PerformancePlan → Score`. The
performance owns the harmony and its `HarmonicContext` timeline (with harmonic actions applied as
recorded edits), the `ActionPlan`, a shared 16th-note `AccentGrid` (structural, backbeat,
syncopation, pickup, push, hole, hit) carrying a per-gesture rhythm cell rotated and thinned per
bar and mutated by actions, the lead statement plan (lawfully varied entries, lead answers to
bass/keys figures), calls → response windows chosen under an `InteractionMemory` that penalizes
repeating (initiator, responder, latency, metric step, transform) — with overlapping, delayed,
chord-crossing and deliberately silent answers — and an `EnsembleBar` per bar (foreground, each
player's mode, the complexity budget, a kinetic target). An ensemble verb where the arrangement
leaves one player on stage becomes that player's pickup.

**Players who listen.** Realized in listening order, each a projection of the same plan: `melody`
(the lead), `comp` (keys comp on the grid around the lead's real onsets, answer with the call's
material — quoted, echoed, inverted, compressed, completed; *Round VIIb found that for a bass or
keys call this "material" was the LEAD's coincident notes, and fixed it — §3.16* — stab the hits,
hold suspensions, leave
space; the pad sustains, shells, carries common tones, swells, adds an upper structure or drops
out), `bass` (its own subset of the gesture cell, a pedal re-struck under the Deflect, a walk
through the Lift, a counterline in the lead's gaps, motif figures and answers, in on the hits) and
the drums (`groove::realize_drums`: the pocket identity kept; the kick interlocks with the bass,
ghosts on weak off-beats, open hats on pickups/pushes, action fills, breaks, half/double time, echo
answers). Ensemble unison figures double the lead's line in keys and bass, accented by the kit. A
held note lifts off when the harmony moves.

**Audible witnesses (`witness.rs`).** Each action is audited against the realized score (a push
needs two players on the step, a break an empty window, an answer its responder's notes, a unison
keys and bass on the same onsets and pitch classes, …). The audit found and drove three fixes
(unrealized unisons, phantom intro tutti, unfragmented Fragment actions). *Round VII reported
"57/57 witnessed"; at the Round VII tip the flagship had 53/54/54 actions, the audit ran only in its
own unit test, and several of its checks were temporal proxies — Round VIIb replaced it with an
exact, stamped audit (§3.16).* `kinetic_curve` reads forward motion from onsets per beat; the
compressed second cycle is measurably more urgent than the statement cycle without being louder.

**Calibration A/Bs.** `human_music_lab -- --ab` renders one composition four ways: fusion,
simple language, actions disabled (mood without action), and clockwork fixed-slot responses;
`--language=`, `--actions=off` and `--responses=clockwork` select each on the normal path
(Round VIIb adds `--calls=every` and `--manifest=fixed`; `--ab` renders six cases).

**Later Round VII commits** (after this section was first written): `dd4e048` — an expectation
needs a resolving tritone unless it points home (Deflected over-tagging 13–16 → 3); `54ddc1e` — a
keys-led re-entry is a call; `191c79f` — the lead is targets first, then justified connectors
(`MelodicEvent`, `LicensedExtension`/`Enclosure`, classification inside the search); `79b47b5` —
exposition and the hook protected from fragmentation, lawful release registers.

### 3.16 Round VIIb — closing the causal gaps

Round VII built the nervous system — actions, one clock, contextual harmony, an ensemble — and ended
mid-integration. Round VIIb is an integration/closure round: it verified, with nine read-only
scouts against the code, where an abstraction existed in a struct or a test without constraining the
realized Score, and connected it.

**Interaction material (`material.rs`).** Found: a keys/bass/drum "answer" transformed whatever the
LEAD played inside the call window — even when the bass or keys called — because the players are
realized in listening order and the caller's notes did not exist yet; a lead "answer" to a bass
figure was scheduled after it but did not derive from it. Now every call owns an
`InteractionMaterial` built in the plan (onsets, durations, accents, a relative scale-step contour —
absent for drums — and its `MaterialSource`), every response derives its own material from the
CALL's (`transform_material`: quote / echo / invert / compress / complete), and caller and responder
realize projections of the same object (`line_of`), so realization order no longer decides
causality. A lead answering a figure enters where that figure's projection left off.
`witness::interaction_receipts` measures, on realized notes, `relation(answer, real caller)` against
every other line sounding in the call window.

**Selective, distributed conversation (`interaction.rs`).** Found: every lead statement became a
call; drums could answer but never call; the planner ignored an action's declared responders; the
LEAD could be planned as a free responder the lead realizer never plays. Now an
`InteractionOpportunity` weighs rhetorical openness, space opened by an action, headroom and
redundancy before a statement calls (`CallPolicy::EveryStatement` is the saturation probe); figures
(pickups, fragments, re-entries, fills — including drum fills, rhythm-only) are stated whether or not
anyone answers, and open calls the band answers; one player states one line at a time.

**One orchestration authority (`ensemble::Stage`).** Found: `apply_arrangement` deleted notes of
phrase-silenced voices AFTER the performance was realized — it silently removed a planned intro fill
and a coda pullback, and the old witness counted the resulting silence as the pullback. Now the
arrangement is only the envelope the stage is seeded from; every action is admitted / recast /
rejected before realization (the admission ledger is in the plan dump); realizers ask the stage who
plays and how loud; `functor::stamp_arrangement` only writes provenance, and
`orchestration_violations()` is asserted empty.

**Exact, causal receipts (`ids.rs`, `score::Provenance`, `witness.rs`).** Typed `ActionId`,
`InteractionId`, `MaterialId`, `ObligationId` replace four raw-`u32` id spaces (and a `u32::MAX`
sentinel and `action: 0` placeholders that aliased real action a0). Every realizer stamps the exact
actions, interaction, material and obligation an event performs; `witness::audit`'s primary evidence
is events carrying the id, then the event shape is held to the action's contract (an ensemble push by
two stamped players ahead of the target; a resolve arriving on the intended harmony; a deflect with
the bass on the ACTUAL root; an answer by the right responder carrying its material). Absences
(break, thin) and harmonic changes are labelled as such (`WitnessClass`). The exact audit found and
this round fixed: a beat-0 "pickup" into the first downbeat (deferred), lone-player accents recast as
pickups that began on their own target, a Deflect root swallowed by a figure, resolutions credited to
a silent lead, overlapping same-player figures, a keys hold cancelled by an answer elsewhere in the
bar, `Sequence` "witnessed" from the plan alone (deferred: no realizer sequences yet), a one-beat
break that silenced the kit for the whole bar, an unperformable harmonic verb left in the plan, and a
pad "re-entry" by a pad that never left. The action vector's id contract is explicit: `id == index`
always; the base plan is time-sorted before ids are assigned; `chronological()` and `get()`.

**Obligations (`discourse.rs`).** Deadlines are enforced (a deferrable debt may cross phrases, not
its deadline; `Pending / Settled / Late / Abandoned`); settlement is many-to-many and names a
specific debt of a compatible kind (`compatible(kind, role)`; `SettleError::{IncompatibleKind, …}`);
a debt opened at phrase 5 no longer gets the global answer phrase 3 as its deadline; each settled
debt is bound to the performance action that discharges it and events carry the obligation id. On
the flagship: the harmonic departure is paid by the cadence hit that lands home, the first
suspended cadence by its resolution, and the second — met by the backbone's miss — is recorded as
**Deflected** by that Deflect rather than as a payment the music never makes.

**Tonal regions (`region.rs`).** `Modulate` now changes the region: a `RegionTimeline` (home /
pivot / modulated / return spans), a closely related target (a whole-step lift under rising
pressure, else the dominant key; the relative key's same collection is refused), a pivot diatonic to
both regions, the functional path transposed into the new region, a planned return; each context is
analysed in its own region (`context::analyze_regions`) and a span that does not establish its key is
downgraded to `Tonicize` with the reason. `Tonicize` stays a brief applied dominant. `Reharmonize`
performs a lawful substitution (tritone sub; diatonic third substitute) and otherwise is relabelled
`Recolor`; a harmonic verb with no lawful edit anywhere in its window is deferred.
`HarmonyContextDiagnostics.region_transitions` now counts true region changes;
`palette_scale_transitions` is the old chord-scale count.

**One complexity budget (`budget.rs`).** One interpretable spend measure (weighted onsets); the lead's
statements and every planned answer and figure are reserved first; the remainder goes to the
accompanists by who is in front; a named ensemble action licenses a burst; keys stabs, bass extra
onsets (a walk halves when unaffordable) and drum ghosts/hats consume their free allowance;
`ComplexityReport` compares planned vs realized per bar.

**Sized verbs, varied manifestations, declared stillness (`action.rs`, `timeline.rs`).** Each
transition carries its prev/next `SemanticState` and an `EffectVector` (strength = half arrival
weight, half size of change, plus deltas, compactness, novelty); lifted actions inherit it and it
sizes pickups, breaks, fills (and their density), hit force (a clap on a big compact arrival),
whether the keys join a faint accent, and the kinetic target. Backbone gestures keep their EFFECT
and identity invariants (every Deflect: the harmonic miss + its hit) but are performed through a
bounded `Manifestation` family rotated per cycle under memory (`ManifestationPolicy::Fixed` is the
rigidity probe). `declare_stasis` declares a Prolong event's still span, a relaxation's hold and a
dissolve's rest; `longest_undeclared_idle_beats` counts only what nobody declared (durative windows
cover only their first two beats).

**Global voice leading (`voicing.rs`).** A bounded Viterbi over ≤24 filtered candidates per context
(close, drop-2, shells ± one licensed extension, rootless, guide + extensions, licensed upper
structures, the legacy voicing): lexicographic cost (gesture violations, then guide-tone breaks,
then weighted motion/retention/crossing/spacing/register/span/doubling/melody clash). Pad and keys
voice from the path; the legacy `VoiceLeader` is the control (the path is never worse under the key).

**Exact length and SFX in the local harmony (`functor.rs`, `score.rs`, `synth.rs`).** A requested
length renders exactly (9 / 10.5 / 17 beats, not 8 / 12 / 16); stings are pitched in the local
harmony (a Warning uses the chord's own tritone or is an OWNED dissonance bounded by its owner's
duration) on the quantized beat, and sit under the band's own accent.

**Receipts (flagship bounce, seed 2112, BLACK_ICE / VAPOR95 / SWISS_SIGNAL, at this revision):**

| | BLACK_ICE | VAPOR95 | SWISS_SIGNAL |
|---|---|---|---|
| exact causal witnesses | 54/54 | 54/54 | 54/54 |
| melody repairs / rejudged at release | 0 / 0 | 0 / 0 | 0 / 0 |
| unjustified (every role) / cross-boundary | 0 / 0 | 0 / 0 | 0 / 0 |
| lead connective / ext / strong-beat ext | 0.22 / 0.32 / 0.69 | 0.09 / 0.42 / 0.60 | 0.09 / 0.42 / 0.60 |
| thesis-return similarity / max boundary leap | 0.78 / 4 | 0.88 / 0 | 0.88 / 0 |
| statements → calls (selective rate) | 10 → 6 (0.60) | 10 → 6 (0.60) | 10 → 6 (0.60) |
| figure calls / drum calls | 5 / 1 | 5 / 1 | 5 / 1 |
| informative receipts related to their real caller | 7/7 | 6/6 | 6/6 |
| declared stasis / longest undeclared idle | 12 b / 10 b | 12 b / 10 b | 12 b / 10 b |
| manifestation recurrence lift/deflect/open/reset (Fixed probe: .50/.75/1.00/1.00) | .33/.35/.33/.00 | same | same |
| budget violations | 0 | 0 | 0 |
| onset recurrence keys / bass / drums | .23 / .47 / .42 | .33 / .47 / .54 | .33 / .47 / .54 |
| region transitions (the flagship does not modulate) | 0 | 0 | 0 |
| SFX: chord / owned / unjustified | 6 / 2 / 0 | 6 / 2 / 0 | 6 / 2 / 0 |

The cinematic story now modulates in every world (e.g. BLACK_ICE: A aeolian → pivot → B aeolian
62–84 → return → home) and is fully witnessed (34/34, 35/35, 35/35).

### 3.17 Round VIII — one harmony: the band hears the same chord

> **Verdict (Round VIIIb, §3.18):** the listen rejected this coupled bed as the default — it sounded
> worse than Round VIIb. It survives as `EnsembleCoupling::CoupledR8`, pinned byte-for-byte as the
> negative control; the default is the R7b band again. The theory below stays: as a ruler.

Round VIIb's listen: the song "may have meaning and may be going somewhere, but we have LOST
SONIC/HARMONIC COHERENCE … harmonically out of tune / wrong despite the individual notes being
supposedly justified … the members of the band are all playing the same song in different styles."
Every note had a reason (`unjustified == 0` since Round IV); nothing asked why THESE notes coexist.
A lawful 9th in the lead, a lawful 13th in the keys, lawful guide tones in the pad and a chord tone
in the bass can sum to garbage. Round VIII adds the vertical layer: one harmonic state the whole
band is realized against.

**Phase 0 — separate the score from the synth (`synth.rs`, lab).** `ProductionControl` toggles one
production factor each (clean waves, zero detune, no saturation, dry, no bus compressor, short
releases) while the Score stays bit-identical; `HARMONIC_REFERENCE` removes them all and renders
`StemMask::harmonic()` (the four pitched buses and the pitched stings, no drums). The lab adds
`--harmonic-reference`, `--production=`, `--pair-stems` (the six pitched-role pairs),
`--dump-notes` (every note and sting, with function and action), `--coupling=independent`,
`--sonority-detail=unowned|missing|flip|bass|nonroot|all`, `--audible-floor=<dB>`; non-NORMAL
renders are peak-normalized for comparison and their receipts describe the written file. Nine
read-only scouts separated the defects: tanh intermodulation (−53 dB) and voice stealing were
refuted; the BLACK_ICE lead's FM aliasing was confirmed — at the hook's G6 the 14th sideband folded
to 2528 Hz (≈ D#7 +28 cents) at −14.5 dB re the fundamental, an out-of-tune partial on the note
the discourse plan calls the culmination. `FmOsc::alias_free_index` now clamps every FM index
(Carson's rule plus one sideband under 0.45·sr): on the real lead stem those partials are
≤ −97 dB. That is the one change to NORMAL production this round.

**The vertical theory (`sonority.rs`) — the ONE definition, used by audit and generator alike.**
- *Owned vs unowned dissonance.* A minor 2nd / minor 9th between two sounding notes is owned, in
  this order, by: a short resolving linear note; a resolving suspension (never against its own
  resolution tone, save the 9–8 over a bass an octave or more below); a bass pedal; a sting's
  planned dissonance (only the pitch with no function — never its chord tone); a planned altered
  package, against the dominant's root, resolving; a one-player chord-tone cluster (≥ MIDI 55); a
  one-player semitone release crossfade. Anything else is `UnownedCollision`.
- *Available is not stable anywhere.* `TensionSpec` gives each available tension its register
  floor (55; 60 for an alteration), owner limit, whether it may be the floor (never, unplanned),
  whether it must resolve, and the chord tone it must sit above (Dorian 13 above the ♭7).
- *The floor.* `BassFunction` names what the bass is over the chord (root, 5th, inversion,
  tension, non-chord); a tension or non-chord floor that is not short, stepwise and resolving (or
  a pedal, or planned) is a violation. A release tail under `MIN_OVERLAP_BEATS` is contact, not
  the next chord's floor. `IdentityFlip`: a non-root floor under a chordal band with no root heard
  within `ROOT_MEMORY_BEATS` (2) — judged per slice, with a cut where the memory runs out.
- *Register.* The conventional low-interval limits apply to every pair, across roles.
- *Colour is a budget.* `ColorPolicy` per world × language (BLACK_ICE 2 / 7 voices / 6 pcs,
  VAPOR95 3 / 8 / 7, SWISS_SIGNAL 1 / 6 / 5; the plain-speech language tightens each). The budget
  counts SELECTED colour — tones the chord symbol does not spell (`is_selected_color`); a written
  9th is the song. Every extension still counts for doubling, contradiction, expense and register.
- *One core sound.* Where a chordal player sounds, the BAND carries the identity tones (guide
  tones plus an altered 5th) — no single player has to.
- *Audible lifetimes.* `audible_voices` re-ends every note on its patch's envelope (40 dB per
  decay/release time constant) and every sting on its own kind's envelope, at −30 dB (default)
  and −20 dB (masking). `EnsembleSonorityDiagnostics` slices the realized Score at every onset,
  end and chord boundary and reports hard classes (unowned m2/m9, bass function, unresolved,
  held flips) apart from soft ones — never one scalar — with the worst slices spelled note by
  note, role-pair counts, and every non-root bass note with its function and reason.

**One ledger, one order (`harmonic_state.rs`, `functor::realize_coupled`).**
`HarmonicEnsembleState` holds the contexts, a `SonorityPlan` per harmony (floor, identity core,
the lead's selected colours, room left), the policy and every committed voice; `hazards` asks the
theory above about a candidate. Realization runs in rigidity order — lead, bass, the keys'
material lines, the joint pad+keys bed, the SFX — each committing to the ledger before the next
chooses. Every re-pitch, octave move and refusal lands in `Score::vertical_decisions` with its
reason. `EnsembleCoupling::Independent` keeps the R7b realization as the control; its composition
is pinned to the fingerprint computed on the R7b tip (`ensemble_probes.rs`).

**The players.**
- *Bass (`bass::realize_bass_coupled`).* The unison P0 is fixed: a unison doubles the lead only
  where the pitch class is a safe floor (root or 5th); anything else becomes the root, a linear
  note is held, and one inversion may be owned by the unison action for at most a beat. Figures
  anchor on the root, lift off at a change unless they land on the new root, and a hazardous
  approach is refused (the bass plays less rather than snapping).
- *Keys lines (`comp::keys_lines_coupled`).* Answers and figures are heard against lead and bass:
  each takes the whole-line octave with the fewest hard hazards, then the least counterpoint
  friction (parallel m2/m9 penalized), then home register; an answer's cornered connector may step
  to a stable neighbour that keeps its contour. `MaterialRole` labels each realized event.
- *The bed (`support::joint_support_paths`).* The pad and the keys are voiced as ONE decision: a
  bounded Viterbi over (pad, keys) candidate pairs per harmony, keyed lexicographically by
  witness (a harmonic edit's new tone sounds; a Hold keeps a tone common to the next chord; a
  Thicken is not thinner, lower, or preceded by something thicker than the control), vertical
  (unowned collisions + identity tones nobody carries + keys struck in the previous harmony that
  ring into this one), the backbone gesture, guide-tone breaks, then weighted soft costs. The
  candidates are R7b's plus complements (octave moves, drops to the role's own floor, guide swaps,
  identity completions) and — where the world does not colour triads — plain-triad voicings
  (`required_pcs_with`). The per-role R7b paths are the control carried in the solve; the keys
  count as carrying a harmony only where they strike its voicing and hold it.
- *Tails and stings.* A pad voice whose −20 dB tail would meet another player's semitone
  neighbour (or its own next voicing a minor 9th away) starts its release early; comping stabs
  stop at the change, a Hold's common tones ring on; a sting moves by whole octaves (±36) to the
  placement with the fewest unowned clashes against what AUDIBLY sounds.

**Adversarial review and fuzz.** Four read-only reviewers (theory, coupled pipeline, synth/lab,
test discrimination), each finding re-attacked by an independent verifier, confirmed 16 findings
— the ruler flattering the band (a sting owning its chord tone, stings on the lead's envelope,
tails judged as the next floor, root memory judged once, a lead-sourced unison counted twice), the
coupled path spending R7b's receipts off the flagship (Holds cut at the change, Thickens voiced
thinner, a keys voicing credited that nobody played, a pad tail lifted for an off-stage keys
step), the harmonic reference muting the stings the audit counts, and tests that could not fail.
Fifteen are fixed — every theory and receipt defect but the pad-tail gating with a test that
fails on the old code, the lab and doc ones in place, the weak tests strengthened; the sixteenth
(a flagship keys-line test that passes with placement disabled) stands, because the
discriminating keys-line tests beside it fail when placement is removed (the verifier's own
mutation run). The ignored release sweep
`fuzz_the_coupled_band_keeps_every_receipt_the_control_keeps` (two stories × six lengths × eight
seeds × two grammars × two languages × three worlds = 1152 compositions per arm) now reads:
**receipts lost 0** (gained 50), unowned pitched-pair collisions 11 200 → 75, missing-core beats
1609.6 → 940.7.

**Receipts (flagship bounce, seed 2112; Independent control → Coupled):**

| | BLACK_ICE | VAPOR95 | SWISS_SIGNAL |
|---|---|---|---|
| unowned m2/m9, written durations (beats) | 17/7 (14.53) → **0/0** | 19/9 (12.50) → **0/0** | 29/14 (20.46) → **0/0** |
| unowned m2/m9, audible −20 dB (beats) | 9/6 (14.45) → **0/0** | 25/19 (18.41) → **1/0 (0.18)** | 23/17 (9.90) → **0/0** |
| unowned m2/m9, audible −30 dB (beats) | 25/10 (16.47) → 9/8 (1.56) | 31/25 (23.68) → 12/2 (2.65) | 30/18 (14.19) → 4/3 (1.16) |
| unowned keys/pad · keys/lead · lead/pad (bass pairs 0 both) | 13 · 4 · 3 → 0 · 0 · 0 | 15 · 5 · 5 → 0 · 0 · 0 | 28 · 4 · 8 → 0 · 0 · 0 |
| bass-function violations | 5 → 0 | 5 → 0 | 5 → 0 |
| identity flips (beats) | 5 (2.27) → 2 (1.35) | 8 (3.70) → 3 (1.56) | 4 (2.25) → 3 (1.56) |
| missing-core slices (beats) | 20 (8.85) → 12 (5.46) | 3 (1.15) → 5 (1.85) | 2 (1.40) → 3 (1.84) |
| duplicate-tension / over-colour / unresolved slices | 39 / 1 / 3 → 31 / 0 / 0 | 42 / 0 / 2 → 42 / 0 / 2 | 45 / 18 / 0 → 41 / 15 / 0 |
| max voices / max pcs / mean pcs | 14 / 8 / 3.90 → 11 / 6 / 4.00 | 14 / 8 / 4.00 → 11 / 6 / 4.05 | 14 / 8 / 3.99 → 11 / 6 / 4.02 |
| crowded / too-many-pcs slices | 46 / 3 → 43 / 0 | 17 / 1 → 12 / 0 | 88 / 9 → 81 / 19 |
| non-root bass notes (tension/non-chord among them) | 51 (8) → 44 (3, all chromatic approaches) | 53 (8) → 45 (3) | 55 (8) → 45 (3) |
| pad mean motion / common tones kept | 4.71 / 1.32 → 7.55 / 0.86 | 3.98 / 2.00 → 7.95 / 1.54 | 4.45 / 1.96 → 7.30 / 1.64 |
| joint solve: the solo pad+keys paths' union → joint (unowned) | 15 → 0 | 17 → 0 | 25 → 0 |
| notes | 401 → 399 | 415 → 411 | 414 → 409 |
| witnesses / melody repairs / informative answers to their caller | 54/54 · 0 · 7/7 (both) | 54/54 · 0 · 6/6 (both) | 54/54 · 0 · 6/6 (both) |

The costs are in the table on purpose: the joint bed moves the pad further and keeps fewer common
tones (the vertical tier outranks motion), SWISS_SIGNAL sounds six pitch classes more often than
its budget of five, and VAPOR95/SWISS_SIGNAL leave a little more of a beat without every identity
tone than the control did.

### 3.18 Round VIIIb — surgical: R7b minus its real harmonic garbage

**The listen.** Round VIIb sounded substantially better than Round VIII's coupled default: "trash",
the band had lost its harmonic coherence. The ear outranks the ruler Round VIII optimized.

**Why Round VIII sounded worse** (four read-only audits of 5644c96, reconciled):
- *Confirmed, primary — the solver ranked audit survival above continuity.* The joint pad+keys
  Viterbi (`support.rs`) orders witness and vertical collisions lexicographically before ALL
  voice-leading (motion, common tones and register live in the last, weighted tier), and its beam
  prunes with no previous voicing, so it is blind to motion. One avoided clash buys any amount of
  motion: 76 / 59 / 79 % of the pad voicings were revoiced (two or more voices each), pad motion
  went 4.71 → 7.55 semitones on BLACK_ICE, common tones 1.32 → 0.86, guide-tone breaks 15 → 21.
  The Open-gesture span rule then locks the wide voicings in for the slot. The clashes it chased
  were real — but local, one note each.
- *Confirmed defect, minor cause — fictional simultaneity.* `union_cost` models every keys comp
  stab as ringing the whole harmony window (a real stab covers ~31 % of it), so a lead or bass note
  elsewhere in the chord "collides". On the flagship only 1 of 24 revoiced harmonies was
  fictional-only. `surgical.rs`'s first test pins the negative control: that scorer counts a clash
  between a stab on beat 3 and a melody note on beat 1 that never sound together.
- *Structural, not the proximate cause:* the lead-first rigidity order (the lead dictates; the bed
  contorts around it).

**The default.** `PerformanceOptions::default()` is `EnsembleCoupling::Independent` — the R7b band,
pinned (`the_default_is_the_r7b_band_again`). `CoupledR8` is the rejected control, pinned at the
composition the listen heard (`the_r8_control_is_the_composition_the_listen_rejected`). The FM
anti-alias clamp stays (synth-side, shared by every arm, not implicated).

**The surgical arm (`EnsembleCoupling::Surgical`, `surgical.rs`).** Realize the R7b score note for
note, then repair only what is actually wrong, and leave everything else alone:
- *Real time only.* Every note sounds to its AUDIBLE end at the masking floor (−20 dB), so a
  0.45-beat stab is judged only against what sounds while it sounds. The score is cut at every
  onset, end and chord boundary by the audit's own `slices`.
- *Hard defects only.* An unowned m2/m9 that lasts ≥ 0.125 beat, a floor naming a tension nothing
  owns, a held identity flip, a linear note that never resolves. A doubled colour, a crowded slice,
  a guide tone missing in a transient: measurements, not rewrite commands.
- *One note per defect, by a ladder.* Release it before the collision (articulation before pitch);
  leave it out when the harmony is carried without it; an octave; a nearby tone of the same
  sonority (a chord tone, or a colour the band already sounds); for the bass, the harmony's root or
  fifth — and the bass re-places a line note before it drops one. Never the lead; never above the
  melody; within the realizers' registers. An edit must strictly reduce the hard defects in one
  shared window. Then comes an edit that keeps the harmony's identity: the melody's yield — Fmaj7
  under a melody F drops its 7th and sounds F6/9 — is only the fallback. Then a support player
  before the bass, the earliest rung, the least displacement.
- *Sound before paperwork.* A receipt is a constraint only while it stays realizable: a clean edit
  that keeps every witness wins its tier; if none does, the repair happens and the action is
  reported deferred on the ledger. A bad note is never kept to keep an audit green.
- *Nothing silent.* `Score::vertical_repairs` records every edit (beat, role, pitch, what it
  became, the defect named by its notes, the rung, any deferred receipt). The lab prints the ledger,
  a perturbation report DIFFED from the R7b score (not taken from the ledger), the stability ruler
  against R7b, and the hard defects each arm still sounds.

**The ruler, corrected once.** `sonority::classify_heard` is `classify_clash` plus the single class a
synthetic witness proved wrong: a minor chord's licensed 9th a semitone under its minor 3rd in the
chordal support — the rootless "B-form" (Dm7 as C E F A), whether one player or the pad and the keys
together hold it. The 9th under the MELODY, the minor-9th version and root over major 7th stay
unowned. The frozen R8 generator keeps calling `classify_clash`.

**The bed has a ruler too.** `voicing::HarmonicStability` — per role, mean / median / max motion
between voicings, common-tone retention, and the share of voicings that differ from a reference
realization — prints beside the vertical numbers, so a collision count can never again be bought
with an unseen doubling of bed motion.

**Receipts (flagship: bounce, DeflectedLift, seed 2112; R7b / R8 / surgical):**

| | BLACK_ICE | VAPOR95 | SWISS_SIGNAL |
|---|---|---|---|
| hard vertical defects sounding (audible, −20 dB) | 22 / 0 / **0** | 49 / 4 / **1** | 29 / 0 / **1** |
| surgical edits (diffed; 0 lead, 0 added) | **14 (3.5 %)** | **28 (6.7 %)** | **16 (3.9 %)** |
| pad mean motion (semitones) | 4.71 / 7.55 / **4.38** | 3.98 / 7.95 / **4.28** | 4.45 / 7.30 / **4.39** |
| pad common tones | 1.32 / 0.86 / **1.32** | 2.00 / 1.54 / **1.85** | 1.96 / 1.64 / **1.86** |
| pad voicings changed vs R7b | 76 % / **10 %** | 59 % / **25 %** | 79 % / **14 %** |
| keys voicings changed vs R7b | 43 % / **0 %** | 58 % / **10 %** | 63 % / **2 %** |
| receipts witnessed / deferred | 54 / 54, 0 | 54 / 54, 0 | 54 / 54, 0 |

The two surgical survivors are the lead against a sting (neither is editable) and a pad E6 against
a sting where every repair would cost Fmaj9 its 7th. Across the 1152-composition sweep
(`fuzz_the_surgical_band_never_loses_a_receipt_silently`) the pass edits 4.25 % of the notes and
loses no receipt silently (9 deferred, all on the ledger). Every piece that edits more than a tenth
of its notes is printed with its ledger — these are the short 24-beat pieces and the dirtiest R7b
realizations, each edit a real clash against the melody or between support players, or a floor.

### 3.19 Round IX — the song is one object: `SongMap`

**The defect.** Every doc since Round II said *CompositionPlan is the song*; the code disagreed.
The plan held the contract, form, discourse, arrangement envelope and the abstract backbone — and
the performance then *discovered* the rest of the song while playing it. Three witnesses (commit
`9257d67`, on the R8b tip `0b4483d`, the same plan dump-equal across rooms and idioms):

- **The room picked the theme.** `MotifBank::generate` read the world's mode: BLACK_ICE (Aeolian)
  stated germ id 1 `[0,3,6,4,3,1]`, VAPOR95 and SWISS_SIGNAL (Ionian) germ id 0 `[0,4,3,5,2]`.
- **The room picked the chart.** `backbone::realize(timeline, world, ..)` searched the chord cell
  in the room's own mode. Degree journey `[lift, deflect, open, reset]`: BLACK_ICE `[1,5,2,0]`
  (open on bIII), the Ionian rooms `[1,5,3,0]` (open on IV). A different song, not a transposition.
- **The idiom rewrote the chart's rhythm.** `MusicalLanguage::harmonic_rhythm_bars` (Simple 2,
  Fusion 1) moved the change points inside `slot_path` (19 vs 31 changes; the statement Lift's
  pointer at a different beat), and no song coordinate owned them.

**Two boundaries, one authority each.** `SongMap::build(trace, seed, grammar)` (`song.rs`) is the
one place a song is generated; `PerformancePlan::from_song(song, world, opts)` is the one place a
performance of it is generated, realized by `functor::perform(song, world, opts)`.
`compose_full` is `SongMap::build` + `perform`; `PerformancePlan::build` and `generate_cell` are
gone, not deprecated. The lab builds ONE song and performs it in every room.

**What is the song, what is the band.**

| Song identity (`SongMap`) | Performance freedom (the fiber) |
|---|---|
| contract; form (phrases, families, exact length); discourse (thesis, roles, closures, culmination, obligation ledger); arrangement envelope (who is seated per phrase) | register, voicing, chord quality and extensions, sevenths, articulation, dynamics, timbre, pan, production |
| backbone slot grid (gesture, cycle, start, length, variation) | swing, microtiming, statement entry offsets, fills, groove |
| `ThematicMap`: the motif bank (germ, hook, cells — scale-degree contours) and one `ThemeSite` per lead-seated phrase (role, motif, handoff) | calls, answers, who answers, response latency, fragmentation of non-identity sites |
| `HarmonicMap`: the DeflectedLift chart as relational roots + the canonical harmonic rhythm | harmonic actions (Tonicize / Reharmonize / Recolor / Modulate — typed, logged edits), the idiom's declared rhythm transform, density, ornament |

**The reference frame.** Relative coordinates are charted once against `REFERENCE_FRAME = Ionian`
(Nashville-number practice: degrees relative to a major-scale reference); a room re-modes them and
never chooses them. A song-level frame was considered and rejected on evidence: the only candidate
trait, the thesis `home_tension`, measures `0.200` on every stock trace — a constant in costume.

**Thematic map.** `MotifBank::generate(frame, seed)` (the old "bright rooms get the rising call"
rule, applied to the song's frame instead of the room's) and the `ThematicTrajectory` run ONCE, in
the song; `plan_interactions` states what the song states at each site. `ThemeSite::is_identity`
(Establish / Restate / Return / Culminate) marks what a performance must state as written.

**Harmonic map.** `ChartRoot = Degree(d) | Chromatic{semitones, quality}`; `ChartCell` holds the
lift, second reach, pointer (always realized as a dominant seventh), expected arrival, deflection,
open, reset and the three satellites. `ChartCell::chart(frame, seed)` runs the existing cell search
once, in the frame at tonic C; `ChartCell::realize(region)` gives each degree the room's diatonic
triad and re-derives the expectation from the pointer in the room. `HarmonicMap.bars_per_chord =
CHART_BARS_PER_CHORD = 2` is the canonical harmonic rhythm; the idiom declares a typed
`HarmonicRhythm` transform of it — `AsCharted` (Simple) or `Diminished` (Fusion: half the chart's
bars per chord, the added changes prolongations, the pointer still closing every Lift). Both keep
every landmark; `Diminished` is not a strict refinement of every charted interior change (a 5-bar
slot charted at beats {0, 10} is diminished to {0, 4, 8, 12, 16}).

**π, checked.** `SongMapConformance::check(song, perf, score)` — exact, no weighted total:
`bank_mismatch`; `missing_theme_sites` (identity sites not stated as written, or not heard: lead
notes stamped with the statement's material); `wrong_harmonic_landmarks` (every slot's entry
anchor and every Lift's closing pointer, read from the score's chords in the region in force);
`illegal_harmonic_transforms` (a root outside its gesture's chart vocabulary, or a change off the
declared rhythm grid, unless a recorded harmonic action explains it); `unwitnessed_song_obligations`;
`form_mismatch`; and `*_checked` counts so a pass over nothing is visible. It reads the
performance's plan and the score's chord spans — the harmony every pitched realizer is justified
against (`unjustified_by_role` is that separate receipt) — not realized pitch content. The bank,
the form, the song claim and "stated otherwise" hold by construction for `perform()` output (they
guard hand-built or mutated performances); the landmarks, the chord changes and the obligations are
where engine output can fail — and did (below). Fingerprints (FNV-1a, toolchain-stable): `SongMap`
over song-defining data only (frame, causal timeline, contract, form, discourse, arrangement roles,
the whole backbone, the thematic and harmonic maps — never a world, a language or a performance
decision); `ThematicMap`, `HarmonicMap`, `PerformancePlan`, `Score` (the R7b/R8 pin formula, now
shared).

**Receipts — ONE SongMap, four performances** (bounce story, DeflectedLift, seed 2112, the R7b
band; `cargo run --release --example human_music_lab -- --acceptance`):

SongMap `0xf4d43f1c1da2ac91` · ThematicMap `0x97b44def62ce6a16` · HarmonicMap `0x9f1c8738dda96d9c`

| | BLACK_ICE / fusion | VAPOR95 / fusion | SWISS_SIGNAL / fusion | BLACK_ICE / simple |
|---|---|---|---|---|
| home, rhythm | A Aeolian, diminished | F Ionian, diminished | C Ionian, diminished | A Aeolian, as charted |
| first cycle | Bm7b5 E7 · Fmaj7 · Dm9 · Am6 | Gm7 C7 · Dm7 · A#add9 · F6 | Dm7 G7 · Am7 · Fadd9 · C6 | Bm7b5 E7 · Fmaj7 · Dm · Am |
| performance fp | `0x52719bb9f4017841` | `0xd12e046d2c9a87da` | `0x43c1f59c56cabdb4` | `0x4e41bb5e117aeda3` |
| score fp | `0xb1958b4b761cdf10` | `0xf987f0cffd6b0f4b` | `0xa3e60cd3b8b286e0` | `0xf12a1b651c89e1f0` |
| chords / actions / notes | 31 / 54 / 414 | 31 / 54 / 415 | 31 / 54 / 414 | 19 / 36 / 204 |
| conformance | PASS | PASS | PASS | PASS |

Every row: 3 identity sites, 15 landmarks, all chord changes checked. Negative controls
(`real_mutations_move_the_song`): the thesis contour, an identity site, the deflection's degree and
a landmark slot's gesture each move the fingerprint AND fail the original performance with the
exact reason; the R8b defect re-enacted (the Aeolian room searching its own mode) is caught — 3
wrong Open landmarks (heard C where the song charts D) and 13 chords outside the chart's
vocabulary; the 2-bar performance fails against a 4-bar re-chart on off-grid changes; a Modulate
added to the causal timeline moves the fingerprint. No room or idiom can move it (`SongMap::build`
takes neither).

**The law beyond the flagship — found by review, then fixed.** A blind adversarial review of the
round (read-only) showed the law had only been checked on one configuration, whose trace cannot
produce a harmonic action. A release sweep — six stories (the five stock traces and a
danger-impact trace that asks for a modulation) forced onto DeflectedLift × 80/120/160 beats ×
seeds 1/7/99/2112 × the four performances = 288 — passed **32**. Two causes, both repaired:
- *Inherited, a half-applied invariant:* Round VIIb's `HarmonicFrame::protected` (the chart's
  landmarks) guarded only the diatonic third substitution. `tonicize` rewrote a Lift's closing
  pointer into V7/x (turning the planned miss into a prepared arrival), the tritone substitution
  could replace any anchor, and a modulation's pivot and new-tonic head could overwrite an anchor
  (the Deflect at beat 16 heard a pivot E). Now no harmonic action replaces a landmark's root; an
  action that cannot be performed lawfully is refused or relabelled and deferred with its reason,
  as the engine already did. The re-keyed pointer inside a modulation stays lawful — read in the
  region in force, it is the song's pointer transposed.
- *Current-round, a miscalibrated instrument:* the checker flagged the remainder of a span an
  action had split (a modulation's tonic head) as off-grid. It now knows the action explains it.
After both: **288/288** conform on theme, chart, rhythm and form. The in-repo
`the_law_holds_across_songs` holds 72 of that shape (seed 2112) to it. Flagship audio is
unchanged by both repairs (every pin held).

**Obligations — inherited, measured, not repaired.** A performance leaves a settled song
obligation with no witnessing action in **208 of 240** performances of the five stock stories on
the R8b tip, and in exactly **208 of 240** on the Round IX tree (two trees, same count): R7b's
obligation binding witnesses every debt only on some lengths (the flagship at 120 beats is one).
`SongMapConformance` reports it (`unwitnessed_song_obligations`) and fails those performances;
`the_law_holds_across_songs` pins the inherited count exactly (60 of its 72).

**What changed audibly — and what did not.** VAPOR95 and SWISS_SIGNAL are byte-identical to the
R8b renders: their rooms had already chosen this germ, and their own cell search IS the song's
chart transposed (measured: every R7b and R8 pin held through the harmonic move). Round IX did not
write them a new song; it made BLACK_ICE stop inventing its own. BLACK_ICE: germ `seed_b` →
`seed_a`, Open bIII → iv; R7b pin `0xb45800e8b7ec8bb7` → `0xb1958b4b761cdf10`, R8 pin
`0x1f9714ee5fead745` → `0xb57a1311b903e6d4`.

**Repaired on the way (a current-round regression; its root an inherited gap), refined after
review.** With the song's
theme in the Aeolian room (before the chart moved) a lead `Resolve` went unwitnessed (53/54): the
plan handed it to the lead because a statement sounds at the target, but nothing made the statement
*arrive* — the answer landed on A over C add9, a licensed extension. A post-hoc snap was tried and
rejected (it tripped `melody_repairs == 0`, correctly). The fix is `LineRequest::arrival`: the first
onset in the resolution window may only be a chord tone — a feasibility constraint, not a cost, so a
line that already arrives is unchanged (the Ionian rooms stayed byte-identical). Review showed the
first version filtered BEFORE the candidate cap (admitting chord tones the cap had excluded); it now
filters the capped candidates, reaching past them only when none is a chord tone. On the final song
it is dormant for the flagship (measured: disabling it changes nothing); on the configuration it
was built for (the song's answer over the Aeolian room's own C add9) it arrives — every action
witnessed, nothing snapped (asserted in `real_mutations_move_the_song`). One arrival per statement.

**Not repaired, pinned and reported.** On its new song BLACK_ICE's lead connects with exactly 2
neighbour tones in 43 notes (0.0465), one below the R7b connective floor 0.05 — every other melodic
receipt passes. The floor was not lowered and the line engine was not tuned — but for BLACK_ICE the
floor is WAIVED: the room is pinned at exactly (2, 43), so it cannot thin further unnoticed. The rejected R8 control (solver frozen) leaves exactly one keys/lead collision on
BLACK_ICE's new song; its 5db3296-era history no longer describes that song, so the R8 claim tests
keep the historical comparison for the unchanged rooms and pin BLACK_ICE's exact counts (1, 1).

**The song space is narrower than the seed suggests.** With the frame fixed at Ionian the "bright"
germ is always chosen (`seed_b` is reachable only through an explicit non-bright frame), and the
chart search is deterministic apart from exact ties — few seeds chart differently. The seed still
varies the germ's transposition, hook and cells, and the whole fiber.

**PARKED** (deliberately not built): a song-level reference frame (and with it the second germ
family); obligation witnessing (the inherited binding gap above); a harmonic map for the
phrase-engine grammars (HookArc / Loop / Riff still let the room pick the chord journey — the
`SongMap.harmonic` doc says so); bass landmarks; rhythmic / groove landmarks; essential ensemble
events; a realized-contour theme check (conformance checks the statement and its stamped notes,
not the pitches the line engine chose); chord-quality conformance (roots only — quality is the
room's); a generic category / sheaf framework; any solver, vertical or language work.

### 3.20 Round X — a listener model: `MeaningPlan`, a prior, a composer (current state)

**The listen.** Round IX's four renders were heard as a band on one page — and the page as written
in a language the listener does not understand: coherent, structurally meaningful, not legible.
The band is frozen this round; the defect is in the composer.

**Three witnesses** (commit `17f16e0`, on the Round IX composer, which stays unchanged as the
control): the content is **meaning-blind** (`calm_loop` — peak pressure < 0.5 — and the unreleased
`rise_unresolved` get the same germ and the same chart); **development before exposition** (the
flagship's first lead statement is the thesis transposed +2; the thesis itself is stated once,
last, at phrase 7; calm and rise never state it; the culminating "hook" `[0,4,4,6,3]` is neither
the thesis nor the bank's hook); **expectation before home** (the chart opens on its lift, the first
deflection sounds at beat 4 and the tonic at beat 12; the deflect and open satellites collapse to
16-beat pedals; home is the least-heard anchor of the song).

**The diagram, made to constrain code** (`meaning.rs`):

```text
  SemanticTrace ──F──▶ MeaningPlan ◀──μ── SongMap ◀──π── Performance
```

- **F** (`MeaningPlan::target(trace, plan)`) reads the story — its peak semantic pressure (the
  *arc*: low / mid / high) and whether a confirmation or resolved section releases that peak (the
  *resolution*: home / open) — and the FORM (phrase grid, roles, the lead's seats, backbone slots).
  Never the theme or the chart. Theme lane, per lead-seated phrase: `Learn` the thesis first;
  `Reinforce` it until learned (`LEARNED_AFTER` = 2 statements as written); `Payoff` at every
  culmination; `Answer(resolution)` — its consequent; `Develop` only once learned; `Recognize` on
  return; and `Thesis(arc)` — how far its salient reach goes. Harmony lane, per backbone slot:
  `Establish` home before the first pointer; `Prepare(High)` at every Lift; `Miss(arc)` at every
  Deflect; `Open`; `Reset`; and no surprise anywhere else.
- **μ** (`MeaningPlan::observe(song)`) reads the theme sites and `backbone::lead_sheet` — the chart
  as charted, in the reference frame, at its canonical rhythm, no colour — on the same grid, in the
  same vocabulary plus the events no plan asks for (`Premature`, `Foreign`, `Unestablished`,
  `Unprepared`, `Unrelated`, `Arrive`, `NoRelief`, `NoHome`, `Stray`). The listener model is small
  and stated: familiarity = literal statements heard; a statement is the thesis as written, a
  *variant* (its rhythm and head), a *derivation* (a contiguous stretch of it, moved and/or
  inverted, rhythm kept or uniformly scaled) or *foreign* (a different tune — never heard as a
  development); expectation = the pointer's dominant pull home;
  **surprise at a miss** = what it keeps of the expected arrival (two tones and a prolongation of
  the pointer — V7→iii — barely registers; two tones — V7→vi — is the textbook miss; one tone —
  V7→IV, V7→bVI — lands hard; none is `Unrelated`); **certainty before it** = home already heard +
  a strong pointer; relief = a familiar step from the miss that does not raise the pull; closure =
  home by a familiar relation; a borrowed or remote chart relation outside the designated miss is
  a `Stray`.
- **The law** (`Commutation::check(song)`): exact categorical equality per (lane, place, class),
  and no unasked `Stray`. Every divergence is owned by the **Composer** (its theme and chart could
  have met the plan) or the **Form** it was handed (a Deflect the form placed with no Lift before
  it, or a Lift the form's end cuts off before its pointer — no chart can fix either). A song whose
  stored plan is no longer the one its story asks for is `stale`, and the law does not hold of it.

The instrument was calibrated before any composer existed (commit `1d2fee0`): the Round IX flagship
diverges in exactly 7 places (six theme sites hear material before it was taught — the thesis is
first *learned* at the return meant to be *recognized*, and both culminating "hooks" are no
derivation of the thesis at all, `Foreign` — and the first pointer sounds before home; its V7→vi
miss is right); a hand-written song on the same form commutes 20/20 and every band plays
it; premature development, a payoff with no setup, the return of the never-learned, no miss, and a
mid song under a calm story each break it for their own reason; **surprise everywhere** (every slot
remote) is a valid song all four bands play that the model refuses.

**`CompositionalPrior`** (`composer.rs`) — what kinds of SONG a composer and its listeners find
syntactically familiar; separate from `MusicalLanguage` (how a BAND performs one). One prior,
`HOOKY_FUSION`: HARD — playable (6 beats, ≥ 5 notes, span ≤ 8 steps, a landing ≥ 1.5 beats,
well-formed rhythm); SOFT (bands, not maxima; a line outside them stays lawful and the profile says
why it is unusual) — mostly steps, one salient leap recovered by a step back, a recurring rhythmic
cell, mid displacement, a tonic-triad landing, within the octave over home, no triple strike;
chart — at most one pedal, home heard at least as long as the miss, a fourth/fifth into every Reset.

**The composer** (`SongMap::compose(trace, seed, grammar, Composer::MeaningDirected)`,
experimental, NOT the default): it takes the Round IX song's FORM unchanged and chooses only the
content, so an A/B isolates the page. A bounded search, filters in order with every stage's
survivors recorded (a filter that would empty the field is skipped and reported), no weighted
total: **chart** — 117 lawful DeflectedLift journeys (lift {I, ii, IV} × deflect {vi, iii, IV, bVI}
× open × the Reset's neighbour; the pointer V7, home I; the lift's partner and the deflect/open
neighbours by rule) → μ agrees with F → the prior's chart filters → the seed; **thesis** — a
72-point grammar (`[pickup] cell cell' reach recovery landing`, 6 beats, and its consequent) →
lawful → means the plan (μ = F: it reaches as far as the arc, is taught before it is developed,
and its consequent settles — or hangs on the fifth or second — as the story does) → typical for
the prior → sits best
on the lead sheet (structural notes on chord tones, the line engine's own structural rule, mode-safe;
fits within 0.1 are equals) → the seed. (The chart space is lawful by construction; the thesis
grammar drops nothing at "lawful"; "means the plan" for a thesis RUNS the law on the candidate
scheduled into the song, and "typical" judges the consequent too, bar its story-given landing.) **Schedule** — the thesis as written wherever the listener is
taught, reminded, paid off or recognizes it (a `Restatement` handoff is identity on ANY role; the
performance now reads identity from the site); its consequent at every answer, as a new
`Handoff::Consequent` — identity too, so no Fragment verb cuts its landing away (it keeps the
`response` provenance label every answer-counting diagnostic reads); a development (Round IX's
trajectory) only once learned. The bank is one idea: identity = hook = the thesis.

**The flagship A/B song** (bounce, seed 2112): SongMap `0x8d901e3de29190a2`. Chart `I V7 | vi ii |
IV IV | I IV` (home first; the textbook miss moving to ii; the open window; home with its plagal
neighbour) — 117 → 6 mean the plan → 5 → 5 → 2 → the seed. Thesis `[0,1,0,1,4,3,2]` on
`[½,½,½,½,1,½,2½]` (a two-note cell stated twice in place, one reach of a fourth, a stepwise fall to
the third), consequent `[0,1,0,1,2,1,0]` settling on the tonic — 72 → 72 → 24 mean the plan → 4
typical → 2 on the chart → the seed.
Learned at phrase 1, paid off at 2 and 5, answered at 3 and 6, recognized at 7; every miss heard
with HIGH certainty (home and a strong pointer before it; the control's first miss is MID). All
four bands conform with every one of the six lead sites identity-checked and stated as written,
0 melody repairs / re-judged, 0 unjustified, exact length.

**Witness 1, flipped:** calm gets a stepwise thesis and a soft miss (V7→iii); the unresolved rise a
wide reach, a hard miss (V7→IV) and a consequent left hanging; each commutes with its own story
and not with the other's. **Across songs** (`the_meaning_composer_across_songs`: 6 stories × 3
lengths × 4 seeds × 4 bands = 288 performances): 0 composer-owned divergences (the 28 form-owned
ones are the control's, song by song), π identity everywhere, 0 repaired / re-judged /
unjustified; at the control's own seed the inherited obligation gap is exactly the control's pin
(60 of 72). A scratch scan of 5 stories at every length from 70 to 170 beats found no
composer-owned divergence (12 form-owned: a final Lift clipped by the form's end).

**Claim discipline.** *New capability:* the listener plan (F), its observation (μ), the law, the
explicit prior, and a composer that selects content against them. *Inherited defect repaired:* the
Round IX conformance checker read a modulation's return pivot — the span's last chord kept because
it is diatonic to both keys, home starting ON it — in the home region, calling the chart's own
transposed satellite off-chart (18 of 288 R10 performances in a scratch sweep before the fix;
unreachable with the control's pedal satellites); a chord that starts a `Return` span is now also
read in the region it leaves — only there — with negative controls on both sides (another root at
the return; a home-only root at the modulation's entry). *Current-round regressions repaired:* the
first composer pick (`[3,4,5,4,5,8,7,9]`) sat a fourth above home, which the band's culmination
register placed at C7 in BLACK_ICE — the prior gained its octave band before any render was
offered; a home-salience filter ("home the most-heard root") that the form's long Open slot made
unsatisfiable was restated as "home over the miss" (the witness's actual defect). From a blind
review of the four commits: μ called ANY material "Develop" once the thesis was learned (now
derivation vs `Foreign`); F read the FIRST of tied saturated peaks (now the last); the band's
Fragment verb cut the flagship's phrase-6 consequent to its head in BLACK_ICE and VAPOR95 (now
`Consequent` identity); a hanging consequent could plunge a seventh (now the fifth or the second,
and the consequent is judged typical too); the thesis's "law" stage was a construction, not a
check (now run); μ exempted every move out of the miss from `Stray` (now only a borrowed chord's
step out); a clipped final Lift was blamed on the composer; a malformed motif could panic μ; a
stored plan could go stale unnoticed.

**PARKED:** a learned statistical prior (corpus, MIDI, ML); style inference; affect → open colour
(the trace's tone could carry warm/tense; not used); a final cycle that finally PAYS the expectation
(needs a form change); the form-owned divergences (false_climax's Lift-less deflects at every
length, one in the 80-beat bounce, clipped final Lifts) and the form-level meaning the model does
not yet read at all (rise's strong dissolve over an unresolved story; eight misses in a story with
no surprise in it) — the form is Round IX's and shared; charts
for the phrase-engine grammars; the inherited obligation-witness gap; register/voicing of the lead
(the band's); any performance, solver, vertical or language work.

## 4. Engines carried over from Round I

These pieces of the Round I vertical slice are still in place. They were not part of the
Round II/III rewrites; the synth, the Score IR and the worlds have since evolved (Round IV mix
wiring, Round V finite SFX lifetime and stems, Round VI bus levels, Round VIIb exact length,
typed provenance and pitched SFX) and `VoiceLeader` is now the legacy control of the voice-path
DP (§3.16):

- **`world::MusicWorld`** — a skin's sonic world / local physics: harmonic vocabulary
  gates (`use_sevenths`, `allow_chromatic_mediant`, `allow_modal_mixture`,
  `allow_secondary_dominant`), per-role bus mix (`pad_mix`/`keys_mix`/`bass_mix`/`lead_mix`,
  wired into the synth in Round IV), groove family (tempo, swing, hat subdivision), voicing
  spread, timbral palette, drum character and production settings.
  A world does not pick notes; it constrains the engines above. Three worlds ship:
  `BLACK_ICE` (dark A Aeolian, straight, 88 BPM), `VAPOR95` (lush F Ionian, swung,
  71 BPM), `SWISS_SIGNAL` (sparse clean C Ionian, straight, 118 BPM).
- **`form::Form`** — the world-independent section skeleton (`Intro`/`A`/
  `Development`/`Climax`/`Contrast`/`Coda`) and piecewise-linear energy/tension/density
  target curves. `Form::from_trace` is no longer on the live musical path (§3.3); `Form`/
  `Section`/`SectionKind` remain as the data types the Score IR carries, populated by
  projecting the `CompositionPlan` rather than by an independent computation.
- **`voicing::VoiceLeader`** — turns a chord symbol into a smoothly moving `Voicing` by
  minimizing total semitone motion from the previous voicing across octave placements,
  clamped to a register window. See §7 for exactly what this does and does not do as of
  Round III.
- **The Score IR (`score::Score`)** — notes (`Role::Pad/Bass/Lead/Keys`), drum hits, SFX
  events, chord spans and sections, each carrying a `Provenance` so a rendered piece's
  choices can be traced back to the plan and semantic events that produced them.
- **The Synthesizer (`synth::HumanMusicSynth`)** — realizes a `Score` to audio via the
  DSP layer, implementing `AudioSource` so the deterministic offline renderer and (under
  `audio-cpal`) a real device drive the same code.

## 5. Backend strategy

The pure-Rust core — DSP, scoring, offline render, WAV writing — is **always compiled**
with **zero new mandatory dependencies**. Real device output is an **optional** Cargo
feature, `audio-cpal`, which pulls in `cpal` (pinned to `=0.18.2`) for device I/O and
`rtrb` (`0.3`, a realtime-safe SPSC queue for the callback). `cpal` 0.18.2 declares
`rust-version = "1.85"` — the same value as LibGibson's MSRV floor, i.e. zero headroom —
which is exactly why the version is pinned rather than left open: a future point release
could silently raise the MSRV. `rtrb`'s own MSRV (1.38) is ample by comparison.

On Linux, `cpal` links ALSA and needs `libasound2-dev` at build time — precisely why the
feature is opt-in. `cargo test`, headless builds and CI never need system audio
libraries as a result.

`gibson::audio` is **not** exposed through the C ABI in this milestone;
`GIBSON_ABI_VERSION` stays `1`. MSRV stays `1.85`.

## 6. Determinism & testing

Given `(seed, trace, sample rate, block size)`, the render is deterministic end to end:
every RNG in the composition engines (`rng::Rng`) is seeded from the caller's `seed`
XORed with a per-engine constant, and every test that composes the same inputs twice
asserts identical output (`functor.rs::deterministic_for_seed`,
`harmony.rs::deterministic_progression_for_seed`,
`groove.rs::velocities_are_bounded_and_deterministic`,
`motif.rs::realize_phrase_is_deterministic`, `discourse.rs::is_deterministic`).

The test suite certifies **structure and safety**, not taste: no NaN/Inf in rendered
audio, bounded peaks, exact sample counts, the category-level laws (identity-is-prolong,
composition sums cost, holonomy), voice-leading register/motion bounds, score validity
(`Score::validate`) across all three worlds, the Round II structural invariants (families
recur with `A'` pointing at its base, the foreground budget is enforced, silence is
present, every voice is heard somewhere, cadences prepare where a phrase's closure calls
for one, secondary dominants resolve, motif restatement happens, register tracks
elevation), and the Round III discourse invariants (§3.4/§3.10: every phrase gets a goal,
referential roles point somewhere, the culmination precedes its answer and stays
unresolved, the resolving demo arc abandons no obligation, the canonical arc reads
Establish→Culminate→Answer→Dissolve in order, the shuffle probe scores strictly worse,
and the anti-overfitting synthetic-trace tests in §3.11). Round VIII adds the vertical
controls and probes (§3.17): the brief's synthetic theory controls in `sonority.rs` (individually
legal / collectively garbage, a lead 9th in the bass, the tension pile and its coordinated
version, a passing collision, a suspension and a broken one, a planned altered dominant),
`ensemble_probes.rs` (the Independent control pinned to its R7b fingerprint; the coupled band
against it on every world; the joint bed against the solo paths; the review's counterexamples),
and the ignored release sweep `fuzz_the_coupled_band_keeps_every_receipt_the_control_keeps`
(`cargo test --release --lib fuzz_the_coupled -- --ignored`). Round VIIIb (§3.18) pins the default
to the R7b composition and the R8 control to the one the listen rejected, adds the surgical
controls in `surgical.rs` (a stab judged only while it sounds, one clash → one edit, silence beats a
worse replacement, a receipt never keeps a bad note, one collision does not move the bed, a unison
bass never floors the lead's 9th, owned tension is not sterilized, the perturbation report diffs
the scores), the B-form / avoid controls in `sonority.rs`, the flagship acceptance probe
`the_surgical_band_is_r7b_minus_its_garbage`, and the release sweep
`fuzz_the_surgical_band_never_loses_a_receipt_silently`. It does not and cannot certify that a
piece sounds good.

## 7. Honest limits

### Current (Round X)

- **The ear decides, and it has not heard this round.** The receipts show a song that meets its
  listener plan and plays cleanly in every band; whether anyone wants to hear it again — and hears
  what it is doing sooner than the control — is the listen's to say. Enjoyment is unverified.
- **The listener model is a model.** Categories, not measurements of a mind: familiarity counts
  literal statements, surprise counts kept tones. It cannot hear a performance; it reads the page.
- **The band re-decides the line.** The frozen line engine snaps structural notes to local chord
  tones and chooses connectives (chromatic in the fusion idiom), so the thesis as written and as
  heard differ; theme fit is measured on the lead sheet assuming downbeat entries.
- **One prior, one grammar.** HOOKY_FUSION and a 72-point thesis grammar; the seed chooses among
  equals, so the song space is bounded by design.
- **The form is Round IX's.** Where the form cannot carry the plan (the form-owned divergences) the
  composer cannot fix it.

### Round IX

- **The ear is the gate, and it has not heard this round.** The fingerprints, conformance and
  byte-identity receipts establish one map and four distinct performances that realize it; whether
  the four WAVs sound like different bands playing ONE composition is the listen's to say. Nothing
  here is "same song" by hash.
- **Two of the four acceptance performances were already this song.** VAPOR95/fusion and
  SWISS_SIGNAL/fusion are byte-identical to R8b; the new audio is BLACK_ICE's (both idioms).
- **The frame is a declared constant** (Ionian), not a song choice; the song-level trait that would
  choose it does not exist yet (thesis `home_tension` is 0.200 on every stock trace).
- **Only DeflectedLift has a chart.** The phrase-engine grammars still let the room choose the
  chord journey (`SongMap.harmonic` is `None` for them).
- **Conformance is structural.** Roots, not qualities; the score's chord spans and the lead's
  stamped statements, not realized pitch contours or what the bass actually roots; regions as the
  performance declares them. The fingerprint hashes the plan types' debug text (an implementation
  receipt that moves if a plan type's debug form changes).
- **Song obligations are not witnessed in most performances off the flagship length** (208 of 240
  across the stock stories — inherited from R7b, unchanged by this round); conformance reports and
  fails them.
- **BLACK_ICE's lead is one neighbour tone thinner** than the R7b connective floor on the new song:
  the floor is waived for that room and pinned at exactly 2 of 43.
- **The song space is narrow**: one germ family with the frame fixed; few seeds chart differently.

### Round VIIIb

- **The ear is the gate, and it has not heard this arm.** The surgical numbers say R7b's hard
  defects are gone while its bed barely moved; only the listen can say whether it sounds like the
  good version with the wrong moments removed. The default stays R7b until it does.
- **"Actual sounding" is modelled.** Lifetimes come from each patch's ADSR at the −20 dB masking
  floor, not from rendered samples; reverb is not modelled. A plucked keys attack struck against
  the melody (BLACK_ICE's keys decay to −20 dB in ~0.11 beat at 88 bpm) falls under the
  0.125-beat overlap floor, so these short crunches stay (at −30 dB the surgical BLACK_ICE still
  shows 12 m2 / 2 m9 totalling 1.4 beats, against 15 / 10 over 12.0 beats for R7b).
- **Repairs subtract more than they add.** Omission is the ladder's second rung for the pad and the
  keys, so the bed thins where it collided (BLACK_ICE: 3 pad voices left out; VAPOR95: 15 notes
  removed). Fmaj7 under a melody F becomes F6/9 by design — and missing-guide-tone time rises a
  little (BLACK_ICE 8.85 → 10.79 beats).
- **The pass cannot revoice.** It edits single notes; a voicing that is wrong as a whole (the ladder's
  "revoice one local voicing" rung) is not implemented. A defect no single-note edit cleans stays,
  and is listed.
- **R8's generator is frozen, not fixed.** Its whole-window keys model and its tier order are
  documented here and left as they were heard.
- The model is experimental; Rust-only; not in the C ABI; version 0.3.1; nothing here is released.

### Round VIII

- **The decisive gate is still the ear.** Every number in §3.17 is a model of vertical coherence
  measured on the Score (and on envelope-approximated lifetimes); none says the band now sounds
  like one harmonic organism. The listening questions are in the Round VIII PR text: does the
  accompaniment make room when the lead takes a colour; does the pad complement the keys rather
  than respell the chord; does the chord react to the bass; do overlapping lines sound like
  conversation; can you hear who owns a dissonance and where it goes.
- **The theory is deliberately narrow.** Clashes are minor 2nds and minor 9ths; a semitone two
  octaves apart (25) is not counted, nor is a tritone outside a sting; detune beating and timbral
  roughness are not modelled. Audible lifetimes come from each patch's ADSR (40 dB per time
  constant), not from rendered samples; reverb tails are not modelled.
- **The sonority plan is only partly authored.** `plan_sonority` always plans a ROOT floor, no
  omissions, no altered package and no shared upper structure: the fields exist and the audit
  honours them (the controls plan them by hand), but generation does not yet decide an inversion,
  a slash bass, an alteration package or an upper-structure triad for the band. `ColorPolicy::
  upper_structures` is reported, not consumed.
- **The lead is not negotiated with.** It is realized first and never moved (its colour choices
  are Round VI/VII listening wins); the rest of the band makes room for it.
- **Two layers are inert on the flagship.** The coupled keys lines (no answer or figure met a
  hazard) and the plain-triad requirement (every flagship triad is an add9, which writes its 9th)
  change nothing at seed 2112; they are exercised by their synthetic tests and the fuzz.
- **Costs the listener may hear.** The pad's voice-leading is rougher under the joint bed (mean
  motion ~4 → ~7.5 semitones, fewer retained common tones); SWISS_SIGNAL exceeds its five-pc
  budget in 19 slices (9 before); identity tones are missing a little longer in VAPOR95 and
  SWISS_SIGNAL. 75 unowned pitched-pair collisions remain across the 1152-composition sweep.
- **Harmony the round did not touch.** BLACK_ICE's home chord is a Dorian Am6 (F♯) in an Aeolian
  world that uses F♮ elsewhere (a cross-relation by design of the spine); SWISS_SIGNAL's
  `use_sevenths` is dead. Both are flagged for the listen, not changed.
- **NORMAL production changed once.** The FM band-limit makes the BLACK_ICE lead's top notes (and
  high stings, and VAPOR95's highest keys) less bright; that is the removal of aliasing, not a
  mix decision.
- The model is experimental; Rust-only; not in the C ABI; version 0.3.1; nothing here is released.

### Round VIIb

- **Perceptual quality is unverified.** Every receipt above is structural/causal; none says the
  music is good. The decisive gate is the maintainer's listen to the Round VIIb renders: can you
  hear THAT musician's idea transformed by the responder; the exact musical consequence of a
  semantic event; complexity distributed rather than piled up; recurring gestures related without
  identical choreography; a modulation that really moves; quiet that feels intended; one group of
  musicians reacting to each other.
- **The material-relation metric shares a germ.** Every figure is grown from the one motif bank,
  so a caller and an unrelated line can resemble each other by common provenance; the receipts
  compare against every other line actually sounding, but a positive margin is evidence of
  causal derivation, not proof of perceptual salience. Two-event answers are marked
  uninformative.
- **Lead answers are statements.** The lead answers a figure through its next statement's entry
  (it continues the figure's contour); it never plays a free 2-beat interjection.
- **`Sequence` has no realizer** (the morphism is deferred with that reason); `Pivot` is produced
  by no semantic event.
- **The flagship never modulates** (its story has no Impact / Danger event); true modulation is
  exercised by the cinematic story and the region tests. A Modulate landing exactly on a Deflect
  slot start cannot establish its key and is relabelled `Tonicize`.
- **Voicing weights and budget constants are hand-set**, calibrated so the flagship neither
  violates nor idles its budget; they were not tuned by listening.
- **Undeclared idle is 10 beats** on the flagship (beats 37.5–47.5, inside cycle 1's four-bar
  Deflect slot: the lead's statement at beat 40 stands alone — it is not a call — and no verb fires
  until the next call at 47.5), reported rather than declared away.
- **The Round VII witness count was not causal.** Anything citing "57/57" predates this round.
- The model is experimental; Rust-only; not in the C ABI; version 0.3.1; nothing here is released.

### Historical (Round III/IV)


This is Round IV of the intended v0.4.0 milestone. Round IV (§3.12) added pitch justification (no
unjustified lead notes), healed the second Culminate/Climax split-brain, gave phrases register
continuity, made the dead `MusicWorld` config honest, and added a `RealizationDiagnostics` layer that
measures the realized score rather than the plan's own targets. The limits below remain candid about
what it still does not claim:

- **Perceptual/aesthetic quality remains unproven.** Machine tests (§6, §3.9, §3.10) can
  certify structural, identity, and directional invariants; they cannot certify that the
  music "slaps." That judgment is pending an actual human listen to the rendered WAVs —
  the decisive, still-open gate for this milestone.
- **Global voice-leading is not done.** `voicing::VoiceLeader` (§4) is still local and
  greedy: for each chord it picks among a small set of octave-shifted candidate
  placements by minimizing `voice_motion` — a symmetric, unsigned sum of per-voice
  semitone distances (`(a - b).abs()` per voice, summed) — against the *immediately
  previous* voicing only. There is no phrase- or section-scope progression optimizer
  with a target-register or directional-motion cost, and only the pad and keys voices
  are voice-led at all (`functor.rs` constructs a `VoiceLeader` for pad and for keys;
  bass and lead are placed by their own register/contour logic, not by this module).
  Deferred to a later round.
- **`WorldSwitch` transport is minimal.** The `world_switch` calibration probe
  (`examples/human_music_lab.rs --calibrate`) joins two sonic dialects (`BLACK_ICE` then
  `VAPOR95`) of the *same* `CompositionPlan` and seed via `compose_with_grammar(..,
  CompositionGrammar::WorldSwitch)` called once per world, then concatenates the two
  offline renders into one WAV. The transported identity is the form/discourse skeleton
  (and the seed), not a single motif germ carried byte-identical audio across two synth
  worlds in one continuous render, and there is no mid-song timbral switch within a
  single render — the two regimes are two separate renders joined end to end.
  `CoherenceContract::infer` never selects `WorldSwitch`; only `for_grammar` and the
  calibration path construct it.
- **The role-assignment heuristic is deliberately a minimal skeleton.** `discourse::role_for`
  derives one `Question` slot (two phrases before the culmination, when it is not a
  restatement), one `Withhold` before the peak, one `Culminate`, one `Answer`, and so on,
  from the culmination/answer anchors — a coherent minimal argument, not the full rhetorical
  variety the `DiscourseRole` vocabulary could express. A trace whose entire run-in is
  `A`-family, for instance, poses no `Question` at all; there are no nested departures or
  multiple questions. The `Question`→`Answer` motif coupling is live on the canonical demo
  (`motif_questions == 4`, `motif_answers == 3` at seed `2112`/`BLACK_ICE`) and is separately
  unit-tested at the motif level (`motif.rs::fragment_then_tail_reconstructs_the_motif`);
  richer, more evidence-driven role assignment is future work.
- **`Modulate` still doesn't change tonal region.** `intent.rs`'s `IntentMorphism::Modulate`
  bumps `MusicIntent::function` to `Function::Dominant` and advances
  `motif.development`, but the harmonic key stays fixed per `MusicWorld` — there is no
  mechanism that shifts the actual tonal center mid-piece.
- **The obligation/motif diagnostics still key on string tags.** Secondary-dominant
  resolution detection (`harmonic_obligation_violations`/`secondary_dominants_resolved`
  in `CoherenceDiagnostics`) matches `ChordSpan::note` strings `"V/…"`/`"res"`; motif
  question/answer/restatement counts (`DiscourseDiagnostics`, `CoherenceDiagnostics`)
  match `Provenance::motif_xform` strings `"question"`/`"answer"`/`"statement"`. These
  are load-bearing string comparisons, not a typed obligation-to-event link.
- The model is experimental and expected to evolve between releases.
- It is Rust-only and intentionally not exposed through the C ABI this milestone.
- The package version stays `0.3.1` until this work is reviewed and a release is cut;
  nothing here has shipped yet.

## 8. How to run

```
cargo run --release --example human_music_lab
```

Writes `black_ice.wav`, `vapor95.wav` and `swiss_signal.wav` (one per world) to a temp
directory by default, along with the plan's structural dump (`CompositionPlan::dump`),
a `CoherenceDiagnostics` report, and render receipts printed to stdout. Flags:

- `--world=black_ice|vapor95|swiss_signal` — render only one world (default: all three).
- `--out=PATH` — output directory (default: a `libgibson_human_music` folder under the
  system temp dir).
- `--seed=N` — composition seed (default `2112`).
- `--beats=N` — length of the semantic trace in beats (default `120`; any length — a partial
  final bar is rendered exactly since Round VIIb).
- `--story=bounce|cinematic|calm|rise` — the semantic fixture (default: the DeflectedLift
  bounce); `--grammar=deflected|hookarc|loop|riff` — the composition grammar.
- `--language=simple`, `--actions=off`, `--responses=clockwork`, `--calls=every`,
  `--manifest=fixed` — the calibration probes (plain speech, mood without action, fixed-slot
  answers, every statement a call, one gesture choreography every cycle).
- `--ab` — the flagship six ways (fusion / simple / actions_off / clockwork / saturated /
  fixed_gestures) with the receipts next to each WAV, then every world realized by the R7b band,
  the rejected R8 coupled bed and the surgical arm (`ab_coupling_<world>_{r7b,r8,surgical}.wav`)
  with the vertical numbers beside each; `--stems` — one WAV per bus.
- Round IX: every normal-path render is a performance of ONE `SongMap` (built once, printed with
  its fingerprints; each world prints its performance/score fingerprints and `SongMapConformance`).
  `--acceptance` renders the acceptance set from that one map —
  `acceptance_{black_ice,vapor95,swiss_signal}_fusion.wav` and `acceptance_black_ice_simple.wav` —
  each with its fingerprints, spine and conformance report (the coupling flag is honoured).
- Round X: `--meaning` renders the A/B — the same story, seed, grammar and band, two composers:
  `black_ice_structural.wav` / `black_ice_meaning.wav` and `vapor95_structural.wav` /
  `vapor95_meaning.wav` (A = the Round IX song, byte-identical to its acceptance renders; B = the
  meaning-directed song), with the prior, the chart and theme filters' survivors, both pages, both
  expectation receipts (`Commutation::report`) and each render's fingerprints and conformance.
- Round VIIIb: `--coupling=r7b|r8|surgical` (default `r7b`, the Round VIIb band; `independent`
  and `coupled` are accepted aliases). Every arm prints the harmonic-stability ruler (against the
  R7b realization off the default) and the hard vertical defects it still sounds; `surgical` adds
  the repair ledger and the perturbation report diffed from R7b.
- Round VIII: `--pair-stems` (the six
  pitched-role pairs); `--harmonic-reference` (the same Score on clean, dry, zero-detune,
  short-release voices, stings included, drums out, peak-normalized) or `--production=`
  `clean,nodetune,nosat,dry,nocomp,shortrel` (single factors); `--dump-notes`
  (`<world>.notes.tsv`: every note and sting with its function, tag, chord and actions);
  `--sonority-detail=unowned|missing|flip|bass|nonroot|all` (every offending slice spelled note
  by note, or every non-root bass note with its reason), with `--audible-floor=<dB>` to slice the
  audible lifetimes instead of the written ones.

The normal path prints, per world: the plan and backbone dumps, the action plan (with its
manifestations, stasis and deferrals), the performance (calls, responses, ensemble), the
coherence / discourse / realization / lead-outline / action / rigidity / harmony-context
diagnostics, the exact causal witness audit, every interaction receipt, opportunity and
admission, the complexity budget, the voice-path diagnostics, the ensemble sonority (written and
audible), the joint solve's report and every explained vertical decision, the lead's repair
counts, and render safety receipts.

The point of the lab, per its own doc comment: the three WAVs should be recognizably the
*same* music in form and meaning — same section skeleton, same motif identity, same
resolutions — while sounding like three different dialects. Machines can certify the
structure (§3.9, §6); only your ears can judge the rest.

```
cargo run --release --example human_music_lab -- --calibrate
```

Renders the four grammar/discourse calibration probes described in §3.11
(`calib_hook_arc.wav`, `calib_loop_evolution.wav`, `calib_riff_drive.wav`,
`calib_world_switch.wav`), each printed with its plan dump, `CoherenceDiagnostics`
report, and `DiscourseDiagnostics` report — the fastest way to look at whether the
discourse layer produced a legible argument (a culmination that precedes its answer, a
non-degenerate closure hierarchy, zero abandoned obligations) for a piece under a
*forced* grammar rather than an inferred one.
