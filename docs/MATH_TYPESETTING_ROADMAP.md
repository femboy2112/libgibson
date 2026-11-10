# Mathematical typesetting — research-to-v1.0 roadmap

**Status:** OWNER-REQUESTED / PLANNED; no math-rendering implementation or terminal-fidelity qualification is claimed.  
**Tracking:** [issue #334](https://github.com/femboy2112/libgibson/issues/334).  
**Source baseline:** main at `43bae9a6e50336e9a8e0d64e313a0e2d3a16e654`, November 9, 2026.  
**Milestones:** v0.8 research and forcing consumer → v0.9 experimental mathematical node → v1.0 qualified stable math presentation, if its own gates pass. v0.7 release engineering proceeds independently.

## 1. Outcome: mathematical paper on a terminal

The owner wants **genuinely typeset mathematics** inside LibGibson: well-positioned mathematical fonts and glyphs, properly stacked fractions, radicals, variable-size operators and delimiters, scripts, matrices, cases, aligned displays and equation numbering, with spacing and baselines of a printed mathematical paper. **TeX syntax is optional**, not the product requirement. The product is a LibGibson scene element, embedded in the existing renderer and usable by agent consoles, scientific instruments and proof viewers.

Do not claim this goal is satisfied by a plain sequence of Unicode symbols, a Braille approximation, a PDF link, or exporting to an image that cannot be placed and managed in LibGibson's actual scene graph.

There are two different success levels:

- **Paper-quality pixels:** font-aware two-dimensional typesetting with a qualified pixel-accurate terminal graphics transport (when genuinely supported) and headless-pixel reference. Requires an independently verified image protocol and lifecycle, clipping, layering, resize and cleanup behavior.
- **Accessible cell fallback:** semantic/source text plus the best tested Braille 2×4, half-block, ANSI or ASCII rendition available. It may remain recognizable, but **cannot guarantee paper-quality glyphs or typesetting** on a cell-only terminal. It must be labelled as a quality downgrade and must not visually fabricate a different formula.

A low-end machine or a 40×15 terminal must remain responsive; complex equations may require explicit scrolling, zoom/pan or a source-first fallback rather than pretending the full display fits.

## 2. Existing substrate and real gaps

Source-verified starting points:

- `src/node.rs` supports layout containers and deferred `NodeKind::Canvas` painting through the standard scene path; `src/layout.rs`, `src/painter.rs`, `src/renderer.rs` own geometry and terminal output.
- `src/raster.rs` provides bounded `RgbRaster`, a half-block surface realization and a monochrome path; `src/frame.rs` and `src/glyph.rs` support raster and Braille 2×4 / half-block / block / ASCII realization.
- `src/capability.rs` already models Kitty graphics, Sixel and iTerm image support as tri-state capabilities, but marks their status **Unknown** by default. A capability enum **does not mean image transport exists**.
- `src/plot`, `src/field_view.rs` and the v0.8 cross-language harness are useful forcing consumers. The SDK must not grow math-specific ABI exports before a Rust public proof-of-concept demonstrates the required common interface.

The missing bridge is a **math layout engine producing faithful vector/pixel geometry**, plus a **supported high-resolution terminal pixel transport**. Neither is currently evidenced as an integrated LibGibson feature. Drawing a raster through half-block cells is not by itself a complete substitute for a graphical-terminal pixel protocol.

## 3. Proposed layered architecture (not a frozen public API)

1. **Input / semantics:** a small, bounded `MathExpr` or `MathDocument` representation: identifiers, numbers, operators, fractions, scripts, radicals, big operators, fenced expressions, matrices/cases, aligned lines, display/inline mode, label, and copyable original source. Optional adapters accept a *restricted* TeX-like subset, MathML, or Typst-derived input. **No need to implement full TeX.**
2. **Typeset intermediate:** independently measured baselines, ascent/descent, glyph outline/placement, line breaks, penalties, rule thickness, math font metrics, operator limits, color and accessibility/text alternatives. Preserve math identity, not only a bitmap.
3. **Backend adapter:** an offline deterministic typesetter producing vector geometry (e.g. SVG) or direct pixels, with a versioned rendering contract. Investigate, but do not prematurely select, Typst SVG export or MathJax SVG output; assess licensing, execution isolation, fonts and dependency footprint. SVG may be rasterized with an audited bounded renderer. A full PDF/TeX engine is neither required nor automatically safe to invoke on model-supplied input.
4. **LibGibson presentation:** measured geometry becomes a layout-aware math element, composited and clipped by the **one existing renderer**, with inline baseline/line-height semantics or centered display and equation label. Respect the same hit testing, scrolling, resizing, color quantization and terminal ownership as all other nodes. No ad hoc ANSI escape injector.
5. **Terminal realization:** choose a genuinely supported image protocol for a high-fidelity pixel path (Kitty graphics first only if qualified; consider Sixel/iTerm separately). On Unknown/Unsupported, use the existing cell-based realizations with an explicit fidelity receipt and a semantic source/accessibility view. Image-protocol lifecycle, overwrites, scrolling, teardown, tmux/SSH and resize must be independently qualified.
6. **Cache / measurement:** bounded source+font+style+size+backend-key cache, deterministic invalidation, no unbounded retained image bytes, no synchronous high-cost repeated typesetting on input/resize, separately report compile/layout/rasterize/terminal-send costs.

This is a **potential** component boundary, not authorization to add public API structs or dependencies before experiments and review.

## 4. Security, portability and fidelity contracts

- **Untrusted input:** a user, model or remote worker can submit hostile mathematical syntax. Never run it as unrestricted TeX/Typst code. No shell escape, command execution, file reads/imports, network/package fetches, unbounded recursion, arbitrary macros or unsandboxed external process. For an external renderer, establish a deterministic restricted grammar and a real isolation boundary; if isolation cannot be demonstrated, restrict the adapter to explicitly trusted input and keep the untrusted path disabled.
- **SVG and fonts:** reject external URLs, scripts, foreign objects, untrusted font references, unsafe data URIs, unbounded filters and oversized dimensions. Rasterization must have checked memory and time budgets. SVG path geometry alone is not accessible/copyable; retain the semantic formula separately. Do not distribute font files without checking their licenses.
- **Rust compatibility:** LibGibson's core MSRV is **1.85**. Check the actual MSRV of every candidate renderer and transitively resolved dependency; some recent SVG crates already require later compilers. Use an optional adapter/process or a compatible locked version rather than silently raising the project's MSRV.
- **Reproducibility:** pin renderer and font environment for golden fixtures, disclose per-host font substitutions, distinguish source-correct output from visual similarity. Typesetting is not proof verification: a beautifully displayed theorem does not imply it is true.
- **Portability:** report paper-quality support only on terminals **observed** to deliver correct pixel content. Unknown support is Unknown; do not infer from TERM alone. The same source must have a usable headless and copyable fallback.
- **Licensing:** determine renderer, math fonts, glyph embedding and optional protocol implementation compatibility with the project's MIT OR Apache-2.0 distribution before enabling by default.

Reference candidates for investigation (not selected dependencies): [Typst SVG export](https://typst.app/docs/reference/svg/), [MathJax SVG output](https://docs.mathjax.org/en/v4.0/output/svg.html), [resvg renderer](https://docs.rs/resvg/latest/resvg/). Treat live upstream changes and dependency MSRVs as separately verified inputs.

## 5. Staged acceptance and version plan

### M0 — v0.8: typesetter/terminal feasibility (R&D, non-release-gating)

- Probe two materially different offline rendering routes (e.g. Typst or MathJax SVG vs a restricted explicit math-box implementation), with explicit font provenance and MSRV/security/licensing assessment.
- Export to one bounded monochrome and one color pixel buffer. Prove exact dimensions, stable baseline, safe SVG rasterization, clipping and reproducible headless output. Compare against a trusted reference independently from the implementation.
- Build a standalone public-API forcing consumer: formula source → verified pixel/grid geometry → actual LibGibson scene → headless capture. Report what fidelity is lost through half-block/Braille.
- Investigate one real graphical terminal's image-protocol handshake/render/cancel/scroll behavior. A mocked capability flag or arbitrary PNG output does **not** qualify this gate.
- Deliver decision report: candidate renderer/backend, its limitations, a falsifying test, licensing, MSRV, resource envelopes and whether a non-core addon is a better boundary. No new default dependency.

### M1 — v0.9: opt-in math block (experimental)

- Introduce an opt-in `MathBlock`-like component only after M0 identifies a valid renderer and renderer-owned pixel transport. Exact Rust API is TBD.
- Inline math must obey text baseline and reflow; display math must support centering, labels, aligned multi-line expressions, viewport clipping and zoom. Rendered geometry must be stable under scroll, resize, backpressure and repeated creation/destruction.
- Real terminal result and headless golden match within a declared antialias/color tolerance on a named font+terminal; cell-only fallback passes separate legibility and semantic/source preservation assertions.
- Explicit disabled/unavailable/error states, graceful fallback without emitting protocol fragments, and no lost terminal restoration. Include a sample proof viewer and a scientific inspector, using the public interfaces only.
- Profile CPU-only performance and memory pressure on realistic long documents. Renderer work must not block the UI loop unboundedly.

### M2 — v1.0: eligible stable contract, not an automatic promotion

- A maintained, documented math presentation API, quality/capability matrix, accessible semantic output, release packaging, error/ownership contract, representative external Rust and at least one other-language consumer where the interface is supported.
- The published feature promise is exactly the capability tested: paper-quality on **qualified** pixel-capable terminals, honest degraded fallback elsewhere. No blanket "LaTeX rendering works on every terminal".
- Independent renderer reference tests, headless golden, physical terminal or emulator capture tests, invalid-input and adversarial resource controls, source/ABI compatibility and MSI/MSRV floors, license audit, and reproducible release artifact.
- **If any gate remains unresolved**, keep math rendering clearly experimental/optional. Do not delay unrelated stable CORE guarantees or claim math is production-ready. The owner may separately choose to make completion of this feature a release dependency after the feasibility result.

## 6. Evaluation corpus and pass/fail controls

The fixed corpus must include at least:

| Family | Required distinction |
|---|---|
| Fraction / nested fraction | horizontal rule, correct numerator/denominator metrics, no baseline collision |
| Radical | overbar and radical stem enclosing full expression |
| Integral and limits | integral glyph, lower/upper limits, differential spacing |
| Sum/product | large operator, limit placements, multiline constraints |
| Scripts and nested scripts | true two-dimensional baselines, no Unicode-only faux superscripts |
| Scalable delimiters | parentheses/bars/curly braces enclosing full expression |
| Matrices and cases | column alignment, balanced row heights, brackets/cases |
| Aligned derivation | equality signs aligned, line spacing and labels |
| Long display / tiny terminal | horizontal scroll/pan/zoom or explicit source fallback without truncating meaning |
| Input rejection | unknown commands, recursive depth, oversized dimensions, missing glyph/font, malformed SVG, external references |

Representative formulas include the Gaussian integral, Euler product, a 3×3 matrix, a two-line aligned derivation, and a piecewise function. Preserve source and visual results as separate fixtures; a fitted or self-generated golden cannot be the only validation.

**Required controls:** baseline original source, independent renderer/viewer reference, deliberate mutant (swap script baselines / reverse fractional placements / corrupt a glyph / break resize), missing fonts, hostile math source, unsupported graphics, interrupted terminal writer, stale cached dimensions, repeated resize, focus and scroll. Source transcript must remain unchanged across terminal color modes even when output pixels differ.

## 7. Dependency map and scope control

- **v0.7:** input/lifecycle correctness, framebuffer adapter, installed consumers, release qualification; math research does not retroactively change its acceptance contract.
- **v0.8:** cross-language semantic parity plus M0 math/layout feasibility; no speculative ABI addition.
- **v0.9:** portability/stress, graphical terminal protocol qualification, optional experimental math node M1.
- **v1.0:** reviewed stable CORE; eligible math M2 if independently qualified, otherwise explicitly optional.
- **Related:** ROADMAP Phase 3 typography/layout and Phase 4 capability/accessibility, Phase 5 optional terminal graphics protocols, scientific instrumentation #125/#135/#136, frame/resource gates #129/#143. Do not duplicate their existing raster, glyph, accessibility or replay machinery.

**Next decisive experiment:** render the same nontrivial fraction+integral+matrix page with one offline typesetter, pass its bounded pixels through existing LibGibson scene/capture at 80×24, compare against a licensed independent reference, then display with a tested pixel-capable terminal and a cell-only terminal. If only the cell path works, math-type layout is observed but **paper-quality terminal rendering remains UNVERIFIED**.
