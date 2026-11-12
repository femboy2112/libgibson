# facade_signal / textured_quad — consumer acceptance (v0.7.0-alpha.2)

Consumer test cases supplied by an external downstream application from its app-local
facade portal (Renaissance XXXII). Each maps to a test in this repository; the column
"Test" is filled in as they land.

| # | Case | Expected | Test |
|---|---|---|---|
| 1 | Projection calibration | `textured_quad` vertex projection equals `Camera::project` within 1e-3 | `tests/textured_quad.rs::case1_vertex_projection_matches_camera_project` — **measured as drawn pixel extents within 1 px of `Camera::project`**, not 1e-3: vertices go through the same `View::project` that `Camera::project` uses, but the observable is rasterised coverage |
| 2 | Affine vs perspective | oblique (~60° yaw) quad: world-space centre samples uv=(0.5,0.5) within 1 texel; affine misses by > 0.05 | `case2_oblique_quad_is_perspective_correct_and_affine_would_miss` (+ ported `texture_mapping_is_perspective_correct_not_affine`; the affine mutation fails both) |
| 3 | Clipping | corners off-screen / behind near plane / whole quad behind camera: no panic, no OOB, drawn pixels ≤ analytic visible area + perimeter | `case3_clipped_coverage_matches_analytic_area`, ported `partially_offscreen_quad_clips_without_panic`, `quad_straddling_near_plane_clips_cleanly`, `quad_behind_camera_draws_nothing` |
| 4 | Degenerate / hostile | NaN, ±inf, coincident, zero-area, 0×0 texture, huge texture: `false`, no panic, no allocation proportional to hostile input | ported `degenerate_quads_are_refused`, `zero_size_texture_is_refused`, `non_finite_corners_are_refused`, `hostile_finite_coordinates_stay_bounded`; `case4_huge_texture_work_is_bounded_by_the_destination` |
| 5 | Depth both ways | filled box in front hides quad; quad hides box behind; wireframe line in front draws over; line behind hidden — independent of draw order | `wireframe_lines_and_textured_facades_occlude_each_other_both_ways`, ported `textured_quad_shares_the_depth_buffer_with_solid_triangles`, `nearer_textured_quad_occludes_farther_regardless_of_draw_order` |
| 6 | Backfacing | documented choice | documented on `Rasterizer::textured_quad`: not culled (shows mirrored from behind); `facade_signal` draws a facade only when the camera is outside its face |
| 7 | Pixel-work bound | documented per-call bound; screen-covering near quad stays within width×height | documented: at most `2 × width × height` depth tests per call (diagonal pixels can be tested by both triangles); distinct pixels touched ≤ `width × height`; `case7_screen_covering_quad_is_bounded` asserts both |
| 8 | Determinism | identical framebuffer hash for identical (camera, texture, corners) | ported `draw_is_deterministic`; `examples/facade_signal.rs::frames_are_deterministic_for_seed_and_t` |
| 9 | Raycaster | deterministic for (seed, t); bounded columns×rows work; seeded maze closed, start reachable | `tests/raycast.rs` (6 tests: deterministic, bounded work, closed + fully reachable seeded maze, rays never leave the grid, hostile views) |
| 10 | facade_signal fallbacks | mono/NO_COLOR/TERM=dumb → text/semantic fallback; <60×20 → fallback; reduced motion → settled frame identical for any t; idle-CPU receipt shows no render spin | `fallbacks_follow_size_and_depth`; reduced motion renders t=0 and blocks on input (live loop); visible-signal area ≥2% at 36 orbit samples (`the_live_signal_is_a_substantial_visible_area_through_the_orbit`); idle-CPU receipt (real PTY 120×40, release build, 3 s window): reduced motion 0 CPU ticks = 0.0% of a core; animated 20 fps 4.67% of a core |
