# R17 pocket PCM freeze at the final consolidation source

**Observed.** `pocket_music_lab --render` was regenerated into a new directory at
`8a0b0cdb6fca1cf2efcebb295d920625d69cee0f` (after the cover repair and holdout declaration; only two
Markdown files were dirty) and compared with `scripts/verify-pocket-freeze.py` against the evidence
frozen at `207f0ab` before consolidation.

- 48/48 required full and stem WAVs byte-identical (`verify.json` lists every SHA256).
- 757/757 regenerated diagnostic and ledger text receipts byte-identical.
- Accepted BLACK_ICE R17 full mix: `6fe3abf55cee22b69e6d3c06684574770eea40445de4183588cfd12707382f4d` (unchanged).
- Verifier exit 0, `"pass": true`, no failures.

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 run --release --example pocket_music_lab -- \
  --out=target/humanmusic-r17/final-post-cover --render
python3 scripts/verify-pocket-freeze.py target/humanmusic-r17/final-post-cover
```

Boundary: byte equivalence to frozen evidence; not new listening acceptance. Factorial audio was not
regenerated (optional for the verifier). The WAVs remain local under `target/`.
