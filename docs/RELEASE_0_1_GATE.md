# LibGibson 0.1.0 — release-candidate gate

**Classification: Engineering Alpha.** `0.1.0` is the first *coherent package release*;
"Engineering Alpha" describes maturity, not the version string. This is not the
publication branch — it is the readiness audit. **Publication is NOT authorized by
this document** (no `v0.1.0` tag, GitHub Release, crates.io, PyPI, or Go tag).

Status vocabulary: **PASS** · **BLOCKED** · **WAIVED (known limitation)** ·
**N/A** · **PENDING (executes at tag time)**.

Candidate branch: `claude/0.1.0-release-gate` (off main `2c0eed2`). The candidate
SHA is frozen only when the release-candidate PR merges (Gate 1). ABI: v1
(`GIBSON_ABI_VERSION = 1`) — unchanged. Supported artifact: Linux x86_64.

| # | Gate | Status | Blocking? |
|---|------|--------|-----------|
| 1 | Tree freeze / candidate SHA | PENDING (freeze at RC-PR merge) | no |
| 2 | Core correctness | PASS | — |
| 3 | Release preflight | PASS (local); CI re-confirms | — |
| 4 | Cargo publish dry-run | PASS | — |
| 5 | Python distribution | PASS (twine substituted) | — |
| 6 | Go module release sim | PASS | — |
| 7 | Native ABI / SONAME | PASS — implemented + tested | — |
| 8 | Public Rust API freeze | PASS | — |
| 9 | Documentation truth | PASS | — |
| 10 | Real-terminal acceptance | BLOCKED on maintainer (manual) | **yes, before tag** |
| 11 | Crossterm #15 | WAIVED (known limitation, Path B) | no |
| 12 | Security / repo settings | PASS + 1 recommendation | no |
| 13 | Release artifact review | PASS | — |
| 14 | Changelog cut | PENDING (at tag time) | no |
| 15 | Exact final SHA | PENDING (at tag time) | no |

---

## Gate 1 — Tree freeze
No release-shaping branch is pending beyond this one. `main` is clean and green
(`2c0eed2`). The candidate SHA is fixed when this RC PR merges; after that, only
release-blocker fixes, documentation-truth fixes, and packaging corrections are
allowed — **no new subsystems.** **PENDING** (by design — freeze is the merge).

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

## Gate 10 — Real-terminal acceptance — BLOCKED on maintainer (manual)
CI/PTYs are not physical terminals. Automated coverage exists (PTY suites, vt100
whole-renderer, restoration, resize), **but the required physical smoke on a
graphical Linux terminal and the kernel virtual console (TTY1) can only be done by
a human and has not been performed on this candidate.** This is the one gate that
must be cleared by the maintainer before tagging. Commands are in the final report.
Record for each run: terminal/emulator, `TERM`, color mode, glyph mode, outcome.

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

## Gate 14 — Changelog cut — PENDING (at tag time)
`CHANGELOG.md` keeps its `[Unreleased]` heading (honest: nothing is published). At
tag time, move entries to `[0.1.0] - <date>` with the real date, leading with:
Engineering Alpha; Linux x86_64 boundary; core architecture; scrollback/live-region
model; glyph fallback; ABI v1 (SONAME `libgibson.so.1`); MSRV 1.85; the known
crossterm limitation (Path B).

## Gate 15 — Exact final SHA — PENDING (at tag time)
The SHA that is tagged must be the SHA that passed CI + Release Preflight + publish
dry-run + Python artifact check + clean-room + the Gate-10 manual smoke — with no
"fixed one typo after CI" drift. Recorded at tag time.

---

## Verdict
**Ready to CREATE a 0.1.0 release candidate: YES.** Every automated and packaging
gate is green; the SONAME and API-surface decisions are made and implemented; the
docs are reconciled. **What still blocks the *tag* (not the RC):** (1) **Gate 10**
— maintainer's manual smoke on a real graphical terminal and TTY1; (2) the tag-time
steps — changelog cut (Gate 14), the freeze/exact-SHA discipline (Gates 1/15), and
`cargo publish`/`twine check` run in CI. #15 does **not** block (Path B). SONAME is
done. The only accidental-API fix worth making pre-tag (`transaction`) is done.
