# Round XIV receipts — heard objects

Theory and results: [../../HUMAN_MUSIC_HEARD_OBJECTS.md](../../HUMAN_MUSIC_HEARD_OBJECTS.md).

## Arms

- A = Round XII `perform_temporal`: the listening baseline. Its WAVs are byte-identical to the
  Round XII renders in `../humanmusic-r13/final-manifest.json`.
- Round XIII `perform_mass` and Round XIIIb `perform_tension`: historical arms, audited only.
- B = Round XIV `perform_coherent`.

Song: StablePropulsion, seed 2112, `deflected_lift_trace(120)`, default options, SWISS_SIGNAL
(118 BPM) and BLACK_ICE (88 BPM).

## `baseline-failures.txt`

The four Round XIV falsifiers, run failing on the tree they were written against (commit 1).

## `frozen/`

From `heard_object_lab` at commit 2, before any production change:

- the heard-identity audit and the gesture audit of Rounds XII, XIII and XIIIb;
- `summary.txt`: one line per arm.

The gesture reports list the 49 SWISS and 52 BLACK_ICE notes the Round XII observer then
supported as `SlidePath` from pitch geometry alone. Each shows its route, onsets, inter-onset
intervals, lengths, attacks, and declared and supported functions. None was slide-dependent.

## `final/`

From `heard_object_lab` on the final tree (see `render.txt`). Per world:

- `{world}_{r12,r13,r13b,r14}.identity.txt`: every rival run; every flipped slice spelled out
  with the chart, the bass, what each role sounds, the root (heard or last heard), the guide
  tones and the rival.
- `{world}_{arm}.gesture.txt`: the gesture audit. It is empty of slide claims since the observer's
  slide rule requires a physical glide.
- `{world}.at-seconds.txt`: every heard pitched event and the identity verdict at 15.0, 15.5,
  16.0, 16.5, 17.0 s and at 43.0–45.0 s, for Rounds XII, XIIIb and XIV.
- `{world}.chart-spans.txt`: every chart span whose pad Round XIV changed, and for SWISS every
  Cmaj7 span. Each line gives both pad voicings and the worst heard identity each arm gives the
  span.
- `{world}.pad-voicing-edits.tsv`: Round XIV's ledger (spacing or rooting, before and after).
- `{world}.perturbation.txt`: an independent diff of A → B. The lab asserts it counts exactly the
  ledger: pitch changes only, pad only, the same voicings.
- `{world}.lines.txt`: every bass and lead note declared ChromaticApproach, ChromaticPassing,
  Neighbor or Enclosure in B. Each is shown with its next note: onsets, the inter-onset
  interval, lengths, attacks, the physical gesture, the chart, and the observer's per-note
  support.
- `{world}.hearings.txt`: every hearing each arm recorded, and every stale one spelled out.

The WAVs are not committed. The lab regenerates them bit-for-bit (hashes are in
`final-manifest.json`).

## Engineering

- `engineering.txt`: the summary.
- `all-features-serial.txt`, `fuzz.txt`, `msrv-all-features.txt`, `example-tests.txt`,
  `preflight.txt`: the raw logs.
- `final-manifest.json`: source, audio and receipt hashes.
