# Closeout listening corpus

**Observed** at the recorded sources. WAVs stay local under `target/humanmusic-closeout/listening/`;
their SHA256s are in `WAV_SHA256SUMS`. Nothing here claims a musical result — the maintainer listens.

## What to audition (only what this round changed)

12 fresh BAND songs, each the falsifier a closeout law was derived from, **before** (pre-round head
`9204ef4`) and **after** (final library source `abc3f9f`): a full mix plus the stem the law touches
(`*.full.wav`, `*.pad|keys|bass|lead.wav`). 24 of the 25 rendered files per side differ; the one
identical file is the swell song's keys stem (the pad answers that case — see `CHANGES.md`). Every
**before** take fails its own receipt (a held-identity flip, a false appoggiatura claim, or a missing
bass figure); every **after** take passes. `CHANGES.md` explains each change from note-level diffs.

Also preserved for the same listen: the 8 Ode-fidelity mixes the **previous** round changed
(`after/ode-fidelity/`, byte-identical to `9204ef4`) and the drum A/B (20 WAVs, unchanged).

## Reproduce

```sh
# after (final head)
HUMANMUSIC_CLOSEOUT_LISTENING_OUT=target/humanmusic-closeout/listening/after/falsifiers \
  cargo +1.98.1 test --release --test audio_closeout_listening -- --ignored --nocapture
cargo +1.98.1 build --release --example drum_restraint_lab --example cover_fidelity_lab
./target/release/examples/drum_restraint_lab --out=target/humanmusic-closeout/listening/after/drums --render
./target/release/examples/cover_fidelity_lab --out=target/humanmusic-closeout/listening/after/ode-fidelity --render
# before: the same commands in a worktree at 9204ef4 with tests/audio_closeout_listening.rs copied in
```
