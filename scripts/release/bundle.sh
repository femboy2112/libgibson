#!/usr/bin/env bash
#
# bundle.sh — assemble the Linux x86_64 engineering-alpha release bundle.
#
# Stages the native SDK into a versioned directory and packs it into
# libgibson-<version>-linux-x86_64.tar.gz plus a .sha256 checksum. The archive is
# made as deterministic as the tooling allows (sorted entries, fixed owner and
# mtime, gzip without a timestamp); byte-for-byte reproducibility of the compiled
# library itself is not promised.
#
# Usage: scripts/release/bundle.sh [OUTPUT_DIR]   (default: target/release-bundle)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
OUTDIR="${1:-$REPO_ROOT/target/release-bundle}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
NAME="libgibson-$VERSION-linux-x86_64"

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-bundle.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT

scripts/release/stage-sdk.sh --prefix "$STAGE/$NAME" --force >&2

mkdir -p "$OUTDIR"
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime='UTC 2020-01-01' \
    -C "$STAGE" -cf "$OUTDIR/$NAME.tar" "$NAME"
gzip -n -f "$OUTDIR/$NAME.tar"
( cd "$OUTDIR" && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256" )

echo "bundle: wrote $OUTDIR/$NAME.tar.gz"
( cd "$OUTDIR" && ls -l "$NAME.tar.gz" "$NAME.tar.gz.sha256" && cat "$NAME.tar.gz.sha256" )
