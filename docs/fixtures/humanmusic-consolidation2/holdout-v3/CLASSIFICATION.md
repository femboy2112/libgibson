# Holdout v3 — first contact and classification

Executed **once**, at `721fffeabffe0d0d37e7ea45e95702c8dcbec421` (the declaration commit; library
source frozen at `362a4dd`), clean tree, `--release` (debug assertions off), one test thread.
`results/` holds the run byte for byte: the executed `config.tsv` (identical to the declared one),
`provenance.tsv`, `summary.tsv`, one receipt per case, and the complete process output
(`run.log.gz`); `results/SHA256SUMS` covers them. The harness exited 101 because two cases failed;
that is the preserved result.

## Result

**46/48 cases pass** (853 checks, 2 failing receipts). Observed:

| Law | Result |
| --- | --- |
| Generated BAND source holds its own `PerformanceReceipt` | **36/36** |
| Recorded composer = declared (incl. StructuralR9 on PropulsiveReturn, V31) | 36/36 |
| Recorded grammar = declared (all six, incl. WorldSwitch) | 36/36 |
| Percussion law the profile declares | 36/36 sources, every lifted cover |
| Identity re-extracted unchanged under a hostile non-identity event | **29/29** rows that pin Motif/Riff/BassFigure |
| Every refusal in the declared family, at its stage | 9/9 |
| Lifted cover admitted (`PerformanceReceipt` + `CoverConformance`) | **35/37** |
| `lift` predictions (12 declared) | 12/12 lifted and admitted |
| `nothing` (Swing Loose/Interpretive) | 2/2 |

Outcomes: 37 lifts; 9 lawful refusals — at lift: `pinned harmony outside target vocabulary` (V10,
E05), `pinned harmony family outside target vocabulary` (V22), `no lawful harmony contains the
pinned simultaneous attacks` (V18, V35), `a pinned bass event has no lawful pitch function in the
target harmony` (E06); at extraction: `MissingAxis(Groove)` from a source that never established
Groove (V15); 4 nothing-to-cover (S01, S02 declared; V03, V29 below).

## The two failures

1. **V19** — `cover / performance-receipt`: the lifted BAND cover (BLACK_ICE HookArc → VAPOR95
   HookArc at tonic G, Faithful) makes **1 false temporal function claim**: a generated BAND
   performance violating its own contract. Diagnosed after contact by re-running the row's
   pipeline outside the harness: the pinned lead event A♯5 at beat 41.54, stepping A5 → A♯5 → C6
   over Cm, is passing motion; the lift labelled it `ChromaticPassing`, but A♯ is in the palette
   scale of the context it sounds in, so the judge reconstructs `DiatonicPassing`. The lift
   classifies a pinned event's passing motion against a different scale than the context it
   sounds in. Family: pinned-event functions (this round's item 5 law, for the lead).
2. **V36** — `cover / motif`: a Loose cover of a VAPOR95 PropulsiveReturn source into a 22-beat
   PropulsiveReturn target. The target song, planned by `SongMap::build`, has **no theme site at
   all** (its AnchorReport says Motif is structurally inapplicable: no seated phrase can hold a
   statement), so the pinned theme is never stated. The lift returned a candidate instead of
   refusing; only its own conformance check (and so `cover`'s `CoverAdmission`) rejects it. Family:
   a missing typed refusal at lift — a pinned theme with no seated phrase to state it.

Neither failure is a source failure, and neither recurs a family repaired this round as such.

## Coverage, stated plainly

- **Swung groove across a swing change was not exercised.** The two groove-only rows that cross
  it (V03 VAPOR95 → BLACK_ICE, V29 SWISS_SIGNAL → VAPOR95) pin Groove alone, which the library
  does not count as song identity (`CoverSpec::has_song_identity`), so they are "nothing to cover"
  — the declared lawful outcome, not evidence about groove transport. Swung grooves were lifted
  and admitted only VAPOR95 → VAPOR95 (V06, V21).
- **Natural identity overlap did not occur**: in all 29 rows no non-identity event shared an onset
  with an identity event of the same role. The overlap law was exercised only by the injected
  hostile event (29/29 unchanged).
- V15's `MissingAxis(Groove)`: the VAPOR95 WorldSwitch source does not declare Groove.

## Disposition

Preserved as is. Repairs made after this contact are recorded in
`docs/HUMAN_MUSIC_MERGE_READINESS.md` with their own falsifiers on fresh seeds; this holdout is
not re-run and its numbers do not change.
