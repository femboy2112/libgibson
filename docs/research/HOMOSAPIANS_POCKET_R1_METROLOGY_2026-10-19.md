# THE HOMOSAPIANS — R0/R1 pocket metrology verdict packet

**Status: measured evidence + a loudness-matched A/B. NOT an accepted audio change. The ear is the
only promotion gate; no default is changed and no C137 artifact moves. October 19, 2026.**

## Provenance
- Branch `impl/homosapians-pocket-commute-2026-10-19`, worktree `/home/leah/LibGibson.pocket`,
  built on the audit HEAD `ac63e6b` (PR #124), which sits on the rehearsal-composer impl
  `e6fbaff` (PR #123).
- Toolchain pinned `cargo +1.99.0`, release built `LTO=off CODEGEN_UNITS=16` on the 8-core / ~4 GB box.
- Instrument: `examples/homosapians_pocket_lab.rs` (new, opt-in; touches no default and no library ABI).
- Reference tempo 104 BPM (the pop feel), warm `pop_world` palette, `SampleRate::STUDIO` (48 kHz).

## R0 — truth gate (G0, G9): GREEN, verified from disk
| check | result |
| --- | --- |
| P1–P4 characterization (`audio_homosapians_pocket_naturality_audit`) | **4 passed, 0 failed** |
| rehearsal regression (`audio_homosapians_rehearsal`) | **19 passed, 0 failed** |
| C137_ACCEPTED_V1 lock (`scripts/verify-c137-accepted-v1.sh`) | **51/51 byte-for-byte OK** |

The pocket pathologies reproduce on the real frozen Score; the baseline is certified before any edit.

## The source-grounded diagnosis (every anchor re-verified, not taken from prose)
- `kernel.rs:906` `const KEYS_ATTACK_COMP: f64 = 0.03;` — a **beats** constant.
- `kernel.rs:1652` riff-comp onset `at = (at_grid + pocket(posin) - KEYS_ATTACK_COMP).max(0.0)`.
  Harmony is read at `bar_start = (at_grid/4).floor()*4` — the **grid** bar — while the note *sounds*
  at `at` (grid − 0.03). On a barline the chord is the *next* bar's, the time is the *previous* bar's.
- `rehearsal.rs:1216` required Keys carrier: `onset + pocket_offset(onset)`, **no subtraction** → the
  comp and the carrier disagree by exactly 0.03 beat on a shared slot (the self-flam, P3).
- The kernel comment (`kernel.rs:902`) justifies the pull as compensation for a "felt attack lag"
  because "the FM e-piano speaks a hair late through the chorus + room."

## R1 — the physical claim, measured (G1)
The VAPOR95 Keys patch (`world.rs:344-358`) has a **3 ms linear** amplitude attack. The warm chorus
(`fx.rs:128-170`, `world.rs:386-390`) is **blend, not replace**: `dry·(1−mix) + wet·mix`, mix 0.4 —
the dry attack arrives **on time** at 0.6 gain; only a quieter wet copy trails 14.5–21.5 ms. Reverb's
earliest wet is ~35 ms, dry undelayed, mix 0.20. So the DSP produces **no 17 ms onset lag** to cancel.

Measured onsets (rectified 1 ms envelope, 25%-of-local-peak threshold; centroid over [−2, +30] ms):

| seed | dry-keys attack | dry-kit (kick) attack | chorus adds to attack edge | comp notes | P2 barline-crossings |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 7    | 1.60 ms | 0.22 ms | <2 ms | 52 | 9 |
| 42   | 2.13 ms | 0.22 ms | <2 ms | 52 | 17 |
| 2112 | 1.65 ms | 0.22 ms | <2 ms | 38 | 13 |
| 9    | 1.28 ms | 0.22 ms | <2 ms | 45 | 9 |
| 13   | 1.29 ms | 0.22 ms | <2 ms | 44 | 6 |

**Reading:** the Keys dry attack lands ~1.5 ms after its scheduled sample; the kick ~0.2 ms; the
chorus shifts the *attack edge* by under 2 ms (its 14–22 ms energy sits in a quieter trailing copy, so
the centroid reads ~16 ms but the heard *onset* does not). There is no physical latency near 17 ms.

**Therefore** `KEYS_ATTACK_COMP` is not a latency correction. Where a comp note shares a slot with a
kick (P1, test-exact), the comp's scheduled lead is 0.03 beat = 17.31 ms; acoustically its attack is
heard ~16 ms *ahead* of the kick. The constant plays the comp early rather than aligning it. And
because it is in **beats**, it scales with tempo (22.5 ms @ 80, 17.31 @ 104, 12.86 @ 140) — the
opposite of how a real patch/chorus latency (fixed in seconds) would behave. This is the architectural
bug the audit named: a physical correction wearing beat-clothes, applied unconditionally.

## The controlled A/B (G10 — the ear decides)
Single isolated factor. Everything else — pitches, riff, chords, bass, drums, voicing, tempo, world,
palette — is byte-identical across arms. The timing treatment is a **Score-level** shift of every
`comp` note by +0.03 beat (no const edit, no note added, no pitch moved), so the comp rides the shared
pocket exactly like the carrier already does.

- `A_current.wav` — the shipping sound (comp pulled 0.03 beat early).
- `B_noadvance.wav` — comp locked to the shared pocket.
- Both full warm band, chorus on, **RMS loudness-matched to ±0.000 dB** (verified from the WAVs).
- 2×2 diagnostic on seed 2112: `2x2_{timingON,timingOFF}_{chorusON,chorusOFF}` — all four at −18.49 dBFS.
- Metrology stems per seed: `stem_dry_keys`, `stem_chorus_keys`, `stem_dry_kit`.
- Per-comp-note receipt `receipt.tsv`; PCM hashes in `SHA256SUMS.lab`.

**What to listen for:** does the keyboard sit *inside* the kit's pocket in B, where in A it rushes the
beat / smears against the pad on the chord changes? If B is tighter → strong result, R2 lands the typed
fix. If A is actually the groove you want (a deliberate lean) → the fix is to make that lean an
*authored, typed* push instead of an accidental constant, and we keep it. Either answer is usable.

## Section-11 summary
- **What changed (candidate, not yet promoted):** remove the unconditional 0.03-beat early pull on the
  riff-comp; let comp share the carrier's pocket. Demonstrated here as a Score-level A/B.
- **Why:** the pull was introduced as physical-latency compensation; measurement shows no such latency
  (dry attack ~1.5 ms, kick ~0.2 ms, chorus <2 ms). It therefore plays the comp ~16 ms early and drags
  next-bar chords into the previous bar (6–17 times/song).
- **Who/what authorizes it:** nothing in the source authorizes an anticipation here — the comp is
  meant to *lock* to the kit; the carrier already does. An intentional push would have to be authored.
- **What verifies the effect:** P1/P3 tests (exact 0.03 beat), the onset table above (dry vs kit), the
  P2 crossing counts, and byte-distinct loudness-matched A/B renders with PCM hashes.
- **Uncertainty:** the aesthetic verdict is unmeasured — only Leah's ear promotes an arm. The lab's
  "nearest-kick" mean is a loose proxy and is not used for the claim. Full-band onset is non-additive
  from stems (saturation/comp/limiter are level-dependent); the clean onset evidence is the dry stems.

## Next (gated)
- **R2** (typed fix) lands *after* the ear verdict, in the direction it confirms: an opt-in
  `pocket_time.rs` carrying a typed Beats×Seconds displacement (`E_β(b,s)=60b/β+s`), so a musical push
  (beats) and any real patch latency (seconds) can never again be conflated in one float. Default stays
  byte-identical; C137 lock stays green.
- **R3–R5** (CompGesture ownership witness, DrumGesturePlan agency, song-form witness) proceed as
  opt-in, ear-independent categorical scaffolding.
