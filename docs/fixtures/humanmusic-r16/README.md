# HumanMusic Round XVI fixtures

Frozen Round XVI phrase-surface outputs (`final/` R14-R16 arms, `frozen/` R14/R15 comparison
dumps, holdout sweeps). This README only records how the files are stored; it makes no new claim
about the round.

## Storage

The large per-arm text dumps in `final/` and `frozen/` (notes, drums, phrase, voicing, ...), and
exact duplicates of them, are stored compacted; no byte was discarded. Every original path, byte
count and sha256 is in `docs/fixtures/humanmusic-archive/MANIFEST.tsv`; the unique bytes are in the
solid archive `docs/fixtures/humanmusic-archive/blobs.tar.xz` (member name = sha256). Policy:
[`HUMANMUSIC_FIXTURE_MANIFEST.md`](../HUMANMUSIC_FIXTURE_MANIFEST.md).

- List what was compacted here: `grep '^docs/fixtures/humanmusic-r16/' docs/fixtures/humanmusic-archive/MANIFEST.tsv | cut -f1,4,5`
- Restore in place: `python3 scripts/fixtures/expand-humanmusic-archives.py expand` (or `--dest /tmp/x`).
- Verify every manifest hash: `python3 scripts/fixtures/expand-humanmusic-archives.py check --expanded`.
- The WAV hashes in `final/SHA256SUMS` are unchanged (audio was never tracked in the repository).

Kept uncompacted on purpose: `first-holdout-sweep.txt`, `holdout-sweep.txt` and
`historical-r14-sweep.txt` (first-contact holdout evidence), and the WAV manifests. The dev
scripts `scripts/dev/humanmusic_r17_witnesses.py` and `humanmusic_r17_receipts.py` read
`final/*.txt`; they call `ensure_expanded` and restore any missing file from the archive before
use (so running them leaves untracked working copies under `final/`).
