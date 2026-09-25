#!/usr/bin/env bash
#
# preflight.sh — the merciless, publication-free release-candidate gate.
#
# Answers one question: "is this tree capable of producing a release candidate?"
# It runs finite, deterministic checks and prints a readable summary ending in
# either RELEASE PREFLIGHT: PASS or RELEASE PREFLIGHT: FAIL. It publishes nothing.
#
# Toolchain selection:
#   STABLE_TOOLCHAIN   e.g. "+1.98.1" — the comprehensive toolchain (default: the
#                      active default toolchain, which must have rustfmt + clippy).
#   MSRV_TOOLCHAIN     the declared MSRV (default "+1.85").

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
STABLE="${STABLE_TOOLCHAIN:-}"
MSRV="${MSRV_TOOLCHAIN:-+1.85}"
fail=0

step()  { printf '\n==== %s ====\n' "$*"; }
ok()    { printf '  [PASS] %s\n' "$*"; }
no()    { printf '  [FAIL] %s\n' "$*"; fail=1; }
run()   { if "$@" >/dev/null 2>&1; then ok "$*"; else no "$*"; fi; }

step "worktree whitespace"
if git diff --check; then ok "git diff --check"; else no "git diff --check"; fi

step "version + ABI consistency"
run scripts/release/check-versions.sh

step "format"
run cargo $STABLE fmt --check

step "clippy (-D warnings)"
run cargo $STABLE clippy --all-targets --all-features -- -D warnings

step "tests"
run cargo $STABLE test

step "rustdoc (-D warnings)"
if RUSTDOCFLAGS="-D warnings" cargo $STABLE doc --no-deps >/dev/null 2>&1; then ok "rustdoc"; else no "rustdoc"; fi

step "MSRV library check — resolved-release floor ($MSRV, committed lock)"
if cargo $MSRV check --locked --lib >/dev/null 2>&1; then ok "MSRV $MSRV check --locked --lib"; else no "MSRV check"; fi

step "MSRV declared-range consumer — fresh resolution ($MSRV)"
if MSRV_TOOLCHAIN="$MSRV" scripts/release/msrv-consumer.sh >/dev/null 2>&1; then
    ok "declared dependency ranges resolve to a $MSRV-buildable graph"
else
    no "declared-range MSRV fresh resolution (a dependency upgrade may have raised the effective MSRV)"
fi

step "cargo package"
run cargo $STABLE package --allow-dirty

step "ABI v1 symbol baseline"
run scripts/release/check-abi.sh

step "clean-room external consumers (C, C++, Python, Go, Rust)"
run scripts/release/cleanroom.sh

step "third-party notices freshness"
tpn="$(mktemp)"
if scripts/release/gen-third-party-notices.sh "$tpn" >/dev/null 2>&1 && diff -q "$tpn" THIRD-PARTY-NOTICES.md >/dev/null 2>&1; then
    ok "THIRD-PARTY-NOTICES.md matches generated"
else
    # A release candidate must not knowingly ship stale dependency notices.
    no "THIRD-PARTY-NOTICES.md differs from freshly generated output (run scripts/release/gen-third-party-notices.sh)"
fi
rm -f "$tpn"

step "license files present"
missing=0
for f in LICENSE-MIT LICENSE-APACHE LICENSES-THIRD-PARTY.md THIRD-PARTY-NOTICES.md; do
    [ -f "$f" ] || { echo "  missing $f"; missing=1; }
done
if [ "$missing" = 0 ]; then ok "license/notice files present"; else no "license files"; fi

echo
if [ "$fail" -ne 0 ]; then echo "RELEASE PREFLIGHT: FAIL"; exit 1; fi
echo "RELEASE PREFLIGHT: PASS"
