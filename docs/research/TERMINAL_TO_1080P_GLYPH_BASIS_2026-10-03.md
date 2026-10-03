# Terminal -> 1080p: minimal glyph basis — research note (2026-10-03)

**Branch:** `research/temporal-braille-persistence-2026-10-03`
**Base:** `origin/main` = `80706c91b88e59495b131bfd8ab23aa3ed013414`
**Instrument:** `examples/terminal_to_1080p_lab.rs` (+
`examples/terminal_to_1080p_lab/{raster,basis}.rs`)

This is the addendum to the temporal Braille note. The temporal note established
that time alone cannot add spatial sample positions to the fixed 2x4 Braille
lattice. This note asks the next question:

> Beyond Braille, what is the **smallest** set of glyphs that adds genuinely new
> spatial directions rather than re-expressing directions Braille already has?

It answers it as linear algebra over a documented coverage model. It is **not** a
Unicode brute-force and it is **not** a claim about any particular font.

## Evidence labels

Same legend as the temporal note: **MEASURED**, **SIMULATED** (model output),
**HUMAN UNVERIFIED**, **CONJECTURED**, **REFUTED**. Everything below is
**SIMULATED** unless marked otherwise; the model is stated so it can be attacked.

## The model, stated plainly

`terminal_to_1080p_lab/raster.rs` defines a **coverage grammar**, not a
rasterizer. A cell is sampled on a fixed P = W x H grid (default 8x16). Each
glyph is a coverage vector in `[0,1]^P`.

- Braille's eight dots are modelled as **round, gapped discs** (default radius
  1.5 samples in a 4x4 dot region), *deliberately* not tiling the cell.
- The candidate bank covers blocks, quadrants, fractional blocks, shades, thin
  box rules, diagonals, sextants, octants and a few geometric shapes — all
  portability-tagged (universal / graphical / low).

The round-gapped-disc choice is the model's most load-bearing assumption and is
treated as a calibration parameter, swept in the ablations. A real font
calibration would substitute measured glyph rasters; the code exposes the shape
so a calibration can be dropped in.

Two reconstruction models are used, both in coverage space:

- **static**: best single glyph plus a two-level fill (`b + a*g`, both clamped to
  `[0,1]`) — one cell, one instant, the per-cell colour model.
- **temporal**: best point of the glyph convex hull plus the same two-level fill
  (Frank-Wolfe with exact line search) — the infinite-K time-multiplexing limit.

## Rank of the Braille dot basis (SIMULATED)

`--mode=rank`.

- Numerical rank **8 of 8**; all Gram eigenvalues **4.0** — the eight dot blobs
  are mutually orthonormal in coverage. Braille already spans all eight
  independent per-dot directions.
- Principal angle of a candidate to that span:

| glyph | family | angle | out-of-span | portable |
| --- | --- | --- | --- | --- |
| left-1/8 | fraction-block | 90.0° | 100 % | universal |
| hbar / vbar | box | 90.0° | 100 % | universal |
| left-1/4 | fraction-block | 69.3° | 87.5 % | universal |
| diag-fwd / diag-back | diagonal | 68.6° | 86.7 % | universal |
| left-3/4 | fraction-block | 62.8° | 79.2 % | universal |
| shades / halves / quadrants | block / shade | 60.0° | 75.0 % | universal |

## The wall, and the fair comparison (SIMULATED)

`--mode=wall` on a 29-tile target corpus (edges, rules, rings, zones, checkers,
noise):

| family | residual energy outside the dot span |
| --- | --- |
| checker | 80.6 % |
| line | 78.1 % |
| zone | 76.4 % |
| noise | 75.9 % |
| edge | 75.3 % |
| curve | 72.9 % |

Because the modelled dots are small gapped discs, most full-cell target energy
lies in the gaps and is unreachable by dots regardless of timing. Static and
temporal errors are therefore **almost identical** within the Braille-only basis
(e.g. edge 0.3535 → 0.3519). **MEASURED-in-model conclusion: on full-cell shapes,
temporal modulation of Braille buys almost no extra *spatial* reach.** Its value
is tonal (the other note) or, when no per-cell background colour is available, as
a poor man's background.

## Lattice equivalence: what is fill vs what is shape (SIMULATED)

`--mode=lattice` projects each glyph out of `span(dots ∪ uniform fill)` and
reports the remaining energy (`shapeOut%`). A glyph that is just a fill/dot
variant scores ~0; a genuinely new direction scores high.

At the default radius (1.5), and robustly across a calibration sweep:

| glyph | r=1.0 | r=2.0 | r=2.4 (tiling) |
| --- | --- | --- | --- |
| full-block | 0 % | 0 % | 0 % |
| shades | 0 % | 0 % | 0 % |
| upper/lower/left/right half | 37.5 % | 12.5 % | 0 % |
| quadrants | 56.2 % | 18.8 % | 0 % |
| left-1/8 | 83.3 % | 79.2 % | 75.0 % |
| left-1/4 | 68.8 % | 56.2 % | 50.0 % |
| hbar / vbar | 83.3 / 66.7 % | 79.2 / 66.7 % | 75 / 75 % |
| diag-fwd | 69.9 % | 56.3 % | 58.7 % |

**MEASURED-in-model, calibration-robust conclusion:** coarse blocks, quadrants
and shades are *fill variants* (they collapse to 0 % as the dots tile the cell).
Fine fractional blocks, thin rules and diagonals stay 50–87 % out of span at
every radius — those are the glyph families that add genuinely new *edges and
lines*, not just fill.

## Minimal augmentation search (SIMULATED)

`--mode=search` — greedy orthogonal matching pursuit over the 29-tile corpus,
train/holdout split by target-family instance, with the uniform-fill direction
already in the seed span (so a full block is correctly rejected as pure fill).
Selected set, 6 glyphs:

| # | glyph | family | portable | holdout static | holdout temporal |
| --- | --- | --- | --- | --- | --- |
| 1 | lower-half | block | universal | 0.2835 | 0.2804 |
| 2 | diamond | geometric | graphical | 0.2787 | 0.2693 |
| 3 | lower-1/4 (U+2583) | fraction-block | universal | 0.2784 | 0.2640 |
| 4 | lower-3/4 (U+2581) | fraction-block | universal | 0.2784 | 0.2600 |
| 5 | diag-fwd (U+2571) | diagonal | universal | 0.2782 | 0.2591 |
| 6 | right-half (U+2590) | block | universal | 0.2680 | 0.2391 |

**MEASURED-in-model:** the minimal set that actually moves holdout error is
dominated by **fractional blocks + diagonals + a coarse fill split**; the
geometric diamond is not portable and earns its place only on this corpus.

## Ablations (SIMULATED)

`--mode=ablations`, held-out tiles, fixed augmentation set
`{left-1/8, left-3/4, diag-fwd, diag-back, hbar, vbar}`:

| basis | static RMSE | temporal RMSE |
| --- | --- | --- |
| Braille only | 0.31597 | 0.31532 |
| augmentation only | 0.32330 | 0.30564 |
| Braille + augmentation | 0.30862 | **0.28540** |
| Braille, binary (no fill colour) | 0.48239 | 0.31532 |

Findings:

1. Braille-only temporal ≈ static (0.3153 vs 0.3160) — no spatial reach (H2
   refuted again, independently).
2. Adding six glyphs improves **temporal** RMSE ~9.5 % (0.3153 → 0.2854) and
   static ~2.3 % (0.3160 → 0.3086).
3. The large temporal-vs-static gap only appears in the **binary** row
   (0.4824 → 0.3153): when no per-cell background colour exists, temporal is
   doing the fill's job. With colour available, the gain shrinks.

## Cross-validation: does the selected basis generalize? (SIMULATED)

`--mode=cv` — **leave-one-family-out**: for each of the six target families
(checker, curve, edge, line, noise, zone) the selector is re-run on every *other*
family and the held-out family is measured. This is the check the earlier
train/holdout split could not make, because the selector had already seen the
holdout family's structure through the shared candidate bank.

| held-out family | n | base static | aug static | base temporal | aug temporal |
| --- | --- | --- | --- | --- | --- |
| checker | 4 | 0.48062 | 0.48062 | 0.48060 | 0.48018 |
| curve | 5 | 0.27489 | 0.27182 | 0.27489 | 0.24935 |
| edge | 8 | 0.35351 | 0.30077 | 0.35193 | 0.23732 |
| line | 5 | 0.24629 | 0.24252 | 0.24610 | 0.22403 |
| noise | 3 | 0.11736 | 0.11552 | 0.11457 | 0.11027 |
| zone | 4 | 0.33543 | 0.31606 | 0.33501 | 0.26824 |
| **mean** | | **0.30135** | **0.28788** | **0.30052** | **0.26157** |

**SIMULATED:** the augmentation basis generalizes — held-out gain **+4.5 %
static, +13.0 % temporal** — so the earlier holdout number was not pure selection
overfit. The selector picks different *names* per fold but the same *families*
(halves, diagonals, fractional blocks), which is the sign of a stable direction
set.

Restricting the selector to **universally-portable** glyphs (`--mode=cv
--portable`) costs almost nothing: **+4.7 % static, +12.0 % temporal**. A basis
that can be built from glyphs every common mono font has is essentially as good
as the unconstrained one, which matters for a real terminal.

## Dot-shape falsification: which "new directions" are real? (SIMULATED)

This is the round's most important addendum result. The whole basis argument rests
on the round-gapped-disc dot shape. `--mode=shape` brackets it with round discs at
r=1.0/1.5/1.9 and axis-aligned **squares** at r=1.5/2.0 (squares tile exactly at
half-pitch), and measures each candidate's out-of-span fraction under every model:

| glyph | disc 1.0 | disc 1.5 | disc 1.9 | square 1.5 | square 2.0 | worst |
| --- | --- | --- | --- | --- | --- | --- |
| upper/lower/left/right half | 37.5 | 37.5 | 12.5 | **0.0** | **0.0** | 0.0 |
| quadrants | 56.2 | 56.2 | 18.8 | **0.0** | **0.0** | 0.0 |
| lower-1/4 | 56.2 | 56.2 | 18.8 | **0.0** | **0.0** | 0.0 |
| lower-3/4 | 18.8 | 18.8 | 6.2 | **0.0** | **0.0** | 0.0 |
| left-1/2b | 37.5 | 37.5 | 12.5 | **0.0** | **0.0** | 0.0 |
| left-1/8 | 83.3 | 83.3 | 79.2 | 75.0 | 75.0 | 75.0 |
| left-3/8 | 44.4 | 44.4 | 26.4 | 25.0 | 25.0 | 25.0 |
| hbar | 83.3 | 83.3 | 79.2 | 75.0 | 75.0 | 75.0 |
| vbar | 66.7 | 66.7 | 66.7 | 75.0 | 75.0 | 66.7 |
| corner-tl / tee-left | 45.1 | 45.1 | 45.5 | 48.9 | 48.9 | 45.1 |
| diag-fwd / diag-back | 69.9 | 69.9 | 56.3 | 58.7 | 58.7 | 56.3 |

- **Robust (>5 % under every model):** fine fractional blocks (`left-1/8` …
  `left-7/8`), thin rules (`hbar`/`vbar`), corners, tees, diagonals. These are
  genuine new spatial directions.
- **Shape artifacts (<1 % once the dots tile):** all halves, all quadrants, and
  the coarse vertical/horizontal fractions (`lower-1/4`, `lower-3/4`, `left-1/2b`).
  They add a direction only because round discs leave gaps; once the dots tile,
  they are a union of dot blobs.

**REFUTED (in-model):** the coarse fill split is *not* a new spatial direction. It
was the round-gap assumption talking. The earlier `--mode=lattice` table (discs
only, radii 1.0–2.4) caught the halves and quadrants; the square bracket additionally
demotes the coarse fractional blocks that disc-only sweeps had defended.

Held-out error is nearly identical across dot models (0.268 → 0.252 static), but
the *named* basis shifts (`lower-half`/`lower-1/4`/`right-half` →
`sextant-2`/`sextant-4`/`disc-small`). Only the robust list is safe to promote.

## Convergence guarantee (MEASURED-in-model)

Two small rigor fixes so the numbers cannot hide a local optimum:

- `temporal_error` now **dominates `static_error` by construction**: a single
  glyph is a feasible point of the temporal problem, so the routine returns
  `min(temporal, static)`. A randomized test checks `temporal ≤ static` on 40
  inputs.
- `hull_gap` computes the Frank-Wolfe **duality gap** `<q-t, q-s*>` at the returned
  point, a nonnegative bound on `f(q) − f*`. On a reachable target it is `< 1e-9`;
  the test also checks it is never negative on an unreachable one.

## Practical minimal basis (SIMULATED, boundary stated)

If the goal is "more spatial directions per cell, cheaply", the model supports a
small portable basis:

- **fine fractional blocks** (`U+258F` left-1/8, `U+2581` lower-1/8, and
  neighbours) — sub-cell edges at eighth positions;
- **box-drawing rules** (`U+2500` horizontal, `U+2502` vertical) — centre rules;
- **diagonals** (`U+2571`/`U+2572`) — slope edges.

Coarse blocks, shades **and quadrants** are **not** part of the basis: they are
fill variants, and the square-dot bracket shows the coarse fractional blocks are
too. Sextants/octants give finer 2x3/2x4 edges but are lower portability; they are
the next rung, not the first.

**Boundary:** these are coverage-model conclusions under a gapped-disc dot shape.
The *relative ordering* (fine > coarse; rules/diagonals genuinely new) is
calibration-robust across radii 1.0–2.4 **and across the disc/square shape choice**;
the absolute percentages are not. A real font calibration is required before these
numbers should be quoted as typography.

## Reproduction

```sh
cargo build --release --example terminal_to_1080p_lab
./target/release/examples/terminal_to_1080p_lab --mode=rank
./target/release/examples/terminal_to_1080p_lab --mode=wall
./target/release/examples/terminal_to_1080p_lab --mode=search
./target/release/examples/terminal_to_1080p_lab --mode=cv
./target/release/examples/terminal_to_1080p_lab --mode=cv --portable
./target/release/examples/terminal_to_1080p_lab --mode=ablations
./target/release/examples/terminal_to_1080p_lab --mode=lattice
./target/release/examples/terminal_to_1080p_lab --mode=shape
# portability-constrained selection:
./target/release/examples/terminal_to_1080p_lab --mode=search --portable
# calibration sensitivity:
./target/release/examples/terminal_to_1080p_lab --mode=lattice --radius=2.4
cargo test --example terminal_to_1080p_lab          # 20 tests
```

## Bottom line

"Terminal -> 1080p" is not reachable by Unicode brute force and not by time
alone. The honest statement is: Braille already spans all eight per-dot
directions; a small basis of **fine fractional blocks, thin rules and diagonals**
adds the missing sub-cell edges and lines. Coarse blocks, quadrants **and the
coarse fractional blocks** add nothing but fill — and the square-dot bracket shows
the last of those was an artifact of the round-gap assumption, not a real
direction. Cross-validation confirms the surviving basis generalizes to held-out
target families (+13 % temporal). All numbers are SIMULATED in a coverage model;
human perception is **HUMAN UNVERIFIED**.
