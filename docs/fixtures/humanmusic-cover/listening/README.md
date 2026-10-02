# Cover listening corpus at 04e81fc

These are the original serial lab outputs, with no replacement seeds or post-contact fitting. WAVs remain local under `target/humanmusic-cover/`; hashes and structural/audio metadata are in `receipt.json` and `WAV_SHA256SUMS`. Human recognition and musical quality are **UNVERIFIED**. The render command returned successfully even when a candidate failed its explicit pipeline checks.

| Run | Declared machine checks pass | Preserved failed candidates | Infeasible targets |
| --- | ---: | ---: | ---: |
| ode | 5 | 1 | 0 |
| swing-partial | 5 | 0 | 0 |
| generated | 0 | 3 | 2 |

Every generated candidate passed its selected cover-axis projection. Failures remain failures: Ode transposed/faster has three temporal-function claims; all three generated-source candidates have `song: false`, and its VAPOR candidate also has one temporal-function claim. The two infeasible generated-source targets violate the pinned-harmony target vocabulary.

The Ode map pins observed soprano motif, canonical attacks and source rest boundaries; unavailable harmony/form/groove/bass remain Unknown. It renders the observed source melody plus six candidate interpretations, including original G-major/100-BPM frame. Every comparison to the first candidate reports changes in support voicing, dynamics, percussion detail and interaction choices.

Swing outputs preserve only the supplied ordered relative harmony and section-family topology. Melody and exact timing remain Unknown; one target bar per chord and the tonic intro are explicit generation assumptions. Passing this partial relation does not establish a recognizable cover of Swing & A Miss.

Generated-source outputs pin motif, groove, harmonic contour and form. All available outputs preserve those coordinates and vary all four measured free-coordinate groups, but their failed song/pipeline checks prevent a lawful-cover claim.

Source facts and normalization limits are recorded in receipt.json, including the raw 245-versus-238 symbolic mismatch and declared seven-unison staff projection. Whitespace-normalized text copies retain original byte hashes; any changed original is also preserved as deterministic gzip in raw/. `collect.py` only collects existing completed files and refuses to overwrite an existing receipt.

Listen without titles where possible: is the recognized melody the same song? Do different bands interpret it rather than merely change patches? The partial skeleton cannot answer source-melody recognition. Accepted R17 BLACK_ICE preservation is a separate frozen comparison, not established by these cover outputs.

## Storage

Large text dumps in this directory, and exact duplicates of dumps kept elsewhere, are stored
compacted; no byte was discarded. Every original path, byte count and sha256 is in
`docs/fixtures/humanmusic-archive/MANIFEST.tsv`; the unique bytes are in the solid archive
`docs/fixtures/humanmusic-archive/blobs.tar.xz` (member name = sha256). See
[`HUMANMUSIC_FIXTURE_MANIFEST.md`](../../HUMANMUSIC_FIXTURE_MANIFEST.md) for the policy.

- List what was compacted here: `grep '^docs/fixtures/humanmusic-cover/listening/' docs/fixtures/humanmusic-archive/MANIFEST.tsv | cut -f1,4,5`
- Restore every compacted file to its original path and bytes:
  `python3 scripts/fixtures/expand-humanmusic-archives.py expand` (or add `--dest /tmp/x` to
  grep a copy instead of touching the tree; expanded files are working copies, do not commit them).
- Verify all manifest hashes: `python3 scripts/fixtures/expand-humanmusic-archives.py check --expanded`
  (after `expand`, any older `sha256sum -c` manifest in this directory passes again).
- Without Python: `mkdir /tmp/blobs && tar -xJf docs/fixtures/humanmusic-archive/blobs.tar.xz -C /tmp/blobs`, then look up the
  member named by the sha256 in the manifest.
