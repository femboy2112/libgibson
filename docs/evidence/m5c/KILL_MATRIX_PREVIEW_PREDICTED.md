# M5-C preview: PREDICTED kill matrix (nothing here was compiled or run)

Status: PREDICTION, written before any heavy run (ORDER #105). The gate
(`docs/evidence/m5c/gate_preview.sh`) records the observed result; this file must not be read
as evidence. Target: `src/figure/preview.rs`, tests `tests/figure_preview.rs`.

| Mutant / control change | Where | Predicted killer (assertion, not golden) | Why |
|---|---|---|---|
| `figure_mutant="preview_stale"`: render cache keyed on counts + options, ignores vertex values | `render_preview` (cfg) | `control_change_changes_subject_pixels` (`changed > 1500` fails: 0 differing pixels) | the moved-vertex render returns the cached first image |
| same mutant | same | `ground_shadow_present_and_control` survives (the key includes the options, so shadow on/off do not collide) | one killer suffices |
| same mutant | same | `material_slot_selects_colour` (same counts, same options: second render returns the red image, so `pg[1] > pg[0]` fails) | stale image |
| same mutant | same | `deterministic_byte_identical` survives (identical inputs) | by construction; it is the control that the cache is otherwise invisible |

Non-mutant controls built into the tests (each must be able to fail):
- identical input twice gives 0 differing pixels, so the moved-vertex diff is not noise;
- `shadow:false` leaves shadowed and far ground pixels equal within 3 luma levels;
- the lit-side pixel is unchanged by the shadow flag.

Not covered (honest boundary): no test judges appearance; the PNG is viewed by the session after
a heavy run (BRIEF §V). Timing (target < 1 s release for ~40k triangles at 960x540) is printed
by the example and is not asserted.
