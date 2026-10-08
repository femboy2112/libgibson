# HumanMusic beef-up — acceptance target

The mission for this round (the `feat/v0.5-humanmusic-beefup` branch): upgrade HumanMusic
so that it can

1. **cover "Attention"** (Charlie Puth) and render a WAV that **out-grooves the botched
   reference MIDI** — where that MIDI's musicians trip over each other, HumanMusic's cover
   holds a pocket;
2. **cover the other reference songs** (the `GOOD_MIDI` corpus);
3. **write new original songs** that show off the upgrades (the dials below).

> **Listening is acceptance.** The deliverable is a WAV the maintainer *hears* and judges.
> Every metric in this document is a **necessary gate, never sufficient** — it can fail a
> take, it can never certify "sounds right." No machine number overrides the ear.

## What the corpus taught us (the targets)

Measured across 11 reference songs (ABBA, Herbie Hancock, Hall & Oates, Men at Work,
MJ/Rod Temperton, 4× Rush, Rembrandts, + Charlie Puth as a negative reference), using the
calibrated instruments in `scripts/humanmusic_beefup/` and the session analysis rig.

### Coherence (the #1 finding — all songs cohere on literal long-range recurrence)

Reproducible via the committed `scripts/humanmusic_beefup/coherence_metrics.py` (consumes a local
MIDI or a `human_music_probe` TSV):

| gate | target | references (calibration) | HumanMusic today |
| --- | --- | --- | --- |
| opening lead motif restated later | **yes** | **YES** (SOS, I Can't Go For That, Off The Wall) | **no (0 / 9)** ← the gap |
| `longrange_recur` (bars that recur > 8 beats later) | ≥ 0.70 | 0.69 – 0.91 | **0.29 – 0.71**, grammar-dependent (vapor95/demo = 0.29) |
| `selfsim_peak` (bar pitch-class self-similarity) | — | ~1.0 | ~1.0 |

Real songs state a riff/hook/section and bring it back **unchanged** (literal), **transposed
to the chord** (I Can't Go For That), or **ornamented on a fixed contour** (Off The Wall).
HumanMusic through-composes and develops — the extreme non-repeating end. Closing this is the
primary coherence lever.

> **Correction (§8, measured 2026-10-08):** the earlier "`selfsim_peak` ~0.74 (OK)" figure came
> from a session-only contour measure that is **not** reproduced by the committed tool, whose
> bar-pitch-class `selfsim_peak` **saturates at ~1.0 for references *and* HumanMusic** — it does
> not discriminate, so it is not a gate. The discriminating coherence signals are **opening-motif
> return** (the real gap: refs restate, HumanMusic never does) and **`longrange_recur`** (where
> HumanMusic is in fact *lower and more variable* than previously stated — as low as 0.29). We do
> not fit the metric to the remembered number.

### Groove / pocket (two failure modes, a band in between)

The corrected `scripts/humanmusic_beefup/groove_metrics.py` (built on `hm_corpus.py`) decomposes
performed timing — **after** choosing each voice's rhythmic grid, so triplets and swing are not
charged as error — into four milibeat components (1 beat = 1000 mb; a sixteenth = 250 mb):
**band_lean** (the ensemble's shared, downbeat-anchored pulse lean), **scatter** (how much the
per-voice leans disagree), **jitter** (residual per-voice wobble after each voice's own groove is
removed), and **swing** (lawful per-phase displacement). Verdict and calibrated bands (set by the
oracle principle — above the calibration envelope, below the negative; see
`scripts/humanmusic_beefup/CALIBRATION.md`):

| state | scatter | jitter | verdict |
| --- | --- | --- | --- |
| **too rigid** (HumanMusic composed Score today) | < 3 | < 3 | RIGID — dead grid, no pocket |
| **the pocket** (calibration references) | ≤ 26 | ≤ 20 | PASS — coordinated lean / swing / complementary role leans |
| **too loose** (negative reference) | 37 | 22 | FAIL — voices trip over each other |

A real pocket is a **COORDINATED lean** — one anchor voice plus complementary leans (I Can't Go
For That: a dragged drum backbeat under a straight bass = scatter 18 with **zero** jitter;
SOS/Limelight: the whole band behind the beat together). It is **not** zero (today's robotic
state) and it is **not** independent per-voice error (the negative reference — whose failure is
dominated by **cross-voice scatter**, each voice on its own offset). The feel layer must move
HumanMusic from RIGID into PASS without ever reaching FAIL. **Note:** this instrument measures
the positions it is given; HumanMusic's composed Score is metric (hence RIGID even for BLACK_ICE,
whose accepted pocket is applied at render) — so the Wave-4 feel gate must read **performed**
positions, which needs a performed-position probe (the current `human_music_probe` dumps the
composed Score).

### Negative reference — "Attention" (`AUD_DW0160.mid`)

An almost-certainly **audio→MIDI transcription** of the record: the syncopated placements are
roughly right, but each voice carries an **independent** systematic offset (−9 … +81 mb) and
large random jitter (median 45 mb, up to 88), and nothing sits cleanly on the 16th *or* the
triplet grid. The result sounds like the band tripping over itself — the inverse of Charlie
Puth's real skill, which is deliberate syncopation that every instrument agrees on so a rigid
measure flows. **The engine's feel layer must never score in this region.** It is the
poster child for why the `rhythm::FeelTransport` design forbids independent jitter: feel is a
*coordinated* transform, not noise.

## The dials (subjective axes are tunable, never hardcoded)

Every aesthetic choice is a parameter with a sensible default, so the same engine can be
pop-repetitive or generative-transformative on request:

| dial | 0 | 1 | default |
| --- | --- | --- | --- |
| `motif_repetition` | free development | literal hook restatement (via transposed-cell / ornamented middle) | lean repetition |
| `section_return` | through-composed | verbatim section returns with arrangement gating | on |
| `harmony_mode` | static modal vamp (re-articulated) | directional functional cadence + melodic cadence-to-tonic | per-world |
| `timing_feel` | dead grid | coordinated per-voice lean/swing (bounded jitter) | small lean |
| `rhythmic_variety` | small onset/dur set | triplets / dotted / ties / weak-16th placement | mid |
| `dynamics_depth` | tiered/constant | phrase-shaped breathing + ghost-note life | mid |
| `meter` (advanced) | fixed | per-section meter + metric departure-and-return | fixed |

## Provenance / distribution note

The reference MIDIs are third-party copyrighted material and are **not committed** to the
repo (only their measured timing/coherence signatures are). HumanMusic covers are procedural
reinterpretations; rendering them locally for the maintainer's own listening is fine.
Publishing or distributing covers of copyrighted songs is a separate decision for the
maintainer, and the cover pipeline's source-custody discipline (`docs/HUMAN_MUSIC_COVER.md`)
applies if that ever happens.

## Status

- Observer (the measuring ear) Phase 0 + 1 built and green on this branch.
- Analysis rig (committed, reproducible, lattice-aware): `scripts/humanmusic_beefup/` —
  `hm_corpus.py` (honest SMF parser + timing decomposition), `groove_metrics.py`,
  `coherence_metrics.py`, `synthetic_controls.py` (§6 self-test), `smf_tests.py` (parser tests),
  `calibrate.py`, `CALIBRATION.md`. Reference corpus split frozen in `CORPUS_MANIFEST.md`.
  Engine probe: `examples/human_music_probe.rs` (promoted from the throwaway `gap_dump`).
- Dials: not yet implemented. Next: `motif_repetition` / `section_return` (the coherence
  levers), then `timing_feel`, each verified against these gates *and* the maintainer's ear.
- Ultimate acceptance test: a HumanMusic cover of "Attention" that scores PASS (pocket) on
  `groove_metrics` and that the maintainer hears flow where the MIDI trips.
