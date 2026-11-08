#!/usr/bin/env bash
# fb_adapter_consumer.sh — v0.7 node J (in-repo, non-GPL prototype).
#
# Builds the generic software-framebuffer adapter (bindings/framebuffer_adapter/) from a TEMP DIR
# against an INSTALLED LibGibson SDK prefix — pkg-config only, never -Iinclude or -Ltarget — then
# runs its property self-check (7 properties, headless + one real-PTY property), the independent
# Python PTY harness, and every negative control (each must FAIL). Exits nonzero on any failure.
#
# Usage:
#   scripts/dev/fb_adapter_consumer.sh [SDK_PREFIX]
#     SDK_PREFIX (or env FB_ADAPTER_SDK_PREFIX): an SDK staged by scripts/release/stage-sdk.sh.
#     Without one, an SDK is staged into a temp prefix with
#       scripts/release/stage-sdk.sh --prefix <tmp> --force      (runs a release cargo build)
#   env FB_ADAPTER_SKIP_CONTROLS=1 skips the negative controls (they take ~2 minutes).
set -euo pipefail
# No child of this check may reach a desktop (an engine or tool that pops a GUI would block).
unset DISPLAY WAYLAND_DISPLAY

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$REPO_ROOT/bindings/framebuffer_adapter"

SDK="${1:-${FB_ADAPTER_SDK_PREFIX:-}}"
STAGE=""
WORK="$(mktemp -d "${TMPDIR:-/tmp}/fb-adapter-app.XXXXXX")"
cleanup() { rm -rf "$WORK"; if [[ -n "$STAGE" ]]; then rm -rf "$STAGE"; fi; }
trap cleanup EXIT

if [[ -z "$SDK" ]]; then
    STAGE="$(mktemp -d "${TMPDIR:-/tmp}/fb-adapter-sdk.XXXXXX")"
    echo "== 0. stage the SDK into a prefix outside the checkout: $STAGE =="
    ( cd "$REPO_ROOT" && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" scripts/release/stage-sdk.sh --prefix "$STAGE" --force )
    SDK="$STAGE"
fi
SDK="$(cd "$SDK" && pwd)"

echo "== 1. copy ONLY the adapter sources out of the checkout: $WORK =="
for f in fb_adapter.h fb_adapter.c synthetic_producer.h synthetic_producer.c fb_adapter_demo.c pty_harness.py; do
    cp "$SRC/$f" "$WORK/"
done

echo "== 2. resolve the installed SDK via pkg-config (prefix $SDK) =="
export PKG_CONFIG_PATH="$SDK/lib/pkgconfig"
pkg-config --exists libgibson || { echo "libgibson.pc not found under $PKG_CONFIG_PATH" >&2; exit 1; }
CFLAGS_PC="$(pkg-config --cflags libgibson)"
LIBS_PC="$(pkg-config --libs libgibson)"
LIBDIR="$(pkg-config --variable=libdir libgibson)"
echo "   version: $(pkg-config --modversion libgibson)"
echo "   cflags:  $CFLAGS_PC"
echo "   libs:    $LIBS_PC"
case "$CFLAGS_PC $LIBS_PC" in
    *"$REPO_ROOT"*) echo "the SDK flags point into the checkout; refusing" >&2; exit 1 ;;
esac

echo "== 3. compile (C11, -Wall -Wextra -Werror) from the temp dir =="
( cd "$WORK" && cc -std=c11 -Wall -Wextra -Werror -O2 fb_adapter.c synthetic_producer.c fb_adapter_demo.c \
    $CFLAGS_PC $LIBS_PC -lm -Wl,-rpath,"$LIBDIR" -o fb_adapter_demo )
LINKED="$(ldd "$WORK/fb_adapter_demo" | awk '/libgibson/ {print $3}')"
echo "   libgibson resolved to: $LINKED"
case "$LINKED" in
    "$SDK"/*|"$(cd "$LIBDIR" && pwd)"/*) ;;
    *) echo "the demo did not link the SDK's libgibson" >&2; exit 1 ;;
esac

echo "== 4a. the P7 bench child rejects runs too short for its windows (no out-of-bounds read) =="
if "$WORK/fb_adapter_demo" --p7-child 80 24 1 none > "$WORK/guard.log" 2>&1; then
    echo "--p7-child accepted a 1-frame run" >&2; cat "$WORK/guard.log" >&2; exit 1
else
    rc=$?
    [[ $rc -eq 2 ]] || { echo "--p7-child 1 frame: exit $rc, want 2" >&2; exit 1; }
    grep -q "needs a non-zero grid and 1200" "$WORK/guard.log" || { cat "$WORK/guard.log" >&2; exit 1; }
    echo "   --p7-child 80 24 1 none -> exit 2: $(cat "$WORK/guard.log")"
fi

echo "== 4. property self-check (P1-P7) =="
"$WORK/fb_adapter_demo" --selfcheck | tee "$WORK/selfcheck.log"
grep -qx "fb_adapter: OK" "$WORK/selfcheck.log"

echo "== 5. real-PTY harness (P2 wire check if pyte is present; P3, P5, P6, P7 through a pty) =="
python3 "$WORK/pty_harness.py" --demo "$WORK/fb_adapter_demo" | tee "$WORK/pty.log"
grep -qx "pty_harness: OK" "$WORK/pty.log"

if [[ "${FB_ADAPTER_SKIP_CONTROLS:-0}" != 1 ]]; then
    echo "== 6. negative controls: every one must FAIL its property =="
    failed_ok=0
    while IFS=$'\t' read -r name prop what; do
        if "$WORK/fb_adapter_demo" --selfcheck --control "$name" > "$WORK/c.log" 2>&1; then
            echo "CONTROL $name ($prop) did NOT fail: the check cannot see '$what'" >&2
            cat "$WORK/c.log" >&2
            exit 1
        fi
        n="${prop#P}"
        grep -q "^PROP $n .*: FAIL" "$WORK/c.log" || { echo "CONTROL $name: no FAIL line for $prop" >&2; cat "$WORK/c.log" >&2; exit 1; }
        echo "   selfcheck control $name -> $prop FAIL (as required)"
        failed_ok=$((failed_ok + 1))
    done < <("$WORK/fb_adapter_demo" --selfcheck --list-controls)
    pty_controls="p3-ignore-input p5-resize-steps p5-stale-map p6-no-restore p6-no-signals p6-retry-io p6-sticky-signal p7-slow-present"
    if python3 -c "import pyte" 2>/dev/null; then pty_controls="p2-wire-shifted-cells $pty_controls"; fi
    for name in $pty_controls; do
        if python3 "$WORK/pty_harness.py" --demo "$WORK/fb_adapter_demo" --control "$name" > "$WORK/c.log" 2>&1; then
            echo "PTY CONTROL $name did NOT fail" >&2
            cat "$WORK/c.log" >&2
            exit 1
        fi
        grep -q "^PTY P.* FAIL" "$WORK/c.log" || { echo "PTY CONTROL $name: no FAIL line" >&2; cat "$WORK/c.log" >&2; exit 1; }
        if [[ "$name" == p2-wire-shifted-cells ]]; then
            # it must fail on cells alone: the state hash matched, and cells mismatched
            if grep -q "hash=MISMATCH" "$WORK/c.log" || ! grep -qE "mismatches=[1-9][0-9]*" "$WORK/c.log"; then
                echo "PTY CONTROL $name did not fail on cells alone" >&2; cat "$WORK/c.log" >&2; exit 1
            fi
        fi
        echo "   pty control $name -> FAIL (as required)"
        failed_ok=$((failed_ok + 1))
    done
    echo "   $failed_ok negative controls failed as required"
fi

echo "== node J framebuffer adapter: installed-SDK consumer PASSED =="
