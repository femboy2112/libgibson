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

### Changed
- The published Rust crate now **excludes** the engineering-demo corpus (examples,
  integration tests and goldens, internal `docs/`, dev scripts, and the language
  bindings). It ships the library, the C header (for native-library builders), and
  the license/readme/design docs. Package manifest: 221 → 47 files.
- The C++ wrapper header moved to `include/gibson.hpp`; `bindings/cpp/gibson.hpp` is
  now a forwarding shim so existing in-repository consumers keep working.

### Removed
- Unused `thiserror` dependency (it was declared but never used in `src/`).

## Planned — 0.1.0 (not yet released)

The first engineering-alpha release. Its intended contents and the exact cut
procedure are documented in [`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md)
and [`docs/RELEASING.md`](docs/RELEASING.md). **This is a plan, not a record:**
0.1.0 has not been tagged, released, or published.
