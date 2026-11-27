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
