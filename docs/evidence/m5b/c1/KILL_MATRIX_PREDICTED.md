# Predicted kill matrix, C-1 continuous motion (PREDICTION, not a measurement)

Status: **predicted, not run.** No compiler or test process was available while
`src/figure/motion.rs` and `tests/figure_motion.rs` were written, so none of the Rust has been
compiled or executed. The numbers come from `docs/evidence/m5b/c1/model_c1.py`, an f64 model that
restates the design (so it shares the design with the Rust and is not an independent check of
it), with each mutant applied by hand; its printed output is `model_c1_output.txt`. The Rust is
f32, so the real numbers will differ in the last digits and by the roundoff term the tests
declare. This matrix must be replaced by a measured one (run under heavy with
`RUSTFLAGS='--cfg figure_mutant="..."'`) before it is cited as evidence.

Mutants: **S** = `snap_transition` (no blend: the transition switches at its start),
**F** = `freeze_on_preempt` (the old pose is held static and faded out, so its velocity is lost),
**L** = `linear_keys` (a Smooth channel falls back to linear).
`K` = predicted to fail (mutant killed). `-` = predicted to pass (mutant survives that test).

## Tests and predicted outcome

| Test (`tests/figure_motion.rs`) | S | F | L | Basis |
|---|---|---|---|---|
| `smooth_matches_hand_computed_catmull_rom` | - | - | K | linear gives 0.25 at t=125 ms where Catmull-Rom gives 0.0859375, and rate 2.0 vs 2.25 at 250 ms |
| `smooth_is_exact_at_keys_and_holds_with_zero_rate` | - | - | - | linear is also exact at keys and holds |
| `smooth_one_shot_velocity_is_continuous` | - | - | K | linear jump 10 (arm), 4.87 (spine), 3.6 (smile) against bounds 0.17, 0.47, 0.056 |
| `loop_seam_velocity_is_continuous` | - | - | K | same, A and B loops (B arm 10 vs 0.31) |
| `reported_rate_matches_finite_difference` | - | - | - | a linear segment's central difference equals its slope; guards the analytic rate only |
| `looped_clip_is_exactly_periodic` | - | - | - | |
| `loop_seam_reported_rate_is_continuous` | - | - | K | at A's seam the linear rate flips from the last chord slope to the first (about 5 apart) |
| `linear_mode_matches_existing_clip_sample` | - | - | - | explicit Linear channels |
| `motion_clip_rejects_an_unknown_channel` | - | - | - | |
| `crossfade_velocity_is_continuous` | K | - | K | S: finite-difference velocity 2078 (arm) at the snap vs bound 0.76. L: 13.3 vs 0.76 at A's keys |
| `crossfade_blends_exactly_over_ease` | K | - | - | S: value at the start is `to`, not `from`; midpoint is `to`, not the mean |
| `preempt_while_moving_preserves_position_and_velocity` | K | K | - | S: position jumps (about 3.5). F: reported rate 0 vs 8.4, finite-difference jump 8.375 vs bound 0.883 |
| `preempt_preserves_rotation_and_weight_rate_at_t0` | K | K | - | F: omega 0 vs -3.69, smile rate 0 vs -2.4. Premises: |omega| = 3.69, |rate| = 2.4 |
| `preempt_offset_reaches_exactly_zero_and_poses_follow_the_new_clip` | - | - | - | every mutant also ends on the new clip; pins the end state |
| `preempt_matches_the_quintic_closed_form_midway` | K | K | - | quintic error 1.7e-6 (TOL 2e-4); F 0.73; S 1.47 |
| `preempt_timeline_is_c1` | K | K | K | arm: S 2469, F 8.375, L 13.3 vs bound 0.883; spine and smile likewise |
| `chained_preempt_is_c1_and_keeps_velocity` | K | K | K | same mechanisms, second preemption inside the first blend |
| `preempt_rotation_about_a_tilted_axis_keeps_pose_and_angular_velocity` | K | K | - | model, 200 random 3-D cases: omega error 1e-14 (f64). S jumps the rotation; F drops omega (5.3 to 0) |
| `preempt_with_zero_ease_switches_immediately` | - | - | - | pins the documented ease = 0 behaviour |
| `preempt_blends_targets_present_on_one_side_through_rest` | K | - | - | S: the arm track is absent at t0, so the lookup panics (a failing test) |
| `look_retarget_has_no_jump_at_the_retarget_instant` | K | - | - | S: aim jumps by 1.414. F and L: from rest there is no velocity to lose |
| `look_retarget_mid_motion_is_c1` | K | K | - | normal 0.096 vs bound 2.495; S 1414; F 7.77 (a 3.1x margin on the finite-difference bound); the reported omega check is the sharp one: normal 4e-10, F 7.77, vs tol 2e-3 |
| `look_converges_to_the_aimed_direction` | - | - | - | |
| `look_handles_an_antipodal_target` | - | - | - | |
| `look_rejects_bad_directions_and_unordered_retargets` | - | - | - | |
| `look_caps_the_number_of_retargets` | - | - | - | |
| `look_overlays_only_its_joint` | - | - | - | |
| `motion_is_a_pure_function_of_time` | - | - | - | |

Every mutant has at least one killer, and no killer depends on the cfg:
S is killed by 11 tests, F by 7, L by 7 (the counts of `K` in each column above).

## Normal-build predictions (the same checks without a mutant)

Largest finite-difference velocity jump on the 1 ms grid against the declared bound
`A dt + 4 u P / dt` (model, f64; the f32 build adds at most the roundoff term that is already in
the bound):

| Check | measured (model) | declared bound |
|---|---|---|
| clip A one-shot / loop: arm, spine, smile | 0.058, 0.028, 0.023 | 0.169, 0.468, 0.056 |
| clip B loop: arm, spine, smile | 0.103, 0.044, 0.053 | 0.312, 0.617, 0.156 |
| crossfade: arm, spine, smile | 0.105, 0.027, 0.023 | 0.761, 0.833, 0.188 |
| preempt timeline: arm, spine, smile | 0.153, 0.032, 0.023 | 0.883, 0.539, 0.162 |
| look aim direction, nested retargets | 0.096 | 2.495 |

The tightest normal-build margin is the smile channel of clip A (0.023 against 0.056, a factor
of 2.4). A bound that the model violated would be a derivation error to fix in the derivation,
never to tune; none is violated.

## What this prediction cannot see

- The Rust is not the model. A transcription error in `motion.rs` (a sign in the Jacobian, an
  index in the wrap accessor, an order of f32 operations) is invisible here; the model only
  shows that the *design* satisfies the tests.
- Roundoff: the declared tolerances (1e-5 for continuity, 1e-3 for rates, 2e-4 for the closed
  form, 2e-3 for angular velocity) are derived in the test comments, not measured in f32.
- The `look_*` finite-difference bound is conservative (about 26 times above normal); the F
  margin on it is only 3.1x, so the reported-omega assertion carries that kill.
