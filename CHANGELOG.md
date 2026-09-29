# Changelog

All notable changes to LibGibson are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
the 0.x Semantic Versioning rules defined in
[`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md) — pre-1.0, a breaking change
bumps the minor slot (`0.y`) and a compatible change bumps the patch slot (`0.y.z`).
The project may also deliberately elect a minor bump to mark a substantial additive
public subsystem or milestone (see the contract's "deliberate minor milestones"); 0.2.0
is such a milestone.

LibGibson is **engineering alpha**. Version 0.1.0 was released on GitHub on
2026-09-25 (Linux x86_64: source plus a native SDK archive). It is **not** published
to any package registry — no crates.io, PyPI, or Go module proxy upload.

## [Unreleased]

Work in progress on `feat/v0.4-humanmusic-audio` (draft PR #70) toward the intended 0.4.0
milestone. **Not released**: the package version stays **0.3.1**, `GIBSON_ABI_VERSION` stays
**1**, MSRV stays **1.85**, and nothing below is exposed through the C ABI.

### Added (experimental, Rust-only)

- **`gibson::audio`** — an offline/real-time audio substrate (sample-accurate time, buffers,
  a block renderer, WAV writer, DSP building blocks) with an optional `audio-cpal` output
  backend (feature-gated; no new mandatory dependencies).
- **HumanMusic** (`gibson::audio::human_music`) — a procedural composer driven by LibGibson's
  semantic style/story state, with three sonic worlds (BLACK_ICE, VAPOR95, SWISS_SIGNAL), no
  prerecorded musical assets, and a structural/causal diagnostics suite. Rounds I–VIII are
  documented in [docs/HUMAN_MUSIC_ARCHITECTURE.md](docs/HUMAN_MUSIC_ARCHITECTURE.md). Round VIIb
  made the performance causal: calls own their material and answers derive from it, one stage
  decides who plays before anybody plays, every event is stamped with the exact actions it
  performs, Modulate really changes the tonal region, the band shares one complexity budget,
  gestures vary their manifestation per cycle, semantic state deltas size the actions, quiet is
  declared, obligations settle named debts by deadline, pad and keys are voiced by a bounded
  voice-path DP, SFX sit in the local harmony, and a requested length is rendered exactly.
  The current round (VIII) makes the band hear the same chord: one vertical theory
  (`sonority.rs` — owned vs unowned minor 2nds/9ths, tension specs, bass function, a per-world
  colour budget, audible lifetimes, `EnsembleSonorityDiagnostics`), one harmonic ledger the
  players are realized against in rigidity order (`harmonic_state.rs`), a bass that never
  octave-copies an upper tension into the floor, keys lines placed against lead and bass, and the
  pad and keys voiced as one joint decision (`support.rs`). `EnsembleCoupling::Independent`
  keeps the Round VIIb realization as a pinned control. On the flagship, unowned minor 2nds/9ths
  drop from 24/28/43 to 0/0/0 per world with every action still witnessed.
- `examples/human_music_lab.rs` — renders the flagship (and A/B probes, stems, pitched-role pair
  stems, a neutral harmonic reference, calibration grammars) with every receipt printed next to
  the WAV paths.

### Fixed (experimental audio)

- The FM oscillator (`dsp::osc::FmOsc`) clamps its modulation index to the alias-free bound, so
  a high note no longer folds sidebands back under Nyquist as inharmonic partials (BLACK_ICE's
  lead climax carried one at −14.5 dB).

Perceptual quality is **unverified** until the maintainer's listening gate.

## [0.3.1] - 2026-09-28

**Temporal hardening + keyed compositing** — a compatible patch release. All work
is additive: the **experimental, Rust-only** `gibson::temporal` axis is extended and
hardened, and two research examples demonstrate keyed-video compositing through the
one existing renderer. **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); MSRV stays
1.85. The temporal feature remains **off by default** and its perceptual benefit is
still setup-dependent and **unproven** (see
[docs/TEMPORAL_DISPLAY_VALIDATION.md](docs/TEMPORAL_DISPLAY_VALIDATION.md)). No new
spatial resolution, no video-playback claim, and no source media is shipped.

### Added (experimental, Rust-only `gibson::temporal`)
- **Regional motion gating** (`set_motion_region`/`clear_motion`): a moving region is
  held static while settled regions may refine; re-activated cells reseed so no stale
  residual leaks in.
- **Incremental regional reprojection** (`set_target_region`): reprojects only the
  overlapping cells and, as of this release, maintains eligibility and every
  diagnostic aggregate **incrementally** — work proportional to the touched region,
  not the whole grid (fixes #68).
- **`advance_into(&mut Surface)`**: reuses the caller's `Surface` allocation, bit-for-bit
  identical to `advance`.
- **Opt-in transport-aware rate-distortion** (`set_transport`): a cell-level
  keep/change decision by a λ benefit with an optional per-frame dirty budget;
  mean-preserving. The default remains the per-dot residual.
- **Capability-resolved emitted-swing safety**, a **jitter gate**, **nondestructive**
  target eligibility, and **live reclassification** on policy/color-depth change;
  Mono is always static.
- Exact sRGB8→linear **LUT** (bit-identical to the transfer function).

### Added (research examples)
- **`temporal_observatory`**: a procedurally-lensed black-hole flagship whose acquire-
  lock state machine drives region motion gating.
- **`temporal_projector_bench`**: projector/runtime microbenchmarks.
- **`temporal_video_compositor`**: chroma-keys a greenscreen clip and composites the
  keyed subject over a designed, temporally-stabilized LibGibson background, exploiting
  the greenscreen's ~80% transparency to reproject only the subject's bbox. Offline
  ffmpeg bake into a cached raw-RGB film with a **source-identity cache fingerprint**
  and atomic writes; the runtime never touches the codec.
- **`libgibson_intro_reaction`**: a directed "reaction cut" that composites a keyed
  subject **into** the existing `libgibson_intro` film via a **surface-local**
  compositor — the base film is reused verbatim and stays byte-identical outside the
  subject — with a deterministic two-clock (edit vs. narrative) director. See
  [docs/TEMPORAL_VIDEO_COMPOSITING.md](docs/TEMPORAL_VIDEO_COMPOSITING.md) and
  [docs/FRANK_REACTION_CUT_PLAN.md](docs/FRANK_REACTION_CUT_PLAN.md).

### Fixed
- **#68**: `set_target_region` no longer performs an O(total-cells) reclassification on
  every call; the accounting is incremental with a bounded worst-swing fallback, proven
  cell-for-cell against the retained full-recompute oracle.

### Unchanged
- C ABI (`GIBSON_ABI_VERSION` = 1, 52/52 symbols), MSRV 1.85, the single rendering
  pipeline (all temporal output is an ordinary `Surface`). The `libgibson_intro`
  example is behaviorally unchanged.

## [0.3.0] - 2026-09-27

**Temporal Rendering Milestone** — a deliberate minor bump (see the release
contract's "deliberate minor milestones"): high-cadence frame-pacing and
observability infrastructure plus an **experimental, Rust-only** temporal cell
realization axis, all flowing through the one existing renderer. **No C ABI change**
(`GIBSON_ABI_VERSION` stays 1); MSRV stays 1.85. The temporal feature is **off by
default** and its perceptual benefit is setup-dependent and **unproven** — see
[docs/TEMPORAL_DISPLAY_VALIDATION.md](docs/TEMPORAL_DISPLAY_VALIDATION.md).

### Added (CORE)
- **`FramePacing`** (`#[non_exhaustive]`, #62): opt-in `PhaseLocked` frame pacing
  that anchors deadlines to the scheduled cadence and skips missed deadlines without
  drifting; `CompletionRelative` remains the default (`Context::set_frame_pacing`).
  Phase-locked missed-deadline observability via
  `FrameScheduler::missed_periods_last_frame`/`missed_periods_total` and the matching
  `Context` accessors.
- **`FrameReport`** (`#[non_exhaustive]`, #64): a Rust-side per-frame snapshot
  separating generation (layout/paint/diff/ANSI construction) from the blocking
  write/flush, with exact/affected/total cells, bytes and full-repaint
  (`Context::last_frame_report`). The ABI-v1 `RenderStats` layout is unchanged.

### Added (EXPERIMENTAL, Rust-only — `gibson::temporal`, #63)
- **`TemporalDisplayProcessor`**: static-first, safety-gated, cadence-aware temporal
  realization emitting ordinary `Surface`s (no second renderer). Default static;
  residual temporal luminance requires a caller-supplied measured `PresentationProfile`
  and passes a per-cell **emitted luminance-swing** check that freezes any cell whose
  real `|Y(fg)−Y(bg)|` exceeds the depth cap, with cadence hysteresis, reduced-motion
  and content-motion overrides.
- **Exact two-color subcell projector**: `project_rgb_subcells` (127 complement-pair
  representatives, bit-identical to a full scan) and the batch `project_braille_image`
  / `BrailleImageProjection`, with ideal and **emitted (post-quantization)** error.
- **`TemporalBrailleField`**: a sigma-delta subcell modulator with a bounded
  residual-over-static path, optional coherent-subsampling-robust dither, explicit
  reset/invalidation, and an authoritative static fallback.
- **`temporal_cadence_beacon`** and **`temporal_display_lab`** examples; the beacon
  logs emission telemetry (`--csv`, `--photodiode`) for the external presentation
  experiment.

### Changed
- `#61`: remaining single-character glyph realizations use `Glyph::from_char`
  (behaviour-preserving allocation cleanup).
- `FrameScheduler::max_fps` is now private; use `set_max_fps` (which resets the timing
  epoch) and the `max_fps()` getter. Breaking for direct field access; no in-tree
  consumer used the field.
- New public 0.3 types are sealed `#[non_exhaustive]` (`FramePacing`, `FrameReport`,
  `PresentationProfile`, `TemporalSafetyPolicy`, `TemporalGate`, `TemporalCellProjection`,
  `TemporalDiagnostics`) so future fields/variants are additive; `ResetPolicy` stays
  exhaustive.

### Documented
- Temporal display processor design; the GPU research contract with a probe + measured
  CPU projector baseline (no `wgpu` in core; GPU left honestly unmeasured); the
  validation report (evidence classes, the one physical experiment, the
  accessibility/safety contract). Reconciled the synchronized-updates policy in the
  release contract (requested, not negotiated; the detected capability is unused).

## [0.2.5] - 2026-09-27

Umbrella-cleanup patch — the sixth release in the v0.2.x train, from the Europa
v0.2.0 consumer campaign. Resolves the last open findings of the Palimpsest
ergonomics umbrella (#48): a `&self` render-stats read (E-09), documentation of the
remaining small sharp edges (E-09), and the experimental-layer pointer for
raw-`Node` consumers (E-04). **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); the
one code change is an additive, source-compatible receiver relaxation
(`&mut self` → `&self`) with no CORE break. `gibson::ui` stays EXPERIMENTAL and
Rust-only.

### Added
- **`Context::stats()` reads through `&self`** (#48 E-09): render statistics can now
  be read through a shared borrow rather than requiring `&mut`. The renderer's
  absolute counters are already mirrored into the scheduler on every render/commit
  path, so the read overlays them onto a copy of the accumulated stats and returns by
  value without mutating the context. Source-compatible — existing `&mut`-receiver
  call sites continue to compile unchanged.

### Documented
- **Kind-specific builders** (#48 E-09): `Node::background` and `Node::scroll_offset`
  now document that they apply only to their supported node kinds (box/bordered, and
  text-input, respectively) and are a silent no-op on any other kind, with the
  supported alternative for giving text a background. A mis-attached doc comment on
  `scroll_offset` (it described `rich_text`) is corrected.
- **Headless color depth** (#48 E-09): `Context::headless` documents that it defaults
  to truecolor — unlike an env-detected real terminal — and points at
  `set_color_depth` / `set_capabilities` to reproduce a lower-depth terminal in a
  capture. The default is intentionally left unchanged to keep captures stable.
- **Narrow panel titles** (#48 E-09): `Node::panel` documents the ≥5-column threshold
  below which no title is drawn, and that an over-long title is truncated without an
  ellipsis above it.
- **Experimental ergonomic layer** (#48 E-04): `docs/UI_LAYER.md` adds a pointer for
  raw-`Node` consumers — keyed focus, lists/tables, design skins and finite motion
  already exist ergonomically in the experimental `gibson::ui` layer — with the
  "experimental, may change" framing and guidance on composing the two paths.

## [0.2.4] - 2026-09-27

Viewport and focus foundations patch — the fifth release in the v0.2.x train, from
the Europa v0.2.0 consumer campaign (#39, #40; #42 resolved by documentation). Adds
the item-oriented viewport windowing and incremental focus-ring eviction that
scrollable, filterable list UIs otherwise re-derive by hand. **No C ABI change**
(`GIBSON_ABI_VERSION` stays 1); every change is an additive Rust method — no CORE
break, no signature change, no new struct fields — and `ViewportState` / `FocusRing`
are Rust-only (never on the C-ABI surface).

### Added
- **Viewport windowing helpers** (#39): `ViewportState::visible_range(total_items,
  item_height, view_height) -> Range<usize>` returns the item indices at least
  partially on screen (edge-clipped items included); `ensure_visible(index,
  item_height, view_height)` scrolls only as far as needed to reveal an item (no-op
  when already visible); `scroll_to_item(index, item_height)` pins an item to the
  top. Zero-height inputs are guarded (no divide-by-zero); like the existing scroll
  methods these do not self-clamp against the content end, so follow with `clamp`
  where that invariant is needed.
- **Incremental focus eviction** (#40): `FocusRing::remove(index) -> Option<FocusId>`
  removes a member (e.g. a list item filtered out), shifts later members down, and
  transfers focus to a surviving neighbour (the next survivor, or the new last), so
  focus is never silently lost. Outstanding `capture` entries are reindexed the same
  way, so a modal opened before the removal still restores the correct widget on
  `release`. Replaces the rebuild-the-whole-ring workaround.

### Documented
- **Responsive layout across size classes** (#42): `docs/UI_LAYER.md` now documents
  that size classes are a view concern — branch on `BuildCx.environment.width` and
  share `.key(...)` across variants; keyed `UiRuntime` reconciliation preserves focus
  and in-flight motion across the per-frame rebuild, so a resize is not special-cased
  and not expensive. `row().responsive(below)` remains the shortcut for the common
  row→column collapse. No "breakpoint framework" is added: how many size classes
  exist and where they break are application policy, not layout mechanics.

## [0.2.3] - 2026-09-27

Layout and custom-graphics-seam patch — the fourth release in the v0.2.x train,
from the Europa v0.2.0 consumer campaign (#36, #43, #48 E-05/E-06). Closes the
"size-blind Surface" gap: custom graphics can now be built at the size the layout
engine assigns, composited through the one existing rendering pipeline. **No C ABI
change** (`GIBSON_ABI_VERSION` stays 1); every change is additive with no CORE
break. `gibson::ui` stays EXPERIMENTAL and Rust-only.

### Added
- **Size-aware deferred canvas** (#43): `Node::canvas(|rect| -> Surface)` builds
  its surface **after** layout, receiving the node's resolved cell rectangle, so
  consumers no longer replicate Taffy's math to guess a size before layout. Sized
  by ordinary layout builders (`flex_grow` / `width` / `percent_width`); composited
  and clipped exactly like a raster. The callback never runs during layout, so
  there is no layout↔paint feedback loop and no second renderer.
- **Percentage sizing on `Element<A>`** (#36): `.percent_width(f32)` /
  `.percent_height(f32)`, lowering to Taffy `Dimension::Percent` — the semantic
  counterpart of `Node::percent_width`, for proportional split panes.
- **Clipped sub-surface view** (#48 E-06): `Surface::clip(Rect) -> SurfaceClip`,
  a translated `&mut` handle whose `set_cell` / `print_str` use clip-local
  coordinates and drop out-of-bounds writes — no per-call bounds guard, wide-glyph
  invariants preserved.
- **Braille compositing helpers** (#43): `BrailleCanvas::blit_to_surface(dst,
  style)` overlays dots onto a surface while preserving each cell's background
  (vector geometry over a shaded raster), and `Glyph::from_char(char)` builds a
  glyph without the `String` allocation of `Glyph::new(&c.to_string())`.

### Documented
- **`Node::surface` flex behavior** (#48 E-05): documented that the preset
  `Dimension::Length` overrides flex allocation (a full-width raster can silently
  push a sibling off-screen) and how to make a raster elastic; `Node::canvas` is
  the size-aware alternative.
- **Dense / canvas UIs** (#48 E-03): a crate-doc note on the "`Node` for chrome,
  `Surface` / `canvas` for the drawing" composition seam.

## [0.2.2] - 2026-09-27

Output, testability, and observability patch — the third release in the v0.2.x
train, from the Europa v0.2.0 consumer campaign (#45, #47, #48 E-02, E-07). Makes
finalized scrollback output intentional, the composed frame inspectable, and the
renderer's exact changed-cell count observable. **No C ABI change**
(`GIBSON_ABI_VERSION` stays 1); every change is additive with no CORE break.
`gibson::ui` remains EXPERIMENTAL and Rust-only.

### Added
- **Wrap-policy-aware scrollback insertion** (#45):
  `Context::commit_text_with_mode(text, WrapMode)` and
  `Context::insert_text_before_live_with_mode(text, WrapMode)` (plus the
  `Renderer` equivalents). `WrapMode::NoWrap` preserves preformatted content —
  aligned tables, diffs, ledgers — at its natural column width instead of
  re-flowing (`WordWrap`, the previous hard-coded behavior) or silently clipping
  wide rows. A preformatted line wider than the terminal is inserted via the
  always-correct repaint path so the live-region anchor never drifts. Control
  characters stay neutralized at the cell-model boundary, so untrusted text still
  cannot inject terminal controls. The existing `commit_text` /
  `insert_text_before_live` are unchanged (word-wrap) and source-compatible.
- **Visible-frame snapshot** (#48 E-02): `Context::last_frame_lines() -> Vec<String>`
  (with `Renderer::last_frame_lines` and `Surface::to_visible_lines`) returns the
  composed live frame as visible text — wide-glyph continuation cells collapsed,
  trailing blanks trimmed — so headless tests can assert on what the screen says
  without building a private terminal emulator. Raw wire bytes remain available
  via `rendered_bytes` / `take_output`.
- **Exact changed-cell observability** (#47): `Context::last_exact_changed_cells()
  -> usize` exposes the renderer's precomputed
  `SurfaceDiff::exact_changed_cell_count()` for the last frame — the exact
  semantic delta, distinct from the logical affected footprint
  (`RenderStats::dirty_cells`) and emitted bytes — with no second
  layout/paint/diff pass in the consumer. Rust-only; the ABI-1 stats struct is
  unchanged.

### Documented
- **Canonical interactive loop** (#48 E-07): `Context::run_once` now carries a
  copy-paste canonical loop (poll → rebuild-on-change → frame-paced render, with
  the non-TTY guard from #46), and `Event::Tick` documents that it is **not**
  emitted by the core input-driven loop — animation is paced via
  `animation_interval` + `render_if_due`, or the `gibson::ui::App` helper delivers
  per-iteration `AppEvent::Tick`.

## [0.2.1] - 2026-09-27

Consumer-correctness and honest-failure patch, driven by the Europa v0.2.0
external-consumer campaign (issues #35, #37, #44, #46, #48 E-01). Fixes narrow
`gibson::ui` interaction defects and makes three silent failure modes
observable. **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); every change is a
bug fix or additive with no CORE source break. `gibson::ui` remains EXPERIMENTAL
and Rust-only.

### Fixed
- **`Alt`+`<char>` chords are no longer consumed as text input** (#35). A focused
  `text_input` / `TextInputState` treated `Alt`-modified characters as printable
  text — inserting a literal character and marking the event consumed — so an
  application's global `Alt` accelerators never fired while an input was focused.
  `Alt` chords are now left uninserted and unconsumed at both the
  `TextInputState` and `UiRuntime::handle_event` layers, so outer hotkeys receive
  them. Plain and `Shift`-modified text and the supported `Ctrl-a/e/u/k` line
  editors are unchanged; unsupported chords remain routable.
- **A stable identity key no longer forces an entrance animation** (#37).
  `UiRuntime` attached `MotionRole::Enter` to every `Key::Named` element, which
  under the `BLACK_ICE` acquisition grammar dissolved non-focused controls to
  blank cells on their first frame — making keyed controls invisible in headless
  captures, tests, and at `t = 0`. Entrance and reconciliation motion is now
  opt-in via `.motion(role)` (or a modal); an identity key no longer implies
  animation. Explicitly requested motion is unchanged.

### Added
- **`Node::separator(style)`** (#44): an untitled horizontal rule that needs no
  turbofish, unlike `Node::rule(None::<String>, style)`, whose generic
  `Option<impl Into<String>>` parameter cannot infer its payload type from a bare
  `None` (E0283). `Node::rule` is unchanged and remains source-compatible.
- **`Context::is_interactive(&self) -> bool`** (#46): reports whether the context
  drives a live TTY that is not a headless capture. An interactive loop built on
  `render` + `poll_event` renders nothing and receives no input on a non-TTY
  stdout; consumers can now detect that and fall back to a headless/`--dump` path
  (or exit with a diagnostic) instead of idling on a blank screen.
- **`Context::empty_root_renders(&self) -> u64`** (#48 E-01): counts `render` /
  `render_now` calls issued with no root set — the common "forgot `set_root`"
  mistake, otherwise a silent no-op indistinguishable from a legitimate empty
  frame. A root-backed render (even of an empty tree) never increments it.
  Rust-only; the ABI-1 stats struct is unchanged.

## [0.2.0] - 2026-09-26

The **experimental semantic UI milestone**. Adds `gibson::ui`, a Rust-only
composition layer above the existing framebuffer / renderer / terminal substrate. The
lower-level architecture is unchanged: UI trees lower to ordinary
`Node`/Taffy/`Surface`/`SurfaceFx`/`Focus`/`Context` objects through the one existing
rendering pipeline. **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); no language
wrapper, renderer, layout engine, or terminal owner was added. The minor bump marks
the size of the new public Rust surface and the project milestone, not an ABI break or
a stability promise (see [`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md) §1).

### Added
- **Experimental `gibson::ui` semantic layer** (Rust-only, EXPERIMENTAL): an
  `Element<A>` component tree with an ordinary-Rust builder API (`panel`, `status`,
  `progress`, `button`, `choice`, `text_input`, `modal`, `toast`, `table`, `viewport`,
  `section`, `sparkline`, …), lowering to ordinary `Node` trees through an inspectable
  `Compiled<A>` boundary (`compile`, `compile_presented`, `UiRuntime::frame`).
- **Three built-in design grammars** (`Skin`): `VAPOR95` (windowed workstation),
  `BLACK_ICE` (compact instrument rails), `SWISS_SIGNAL` (editorial / Bauhaus). Each is
  a structural grammar (`Chrome::Window | Rail | Editorial`) that stays distinct in
  Mono and low color depths, not merely a repalette; all tokens are public for local
  customization, and the existing `Theme` gains no new semantics.
- **Typed interaction sidecar** (`InteractionMap<A>`): typed actions routed alongside
  the lowered `Node`, with an `EventOutcome<A>` consumed/actions dispatch contract.
- **Stable-key focus and presentation continuity** (`UiRuntime`): explicit named keys
  survive reorder / responsive reflow / same-key replacement / modal transitions /
  skin switching; focus capture and restoration across nested and sibling modals;
  duplicate keys are rejected transactionally before any state changes. Retained state
  is bounded by live semantic state — no frame history, no exit ghosts.
- **`BuildCx` / `PresentationCx` phase separation**: the construction-time environment
  (`BuildCx`, no focus/motion) is distinct from reconciled current-frame presentation
  (`PresentationCx`); `presented(...)` custom nodes run after reconciliation and observe
  current-frame focus/motion while returning ordinary `Node`s.
- **Finite semantic motion**: deterministic `elapsed → SurfaceFx` plans that settle to
  exactly nothing (settled frames emit zero bytes), with `MotionPreference`
  Full / Reduced / None, lowered onto the existing `SurfaceFx` operations.
- **Controlled text-input routing**: a focused editor owns editor-class input
  (printable / paste / Backspace / Delete / Left / Right / Home / End and the
  Ctrl-a/e/u/k line editors) even without an `on_edit` callback; the application owns
  all editor and domain state; unsupported chords remain routable.
- **Escape hatches** into the substrate remain local and explicit — `raw(Node)`,
  `surface(Arc<Surface>)`, `raster(Surface)`, `presented(...)`, a custom `Component`,
  and Scene / `SurfaceFx` integration — with no second rendering pipeline.
- **`ui_showcase` and headless visual proving grounds**: a deterministic operations
  workspace under all skins with `--dump` / `--transition` / `--at-ms` / `--profile`,
  plus automated skin × size × color-depth matrices and structural Mono goldens.

### Boundaries
- `gibson::ui` is **EXPERIMENTAL** and **Rust-only**: no C ABI surface and no
  C/C++/Python/Go bindings; it may change or be removed between minor releases and is
  not promoted to CORE by the move to 0.2.0.
- No mouse / hover router, no list virtualization, no exit-ghost retention, no async
  framework machinery.
- **Not a production-stability, API-finality, or universal-portability claim.** Existing
  platform limitations are unchanged: the core engine is implemented and tested on
  Linux x86_64 only, and the crossterm 0.29 resize/input-readiness collision (issue #15)
  still applies. Visual and hostile review to date is implementation-agent evidence, not
  independent human usability research.

## [0.1.1] - 2026-09-26

Additive, backwards-compatible fixes surfaced by external consumers testing the
v0.1.0 SDK. **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); a consumer pinned
`libgibson = "0.1"` (`>=0.1.0, <0.2.0`) upgrades to this automatically.

### Added
- **Headless output capture** (issue #27): a context created with
  `Context::headless(...)` now renders into an in-memory buffer instead of the
  process stdout, so callers can read the exact rendered bytes back for
  snapshot / assertion testing without escape codes reaching a real terminal.
  New `Context::rendered_bytes() -> &[u8]` and `Context::take_output() -> String`;
  interactive contexts are unchanged.
- **`RenderMode` re-exported from the `context` module** (issue #28): so
  `use gibson::context::RenderMode;` resolves next to the `Context` methods that
  consume it, in addition to the existing crate-root `gibson::RenderMode`.

### Fixed
- `Context::headless` advertised itself for "automation, snapshotting and tests"
  but wrote rendered frames straight to the real process stdout, making output
  assertions impossible (issue #27).

## [0.1.0] - 2026-09-25

The first tagged release — **Engineering Alpha**, Linux x86_64. LibGibson is a
cell-framebuffer terminal UI engine: a declarative `Node`/Flexbox layout over a
grapheme-aware 2D cell surface, a differential ANSI compiler, and the flagship
**immutable-scrollback / mutable-live-region** model with an explicit physical
anchor. It exposes a language-neutral **C ABI v1** (ELF SONAME `libgibson.so.1`)
with C/C++/Python/Go wrappers, a color-capability ladder
(TrueColor → ANSI256 → ANSI16 → Mono) and an orthogonal glyph-realization ladder
(Braille → HalfBlock → Block → ASCII), plus the experimental Scene/Story and
software-graphics stack and the `libgibson_intro` short film (with its
first-contact prologue). **MSRV Rust 1.85**; dual-licensed **MIT OR Apache-2.0**.

Distribution is GitHub source plus a Linux x86_64 native SDK archive attached to
the release; it is **not** published to crates.io, PyPI, or the Go module proxy.

Known limitation: under a resize/input-readiness collision in crossterm 0.29, a
key can be briefly queued until a later key releases it (issue #15; upstream fix
filed as [crossterm#1128](https://github.com/crossterm-rs/crossterm/pull/1128),
not vendored). This is documented, not a claim of lossless delivery under that
collision.

### Added
- Versioned native shared object: the Linux `cdylib` now carries an ELF **SONAME
  `libgibson.so.1`** (stamped by a new `build.rs` via `-Wl,-soname`, guarded to
  Linux and scoped to the cdylib), whose major tracks `GIBSON_ABI_VERSION`. The
  staged SDK installs `libgibson.so.1` plus a `libgibson.so → libgibson.so.1`
  development symlink, so a dynamic consumer linked through `-lgibson` records
  `NEEDED = libgibson.so.1`. `libgibson.a` and pkg-config are unaffected.
- Pre-1.0 release contract (`docs/RELEASE_CONTRACT.md`): Rust API stability tiers
  (CORE vs experimental modules), the C ABI v1 policy and its independence from the
  package version, the wrapper ABI-compatibility policy, the MSRV floor with its
  evidence, and the supported-platform matrix.
- Declared MSRV: **Rust 1.85** (`rust-version` in `Cargo.toml`), established
  empirically and enforced by a scoped CI check.
- Relocatable native SDK staging (`scripts/release/stage-sdk.sh`): stages headers,
  shared/static libraries, relocatable `pkg-config` metadata, and license/doc files
  into a prefix with a deterministic checksum manifest.
- Canonical installable C++ header at `include/gibson.hpp`.
- C ABI v1 exported-symbol baseline (`abi/gibson-abi-v1.symbols`) and a regression
  check that guards against symbols silently disappearing or changing.
- C++ ABI gate: `gibson::Context`'s constructor now verifies
  `gibson_abi_version() == GIBSON_ABI_VERSION` before any other ABI call and throws
  `std::runtime_error` on mismatch (matching the Python/Go load-time checks); a new
  `gibson::Context::abi_compatible()` exposes the predicate.
- Declared-range consumer MSRV experiment (`scripts/release/msrv-consumer.sh`): a
  fresh, unlocked resolution of the declared dependency ranges on Rust 1.85, wired
  into the CI MSRV job and the release preflight so a dependency upgrade cannot
  silently raise the effective MSRV.
- Distributable dual-license payloads: `LICENSE`, `LICENSE-MIT`, and `LICENSE-APACHE`
  now ship in the Go module root and in the Python wheel/sdist; a
  `check-versions.sh` guard keeps them byte-identical to the repository-root
  originals.
- Sub-cell **glyph realization** layer (`src/glyph.rs`): `SubcellGlyphMode`
  (`Braille2x4`/`HalfBlock1x2`/`Block`/`Ascii`) realizes a 2×4 dot mask through
  progressively more portable glyph families; `transcode_surface_glyphs` is the
  generic chokepoint (a Braille glyph losslessly encodes its mask, so one pass
  re-realizes a whole surface and `Braille2x4` is a no-op — the hero path is
  byte-identical). `detect_glyph_mode` resolves `--glyphs=` / `LIBGIBSON_GLYPHS`
  / `Auto(TERM)` with a console-safe default on `TERM=linux`. Glyph realization
  is a distinct capability axis from terminal protocol/color and adds no C ABI
  surface (`GIBSON_ABI_VERSION` unchanged). See [`docs/GLYPHS.md`](docs/GLYPHS.md).
- `BrailleCanvas` gains `mask_at`, `glyph_at_mode`, `to_lines_mode`, and
  `to_surface_mode`; `glyph_at` now delegates to `glyph_at_mode(.., Braille2x4)`
  (behavior identical for all 256 masks).
- `glyph_capability_lab` example: a visual probe rendering each glyph family and
  one sub-cell field realized four ways, stating that font-repertoire realization
  requires visual inspection (documents the Linux VT observation).
- `libgibson_intro`, `fx_lab` (new "Glyph ladder" scene), and `runtime_observatory`
  (live sparklines) honor the glyph mode; deterministic dumps stay Braille.
- **First-contact prologue** for `libgibson_intro`: the default experience opens
  in an ordinary terminal and boots a credible — explicitly *simulated* — agent
  harness into immutable scrollback, then that information visibly *acquires
  structure*. A live region attaches while the boot log is still streaming (the
  two coexist), and the Harness framing assembles progressively: a header rail,
  the objective, the orchestration frame, then the four worker slots arriving one
  at a time in correspondence with their boot receipts, each resolving from a
  skeletal placeholder into a full agent record. ARCHITECT ignites to "active"
  only when the plan begins executing, its progress bar settling at exactly the
  film's `t = 0` value (an ignited head, no work done). After a short readable
  hold, terminal ownership hands cleanly to the fullscreen film at that same
  `t = 0` — a match cut into the existing 72-second timeline (unchanged; the boot
  log persists in scrollback). Layout is height-aware (compact on short
  terminals) and the assembly is monotonic (the live region never shrinks).
  `--no-prologue`/`--stage`/`--at`/`--dump` bypass it. Deterministic prelude clock;
  choreography witnesses plus a Unix PTY test cover the boot → assembly → handoff
  → film → restore arc.

### Changed
- The `transaction` module (`TerminalTransaction`, internal renderer plumbing) is
  now `pub(crate)` — it is no longer part of the public Rust API. It had no external
  consumers and was never re-exported; privatizing it before the first tag avoids a
  later `0.y` break. See `docs/RELEASE_CONTRACT.md` §2.
- The extension-point public enums are now `#[non_exhaustive]` (decided before the
  first release, since adding it later is itself source-breaking): `node::NodeKind`,
  `input::Event`, `input::KeyCode`, and `glyph::SubcellGlyphMode`. Adding a variant
  to these is a non-breaking `0.1.z` change; downstream `match`es must carry a `_`
  arm. `capability::Capability`/`ColorDepth` and `glyph::GlyphChoice` are left
  exhaustive (closed value sets). No `#[repr]`/C-ABI change (`GIBSON_ABI_VERSION`
  unchanged). See `docs/RELEASE_CONTRACT.md` §2.
- The published Rust crate now **excludes** the engineering-demo corpus (examples,
  integration tests and goldens, internal `docs/`, dev scripts, and the language
  bindings). It ships the library, the C header (for native-library builders), and
  the license/readme/design docs. Packaged manifest: 53 files (the demo corpus is
  excluded).
- The C++ wrapper header moved to `include/gibson.hpp`; `bindings/cpp/gibson.hpp` is
  now a forwarding shim so existing in-repository consumers keep working.
- The release preflight now **fails** (was a warning) when `THIRD-PARTY-NOTICES.md`
  differs from freshly generated output — a release candidate must not ship stale
  dependency notices.
- The Go clean-room consumer is now built from a module copy staged **outside** the
  checkout (no repository-relative `replace`), with `GOPROXY=off`, asserting no
  checkout path in `go.mod` and scanning the built Go binary for the checkout path —
  proving the Go wrapper has no dependency on repository-relative layout.
- `stage-sdk.sh --force` path guard hardened: it now normalizes the prefix and
  refuses catastrophic recursive-remove targets including system-path subpaths
  (`/etc/*`, `/lib/*`, `/bin/*`, `/boot/*`, …) and `$HOME` itself.

### Removed
- Unused `thiserror` dependency (it was declared but never used in `src/`).

---

The contents and exact cut procedure for a release are documented in
[`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md) and
[`docs/RELEASING.md`](docs/RELEASING.md). Ecosystem-registry publication
(crates.io, PyPI, Go module proxy) remains a separate, later decision.

[Unreleased]: https://github.com/femboy2112/libgibson/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/femboy2112/libgibson/releases/tag/v0.3.1
[0.3.0]: https://github.com/femboy2112/libgibson/releases/tag/v0.3.0
[0.2.5]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.5
[0.2.4]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.4
[0.2.3]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.3
[0.2.2]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.2
[0.2.1]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.1
[0.2.0]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.0
[0.1.1]: https://github.com/femboy2112/libgibson/releases/tag/v0.1.1
[0.1.0]: https://github.com/femboy2112/libgibson/releases/tag/v0.1.0
