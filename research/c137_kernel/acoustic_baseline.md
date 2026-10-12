# C137 held-bass acoustic controls

This receipt records actual baseline rendering and independent PCM/TSV parsing.
Runtime source is PR #102 head
`29527a891ed7bfed9657a99046884a61670897e3`; the isolated worktree additionally
contains research-only bass tests/notes at
`dca3b4182c988c61a5ef0061d07b106ce77a6ed5`. No runtime code changed here.
There is **no human listening verdict** in this receipt.

## Reproduction and custody

Working directory: `/home/leah/LibGibson.kernel-bass`.

```bash
CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo build --example fusion_gates
```

For each `production` in `normal`, `nocomp`, `reference`, the actual example was
run with `--seed=2112 --beats=128 --tempo=144`, `--production=production`,
`--wav=OUT`, and `--dump-tsv=OUT`. Counterpoint OFF additionally supplied
`--no-counterpoint`; ON used the example's existing default. `BASS_WEAVE` stayed
false in both arms. The complete per-run command and stdout are retained at
`target/c137-controls/counterpoint-{off,on}/{production}/{command.txt,render.log}`.
Each directory contains its real WAV and `fusion_2112.tsv`.

The checked-in measurement tool can be rerun with:

```bash
python3 research/c137_kernel/measure_audio.py \
  target/c137-controls/counterpoint-*/*/*.wav \
  target/c137-controls/counterpoint-*/*/*.tsv \
  target/reconstructed-teacher/narrative_*.wav \
  --out=target/c137-controls/measurements.json
python3 research/c137_kernel/measure_audio.py \
  target/c137-controls/same-score-on/*.wav \
  target/c137-controls/same-score-on/*.tsv \
  --out=target/c137-controls/same-score-measurements.json
```

The tool uses the Python standard library for file hashes, integer PCM decoding,
sample statistics, and score parsing. Optional NumPy (available version 2.5.0)
adds spectral occupancy. No packages were downloaded. A local synthetic check
covered 8/16/24/32-bit PCM conversion, a 50 Hz sine's known RMS and spectral band,
TSV role counts, velocity mean, interval union, and overlap counting; all passed.

## Rendered baseline

All six full mixes have stereo 16-bit PCM at 48 kHz, 2,680,000 frames, and
55.833333 seconds. These include synthesis tails beyond the 128-beat score.
Peak and RMS below are measured from the written WAV, not copied from the
synth's floating-point meter.

| Counterpoint | Production | Score fingerprint | PCM RMS | Peak | 20–200 Hz power fraction |
|---|---|---|---:|---:|---:|
| OFF | normal | `e94d39e710182f7a` | 0.133265 | 0.823761 | 0.658408 |
| OFF | nocomp | `e94d39e710182f7a` | 0.125655 | 0.693115 | 0.686069 |
| OFF | reference | `e94d39e710182f7a` | 0.086486 | 0.766083 | 0.728102 |
| ON | normal | `001e6801b304a08a` | 0.135643 | 0.806732 | 0.639377 |
| ON | nocomp | `001e6801b304a08a` | 0.128568 | 0.743988 | 0.666583 |
| ON | reference | `001e6801b304a08a` | 0.087372 | 0.744415 | 0.719786 |

The spectral statistic sums per-channel Hann-window rFFT power across complete
one-second windows after DC removal; the denominator is 20–20,000 Hz power.
It measures occupancy, **not masking or intelligibility**. The mono mix is not
used for the FFT, avoiding cancellation from stereo summation. The end partial
window is excluded from this statistic. RMS is unweighted, not perceived loudness.
Decoding a finite integer WAV cannot establish that every pre-quantization float
was finite; this tool explicitly reports its observation boundary.

| Arm/production | WAV SHA-256 |
|---|---|
| OFF/normal | `a8925a00d110e106583f713b9be1f0a538caf75eb785fa6544c256be376d28e2` |
| OFF/nocomp | `1546fa1ba10fa5001c10a655159d60a0d686fdf228e7c27f1f607e7f5347c93d` |
| OFF/reference | `80303c64299983e4f1c43e89627abba4aef9630be6e7e37d7fb2c1ea34d27d80` |
| ON/normal | `03bbe35ea057f538fdf4a9ba86e88a36b739a22164a5366fc6130a8c910740f2` |
| ON/nocomp | `ec6f77d9970d7ea101c15e43a5f79ec9ee3307fc08cfe95faaa264e073a7636c` |
| ON/reference | `a22dfa0ff5370592dd110f132dfa6404c374e0cd0b6908a80d2a01ce064b4bbc` |

The TSV is byte-identical across production settings within each arm:

- OFF: `fed6d95830e2d5fbc32c375f978a1a0776d4dc69c5fd10fb3df463ece4b65994`.
- ON: `73e747846421cb6923b826a7a348b82ae3d65030df183402d8291c581e5f1b04`.

All runs report 9/9 fusion rejection gates and 3/3 weave rejection gates. The
OFF arm's weave result is vacuous without a countervoice. These are existing
reject-only diagnostics and establish no acoustic acceptance.

## Role behavior in the stored Score

| Role | OFF events | ON events | OFF velocity mean/SD | ON velocity mean/SD | Active bars OFF/ON |
|---|---:|---:|---|---|---|
| Bass | 246 | 246 | 0.3097 / 0.0687 | 0.3097 / 0.0687 | 32 / 32 |
| Drums | 539 | 539 | 0.3213 / 0.1550 | 0.3213 / 0.1550 | 27 / 27 |
| Keys | 156 | 186 | 0.2281 / 0.0435 | 0.3018 / 0.1533 | 23 / 24 |
| Lead | 126 | 126 | 0.6582 / 0.1042 | 0.6582 / 0.1042 | 28 / 28 |
| Pad | 124 | 124 | 0.1819 / 0.0501 | 0.1819 / 0.0501 | 31 / 31 |

Bass, drums, lead, and pad TSV rows are byte-identical across the ON/OFF arms.
Bass has zero attacks inside a prior written gate and zero adjacent same-pitch
retriggers within one quarter beat. This is a clean **held-bass control**, not
proof of the feasibility of the rejected weave. Keys are polyphonic, so their
written overlaps are not a mono-bass capacity test. Neither stored velocity
statistics nor activity counts measure recognizable musical intention.

## One actual Score, alternate production and stems

A local harness copies the unchanged `fusion_gates.rs`, renames its CLI entry,
and appends a driver using the same SongMap/argument/world/profile construction.
It calls `perform_argument` **once** for the ON arm and passes that same immutable
Score to normal/nocomp/reference and five `StemMask::solo` renders. Source and
binary are retained at `target/c137-controls/fusion_same_score_probe.{rs,}`.
The source SHA-256 is
`f229780651293330f66dd771f8cfb70d79b066b589750a1beae1ac76ef2f46dc`.
Compilation used the just-built baseline library:

```bash
rustc --edition=2021 -Awarnings target/c137-controls/fusion_same_score_probe.rs \
  --extern gibson=target/debug/deps/libgibson.rlib \
  -L dependency=target/debug/deps -o target/c137-controls/fusion_same_score_probe
./target/c137-controls/fusion_same_score_probe \
  > target/c137-controls/same_score_probe.log
```

The three full-mix WAVs are byte-identical to the corresponding actual
`fusion_gates` outputs. Artifacts are in `target/c137-controls/same-score-on`:
`fusion_{normal,nocomp,reference,bass,keys,lead,drums,pad}.wav`, `score.txt`,
`fusion_2112.tsv`, and `bus_levels.txt`.

This direct Score reports fingerprint `001e6801b304a08a` and
`score.stale_hearings().len() == 3`. This is a concrete stale-hearing witness on
the rendered baseline, separately from the velocity omission in the legacy
observer. It does not by itself identify every invalid dependency.

| Bus | Pre-master RMS | Solo WAV float-render RMS |
|---|---:|---:|
| Bass | 0.075319916 | 0.06342824 |
| Keys | 0.05225238 | 0.044869356 |
| Lead | 0.06542216 | 0.05471318 |
| Drums | 0.14675312 | 0.112808526 |
| Pad | 0.020102214 | 0.016505335 |
| SFX | 0.0073081157 | not soloed |

Bus meters are before the master chain. Solo outputs traverse that chain and
need not sum to the mastered full mix. `nocomp` changes the compressor toggle;
other production stages remain. `reference` changes several timbral and
production factors, so it cannot isolate one cause of a difference. This is a
matched descriptive experiment, not a perceptual masking threshold experiment.

## Teacher boundary and next listening gate

The provenance investigation is in [bass_teacher.md](bass_teacher.md). One local
teacher-named narrative group was recovered; the handwritten example freshly
reproduces its WAV **byte-exactly**. The normal narrative WAV hash is
`79cc0b73846c2b6e3658e6cf980e1a1415bc2b9b27c7834dbbdf8a22296b0a36`.
The newly rendered copies remain labeled reconstructed. The entire accepted
`teacher_golden7` set and its acceptance/custody record are **unavailable** here.

That narrative same-Score normal/nocomp/reference and five stems are present in
`target/reconstructed-teacher`; their fresh measurements are included in
`target/c137-controls/measurements.json`. Its normal PCM RMS is 0.173419 and
20–200 Hz power fraction is 0.602138. Its different composition, duration,
density, and tempo prevent attributing the baseline difference to mastering,
musician agency, or bass masking. No level normalization was imposed and no
listening superiority is inferred.

The next meaningful acoustic test is the new conversation beside these exact
held-bass and teacher references, including solo bass and masker stems, declared
source/cue/mute controls, and the maintainer's listening judgment. No new-path
acceptance or bass-weave rehabilitation follows from this receipt.
