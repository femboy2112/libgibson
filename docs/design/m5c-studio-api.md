# M5-C studio API: `figure::studio` (DRAFT shape, agreed with the Avatar Studio consumer)

Status: DRAFT. It freezes after the rev.v1 DRAFT in `docs/FIGURE_CONTRACT.md` lands and the first
real-body lease measures the data. libgibson stays generic: no Being, persona or character semantics.
Consumers map their own recipe onto `RecipeV1`.

## 1. Inputs: `RecipeV1` (generic, versioned, all fields explicit)
- **Body controls:** about 12 `f32` values: stature, torso/leg proportion, shoulder breadth,
  ribcage, bust projection, bust fullness, waist, hip breadth, glute projection, thigh fullness,
  one face parameter and one posture parameter.
  - Each has a documented domain, but values outside the domain are **not** silently clamped.
  - The compiler reports `ControlOutOfDomain` as a diagnostic and still applies the value, unless
    the geometry check fails.
  - There is no downward compensation between controls.
- **Footwear:** heel height (m) and toe pitch. These feed the stance, ankle and foot-contact
  calibration.
- **Garment set:** a list of garment ids drawn from the asset manifest. Each garment carries its
  own material slot bindings.
- **Hair:** a style id and a colour.
- **Attachments:** a list of accessory ids. Each names an anchor (a landmark or a bone) and an
  optional emissive colour.
- **Material slots:**
  - Named slots: `base`, `panel`, `trim`, `ornament`, `gem_emissive`, `mask`, `iris` and
    `accessory_emissive`.
  - Each slot has a linear RGB albedo, roughness and metalness, plus an optional emissive
    colour and intensity.
  - A consumer tint can therefore touch only the slots it names.
- **Face cover flag:** `lower_face_covered: bool`. When it is set, expression routes to the eyes,
  lids and brows; jaw and mouth channels are inert, and the compiler emits a diagnostic if they
  are driven anyway.
- **Motion style:** a preset id (for example `restrained` or `expressive`) plus scalar
  amplitude, tempo and settle-ease values.
- **Provenance passthrough:** an opaque consumer key plus a sha. It is echoed in `Compiled`. It
  never enters any identity hash, so it cannot rewrite the body.

## 2. Identities (three, distinct; see rev.v1)
| id | hashes | invariant |
|---|---|---|
| `body_rev_id` | neutral asset sha, body controls, footwear | garment, material, hair or style edits leave it bit-identical |
| `fit_id` | `body_rev_id`, garment set, fit params | carries a `FitStatus` per garment; a missing or failed verdict means the garment is not drawn |
| `present_id` | `fit_id`, materials, hair, attachments, style | presentation cache key only |

## 3. Calls
```text
studio::compile(&RecipeV1, &AssetSet) -> Result<Compiled, CompileError>
Compiled::{body_rev_id, fit_id, present_id, fit_status(), diagnostics(), provenance()}
Compiled::preview(&Camera, w, h) -> RgbRaster          // software reference renderer
Compiled::perform(&PerformanceScript, fps) -> Tape     // offline, deterministic
Tape: {present_id, fps, frames: per-frame bone locals + root, sha256}
Compiled::live() -> Live                               // incremental, interactive
Live::submit(Intent{kind, intensity, target?, ease_ms, priority, expires_ms}, now_ms)
Live::tick(now_ms) -> Pose                             // + Live::render(&Camera,w,h) -> RgbRaster
Live::set_reduced_motion(bool); Live::quiesce(); Live::stop()
```
- **Continuity:**
  - Every `submit` blends from the current evaluated pose, with position and velocity matched
    (C-1 inertialization).
  - An expired intent eases out; it never snaps.
  - `stop()` is immediate: the next `tick` returns the held pose and schedules no work.
- **`quiesce()` and hidden presentation:** `tick` returns without evaluating anything. This is the
  same zero-work contract as `stage::motion`.
- **Reduced motion:**
  - Amplitude is clamped to a low-motion envelope and locomotion becomes a cross-dissolve to the
    target pose.
  - This is a lifecycle policy, not an aesthetic clamp on the body.
- **One semantic stream:** the same admitted intents drive the sprite backend and the figure
  backend. `Intent.kind` is a generic, closed, `#[non_exhaustive]` set (look, acknowledge, wave,
  sit, stand, walk_to, idle, express{channel}).
- **Same tape for both renderers:** the libgibson preview and the Blender offline adapter both
  consume `Tape`. The offline render is labelled "offline reference render of a runtime-evaluated
  performance".

## 4. Out of scope for FIRST PERFORMANCE
Multiple actors, scene graphs, full wardrobe, cloth or hair dynamics, and lip sync. These belong in
`docs/design/DIRECTOR_ROADMAP.md`.

## 5. Tape file format `gibson.figure.tape.v1` (the shared contract between runtime, preview and Blender adapter)
- **Files:** `<name>.tape.json` (the header) plus `<name>.tape.bin` (little-endian f32, no padding).
- **Header fields:**
  - `format`: `"gibson.figure.tape.v1"`.
  - Identities: `body_rev_id`, `fit_id`, `present_id` (hex).
  - `asset_manifest_sha256`.
  - `controls`: {name: value}, in exact recipe order.
  - `garments`: [id], plus `fit_status` {id: verdict}.
  - `materials`: {slot: {...}}.
  - `style`: {preset, amplitude, tempo, settle_ms}.
  - `fps`, `frame_count`.
  - `joints`: [names], in skeleton order. Every name must exist in the exported rig.
  - `channels`: `["root_t3", "local_t3", "local_r4", "local_s3"]`.
  - `face`: [channel names].
  - `events`: [{frame, kind, label}].
  - `camera_cuts`: [{frame, camera_id}].
  - `bin_sha256`.
- **Body bytes:** each frame is laid out in this order:
  1. root translation (3 values);
  2. per joint, in `joints` order: local translation (3), rotation quaternion xyzw (4), scale (3);
  3. face channels (`len(face)` values).
- **Determinism:** the runtime evaluates `pose(t)` at `t = frame / fps` with fixed f32 order. The
  same recipe plus the same script gives byte-identical tape files.
- **Consumers:**
  - the libgibson software preview, which skins with the selected `SkinningMode`;
  - the Blender adapter, which sets pose bones from the locals, applies the same control values
    through MPFB, and fits the same garments.
  - The oracle compares the posed vertices of both on sampled frames.

## 6. Naming contract v1 (rig, controls, face channels): the names consumers map onto
Asset family `gibson.figure.asset.v1`, generated from the MPFB/MakeHuman CC0 assets by
`tools/figure_assets/`. Names are exact, case-sensitive ASCII. Anything that is not on these lists
is unmapped, and a consumer must return a typed error for it. It must never guess.

### 6.1 Joints (MPFB `game_engine` rig, 53 joints, parent order)
`Root pelvis spine_01 spine_02 spine_03 clavicle_l upperarm_l lowerarm_l hand_l index_01_l index_02_l
index_03_l middle_01_l middle_02_l middle_03_l pinky_01_l pinky_02_l pinky_03_l ring_01_l ring_02_l
ring_03_l thumb_01_l thumb_02_l thumb_03_l clavicle_r upperarm_r lowerarm_r hand_r index_01_r
index_02_r index_03_r middle_01_r middle_02_r middle_03_r pinky_01_r pinky_02_r pinky_03_r ring_01_r
ring_02_r ring_03_r thumb_01_r thumb_02_r thumb_03_r neck_01 head thigh_l calf_l foot_l ball_l
thigh_r calf_r foot_r ball_r`
- **Roles used by `perform::RigRoles`:**
  - pelvis = `pelvis`;
  - spine = `spine_01..03`;
  - neck = `neck_01`;
  - head = `head`;
  - arms = `upperarm_*`, `lowerarm_*` and `hand_*`;
  - legs = `thigh_*`, `calf_*`, `foot_*` and `ball_*`.
- The rig has no jaw, eye or eyelid bones. The face is driven by the morph and eye channels below.

### 6.2 Body controls (13; bipolar, documented domain [-1, 1], never clamped)
`stature leg_length torso_length shoulder_breadth ribcage bust_projection bust_fullness waist
hip_breadth glute_projection thigh_fullness jaw_width posture_lean`
- Each control maps to MakeHuman target files, listed in `tools/figure_assets/controls.json`.
- The face parameter is `jaw_width`, and the posture parameter is `posture_lean`.

### 6.3 Face channels (tape `face` array and `Express{channel}`), value range [0, 1] unless noted
Morph channels are driven by MakeHuman expression-unit targets (CC0). They are blended across the
three bundled ethnic unit sets with the body's ethnic weights (1/3 each for the generic presets).
| channel | expression-unit target(s) |
|---|---|
| `brow_up_l` / `brow_up_r` | eyebrows-{left,right}-up |
| `brow_down_l` / `brow_down_r` | eyebrows-{left,right}-down |
| `brow_inner_up_l` / `brow_inner_up_r` | eyebrows-{left,right}-inner-up |
| `brow_outer_up_l` / `brow_outer_up_r` | eyebrows-{left,right}-extern-up |
| `lid_close_l` / `lid_close_r` | eye-{left,right}-closure |
| `lid_wide_l` / `lid_wide_r` | eye-{left,right}-opened-up |
| `lid_squint_l` / `lid_squint_r` | eye-{left,right}-slit |
| `jaw_open` | mouth-open |
| `mouth_smile` | mouth-corner-puller |
| `mouth_frown` | mouth-depression |
| `mouth_press` | mouth-compression |
| `mouth_pucker` | mouth-pursing |
| `nose_wrinkle` | nose-compression |
| `gaze_yaw` / `gaze_pitch` | not a morph: a rigid rotation of both eye meshes about their centroids, in degrees, ±35 yaw and ±25 pitch |
- When `lower_face_covered` is set, the `jaw_*`, `mouth_*` and `nose_*` channels are inert, and
  driving them produces `Diagnostic::FaceChannelCovered`.
- **Blink and saccade** are composed from `lid_close_*` and `gaze_*`. They are never separate names.

### 6.4 Garments, hair and material slots
- **Garment ids:** `female_casualsuit01` (fitted) and `skirt_a_line` (self-authored, loose).
- **Hair ids:** `long01` and `ponytail01` (CC0).
- **Material slots:** see section 1.
- Ids come from the asset `index.txt`. Asking for an id that is not there gives `CompileError::UnknownPart`.
