# Known R17 sweep replay at the hardening-round source

**Observed.** The known (contaminated, non-holdout) 120-case R17 sweep was re-executed once after
every hardening-round source change up to `96566a2` (U1, U2, rehearsed admission, the keys-hold
fix, cover-side repairs, the percussion surface, the fidelity dial and derived-harmony v2) and
compared with the frozen original using the calibrated `parse` of
`../../humanmusic-consolidation/known-r17-replay/classify.py` (imported without its
side-effecting tail).

| | original | this replay |
| --- | ---: | ---: |
| executed | 120 | 120 |
| complete passes | 61 | 61 |
| first-assertion failures | 59 | 59 |
| accepted pitch changed / off-lattice / new temporal claim / new held identity flip | 14 / 38 / 5 / 2 | 14 / 38 / 5 / 2 |

All 59 first-failure reasons and all 61 passing labels are identical (`comparison.json`: no new,
removed or changed case). The sweep runs the historical POCKET arm, which this round kept
byte-exact; the equality is consistent with that, not a generalization claim. `sweep.txt.gz` is the
complete process output (`gzip -dc sweep.txt.gz`).

The fixture compaction had archived the classifier's two inputs
(`humanmusic-r17/verification/fresh-failures.json`, `fresh-sweep.txt`); they were restored in place,
byte-identical to their archive manifest rows, because committed tooling reads them.
