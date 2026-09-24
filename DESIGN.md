# LibGibson Architecture & Design Document

For current maturity, known defects, release blockers and priorities, see
[State of LibGibson](docs/STATE_OF_LIBGIBSON.md). Historical run evidence is in
the [validation index](docs/VALIDATION_INDEX.md).

## 1. Overview and Core Philosophy

LibGibson solves a fundamental deficiency in modern CLI applications: the friction between **terminal scrollback history** and **interactive mutable user interfaces**.

In standard terminal programming, developers either:

1. Use an alternate screen buffer (`CSI ? 1049 h`), seizing total control of the screen but discarding the user's scrollback and terminal context.
2. Use raw escape codes and `\r\x1b[K` macros, which break down as soon as terminal width changes, text wraps, or multiline widgets update.

LibGibson treats the terminal as a **2D character-cell framebuffer** with an explicit boundary between **immutable terminal scrollback** and an **active live region**.

```
    [Terminal History]
    Committed output (immutable, native terminal scrollback)
    ▲
    │ commit() transitions finalized state permanently
    ▼
    [Live Viewport]
    Mutable frame-buffered region (diff-rendered, anchor-tracked)
```

### Verification labels used in this document

Because earlier revisions of this document overstated completion, architectural claims below carry an explicit status:

| Label | Meaning |
| --- | --- |
| **IMPLEMENTED** | The described code path exists and is reached in normal operation. |
| **TESTED** | Covered by an automated test in this repository (`cargo test`, 574 tests: 226 unit + 348 integration) that exercises the behavior described. |
| **PARTIALLY TESTED** | Implemented, and some behavior is covered, but at least one named facet is not automatically verified. The gap is stated explicitly. |
| **UNVERIFIED** | Written down because it exists in source or is a documented assumption, but has not been compiled or executed in any environment we can attest to. |

The overall state of the core engine is **IMPLEMENTED + TESTED on Linux x86_64 only**. See §15 for the honest limitation list.

---

## 2. The Rendering Pipeline

The rendering lifecycle follows a decoupled vertical pipeline. Every stage is **IMPLEMENTED**; the end-to-end byte stream is **TESTED** through a virtual terminal in `tests/whole_renderer_vt100.rs`.

```
┌─────────────────────────┐
│     Declarative Tree    │  Node hierarchy (Box, Row, Col, Text, RichText,
└───────────┬─────────────┘  Rule, Rail, Border, Spinner, TextInput)
            │
            ▼
┌─────────────────────────┐
│     Taffy Flexbox       │  Maps declarative nodes to integer Rects
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│       Cell Painter      │  Paints laid-out nodes into next Surface framebuffer
└───────────┬─────────────┘  (returns a PaintContext: cursor position request)
            │
            ▼
┌─────────────────────────┐
│     Differential Diff   │  Compares previous Surface with next Surface
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│      ANSI Compiler      │  Emits stateful minimal cursor & SGR byte stream
└───────────┬─────────────┘  (AnsiCompiler does NOT own sync-update state)
            │
            ▼
┌─────────────────────────┐
│  TerminalTransaction    │  Batches sync-update begin/end, private modes,
└───────────┬─────────────┘  cursor motion, diff bytes, cursor placement and
            │                visibility into ONE write_all + flush
            ▼
┌─────────────────────────┐
│     Terminal Output     │  stdout
└─────────────────────────┘
```

The pipeline is driven by `Renderer::render` (`src/renderer.rs`). The renderer also owns the explicit **physical anchor** (§6), which is why an empty cell diff does not automatically mean "emit nothing".

---

## 3. Cell and Glyph Model

**IMPLEMENTED + TESTED** (`src/cell.rs`, `src/surface.rs`, unit tests plus `whole_renderer_vt100`).

Terminal cells cannot be represented as simple `char` or ASCII byte arrays. Unicode requires handling:

- Multi-byte UTF-8 sequences.
- Extended grapheme clusters (e.g., base characters with combining accents: `e` + `U+0301` → `é`).
- Wide full-width CJK characters (`display_width == 2`).
- Emoji sequences with Zero-Width Joiners (ZWJ) and skin-tone modifiers (`display_width == 2`).
- Zero-width codepoints.

### Memory Optimization: `CompactString`

A naive implementation using `String` per cell allocates on every cell mutation. LibGibson uses `CompactString` (from the `compact_str` crate) inside `Glyph`:

- 24 bytes total on 64-bit platforms.
- Stack-allocated for all strings up to 24 UTF-8 bytes (which covers the overwhelming majority of Unicode grapheme clusters and emoji).
- No heap allocation during cell construction and updates for that common case.

### Wide Glyph Overwrite Safety

A double-width character occupies two consecutive terminal columns: `(x, y)` as the lead glyph, and `(x+1, y)` as a continuation marker. If software overwrites half of a wide character, terminals can render corrupt text.

LibGibson enforces strict invariants in `Surface::set_cell`:

1. **Overwriting a continuation cell**: If cell `(x, y)` is a continuation, the lead character at `(x-1, y)` is automatically cleared to a blank space.
2. **Overwriting a wide lead**: If cell `(x, y)` is a wide lead and is overwritten with a single-width character, the continuation cell at `(x+1, y)` is automatically cleared.
3. **Inserting a wide character**: If a wide character is placed at `(x, y)`, and `(x+1, y)` was previously the lead of another wide glyph, `(x+2, y)` is cleared so no orphaned continuation remains.
4. **Right-edge clipping**: If a wide character is placed at the rightmost boundary (`x + 1 >= width`), it cannot fit and is safely clipped rather than corrupting line wraps.

### Control-Character Neutralization

`Surface::print_str` **skips any grapheme containing a control character** (`char::is_control`). This happens at the cell model boundary, so text passed to structured output paths (`commit_text`, `commit_rich_text`, `Node::text`, `RichText`) can never inject `ESC` / `OSC` / `CSI` into the terminal. The explicit raw-ANSI escape hatch intentionally bypasses this (§10). **IMPLEMENTED + TESTED** (`tests/commit_invariance.rs::test_commit_does_not_interpret_control_characters`).

---

## 4. Layout Engine

**IMPLEMENTED + TESTED** (`src/layout.rs`, unit tests plus `whole_renderer_vt100` responsive cases).

LibGibson integrates the `taffy` crate (v0.14) without exposing Taffy types across the public API.

- **Flexbox Containers**: Supports `FlexDirection::Row` and `FlexDirection::Column`, flex grow, flex shrink, gap, padding, `align_items`, and `justify_content`.
- **Intrinsic Text Measurement**: Text nodes implement custom leaf measurement in Taffy:
  - `WrapMode::NoWrap`: Intrinsic width equals the maximum line length; height equals line count.
  - `WrapMode::WordWrap`: Wraps on word and whitespace boundaries; falls back to grapheme breaks when a single word exceeds container width.
  - `WrapMode::CharWrap`: Wraps strictly at grapheme cluster boundaries.

The layout engine is also reused for **static committed output** (§10), so `commit_text`, `commit_rich_text`, and `commit_node` wrap and align identically to live nodes. No wrapping logic is duplicated outside Rust.

---

## 5. Differential Diff

**IMPLEMENTED + TESTED** (`src/diff.rs`, `tests/diff_golden.rs`).

LibGibson does not repaint unchanged cells and does not clear the entire screen on every frame.

1. For each row `y`:
   - Compare `prev_cells[x]` with `next_cells[x]`.
   - Skip rows where all cells are identical.
   - For rows with changes, group contiguous dirty cells into `CellRun { x, cells }`.
2. **Erase to End of Line Optimization**: If a line shrinks in width (e.g. status changes from `"Processing query 12345"` to `"Done"`), the diff detects that trailing cells became blank and emits `erase_eol_from: Some(x)`, compiling to `\x1b[K` (`CSI K`) instead of emitting dozens of space characters.
3. **Orphaned Row Clearing**: If the live region shrinks in height from `H1` to `H2`, the diff identifies `rows_to_clear = H1 - H2` and erases the leftover rows below with `\x1b[K`.

---

## 6. Explicit Physical Anchor Model

**IMPLEMENTED + TESTED** (`src/renderer.rs`; `tests/whole_renderer_vt100.rs`, `tests/pty_resize_torture.rs`, `tests/resize_torture.rs`).

The renderer tracks more physical state than the cell framebuffer:

- `AnchorState::{ Invalid, Stable { cols, rows, live_height } }` — the renderer's belief about where the live region actually sits on the terminal.
- The last committed hardware cursor position (`last_cursor_x`, `last_cursor_y`).
- The last committed cursor visibility (`last_cursor_visible`).

**Surface equality alone is not physical truth.** If the cell diff is empty but the requested cursor position or visibility changed, the renderer still emits a transaction. This is asserted directly in `whole_renderer_vt100::whole_renderer_cursor_visibility_is_physical_truth`.

On terminal geometry change (`observe_geometry`):

1. The anchor is set to `AnchorState::Invalid`.
2. The diff baseline (`previous_surface`) is discarded.
3. `anchor_resyncs` is incremented.
4. The next frame performs a **re-anchor**: it erases from the current cursor row downward (`\r\x1b[J`) and rebuilds the live region from scratch, then allocates `H - 1` rows and rewinds to the region top (inline mode).

Re-anchoring is **PARTIALLY TESTED**. The relative erase-from-cursor-down reanchor is exercised in the whole-renderer and PTY tests. An **absolute cursor query (DSR) is not implemented**, so the engine cannot learn its absolute row from the terminal; it re-establishes the region by best-effort relative motion. This is a stated limitation, not a claim of exact recovery.

---

## 7. Stateful ANSI Compiler

**IMPLEMENTED + TESTED** (`src/ansi.rs`, unit tests plus `screen_state_vt100` / `whole_renderer_vt100`).

`AnsiCompiler` converts surface diffs into compact byte streams and maintains:

- `cursor_x`, `cursor_y`
- Current `Style` (foreground, background, bold, dim, italic, underline, reverse)

It does **not** maintain a `sync_updates` flag. Synchronized-update ownership lives in `TerminalTransaction` (§8).

Key optimizations:

- **Cursor Motion**: Compares relative forward/backward jumps (`\x1b[C`, `\x1b[D`), absolute column positioning (`\x1b[<col>G`), and carriage returns (`\r`), selecting the minimal byte sequence.
- **SGR Minimization**: Emits style change codes only when attributes differ from current compiler state. Attributes are not reset between characters unnecessarily.
- **Erase to End of Line / orphaned rows**: Emits `CSI K` when a line shrinks or rows are removed.
- **Continuation Cells**: Continuation cells in double-width glyphs are omitted from the ANSI stream because standard terminals advance the hardware cursor by 2 columns automatically.
- **Autowrap Protection**: Wraps diff emission in `\x1b[?7l` (disable DECAWM) and `\x1b[?7h` (re-enable) so right-margin output cannot trigger autowrap.

---

## 8. TerminalTransaction

**IMPLEMENTED + TESTED** (`src/transaction.rs`; exercised by every whole-renderer test).

`TerminalTransaction` represents a single atomic write to the terminal. Its buffer accumulates:

1. Synchronized-update begin `\x1b[?2026h` (when enabled).
2. Cursor motion / temporary private modes emitted by the renderer or compiler.
3. Framebuffer diff bytes.
4. Final cursor placement.
5. Cursor visibility (`\x1b[?25h` / `\x1b[?25l`).
6. Synchronized-update end `\x1b[?2026l`.

`TerminalTransaction::commit` performs **one `write_all` followed by one `flush`**. This is what makes a frame atomic from the terminal's perspective and is the reason sync-update ownership was moved out of `AnsiCompiler`: the transaction, not the compiler, decides whether a given operation is wrapped. The compiler may legitimately be used to produce a patch that is embedded inside a larger transaction.

---

## 9. Inline Mode vs Fullscreen Mode

**IMPLEMENTED + TESTED** (`whole_renderer_vt100`, `pty_integration`).

### Inline Mode (flagship)

- Coexists with normal shell history.
- The live region has height `H`.
- On the first frame of a live region, `H - 1` newlines are emitted to allocate terminal lines without overwriting scrollback, followed by a cursor rewind `\x1b[{H-1}A\r`.
- Subsequent frame diffs rewind the cursor relative to `last_cursor_y` and render within the live region.
- If the live region grows, extra rows are allocated at the bottom before re-rendering.
- After a geometry change the region is re-anchored (§6), which begins with an erase from the cursor down.

### Fullscreen Mode

- Enters the alternate screen buffer (`\x1b[?1049h`).
- Dimensions match terminal rows and columns.
- Reuses the identical Surface, Layout, Painter, Diff, and ANSI compiler pipelines.
- Each frame emits an absolute cursor home (`\x1b[H`) before compiling the diff. This is required: the compiler performs relative cursor motion, so resetting only its bookkeeping (without homing the *physical* cursor) made every frame drift by the previous frame's final cursor and eventually scroll the alternate screen. Covered by `whole_renderer_vt100::whole_renderer_fullscreen_owns_the_canvas`.

### Border integrity

Bordered containers (`Node::box`/`Node::border_box`, `Node::panel`) clip their children to the inside of the frame, so a panel that the flex layout shrinks below its content height clips the content instead of painting over the border. Covered by `painter::border_clip_tests::panel_children_never_overwrite_the_border`.

---

## 10. Commit Semantics and Static Output

**IMPLEMENTED + TESTED** (`tests/commit_invariance.rs`, `tests/whole_renderer_vt100.rs`, `tests/non_tty_redirection.rs`).

### Structured commit paths

- `Context::commit_text(&str)` — width-aware wrapped plain text. Control characters are neutralized at the cell model boundary.
- `Context::commit_rich_text(&RichText)` — styled spans/lines, wrapped and aligned by the same engine as live nodes.
- `Context::commit_node(&mut Node)` — a laid-out node tree serialized directly to scrollback.
- `Context::commit` — backwards-compatible alias for `commit_text`.

All three structured paths route through the same width-aware layout/wrapping engine. When `ctx.commit(...)` is called (TTY path):

1. The terminal cursor is rewound relative to the live region.
2. The live region rows are cleared with `\x1b[K`.
3. The committed text lines are emitted directly to stdout, followed by `\r\n`.
4. The committed text is now in the native terminal scrollback buffer.
5. Live state is invalidated (`invalidate_anchor(false)`): the next live frame starts fresh.
6. Because committed text is never retained in the mutable framebuffer, **the rendering cost of a live prompt or spinner is independent of the length of the transcript history**.

### Raw escape hatch

`Context::commit_raw_ansi_unchecked(&str)` writes the payload verbatim. The caller is responsible for its safety. Arbitrary `OSC` / `DCS` / `CSI` input is **not sanitized**. Use the structured paths for untrusted text.

### Non-TTY redirection

When stdout is not a TTY, live interactive frames are suppressed. Structured commits emit clean plain UTF-8 with zero escape sequences (`tests/non_tty_redirection.rs`). A homemade ANSI stripper (`strip_ansi_escapes`) is used **only** to render already-engine-generated escape sequences back to readable text for non-TTY logs. It is documented as **not a sanitizer** for untrusted input.

---

## 11. Asynchronous Scrollback Insertion

**IMPLEMENTED + TESTED** (`src/renderer.rs`; `whole_renderer_vt100` tests both strategies directly).

In real agent CLIs, asynchronous events occur while the user is typing or a spinner is running (git filesystem notifications, LSP diagnostics, streaming logs). Insertion into native scrollback **above** the live region is split by safety: `insert_text_before_live` / `insert_rich_text_before_live` / `insert_node_before_live` are safe (width-aware, controls neutralized), while `insert_raw_lines_before_live_unchecked` is the explicit raw escape hatch. There are **two named strategies**, exposed via `InsertStrategy` and counted in metrics:

### `InsertLineFastPath`

Uses `CSI L` (Insert Lines) at the region top so the existing live framebuffer is **not repainted**. The renderer moves to the region bottom, scrolls to make room, returns to the top, inserts `M` lines, prints the history, and restores the cursor's relative position within the region. The fast path is selected only when:

- the live region is non-empty,
- the anchor is `AnchorState::Stable`,
- combined history + live height fits within the terminal rows,
- a preserved `previous_surface` exists.

This path is proven **without any live repaint** in `whole_renderer_vt100::insert_line_fast_path_inserts_history_without_repainting_live` and the multi-row/cursor-restore variant.

### `RepaintFallback`

Always correct, used when the fast path's preconditions are not met. It erases from the region top down (`\x1b[J`), prints the history rows, repaints the preserved live framebuffer, and restores the exact cursor x/y and visibility. `previous_surface` is preserved, so the next ordinary diff render can emit zero bytes. Tested in `whole_renderer_vt100::repaint_fallback_used_when_no_room_and_restores_live_and_cursor`.

> **Do not claim universal zero-repaint.** When the fast path is unavailable or the anchor is untrustworthy, the engine repaints the live region. `insertion_repaints` vs `fast_insertions` in metrics report which happened.

Non-TTY insertion prints stripped plain lines; the engine does not perform terminal surgery there.

---

## 12. Unicode Input Invariant

**IMPLEMENTED + TESTED** (`src/input.rs`; unit tests including deterministic randomized edit fuzzing, plus `whole_renderer_vt100` long-input scrolling).

`TextInputState` is a grapheme-aware single-line editor. After **every** public mutation:

```text
cursor_grapheme <= text.graphemes(true).count()
```

Mutations are expressed as **byte-range edits**, and the cursor index is then **re-derived from the segmentation of the complete resulting string**. This matters because grapheme boundaries can merge across an insertion boundary: inserting `U+0301 COMBINING ACUTE ACCENT` after `e` produces a single `é` cluster. The naive "insert then add the inserted grapheme count" approach leaves the cursor past the end of the buffer and can panic on the next backspace.

The test suite covers boundary-merging insertions for combining accents, ZWJ emoji, skin-tone (Fitzpatrick) modifiers, and regional-indicator flags, then a 4000-step deterministic randomized edit fuzz asserting the invariant.

**Single-line paste policy**: all line breaks (`\r\n`, `\r`, `\n`) are normalized to a single space before insertion, so multiline content is flattened rather than rejected.

---

## 13. Language-Neutral Rich Text ABI

**IMPLEMENTED + TESTED for C, C++, Python. Go build/example smoke tested.**

The C ABI exposes opaque `gibson_line_t` and `gibson_rich_text_t` handles with span/align builders:

- `gibson_line_new`, `gibson_line_add_span`, `gibson_line_set_align`, `gibson_line_free`
- `gibson_rich_text_new`, `gibson_rich_text_add_line`, `gibson_rich_text_free`
- `gibson_node_rich_text`, `gibson_commit_rich_text`, `gibson_insert_rich_text_before_live`

Foreign callers build structured rich text without duplicating any wrapping logic; the Rust layout engine handles width, wrapping, and alignment. Covered by `tests/ffi_lifecycle.rs::test_ffi_rich_text_abi` and `test_ffi_bad_align_rejected`, and the C / C++ / Python examples.

The Go module now passes local and public Linux build/example smoke checks.
This is not exhaustive wrapper parity; the package has no Go unit tests.
See [current validation evidence](docs/STATE_OF_LIBGIBSON.md).

---

## 14. Metrics and Byte Accounting

**IMPLEMENTED + TESTED** (`src/scheduler.rs`, unit tests; `tests/ffi_lifecycle.rs` for the C struct).

`RenderStats` exposes:

| Field | Meaning |
| --- | --- |
| `frames`, `skipped_frames` | Frame accounting. |
| `dirty_cells`, `total_cells` | Diff volume. |
| `frame_bytes` | Wire bytes for live differential frames (control sequences included). |
| `full_repaints` | Frames that rebuilt the region from scratch. |
| `last_render_duration_micros` | Duration of the last frame. |
| `history_insertions` | Total scrollback-insertion operations. |
| `insertion_repaints` | Insertions that used `RepaintFallback`. |
| `fast_insertions` | Insertions that used `InsertLineFastPath`. |
| `insertion_bytes` | Wire bytes for insertion operations. |
| `anchor_resyncs` | Physical anchor invalidations / re-establishments. |
| `commit_bytes` | Wire bytes for commits to scrollback. |
| `control_bytes` | Wire bytes for standalone control operations (e.g. `clear_live_region`). |

`total_terminal_bytes()` is the sum of `frame_bytes + commit_bytes + insertion_bytes + control_bytes`. Byte counters measure **wire bytes for each operation**, including engine-emitted control sequences, grouped by operation rather than by character class. `RenderStats::bytes_emitted()` is a **deprecated** alias for `frame_bytes`; the old name misleadingly implied total wire output.

---

## 15. Scheduler and Runtime Model

**IMPLEMENTED + TESTED** (`src/scheduler.rs`, `src/context.rs`).

- `DEFAULT_ANIMATION_INTERVAL = 80ms`. **60 FPS is a ceiling for input latency, not a spinner target.** A 12.5 Hz spinner is visually smooth and far cheaper.
- `Context::run_once(max_wait)` is the minimal runtime step. It waits up to `min(max_wait, next frame deadline)`, polls input (**input has priority** over decorative animation), then renders if the scheduler permits. It returns the input event, if any.
- `Context::render_if_due`, `Context::request_render`, `Context::animation_interval`, and `Context::frame_budget` round out the loop.
- The showcase demos (`polished_agent`, `hack_the_gibson`, `resize_test_app`) drive their loops through `run_once` rather than `render() + sleep()`.

`DEFAULT_ANIMATION_INTERVAL` and the scheduler are **IMPLEMENTED + TESTED** as a module. The end-to-end interactive cadence of the demos is **PARTIALLY TESTED**: the deterministic `--auto` modes run in CI/PTY tests, but subjective smoothness and real input latency are not automatically measured.

---

## 16. Theme and Semantic Styles

**IMPLEMENTED + TESTED** (`src/cell.rs`, unit tests; demos exercise all modes).

`Theme::styles()` returns a `ThemeStyles` struct of semantic `Style` roles:

`text`, `muted`, `faint`, `accent`, `success`, `warning`, `error`, `border`, `rail`, `code`, `link`, `selection`.

Design rules:

- `muted` is deliberately **default foreground + dim**, not a hardcoded `BrightBlack`, which is unreadable on some light terminals.
- `Theme::no_color()` emits no color attributes.
- Demos support `--light` / `--dark` / `--no-color` and never paint a background canvas — the native terminal background is respected.

`Theme` also carries a base palette (`text`, `text_muted`, `accent`, …, `bg`); the semantic `ThemeStyles` roles are the preferred surface for components.

---

## 17. C ABI Boundary and Foreign Language Safety

**IMPLEMENTED + TESTED** (`src/ffi.rs`; `tests/ffi_lifecycle.rs`, 16 tests).

The C ABI is designed around strict safety invariants:

1. **ABI versioning**: `GIBSON_ABI_VERSION = 1`. `gibson_abi_version()` reports it, and `gibson_stats_init()` initializes a stats header.
2. **Dedicated versioned stats struct**: `gibson_stats_t` is its own `#[repr(C)]` struct — **not** `RenderStats`. It begins with `struct_size` (u32) + `abi_version` (u32), followed by 14 `u64` fields. `gibson_get_stats` validates `abi_version` and **refuses an undersized buffer instead of overflowing it**. This fixed a real 32-byte overflow and is covered by `test_ffi_stats_overflow_is_prevented` and `test_ffi_stats_rejects_wrong_abi_version`.
3. **Enum-like inputs cross as raw `int32`**: render mode, border type, color type, wrap mode, align, and event type are all transported as raw `int32` and validated. Invalid values return `GIBSON_ERR_INVALID_PARAM`. `GibsonColor.color_type` is `int32`.
4. **Opaque pointers**: `gibson_context_t`, `gibson_node_t`, `gibson_line_t`, `gibson_rich_text_t` hide internal Rust layouts.
5. **No panics across FFI**: every public `extern "C"` function is wrapped in `std::panic::catch_unwind`. A caught panic records a thread-local message and returns `GIBSON_ERR_PANIC`.
6. **Explicit memory ownership**: dedicated free functions (`gibson_node_free`, `gibson_line_free`, `gibson_rich_text_free`, `gibson_destroy_context`).

Hostile-input tests cover invalid mode / border / color / wrap values, null pointers, malformed UTF-8, wrong ABI version, undersized stats buffer, and non-finite layout floats.

---

## 18. Visual Doctrine & Clean-Room Design Philosophy

**IMPLEMENTED** in the primitives and **PARTIALLY TESTED** through the deterministic demo modes.

1. **Native Background Respect**: the default terminal background must remain transparent or default. Never draw solid dark/colored rectangular canvases over the viewport. Background colors are reserved for subtle highlights.
2. **The Rail Callout Doctrine (`Node::rail`)**: heavy box chrome consumes screen real estate; a left-border rail provides containment with minimal visual weight.
3. **Structured Text Layout (`RichText`, `Line`, `Span`, `Theme`)**: no hardcoded ANSI string literals in components; semantic theme tokens compose styles; instant theme switching and plain-text rendering for pipes.
4. **Zero-Escape Non-TTY Redirection**: piped output suppresses interactive escapes and emits clean plain UTF-8.

---

## 19. Terminal Autowrap and Right-Margin Safety

**IMPLEMENTED + TESTED** (`tests/screen_state_vt100.rs::test_vt100_autowrap_protection_at_right_margin`).

When text or background cells reach column `width - 1`, standard VT100/ANSI terminals trigger autowrap (DECAWM), which can cause tearing and vertical drift during differential rendering. LibGibson protects against this with:

1. **DECAWM Autowrap Disabling**: the ANSI compiler wraps diff emission in `\x1b[?7l` before cell runs and `\x1b[?7h` afterward.
2. **Surface Right-Edge Clipping**: wide glyphs occupying 2 columns are clipped if `x + 1 >= width`.

---

## 20. Layer Compositor (Stacks and Explicit Transparency)

**IMPLEMENTED + TESTED** (`tests/compositor.rs`, `whole_renderer_vt100`, demos).

The node tree is no longer only rectangular flow. `Node::stack()` is an overlay
container whose children are absolutely positioned to fill the stack's content
box and composited in child order (later = on top).

Transparency is **explicit**, never inferred from blank cells:

- A scratch layer for an overlay is a `Surface::new_transparent()`; untouched
  cells carry `Cell::transparent == true` and contribute nothing.
- Opaque cells replace the destination cell (with the existing wide-glyph
  invariants preserved).
- Style-only cells (`Cell::style_only`, produced by `Node::dim()`) merge their
  style onto the destination cell without replacing its glyph. This powers
  dimming veils, scanlines and hover washes.

Painting a stack composites each child through a transparent scratch surface and
`Surface::blit_transparent`. Consequences proven by tests:

- a transparent overlay does not alter the lower glyph;
- an opaque overlay replaces it;
- removing an overlay leaves **no ghost cells** (the diff drives the erase);
- a floating modal does not reflow the layout beneath it;
- a dim veil preserves existing bold while adding dim;
- wide glyphs are correctly cleared when an overlay writes their continuation.

`Stack` and `Dim` cross the C ABI as `gibson_node_stack` / `gibson_node_dim`.

## 21. Deterministic Time and Motion

**IMPLEMENTED + TESTED** (`src/clock.rs`, `tests/visual_goldens.rs`).

Animation is a pure function of time. `TimeSource::{real, fixed}` wraps either a
wall clock (`RealClock`) or a `FixedStepClock` whose time is exactly
`frame_index * step`. Demos advance the source once per frame, so
`--deterministic --freeze-at=N` yields a reproducible frame regardless of CPU
speed or scheduler jitter. A small motion toolkit (`phase`, `pulse`, `saw`,
`triangle`, `lerp`, `ease_in/out/in_out`, `spring`) keeps motion out of
application state. This is what makes `tests/goldens/` possible.

## 22. Sub-cell Canvases

**IMPLEMENTED + TESTED** (`src/canvas.rs`).

Two ordinary-cell raster backends, no graphics protocol:

- `BrailleCanvas` — 2×4 binary dots per cell (`U+2800`..`U+28FF`), one color per
  cell, Bresenham line/polyline, exact per-dot tests.
- `HalfBlockCanvas` — **one horizontal and two vertical RGB samples per cell**:
  the addressable grid is exactly `width` × `(2 * height)` pixels, rendered with
  `▀` (fg = top pixel, bg = bottom pixel), with explicit transparency where both
  pixels are `None`. Every horizontal pixel maps to its own cell; no column is
  discarded.

Both convert to a `Surface` and therefore flow through the normal
diff/compiler pipeline. `braille_oscilloscope` renders a waveform. Per-frame
phase updates are asserted to dirty a minority of cells (`tests/effects_perf.rs`).

## 23. Capability Model and Color Ladder

**IMPLEMENTED + TESTED** (`src/capability.rs`, `tests/capability_fallback.rs`).

`TerminalCapabilities` carries a `ColorDepth` plus tri-state `Capability`
values (`Supported` / `Unsupported` / `Unknown`) for synchronized updates,
`CSI L`, OSC 8, Kitty keyboard/graphics, Sixel and iTerm images. Detection is
passive (environment only); `Unknown` is **not** treated as supported, so the
`insert_line` fast path requires explicit support and otherwise uses the
repaint fallback.

Color intent is separated from representation: application code emits RGB or
semantic colours, and a central `quantize_style`/`quantize_color` maps them to
the session's depth. Tests prove the same UI emits `38;2` only under TrueColor,
quantizes to indexed under Ansi256, base colors under Ansi16, and no color
attributes under Mono. `Color::lerp` documents that it approximates
`Color::Reset`; `Color::lerp_resolved` refuses to fabricate a value for it.

## 24. Visual Regression Methodology

**IMPLEMENTED + TESTED** (`tests/visual_goldens.rs`).

Because time is deterministic, whole demo frames can be captured exactly. Each
demo is run as `--deterministic --freeze-at=<frame> --no-color` in a real PTY;
the complete byte stream is replayed through `vt100` and the reconstructed
**plain screen text** is compared with a small file under `tests/goldens/`.
Goldens are never updated implicitly: use
`UPDATE_GOLDENS=1 cargo test --test visual_goldens` or
`scripts/dev/update_visual_goldens.sh`, then review the diff. `tests/demo_render.rs`
keeps the cheaper structural smoke checks (panel presence, width fit,
no-truecolor under `--no-color`).

## 25. Positioned Layers and Scene Composition

**IMPLEMENTED + TESTED** (`tests/scene.rs`).

`Node::offset(x, y)` marks a node as absolutely positioned inside its parent; it
leaves flex flow, so moving a sprite never reflows its siblings. Offsets are
signed, so a layer may sit partly or fully off-screen. Painting accumulates the
offset down the subtree and clips to the parent.

The subtlety is **left/top clipping**: a node clipped on its leading edge cannot
be drawn in place (surface primitives would start at the *visible* origin and
lose the off-screen part). Such a node is rendered into a translated scratch of
its natural extent and blitted clipped, which also keeps wide-glyph invariants
intact at the boundary.

## 26. Camera Viewports

**IMPLEMENTED + TESTED** (`tests/scene.rs`).

`Node::viewport(cam_x, cam_y)` wraps a potentially oversized world, translating
it by the negative camera offset and clipping to the viewport rectangle. The
world does not shrink (its flex-shrink is pinned to 0). `ViewportState` owns the
camera (`scroll_by`, `page`, `home`, `end`, `clamp`) and is deliberately integer
and small; camera motion produces ordinary Surface diffs (no cursor tricks).

**Viewport ≠ virtualization.** A camera viewport is a *clipped translation* of a
world that has already been laid out and painted. Rendering a node clipped on its
left/top edge allocates a scratch surface sized `max(target, natural node size)`.
That is fine for bounded worlds (a diff viewport, a data city, a transcript), but
it is **not** a source-window virtualizer: a `120 × 100_000`-cell world would
still be laid out and painted in full. Virtualized source-window rendering for
giant worlds is **NOT IMPLEMENTED**; the demos do not claim otherwise.

## 27. Raster Embedding

**IMPLEMENTED + TESTED**.

`Node::raster(Surface)` / `Node::surface(Arc<Surface>)` embeds an already-rendered
surface as a scene node. The buffer is shared behind an `Arc`, so cloning a node
never duplicates it. `Surface::blit_transparent_clipped` composites with signed
origin and explicit clipping. This is the efficient path for canvases, glitch
output and damage overlays — no forced round-trip through `RichText`.

> This primitive is Rust-only for now; it is **not** claimed in the C ABI.

## 28. Vector and Sub-cell Geometry

**IMPLEMENTED + TESTED** (`src/canvas.rs`).

`BrailleCanvas` gains rectangle/filled-rectangle, circle/filled-circle, ellipse
and polygon primitives on top of Bresenham lines. These are the drawing basis
for the 3D projector, fields and particle rendering.

## 29. 3D Wireframe Projection

**IMPLEMENTED + TESTED** (`src/geom.rs`).

`Vec3`/`Transform3`/`Mesh`/`Projector` provide just enough 3D: rotate, clip
against the near plane, perspective-project, and draw edges as Braille lines.
Shape generators cover cube, octahedron, torus, general `box_xyz`, a composable
`grid_xz` circuit plane and vertical `data_tower`s. Projection rejects non-finite
coordinates, so NaNs never reach the canvas; wireframe frames are deterministic
under `FixedStepClock`. The demos rotate real geometry — no ASCII-art frames.

**Near-plane convention (single source of truth).** `depth(p) = camera_z - p.z`;
a point is visible when `depth >= near` — the near plane itself is *inclusive*.
`project` and `clip_near` both use this predicate. A segment with one endpoint
behind the plane is truncated, and the crossing is nudged a few ULPs *inside* the
visible half-space so the subsequent `project` cannot re-reject it. Regression
tests cover visible/visible, behind/behind, both crossing directions, endpoint
exactly on the near plane, very shallow crossings, NaN/infinity, and a drawn
truncated edge. This removes the old "clipped edge disappears" bug.

`Projector::project_mesh` returns `ProjectedEdge { a, b, depth }` so callers can
fake near/far styling (bright near edges, dim far edges) by drawing into two
layers. The projector remains geometry-only: no z-buffer, no occlusion claim.

## 30. Particles

**IMPLEMENTED + TESTED** (`src/particles.rs`).

A tiny deterministic particle system: a seeded xorshift64* PRNG, burst emission,
linear integration and expiry. It renders to Braille dots or cell sprites. No
thread RNG, no ECS, no physics engine; `--deterministic` reproduces exactly.

Emission is explicit: `burst` is a plain radial burst; `burst_with_life_variance`
scales lifetime only; `burst_directional(n, x, y, speed, life, heading,
angular_spread)` emits around a heading. (The old trailing `spread` parameter
scaled lifetime, not angle — it was renamed, and tests assert that lifetime
variance does not change headings.)

## 31. Procedural Fields

**IMPLEMENTED + TESTED** (`src/field.rs`).

Deterministic scalar fields (plasma, interference, radial pulse) plus an
intensity colour ramp rendered through `HalfBlockCanvas`. Under Mono the RGB is
stripped centrally and a Braille density fallback preserves shape; the fallback
now uses a deterministic 4×4 Bayer ordered dither (`render_field_braille_dithered`,
`bayer4_threshold`) so scalar structure degrades into dot *density* instead of
collapsing into solid blocks. A full-field plasma legitimately dirties most
cells; locally-moving effects stay bounded.

## 32. Text Transitions and Safe Glitch

**IMPLEMENTED + TESTED** (`src/transition.rs`, `src/glitch.rs`).

Transitions are pure `(text, t, seed)` functions over **grapheme clusters**
(type-on, dissolve, seeded scramble), exact at `t >= 1`. Glitch effects mutate
cell content only — row shift, tearing, inversion, seeded substitution — and
re-sanitize wide-glyph invariants afterwards. Glitch never emits malformed ANSI:
the transport remains owned by the ANSI compiler and `TerminalTransaction`.

## 33. Damage / Debug Model

**IMPLEMENTED + TESTED**.

Three distinct measurements must not be collapsed:

* **Exact semantic delta** — cells whose realized state actually differs,
  including changed cells in removed previous rows. Exposed as
  `SurfaceDiff::exact_changed_cell_count()` / `exact_changed_cells()`.
* **Affected footprint** — cells addressed by update semantics: explicit runs
  **∪** the region erased by `CSI K` (`erase_eol_from`) **∪** cleared trailing
  rows. This may include already-blank cells. Exposed as
  `affected_cell_count()` and the existing `logical_dirty_count()` /
  `logical_dirty_cells()` APIs. The legacy names remain supported.
* **Wire cost** — actual bytes emitted, returned by `Renderer::render` and
  tracked by the scheduler. A short erase command can address many cells.

`explicit_dirty_count()` / `explicit_dirty_cells()` count only `CellRun`
writes; they are neither the complete affected footprint nor a general exact
state-delta measure.

`Renderer` reports the affected footprint and, with `capture_damage`, records
its coordinates; `Context::last_dirty_cells()` exposes them. Fullscreen resize
re-anchoring also accounts for its canvas clear. Damage maps and `fx_lab`'s
heatmap therefore show where update operations act, not exclusively where
cell state changed. An affected-footprint percentage can exceed the new frame
area when old rows are removed; it must not be described as an exact “dirty
percentage.” Label the numerator and denominator. Damage capture is opt-in
(it allocates). The `fx_lab` damage scene and `--debug-damage` show the
footprint/wire contrast.

## 34. Capability Degradation for Effects

**IMPLEMENTED + TESTED**.

Every new effect degrades through the central color ladder: `--truecolor` RGB
plasma, `--ansi256` quantized, `--ansi16` coarse bands, `--mono` intensity/shape
only. Demos and `fx_lab` accept `--mono/--ansi16/--ansi256/--truecolor`,
`--no-sync` and `--no-insert-line` to prove fallbacks without a real terminal.
Geometry (wireframe) and motion (particles) survive with colour removed.

## 35. Deterministic Goldens for Effects

**IMPLEMENTED + TESTED**. Extends the methodology in section 24: the golden set
now covers `polished_agent` (inline by default plus one `--fullscreen` proof),
representative `hack_the_gibson` narrative beats (`--act=<name>` deterministically
fast-forwards state so each beat is capturable at a small frame count), and the
`fx_lab` regression scenes (torus, plasma, near-plane clip, wide-glyph clip,
logical-vs-wire, dithering, data city, packets, water). Goldens are reproducible
because effect time is deterministic and headers hide non-deterministic counters
(`--debug-renderer` reveals them in `fx_lab`).

## 36. Focus and Event Routing

**IMPLEMENTED (minimal) + TESTED** (`src/focus.rs`, `tests/pty_demos.rs`).

`FocusId` / `FocusRing` track which widget owns the keyboard: cycle with
`focus_next`/`focus_prev` (Tab / Shift-Tab), `set` a known id, `capture` on modal
open and `release` on close to restore the previous owner (nestable). This is
deliberately *not* a DOM/event router — applications keep their own dispatch, and
the ring only tracks identity. `polished_agent` uses it to route Tab/arrows
between the prompt and the code viewport and to capture focus while the
permission modal is open; mouse remains deferred.

## 37. Scene Algebra and Story Director

**IMPLEMENTED (EXPERIMENTAL, RUST-ONLY) + TESTED** (`src/scene.rs`, `src/story.rs`,
`tests/scene_algebra.rs`).

Scene Algebra is a small, deterministic semantics of visual change that lets
effects act on ordinary UI objects abstractly. It exists so that visual dynamism
does not have to be hand-wired into application state (`if plague_frames > 0 …`),
which forces the effect, the scene object and the application phase to know too
much about each other.

The category-theoretic language below is **not required to use the API**; it
names the composition laws the API is designed to preserve.

- **Objects** are scene states; **morphisms** are `Effect`s. `Scene` holds
  semantically identified `SceneEntity`s (stable `SceneId`, interned `TagId`s),
  each wrapping an ordinary `Node` plus z-order. A `SceneEntity` never duplicates
  `Node` functionality.
- **Composition** `f ; g` is `Effect::sequence`. `Effect::identity()` is the
  no-op, so conditional effects stay regular:
  `if cond { effect } else { Effect::identity() }`.
- **Monoidal product** `f ⊗ g` is `Effect::parallel` — run both over the same
  interval. Parallel effects on independent channels/targets commute; effects on
  the *same* channel are resolved by explicit write order, never by assuming
  commutativity.
- **Functor** `Render : SCENE → UI`: `Scene::to_node` maps a `Presentation` into
  ordinary `Node`s — a `Stack` of offset layers inside an optional camera
  `Viewport`. It is not a second renderer; it emits the declarative render objects
  that flow through the existing layout → paint → diff → ANSI path. Translating
  one sprite therefore yields a bounded framebuffer diff (asserted in
  `tests/scene_algebra.rs`), not a whole-screen repaint.
- **Presentation** is per-channel (`Position`, `Visibility`, `Camera`,
  `Custom(id)`). Effects write channels; they never mutate a `Node`. Integer cell
  coordinates are produced at evaluation time (easing is evaluated, then rounded),
  so no fractional terminal positions exist.

The **Story Director** (`src/story.rs`) models narrative as a graph of `Beat`s
whose transitions are generating morphisms; a playable session is a path through
the free category generated by that graph. User choices select different arrows
without a combinatorial state machine, and branches may reconverge. Narrative
truth is stored as **`Facts`** (semantic world state) rather than countdown
timers: `PlagueActive = true` stays true until something explicitly clears it, so
effects attach to the fact. `EffectBundle`s can be mounted/unmounted as one
semantic cause with many coordinated presentations.

**The one-arrow law.** `StoryDirector::update(dt, events)` performs at most one
categorical arrow. All events are trace-recorded in input order; the *first* event
selecting an outgoing arrow from the current beat triggers exactly one transition,
after which no further transition is evaluated that update — including a fact,
`After(Duration::ZERO)` or default transition on the newly entered beat. Remaining
events are recorded but not re-applied; call `update` again to handle them. This
guarantees every entered beat is observable for at least one update, and makes
`After(ZERO)` a deliberate one-frame beat rather than an accidental collapse. Epsilon
chaining is intentionally not provided.

**Replay is exact.** `StoryTrace` records the ordered `(dt, events)` update steps
(plus entered beats), not timestamped events. Because `update` is a pure function
of `(story, ordered steps)`, `Story::replay` reproduces the identical beat
sequence, facts, mounted bundles and final beat for **any** original cadence,
including irregular timesteps — a genuine deterministic replay theorem for the
implemented semantics.

**Graph integrity.** Entity labels within a `Scene` are unique: `Scene::add`
panics loudly on a duplicate and `Scene::try_add` returns `SceneError`. Story beat
ids and bundle names likewise cannot be silently redefined (the builders panic),
and `Story::validate` checks reference integrity (start exists, every transition
and default target exists, every mounted bundle is defined).

**Capability realization is separate from story logic.** Story and scene code are
colour-depth agnostic; the existing capability ladder realizes the same semantic
effect differently (TrueColor half-block → 256 → 16 → dithered mono).

Migration status: `polished_agent` is the restrained proof — its beats, facts,
permission ordering and overlay motion are story/scene-driven. `hack_the_gibson`
routes its tactical branch through a `StoryDirector` (branch → local facts →
reconverge on Download) and uses semantic facts for Plague presence and a real
`Replication` graph for rabbit/cookie; its *act timeline* still uses the older
`Phase` machine (partial migration).

## 38. Known Limitations

- **Raster/`Node::raster`, positioned layers and viewports are Rust-only** for now: no C ABI representation yet, and language-neutral support is not claimed.
- **Scene Algebra and Story Director are EXPERIMENTAL and Rust-only**: no FFI is exposed deliberately, because freezing an immature ABI is worse than waiting. Their semantics may change.
- **`hack_the_gibson`'s act timeline is PARTIALLY migrated**: tactical branching, Plague persistence and rabbit/cookie use the story/scene layer, but the act progression itself still uses the older `Phase` machine.
- **Replication ("rabbit") is a bounded deterministic graph**, not a general simulation: growth is capped by generation and node count and is not physically modelled.
- **Virtualization / source-window rendering is NOT IMPLEMENTED**: camera viewports clip an already-painted world; they do not virtualize giant `m × n` worlds. Bounded worlds only.
- **Damage API exposes logical vs explicit counts plus coordinates**, but there is no standalone public `DamageMap` type; the demos and `fx_lab` build heatmaps from coordinates.
- **Focus is a minimal ring, not an event router**: applications still own dispatch. Mouse is deferred.
- **Structured Markdown streaming is NOT implemented**: `polished_agent` streams plain/styled text, a semantic diff and a scrollable code viewport, not a CommonMark renderer.
- **The edge projector in `geom` is wireframe-only**: it exposes depth per edge for near/far styling. The separate experimental `raster3d` module provides filled triangles, a depth buffer, shading and occlusion (§43).
The following are **not** implemented or **not** verified. Do not describe them as complete:

- **Go wrapper parity is incomplete** — local and public Linux vet/build/example smoke passes, but there are no Go unit tests or independently installable native-library packages.
- **Windows / ConPTY is UNVERIFIED** — only Linux x86_64 (Ubuntu 24.04) was exercised.
- **tmux / screen / SSH matrix is UNVERIFIED.**
- **Active terminal capability negotiation is NOT implemented.** Capabilities are inferred passively from the environment (`NO_COLOR`, `TERM`, `COLORTERM`). `CSI L` is marked supported for non-dumb terminals, but `Unknown` is never treated as supported; device-attribute queries and graphics negotiation are future work.
- **Absolute cursor query (DSR) is NOT implemented.** Re-anchoring on resize is best-effort relative erase-from-cursor-down, not exact absolute recovery.
- **Full-screen dim veils may dirty most cells** on first appearance (a whole-screen style change); this is a deliberate, measured cost, not an accident. Moving scanlines and braille phase updates are bounded (`tests/effects_perf.rs`).
- **Public Linux CI now executes**; the former private-repository billing block is historical. Exact successful snapshots are recorded in [State of LibGibson](docs/STATE_OF_LIBGIBSON.md) and [intro validation](docs/INTRODUCTORY_CINEMA.md), not a guarantee for future commits or other platforms.
- **Hard `SIGKILL` cannot be intercepted** by any userland process.
- **Ctrl-C handling** in the interactive demos is implemented as raw-mode key events; the engine relies on RAII / panic-hook restoration for terminal state. Signal handling is not a general engine guarantee.
- **Fuzzing**: `TextInputState` has deterministic randomized edit fuzzing, but there is no `cargo-fuzz` / AFL target for arbitrary byte streams or resize storms.

## 39. Entity post-processing and SurfaceFx

**EXPERIMENTAL, Rust-only.** An ordinary `Node` carries an optional, already
realized `surface_fx` chain. `Scene::evaluate` appends the entity's Presentation
chain to a cloned node; neither the original widget nor layout changes. The
painter only allocates the extra transparent entity-sized surface when the chain
is nonempty. It paints the subtree at its natural extent, applies the chain, then
composites through the existing clipping-aware blit. No second renderer, timers,
story state, raw ANSI, arbitrary alpha, or new C ABI is involved.

`SurfaceFx` are ordered endomorphisms on realized entity surfaces. The empty
chain is identity; concatenation is associative; application is left to right.
Order is significant: successive foreground overlays choose the later colour.
Available operations are style overlay, stable fractional style mask, dim,
reverse attribute, row shift, strip tear, narrow-glyph scramble, seeded dissolve,
and scanline. These preserve transparent holes and style-only cells. Vacated
shift cells are transparent. Wide graphemes move or disappear as complete pairs.
Dissolve fraction zero hides everything; one preserves the exact original.
StyleMask selects the same kind of stable threshold but styles selected cells
instead of deleting them. Neither operation is opacity.

Masks use entity-local coordinates and an explicit seed; tag-targeted effects
mix in `SceneId` so objects get distinct reproducible masks. Clipping does not
reshuffle them. Removing a bundle yields the underlying declarative rendering
on the next frame, with cleanup provided by ordinary differential rendering.

`SurfaceFx::Scoped { mask: FxMask, effect }` restricts an ordinary operation to
entity-local cells. Rectangles clip in cells; horizontal/vertical wipes, radial
frontiers and traveling horizontal bands use normalized coordinates. Seeded
noise provides a stable alternative. Wipe/radial/noise fraction zero is identity
and one applies the entire inner effect. The fraction selects cells, not alpha.
A radial frontier is an ellipse in normalized cell coordinates; it is not a
claim of physical pixel-circular geometry.

A wide grapheme participates only when both cells are selected. Partial scopes
isolate selected input on a transparent scratch, run the inner effect, and commit
only complete output glyphs inside the selection. This prevents a tear from
importing outside text or exporting half a glyph. Outside cells stay exact;
style-only cells keep their semantics. Empty and full scopes take no-op/direct
paths. Nested masks share the full entity coordinate system. Scoping adds no
layout, clock, story, capability or terminal-protocol dependency.

Processed subtrees do not publish a hardware input cursor because a mask or tear
can invalidate its position. An unaffected focused input can retain its cursor;
this is the demo's stable command island. TextInput itself still owns ordinary
viewport scrolling. Identity-effects tests exposed and corrected trailing-edge
positioned-panel clipping: clipping a border must not redraw it inward at the visible edge.

## 40. Placement, displacement and persistent effects

`Translate` and legacy `Shake` keep their absolute placement semantics.
`Displace` and `Jitter` write a separate additive channel. Effective position is
placement (or baseline) plus the sum of rounded cell displacements. Contributions
accumulate in a wider integer and clamp only on the final conversion to cell
coordinates. Independent displacements commute; ordered surface effects do not.
A fresh `Presentation` is required per frame, as produced by StoryDirector.

`Loop(inner)` evaluates at exact nanosecond `t mod duration(inner)`. A
zero-duration inner is a no-op. Its public duration is `Duration::MAX`, a
practical persistent sentinel; remove the mounted bundle to stop it. Arithmetic
saturates for public durations, while Sequence and Repeat preserve exact elapsed
remainders at large times. Reverse a finite inner before looping; an infinite
animation has no natural final frame to reverse from.

Sequence retains completed contributions. For additive motion, successive
segments express additional displacement, not replacements for earlier values.
For SurfaceFx, a completed zero dissolve remains in the chain and still hides
later content. Remove/replace the bundle for a reveal-again lifecycle; do not
expect a later mask to undo an earlier one.

## 41. Reactions and the one-transition law

Story transitions are morphisms between beats. Reactions are endomorphisms on
the current beat. `Beat::reaction(event_condition, actions)` alters facts and
mounted bundles without changing beats or resetting beat time. Reactions use
only event conditions; story validation rejects timer/fact reaction guards.

Each update records its exact `(dt, events)` step, then executes all matching
reactions on the original beat in event, declaration, and action order. Reactions
ignore `min_duration`. Next, the first matching event transition may fire. If
none fires, at most one automatic transition is selected; its fact guards see
the final reaction facts. New-beat entry actions run last. Events are never
reprocessed against the newly entered beat. There is at most one **transition**
per update, even when multiple reactions run.

Replay remains the ordered update trace against the same Story definition.
Direct `facts_mut` and `jump_to` changes are not trace-recorded. The cinematic
demo therefore constructs inspection stages as declared starts with coherent
initial facts and bundles, and records every subsequent controller decision as
a StoryEvent. Presentation also replays, including bundle mount times.

## 42. Acid vs Crash encounter

The demo now has two deterministic reducers with different responsibilities.
`examples/acid_vs_crash/battle.rs` owns a seven-node `BattleGraph` (six local
subsystems plus a dormant mirror), influence, integrity, connectivity, visibility,
activity, resource reserves, cooldowns, planner memory and outcome quality.
`StoryDirector` owns broad dramatic acts, categorical Facts and effect-bundle
lifecycles. No hacking ontology enters the library. There are no sockets,
external hosts, credentials, system commands or persistence.

Control uses fixed-point integers from -1000 (Acid) to +1000 (Crash). Ownership
is derived at ±350 thresholds; integrity is a separate 0–1000 quantity.
Edges explicitly connect nodes and carry cost and pressure. Routing respects
severed edges and isolated endpoints. The planner scores only reachable targets,
using objective value, novelty, structural vulnerability, personality, prior
isolation, trace exposure and route cost. Stable seeded ties make selection
repeatable. A fortified intermediate node must yield before forward movement.
Repeated decoys can cause a feint: approach on a legal edge, then withdraw
without applying influence to the mirror.

The reducer applies ordered commands at an update boundary and advances exact
50ms quanta with a retained nanosecond remainder. Its finite horizon bounds
extreme-duration work; it does not promise an unbounded simulation. TRACE costs
reserves and increases awareness; isolation blocks real routes and hides local
telemetry; decoys cost reserves and become recognizable; KILL removes influence
from one lease while others survive. Final actions have prerequisites. Explicit
LET HER IN reopens a declared invitation corridor and sacrifices its control;
a timeout cannot silently reopen that corridor. Outcome metadata records costly,
clean or traced containment, temporary or decisive possession, and two kinds of
stalemate. Integrity and altered-file receipts survive the resolution.

Only categorical truths cross into Story Facts: ownership, isolation, footholds,
trace threshold, display pressure, release completion and outcome. Predeclared
Reactions project these facts and mount/unmount bundles. Continuous values stay
in the model. Milestones can move the broad acts after breathing room; fallback
timeouts preserve dramatic progress without granting control. Final-resolution
events remain immediate. Terminal acts freeze the world only after its release
and semantic projection agree.

`EncounterTrace` records the initial inspection stage and fixed seed plus every
exact dt and ordered input event batch. A fresh encounter re-runs the battlefield
reducer, derives milestone events, then replays the director. Tests compare the
entire graph/planner/resources/cooldowns/history/outcome, StoryTrace, facts,
mounted bundles, Presentation and realized frames. The narrower StoryTrace still
reproduces its semantic projection independently. Input editor and inspector
selection are UI state outside world replay. This is an in-memory example trace,
not a promised stable disk serialization format.

Rendering projects actual graph edges, broken islands, trace pulses, owned
corridors, decoy geometry and the planner's path. The ghost follows path segments;
remote typing follows sparse action-driven dialogue. Scoped SurfaceFx advance
through the existing session and display entities according to their influence.
Ordinary widgets do not contain cinematic corruption logic. A live, reachable
DISPLAY corridor is required for its mounted invasion effects. During escalation
the same map becomes a full-width hero view while session/remote entities shrink
to witnesses; default debug-free composition preserves the Crash command island.
No bounds/resize core channel was necessary: responsive node reflow plus existing
Scene displacement and entity-local effects suffice. Mono retains explicit
ownership, distinct line grammar and reverse highlights.

Shared-prefix isolation/decoy counterfactual tests require different legal paths,
targets, facts and realized screens. A severed display path remains severed even
when the story advances. Stage fixtures establish topology before Story start,
so inspection and replay share the same initial cause. Existing renderer/VT100
checks cover 56x24, 80x24, 120x32 and 160x40 plus live resize. PTYs exercise actual
commands, opponent adaptation and terminal restoration. These establish behavior
within the tested environment, not subjective cinematic quality on all terminals.

Fullscreen geometry changes now clear the invalidated physical canvas in the
renderer transaction before compiling a fresh diff. Fresh diffs omit default
blanks; without the clear, old map fragments survived resize behind dissolved
panels. Renderer affected-footprint accounting includes the entire clear, while
SurfaceDiff's exact semantic delta remains its separate framebuffer comparison.
A following identical frame still emits zero bytes. Flow-widget clipping and
all preexisting visual snapshots are preserved.

## 43. RGB subcell graphics and filled depth

**IMPLEMENTED + TESTED, EXPERIMENTAL, Rust-only** (`raster`, `raster3d`,
`raster_fx`). These generators end at an ordinary `Surface`; they do not own
terminal transport, story state, or a second cell compositor.

`RgbRaster` stores opaque RGB pixel samples. A terminal region of W×H cells
normally uses W×2H pixels: each `▀` uses foreground for the upper sample and
background for the lower sample. Software RGB interpolation happens before
cell realization, so this introduces no terminal alpha. Pixel dimensions are
explicit and capped at 2048 per axis. Out-of-bounds pixel writes are ignored;
line/disc work is bounded by the raster. Odd final rows use black for the
missing lower pixel. `write_ppm` is an optional dependency-free inspection path.

`TriangleMesh` supplies indexed faces, with box/cube/octahedron constructors.
`Rasterizer` uses the existing Vec3/Transform3 vocabulary. A look-at camera
transforms into positive-forward camera depth; six frustum planes clip before
projection. Pixel-center barycentric interpolation operates on reciprocal Z,
then reconstructs camera depth for the actual depth buffer. The buffer is
cleared with color and metrics. Exact coplanar ties have a stable color tie-break.
Degenerate/nonfinite geometry and invalid cameras are rejected; finite hostile
coordinates cannot produce coordinate-sized raster walks. Depth-tested lines
share the same buffer. Optional backface culling is independent of correctness.

Flat face lighting is ambient + diffuse·max(0,n·l) + emissive, with RGB
saturation and linear camera-depth fog. This is intentionally small software
rendering, not a material system. Metrics expose submitted/drawn triangles and
Z tests. Drawn counts depend on whether a submitted face writes pixels; they
are work counters, not a canonical count of final visible faces.

`RasterFx` are ordered endomorphisms on an RGB raster: empty is identity;
concatenation applies A then B and generally does not commute. Glow, chromatic
split, sine warp, vignette and scanlines operate before cell realization.
`RasterFxWorkspace` retains one source scratch shared across the chain. Glow is
bounded to radius three. Finite-safe radial/metaball/vortex/ring functions and
a palette interpolator provide fields without introducing a shader language.
`SurfaceFx` remains the separate layer of grapheme/cell transformations.

`FeedbackBuffer` is explicit state, not a pure Scene effect. Each update decays
floating RGB history by an explicit half-life and adds the supplied emission;
conversion saturates to RGB bytes. Zero dt is identity. Reset and changes to
effective (clamped) dimensions clear history; resizing to the same effective
dimensions preserves it. Exact replay means the same ordered dt/emission inputs; arbitrary
repartitioning of emission updates is not claimed equivalent. Black input
predictably decays, including sub-byte energy; NaNs never enter the buffer.

`MAX_RASTER_DIMENSION` is a hard allocation bound, not a recommended operating
size. A maximum 2048×2048 feedback buffer holds 96 MiB of floating energy plus
12 MiB of RGB output (about 108 MiB before allocator overhead). Normal terminal
rasters are orders of magnitude smaller.

TrueColor preserves RGB; ANSI256/ANSI16 quantize in the existing ANSI compiler.
Mono has a separate luminance-to-Braille ordered-dither realization rather than
solid white half blocks. Color is not the only ownership signal: Acid, contest,
and isolated labels retain distinct glyph grammar. Full animated rasters can
legitimately change broad regions. Exact delta, affected footprint and wire
cost remain distinct (§33); identical frozen graphics still emit zero bytes.

## 44. One world, cinematic realizations

The demo-local `presentation.rs` chooses a `ShotPlan` from BattleGraph and
recorded visual history. This replaces the default flat-UI/dive distinction;
legacy projections remain explicitly selectable. Nothing in shot selection
changes the graph, planner, actions, story, facts, bundles or outcome.

Shots cover establishment, arrival, route contest, node closeup, trace,
isolation, decoy, evasion, DISPLAY assault, final duel, three resolutions and
aftermath. Each selects a camera pose, focal node, light/field exposure and
label policy. Stable subsystem positions preserve anchors across shots.
`world_geom.rs` gives those anchors distinct silhouettes: gateway rings,
switching prism, nested vault, sloped console, data stacks/mirror, and portal.
Integrity removes pieces; foreign influence misaligns layers and changes the
light field. Filled faces and emissive structural rails share camera/depth;
a sparse, depth-checked Braille overlay adds finer edges over opaque RGB mass.
No generic engine API was added for this art direction.

`ShotHistory` is derived update state with a recorded prior camera, selected
shot, action cue, exposure and entry clock. It updates even for zero-duration
semantic commands. Actions get a bounded 2.4-second attention window; ordinary
shot changes have a minimum hold. Camera and exposure interpolate over 1.25
seconds from the prior presentation. Viewport dimensions adjust framing at
paint time without changing history. Outcome/aftermath time is explicit.
Exact update replay rebuilds the shot/camera sequence and final raster; no
wall clock or paint count participates. This is exact ordered-input replay,
not equivalence under arbitrary timestep repartitioning.

`cyber.rs` remains the common raster generator. Acid's actor follows planner
progress along valid graph edges; normal traffic dots have separate phases.
Trace light follows the reverse path. Incomplete feint curves are presentation
of a real feint tactic, not secretly connected graph edges. Isolation exposes
broken routes and bounded sparks. A one-level semantic topology/trace engraving
lives in DISPLAY's geometry, and an ordinary text echo of the live input can
appear at its projected position with the existing entity SurfaceFx. The real
input buffer is never modified by the echo or Acid's actor.

The compositional reading is `C(W, shot)`: text, topology, depth and fields are
projections of the same semantic object W, selected together by the shot.
They are not separate copies of narrative truth. Three opaque command rows
are the stable local-control boundary. Richer receipts/scars appear only in
aftermath or debug inspection. Uppercase shortcuts leave lowercase typed
commands untouched; numeric final shortcuts are active only on empty input.

VisualHistory retains at most 48 world-space light samples. Painting rebuilds
feedback in the current camera/dimensions; resize reprojects and repeated paint
adds no energy. The raster is bounded to 320×240 RGB samples. Whole moving
camera/raster views can change broadly, while identical frozen frames still
have zero exact delta, affected footprint and emitted bytes (§33).

Crash resolution pulls outward and preserves damage, Acid assembles tiny
bitmap lettering from illuminated raster fragments of the current machine,
and stalemate creates a stationary interference boundary. This is original
demo-specific choreography, not arbitrary mesh morphing or a font engine.
Aftermath is a compact scar/trace/replay view over the same architecture.

Legacy `--presentation=legacy` retains the original automatic dive;
`--visual=flat|cyber` retains individual historical projections for comparison.
The cinematic default starts with geometry already present. TrueColor, central
ANSI quantization and Mono density/bright structural rails share world truth;
color fidelity and perceptual equivalence across terminals are not guaranteed.

## 45. Seekable introductory cinema and shared routes

The introductory demo realizes one finite job graph as Node UI, a refracted
Surface, depth-tested information architecture and a planetary title composition.
Its demo-local cue sheet derives presentation from explicit time; seeking does not
replay hidden paint-time mutations. It intentionally uses no growing per-frame
StoryTrace because this linear film has no interactive narrative branches.

`geom::CubicPath3` supplies shared world-space interpolation and unit tangents for
semantic couriers and camera attention. It is pure, clamped and finite-safe; it
does not introduce actors, clocks or another scene graph into core. The runner
reuses held Nodes while the presentation key is unchanged. The film now keeps
identity, scalar membrane, facade planes, named camera subshots and title art in
demo-local modules. Transient hints use explicit presentation time; painting never
starts their timer. The world model and core APIs are unchanged by art direction.
Full details and validation are in [Introductory cinema](docs/INTRODUCTORY_CINEMA.md).

The accompanying core hardening preserves Rect's saturated half-open extent
contract. Exact SurfaceDiff spans now retain erasures/removals independently of
ANSI patches, so coordinate enumeration agrees with the semantic delta count.
Affected footprint still includes already-blank erased cells; wire cost is still
measured separately. The added public Rust metadata field changes exhaustive
struct-literal construction; the C ABI is unchanged.
