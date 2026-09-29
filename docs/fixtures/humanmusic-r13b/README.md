# Round XIIIb receipts — sounding tension

Theory and results: [../../HUMAN_MUSIC_SOUNDING_TENSION.md](../../HUMAN_MUSIC_SOUNDING_TENSION.md).

## Arms

- A = Round XII `perform_temporal`
- B = Round XIII `perform_mass`: the renders the maintainer heard. Its score fingerprints are
  asserted equal to the Round XIII receipts, and its WAVs are byte-identical.
- C = Round XIIIb `perform_tension`

Song: StablePropulsion, seed 2112, `deflected_lift_trace(120)`, default options. The
MeaningDirected/DeflectedLift song is the control.

## `final/` (from `sounding_tension_lab`, see `render.txt`)

- `{world}_{regime}_{r12,mass,tension}.tension.txt`: the sounding-tension audit of each arm. It
  gives counts, then every clash that is not Transient: beat, seconds, responsible note, partner,
  interval, overlap, salience, exposure, class, continuation and verdict.
- `{world}_{regime}.tension-changes.tsv`: the gate's ledger. The first line is the perturbation
  receipt from an independent B→C diff; the lab asserts the diff's counts equal the ledger's.
- `{world}_stable.around-{16,44}s.txt`: before/after receipts at the maintainer's timestamps:
  - what every role sounds at 1 s, ½ s and 0 s either side of the timestamp;
  - every clash in the window before and after;
  - every edit in the window.
- `{world}_{mass,tension}.{notes,chords}.tsv`: every note (with audible seconds) and the chart,
  as input to the independent grader.

## `independent/`: second implementations, not the gate's code

- `tension_audit.py` → `tension_audit.txt`: the same law in Python, stricter (release tails
  never masked), plus the out-of-key non-chord tally.
- `roughness.py` → `roughness.txt`: Sethares sensory roughness of the pitched stems (drums and
  SFX muted), mass vs tension, over the witnessed windows.
- `peaks.py` → `peaks.txt`: spectral peaks at 14.6, 15.0, 15.9 and 16.3 s (SWISS) and 19.6 s
  (BLACK_ICE).
- `tone_energy.py` → `tone_energy.txt`: how long the lead's C#5 sounds above −24 / −30 dB at
  four BLACK_ICE spots.

The WAVs are not committed; the lab regenerates them bit-for-bit (hashes are in
`final-manifest.json`).

## Engineering

- `engineering.txt`: the summary.
- `all-features-serial.txt`, `fuzz.txt`, `msrv-all-features.txt`, `example-tests.txt`,
  `preflight.txt`: raw logs.
- `final-manifest.json`: source, audio and receipt hashes.
