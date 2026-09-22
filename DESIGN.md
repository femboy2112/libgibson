# LibGibson Architecture & Design Document

## 1. Overview and Core Philosophy

LibGibson was designed to solve a fundamental deficiency in modern CLI applications: the friction between **terminal scrollback history** and **interactive mutable user interfaces**.

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
    Mutable frame-buffered region (diff-rendered, cursor-rewound)
```

---

## 2. The Rendering Pipeline

The rendering lifecycle follows a strictly decoupled vertical pipeline:

```
┌─────────────────────────┐
│     Declarative Tree    │  Node hierarchy (Box, Row, Col, Text, Spinner, TextInput)
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│     Taffy Flexbox       │  Maps declarative nodes to integer Rects
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│       Cell Painter      │  Paints laid-out nodes into next Surface framebuffer
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│     Differential Diff   │  Compares previous Surface with next Surface
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│      ANSI Compiler      │  Emits stateful minimal cursor & SGR update byte stream
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│     Terminal Output     │  Synchronized flush (CSI ? 2026 h/l) to stdout
└─────────────────────────┘
```

---

## 3. Cell and Glyph Model

Terminal cells cannot be represented as simple `char` or ASCII byte arrays. Unicode requires handling:
- Multi-byte UTF-8 sequences.
- Extended grapheme clusters (e.g., base characters with combining accents: `e` + `\u{0301}` -> `é`).
- Wide full-width CJK characters (`display_width == 2`).
- Emoji sequences with Zero-Width Joiners (ZWJ) and skin tone modifiers (`display_width == 2`).
- Zero-width codepoints.

### Memory Optimization: `CompactString`

A naive implementation using `String` per cell allocates on every cell mutation. LibGibson uses `CompactString` (from the `compact_str` crate) inside `Glyph`:
- 24 bytes total on 64-bit platforms.
- Completely stack-allocated for all strings up to 24 UTF-8 bytes (covering >99.9% of all Unicode grapheme clusters and emojis).
- Zero heap allocations during cell construction and updates.

### Wide Glyph Overwrite Safety

A double-width character occupies two consecutive terminal columns: `(x, y)` as the lead glyph, and `(x+1, y)` as a continuation marker. If software overwrites half of a wide character, terminals can render corrupt text.

LibGibson enforces four strict invariants in `Surface::set_cell`:
1. **Overwriting a continuation cell**: If cell `(x, y)` is a continuation, the lead character at `(x-1, y)` is automatically cleared to a blank space.
2. **Overwriting a wide lead**: If cell `(x, y)` is a wide lead and is overwritten with a single-width character, the continuation cell at `(x+1, y)` is automatically cleared.
3. **Inserting a wide character**: If a wide character is placed at `(x, y)`, and `(x+1, y)` was previously the lead of another wide glyph, `(x+2, y)` is cleared so no orphaned continuation remains.
4. **Right-edge clipping**: If a wide character is placed at the rightmost boundary (`x + 1 >= width`), it cannot fit and is safely clipped rather than corrupting line wraps.

---

## 4. Layout Engine

LibGibson integrates the `taffy` crate (v0.14) without exposing Taffy types across the public API.

- **Flexbox Containers**: Supports `FlexDirection::Row` and `FlexDirection::Column`, flex grow, flex shrink, gap, padding, `align_items`, and `justify_content`.
- **Intrinsic Text Measurement**: Text nodes implement custom leaf measurement in Taffy:
  - `WrapMode::NoWrap`: Intrinsic width equals the maximum line length; height equals line count.
  - `WrapMode::WordWrap`: Wraps on word and whitespace boundaries; falls back to grapheme breaks when a single word exceeds container width.
  - `WrapMode::CharWrap`: Wraps strictly at grapheme cluster boundaries.

---

## 5. Differential Diff & ANSI Compiler

LibGibson never repaints unchanged cells and never clears the entire screen on every frame.

### Diff Algorithm (`diff.rs`)

1. For each row `y`:
   - Compare `prev_cells[x]` with `next_cells[x]`.
   - Skip rows where all cells are identical.
   - For rows with changes, group contiguous dirty cells into `CellRun { x, cells }`.
2. **Erase to End of Line Optimization**: If a line shrinks in width (e.g. status changes from `"Processing query 12345"` to `"Done"`), the diff detects that trailing cells became blank and emits `erase_eol_from: Some(x)`, compiling to `\x1b[K` (`CSI K`) instead of emitting dozens of space characters.
3. **Orphaned Row Clearing**: If the live region shrinks in height from $H_1$ to $H_2$, the diff identifies `rows_to_clear = H1 - H2` and erases the leftover rows below with `\x1b[K`.

### Stateful ANSI Compiler (`ansi.rs`)

The `AnsiCompiler` maintains active terminal state:
- `cursor_x`, `cursor_y`
- Current `Style` (foreground, background, bold, dim, italic, underline, reverse)
- `sync_updates` flag

Key optimizations:
- **Cursor Motion**: Compares relative forward/backward jumps (`\x1b[C`, `\x1b[D`), absolute column positioning (`\x1b[<col>G`), and carriage returns (`\r`), selecting the minimal byte sequence.
- **SGR Minimization**: Emits style change codes only when attributes differ from current terminal state. Attributes are not reset between characters.
- **Continuation Cells**: Continuation cells in double-width glyphs are omitted from the ANSI stream because standard terminals advance hardware cursors by 2 columns automatically.
- **Synchronized Updates**: Wraps frame output in `\x1b[?2026h` and `\x1b[?2026l` for flicker-free atomic updates on supported terminals.

---

## 6. Inline Mode vs Fullscreen Mode

### Inline Mode (Flagship)
- Coexists with normal shell history.
- The live region has height $H$.
- On frame 0, $H - 1$ newlines are emitted to allocate terminal lines without overwriting scrollback, followed by cursor rewind `\x1b[{H-1}A\r`.
- Subsequent frame diffs rewind the cursor strictly within the live region (`\x1b[{y}A\r`).
- Cursor rewind is strictly bounded by live region height.

### Fullscreen Mode
- Enters alternate screen buffer (`\x1b[?1049h`).
- Dimensions match terminal rows and columns.
- Reuses the identical Surface, Layout, Painter, Diff, and ANSI compiler pipelines.

---

## 7. Commit Semantics ($O(1)$ Scrollback Invariance)

When `ctx.commit(text)` is called:
1. The terminal cursor is rewound to row 0 of the active live region.
2. The live region rows are cleared with `\x1b[K`.
3. The committed text lines are emitted directly to stdout, followed by newlines.
4. The committed text is now in the native terminal scrollback buffer.
5. The mutable live framebuffer is completely purged (`previous_surface = None`, `live_region_height = 0`).
6. The subsequent live frame begins on the line below the committed text.

Because committed text is never retained in the mutable framebuffer, **the rendering cost of a live prompt or spinner is completely independent of the length of the transcript history**.

---

## 8. Input and TextInput Component

The `TextInputState` component handles keyboard interactions using grapheme cluster semantics:
- Cursor position is measured in grapheme indices, not byte offsets.
- Deleting backwards over a combining mark (`é`) or multi-codepoint emoji deletes the entire grapheme cluster.
- Supports printable Unicode insertion, Left, Right, Home, End, Ctrl-A, Ctrl-E, Backspace, Delete, and bracketed paste blocks.
- Computes horizontal scroll offsets so the cursor remains visible inside constrained bounding boxes.

---

## 9. C ABI Boundary and Foreign Language Safety

The C ABI is designed around strict safety invariants:
1. **Opaque Pointers**: `gibson_context_t` and `gibson_node_t` hide internal Rust layouts.
2. **Explicit Data Widths**: All integers use `int32_t`, `uint32_t`, `uint64_t`, or `float`.
3. **No Panics Across FFI**: Every public `extern "C"` function is wrapped in `std::panic::catch_unwind`. If an internal panic occurs, it is captured, a thread-local error message is recorded, and `GIBSON_ERR_PANIC` is returned.
4. **Memory Ownership**: Explicit free functions (`gibson_node_free`, `gibson_destroy_context`) ensure no cross-allocator mismatches.

---

## 10. Visual Doctrine & Clean-Room Design Philosophy

Modern developer tools and agent CLIs (such as Claude Code) succeed through **restraint, typographic clarity, and native terminal immersion**. LibGibson codifies these principles into its core visual doctrine:

1. **Native Background Respect**:
   - The default terminal background must remain transparent or default.
   - Never draw solid dark or colored rectangular canvas backgrounds over the entire viewport; terminal users select customized color themes, transparencies, and background blurs.
   - Background colors are strictly reserved for subtle highlights (e.g. text input cursor focus or active option selection).

2. **The Rail Callout Doctrine (`Node::rail`)**:
   - Heavy ASCII boxes (`╭───╮`, `│   │`, `╰───╯`) consume 2 vertical lines and 2 horizontal columns of screen real estate per box. In long terminal sessions, stacked boxes clutter the scrollback.
   - LibGibson introduces the **Rail** primitive (`│` left-border callout):
     ```
     ─── Tool Execution: AST Code Search ──────────────────────────────
     │ ● Query: sync_update in src/ansi.rs
     │ ● Result: Found DECSM 2026 atomic batching; 0 tear frames.
     ```
   - Rails provide clean visual containment with 0 wasted top/bottom rows and minimal visual weight.

3. **Structured Text Layout (`RichText`, `Line`, `Span`, `Theme`)**:
   - No hardcoded ANSI string literals (`\x1b[31m`) in user interfaces.
   - Components compose semantic styles via `Theme` tokens (`theme.accent`, `theme.text_muted`, `theme.rail`, `theme.success`, `theme.warning`, `theme.error`).
   - Enables instant theme switching and guaranteed zero-escape plain-text rendering for pipes and non-TTY outputs.

4. **Zero-Escape Non-TTY Redirection**:
   - When stdout is piped to a file or CI runner (`app > out.txt` or `app | cat`), interactive escape sequences (cursor movement, clear line, SGR colors) create unreadable log garbage.
   - In non-TTY mode, LibGibson automatically suppresses live interactive frames and converts all committed structured nodes and rich text to clean, plain UTF-8 text with zero ANSI escapes.

---

## 11. Asynchronous Scrollback Insertion (`insert_before_live`)

In real-world agent CLIs, asynchronous background events occur while the user is actively typing or while a live spinner is spinning (e.g., git filesystem change notifications, LSP diagnostics, streaming log messages).

Naive terminal applications either:
1. Print directly, which corrupts the live region and leaves orphaned lines.
2. Buffer until the user finishes typing, delaying critical notifications.

LibGibson's `insert_before_live` solves this with differential surgical precision:
1. Temporarily rewinds the hardware cursor to row 0 of the active live region.
2. Clears the live region rows using `CSI K`.
3. Emits the inserted lines directly into native terminal scrollback, followed by `\r\n`.
4. Advances the active line position downward by the number of inserted lines.
5. Re-renders the live surface without invalidating `previous_surface`.
6. Compiles a minimal differential patch that repaints the live region at its new row offset and restores the hardware cursor.
7. Subsequent frames diff against the preserved surface with zero dirty cells.

---

## 12. Terminal Autowrap and Right-Margin Safety

When text or background cells reach column `width - 1` (the rightmost column of the terminal window), standard VT100/ANSI terminal emulators trigger autowrap (DECAWM): the hardware cursor advances to column 0 of the *next* row, scrolling the terminal window up if already at the bottom.

In differential rendering, this creates disastrous screen tearing and vertical drift.

LibGibson protects against right-margin autowrap through two interlocking mechanisms:
1. **DECAWM Autowrap Disabling**: Diff byte compilation wraps all rendering in `\x1b[?7l` (disable autowrap) before emitting cell runs, and `\x1b[?7h` (re-enable autowrap) after restoring cursor position.
2. **Surface Right-Edge Clipping**: Wide CJK characters and emojis occupying 2 columns are clipped if `x + 1 >= width`, preventing double-width glyphs from crossing the terminal boundary.

