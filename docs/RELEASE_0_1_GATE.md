# LibGibson 0.1.0 — release gate

**Classification: Engineering Alpha.** `0.1.0` is the first *coherent package release*;
"Engineering Alpha" describes maturity, not the version string. The readiness audit
below is cleared and `v0.1.0` is **released on GitHub** (2026-09-25): tag +
prerelease-flagged GitHub Release + Linux x86_64 artifacts, tag → `a3f1e29`.
**Ecosystem-registry publication remains a separate, undone decision** — no
crates.io upload, no PyPI/TestPyPI upload, no `bindings/go/v0.1.0` module tag, no
OS-package formulas. `cargo publish --dry-run` and `twine check` were validation
only.

Status vocabulary: **PASS** · **BLOCKED** · **WAIVED (known limitation)** ·
**N/A** · **PENDING (executes at tag time)**.

Release-prep branch: `claude/release-v0.1.0` (off main `4ba5ea4`, the PR #23
merge). The final release SHA is frozen when the release-prep PR merges (Gate 1)
and recorded with the tag (Gate 15). ABI: v1 (`GIBSON_ABI_VERSION = 1`) —
unchanged. Supported artifact: Linux x86_64.

| # | Gate | Status | Blocking? |
|---|------|--------|-----------|
| 1 | Tree freeze / candidate SHA | PASS (frozen at `a3f1e29`) | — |
| 2 | Core correctness | PASS | — |
| 3 | Release preflight | PASS (local); CI re-confirms | — |
| 4 | Cargo publish dry-run | PASS | — |
| 5 | Python distribution | PASS (twine substituted) | — |
| 6 | Go module release sim | PASS | — |
| 7 | Native ABI / SONAME | PASS — implemented + tested | — |
| 8 | Public Rust API freeze | PASS | — |
| 9 | Documentation truth | PASS | — |
| 10 | Real-terminal acceptance | PASS (maintainer manual smoke) | — |
| 11 | Crossterm #15 | WAIVED (known limitation, Path B) | no |
| 12 | Security / repo settings | PASS + 1 recommendation | no |
| 13 | Release artifact review | PASS | — |
| 14 | Changelog cut | PASS (0.1.0 cut in release-prep) | — |
| 15 | Exact final SHA | PASS (`v0.1.0` → `a3f1e29`) | — |

---

## Gate 1 — Tree freeze — PASS
The release-prep PR (#24) merged into `main`, freezing the release at
**`a3f1e29`**. No release-shaping branch was pending beyond it; only changelog,
documentation-truth, and packaging corrections landed — no new subsystems. This is
the SHA that carries the `v0.1.0` tag (Gate 15).

## Gate 2 — Core correctness — PASS
- **657 tests** green: **247 library** unit + **410 integration** (`cargo test`,
  and again under `cargo +1.98.1 test` in the preflight).
- Strict Clippy (`--all-targets --all-features -D warnings`) and strict rustdoc
  (`RUSTDOCFLAGS=-D warnings`): clean on nightly and 1.98.1.
- Exactly **one** ignored test — `tests/event_pressure_pty.rs:169`, the *expected*
  crossterm-#15 collision acceptance (Gate 11); no unexpected ignores.
- **Flake fixed:** a pre-existing PTY test
  (`pty_demos.rs::acid_final_cut_link_resolves_and_replays_before_restoring_terminal`)
  asserted the "replay" aftercredits hint on the same snapshot that only waited for
  "CRASH CONTAINS"; that render/input gap widened under full-suite CPU contention,
  making `cargo test` flaky. Fixed by waiting for "replay" as its own step.
  Validated **0/40** under CPU saturation and **0/6** full-suite loops.
- Coverage present in-suite: PTY restoration (`intro_pty`, `pty_integration`,
  `terminal_ownership`), resize torture (`resize_torture`, `pty_resize_torture`),
  whole-renderer VT100 (`whole_renderer_vt100`, `screen_state_vt100`), runtime
  ownership lease (`terminal_ownership`), glyph fallbacks (`glyph_realization`),
  long-session retention (`event_pressure_*`, `commit_invariance`). Sanitizers
  (ASan C/C++) run in CI.

## Gate 3 — Release preflight — PASS (local)
`scripts/release/preflight.sh` on the branch: **PASS** on every leg — whitespace,
version+ABI consistency, fmt, clippy, tests, rustdoc, MSRV resolved-floor (`+1.85.1
--locked --lib`) and declared-range consumer (fresh unlocked resolution), `cargo
package`, ABI v1 symbol baseline (`check-abi.sh`, 52 symbols, unchanged by the
SONAME/`transaction` changes), clean-room C/C++/Python/Go/Rust consumers against
the staged SDK, third-party-notice freshness, license-file presence. The CI
`Release Preflight` workflow re-confirms on the exact tip.

## Gate 4 — Cargo publish dry-run — PASS
`cargo +1.98.1 publish --dry-run`: packaged **53 files** and verified by compiling
the packaged crate in isolation; upload aborted (dry run). Metadata as Cargo sees
it: name `libgibson`, lib target `gibson`, `rust-version = 1.85`, dual license
`MIT OR Apache-2.0`, repository set, README present, dependency constraints intact.
The added `build.rs` packages cleanly and is a Linux-cdylib-only no-op for rlib
consumers. (Example/test-ignore warnings are expected — the demo corpus is
`exclude`d.)

## Gate 5 — Python distribution — PASS
`sdist` (`libgibson-0.1.0.tar.gz`) and `wheel`
(`libgibson-0.1.0-py3-none-any.whl`) build via the declared setuptools backend.
Wheel `METADATA`: `Name: libgibson`, `Version: 0.1.0`, `Requires-Python: >=3.8`,
`License-File: LICENSE / LICENSE-MIT / LICENSE-APACHE` (all three also present in
`.dist-info/` and the sdist root), and the summary states the separately-installed
native-library requirement. A **fresh-venv install + ABI smoke** against the staged
`.so` passes (clean-room). `twine check` was unavailable in this environment; the
artifact metadata was inspected directly instead (Metadata-Version 2.1) and should
also be run in CI/at tag time.

## Gate 6 — Go module release simulation — PASS
Module path `github.com/femboy2112/libgibson/bindings/go` (correct for a
`bindings/go/v0.1.0` tag). License text (`LICENSE`, `LICENSE-MIT`, `LICENSE-APACHE`)
is in the **module root** so the tagged zip ships it. cgo + pkg-config requirement
is documented in `bindings/go/README.md` (`#cgo pkg-config: libgibson`,
`PKG_CONFIG_PATH`). The clean-room builds a Go consumer from a **module copy staged
outside the checkout** with `GOPROXY=off` against the staged SDK — no source-tree
path leaks into `go.mod` or the build. Tag not pushed.

## Gate 7 — Native ABI / SONAME — PASS (implemented + tested)
Decision: **implement before 0.1.0** (the mechanism is clean, not a hack, and the
first public `.so` should be ABI-versioned from the start). `build.rs` stamps ELF
`DT_SONAME = libgibson.so.1` on the Linux cdylib via
`cargo:rustc-cdylib-link-arg=-Wl,-soname,...` (cdylib-scoped; no effect on rlib,
staticlib, examples/tests; guarded to Linux). Staging installs `libgibson.so.1` +
a `libgibson.so → libgibson.so.1` dev symlink. Verified: `readelf -d` shows the
SONAME; a C consumer linked via `-lgibson` records `NEEDED = libgibson.so.1` and
runs (`abi=1`). The major tracks `GIBSON_ABI_VERSION`; `libgibson.a` and pkg-config
are unaffected. A real C ABI break bumps both the SONAME major and the ABI version.
In-tree consumers that link the raw `target/` build output (CI's C/C++/ASan jobs and
`scripts/dev/bindings_smoke.sh`) now create the matching `libgibson.so.1` symlink
next to it, mirroring what the staged SDK ships.

## Gate 8 — Public Rust API freeze — PASS
- **`transaction` module → `pub(crate)`** — `TerminalTransaction` (and its leaked
  `pub buffer: Vec<u8>`) had zero external users and was never re-exported; the
  contract already flagged it for privatization. Done now (pre-tag) to avoid a
  later `0.y` break.
- `ffi` stays `pub` (required by cdylib/staticlib; stability is the C ABI, not a
  Rust API — documented).
- Extension-point enums are `#[non_exhaustive]` (`NodeKind`, `Event`, `KeyCode`,
  `SubcellGlyphMode`); closed sets left exhaustive (`Capability`, `ColorDepth`,
  `GlyphChoice`). (Landed in PR #22.)
- **Deliberately kept** (documented, non-blocking): the `renderer` internal
  bookkeeping fields (counters/cursor/anchor) used only by the crate's own
  white-box tests — the contract scopes CORE stability to named items, not fields;
  encapsulation via getters is deferred to avoid eve-of-release churn. Two
  test-only free functions (`strip_ansi_escapes`, `render_node_to_lines`) likewise
  kept. No library redesign.

## Gate 9 — Documentation truth — PASS
README answers what/why/platforms/install/native-SDK/C-Python-Go models/
experimental/how-to-run-the-flagship. Reconciled this round: glyph realization
described as a policy/override axis (not font detection); SONAME now shipped (was
"future work"); `transaction` now private; test count **657**; packaged manifest
**53 files**; crossterm-#15 limitation stated. Deep evidence lives in `docs/`, not
the README.

## Gate 10 — Real-terminal acceptance — PASS (maintainer manual smoke)
CI/PTYs are not physical terminals; the required physical smoke can only be done by
a human. The maintainer has now run the release candidate on real hardware:

- **Graphical Linux terminal: PASS** — the default intro (with prologue) renders
  correctly and the terminal restores cleanly on exit.
- **Kernel virtual console / TTY1: PASS** — the console-safe glyph fallback works
  and the terminal restores correctly.

Exact terminal/emulator name, `TERM` value, dimensions, color mode, and glyph mode
were **not recorded** by the maintainer beyond the above; only these outcomes are
attested. This clears the one gate that automated coverage (PTY suites, vt100
whole-renderer, restoration, resize) cannot establish.

## Gate 11 — Crossterm #15 — WAIVED (known limitation, Path B)
Upstream fix `crossterm#1128` is filed but **not merged**; LibGibson stays on stock
`crossterm 0.29.0` and does **not** vendor the fork. Per the release policy, 0.1.0
ships as Engineering Alpha with the exact resize/input collision limitation
documented (`RELEASE_CONTRACT.md` §9, README, and the explicitly-ignored
acceptance test), linking the Event Pressure Lab; no claim of lossless input under
that collision. Issue #15 stays open; a merged upstream fix lets it close.

## Gate 12 — Security / repo settings — PASS + 1 recommendation
`SECURITY.md` is current (pre-1.0 policy; private reporting via GitHub Security
Advisories once public). Release artifacts carry license text (Gates 3/5/6/13). No
credentials/secrets in artifacts (Gate 13). CI actions are **SHA-pinned** with
version comments. **Recommendation (not auto-applied):** enable branch protection
on `main` requiring a PR and passing CI before a public release — this is a
repo-owner policy toggle and was left to the maintainer to avoid disrupting the
current merge workflow or locking anyone out. Suggested setting: *Settings →
Branches → Add rule for `main` → Require a pull request before merging + Require
status checks to pass (CI) + Require branches up to date.*

## Gate 13 — Release artifact review — PASS
Built from the candidate: Rust `.crate` (53 files, via publish dry-run), the Linux
x86_64 native SDK tarball (`libgibson-0.1.0-linux-x86_64.tar.gz`, ~2.6 MiB, +
`.sha256`, built with deterministic `tar --sort=name --mtime` and a SHA256SUMS
manifest), Python wheel + sdist. Go tag payload is the `bindings/go` module tree.
Tarball contents inspected: headers, `libgibson.a` + `libgibson.so → .so.1` +
`libgibson.so.1`, relocatable `libgibson.pc`, and the license/notice payload — no
`target/` junk, no local usernames or checkout paths, no secrets, no test corpora;
pkg-config is relocatable via `${pcfiledir}` and the shipped `.so.1` carries a
SONAME but **no RPATH/RUNPATH** to the build tree.

## Gate 14 — Changelog cut — PASS
`CHANGELOG.md` now carries a `## [0.1.0] - 2026-09-25` entry: a lead summary
(Engineering Alpha; Linux x86_64 boundary; core architecture; scrollback/live-region
model; color + glyph ladders; ABI v1 with SONAME `libgibson.so.1`; MSRV 1.85; the
known crossterm-#15 limitation, Path B) followed by the moved Added/Changed/Removed
detail. `[Unreleased]` is kept and empty. The banner states 0.1.0 is prepared as the
GitHub release and is not published to any registry; the post-tag docs pass will
finalize the "released" wording and compare links (that update is not part of the
`v0.1.0` tag).

## Gate 15 — Exact final SHA — PASS
The annotated tag **`v0.1.0`** points to **`a3f1e29fb4dcb94e5fb93225041dd92252bb5df7`**
(the PR #24 merge), verified on the remote (`refs/tags/v0.1.0^{} = a3f1e29`). That
exact SHA passed, with no post-check drift:
- normal **CI** on `a3f1e29`: run **36165658206** — success (all 6 jobs);
- **Release Preflight** on `a3f1e29`: run **36165709056** — success;
- `cargo publish --dry-run` (53 files) and `twine check` (wheel + sdist) — PASS,
  re-run on exact final `main`;
- Gate-10 manual smoke (graphical terminal + TTY1) — PASS.

Release assets were built from the tagged tree (deterministic; the SDK tarball
reproduced the same SHA256 `65fda7aa…` as the pre-merge build) and the uploaded
archive verified byte-for-byte against its published `.sha256`.

---

## Verdict
**RELEASED.** All 15 gates are cleared (Gate 11 WAIVED, Path B). LibGibson **v0.1.0
— Engineering Alpha** is published as a GitHub Release
(<https://github.com/femboy2112/libgibson/releases/tag/v0.1.0>), marked prerelease,
with the Linux x86_64 native SDK (+ `.sha256`) and the Python wrapper wheel/sdist
attached. The tag `v0.1.0` points to `a3f1e29`. #15 remains open (documented
limitation). Scope was a **GitHub release only** — crates.io, PyPI, and the Go
module tag were **not** published and remain separate, later decisions.
