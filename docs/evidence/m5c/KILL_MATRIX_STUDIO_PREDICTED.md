# Predicted kill matrix, M5-C `figure::studio` (PREDICTION, not a measurement)

Status: **predicted, not run.** `src/figure/studio.rs` and `tests/figure_studio.rs` were written
without a compiler or a test process (rustfmt only), so none of the Rust has been compiled or
executed. The fixture numbers the tests assert were cross-checked in f64 against
`docs/evidence/m5c/oracle_shape.py` (an independent reader of the same fixture files; the check
printed, for the tabard, tunic and cap vertices of the garment-geometry test, a worst deviation of
1.3e-8 from the closed forms), and the gating verdicts (shell Fitted at bulge 0.2, NeedsAdaptation
at 0.5 with depth 0.00398, Rejected at 1.0 with depth 0.0199) were read off the same oracle. Both
are f64 restatements of the design, not the Rust. The first heavy run decides; replace this file
with a measured matrix from `docs/evidence/m5c/gate_studio.sh`.

Mutants (`RUSTFLAGS='--cfg figure_mutant="..."'`, all three listed in `Cargo.toml`):
**I** = `ignore_skeleton_update_v1` (the refit is skipped: the declared skeleton and its inverse
binds are kept), **S** = `ignore_surface_update` (the control deltas are skipped: the neutral
surface is kept, the refit then follows the neutral surface), **F** = `stale_fit` (garments and
hair are fitted and gated against the NEUTRAL body, as if a fit from before the body edit were
carried over). `K` = predicted to fail (killed), `-` = predicted to pass. There is no golden id
in `tests/figure_studio.rs`; every killer below is a closed-form or relational assertion.

| Test (`tests/figure_studio.rs`) | I | S | F | Basis |
|---|---|---|---|---|
| `each_control_moves_its_measurement_monotonically_by_the_designed_amount` | - | K | - | S: every landmark measure stays at its neutral value, so the strict increase and the designed `0.4 (1 + 0.5 c)` (and the other three) fail |
| `a_control_does_not_move_the_other_controls_measurements` | - | - | - | holds when nothing moves; pins the no-compensation rule only |
| `an_out_of_domain_control_is_applied_unclamped_and_reported` | - | K | - | S: height stays 1.5, not 2.0625 / 0.9 |
| `bad_control_input_is_a_typed_error` | - | - | - | validation precedes derivation |
| `the_lowest_body_vertex_is_grounded` | - | K | - | S: no `sink` delta, so the lift is 0 where 0.05 c is designed |
| `refit_joints_are_the_anchor_centroids_of_the_shaped_mesh` | K | K | - | I: the declared neck is at z 1.4, designed 1.61, and no joint equals its shaped centroid; S: the joints equal the (unshaped) centroids but not the designed shaped positions |
| `bone_lengths_are_outputs_of_the_surface` | K | K | - | I: the declared 0.5 never changes; S: the same |
| `the_rest_orientation_chain_survives_the_refit` | - | - | - | rotations are carried in every variant |
| `revision_isolation` | - | K | - | S: the control edit (`stature 0.31`) leaves the body id unchanged, so `assert_ne` fails; I and F do not change what the ids hash |
| `resolved_content_not_recipe_text_is_identified` | - | K | - | S: `plain != ab` fails (no deltas, equal ids) |
| `the_body_bytes_follow_the_contract_layout` | - | - | - | the probed fields (prefix, mode, asset sha, first vertex at z = 0, footwear tail) do not depend on the three mutants |
| `the_fit_id_and_presentation_id_are_rebuilt_by_hand` | - | - | - | rebuilt from the live body id, so S and F cannot disturb it |
| `compile_is_deterministic` | - | - | - | every variant is deterministic |
| `a_garment_the_body_bulges_through_is_not_drawn` | - | K | K | S: the shell stays Fitted at bulge 0.5 / 1.0; F: gated against the neutral body, so it stays Fitted |
| `a_verdict_is_never_carried_across_a_body_edit` | - | K | K | S: the two bodies have one id; F: the bulged shell is Fitted, not Rejected |
| `garments_follow_the_shaped_body_with_per_axis_offset_scales` | - | K | K | S and F: the tabard, tunic and cap stay at their neutral positions, e.g. tabard v0.x 0.25 where `0.25 (1 + 0.5 c)` is designed |
| `bad_garment_hair_and_slot_input_is_a_typed_error` | - | - | - | |
| `a_covered_lower_face_reports_driven_jaw_and_mouth_channels` | - | - | - | |
| `garments_follow_the_pose_and_only_drawn_garments_are_posed` | - | K | K | S and F: the shell is Fitted and drawn where Rejected is designed (`assert!(!drawn(shell))`). I is not killed here: the declared root is at the origin like the shaped one |
| `a_bad_frame_is_a_typed_error` | - | - | - | |
| `hostile_asset_bytes_are_typed_errors` | - | - | - | |
| `the_asset_set_loads_from_disk_and_verifies_the_index` | - | - | - | |

Kill counts: **I** 2 tests (`refit_joints...`, `bone_lengths...`), **S** 11 tests, **F** 4 tests
(`a_garment_the_body_bulges...`, `a_verdict_is_never...`, `garments_follow_the_shaped_body...`,
`garments_follow_the_pose...`). `gate_studio.sh` hard-requires, per mutant, the killers named in
its `check_mutant` calls (the first four S rows, both I rows, the first three F rows).

What this does not establish. That any of it compiles or passes (the predictions use closed forms
and the oracle, not a run); that the nearest-vertex normal-distance penetration metric is a good
garment-fit measure on real clothes (on the real converted asset the oracle, at the default
tolerance 0.002 m, reports 157 of 2197 `female_casualsuit01` vertices below the surface at the
NEUTRAL body, deepest 9.7 mm, so the default thresholds would not mark that suit Fitted: they are
fit parameters and are hashed, and the first real-body lease must calibrate them); and the
fused-multiply-add and reassociation freedom of the optimiser, which the scalar code forbids by
construction but a run alone confirms. The bearing on I is thin (2 tests, both through joint
positions); a posed-vertex test about a non-root joint, whose expected value needs the DQS blend
in closed form, is not in this slice.

## Addendum (ORDER #105 and the delete-set ruling): four more tests, a fourth mutant

**C** = `stale_control` (`Compiled::with_control` returns the old compile unchanged). Added tests
(`tests/figure_studio.rs`, 26 in all), predicted outcomes:

| Test | I | S | F | C | Basis |
|---|---|---|---|---|---|
| `a_bulge_outside_the_delete_set_still_rejects_and_inside_it_does_not` | - | K | K | - | S: the bulge never reaches the shell, which stays Fitted; F: the neutral body is measured, so the shell stays Fitted. Oracle: shell Rejected (depth 0.0199) and plug Fitted at bulge 1.0 |
| `with_control_is_bit_identical_to_a_full_compile_and_moves_the_measurement` | - | K | K | K | C: the hip distance stays 0.4 where 0.6 is designed, and every `assert_same` against the full compile fails; S: the same measurement; F: the verdict flip (`bulged` shell NeedsAdaptation) fails because the neutral body is measured |
| `drawable_meshes_hide_deleted_body_triangles_and_carry_slots` | - | K | K | - | S and F: the shell is Fitted and drawn where its absence is asserted |
| `a_tape_frame_is_read_and_mapped_to_the_rig` | - | - | - | - | reader tests; no mutant touches them |

Rows of the first table that change with the delete-set ruling: none of their verdicts (the shell
now hides ring 0, which the bulge does not move). Kill counts become **I** 2, **S** 14, **F** 7,
**C** 1 (`with_control_is_bit_identical_...`; the single killer is by design: C only touches
`with_control`). `ignore_surface_update` also kills the `with_control` test, so C has no
second bearing.

The oracle (`oracle_shape.py`, now skipping a part vertex whose nearest body vertex is in the
part's delete set, the rule `studio` implements) gives, at the neutral real body, `skirt` Fitted and
`female_casualsuit01` 44 of 2197 vertices deeper than 2 mm (deepest 7.4 mm, NeedsAdaptation); before
the masking it was 157 and 9.7 mm. Thresholds are unchanged.

## Addendum (garment adaptation): five more tests, a fifth mutant

**A** = `no_adapt` (the opt-in adaptation step, `FitParamsV1::adaptation`, is skipped; the fit id is
unchanged). Added tests (`tests/figure_studio.rs`, 31 in all), predicted outcomes (PREDICTION, not a
measurement; the fixture numbers below were read off an independent f64 python port of the
algorithm run on the fixture's design, not off the Rust):

| Test | I | S | F | C | A | Basis |
|---|---|---|---|---|---|---|
| `adaptation_makes_a_penetrating_garment_fit_and_leaves_the_rest_untouched` | - | K | K | - | K | A: the shell at bulge 0.3, lean 0.05 stays NeedsAdaptation and has no report; S: the bulge and lean never reach the shell so the unadapted precondition (NeedsAdaptation) fails; F: the neutral body is measured, the same |
| `adaptation_pushes_a_uniformly_penetrating_garment_out_to_the_clearance` | - | K | K | - | K | A: stays NeedsAdaptation (4 mm), no report |
| `adaptation_leaves_a_garment_that_already_clears_the_body_bit_identical` | - | - | - | - | K | A: the per-garment report is absent (`expect` fails); nothing else touches it |
| `an_adaptation_over_the_cap_is_refused_and_the_unadapted_verdict_stands` | - | K | K | - | K | A: no report, no refusal diagnostic; S and F: the shell is Fitted, `assert_ne!(st, Fitted)` fails |
| `the_fit_id_covers_the_adaptation_parameters` | - | - | - | - | - | A does not change ids (by design: the parameters, not the step, are hashed); an id test only |

Predicted: **A** is killed by 4 tests. Fixture facts used (f64 port): shell at bulge 0.3, lean 0.05:
signed distance per ring-2 vertex 2.60, -0.98, -2.59, -1.28, 2.18, 5.75, 7.36, 6.05 mm, so
vertices 1..3 are pushed (3 of 8), the 1-ring is {0, 4}, vertices 5..8 are untouched, the largest
displacement is 4.59 mm; at bulge 0.5 all 8 vertices are pushed by about 6.0 to 6.3 mm; at bulge 1.0 by
22.0 to 23.2 mm (over the 20 mm default cap, refused).

## Addendum (Rejected-guard): one more test, a sixth mutant

**R** = `adapt_rejected` (the guard that keeps a garment `Rejected` before adaptation from being
adapted is skipped). `a_rejected_garment_is_never_adapted_even_under_the_cap` (32 tests in all):
bulge 1.0 with a 50 mm cap; under R the shell is adapted (about 22 mm) and becomes `Fitted`, so the
verdict equality fails. Predicted: **R** killed by 1 test. PREDICTION, not a measurement.
