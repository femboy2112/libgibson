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

| gate | target | HumanMusic today |
| --- | --- | --- |
| `selfsim_peak` (bar-level self-similarity) | ≥ 0.65 | ~0.74 (OK) |
| `longrange_recur` (pitch-class sets that return > 8 beats later) | ≥ 0.70 | ~0.69, drops to 0.50 on some grammars |
| opening lead motif restated later | **yes** | **no (0 / 9)** ← the gap |

Real songs state a riff/hook/section and bring it back **unchanged** (literal), **transposed
to the chord** (I Can't Go For That), or **ornamented on a fixed contour** (Off The Wall).
HumanMusic through-composes and develops — the extreme non-repeating end. Closing this is the
primary coherence lever.

### Groove / pocket (two failure modes, a band in between)

Cross-voice **offset scatter** (how differently voices are shifted) and within-voice
**jitter** (note-to-note wobble), in milibeat (1 beat = 1000 mb; a sixteenth = 250 mb),
via `scripts/humanmusic_beefup/groove_metrics.py`:

| state | scatter | jitter | verdict |
| --- | --- | --- | --- |
| **too rigid** (HumanMusic today) | ~0 | ~0 | RIGID — dead grid, no pocket |
| **the pocket** (funk/pop refs) | ~1–12 | ~2–16 | PASS — coordinated lean |
| **too loose** (negative reference) | ~26 | ~45 | FAIL — voices trip over each other |

A real pocket is a **small, COORDINATED lean** — one anchor voice plus complementary leans
(Off The Wall: swung bass over straight drums; I Can't Go For That: dragged drum backbeat
under a straight bass; Tom Sawyer: everyone ~8 mb behind, together). It is **not** zero
(that is today's robotic state) and it is **not** independent per-voice jitter (that is the
negative reference). The feel layer must move HumanMusic from RIGID into PASS without ever
reaching FAIL.

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
- Analysis rig: `scripts/humanmusic_beefup/groove_metrics.py` (committed) + session instruments.
- Dials: not yet implemented. Next: `motif_repetition` / `section_return` (the coherence
  levers), then `timing_feel`, each verified against these gates *and* the maintainer's ear.
- Ultimate acceptance test: a HumanMusic cover of "Attention" that scores PASS (pocket) on
  `groove_metrics` and that the maintainer hears flow where the MIDI trips.
