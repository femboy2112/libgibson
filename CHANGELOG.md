# Changelog

All notable changes to LibGibson are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
the 0.x Semantic Versioning rules defined in
[`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md) — pre-1.0, a breaking change
bumps the minor slot (`0.y`) and a compatible change bumps the patch slot (`0.1.z`).

LibGibson is **engineering alpha**. Nothing below has been tagged, built as a GitHub
release, or published to any package registry.

## [Unreleased]

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

## Planned — 0.1.0 (not yet released)

The first engineering-alpha release. Its intended contents and the exact cut
procedure are documented in [`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md)
and [`docs/RELEASING.md`](docs/RELEASING.md). **This is a plan, not a record:**
0.1.0 has not been tagged, released, or published.
