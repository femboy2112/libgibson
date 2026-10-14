# C137_ACCEPTED_V1 — the immutable acoustic regression lock

This fixture freezes the **accepted** C137 HumanMusic output as an external-to-candidate
reference, so future engine work (the smooth-pop domain, Johnny Five initiative, 32-bar forms)
cannot silently degrade music the maintainer has already accepted by ear.

**Why it exists:** before this fixture there was **no WAV-byte lock anywhere** in the tree. Every
C137 test in `tests/audio_c137_song.rs` is *relational* (A == B rebuilds); the only hard-pinned
hash in the repo was one unrelated `SongMap` canonical value. Comments in the test file that claim
frozen fingerprints are "pinned in the canonical-fingerprint and PCM gates" describe a gate that
did not exist. This fixture is that gate.

## What is frozen

Each lock level is one `c137_song` invocation (common flags `--seeds=2112,7,99 --tempo=144`), with
block size 256 and `SampleRate::STUDIO` (48 kHz), both hardcoded in the example's render path.

| Level | Artifact | Extra flags | Acceptance scope |
| --- | --- | --- | --- |
| A1 | 8-bar short/plain | `--jerry=short,plain` | archival control (old default); **not** ear-gated as music |
| A2 | 16-bar plain | `--jerry=plain` | characterized control |
| B | 16-bar **Full** | *(none — the public default)* | **EAR-ACCEPTED** (PR #111); provenance-verified |
| C1 | 32-bar song | `--form=song` | characterized control — **not** ear-approved |
| C2 | 32-bar song-intro | `--form=song-intro` | characterized control — **not** ear-approved |
| D1 | 32-bar narrative bridge | `--form=song-narrative` | **EAR-ACCEPTED** ("sounding MUCH BETTER :D") |
| D2 | 32-bar narrative intro | `--form=song-intro-narrative` | **EAR-ACCEPTED** ("hell yeah") |

`SHA256SUMS` pins, per level × seed:
- `full.normal.wav` — the rendered stereo PCM16 bytes (the acoustic lock).
- `full.nolead.wav` — the lead-muted survival render (levels B, D1, D2 only; the ear-relevant arms).
- `score.tsv` — the full audible-event dump (a third, human-diffable layer: it explains *why* a
  hash moved).

`MANIFEST.tsv` additionally records the `Score::fingerprint()` and frame count per level × seed.

## Provenance (verified, not assumed)

Level **B** (16-bar Full) was rendered by two independent binaries at different commits and matched
**byte-for-byte** on all three seeds:
- candidate `impl/humanmusic-smooth-pop-2026-10-14` (off `b536699`, PR #114 head), and
- accepted parent `f68c9bf` (PR #111, the branch the maintainer accepted by ear).

So `C137_ACCEPTED_V1` is the ear-accepted music, not merely "whatever today's build happens to emit."

## The fingerprint-vs-WAV distinction (load-bearing)

`Score::fingerprint()` (`src/audio/human_music/score.rs:539`) is FNV-1a over
`notes | drums | sfx | chords | melody_repairs | melody_rejudged` — the audible-event content. It
**excludes** `tempo_bpm`, `total_beats`, `beats_per_bar` and `sections`. A tempo change therefore
moves the WAV but **not** the Score fingerprint. That gap is exactly why this lock also pins tempo
(in `MANIFEST.tsv`) and freezes the rendered WAV bytes, not just the Score fingerprint.
`tests/audio_c137_accepted_v1_lock.rs` calibrates this blindness explicitly (red-first).

## Reproduce / verify

```
scripts/verify-c137-accepted-v1.sh            # renders via the real binary, sha256sum -c
scripts/verify-c137-accepted-v1.sh --calibrate # red-first: corrupt 1 byte, prove the check FAILS
```

The WAVs themselves are **local-only by repo policy** (see
`docs/research/HUMANMUSIC_C137_LONGFORM_INTEGRATION.md`); only the hashes are committed. Re-render
with the exact flags above to regenerate the audio for an A/B listen.

## Discipline

- **A baseline mismatch is a STOP / triage, not permission to rebaseline.** Investigate the diff
  (the `score.tsv` layer tells you if it is an audible-content change or a synth/master change).
  Only re-freeze after the change is understood, intended, and (for an ear-gated level) re-accepted.
- A generated candidate must **never** update its own goldens in the same command. The verifier only
  reads `SHA256SUMS`; it never writes it.
