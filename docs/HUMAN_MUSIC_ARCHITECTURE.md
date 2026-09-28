# HumanMusic: the `gibson::audio` architecture (experimental)

This documents LibGibson's **audio realization axis** — `gibson::audio` and, on top of
it, the **HumanMusic** procedural composition engine (`gibson::audio::human_music`). It
is a research/engineering-alpha subsystem, Rust-only, not yet part of the C ABI. Read
this alongside the module docs in `src/audio/mod.rs` and
`src/audio/human_music/mod.rs`, which this document expands on.

This is the **Round III** revision of this document. The three rounds are a layer
progression, each fixing what the previous one didn't reach:

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

## 3. What actually happens: the Round III pipeline

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

**Honest, deferred (not half-built).** `repairs_performed` is instrumented but not yet 0
(6/1/1 across the worlds) — moving justification INTO candidate generation (targets-then-connect, a
`MelodicEvent` representation with first-class rests, licensed strong-beat extensions/suspensions) is
the deeper melody rewrite for a later round. The full fusion **ensemble** — a shared `AccentGrid`, an
`EnsemblePlan` with call/response, unison figures and a complexity budget, and a bass melodic engine
beyond the R5 walk — is staged; Round VI delivers the keys step. SFX pitches relative to the local
harmony, and the R4/R5-carried debts, remain. The decisive human listen is still the gate.

## 4. Engines carried over unchanged from Round I

These pieces of the Round I vertical slice are still in place and were not part of the
Round II or Round III rewrites:

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
and the anti-overfitting synthetic-trace tests in §3.11). It does not and cannot certify
that a piece sounds good.

## 7. Honest limits (Engineering Alpha, Round III/IV)

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
- `--beats=N` — length of the demo semantic trace in beats (default `120`).

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
