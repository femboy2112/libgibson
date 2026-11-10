#!/usr/bin/env bash
# Build the committed candidate in a fresh archive and check that library explicitly.
# This checks export names only, not function signatures or struct layouts.
set -euo pipefail
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$ROOT"
sha=$(git rev-parse --verify 'HEAD^{commit}')
work=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-abi-candidate.XXXXXXXX")
mkdir "$work/source"
echo "candidate ABI: source commit $sha"
echo "candidate ABI: retained scratch directory $work (remove only after auditing it)"
git archive --format=tar "$sha" | tar -xf - -C "$work/source"
(
    cd "$work/source"
    stable=${STABLE_TOOLCHAIN:-}
    if [[ -n "$stable" ]]; then
        cargo "$stable" build --quiet --locked --release --lib --target-dir "$work/target"
    else
        cargo build --quiet --locked --release --lib --target-dir "$work/target"
    fi
    so="$work/target/release/libgibson.so"
    [[ -f "$so" ]] || { echo "candidate ABI: build did not produce $so" >&2; exit 1; }
    sha256sum "$so"
    scripts/release/check-abi.sh --so "$so"
)
