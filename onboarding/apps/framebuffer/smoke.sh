#!/usr/bin/env bash
# Build from copied consumer and copied adapter source, against a staged SDK.
set -euo pipefail
: "${GIBSON_SDK:?Set GIBSON_SDK to the staged SDK prefix}"
: "${GIBSON_FB_ADAPTER:?Set GIBSON_FB_ADAPTER to a directory containing fb_adapter.c and fb_adapter.h}"
command -v cc >/dev/null || { echo 'missing C compiler' >&2; exit 1; }
[ -f "$GIBSON_SDK/include/gibson.h" ] && [ -f "$GIBSON_SDK/lib/libgibson.so.1" ] || {
    echo 'missing SDK headers or shared library' >&2; exit 1;
}
[ -f "$GIBSON_FB_ADAPTER/fb_adapter.c" ] && [ -f "$GIBSON_FB_ADAPTER/fb_adapter.h" ] || {
    echo 'missing adapter source or header' >&2; exit 1;
}
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-framebuffer.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cc -std=c11 -Wall -Wextra -Werror -O2 -I"$GIBSON_SDK/include" -I"$GIBSON_FB_ADAPTER" \
    "$here/main.c" "$GIBSON_FB_ADAPTER/fb_adapter.c" -L"$GIBSON_SDK/lib" -lgibson -lm -o "$work/app"
export LD_LIBRARY_PATH="$GIBSON_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
"$work/app"
if "$work/app" --wrong-oracle >"$work/negative.log" 2>&1; then
    echo 'wrong-oracle control unexpectedly passed' >&2; exit 1
fi
if ! grep -q 'cell mismatch at 0,0' "$work/negative.log"; then
    cat "$work/negative.log" >&2
    echo 'wrong-oracle control failed for an unexpected reason' >&2; exit 1
fi
echo 'wrong-oracle control rejected as expected'
