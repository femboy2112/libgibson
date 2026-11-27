# Predicted kill matrix, M5-C DQS (PREDICTION, not a measurement)

Status: **predicted, not run.** No compiler or test process was available while
`src/figure/math.rs` (dual-quaternion helpers), `src/figure/skin.rs` (`SkinningMode`, `skin_with`,
`skin_normals`), `src/figure/revision.rs` (`evaluate_with`) and `tests/figure_dqs.rs` were
written, so none of the Rust has been compiled or executed. The numbers come from
`docs/evidence/m5c/model_dqs.py` (printed output: `model_dqs_output.txt`), an f64 restatement of
the fixtures, which shares the design with the Rust and is not an independent bearing on it. The
independent bearing is the f64 oracle inside `tests/figure_dqs.rs` (`dq_oracle`), which is
exercised only when the suite is run under heavy. This matrix must be replaced by a measured one
(`docs/evidence/m5c/gate_dqs.sh`) before it is cited as evidence.

Mutants (`RUSTFLAGS='--cfg figure_mutant="..."'`, both listed in `Cargo.toml`):
**A** = `dqs_as_lbs` (DualQuat falls back to linear blending, in `skin_with` and `skin_normals`),
**B** = `dqs_no_align` (`DualQuat::aligned_to` never flips a sign).
`K` = predicted to fail (mutant killed). `-` = predicted to pass. `?` = depends on the quaternion
extraction's sign branches and cannot be predicted from the model.

## Tests and predicted outcome (`tests/figure_dqs.rs`; there is no golden in this file)

| Test | A | B | Basis |
|---|---|---|---|
| `dualquat_matches_f64_oracle_on_two_bone_fixture` | K | ? | A: Linear vs DualQuat differ by up to 0.045 / 0.52 / 2.89 over the three poses (premise floor 0.02, TOL 3e-4); the oracle mismatch is the same size. B: the model's no-align difference is 2.93 on pose 3, where 8 terms flip under the continuous-path sign convention, but the runtime's extracted signs follow Shepperd's branches, so whether any pair is opposite is not predicted |
| `dualquat_matches_f64_oracle_on_three_bone_fixture` | K | ? | A: gaps 0.35 / 1.65 / 1.08. B: model no-align difference 3.83 on pose 2 (5 flips), same caveat |
| `dualquat_normals_match_oracle_and_stay_unit` | K | ? | A: Linear renormalized normals differ from the blended-rotation normals wherever the modes differ. B: only through the two-bone pose 3 |
| `linear_mode_is_evaluate_and_never_touches_rev_id` | - | - | Linear is `skin`; neither mutant touches it |
| `dualquat_at_bind_pose_is_the_neutral_mesh` | - | - | every skin matrix is the identity, so every blend is the identity |
| `elbow_cross_section_dualquat_keeps_area_and_linear_collapses` | K | - | A: DualQuat min blend ratio becomes 0.500 (model, Linear at 120 degrees) against the bar 0.85, and the mid ratio 0.5 against 1. B: the elbow's first influence is the identity, whose raw quaternion has `w > 0`, and so does joint 1's at 120 degrees, so no flip is needed |
| `blend_is_invariant_to_the_sign_of_a_later_term_and_takes_the_short_way` | - | K | B: case 0 (100 and -100 degrees about z) returns the identity instead of a half turn, and negating the second term changes the blend in cases 0 and 1. A does not touch `DualQuat::blend` |
| `sweep_across_a_quaternion_sign_flip_is_continuous` | K | K | A: the linear blend of two z rotations scales the point by cos of half the relative angle (0.82 to 0.97), so it leaves the analytic bisector curve by about 0.1 to 0.4. B: at the trace-zero crossing (120 degrees about -z) the extracted quaternion changes sign relative to the first influence, which gives a jump of order 1. Premise: the test asserts the raw sign change |
| `a_cancelled_blend_is_none` | - | - | exact cancellation, both mutants leave it |
| `a_non_rigid_skin_matrix_is_rejected_not_dropped` | K | - | A: `skin_with` returns the linear result instead of `NonRigid { joint: 1 }` |
| `dual_quat_helpers_match_hand_values` | - | - | helper algebra only |

Every mutant has at least one deterministic killer, and none depends on the cfg. A is killed
by 6 tests (the two oracle fixtures, the normals test, the elbow test, the sweep and the
non-rigid test). B is killed by 2 certain tests (the sign-invariance test and the sweep) plus up
to 3 `?` ones. `gate_dqs.sh` hard-requires the predicted killers named in its two
`check_mutant` calls (for B only the two certain ones).

## Normal-build predictions (the same checks without a mutant)

From `model_dqs.py` (f64; the f32 build adds the roundoff declared in `TOL`):

| Check | model | declared bar |
|---|---|---|
| elbow, min blend-zone area ratio, 120 degrees, DualQuat | 1.0000 | >= 0.85 |
| elbow, min blend-zone area ratio, 120 degrees, Linear (control) | 0.5000 | < 0.85 (must fail the bar) |
| elbow, 20 degrees, Linear (calibration) | 0.9848 (= cos 10 degrees) | >= 0.85 |
| smallest oracle-visible Linear vs DualQuat gap per pose | 0.0448 | > 0.02 (`MIN_MODE_GAP`) |
| smallest `|dot|` to the first influence over every vertex and pose (how near a sign decision) | 0.0872 | not a test, but it is 5 orders above f32 roundoff, so the oracle and the runtime take the same sign decisions |

## What this does not establish

- Nothing here ran. The oracle and the runtime may disagree for a reason neither the model nor
  this document foresaw; the first heavy run decides.
- The cross-section check is a ring-polygon area on a straight cylinder, not a body. The M5-C
  real-body gate re-applies the same ratio to the real mesh at the elbow, knee and hip.
- DQS is genuinely discontinuous where two influences are exactly 180 degrees apart; no test
  claims otherwise, and the fixtures keep every vertex at least 0.087 away in `|dot|`.
