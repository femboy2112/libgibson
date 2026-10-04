# SAI → `gibson::plot` architectural crossover

What the v0.5 Observable Geometry kernel borrowed as a **generic law** from the
Semantic Audio Inversion (SAI / "Reverse HumanMusic") WIP, what it refused, and
why — so a future reader can tell an independently-derived idea from a borrowed
law from a cargo-culted one.

**Source read (read-only):** `femboy2112/libgibson-agent-native-ui-lab`, branch
`feat/sai-round2-cover-synthesis`. Remote PR #8 head `c935178`; the local worktree
audited was one commit behind at `e25b67c` (the laws below are stable architecture,
not the tip commit, so this did not matter). Paths relative to
`experiments/semantic-audio-inversion/`.

**Nothing was copied.** No SAI source, type, or algorithm was lifted into
LibGibson — not a line. The crossover is at the level of *architectural law*, and
each law below was re-implemented from scratch against the plot kernel's own
concepts. SAI is WIP and makes no success claim it hasn't earned (its own ledger:
the native cover is Refuted for recognition, measurement playback "severely
garbled", harmony/groove `unknown`, causal inversion not established) — so it is a
*research input*, never a merge source.

## ADAPTED (generic law re-derived, not code)

- **A. Unknown ≠ Free ≠ Ambiguous → invalid configuration ≠ invalid sample.**
  SAI types three distinct knowledge states (`crates/sai-core/src/quotient.rs`
  `enum AxisKnowledge { Established, Derived, Free, Unknown, Ambiguous }`) and
  refuses to fill an absence because a stronger preset was requested. The plot
  kernel had collapsed two *different* failure causes into one misleading counter:
  a malformed scale **view** (`Log10` min ≤ 0) fabricated one
  `scale_domain_rejected` per finite sample, indistinguishable from genuine
  out-of-domain **samples**. Fixed by splitting the ontologies at the smallest
  possible API: `compile` now returns `Result<_, PlotError>`, where
  `InvalidXAxisDomain`/`InvalidYAxisDomain` is a *configuration* fault that reads
  no samples, distinct from an `Ok` compile that counts a bad sample in
  `scale_domain_rejected`, distinct again from a valid-but-empty (zero-area)
  `Ok`. **We did NOT import a generic epistemic enum** (SAI's `AxisKnowledge`) —
  the three states the plot layer actually has are expressed by `Result` +
  existing counters, nothing more.

- **C. Requested vs effective (+ lowering reason) → reducer contract.**
  SAI's `AxisResult { requested, effective, lowering }` (`quotient.rs:232`) records
  that an operation honestly degraded and why. `Reduce::ExtremaPerColumn` has
  exactly that shape: valid only on monotone-X, previously applied with no check
  ("does not panic" outside its domain). Now the kernel validates nondecreasing-X
  and, off-contract, **declines** (renders unreduced) and records it:
  `reducers_requested` / `reducers_declined`. One lowering reason is possible here
  (non-monotone X), so a counter *is* the reason — no enum needed. The low-level
  `reduce_extrema` was made `pub(crate)`; `Series::reduce(..)` is the only entry.

- **D. Separate gates → two-gate receipt + conservation laws.**
  SAI ranks lexicographically over independent gates (`python/sai_v2/rank.py`),
  and a downstream gate never repairs an upstream failure ("best" ≠ "admitted").
  The plot receipt now documents and tests two gates that never cross: *acceptance*
  (`finite_samples`, `nonfinite_rejected`, `scale_domain_rejected` — size/
  capability/reduction independent) vs *realization* (`segments_*`,
  `points_clipped`, `primitives_emitted`). Conservation is stated per kind and kept
  separate (scatter: `finite = domain_rejected + emitted + points_clipped`; line:
  `segments_considered = emitted + segments_clipped`) — points and segments are not
  the same object, so there is no single bogus unifying equation.

## ADAPTED AS DISCIPLINE ONLY (no new type)

- **B. Authoritative vs derived coordinate.** SAI keeps authoritative seconds
  beside a derived beat coordinate (`transport.rs` `BeatCoordinate { seconds, beat }`)
  and returns `QuantizedBeat { ticks, residual_beats }` rather than pretending the
  approximation is exact. The plot kernel **already** holds the semantic coordinate
  as authoritative (`AxisTransform::{project,unproject}`, annotations in data
  coords, `PlotView` never rewrites data) and derives normalized/device coordinates.
  A search for duplicated reconstruction logic in the *library* found none — the
  redundancy lives in *consumers* (e.g. Project Pulsar hand-rolling `cell_of` /
  `data_at`). So **no `CoordinateWitness` wrapper type was added** (§9 "only if
  earned" — it isn't yet); the law is carried by the §4 boundary-convention doc and
  the project/unproject round-trip tests. If consumer demand recurs, a
  `layout.cell_of(x,y)` / `data_at(cell)` helper is the earned next step — recorded,
  not built.

## NOT ADAPTED (quarantined — belongs to the application, never to plotting)

Inverse causal synthesis; instrument models; harmonic-family competition
(`scripts/harmonic_competition_probe.py`, STFT/CQT/NNLS); the audio evidence schema
and stem separation (Demucs, `observe.py` librosa/Welch-PSD); production/mix fitting
(`python/sai_v2/production.py`); source-specific heuristics; the whole cover-synthesis
renderer (`round2/renderer/`). None of it crossed, and none ever can: `gibson::plot`
owns **realization**, not analysis. It displays rival hypotheses / uncertainty the
caller supplies; it never infers peaks, spectra, statistics, regressions, labels,
or causes. "Explain downward before quotienting upward" is a law for analysis
systems and is deliberately kept *out* of the plot layer (see
[`PLOT_OBSERVABLE_GEOMETRY.md` §1](../PLOT_OBSERVABLE_GEOMETRY.md)).

## WHY this and not more

The temptation with a categorical-sounding framework is to port its vocabulary.
We ported *pressure*, not vocabulary: each law was kept only where the plot kernel
had a concrete, repeated structural defect it fixed (a conflated counter, an
unchecked reducer, a receipt that could mask a loss), and expressed with the
smallest API that preserves the distinction — a `Result`, two counters, two
documented conservation laws. No generic epistemology type, no fidelity system, no
coordinate wrapper. Adapt only what concrete repeated structure earns.
