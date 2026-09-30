# Known R17 replay — classification boundary

Exact executed source: 44e2d7efabc28e684701240b5466d06a91d541dd. This is deliberately the consolidated core before evolving cover implementation. The old test and assertions are unchanged. No fresh holdout is being consumed.

The original result is 120 executed / 61 complete invariant passes / 59 first-assertion failures. Parser calibration against the original raw log reproduces all59 `(case, reason)` records exactly. Observed replay: 120 executed / 61 complete invariant passes / 59 first-assertion failures, exit101 preserved. All59 first-reason strings and all61 passing case labels are identical to the original receipts. No new/removed/changed failures. Runtime18.377 seconds, Rust1.98.1 release, one test thread.

## Four families

| Family | Original count | Decomposition | Claim boundary |
|---|---:|---|---|
| Accepted-output pitch differences |14|10 SWISS_SIGNAL,4 BLACK_ICE|Frozen accepted-output comparator remains failed; authored pitch and historical repaired pitch are different coordinates.|
| Transported-lattice |38|All VAPOR95; all first witnesses Keys,26 `comp`,12 `figure`|The inherited structural metric onset is not automatically a swung performed slot. New typed coordinates do not move these frozen notes.|
| New temporal-function claims |5|All BLACK_ICE, MeaningDirected demo at76BPM; seeds3/7/19/43/101|Potential real source defect; old repair supporting a held note cannot be replaced by a relabeling.|
| New held identity flips |2|VAPOR95, StablePropulsion story, seed43,71/59BPM|Actual current diagnostic failure. Two tempo conditions share one composition; they are not independent songs.|

### Accepted pitch coordinates

SongMap owns authored form/theme/harmony/closure coordinates. A realized source event may subsequently be substituted by the archived R15/R16 expression repair. That repaired accepted output is a historical performance coordinate, not automatically the authored identity.

The committed `humanmusic-r17/verification/fresh-classification.md` and raw probe establish two distinct local mechanisms: SWISS story seed3/106BPM authored F4 becomes G4 in R15/R16 while R17 retains/re-times F4; SWISS demo seed3/106BPM retains the keys' voice path but new lead attacks alter comp-slot admission, changing alternating voice count from2 to3 at67.25. Both continue failing the literal frozen comparator. This does not establish that every one of the14 cases has the same cause, nor declare all changes musically acceptable.

### VAPOR coordinates

`MetricPosition` is exact rational quarter-note identity. `GrooveTransport` maps to a tagged `PerformedPosition`; its metric projection is exact. The historical sparse eighth transport is not a bijection on arbitrary real-valued beats, so arbitrary imported grids must check monotonicity. Existing structural Keys events can retain canonical beat0.5/16.5 while the all-onset R17 witness expects transported eighth positions. The old assertion remains untouched and these cases remain failures. No note was moved and no witness weakened to improve counts. Full note-level typed provenance is still an incremental migration, not a claim every old f64 has been reinterpreted canonically.

### Temporal claims

The existing source replay establishes BLACK demo MeaningDirected seed3/76BPM structural bass D2 at62/gate1.35 crosses G→F#7 at63. R15/R16 substitute authored C2 at63.5 with C#2, supplying D2's stepwise held resolution; R14/R17 retain C2 and lose that justification. This is a source-path defect historically masked by substitution, not evidence that preserving source pitch automatically makes every held-function claim lawful. The five first failures stay red. The original probe directly diagnoses one case; sharing trace/composer/tempo is not independent proof of every seed's internal cause.

### Held identity

Both surviving first witnesses are chart F-major6 heard as A minor beginning114.25, with held run0.9850483623016479 seconds at71BPM and1.1397392152101933 seconds at59BPM. No hold threshold was changed. The observation policy repair addresses unlinked modern envelope masking; the historical R17 explicit-graph path is unchanged. No universal source repair for this family is claimed.

## Evidence policy

Known failures are regression/classification evidence, not optimization targets. Equal case/reason receipts support preservation within this executed boundary, not generalization or human acceptance. Only the maintainer's accepted BLACK_ICE flagship supplies the stated pocket listening evidence.


## Reproduction and files

The test was compiled from a clean detached worktree at the exact source above, with136 source/build/test files guarded before execution. `source-manifest.json` records hashes and toolchain. `build.jsonl` and `build.txt` preserve the successful build. `run.json` records command, binary hash, source guard, runtime and exit. `sweep.txt` is unedited combined stdout/stderr, including all panic evidence. `classification.json` preserves every failed case/reason and passing label plus the comparison against the original.

Binary SHA256: `b9c60cbc30b2b21bbee554baaf010b143160adf65303bdeaf715ddecc8e45427`.

```sh
# Run at the exact detached source; exit101 is the preserved known-red result.
CARGO_BUILD_JOBS=1 cargo +1.98.1 test --release --all-features \
  --test audio_pocket_integration r17_fresh_world_seed_tempo_sweep \
  -- --exact --ignored --nocapture --test-threads=1

# Reclassify saved evidence, without executing the sweep.
python3 docs/fixtures/humanmusic-consolidation/known-r17-replay/classify.py \
  docs/fixtures/humanmusic-consolidation/known-r17-replay/sweep.txt
```

The parser first validates itself against the committed original raw output and JSON. It strips only the standard single-thread test-harness prefix from a PASS line; it does not edit failure reasons. The known sweep is a historical archival/expensive invariant test, opt-in with `--ignored`; it performs no PCM render. No new holdout ran here.
