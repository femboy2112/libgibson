#!/usr/bin/env bash
# Exercise real C/C++ consumers and a missing-package control without Cargo.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
: "${GIBSON_SDK:?Set GIBSON_SDK to a staged native SDK prefix}"
python="${PYTHON:-python3}"
work="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-sdk-path-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/staged SDK"
cp -R "$GIBSON_SDK/." "$work/staged SDK/"
export PKG_CONFIG_PATH="$work/staged SDK/lib/pkgconfig"
export PKG_CONFIG_LIBDIR="$work/no-system-packages"
export LD_LIBRARY_PATH="$work/staged SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Calibrate the path: the former unquoted shell expansion must fail here.
if cc -std=c11 "$here/c/main.c" $(pkg-config --cflags --libs libgibson) -o "$work/legacy" >"$work/legacy.log" 2>&1; then
    echo 'legacy unquoted pkg-config expansion unexpectedly succeeded' >&2
    exit 1
fi
"$python" "$here/pkg-config-cc.py" cc -std=c11 -Wall -Wextra -Werror "$here/c/main.c" -o "$work/c-example"
"$work/c-example"
"$python" "$here/pkg-config-cc.py" c++ -std=c++17 -Wall -Wextra -Werror "$here/cpp/main.cpp" -o "$work/cpp-example"
"$work/cpp-example"
if PKG_CONFIG_PATH="$work/no-packages" "$python" "$here/pkg-config-cc.py" cc "$here/c/main.c" -o "$work/should-not-exist" >"$work/missing.log" 2>&1; then
    echo 'missing pkg-config package unexpectedly succeeded' >&2
    exit 1
fi
test ! -e "$work/should-not-exist"
echo 'Space-containing SDK path passed; legacy expansion and missing package were rejected.'
