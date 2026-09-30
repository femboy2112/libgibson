# Cover listening corpus after the pinned-groove repair

**Observed.** The three `cover_music_lab --render` runs of the [original corpus](../listening/README.md)
(source-bound to `04e81fc`) were regenerated once, with identical inputs, at
`ced9833c3fc2cf55eae2bd4cd573aceb972777b6` — after `1a06575` (no groove debt for a pinned kit).
No seed, target or profile was replaced.

| Run | Declared machine checks pass | Preserved failed candidates | Infeasible targets | vs `04e81fc` |
| --- | ---: | ---: | ---: | --- |
| ode | 5 | 1 (`transposed_faster`: 3 temporal-function claims) | 0 | report byte-identical |
| swing-partial | 5 | 0 | 0 | report byte-identical |
| generated | 2 (`black_seed_1`, `black_seed_2`) | 1 (`vapor`: 1 temporal-function claim) | 2 (pinned harmony outside target vocabulary) | see below |

- **All 16 WAVs are byte-identical to the `04e81fc` corpus** (`WAV_SHA256SUMS` here equals
  `../listening/WAV_SHA256SUMS` digest for digest). The repair changed no audio.
- In the generated run only the `perf=` canonical fingerprints changed; every `score=` fingerprint is
  unchanged. `song` moved from `false` to `true` for all three generated candidates
  (`*.song.txt`: SongMapConformance PASS), which admits both BLACK_ICE candidates. VAPOR95 remains red on
  its separate temporal-function claim; SWISS_SIGNAL and `transposed_faster` remain explicit refusals.

The listening WAVs therefore remain the files under `target/humanmusic-cover/*-final/` (or the identical
`*-post-repair/` copies). Human recognition and musical quality are **UNVERIFIED**. The Swing & A Miss
outputs are a partial-cover skeleton (ordered relative harmony and section topology only); they carry
no source-melody recognition claim.

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 run --release --example cover_music_lab -- --reference=docs/fixtures/humanmusic-cover/ode-import/reference.tsv --voice=sop --out=target/humanmusic-cover/ode-post-repair --render
CARGO_BUILD_JOBS=1 cargo +1.98.1 run --release --example cover_music_lab -- --ordered-chart=docs/fixtures/humanmusic-cover/swing-partial/ordered-chart.tsv --out=target/humanmusic-cover/swing-partial-post-repair --render
CARGO_BUILD_JOBS=1 cargo +1.98.1 run --release --example cover_music_lab -- --out=target/humanmusic-cover/generated-post-repair --render
```

Text receipts are whitespace-normalized copies (trailing whitespace stripped); `run.log` is the
command log.
