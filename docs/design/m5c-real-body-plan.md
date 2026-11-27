# M5-C real-body plan against the fidelity bar (PLAN; ratified by coordinator ruling #82)

Status: PLAN ONLY. No code, no asset download and no compute until the coordinator lifts the
current HOLD and gives a GO. The box mannequin of the VIS-3D kit (`docs/evidence/m5b/vis3d/`) is a
motion and plumbing proof only. It is never the bar for a real body.

## Fidelity bar (owner ruling, via coordinator ruling #81)
A real-body checkpoint must show all of the following at once:
- a sculpted, anatomically shaped mesh at subdivision level, with no visible boxes or limb segments;
- joints that deform cleanly, with no candy-wrapper, elbow pinch or hip gaps;
- a face with features (eyes, brows, mouth, nose), so Look and Express are visible;
- hands with fingers;
- planted feet with no foot slide;
- real lighting and shading with a ground shadow;
- stills at ≥1080p and clips at ≥720p.

A kit below this bar is reported as "plumbing", never as an avatar checkpoint.

## 0. Principle
- libgibson stays generic. It gains generic mechanisms only:
  - dual-quaternion skinning;
  - measured correctives;
  - foot-contact IK;
  - a better reference renderer in an example.
- It never ships a body asset inside the crate, and it knows nothing about who the body is.
- The body is data: a CC0 base mesh, converted offline into a gibson-native fixture.
- Every fidelity item maps to a numeric gate check with a control that must FAIL. These are
  ratified as the acceptance bar (ruling #82). The owner's visual verdict at the checkpoint
  applies on top.

## 1. Asset (ruling #82 Q1: APPROVED with conditions)
- Source: the MakeHuman 1.x base mesh and default rig.
  - The mesh is about 13k vertices of quads with a sculpted face.
  - The rig has fingered hands, about 19 joints per hand, and skin weights.
- **Per-file licence check.**
  - Every file used gets a manifest entry with its source URL, its sha256 and the quoted CC0 text.
  - A file whose licence cannot be verified is dropped, and that part goes procedural.
- **No GPL code and no MakeHuman Python** in the repo or in the converter. The converter is our
  own `python3 -I` script that reads only the data files (OBJ, weights, rig).
- **Fixture (Q3).**
  - Committed only if it is ≤2 MB and verified CC0.
  - Otherwise it stays local with its sha256 pinned, and a fetch-and-verify script is committed instead.
- **Blender / MPFB (Q2): LATER.** It goes to the owner as a question when the ~12 parametric
  controls start. Nothing is installed now.

## 2. Library work (generic, behind `figure`, zero runtime deps per ruling #39)
| Item | API sketch | Oracle / control |
|---|---|---|
| Dual-quaternion skinning (**first**, Q4) | `SkinningMode::{Linear, DualQuat}` on the revision. Additive; LBS stays the default. | f64 DQS oracle; mutant `dqs_as_lbs`; the cross-section check below |
| Pose-space correctives (**only where a measured DQS check fails**, Q4) | Morph target driven by a joint-angle reader (cone/twist to weight, smoothstep) | Analytic driver curve; one mutant per corrective |
| Catmull-Clark subdivision (optional) | `subdivide(mesh, levels)` before skinning, with interpolated weights | Euler-characteristic and limit-surface checks; mutant `no_crease` |
| Two-bone IK and foot lock | A `FootContact` motion wrapper. Stance windows lock the foot in world space. The knee uses a pole plane. Blend in and out is C1 over the settle ease. | Analytic reach test; foot-slide metric; mutant `contact_ignored` |
| Root motion | The clip carries pelvis translation, and the walk is matched to stride length | Stride and velocity consistency test |

Existing limits suffice:
- `MAX_JOINTS` 256 (a full rig needs about 70–160);
- `MAX_SKIN_VERTICES` 262k (about 53k after one subdivision level).

`MAX_MORPH_DELTAS` (262k) may bind with full-face expressions. Measure it first; any raise needs a
ruling.

## 3. Reference renderer (example, deterministic, no deps)
- Smooth normals, with Blinn-Phong key, fill and rim lights.
- 2×2 supersampling.
- A planar projected ground shadow with a deterministic soft blur.
- Stills at 1920×1080. Clips at 1280×720, 30 fps, H.264 kept local.
- Committed stills are downscaled PNGs, a few hundred KB at most in total.

## 4. Gate and checkpoint (same shape as C-1 and VIS-3D)
The instruments are committed in-branch and run in a detached worktree under heavy:
- RED;
- head tests;
- the mutant kill matrix (each mutant killed by a non-golden assertion);
- clippy `-D warnings`;
- `+1.85` check;
- default check.

| Fidelity item | Numeric check | Control that must FAIL |
|---|---|---|
| no box segments | one connected manifold per body region; no duplicated seam vertices; subdivision on | the VIS-3D box mannequin |
| clean joints | elbow, knee and hip cross-section area ratio ≥ 0.85 at 120° flex | LBS on the same pose |
| face | eye, brow, mouth and nose landmark sets are visible from the front camera (pixel coverage > 0) | a face-less mesh |
| hands | ≥ 5 finger chains per hand, each with ≥ 2 joints; a fist clip renders | a mitten rig |
| planted feet | stance-phase foot world velocity ≤ 2 mm/s | the walk with IK disabled |
| shading and shadow | shadow pixel coverage under the feet > 0; luminance range above threshold | the renderer with the shadow pass off |
| resolution | stills ≥ 1920×1080 and clips ≥ 1280×720, asserted from the file headers | a 320×320 render |
| continuous motion | the C-1 jump metric J ≤ bound for the loop seam, preempt, Look and contact blend in/out | the VIS-3D hard-cut control |

At the checkpoint the session views every still plus a frame strip per clip and names the
defects it sees. The owner's verdict comes via the coordinator.

## 5. Order
Each slice is committed, pushed to a feature branch, opened as a draft PR and recorded in state.
1. Asset licence verification and the converter. The fixture is committed or pinned per Q3.
2. DQS, its oracle and its mutant.
3. Correctives, only for measured DQS failures; subdivision.
4. Foot-contact IK and the root-motion walk.
5. Renderer upgrade.
6. The real-body kit lease, the RESULT, then the checkpoint.

Labour: sonnet/high for the math in steps 2–4, and sonnet/medium for steps 1, 5 and 6.

## 6. Compute budget (Q5)
The first real-body run is a single timed `s` lease of at most 20 minutes, for measurement.
Later leases are sized from the heavy log of that run.
