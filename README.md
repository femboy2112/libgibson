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

Core engine behavior is **IMPLEMENTED + TESTED on Linux x86_64 only**. The repository currently runs **464 tests**: 226 library unit tests and 238 integration tests (across `acid_battle`, `acid_battlefield`, `acid_battlefield_goldens`, `acid_render`, `acid_story`, `scene_cinematic`, `story_reactions`, `surface_fx`, `capability_fallback`, `commit_invariance`, `compositor`, `demo_render`, `diff_golden`, `effects_perf`, `ffi_lifecycle`, `non_tty_redirection`, `pty_demos`, `pty_integration`, `pty_resize_torture`, `resize_torture`, `safety_api`, `scene`, `scene_algebra`, `screen_state_vt100`, `structured_output`, `visual_goldens`, and `whole_renderer_vt100`).

`cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, and `cargo build --release` are clean. The C and C++ examples compile and run under AddressSanitizer + UndefinedBehaviorSanitizer (LeakSanitizer disabled), and the Python `ctypes` example runs. The **Go bindings are UNVERIFIED** — no Go toolchain was available, so they were never compiled. Windows, tmux/screen/SSH, terminal capability negotiation, and DSR absolute anchoring are **not** verified or implemented. See [Current Platform Support & Limitations](#current-platform-support--limitations).

---

## Core Features

- **Grapheme-Aware Cell Model** (TESTED): Unicode grapheme clusters (`unicode-segmentation`), CJK full-width characters (`display_width == 2`), zero-width combining marks, and emojis.
- **Wide Glyph Overwrite Protection** (TESTED): Writing into a cell occupied by or adjacent to a wide character safely clears orphaned continuation cells.
- **Explicit Physical Anchor** (TESTED): The renderer tracks `AnchorState` plus last cursor position/visibility; empty cell diffs still emit when cursor state changes. Geometry changes invalidate the anchor and trigger a re-anchor.
- **Structured `RichText` / `Line` / `Span` + Semantic `Theme::styles()`** (TESTED): Style roles (`text`, `muted`, `faint`, `accent`, `success`, `warning`, `error`, `border`, `rail`, `code`, `link`, `selection`) instead of hardcoded ANSI. `muted` is default foreground + dim, readable on light and dark terminals. `Theme::no_color()` is available.
- **Layer compositor** (TESTED): `Node::stack()` overlays children in one content box with **explicit** cell transparency (`Cell::transparent`). Transparent cells leave the lower layer untouched; `Node::dim()` is a style-only veil. Overlay removal is diff-driven and leaves no ghost cells; a floating modal does not reflow the layout beneath it. `Stack`/`Dim` have C ABI constructors (`gibson_node_stack`, `gibson_node_dim`).
- **Sub-cell canvases** (TESTED): `BrailleCanvas` (2×4 dots/cell) and `HalfBlockCanvas` (1 horizontal × 2 vertical RGB samples per cell, so the grid is `width` × `2*height` pixels, via `▀`), with Bresenham lines/polylines and exact glyph/colour tests. No graphics protocol required.
- **Deterministic clock** (TESTED): `TimeSource::{real,fixed}` + `FixedStepClock` and a small motion toolkit (`phase`, `pulse`, `saw`, `triangle`, easings). Scripted frames are `frame * step`, enabling reproducible goldens.
- **Scene composition** (TESTED): `Node::offset(x,y)` positions a node absolutely inside its parent (out of flow) with signed, clipped placement; `Node::viewport(cam_x,cam_y)` is a clipped camera; `Node::raster(Arc<Surface>)`/`Node::surface` embeds a prebuilt surface. `ViewportState` owns pan/page/home/end + clamping. (`tests/scene.rs`)
- **FX substrate** (TESTED): deterministic `geom` 3D wireframe projector (cube/octahedron/torus/box/grid/data-tower, inclusive near-plane clipping, per-edge depth), seeded `particles` (radial / life-variance / directional bursts), procedural `field` (plasma/interference + heat ramp + Bayer-dithered mono fallback), grapheme-safe `transition` helpers (type-on/dissolve/scramble), and safe `glitch` primitives (row shift/tear/invert/scramble) that mutate only cell content.
- **Damage inspection** (TESTED): logical damage (`SurfaceDiff::logical_dirty_count`/`logical_dirty_cells`, including erase-to-EOL and cleared rows) plus explicit run counts and independently measured wire bytes; `Renderer::capture_damage`/`Context::last_dirty_cells()` expose per-frame logical coordinates for debug overlays (the demos visualize the renderer's own damage).
- **Minimal focus** (TESTED): `FocusId`/`FocusRing` cycle (Tab/Shift-Tab), capture on modal open and restore on close, without a DOM/event-router. (`src/focus.rs`)
- **Clip containment** (TESTED): `blit_transparent_clipped` places a width-2 glyph only when lead **and** continuation lie inside the clip; otherwise it is suppressed, so wide glyphs cannot leak a continuation past a raster node, viewport or positioned layer.
- **Capability & color ladder** (TESTED): `TerminalCapabilities` with tri-state `Capability` values and `ColorDepth` (TrueColor/Ansi256/Ansi16/Mono). A central quantizer makes the same UI degrade cleanly; `Unknown` is never treated as supported for `CSI L` insertion.
- **Visual composition helpers** (TESTED): `gibson::show` provides gradient spans, sub-cell progress meters (`▏▎▍▌▋▊▉█`), sparklines (`▁▂▃▄▅▆▇█`) and deterministic hex dumps as plain `Span`s/`Line`s — no widgets and no new layout semantics. `Node::panel` adds titled bordered containers and `Color::lerp` enables gradients. All degrade to zero color under `--no-color`.
- **Static structured output** (TESTED): `commit_text`, `commit_rich_text`, and `commit_node` route through the same width-aware layout/wrapping engine as live nodes. Control characters in text are neutralized at the cell model boundary, so they cannot inject `ESC`/`OSC`/`CSI`. `commit_raw_ansi_unchecked` is the explicit escape hatch.
- **Taffy-Powered Flexbox Layout** (TESTED): Flex containers (`Row`, `Column`), percentage width, min/max constraints, padding, gap, alignment, justification, and intrinsic text measurement with word wrapping.
- **Stateful Differential ANSI Compiler** (TESTED): Groups dirty cells into contiguous runs, computes minimum-distance cursor repositioning, uses `CSI K` when content shrinks, and wraps diff emission in autowrap disabling (`CSI ? 7 l/h`). Synchronized-update (`CSI ? 2026 h/l`) ownership lives in `TerminalTransaction`, not the compiler.
- **Atomic `TerminalTransaction`** (TESTED): Batches sync-update markers, temporary private modes, cursor motion, framebuffer diff, final cursor placement, and cursor visibility into one `write_all` + `flush`.
- **Asynchronous Scrollback Insertion** (TESTED): The **safe** entry points are `insert_text_before_live`, `insert_rich_text_before_live` and `insert_node_before_live`; the only raw path is explicitly `insert_raw_lines_before_live_unchecked`. Two named strategies — `InsertLineFastPath` uses `CSI L` to insert history above the live region without repainting it, and `RepaintFallback` erases, prints, repaints, and restores the exact cursor. Strategy counts are exposed in metrics. This is **not** universally zero-repaint: the fallback repaints when the anchor is untrustworthy or space is insufficient.
- **Grapheme-Based TextInput** (TESTED): Single-line input with display-width-aware navigation, horizontal scrolling, and bracketed paste. Mutations are byte-range edits and the cursor grapheme index is re-derived from the full resulting string, so boundary-merging insertions (combining accents, ZWJ emoji, skin-tone modifiers, flags) preserve `cursor_grapheme <= grapheme_count`. Single-line paste normalizes `\r\n`, `\r`, and `\n` to a space.
- **Scheduler / Runtime** (TESTED as a module): `DEFAULT_ANIMATION_INTERVAL = 80ms` (60 FPS is an input-latency ceiling, not a spinner target). `Context::run_once(max_wait)` waits up to the next frame deadline, gives input priority, then renders if due. `render_if_due`, `request_render`, `animation_interval`, and `frame_budget` are available. The demos use these instead of `render() + sleep()`.
- **Clean Plain-Text Degradation** (TESTED): Detects redirected output and suppresses interactive escapes while emitting clean plain text with zero escape sequences. The internal ANSI stripper is for engine-generated output only, **not** a sanitizer for untrusted input.
- **Language-Neutral Rich Text ABI** (TESTED for C/C++/Python; Go UNVERIFIED): Opaque `gibson_line_t` / `gibson_rich_text_t` with span/align builders. No wrapping logic is duplicated outside Rust.
- **Versioned C ABI** (TESTED): `GIBSON_ABI_VERSION = 1`, `gibson_abi_version()`, `gibson_stats_init()`. Enum-like inputs cross as raw `int32` and are validated. `gibson_get_stats` validates the ABI version and refuses an undersized buffer instead of overflowing it.
- **Safe Terminal Lifecycle** (TESTED): RAII guard plus a global panic hook restore raw mode, cursor visibility, alternate buffer, and bracketed paste on normal exit, error, or Rust panic. Interactive demo Ctrl-C is handled as a raw-mode key event; see limitations.
- **Bounded 2D line clipping** (TESTED): `clip_line_to_bounds` runs Liang–Barsky before Bresenham in both sub-cell canvases, so a finite near-camera projection with coordinates in the tens of millions draws only its visible portion instead of walking millions of steps, and near-`i32`-extreme endpoints cannot overflow. Hostile regression tests in `src/canvas.rs`.
- **Honest damage accounting** (TESTED): `SurfaceDiff` exposes `exact_changed_cell_count` (a true per-cell state delta) and `affected_cell_count` (cells *addressed* by update semantics — explicit runs ∪ erase-to-EOL ∪ cleared rows, which may exceed the live area when rows are removed). The earlier over-strong "every visible cell whose state changes" wording is corrected.
- **Scene Algebra** (TESTED, EXPERIMENTAL, Rust-only): `Scene`/`SceneEntity`/`SceneId`/`TagId` wrap ordinary `Node`s with identity; `Effect` provides `identity`, `sequence` (composition) and `parallel` (monoidal product) over presentation channels. `Scene::to_node` is the `Render : SCENE → UI` functor — it emits ordinary nodes through the existing pipeline, and moving one entity produces a bounded framebuffer diff rather than a whole-screen repaint.
- **Story Director** (TESTED, EXPERIMENTAL, Rust-only): `Facts` store semantic world state (not countdown timers), `Beat`/`Condition`/`Transition` form a free-category story graph where user choices branch and reconverge, and `StoryTrace` records exact `(dt, events)` update steps so `Story::replay` reproduces a session deterministically at any cadence. `StoryDirector::update` follows **at most one transition per call** (see DESIGN §37).
- **Real replication primitive** (TESTED): `Replication` is a bounded, deterministic branching graph with freeze and neutralize; the Hackers rabbit/cookie interaction is a real entity, not a generic particle burst.

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
# Cinematic Scene Algebra flagship: Crash's UI becomes contested territory
cargo run --example acid_vs_crash
cargo run --example acid_vs_crash -- --auto
cargo run --example acid_vs_crash -- --deterministic --stage=display-intrusion
cargo run --example acid_vs_crash -- --stage=climax --color=mono

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

`acid_vs_crash` is an offline fictional terminal short film viewed from Crash
Override's machine. Acid Burn gradually occupies the same ordinary map, session,
and event panels through Scene-mounted SurfaceFx. Type `trace`, `isolate`,
`isolate auth`, `decoy`, `kill`, or `hard isolate`; the story keeps moving while
you type. Final moves are `cut link`, `turn trace`, `spring decoy`, and
`let her in`. The bottom command island remains usable during display takeover.
After resolution, `replay`, `facts`, `trace`, `damage`, `scene`, `acid`, `crash`,
`reset`, and `exit` inspect the encounter. These are simulated semantic commands;
there is no networking, exploitation, or host shell execution.

`--auto` runs a state-based defender against Acid's deterministic planner on the
same live topology. Influence changes gradually; isolation severs real paths,
trace consumes reserves and provokes evasion, and a familiar decoy can draw a
feint. The action surface shows prerequisites, cooldowns and costs. StoryDirector
sets the dramatic pace while the graph decides what actually happens.

`--deterministic` fixes time without replacing the human player. Add
`--debug-battle` (or `--debug-ai`) to inspect goals, candidate scores, routes and
influence. Inspection hooks include
`--stage=quiet|signature|route-contested|first-breach|display-intrusion|decoy|trace|trap|climax|takeover|crash-win|acid-win|stalemate`,
`--freeze-at=N`, `--seconds=N`, `--speed=N`, and
`--color=mono|ansi16|ansi256|truecolor`. For a short movie smoke run, use
`--auto --speed=20`. Stage aliases preserve inspection entry points; several
now start within the same broad act. After the battle, `replay` checks the full
graph, planner memory, world state and story against exact recorded inputs.
Scene/Story/SurfaceFx remain **EXPERIMENTAL, Rust-only**.
See the [round II evidence record](docs/acid-vs-crash-round2-validation.md) and
[round I record](docs/acid-vs-crash-validation.md) for measured coverage and
limitations. FX Lab scene 20 isolates the generic substrate; `r` attaches or
removes an effect through an in-beat reaction.

`polished_agent` is **inline by default** (`--fullscreen` opts into the alternate screen): the session header and the typed user request are committed to *real terminal scrollback*, while a mutable live foreground shows a task plan (queued/running/done/warn/failed/skipped), streaming `RichText`, a scrollable code/diff viewport (`Tab` focuses it; arrows/PageUp/PageDown/Home/End scroll), an event feed, an interactive permission modal that captures and restores focus, persistent Unicode input and a completion summary — including a real failure/recovery loop. `hack_the_gibson` remains fullscreen and is an act-based *Hackers* (1995) homage: a projected Gibson data city with near/far depth cues, the Plague, the Da Vinci worm, a pirate broadcast, a Grand Central-style coordinated attack with packet particles, Joey's garbage-file download, a safe geometry-collapse crash, a rooftop-pool particle payoff and a CRASH AND BURN curtain — followed by a real root shell whose commands trigger reusable effects. It uses zero raw ANSI literals and never glitches the wire protocol (effects mutate Surface state only). Overlays composite through the `Stack`/raster layer engine without reflowing the dashboard beneath. `examples/fx_lab.rs` is a developer gallery (`n`/`p` or `[`/`]` cycle scenes, `1`-`9`/`0` jump) that now includes near-plane clipping, wide-glyph clip containment, logical-damage-vs-wire-cost (`--debug-damage`), mono dithering, data city, packet routes and water particles. Demos support `--auto`/`--scripted`/`--deterministic --freeze-at=<frame>`, hack's `--act=<name>` for deterministic beats, capability overrides (`--mono`/`--ansi16`/`--ansi256`/`--truecolor`, `--no-sync`, `--no-insert-line`) and theme proofs `--light`, `--dark`, `--no-color`.

---

## Building and Testing

```bash
# Build library and release artifacts (.so, .a)
cargo build --release

# Run the full test suite (464 tests: 226 unit + 238 integration)
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
