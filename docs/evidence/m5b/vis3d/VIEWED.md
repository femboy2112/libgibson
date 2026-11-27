# VIS-3D kit: lease result and viewed defects

Lease run at `973afc4` (instrument `docs/evidence/m5b/vis3d/lease_vis3d.sh` @ `973afc4`). It ran
under the heavy `s` slot from 00:28Z to 00:38Z with `LEASE_FAILS=1`.

## Steps
1. `gate_c1.sh`: **GREEN**.
   - RED 27/27 fail.
   - Head tests: motion 28, sha256 10, clip 19, mechanics 37, canonical_bytes 4.
   - Mutants are killed: snap_transition 12, freeze_on_preempt 7, linear_keys 7, with 0 src panics.
   - clippy, `+1.85` and the default check all pass.
2. clippy on the `figure_vis3d_kit` example: **FAIL**.
   - `too_many_arguments` on `analyse_event`.
   - `needless_range_loop` in the 1 ms sweep.
   - Both are fixed in the commit that adds this file. That fix is UNCOMPILED until the next gate or CI.
3. Kit run: exit 0, with `all_ok=true`. The two runs are determinism-identical: the PPM trees and
   the analysis JSON match bit for bit.

## Motion numbers (1 ms sweep, J = |p(t+h) - 2p(t) + p(t-h)| / h)
| Scene | Event | J at the event (m/s) | J / bound | Verdict |
|---|---|---|---|---|
| loop | seam 1200 ms and seam 2400 ms | 0.0348 (window max 0.0377) | 0.981 | PASS, thin margin |
| preempt | 1450 ms (v != 0) | 0.0175 (window max 0.0313) | 0.199 | PASS |
| look | retarget 1450 ms | 0.0214 | 0.207 | PASS |
| look | retarget 2450 ms | 0.0282 | 0.241 | PASS |
| control_snap | hard cut 1450 ms | **298.75** (window max 301.56) | **2148** | **FAIL, as required** |

At 30 fps the largest per-frame vertex displacement is 0.127 m (loop), 0.211 m (preempt) and
0.268 m at the hard cut. The largest per-frame dv is 1.09, 1.91 and 10.93 m/s respectively.

## Defects seen by the session (stills, contact sheet, preempt and look strips around the events)
1. **The walk runs in place.** The pelvis does not translate, so the feet slide on the floor. There
   are no foot plants, and the figure reads as walking on a treadmill.
2. **The Look retarget is not legible.** The head is a small box with no face or nose cue, so the
   head turns at 1450 ms and 2450 ms cannot be seen in the frames. Only the numbers show the
   retarget.
3. **The raised arm passes through the head region** in the preempt "wave" (frames 60–80). There is
   no self-collision.
4. **Joints pinch and gap.** The forearm thins at the elbow during the wave (candy-wrapper LBS),
   and thigh boxes poke out at the hips mid-stride.
5. **`morph_front.png` is not a front view.** It is rendered at `camera(25.0, 8.0)`, a 25° yaw,
   so it cannot be compared pixel for pixel with `front.png`. The `bulk` / `head_size` = 1.0
   morph is barely visible at this size.
6. **The floor is a finite plate.** Its corner and edge show in turntable frames, giving a kinked
   horizon. A thin bright horizontal line crosses the sky in some side-on contact-sheet tiles.
7. **Flat lighting with no ground shadow.** Ground contact is ambiguous.
8. **The loop seam passes at 0.98 of its bound.** The window maximum equals the global baseline at
   t = 1 ms, the sweep start in the same phase, so this is not a seam defect. The margin is thin,
   though, and the bound model should be revisited before this number is called comfortable.

Viewing is not acceptance. Leah's verdict comes via the coordinator.
