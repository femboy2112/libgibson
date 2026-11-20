# GIBSON WORLD — LibGibson 2.0 Architectural Specification

**Status:** PROPOSED / R0 documentation. No native-window backend, GPU backend, desktop-control authority, avatar rig, or 2.0 release is implemented or enabled by this RFC.  
**Owner direction:** 2026-11-19, “Gibson World”; build game-grade real GUI windows from LibGibson's semantic/Stage foundations.  
**Engineering baseline:** LibGibson main at 9e3f9cf55b027bcec24ca9128d69a9b335e6be1f; v0.8.1 released, v0.9 feature work in draft/RED branches.  
**Publication:** This repository is public. Reference consumers by public architectural contracts only; **no private art, consumer code, paths, model prompts, host identities, telemetry or credentials** enter this document.

> **Thesis:** A window is a viewport onto a world; a world is not a window. LibGibson should let applications declare semantically meaningful entities, interactions and visual experiences once, then realize them as terminal-cell, high-resolution terminal, native-window or offscreen projections without falsely promoting presentation into host authority.

## 0. Product objective and acceptance

The target is an **application-neutral, cinematic GUI engine** in the *Hack the Planet / game-menu* lineage: perspective blades, Cover Flow, orbital worlds, gorgeous typography, dynamic depth, camera movement, lighting, transparent layers and responsive instruments. It must be just as useful for serious work as for demonstration films.

An external agent application named MINERVA is the forcing consumer, but LibGibson must not contain agent permissions, inference, personas, private avatar assets or host-state policy. The consumer's **process and machine-substrate ownership** are outside this engine: LibGibson is a renderer/presenter, never a privileged desktop agent.

**Pass when:** one host-approved semantic experience can be rendered in (a) existing terminal output and (b) a real native desktop window; equivalent essential facts/actions remain available; source assets stay high resolution in the GUI; interactive focus, pointer, time, DPI, resize, disconnect, device failure and fallback are deterministic or accurately diagnosed. The old terminal path and scrollback guarantees continue.

This is a **research/program name**, not permission to set Cargo's package version to 2.0.0. The existing Release Contract governs versions, MSRV, CORE/API promotion, native SDK and Go/Python/C++ compatibility. The active v0.9 track remains independent.

## 1. Existing seams: reuse, do not fork

Audited from the specified main baseline:

| Existing API / file | Valuable invariant | 2.0 work |
| --- | --- | --- |
| src/ui/experience/; docs/EXPERIENCE_GRAMMARS.md | Semantic application / presentation grammar split; six experiences (STANDARD, MEDIA_SHELF, CROSS_MEDIA, PANORAMA, ORBITAL, BLADES) | Extend grammar output to native layouts; never fork application meaning per renderer |
| src/ui/{element,compile,runtime,interaction}.rs | Keyed semantic tree, typed events, focus, modal handling | Introduce device-independent presentation/hit geometry without inventing a second action router |
| src/stage/{driver,raster_plane}.rs; docs/STAGE.md | Resolved keyed planes, explicit-time frame snapshots, bounded resource limits | Preserve high-resolution raster sources until backend projection; move terminal cell conversion to the terminal adapter |
| src/scene.rs, src/rig.rs, src/transition.rs | Entity/effect algebra and caller-controlled time | Reuse identities, effect laws and camera trajectories at pixel resolution |
| src/raster.rs, src/raster3d.rs | RGBA compositing and software 3D raster | Reference oracle + software/fallback backend, not the only GUI image representation |
| src/temporal.rs; PHOSPHOR draft PR #419 | Temporal perceptual fidelity tools | Terminal projection only; do not claim eye/CRT effects create nonexistent spatial information |
| docs/KITTY_IMAGES_DESIGN.md; draft PR #438 | Proposed terminal high-resolution image transport | Qualify as another terminal backend; not a replacement for native desktop composition |
| draft PR #434 | Proposed SurfaceFx transition semantics | Coordinate with native transitions; RED stubs are not shipped implementations |
| src/context.rs, src/renderer.rs, src/session.rs | One terminal owner; true scrollback / live-region history; counted writes and recovery | Preserve exactly; do not make native GUI own the terminal or change its physical anchor contract |

**Critical present limitation:** Stage presently resolves an RGB image at **one pixel wide by two pixels tall per terminal cell** before converting to a Surface. If the GUI samples this derived Surface, lost source frequencies are unrecoverable. A native backend must start from a pre-quantization scene/pixel source. This is a pathway gap, not a demand to rewrite Stage all at once.

**Other known gaps:** Experience grammars' input interpretation is currently keyboard-oriented; pointer/spatial interpretation requires a deliberate extension. GPU/window/Wayland support is not in the current released LibGibson. Draft v0.9 PRs require exact merged-head verification, not citation as shipped.

## 2. Governing architectural laws

**GW-01 — Meaning before appearance.** The application supplies semantic identity, content, permitted interactions, provenance/meaning labels and stable keys; a grammar chooses spatial composition; a backend chooses how pixels/cells are realized. Skins provide style and motion tokens, not application authority.

**GW-02 — Window is a camera.** Native windows are independent projections of one immutable semantic snapshot, potentially with independent viewport, focus, transform, theme and visibility. Window creation/destruction never creates/destroys the underlying application state by implication.

**GW-03 — One host action route.** A rendered hit produces a typed **intent** to the application host. Neither shaders, window-system callbacks nor animation can grant or execute privileged actions. The host's action routing and permission ceremony are external and authoritative.

**GW-04 — Essential semantic preservation.** Let S be one authorized semantic snapshot, T the terminal projection, G the GUI projection and E the extractor for essential observable facts/actions. At settled states, require

    E(T(S)) = E(G(S)) = E(S).

Visual equality is NOT required. Capacity-limited views may *explicitly report* legitimate omissions, but must not omit required controls/status, counterfeit verified outcomes, or claim unknown values are zero. The equality is a testable design law, not a proven theorem for unbuilt backends.

**GW-05 — Diagram for actions commutes.** If a key and pointer hit represent the same semantic control, their routes must select the same application action identifier after host validation. Hit testing is presentation-dependent; authorization is not. Raw pointer coordinates must never be trusted as a privileged target.

**GW-06 — Real-pixel source integrity.** Decode bounded, authenticated image/mesh/font inputs before projection. Preserve source dimensions and alpha in the native path. Terminal conversion is a last-mile reduction with explicit quality capability.

**GW-07 — Deterministic time.** Presentation is a function of snapshot, validated scene, renderer capability and explicit monotonic presentation time. Animation does not read hidden wall clocks or generate world events. OnChange and settle deadlines permit idle zero-work frames.

**GW-08 — Focus/authority precedence.** Independent emergency stop / host critical notices / authorization and cloud-egress disclosures / operator focus and cancel / selected instruments / optional scene directives. Renderer-origin decorative layers must not obscure or impersonate protected UI; actual enforcement belongs to the host.

**GW-09 — Device failure is not authority failure.** On GPU loss, scene rejection, enormous allocation, missing asset, compositor disconnect or render panic: preserve the application task and host IPC; render a safe software/terminal/text fallback with explicit status. No silent 3D-to-“verified” status transition.

**GW-10 — Opt-in platform capabilities.** Rendering a window is not observing other applications, injecting pointer events into them, capturing screen/audio/camera, changing system settings or installing an agent service. Each requires independent policy in its owning host.

**GW-11 — Accessibility parity.** Keyboard-only route, accessible labels/semantic navigation, focus visibility, reduced motion, contrast and legible text do not depend on beautiful shader output. No color-only truth marking. A terminal/text projection remains viable.

**GW-12 — Honest resource isolation.** GPU work, VRAM, CPU upload, text shaping, shaders and idle ambient effects must be budgeted; external consumers may run heavy inference simultaneously. Host responsiveness and stop/cancel outrank visual fidelity.

## 3. Proposed layer model

    Application host: immutable state snapshot + permissioned actions
                       |
             Semantic Experience
             (destinations / items / events / essential controls)
                       |
               Grammar / Skin
        (camera, layout policy, style, motion, omissions)
                       |
                 WorldSnapshot
        (stable entities, transforms, clips, z, typed hit IDs,
          text runs, vector geometry, raster/mesh references,
          asset provenance, animation deadlines)
                       |
             Backend-independent DisplayList
                       |
           +-----------+-----------+---------------+
           |                       |               |
       Terminal cells         Native pixels     Offscreen oracle
       ANSI + glyph ladder    window/GPU        deterministic image
       optional Kitty pixels  software fallback  & hit-map capture

The **WorldSnapshot/DisplayList boundary is a design target**, not a commitment to replace every existing Element/Node. Start with a small adapter that lowers today's compiled semantic subtree and Stage keyed raster planes to a **native-ready pixel list**, leaving terminal Context/Renderer untouched. Extend component types only when a forcing slice proves a missing invariant.

**Renderer core contract (illustrative, not accepted Rust API):**

    SemanticState -> Experience::present -> PresentedScene
    PresentedScene + ViewEnvironment + Clock -> ResolvedWorld
    ResolvedWorld + TargetCapabilities -> FrameOutput
    PointerOrKey + FrameHitMap -> TypedSemanticAction
    TypedSemanticAction -> ApplicationHost (authorization/dispatch)

Key design types to prove in prototypes before promotion:
- SceneKey, EntityId, WindowViewId: distinct namespaces; no accidental tag collisions or recycled-key aliasing.
- LogicalRect, PixelRect, TextureSize, CellRect: explicit coordinate/newtype conversions; DPI scaling and transforms documented.
- ViewEnvironment: logical size, physical framebuffer size, scaling factor, capability flags, motion preference, accessibility contract.
- ResolvedEntity: key, local/world transforms, clip stack, z-order, optional text/raster/mesh, provenance/meaning label, hit tag.
- PresentationReceipt: essential IDs actually presented, declared omissions, fallback reason, focus, resource cost, next frame demand.
- RenderOutcome: Presented, Degraded, Rejected, DeviceLost, Suspended, with fault/source annotation.

The renderer may expose host-neutral **semantic status slots** but may not fabricate the contents. The output/receipt says what was painted, not that a world effect occurred.

## 4. Native GUI renderer and windows

### 4.1 Target candidate (unfrozen)

Evaluate **winit + wgpu** first for cross-platform native window/event lifecycle and a GPU surface. An alternative toolkit is admissible if it wins a preregistered gate on Linux desktop correctness, input/IME, resource cost, and embedding. Keep actual crates/versions pinned per spike and compatible with the chosen workspace MSRV; libgibson currently declares MSRV 1.85. Do not raise it globally merely to add an optional graphical backend.

Proposed boundaries:
- **gibson-world-core** (working name): reusable pre-quantization scene/display-list contracts and deterministic software oracle; no window dependency.
- **gibson-world-native** (working name): optional winit/wgpu desktop backend, one compositor owner, window lifecycle; possibly independent workspace crate if necessary for MSRV/SDK isolation.
- **gibson** existing terminal crate: exports adaptors without changing terminal lifecycle/ABI unless qualified separately.
- No new core dependency on desktop compositor, graphics device, system libraries, model runtime or private consumer code.

Do not decide final crate boundaries before building the smallest first window; compare the thin feature-gate vs separate crate on compile footprint and API reach.

### 4.2 Retained native frame

Prefer logical scene graphs with dirty-region invalidation and cached assets, NOT re-rasterizing all glyphs or all mesh data every UI tick. Shape text correctly (Unicode, bidi, ligatures, grapheme/selection/caret/IME), retain a glyph atlas, composite RGBA in a consistent color space, and keep an explicit layer/z/clip/hit ordering. Display windows can have different DPI and aspect ratio while reading the same semantic snapshot.

The terminal's immutable scrollback has **no direct native equivalent**. The semantic transcript/journal belongs to the application; the native view supplies virtualized scroll backed by that source. Never couple GUI scrolling with terminal ANSI history or claim both have identical physical history.

### 4.3 Multiple windows as projections

A main window, inspector popout, graphics lab and (later) a transparent companion window may project the same world. Each owns a ViewId and per-view layout/focus/camera; all subscribe to immutable state revisions. View-specific animation and pointer position must not contaminate shared source state.

For a transparent native companion, prove both compositor behavior and hit-region semantics on the actual X11/Wayland target. A transparent overlay is **not** an OS-global click-through/focus/capture privilege. Do not promise always-on-top, unrestricted positioning, global pointer observation or arbitrary other-app input under Wayland.

### 4.4 Input contract

One input route from the window event source through spatial hit testing/capture-target-bubble/modal occlusion to typed host actions. Explicitly handle focus transfer, drag capture release when a window closes, click-through only on qualified platform paths, keyboard navigation, IME, gesture, resizes, monitor hot-plug, clipboard/accessibility and pointer scaling. Model-authored visual content may request **local UI manipulations** only after host validation.

A native "Approve" button is not an authorization shortcut. The host retains the exact attended consent ceremony.

## 5. Cinematic grammar without a widget prison

The six existing Experience grammars are the semantic starting vocabulary, NOT a mandate that every application become a list, fixed dashboard or single house skin. Preserve custom-content escape hatches and user-authored scenes.

**Reference product grammars to explore:**
- **Citadel:** physical depth-plane blades, perspective navigation, large restful conversation and living presence anchor; a host-controlled protected decision band.
- **Specter (future):** a minimal desktop companion projection of the same world, with deliberate focus and consent boundaries.
- **Observatory:** source-and-claim maps, activity/effect graphs, timeline, geometry/instruments, explicit uncertainty.
- **Workbench:** manipulation of actual scene entities, plotting, simulation and readable properties, not only ornaments.
- **Cinema:** precise deterministic camera/transition sequences with semantic fixtures; no fabricated actions.

These are product examples; only generic components enter LibGibson. Semantic recipes and palettes may be packaged as optional skins/grammars.

**Experience acceptance:** a non-media operations application must remain usable through all grammars; no hidden dependence on cover art, 3D models or color. A tiny window with restricted permissions must still disclose its essential state and allow a safe exit, or refuse the action with an explicit reason.

## 6. Generic avatar/character asset seam

LibGibson MAY own generic Texture, SpriteAtlas, BoneHierarchy, Mesh, Material, AnimationClip, ExpressionChannel and rig/pose/camera playback contracts once qualified. It must NOT own a specific character identity, body geometry, clothing policy, emotional state, consent source or inference logic.

Three independent visual tiers:
1. **Approved RGBA 2D art in a real GUI**: original texture dimensions, alpha, correct color and centering; no character animation fabricated.
2. **Approved 2.5D rig**: separate eyes/eyelids, head/hair/veil and bounded pose layers with explicit authoring/provenance. All deformation/landmarks need independently approved assets.
3. **Approved 3D rig**: textured geometry, bones, facial blendshapes, authored motion clips and lighting. Never reconstruct unseen source geometry as if it were authenticated from a few flat crops.

Asset revocation invalidates cached GPU resources; missing/malformed/unapproved source yields a labeled fallback. Exact asset approval belongs to the application. Model-generated imagery may be useful for **draft art** only if separately approved, not as runtime authoritative substitution.

## 7. Shaders, optical effects and performance

Use GPU capabilities for **true** pixels, subpixel transforms, gradients, transparency, glow, lighting, postprocessing and 3D; the goal is game-menu coherence with a readable working application. Scanning/CRT/phosphor/temporal tricks may be opt-in *presentation treatments* and must never corrupt evidence labels, text legibility or source-asset identity.

Do not claim perceptual super-resolution without calibrated measurements. Temporal dithering/hue pulses are constrained by flicker/accessibility, screen cadence and CPU/GPU resource budgets. A settled screen should not animate or spend power merely to appear alive unless a user explicitly selects an ambient mode.

The first hardware gate samples **frame time, input-to-present latency, p95/p99 host stop path, peak RSS/VRAM, CPU utilization, swap faults and lost/late frames** under idle/streaming/heavy-inference/swap-pressure regimes. Freeze thresholds *before* reading candidate results and compare terminal, CPU GUI and GPU GUI. No invented 60-fps success claims.

## 8. Falsifiable qualification matrix

| Probe | Positive | Negative / mutation | Pass / failure |
| --- | --- | --- | --- |
| Semantic preservation | Same snapshot → terminal & GUI essentials | Remove a required status/action in either projection | Missing essential is detected, not silently omitted |
| Input transport | Key / pointer hit same semantic control | Shift hit-map, close window during drag, inject stale revision | Same authorized action or a typed rejection; no privileged synthetic click |
| Fidelity | Synthetic lossless RGBA checker + alpha | Force old 1×2-cell pre-quantization | Native source detail survives; mutation is distinguishable |
| Frame time | Fixed state/clock snapshot twice | Inject hidden wall clock or nonsettling animation | Deterministic settled frame; demand goes OnChange |
| Resources | Heavy inference + window resize + streaming | Saturate CPU/allocations, zero-size window, invalid textures | Bound memory, keep input/stop responsive, explicit degradation |
| Window lifecycle | Open, close, suspend, reopen, DPI move | GPU loss, compositor disconnect, destroyed view | Source task persists; no stale focus; software/text fallback |
| Platform | Real X11 & Wayland target checks | Denied portal/overlay/capture features | Honest capability report, no privilege confusion |
| Avatar assets | Synthetic atlas + approval reference interface | Change source digest, proportions or cached texture after revocation | Reject/fallback; no silent identity substitution |
| Accessibility | Keyboard-only, mono/text, reduced motion | Remove color and animation | Essential action/status and reading order survive |
| Security | Host notices/modal over world scene | High-z model layer, spoofed verified badge, click-through to protected area | Host chrome/policy wins or request refused |
| ABI regression | Existing terminal SDK consumer & emulator fixtures | Force new desktop dependencies into default build | Terminal behavior/ABI/scrollback remain contract-correct |

**Calibration:** each test family needs a known-good, known-bad, null/no-op, and where useful mutation control. Keep exact SHA, compiler/deps, synthetic fixture hashes and raw results. GUI snapshots alone do not verify external host effects. An appealing demo is not a proof of authority or portability.

## 9. Work-package dependency graph

    W0 contract inventory and capability evidence
      ├─ W1 scene/display-list adapter (software oracle)
      │    ├─ W2 native first window + text/input
      │    ├─ W3 high-resolution RGBA/GPU composition
      │    └─ W4 semantic/input/receipt parity
      ├─ W5 grammar/camera transitions (after W2–W4)
      ├─ W6 multi-window projection + lifecycle (after W4)
      ├─ W7 synthetic character/demo facade (after W3–W5)
      └─ W8 platform/pressure/accessibility hostile matrix (each package continuous)
           └─ W9 qualified opt-in 2.0 track / SDK decision
    Consumer-native integration requires W2 + W3 + W4; no dependency
    on desktop-wide control, sensing, resident service or full 3D character rig.

**W0:** read exact main, upstream PR stacks (#419, #434, #438 and existing issue #127/#133/#138/#139/#140/#146), capability matrix, platform MSRV/build cost, and the consumer's public need; write a minimal pre-quantization forcing fixture.

**W1:** implement generic pixel-space compositional oracle with bounded DisplayList and keyed hit geometry; keep terminal emission identical to baseline.

**W2:** native Hello World window, true text shaping, input, DPI, lifecycle, software fallback; run x11/wayland capability matrix.

**W3:** texture/plane composition using synthetic assets at native resolution; prove alpha correctness and difference from terminal cell encoding.

**W4:** key/pointer action equivalence, focus and modal occlusion, essential-action/omission receipts, negative source spoofing cases.

**W5:** Experience grammar translation, finite Blades/Media Shelf camera motion, non-media functional holdout, user-visible rhythm; avoid permanent panel chrome.

**W6:** two simultaneous views of the same semantic snapshot; detach/reattach must not restart host action or leak focus across windows.

**W7:** generic source-approved-asset facade tests only. Character identity and full rig belong to the consumer.

**W8:** exact hardware input-pressure/accessibility tests (including model saturation) plus independent hostile implementation/review; no inference-side timing claims from this repo.

**W9:** decide whether API becomes experimental Rust crate/features, requires new C ABI, or remains separate SDK. No release/tag/MSRV change without Release Contract approval and qualified consumer gating.

Work packages are separate branches/PRs. A red-first test is not a green implementation; a prototype is not a released capability. Keep every consumer handoff source-safe and identify what remains unmeasured.

## 10. Explicit non-goals / boundaries

- **Not a replacement for Linux, X11, Wayland or the OS compositor.** An application can present beautiful windows while the OS remains the real kernel/resource/permission boundary.
- **Not a privileged desktop-agent toolkit.** Renderers cannot open files on behalf of models, click other apps, watch the screen, grant microphone/camera, or execute tools.
- **Not a rewrite of existing terminal/scrollback renderer.** Its physical-state recovery, lease and immutable history are retained.
- **Not a demand to pick a single UI skin.** Native designers may author interactive game-grade presentations using generic semantics.
- **Not a claim that the existing approved flat art is a 3D model.**
- **Not a v0.9 scope expansion or a 2.0.0 SemVer authorization.**
- **Not a guarantee of universal equivalence/identity.** Semantic parity applies to explicitly defined essential controls/facts and tested projections; unknown capability states remain unknown.

## 11. Claim ledger and next discriminators

| ID | Claim | State | Verdict-changing probe |
| --- | --- | --- | --- |
| GW-C1 | Current semantic grammars and Stage source seams exist | OBSERVED (repository source and docs on named baseline) | Compile/runtime API fixture on exact SHA |
| GW-C2 | Native pre-quantization projection is necessary for full source fidelity | DISCLOSED under 1×2-cell reduction model; implementation absent | Inject distinguishable high-frequency synthetic RGBA source through both routes |
| GW-C3 | Native window and terminal can preserve the same essentials | CONJECTURED / UNVERIFIED | W1–W4 two-renderer host fixture + mutation |
| GW-C4 | winit/wgpu wins the target hardware/toolchain envelope | UNVERIFIED | W0/W2 preregistered spike vs alternatives |
| GW-C5 | Multi-window projections can remain source/action consistent | UNVERIFIED | W6 stale-revision and focus race probes |
| GW-C6 | Transparent desktop projection is portable across compositors | UNVERIFIED / likely capability-conditional | X11/Wayland compositor matrix on owner target |
| GW-C7 | GPU/animation overhead can remain subordinate to CPU inference | UNVERIFIED | W8 matched heavy-inference and contention runs |
| GW-C8 | Full character embodiment follows from existing flat sprite atlas | REFUTED as an authenticated-content inference: missing rig/geometry layers | New independently approved 2.5D/3D asset and silhouette gate |

**Source-map caveat:** a GitHub code-read demonstrates current declarations, not runtime host qualification or independent implementation verification. The code path may move while other workers build v0.9; rebase intentionally and rerun the provenance/consumer matrix.

## 12. Coordination, privacy and release contract

This public RFC owns the **generic engine** and deliberately avoids private consumer implementation detail. Related application plans live in their owning private repository. Runtime autonomy, model source, telemetry, journaling, consent, overlay permissions and avatar approvals stay with that host. An inference gateway stays behind the host-provider boundary.

**Decision rule for any requested feature:** (a) if generic scene/presentation physics, implement here; (b) if host world/effect/permission or specific identity, implement in the consumer; (c) if inference/model control, implement in the inference service; (d) if OS/window-manager policy, report a platform capability and obey it.

### Existing references

- [Experience Grammars](EXPERIENCE_GRAMMARS.md), [UI layer](UI_LAYER.md), [Stage](STAGE.md), [Scene algebra laws](SCENE_ALGEBRA_LAWS.md).
- [Kitty terminal graphics design](KITTY_IMAGES_DESIGN.md), [LibGibson state](STATE_OF_LIBGIBSON.md), [Release Contract](RELEASE_CONTRACT.md).
- Public terminal perceptual fidelity [PHOSPHOR #419](https://github.com/femboy2112/libgibson/pull/419), native transition [draft #434](https://github.com/femboy2112/libgibson/pull/434), Kitty prototype [draft #438](https://github.com/femboy2112/libgibson/pull/438).
- [Gibson World implementation and test plan](GIBSON_WORLD_2_0_EXECUTION.md) and [agent handoff](../handoffs/GIBSON_WORLD_2_0_COORDINATOR_HANDOFF.md).

**Verdict:** Source-grounded R0 design. No code, runtime gate, performance benchmark, host-screen measurement or new permission is claimed.
