# Hardening holdout v2 — post-contact classification

Analysis **after** the single contact. The raw receipts in [`results/`](results/) are committed unchanged
(`184dc0a`). Nothing here changed source, gates, thresholds or configuration; no family below was
repaired after contact. Any repair first writes its counterexample and is judged by a separately declared
holdout v3.

Labels: **Observed** = read in the receipts, or re-derived by a diagnostic probe on these known rows
(not committed); **Conjectured** = mechanism inferred from code reading. Failures on one row are
correlated, not independent bearings.

## Result

**33/48 cases pass** (1,685 checks, 19 failing receipts). 25/36 generated rows, 7/8 Ode rows and 4/4 Swing
rows pass. For comparison only (different rows, not a like-for-like score): holdout v1 passed 7/36.

Source laws that failed **0 times** across all 36 generated sources and their covers include score
domains, held identity, stage boundaries, causal hearing, hearing-ledger coverage, continuation identity,
the direct-voice lifetime, anchor reports (no declared-but-missing anchor) and phrase destinations.
Under the `BAND` profile the action-receipt audit failed 0 times on 32 generated sources; the one
action-receipt failure is H18, a historical `POCKET` source.

## Families

| Family | Rows | Receipts | Mechanism | Label |
| --- | --- | ---: | --- | --- |
| **Pre-registration errors (mine)**: lawful refusal or honest absence under a declared `lift` | H01, H05, H25; H22; H32 | 5 | H01: BLACK_ICE Aeolian harmony into SWISS_SIGNAL (mode change) → `pinned harmony outside target vocabulary`. H25: SWISS RiffDrive riff+bass into SWISS → `no lawful harmony contains the pinned simultaneous attacks` (every pinned attack must be a chord tone of its window). H22: Loose keeps only the theme; the 10.75-beat source has no lead line, so nothing is pinned (`identity-present`). H32: a binary motif-only spec on an 11-beat source with no lead → `MissingAxis(Motif)` (the `established` spec would not have asked). | Observed; lawful behaviour, wrong prediction |
| **Source vocabulary leak** (inside H05) | H05 | (1, above) | the SWISS_SIGNAL PropulsiveReturn source sounds `Maj6`/`Maj7` Reset chords (probe: every span of the 17.25-beat source) although SWISS_SIGNAL's vocabulary excludes four-note qualities; the target, correctly, refuses to pin them. The backbone's Reset gesture ignores `world.use_sevenths`. | Observed (probe); new source-generation finding |
| **Extraction: unprojectable timing** | H23, H31, H35 | 3 | DeflectedLift sources with Groove requested (Strict/Faithful/Interpretive) → `UnprojectableTiming`. Before the pre-freeze honesty fix (`1389e71`) these would have extracted as zero-identity maps and passed. Leading candidate: kick/snare figure strokes at material onsets off the quarter grid the legacy projection admits (the v1 G18 family). | Observed symptom; mechanism Conjectured |
| **Extraction: non-monophonic lead line** | H06 | 1 | VAPOR95 HookArc source at Faithful → `line requires ordered monophonic metric events` (two lead events at one canonical position) | Observed symptom; mechanism unresolved |
| **Source: ownership span** | H12 | 1 | 7.25-beat partial-final-bar source (BAND, Expressive): `invalid ownership span for Bass` — the U1 repair bounded the bass *line*; an ownership span derived for it can still be invalid at the piece end | Observed; new, adjacent to U1 |
| **Source: unwitnessed song obligation** | H16, H28, H30 (+ H16 cover) | 4 | one settled obligation with no discharging event each — the named remaining U3 classes (cadence debt without a home chord / an unperformed planned Resolve / an unanswered Simple motif question) | Observed count; class per row not isolated |
| **Source: temporal claim** | H16 | 1 | one false function claim (9 orphan extensions) on a 48-beat SWISS DeflectedLift source | Observed |
| **Historical arm, as characterized** | H18 | 1 | `POCKET` (Planned admission): 13/17 verbs witnessed — the U3 falsifier's defect in the byte-exact historical arm | Observed; expected |
| **Cover lift: pinned bass without pitch function** | E04, H16, H18 | 3 | covers pinning a bass line (Strict/Faithful) leave some pinned bass notes unclassified (E04 probe: 3 of 60 `cover-identity` bass notes, beats 11/27/59) | Observed; new |

## What v2 says

- The repaired v1 families did not recur as such: no `invalid reservation` (U1), no declared-but-missing
  anchor (U2), no missing theme site, no unwitnessed verb under rehearsed admission (U3 actions), no
  keys-hold temporal claim (G02/G22), no riff-lane flip, no dropped pinned stroke, no unplayed-chart
  claim. Adjacent defects did appear (H12's ownership span; the remaining obligation classes).
- The fidelity dial held its epistemic laws on every row that extracted: no Unknown axis pinned, derived
  harmony always labelled, Swing Loose/Interpretive left nothing to cover and Faithful/Strict reproduced
  the skeleton.
- The most common failure is my own pre-registration (5 cases): I predicted lifts where the library
  lawfully refused or had nothing to pin. That is a prediction error, not a code defect, and is kept red.
- New findings to carry forward: the SWISS Reset-chord vocabulary leak (H05), unprojectable DeflectedLift
  grooves (H23/H31/H35), the non-monophonic lead (H06), the partial-bar ownership span (H12), and the
  unclassified pinned bass notes (E04/H16/H18).
