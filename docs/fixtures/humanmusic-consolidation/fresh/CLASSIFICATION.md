# Consolidated holdout v1 — post-contact classification

This is **analysis after first contact**, kept separate from the raw receipts in [`results/`](results/)
(committed unchanged at `8a0b0cd`). Nothing here changed source, gates, thresholds or configuration.
No family below was repaired after contact; the declared rule stands: any repair first writes its
counterexample and is then judged on a second, untouched holdout.

Labels: **Observed** = read directly from the receipts; **Conjectured** = mechanism inferred from
reading code, not executed. Every failing observer is library-owned (`witness::audit`,
`SongMapConformance`, `occupancy::violations`, `TemporalPitchDiagnostics`, cover extraction), so these are
internal-consistency failures of the generator against its own contracts, not external-oracle results.
Failures on the same row are correlated and are not independent bearings.

## Result

7/36 cases pass: all six external Ode covers (E01–E06) and G29. 29/30 generated cases fail with 67
failing receipts out of 970 checks. Held identity failed 0/30 sources and 0/17 covers; source
stage-boundary, causal-hearing and pitch-function-presence, and cover action-receipts had 0 failures.

## Families

| Family | Rows | Receipts | Mechanism | Label | vs known R17 |
| --- | --- | ---: | --- | --- | --- |
| **U1 partial final bar** — `invalid reservation for Bass` | 16 sources; covers G02, G16 | 18 | the only occupancy string in the set. Source occupancy fails 0/9 bar-aligned (16/20/28 beats) vs 16/21 partial-final-bar pieces. Candidate: bass reservation end from the unclipped note (`occupancy.rs` reservation construction) while `clip_to_end` (`functor.rs`) clips notes but not `score.occupancy` | correlation Observed; mechanism Conjectured | new (the known sweep had no partial bar) |
| **U2 declared axis never sounded** — `MissingAxis(Groove)` | G01 G03 G05 G10 G11 G13 G15 G20 G21 G25 G30 | 11 | every one of these sources has `drums=0` in its canonical inventory: short RiffDrive/PropulsiveReturn forms seat the kit Silent (Intro/Break/Coda arrangement), while `CoverSpec::from_contract` pins Groove from the contract anchors. Extraction refuses honestly (`strokes.is_empty()`), not a timing error | Observed | new |
| U2 variant — `MissingAxis(Motif)` and `missing_theme_site` | G12, G27 (Motif); G02 G12 G17 G27 (theme site) | 2 + 4 | pieces ≤ 9.25 beats whose planned identity statement is never sounded by a Lead note | Observed | new |
| Source action receipts, Simple language | G04 G10 G14 G20 G24 G30 | 6 | Simple-language sources: fragment 0/12, hit 6/24, thicken 6/12, resolve 7/12 witnessed; every Simple row with any action fails | Observed | new: the known sweep ran fusion only and compared witnesses relative to an older arm |
| Source action receipts, fusion | G02 G06 G08 G12 G13 G16 G22 G26 G28 | 9 | heterogeneous kinds (resolve, hit, thicken, fragment, re-entry) | Observed | new (absolute check never asserted before) |
| **U3 unwitnessed song obligation** (source) | G08 G16 G26 | 3 | long positional-grammar sources with drums but no ReEntry/Fill action. Leading candidate: the ordinary-path twin of the groove-debt signature repaired for covers in `1a06575` (ordinary generation remains `KitMotion::RoleDriven`); rival: RegisterAscent, which is unwitnessable by design. Their Groove-pinned covers all pass song projection | Conjectured | new |
| Source temporal-function | G02, G22 | 2 | one held Keys note crossing a harmony change without held resolution | Observed | same detector, different role/regime |
| Cover lift refusal: pinned attacks vs SWISS vocabulary | G04, G09 | 2 | BLACK_ICE chromatic motif into SWISS_SIGNAL (no sevenths, no mixture): no admitted chord contains the pinned simultaneous attacks | Observed; lawful explicit refusal | n/a |
| Cover lift refusal: pinned harmony vs target vocabulary | G06, G22 | 2 | fusion-source harmony into a Simple-language target | Observed; lawful explicit refusal | n/a |
| Extraction: timing / pin conflict | G18 (`UnprojectableTiming`), G28 (`ConflictingPins`) | 2 | unresolved; both 28-beat `all` rows while G08 (28 beats, `all`) extracts | unresolved | new |
| Cover pin/stage conflict | G23 | 1 | `all` pins Orchestration and Groove; pinned strokes are dropped when the pinned stage has no drum seat, so the cover re-extracts no Groove | Conjectured | new |
| Cover chart-grid conflict | G14, G24 | 2 | `illegal_harmonic_transform` off the declared rhythm grid: motif-only covers into a PropulsiveReturn target chart; harmony placed at pinned-attack windows ignores the chart grid | Conjectured | new |
| Cover riff role flip | G17 | 1 | Riff re-extraction picks Lead if any Lead note exists, else Bass | Conjectured | new |
| Cover temporal-function | G08 | 1 | a Lead note declared ChromaticPassing whose local reading is DiatonicPassing | Observed | same detector |

## What the holdout says about cover mode

- The external Ode path, the one route with an external symbolic source, passes 6/6 across all three
  worlds, both languages, four target keys and three altered tempi.
- Generated-source cover evidence is thin: 15/30 rows produced no map (mostly U2), 4 more were explicit
  lift refusals, leaving 11 generated covers, of which 7 (G02 G08 G14 G16 G17 G23 G24) fail at least one
  cover-side receipt. Groove-pinned covers
  exist for G08, G16, G23 and G26; G23 fails on the pin/stage conflict above.
- The largest families are **source-side**: ordinary `PerformanceProfile::POCKET` generation on fresh
  short, partial-bar and Simple-language inputs violates its own occupancy, action-witness and
  obligation contracts. The known R17 sweep could not see these because it used bar-aligned lengths,
  fusion only, and relative (arm-to-arm) comparisons.

## Counterexample sites (not written; any repair needs a second untouched holdout)

- U1: a `perform_with_profile(.., POCKET)` source with `total_beats % 4 != 0`, asserting
  `occupancy::violations(.., true)` is empty.
- U2: extraction of `CoverSpec::from_contract` from a drumless generated source — decide whether the
  contract default should pin only axes the source actually sounds (an explicit `Unknown`, as the external
  path already does) or keep refusing.
- U3: the obligation ledgers of G08/G16/G26 must be opened before this is a candidate.
