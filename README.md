<div align="center">

<img src="docs/assets/libgibson-intro-finale.png" alt="LibGibson intro finale — the &quot;libGibson&quot; wordmark over a network Earth with a &quot;HACK THE PLANET!&quot; end card, rendered entirely in Unicode cells and terminal colour" width="820">

# LibGibson

**Treat a terminal like a small character-cell framebuffer.**

A cell-framebuffer terminal UI engine with native scrollback / live-region
semantics — plus experimental compositional animation and software graphics.

[![release](https://img.shields.io/github/v/release/femboy2112/libgibson?include_prereleases&sort=semver&label=release&color=8a2be2)](https://github.com/femboy2112/libgibson/releases/latest)
[![CI](https://img.shields.io/github/actions/workflow/status/femboy2112/libgibson/ci.yml?branch=main&label=CI)](https://github.com/femboy2112/libgibson/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-Rust%201.85-orange)](docs/RELEASE_CONTRACT.md)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

</div>

> **Current release: [v0.1.1 — Engineering Alpha](https://github.com/femboy2112/libgibson/releases/tag/v0.1.1)** (2026-09-26).
> Linux x86_64. A native SDK archive is attached to the release. LibGibson is
> distributed as **GitHub source + that SDK** — it is *not* on crates.io, PyPI, or
> the Go module proxy. See the [changelog](CHANGELOG.md) and
> [install options](#install--use) below.

> **Engineering alpha.** Linux x86_64 is the best-tested environment. The core
> engine is implemented and tested; Scene, Story, and software graphics are
> experimental, Rust-only APIs. *Capability is not an API-stability promise* — see
> [status & support](#status--support).

---

## Why LibGibson

Traditional terminal libraries force a choice: take over the whole screen with an
alternate buffer (destroying the user's scrollback), or sprinkle `println!` and ANSI
helpers that corrupt the display the moment output arrives asynchronously.

LibGibson refuses the choice. It treats the terminal as a **2D logical cell
framebuffer** with a strict architectural split between **mutable live interactive
state** and **immutable terminal scrollback history**:

```
┌─────────────────────────────────────────────────────────────┐
│ COMMITTED TERMINAL SCROLLBACK                                │
│ Immutable from the engine's perspective                     │
│   prior tool output · finalized messages · past commands    │
├─────────────────────────────────────────────────────────────┤
│ LIVE MUTABLE REGION                                         │
│   spinners/status · streaming tokens · menus & modals       │
│   grapheme-aware text input                                  │
│   → differential framebuffer rendering + explicit anchor    │
└─────────────────────────────────────────────────────────────┘
```

When live content is finalized you call `ctx.commit_text(...)` (aliased
`ctx.commit(...)`): the committed lines transition permanently into the terminal's
**native** scrollback and are purged from the live framebuffer. The payoff:

> Repainting a 3-line live prompt costs the same whether there are 10 lines or
> 100,000 lines behind it. **Live-region cost is independent of transcript size.**

The rendering path is a clean vertical pipeline — a single atomic write per frame,
with an explicit physical **anchor** so the renderer always knows where its live
region actually is:

```
  UI tree (declarative Node hierarchy)
      │  Layout (Taffy Flexbox, integer cells)
      ▼  Surface (grapheme-aware 2D cell framebuffer)
  Diff (previous vs next frame, per cell)
      │  AnsiCompiler (stateful minimal patch stream)
      ▼  TerminalTransaction (sync-update + cursor motion + diff bytes,
  Terminal      as ONE write_all + flush) ← anchored by Renderer::AnchorState
```

An unchanged framebuffer does **not** mean "nothing to emit": a cursor move or a
visibility change is physical truth and is still written.

## Feature highlights

The core engine is **implemented and tested on Linux x86_64**. A few of the load-bearing pieces:

- **Grapheme-aware cell model** — Unicode clusters, CJK width, combining marks, emoji,
  with wide-glyph overwrite protection.
- **Differential ANSI rendering** — minimal per-cell diff, contiguous-run batching,
  and one atomic `TerminalTransaction` per frame with synchronized-update framing.
- **Scrollback insertion without repaint** — `CSI L` fast path to insert history above
  a live region, with an always-correct repaint fallback.
- **Flexbox layout** (Taffy) — rows/columns, constraints, padding, gaps, alignment,
  word wrapping, plus a layer compositor with explicit cell transparency.
- **Sub-cell graphics** — `BrailleCanvas` (2×4) and `HalfBlockCanvas` (1×2 RGB) with
  Bresenham lines and bounded clipping; **no image protocol required**.
- **Sub-cell glyph realization** — one 2×4 mask realized down a portability ladder
  (`Braille → HalfBlock → Block → ASCII`), because glyph *realization* is a separate
  axis from terminal *protocol* (the Linux VT proves it). See [`docs/GLYPHS.md`](docs/GLYPHS.md).
- **Capability & colour ladder** — `TrueColor → ANSI256 → ANSI16 → Mono` degrade
  through one central quantizer.
- **Safe terminal lifecycle** — an RAII guard + owner-thread panic hook restore raw
  mode, cursor, alt-buffer, and bracketed paste on exit, error, and panic.
- **Versioned C ABI** — `GIBSON_ABI_VERSION = 1`, ELF SONAME `libgibson.so.1`,
  validated inputs, with C / C++ / Python / Go wrappers.
- **Experimental, Rust-only** — Scene algebra (`Render : SCENE → UI` functor), a Story
  director with deterministic replay, and a software-graphics FX substrate.

📖 **Full tested-feature catalog: [`docs/FEATURES.md`](docs/FEATURES.md).**

## The flagship demo

A 72-second terminal short film that turns a live agent harness into a wireframe city
of information and pulls back to Earth:

```sh
cargo run --release --example libgibson_intro -- --auto --color=truecolor
```

By default it opens with a **first-contact prologue**: an ordinary terminal boots a
credible (explicitly *simulated*) agent harness into immutable scrollback, that
information visibly *acquires structure* as a live LibGibson region attaches, and the
completed pose **match-cuts** into the fullscreen film at its existing `t = 0`. The
boot log stays in scrollback after the film exits. `--no-prologue` (or
`--stage`/`--at`/`--dump`) enters the film directly. It is a local simulation drawn in
Unicode and terminal colour — no image protocol. The hero image above is a real frame
of this film ([how it was captured](docs/assets/README.md)).

[Film structure, controls, and engineering notes →](docs/INTRODUCTORY_CINEMA.md)

## Install & use

LibGibson ships as **GitHub source plus a native SDK archive** on the
[release](https://github.com/femboy2112/libgibson/releases/tag/v0.1.1). It is not
published to any package registry.

### Rust

Not on crates.io — depend on the tagged GitHub source. The package is `libgibson`; the
crate you `use` is `gibson`:

```toml
[dependencies]
libgibson = { git = "https://github.com/femboy2112/libgibson", tag = "v0.1.1" }
```

```rust
use gibson::cell::{Line, RichText, Span, Style, Theme};
use gibson::context::Context;
use gibson::node::Node;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = Context::inline()?;
    let styles = Theme::default().styles();

    // Commit structured history (width-aware, control-safe) into real scrollback.
    ctx.commit_text("[system] Session initialized.")?;
    ctx.commit_rich_text(&RichText::new().line(
        Line::new()
            .span(Span::styled("status ", styles.muted))
            .span(Span::styled("ready", styles.success)),
    ))?;

    // Declarative live UI.
    ctx.set_root(Node::col().child(Node::text("Active task execution", Style::default())));

    // One runtime step: wait for input up to the frame deadline, render if due.
    let _event = ctx.run_once(Duration::from_millis(100))?;

    ctx.commit_text("[task] Completed successfully.")?;
    ctx.restore()?;
    Ok(())
}
```

For a full loop, call `ctx.run_once(max_wait)` repeatedly — input wakes it immediately;
animation is throttled by `ctx.animation_interval()`.

### Native SDK (C / C++ / Python / Go)

The release attaches `libgibson-0.1.1-linux-x86_64.tar.gz` (with a `.sha256`): headers,
`libgibson.a`, the versioned shared object (`libgibson.so.1` + a `libgibson.so` dev
symlink), and a relocatable `pkg-config` file.

```bash
sha256sum -c libgibson-0.1.1-linux-x86_64.tar.gz.sha256
tar -xzf libgibson-0.1.1-linux-x86_64.tar.gz          # -> ./libgibson-0.1.1-linux-x86_64/
export PKG_CONFIG_PATH="$PWD/libgibson-0.1.1-linux-x86_64/lib/pkgconfig:$PKG_CONFIG_PATH"

cc app.c $(pkg-config --cflags --libs libgibson) -o app   # C consumer
```

```c
#include "gibson.h"
int main(void) {
    gibson_context_t *ctx = NULL;
    gibson_create_context(GIBSON_MODE_INLINE, &ctx);
    gibson_commit_text(ctx, "Hello from C FFI");
    gibson_node_t *root = NULL; gibson_node_box_col(&root);
    gibson_node_t *text = NULL;
    gibson_node_text("Live node", NULL, GIBSON_WRAP_WORD, &text);
    gibson_node_add_child(root, text);
    gibson_set_root_node(ctx, root);
    gibson_render(ctx);
    gibson_destroy_context(ctx);
    return 0;
}
```

- **C++** — `#include "gibson.hpp"`; `gibson::Context` is an RAII wrapper that verifies
  the loaded ABI on construction.
- **Python** — a `ctypes` wrapper that loads a **separately installed** native
  LibGibson. Not on PyPI; the release attaches the built wheel and sdist as
  clearly-marked wrapper packages (`*-py3-none-any.whl`, `*-python-sdist.tar.gz`).
  See [`bindings/python/README.md`](bindings/python/README.md).
- **Go** — a cgo module that consumes the same SDK via `#cgo pkg-config: libgibson`. Not
  on the Go module proxy. See [`bindings/go/README.md`](bindings/go/README.md).

> These wrappers expose the established UI/output subset of the C ABI. Scene/Story and
> software graphics are Rust-only.

## Try the demos

```bash
# ── Start here ───────────────────────────────────────────────
cargo run --release --example libgibson_intro -- --auto --color=truecolor  # the short film
cargo run --example polished_agent -- --auto        # restrained flagship product demo
cargo run --example glyph_capability_lab            # the glyph-realization axis, visually

# ── Go deeper ────────────────────────────────────────────────
cargo run --release --example acid_vs_crash         # maximalist RGB cinematic (release build!)
cargo run --release --example fx_lab -- --scene=filled-3d --truecolor   # developer FX gallery
cargo run --example hack_the_gibson                 # a *Hackers* (1995) homage on the same primitives

# ── Diagnostics ──────────────────────────────────────────────
cargo run --release --example runtime_observatory -- --help   # runtime-contract instrument
cargo run --release --example event_pressure_lab -- --auto    # PTY input/resize/pressure trace
cargo run --example resize_test_app                 # interactive anchor / re-anchor exercise
```

Most demos accept `--auto`/`--deterministic`/`--freeze-at=<frame>`, capability overrides
(`--truecolor`/`--ansi256`/`--ansi16`/`--mono`, `--no-color`), and theme proofs
(`--light`/`--dark`). `acid_vs_crash` is a shot-directed offline short film viewed from
one machine — simulated semantic commands only, no networking, exploitation, or host
shell. The RGB spectacle wants a `--release` build. Everything is Unicode plus ordinary
foreground/background colour: **no Kitty, Sixel, images, or video.**

Deep dives: [introductory cinema](docs/INTRODUCTORY_CINEMA.md) ·
[acid_vs_crash validation](docs/acid-vs-crash-cinematic-validation.md) ·
[Runtime Observatory](docs/RUNTIME_OBSERVATORY.md) ·
[Event Pressure Lab](docs/EVENT_PRESSURE_LAB.md).

## Status & support

Core engine behaviour is **IMPLEMENTED + TESTED on Linux x86_64 only**. The suite is
**661 tests** (247 library unit + 413 integration + 1 doctest); `cargo clippy --all-targets
--all-features -D warnings`, `cargo fmt --check`, and `cargo build --release` are clean.
C/C++ examples run under ASan + UBSan (LSan disabled); the Python `ctypes` example runs;
the Go bindings pass local + public Linux build/vet/example smoke (no Go unit tests
exist). Public CI exercises Rust, MSRV 1.85, C/C++/Python, native sanitizers, PTYs, and
Go — see the [exact evidence and limits](docs/STATE_OF_LIBGIBSON.md#executed-public-ci-evidence).

**Not verified / not implemented:** Windows / ConPTY · macOS · tmux / screen / SSH · a
broad terminal matrix (xterm, Alacritty, Kitty, WezTerm, iTerm2, …) · terminal
capability *negotiation* (the `CSI L` fast path is assumed, not probed; the repaint
fallback is always correct) · absolute DSR cursor anchoring (resize re-anchor is a
best-effort erase-and-rebuild). A hard `SIGKILL` cannot be intercepted by any userland
process.

### Known limitation

Under a resize / input-readiness collision in crossterm 0.29, a key can be briefly
queued until a later key releases it ([issue #15](https://github.com/femboy2112/libgibson/issues/15)).
The upstream fix is filed as [crossterm#1128](https://github.com/crossterm-rs/crossterm/pull/1128)
and is **not** vendored. This is documented, not a claim of lossless delivery under that
collision; the event-pressure acceptance test that targets it is intentionally ignored
and still fails when run.

## Documentation

**Start here** · this README → [`docs/FEATURES.md`](docs/FEATURES.md) (what it does) →
[`docs/STATE_OF_LIBGIBSON.md`](docs/STATE_OF_LIBGIBSON.md) (architecture, core vs
experimental, blockers, priorities).

| User-facing | Engineering / release | Experimental & research |
|---|---|---|
| [FEATURES](docs/FEATURES.md) | [RELEASE_CONTRACT](docs/RELEASE_CONTRACT.md) | [DESIGN](DESIGN.md) |
| [GLYPHS](docs/GLYPHS.md) | [RELEASING](docs/RELEASING.md) | [INTRODUCTORY_CINEMA](docs/INTRODUCTORY_CINEMA.md) |
| [CHANGELOG](CHANGELOG.md) | [ROADMAP](ROADMAP.md) | [RUNTIME_OBSERVATORY](docs/RUNTIME_OBSERVATORY.md) |
| [SECURITY](SECURITY.md) | [VALIDATION_INDEX](docs/VALIDATION_INDEX.md) | [EVENT_PRESSURE_LAB](docs/EVENT_PRESSURE_LAB.md) |
| [STATE_OF_LIBGIBSON](docs/STATE_OF_LIBGIBSON.md) | [third-party licenses](LICENSES-THIRD-PARTY.md) | [expressivity campaign](docs/research/AGENT_NATIVE_UI_CAMPAIGN_RESULTS_2026-09-24.md) |

The [validation index](docs/VALIDATION_INDEX.md) preserves the historical evidence
trail; the [expressivity campaign](docs/research/AGENT_NATIVE_UI_CAMPAIGN_RESULTS_2026-09-24.md)
tests six consumers against the unchanged public API (it does not claim universal UI
expressivity or stable APIs).

### Repository layout

```
src/          engine: cell/surface/layout/node/diff/ansi/transaction/renderer/
              session/input/scheduler/context/ffi + canvas/glyph/capability/
              scene/story/replication + raster & FX (geom/particles/field/…)
include/      gibson.h (canonical C ABI) · gibson.hpp · termframe.h (compat aliases)
bindings/     c/ · cpp/ · python/ · go/    (native-SDK consumers)
examples/     13 runnable examples (see "Try the demos")
tests/        44 integration suites (PTY, vt100, goldens, safety, FFI, …)
scripts/      release/ tooling (staging, ABI check, clean-room, preflight)
docs/         design, release, validation, and research docs
```

## Build & test

```bash
cargo build --release                                   # library + .so/.a
cargo test                                              # 661 pass; one known-red acceptance is ignored
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

The C and C++ recipes are shown above; the [`bindings/`](bindings) directory holds the
Python and Go wrapper READMEs, and [`docs/RELEASING.md`](docs/RELEASING.md) documents
building against a staged SDK from outside the checkout.

## Contributing

Issues and PRs are welcome. Before a substantive change, skim
[`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md) (API-stability tiers, the C ABI
policy, and the MSRV floor) so a patch doesn't accidentally break the contract. Please
run the [build & test](#build--test) checks locally. Security reports go through
[`SECURITY.md`](SECURITY.md).

## Support

If LibGibson is useful to you, you can [**buy me a coffee ☕**](https://ko-fi.com/leah2112).

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option; both texts are included. See the
[direct-dependency license summary](LICENSES-THIRD-PARTY.md) and the
[security reporting policy](SECURITY.md).
