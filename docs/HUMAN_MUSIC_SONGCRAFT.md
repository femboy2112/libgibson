# HumanMusic songcraft — the ear-first ladder, the dials, and the Rick-level target

Record of the hand-authored Score-IR listening ladder on `feat/v0.5-humanmusic-beefup` that
established, rung by rung by the maintainer's ear, what a *coherent song* needs — and the next
frontier. The probes bypass the generator and render hand-authored songs through the engine's own
synth/worlds, so each rung is a controlled A/B that holds the song fixed and changes **one** axis.

> **Listening is acceptance.** Every rung's verdict is the maintainer *hearing* a WAV and judging.
> Machine metrics are necessary gates, never sufficient — they can fail a take, never certify
> "sounds right." No number overrides the ear.

## The ladder (all ear-confirmed, monotonic improvement)

| Rung | Probe | Ingredient proven | Verdict |
| --- | --- | --- | --- |
| 1 | `song_form_probe` | literal returning verse/hook over a cycling loop | "much better, but still hollow" — song-form is real but not the disease |
| 2 | `ensemble_probe` | **interdependent voicing** — no voice states a full chord; quality emerges across voices | "much better… the right direction" — common-mode agreement was the disease |
| 3 | `groove_probe` | groove as **one shared pocket** (kick/hats anchor, band leans together ~13ms; no per-voice swing) | "much better" — a per-instrument-clock swing *galloped* and was rejected |
| 4 | `songcraft_probe` | germ melody (verse asks / hook answers), a bridge, structure-coherent dynamics | **Morty-level achieved** (see dials below) |

The root cause rung 2 fixed: the engine composed adaptive *underscore*, where voices redundantly
agree on one stated harmony ("common-mode"). The fix — voices that *complete each other* — is
**load-bearing** and stays central in every rung above it.

## `songcraft_probe` — the dials (rung 4)

Same confirmed backing (interdependent voicing + shared pocket), plus: one germ ("reach upward
through the chord") realized as a verse **antecedent** (over-reaches, ends unresolved — a question)
and a hook **consequent** (resolves home — the answer); a **bridge** on Dm–F–C–**E** (the colored
dominant, raised G# → a V-ish pull home) that climbs into a biggest final chorus; and velocity
shaping made **coherent over song structure** (phrase arc **plus** section terrace: verse 0.90 /
chorus 1.00 / bridge 0.95→1.05 / final chorus 1.12).

Three single-axis isolation dials, each ear-ruled against the shaped/pocket/resolved reference:

| Dial | Flag | Verdict | Default |
| --- | --- | --- | --- |
| Velocity shaping | `--flatvel` (flatten lead, keep grace) | **shaped > flat** — shaping earns its place | **shaped** |
| Micro-timing pocket | `--grid` (zero the lean) | either-ok **dial**; grid is rigid, pocket "more natural" but can de-emphasize drums (a mix/taste concern) | **pocket** |
| Ending | `--open` (final hook hangs on D, outro on B) | both good — a **dial** | **resolve home** |
| Bridge | `--no-bridge` (40-bar form) | — | **bridge** |

**Calibration (important).** Flat velocity and dead-grid timing are *limitations the maintainer hit*,
not aesthetic verdicts — so the engine should **exceed** them, as additive features done coherently
over song structure (structured/relational only, **never random onset jitter**). Harmony is **color
used functionally** — V-ish→I-ish, because bare V–I is uninteresting; the function is present, the
color makes it so. (Dial priorities were informed by a study of the maintainer's own compositions,
kept out of this repo as private material, plus the `GOOD_MIDI` reference corpus.)

## Reproduce

```
cargo +1.99.0 run --release --example songcraft_probe -- \
  --out=target/humanmusic-beefup/songcraft [--world=black_ice|vapor95|swiss_signal] \
  [--flatvel] [--grid] [--open] [--no-bridge]
```

Render → `HumanMusicSynth` + `OfflineRenderer` → `write_wav_i16`; `black_ice` is the apples-to-apples
voice the maintainer judges. These are listening controls, **not shipped** — examples excluded from the
published crate.

## Next: Rick-level (the north star)

Morty-level is *correct but sparse* — it reads as "generic mainstream music, in a good way." The gap to
Rick-level ("Casiopea-level") is **semantic-information throughput: units of semantically *coherent*
musical information per unit time.** Casiopea holds the same coherence at many times the density — the
bass is a melody, the comp is counterpoint, the harmony reharmonizes under a line that is still singing,
and none of it turns to mush.

**Hard constraint: raising throughput must NOT decohere the global song structure.** Naively adding notes
/ faster chords / busier drums is exactly how the engine regresses to "a machine mimicking music." The
density must be **structurally bound** — subordinated to the form. Candidate coherence-preserving channels
(density that can't decohere because each unit is bound to the structure):

- **Complementary rhythm** — interlocking subdivisions (¼ / 8th / 16th / half-time), each voice owning a
  layer, more events with zero collision.
- **Deferred / inherited harmony** — the 3rd arrives a bar late; a held common tone changes function under
  a moving chord — more harmonic information without more simultaneous notes.
- **Overlapping-sustain motion** — continuous harmonic motion out of staggered releases.
- **Re-rooting / planing / sequencing** — more harmonic events bound to a cell that still reads as *one* thing.

This is the layer beneath the five note-level gaps in the beef-up analysis (thin rhythm vocabulary, static
harmony, no developed restatement, flat inter-voice activity). The approach stays the same: an ear-first
ladder, each rung proving added density *without* decoherence.
