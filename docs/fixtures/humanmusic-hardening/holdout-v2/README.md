# Hardening holdout v2

Status: **configuration and harness committed after every hardening-round source change was frozen in
commits (source HEAD `1389e718b84c2cf9d9adcd3a486b0346d33a4737`); no case executed at commit time.** First-contact results are recorded
separately under `results/`. Holdout v1 (`../../humanmusic-consolidation/fresh/`) is known data and
stays immutable; v2 judges whether this round's repairs generalize.

Configuration SHA256: `3217c5188cb4c79810ad0dcee206f612d1bfd2e4867b3b4f0c19e9c008737143` (`config.tsv`, 48 rows, 20 tab-separated fields). Harness:
`tests/audio_hardening_holdout.rs` (its `inspect` observer block is v1's, byte for byte).

Coverage declared before contact:

- 36 generated sources across BLACK_ICE, SWISS_SIGNAL and VAPOR95 (straight and swung), all five
  grammars and three composers, Simple and Fusion, lengths 3.0-64 beats: shorter than a bar
  (3.0), exactly one bar (4.0), shorter than a phrase (5.0-9.25), partial final bars (6.5, 7.25,
  10.75, 13.5, 17.25, 18.75, 22.5, 26.5, 30.75, 34.25, 38.5, 44.25, 56.5) and bar-aligned pieces;
  Groove-declared pieces with and without a full bar of room, Motif-declared short pieces, and
  action-heavy 64-beat pieces.
- Source/target profiles: the historical `POCKET` arm (4 rows) and `BAND` at every drum restraint
  (Foundation, Balanced, Expressive, Busy).
- Selections: the established spec (`CoverSpec::established`), established without Groove, the binary
  motif-only spec, and every fidelity preset.
- Cross-world covers, target tonic/tempo/language changes, predicted lawful refusals (vocabulary or
  mode change), 8 external Ode targets (every preset; with and without the declared derived harmony;
  the v1 motif-only spec) and 4 Swing partial-chart rows (Loose/Interpretive declared to leave nothing
  to cover; Faithful/Strict the skeleton).
- 84 seeds (92xxxxxx), all distinct; a word-bounded search found none in `src`, `tests`, `examples`,
  `docs` or `scripts`, nor inside the compacted fixture archive, before this configuration existed.

## Declared expectations

`expect` is part of the pre-registration: `lift` must lift and conform; `refusal` must be an explicit
typed lawful refusal; `lawful` accepts either; `nothing` requires that no map survives the preset. The
lawful refusal family is fixed in the header of `config.tsv` and in `lawful_refusal`. A refusal outside
it, or at the wrong stage, fails.

## Pre-freeze harness review

An adversarial read-only review (no execution) found, and root corrected before this commit:

- **Swing Form.** The first draft required Form to be Unknown for the Swing chart, which pins its section
  topology by design: S03/S04 would have failed regardless of the architecture. Corrected per source.
- **A library honesty bug, not only a harness bug.** `CoverMap::extract_fidelity` read ANY extraction
  error as observed absence ("the source sounds no such line"), so a timing failure could become a
  zero-identity cover that passes. Fixed in the library with a falsifier (`ad13972`, `1389e71`), and
  the harness now fails a lift whose map pins nothing (`identity-present`).
- **Guards.** `percussion-law`, `ceiling-honest`, `derived-harmony-provenance` and
  `derived-harmony-labelled` can only fail by a library regression (they restate construction); they
  are labelled `guard:` and are not independent evidence. The swing `metric-length` receipt is a
  literal `true` in the ordered-chart check.
- **Expectations revised before contact.** E05/E06 are the first Ode lifts pinning melody and bass
  together; the pinned-attack harmony search has never met SATB simultaneities, so a "no lawful harmony"
  refusal would be lawful: declared `lawful`, not `lift`.

Retained risks, declared before contact: `AnchorReport` has never judged a cover's own song (a pinned
Silent kit under a Groove-declaring target grammar would read as a cover-side anchor violation); the
external rows' extraction outcomes are already asserted by unit tests and carry little held-out
information; there is no per-case timeout (the process runs under an outer `timeout`).

## Execution boundary

Compilation alone is permitted before freezing and executes no case:

```sh
CARGO_BUILD_JOBS=2 cargo +1.98.1 test --release --test audio_hardening_holdout --no-run
```

After this commit, root executes once, into a new directory, preserving the complete log and exit:

```sh
HUMANMUSIC_HOLDOUT_OUT=target/humanmusic-hardening/holdout-v2-results \
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 \
cargo +1.98.1 test --release --test audio_hardening_holdout hardening_holdout_v2 -- --ignored --exact --nocapture
```

No source, gate or configuration changes after contact. A catastrophic universal defect would be
preserved, repaired only after its counterexample, and judged by a separately declared holdout v3.

## Independence limits

The generator and most observers share repository provenance; this is fresh input coverage for the
hardened implementation, not an independent implementation or a listening test. The Ode fixture, its
parser and the Swing chart were inspected and used in development; only the new target combinations are
held out. The finite, deliberately chosen matrix is not a population sample. No result establishes that a
person recognizes, or likes, a cover or a drummer.
