# Predicted kill matrix, M5-B Slice 1 (PREDICTION, not a measurement)

Status: **predicted, not run.** No compiler or test process was available when the fixture and
tests were written, so none of the Rust below has been compiled or executed. The numbers come
from `docs/evidence/m5b/predict_kills.py`: a numpy float32 model of the implementation (it
shares formulas with the golden script, and its skinning blend order is an assumption) set
against an f64 row-major oracle, with each planted mutant applied by hand. Each number is the
largest `|impl - oracle| / max(1, |oracle|)`; a test with `TOL = 1e-4` kills the mutant when the
number is above `1e-4`. The matrix must be replaced by a measured one (run under heavy with
`RUSTFLAGS='--cfg figure_mutant="..."'`) before it is cited as evidence.

Mutants: **A** = `ignored_skeleton_update`, **B** = `stale_bind`, **C** = `noop_morph`.
`K` = predicted to fail the test (mutant killed). `-` = predicted to pass (mutant survives that
test). `n/a` = not predicted here.

## Kill matrix by bearing

| Mutant | golden (`golden_rev_id`, REGRESSION PIN, not independent) | oracle-posed (`rich_posed_mesh_matches_oracle`) | oracle-world_bind / IB (`rich_world_bind_and_inverse_binds_match_oracle_directly`) | hand-bytes (`tests/figure_canonical_bytes.rs`) |
|---|---|---|---|---|
| A `ignored_skeleton_update` | K (shaped joint translations and IB bytes change) | K only in cases that replace a scaled child's local: 7.6e-2 to 2.1e-1. `-` at the bind pose and in the root-only case (2.4e-7). | K (inverse bind 2.0e-1, world bind 1.9e-1) | n/a (separate author, its own tiny fixture) |
| B `stale_bind` | K (IB bytes change) | K in every case: 1.0e-1 to 1.3e-1, including the bind pose | K on the inverse binds (2.0e-1). `-` on the world bind (1.6e-7): W is shaped, only IB is stale. | n/a |
| C `noop_morph` | K (shaped positions change) | K in every case with a non-zero morph weight: 6.0e-2 to 1.0e-1 | `-` (1.8e-7 and 1.6e-7): morphs do not touch skeleton matrices | n/a |

## Non-golden killers, by test

Every mutant has at least one killer that is not the golden.

| Test | A | B | C | Note |
|---|---|---|---|---|
| `rich_world_bind_and_inverse_binds_match_oracle_directly` | K | K | - | direct W and IB, 4 control sets |
| `rich_bone_length_controls_scale_direct_children_only` | K | - | - | exact rest translations |
| `rich_landmarks_match_oracle_at_bind_and_posed` | K | - | - | landmarks at bind and posed; both use W only, which B does not change |
| `rich_posed_mesh_matches_oracle` | K (scaled-child cases only) | K | K | see the bearing table |
| `rich_identity_pose_reproduces_the_shaped_neutral` | - | K | K | A cancels at the bind pose |
| `rich_morph_controls_accumulate_on_one_target` | - | - | K | compile-time morph, two controls on one target |
| `rich_pose_time_morph_moves_vertices_before_skinning` | - | - | K | pose-time path, compile-time weights zero |
| `rich_each_control_changes_rev_id` | K | - | K | |
| `bone_length_control_reshapes_skeleton` (simple fixture, hand-computed) | K | - | - | |
| `identity_pose_equals_bind_for_shaped_bodies` (simple) | - | K | K | |
| `inverse_binds_times_world_bind_is_identity` (simple) | - | K | - | |
| `oracle_matches_implementation` (simple) | K | K | K | |
| `morph_control_moves_vertices` (simple) | - | - | K | |
| `pose_morph_weight_moves_vertices` (simple) | - | - | K | pose-time path |
| `each_control_changes_rev_id` (simple) | K | - | K | |
| `weights_are_applied_as_declared_never_renormalized` | - | - | - | kills a renormalizing implementation, which is not a planted mutant |

## What each test cannot see (so nobody over-reads a pass)

- The posed mesh at the bind pose, with a root-only pose, or with any pose that does not replace
  a bone-length-scaled child, cannot see mutant A: the scaled local cancels in `W' * IB`
  (PASS1 finding 5). The `root-only override` case is in the suite on purpose as the control
  that documents this.
- The world bind comparison cannot see mutant B; only the inverse bind and the skinning product
  can.
- No oracle comparison can see a skin weight sum of `1 +- 5e-5` being renormalized (`5e-5 * |x|`
  is below `TOL * max(1, |x|)`). Only the absolute-tolerance test does.
- The golden is a regression pin that shares the implementation's formulas. A formula error that
  the implementation and the script share is invisible to it.

## Model checks (python3 -I docs/evidence/m5b/predict_kills.py)

The unmutated model agrees with the oracle to `1.1e-7` to `2.1e-7` in every pose (about 500x
below TOL), which is consistent with the `N * u * S` bound of `4.6e-5` being very loose in
practice. This says nothing about the Rust implementation until it is run.
