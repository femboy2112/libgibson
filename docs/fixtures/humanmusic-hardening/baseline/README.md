# Hardening-round baseline (before any source edit)

**Observed.** At `a84ca083144c46a5fd14d060ce980f1dce788ef3`, before any library change in the hardening
round, the listening corpus and the accepted R17 pocket were regenerated into new directories and
compared with the committed evidence. The only edit was render-only: `cover_music_lab --stems` adds a
drums-only bus next to every full mix so later percussion changes can be heard and hashed in isolation.

- Accepted R17 pocket (`pocket_music_lab --render`, verified with `scripts/verify-pocket-freeze.py`):
  48/48 required full and stem WAVs and 757/757 text receipts byte-identical (`r17-verify.json`).
- Cover listening corpus (ode, swing-partial, generated): all 16 full-mix WAVs byte-identical to
  `../../humanmusic-cover/listening/WAV_SHA256SUMS`, and all 20 text receipts equal to
  `../../humanmusic-cover/listening-post-repair/` (trailing whitespace normalized). Adding the full stem
  mask explicitly did not change a byte.
- 16 drums-only stems are new; their hashes are the percussion baseline for the drum A/B
  (`WAV_SHA256SUMS`, `*.drums.wav`).
- Historical control adapters are pinned by `tests/audio_consolidation_characterization.rs` and
  `tests/audio_canonical_fingerprint.rs`; they are re-run at every gate of this round.

```sh
cargo +1.98.1 build --release --example cover_music_lab --example pocket_music_lab
B=target/humanmusic-hardening/baseline
./target/release/examples/cover_music_lab --reference=docs/fixtures/humanmusic-cover/ode-import/reference.tsv --voice=sop --out=$B/ode --render --stems
./target/release/examples/cover_music_lab --ordered-chart=docs/fixtures/humanmusic-cover/swing-partial/ordered-chart.tsv --out=$B/swing-partial --render --stems
./target/release/examples/cover_music_lab --out=$B/generated --render --stems
./target/release/examples/pocket_music_lab --out=$B/r17 --render
python3 scripts/verify-pocket-freeze.py $B/r17
```

Boundary: byte equivalence only; no listening claim. WAVs stay local under `target/`.
