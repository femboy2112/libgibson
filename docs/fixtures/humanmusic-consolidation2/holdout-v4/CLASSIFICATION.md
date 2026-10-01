# Holdout v4 — first contact

**One contact, no fitting.** Executed once, release profile, on a clean tree at the declaration
commit `d869f72b43d6ea4d2bef762dd17bdc22304e13f7` (library source frozen at
`006bacfda5d7c79f2e3c9266f78095d4bf070363`; config SHA256
`27be8e4a31e8a7c871e9606282139318f59f26d0ad27017dc526881baaed57ba`). Raw receipts are under
`results/`; nothing here was tuned to them.

## Result

**24 / 28 cases pass; 4 fail, with 4 independent failing receipts (472 checks total).**

Every failure is a **declaration error in this holdout**, not an internally invalid performance:

| Case | Failing law | Cause | Disposition |
| --- | --- | --- | --- |
| V09 | `source declared-grammar` | declared `WorldSwitch` but the source composer is `StablePropulsion`, which selects `PropulsiveReturn` by design (recorded `grammar=PropulsiveReturn`). | config error; the source's own `PerformanceReceipt` and its `groove+bass` lift both PASS |
| V14 | `source declared-grammar` | declared `RiffDrive` with `StablePropulsion` (records `PropulsiveReturn`). | config error; source receipt and lift PASS |
| V20 | `source declared-grammar` | declared `HookArc` with `StablePropulsion` (records `PropulsiveReturn`). | config error; source receipt and `groove+motif` lift PASS |
| O02 | `cover candidate-lift` | `expect=lift`, but the derived Ode harmony is lawfully refused by SWISS_SIGNAL as `pinned harmony outside target vocabulary` (SWISS declares no sevenths and no mixture). | failed prediction; the refusal is in the declared lawful family |

**No internally invalid BAND performance appeared.** Every generated source that ran (all 20)
passed its own `PerformanceReceipt`; every admitted cover passed `PerformanceReceipt +
CoverConformance`; the one refusal is lawful.

## The declared cross-swing gap

`cross-swing-groove-quotient` ran on every admitted cover whose map pins `CoverAxis::Groove`,
including the new combined selections:

| Case | Source → target swing | Result |
| --- | --- | --- |
| V06 | SWISS straight → VAPOR95 swung, groove+motif | PASS, 16 pinned strokes, 0 moved |
| V07 | VAPOR95 swung → BLACK_ICE straight, groove+motif (`lift`) | PASS, 6 pinned strokes, 0 moved |
| V08 | VAPOR95 → SWISS, groove+harmonic | extraction/lift refused/ lawful |
| V09 | BLACK_ICE → VAPOR95, groove+bass | PASS, 18 pinned strokes, **2 carried to another performed float** |
| V20 | VAPOR95 → VAPOR95, groove+motif (`lift`) | PASS, 19 pinned strokes, **2 carried to another performed float** |

Where the transport actually moved strokes (V03, V09, V20), the canonical metric quotient still
matched and no performed-float equality was required. V17 (groove-only) correctly left nothing to
cover.

## Post-contact note (disclosed, not fitted)

This first contact stands as recorded. It was executed before the still-running broad fresh BAND
search finished; that search then found additional pre-existing internal reds at other seeds
(metric-vs-performed Groove bar assignment, an interaction-material window past the domain, an
occupancy epsilon), plus a regression introduced by the first version of the support-lifetime law.
Those were corrected at source after contact; **no v5 was run in this round** and this raw result
was not altered. See `docs/HUMAN_MUSIC_MERGE_READINESS.md`.
