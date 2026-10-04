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

And the matching prohibition, so the division of labour is unambiguous: **the plot
layer may *display* rival hypotheses, uncertainty, intervals or candidate
structure that the caller supplies as observables — it never *manufactures* them.**
It infers no peaks, no FFT components, no regressions, no statistics, no semantic
labels, no uncertainty from raw measurements. Realization explains nothing
downward about causes; it only places what it was given. (If you came here from an
analysis/inversion system: that machinery stays in the application — see
[`docs/research/SAI_ARCHITECTURAL_CROSSOVER.md`](research/SAI_ARCHITECTURAL_CROSSOVER.md)
for which generic laws crossed over and which emphatically did not.)

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
| `Reduce` | `None` \| `ExtremaPerColumn` (explicit, monotone-X only; *requested*, may be *declined* — §6) |
| `Series` | `kind`, `points: Vec<(f64,f64)>`, `color`, `reduce` |
| `Annotation` | `VLine{x}` \| `HLine{y}` \| `Point{x,y,label}` — in **data** coords |
| `PlotSpec` | `x: AxisSpec`, `y: AxisSpec`, `series`, `annotations`, `title` |
| `Tick` | `{ value: f64, label: String }` |
| `PlotLayout` | compiled device geometry (plot rect, ticks, projected prims, labels) |
| `PlotReport` | execution receipt (counts; see §5) |
| `PlotError` | `InvalidXAxisDomain` \| `InvalidYAxisDomain` — a **configuration** fault (§4) |

`compile` and the `plot` convenience both return `Result<_, PlotError>`: a
malformed configuration is an `Err`, every valid configuration (including a
valid-but-empty area) is `Ok`. The low-level extrema reducer is **not** public
surface — the only reduction entry point is `Series::reduce(Reduce::ExtremaPerColumn)`,
so the kernel always gets to check the precondition before it runs (§6).

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

### Invalid configuration ≠ invalid sample (two ontologies, never conflated)

A `Log10` **view** whose `min ≤ 0` is not a valid domain for the scale: the axis
transform has no definition at all, so *no* sample — not even one that would be
perfectly valid under a well-formed log view, like `x = 10` — could be placed.
That is a **configuration** fault, and `compile` returns
`Err(PlotError::InvalidXAxisDomain)` / `InvalidYAxisDomain` (X checked first),
reading not a single sample. It is categorically different from a *sample* with no
image under a **valid** scale (e.g. `x ≤ 0` on a valid `1..1000` log axis), which
is an `Ok` compile with that sample counted in `scale_domain_rejected`. The old
kernel collapsed the two — an invalid log *view* fabricated one
`scale_domain_rejected` per finite sample, so a malformed request looked like a
pile of bad data. A bad request and a bad datum are different states and now carry
different results: `Err` versus a receipt counter. (A **zero-area** plot rectangle
is a third, distinct thing: a *valid* configuration that simply realizes nothing —
`Ok`, `transform == None`, zero rejections. Empty is not invalid.)

## 5. The receipt — `PlotReport`

Plotting must never silently eat data. Every **successful** `compile` returns a
bounded report (a configuration fault is an `Err` instead — §4 — and produces no
receipt, so a malformed request can never masquerade as counted data).

The counters split into two **gates that never cross**: *acceptance* is a property
of the spec and `(scale, view)` alone — size-, capability- and realization-
independent — and *realization* depends on the device rectangle. A terminal resize
or a glyph-mode change can move a realization counter; it can **never** move an
acceptance counter.

Acceptance gate:
- `samples_seen` — inputs offered;
- `finite_samples` — inputs with both coords finite;
- `nonfinite_rejected` — dropped for NaN/±∞;
- `scale_domain_rejected` — finite samples with **no image under a valid axis
  transform** (e.g. `x ≤ 0` on a *valid* `Log10` axis), or a projection that
  overflows to non-finite. Never includes a configuration fault (§4);
- `reducers_requested` — series carrying a reduce policy (`reduce != Reduce::None`).
  A property of the **spec**, so it is reported even at zero area (the request was
  made; it simply wasn't realized);
- `reducers_declined` — requested reducers that are **not applicable** and render
  unreduced: a scatter series (cannot reduce) or a line whose X is not nondecreasing
  (`ExtremaPerColumn`'s proven domain — §6). Also size-independent.
  `requested − declined` were applied.

Realization gate (all device-dependent):
- `segments_considered` / `segments_clipped` — line adjacency pairs examined vs.
  clipped away at the plot boundary;
- `points_clipped` — scatter points that were finite **and** in the scale domain
  but projected outside the plot viewport (the scatter analogue of
  `segments_clipped`). Without it a fit-to-extent scatter could lose points while
  every rejection count read zero;
- `points_emitted` — scatter **points** actually drawn (scatter series only);
- `segments_emitted` — line **segments** actually drawn (line series only). Kept
  distinct from `points_emitted`: points and segments are not the same object, so
  they are **never** summed into one counter — a single shared "primitives drawn"
  count made a mixed Line+Scatter plot un-auditable (one kind's output masked the
  other's loss). The split keeps each kind independently conserved;
- `reduced_from` / `reduced_to` — original and produced lengths of the **applied**
  reducers only (both `0` when nothing was applied, e.g. at zero area). Both include
  any gap/`(NaN,NaN)` sentinels, so they agree; neither is in a conservation law.

This is **not** a quality score. It lets a user distinguish "nothing visible
because the data is off-viewport" (`points_clipped` / `segments_clipped`) from
"nothing visible because every value was invalid for a valid log scale"
(`scale_domain_rejected`) from "the request itself was malformed" (`Err`).

**Conservation laws** (honest, and kept separate so one success never masks
another failure — points and segments are not the same object, and are counted
by distinct fields precisely so these laws survive a mixed plot):

- *Line* (holds on **any** plot, mixed or not — every term is line-only):
  `segments_considered = segments_emitted + segments_clipped`.
- *Scatter* (holds when **every series is Scatter**, so no line points inflate
  `finite_samples`): `finite_samples = scale_domain_rejected + points_emitted +
  points_clipped`. Every finite sample is drawn, counted-off-view, or
  counted-out-of-domain — never silently gone.
- *Mixed* Line+Scatter: the line law above still holds universally, and the
  scatter points are conserved within the scatter-kind counters (`points_emitted
  + points_clipped` + scatter-domain-rejected). What does **not** hold on a mixed
  plot is a single whole-receipt equation summing both kinds — there is no such
  equation, by design: a line's points become *segments*, not emitted points, so
  point-level and segment-level accounts are never added. (The earlier shared
  `primitives_emitted` made the naive sum look checkable and it silently failed on
  the commonest overlay, scatter + fit line; the split fields remove that trap.)

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
- **Reduction is explicit, narrow, and requested-vs-effective.**
  `Reduce::ExtremaPerColumn` is the only reducer, valid for **monotone-X line
  series** only. Per device column it keeps a `first / min / max / last` envelope,
  so a one-sample spike between sampled columns **cannot vanish** (the
  `show::sparkline` nearest-neighbour failure). It preserves gaps. The caller
  *requests* it per series; the kernel checks the precondition (X nondecreasing
  over the finite samples — a NaN gap is transparent to the check, so it neither
  blocks a monotone series nor hides a descending step across itself) and only
  *applies* it when the contract holds. Outside that proven domain (descending or
  self-intersecting X) the reducer is **declined, not silently run on data it
  would mangle**: the series renders unreduced (still correct, just O(N)) and the
  decline is recorded in `reducers_declined`. The low-level reducer is internal;
  there is no public way to invoke it off-contract. Default is `Reduce::None`.
  Columns are taken in the **axis's projected space**, so a `Log10` axis reduces
  in log columns (one device column per on-screen column), and samples **outside
  the view** occupy their own edge buckets — they can never evict an in-view
  column's envelope, so a one-sample in-view spike survives even under a zoomed
  view (POST-CANARY fix for the PULSAR-2 round: the earlier reducer bucketed
  linearly in raw x and *clamped* out-of-view columns into the edges, silently
  eating in-view data under zoom and mangling log-axis envelopes).

## 7. Ticks

Semantic, deterministic, size-independent values:

- **Linear** — a nice-number lattice `1 / 2 / 5 × 10^k` chosen for the view range
  and a target count.
- **Log10** — majors at powers of ten. When the view spans **fewer than two
  decades** (e.g. `2..8`, no power of ten, or `2..60`, a single major) the powers
  of ten alone give zero or one tick and the axis is unreadable, so the `2..9×10^k`
  minors are promoted to **labelled** ticks (merged with any major, thinned toward
  the target so they do not collide). This wiring was missing before the PULSAR-2
  round — `log10_minor_ticks` existed but `compile` never called it.

Responsive rendering may **drop a tick label** as space shrinks — it **drops it
whole, never truncating it into a different number** (a right-truncated
`-1000000` reading `-100000` is a lie, not a smaller label) and never runs two
labels together (a label that would collide with its neighbour is dropped). It
never moves a
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
| K | **configuration ≠ sample** — an invalid scale *view* is `Err(PlotError)` reading no samples; a bad *sample* under a valid scale is an `Ok` with a `scale_domain_rejected`; a zero-area rect is an `Ok` with zero rejections |
| L | **reducer requested-vs-effective** — `ExtremaPerColumn` is applied only on nondecreasing-X; off-contract it is declined and recorded (`reducers_declined`), never run; a declined series is byte-identical to `Reduce::None` |
| M | **conservation** — line (any plot): `segments_considered = segments_emitted + segments_clipped`; scatter (all-scatter plot): `finite = scale_domain_rejected + points_emitted + points_clipped`. Points and segments have distinct emitted counters and are never summed into one whole-receipt equation (segments ≠ points) |

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
