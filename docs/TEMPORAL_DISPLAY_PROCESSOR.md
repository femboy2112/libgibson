# Temporal Display Processor research

**Status:** experimental 0.3.0 research. Default off. Not a terminal protocol, not display-vsync, and not a production-stability claim.

This document records the post-v0.2.5 design direction for temporal cell realization: emit only ordinary LibGibson `Surface` frames, but deliberately vary subcell masks and/or colors between presented frames so a human observer may perceive a higher-fidelity time average.

## Why this is narrower than the original idea

An external feasibility study (Fable, 2026-09-27) attacked the proposal from human vision, display hardware, terminal scheduling, linear algebra, font rasterization and safety. Its useful conclusion is not "terminals become pixel displays." It is:

- **Tonal/color residual dithering is plausible** on measured high-refresh presentation paths.
- **Human-visible spatial super-resolution beyond the glyph basis is unlikely.** Time gives convex interpolation of glyph rasters, not new spatial basis vectors.
- **General/default deployment is not justified.** A practical mode must be calibrated, opt-in, bounded, depth-capped and able to fall back to the best static frame immediately.

The name **Temporal Glyph Synthesis** remains useful for the broad research program. The feature worth engineering first is **residual temporal dithering**.

## Measured basis result

The supplied font experiment rasterized FreeMono at 12x20 and optimized static/temporal cell tiles. Representative findings:

- Braille fill factor was about 21.7%, so background color must carry the base luminance; white-on-black Braille alone is not a neutral image basis.
- Effective basis rank was small relative to physical cell pixels: Braille 8; block families roughly 11/20 dimensions for 90%/99% variance; box drawing roughly 15/33; a much larger geometric/text basis roughly 35/98.
- On natural grayscale tiles, good static foreground/background fitting was already strong (roughly 0.038-0.039 RMSE for useful bases); four temporal phases improved this to roughly 0.031-0.032.
- Sharp-edge/disc tiles improved only around 15-20% at four phases, with little further infinite-time gain: the missing edge orientations are not created by averaging.

These numbers are evidence for residual tonal/color improvement, not a universal benchmark.

## Reachable-space model

For glyph raster mask G_c(x,y), foreground F_t, background B_t and temporal weights w_t, the time-averaged tile is a convex combination of ordinary renderable tiles. Consequences:

1. Time interpolates; it cannot extrapolate outside the static glyph/color hull.
2. Physical pixels treated identically by every chosen glyph remain indistinguishable forever.
3. More phases eventually stop helping because basis rank, presentation cadence and the eye's temporal sensitivity dominate.

## LibGibson architecture

Temporal realization is **orthogonal** to `RenderMode`.

`RenderMode::Inline` / `Fullscreen` answers: who owns terminal space?

`static` / `temporal` realization answers: how is a logical visual target converted into the next ordinary `Surface`?

Therefore 0.3.0 should **not** add `RenderMode::Temporal`. A temporal generator returns ordinary `Surface`s and can be embedded through the existing `Node::surface`, `Node::canvas`, viewport, Scene and semantic UI paths. The differential renderer and terminal ownership model remain untouched.

The first branch implementation is `temporal::TemporalBrailleField`: a stateful grid of eight logical subcell duty cycles per terminal cell. Each `advance` call performs first-order sigma-delta modulation with deterministic per-dot phase decorrelation and returns one ordinary `Surface`.

That primitive is intentionally lower-level than an image projector. It proves the state model and composition boundary before we add RGB-to-cell fitting.

## Presentation profile: the missing observable

A PTY does not tell LibGibson when a frame reaches the user's panel. Application emission rate is not terminal presentation rate. Terminal emulators may coalesce writes, skip intermediate states, render on their own clock, or be compositor-limited.

`PresentationProfile` is therefore explicitly caller/calibration supplied:

- measured presentation Hz
- emitted-frame survival fraction
- jitter percentile
- whether values were actually measured

Never infer this profile solely from monitor refresh rate.

### Cadence beacon

The first real-world experiment should be an external calibration program, not a library assumption:

1. Emit a frame-counter / phase beacon in a bounded region.
2. Film at >=240 fps or use a photodiode/high-speed capture.
3. Recover terminal presentation rate, distinct-frame survival and jitter.
4. Run a forced-choice staircase over modulation depth.
5. Compare best static rendering vs temporal residual rendering at the same mean luminance.

If invisible residual depth is below ~5%, the luminance path is not worth pursuing on that setup. Around 5-10% is niche portraits/gradients territory. >=10% at a measured stable high cadence warrants deeper experiments.

## Pacing foundation

Historical `FrameScheduler` pacing is completion-relative: the next frame budget starts after the previous render/write completes. That silently stretches high-cadence schedules by terminal write time.

The 0.3 branch adds opt-in `FramePacing::PhaseLocked`. Deadlines advance from the scheduled phase and skip missed periods rather than accumulating lag. `CompletionRelative` remains the compatibility default.

This improves emission regularity but **does not create vsync**. Phase locking is necessary infrastructure, not proof that the terminal presents every phase.

## Per-frame observability

Temporal quality control needs to know whether the application is compute-bound or writer/backpressure-bound. The existing ABI-v1 `RenderStats::last_render_duration_micros` deliberately remains compatible, but the branch adds a Rust-side `FrameReport` separating:

- generation duration (layout + paint + diff + ANSI transaction construction)
- blocking write/flush duration
- exact changed cells
- affected cells
- bytes emitted
- total cells / full repaint

A future temporal controller can disable modulation after sustained writer stalls without guessing which stage failed.

## Modulation strategy

Fixed ABAB or short K-frame movies are fragile: if the terminal/compositor systematically drops one phase, the mean becomes biased and low-frequency flicker appears.

Preferred approach:

1. Start from the best static two-color/glyph realization.
2. Represent only the residual error as a small duty-cycle correction.
3. Use first-order or noise-shaped sigma-delta so irregular frame survival degrades gracefully.
4. Decorrelate phases spatially; do not flash a uniform field in lockstep.
5. On moving content, reduce temporal depth or fall immediately to static.
6. Cluster/hold foreground/background palettes where possible so ANSI style churn does not create the very dropped frames the technique is trying to exploit.

Blue-noise/STBN sequences are a research upgrade, not a prerequisite for the first perceptual experiment.

## Safety / accessibility gate

Temporal modulation is opt-in and **off by default**.

The branch's conservative luminance gate currently requires:

- a measured profile
- at least 100 Hz measured presentation cadence
- at least 90% frame survival
- requested linear-light luminance modulation depth <=10%
- reduced-motion/static preference not active

These are research guardrails, not medical certification. A production design also needs region/area limits, saturated-red handling, hysteresis when cadence degrades, an explicit user warning and an always-available static equivalent.

At 60 Hz, high-contrast luminance modulation commonly lands in the eye's most sensitive flicker band. Constant-luminance chroma modulation may eventually be worth testing separately, but it is not a loophole for enabling unsafe luminance modulation.

## GPU acceleration

The funny answer is: **yes, a GPU-accelerated terminal rasterizer is technically legitimate, but not for the part you first expect.**

Runtime sigma-delta itself is tiny: about eight accumulator updates per terminal cell. At 120x40 that is only 38,400 logical subpixels per frame. CPU scalar/SIMD work will normally beat GPU dispatch + synchronization, and ANSI emission still needs CPU-visible cell decisions.

GPU compute becomes interesting for the *projector/calibration* stage:

- rasterizing/calibrating large glyph bases
- scoring many (glyph, fg, bg) candidates for image tiles
- codebook generation / vector quantization
- batch projection of a large RGB raster into candidate cell states
- offline or amortized fitting of font-specific glyph bases

On an integrated GPU/shared-memory system, `wgpu` may reduce transfer pain, but it still must win measured end-to-end latency over a CPU codebook/SIMD implementation. Do not add `wgpu` as a core dependency until that comparison exists.

Recommended shape: an optional external `temporal-gpu-lab` or feature-gated research backend implementing the same projector trait as a CPU reference. The runtime modulator stays dependency-light.

## Experiments required before 0.3.0 promotion

### Stage 0 — algorithm correctness

- sigma-delta mean convergence
- stable 0%/100% duty
- deterministic phase decorrelation
- ordinary Surface output
- static fallback identity

### Stage 1 — cadence measurement

- beacon on multiple terminal emulators
- presentation Hz / survival / jitter
- synchronized-update on/off comparison
- compositor load / tmux / SSH controls

### Stage 2 — perceptual tonal test

- static best two-color baseline
- spatial/static dithering baseline
- residual sigma-delta
- decorrelated/blue-noise temporal residual
- forced-choice depth staircase
- long-exposure/high-speed capture as secondary instrument

### Stage 3 — image projector

- static per-cell fg/bg + glyph optimizer first
- residual target only after static optimum
- CPU codebook implementation
- optional GPU projector benchmark

### Stage 4 — integration

- bounded temporal viewport/surface example
- phase-locked emission
- automatic static fallback on measured quality degradation
- accessibility controls
- no changes to `Cell`, C ABI or terminal ownership

## Stop conditions

Stop or narrow the feature if:

- measured presentation is generally <=60 Hz for target terminals,
- required invisible modulation depth is <5%,
- frame survival cannot be kept >=90% in a bounded region,
- static two-color fitting captures nearly all useful image improvement,
- terminal style churn causes enough drops to move energy back into low-frequency flicker,
- real users consistently perceive shimmer/noise rather than improved fidelity.

The research can still leave behind useful general improvements: phase-locked pacing, split frame reporting, better calibration tooling and faster subcell realization.
