# PHOSPHOR — Perception-aware terminal rendering (research RFC)

**Status:** proposal; no PHOSPHOR runtime is implemented by this RFC. **Date:** 2026-11-19.
**Intended consumer:** ProjectGHOST / MINERVA, with generic applicability to LibGibson's graphics, instruments and cinematic SIGNAL work.
**Owner/merge contract:** separate experimental work, no default behavior or public ABI changes, no mainline merge or release from this document alone. Implementers start with [PHOSPHOR_AGENT_HANDOFF.md](PHOSPHOR_AGENT_HANDOFF.md).

> **Design thesis:** Compile a high-resolution, source-authenticated scene into the best *physically realizable light field* permitted by the particular terminal and font, while optimizing for contours, identity, stability, and cost in the human observer's visual field. The character grid is the transport surface; it is not the full description of what the observer sees.

This is a **fidelity research program**, not a claim that Unicode terminals already possess arbitrarily high spatial resolution. Maximize useful *perceived* detail and preserve source information. Make stronger claims only after physical and human discrimination tests.

## 0. Recover the existing baseline before writing rendering code

Pinned inspection baseline: LibGibson `68e53020f198df725a3276333bdb2bbb643245fa` (`main`, merged #418, 2026-11-19); ProjectGHOST `e8703feae0d761a972f54df1ef30ceb4fff9b285` (`main`, 2026-11-19); Gilded Signal is a **separate draft**, [ProjectGHOST PR #296](https://github.com/femboy2112/ProjectGHOST/pull/296), stacked on #295 and its Living Stage integration. No Gilded result is evidence that it is merged into either default.

Verify again against current HEAD and the repository's current constraints before implementing. Read at minimum:
- [`src/raster.rs`](../src/raster.rs) / [`src/raster/rgba.rs`](../src/raster/rgba.rs): `RgbRaster::to_surface()` emits `▀` with top pixel as foreground and bottom as background. `RgbaRaster` and `composite_over` handle image alpha.
- [`src/stage/raster_plane.rs`](../src/stage/raster_plane.rs), [`src/stage/driver.rs`](../src/stage/driver.rs), [`docs/STAGE.md`](STAGE.md): Stage resolves semantic anchors, clips and composites into **one RGB sample horizontally and two vertically per terminal cell**. `compose_prepared` then calls `pixels.to_surface()`. This is an explicit *early information-loss seam* for higher-resolution sources, not a defect in `AnsiCompiler`.
- [`src/temporal.rs`](../src/temporal.rs), [`docs/TEMPORAL_DISPLAY_PROCESSOR.md`](TEMPORAL_DISPLAY_PROCESSOR.md), [`docs/TEMPORAL_DISPLAY_VALIDATION.md`](TEMPORAL_DISPLAY_VALIDATION.md): validated logical 2x4 Braille projection, 127 complementary two-color partition candidates, sigma-delta residual modulation, transport-aware cell-change tradeoffs, display-profile gating, fallback. **Do not reimplement or weaken them.**
- [`src/canvas.rs`](../src/canvas.rs), [`src/glyph.rs`](../src/glyph.rs), [`docs/GLYPHS.md`](GLYPHS.md): Braille 2x4 dot masks, half-block 1x2, block/ASCII fallback, separate terminal-protocol and glyph-repertoire capability axes. A UTF-8 Linux VT does not thereby support Braille.
- [`src/ansi.rs`](../src/ansi.rs), [`README.md`](../README.md), [`docs/RELEASE_CONTRACT.md`](RELEASE_CONTRACT.md): one terminal owner, differential output, counted writes, uncertain partial writes and recovery, native scrollback semantics. Synchronized-update framing is not atomic I/O or display-vsync.
- [`docs/KITTY_IMAGES_DESIGN.md`](KITTY_IMAGES_DESIGN.md): **already specifies an unimplemented**, opt-in Kitty image-transport path. PHOSPHOR must extend that design, not create a competing protocol or pretend support already ships.
- [`docs/V07_RESOURCE_ENVELOPE.md`](V07_RESOURCE_ENVELOPE.md) and [`research/temporal_gpu/README.md`](../research/temporal_gpu/README.md): existing CPU/terminal cost envelopes, projector baseline; GPU is not justified without end-to-end superiority.

**Consumer-specific source truth:** ProjectGHOST [`docs/AVATAR_CANON.md`](https://github.com/femboy2112/ProjectGHOST/blob/main/docs/AVATAR_CANON.md) and [`docs/AVATAR_ART_PREP.md`](https://github.com/femboy2112/ProjectGHOST/blob/main/docs/AVATAR_ART_PREP.md). The existing canonical atlas carries **approved** downsampled RGBA sprites; the legacy documented expression rungs include 12/16/24/32/48 px widths and body views up to 46x80 px. The selected Mk.0 pack must be inspected separately; do not assume its rungs are identical. No private artwork is in git. A higher-resolution original cannot be recovered from a previously downsampled tile by a new glyph algorithm. Source/atlas checksums, profile identity and explicit owner approval remain binding. New face, gaze, eyelid, viseme or anatomy details require separate approved art; software must not invent them.

## 1. Human observer as part of the display transform

The observer samples **light at an angle** and integrates it across spatial and temporal sensitivity functions. Neither 60 Hz nor 60 pixels/degree is a universal human limit.

- **Foveal achromatic detail:** Ashraf, Chapiro & Mantiuk (Nature Communications, 2025) measure resolution limits up to a reported 94 pixels per visual degree (ppd) for foveal achromatic patterns, 89 for red/green and 53 for yellow/violet in their particular tests. The numbers describe *conditions and tasks*, not a fixed universal human threshold. Peripheral acuity and chromatic resolution differ.
- **Contrast sensitivity:** spatial frequency and edge contrast, not only sample density, control visibility. Fine low-contrast pores/textures may disappear while a small correctly located edge remains legible. Quantify with a **declared** contrast-sensitive loss; do not assert a universal curve for every user.
- **Temporal sensitivity:** a 2023 human study found maximum flicker sensitivity around 10–20 Hz in the tested conditions; luminance, stimulus size and eccentricity matter. Emit Hz is not panel-presentation Hz. There is no single human "frame rate."
- **Fixational eye motion:** drift and small eye movements can affect extraction of fine detail; motion may improve or degrade information depending on geometry. That is a hypothesis generator for structured moving scenes, **not** a license to flicker a face.
- **Object inference:** silhouettes, occlusion, coherent shading, eye placement and stable perspective can allow the observer to recognize a detailed 3D object even when local pixel-level detail is absent. Record any identity/reconstruction gain through a human test, not a renderer assertion.

Define distinct quantities:
1. **Source resolution**: independently specified detail actually present in the authorized asset/model.
2. **Transport/display addressability**: independent physical colors or glyph shapes the terminal can produce.
3. **Effective reconstruction**: which parts of the source are discriminable after panel response and viewing geometry.
4. **Recognition/identity fidelity**: whether the human perceives the correct character and expression.

Never call #3 or #4 more *physical pixels*.

### Angular geometry and a sanity-check example

Let display-pixel pitch be `p` millimeters and viewing distance `d` millimeters. For fronto-parallel small angles:

```text
angular_pitch_degrees = (180/pi) * 2 * atan(p / (2*d))
physical_ppd         = 1 / angular_pitch_degrees
```

Illustration only (not a measurement of a user's terminal): 120 columns x 40 rows at 9x18 **panel pixels per cell** implies a 1080x720 display region. With `p=0.277 mm` and `d=600 mm`, ~37.8 physical pixels/degree. Half-block color offers 120x80 **logical RGB samples**; Braille offers 240x160 **binary dot sites** (~8.4 dot sites/degree in either axis for this exact geometry), NOT independently colored pixels or the same shape as physical panel pixels. Each Unicode glyph is rasterized by the chosen font; effective coverage and subpixel boundaries differ. Measure font-cell pixel geometry, device pixel ratio, scaling, fallback fonts and panel behavior rather than trusting the terminal cell dimensions alone.

## 2. Physical forward model: from glyph and RGB to retinal stimulus

For a calibrated cell glyph `g`, let `M_g(x,y)` be its measured **foreground coverage** in [0,1] over panel-pixel sample locations. Let `F,B` be foreground and background linear-light RGB. Simplified emitted cell:

```text
R_g(x,y) = M_g(x,y)*F + (1-M_g(x,y))*B
```

In reality the terminal font renderer, LCD subpixel layout, sRGB code-to-light transform, alpha, panel modulation transfer, adaptation, ambient light and eye optics matter. Represent them as a declared transfer `H_device` and optional modeled/validated `H_eye`:

```text
L_observed ≈ H_eye( H_device( R(g, F, B, font, geometry, code_depth) ) )
```

**Scope boundary:** this is an operational forward model, not a comprehensive model of perception; a calibrated software screenshot is NOT a panel measurement. A display's physical emission and human interpretation require separate instruments.

**Spatial phase is meaningful only with declared coordinates.** E.g., a contour's signed offset in *measured panel pixels*, font-cell origin, or explicit subcell registration. A rotated/shifted raster basis can approximate a contour between ideal Braille dot centers, but cannot create spatial modes outside the realizable glyph/panel basis. No unmeasured "phase amplification" claims.

### Reachable-space boundary

For **fixed font, device geometry and color behavior**, a temporally averaged stack of ordinary renderable cells is a convex combination of their emitted pixel fields under a linear temporal integration model. Averaging does not create independent arbitrary high-frequency image features absent from every emitted state; it can refine color/tone and reduce quantization bias. A moving 3D object can transmit *additional cues sequentially* if pose/shading changes are authentic; that is a different, time-dependent task. Distinguish actual static-image super-resolution from enhanced object perception.

## 3. PHOSPHOR static compiler: the primary engineering bet

**Optimize the emitted glyph raster instead of treating a Braille dot as a perfect square sample.** The current `project_rgb_subcells` performs a correct useful logical 2x4 two-color partition fit; PHOSPHOR adds an optional *font-calibrated*, multi-glyph realization backend with a higher-resolution source before quantization.

### Proposed internal types (names sketch only, NOT stable public API)

```rust
// Research-only design sketch, NOT existing public API.
struct GlyphCoverage {
    scalar: char,                 // validated single-cell grapheme
    ink: Vec<u16>,                // fixed-grid alpha/coverage samples
    sample_width: u16,
    sample_height: u16,
}

struct DisplayCalibration {
    id: String,                   // content/version digest, no secret values
    provenance: CalibrationSource, // Synthetic | Screenshot | MeasuredPanel
    terminal_family: String,
    font_identity: String,
    font_cell_px: (u16, u16),
    device_scale: f32,
    color_depth: ColorDepth,
    coverage: Vec<GlyphCoverage>,
    // Declared uncertainty, sampling, gamma/transfer model, etc.
}

struct FidelityPolicy {
    mode: FidelityMode,          // Portable | StaticCalibrated | PixelGraphics
    max_source_px: u64,
    max_candidate_work: u64,
    max_changed_cells: usize,
    reduced_motion: bool,
}

struct ProjectedRegion {
    surface: Surface,            // normal terminal-owned output
    error_receipt: ProjectionReceipt,
}
```

The exact public API, if ever needed, should be justified by a consumer forcing test and tier review. The calibration format must have a version and validation limits (dimensions, bytes, whitelist of single-cell glyphs, fallback font identity, max glyph count); reject malicious content and control sequences. Do not commit font binaries or private screenshots. A reproducible **synthetic** mask fixture is sufficient to build and test the first CPU reference.

### Static candidate search

For each terminal cell, sample its **higher-resolution target RGB tile**. For each allowed glyph `g`, solve bounded linear-light foreground/background colors `F,B`, then quantize **as the actual terminal will** before scoring. Candidate score on a region/neighbor pair:

```text
E_total = w_pixel E_filtered
        + w_edge E_contour
        + w_identity E_salience
        + w_wire E_changed_cells_bytes
        + w_stability E_style_glyph_changes
```

- `E_filtered`: full tile error after an explicitly specified spatial filter; include unfiltered RMSE as a separate receipt.
- `E_contour`: gradient, orientation and connected-boundary continuity (not merely high-frequency energy). Add a cross-cell boundary penalty; independent per-cell minima can sever an eye/veil contour.
- `E_salience`: **caller-supplied optional** masks (eyes, veil edge, important text, icon) in scene coordinates, never hard-coded anatomy in the library. Negative control: incorrect/scrambled salience mask must not improve identity discrimination.
- `E_changed_cells_bytes`: actual `SurfaceDiff`/`AnsiCompiler` byte and cell cost, **not** just dot-toggle count. Stable style palettes and clustering are desirable.
- `E_style_glyph_changes`: penalize unstable glyph/color choice for unchanged source, especially around salient edges. Regularization weights and units must be recorded in the experiment.
- Always emit valid single-column glyphs under measured/declared font capability, no wide-glyph overwrites, text controls, or unsupported glyph assumptions.

Calibrate/render glyph coverage **per font and rasterizer**. Compare baseline basis families: half-block, Braille, block shades, quadrants, selected box/diagonal forms and **optional** sextants (subject to correct Unicode and font support). More glyphs are not automatically better: mixed glyphs may introduce uneven texture, width/fallback failures and extra wire bytes. Include ablations per glyph family and a deterministic ordering/tie-break.

Research phases: exhaustive CPU oracle for tiny tiles; constrained least squares and top-k candidate search for release-sized targets; multi-cell seam optimization, then memoized/codebook tiles and dirty-region reproject. Do not add a GPU dependency absent an end-to-end benchmark win.

## 4. Fix the high-resolution source-to-Stage boundary

**Today:** `RasterPlane` sources are `RgbaRaster`, with offsets in one-cell-wide/two-cell-high raster samples. `compose_prepared` creates a matching `RgbRaster` underlay, alpha-composites and ends in `pixels.to_surface()` (half-block). Thus a 4x or 8x source cannot magically survive as high-frequency data if it is first reduced to this 1x2-per-cell substrate.

**Target:** keep high-resolution source geometry through composition and clipping; lower only once, at the output backend. Preserve Stage's semantic keys, one Taffy layout, modal/input sidecars, parent clips, signed placement, depth/z, deterministic sampling, and exact terminal ownership.

Proposed experimental join (NOT an API demand yet):

```text
approved source / software scene / 3D / SIGNAL
    -> native RGBA or scalar/vector light field, declared sampling coordinates
    -> one semantic UiRuntime frame + keyed Stage geometry
    -> source-over composite at explicit higher-res density
    -> choose *one* realization:
          (A) existing half-block RGB (compatibility)
          (B) calibrated glyph optical projector (ANSI, static)
          (C) qualified real pixel image transport
    -> normal Surface/terminal writer (or terminal-owned image transport)
    -> receipt: source coverage, clipping, errors, bytes, changed cells
```

Choose a bounded internal research type such as `HighResPlane` or `OpticalFrame`. Do NOT silently change CORE `RgbRaster` semantics, `StageFrame` type identity, the C ABI, or all callers of `resolve_compiled_raster`. If a source is higher-resolution than a raster destination, use one specified premultiplied-alpha area filter / linear-light policy before the **final** projection, respecting source aspect and pixel phase. Never upscale an underspecified approved portrait and claim its original detail has returned.

Hard tests:
- odd signed offsets; negative and offscreen clips; layer z-order independent of insertion; source-over with partially transparent edge; output quantization fallback;
- resize/reorder/focus/modal overlay; animated face (where a real authorized source exists); null/static pixels; no OOB;
- semantic UI and text remain crisp and remain above raster unless deliberate z rules say otherwise;
- same original `Surface`/ANSI bytes in default mode, including FLOW, inline scrollback, resize, send/commit, and terminal teardown;
- bounded source dimensions, source planes, work units, memory, frame bytes and artifacts; reject oversize/untrusted images, not silently clamp metadata.

## 5. CRT ideas: useful physical analogues vs visual decoration

A CRT is not equivalent to a sample-and-hold LCD displaying terminal glyphs. Treat these as *design tools*, not a claim that ANSI output reproduces electron-gun physics.

| Candidate | Mechanism | Recommended role | Limitation / falsifier |
|---|---|---|---|
| Calibrated spot/point-spread reconstruction | approximate a finite luminous spot and anti-aliased edge | static contour optimization | broad blur may destroy precisely the edges needed; compare edge MTF and visual identity |
| Subcell coverage / spatial-phase fit | measured glyph masks place fractional coverage at actual font-pixel positions | **core static fidelity** | cannot extend beyond reachable basis; demand independent-renderer captures |
| Edge-aware antialiasing | preserve signed distance/contour orientation before quantization | face, veil, gold rim | oversmoothing harms identity; compare edge map in holdout |
| Controlled halation/bloom | source-derived local light spill | background, crown/laurel highlights | false halos around eyes/face or opacity; cap radius/energy, measure |
| Scanlines/aperture/grille texture | patterned modulation of visual energy | optional visual style ONLY | can lose contrast and detail; default off |
| Phosphor-like persistence | temporal low-pass/feedback via existing `FeedbackBuffer` | deliberate environment trails | ghosting/smear for moving facial edges; prohibit as core resolution trick |
| Motion parallax/occlusion/highlights | coherent geometric constraints across authentic views | long-range research for approved animation | requires actual 3D/pose art, not generated contradictory expressions |
| Temporal residual modulation | bounded duty-cycle color/tone correction | **experimental, background/low-salience only** | required measured stable high cadence and safety; no static spatial-resolution claim |

For source-derived bloom, estimate luma from *linear-light* source, convolve within a bounded ROI, composite behind approved alpha edges, and reject added light over preserved opaque source features. Preserve sharp foreground contours. All styles must support disabled comparison and stable static output.

## 6. Temporal policy (reuse existing code, do not overclaim)

The existing [temporal research](TEMPORAL_DISPLAY_PROCESSOR.md) found a narrow win on its simulated font bases (FreeMono 12x20): static natural grayscale RMSE ~0.038–0.039 vs four-phase ~0.031–0.032; sharp/disc tiles gained less. These are **prior model/software measurements**, not reproducible benchmarks of this RFC, nor a perception study. [Validation](TEMPORAL_DISPLAY_VALIDATION.md) says HUMAN perceptual benefit is unverified.

Rules for PHOSPHOR:
- **Static-first:** always optimize the best static glyph/color representation first. Preserve exact fallback and stationary identity features.
- **Temporal is opt-in**, default off. `PresentationProfile` must be **measured** on the actual terminal. Existing `TemporalSafetyPolicy` (minimum 100 Hz presented, ≥90% survival, p95 jitter bound, ≤10% *emitted* linear-luminance swing, reduced-motion and motion gates, hysteresis) is a research guardrail, **not medical certification**. Do not weaken it.
- **Protected zones:** eyes, face contour, text/caret, modal, authority and high-contrast lines should be static or ordinary source-driven motion; prohibit rapid pattern reversal of large areas. No full-frame strobe, synchronized red modulation or fake 120-Hz simulation output.
- **Do not infer terminal-presented FPS** from emission count/monitor marketing; use existing `temporal_cadence_beacon` and physical capture. Tmux, compositor, SSH and the screen can drop/reorder phases.
- **Moving source invalidates residual history.** Reset/invalidate on ROI changes and clear vacated positions, as already modeled by [`TEMPORAL_VIDEO_COMPOSITING.md`](TEMPORAL_VIDEO_COMPOSITING.md).
- **No mechanism claim** from flicker-free perception alone. A 120-emitted/60-presented alias may bias a naïve periodic scheme; source receipts and capture are mandatory.
- A reduced-motion/static setting must disable all experimental modulation and provide an equal-identity, nondegraded static image.

## 7. Pixel graphics transport (distinct project, share same source)

The absolute highest *physical* detail for terminal-capable environments likely comes from sending actual image pixels, not more Unicode. This is **not ordinary glyph-only rendering**.

Use [`KITTY_IMAGES_DESIGN.md`](KITTY_IMAGES_DESIGN.md) as authoritative existing workstream. It already proposes opt-in capability, placeholder-cell positioning, image id/reuse, bounded uploads, fallback, cleanup, byte-exact goldens and real kitty tests. The [official protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/) supports pixel image placement with transparent content and scoped z-order; actual terminal support must be probed/qualified separately.

Rules: keep exactly the same Stage layout box and semantic inputs; never auto-enable from environment guesses; do not fork alternate terminal owner or write unsynchronized graphics escapes; verify Unicode placeholder, placement, deletion and restoration behavior in real kitty and comparable terminals before enabling. Current library does **not** ship Kitty image transport. In unsupported environments: calibrated/static ANSI if qualified, then existing half-block/mono/ASCII fallbacks. An image transport cannot recover pixels absent from the runtime-approved source.

## 8. MINERVA as the discriminating use case

### Source and integration path

On Gilded Signal draft PR #296, `src/glove/presence_signal.rs` currently composites the selected `CanonSprite` 1:1 over a bounded RGB scene and invokes `paint(...).to_surface()`. It carefully keeps opaque source pixels authoritative over wireframe background; this is the right preservation rule. Rendering uses a centered golden rail (draft #295) and has separate Stage/console acceptance holds.

The highest-value improvements for Minerva:
1. **Remove premature raster loss** in the avatar path (conditional high-res source; one final static glyph optimization), while keeping centered rail and typed caption/actual pose.
2. **Structure-aware static rendering**: priority on iris geometry, eye outline, veil boundary, laurel/gold ornaments, face silhouette and recognizable expression. Source salience masks are optional caller data, not source generation.
3. **Separate background from identity:** soften atmospheric SIGNAL fields and gold wireframe, but preserve foreground art and crisp facial features.
4. **Subtle, causal animation only** from approved poses/art: correct occlusion, focus/gaze with actual overlays if/when approved, no invented lip movement. Normal motion and accessibility fallback remain.
5. **Real pixel branch** for qualified image-capable terminals, same stage/source, no image-only behavior dependency.

Do not edit ProjectGHOST under this LibGibson research PR. Treat its private sprites, exact selected-profile digest and consent/provenance as external inputs. Synthetic fixtures only in LibGibson. For new avatar master resolution or eyelid/gaze overlays, request separate operator art approval and a new digest-bound import; `AVATAR_ART_PREP.md` currently marks such features as not built.

## 9. Evidence, falsification and qualification

**Claim ledger at RFC publication:**
- **Observed / code-inspected:** 1x2 RGB Stage realization, half-block output, existing logical Braille projector, glyph family fallback, temporal gates, design-only Kitty route, and downstream Gilded Signal draft. These are checked against the cited files and branch snapshot; no fresh test run in this write-up.
- **Prior software measurement:** static Braille codebook ~28.6 ms at 160x50 and model RMSE figures, as recorded in historical research docs. Setup-bound, not this RFC's benchmark.
- **Corroborated external literature:** human acuity, temporal sensitivity and eye-motion effects within their study conditions. Transfer to LibGibson is **conjectured**.
- **Unverified:** physical glyph coverage on the operator's terminal, human-judged Minerva gain, high-cadence panel survival and perceptual efficacy of CRT-style effects.
- **Boundary:** no arbitrary static high-frequency reconstruction from merely averaging the same glyph basis; no return of information already removed from the source.
- **Next verdict-changing lamp:** reproducible physical terminal calibration plus a blinded held-out recognition/contour test against **both** current renderers.

Primary discriminating experiment:
- **Pre-register** source fixtures, target terminal/font, viewing distance, calibration protocol, candidate set, cost limits and objective weights *before* looking at holdout results. Freeze training/test split.
- **Controls:** half-block truecolor, logical Braille static, spatial Bayer static, calibrated single-family (ablation), mixed-glyph calibrated, optional image graphics *only on qualified terminals*. Equal source/pose/layout and honest color depth.
- **Positive:** a target literally generated from a known glyph mask under calibrated colors should be recoverable at the correct glyph/position.
- **Null:** exact constant-color tile and unchanged idle frame should not create novel texture or I/O.
- **Negative/hostile:** subpixel structure never present in approved source; unseen diagonal/pixel-phase edges; hard red/black flicker; unsupported font; broken fallback/ligatures; 120 Hz emission with systematic 2:1 frame loss; altered font after calibration.
- **Mutation:** swap fg/bg coverage without complementing glyph; shift glyph calibration by one measured panel pixel; scramble salience map; disable source-alpha protection. Tests must detect these mutations.
- **Holdout:** independently rasterized source/renderer, *different font/terminal* (leave one calibration family out), source images not used to fit any weights, a real operator-approved avatar locally. Same-renderer render/re-render is a consistency check, **not independent evidence**.
- **Metrics:** emitted linear-light RMSE, multiscale error, edge/registration displacement, line connectivity, color delta, identity forced-choice accuracy, operator comfort, p50/p95 CPU projection, p95 write/flush, bytes/changed cells, frame survival/jitter. Report each; no composite "true resolution score".
- **Pass/fail protocol:** require measured non-regression in preserved default behavior; proposed visual gain must beat the **best** preregistered static baseline on blinded holdouts under comparable performance and no significant identity regressions. Define quantitative thresholds before evaluating the holdout, not after. If better RMSE but worse eyes/identity, **fail** Minerva goal. If no improvement outside fitted font, retain as font-specific mode, not generic default.

### Build/stop gates

See [agent handoff](PHOSPHOR_AGENT_HANDOFF.md) for work packages and exact gates. Core principles:
1. Source authenticity > presentation fidelity > decorative effects.
2. Preserve every default byte and all semantic/terminal ownership guarantees.
3. Static measurable gain before temporal experiments or GPU acceleration.
4. A physical capture does not imply human preference; a forced-choice human result does not imply new physical pixels.
5. Any no-coverage / no-performance win is a valuable negative result; record it, do not make performance and visual claims stronger.

## 10. Primary references and internal neighbors

Research:
- Ashraf, Chapiro & Mantiuk (2025), [*Resolution limit of the eye — how many pixels can we see?*](https://www.nature.com/articles/s41467-025-64679-2), Nature Communications: foveal/peripheral achromatic/chromatic psychophysics (the 94/89/53 ppd conditions).
- [*Electrophysiological model of human temporal contrast sensitivity based on SSVEP*](https://pmc.ncbi.nlm.nih.gov/articles/PMC10433170/) (2023): psychophysical/electrophysiological temporal sensitivity around 10–20 Hz under tested conditions.
- Wu et al. (2024), [*Fixational eye movements enhance the precision of visual information transmitted by the primate retina*](https://www.nature.com/articles/s41467-024-52304-7), Nature Communications: retinal-signal study; do **not** equate its results to proven terminal user gains.
- [Kitty terminal graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/) (live specification; verify exact placeholder/deletion behavior).
- [`docs/TEMPORAL_DISPLAY_PROCESSOR.md`](TEMPORAL_DISPLAY_PROCESSOR.md), [`docs/TEMPORAL_DISPLAY_VALIDATION.md`](TEMPORAL_DISPLAY_VALIDATION.md), [`docs/TEMPORAL_VIDEO_COMPOSITING.md`](TEMPORAL_VIDEO_COMPOSITING.md).
- [`docs/KITTY_IMAGES_DESIGN.md`](KITTY_IMAGES_DESIGN.md), [`docs/GLYPHS.md`](GLYPHS.md), [`docs/STAGE.md`](STAGE.md), [`docs/V07_RESOURCE_ENVELOPE.md`](V07_RESOURCE_ENVELOPE.md).

**Next document:** [PHOSPHOR_AGENT_HANDOFF.md](PHOSPHOR_AGENT_HANDOFF.md) turns this into independent bounded research and engineering tasks. No pixels, source art or font binaries are bundled.
