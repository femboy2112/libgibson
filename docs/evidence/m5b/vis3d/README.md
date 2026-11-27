# VIS-3D visual checkpoint kit (M5-B, `figure`)

Status: DRAFTED WITHOUT COMPILING OR RUNNING. Nothing here has been built or executed; every
number below is a prediction or a criterion, not an observation. The coordinator runs it under
the heavy lock.

## What it shows

`examples/figure_vis3d_kit.rs` is a deterministic software rasterizer (no threads, no clock, no
dependency; fixed perspective camera, z-buffered flat Lambert shading, 320x320 binary PPM) over a
synthetic procedural humanoid `BodyFamily` (`synthetic.humanoid@1`):

- 17 joints: `pelvis spine chest neck head`, `{l,r}_{upper_arm,lower_arm,hand}`,
  `{l,r}_{upper_leg,lower_leg,foot}`. Identity rest rotations, parent before child.
- 17 tapered 8-sided prisms, 4 sections each, 714 vertices, 1360 triangles. Rings near a joint
  blend two joints (0.5/0.5 at the joint ring, 0.85/0.15 next to it), so bends deform smoothly.
- Morphs `broad`, `head_big`; controls `bulk`, `head_size` (one morph weight each);
  5 landmarks (`head_top`, `l_hand_tip`, `r_hand_tip`, `l_toe`, `r_toe`).

Stages (`figure_vis3d_kit <out_dir> [stills|turntable|motion|analysis|all]`):

| output | content |
|---|---|
| `stills/` | bind pose `front`, `side`, `three_quarter`, `back`, plus `morph_front` (bulk=1, head_size=1) |
| `turntable/` | 72 frames, 5 degrees each |
| `motion/loop` | `Smooth` + `Repeat::Loop` walk-ish clip (1.2 s period), 3.6 s at 30 fps, loop seams at 1200 and 2400 ms |
| `motion/preempt` | the same loop `Preempt`ed at 1450 ms (not on the frame grid; arm at high speed) by a one-shot wave clip, ease 400 ms |
| `motion/look` | the loop plus a `Look` on `head`, retargets at 1450 and 2450 ms, ease 350 ms |
| `analysis/` | `motion_analysis.json`, `frames_*.csv`, `points_*.csv`, `substep_*.csv` |

The walk-ish clip is not a locomotion solution (feet are not planted). A fourth analysis-only
scene, `control_snap`, is a hard cut (not a library motion) at the preempt instant: the instrument
control.

## Run

```
CARGO="cargo +1.98.1" docs/evidence/m5b/vis3d/run_kit.sh /path/to/local/dir
```

Needs `convert`, `montage` (ImageMagick) and `ffmpeg`. `KIT_BIN` skips the build. The script runs
the example twice and requires bit-identical PPM trees and analysis JSON, writes full-size PNGs and
H.264 MP4s (yuv420p, 30 fps, upscaled 2x nearest) per motion scene and the turntable into the
LOCAL dir (never committed), and stages the committed subset in `out/`: 5 stills, a contact sheet
(5 stills + 12 turntable thumbnails, 6 columns), first/mid/last frames (000, 054, 107) of each
motion scene, `motion_analysis.json`, `manifest.txt` (sha256 of every committed file, plus
tree hashes of the local PPM directories). Expected size: roughly 100 to 250 KB; the script
warns above 400 KB. `out/.gitignore` admits nothing else.

## Metrics

Tracked points: the 17 joint origins plus the 5 landmarks (landmarks make head and hand rotation
visible, since a joint origin does not move when the joint rotates in place).

At 30 fps, per frame: `max_vertex_disp_m` (largest vertex displacement since the previous frame),
and per point the finite-difference speed and the frame-to-frame velocity change `dv` (m/s);
the max of each, its frame, its nearest event and the offset in ms are reported.

At 1 ms sub-step, for every event, over a window of +/-25 ms: the largest per-ms vertex step (mm)
and the velocity-jump estimate `J = |p(t+h) - 2p(t) + p(t-h)| / h`, `h = 1 ms`, per point. For a
C1 path `J <= A h` (`A` = largest acceleration); a true velocity jump of size `V` appears as
about `V`. `scaling_ratio_2ms_over_1ms` is `J(2 ms)/J(1 ms)` at the instant: about 2 for a smooth
path, about 1 for a jump (informational).

Bounds (declared before any run):

- Transitions (preempt, retarget). From the C-1 model (`tests/figure_motion.rs`, "Quintic
  offset"): the inertialization offset has `|offset''| <= 5.78|x0|/T^2 + 4|v0|/T`. Per point,
  `A_bound = A_old + A_new + 5.78 x0/T^2 + 4 v0/T`, where `A_old`, `A_new` are the largest
  1 ms second-difference accelerations of the old and new motions alone over
  `[t0-30 ms, t0+T]`, `x0` is `|p_old(t0) - p_new(t0)|`, `v0` the world velocity difference at
  `t0` (central 1 ms difference), `T` the ease. The pass bound is
  `SAFETY * A_bound * h + 4 u P / h` with `SAFETY = 2` (slack for forward kinematics being
  nonlinear, since the derived bound is for channel space), `u = 2^-24`, `P = 3 m`.
- Loop seams: no derived channel bound is available at the seam, so the criterion is
  `J(seam window) <= baseline + roundoff`, where `baseline` is the largest `J` over the whole
  scene outside every event window (the loop is periodic, so the seam must not exceed the
  interior).

`worst_ratio_j_over_bound <= 1` is a pass.

## Acceptance numbers (for the coordinator)

1. Determinism: the script's two runs agree (it exits 1 otherwise).
2. Scene `loop`: both seam events pass; `max_velocity_jump_m_s` at the seams within the baseline.
3. Scene `preempt`, event `preempt_1450ms`: `pass: true`, `worst_ratio_j_over_bound <= 1`, and
   `velocity_jump_at_instant_m_s` of the order `A h` (a few 1e-2 m/s at most), far below the
   hard-cut value of the control.
4. Scene `look`, events `retarget_1450ms` and `retarget_2450ms`: both `pass: true`.
5. Control `control_snap`: its event must show `pass: false` with
   `velocity_jump_at_instant_m_s` orders of magnitude above the bound (the hard cut is a position
   jump, so `J` is about `dx / h`). If the control passes, the instrument is blind and no other
   number counts.
6. `all_ok: true` in `motion_analysis.json`.
7. 30 fps context (informational, no derived bound): `max_vertex_disp_m` and `max_frame_dv_m_s`
   should not peak at the preempt or retarget frames merely because of the transition; report
   where they peak relative to the events.

## Assumptions not verified (nothing was compiled)

- The example compiles and the family passes `compile` (including ruling #70 id rules; all ids
  are lowercase ASCII with `_` or `.`).
- World-space application of the channel-space offset bound (hence `SAFETY = 2`) is adequate;
  if a legitimate transition fails by a small ratio, that is a bound-model question, not
  automatically a motion defect.
- 1 ms sampling is exact at the event instants because they are integer milliseconds.
- Rotation sign conventions (arm and leg swing directions) look plausible; that is a pixel-review
  question.

## What this does not certify

This kit produces measurements and images. It does not decide that the figure "looks right".
Pixel review by the coordinator (viewing stills, contact sheet and MP4s) and the owner's review
are separate steps and neither is implied by a passing analysis.
