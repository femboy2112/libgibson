# C137 hearing baseline — registered falsifiers

Source: `29527a891ed7bfed9657a99046884a61670897e3` (PR #102 held-bass arm).
Research hypothesis source: PR #103, `HUMANMUSIC_C137_PERFORMANCE_KERNEL_ROAST.md`.
These are actual Rust probes, not a transcription of the Python seam model.

## Predictions registered before execution

1. Changing only a heard keys note's velocity from 0.25 to 0.75 leaves legacy
   `Hearing` equal and `Score::stale_hearings()` empty. The Score fingerprint must
   change. Changing its pitch afterward must produce a nonempty stale witness.
   The desired velocity-sensitive invariant is also retained as an ignored red
   test, to run explicitly against the baseline.
2. The production fusion construction used by `fusion_gates` at seed 2112,
   128 beats, A Aeolian, 108 BPM, `PerformanceProfile::BAND` is expected to have
   no stale hearings with counterpoint OFF. Turning counterpoint ON is expected
   to leave a nonempty drummer/keys stale witness, because the counter is inserted
   after the drummer consumes keys. Velocity blindness is independent of this
   expected event mismatch. This is an empirical prediction, not a claimed run.
3. Two lead calls with equal onset count, identical onsets/gates/velocities but
   opposite pitch contours and distinct material identity produce identical
   `keys_comp` output when its harmonic voice path is held fixed. This isolates
   the Listen/comp policy; it does not claim the entire keys realizer or the
   interaction system is insensitive to pitch.
4. Existing `InteractionMaterial` plus `Transform::Complete` produces distinct
   ascending and descending answers from those equal-count, equal-energy calls.
   `Transform::Silence` returns no material. A metadata-only source relabel leaves
   transformed event contents unchanged. No additional cue ontology is needed
   to express these distinctions.

## Source mutation ledger at the pinned head

- `score.rs:499–521`: `HeardNote` is onset, duration, pitch, function. Velocity,
  provenance, role duty, production and voice-continuity identity are absent.
- `functor.rs:1041–1075`: drums consume bass and (arbitrated policy) keys, then
  construct their part.
- `functor.rs:1087–1160`: optional counters remove/insert keys events; the held
  bass arm is still disabled. The dormant bass arm removes foundation, ties
  retriggers, clips tails and recomputes continuity after hearings.
- `functor.rs:1162–1165`: argument section-energy gains modify pad, keys, bass
  and drums after the hearings. Existing hearings cannot detect this field.
- `functor.rs:1172–1200`: optional sounding-tension repair edits finished notes
  while preserving action receipts, not all hearing fields. `perform_argument`
  refuses historical repair, so this writer is outside the fusion test domain.
- `functor.rs:1205–1225`: end clipping (explicitly normalized by legacy hearing),
  provenance stamping (outside its tuple), and Surgical repair (refused by the
  argument route) are later writers.
- `comp.rs:655–787`: comp placement reads lead onset collisions and drive-path
  density thresholds; source material identity and melodic direction are not
  inputs to the Listen allowance. `material.rs:224–305` already supports actual
  source-derived replies and explicit silence.

The claim is a causal-data-contract limitation, not an acoustic acceptance claim.
Counterpoint OFF is a control, not a proof that late velocities were faithfully
heard. PCM and maintainer listening remain separate gates.

## Execution results

Initial focused Rust run: 4 passed, 0 failed, 2 ignored in 2.70 s after a 47.71 s
clean dependency build. The predicted negative controls all reproduced.

| Arm | Song fingerprint | Score fingerprint | Hearings | Stale |
| --- | --- | --- | --- | --- |
| counterpoint OFF | `56f2426b736f1ea7` | `d6b86b204a7d51dc` | 9 | 0 |
| counterpoint ON | `56f2426b736f1ea7` | `49bffeb100d9e128` | 9 | 3 |

All three stale edges consume keys: bass, pad, drums. Each has 11 missing and
41 unheard notes. First missing tuple: `(94.25, 0.9, 68, Some(ChordTone))`.
First unheard tuple: `(23.25, 0.675, 64, Some(LicensedExtension))`.
The bass part is byte-identical under Debug comparison across OFF/ON.

Follow-up prediction registered before execution: because drum realization occurs
before the toggle's changed data exists, the actual drum events and percussion
reports should remain identical OFF/ON. At least one reported `band_onsets`
should disagree with final lead+bass+keys events ON, and none should disagree
OFF. This tests a field the actual dependent strategy consumes, independently of
the velocity-blindness test.

Boundary correction to the audit's dynamic interpretation: the current arbitrated
drummer consumes onset counts (`groove.rs:1150–1189`), not input-note velocities.
The velocity probe disproves a broad final-effective-dynamics claim for legacy
Hearing; it does not establish a failed velocity-dependent drummer decision.

The follow-up passed: drum events and entire percussion reports are unchanged;
15 bars' final counts disagree with the drummer's evidence. `(bar, heard, final)`:

```text
[(5,20,21), (6,20,22), (7,16,17), (9,19,20), (10,19,21),
 (11,14,15), (13,11,13), (14,20,22), (15,16,17), (23,19,21),
 (25,20,21), (26,14,15), (27,20,23), (28,16,17), (29,18,20)]
```

The strengthened normal test run passed 4, ignored 2, failed 0 in 2.18 s.
Explicitly running the two desired invariants returned exit 101: both failed,
as predicted. Dynamic assertion: legacy Hearing does not detect a post-observation
velocity mutation. Event assertion: `c.score.stale_hearings().is_empty()` fails
for current fusion counterpoint ON. These red tests remain ignored only because
they intentionally demand a contract that the archived API does not have; the
ordinary tests actively preserve the negative-control characterization.

Commands from this worktree (jobs bounded to 2, private target directory):

```sh
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/libgibson-c137-hearing-target cargo test --test audio_c137_kernel_baseline -- --nocapture
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/libgibson-c137-hearing-target cargo test --test audio_c137_kernel_baseline desired_ -- --ignored --nocapture
cargo fmt --check
git diff --check
```

Claims established: source-projection blind spot; actual post-hearing event
mutation; outdated consumed onset counts; count-based comp's inability to
distinguish these source contours; existing source transforms' ability to do so.
Not established: perceived improvement, acoustic masking, the size of any
velocity-dependent musical effect, or that every count mismatch changes an
arbitration result. Those require their own probes.
