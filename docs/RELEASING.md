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

## Disk hygiene (local dev)

The release scripts each trap-clean their own temp trees, so `preflight.sh`,
`cleanroom.sh`, `bundle.sh`, and `msrv-consumer.sh` leave nothing behind. Two things
are **not** auto-cleaned on a dev box and accumulate across a patch train:

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
