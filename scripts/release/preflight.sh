#!/usr/bin/env bash
# Run every release check from an isolated checkout of one captured commit.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
sha=$(git rev-parse --verify 'HEAD^{commit}')
work=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-preflight.XXXXXXXX")
work=$(cd "$work" && pwd -P)
echo "release preflight: captured commit $sha"
echo "release preflight: retained scratch $work (remove only after auditing it)"
# A clone is independent of the caller's index, ignored files, and build outputs.
# No source branches or existing worktrees are modified; no remote is contacted.
git -c core.hooksPath=/dev/null clone --quiet --no-hardlinks --no-checkout "$root" "$work/checkout"
git -C "$work/checkout" -c core.hooksPath=/dev/null checkout --quiet --detach "$sha"
[[ $(git -C "$work/checkout" rev-parse --verify 'HEAD^{commit}') == "$sha" ]] || {
    echo "release preflight: detached checkout did not resolve to captured commit" >&2; exit 1;
}
# Do not inherit a target directory that could contain another candidate's artifacts.
export CARGO_TARGET_DIR="$work/cargo-target"
cd "$work/checkout"
exec scripts/release/preflight-inner.sh "$sha"
