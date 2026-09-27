# LibGibson 0.3.0 development plan

**Branch:** `feat/v0.3-temporal-display-processor`

**Baseline:** released v0.2.5 plus post-release main. 0.3.0 is not yet a release candidate.

## What v0.2.x achieved

The Europa-shot consumer campaign produced issues #35-#48. The v0.2.1 through v0.2.5 patch train resolved those findings coherently without changing C ABI v1 or MSRV 1.85:

- 0.2.1: input correctness / initial UI motion / honest startup diagnostics.
- 0.2.2: safe finalized output, visible frame inspection, changed-cell observability and canonical loop documentation.
- 0.2.3: semantic percentage sizing, deferred post-layout `Node::canvas`, clipped Surface views, Braille compositing and `Glyph::from_char`.
- 0.2.4: viewport visibility math and dynamic focus membership.
- 0.2.5: large-collection guidance/example and remaining ergonomics/documentation cleanup.

This was a strong patch train. Do not reopen those issues merely to justify 0.3.0.

## Residual post-0.2.5 audit

Concrete new findings discovered while preparing 0.3.0:

- #61: `BrailleCanvas::paint_into` still used `Glyph::new(&ch.to_string())` even though 0.2.3 added `Glyph::from_char` to remove that temporary allocation. Branch fix is behavior-preserving.
- #62: `FrameScheduler` remains completion-relative, causing requested high frame cadence to drift by render/write time. Branch adds opt-in phase-locked pacing while preserving the default.
- #64: `last_render_duration_micros` combines generation and blocking terminal write. Branch adds an additive Rust-side `FrameReport`; ABI-v1 stats remain unchanged.
- #63: temporal display processor research umbrella.

Open #15 remains separate: the resize/input readiness collision appears tied to Crossterm behavior and must not be papered over by this milestone.

## 0.3.0 thesis

0.3.0 should earn its minor version through **rendering-time semantics and advanced realization**, not through a random component grab-bag.

The proposed milestone is:

> LibGibson gains explicit high-cadence frame-pacing/observability foundations and an experimental temporal realization axis that can generate ordinary surfaces for any viewport or composition, while preserving the one existing renderer.

## Workstreams

### A. Finish post-0.2.5 foundations

1. Validate #61 allocation cleanup through full CI.
2. Harden `FramePacing::PhaseLocked` and add deterministic scheduler regressions for underrun, overrun, policy switching and FPS changes.
3. Finish `FrameReport` with exact semantics and tests for settled frames, full repaints and writer stalls.
4. Decide whether direct public mutation of `FrameScheduler::max_fps` needs an additive setter/deprecation path so timing epochs cannot become stale.
5. Re-audit `TerminalCapabilities::synchronized_updates` versus `TerminalSession::sync_updates_enabled`; document or reconcile the apparent dual policy rather than changing behavior blindly.

### B. Temporal algorithm lab

1. Keep `TemporalBrailleField` experimental.
2. Add a deterministic static best-frame control.
3. Add residual targets: static base + bounded temporal correction, instead of full-depth duty cycles as the normal path.
4. Add motion invalidation so changed/moving regions reset residual state and can fall back to static.
5. Evaluate phase-decorrelated first-order sigma-delta against fixed schedules and an STBN/blue-noise sequence.
6. Add explicit color/chroma experiments only after luminance controls are understood.

### C. Cadence beacon / physical experiment

Build an example or sibling lab that can emit:

- a monotonically encoded frame/phase beacon,
- a bounded temporal test region,
- synchronized updates on/off,
- configurable emission FPS,
- deterministic target patterns,
- a static control side-by-side.

Presentation must be measured externally (high-speed camera/photodiode). Do not pretend the PTY can report display cadence.

### D. Static image projector first

Before trying to temporal-dither arbitrary images, build the best static tile projector:

- choose glyph basis,
- fit foreground/background colors,
- measure error in linear light,
- cache candidate codebooks,
- use the temporal modulator only on the residual.

This is where most image quality is expected to come from.

### E. GPU research backend

Implement a CPU reference projector first.

Then, if a Vulkan-capable adapter is available, prototype the same batch fitting with `wgpu` outside the core dependency graph. GPU acceleration is potentially useful for large candidate searches/codebook projection, not for the 38k-subpixel sigma-delta update itself.

Acceptance for GPU work:

- same deterministic target/error metric as CPU reference,
- no mandatory GPU dependency in core,
- explicit adapter discovery/fallback,
- end-to-end benchmark including upload/dispatch/readback,
- measurable win on realistic raster sizes rather than kernel-only throughput.

### F. Viewport / semantic integration

Temporal realization should work wherever an ordinary Surface works.

Preferred integration:

- application owns `TemporalBrailleField` or future `TemporalSurface`,
- each tick produces a Surface,
- use `Node::surface`, `Node::canvas`, viewport clipping, Scene or `ui::surface`,
- renderer remains unaware of temporal intent.

A convenience `TemporalViewport` wrapper may be added later if repeated external code proves it useful, but it must be a thin composition helper, not a second renderer.

### G. Safety / fallback

Before any interactive hero demo:

- default off,
- measured-profile requirement for luminance modulation,
- depth cap,
- region/area budget,
- reduced-motion/static override,
- hysteresis after timing degradation,
- static fallback equivalent in mean luminance,
- no saturated-red modulation in early experiments.

## 0.3.0 release gate

0.3.0 should not be cut merely because the temporal module compiles.

Minimum evidence:

1. v0.2.5 regressions remain green; C ABI stays v1 unless separately authorized.
2. MSRV remains 1.85 unless deliberately reviewed as a minor-version change.
3. #61/#62/#64 foundations are fully tested.
4. Temporal algorithm tests prove duty/error convergence and deterministic fallback.
5. At least one measured high-refresh terminal/display setup shows a perceptual advantage over the best static baseline at an acceptable modulation depth.
6. At least one ordinary 60 Hz setup automatically refuses luminance temporal mode and produces the static fallback.
7. A viewport/canvas/Scene consumer demonstrates that no second renderer was created.
8. CPU vs optional GPU projector results are recorded honestly.
9. Accessibility/safety behavior is explicit in API/docs/example.
10. #15 is either independently fixed through supported upstream evidence or remains explicitly open; it is not a 0.3.0 hostage unless the temporal experiments expose a direct dependency.

## Likely release shape

Potential 0.3.0 additions if the experiment succeeds:

- `FramePacing`
- `FrameReport`
- experimental `temporal` module
- cadence-beacon / temporal-display lab example
- static/residual tile projector
- optional external GPU calibration/projector prototype
- docs and measured validation report

Potential things that should **not** enter 0.3.0 without new evidence:

- arbitrary retained widget framework
- CSS-like styling
- a new terminal protocol
- temporal fields on `Cell` or `Style`
- mandatory `wgpu` dependency
- default-on flicker/dithering
- claims of spatial pixel super-resolution
- display-vsync claims.

## North star

Keep the categorical boundary clean:

`target visual -> static cell projection -> bounded temporal residual -> ordinary Surface -> existing diff/ANSI renderer -> terminal`

Every stage should be independently testable, replaceable and able to collapse to the identity/static path.
