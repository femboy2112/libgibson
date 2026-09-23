# LibGibson Architectural Roadmap

This document outlines completed work and the planned future milestones for LibGibson.

## Verification labels

- **IMPLEMENTED + TESTED** — the code exists and is covered by automated tests in `cargo test`.
- **PARTIALLY TESTED** — implemented, with a named facet that is not automatically verified.
- **UNVERIFIED** — present in source or assumed, but never compiled/executed in an attested environment.
- **PLANNED** — not implemented.

> Historical note: earlier revisions of this roadmap marked phases "Completed" and listed Go bindings as verified. Go is **UNVERIFIED** (no Go toolchain), and the hardening round below re-states what is actually proven.

---

## Living battlefield round — IMPLEMENTED + TESTED, EXPERIMENTAL Rust-only

- [x] Scoped SurfaceFx: rect, normalized wipes/radial/band, seeded noise; wide
  glyph, transparency, outside-cell preservation and local-damage tests.
- [x] Demo-local topology reducer with fixed-point influence, separate integrity,
  legal path traversal, telemetry loss, resource costs and cooldowns.
- [x] Deterministic scored Acid planner with personality, trace/isolation/decoy
  memory, legal pivots and repeat-decoy feints. State-based auto defender.
- [x] Broad Story acts consume semantic milestones; in-beat battles continue
  without tactical micro-beats or timer-authored ownership.
- [x] Full EncounterTrace replay includes graph, planner, resources, history,
  StoryDirector, mounts and realized scene. Shared-prefix counterfactuals change
  targets, legal routes, facts and screens.
- [x] Directed influence fronts, actual graph traffic and ghost trajectory,
  full-width climax composition, stable command island, aftermath quality/scars.
- [x] Eight new glyph/style goldens; 13 retained/refreshed Acid PTY cases; all 30
  other demo snapshots unchanged. Eight Acid PTY tests cover genuine adaptation,
  telemetry trade-offs, replay, auto completion and terminal restoration.
- [x] Local fmt, all-target/all-feature strict Clippy, **464 tests** (226 unit +
  238 integration), release, C/C++/Python and C/C++ ASan/UBSan smoke.
- [x] DESIGN §33 distinguishes exact semantic delta, affected footprint and wire
  cost without renaming existing APIs.
- [ ] Extended human playtesting and subjective cinematic impact remain
  **PARTIALLY TESTED**. Linux terminal/VT100 evidence is not universal portability.
- [ ] Go, Windows and broader emulator/multiplexer/SSH behavior are **UNVERIFIED**.
- [ ] No stable serialized encounter replay format; traces are in-memory.
- [ ] Remote Actions remain **BLOCKED / ENVIRONMENTAL** (billing, zero steps).

See [round II architecture, commands and evidence](docs/acid-vs-crash-round2-validation.md).

## Cinematic frontier — IMPLEMENTED + TESTED, EXPERIMENTAL Rust-only

- [x] Ordered entity `SurfaceFx` on arbitrary ordinary nodes; transparent scratch
  only when active. Glyph-safe style/scan/tear/scramble/dissolve and progressive
  StyleMask; removal restores the underlying node.
- [x] Additive displacement/jitter independent of legacy placement; persistent
  finite-period loops and overflow-safe timing.
- [x] In-beat Reactions with explicit event/action order, one transition per
  update, validation, exact replay and bundle clocks.
- [x] `acid_vs_crash`: coherent fictional machine, semantic ownership, deterministic
  adversary/auto defender, interactive counters, local branches/rejoin, display
  possession, stable command island and three endings. No real attack capability.
- [x] 13 new semantic PTY goldens; seven PTY interaction/capability cases;
  whole-renderer frames at 56x24, 80x24, 120x32, 160x40; live resize and input
  preservation; stage/branch replay and bounded damage checks.
- [x] FX Lab scene 20 demonstrates generic post-processing, additive motion and
  reactions without transitions.
- [x] Fixed natural clipping of positioned panels and stale blank cells on
  fullscreen resize. Existing 30 visual goldens remain unchanged.
- [x] Round I verification: fmt, all-target/all-feature clippy, **430 tests**
  (226 unit + 204 integration), release build, C/C++/Python smoke and ASan/UBSan.
- [ ] Subjective cinematic taste, extended human playtesting and terminals beyond
  the local Linux/VT100 evidence remain **PARTIALLY TESTED / UNVERIFIED**.
- [ ] Go remains **UNVERIFIED**: no toolchain installed.
- [ ] Remote CI remains **BLOCKED / ENVIRONMENTAL**: live check confirms billing
  rejection with zero executed steps. No automatic merge.

See [the evidence and command record](docs/acid-vs-crash-validation.md).

## Scene Algebra & Story Director Round — IMPLEMENTED (EXPERIMENTAL) + TESTED

This round stops hand-wiring visual effects to application state by introducing a
small deterministic Scene Algebra and Story Director.

- [x] **Bounded 2D line clipping** (Liang–Barsky) before Bresenham, in both
      canvases: huge finite / near-`i32`-extreme endpoints can no longer cause
      magnitude-proportional walks or overflow. Hostile clipping regression tests.
- [x] **Honest damage metrics**: `exact_changed_cell_count` (true semantic delta)
      alongside `affected_cell_count` (cells *addressed* by update semantics, may
      exceed the live area when rows are removed). Docs corrected.
- [x] **Projector parameter validation**: `Projector::new`/`is_valid` reject
      pathological parameters; no NaN reaches raster math.
- [x] **Scene Algebra** (`src/scene.rs`): `Scene`/`SceneEntity`/`SceneId`/`TagId`,
      `Effect` with `identity`, `sequence` (composition) and `parallel` (monoidal
      product), `Presentation`, `EffectBundle`, and the `Render : SCENE → UI`
      functor (`Scene::to_node`). Effects write presentation channels; they never
      mutate `Node`s.
- [x] **Story Director** (`src/story.rs`): `Facts`, `StoryEvent`, `Beat`,
      `Condition`, `StoryAction`, `StoryDirector`, `StoryTrace` + deterministic
      `Story::replay`. Branch/rejoin, mounted bundles, semantic (persistent) facts.
      Hardened in the merge-gate pass: **at most one transition per `update`**,
      **exact `(dt, events)` step replay**, duplicate beat/bundle rejection, and
      `Story::validate` reference checking. Scene entity labels are unique
      (`Scene::add` panics / `Scene::try_add` errors).
- [x] **Real replication effect** (`src/replication.rs`): bounded deterministic
      branching graph with freeze/neutralize; rabbit/cookie in the demo are a real
      entity, not a generic particle burst.
- [x] **`polished_agent` migration**: story-directed beats, permission *before*
      mutation, truthful approve/reject/cancel outcomes, focus actually routes
      input, overlays animated through Scene effects, `--stage=` fast starts.
- [x] **`hack_the_gibson` partial migration**: tactical branch via `StoryDirector`,
      Plague as persistent semantic state, rabbit/cookie real effect, shell
      commands mutate semantic scene state. The act timeline remains `Phase`-based.
- [x] **Docs truth pass** incl. a new DESIGN §37.

Integration tests: `tests/scene_algebra.rs` (functor through the renderer, bounded
damage, associativity, branch/rejoin, replay). Merge-gate hardening included the
one-arrow law, exact step replay and semantic-identity validation. Total:
**362 tests**.

---

## Phase 1: Core Engine & Minimal Vertical Slice — IMPLEMENTED + TESTED

- [x] Clean-room cell framebuffer architecture
- [x] Compact, stack-allocated grapheme cluster model (`CompactString`)
- [x] Unicode width measurement and wide glyph overwrite protection
- [x] Taffy Flexbox integration with custom text measurement
- [x] Declarative UI node tree (Box, Row, Column, Text, RichText, Border, Spinner, TextInput)
- [x] Differential diff engine with run coalescing and erase-to-end-of-line (`CSI K`)
- [x] Stateful minimal ANSI escape sequence compiler
- [x] Flagship Inline Mode with immutable-scrollback commit semantics
- [x] Fullscreen alternate-buffer mode reusing the same rendering pipeline
- [x] Frame scheduler with frame-budget throttling and telemetry metrics
- [x] Grapheme-aware text input component with navigation and bracketed paste
- [x] RAII terminal lifecycle guard and global panic hook restoration
- [x] Non-interactive / CI redirection detection and plain text degradation
- [x] Stable `extern "C"` ABI with opaque handles and panic containment
- [x] Real PTY integration testing with `portable-pty` (`pty_integration`, `pty_resize_torture`)

---

## Phase 1.5: Agent-Class Typography & Interactive Primitives — IMPLEMENTED + TESTED

- [x] **Chrome Primitives**: `Node::rule` (horizontal divider with optional title) and `Node::rail` (left-border callout).
- [x] **Typography & Hierarchy System**: `Span`, `Line`, `RichText`, and semantic `Theme::styles()` roles with zero raw SGR escape codes in components.
- [x] **Asynchronous Scrollback Insertion (`insert_before_live`)**: two named strategies — `InsertLineFastPath` (uses `CSI L`; no live repaint) and `RepaintFallback` (always correct; repaints and restores cursor). Proven directly in `whole_renderer_vt100`.
- [x] **Display-Width-Aware TextInput**: horizontal scrolling, wide CJK, and emoji display-width calculations.
- [x] **Right-Margin Autowrap Protection**: DECAWM `\x1b[?7l` disabling and right-boundary wide glyph clipping.
- [x] **Responsive Layout Sizing**: percentage width (`percent_width`), min/max dimensions, and per-side padding.
- [x] **Virtual Terminal Screen State Verification**: `vt100` parser automated tests verifying exact screen grids and cursor positions (`screen_state_vt100`, `whole_renderer_vt100`).
- [x] **Rapid Resize & Narrow Terminal Torture Tests**: bounded inline heights and valid cursor coordinates across cyclic resizing down to narrow widths. The **re-anchor path is PARTIALLY TESTED** — erase-from-cursor-down recovery is exercised, but exact absolute recovery is not (DSR unimplemented).
- [x] **Flagship `polished_agent` CLI Demo**: restrained, typography-driven agent interface with rule header, rail callouts, stable-height live plan, permission selector, persistent Unicode input, and zero-escape non-TTY redirection. `hack_the_gibson` is the maximalist twin with zero raw ANSI literals.

---

## Phase 1.75: Documentation Truth & Engine Hardening Round — IMPLEMENTED + TESTED

This round converted earlier over-strong claims into verified behavior and explicit gaps.

- [x] **Atomic `TerminalTransaction`**: one `write_all` + `flush` batching synchronized-update begin/end (`CSI ?2026 h/l`), private modes, cursor motion, diff bytes, final cursor placement, and visibility. Sync-update ownership removed from `AnsiCompiler`.
- [x] **Explicit Physical Anchor**: `AnchorState::{Invalid, Stable{cols,rows,live_height}}` plus last cursor position/visibility. Empty cell diffs still emit when cursor state changes. Geometry changes invalidate the anchor, discard the diff baseline, re-anchor, and increment `anchor_resyncs`. Tested in `whole_renderer_vt100` and the PTY resize probe.
- [x] **Two Insertion Strategies**: `InsertLineFastPath` and `RepaintFallback`, reported through `fast_insertions` / `insertion_repaints`. Universal zero-repaint is explicitly **not** claimed.
- [x] **Unicode Mutation Invariant**: `TextInputState` uses byte-range edits and re-derives the cursor grapheme from the complete resulting string. `cursor_grapheme <= grapheme_count` holds across combining accents, ZWJ emoji, skin-tone modifiers, and flags. Deterministic randomized edit fuzzing included. Single-line paste normalizes `\r\n`, `\r`, `\n` to a space.
- [x] **Structured Static Output**: `commit_text` / `commit_rich_text` / `commit_node` share the width-aware layout engine; control characters are neutralized at the cell model boundary; `commit_raw_ansi_unchecked` is the escape hatch; the ANSI stripper is documented as **not** a sanitizer.
- [x] **Language-Neutral Rich Text ABI**: opaque `gibson_line_t` / `gibson_rich_text_t` with span/align builders. **C, C++, Python: IMPLEMENTED + TESTED. Go: UNVERIFIED.**
- [x] **Metrics / Byte Accounting**: `frame_bytes`, `commit_bytes`, `insertion_bytes`, `control_bytes`, `total_terminal_bytes()`, plus `fast_insertions` / `insertion_repaints` / `anchor_resyncs`. `bytes_emitted()` deprecated in favor of `frame_bytes`.
- [x] **Scheduler / Runtime Model**: `DEFAULT_ANIMATION_INTERVAL = 80ms` and `Context::run_once(max_wait)` with input priority. Demos use `run_once` instead of `render() + sleep()`.
- [x] **Versioned C ABI**: `GIBSON_ABI_VERSION = 1`, `gibson_abi_version()`, `gibson_stats_init()`, dedicated `#[repr(C)]` `gibson_stats_t` with `struct_size` + `abi_version` + 14 `u64` fields. `gibson_get_stats` validates the version and refuses an undersized buffer (fixing a real 32-byte overflow). Enum-like inputs cross as validated raw `int32`.
- [x] **Hostile-Input Test Coverage**: invalid mode/border/color/wrap, null pointers, malformed UTF-8, wrong ABI version, undersized stats buffer, and non-finite layout floats.
- [x] **Verification Toolchain**: 100 tests (49 unit + 51 integration), clean `clippy -D warnings`, clean `fmt --check`, successful release build, and C/C++ examples clean under ASan + UBSan (LeakSanitizer disabled). Python `ctypes` example runs.

### Known gaps carried out of this round

- Go bindings **UNVERIFIED** (no toolchain).
- Windows / ConPTY **UNVERIFIED**; only Linux x86_64 exercised.
- tmux / screen / SSH matrix **UNVERIFIED**.
- Terminal capability negotiation **NOT implemented**; the `CSI L` fast-insertion assumption is not probed.
- Absolute cursor query (DSR) **NOT implemented**.
- Hard `SIGKILL` cannot be intercepted.
- Ctrl-C is a raw-mode key event in demos; no general signal-handling guarantee.
- No `cargo-fuzz` / AFL target yet.

---

## Phase 1.85: Compositor, Sub-cell Canvas & Determinism — IMPLEMENTED + TESTED

- **Safety split**: safe `insert_text_before_live` / `insert_rich_text_before_live` / `insert_node_before_live`; raw is explicitly `insert_raw_lines_before_live_unchecked`. `Renderer::commit` is now safe text (`commit_text`), fixing the old name-inverts-safety bug. (`tests/safety_api.rs`)
- **Exact wire metrics**: `TerminalTransaction::commit` returns the exact byte count including the synchronized-update terminator; renderer metrics no longer undercount. (`src/transaction.rs`)
- **Layer compositor**: `Node::stack()` + explicit `Cell::transparent` + `Node::dim()` style-only veil; overlay removal leaves no ghosts; floating modals do not reflow. (`tests/compositor.rs`, `whole_renderer_vt100`)
- **Sub-cell canvases**: `BrailleCanvas`, `HalfBlockCanvas`, `braille_oscilloscope`; exact glyph/colour tests; bounded update cost. (`src/canvas.rs`, `tests/effects_perf.rs`)
- **Deterministic clock**: `TimeSource`/`FixedStepClock` + motion helpers; scripted frames are reproducible. (`src/clock.rs`)
- **Capability + color ladder**: `TerminalCapabilities`, tri-state `Capability`, `ColorDepth`, central quantization; the `CSI L` fast path requires explicit support. (`src/capability.rs`, `tests/capability_fallback.rs`)
- **Visual goldens**: deterministic PTY→vt100 screen snapshots under `tests/goldens/` with explicit regeneration. (`tests/visual_goldens.rs`)
- **Demos**: `polished_agent` gains a floating permission modal, dim veil, toast overlay and a Braille telemetry scope; `hack_the_gibson` gains a Braille virus-scan instrument and a CRT scanline overlay.
- **CI status**: the workflow now also runs the visual goldens and capability tests. Remote GitHub Actions is currently **blocked by account billing** (the job is refused before any step runs: "recent account payments have failed or your spending limit needs to be increased"). This is environmental, not a code failure; local verification is green.

## Phase 1.9: Scene/Effects Substrate — IMPLEMENTED + TESTED

- **Positioned layers** (`Node::offset`) with clip-aware leading-edge rendering. (`tests/scene.rs`)
- **Camera viewports** (`Node::viewport`, `ViewportState`). (`tests/scene.rs`)
- **Raster embedding** (`Node::raster`/`Node::surface`, `blit_transparent_clipped`). (Rust-only, no C ABI yet.)
- **Vector primitives** on `BrailleCanvas` (rect/circle/ellipse/polygon).
- **3D wireframe projector** (`geom`: Vec3/Transform3/Mesh/Projector, near-plane clip). (`src/geom.rs`)
- **Particles** (seeded deterministic system). (`src/particles.rs`)
- **Procedural fields** (plasma/interference/heat). (`src/field.rs`)
- **Text transitions + safe glitch** (grapheme-safe; content-only). (`src/transition.rs`, `src/glitch.rs`)
- **Damage inspection** (`SurfaceDiff::dirty_cells`, `Context::last_dirty_cells`) powering live damage maps.
- **Demos**: `hack_the_gibson` rotating wireframe/plasma/particles/glitch/damage map; `polished_agent` modal shadow + transcript camera; **new `examples/fx_lab.rs`** gallery.
- **Tests**: bounded effect damage, resize-during-wireframe, deterministic goldens incl. `fx_lab`. 233 tests total.
- **Remote CI remains BLOCKED / ENVIRONMENTAL** (GitHub account billing); local verification green.

## Phase 1.10: Correctness Hardening + Interactive Demos — IMPLEMENTED + TESTED

- **Near-plane clipping made consistent**: inclusive `depth >= near` predicate shared by `project`/`clip_near`; crossings no longer disappear. Decisive tests. (`src/geom.rs`)
- **Wide-glyph clip containment**: clipping wins; no continuation leaks past any clip rectangle. Regression tests for raster nodes, viewports and positioned layers. (`src/surface.rs`, `tests/scene.rs`)
- **Logical damage model**: runs ∪ erase-to-EOL ∪ cleared rows, separate from wire cost. (`src/diff.rs`, `src/renderer.rs`, `tests/effects_perf.rs`)
- **TextInput cursor policy**: the placeholder stays intact; the software cursor highlights an existing glyph rather than blanking it. (`src/painter.rs`)
- **Particle API truth**: radial vs life-variance vs directional bursts. (`src/particles.rs`)
- **Mono field ordered dithering** (4×4 Bayer). (`src/field.rs`)
- **Minimal focus ring** + `PageUp`/`PageDown` keys. (`src/focus.rs`, `src/input.rs`)
- **Demos**: `polished_agent` is **inline by default** (real scrollback + mutable live foreground), with a richer plan (failure/recovery), scrollable code viewport and a completion summary; `hack_the_gibson` is an act-based *Hackers* (1995) homage (Gibson data city, Plague, Da Vinci, broadcast, Grand Central attack, rooftop pool, CRASH AND BURN) with a real root shell. `fx_lab` adds regression scenes and `--debug-damage`.
- **Tests**: 291 total (was 233), incl. interactive PTY tests (`tests/pty_demos.rs`) and expanded visual goldens.
- **Remote CI remains BLOCKED / ENVIRONMENTAL** (GitHub account billing); local verification green.

## Phase 2: Input Protocols & Interaction Enhancements — PARTIAL

- [x] **Minimal focus ring** (`FocusId`/`FocusRing`, Tab/Shift-Tab, modal capture/restore) — implemented; a full hierarchical focus tree and event bubbling remain planned.

- [ ] **Kitty Keyboard Protocol**: progressive enhancement for disambiguated escape keys, key release events, and modifier combinations.
- [ ] **Focus Management Tree**: hierarchical focus tree with Tab / Shift-Tab cycling and focus restoration.
- [ ] **Event Bubbling and Capture**: structured event dispatch pipeline allowing parent containers to intercept or bubble user events.
- [ ] **Mouse Tracking**: optional SGR mouse reporting (`CSI ? 1006 h`) for click-to-focus, scroll wheel handling, and selection.
- [ ] **OSC 8 Terminal Hyperlinks**: embedded clickable URLs in Text nodes with fallback plain-text formatting.

---

## Phase 3: Advanced Layout & Rich Components — PLANNED

- [ ] **Incremental Layout Caching**: cache Taffy layout subtrees across frames when node contents are unmodified.
- [ ] **Scrollable Viewport Widgets**: scrollable virtual boxes with vertical and horizontal scrollbars.
- [ ] **Virtualization Engine**: virtual list and table rendering supporting very large datasets without memory pressure.
- [ ] **Rich Component Library**:
  - Tables with auto-sizing columns and alignment.
  - Progress bars with smooth Unicode fraction characters (`▏▎▍▌▋▊▉█`).
  - Tree views with expandable/collapsible nodes.
  - Tab headers and segmented controls.
- [ ] **Markdown and Syntax Highlighting**: streaming Markdown parser with Syntect or Tree-sitter token colorization.

---

## Phase 4: Styling, Themes, and Accessibility — PLANNED

- [ ] **24-bit Truecolor Palettes and Themes**: CSS-like theme definitions with automatic fallback to ANSI-256 or 16-color ANSI.
- [ ] **Terminal Capability Negotiation**: automatic detection via Primary and Secondary Device Attributes (`CSI c`, `CSI > c`) for synchronized output, color depth, and graphics protocols. This also lets the engine choose the insertion strategy from measured capability rather than assumption.
- [ ] **DSR Absolute Anchoring**: query the cursor position (`CSI 6 n`) to re-anchor exactly after resize/reflow instead of the current best-effort relative rebuild.
- [ ] **Accessibility (A11y)**: screen reader annotations, semantic headings, and ARIA-like terminal roles for assistive technology.

---

## Phase 5: Protocol Extensions & Hardening — PLANNED

- [ ] **Terminal Graphics Protocols**: inline image rendering via Kitty graphics protocol, iTerm2 inline images, and Sixel graphics.
- [ ] **Windows ConPTY Torture Testing**: extended automated testing under the Windows Console API and ConPTY.
- [ ] **Multiplexer & Remote Shell Hardening**: specialized test matrix for tmux, screen, and SSH connections over high-latency networks.
- [ ] **Property & Fuzz Testing**: `cargo-fuzz` / AFL suite exercising arbitrary Unicode sequences, arbitrary terminal byte streams, invalid ANSI input, and rapid terminal resizes.
- [ ] **Go / Foreign Binding CI**: compile and run the Go bindings once a Go toolchain is available; add binding smoke tests to CI.
