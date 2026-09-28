# HumanMusic: the `gibson::audio` architecture (experimental)

This documents LibGibson's new **audio realization axis** — `gibson::audio` and, on top
of it, the **HumanMusic** procedural composition engine (`gibson::audio::human_music`).
It is a research/engineering-alpha subsystem, Rust-only, not yet part of the C ABI. Read
this alongside the module docs in `src/audio/mod.rs` and `src/audio/human_music/mod.rs`,
which this document expands on.

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
layer it's built from. HumanMusic sits on top of both.

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

## 3. The categorical model

This is the heart of HumanMusic. It is not decorative category theory: the types and
laws below are load-bearing and tested (`src/audio/human_music/intent.rs`,
`src/audio/human_music/functor.rs`).

### The semantic category 𝒮

Objects are LibGibson presentation/story states — locally, `SemanticState { tone,
emphasis, density, elevation }` plus an event kind (`EventKind`) on the transitions
between them (mirroring `gibson::ui`'s `Tone`/`Emphasis` axes, kept self-contained so the
audio crate has no dependency on the UI layer). Morphisms are *meaningful* transitions,
never frame ticks.

**Identity morphism = prolongation.** A settled UI generates no musical novelty just
because time passed; the module doc states this directly and `IntentMorphism::Prolong`
(§ below) realizes it — applying it barely changes the running intent.

### The musical-intent category 𝓜

Objects are `MusicIntent`:

```rust
pub struct MusicIntent {
    pub energy: f32,        // overall drive [0,1]
    pub tension: f32,       // harmonic/melodic instability [0,1]
    pub density: f32,       // event density [0,1]
    pub register: f32,      // register bias [0,1]
    pub function: Function, // current harmonic function
    pub motif: MotifState,  // motif development (path memory)
    pub expectation: f32,   // accumulated expectation / harmonic pull [0,1]
}
```

This is deliberately far richer than "the current chord" — it carries accumulated
expectation and motif development as first-class path memory, not just a snapshot.

Morphisms are the typed transformations in `IntentMorphism`: `Prolong`, `Prepare`,
`Intensify`, `Relax`, `Suspend`, `Pivot`, `Resolve`, `Modulate`, `Reharmonize`,
`FragmentMotif`, `SequenceMotif`, `Augment`, `Diminish`, `Syncopate`, `ThinTexture`,
`ThickenTexture`, `Cadence`.

Hom-sets are **enriched** over a `MorphismCost` vector, not a single opaque scalar:

```rust
pub struct MorphismCost {
    pub voice_leading: f32,
    pub tension_error: f32,
    pub register_violation: f32,
    pub parallel_motion: f32,
    pub groove_disruption: f32,
    pub motif_loss: f32,
    pub novelty: f32,
    pub repetition: f32,
}
```

`MorphismCost::combine` sums two costs — **composition of morphisms adds cost** — and
`intent::compose()` threads a `MusicIntent` through a sequence of morphisms while
accumulating that sum. A `CostWeights` struct collapses the vector to a scalar for greedy selection
when a planner needs one.

### Path dependence / holonomy

Because `MusicIntent` carries accumulated `expectation` and `motif.development`, a
journey `A → B → C → A` is **not** `identity(A)`. Returning to the same harmonic
function label after a journey leaves a different phenomenological state behind: more
expectation released, more motif development accrued. This is a deliberate design
property, not an artifact, and it is directly tested
(`intent.rs::holonomy_returning_is_not_identity`): composing `Modulate → SequenceMotif →
Modulate → Resolve` from the default intent returns to `Function::Tonic` (the same
"chord label") but is `assert_ne!` to the starting `MusicIntent`, and its motif
development is strictly greater.

### The functor F_skin : 𝒮 → 𝓜

The central construction (`functor.rs`) is a (lax-monoidal) functor `F_skin`,
parameterized by a `MusicWorld`, that turns a `SemanticTrace` into a `Score`. Walking the
trace, each semantic event maps (via `event_to_morphisms`) to a sequence of
`IntentMorphism`s that evolve a running `MusicIntent`; those choices drive motif
development, SFX placement and local density.

Concretely, `compose()` builds one `Form` (structural plan) and one `HarmonyEngine`
progression, then derives *all four* simultaneous voices — comp (pad + keys), bass,
melody, and SFX/provenance — from that shared form and progression. Simultaneous
semantic layers (tone + emphasis + density + elevation) map to simultaneous musical
voices (harmony + bass + drums + melody) through this one shared coherence map, not as
independent sums per layer.

### Skins as natural transformations

Swapping the `MusicWorld` is a natural transformation, not a new composition: the same
semantic trace under BLACK_ICE / VAPOR95 / SWISS_SIGNAL preserves the same high-level
structure — identical section skeleton, motif identity, resolutions — while sounding
like three different dialects. This is verified directly:
`functor.rs::same_trace_gives_same_form_across_worlds` asserts the three worlds' section
kind sequences are pairwise equal while their tempos differ. The form skeleton itself is
derived from the trace alone (`Form::from_trace` takes no `MusicWorld` argument), so it
is world-independent by construction; only the harmonic/timbral/rhythmic realization on
top of it differs per world.

## 4. The engines

Each engine below is deterministic given its inputs and a seed, and is unit-tested in
its own module.

- **`world::MusicWorld`** — a skin's sonic world / local physics. A world does **not**
  pick notes; it *constrains* the composition engines: harmonic vocabulary
  (`use_sevenths`, `allow_extensions`, `allow_chromatic_mediant`, `allow_modal_mixture`,
  `allow_secondary_dominant`), groove family (tempo, swing, hat subdivision), voicing
  spread, timbral palette (`Patch`es for pad/bass/lead/keys), drum character (kick/snare
  params, hat cutoff, ghost amount, drum density) and production (reverb, saturation,
  master ceiling, bus mix levels). Round I ships exactly three worlds: `BLACK_ICE` (dark
  A Aeolian, straight, 88 BPM, full chromatic vocabulary), `VAPOR95` (lush F Ionian,
  swung, 71 BPM, big reverb), and `SWISS_SIGNAL` (sparse clean C Ionian, straight,
  118 BPM, restrained triadic vocabulary, open voicings).
- **`form::Form`** — the global plan: world-independent sections (`Intro`, `A`,
  `Development`, `Climax`, `Contrast`, `Coda`) plus piecewise-linear energy/tension/
  density *target* curves, derived purely from the trace via `Form::from_trace`. Each region
  between consecutive semantic events becomes a section; the section with the highest
  semantic pressure (`SemanticState::pressure()`) becomes the Climax, the first region is
  the Intro, the last is the Coda. This is the long-range plan that keeps local
  reactivity from destroying structure — a warning event may perturb a phrase, but it
  must not modulate the whole song.
- **`harmony::HarmonyEngine`** — generates a functional chord progression over a `Form`.
  Diatonic chord quality is classified from the actual stacked scale thirds (not a
  fixed major/minor table), so it comes out correct in any mode. Harmonic function
  (`Function::Tonic`/`Predominant`/`Dominant`) tracks the form's tension curve, and
  phrase ends always cadence to tonic. World-gated embellishments — secondary dominants,
  modal mixture (borrowed bVI), chromatic mediants — color richer worlds and never occur
  at a cadence.
- **`voicing::VoiceLeader`** — turns a bare chord symbol into an explicit, smoothly
  moving `Voicing`. It builds a close-position voicing of chord tones near a target
  center (preferring common tones with the previous chord), searches three octave
  placements (down/same/up) and picks the one minimizing total semitone motion from the
  previous voicing, clamps the result to a register window, and opens the spacing for
  high-`voicing_spread` worlds.
- **`motif::Motif`** — a scale-degree contour + parallel rhythm vector sharing a stable
  `id`. Transformations (`transpose`, `invert`, `retrograde`, `scale_rhythm` for
  augmentation/diminution, `fragment`, `sequence`) all preserve that identity while
  developing the material, so the melody engine can grow one idea across a whole piece
  instead of inventing a new tune every few bars.
- **`groove::GrooveEngine`** — an interlocking kick/snare/hat pattern with an accent
  hierarchy, deterministic microtiming (humanization, not jitter), ghost notes,
  syncopation and phrase-end fills, all scaled by the form's energy/density curve so the
  pattern thins in the intro and thickens into the climax without a new pattern being
  invented per bar. World `swing` delays odd hat subdivisions. Crucially, the groove
  engine exports `kick_beats` so the bass engine can lock its note onsets to the kick
  (`add_bass` in `functor.rs`), giving a genuine rhythm-section interlock rather than
  independently-generated bass and drums.
- **Bass** (part of `functor.rs::add_bass`) — kick-locked: a bass note lands on every
  kick within a chord span, choosing between the root, a fifth (for drive at high
  energy), or a chromatic/scale approach tone into the next chord's root.
- **The Score IR (`score::Score`)** — an inspectable, provenance-tagged intermediate
  representation: notes (`Role::Pad/Bass/Lead/Keys`), drum hits, SFX events, chord spans
  and sections, each carrying a `Provenance` (section kind, motif id, morphism label,
  role note) so a rendered piece's choices can be traced back to the semantic events and
  category-level morphisms that produced them.
- **The Synthesizer (`synth::HumanMusicSynth`)** — realizes a `Score` to audio via the
  DSP layer. It implements the `AudioSource` trait, so the deterministic offline
  renderer and (under `audio-cpal`) a real device drive the *same* code — one
  synthesizer, not a test double. Per-role voice pools plus synthesized drum voices
  (`Kick`/`Snare`/`Hat`/`Clap`) are triggered as the render loop's monotonic playhead
  crosses pre-scheduled event times; world production (saturation → reverb → bus
  compression → limiter) is applied before writing the master block.

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
asserts identical output (see `functor.rs::deterministic_for_seed`,
`harmony.rs::deterministic_progression_for_seed`, `groove.rs::velocities_are_bounded_and_deterministic`).

The test suite certifies **structure and safety**, not taste: no NaN/Inf in rendered
audio, bounded peaks, exact sample counts, the category laws above (identity-is-prolong,
composition sums cost, holonomy), voice-leading register/motion bounds, and score
validity (`Score::validate`) across all three worlds. It does not and cannot certify
that a piece sounds good.

Measured receipts from the lab example, `examples/human_music_lab.rs` (one frozen
`demo_trace`, three worlds, 48 kHz/16-bit stereo):

- All three renders produce valid WAVs with the same section skeleton but distinct
  spectral centroids: VAPOR95 ≈ 3787 Hz < BLACK_ICE ≈ 5452 Hz < SWISS_SIGNAL ≈ 6321 Hz.
- No clipping.
- Roughly 20–23 simultaneous voices at peak density.
- Offline render runs 27–42× realtime.

## 7. Honest limits (Engineering Alpha)

This is Round I of the intended v0.4.0 milestone, and it is candid about what it does
not yet claim:

- **Perceptual/aesthetic quality is unproven.** Machine tests can certify structural and
  safety invariants; they cannot certify that the music "slaps". That judgment is
  pending an actual human listen to the rendered WAVs.
- The harmony, groove and production choices are a first coherent pass — genuinely
  functional and voice-led, not a finished, polished score.
- The model is experimental and expected to evolve between releases.
- It is Rust-only and intentionally not exposed through the C ABI this milestone.
- The package version stays `0.3.1` until this work is reviewed and a release is cut;
  nothing here has shipped yet.

## 8. How to run

```
cargo run --release --example human_music_lab
```

Writes `black_ice.wav`, `vapor95.wav` and `swiss_signal.wav` (one per world) to a temp
directory by default, along with a structural summary and render receipts printed to
stdout. Flags:

- `--world=black_ice|vapor95|swiss_signal` — render only one world (default: all three).
- `--out=PATH` — output directory (default: a `libgibson_human_music` folder under the
  system temp dir).
- `--seed=N` — composition seed (default `2112`).
- `--beats=N` — length of the demo semantic trace in beats (default `120`).

The point of the lab, per its own doc comment: the three WAVs should be recognizably the
*same* music in form and meaning — same section skeleton, same motif identity, same
resolutions — while sounding like three different dialects. Machines can certify the
structure; only your ears can judge the rest.
