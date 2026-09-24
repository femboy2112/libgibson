# Releasing LibGibson

LibGibson is **engineering alpha**. This is the procedure to produce and inspect a
release candidate and — **only after explicit human approval** — to publish it.
Nothing here happens automatically, and Claude does not perform the publication
steps on its own. What a release promises is defined in
[`RELEASE_CONTRACT.md`](RELEASE_CONTRACT.md).

## 0. Preconditions

- `main` is up to date and green.
- The crossterm / dependency status is understood (see the issue tracker; #15 tracks
  the crossterm readiness-batch fix filed upstream as crossterm#1128).
- You are **not** publishing until a human explicitly approves the final step for a
  specific version.

## 1. Update the changelog

- Move the relevant `[Unreleased]` entries in [`../CHANGELOG.md`](../CHANGELOG.md)
  under a new `## [x.y.z]` heading with the date.
- Do not write "released" until it actually is.

## 2. Verify the version

- Confirm `Cargo.toml` `[package] version` is the intended release version.
- The crate, native SDK, Python wrapper, and Go module release in lockstep at that
  version (0.1.x). `scripts/release/check-versions.sh` verifies the derived metadata
  agrees; the preflight runs it too.

## 3. Run the release preflight

```
./scripts/release/preflight.sh
```

This is the merciless gate: format, clippy, tests, rustdoc, MSRV, `cargo package`,
package-content audit, the ABI-v1 symbol baseline, a staged SDK, clean external C /
C++ / Python / Go consumers, license presence, and a repository-path-leak check. It
must print `RELEASE PREFLIGHT: PASS`.

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

1. **Tag:** `git tag v<version> && git push origin v<version>`
2. **GitHub Release:** `gh release create v<version>` with notes from the changelog;
   attach the `linux-x86_64` bundle and its `SHA256SUMS`.
3. **crates.io:** `cargo publish --dry-run` first, then `cargo publish`.
4. **Python wrapper:** build the sdist/wheel and, once the wrapper publication
   pipeline is supported, upload it.
5. **Go module:** create the module tag using Go's subdirectory-module convention
   (the module lives under `bindings/go`; the exact tag form is documented in
   `bindings/go/README.md`). Verify the tag form before pushing it.

## Do NOT

- publish to any registry, create a git tag, or create a GitHub Release without
  explicit human approval for that specific version.
