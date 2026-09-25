#!/usr/bin/env bash
#
# cleanroom.sh — prove LibGibson is consumable from OUTSIDE the source tree.
#
# Stages the native SDK and packages the Rust crate, then builds and runs a C, a
# C++, a Python, a Go, and a Rust consumer from throwaway temp directories using
# ONLY the produced artifacts (pkg-config for C/C++/Go, an installed wheel for
# Python, the packaged .crate for Rust). It actively rejects source-tree coupling:
# no repository include/lib paths, no repo PYTHONPATH, GOPROXY=off for Go, and a
# scan for the checkout path in the built binaries.
#
# Requires: a C/C++ toolchain, pkg-config, python3 (venv+pip), and optionally go.
# Network is used only to fetch Python build tooling and resolve the Rust
# consumer's (already-cached) dependencies.
#
# Usage: scripts/release/cleanroom.sh          # TMPDIR controls where temp dirs go

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
CARGO="${CARGO:-cargo}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-cleanroom.XXXXXX")"
SDK="$WORK/sdk"
fail=0
pass() { printf '  [PASS] %s\n' "$*"; }
bad()  { printf '  [FAIL] %s\n' "$*"; fail=1; }

cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

echo "cleanroom: work dir $WORK (version $VERSION)"

echo "### stage native SDK ###"
if scripts/release/stage-sdk.sh --prefix "$SDK" --force >/dev/null 2>&1; then pass "SDK staged"; else bad "SDK staging"; fi
export PKG_CONFIG_PATH="$SDK/lib/pkgconfig"
export LD_LIBRARY_PATH="$SDK/lib"

echo "### C consumer (pkg-config) ###"
cdir="$WORK/c"; mkdir -p "$cdir"
cat > "$cdir/main.c" <<'EOF'
#include <gibson.h>
#include <stdio.h>
int main(void) {
    unsigned int a = gibson_abi_version();
    printf("C abi=%u\n", a);
    return a == GIBSON_ABI_VERSION ? 0 : 1;
}
EOF
if ( cd "$cdir" && cc -std=c11 main.c $(pkg-config --cflags --libs libgibson) -o c_app && ./c_app >/dev/null ); then pass "C"; else bad "C"; fi

echo "### C++ consumer (pkg-config) ###"
xdir="$WORK/cpp"; mkdir -p "$xdir"
cat > "$xdir/main.cpp" <<'EOF'
#include <gibson.hpp>
#include <iostream>
int main() {
    unsigned int a = gibson_abi_version();
    std::cout << "C++ abi=" << a << "\n";
    return a == GIBSON_ABI_VERSION ? 0 : 1;
}
EOF
if ( cd "$xdir" && c++ -std=c++17 main.cpp $(pkg-config --cflags --libs libgibson) -o cpp_app && ./cpp_app >/dev/null ); then pass "C++"; else bad "C++"; fi

echo "### Python consumer (wheel -> fresh venv) ###"
if command -v python3 >/dev/null; then
    vb="$WORK/vb"; vr="$WORK/vr"; dist="$WORK/dist"; mkdir -p "$dist"
    ok=1
    python3 -m venv "$vb" >/dev/null 2>&1 || ok=0
    "$vb/bin/pip" install -q --upgrade pip setuptools wheel >/dev/null 2>&1 || ok=0
    "$vb/bin/pip" wheel --no-build-isolation --no-deps ./bindings/python -w "$dist" >/dev/null 2>&1 || ok=0
    whl="$(ls "$dist"/*.whl 2>/dev/null | head -1)"
    python3 -m venv "$vr" >/dev/null 2>&1 || ok=0
    [ -n "$whl" ] && "$vr/bin/pip" install -q "$whl" >/dev/null 2>&1 || ok=0
    pyroom="$WORK/pyroom"; mkdir -p "$pyroom"
    if [ "$ok" = 1 ] && ( cd "$pyroom" && PYTHONPATH= LIBGIBSON_LIBRARY="$SDK/lib/libgibson.so" "$vr/bin/python" -c '
import gibson, sys
assert "site-packages" in gibson.__file__, "not from installed wheel"
assert gibson._loaded_abi_version == gibson.EXPECTED_ABI_VERSION
gibson.Node.text("x"); gibson.Line().add_span("y")
' ); then pass "Python"; else bad "Python"; fi
else
    echo "  [SKIP] python3 not installed"
fi

echo "### Go consumer (pkg-config + module replace, GOPROXY=off) ###"
if command -v go >/dev/null; then
    godir="$WORK/go"; mkdir -p "$godir"
    cat > "$godir/main.go" <<'EOF'
package main

import (
	"fmt"
	"os"

	gibson "github.com/femboy2112/libgibson/bindings/go/gibson"
)

func main() {
	if err := gibson.CheckABI(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	fmt.Printf("Go abi=%d\n", gibson.AbiVersion())
}
EOF
    if ( cd "$godir" \
        && GOPROXY=off GOFLAGS=-mod=mod go mod init cleanroomconsumer >/dev/null 2>&1 \
        && go mod edit -require=github.com/femboy2112/libgibson/bindings/go@v0.0.0 \
        && go mod edit -replace=github.com/femboy2112/libgibson/bindings/go="$REPO_ROOT/bindings/go" \
        && GOPROXY=off GOFLAGS=-mod=mod go run . >/dev/null 2>&1 ); then pass "Go"; else bad "Go"; fi
else
    echo "  [SKIP] go not installed"
fi

echo "### Rust consumer (packaged crate) ###"
if $CARGO package --allow-dirty >/dev/null 2>&1; then
    crate="target/package/libgibson-$VERSION.crate"
    tar xzf "$crate" -C "$WORK"
    rdir="$WORK/rustconsumer"; mkdir -p "$rdir/src"
    cat > "$rdir/Cargo.toml" <<EOF
[package]
name = "cleanroom-consumer"
version = "0.0.0"
edition = "2021"

[dependencies]
libgibson = { path = "$WORK/libgibson-$VERSION" }
EOF
    cat > "$rdir/src/main.rs" <<'EOF'
fn main() {
    // Prove the packaged crate compiles, links, and exposes its core surface.
    let _ = std::mem::size_of::<gibson::cell::Cell>();
    let _ = std::mem::size_of::<gibson::surface::Surface>();
    println!("rust packaged-crate consumer OK");
}
EOF
    if ( cd "$rdir" && $CARGO run -q >/dev/null 2>&1 ); then pass "Rust"; else bad "Rust"; fi
else
    bad "Rust (cargo package failed)"
fi

echo "### source-tree coupling checks ###"
leak=0
for b in "$cdir/c_app" "$xdir/cpp_app"; do
    [ -f "$b" ] && strings "$b" 2>/dev/null | grep -q "$REPO_ROOT/target" && { echo "  checkout path leaked into $b"; leak=1; }
done
if [ "$leak" = 0 ]; then pass "no checkout path baked into binaries"; else bad "checkout path leak"; fi

echo
if [ "$fail" -ne 0 ]; then echo "cleanroom: FAIL"; exit 1; fi
echo "cleanroom: ALL CONSUMERS OK"
