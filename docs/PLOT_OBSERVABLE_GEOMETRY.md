# Observable Geometry — the `gibson::plot` architecture (v0.5, EXPERIMENTAL)

> Status: **experimental, Rust-only.** No C ABI surface this round. Names and
> signatures may change until v0.5.0 ships. This document is the contract the
> module is built against; the tests enforce it.

## 1. What this is (and is not)

`gibson::plot` is a **semantic plotting layer**, not a chart-widget library. It
does not own analysis — it owns **realization**. The application computes
observables (FFT, regression, filtering, statistics, simulation — *none of that
lives here*); `gibson::plot` faithfully turns an indexed set of scientific values
into terminal geometry and tells you, via a receipt, exactly what it did with
every sample.

The one-sentence law: **a scientific plot is a finite diagram of observables in
coordinate spaces, projected through validated axis transforms into a normalized
square, placed in a viewport, and realized under the terminal's capabilities.**

## 2. The mathematical model

For each series `j`:

```
  I_j  ──s_j──▶  X × Y  ──σx × σy──▶  [0,1]²  ──viewport──▶  plot geometry  ──realize──▶  Surface
  │             │                     │                     │                             │
  index/        scientific            validated axis         normalized plot              capability
  sample set    values in the         transforms (partial,   square mapped to the         realization
                semantic spaces       invertible where        drawable plot Rect           (Braille /
                X, Y                   promised)                                            block / mono)
```

- `I_j` — the sample/index object (0..N). `SeriesKind` fixes its *topology*:
  `Scatter` = discrete points, no adjacency; `Line` = ordered samples with
  adjacency between **consecutive valid** samples.
- `s_j : I_j → X × Y` — the scientific values (`(f64, f64)` pairs).
- `σx, σy : Domain ⇀ [0,1]` — **validated partial maps** (`AxisTransform`), one
  per axis, parameterized by an `AxisScale` (`Linear` | `Log10`) and the visible
  `FiniteRange`. Partial: a sample outside the domain (e.g. `x ≤ 0` under
  `Log10`, or any non-finite value) has **no image** and is rejected explicitly,
  counted on the receipt. Invertible wherever the API promises (`unproject`).
- `viewport` — affine map `[0,1]² → plot Rect` in device cells. `y` flips
  (data-up → screen-down).
- `realize` — glyph realization under `ColorDepth` + glyph capability. Reuses
  `BrailleCanvas` (subpixel line/scatter), `Surface`/`SurfaceClip` (axes, ticks,
  labels, border, annotations). **No second renderer, no raw ANSI.**

### The compilation seam

Semantic compilation is kept **separate** from glyph realization:

```
  PlotSpec + PlotView + Rect  ──compile──▶  PlotLayout  ──render──▶  Surface
                                   │                          │
                              PlotReport (receipt)       capability-aware
```

`PlotLayout` holds resolved **device-space** geometry: the plot rectangle, the
semantic ticks, the projected primitives (polyline vertices, scatter points,
annotation lines/points), and label text. It depends on size and view **but not
on color depth or glyph set**. Realization reads one `PlotLayout` and draws it
for whatever capability is in force. This gives a testable seam: geometry tests
never depend on glyph appearance, and `unproject`/hit-testing share the exact
same transform the renderer used.

## 3. The type surface (vertical slice)

| Type | Role |
|---|---|
| `FiniteRange` | validated `[min, max]`, both finite, `min < max`, **and span `max - min` finite** (no overflow) |
| `AxisScale` | `Linear` \| `Log10` |
| `AxisTransform` | σ for one axis over a view range: `project`/`unproject`, validated |
| `PlotTransform2D` | the product σx × σy ∘ viewport; `project(x,y)->Option<(f64,f64)>` cell coords |
| `AxisSpec` | semantic axis: `scale`, `label`, optional `unit` (strings; no dimensional analysis) |
| `PlotView` | currently visible `x`/`y` `FiniteRange` — pan/zoom changes THIS, never the data |
| `SeriesKind` | `Scatter` \| `Line` |
| `Reduce` | `None` \| `ExtremaPerColumn` (explicit, monotone-X only) |
| `Series` | `kind`, `points: Vec<(f64,f64)>`, `color`, `reduce` |
| `Annotation` | `VLine{x}` \| `HLine{y}` \| `Point{x,y,label}` — in **data** coords |
| `PlotSpec` | `x: AxisSpec`, `y: AxisSpec`, `series`, `annotations`, `title` |
| `Tick` | `{ value: f64, label: String }` |
| `PlotLayout` | compiled device geometry (plot rect, ticks, projected prims, labels) |
| `PlotReport` | execution receipt (counts; see §5) |

## 4. Axis transforms as partial morphisms

`AxisScale::Linear` over `[a,b]`: `project(v) = (v - a) / (b - a)`;
`unproject(u) = a + u·(b - a)`.

`AxisScale::Log10` over `[a,b]` with `a > 0`: `project(v) = (log10 v - log10 a) /
(log10 b - log10 a)` for `v > 0`, else **None**; `unproject(u) = 10^(log10 a +
u·(log10 b − log10 a))`.

Validation rules (enforced at construction / rejected at project time):

- range ends finite, `min < max`, **and span `max - min` finite** (a range whose
  width overflows `f64`, e.g. `-1e308..1e308`, fails construction — otherwise the
  span is `+∞` and a Linear projection yields `NaN`);
- `Log10` requires `min > 0` (range construction fails otherwise);
- non-finite samples → `None` (rejected, counted);
- invalid log-domain samples (`v ≤ 0`) → `None` (rejected, counted);
- **no NaN or ∞ ever reaches raster/integer math** — `project` returns `Option`,
  and the `Some` case is **guaranteed finite**: any value whose projection is not
  finite (e.g. a far out-of-range datum whose `v - min` overflows) returns `None`.

## 5. The receipt — `PlotReport`

Plotting must never silently eat data. Every `compile` returns a bounded report:

- `samples_seen` — inputs offered;
- `finite_samples` — inputs with both coords finite;
- `nonfinite_rejected` — dropped for NaN/±∞;
- `scale_domain_rejected` — finite samples with **no image under the axis
  transform**: outside the scale domain (e.g. `x ≤ 0` under `Log10`), or a
  projection that overflows to non-finite. Counted from the axis transforms
  alone, so it is independent of terminal size and of any reduction (law E);
- `segments_considered` / `segments_clipped` — line adjacency pairs examined vs.
  clipped away at the plot boundary;
- `points_clipped` — scatter points that were finite **and** in the scale domain
  but projected outside the plot viewport (the scatter analogue of
  `segments_clipped`). Without it a fit-to-extent scatter could lose points while
  every rejection count read zero;
- `primitives_emitted` — points/segments actually drawn;
- `reduced_from` / `reduced_to` — present only when a `Reduce` policy ran.

This is **not** a quality score. It lets a user distinguish "nothing visible
because the data is off-viewport" (`points_clipped`/`segments_clipped`) from
"nothing visible because every value was invalid for the log scale"
(`scale_domain_rejected`). For a scatter series every finite sample is accounted
for: `finite_samples = scale_domain_rejected + primitives_emitted +
points_clipped`.

### Boundary convention (one last-index)

The drawable subpixel grid is indexed `0..=px-1`, so the normalized square maps
onto `[0, px-1]`: `u=1` / `v=0` land on the **last valid index**, not one subpixel
past it. Data, annotations, and tick cells all go through this one map, so a datum
sitting exactly on the view extent is drawn, an edge reference line draws at either
bound, and a datum on a tick value realizes in that tick's own cell.

## 6. Gaps and honest downsampling

- **Gaps.** A non-finite sample, or a log-invalid sample, **breaks the path**: a
  `Line` never bridges across it (`A → invalid → B` draws `A`-stub and `B`-stub,
  never `A→B`). Scatter simply drops the invalid point. Tested.
- **Reduction is explicit and narrow.** `Reduce::ExtremaPerColumn` is the only
  reducer, valid for **monotone-X line series** only. Per device column it keeps a
  `first / min / max / last` envelope, so a one-sample spike between sampled
  columns **cannot vanish** (the `show::sparkline` nearest-neighbour failure). It
  preserves gaps, and it never claims to handle arbitrary self-intersecting
  parametric paths. Default is `Reduce::None` (render every segment, O(N)).

## 7. Ticks

Semantic, deterministic, size-independent values:

- **Linear** — a nice-number lattice `1 / 2 / 5 × 10^k` chosen for the view range
  and a target count.
- **Log10** — majors at powers of ten; minors (`2..9 × 10^k`) only if space
  supports them.

Responsive rendering may **drop tick labels** as space shrinks, but never moves a
semantic tick value. Priority as the plot shrinks: (1) data viewport, (2) major
reference structure, (3) labels/legend. Scientific data outranks chrome.

## 8. Algebraic laws (enforced by tests)

| | Law |
|---|---|
| A | `unproject(project(x)) ≈ x` within a documented tolerance |
| B | increasing scale ⇒ `x1 < x2` ⇒ `project(x1) ≤ project(x2)` (pre-quantization) |
| C | `PlotTransform2D` ≡ apply σx, σy, then viewport, explicitly |
| D | project→device→unproject error ≤ ~½ a device subcell |
| E | **view naturality** — terminal size changes realization, not data / axis range / reject counts |
| F | **capability naturality** — depth/glyph changes realization, not accepted samples / transforms / tick values / annotations |
| G | **clipping locality** — a primitive outside the plot rect cannot mutate a cell outside it |
| H | **layer monoid** — empty layer list = identity; concatenation associative; z-order matters (NOT commutative) |
| I | **gap law** — no segment crosses an explicit/invalid gap |
| J | **extrema preservation** — a one-sample spike in a dense bucket survives `ExtremaPerColumn` |

## 9. Composition with the rest of LibGibson

A plot renders into a `Surface`, so it drops straight into `Node::canvas(|rect| ->
Surface)` as a hero, or into a `gibson::ui` panel as chrome. Because it is a pure
`(PlotSpec, PlotView, Rect) -> Surface + PlotReport`, a scientific plot can itself
be **one representation inside a Representation Atlas** (see
[`AI_VISUAL_AUTHORING.md` §6](AI_VISUAL_AUTHORING.md#6-directed-representation-the-atlas)):
the observables are the semantic model, the axes/ticks a basis, and a view change
is a seekable transport.

## 10. Deferred (NOT in v0.5 initial slice)

Bar charts, pie charts, histograms, heatmaps / scalar fields, vector fields, 3D
plots, error-bar/interval fibers, bands, dual Y axes, polar plots, SymLog, LTTB or
any adaptive reducer beyond `ExtremaPerColumn`, floating tooltips, mouse hit-test
UI, legends beyond a static label list, and any publication/stability promise. The
model is designed so these are *addable*, not *present*. FFT/regression/statistics
are **permanently** out of scope — those are the application's observables.
