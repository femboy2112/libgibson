# Predicted kill matrix, M5-C skeleton refit (PREDICTION, not a measurement)

Status: **predicted, not run.** `src/figure/refit.rs` and `tests/figure_refit.rs` were written
without a compiler or a test process. Every entry below is derived by hand from the assertions
and the fixtures; there is no model script for this slice (the analytic fixtures are small enough
to derive on paper, and the derivations are in the test's doc comments). The first heavy run
decides. Replace this with a measured matrix from `docs/evidence/m5c/gate_dqs.sh`.

Mutant: **R** = `ignored_refit` (the anchors are ignored; every joint keeps its declared rest
translation, so the surface no longer drives the skeleton). `K` = predicted to fail (killed),
`-` = predicted to pass.

| Test (`tests/figure_refit.rs`) | R | Basis |
|---|---|---|
| `joints_land_on_the_group_centroids_in_a_rotated_frame` | K | the declared junk (9,9,9) and (7,7,7) survive instead of (2,0,0) |
| `a_shaped_surface_moves_the_skeleton_and_the_unanchored_child_rides_along` | K | the arm stays at (0,1,0) instead of (0,1.3,0.1); the half of the test with the NEUTRAL surface passes under R, the shaped half is what kills |
| `derived_skeleton_matches_the_f64_centroids_and_inverse_binds` | K | the oracle world positions are far from the f64 centroids (declared translation 8,8,8) |
| `the_centroid_sums_in_declared_order` | K | the root translation stays 0 where 1/3 is expected |
| `no_anchors_changes_nothing` | - | with no anchors R and the real code agree |
| `bad_anchors_are_typed_errors` | - | validation precedes derivation |
| `group_limits_are_enforced` | - | limits are checked before derivation |

R is killed by 4 tests, none of them a golden (there is no golden in this file). What this does
not establish: that the oracle comparison and the analytic values agree to the declared `TOL`
(1e-4) in `f32`; that the order test's f32 absorption (`1e8 + 1 == 1e8`) holds on the build (it
does for IEEE-754 binary32 and the crate forbids fused multiply-add by construction, but it has
not been run).
