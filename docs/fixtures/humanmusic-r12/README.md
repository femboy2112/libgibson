# Round XII receipts

Starting source: `9ccb84b4462607eb04e10f0f98ecc6f9e5585c03`.
Branch: `feat/v0.4-humanmusic-audio`; main/base `7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`.

`baseline-failures.txt` records the actual pre-intervention run:

```sh
cargo +1.98.1 test --lib r12_ -- --include-ignored
```

Four desired contracts fail: physical suspension evidence, earned arrival,
literal chord-tone landing, and next-target influence. A fifth probe passes by
showing the old local classifier returns the same extension label for an orphan
and a potentially owned common-tone color. The initially ignored failing probes
are enabled or replaced by calibrated positive/negative probes as implementation lands.

`next-targets-baseline.txt` records the three original references: declaration,
construction, and test. There is no generation read at the starting tip.

These are source/execution observations, not estimates of perceived musical quality.
The maintainer's audition is the evidence that the old zero-unjustified receipt
was insufficient. Three Astra/High scouts inspected separate modules in the same
repository; they share source provenance and are not three independent auditions.

## Observer revision

`baseline/` is the literal pre-generation diagnostic checkpoint. After integration,
a probe exposed an observer omission: an immediate bass successor in a quoted
material remains the same monophonic voice. The final observer connects those
neighbors, keeps material endpoint contracts separate, and has an octave-jump
negative control. It also explicitly rejects restruck notes as physical holds.
Final comparison reports re-audit **both** arms with this same observer; the four
old score fingerprints remain identical. Connecting material boundaries removes
one false approach finding. Requiring the held event to end by its resolving
successor exposes two additional carry defects in each deflected score. The net
old deflected false-function counts change from 7/9 to 9/10 (Swiss/BLACK_ICE).
The initial receipt is retained.

Final reports/notes/change ledgers are under `final/`. Full score Debug dumps and
WAV files remain local in `target/humanmusic-r12/final/`; the manifest binds hashes.
`*.good-paths.txt` lists temporally supported non-chord-function examples, and does
not erase separate vertical slice warnings.

`anticipation-fixture-failure.txt` retains a final-gate failure: an older negative
fixture ended exactly at the harmonic boundary, which the new positive contract
correctly accepts. Its gate was shortened to release before the change; the
disconnected-successor rejection and exact-boundary acceptance both pass in
`pitch-tests.txt`. This final correction only changes test data, not realization.

Committed console logs normalize trailing whitespace only; the manifest also
records their raw pre-normalization SHA256 values. Numeric receipts are unchanged.

## Storage

Large text dumps in this directory, and exact duplicates of dumps kept elsewhere, are stored
compacted; no byte was discarded. Every original path, byte count and sha256 is in
`docs/fixtures/humanmusic-archive/MANIFEST.tsv`; the unique bytes are in the solid archive
`docs/fixtures/humanmusic-archive/blobs.tar.xz` (member name = sha256). See
[`HUMANMUSIC_FIXTURE_MANIFEST.md`](../HUMANMUSIC_FIXTURE_MANIFEST.md) for the policy.

- List what was compacted here: `grep '^docs/fixtures/humanmusic-r12/' docs/fixtures/humanmusic-archive/MANIFEST.tsv | cut -f1,4,5`
- Restore every compacted file to its original path and bytes:
  `python3 scripts/fixtures/expand-humanmusic-archives.py expand` (or add `--dest /tmp/x` to
  grep a copy instead of touching the tree; expanded files are working copies, do not commit them).
- Verify all manifest hashes: `python3 scripts/fixtures/expand-humanmusic-archives.py check --expanded`
  (after `expand`, any older `sha256sum -c` manifest in this directory passes again).
- Without Python: `mkdir /tmp/blobs && tar -xJf docs/fixtures/humanmusic-archive/blobs.tar.xz -C /tmp/blobs`, then look up the
  member named by the sha256 in the manifest.
