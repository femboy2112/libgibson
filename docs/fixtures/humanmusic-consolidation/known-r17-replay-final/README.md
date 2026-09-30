# Known R17 sweep replay at the final consolidation source

**Observed.** The known (contaminated, non-holdout) 120-case R17 sweep was re-executed once at
`8a0b0cdb6fca1cf2efcebb295d920625d69cee0f` (after the cover repair `1a06575`, cover laws
`3216021` and holdout declaration `6ac812f`). The only dirty paths at execution were two Markdown
documents (`run.meta`), which do not enter the build.

| | original (`docs/fixtures/humanmusic-r17/verification`) | this replay |
| --- | ---: | ---: |
| executed | 120 | 120 |
| complete passes | 61 | 61 |
| first-assertion failures | 59 | 59 |
| accepted pitch changed / off-lattice / new temporal claim / new held identity flip | 14 / 38 / 5 / 2 | 14 / 38 / 5 / 2 |

All 59 first-failure reason strings and all 61 passing labels are identical to the frozen original
(no new, removed or changed failure case). Exit status 101 is the preserved known-red result.

Comparison used the calibrated `parse` from `../known-r17-replay/classify.py` without its
side-effecting tail (running that script with an argument would overwrite the committed
`../known-r17-replay/classification.json` and stamp the older source). `comparison.json` records
the result.

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 test --release --all-features \
  --test audio_pocket_integration r17_fresh_world_seed_tempo_sweep \
  -- --exact --ignored --nocapture --test-threads=1
```

Boundary: equal case/reason receipts show preservation of this known sweep's first failures, not
generalization, PCM identity (see the pocket freeze receipt) or musical acceptance.
