# PHOSPHOR — agent execution handoff

**Research RFC:** [PHOSPHOR_RFC.md](PHOSPHOR_RFC.md).
**Purpose:** turn eye/display/terminal geometry into a measurable static fidelity improvement, then integrate safely with SIGNAL/Stage and a separately approved MINERVA consumer.
**Status:** planning/coordination document only, 2026-11-19. Implementation, current tests, human visual results, font calibration and real terminal captures **not run by this document**.
**Inspection baseline:** LibGibson main `68e53020f198df725a3276333bdb2bbb643245fa`. **Re-check HEAD** and intersect with each agent's actual starting branch before work.

## Commander intent

The desired observable is **a recognizably more detailed, identity-faithful MINERVA render on ordinary terminals**, while retaining a path to actual pixel graphics in terminals that support it. Do not define victory as "more Braille characters", "more frames per second", "higher PNG dimensions", "lower same-font training RMSE" or "looks like a CRT". The goal is source fidelity and observer-visible geometric detail under physical, wire, CPU and safety constraints.

**Rank the interventions by expected value:** (1) stop source information being downsampled too early; (2) improve **static** reconstruction with a calibrated glyph basis; (3) protect high-information contours and color placement; (4) add bounded optical/CRT-looking scene effects; (5) investigate physically measured temporal residual only when its existing gates pass; (6) negotiate real image transport as a separate, qualified backend. Do not reverse this order for spectacle.

**Authority:** this handoff does not authorize merging any implementation, changing supported defaults, installing/releasing binaries, replacing approved avatar art, or weakening consumer/terminal gates. Agents may implement on isolated branches once their orchestrator grants code-work authorization; write draft PRs, exact SHA reports and clean handoffs for review. No shared-branch force-push, no private artwork/fonts/capture bytes or secrets in git.

## Repository custody and mandatory preflight

Before code, each agent records: repo path, branch, HEAD SHA, parent SHA, tree status, relevant instructions, caller/tool versions, Rust MSRV, current baseline tests, any worktree/branch collisions and the exact files it owns. Read `README.md`, `DESIGN.md`, `ROADMAP.md`, `docs/RELEASE_CONTRACT.md` and RFC section 0 plus component-specific contracts. Follow current repo tier labels and CI policy; do not assume older historical milestone status is current.

LibGibson has **one terminal owner**, immutable/native scrollback and a live region. Stage has **one** `UiRuntime`/`Context` and receives the same action sidecars and resolved geometry used to paint. All new optical projections feed that surface and its diff compiler, or use the existing [Kitty images design](KITTY_IMAGES_DESIGN.md) for its own qualified transport. Do not build a parallel cursor writer, semantic stage, update loop, input owner or panic path.

## Workstream matrix (parallelize only independent work)

| ID | Owner code/doc fence | Output / first usable deliverable | Dependency | Verdict gate |
|---|---|---|---|---|
| A — calibration & physics | `research/phosphor/*`, synthetic fixtures | bounded font/terminal profile format, independent mask rasterizer or calibration importer, diagnostic examples, measured-vs-synthetic provenance | none | known glyph mask and false font profile distinguished; reproducible digests |
| B — static glyph projector | experiment-local implementation first, then `src/phosphor/*` only after design review | CPU oracle for glyph + FG/BG; costed mixed-glyph fitter; standard `Surface` output | A synthetic basis (not physical capture) | best-baseline holdouts, alpha/color correctness, bounded CPU/wire |
| C — source/Stage preservation | `src/stage/*`, `src/raster/*` as separate experimental branch | prototype high-res plane/sampling join; existing half-block path **byte-identical** by default | A/B target interface settled | full geometry/ownership/clip regression and memory bounds |
| D — MINERVA downstream | *ProjectGHOST* branch **only after consumer authorization**; no art in git | consumer-local high-fidelity portrait with exact approved atlas and text-only fallback | B + C; art source authority | operator-held portraits and real PTY/interaction comparisons |
| E — optical/CRT design | experiment-only `examples/*` / `research/phosphor/*` | static spot, halation, contour continuity, scene-coherent highlight tests | B; preferably D synthetic consumer | strict ablations and no silhouette/eye/contrast degradation |
| F — pixel image transport | existing [`KITTY_IMAGES_DESIGN.md`](KITTY_IMAGES_DESIGN.md) workstream, independent branch | qualified pixel backend with same Stage box/fallback | separate terminal protocol gate | real kitty/PTY, byte-exact goldens, cleanup, redraw/restoration |
| G — human/physical verification | `research/phosphor/validation/*`; local operator hardware excluded from git | measurement matrix, holdout protocol, blinded choice forms, frozen decision criteria | A, B, possibly D/F | physical evidence vs model evidence separated; no inflated claim |

A, the synthetic part of B, and the verification **protocol** of G can begin immediately and independently. C should be a separately scoped engineering branch once a specific pixel-dimension interface has a worked example. F should **continue** the existing Kitty design, not compete with it. E should wait until static results exist. D requires user-selected exact approved art and ProjectGHOST integration consent.

## A — Optical basis / calibration research

Start from `src/glyph.rs`, `src/canvas.rs` and the font experiment summarized in [`TEMPORAL_DISPLAY_PROCESSOR.md`](TEMPORAL_DISPLAY_PROCESSOR.md). Build an explicitly **synthetic** 9x18 mask fixture containing half/full block, Braille-like dots, diagonal and a shade glyph, plus a second perturbed profile with shifted/blurred coverage. The prototype must accept arbitrary width/height cell geometry within caps.

Required deliverables:
1. `CalibrationSource = Synthetic | Screenshot | MeasuredPanel` with recorded uncertainty. A screenshot represents a **software-rendered screen image**, not the physical panel light field; do not mark it as physically measured.
2. Versioned serializable profile or deterministic built-in fixture with source hash; UTF-8 identity limited to glyph single-cell scalars, untrusted byte caps, stable sorting, glyph width validation and no terminal control sequences.
3. Optional calibration experiment on actual terminal: compare known-color glyph chart against a separate reference image, save only sanitized non-private numeric masks/metadata; record term/font/version/scale, renderer and cell geometry. Font not present => explicit unsupported, no lying based on TERM.
4. Test that two independently generated synthetic glyph chart instances produce equal profiles; forced 1-pixel shift or FG/BG swap changes the expected test verdict. No external font binaries in repo.
5. Produce a small table showing best/poor glyph-mask fit for synthetic edge/gradient fixtures and the error *after emitted color-depth quantization*.

**A stopping rule:** if the font is unobservable, use deterministic synthetic defaults for algorithm development and mark physical fidelity UNVERIFIED. Never label a simulated font profile "calibrated" without the relevant instrument.

## B — Static optical projector (highest algorithm priority)

Reuse the mathematical structure of `temporal::project_rgb_subcells` rather than copying a second global temporal engine. Implement a bounded CPU reference over a known `GlyphCoverage` codebook:
- Fit foreground and background colors in **linear light** (clamp to representable range) per candidate mask and score after quantization to the supported ANSI color depth.
- Compare **fully renderable** baseline half block, existing logical Braille projection, Bayer/static dithering and candidate mixed-glyph output on exactly the same source and layout.
- For seams, measure orientation-dependent contour error at adjacent cells. Start per-cell, add a bounded 2-cell neighborhood search or seam continuity penalty only if independent holdouts show a difference.
- Keep `Surface` and `Cell` semantics; do not introduce new terminal escapes. Reject unsupported glyphs and ambiguous-width characters. Avoid breaking text, overlays or high-contrast accessibility affordances.
- Use deterministic tie-breaks, cap candidates/iterations/source samples, cache unchanged source tiles, and report work units/bytes/changed cells. No GPU unless CPU benchmark and pipeline metrics say it wins.

Recommended internal milestones:
1. **B0:** exhaustive oracle over 2x2/3x3 synthetic tiles. At least one positive exact-glyph case, one degenerate constant-color case and one impossible out-of-basis case. Report ideal and **emitted** error separately.
2. **B1:** pure region projector API with `Surface` result and receipt; compare B0 oracle vs accelerated code bit-for-bit on seeded independent source samples.
3. **B2:** selected joint glyph/color palette and adjacent seam ablation; beat the *best* preregistered static baseline on held-out edges and gradients at a fixed quality/cost budget. If it does not, record a negative result and retain the best baseline.
4. **B3:** bounded production-shaped CPU path for 80x24, 120x40, 160x50; report p50/p95 runtime, allocations, wire bytes and changed cells. A result that improves one pixel metric but destroys sharp face contours is a failure for the primary use case.

Never compare only to a straw-man fixed ASCII renderer; the real adversaries are existing truecolor half-block and `project_braille_image`. Do not use the fitting fixtures as validation holdouts.

## C — High-resolution source-preserving Stage prototype

Current `src/stage/raster_plane.rs` fits the composited pixels to `viewport_width x (2*viewport_height)` **before** `RgbRaster::to_surface()`. Merely inserting the B projector after this point misses horizontal source frequencies. Prototype **off the CORE path**:
- Retain a source-plane aspect/extent/sampling transform and original RGBA samples through keyed Stage clipping and source-over composition.
- Map Stage cell position to an explicit fine sampling grid; do not change Taffy layout or accepted semantics. Keep signed negative offsets and odd vertical positions correct.
- Preserve original full-opacity avatar pixels exactly at their represented source positions; place background/bloom **behind** them.
- Reconstruct to either existing 1x2 half-block, B calibrated glyphs or future F images from one declared composited source. No extra dynamic stage, no second semantic/UI compilation.
- Validate alpha premultiplication, color-space interpolation, exact border origin, integer overflow, pathological source dimensions and memory ceilings. Reuse `RgbaRaster` where appropriate without changing its stable behavior.
- Support old `resolve_compiled_raster` exactly unchanged when PHOSPHOR is disabled; source-sharing `Arc` must not create unbounded retained content.
- Add real PTY/VTE tests where terminal writing/scrollback could be touched: inline send, cursor retention, resize, clip/reorder, finalization, restoration. Physical glyph perception still a separate test.

**C acceptance:** default headless `Surface` and ANSI bytes byte-identical on seeded fixtures; new mode demonstrably retains at least one high-frequency signal whose loss can be proven under the existing 1x2 substrate. If high-res source isn't supplied, honest fallback and explicit source-resolution receipt.

## D — MINERVA integration once separately authorized

Do not start by modifying the user's approved art or by dropping any of the existing Gilded Signal presentation. Read ProjectGHOST's pending PR #296, #295, #280 and its [art approval policy](https://github.com/femboy2112/ProjectGHOST/blob/main/docs/AVATAR_CANON.md). Its current scene uses selected `CanonSprite::composite_at` over RGB atmospheric/scenery and then `to_surface()`; the exact selected avatar is profile- and digest-bound.

First implement a **synthetic identity fixture**: veil/eyes/laurel/silhouette regions represented as shapes whose exact ground truth is known; no inferred anatomy. Include:
- 24/32/48-column head budgets and wider rails at 80x24, 120x40, 160x48;
- centered placement, caption and actual selected pose, low/light/dark terminal themes as feasible;
- opacity/alpha, highlight behind foreground, reduced-motion/static and fully hidden avatar;
- focus, authority prompt, Details inspector (Alt+I), chat send/redraw, terminal resize, no-color and narrow layouts.
- capture baseline/PHOSPHOR in exact same semantic state and source geometry; local approved art only after explicit owner agreement, never uploaded to LibGibson.

The current atlas has **no** independent eyelid, mouth or gaze layers in the documented legacy pack. Do not simulate moving eyes by repainting invented pixels. Future overlay work requires separately approved new pack and exact digest review.

## E — CRT/illusion effects: subordinate to shape truth

Start with synthetic controlled source highlights and an explicit effect-off control; do not add "scanlines" as a synonym for detail. Candidates:
1. static spot/PSF kernel at source-sample scale, optimize high-contrast edge continuity, compare before and after output quantization;
2. bounded low-intensity source-derived bloom behind foreground, preserve source alpha/opaque facial regions;
3. multi-scale edge-aware downsampling; compare to ordinary premultiplied area resample;
4. **only with authorized pose sequences**, coherent motion/occlusion and correct light response, no random per-frame face texture;
5. scanline/phosphor persistence only as visual styles with no resolution claim.

Metric failures: loss of iris or veil contrast, false halo, outline shift, broken line continuity or poor reduced-motion fallback. Freeze/omit the effect if it fails. No temporal modulation required for any E deliverable.

## F — Kitty graphic pixels

Continue the existing [design](KITTY_IMAGES_DESIGN.md): opt-in capability, same semantic layout box, virtual placeholder placement, image ids/reuse, robust clipping, bounded chunked upload, fallback while pending, delete/teardown/panic cleanup and real terminal qualification. Do not enable by environment guess, don't steal input for capability probes, and do not ship side-channel image bytes through unauthorized contexts. Preserve the character-cell fallback as the universal path.

Pass matrix: ordinary terminal (no capability), kitty with explicit capability, truecolor/256/16/mono, inline vs fullscreen, resize, partial write, scrollback commit, terminal switch/reset/teardown, duplicate image ids, huge malicious payload, stale placement. Confirm each advanced protocol assertion against the **current** Kitty specification and physical host.

**Scope:** this is real pixel image transport, not a glyph-family trick. It may offer the largest physical detail jump if the approved source actually contains higher resolution; still preserve all source approvals.

## G — Reproducible optics/human test plan

Three independent instruments: **(i) code/algorithm oracle**, **(ii) physical terminal capture**, **(iii) blinded human judgment**. Do not count repeated outputs from one internal renderer as independent ground truth.

| Test | Controls | Predicted discriminator |
|---|---|---|
| Known synthetic glyph | exact same glyph + shifted/incorrect glyph mask | exact/near-exact recovery vs measurable error |
| Source phase / diagonals | half-block, Braille, mixed basis, calibration-shift mutant | spatial contour error and seam continuity |
| Full-opacity/alpha source | no overlay; correct composite; flipped-alpha mutant | approved foreground remains exact |
| High-res loss | source-before-1x2 vs after-1x2 vs high-res path | retain edge distinctions absent after irreversible averaging |
| Font generalization | training font, held-out font, no-glyph Linux VT | certified mode succeeds or cleanly refuses/falls back |
| Temporal aliasing | static, residual, ABAB control, 2:1 drop simulation | detect DC bias, shimmer/hysteresis and invalid gate |
| Physical cadence | beacon on/off sync, compositor load, local tty | presented-frame survival/jitter; never infer from emitted Hz |
| Human identity | blind A/B baseline vs candidate, same source/size, randomized order | recognition and comfort without halo/flicker |
| Cost/failure | no-op idle, animated dirty bbox, slow PTY, torn write, SIGWINCH | bounded changed cells/bytes, no lost prompts or history |

Write pre-registered thresholds in a test protocol *before tuning* (minimum edge/identity improvement, allowed performance/regression, sufficient sample size). Separate subjective anecdote from forced-choice statistics. Verify natural-image targets and geometry holdouts; calibrate glyph basis on one font, test another. Maintain raw and source-transformed views, hash tested inputs and exact code versions; report unsuccessful trials.

Human/physical safety: start static; warn before opting into any temporal luminance experiments; honor reduced-motion and the stricter existing gates, do not show red full-field strobe, never assume 120 fps emits/appears as 120 physical frames.

### Measurement receipts

Each agent must write a *small* durable `research/phosphor/receipts/` Markdown receipt (no private assets, fonts, captured personal UI or secrets):
- exact branch/commit/source SHA, toolchain/terminal/font, synthetic vs real status;
- input hash/dimensions/viewport/feature flags/seed;
- baseline, candidate and hostile-control algorithms;
- tests executed, pass/fail counts, mutation controls, and the exact repro command;
- static pixel error vs perceptual/recognition results separately;
- CPU p50/p95, allocs, changed cells and ANSI bytes; physical frame survival/jitter when actually measured;
- known failure cases, untested claims, next discriminating experiment.

If no live hardware or private atlas is available: **report access-gap; preserve synthetic progress; never invent an observation**.

## Coordinator sequencing

1. **Immediately:** agent A produces bounded synthetic calibration fixture and profile; agent G drafts the frozen-probe/holdout protocol; agent B implements independent oracle against A's *published synthetic fixture contract*. No contention on `src/stage/*`.
2. **After A/B reference check:** agent B creates a draft PR with static projection and measured CPU/cell-transport receipts. G reviews with unseen fixtures and a font-basis mutation.
3. **Next:** C adds a strictly opt-in high-res Stage source path and demonstrates that 1x2 losses are avoided, without moving existing API defaults.
4. **Separately:** F advances existing Kitty images design; do not block B/C on it.
5. **Only when the new sources are stable and user authorizes consumer work:** D demonstrates Minerva, preserving all pending Gilded Signal/Stage holds and profile approvals. E experiments remain optional and subordinate.
6. **Promotion:** request exact-head review for each independently qualified slice. Do not merge into shared main, tag or release on this document alone. Provide a held branch/PR for the orchestrator and operator.

### Completion checklist for an implementer PR

- [ ] No duplicate temporal engine, terminal writer, image protocol or UI loop.
- [ ] Explicit stable default/fallback with zero byte difference unless opt-in.
- [ ] Synthetic independent oracle + hostile mutation/failure control.
- [ ] Holdout includes high-salience contours and source-loss counterexamples.
- [ ] All image/glyph/frame input dimensions are capped and validated.
- [ ] Source/provenance, actual terminal font and measured vs simulated status recorded.
- [ ] Tests and performance receipts tied to exact SHA, not described from memory.
- [ ] No raw private image, screenshot, font, personal data, credential or sensitive path in git.
- [ ] Real terminal and human claims absent unless independently observed.
- [ ] No unsupported "infinite resolution", "native CRT persistence", "all fonts", "120 fps displayed" or "perceptual improvement proven" wording.
- [ ] Evidence/holds carried to a draft PR; operator inputs and cross-repo changes remain gated.

**Primary verdict:** more recognizable, source-faithful detail at fixed terminal geometry without flicker, provenance loss, UI breakage or unjustified wire/CPU cost. A failed hypothesis that produces a clear limitation and a better static fallback is a valid research result.
