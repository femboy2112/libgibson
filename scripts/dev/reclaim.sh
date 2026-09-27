#!/usr/bin/env bash
#
# reclaim.sh — local dev disk hygiene for the LibGibson release workflow.
#
# The release scripts (cleanroom.sh, bundle.sh, msrv-consumer.sh, preflight.sh) each
# trap-clean their own mktemp trees on EXIT, so they need no help. This script covers
# the parts that are NOT auto-cleaned on a developer machine:
#
#   1. the throwaway consumer-acceptance lab clone (a clone of the external Europa
#      lab, repointed to a release candidate) — see docs/RELEASING.md;
#   2. orphaned /tmp/libgibson-*.XXXXXX build dirs left behind if a build was
#      hard-killed before its EXIT trap could fire;
#   3. opt-in, a full `cargo clean` of the repo target/ (which under the dev profile
#      still grows over a patch train — see the [profile.dev] note in Cargo.toml).
#
# It never deletes anything outside the canonical scratch location or the matched
# /tmp orphan pattern — it will not touch arbitrary paths in your home directory.
#
# Usage:
#   scripts/dev/reclaim.sh                       # sweep lab scratch + orphan temps
#   RECLAIM_CARGO_CLEAN=1 scripts/dev/reclaim.sh # also `cargo clean` the repo target/
#   scripts/dev/reclaim.sh --guard 15            # exit 1 unless >= 15 GiB free (pre-build guard)
#
# Env:
#   LIBGIBSON_LAB_SCRATCH   canonical acceptance-clone dir (default: $TMPDIR/libgibson-lab-scratch)
#   RECLAIM_CARGO_CLEAN=1   also run `cargo clean`
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP="${TMPDIR:-/tmp}"
LAB_SCRATCH="${LIBGIBSON_LAB_SCRATCH:-$TMP/libgibson-lab-scratch}"

free_gb() { df -Pk "${1:-.}" | awk 'NR==2 { print int($4 / 1024 / 1024) }'; }

# --guard N: fail loud BEFORE growing a build tree if free space is under N GiB.
# This is the P3 lesson ("check df FIRST") as code, not a scratchpad reminder.
if [[ "${1:-}" == "--guard" ]]; then
    need="${2:?usage: reclaim.sh --guard <GiB>}"
    have="$(free_gb "$REPO_ROOT")"
    if (( have < need )); then
        echo "reclaim: FATAL — only ${have} GiB free at $REPO_ROOT, need ${need} GiB." >&2
        echo "reclaim: run 'scripts/dev/reclaim.sh' (optionally RECLAIM_CARGO_CLEAN=1) first." >&2
        exit 1
    fi
    echo "reclaim: OK — ${have} GiB free (>= ${need} required)."
    exit 0
fi

echo "=== reclaim: before ==="
df -h "$REPO_ROOT" | awk 'NR==1 || NR==2'

# 1. Ephemeral consumer-acceptance lab clone.
if [[ -e "$LAB_SCRATCH" ]]; then
    echo "reclaim: removing lab scratch $LAB_SCRATCH"
    rm -rf "$LAB_SCRATCH"
fi

# 2. Orphaned release-script / python-build temp dirs whose EXIT trap did not fire.
shopt -s nullglob
orphans=(
    "$TMP"/libgibson-cleanroom.*
    "$TMP"/libgibson-bundle.*
    "$TMP"/libgibson-msrv.*
    "$TMP"/libgibson-py.*
)
if (( ${#orphans[@]} )); then
    echo "reclaim: removing ${#orphans[@]} orphaned temp dir(s):"
    printf '  %s\n' "${orphans[@]}"
    rm -rf "${orphans[@]}"
fi
shopt -u nullglob

# 3. Opt-in: full cargo clean of the repo target/.
if [[ "${RECLAIM_CARGO_CLEAN:-0}" == "1" ]]; then
    echo "reclaim: cargo clean ($REPO_ROOT/target)"
    ( cd "$REPO_ROOT" && cargo clean )
fi

echo "=== reclaim: after ==="
df -h "$REPO_ROOT" | awk 'NR==2'
