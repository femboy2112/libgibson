# Releasing LibGibson

LibGibson is **engineering alpha**. This is the procedure to produce and inspect a
release candidate and — **only after explicit human approval** — to publish it.
Nothing here happens automatically, and Claude does not perform the publication
steps on its own. What a release promises is defined in
[`RELEASE_CONTRACT.md`](RELEASE_CONTRACT.md).

## 0. Preconditions

- `main` is up to date and green.
- The crossterm / dependency status is understood (see the issue tracker). LibGibson reads a
  Unix controlling terminal with its own source (`src/tty_source.rs`, #269), which avoids
  crossterm 0.29's hung-terminal spin and its readiness-batch stall (#15, upstream
  crossterm#1128); crossterm still writes output and handles every other input path.
- You are **not** publishing until a human explicitly approves the final step for a
  specific version.
- You have enough free disk. A full patch cycle builds several trees; run
  `scripts/dev/reclaim.sh --guard 15` first to fail fast if space is short, and see
  **Disk hygiene** below.

## 1. Update the changelog

- Move the relevant `[Unreleased]` entries in [`../CHANGELOG.md`](../CHANGELOG.md)
  under a new `## [x.y.z]` heading with the date.
- Do not write "released" until it actually is.

## 2. Verify the version

- Confirm `Cargo.toml` `[package] version` is the intended release version.
- The crate, native SDK, Python wrapper, and Go module release in lockstep at that
  one version. `scripts/release/check-versions.sh` verifies the derived metadata
  agrees; the preflight runs it too.

## 3. Run the release preflight

The Linux packaging host needs `objcopy` (or an equivalent executable selected
with `OBJCOPY`). SDK staging removes embedded `.llvmbc` and `.llvmcmd` metadata
from the installed static archive before checksumming it; it preserves the
original Cargo artifact. The native-archive controls compare members, symbols,
machine code and static clients, and require a Go toolchain with the 1.27.4 /
1.26.11 cgo security checks. This packaging prerequisite does not change the
library's Rust MSRV or require `objcopy` on an SDK consumer's machine.

```
./scripts/release/preflight.sh
```

This is the merciless gate: format, clippy, tests, rustdoc, both MSRV floors (the
committed-lock `cargo +1.85 check --locked --lib` and the declared-range
fresh-resolution consumer, `scripts/release/msrv-consumer.sh`), `cargo package`,
package-content audit, the ABI-v1 symbol baseline, a staged SDK, clean external C /
C++ / Python / Go consumers (the Go consumer builds from a module copy staged
outside the checkout), license presence **and** distributable license-payload
assertions (Python wheel/sdist and the Go module root carry the dual license),
enforced third-party-notice freshness, and a repository-path-leak check. It must
print `RELEASE PREFLIGHT: PASS`.

Preflight captures the caller's HEAD and runs its checks in a separate local clone at
that commit, with a fresh Cargo target. The clone's checkout is checked for changes
and assume-unchanged/skip-worktree flags at both boundaries. Its scratch directory is
reported and retained for audited cleanup. The caller's uncommitted changes and ignored
files are not part of this witness. The clone is not mounted read-only, so these
checks do not prove that no tool transiently changed a file during execution or that
the toolchain was not substituted. External toolchains, environment and caches still
need to be controlled. The ABI symbol gate builds that captured commit from a
Git archive in an isolated target
and passes that exact path to `check-abi.sh --so`; it reports the commit and library
digest. It checks exported names only, not C signatures or structure layouts. A release
must separately qualify layouts and signatures, and use a clean candidate checkout for
the other preflight steps. `check-candidate-abi.sh` retains its named temporary directory
for audited local cleanup rather than recursively deleting it automatically.

SDK staging uses Python 3 to select the native-library paths reported by that Cargo
build, including custom target directories. It makes private copies of the selected
files before the subsequent native-link-flags query, checks each copy against its
original immediately, and stages the copies. The query may legitimately rebuild the
originals. These checks are not a defense against a hostile toolchain or concurrent
process. `--no-build`
uses existing artifacts and does not establish their source provenance.

**Hangup-test execution is a separate mandatory release witness.** The four
terminal-hangup liveness tests are marked `#[ignore]` for the normal parallel
`cargo test` invocation and instead execute serially in the dedicated PTY CI job.
The release preflight now explicitly invokes
`cargo test --test pty_hangup --test pty_hangup_restore -- --test-threads=1 --include-ignored`
on the supported Linux x86_64 release runner. A compile-only or ignored result
is not a test pass. A failure fails preflight; do not merely raise timeouts or
relabel an intermittent hang as proven safe. Record the exact RC SHA and the
individual PTY execution result alongside the full CI run.

## 4. Inspect the artifacts

- `target/package/libgibson-<version>.crate` (`cargo package`)
- the staged SDK (`scripts/release/stage-sdk.sh --prefix <dir>`)
- the release bundle (`scripts/release/bundle.sh`):
  `libgibson-<version>-linux-x86_64.tar.gz` + `SHA256SUMS`

Confirm: no repository paths baked in, no accidental corpus, all license/notice
files present.

## 5. Open a release PR (if release files changed)

Land any changelog/version changes via PR; keep `main` green.

## 6. Publish — ONLY after explicit human approval

These are the ignition steps. Perform them **only** when a human has explicitly said
to publish this specific version.

1. **Tag:** `git tag -a v<version>` with its first message line
   `LibGibson v<version> — <name>`, then `git push origin v<version>`. Look the name up in
   [decision 0004](decisions/0004-release-naming.md). The tag ref stays plain `v<version>`.
2. **GitHub Release:** `gh release create v<version>` titled `LibGibson v<version> — <name>`,
   with notes from the changelog;
   attach the `linux-x86_64` bundle and its `SHA256SUMS`.
3. **crates.io:** `cargo publish --dry-run` first, then `cargo publish`.
4. **Python wrapper:** build the sdist/wheel and, once the wrapper publication
   pipeline is supported, upload it.
5. **Go module:** create the module tag using Go's subdirectory-module convention
   (the module lives under `bindings/go`; the exact tag form is documented in
   `bindings/go/README.md`). Verify the tag form before pushing it.

## 7. Maintenance-line release — *Approved 2026-11-08 (owner decision #287)*

The procedure for the gated exception in
[Release Contract §1](RELEASE_CONTRACT.md#1-versioning-model). Every step acts on one **exact
candidate SHA**, never on a branch name; any new commit on the branch is a new candidate and
restarts at step 3, with no green run inherited from an earlier SHA.

1. **Branch** `release/v<x.y.z>-rc` (or `release/<x.y>`) from the baseline: the commit at which
   the version was prepared in source (0.6.0: `9a899e1`), or the line's previous release tag.
2. **Land only fixes and release engineering.** A backport cites its `main` counterpart (issue, PR
   and commit); a release-engineering commit (changelog, test or CI adaptation, a patch version
   bump) says so. Nothing else.
3. **Freeze a candidate**: the branch head's full 40-hex SHA.
4. **Run the candidate checker** with the qualification record's commit list, one line per commit
   in `<baseline>..<candidate>`, `<sha> <backport|release-engineering> <counterpart or reason>`:

   ```
   scripts/release/check-maintenance-candidate.sh --baseline <baseline> --candidate <sha> \
       --allow-commits-file <list>
   ```

   It must print `RESULT: PASS`: ancestry; `Cargo.toml`, `Cargo.lock` and `pyproject.toml` at the
   release version; `abi/`, the `gibson_*` exports and the Rust C-ABI surface of `src/`
   (signatures, `#[repr]` layouts, `GIBSON_*` constants) equal to the baseline's, and `include/`
   equal or different only in comments; `rust-version` unchanged; `src/audio/` and
   `tests/audio_pocket_voice.rs` (the frozen PCM golden) and `tests/ffi_abi_layout_freeze.rs` (the
   layout oracle) unchanged; no tool failure behind any check; the candidate not the baseline,
   and on a branch named exactly `release/v<x.y.z>-rc` or `release/<x.y>`; every commit listed.
   Calibrate the checker on the same candidate first:
   `scripts/release/tests/test-maintenance-checker.sh --baseline <baseline> --candidate <sha>
   --allow-commits-file <list> --so <built library> --foreign-so <a library built from another
   tree> --dev-candidate <a main commit>` must print `CALIBRATION: PASS`: the valid candidate and
   its comment-only and doc-only variants pass, and every negative case (development commit, ABI
   baseline, ancestry, the baseline itself, allow-list, library, version, tags, header declarations,
   splice and trigraph tricks, string literals, Rust layouts, signatures, constants, attributes,
   aliases, exports, the layout oracle, MSRV, PCM, branch, an unreadable source, and four injected
   tool failures) fails for that reason alone. The final run of this step uses `--so` with a library
   built from the candidate (step 9 repeats it on the tagged tree's build).
5. **Qualify the same SHA**: full CI at exactly that SHA (every job), the publication-free Release
   Preflight at that SHA printing `RELEASE PREFLIGHT: PASS` (both MSRV floors, cleanroom
   consumers, package audit), the hangup PTY tests passing on their first attempt (§3), and an
   independent review of every backported fix.
6. **Qualification record.** Post on the release's decision issue, and copy into its decision
   record under `docs/decisions/`: version, baseline SHA, branch, candidate SHA, the commit list,
   the checker output, the CI and preflight run IDs, the review outcome and the known limitations
   the release carries.
7. **Owner approval.** The owner approves that record in writing, naming the version, the exact
   candidate SHA, the baseline and the branch, and stating that the maintenance-line exception
   applies; or the owner's advance conditional approval (Release Contract §1 condition 5) applies
   and the record shows every gate of steps 4–6 passed on that SHA. An approval covers that one
   SHA. Without it, stop.
8. **Tag** the approved SHA, not the branch: an annotated `v<x.y.z>` whose message names the
   baseline, the branch and the qualification record, and `bindings/go/v<x.y.z>` on the same
   commit. Before pushing, `git rev-parse 'v<x.y.z>^{commit}' 'bindings/go/v<x.y.z>^{commit}'`
   prints the approved SHA twice. Push those two tags only; confirm the remote peel with
   `git ls-remote origin 'refs/tags/v<x.y.z>^{}' 'refs/tags/bindings/go/v<x.y.z>^{}'`, then fetch the
   tags and re-run step 4; its tag checks must now pass.
9. **Build the artifacts from the tagged tree**: a fresh export of the tag (`git archive
   v<x.y.z>` into an empty directory, or a new worktree at the tag), never a working branch.
   Build the native SDK bundle (`scripts/release/bundle.sh`:
   `libgibson-<x.y.z>-linux-x86_64.tar.gz` and its `.sha256`) and the Python wheel and sdist.
   Re-run step 4 with `--so <export>/target/release/libgibson.so`: the built library must export
   exactly the baseline's symbols.
10. **GitHub prerelease** of `v<x.y.z>` with those four assets
    (`libgibson-<x.y.z>-linux-x86_64.tar.gz`, `libgibson-<x.y.z>-linux-x86_64.tar.gz.sha256`,
    `libgibson-<x.y.z>-py3-none-any.whl`, `libgibson-<x.y.z>-python-sdist.tar.gz`). The notes say it
    is a maintenance-line release, name the baseline and branch, and list the backports and known
    limitations. No registry upload: state 3 is a separate approval.
11. **Download-verify**: download the native asset and its `.sha256` from the release into an
    empty directory, run `sha256sum -c`, and confirm both digests equal the locally built ones.
12. **Post-release records**, by a docs PR on `main` that leaves `main`'s version alone: the
    CHANGELOG `[x.y.z]` heading (date, tag → SHA, branch), the Release Contract §1 milestone entry
    (state 2), the README / ROADMAP / `docs/STATE_OF_LIBGIBSON.md` current-release references, and
    the decision record's outcome.
13. **Optional back-merge into `main`**: its own PR and its own acceptance, never implied by the
    release. It is a merge commit (no squash, no rebase), so afterwards
    `git merge-base --is-ancestor v<x.y.z> origin/main` holds. Every version conflict resolves to
    `main` (`Cargo.toml`, `Cargo.lock`, `pyproject.toml` keep `main`'s version); `main`'s `abi/`,
    `include/` and exported symbols are unchanged by the merge; `main` already carries each fix,
    so any source difference from the merge's first parent is justified line by line; and `main`'s
    full CI is green at the merge commit.

## Disk hygiene (local dev)

`cleanroom.sh`, `bundle.sh`, and `msrv-consumer.sh` trap-clean their own temp trees.
The ABI candidate and fault-control scripts instead report and retain their temporary
directories for audited cleanup. On a dev box these and two other things can accumulate
across a patch train:

- **The consumer-acceptance lab clone.** When validating a candidate against the
  external Europa consumers, clone the lab into the canonical throwaway location
  `$LIBGIBSON_LAB_SCRATCH` (default `$TMPDIR/libgibson-lab-scratch`), repoint its
  `libgibson` dependency at the candidate rev, run the consumer suites, then let
  `reclaim.sh` wipe it — never keep a standing clone with a resident `target/`
  (three coexisting ~3–4 GiB clones once filled the disk mid-release).
- **The repo `target/`.** Under `[profile.dev]` the debug/test tree is kept lean
  (`debug = "line-tables-only"`; see `Cargo.toml`), but it still grows over many
  patches. `cargo clean` between patches when space is tight.

Run `scripts/dev/reclaim.sh` to sweep the lab scratch and any orphaned
`/tmp/libgibson-*` build dirs; add `RECLAIM_CARGO_CLEAN=1` to also `cargo clean` the
repo `target/`. Use `scripts/dev/reclaim.sh --guard <GiB>` as a pre-build guard that
fails loudly *before* a build starts, rather than as `No space left on device`
several gates in. CI runners are ephemeral (`CARGO_INCREMENTAL=0` is set there to
skip the never-reused incremental cache), so this is a local-dev concern only.

## Do NOT

- publish to any registry, create a git tag, or create a GitHub Release without
  explicit human approval for that specific version.
