# HumanMusic Round XVII fixtures

`final/` (frozen pocket-freeze receipts and the 2^4 factorial cells), `baseline/`, `knockouts/`,
`support-no-go/`, `verification/`, `diagnostic-failures/`. This README only records how the files
are stored and how the frozen claim is still verified; it makes no new claim about the round.

## Storage

`final/` held 766 files (72.9 MB) but only 18.5 MB of distinct content: the factorial cells
`{black_ice,swiss}_f0000..f1111`, `*_stable_precursors` and the R14/R16/R17 arms repeat identical
per-arm dumps, also duplicated in `humanmusic-r16/`. No byte was discarded:

- `final/RECEIPTS_SHA256SUMS` pins the name and sha256 of **every** original `final/*.txt`
  receipt (`sha256sum -c` format; run it from `final/` after expanding).
- Files read by Rust (`tests/audio_consolidation_characterization.rs` reads
  `final/{black_ice,swiss}_r17.{notes,drums,continuity_links,phrase_plans,expression_decisions,occupancy,hearings,actions,pocket,phrase,conformance}.txt`)
  stay in place, byte-identical, as the canonical copies. Other exact duplicates point to them
  (`storage=inplace` in the manifest); unique dumps live in
  `docs/fixtures/humanmusic-archive/blobs.tar.xz`.
- `final/SHA256SUMS` (WAV hashes) is unchanged.

Frozen-claim check (byte-for-byte equivalent to the old one: it compares each regenerated
receipt's sha256 with `RECEIPTS_SHA256SUMS` and, on a mismatch, shows a diff against the original
recovered from the tree or archive):
`python3 scripts/verify-pocket-freeze.py <dir regenerated with pocket_music_lab --render>`.

- List what was compacted here: `grep '^docs/fixtures/humanmusic-r17/' docs/fixtures/humanmusic-archive/MANIFEST.tsv | cut -f1,4,5`
- Restore in place: `python3 scripts/fixtures/expand-humanmusic-archives.py expand` (or `--dest /tmp/x`).
- Verify every manifest hash: `python3 scripts/fixtures/expand-humanmusic-archives.py check --expanded`.
- Without Python: `mkdir /tmp/blobs && tar -xJf docs/fixtures/humanmusic-archive/blobs.tar.xz -C /tmp/blobs`
  and look the sha256 up in the manifest.

Policy and per-directory accounting: [`HUMANMUSIC_FIXTURE_MANIFEST.md`](../HUMANMUSIC_FIXTURE_MANIFEST.md).
