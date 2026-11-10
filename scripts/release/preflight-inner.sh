#!/usr/bin/env bash
#
# preflight-inner.sh — internal body of the isolated release-candidate gate.
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
[[ $# == 1 ]] || { echo 'invoke scripts/release/preflight.sh instead' >&2; exit 2; }
expected_sha=$1
if ! candidate_sha=$(git rev-parse --verify 'HEAD^{commit}') || [ "$candidate_sha" != "$expected_sha" ]; then
    echo 'RELEASE PREFLIGHT: FAIL — cannot resolve candidate HEAD' >&2
    exit 1
fi

step()  { printf '\n==== %s ====\n' "$*"; }
ok()    { printf '  [PASS] %s\n' "$*"; }
no()    { printf '  [FAIL] %s\n' "$*"; fail=1; }
# Surface the cause of a failed step. A release gate that discards its own output to /dev/null is
# unreviewable: a red preflight (e.g. CI run 41375536305) then reports *that* it failed but never
# *why*, so the app/SDK defect cannot be separated from a toolchain/golden effect. Dumped ONLY on
# failure (success stays quiet). The cause often sits FAR above the end of a long log — e.g. one
# FAILED test among hundreds of passing ones, which a plain tail would miss — so surface the
# failure-signature lines from the WHOLE log first, then a bounded tail for context. Both are
# capped so a megabyte of output can never bury the PASS/FAIL summary.
dump()  {
    local sig
    sig="$(grep -nE 'error(\[|:)|FAILED|panicked|assertion|\[FAIL\]|cannot find|unresolved|undefined (reference|symbol)|mismatch' "$1" | head -n 30)"
    [ -n "$sig" ] && printf '%s\n' "$sig" | sed 's/^/    !| /'
    tail -n 40 "$1" | sed 's/^/    | /'
}
run()   {
    local log; log="$(mktemp)"
    if "$@" >"$log" 2>&1; then ok "$*"; else no "$*"; dump "$log"; fi
    rm -f "$log"
}

step "clean candidate checkout ($candidate_sha)"
if checkout_status=$(git status --porcelain --untracked-files=all) && [ -z "$checkout_status" ]; then
    ok "git status reports no tracked or untracked changes"
else
    no "candidate checkout is dirty or cannot be inspected"
    printf '%s\n' "$checkout_status"
fi
if scripts/release/check-clean-index.py; then
    ok "no assume-unchanged or skip-worktree flags reported"
else
    no "index has flags hiding tracked changes or could not be inspected"
fi

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

# The terminal-hangup liveness tests (#79, #269) also run in the full suite above;
# this gate runs them again on their own, serially and once, on the supported Linux
# release host, so a release names them explicitly. --include-ignored keeps the gate
# honest if one is ever ignored again. Fail rather than silently accept a skipped gate.
step "terminal hangup regressions (Linux PTY, serial, single attempt)"
if [ "$(uname -s)" = "Linux" ]; then
    run cargo $STABLE test --test pty_hangup --test pty_hangup_restore --test pty_hangup_matrix -- --test-threads=1 --include-ignored
else
    no "PTY hangup release gate requires the supported Linux x86_64 runner"
fi

step "rustdoc (-D warnings)"
dlog="$(mktemp)"
if RUSTDOCFLAGS="-D warnings" cargo $STABLE doc --no-deps >"$dlog" 2>&1; then ok "rustdoc"; else no "rustdoc"; dump "$dlog"; fi
rm -f "$dlog"

step "MSRV library check — resolved-release floor ($MSRV, committed lock)"
dlog="$(mktemp)"
if cargo $MSRV check --locked --lib >"$dlog" 2>&1; then ok "MSRV $MSRV check --locked --lib"; else no "MSRV check"; dump "$dlog"; fi
rm -f "$dlog"

step "MSRV declared-range consumer — fresh resolution ($MSRV)"
dlog="$(mktemp)"
if MSRV_TOOLCHAIN="$MSRV" scripts/release/msrv-consumer.sh >"$dlog" 2>&1; then
    ok "declared dependency ranges resolve to a $MSRV-buildable graph"
else
    no "declared-range MSRV fresh resolution (a dependency upgrade may have raised the effective MSRV)"
    dump "$dlog"
fi
rm -f "$dlog"

step "cargo package"
run cargo $STABLE package --allow-dirty

step "ABI v1 symbol baseline (fresh build of committed candidate)"
if scripts/release/tests/test-stage-sdk-target.sh; then
    ok "SDK staging artifact-selection controls"
else
    no "SDK staging artifact-selection controls"
fi
if scripts/release/tests/test-cleanroom-package.sh; then
    ok "cleanroom crate selection controls"
else
    no "cleanroom crate selection controls"
fi
if scripts/release/tests/test-check-abi.sh; then
    ok "ABI checker fault controls"
else
    no "ABI checker fault controls"
fi
if scripts/release/tests/test-check-clean-index.sh; then
    ok "release index flag fault controls"
else
    no "release index flag fault controls"
fi
if scripts/release/check-candidate-abi.sh "$candidate_sha"; then
    ok "committed candidate ABI symbol baseline"
else
    no "committed candidate ABI symbol baseline"
fi

step "clean-room external consumers (C, C++, Python, Go, Rust)"
run scripts/release/cleanroom.sh

step "third-party notices freshness"
tpn="$(mktemp)"
glog="$(mktemp)"
if scripts/release/gen-third-party-notices.sh "$tpn" >"$glog" 2>&1 && diff -q "$tpn" THIRD-PARTY-NOTICES.md >/dev/null 2>&1; then
    ok "THIRD-PARTY-NOTICES.md matches generated"
else
    # A release candidate must not knowingly ship stale dependency notices.
    no "THIRD-PARTY-NOTICES.md differs from freshly generated output (run scripts/release/gen-third-party-notices.sh)"
    # Show the generator's own errors, then the actual diff, so the cause is reviewable.
    dump "$glog"
    diff -u THIRD-PARTY-NOTICES.md "$tpn" 2>&1 | sed 's/^/    | /' | head -n 60
fi
rm -f "$tpn" "$glog"

step "license files present"
missing=0
for f in LICENSE-MIT LICENSE-APACHE LICENSES-THIRD-PARTY.md THIRD-PARTY-NOTICES.md; do
    [ -f "$f" ] || { echo "  missing $f"; missing=1; }
done
if [ "$missing" = 0 ]; then ok "license/notice files present"; else no "license files"; fi

step "candidate identity after preflight"
if final_sha=$(git rev-parse --verify 'HEAD^{commit}') && \
   [ "$final_sha" = "$candidate_sha" ] && \
   checkout_status=$(git status --porcelain --untracked-files=all) && [ -z "$checkout_status" ] && \
   scripts/release/check-clean-index.py; then
    ok "candidate $candidate_sha remains checked out; git status reports clean"
else
    no "candidate changed or checkout became dirty during preflight"
    printf '%s\n' "${checkout_status:-}"
fi

echo
if [ "$fail" -ne 0 ]; then echo "RELEASE PREFLIGHT: FAIL"; exit 1; fi
echo "RELEASE PREFLIGHT: PASS"
