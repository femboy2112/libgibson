# LibGibson Release Contract

**Status: engineering alpha · pre-1.0 (0.x) · Linux x86_64.**

This document is the authoritative statement of what LibGibson promises to the
people who consume it. It defines the versioning model, the Rust API stability
policy, the C ABI policy, the wrapper compatibility policy, the supported Rust
compiler floor (MSRV), the supported platforms, and what a release artifact
contains. The step-by-step release procedure lives in
[`RELEASING.md`](RELEASING.md); the human-readable change history lives in
[`../CHANGELOG.md`](../CHANGELOG.md).

"Engineering alpha" means: **a reproducible, installable artifact with documented
contracts.** It does *not* mean production-ready or forever-stable. 0.1.0 is a
coherent thing to release under that classification; it is not a 1.0.

---

## 1. Versioning model

There is **one** LibGibson version, canonical in `Cargo.toml` (`[package] version`).
For the 0.1.x line the Rust crate, the native SDK, the Python wrapper, and the Go
module all release in lockstep at that single version — no independent wrapper
versioning until a real need forces it.

The **C ABI version** (`GIBSON_ABI_VERSION`) is a *separate* integer and is **not**
tied to the package version (see §5). A package release can advance without
touching the ABI version; a breaking ABI change bumps `GIBSON_ABI_VERSION` even
while the package stays 0.x.

### 0.x SemVer semantics

LibGibson is pre-1.0, so Cargo's 0.x compatibility rules apply. A consumer who
writes `libgibson = "0.1"` accepts `>=0.1.0, <0.2.0`. Therefore:

| Change | Version bump | Notes |
|---|---|---|
| Bug fix, internal change, additive API (new fn/type/module), doc | `0.1.z → 0.1.(z+1)` (patch) | Source-compatible for CORE consumers. |
| Breaking change to a CORE Rust API; removing/renaming public items; adding an enum variant (see §3); raising MSRV; changing an experimental API in a way that breaks a patch consumer | `0.1.z → 0.2.0` (the 0.x "breaking" slot) | Recorded in CHANGELOG. |
| C ABI break | independent `GIBSON_ABI_VERSION` bump (§5) **plus** the package bump the source change warrants | The two version numbers move independently. |

There is no 1.0 yet. 1.0 is when the CORE surface (§3) is frozen; that is out of
scope for this round.

---

## 2. Rust API stability (pre-1.0)

**Import note.** The package is `libgibson` but the library target is `gibson`
(`Cargo.toml` `[lib] name = "gibson"`). After `cargo add libgibson`, Rust code
imports as **`use gibson::…`**, not `use libgibson::…`. Examples in docs must use
`gibson::`.

The crate is already self-classified as "Engineering alpha" in its crate-level
docs (`src/lib.rs`). This contract makes the tiers explicit.

### CORE (best-effort stability at 0.x)

These modules are the load-bearing, C-ABI-adjacent surface. Within a `0.1.z` patch
they receive no breaking changes; breaking changes to them bump the `0.y` slot and
are recorded in the CHANGELOG:

- `cell` — `Cell`, `Color`, `Style`, `Span`, `Line`, `RichText`, `Theme`, `TextAlign`
- `surface` — `Surface`, `Rect`, `BorderType`
- `node` — `Node`, `NodeKind`, `LayoutStyle`, and the layout-style enums
- `layout` — `compute_layout`, `wrap_text`, `wrap_rich_text`
- `painter` — `paint`, `PaintContext`
- `renderer` — `Renderer`, `RenderMode`, `InsertStrategy`, `AnchorState`
- `context` — `Context`
- `session` — `TerminalSession`, `TerminalLease` (process-global terminal-ownership lease)
- `input` — `Event`, `KeyCode`, `KeyEvent`, `poll_event`
- `capability` — `TerminalCapabilities`, `Capability`, `ColorDepth`, quantization
- the **C-ABI-backed subset** (see §5)

A lower-risk **utility tier** — `ansi`, `canvas`, `clock`, `diff`, `field`,
`focus`, `geom`, `glitch`, `particles`, `replication`, `scheduler`, `show`,
`transition`, `viewport` — is public and treated as CORE (same 0.x promise) unless
a specific item is marked experimental. Two known specifics: `geom` is core vector
math *except* `CubicPath3`, which is marked experimental; `show` (composition
helpers such as sparklines/gradients/progress) is core-intent but currently
reachable only via its full path `gibson::show::…` (it is not re-exported at the
crate root).

### EXPERIMENTAL (may change or be removed between minor releases)

These are already marked "Experimental" in their own module docs and in the
crate-level docs (`src/lib.rs`). They exist and are useful, but they carry **no**
`0.y`-level stability promise: they may change signature, behavior, or be removed
in any `0.2.0`-style release, and we will avoid — but do not guarantee against —
breaking them in a pure patch:

- `scene` — entity/effect composition graph (`Scene`, `SceneEntity`, `Effect`, …)
- `story` — narrative beat/reaction director (`Story`, `StoryDirector`, `Beat`, `TraceRetention`, …)
- `surface_fx` — ordered endomorphisms over a realized surface
- `raster`, `raster3d`, `raster_fx` — the software RGB/3D rasterizer stack

### `ffi` module

`gibson::ffi` is `pub` because the `cdylib`/`staticlib` build targets require it,
but it is **not a supported Rust API**. Rust consumers should use the safe modules
above. The stability promise attached to `ffi` is the **C ABI promise** (§5), not a
Rust-level one.

`transaction::TerminalTransaction` is internal plumbing (used only by the
renderer). It is currently `pub` but is not intended for external use and may
become private in a future `0.y` release.

### Enums are exhaustive (forward-compatibility note)

No public type is `#[non_exhaustive]` today. Consequently, **adding a variant to a
public enum is a breaking change** under this contract (it bumps `0.y`), because
downstream `match` arms without a wildcard would no longer be exhaustive. The
highest-churn case is `node::NodeKind` (the natural extension point for new
component kinds). We may adopt `#[non_exhaustive]` on the extension-point enums
before 1.0; **downstream code that matches LibGibson enums should include a `_`
wildcard arm** to stay forward-compatible.

### Deprecations

Pre-1.0, deprecated items are marked with `#[deprecated]` where practical and may
be removed in the next `0.y` release. We do not promise a long deprecation window
before 1.0.

---

## 3. C ABI policy

The C ABI is the foundation the C, C++, Python, and Go bindings all stand on.

- `GIBSON_ABI_VERSION` is currently **1** (`include/gibson.h`). It is preserved.
- `gibson_abi_version()` returns it at runtime.
- `gibson_stats_t` carries `struct_size` and `abi_version` as its first two fields
  specifically so the struct can evolve under version control. The current
  exact-version discipline — the caller sets `struct_size = sizeof(gibson_stats_t)`
  and the engine validates `abi_version` against `GIBSON_ABI_VERSION` — **is
  preserved and must not be quietly changed.**
- The ABI v1 surface is the set of **52 exported `gibson_*` symbols**, captured as a
  baseline in [`../abi/gibson-abi-v1.symbols`](../abi/gibson-abi-v1.symbols) and
  guarded in CI. Symbols in that set must not disappear or change signature without
  an ABI-version bump.

### ABI-compatible changes (no `GIBSON_ABI_VERSION` bump)

- adding new functions
- adding new opaque handle types
- internal implementation changes with unchanged observable behavior
- appending new fields to the **end** of `gibson_stats_t` under the `struct_size`
  discipline (a caller built against an older, smaller struct remains correct)
- adding new enum constants **where the output-direction semantics permit** and
  callers are expected to treat unknown constants defensively

### ABI-breaking changes (**must** bump `GIBSON_ABI_VERSION`)

- changing the signature of an existing exported function
- changing calling convention
- changing ownership/lifetime semantics incompatibly
- removing an exported symbol
- an incompatible public struct layout change (reordering or removing existing
  `gibson_stats_t` fields, or changing a field's meaning)

### Relationship to the package version

Crate/package semantic version **≠** C ABI version. The package can move from
`0.1.z` to `0.2.0` with the ABI still at 1. A breaking C ABI change **must** bump
`GIBSON_ABI_VERSION` (to 2, …) **even while LibGibson remains 0.x.**

---

## 4. Wrapper compatibility policy

The C++, Python, and Go wrappers all target native ABI **v1**. Each wrapper knows
the ABI version it was built for (`EXPECTED_ABI = 1`) and checks the loaded native
library's `gibson_abi_version()` **before** relying on it, failing with a clear,
actionable message on mismatch (never "segfault first, explain later"):

- **Python** — an `EXPECTED_ABI` constant and a runtime check on library load that
  raises a precise exception (e.g. "expected LibGibson ABI 1, loaded ABI 2; install
  a compatible libgibson").
- **Go** — an `EXPECTED_ABI` constant and a runtime check that returns an error on
  mismatch.
- **C++** — the header compiles against `gibson.h` and exposes
  `gibson::Context::abi_version()`; a convenience check compares it to
  `GIBSON_ABI_VERSION` at construction.

There is no negotiation protocol; incompatible native ABI is a hard, clear failure.

---

## 5. Minimum Supported Rust Version (MSRV)

**MSRV = Rust 1.85** (declared as `rust-version = "1.85"` in `Cargo.toml`).

This is an empirically established floor, not the compiler CI happens to run:

- The maximum `rust-version` in LibGibson's **resolved dependency graph** is
  `unicode-segmentation 1.13.3`, which requires **rustc 1.85.0**. It is a direct
  dependency, and under the resolver a fresh crates.io consumer of our declared
  ranges resolves it, so the floor a consumer faces is 1.85.0.
- **Positive control:** `cargo +1.85.1 check --locked --lib` and
  `cargo +1.85.1 test --locked --lib` both pass (236 library unit tests).
- **Negative control:** `cargo +1.84.1 check --locked --lib` is *refused* by Cargo
  with: `unicode-segmentation@1.13.3 requires rustc 1.85.0`.

The floor is therefore a **dependency requirement**, not a LibGibson language/API
requirement — LibGibson's own source may compile on older compilers, but the
supported dependency set does not, so we do not promise below 1.85.

Scope: the MSRV covers **building the supported library** (`cargo check`/`test
--lib`). It does not promise that every example, dev tool, or the full test harness
builds under the MSRV toolchain. Raising the MSRV is a `0.y` (minor) change and is
recorded in the CHANGELOG. CI enforces the floor with a scoped
`cargo +1.85 check --locked --lib` job.

---

## 6. Supported platforms

- **Linux x86_64 (glibc)** is the only platform built and tested this round. The
  release artifact is labeled `linux-x86_64` engineering alpha accordingly.
- Negotiated terminal capability axes: color depth (TrueColor / 256 / 16 / mono),
  synchronized updates, insert-line support.
- **Glyph realization is not negotiated and not promised.** A terminal accepting
  UTF-8/Unicode does **not** imply its font can realize every glyph. Concretely: the
  Linux kernel **virtual console** (TTY) cannot realize the Braille block
  (U+2800–U+28FF) — its loaded console font has a bounded glyph repertoire — so
  sub-cell Braille graphics degrade there even though layout, text, and overall
  structure render correctly. Applications that rely on Braille/half-block sub-cell
  rendering must not assume it is available on every terminal. (A glyph-capability
  fallback ladder is future work, not part of this release.)
- **Not covered this round:** Windows, macOS, tmux/screen/SSH certification, Kitty
  graphics protocol. These may follow; they are not promised here.

---

## 7. What a release contains

- **Rust crate** (crates.io-ready `libgibson`): the Rust library, the C header
  (`include/gibson.h`, for native-library builders), and license/readme/design
  docs. The engineering-demo corpus (examples, integration tests + goldens,
  internal `docs/`, dev scripts, and the language bindings) stays in the repository
  but is excluded from the crate (`Cargo.toml` `exclude`).
- **Native SDK** (Linux x86_64): `libgibson.so`, `libgibson.a`, `gibson.h`,
  `gibson.hpp`, a `libgibson.pc` pkg-config file, and the license/notice files.
  (The shared library is an unversioned `libgibson.so` this round; SONAME versioning
  is future work.)
- **Python wrapper** package (sdist + wheel) that depends on a separately installed,
  ABI-compatible native LibGibson.
- **Go module** that consumes the installed native SDK via `pkg-config`.
- **Release bundle**: `libgibson-0.1.0-linux-x86_64.tar.gz` (the staged native SDK)
  plus a checksums file.

Artifact contents are made deterministic where practical (stable file manifest +
checksums). Byte-for-byte reproducibility is **not** promised this round.

---

## 8. Security support

See [`../SECURITY.md`](../SECURITY.md). Pre-1.0: only the latest 0.x line receives
fixes; there is no long-term stable security-support window; the private reporting
path is as stated in `SECURITY.md`.

---

## 9. What we explicitly do NOT promise (pre-1.0)

- No Rust API is frozen. 1.0 will define the frozen surface.
- Experimental modules (§2) may change or be removed.
- No cross-platform binary wheels or non-Linux artifacts yet.
- No byte-for-byte build reproducibility (deterministic manifest + checksums instead).
- No package-manager ecosystem distribution (Homebrew/AUR/deb/rpm) yet.
