# Bounded classification of three fresh failures

Production source: `f886e16008b8b87315270a7d4ca7bc51f6efd52c`. Read-only follow-up to `fresh-failures.json`; seed 3 only, three specified BLACK/SWISS cases. No production changes, threshold changes, retuning, full sweep rerun, or VAPOR95 evaluation. The original 61-pass/59-fail sweep stays failed.

**Verified — BLACK demo / MeaningDirected / 76 BPM.** The new false claim is the structural bass D2 (MIDI 38) at beat 62, gate 1.35: it crosses from G into F#7 at beat 63. R14 and R17 both retain C2 (36) at 63.5, followed by B1 (35) at 64. R15/R16 explicitly substitute that C2 with C#2 (37). C# is a member of F#7 and supplies the downward step that makes D2's held-resolution predicate true; C natural does not. The unchanged D2 is therefore supported as ChordTone/Suspension in R15/R16 but lacks its declared ChordTone evidence in R14/R17. False-function counts are respectively **1 / 0 / 0 / 1**. This reproduces an authored-path problem that the historical substitution had masked; it is still a regression against the frozen R15/R16 acceptance comparator. The explicit Push/Hit at 63.5 is retained in the new arm, without applying the old pitch substitution.

Source: `temporal.rs:258–279` defines the held-resolution and illegal-sustain predicates; `expression.rs:623–668` implements the historical stable substitution; `pocket.rs:362–382` preserves the action-anchored event when its short candidate is not accepted. Raw evidence: `fresh-classification.txt`, first case, includes all four D2 proof records and the C2→C#2 decision receipts.

**Verified — SWISS deflected / StablePropulsion / 106 BPM.** The authored/R14 lead is F4 (65) at 35.5, gate 0.5. R15/R16 have an explicit `Substituted` decision to G4 (67) at that same onset. R17 retains the authored F4, moves it to 35.75, and gives it gate 0.7275 and velocity 0.681296. R17 did not edit the source pitch; its performed pitch nevertheless differs from the frozen R16 output. “No literal source pitch edit” and “same accepted output pitches” are different predicates. The latter failure is not dismissed or renamed a pass. Raw evidence: final case's neighborhoods and source decisions.

**Verified — SWISS demo / StablePropulsion / 106 BPM.** Keys at 67.25 change from B4/F#4 (71/66), gate 0.4, to E5/B4/F#4 (76/71/66), gate 0.45. A controlled source replay holds performance, world, seed, and authored occupancy fixed and changes only the supplied final lead. It reproduces both keys results. The lead's new attack at 67.75 makes that comp slot ineligible; the selected stab positions change from `[64.75,67.25,67.75]` to `[64.75,65.25,67.25]`. Consequently 67.25 changes from odd stab index to even: the existing rule takes three voices instead of two. The next-stab gap also changes its gate. The underlying keys voicing at 67.25 is identical with either lead (checked at both three and four voices). E5 is a newly included existing voicing member, not a re-pitched B4/F#4; this remains an out-of-scope change to the accepted keys result.

Source: `occupancy.rs:236–241` checks authored reservations and final attack clearance; `comp.rs:448–474` filters and chooses stab slots; `comp.rs:511–522` alternates voice count and derives duration; `voicing.rs:1398` documents the selected stab voices. Raw evidence: `KEYS_SOURCE_INTERVENTION`, `COMP_ELIGIBILITY`, `REPLAY`, and `KEYS_PATH` rows.

**Boundary.** These diagnostics and source replays share production implementation provenance. They establish these three local execution mechanisms, not independent musical acceptance or exhaustive attribution of all 59 failed cases. No listening judgment is claimed. No failed gate or test was weakened. VAPOR95 remains the failed, untuned holdout from the original sweep.

Reproduce without rerunning the sweep:

```sh
cargo +1.98.1 run --example pocket_failure_probe > target/r17-fresh-classification.txt
diff -u docs/fixtures/humanmusic-r17/verification/fresh-classification.txt target/r17-fresh-classification.txt
```

The actual captured run used Rust 1.98.1 directly against the current built library to avoid the engineering suite's Cargo build lock:

```sh
rustc +1.98.1 --edition=2021 examples/pocket_failure_probe.rs \
  --extern gibson=target/debug/deps/libgibson.rlib \
  -L dependency=target/debug/deps -o /tmp/r17_pocket_failure_probe
/tmp/r17_pocket_failure_probe > docs/fixtures/humanmusic-r17/verification/fresh-classification.txt
```
