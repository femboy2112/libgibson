# LibGibson feature catalog

The full enumeration of implemented capabilities. Everything below is marked
**TESTED on Linux x86_64** unless noted otherwise; `EXPERIMENTAL, Rust-only`
marks the UI/Scene/Story/FX surfaces that are outside the stable core and the C ABI.
For the architecture and the core/experimental boundary see
[`STATE_OF_LIBGIBSON.md`](STATE_OF_LIBGIBSON.md) and the API stability tiers in
[`RELEASE_CONTRACT.md`](RELEASE_CONTRACT.md). For a tour rather than a catalog,
start from the [README](../README.md).

## Cell model & text

- **Grapheme-aware cell model** (TESTED): Unicode grapheme clusters
  (`unicode-segmentation`), CJK full-width characters (`display_width == 2`),
  zero-width combining marks, and emojis.
- **Wide glyph overwrite protection** (TESTED): writing into a cell occupied by or
  adjacent to a wide character safely clears orphaned continuation cells.
- **Structured `RichText` / `Line` / `Span` + semantic `Theme::styles()`** (TESTED):
  style roles (`text`, `muted`, `faint`, `accent`, `success`, `warning`, `error`,
  `border`, `rail`, `code`, `link`, `selection`) instead of hardcoded ANSI. `muted`
  is default foreground + dim, readable on light and dark terminals.
  `Theme::no_color()` is available.
- **Grapheme-based `TextInput`** (TESTED): single-line input with display-width-aware
  navigation, horizontal scrolling, and bracketed paste. Mutations are byte-range
  edits and the cursor grapheme index is re-derived from the full resulting string, so
  boundary-merging insertions (combining accents, ZWJ emoji, skin-tone modifiers,
  flags) preserve `cursor_grapheme <= grapheme_count`. Single-line paste normalizes
  `\r\n`, `\r`, and `\n` to a space.
- **Static structured output** (TESTED): `commit_text`, `commit_rich_text`, and
  `commit_node` route through the same width-aware layout/wrapping engine as live
  nodes. Control characters in text are neutralized at the cell-model boundary, so they
  cannot inject `ESC`/`OSC`/`CSI`. `commit_raw_ansi_unchecked` is the explicit escape
  hatch.
- **Clean plain-text degradation** (TESTED): detects redirected output and suppresses
  interactive escapes while emitting clean plain text with zero escape sequences. The
  internal ANSI stripper is for engine-generated output only, **not** a sanitizer for
  untrusted input.

## Rendering pipeline

- **Explicit physical anchor** (TESTED): the renderer tracks `AnchorState` plus last
  cursor position/visibility; empty cell diffs still emit when cursor state changes.
  Geometry changes invalidate the anchor and trigger a re-anchor.
- **Stateful differential ANSI compiler** (TESTED): groups dirty cells into contiguous
  runs, computes minimum-distance cursor repositioning, uses `CSI K` when content
  shrinks, and wraps diff emission in autowrap disabling (`CSI ? 7 l/h`).
  Synchronized-update (`CSI ? 2026 h/l`) ownership lives in `TerminalTransaction`, not
  the compiler.
- **Atomic `TerminalTransaction`** (TESTED): batches sync-update markers, temporary
  private modes, cursor motion, framebuffer diff, final cursor placement, and cursor
  visibility into one `write_all` + `flush`.
- **Asynchronous scrollback insertion** (TESTED): the **safe** entry points are
  `insert_text_before_live`, `insert_rich_text_before_live` and
  `insert_node_before_live`; the only raw path is explicitly
  `insert_raw_lines_before_live_unchecked`. Two named strategies —
  `InsertLineFastPath` uses `CSI L` to insert history above the live region without
  repainting it, and `RepaintFallback` erases, prints, repaints, and restores the exact
  cursor. Strategy counts are exposed in metrics. This is **not** universally
  zero-repaint: the fallback repaints when the anchor is untrustworthy or space is
  insufficient.
- **Damage inspection** (TESTED): logical damage
  (`SurfaceDiff::logical_dirty_count`/`logical_dirty_cells`, including erase-to-EOL and
  cleared rows) plus explicit run counts and independently measured wire bytes;
  `Renderer::capture_damage`/`Context::last_dirty_cells()` expose per-frame logical
  coordinates for debug overlays.
- **Honest damage accounting** (TESTED): `SurfaceDiff` exposes
  `exact_changed_cell_count` (a true per-cell state delta) and `affected_cell_count`
  (cells *addressed* by update semantics — explicit runs ∪ erase-to-EOL ∪ cleared rows,
  which may exceed the live area when rows are removed).

## Layout & scene composition

- **Taffy-powered Flexbox layout** (TESTED): flex containers (`Row`, `Column`),
  percentage width, min/max constraints, padding, gap, alignment, justification, and
  intrinsic text measurement with word wrapping.
- **Layer compositor** (TESTED): `Node::stack()` overlays children in one content box
  with **explicit** cell transparency (`Cell::transparent`). Transparent cells leave
  the lower layer untouched; `Node::dim()` is a style-only veil. Overlay removal is
  diff-driven and leaves no ghost cells; a floating modal does not reflow the layout
  beneath it. `Stack`/`Dim` have C ABI constructors (`gibson_node_stack`,
  `gibson_node_dim`).
- **Scene composition** (TESTED): `Node::offset(x,y)` positions a node absolutely
  inside its parent (out of flow) with signed, clipped placement;
  `Node::viewport(cam_x,cam_y)` is a clipped camera;
  `Node::raster(Surface)`/`Node::surface(Arc<Surface>)` embeds a prebuilt surface.
  `ViewportState` owns pan/page/home/end + clamping.
- **Minimal focus** (TESTED): `FocusId`/`FocusRing` cycle (Tab/Shift-Tab), capture on
  modal open and restore on close, without a DOM/event-router.
- **Clip containment** (TESTED): `blit_transparent_clipped` places a width-2 glyph only
  when lead **and** continuation lie inside the clip; otherwise it is suppressed, so
  wide glyphs cannot leak a continuation past a raster node, viewport or positioned
  layer.

## Sub-cell graphics & glyphs

- **Sub-cell canvases** (TESTED): `BrailleCanvas` (2×4 dots/cell) and `HalfBlockCanvas`
  (1 horizontal × 2 vertical RGB samples per cell, so the grid is `width` ×
  `2*height` pixels, via `▀`), with Bresenham lines/polylines and exact glyph/colour
  tests. No graphics protocol required.
- **Sub-cell glyph realization** (TESTED): `SubcellGlyphMode`
  (`Braille2x4` → `HalfBlock1x2` → `Block` → `Ascii`) realizes the same 2×4 dot mask
  through progressively more portable glyph families, because glyph *realization* is a
  distinct axis from terminal *protocol* capability — a UTF-8 terminal need not carry
  the Braille block (as the Linux kernel VT demonstrates). `transcode_surface_glyphs`
  re-realizes any Braille surface in one pass (Braille is byte-identical, so the hero
  path never changes); `--glyphs=auto|braille|halfblock|block|ascii` / `LIBGIBSON_GLYPHS`
  override, with a console-safe `Auto` default on `TERM=linux`. Orthogonal to color; no
  C ABI change. See [`GLYPHS.md`](GLYPHS.md) and `cargo run --example glyph_capability_lab`.
- **Bounded 2D line clipping** (TESTED): `clip_line_to_bounds` runs Liang–Barsky before
  Bresenham in both sub-cell canvases, so a finite near-camera projection with
  coordinates in the tens of millions draws only its visible portion instead of walking
  millions of steps, and near-`i32`-extreme endpoints cannot overflow.
- **Visual composition helpers** (TESTED): `gibson::show` provides gradient spans,
  sub-cell progress meters (`▏▎▍▌▋▊▉█`), sparklines (`▁▂▃▄▅▆▇█`) and deterministic hex
  dumps as plain `Span`s/`Line`s — no widgets and no new layout semantics. `Node::panel`
  adds titled bordered containers and `Color::lerp` enables gradients. All degrade to
  zero color under `--no-color`.
- **Deterministic clock** (TESTED): `TimeSource::{real,fixed}` + `FixedStepClock` and a
  small motion toolkit (`phase`, `pulse`, `saw`, `triangle`, easings). Scripted frames
  are `frame * step`, enabling reproducible goldens.
- **FX substrate** (TESTED): deterministic `geom` 3D wireframe projector
  (cube/octahedron/torus/box/grid/data-tower, inclusive near-plane clipping, per-edge
  depth), seeded `particles` (radial / life-variance / directional bursts), procedural
  `field` (plasma/interference + heat ramp + Bayer-dithered mono fallback),
  grapheme-safe `transition` helpers (type-on/dissolve/scramble), and safe `glitch`
  primitives (row shift/tear/invert/scramble) that mutate only cell content.

## Capability & color

- **Capability & color ladder** (TESTED): `TerminalCapabilities` with tri-state
  `Capability` values and `ColorDepth` (TrueColor/Ansi256/Ansi16/Mono). A central
  quantizer makes the same UI degrade cleanly; `Unknown` is never treated as supported
  for `CSI L` insertion.

## Runtime & lifecycle

- **Scheduler / runtime** (TESTED as a module): `DEFAULT_ANIMATION_INTERVAL = 80ms`
  (60 FPS is an input-latency ceiling, not a spinner target). `Context::run_once(max_wait)`
  waits up to the next frame deadline, gives input priority, then renders if due.
  `render_if_due`, `request_render`, `animation_interval`, and `frame_budget` are
  available.
- **Safe terminal lifecycle** (TESTED): the RAII guard restores raw mode, cursor
  visibility, alternate buffer, and bracketed paste on normal exit and on error.
  LibGibson's installed panic hook adds best-effort restoration for an unhandled panic
  **on the terminal-owning thread**; a recoverable panic on a *non-owner* worker thread
  deliberately does **not** tear down the owner's terminal (panic restoration is
  owner-thread-scoped), and a pre-existing host panic hook is chained.

## FFI / C ABI

- **Language-neutral rich-text ABI** (TESTED for C/C++/Python; Go compiles, rich-text
  runtime coverage pending): opaque `gibson_line_t` / `gibson_rich_text_t` with
  span/align builders. No wrapping logic is duplicated outside Rust.
- **Versioned C ABI** (TESTED): `GIBSON_ABI_VERSION = 1`, `gibson_abi_version()`,
  `gibson_stats_init()`. Enum-like inputs cross as raw `int32` and are validated.
  `gibson_get_stats` validates the ABI version and refuses an undersized buffer instead
  of overflowing it. The Linux shared object carries ELF SONAME `libgibson.so.1`.

## Experimental (Rust-only)

- **Semantic UI composition** (EXPERIMENTAL, Rust-only; branch validation in
  [UI_LAYER_VALIDATION.md](UI_LAYER_VALIDATION.md)): `gibson::ui` lowers
  `Element<Action>` trees to inspectable ordinary Nodes plus typed interaction
  and key metadata. `App` is optional; `UiRuntime` works with caller-owned
  Context loops. Controlled editor state, keyed focus continuity, modal
  capture/restoration and finite semantic motion remain separate from business
  state. Vapor95, BLACK_ICE and SWISS_SIGNAL layer structural design grammars
  above existing Theme palettes, including capability-aware Mono treatment.
  Local Node/Surface/custom-component/Scene/SurfaceFx routes remain open.
  No exit ghosts, mouse router, virtualization or foreign ABI is added. See
  [UI_LAYER.md](UI_LAYER.md); this module is not included in the v0.1.1 tag.
- **Scene algebra** (TESTED, EXPERIMENTAL, Rust-only): `Scene`/`SceneEntity`/`SceneId`/
  `TagId` wrap ordinary `Node`s with identity; `Effect` provides `identity`, `sequence`
  (composition) and `parallel` (monoidal product) over presentation channels.
  `Scene::to_node` is the `Render : SCENE → UI` functor — it emits ordinary nodes
  through the existing pipeline, and moving one entity produces a bounded framebuffer
  diff rather than a whole-screen repaint.
- **Story director** (TESTED, EXPERIMENTAL, Rust-only): `Facts` store semantic world
  state (not countdown timers), `Beat`/`Condition`/`Transition` form a free-category
  story graph where user choices branch and reconverge, and `StoryTrace` records exact
  `(dt, events)` update steps so `Story::replay` reproduces a session deterministically
  at any cadence. `StoryDirector::update` follows **at most one transition per call**
  (see [`../DESIGN.md`](../DESIGN.md) §37).
- **Real replication primitive** (TESTED): `Replication` is a bounded, deterministic
  branching graph with freeze and neutralize; the Hackers rabbit/cookie interaction is
  a real entity, not a generic particle burst.
