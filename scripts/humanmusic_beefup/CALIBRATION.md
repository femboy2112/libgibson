# Groove / coherence instrument — calibration record

How the corrected measurement rig was made trustworthy before it was used as a gate, and why its
bands sit where they do. Non-reconstructive: aggregate per-reference numbers only, no note content.

## The oracle principle

The reference corpus was chosen because it **sounds coherent** — the heard experience is ground
truth; these metrics are a downstream lens. So the ruler is set to **agree with the ear**: if the
corrected metric scored a heard-coherent reference as out-of-pocket, that refutes the **metric**,
not the reference. Concretely: every calibration reference must land **non-FAIL**, and only the
declared negative ("Attention") may FAIL. A threshold that would reject a reference is wrong.

## The four bugs the old `groove_metrics.py` had (demonstrated, then fixed)

| bug | what it did | fix |
| --- | --- | --- |
| **A drums dropped** | SMF channel 9 and TSV `kind=drum` were discarded, so backbeat-vs-band pocket was invisible | drums parsed, split by GM lane (kick / snare / hats / toms / perc) as their own voices |
| **B 16th-residual = "jitter"** | `onset − round(onset·4)/4` charged triplets, swing and dotted figures as jitter | each voice's **grid is chosen first** (16th / 8th-triplet / 16th-triplet); deviation is measured against the voice's own lattice; swing is a per-metric-phase systematic, not jitter |
| **C common lean invisible** | a band sitting +8 mb behind *together* read scatter≈0/jitter≈0 → "RIGID" | **band lean** (phase-independent, anchored to the downbeat) is a first-class output; +8-together now reads COORDINATED LEAN, not rigid |
| **D track=role fiction** | voices were implicitly treated as bass/keys/lead | voice key is pure `t{track}c{channel}` / `drum:{lane}` provenance; no musical-role label is asserted |

## Synthetic controls (§6) — the instrument must classify intent

`synthetic_controls.py` (runs as a self-test, exits non-zero on misclassification):

| control | B | scatter | jitter | swing | verdict | required |
| --- | --- | --- | --- | --- | --- | --- |
| A perfect-quantized | +0.0 | 0.0 | 0.0 | 0.0 | RIGID | RIGID |
| B whole-band +8 mb | +8.0 | 0.0 | 0.0 | 0.0 | PASS | coordinated lean |
| C shared swing | +0.0 | 0.0 | 0.0 | 60.0 | PASS | lawful groove |
| D role leans | +2.8 | 4.6 | 0.0 | 0.0 | PASS | pocket |
| E independent ±40 mb | +0.8 | 11.2 | 38.7 | 79.1 | FAIL | sloppy |
| F mixed 16th+triplet | +0.0 | 0.0 | 0.0 | 0.0 | RIGID | **not FAIL**, triplets recognised (`grids={q16:2, trip8:2}`) |

## Calibration readings (the oracle) — milibeat

`calibrate.py` on the 7 calibration references + the negative (holdout **never** touched):

| reference | verdict | band_lean | scatter | jitter | swing |
| --- | --- | --- | --- | --- | --- |
| ABBA — SOS | PASS | −11.0 | 8.4 | 13.1 | 0.0 |
| Hall & Oates — I Can't Go For That | PASS | −0.1 | 18.1 | 0.0 | 0.0 |
| Men at Work — Overkill | RIGID | +0.5 | 1.6 | 0.0 | 0.0 |
| MJ — Off The Wall | RIGID | +0.0 | 0.0 | 0.0 | 0.0 |
| Rush — Limelight | PASS | −22.0 | 10.2 | 8.1 | 0.0 |
| Rush — Subdivisions | PASS | +0.0 | 0.0 | 9.2 | 0.0 |
| Rush — Tom Sawyer | PASS | −2.1 | 4.2 | 15.2 | 0.0 |
| **Charlie Puth — Attention (NEG)** | **FAIL** | +14.2 | **37.1** | **21.7** | 39.6 |

Calibration envelope: **max scatter 18.1, max jitter 15.2**. The ceilings are set ABOVE it and
BELOW the negative: **`SCATTER_CEIL = 26`, `JITTER_CEIL = 20`, `RIGID_FLOOR = 3`**.

### What this revealed

- **Scatter is the strong discriminator.** Attention's scatter (37.1) is ~2× the most-spread
  reference (ICGFT 18.1), while its jitter (21.7) barely clears the band. This matches the
  diagnosis: Attention's pathology is **independent per-voice offsets** (each musician off in its
  own way), which the decomposition reads as cross-voice scatter — not primarily within-voice wobble.
- **I Can't Go For That at scatter 18.1 is a pocket, not a trip** — the famous dragged backbeat
  under a straight bass is a large but *intentional* differential lean, with **zero jitter**. An
  earlier naive ceiling of 18 would have rejected it; the oracle says raise the ceiling.
- **Two references read RIGID** (Overkill, Off The Wall) and that is **not** a contradiction with
  "sounds coherent": groove-pocket and song-coherence are different axes. Overkill is a
  step-programmed transcription with little micro-timing. Off The Wall is the one the instrument
  **cannot fully account for** — its known swung bass gets absorbed into a triplet-grid
  classification (read as lawful on-grid placement rather than as swing displacement), so its swing
  is invisible to this instrument. That is a reported **instrument limit**, not a judgement on the
  song.

## HumanMusic baseline (composed Score, via `human_music_probe`)

Across 3 worlds × 3 story traces @ seed 2112: **RIGID** (structure < 3 mb, jitter < 3 mb),
drums included. Note: `black_ice` — which carries the accepted R17 pocket — **also reads RIGID
here**, because that pocket is applied at **render**, downstream of `compose()`; the probe dumps
composed (metric) positions. **Measuring performed feel (Wave 4) requires a performed-position
probe**, not this one. HumanMusic today is the opposite failure from Attention: dead grid, not loose.
