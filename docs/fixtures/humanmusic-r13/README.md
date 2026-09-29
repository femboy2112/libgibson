# Round XIII receipts — temporal mass

Starting source: `d72b72f1d4d4c670f3ac2f2a5a3ccd808051d911`.
Branch: `feat/v0.4-humanmusic-audio`; main/base `7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`.
PR #70 remains draft. No package, ABI, MSRV or release change.

## Commit 1 — failing witnesses only

`baseline-failures.txt` is the literal pre-intervention run of the four ignored falsifiers in
`src/audio/human_music/mass_witnesses.rs`. No production source changed. Each witness fails
because the Round XII observer returns an **identical** verdict for two events that differ
only in time:

| Witness | Pair judged identical by Round XII |
|---|---|
| SWISS 15–17 s / 43–45 s | keys D5 struck at beats 32 and 88 vs the offbeat D5 comp stab at 30.75 (all `Orphaned OrphanColor`) |
| Tempo dilation | the same one-beat pad D over C6 at 160 and 88 BPM |
| Fast slide vs slow staircase | C# inside C–C#–D–D#–E at 1/4 beat vs 1 beat per step |
| Long orphan ninth | a D over C6 held from the harmony's arrival vs a 0.2-beat weak D stab (keys and pad) |

The authored control inside the last witness (the same held D over `Cadd9`, whose written
chord contains the ninth) already passes and must keep passing.

The staircase witness first asserted that Round XII lets the fast C# inherit the slide. That
assertion failed: Round XII marks the fast C# suspect too, because the D it moves through is
a palette tension, which the audit labels `OrphanColor` at any speed, and the suspicion is
propagated to the connector. The witness now asserts the actual defect (the same verdict for
both parameterizations). The first form was not committed; this note preserves it.

## Source observation that changes the witness reading

The SWISS witnesses are written as a 1.94-beat and a 3.90-beat keys "hold". Written duration
is not what the listener receives on these patches:

- SWISS keys patch `adsr = (0.002, 0.18, 0.0, 0.1)` and BLACK_ICE keys `(0.003, 0.14, 0.0, 0.1)`
  have zero sustain. `sonority::audible_end` (Round VIII) puts every keys note at
  `a + (30/40)·d` = 0.137 s (SWISS) / 0.108 s (BLACK_ICE) above the 30 dB floor, whatever its
  written length. SWISS lead `(0.003, 0.12, 0.0, 0.08)` is also percussive; BLACK_ICE lead
  (sustain 0.4), both pads and both basses sustain.
- A second, independent route: the keys stem of the Round XII SWISS StablePropulsion score
  (seed 2112, normal production with reverb) was rendered solo with a temporary uncommitted
  probe, and D5 (587.33 Hz) energy was measured with a 1024-sample Hann Goertzel window,
  dB relative to D5's song peak:

  | Time (s) | Beat | Event | dB |
  |---:|---:|---|---:|
  | 15.65 | 30.78 | offbeat stab onset | −2.8 |
  | 15.75 | 30.98 | | −24.4 |
  | 15.85 | 31.17 | | −55.3 |
  | 16.30 | 32.06 | "hold" onset | −7.1 |
  | 16.40 | 32.25 | | −30.8 |
  | 16.50 | 32.45 | | −55.7 |

  The 1.94-beat hold and the 0.45-beat stab decay identically. Hold velocities are
  0.2008 (beat 32) and 0.2362 (beat 88); the neighbouring stabs are 0.2231.

What does distinguish the two witnesses from the other 29 keys D5 attacks in SWISS: they are
the only keys D5 attacks at the harmony's own articulation (beat 32 = C6 arrival, beat 88 = C6
arrival). They fuse with the pad's re-attacked C6 voicing `E4 A4 C5 G5`, whose C5 lies a whole
step below, and with the bass root. The other D5 attacks are syncopated between harmonic
articulations. Round XIII therefore measures onset fusion with the harmonic articulation as
temporal evidence alongside dwell. It does not use written duration as a proxy for exposure.

These are source and signal observations. Whether the maintainer's 15–17 s and 43–45 s
reports are caused by these exact events remains a listening question. The 43–45 s window
also contains a bass/keys/lead unison C# (beat 84.5, 0.229 s) against the pad's sustained C5,
and the pad's own unwritten D5 over Cmaj7 begins at beat 92 (46.78 s).
