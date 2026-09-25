#!/usr/bin/env bash
#
# Local binding smoke test for LibGibson.
#
# Builds the release library, then compiles and runs the C, C++ and Python
# examples. With `--asan`, the C and C++ examples are additionally compiled and
# run under AddressSanitizer + UndefinedBehaviorSanitizer. LeakSanitizer is
# disabled (detect_leaks=0) because the engine intentionally keeps process-global
# terminal state; ASan/UBSan remain active.
#
# Go bindings are checked when a Go toolchain is installed; absence is reported.
# A present but failing toolchain is a hard failure, not a best-effort pass.
#
# Usage: scripts/dev/bindings_smoke.sh [--asan]

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

ASAN=0
for arg in "$@"; do
    case "$arg" in
        --asan) ASAN=1 ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

echo "==> cargo build --release"
cargo build --release

# The cdylib carries an ELF SONAME of libgibson.so.1 (build.rs), so a consumer that
# links the in-tree build output records NEEDED=libgibson.so.1 and needs that name
# present at runtime. The staged SDK ships the symlink; mirror it next to the raw
# build output so in-tree C/C++ examples load.
ln -sf libgibson.so "$ROOT/target/release/libgibson.so.1"

RPATH="$ROOT/target/release"
OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT

run_native() {
    local name="$1" compiler="$2" src="$3" std="$4"
    local bin="$OUT/example_$name"
    echo "==> C++/C example: $name"
    if [ -n "$std" ]; then
        "$compiler" "$std" -Iinclude "$src" -Ltarget/release -lgibson \
            -Wl,-rpath,"$RPATH" -o "$bin"
    else
        "$compiler" -Iinclude "$src" -Ltarget/release -lgibson \
            -Wl,-rpath,"$RPATH" -o "$bin"
    fi
    "$bin"
}

run_asan() {
    local name="$1" compiler="$2" src="$3" std="$4"
    local bin="$OUT/example_${name}_asan"
    echo "==> ASan/UBSan example: $name"
    if [ -n "$std" ]; then
        "$compiler" "$std" -g -fsanitize=address,undefined -Iinclude "$src" \
            -Ltarget/release -lgibson -Wl,-rpath,"$RPATH" -o "$bin"
    else
        "$compiler" -g -fsanitize=address,undefined -Iinclude "$src" \
            -Ltarget/release -lgibson -Wl,-rpath,"$RPATH" -o "$bin"
    fi
    ASAN_OPTIONS=detect_leaks=0 "$bin"
}

run_native  c   gcc  bindings/c/example.c ""
run_native  cpp g++  bindings/cpp/example.cpp "-std=c++17"
LIBGIBSON_LIBRARY="$ROOT/target/release/libgibson.so" \
    PYTHONPATH=bindings/python python3 bindings/python/example.py

if [ "$ASAN" -eq 1 ]; then
    run_asan c   gcc bindings/c/example.c ""
    run_asan cpp g++ bindings/cpp/example.cpp "-std=c++17"
fi

if command -v go >/dev/null 2>&1; then
    echo "==> Go bindings (vet, test, build, example)"
    (
        cd bindings/go
        export LD_LIBRARY_PATH="$ROOT/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        go vet ./...
        go test ./...
        go build ./...
        go run ./cmd/example
    )
else
    echo "==> Go toolchain not found; skipping Go bindings (UNVERIFIED)."
fi

echo "==> bindings smoke test complete"
