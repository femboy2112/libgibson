# GIBSON WORLD — Puppet Runtime Agent Handoff

**Class:** R0 engine design, no code or release changed. **Baseline:** main 793f6ee4a167e0236f848b4495f1be83e95bdc29.  
**Primary:** [generic Puppet Runtime RFC](../docs/GIBSON_WORLD_PUPPET_RUNTIME_RFC.md).  
**Parent:** [Gibson World 2.0 draft PR #439](https://github.com/femboy2112/libgibson/pull/439); not merged, native GPU absent.

## Owner's new goal

Make generic 3D geometry puppet-ready: articulated body, face expression/gaze, RPG morphology sliders and scriptable interactions with scene objects. The consumer may fit imperfect/single-view images privately, but LibGibson owns **only generic rig/clip/mesh math and playback**, not the model/photo source or identity approvals.

## Immediate exact-head work

1. **PUP-0 inventory:** exact main SHA, current camera-only src/rig.rs, software raster3d, Scene/Stage/Experience, Release Contract, v0.9 native/kitty drafts; identify interface shared with #439, no unreviewed merges or code copies from private consumer.
2. **PUP-1 calibrated synthetic actor:** define neutral mesh, two-joint bone DAG, inverse bind matrices, bounded skin weights, deterministic CPU deformation. Prove known rest pose, rotated pose and undo.
3. **PUP-2 shape controls:** eight semantic axes with **one neutral geometry model**, explicit parameter coupling/Jacobian tests, two-camera comparison, no independent projected sprite stretching.
4. **PUP-3 strict clip grammar:** finite explicit-time channels, validated rig/prop IDs, controlled curve interpolation, no eval/file/network/OS tools. Reject malicious/unknown/unbounded scripts.
5. **PUP-4/PUP-5:** small face morph/gaze, two-clip blending, IK reach, prop attach/release, and deterministic scene-local action maps.
6. **PUP-6/PUP-7:** qualified GLB importer/optional VRM semantics, then native GPU path only when #439's actual source components are available. Keep terminal half-block static/baked-asset fallback.
7. **PUP-8:** SDK/release gate: only after independent positive/negative/null/mutant/holdout evidence, exact-head downstream review, ABI/MSRV/toolchain comparison and owner release decision.

## Falsifiers and hard boundaries

- Camera rig alone must NOT be claimed as character rig.
- Changed neutral mesh hash or invalid bone weights MUST refuse. Source-image hash authenticates image bytes only, never identity or hidden shape.
- Wrong detached view/clip cannot generate an authorized host action; a prop contact is **scene-local**, not desktop input.
- A “face blendshape” without authored geometry is not source data. No private avatar photos, textures, annotations or hardware IDs in the public engine or fixtures.
- Shape deformation that only matches one camera while changing geometry inconsistently across others is a **failed** coupling/path.
- OnChange static idle and reduced-motion semantic parity matter; CPU/VRAM/thermal no-free-GPU assumptions.

**Handoff output from an engineering agent:** exact base/head, changed files, source claims, test commands and exit statuses, controls and synthetic fixture digest, performance budget, unresolved conditions, privacy scan, next verdict-changing probe. Reconcile all open v0.9 and Gibson World branches before proposing integration.

**No implementation/probe results are claimed by this R0 handoff.**
