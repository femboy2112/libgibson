# Round XI receipts

See [the experiment report](../../HUMAN_MUSIC_PHENOMENAL_REGIMES.md) for definitions,
claim limits, theory, controls and the listening question.

- `baseline-failures.txt`: two actual red tests before implementation, commit `95ce5af`.
- `phenomenal-tests.txt`: seven controls, including content mutations and exact A/B seats.
- `round-ix-controls.txt`, `round-x-controls.txt`: preserved identity/meaning controls.
- `prior-fuzz-controls.txt`: both explicit release sweeps, 1,152 compositions each;
  zero coupled receipts lost and zero surgical receipts lost silently.
- `render.txt`: exact options, world configurations, chart/theme pages, target contour,
  commutation, conformance, score fingerprints, note validity and render statistics.
- `*.trajectory.txt`: one row per structural slot, with independent dimensions and evidence.
- `manifest.json`: SHA-256 of the final source bytes, protected unchanged paths, and eight WAVs;
  PCM format/frame verification and unchanged R10 audio comparison.
- `engineering.txt`: final gate commands, status and test summaries.
- `all-features-serial.txt`: the complete final 1,208-test run.
- `preflight.txt`: publication-free release preflight output.
- `all-features-concurrent-failure.txt`, `pty-isolated.txt`: the retained two FX Lab
  header failures during concurrent gate execution, and all seven PTY cases passing
  serially with screen capture. No UI production repair is claimed.

The final acceptance audio is under `/home/leah/LibGibson/target/humanmusic-r11/`:

| World | A | B |
|---|---|---|
| SWISS_SIGNAL | `swiss_signal/tension_deflection.wav` | `swiss_signal/stable_propulsion.wav` |
| BLACK_ICE | `black_ice/tension_deflection.wav` | `black_ice/stable_propulsion.wav` |

Each file also has a `.harmonic_reference.wav` counterpart. Raw performed notes are
in matching `.notes.tsv` files beside the audio. Binary audio is not part of the crate
or git history; the documented lab command recreates it. The initial, unmatched-seating
experiment is excluded from acceptance receipts. No listening verdict is encoded here.

## Storage

Large text dumps in this directory, and exact duplicates of dumps kept elsewhere, are stored
compacted; no byte was discarded. Every original path, byte count and sha256 is in
`docs/fixtures/humanmusic-archive/MANIFEST.tsv`; the unique bytes are in the solid archive
`docs/fixtures/humanmusic-archive/blobs.tar.xz` (member name = sha256). See
[`HUMANMUSIC_FIXTURE_MANIFEST.md`](../HUMANMUSIC_FIXTURE_MANIFEST.md) for the policy.

- List what was compacted here: `grep '^docs/fixtures/humanmusic-r11/' docs/fixtures/humanmusic-archive/MANIFEST.tsv | cut -f1,4,5`
- Restore every compacted file to its original path and bytes:
  `python3 scripts/fixtures/expand-humanmusic-archives.py expand` (or add `--dest /tmp/x` to
  grep a copy instead of touching the tree; expanded files are working copies, do not commit them).
- Verify all manifest hashes: `python3 scripts/fixtures/expand-humanmusic-archives.py check --expanded`
  (after `expand`, any older `sha256sum -c` manifest in this directory passes again).
- Without Python: `mkdir /tmp/blobs && tar -xJf docs/fixtures/humanmusic-archive/blobs.tar.xz -C /tmp/blobs`, then look up the
  member named by the sha256 in the manifest.
