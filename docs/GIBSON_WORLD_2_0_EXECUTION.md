# GIBSON WORLD 2.0 — Implementation Program and Acceptance Instruments

**Classification:** R0 execution plan only; **no implementation authorized by this file**, no existing test outcomes asserted.  
**Companion contract:** [GIBSON_WORLD_2_0_SPEC.md](GIBSON_WORLD_2_0_SPEC.md).  
**Baseline checked by architecture author:** main 9e3f9cf55b027bcec24ca9128d69a9b335e6be1f (2026-11-19 local). First engineering worker MUST refresh exact head and open v0.9 PRs; do not transplant historical assumptions.

## 0. The forcing slice

Deliver the smallest **real native GUI application** using LibGibson semantics that:

1. Presents a synthetic application world (three destination blades, a text conversation, one source-labelled inspection instrument, one protected host-owned decision region and a synthetic RGBA character) through the **existing terminal backend**.
2. Presents the same semantic snapshot through a **native window**, using pixel-accurate source textures and readable text.
3. Switches destination with keyboard or pointer while preserving action identity, logical selection, source and a full audit receipt. A transient GUI focus event does not alter underlying host state.
4. Resizes across DPI and desktop-window lifecycles without losing source data, causing input action duplication, or starving an external agent's cancel/stop lane.
5. Has no microphone, camera, desktop capture, other-app input or background service. Everything is synthetic and permission-free.
6. Produces tests and screenshots that are explicitly labelled **synthetic/projection-only**. A green rendering fixture cannot certify any real host tool effects.

The native GUI is not built by rendering an ANSI cell screenshot into a larger window. It operates from a shared pre-cell-resolution source and display-list representation.

## 1. Branch and ownership choreography

| Lane | Owns | Do not touch | Blocking dependency |
| --- | --- | --- | --- |
| **A — Formal source audit** | Reconstruct current UI/Stage/Scene interfaces, versions, v0.9 PR statuses, target Linux window system/toolchain, current tests | Code/runtime; consumer-private facts | None |
| **B — Pixel seam** | Generic retained semantic Scene/DisplayList, coordinate namespaces, synthetic losslessness oracle, hardware-independent headless frame | Terminal renderer/session/ANSI; host authority | A |
| **C — Native shell** | Native window, DPI/resize/input/IME, offscreen software mode, GPU candidate isolated by feature or crate | Core MSRV/ABI by default; general application state | A, B contract |
| **D — Grammar bridge** | One real Experience grammar into native projection; view/hit/action receipt parity; pointer semantics | Model, tool dispatch, permissions | B, C |
| **E — RGB and image** | RGBA texture upload/composition, color/alpha/dirty regions, postprocessing opt-in; approved-like synthetic atlas fixture | Private avatar art; runtime redraw of unknown source | B, C |
| **F — Multi-view** | Two windows reading same immutable snapshot, view-local focus/camera, safe reopen/device loss | Project daemon lifecycle | C, D, E |
| **G — Adversarial reviewer** | Mutation/holdout tests, X11/Wayland probes, CPU pressure and accessibility matrix, clean SDK consumer | Authority changes, self-approval | Every lane continuously |
| **H — SDK/release** | Version/API tiering, downstream public consumer receipts, release notes/decisions on explicit approval | Unapproved tag, force-push, release | B–G accepted |

No two workers edit the same mutable path concurrently. Shared dependencies are read-only; staging uses isolated worktrees and dedicated Cargo target directories.

**Existing draft interference:** PHOSPHOR #419, view-transition #434, Kitty #438 and pending v0.9 interface changes are independent candidate branches. Their RED tests must not be treated as regressions or as shipped success. Prefer cherry-free integration through the original owners after exact-head comparison and holdout tests.

## 2. Pre-registered milestone gates

### GW0 — Extract a reproducible baseline (probe-gap closure)

**Inputs:** exact remote SHA, AGENTS/README/ROADMAP/Release Contract, Experience/Stage docs, compiled example, current v0.9 candidate PR heads. Check whether newer code already provides a portable pixel seam or pointer extension.

**Tests to run locally (when host/build accessible):** existing fmt, clippy, unit/integration, headless Stage fixture, Experience lab settled frame for truecolor/mono, and one external SDK consumer baseline. Record compilers, profile, target, feature set, test counts and failing cases. Do not claim a pass until run.

**Falsifier:** if the smallest consumer can already get full native-source pixels and typed GUI hit semantics through a public API, cut this program down to only the missing window/backend adapters. Stop speculative refactoring.

**Deliverable:** status ledger with actual file:line references, linker/dependency size, MSRV, selected renderer alternative(s), and explicit integration boundary. No code changed.

### GW1 — The prequantization seam

**Design objective:** preserve original RGB/RGBA and logical text/vector/mesh into one intermediate representation.

**Minimal synthetic fixture:**
- 16×16 RGBA pattern with independently varying RGB/alpha at every pixel (including fully transparent nonzero RGB) and an intentional 1-pixel stripe.
- two semantic buttons under a partially translucent overlay;
- a Unicode text run with wide/combining graphemes and a right-to-left sample;
- a keyed, offset raster plane under a clip and modal plane.

**Pass:** software reference compositor yields the exact documented alpha/color result; texture dimensions are preserved; terminal projection demonstrably loses information in its expected 1×2 reduction without contaminating the native source. Distinguish **source quality** from **physical monitor pixels**.

**Mutation controls:** force Stage's 1×2-downsample before native projection; swap clip order; alias two keys; erase alpha. All must fail at least one exact oracle.

**API constraints:** if old consumers can still compile and get byte-identical terminal frames, prefer additive experimental adapters. If no viable adapter exists, propose a controlled breaking change with release notes, never silently modify CORE.

### GW2 — A first genuine native window

**Design objective:** own exactly one native window/event queue, deliver an independently rendered frame.

**Test matrix:** empty/zero-size, 640×480, 1280×720, HiDPI fractional scaling, multi-monitor move, minimize/restore, close/reopen, alt-tab, keyboard-only, pointer, cursor/focus, clipboard/IME. Test headless software capture separately from physical OS behavior. Where Wayland session is unavailable, mark that cell UNVERIFIED and do not fill it with an Xvfb proxy.

**Pass:** semantic frame and input still work without a GPU; GPU device-loss is distinguished from process/task failure; host can always close/stop the native view. No GPU packages in default terminal-only SDK unless qualified.

**Negative:** kill compositor, lose GPU device, generate synthetic 30 Hz resize storm, deliver stale input after view generation increments. No action duplication, infinite queue or panic affecting another view.

### GW3 — Typed key/pointer equivalence

**Design objective:** one semantic action space for both terminal and native projections.

**Pass:** selected element, source identity, refusal/unknown states, disabled states, focus, modal and dismissal semantics agree at settled states. Verify action IDs and trusted host-route decisions, not just screenshots.

**Negative:** forged clickable pixels under a host-owned modal, high-z element covering protected region, click at stale position after resize, drag release after popout destroyed, synthetic “verified” decoration. A model-origin node has no right to create authorization.

**Mutation:** route pointer directly to host execution instead of typed action parser; test must reject via policy fixture.

### GW4 — Game-menu grammars, not dashboard skins

**Target:** physical **BLADES** and **MEDIA_SHELF** through native perspective geometry, with a non-media fixture; optionally CROSS_MEDIA/ORBITAL as stretch. Interaction responds to spatial position and keyboard without a second per-grammar application implementation.

**Pass:** grammar switch preserves active selection and required actions; layout supports a readable long conversation, contextual inspector, font size changes and reduced motion. An explicit finite settling deadline causes zero animation work thereafter; nonstop ambient effects are opt-in with measured resource use.

**Hostile:** 40×15-equivalent tiny semantic viewport, monochrome/text, oversized text, 10k virtualized entries, focus conflict and state revision race. Preserve essential controls or refuse with a labelled reason.

### GW5 — Window as camera, two concurrent projections

**Target:** main Citadel-style window + small inspector window reading identical immutable model snapshots, with independent camera and focus.

**Pass:** close/reopen either window does not corrupt other window's focus, session, queue, source labels or selection; resubscription uses revision/epoch checks. Closing last window does not by itself grant permissions or create persistence—the *consumer* defines daemon lifespan.

**Negative:** stale frame arrives after window closed, pointer ID is recycled, popup attempts to obscure protected host controls, GPU cache retains revoked source.

### GW6 — Hardware pressure, software fallback, and honest receipt

**Measure:** input-to-present latency; full-frame and damaged-region time distributions; memory RSS/VRAM; time spent in uploads/shaders/shaping; swap faults; rendering demand at idle; external task/stop p95/p99. Compare:
- terminal baseline;
- native CPU/software frame;
- native GPU frame;
- high-load external CPU consumer with GUI active;
- reduced motion / static mode;
- device loss / denied capability.

Calibrate a null control, negative mutation (busy frame loop), positive motion control and fresh holdout. Predeclare thresholds using baseline evidence, not aesthetics. Explicitly record UNKNOWN GPU hardware/OS support if unavailable.

**Pass:** selected target thresholds, no unfounded claims of 60 fps or zero CPU. If graphics fails under pressure, degrade fidelity in order: ambient FX → visual transition cadence → raster resolution/cached effects → minimal native text, preserving interactive/stop controls.

### GW7 — Public API/consumer qualification

Only after the preceding gates:
1. Freeze experimental API surface with explicit version/feature gates.
2. Run public Rust consumer fixture; evaluate the C/C++/Python/Go wrapper reach honestly. No ABI expansion promised until requested.
3. Run terminal golden and PTY recovery matrix including scrollback/anchor invariants.
4. Run independently authored hostile validation against exact candidate and separate baseline.
5. Publish changelog/release notes solely under Release Contract with owner approval.

**Not required:** consumer-specific avatar rig, workstation daemon, desktop automation authority, gaze-from-webcam or local model.

## 3. Rendering-quality budget and identity tests

Native GUI visual goals are *real constraints*:
- text readable and selectable at normal window sizes; mixed Unicode, emphasis and code not drawn as blurry image rectangles;
- source RGBA at correct aspect and alpha, even when the terminal projection has fewer bits/pixels;
- physically convincing perspective movement without sacrificing hit accuracy;
- per-view precision of keyboard/pointer input after transform, clip and DPI;
- shaders/effects subordinate to contrast, semantic status and accessibility;
- no regeneration of privately approved character art inside this public library.

Use **separate** quantitative and operator-quality acceptance: screenshot reference pixels and hit-map proofs can pass while the cinematic “feels like a game menu” judgment fails; record this as an outstanding visual-acceptance gate, not a software proof.

## 4. Shared specification boundary with consumers

The consuming host—not LibGibson—defines:
- task/session/process residency and whether a UI window may detach;
- the meaning of “the host machine is the character's body”;
- sensors, desktop automation, camera/mic, files, egress, model selection and scopes;
- provenance, tool effects, journal and stop/revoke;
- approved character sources, identity, expressions and new rigs;
- accessible host-critical disclosures and independent stop.

In this engine, a custom View can show **world state supplied by the host** and send typed **proposals** back. It must never observe the host, execute a command, or claim a verified effect on its own.

The first reference consumer is a **synthetic** MINERVA-like facade with no private asset, provider or permission. A real consumer integrates through its own independently gated branch after this upstream contract is accepted.

## 5. Commit, PR, and provenance discipline

**Every lane** submits: exact baseline/head SHA; paths changed; objective; candidate interface and migration; feature status (source-only vs implemented vs tested vs physical verified); test commands + exit/results; negative/mutation controls; recognized residuals; CPU/RAM envelope; no known private data; issue cross-links; next discriminator. Mark unrun tests **UNVERIFIED**.

**No merges or version tags from this dossier alone.** A worker may build a branch and propose a PR under the engine's own rules; the relevant release/integration coordinator decides promotion. Do not overwrite, coerce or force-push sibling branches.

## 6. Coordinator's next turn

1. Read the [architecture spec](GIBSON_WORLD_2_0_SPEC.md), [current state](STATE_OF_LIBGIBSON.md), [Stage](STAGE.md), [Experience](EXPERIENCE_GRAMMARS.md), and v0.9 #419/#434/#438.
2. Assign A (read-only inventory) and B (synthetic source-preserving renderer seam) first; C (window backend spike) can start with a frozen B draft protocol, but no premature SDK change.
3. Publish one exact native-window proof before designing 3D actors or desktop overlays.
4. Ask real consumer maintainers for their tests only after upstream API stabilizes.
5. Continue v0.9 and current release plans independently: Gibson World is a **parallel program**, not a branch pile dumped into main.

**Deliverable of this R0 document:** a testable execution contract and dependency order. No native implementation, benchmark or visual sign-off claimed.
