# LibGibson (`termframe`)

> **"Treat a terminal like a small character-cell framebuffer."**

LibGibson is a language-neutral, high-performance terminal UI and differential rendering engine designed for modern agent CLIs, developer tools, and interactive consoles.

LibGibson treats the terminal as a 2D logical cell framebuffer with an explicit architectural separation between **mutable live interactive state** and **immutable terminal scrollback history**.

---

## The Mental Model

Traditional terminal libraries either treat the screen as an alternate-buffer fullscreen application (destroying the user's terminal history) or rely on primitive `println!` / ANSI helper macros that corrupt screen state when output updates asynchronously.

LibGibson introduces a clean vertical pipeline. The `TerminalTransaction` at the end is what makes each frame a single atomic write, and the explicit **anchor** is what lets the renderer know where its live region physically is:

```
    UI tree (declarative Node hierarchy)
        │
        ▼
    Layout (Taffy Flexbox integer cell engine)
        │
        ▼
    Surface (Grapheme-aware 2D cell framebuffer)
        │
        ▼
    Diff (Previous-frame vs Next-frame cell comparison)
        │
        ▼
    AnsiCompiler (stateful minimal patch stream)
        │
        ▼
    TerminalTransaction (sync-update begin/end + cursor motion +
        │                diff bytes + cursor placement/visibility,
        │                written as ONE write_all + flush)
        ▼
    Terminal  ← anchored by Renderer::AnchorState
```

The renderer's anchor (`AnchorState::{Invalid, Stable{cols,rows,live_height}}`) records the physical live region and the last committed cursor position/visibility. That matters because **an unchanged framebuffer does not mean "nothing to emit"** — a cursor move or visibility change is physical truth and is still written.

### The Flagship Invariant: Mutable Live Region vs Immutable Scrollback

LibGibson enforces a strict invariant:

```
┌─────────────────────────────────────────────────────────────┐
│ COMMITTED TERMINAL SCROLLBACK                               │
│ Immutable from the engine's perspective                     │
│                                                             │
│ - Prior tool outputs                                        │
│ - Finalized assistant messages                              │
│ - Historical user commands                                  │
├─────────────────────────────────────────────────────────────┤
│ LIVE MUTABLE REGION                                         │
│ - Animated spinner / status lines                           │
│ - Streaming LLM token blocks                                │
│ - Interactive menus / permission selectors                  │
│ - Editable grapheme-aware text inputs                       │
│                                                             │
│ Differential framebuffer rendering + explicit anchor        │
└─────────────────────────────────────────────────────────────┘
```

When live content is finalized, you call:

```rust
ctx.commit_text("Finalized output text")?; // ctx.commit(...) is an alias
```

**Upon commitment:**

- The committed lines transition permanently into the terminal's native scrollback.
- The committed lines are purged from the mutable live framebuffer (the anchor is invalidated).
- Repainting a 3-line live prompt costs the same whether there are 10 lines or 100,000 lines in the session scrollback: cost is independent of transcript size.

---

## Verification Status

Core engine behavior is **IMPLEMENTED + TESTED on Linux x86_64 only**. The repository currently runs **121 tests**: 59 library unit tests and 62 integration tests (across `commit_invariance`, `demo_render`, `diff_golden`, `ffi_lifecycle`, `non_tty_redirection`, `pty_integration`, `pty_resize_torture`, `resize_torture`, `screen_state_vt100`, `structured_output`, and `whole_renderer_vt100`).

`cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, and `cargo build --release` are clean. The C and C++ examples compile and run under AddressSanitizer + UndefinedBehaviorSanitizer (LeakSanitizer disabled), and the Python `ctypes` example runs. The **Go bindings are UNVERIFIED** — no Go toolchain was available, so they were never compiled. Windows, tmux/screen/SSH, terminal capability negotiation, and DSR absolute anchoring are **not** verified or implemented. See [Current Platform Support & Limitations](#current-platform-support--limitations).

---

## Core Features

- **Grapheme-Aware Cell Model** (TESTED): Unicode grapheme clusters (`unicode-segmentation`), CJK full-width characters (`display_width == 2`), zero-width combining marks, and emojis.
- **Wide Glyph Overwrite Protection** (TESTED): Writing into a cell occupied by or adjacent to a wide character safely clears orphaned continuation cells.
- **Explicit Physical Anchor** (TESTED): The renderer tracks `AnchorState` plus last cursor position/visibility; empty cell diffs still emit when cursor state changes. Geometry changes invalidate the anchor and trigger a re-anchor.
- **Structured `RichText` / `Line` / `Span` + Semantic `Theme::styles()`** (TESTED): Style roles (`text`, `muted`, `faint`, `accent`, `success`, `warning`, `error`, `border`, `rail`, `code`, `link`, `selection`) instead of hardcoded ANSI. `muted` is default foreground + dim, readable on light and dark terminals. `Theme::no_color()` is available.
- **Visual composition helpers** (TESTED): `gibson::show` provides gradient spans, sub-cell progress meters (`▏▎▍▌▋▊▉█`), sparklines (`▁▂▃▄▅▆▇█`) and deterministic hex dumps as plain `Span`s/`Line`s — no widgets and no new layout semantics. `Node::panel` adds titled bordered containers and `Color::lerp` enables gradients. All degrade to zero color under `--no-color`.
- **Static structured output** (TESTED): `commit_text`, `commit_rich_text`, and `commit_node` route through the same width-aware layout/wrapping engine as live nodes. Control characters in text are neutralized at the cell model boundary, so they cannot inject `ESC`/`OSC`/`CSI`. `commit_raw_ansi_unchecked` is the explicit escape hatch.
- **Taffy-Powered Flexbox Layout** (TESTED): Flex containers (`Row`, `Column`), percentage width, min/max constraints, padding, gap, alignment, justification, and intrinsic text measurement with word wrapping.
- **Stateful Differential ANSI Compiler** (TESTED): Groups dirty cells into contiguous runs, computes minimum-distance cursor repositioning, uses `CSI K` when content shrinks, and wraps diff emission in autowrap disabling (`CSI ? 7 l/h`). Synchronized-update (`CSI ? 2026 h/l`) ownership lives in `TerminalTransaction`, not the compiler.
- **Atomic `TerminalTransaction`** (TESTED): Batches sync-update markers, temporary private modes, cursor motion, framebuffer diff, final cursor placement, and cursor visibility into one `write_all` + `flush`.
- **Asynchronous Scrollback Insertion** (`insert_before_live`) (TESTED): Two named strategies — `InsertLineFastPath` uses `CSI L` to insert history above the live region without repainting it, and `RepaintFallback` erases, prints, repaints, and restores the exact cursor. Strategy counts are exposed in metrics. This is **not** universally zero-repaint: the fallback repaints when the anchor is untrustworthy or space is insufficient.
- **Grapheme-Based TextInput** (TESTED): Single-line input with display-width-aware navigation, horizontal scrolling, and bracketed paste. Mutations are byte-range edits and the cursor grapheme index is re-derived from the full resulting string, so boundary-merging insertions (combining accents, ZWJ emoji, skin-tone modifiers, flags) preserve `cursor_grapheme <= grapheme_count`. Single-line paste normalizes `\r\n`, `\r`, and `\n` to a space.
- **Scheduler / Runtime** (TESTED as a module): `DEFAULT_ANIMATION_INTERVAL = 80ms` (60 FPS is an input-latency ceiling, not a spinner target). `Context::run_once(max_wait)` waits up to the next frame deadline, gives input priority, then renders if due. `render_if_due`, `request_render`, `animation_interval`, and `frame_budget` are available. The demos use these instead of `render() + sleep()`.
- **Clean Plain-Text Degradation** (TESTED): Detects redirected output and suppresses interactive escapes while emitting clean plain text with zero escape sequences. The internal ANSI stripper is for engine-generated output only, **not** a sanitizer for untrusted input.
- **Language-Neutral Rich Text ABI** (TESTED for C/C++/Python; Go UNVERIFIED): Opaque `gibson_line_t` / `gibson_rich_text_t` with span/align builders. No wrapping logic is duplicated outside Rust.
- **Versioned C ABI** (TESTED): `GIBSON_ABI_VERSION = 1`, `gibson_abi_version()`, `gibson_stats_init()`. Enum-like inputs cross as raw `int32` and are validated. `gibson_get_stats` validates the ABI version and refuses an undersized buffer instead of overflowing it.
- **Safe Terminal Lifecycle** (TESTED): RAII guard plus a global panic hook restore raw mode, cursor visibility, alternate buffer, and bracketed paste on normal exit, error, or Rust panic. Interactive demo Ctrl-C is handled as a raw-mode key event; see limitations.

---

## Quickstart

### 1. Rust Usage

Add `libgibson` to your `Cargo.toml`:

```rust
use gibson::cell::{Line, RichText, Span, Style, Theme};
use gibson::context::Context;
use gibson::node::Node;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = Context::inline()?;
    let styles = Theme::default().styles();

    // Commit structured historical output (width-aware, control-safe)
    ctx.commit_text("[system] Session initialized.")?;
    ctx.commit_rich_text(
        &RichText::new().line(
            Line::new()
                .span(Span::styled("status ", styles.muted))
                .span(Span::styled("ready", styles.success)),
        ),
    )?;

    // Build declarative live UI
    let root = Node::col().child(Node::text("Active task execution", Style::default()));
    ctx.set_root(root);

    // Minimal runtime step: waits for input up to the frame deadline, then renders if due.
    let _event = ctx.run_once(Duration::from_millis(100))?;

    // Finalize
    ctx.commit_text("[task] Completed successfully.")?;
    ctx.restore()?;
    Ok(())
}
```

For a full loop, call `ctx.run_once(max_wait)` repeatedly; input wakes it up immediately, and animation is throttled by `ctx.animation_interval()`.

### 2. Native C Usage

Include `gibson.h` and link `-lgibson`:

```c
#include "gibson.h"

int main(void) {
    gibson_context_t *ctx = NULL;
    gibson_create_context(GIBSON_MODE_INLINE, &ctx);

    gibson_commit_text(ctx, "Starting session...");

    gibson_node_t *root = NULL;
    gibson_node_box_col(&root);

    gibson_node_t *text = NULL;
    gibson_node_text("Hello from C FFI", NULL, GIBSON_WRAP_WORD, &text);
    gibson_node_add_child(root, text);

    gibson_set_root_node(ctx, root);
    gibson_render(ctx);

    gibson_commit_text(ctx, "Finished.");
    gibson_destroy_context(ctx);
    return 0;
}
```

For metrics, initialize the versioned struct before reading it:

```c
gibson_stats_t stats;
gibson_stats_init(&stats);
gibson_get_stats(ctx, &stats);
```

### 3. Modern C++ RAII

```cpp
#include "gibson.hpp"

int main() {
    gibson::Context ctx(GIBSON_MODE_INLINE);
    ctx.commit_text("C++ RAII session started");

    auto root = gibson::Node::col();
    root.add_child(gibson::Node::text("C++ Modern Interface"));

    ctx.set_root(std::move(root));
    ctx.render();

    ctx.commit_text("Done.");
    return 0;
}
```

### 4. Python (`ctypes`)

```python
from gibson import Context, Node

with Context() as ctx:
    ctx.commit_text("Python session active")
    root = Node.col()
    root.add_child(Node.text("Hello from Python"))
    ctx.set_root(root)
    ctx.render()
    ctx.commit_text("Finished.")
```

---

## Running the Demos

```bash
# Restrained flagship product demo (recommended starting point):
cargo run --example polished_agent
cargo run --example polished_agent -- --auto          # deterministic timeline

# Maximalist twin, built on the same primitives:
cargo run --example hack_the_gibson
cargo run --example hack_the_gibson -- --auto

# Theme proofs:
cargo run --example polished_agent -- --light
cargo run --example polished_agent -- --dark
cargo run --example polished_agent -- --no-color

# Interactive resize exercise (drives the anchor/re-anchor path):
cargo run --example resize_test_app
```

`polished_agent` and `hack_the_gibson` are now **full-screen dashboards**: they own the character-cell framebuffer and render persistent multi-panel UIs with gradient/shimmer banners, sub-cell progress meters, live sparklines, hex dumps, scan sweeps, an event feed and a prompt/selector footer. `polished_agent` demonstrates responsive layout (40/60/80/120/160 columns), a stable-height task plan, streaming `RichText`, tool operations with queued/running/success/warning/error states, an async background event, a permission selector with wraparound + number keys, persistent Unicode input, background events while typing, Ctrl-C cancel, and measured telemetry. `hack_the_gibson` uses zero raw ANSI literals and adds a tactical payload selector and a root shell. Both support `--inline` to exercise the scrollback `insert_before_live` path and `--auto`/`--scripted` for deterministic runs; theme proofs are `--light`, `--dark`, `--no-color` (which emits no color at all).

---

## Building and Testing

```bash
# Build library and release artifacts (.so, .a)
cargo build --release

# Run the full test suite (121 tests: 59 unit + 62 integration)
cargo test

# Static analysis and formatting checks
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

### Compiling Language Examples

```bash
# Compile and run C example:
gcc -Iinclude bindings/c/example.c -Ltarget/release -lgibson \
    -Wl,-rpath,'$ORIGIN/../../target/release' -o bindings/c/example_c
./bindings/c/example_c

# Compile and run C++ example:
g++ -std=c++17 -Iinclude bindings/cpp/example.cpp -Ltarget/release -lgibson \
    -Wl,-rpath,'$ORIGIN/../../target/release' -o bindings/cpp/example_cpp
./bindings/cpp/example_cpp

# Run Python example:
PYTHONPATH=bindings/python python3 bindings/python/example.py
```

The C and C++ examples are exercised under AddressSanitizer + UndefinedBehaviorSanitizer with LeakSanitizer disabled. For example:

```bash
gcc -fsanitize=address,undefined -fno-sanitize=leak -Iinclude \
    bindings/c/example.c -Ltarget/release -lgibson \
    -Wl,-rpath,'$ORIGIN/../../target/release' -o /tmp/example_c_asan
ASAN_OPTIONS=detect_leaks=0 /tmp/example_c_asan
```

The Go bindings in `bindings/go/` are source-only and **UNVERIFIED** because no Go toolchain was available to compile them.

---

## Project Structure

```
.
├── Cargo.toml
├── LICENSE
├── LICENSES-THIRD-PARTY.md
├── README.md
├── DESIGN.md
├── ROADMAP.md
├── include/
│   ├── gibson.h             # Canonical C ABI header
│   └── termframe.h          # Compatibility header (tf_* aliases)
├── src/
│   ├── lib.rs               # Library root and re-exports
│   ├── cell.rs              # Grapheme, Glyph, Style, Line, RichText, Theme
│   ├── surface.rs           # 2D cell grid with wide overwrite + control neutralization
│   ├── layout.rs            # Flexbox engine (Taffy bridge, word wrapping)
│   ├── node.rs              # Declarative UI tree and component nodes
│   ├── painter.rs           # Surface painter and cursor tracking
│   ├── diff.rs              # Framebuffer differential comparator
│   ├── ansi.rs              # Minimal ANSI escape sequence compiler
│   ├── transaction.rs       # Atomic TerminalTransaction (one write_all + flush)
│   ├── renderer.rs          # Inline/fullscreen renderer, anchor, commit, insertion
│   ├── session.rs           # Terminal lifecycle, raw mode, RAII drop guard
│   ├── input.rs             # Event polling and grapheme-aware TextInput
│   ├── scheduler.rs         # Frame throttling and telemetry statistics
│   ├── context.rs           # High-level engine coordinator
│   └── ffi.rs               # extern "C" ABI implementation
├── bindings/
│   ├── c/                   # Native C example
│   ├── cpp/                 # Modern C++ RAII header and example
│   ├── python/              # Python ctypes wrapper and example
│   └── go/                  # Go cgo wrapper and example (UNVERIFIED)
├── examples/
│   ├── polished_agent.rs    # Flagship product demo
│   ├── hack_the_gibson.rs   # Maximalist demo (no raw ANSI literals)
│   ├── agent_chat.rs        # Legacy interactive agent chat demo
│   ├── resize_test_app.rs   # Interactive resize / re-anchor exercise
│   └── perf_probe.rs        # Benchmark and invariance probe
└── tests/
    ├── whole_renderer_vt100.rs # Full Renderer -> Transaction -> vt100 screen tests
    ├── screen_state_vt100.rs   # Surface -> Diff -> AnsiCompiler vt100 tests
    ├── pty_integration.rs      # Real PTY session lifecycle tests
    ├── pty_resize_torture.rs   # PTY resize storm with assertions
    ├── diff_golden.rs          # Minimal ANSI patch & diff tests
    ├── commit_invariance.rs    # Scrollback separation & O(1) diff tests
    ├── demo_render.rs         # Full-screen demo PTY -> vt100 structure checks
    ├── ffi_lifecycle.rs        # C ABI lifecycle, validation, hostile-input tests
    ├── non_tty_redirection.rs  # Plain-text degradation tests (zero escapes)
    ├── structured_output.rs    # commit_text/rich/node layout parity + control safety
    └── resize_torture.rs       # Rapid resize and narrow-terminal torture tests
```

---

## Current Platform Support & Limitations

- **Platforms exercised**: Linux x86_64 (Ubuntu 24.04) only, with the Rust, C, C++, and Python toolchains.
- **Terminals**: the engine targets ANSI-compatible terminals and was exercised under Linux terminals on x86_64. A broad terminal matrix (xterm, Alacritty, Kitty, WezTerm, iTerm2, GNOME Terminal, Windows Terminal, …) is **not** automatically verified.
- **Windows / ConPTY**: **UNVERIFIED**. Only Linux x86_64 was exercised.
- **tmux / screen / SSH**: **UNVERIFIED**.
- **Go bindings**: source updated but **UNVERIFIED** — no Go compiler was installed, so they were never compiled or run.
- **Terminal capability negotiation**: **NOT implemented**. The fast insertion path assumes `CSI L` support; this is not probed at runtime. On a terminal without it, or when the anchor is untrustworthy or space is insufficient, the always-correct `RepaintFallback` is used.
- **Absolute cursor anchoring**: **NOT implemented**. Resize re-anchoring is a best-effort erase-from-cursor-down rebuild, not an absolute DSR query.
- **Signals**: RAII and the panic hook restore terminal state on normal exit, errors, and Rust panics. Interactive demos handle Ctrl-C as a raw-mode key event. Hard `SIGKILL` (`kill -9`) cannot be intercepted by any userland process; this is an operating-system boundary.

---

## License

Licensed under either of:

- Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (http://opensource.org/licenses/MIT)

at your option. Both license texts are included in [LICENSE](LICENSE).
