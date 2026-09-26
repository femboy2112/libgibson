# Changelog

All notable changes to LibGibson are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
the 0.x Semantic Versioning rules defined in
[`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md) — pre-1.0, a breaking change
bumps the minor slot (`0.y`) and a compatible change bumps the patch slot (`0.1.z`).

LibGibson is **engineering alpha**. Version 0.1.0 was released on GitHub on
2026-09-25 (Linux x86_64: source plus a native SDK archive). It is **not** published
to any package registry — no crates.io, PyPI, or Go module proxy upload.

## [Unreleased]

Nothing yet.

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

[Unreleased]: https://github.com/femboy2112/libgibson/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/femboy2112/libgibson/releases/tag/v0.1.0
