# Predicted kill matrix, M5-C performance (PREDICTION, not a measurement)

Status: **predicted, not run.** `src/figure/ik.rs`, `src/figure/perform.rs` and
`tests/figure_perform.rs` were written without a compiler or a test process (heavy-only rule). The
numbers the tests assert come from `docs/evidence/m5c/model_perform.py` (pure python, run). The
first heavy run decides; replace this file with the measured matrix from
`docs/evidence/m5c/gate_perform.sh`.

Mutants (all in `src/figure/perform.rs`, selected by `RUSTFLAGS='--cfg figure_mutant="..."'`):

* **C** = `contact_ignored`: the foot IK weight is forced to 0 (the feet follow the pelvis).
* **S** = `snap_beats`: every beat boundary (and every `Live` start) is joined with a zero ease, so
  nothing is inertialized.
* **R** = `no_root_motion`: the root keeps its start translation and the foot targets are shifted by
  the root travel (the figure walks in place on a treadmill).

`K` = predicted to fail (killed), `-` = predicted to pass. No golden anywhere: every kill is a
behavioural assertion.

| Test (`tests/figure_perform.rs`) | C | S | R | Basis |
|---|---|---|---|---|
| `ik_reaches_reachable_targets_to_1e4_and_keeps_bone_lengths` | - | - | - | the mutants do not touch `ik.rs` |
| `ik_mid_joint_lies_in_the_pole_plane_on_the_pole_side` | - | - | - | same |
| `ik_unreachable_targets_move_the_limb_continuously_and_a_hard_clamp_does_not` | - | - | - | same; its own control (hard clamp, `soft_start = 1`) is asserted to fail the 3.5 mm/mm bound at 31.6 mm/mm (model) |
| `walk_root_displacement_is_stride_times_steps_and_the_body_follows` | - | - | K | R: root displacement 0, not `0.5 * 6`; C leaves the ankles travelling with the pelvis, so it passes |
| `stance_feet_are_world_locked_and_on_the_ground` | K | - | K | C: the planted ankle follows the pelvis at about 1 m/s and sinks by the crouch; R: the planted target is shifted backwards at the root speed |
| `stance_control_without_foot_ik_slides` | - | - | - | the control itself (foot lock off through the option) |
| `walk_with_a_turn_keeps_the_feet_planted` | K | - | K | same as above, while turning |
| `every_beat_boundary_and_foot_lock_transition_is_c1` | - | K | - | S: the wave cut at 1.2 s becomes a 0.7 m jump, `J = 700 m/s` against 0.09 |
| `control_a_snapped_beat_cut_breaks_the_same_metric` | - | K | - | the control passes by itself (option off); its second half asserts the shipping build is smooth at the cut, which S breaks |
| `the_cut_blend_takes_the_settle_ease_and_lowers_the_hand_monotonically` | - | K | - | S: the hand jumps 0.7 m between samples 4 ms apart |
| `the_waving_hand_stays_out_of_the_head_sphere` | - | - | - | its control (clearance off) is asserted to violate by more than 2 cm |
| `a_target_forty_five_degrees_off_turns_the_head_legibly` | - | - | - | |
| `sit_settles_the_pelvis_at_the_seat_with_planted_feet_and_stand_returns` | K | - | - | C: both ankles drop with the pelvis by about 0.45 m |
| `an_unreachable_seat_and_a_walk_while_seated_are_typed_errors` | - | - | - | |
| `restrained_and_expressive_differ_measurably_in_amplitude` | - | - | - | |
| `reduced_motion_lowers_amplitude_and_turns_the_walk_into_a_dissolve` | - | - | K | R: the dissolve target is never reached (the root is pinned) |
| `the_tape_is_deterministic_well_formed_and_hashes_its_body` | - | - | K | R: the tape root ends at 0, not 1.5 m |
| `live_stop_is_immediate_and_schedules_no_work` | - | - | - | `Live` does not apply R |
| `live_quiesce_returns_the_held_pose_with_zero_work` | - | - | - | |
| `live_preemption_blends_from_the_current_pose_and_a_snap_does_not` | - | K | - | S: the nod at 1200 ms snaps the raised hand |
| `live_intents_queue_by_priority_and_an_expired_one_eases_out` | - | K | - | S: the expiry at 1500 ms snaps |
| `live_reduced_motion_lowers_amplitude` | - | K | - | S: switching to reduced motion mid-wave snaps (the amplitude half passes) |
| `live_walk_reaches_the_target_planted` | K | - | - | C: with the feet following the pelvis the ankle never rises above its rest height |
| `loop_seam_matches_a_seamless_reference_and_the_velocity_gap_closes` | - | - | - | independent of the performance mutants; the existing `motion.rs` mutant `linear_keys` is predicted to kill it (a velocity step at the seam) |

Killers per mutant (predicted): C 4, S 6, R 5. Each is killed by at least one assertion that is not a
golden. `figure_motion`'s own mutants (`snap_transition`, `freeze_on_preempt`) are also expected to
break S-class tests because every beat boundary here is a `motion::Preempt`; they are not part of this
gate.

## What this does not establish

* That the file compiles (it has never been through `rustc`); clippy, `+1.85` and the default check
  are gate steps for that reason.
* That the declared continuity bound `J_BOUND = 0.09` holds through the IK map in `f32`. The planned
  drivers peak at 36.7 m/s^2 on the foot and about 60 m/s^2 on a cut hand (python model and hand
  derivation in the test header), the bound assumes 80 m/s^2 after the IK; the first run measures it. A failure there is a number to read, not a threshold to relax.
* That the soft reach limit (0.98 of full extension) is never entered during a walk. The planner
  designs the crouch for a 0.95 ratio and the model shows a worst 0.963 including bob and sway; a turn
  shortens or lengthens the hip-to-ankle distance by a few percent more, which `walk_with_a_turn_*`
  would show as a foot that no longer reaches its target.
* Anything about how the figure looks. Pixels are viewed separately (the section 5 rule).
