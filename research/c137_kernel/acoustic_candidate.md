# C137 four-bar candidate: independent artifact measurements

Observed source revision: `ac97bccba88beb768720c67ac64200f6778d0584`.
Artifact root: `/home/leah/LibGibson.kernel/target/c137-audition-ac97bcc`.
This receipt analyzes actual written audio and event exports. It makes **no
claim of human recognition, perceptual masking removal, or musical acceptance**.
It remains evidence for this exact revision if later code changes.

## Custody and measurement

- **178 files** found: all 177 manifest entries plus `manifest.tsv` itself.
  Every listed size and FNV checksum was independently recomputed and matched;
  there were no missing or unexpected files.
- [candidate_sha256.txt](candidate_sha256.txt) contains independently computed
  SHA-256 for **every file**, including the manifest. Paths are relative to the
  artifact root stated above and in the receipt header.
- [candidate_measurements.json](candidate_measurements.json) contains all
  **81 WAV** and **18 Score TSV** measurements. [candidate_summary.json](candidate_summary.json)
  preserves comparisons, musical-span measurements, parsed runtime receipts,
  and hash-checked baseline references.
- All WAVs are stereo PCM16, 48 kHz, 440,000 frames, 9.166667 seconds, including
  a 2.5-second render tail after the 16 beats at 144 BPM. Musical-span RMS below
  excludes that tail. Full-file RMS includes it.
- Analyzer: `research/c137_kernel/measure_audio.py`, SHA-256
  `a5213b1a276a06513b3f442db4066774d2b20bc32f1de2f57eff444b4a743899`.
  Python parsed stored events and integer PCM; NumPy 2.5.0 supplied FFTs.
  No new build or render was run by this measurement pass.

```bash
packet=/home/leah/LibGibson.kernel/target/c137-audition-ac97bcc
python3 research/c137_kernel/measure_audio.py \
  "$packet"/*/*/*.wav "$packet"/*/*/score.tsv \
  --out=research/c137_kernel/candidate_measurements.json
```

The example's nine committed-take receipts report valid event contracts,
matching effective hearings, finite pre-quantization float samples, and bitwise
equality across render blocks 127/256. Those are **runtime emitter receipts**;
this pass independently checked written files and their comparisons. Finite
integer PCM alone cannot establish finite pre-quantization floats.

## Diversity limit

**Three seeds yield only two distinct candidate performances.** Seeds 2112 and 7
have byte-identical Score TSVs and all eight candidate WAVs, despite distinct
source Debug dumps. Seed 99 changes the lead; its keys, bass, drums, and pad solo
WAVs are byte-identical to the corresponding buses in the other two candidates.
Across the entire packet, the 81 WAV files have only **33 distinct SHA-256s**.
Do not count duplicates, source-label changes, or alternate production as
independent musical cases or held-out generalization.

| Candidate | Score fingerprint | Normal WAV SHA-256 |
|---|---|---|
| Seeds 2112 and 7 | `468ddeeb851c6101` | `32556ee50f8c726031ab1129bd5d09b01b8b9ff1ca2b947f9f2767541ed34234` |
| Seed 99 | `32509f436ee58573` | `0917f6ab30081e7896ffd423c012c75053eaa7f36fbac982d71c07daa21d9225` |

## Written duties and real silence

All candidates have these attack counts. Ranges below describe seed 2112/7;
the seed-99 lead pitch content differs.

| Role | Attacks by bar | MIDI range | Stored velocity range | Written occupancy |
|---|---|---|---|---:|
| Lead | 4, 0, 0, 5 | 69–76 | 0.5328–1.0000 | 29.69% |
| Keys | 0, 4, 0, 0 | 50–62 | 0.3948–0.5209 | 10.00% |
| Bass | 2, 2, 3, 2 | 36–45 | 0.5700–0.7600 | 53.13% |
| Drums | 12, 12, 14, 12 | — | 0.2500–0.7800 | — |
| Pad | 0, 0, 0, 0 | — | — | 0.00% |

Lead, keys, and bass have no overlapping written gates and no same-pitch
adjacent retriggers within a quarter beat. Lead/keys/bass/drums have respectively
9/4/3/6 distinct stored velocities. These are descriptive statistics, not a
semantic-density or intelligence score.

Independent normal solo-bus PCM RMS per musical bar, seed 2112:

| Bus | Bar 1 | Bar 2 | Bar 3 | Bar 4 |
|---|---:|---:|---:|---:|
| Lead | 0.049795 | 0 | 0 | 0.082041 |
| Keys | 0 | 0.048245 | 0 | 0 |
| Bass | 0.094949 | 0.098318 | 0.106013 | 0.095880 |
| Drums | 0.121849 | 0.121392 | 0.123190 | 0.121200 |
| Pad | 0 | 0 | 0 | 0 |

Thus the lead breath and withheld keyboard opportunity survive into these
written PCM stems, and every pad sample is exactly zero. The full normal mix's
four bar RMS values remain close: `0.141940, 0.143189, 0.146648, 0.146146`, a
max/min span of only **0.2835 dB**. Distinct role behavior is demonstrated; a
dramatically changing full-band energy envelope is not. Four bars do not support
a useful general statistical claim about ensemble energy correlations.

All four keyboard attacks have offset **0.0625 beat**, or **26.04 ms** at 144 BPM,
relative to the common eighth-note lattice. The offset does not accumulate in
this phrase. This verifies the bounded example, not long-form phase recovery.

## Causal subtraction versus exported mute

For every seed, **source-authorized keys withholding followed by re-performance**:

- Removes four keys events and two later `heard-keys` snare hits.
- Preserves all lead/bass events, chord rows, and 48 `pulse` drum events.
- Preserves lead and bass solo PCM byte-exactly.
- Changes drum solo PCM first at **4.375 seconds = beat 10.5**.

The replanned mix and the exported keys-mute mix also first differ at 4.375
seconds. The exported mute retains the two causally dependent snare hits;
withholding removes them. This is an observed downstream decision difference,
not merely a rejection receipt or output amplitude change.

The closed-call source control preserves the initial call's onset count,
gates, and velocities while changing its final pitch. Seed 2112 changes
`69,72,74,76` to `69,72,74,69`; seed 99 changes `72,74,76,77` to `72,74,76,69`.
Both closed calls produce silence from keys and omit the dependent fill, while
retaining bass and drum anchors. This defeats a count/velocity-only response
for this contrast. It does not show that all different open calls yield
different replies: the two tested open candidate melodies have identical keys
output after the current transformation and chord projection.

The exported mutes remain **uncommitted controls**. For seed 2112:

| Muted event set | First written PCM difference | Runtime rejection |
|---|---:|---|
| Keys | 1.692729 s | `StaleObservation(1)` |
| Bass approach | 4.791688 s | `ChangedCommitment(Bass)` |
| Dependent drum fill | 4.375000 s | `ChangedCommitment(Drums)` |

The bass approach is MIDI 44 at beat 11.5, gate 0.3 beat, preceding the source
destination MIDI 45 at beat 12. **Its exported mute does not replan or alter any
dependent musician decision.** It demonstrates a changed audible contribution
and commitment. The approach anticipates declared future harmony; it is not
evidence of hearing the later lead. Its perceptual value remains a listening
question. Likewise, validation rejection alone is not a musical consequence.

## Same Score, alternate production and frequency content

Each committed take uses one Score fingerprint for normal, nocomp, harmonic
reference, and all solo stems. Seed 2112/7 written PCM measurements:

| Production | Full-file RMS | Musical-span RMS | Peak | 20–200 Hz power fraction |
|---|---:|---:|---:|---:|
| Normal | 0.123225 | 0.144494 | 0.814026 | 62.46% |
| nocomp | 0.122009 | 0.143068 | 0.684906 | 68.18% |
| Harmonic reference | 0.087094 | 0.102126 | 0.728424 | 74.61% |

Seed 99 normal full-file RMS is 0.122828 and peak 0.813782. No level normalization
was applied. `nocomp` isolates one production toggle while limiting remains;
harmonic reference changes multiple factors and cannot identify one mechanism.

Normal isolated-bus spectra, seed 2112/7 (identical bass/keys/drums for seed 99):

| Bus | 20–80 Hz fraction | 80–200 Hz fraction | 200–1000 Hz fraction |
|---|---:|---:|---:|
| Bass | 40.30% | 43.26% | 16.08% |
| Keys | 1.87% | 33.29% | 55.82% |
| Drums | 36.80% | 24.31% | 10.04% |
| Lead | 0.00017% | 0.00046% | 87.00% |

These sum Hann-window per-channel FFT power over complete one-second windows;
the denominator is 20–20,000 Hz. They are **spectral occupancy, not perceptual
masking**. Keys reach MIDI 50 and share substantial low-band energy with bass
and drums. No removal of audible mud follows from the clean gate/retrigger
counts. Solo buses still traverse production and are not additive pre-master
separations. The old bass weave remains disabled.

## Historical reference boundary

Selected local baseline/teacher files were rehashed against the prior receipts:

| Reference | Score length / tempo | Render duration | Normal RMS | 20–200 Hz fraction |
|---|---|---:|---:|---:|
| PR #102 held bass, counterpoint OFF | 128 beats / 144 BPM | 55.833 s | 0.133265 | 65.84% |
| PR #102 held bass, keys counterpoint ON | 128 beats / 144 BPM | 55.833 s | 0.135643 | 63.94% |
| Reconstructed narrative teacher | 224 beats / 138 BPM | 99.891 s | 0.173419 | 60.21% |
| New candidate, seed 2112 | 16 beats / 144 BPM | 9.167 s | 0.123225 | 62.46% |

The held-bass baseline already has zero written overlaps and rapid equal-pitch
retriggers; retaining those properties does not rehabilitate the rejected weave.
Its bass has 246 attacks and 88.98% written occupancy, versus the candidate's
9 attacks and 53.13%. The comparison is **confounded by source, arrangement,
duration, density, register, and teacher tempo**. It cannot assign a quality,
masking, or mastering advantage. The complete accepted `teacher_golden7` set
and its acceptance record remain unavailable; one narrative reference was
recovered and reproduced byte-exactly as documented in [bass_teacher.md](bass_teacher.md).

The pending gate is maintainer listening to these actual candidate/control
mixes and stems. This is a verified bounded causal conversation with explicit
acoustic limits, not C137 musical acceptance or a full-band replacement.
