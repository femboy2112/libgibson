#!/usr/bin/env bash
#
# check-abi.sh — verify the C ABI v1 exported-symbol baseline is intact.
#
# Every symbol listed in abi/gibson-abi-v1.symbols must still be exported by the
# built cdylib. New gibson_* exports are allowed (adding functions is an
# ABI-compatible change per docs/RELEASE_CONTRACT.md) and are reported so the
# baseline can be updated deliberately. A baseline symbol that has DISAPPEARED is a
# failure: removing or renaming an exported symbol is an ABI break that requires a
# GIBSON_ABI_VERSION bump.
#
# This is a release guard for the Linux x86_64 ABI surface, not a full
# cross-platform binary-compatibility proof.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

BASELINE="abi/gibson-abi-v1.symbols"
[[ -f "$BASELINE" ]] || { echo "check-abi: missing $BASELINE" >&2; exit 2; }

SO=""
for cand in target/release/libgibson.so target/debug/libgibson.so; do
    [[ -f "$cand" ]] && { SO="$cand"; break; }
done
if [[ -z "$SO" ]]; then
    echo "check-abi: no libgibson.so found; building (debug)..." >&2
    cargo build >&2
    SO="target/debug/libgibson.so"
fi

CUR="$(nm -D --defined-only "$SO" 2>/dev/null | awk '$2=="T"{print $3}' | grep '^gibson_' | sort)"
BASE="$(grep -v '^#' "$BASELINE" | grep -v '^[[:space:]]*$' | sort)"

missing="$(comm -23 <(printf '%s\n' "$BASE") <(printf '%s\n' "$CUR") || true)"
added="$(comm -13 <(printf '%s\n' "$BASE") <(printf '%s\n' "$CUR") || true)"

echo "check-abi: baseline $(printf '%s\n' "$BASE" | grep -c .) symbols; built $(printf '%s\n' "$CUR" | grep -c .) symbols (from $SO)"

if [[ -n "$added" ]]; then
    echo "check-abi: new exports present (additive / ABI-compatible — update the baseline if intended):"
    printf '  + %s\n' $added
fi

if [[ -n "$missing" ]]; then
    echo "check-abi: FAIL — ABI v1 baseline symbols are missing from the built library." >&2
    echo "check-abi: removing/renaming an exported symbol is an ABI break requiring a GIBSON_ABI_VERSION bump:" >&2
    printf '  - %s\n' $missing >&2
    exit 1
fi

echo "check-abi: OK — all ABI v1 symbols present."
