# Gibson World — Character Rig, Animation and Puppet Runtime (R0 RFC)

**Status:** R0 proposed generic engine contract, documentation only. No skinned 3D avatar, 3D authoring studio, face reconstruction, slider creator, animation scripting runtime, desktop GUI or new permission is implemented by this paper.

**Context:** [Gibson World 2.0 draft #439](https://github.com/femboy2112/libgibson/pull/439) (not merged). Source baseline reviewed: main 793f6ee4a167e0236f848b4495f1be83e95bdc29, including src/scene.rs, src/rig.rs, src/raster3d.rs, src/stage.rs, src/ui/experience, and docs/SCENE_ALGEBRA_LAWS.md.

**Public-repo boundary:** synthetic/procedural or lawfully distributable reference assets only; no private consumer images, face, body measurements, mesh, generated textures, internal paths, approval records or hardware identifiers.

## 0. Purpose, ownership and first useful slice

Provide a **generic interactive rigged actor and object system**: authored meshes, skeletal skinning, face morph targets, gaze, deterministic animation, user-editable shape parameters and scene-local prop interaction. The consumer may fit an actor from photos **offline**, but LibGibson is not an image-to-identity reconstructor and cannot certify unseen anatomy.

**First useful slice:** a procedural synthetic two-joint actor with one skinned mesh, one blendshape, a keyframed look/reach, a prop attach/release, eight typed morphology controls, deterministic offscreen frames and truthful terminal fallback. Do this **before** trying real photography, full-body cloth or GPU-native Minerva visuals.

Existing seams must not be misreported:
- src/rig.rs handles **camera Dolly/Orbit/Transition**, not humanoid bone rigs.
- src/raster3d.rs draws experimental software triangle meshes and textured quads; output passes through terminal RGB/half-block surfaces. It does not implement general skinning or face animation.
- Scene/Stage/Experience provide semantic entities and finite explicit-time effects; action order and interchange laws are conditional, not universally commutative.
- Real-pixel GUI is proposed by draft #439 and is **not yet a shipped renderer**.
- A consumer's model identity, body approval, reference photos, facial features and permission policy are NOT LibGibson data.

## 1. Immutable assets, interoperable formats and integrity

Candidate exchange: **glTF 2.0 / GLB** for mesh, node graph, skin, morph target and keyed animations; optional **VRM 1.0** humanoid adapter for expressions, gaze and spring/constraint semantics. Do not require VRM for robots, animals or nonhumanoids. Official standards: [glTF 2.0](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html), [VRM](https://vrm.dev/en/vrm/vrm_features/).

Proposed bounded generic records:
- MeshAsset: topology, vertices, normal/tangent, UV seams, material groups, morph deltas and limits.
- RigAsset: hierarchy DAG, neutral/bind pose, joint frame, inverse bind, skin weights, bone limits, per-rig orientation.
- AppearanceAsset: texture atlas and materials, explicit resolution/alpha/color, no implicit network fetch.
- ClipAsset: keyed channels, curves, event markers, finite duration, explicit loop count and layer priorities.
- PropAsset: independent mesh, transform, pivot, allowed hand socket, collision/scene-only interaction.
- ControlSchema: named semantic parameters, ranges/units, coupling, per-axis scope, revision lineage and constraints.
- BundleManifest: schema/version plus source/normalized geometry/texture/clip SHA-256, import settings and application-owned approval reference.

**Separate four meanings of hash.** (1) Exact image/mesh bytes; (2) normalized authoring recipe digest (algorithm, version, seed, camera and manual edits); (3) canonical **neutral geometry** digest covering topology, ordered vertices, bind pose and quantized configuration with explicitly declared serialization; (4) consumer-approved identity/body/appearance revision pointer. A photo hash cannot generate its hidden geometry. A geometry digest proves byte identity, **not** correctness of likeness or authorization. Posing must not alter its parent neutral geometry digest.

Imported data: reject cycles, invalid indices, NaN/Inf, degenerate triangles, inconsistent inverse binds, impossible skin weights, unsafe allocation, unknown external URI, path traversal, unbounded textures, excessive morph counts, arbitrary shaders and malformed animation streams. Model-generated art may be treated as a candidate only; approval and revocation stay with the consumer.

## 2. Geometry is not a stretched sprite

A generic morphology model can take neutral vertex coordinates G0, user shape controls p, expression weights e and articulated pose q:

    NeutralShape(p,e) = G0 + ShapeBasis(p) + FaceBasis(e) + Corrections(p,e)
    WorldShape(p,e,q) = Skin(q, NeutralShape(p,e)).

This is a proposed mathematical interface, **not a theorem about source photographs**, and not yet implemented in raster3d. The asset may declare a different corrective-morph order (pre-/post-skin); make this explicit and test rather than silently assuming one order is universal.

**Basic shape controls:** height, head/body proportion, shoulder breadth, chest projection, waist profile, hip shape, leg length, selected face width/eye spacing. These are application-selected aliases over an authored shape space, not independent image warps.

**Expert controls:** per-side asymmetry, bone anchors and segment lengths, authored shape-blend components, landmarks, custom per-region constraints, surface/texture controls, skin weights, garment fit and topology visualizer. In all modes, shape changes create **draft derived revisions**; approval and activation are separate external decisions.

Use the explicit shape derivative

    delta(G) ≈ J_p delta(p),  J_p[i,j] = partial G_i / partial p_j

for sensitivity, coupled control projection and near-boundary diagnostics. This Jacobian describes the model response only. It does **not** establish single-photo depth or imply that two body sliders are physically independent. Clamp or reject a draft when calibrated constraints detect joint-length inconsistency, inverted faces, invalid volume, skin tearing, penetration or topology degradation. Provide deterministic undo/reset and expose uncertain geometry rather than a fake metric PASS.

A clothing/body change requires new garment-fit validation by the application. Do not silently resize a fitted outfit and mark it compatible.

## 3. Runtime puppet channels and evaluation

Generic addressable channels:

| Channel | Scope | Negative constraint |
| --- | --- | --- |
| Root and bones | bind/local transforms, forward and constrained inverse kinematics, joint limits | Animations cannot mutate bind geometry silently |
| Face | morph target weights, jaw/brow/eye/viseme masks | No new facial pixels or identity from an animation track |
| Gaze | window-local scene target or explicit vector | No automatic desktop-camera observation or tracking |
| Props | reach/contact, attach, rotate, release, ground placement | An animated prop does not click an actual OS window |
| Appearance | preapproved textures, material variants | A shader cannot bless a new body or replace unapproved art |
| Hair/veil/cloth | authored rig or optional secondary springs | No unconstrained simulation or anatomy invention |
| Camera/light | existing explicit camera time / world view | Decorative motion cannot become an observed host event |

**Order and time law:** Resolve immutable neutral assets → sample bounded finite clips at explicit time → blend per-channel priorities → solve skeletal constraints/IK → apply declared deformation stack and skin → update props and optional bounded secondary simulation → present/capture one frame with source/semantic receipt. Since facial correction ordering can differ by asset, record the stack in its manifest and validate both branches with a known result.

Same asset + source state + explicit clock + validated seed must reproduce the same neutral pose, CPU geometry and semantic action map. GPU floating-point output may require a declared tolerance/implementation-specific snapshot. Completed finite clips settle and request **no future repaint**. Reduced motion yields a designed static pose, not hidden motion.

## 4. Scriptable animation without scripting the host

Default authoring **is a closed data-only clip language** with versioned typed channels, no evaluation of shell/Python/JavaScript/network/file/camera calls or model-generated executable code. A user can choose a preset, edit keyframes/easing and preview/undo with no programming.

Illustrative **non-executable** clip grammar:

    format: gibson.puppet.clip.v0
    clip: inspect_prop
    duration_ms: 2300
    loop_count: 1
    channels:
      - target: bone.head
        property: rotation
        keys: [ [0, [0,0,0]], [550, [0,0.18,0]] ]
      - target: expression.blink
        property: weight
        keys: [ [700,0], [760,1], [880,0] ]
      - target: prop.globe
        property: attachment
        keys: [ [800,"hand.right"], [2000,"none"] ]

**Easy studio:** timeline blocks Idle → Look → Reach → Hold → Release → Return, target-picker, curve presets, local play/seek, Preview, Save As Clip, and versioned undo. For accessibility, everything is keyboard-reachable and has reduced-motion equivalent.

**Power-user studio:** multiple animation layers, blend masks, full bone/facial morph and F-curves, inverse kinematics targets, retargeting, conditional finite state machines driven by **host supplied and validated read-only events**, events/props/attachments, and isolated import/export. If programmable procedural graph nodes are later required, explore deterministic fuel-bounded WASM with **no WASI, filesystem, network, clock or host tool access**; do not enable before independent hostile validation. Never use arbitrary runtime eval.

All names are resolved against the currently approved scene revision; unknown bone/prop/clip/cycle or stale revision fails closed. Script-only fake “task succeeded” markers cannot alter tool/evidence statuses. An animation cannot cover host-critical security/egress/stop UI.

## 5. Props as a scene-local causal graph

Model prop interaction as:

    Select → Reach → Contact → Attach → Manipulate → Release

Start with finite synthetic constraint solving, object-local pivots and deterministic attachment frames, not a heavyweight game-physics dependency. Add optional collision/contact/physics only after calibration and bounded CPU budget. Objects may be independent interactive scene nodes with typed UI actions. **Scene-local contact is not real-world contact**; pushing a virtual button is not sending a shell command.

## 6. Rendering, performance and integration

**Native future:** skinned mesh pipelines, morph targets, PBR/UV textures, camera perspectives, facial close-up quality, object buffers, realistic alpha/veil, retained GPU cache, LOD, offline baked asset loading. Keep the canonical source atlas exact and private in the consumer. No direct LibGibson photos.

**Headless first:** deterministic CPU geometry/transform/collision oracle with synthetic meshes/controls. Optionally software raster small frames; validate mathematical behavior independently of GPU style.

**Terminal path:** use approved 2D assets or explicitly baked, approved views. A half-block projection is a **semantic fallback**, not a claim of native-mesh pixel parity. Preserve existing renderer and scrollback contract, Rust MSRV and public SDK policy.

**Laptop resource envelope:** test integrated GPU RAM sharing with CPU-bound inference on the actual consumer machine; aim for settle/on-change behavior, static idle scenes, finite animations, and graceful LOD degradation. Never claim 60 fps, cloth simulation throughput or negligible contention without measured receipts.

## 7. Dependencies, probes, and acceptance gates

| Phase | Deliverable | Required distinguishing probe |
| --- | --- | --- |
| PUP-0 | Exact API, stability, dependency/ABI and v0.9/Gibson World source audit | Show camera rig is NOT an actor rig; do not conflate RED draft with released |
| PUP-1 | Two-joint synthetic rig with CPU skinning/neutral geometry digests | Cycle/NaN/unbound weight/invalid inverse bind mutants fail |
| PUP-2 | 8 coupled named morphology controls + Jacobian sensitivity | Two-angle coherent shape; independent projected sprite-warp mutant fails; undo digest stable |
| PUP-3 | Closed typed finite clip grammar + parser/limits | Unknown target, infinite loops, huge memory, host-command keys fail |
| PUP-4 | Face/gaze/clip layers and deterministic seek/blend | Same-time CPU pose identity and finite settle; reduced motion static |
| PUP-5 | IK + prop contact/attach/release | No hand-target mismatch, bone/prop identifier alias, or false OS action |
| PUP-6 | Safe GLB loader and optional VRM semantics | Synthetic round trip plus malicious buffers/URIs and license check |
| PUP-7 | Gibson World native skinned presentation | Needs a REAL upstream native renderer accepted, not #439 alone |
| PUP-8 | Public API and consumer qualification | Exact head, terminal regression, physical GPU + pressure + independent holdouts |

**Controls:** calibrated positive, negative, null and mutation fixtures for each phase. Do not set tolerances by looking at the final test case. No private avatar artwork in public test fixtures. A screenshot that resembles the input does not certify 3D reconstruction accuracy or closed-form geometry proof.

**Claim ledger:** OBSERVED in code = camera rig, software raster3D, Scene/Stage and semantic Experience. CONJECTURED = proposed interoperable puppet stack; UNVERIFIED = new GPU skinning, morph/collisions, Blender/VRM import, exact consumer pixel performance and GUI scripting. All real host or camera capabilities are outside this repository.

## 8. Explicit handoff

The public engine owns **rig math, format adapters, scene/object semantics, clip execution, constraints, generic shape controls, rendering, resource safety and test oracles**. The private consuming application owns **reference-image acquisition, reconstruction hypotheses, identity/body/face revisions, artistic decisions, approved assets, slider defaults and authoring UX**. Any specialized model/gateway experiment must preserve its own inferencer and resource policies.

**Next actionable engineering probe:** headless two-joint skinning + one morph with a synthetic declarative clip, and a signed-geometry two-camera comparison under a slider perturbation. Once those invariants are proved, the native GPU bridge and a real consumer creator can follow. Nothing about this RFC changes releases or grants execution authority.
