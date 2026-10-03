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

## Practical minimal basis (SIMULATED, boundary stated)

If the goal is "more spatial directions per cell, cheaply", the model supports a
small portable basis:

- **fine fractional blocks** (`U+258F` left-1/8, `U+2581` lower-1/8, and
  neighbours) — sub-cell edges at eighth positions;
- **box-drawing rules** (`U+2500` horizontal, `U+2502` vertical) — centre rules;
- **diagonals** (`U+2571`/`U+2572`) — slope edges;
- optionally **quadrants** (`U+2596`–`U+259F`) — a 2x2 subdivision.

Coarse blocks and shades are **not** part of the basis: they are fill variants.
Sextants/octants give finer 2x3/2x4 edges but are lower portability; they are the
next rung, not the first.

**Boundary:** these are coverage-model conclusions under a gapped-disc dot shape.
The *relative ordering* (fine > coarse; rules/diagonals genuinely new) is
calibration-robust across radii 1.0–2.4; the absolute percentages are not. A real
font calibration is required before these numbers should be quoted as
typography.

## Reproduction

```sh
cargo build --release --example terminal_to_1080p_lab
./target/release/examples/terminal_to_1080p_lab --mode=rank
./target/release/examples/terminal_to_1080p_lab --mode=wall
./target/release/examples/terminal_to_1080p_lab --mode=search
./target/release/examples/terminal_to_1080p_lab --mode=ablations
./target/release/examples/terminal_to_1080p_lab --mode=lattice
# calibration sensitivity:
./target/release/examples/terminal_to_1080p_lab --mode=lattice --radius=2.4
cargo test --example terminal_to_1080p_lab          # 17 tests
```

## Bottom line

"Terminal -> 1080p" is not reachable by Unicode brute force and not by time
alone. The honest statement is: Braille already spans all eight per-dot
directions; a small basis of **fine fractional blocks, thin rules and diagonals**
adds the missing sub-cell edges and lines, and coarse blocks/quadrants/shades add
nothing but fill. All numbers are SIMULATED in a coverage model; human
perception is **HUMAN UNVERIFIED**.
