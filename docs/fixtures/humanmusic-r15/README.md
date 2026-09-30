# HumanMusic Round XV fixtures

Frozen Round XV expressive-arm outputs (`final/`, `frozen-ruler.txt`, ...). This README only
records how the files are stored; it makes no new claim about the round.

## Storage

The large per-arm text dumps in `final/` (notes, hearings, decisions, ...), and exact duplicates
of them, are stored compacted; no byte was discarded. Every original path, byte count and sha256
is in `docs/fixtures/humanmusic-archive/MANIFEST.tsv`; the unique bytes are in the solid archive
`docs/fixtures/humanmusic-archive/blobs.tar.xz` (member name = sha256). Policy:
[`HUMANMUSIC_FIXTURE_MANIFEST.md`](../HUMANMUSIC_FIXTURE_MANIFEST.md).

- List what was compacted here: `grep '^docs/fixtures/humanmusic-r15/' docs/fixtures/humanmusic-archive/MANIFEST.tsv | cut -f1,4,5`
- Restore in place: `python3 scripts/fixtures/expand-humanmusic-archives.py expand` (or `--dest /tmp/x`).
- Verify every manifest hash: `python3 scripts/fixtures/expand-humanmusic-archives.py check --expanded`.
- The WAV hashes in `final/SHA256SUMS` are unchanged (audio was never tracked in the repository).
