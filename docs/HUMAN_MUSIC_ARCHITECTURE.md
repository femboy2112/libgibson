# HumanMusic: the `gibson::audio` architecture (experimental)

This documents LibGibson's **audio realization axis** — `gibson::audio` and, on top of
it, the **HumanMusic** procedural composition engine (`gibson::audio::human_music`). It
is a research/engineering-alpha subsystem, Rust-only, not yet part of the C ABI. Read
this alongside the module docs in `src/audio/mod.rs` and
`src/audio/human_music/mod.rs`, which this document expands on.

This is the **Round II** revision of this document. Round I shipped a vertical slice
that got the categorical vocabulary right and the causal wiring wrong: a running
`MusicIntent` was computed and then thrown away, and the four voice generators
(comp/bass/melody/SFX) each made local decisions with private RNGs off a shared `Form`
and `HarmonyEngine` — "locally valid, globally incoherent," in the words of the
maintainer's own ears. Round II inserts a real planning boundary between semantic
meaning and note generation so the plan, not the generators, decides what recurs, who
plays, what the melody develops, and where the band shuts up. That is what this document
now describes.

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
layer it's built from. HumanMusic sits on top of both. (This section is unchanged from
Round I — the substrate and DSP layers were not touched by the Round II rewrite.)

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

## 3. What actually happens: the Round II pipeline

`gibson::audio::human_music` still uses categorical vocabulary in a few places
(`MusicIntent`, `IntentMorphism`, `MorphismCost` in `intent.rs`), and those types are
real and load-bearing — a `MorphismCost` vector genuinely drives the harmony engine's
chord choices now (§3.4). But this document no longer frames the module as a grand
functor construction: the causal pipeline below is what the code does, and it is a
planning pipeline with an explicit boundary between "what the piece is" and "which notes
get played," not a natural-transformation proof.

The pipeline, end to end:

```text
SemanticTrace
  -> IntentTimeline        (causal walk; every transition inspectable)
  -> CoherenceContract     (declared identity axes + budgets + a grammar family)
  -> CompositionPlan       (FormGraph with recurring families + phrase obligations;
                             ArrangementPlan with an enforced foreground budget and
                             first-class silence)
  -> realizers              (harmony trajectories; one threaded developing motif;
                              a deterministic groove cell; a kick-locked bass figure;
                              a sparse groove-locked comp)
  -> apply_arrangement      (gates/scales every voice, stamps real provenance)
  -> Score IR
  -> HumanMusicSynth / DSP  (unchanged from Round I, see §2)
```

`CoherenceDiagnostics` (§3.7) then measures whether the realized `Score` actually
honored the plan it was built from — a structural measurement, not a quality score.

### 3.1 `IntentTimeline` — the causal spine

`timeline.rs`'s `IntentTimeline::walk` walks a `SemanticTrace` **once** and materializes
a running `MusicIntent` over time as a sequence of `IntentTransition`s: the intent
before the event, the `IntentMorphism`s the event applied (via `event_to_morphisms`,
still the same category-level event→morphism mapping as Round I), the intent after, the
step's `MorphismCost`, and the cost accumulated since the start of the trace. Every
downstream planning stage reads its intent from `IntentTimeline::intent_at(beat)`, so
the concrete score is derived from this spine instead of generators inventing structure
independently — this is the fix for the Round I defect described above.

The walk also carries the semantic **elevation** axis into `MusicIntent::register`
(`SemanticState::register_bias()`), which Round I computed but never consulted. Register
now genuinely tracks elevation downstream (in phrase-level octave choice, §3.5).

`MorphismCost::combine` sums two costs, and because `MusicIntent` accumulates
`expectation` and `motif.development` along the walk, a round trip through several
morphisms back to the same harmonic function is not the identity — a returning path
leaves a different accumulated state behind (`intent.rs::holonomy_returning_is_not_identity`
still tests this).

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
**`infer` never selects `WorldSwitch`**; that grammar exists as a declared contract
family (constructible via `for_grammar` and covered by its own unit tests) but nothing
in the live `compose`/`compose_with_plan` pipeline currently reaches it — see the honest
limits in §5.

### 3.3 `CompositionPlan` — the planning boundary

`plan.rs` is where Round II's structural commitments live, built from the timeline and
a bar budget (`CompositionPlan::build`):

- **`FormGraph`** — a hierarchy of `Phrase`s tiled on the contract's phrase grid (a
  musically legible 2/4/8-bar grid), each with a `SectionFamily` (`Intro`, `A`,
  `APrime { base }`, `B`, `Break`, `Climax`, `Coda`) and a `PhraseObligation` (`Arrival`,
  `Continuation`, `Lift`, `Suspension`, `Breakdown`, `ReEntry`, `Release`). Recurrence is
  explicit: interior odd phrases become the recurring `A` family, and every later
  occurrence is `SectionFamily::APrime { base }`, carrying the index of the specific `A`
  phrase it is a bounded transform of — not a new area that happens to share a label.
  The climax lands on the phrase holding the peak-energy transition from the timeline,
  clamped away from the intro/coda. Each phrase's representative `MusicIntent` is read
  straight from `IntentTimeline::intent_at`, so the plan is derived from the semantic
  walk rather than reinventing structure.
- **`ArrangementPlan`** — a per-phrase assignment of every voice (pad/keys/bass/lead/
  drums) to an `ArrangementRole` (`Foreground`/`Support`/`Foundation`/`Pulse`/`Texture`/
  `Punctuation`/`Silent`), each with a fixed gain multiplier and `Silent` meaning
  literally zero. `ArrangementPlan::build` assigns roles per section family (e.g. the
  intro silences keys/bass/lead/drums and leaves only a textural pad; `A`-family phrases
  put the lead in the foreground; `B` swaps the foreground to keys and silences the
  lead so "the melody breathes"; the climax licenses more simultaneous foreground if the
  contract's budget allows it), then enforces the contract's `foreground_budget` by
  demoting any phrase's excess foreground voices to support (lead keeps priority), and
  finally runs a coverage guard that promotes any voice that would otherwise never sound
  anywhere into an audible role in the plan's highest-energy phrase.
- **`CompositionPlan::dump()`** renders a structural summary — contract, anchors,
  budgets, and one line per phrase (bars, family, obligation, energy/tension/register,
  rupture flag, and each voice's arrangement role) — for a cold reader (or the lab
  example) to answer "what recurs, what changed, why is this instrument playing."

### 3.4 Harmony (`harmony.rs`) — phrase-scoped, cadence-prepared, cost-selected

`HarmonyEngine::generate` now plans **per phrase**, not per scalar tension sample. Each
phrase is carved into contiguous chord slots (`carve_slots`, sized from the form's local
density) and closed with a genuinely prepared cadence: the last slot is a tonic triad,
the second-to-last is the dominant that resolves into it, and (for phrases with three or
more slots) the slot before that is a pre-dominant. A secondary dominant is a real
obligation, not decoration: emitting a `V/x` (`note: "V/of"`) sets a `pending_resolve`
that the *next* interior slot must pay off (tagged `note: "res"`), reset at each phrase
boundary so cadences don't inherit debts. Interior diatonic degree choice
(`choose_interior_degree`) minimizes a weighted `MorphismCost` — voice-leading (nearest
semitone root motion), tension error against the form's target tension, and repetition —
over the pool of degrees the current harmonic function allows; the RNG only breaks an
exact tie. Borrowed/mixture (`bVI mix`) and chromatic-mediant colors can tint an
already-chosen interior slot but are explicitly guarded off cadences, cadence prep, and
resolutions. Diatonic chord quality is still classified from the actual stacked scale
thirds (`classify`), so it comes out correct in any mode.

**This machinery is not yet grammar-differentiated.** `HarmonyEngine::generate` always
produces the same prepared-cadence, functional-harmony shape described above, regardless
of which `CompositionGrammar` the contract declares. `CoherenceContract::resolution` can
be `Loop` or `ModalPedal` (declared for `LoopEvolution` and `RiffDrive` respectively),
but nothing in `harmony.rs` reads `ResolutionPolicy` — the field is carried on the
contract and asserted in `contract.rs`'s own tests, but the harmony engine has no branch
on it. See §5.

### 3.5 Motif (`motif.rs`) — one developing idea, threaded jointly against harmony

`motif::MotifBank::generate` grows a small, deterministic, related roster from a single
germ (an `identity` motif, a `hook` fragment, a `rhythmic_cell` diminution, a `bass_cell`
register-dropped opening, an optional inverted `countermotif`) — one idea in several
costumes, generated once per composition from `(scale, seed)`.

`MotifIdentity` is a transposition- and tempo-invariant fingerprint (interval contour,
normalized rhythm profile, direction signature), and `motif_similarity` scores how
related two statements are (weighted blend, length-mismatch penalized) — tested to be
≥0.9 under transposition or tempo change and <0.6 against an unrelated contour.

In `functor.rs::add_melody`, the lead voice threads one *developing* motif object across
the plan's phrases (not a fresh re-seed per section): at an `A`-family phrase, or
whenever the current object's similarity to the germ has drifted below
`1 - contract.max_transform`, it restates the identity (or, at the climax, states the
`hook`); otherwise it develops the *current* object further, bounded by the phrase's
`PhraseObligation` (`develop_current`: `Lift` transposes up, `Release` diminishes the
rhythm, `Continuation` sequences down a step, other obligations pick transposition or
augmentation). Statements enter grid-aligned (no random offset) and register comes from
the phrase's live `intent.register` (elevation, finally consulted — see §3.1), except at
the climax which is fixed to a higher octave. The lead only sounds in phrases where the
arrangement gives it an audible role, so it breathes.

Each whole statement is realized against the harmony **jointly**, not note by note:
`motif::realize_phrase` runs a bounded dynamic program over a small per-note candidate
set (chord tones on strong beats, scale tones on weak beats) that scores an entire
path at once — anchor-pitch fit, voice-leading motion between successive notes, and
(heavily weighted) agreement with the original motif's contour direction — so the
realized line stays harmonically valid while keeping the motif's shape instead of
snapping each note to the nearest chord tone in isolation.

### 3.6 Groove and bass (`groove.rs`, `functor.rs::add_bass`)

`GrooveEngine::generate` realizes a deterministic 2-bar groove cell (bar 0 the plain
statement, bar 1 the bounded variation — syncopated kick anticipations, ghost snares,
an open-hat lift — gated by energy) instead of an independent per-bar coin flip per
variation. Fills are driven by the plan: `is_fill_bar` checks whether the *next* bar is
a `phrase_end_bar` (passed in from `plan.form.phrases`), so extra fill snares land
because a phrase is ending, not because a die rolled. Micro-timing humanization stays
the one deterministic-but-seeded stochastic element (±~6 ms). The engine exports
`kick_beats` so the bass can lock onto it.

`add_bass` places a persistent, position-determined figure on every kick within a chord
span: the root on the first kick, a fifth on offbeat kicks at high energy, and a
chromatic/scale approach tone into the next chord's root on the last kick before a chord
change — a repeatable shape rather than a re-rolled choice at every onset, still
locked to the groove's real kick placements.

### 3.7 `apply_arrangement` and provenance

`functor.rs::apply_arrangement` is the pass that turns "everyone plays all the time"
into a real arrangement: for every note and drum hit it looks up the phrase it falls in
(`FormGraph::phrase_at`) and that phrase's `ArrangementRole` for its voice; a `Silent`
role drops the event outright (`Vec::retain_mut`), an audible role scales its velocity
by the role's fixed gain, and the event's provenance is restamped with the *real*
section family, phrase index, family label, role label and obligation label from the
plan — replacing a Round-I defect where provenance was hardcoded to a fixed
`SectionKind::A`.

### 3.8 `CoherenceDiagnostics` — structural measurement, not a quality score

`diagnostics.rs::CoherenceDiagnostics::measure(plan, score)` computes a vector of plain
counts and ratios against a realized `(CompositionPlan, Score)` pair: number of
recurring section families, foreground-budget collisions (should be 0), the fraction of
phrases with at least one silent voice, the fraction of multi-chord phrases that close
on a prepared Dominant→Tonic cadence, unresolved-secondary-dominant violations (should
be 0) versus resolved ones, motif restatement count, lead-note count, sounding register,
and total note/drum counts. The module's own tests include positive (a real composition
measures clean), null (an empty score measures to zeros without panicking), and mutation
controls (a hand-injected unresolved `V/x`, and a hand-built budget-violating
arrangement are both caught). This is explicitly **not** a taste or "does it slap"
metric — it cannot be, and the module doc says so directly.

## 4. Engines carried over unchanged from Round I

These pieces of the Round I vertical slice are still in place and are not part of the
Round II rewrite:

- **`world::MusicWorld`** — a skin's sonic world / local physics: harmonic vocabulary
  gates (`use_sevenths`, `allow_extensions`, `allow_chromatic_mediant`,
  `allow_modal_mixture`, `allow_secondary_dominant`), groove family (tempo, swing, hat
  subdivision), voicing spread, timbral palette, drum character and production settings.
  A world does not pick notes; it constrains the engines above. Three worlds ship:
  `BLACK_ICE` (dark A Aeolian, straight, 88 BPM), `VAPOR95` (lush F Ionian, swung,
  71 BPM), `SWISS_SIGNAL` (sparse clean C Ionian, straight, 118 BPM).
- **`form::Form`** — the world-independent section skeleton (`Intro`/`A`/
  `Development`/`Climax`/`Contrast`/`Coda`) and piecewise-linear energy/tension/density
  target curves, derived from the trace (`Form::from_trace`). `FormGraph` (§3.3) is a
  finer-grained phrase structure built alongside `Form`, not a replacement for it — the
  Score IR still carries `Form`'s section list.
- **`voicing::VoiceLeader`** — turns a chord symbol into a smoothly moving `Voicing` by
  minimizing total semitone motion from the previous voicing across three octave
  placements, clamped to a register window.
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
`motif.rs::realize_phrase_is_deterministic`).

The test suite certifies **structure and safety**, not taste: no NaN/Inf in rendered
audio, bounded peaks, exact sample counts, the category-level laws (identity-is-prolong,
composition sums cost, holonomy), voice-leading register/motion bounds, score validity
(`Score::validate`) across all three worlds, and the Round II structural invariants
described in §3 (families recur with `A'` pointing at its base, the foreground budget is
enforced, silence is present, every voice is heard somewhere, cadences prepare, secondary
dominants resolve, motif restatement happens, register tracks elevation). It does not
and cannot certify that a piece sounds good.

## 7. Honest limits (Engineering Alpha, Round II)

This is Round II of the intended v0.4.0 milestone. It fixed the causal-wiring defect
Round I shipped with, but it is candid about what it still does not claim:

- **Perceptual/aesthetic quality remains unproven.** Machine tests (§6, §3.8) can
  certify structural and safety invariants; they cannot certify that the music "slaps."
  That judgment is pending an actual human listen to the rendered WAVs — the decisive,
  still-open gate for this round.
- **Grammar-differentiated harmonic realization is not wired up.** The
  `CoherenceContract` declares a `ResolutionPolicy` per grammar (`Loop` for
  `LoopEvolution`, `ModalPedal` for `RiffDrive`, `Functional` for `HookArc` /
  `WorldSwitch`), but `HarmonyEngine::generate` does not branch on it — it always
  produces the same prepared functional-cadence shape described in §3.4, regardless of
  the declared grammar. A `LoopEvolution` or `RiffDrive` piece is currently harmonized
  identically to a `HookArc` piece.
- **`WorldSwitch` has no transport implementation.** The grammar exists as a declared
  `CompositionGrammar` variant with its own canonical contract and unit tests, but
  `CoherenceContract::infer` never selects it, and nothing in the live
  `compose`/`compose_with_plan` pipeline performs an explicit mid-piece world switch or
  builds a two-regime piece from it. It is a designed-for-later type, not a running
  feature.
- **No calibration compositions exist yet.** There is no code in the tree (as of this
  writing) that constructs dedicated `hook_arc` / `loop_evolution` / `riff_drive` /
  `world_switch` calibration probes; the only composed material comes from the shared
  `demo_trace` used by tests and the `human_music_lab` example.
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
structure (§3.8, §6); only your ears can judge the rest.
