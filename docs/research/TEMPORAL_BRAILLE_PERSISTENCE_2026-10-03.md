# Temporal Braille persistence — research note (2026-10-03)

**Branch:** `research/temporal-braille-persistence-2026-10-03`
**Base:** `origin/main` = `80706c91b88e59495b131bfd8ab23aa3ed013414`
**Instrument:** `examples/temporal_braille_persistence_lab.rs` (+
`examples/temporal_braille_persistence_lab/{targets,schedules,metrics,framelocal,floors}.rs`)

> This note is engineering evidence, not a perceptual claim. The perceptual
> question ("does it *look* like a 2W x 4H field?") cannot be settled from this
> machine and is marked **HUMAN UNVERIFIED** wherever it appears.

## Evidence labels

| Label | Meaning |
| --- | --- |
| **MEASURED** | A number emitted by a deterministic program run on this machine, reproducible from the commands below. |
| **SIMULATED** | A number produced by the lab's own model (e.g. the glyph coverage grammar), not by the shipping library or a human. |
| **HUMAN UNVERIFIED** | A perceptual claim that requires a human in front of a real terminal; no such observation was made. |
| **CONJECTURED** | An inference from the measurements that has not been separately tested. |
| **REFUTED** | A hypothesis the data contradicts. |

## The hypothesis under test

> Rapid *ordinary* Braille-cell `Surface`s can act as temporal pulse-density
> modulation (PDM) so that a W x H terminal behaves perceptually like a
> 2W x 4H logical field with finer apparent luminance resolution, with the
> quantization residual shaped into fine analog grain rather than coherent
> flicker.

Two sub-claims are separated throughout, because they have very different
epistemic status:

- **H1 (tonal resolution)**: time-averaging varies each *dot's* intensity and so
  adds per-dot grey levels. This is ordinary temporal dithering and is
  mathematically true.
- **H2 (new spatial positions)**: time-averaging places light *between* the fixed
  2x4 dots, i.e. adds spatial sample positions. This is **REFUTED** — see the
  reach demonstration.

## What was built (and what was not)

No renderer was added. Every emitted frame is an ordinary `gibson::Surface`; the
single existing renderer still owns terminal state. The lab reuses the shipping
primitives directly:

- `temporal::project_braille_image` / `TemporalBrailleField` for the baseline,
  static and residual algorithms;
- `SubcellGlyphMode` and the existing projector for static braille;
- `compute_diff` + `AnsiCompiler` for exact wire-byte pressure;
- `capability::quantize_color` for the live grain safety mirror.

The lab adds only deterministic schedules and measurement code. The schedules are
mask sequences fed through the same fields; the shipping `advance_*` paths are
also measured alongside them so the lab's claims are not confined to toy code.

## A. Static vs temporal reconstruction (MEASURED)

`--mode=matrix`, 56x18 cells, 12 targets.

- Static baseline (best single Braille mask + two-level colours): mean emitted
  RMSE **0.00401**.
- Best temporal schedule at K=8 subframes: RMSE **0.00277–0.00283**, a
  **~29–31 %** reduction.

| schedule | K=8 RMSE | improvement |
| --- | --- | --- |
| naive-aligned | 0.00277 | 30.9 % |
| windowed-ef | 0.00283 | 29.4 % |
| lib-full-sigma-delta | 0.00283 | 29.4 % |
| vdc-balanced | 0.00283 | 29.4 % |
| residual-windowed-ef | 0.00296 | 26.2 % |
| lib-residual-sigma-delta | 0.00294 | 26.6 % |
| stochastic-round | 0.00325 | 19.0 % |
| lib-dithered-residual | 0.00848 | −111.7 % |

Findings:

1. **MEASURED** — temporal PDM buys ~30 % RMSE over the static projector on
   this corpus. This is the software metric only.
2. **MEASURED** — the shipping *gated residual* path
   (`lib-residual-sigma-delta`) is **worse** than full sigma-delta (26.6 % vs
   29.4 %). Holding the static mask and integrating only the residual is a
   *stability* choice, not a fidelity choice. This is an honest limit of the
   existing primitive.
3. **MEASURED** — the shipping *dithered* residual path is much worse on static
   fidelity (negative improvement). Dithered residual trades accuracy for
   temporal dispersion; it should not be sold as a fidelity mode.
4. **MEASURED** — improvement grows with K and saturates by K≈6. K is bounded by
   `f_presentation / f_source`; there is no free lunch beyond that.

## B. Residual spectrum / coherence proxies (MEASURED)

`--mode=spectrum`, portrait, 1008 cells, n=256, frame-local K=4.

| schedule | lfRatio | acf1 | flashVar | dotCorr |
| --- | --- | --- | --- | --- |
| naive-aligned | 0.000 | −0.279 | 8.7e-4 | **0.576** |
| windowed-ef | 0.126 | −0.225 | 2e-6 | 0.0007 |
| lib-full-sigma-delta | 0.070 | −0.299 | 2e-6 | −0.007 |
| lib-residual-sigma-delta | 0.072 | −0.297 | 4e-6 | −0.006 |
| vdc-balanced | 0.149 | −0.167 | 4e-5 | 0.018 |
| stochastic-round | 0.251 | −0.004 | 2e-6 | 0.003 |
| lib-dithered-residual | 0.218 | −0.055 | 1e-4 | 0.082 |

`dotCorr` is the mean pairwise correlation of the eight dots *within* a cell;
`flashVar` is the variance of the whole-region lit fraction; `lfRatio` is AC
energy in the lowest temporal eighth.

Findings:

1. **MEASURED** — `naive-aligned` is the best on RMSE **and the worst on
   coherence**: all eight dots of a cell switch together (`dotCorr` 0.58),
   producing whole-cell flashing. This is exactly the coherent-flicker failure
   mode the hypothesis wants to avoid.
2. **MEASURED** — `windowed-ef` and the sigma-delta family break that coherence
   (`dotCorr` ≈ 0, `flashVar` ≈ 2e-6) at a ~0.5 % RMSE cost.
3. **MEASURED** — `stochastic-round` has the whitest lag-1 correlation (−0.004)
   but the highest low-frequency energy (0.251); it is the "grain" end of the
   spectrum.
4. **CONJECTURED** — the low `dotCorr` of WEF/sigma-delta is the mechanism that
   converts coherent flicker into dispersed grain. Human confirmation is
   **HUMAN UNVERIFIED**; only a capture/photodiode probe can settle it.

## C. Presentation-loss hostility (MEASURED)

`--mode=loss`. Under `half-2:1` (every other subframe dropped), K=4 naive-aligned
bias rises 0.035 → 0.097; K=8 rises 0.011 → 0.047. Higher K is more robust to
dropped phases. `random-10%` is nearly free; `random-20%` costs some bias.
(MEASURED, software simulation of loss — the *real* jitter/jank path is not
exercised here.)

## D. Frame-local temporal video (MEASURED)

`--mode=framelocal`, moving bar, 24 source frames, 56x18. Reset/reseed each
source frame to kill stale history.

| schedule | K=8 RMSE | improvement | plan µs/source-frame | ANSI bytes/source-frame |
| --- | --- | --- | --- | --- |
| naive-aligned | 0.00182 | 23.3 % | 55 | 103 k |
| windowed-ef | 0.00190 | 20.1 % | 85 | 241 k |
| vdc-balanced | 0.00190 | 20.1 % | 790 | 232 k |
| residual-windowed-ef | 0.00197 | 17.1 % | 188 | 221 k |
| lib-full-stateful (no reset) | 0.00212 | 10.6 % | — | — |

Findings:

1. **MEASURED** — frame-local reset makes `naive-aligned` both the cheapest and
   the most accurate. Its coherence weakness resurfaces only when the window is
   short relative to the eye; K=8 over a 30 fps source is ~7 ms, which may or may
   not be long enough. **HUMAN UNVERIFIED**.
2. **MEASURED** — `vdc-balanced` costs ~10x the planning time of WEF for the same
   RMSE. It is not justified for this use.
3. **MEASURED** — ANSI pressure is large: 100–240 kB **per source frame**, i.e.
   multiple MB/s at video rates. This is the dominant practical constraint, not
   the maths.
4. **MEASURED** — keeping the accumulator across frames (`lib-full-stateful`)
   loses ~10 points of improvement versus resetting. The shipping `ResetPolicy`
   choice is load-bearing.

## E. Reachable space / H2 (MEASURED)

`--mode=reach`.

- The 2x4 Braille dot-indicator span has rank **8 of 8**.
- Temporal masks live in `{0,1}^8`; the equal-weight K-phase average is in
  `[0,1]^8`, the convex hull of `{0,1}^8`. **No phase sequence leaves that hull.**
- Static vs K=8 vs infinite-K (free per-dot duty), per target: the static/inf-K
  ratio is **1.37–1.76** for shaped targets. *(Superseded by §E2: that number
  measured the static partition's own colour segment, not the free-duty floor,
  which is zero for collinear grayscale targets.)*

**Verdict:** **H1 confirmed**, **H2 REFUTED**. Time changes each dot's intensity,
not its position. Spatial rank stays 8 for any pulse sequence. A 2W x 4H
*spatial* field is not reachable by time alone. This is the honest ceiling on
the "2W x 4H" framing.

## E2. The infinite-K floor, rigorously bracketed and decomposed (MEASURED)

`--mode=reach`, `--mode=decompose`. The single most important correction of the
second round: the earlier "static/inf-K ratio 1.37–1.76" was **not** the free-duty
floor. It was the continuous error along the *static-optimal partition's own*
background-to-foreground colour segment. The new `floors.rs` brackets the true
floor from below (PCA affine-line residual) and above (best clamped segment),
and `--mode=decompose` splits the static error into three energy fractions:

| corpus | tonal% (H1, time removes) | colr% (colour quantization + two-colour model) | floor% (unreachable) |
| --- | --- | --- | --- |
| grayscale (11 targets) | 56.1 | 43.9 | **0.0** |
| chromatic (3 targets) | 40.7 | 47.0 | **12.3** |
| all | 49.8 | 45.2 | 5.0 |

Per-target chromatic floors: `color-ramp` 1.6 %, `chroma-zone` 9.4 %,
`chroma-edge` **35.1 %**. Grayscale targets (including every shaped target in the
corpus) have `segFree = pcaLo = 0.00000`: their eight per-cell samples are
collinear on the grey axis, so *free per-dot duty with optimal colours reaches
them exactly*. The entire remaining static error is finite-K plus 8-bit colour
quantization.

**Correction to §E:** for collinear (grayscale) targets the infinite-K floor is
**zero**, not ~1.8× below static. The practical bound on this corpus is set by
finite `K` (the matrix mode's K=8) and by 8-bit colour, not by the fixed lattice.
The fixed lattice bounds only *spatial* directions (H2), which stays refuted.

## F. Static/temporal colour tradeoff frontier (MEASURED)

`--mode=color`. A cell's background/foreground pair fixes the *line* along which
per-dot duty moves, and the shipping projector picks that pair to minimise the
**static** error — which is not the pair that minimises the temporal floor. For a
range of allowed static-inflation factors `mu`, the mode picks, per cell, the
lowest-floor pair whose static error stays within `mu` times the projector's.

Mean over all targets (RMS, and fraction of static error energy recovered):

| mu | floor RMS | committed static RMS | floor/emit | recov% |
| --- | --- | --- | --- | --- |
| 1.00 | 0.01321 | 0.01865 | 100.0 % | 0.0 |
| 1.25 | 0.01268 | 0.01896 | 92.2 % | 3.9 |
| 1.50 | 0.01226 | 0.01954 | 86.1 % | 6.9 |
| 2.00 | 0.01140 | 0.02116 | 74.5 % | 12.8 |
| 5.00 | 0.00992 | 0.02692 | 56.4 % | 21.8 |
| inf | 0.00899 | 0.09083 | 46.3 % | 26.9 |

**MEASURED** — the static-optimal colour choice leaves ~44 % of the grayscale
temporal error energy on the table; a temporal-first choice can recover it, and
the frontier prices the static fallback. The knee is at `mu ≈ 1.5–3` (recover
7–19 % of error energy for a 5–29 % static inflation). `mu = inf`
(black/white, the extreme) destroys the static fallback and is not a
recommendation. This is a concrete optimisation for a temporal-first renderer
that keeps an exact static fallback.

## A2/B2. Fidelity spread and the coherence Pareto frontier (MEASURED)

`--mode=montecarlo --k=8 --seeds=48` and `--mode=pareto --k=8 --n=512`. Every
schedule except `naive-aligned` is a deterministic function of its seed, so the
headline improvement carries a seed-to-seed spread:

| schedule | mean RMSE | std | impr% | win% |
| --- | --- | --- | --- | --- |
| naive-aligned | 0.00277 | 0.000000 | 30.87 | 100 |
| lib-full-sigma-delta | 0.00283 | 0.000003 | 29.46 | 0 |
| windowed-ef | 0.00283 | 0.000003 | 29.43 | 0 |
| vdc-balanced | 0.00283 | 0.000003 | 29.43 | 0 |
| residual-windowed-ef | 0.00295 | 0.000005 | 26.30 | 0 |
| lib-residual-sigma-delta | 0.00295 | 0.000005 | 26.36 | 0 |
| stochastic-round | 0.00324 | 0.000018 | 19.25 | 0 |
| lib-dithered-residual | 0.00860 | 0.000085 | −114.56 | 0 |

The fidelity spread is tiny for the balanced schedules (std ~3e-6), so their
~29.4 % is a solid number. But fidelity alone misleads, because the winner is
the coherent-flicker loser. The Pareto frontier (max improvement, min `dotCorr`,
min `flashVar`):

| schedule | impr% | dotCorr | flashVar | Pareto |
| --- | --- | --- | --- | --- |
| naive-aligned | 30.87 | 0.5051 | 8.9e-4 | **YES** |
| windowed-ef | 29.44 | 0.0009 | 3e-6 | **YES** |
| lib-full-sigma-delta | 29.44 | −0.0050 | 2e-6 | **YES** |
| stochastic-round | 19.43 | 0.0028 | 2e-6 | **YES** |
| residual-windowed-ef | 26.29 | 0.0117 | 2e-5 | no |
| vdc-balanced | 29.41 | 0.0029 | 7e-6 | no |
| lib-residual-sigma-delta | 26.38 | −0.0045 | 3e-6 | no |
| lib-dithered-residual | −114.66 | 0.0816 | 5.2e-5 | no |

**MEASURED** — the shipping `lib-full-sigma-delta` sits on the Pareto frontier:
it matches the best fidelity (29.4 %, within the seed noise of `windowed-ef`)
with near-zero coherence (`dotCorr` −0.005). The honest headline is the frontier,
not the single best RMSE: `naive-aligned` buys its extra 1.4 points with a
`dotCorr` of 0.51.

## Live harness (HUMAN UNVERIFIED)

`--mode=live` runs an A/B/grain demo on a real terminal: AUTO-cycling
STATIC / TEMPORAL-FIDELITY / TEMPORAL-GRAIN. The grain arm mirrors the shipping
safety gate (`quantize_color` + bounded per-cell luminance swing) and falls back
to static when the measured `PresentationProfile` is not `Enabled`. It was **not
run** here (no interactive TTY in this environment), so every perceptual
statement remains **HUMAN UNVERIFIED**.

The cheapest falsifying probe that would upgrade these labels: run
`--mode=live` behind `temporal_cadence_beacon` and capture with a camera or
photodiode; compare coherent-flicker visibility between `naive-aligned` and
`windowed-ef` at the same RMSE. Until then, the perceptual half of the hypothesis
is unproven.

## Reproduction

```sh
cargo build --release --example temporal_braille_persistence_lab
./target/release/examples/temporal_braille_persistence_lab --mode=matrix
./target/release/examples/temporal_braille_persistence_lab --mode=montecarlo --k=8 --seeds=48
./target/release/examples/temporal_braille_persistence_lab --mode=spectrum
./target/release/examples/temporal_braille_persistence_lab --mode=pareto --k=8 --n=512
./target/release/examples/temporal_braille_persistence_lab --mode=loss
./target/release/examples/temporal_braille_persistence_lab --mode=framelocal
./target/release/examples/temporal_braille_persistence_lab --mode=reach
./target/release/examples/temporal_braille_persistence_lab --mode=decompose
./target/release/examples/temporal_braille_persistence_lab --mode=color
cargo test --example temporal_braille_persistence_lab   # 32 tests
```

## Bottom line

- The **tonal** half of the hypothesis is real and MEASURED (~30 % RMSE over
  static). The infinite-K floor for grayscale is **zero** (collinear samples);
  the practical limits are finite `K` and 8-bit colour, not the lattice.
- The **spatial** half is REFUTED: rank stays 8; no new dot positions.
- The **grain vs flicker** half is MEASURED as a spectral difference
  (`dotCorr` 0.58 → ~0) and as a fidelity/coherence Pareto frontier on which the
  shipping full sigma-delta is optimal; but the *perception* remains **HUMAN
  UNVERIFIED**.
- A temporal-first colour choice recovers ~44 % of the grayscale temporal error
  energy the static-optimal colours leave behind; the frontier prices the cost.
- The dominant real-world cost is ANSI bandwidth and the presentation-loss
  sensitivity of the duty estimate, not the reconstruction maths.
