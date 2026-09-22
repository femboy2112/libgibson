# LibGibson (`termframe`)

> **"Treat a terminal like a small character-cell framebuffer."**

LibGibson is a language-neutral, high-performance terminal UI and differential rendering engine designed for modern agent CLIs, developer tools, and interactive consoles.

LibGibson treats the terminal as a 2D logical cell framebuffer with an explicit architectural separation between **mutable live interactive state** and **immutable terminal scrollback history**.

---

## The Mental Model

Traditional terminal libraries either treat the screen as an alternate-buffer fullscreen application (destroying the user's terminal history) or rely on primitive `println!` / ANSI helper macros that corrupt screen state when output updates asynchronously.

LibGibson introduces a clean vertical pipeline:

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
    ANSI Compiler (Stateful minimal patch stream)
        │
        ▼
    Terminal (Synchronized atomic frame updates)
```

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
│ 100% differential framebuffer rendering                      │
└─────────────────────────────────────────────────────────────┘
```

When live content is finalized, you call:

```rust
ctx.commit("Finalized output text")?;
```

**Upon commitment:**
- The committed lines transition permanently into the terminal's native scrollback.
- The committed lines are **completely purged from the mutable live framebuffer**.
- All cursor rewinds and diff calculations are **strictly bounded to the active live region**.
- Repainting a 3-line live prompt costs exactly the same whether there are 10 lines or 100,000 lines in the session scrollback: **$O(1)$ with respect to transcript size**.

---

## Core Features

- **Grapheme-Aware Cell Model**: Robust support for Unicode grapheme clusters (`unicode-segmentation`), CJK full-width characters (`display_width == 2`), zero-width combining marks, and emojis.
- **Wide Glyph Overwrite Protection**: Writing into a cell occupied by or adjacent to a wide character safely clears orphaned continuation cells, preventing screen corruption.
- **Taffy-Powered Flexbox Layout**: Flex containers (`Row`, `Column`), dimensions, padding, gap, alignment, justification, and intrinsic text measurement with word wrapping.
- **Stateful Differential ANSI Compiler**:
  - Emits zero bytes when frames are identical.
  - Groups dirty cells into contiguous runs.
  - Calculates minimum-distance cursor repositioning (relative vs absolute horizontal jumps).
  - Uses `CSI K` (erase to end of line) when content shrinks.
  - Wraps frames in synchronized updates (`CSI ? 2026 h/l`) for flicker-free rendering.
- **Grapheme-Based TextInput**: Single-line text input with grapheme-cluster navigation (Left, Right, Home, End, Ctrl-A, Ctrl-E, Backspace, Delete), horizontal scrolling, and bracketed paste.
- **Safe Terminal Lifecycle**: RAII guard restores raw mode, cursor visibility, alternate buffer, and bracketed paste on normal exit, error, Ctrl-C, or Rust panic.
- **Clean Plain-Text Degradation**: Automatically detects redirected output (`stdout | cat`, CI logs) and suppresses interactive escape sequences while emitting clean plain text.
- **Stable Language-Neutral C ABI**: Opaque handles, `#[repr(C)]` types, explicit integer widths, thread-local error messages, and `catch_unwind` safety. Call easily from C, C++, Python, Go, and any FFI-capable language.

---

## Quickstart

### 1. Rust Usage

Add `libgibson` to your `Cargo.toml`:

```rust
use gibson::context::Context;
use gibson::cell::{Style, Color};
use gibson::node::{Node, BorderType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = Context::inline()?;

    // Commit historical output
    ctx.commit("[system] Session initialized.")?;

    // Build declarative live UI
    let root = Node::col()
        .child(Node::spinner(0, Style::new().fg(Color::Cyan), Some("Processing...")))
        .child(Node::border_box(BorderType::Rounded, Style::default())
            .child(Node::text("Active task execution", Style::default())));

    ctx.set_root(root);
    ctx.render()?;

    // Finalize and commit
    ctx.commit("[task] Completed successfully.")?;

    ctx.restore()?;
    Ok(())
}
```

### 2. Native C Usage

Include `gibson.h` and link `-lgibson`:

```c
#include "gibson.h"

int main(void) {
    gibson_context_t *ctx = NULL;
    gibson_create_context(GIBSON_MODE_INLINE, &ctx);

    gibson_commit(ctx, "Starting session...");

    gibson_node_t *root = NULL;
    gibson_node_box_col(&root);

    gibson_node_t *text = NULL;
    gibson_node_text("Hello from C FFI", NULL, GIBSON_WRAP_WORD, &text);
    gibson_node_add_child(root, text);

    gibson_set_root_node(ctx, root);
    gibson_render(ctx);

    gibson_commit(ctx, "Finished.");
    gibson_destroy_context(ctx);
    return 0;
}
```

### 3. Modern C++ RAII

```cpp
#include "gibson.hpp"

int main() {
    gibson::Context ctx(GIBSON_MODE_INLINE);
    ctx.commit("C++ RAII session started");

    auto root = gibson::Node::col();
    root.add_child(gibson::Node::text("C++ Modern Interface"));

    ctx.set_root(std::move(root));
    ctx.render();

    ctx.commit("Done.");
    return 0;
}
```

### 4. Python (`ctypes`)

```python
from gibson import Context, Node

with Context() as ctx:
    ctx.commit("Python session active")
    root = Node.col()
    root.add_child(Node.text("Hello from Python"))
    ctx.set_root(root)
    ctx.render()
    ctx.commit("Finished.")
```

---

## Running the Demo

Run the flagship demonstration showcasing streaming token updates, live spinners, keyboard permission selection, and interactive text input:

```bash
# Interactive mode (requires interactive TTY):
cargo run --example agent_chat

# Automated / non-interactive headless mode:
cargo run --example agent_chat -- --auto
```

Run the asymptotic performance probe measuring diff reduction and scrollback size invariance:

```bash
cargo run --example perf_probe
```

---

## Building and Testing

```bash
# Build library and release artifacts (.so, .a)
cargo build --release

# Run comprehensive test suite (43 unit & integration tests)
cargo test

# Run static analysis and formatting checks
cargo clippy --all-targets
cargo fmt --check
```

### Compiling Language Examples

```bash
# Compile and run C example:
gcc -Iinclude bindings/c/example.c -Ltarget/release -lgibson -Wl,-rpath,'$ORIGIN/../../target/release' -o bindings/c/example_c
./bindings/c/example_c

# Compile and run C++ example:
g++ -std=c++17 -Iinclude bindings/cpp/example.cpp -Ltarget/release -lgibson -Wl,-rpath,'$ORIGIN/../../target/release' -o bindings/cpp/example_cpp
./bindings/cpp/example_cpp

# Run Python example:
PYTHONPATH=bindings/python python3 bindings/python/example.py
```

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
│   ├── cell.rs              # Grapheme, Glyph, Style, Cell models
│   ├── surface.rs           # 2D cell grid with wide overwrite protection
│   ├── layout.rs            # Flexbox engine (Taffy bridge, word wrapping)
│   ├── node.rs              # Declarative UI tree and component nodes
│   ├── painter.rs           # Surface painter and cursor tracking
│   ├── diff.rs              # Framebuffer differential comparator
│   ├── ansi.rs              # Minimal ANSI escape sequence compiler
│   ├── renderer.rs          # Inline & fullscreen renderer + commit logic
│   ├── session.rs           # Terminal lifecycle, raw mode, RAII drop guard
│   ├── input.rs             # Event polling and grapheme-aware TextInput
│   ├── scheduler.rs         # Frame throttling and telemetry statistics
│   ├── context.rs           # High-level engine coordinator
│   └── ffi.rs               # extern "C" ABI implementation
├── bindings/
│   ├── c/                   # Native C example
│   ├── cpp/                 # Modern C++ RAII header and example
│   ├── python/              # Python ctypes wrapper and example
│   └── go/                  # Go cgo wrapper and example
├── examples/
│   ├── agent_chat.rs        # Flagship interactive agent CLI demo
│   └── perf_probe.rs        # Benchmark and asymptotic invariance probe
└── tests/
    ├── pty_integration.rs   # Real PTY terminal session lifecycle tests
    ├── diff_golden.rs       # Minimal ANSI patch & diff unit tests
    ├── commit_invariance.rs # Scrollback separation & O(1) diff tests
    ├── ffi_lifecycle.rs     # C ABI repeated creation and safety tests
    └── non_tty_redirection.rs # Plain-text degradation tests
```

---

## Current Platform Support & Limitations

- **Platforms Tested**: Linux x86_64 (tested on Ubuntu 24.04 with rustc nightly, gcc 13.3, clang 18.1, python 3.12).
- **Terminals Supported**: xterm, Alacritty, Kitty, WezTerm, iTerm2, GNOME Terminal, Windows Terminal, and ANSI-compatible terminals.
- **Go Bindings**: Coherent cgo source provided; marked UNVERIFIED in report because local Go compiler was not installed.
- **Signals**: RAII cleans up on panics, normal exits, and errors. Hard `SIGKILL` (`kill -9`) cannot be intercepted by any userland process; this is an operating system boundary.

---

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE) or http://opensource.org/licenses/MIT)

at your option.
