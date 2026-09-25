#!/usr/bin/env bash
#
# cleanroom.sh — prove LibGibson is consumable from OUTSIDE the source tree.
#
# Stages the native SDK and packages the Rust crate, then builds and runs a C, a
# C++, a Python, a Go, and a Rust consumer from throwaway temp directories using
# ONLY the produced artifacts (pkg-config for C/C++/Go, an installed wheel for
# Python, the packaged .crate for Rust). It actively rejects source-tree coupling:
# no repository include/lib paths, no repo PYTHONPATH, GOPROXY=off for Go, the Go
# module is built from a copy staged OUTSIDE the checkout, and it scans the built
# binaries for the checkout path.
#
# It also asserts the actual distributable artifacts carry the project's license
# terms (Python wheel + sdist, Go module payload) — classifiers/metadata alone are
# not accepted as evidence.
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
godir=""   # set only if the Go consumer is built; referenced under `set -u` later
fail=0
pass() { printf '  [PASS] %s\n' "$*"; }
bad()  { printf '  [FAIL] %s\n' "$*"; fail=1; }

# D6-3 license-payload checkers for the ACTUAL built Python artifacts.
py_wheel_license_ok() {
    python3 - "$1" <<'PY'
import sys, zipfile
z = zipfile.ZipFile(sys.argv[1]); names = z.namelist()
need = ["LICENSE", "LICENSE-MIT", "LICENSE-APACHE"]
have_files = all(any(n.endswith("dist-info/" + f) for n in names) for f in need)
meta = [n for n in names if n.endswith("METADATA")][0]
body = z.read(meta).decode()
have_meta = "License-File: LICENSE-MIT" in body and "License-File: LICENSE-APACHE" in body
sys.exit(0 if (have_files and have_meta) else 1)
PY
}
py_sdist_license_ok() {
    python3 - "$1" <<'PY'
import sys, tarfile
t = tarfile.open(sys.argv[1]); names = t.getnames()
need = ["LICENSE", "LICENSE-MIT", "LICENSE-APACHE"]
sys.exit(0 if all(any(n.endswith("/" + f) for n in names) for f in need) else 1)
PY
}

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
    // Positive agreement path of the C++ ABI gate (D6-1): abi_compatible() must be
    // true when the header and the loaded native library agree on the ABI version.
    // The Context constructor enforces the same predicate and throws on mismatch.
    if (!gibson::Context::abi_compatible()) {
        std::cerr << "C++ ABI gate rejected a matching library\n";
        return 1;
    }
    unsigned int a = gibson::Context::abi_version();
    std::cout << "C++ abi=" << a << "\n";
    return a == GIBSON_ABI_VERSION ? 0 : 1;
}
EOF
if ( cd "$xdir" && c++ -std=c++17 main.cpp $(pkg-config --cflags --libs libgibson) -o cpp_app && ./cpp_app >/dev/null ); then pass "C++"; else bad "C++"; fi

echo "### Python consumer (wheel + sdist -> fresh venv) ###"
if command -v python3 >/dev/null; then
    # Build both artifacts from a COPY so the source tree is never littered.
    pysrc="$WORK/pysrc"; cp -a bindings/python "$pysrc"
    rm -rf "$pysrc/build" "$pysrc/dist" "$pysrc"/*.egg-info "$pysrc/__pycache__"
    vr="$WORK/vr"; dist="$WORK/dist"; sdout="$WORK/sdist"; mkdir -p "$dist" "$sdout"
    ok=1
    ( cd "$pysrc" && python3 -c "from setuptools import build_meta as b; b.build_wheel('$dist')" ) >/dev/null 2>&1 || ok=0
    ( cd "$pysrc" && python3 -c "from setuptools import build_meta as b; b.build_sdist('$sdout')" ) >/dev/null 2>&1 || ok=0
    whl="$(ls "$dist"/*.whl 2>/dev/null | head -1)"
    sdist="$(ls "$sdout"/*.tar.gz 2>/dev/null | head -1)"
    # D6-3: the ACTUAL built artifacts must physically carry the dual-license text.
    if [ -n "$whl" ] && py_wheel_license_ok "$whl"; then pass "Python wheel carries license text + metadata"; else bad "Python wheel license payload"; fi
    if [ -n "$sdist" ] && py_sdist_license_ok "$sdist"; then pass "Python sdist carries license text"; else bad "Python sdist license payload"; fi
    # Install the wheel into a fresh venv and exercise it against the staged .so.
    python3 -m venv "$vr" >/dev/null 2>&1 || ok=0
    [ -n "$whl" ] && "$vr/bin/pip" install -q "$whl" >/dev/null 2>&1 || ok=0
    pyroom="$WORK/pyroom"; mkdir -p "$pyroom"
    if [ "$ok" = 1 ] && ( cd "$pyroom" && PYTHONPATH= LIBGIBSON_LIBRARY="$SDK/lib/libgibson.so" "$vr/bin/python" -c '
import gibson
assert "site-packages" in gibson.__file__, "not from installed wheel"
assert gibson._loaded_abi_version == gibson.EXPECTED_ABI_VERSION
gibson.Node.text("x"); gibson.Line().add_span("y")
' ); then pass "Python"; else bad "Python"; fi
else
    echo "  [SKIP] python3 not installed"
fi

echo "### Go consumer (staged module copy + pkg-config, GOPROXY=off) ###"
if command -v go >/dev/null; then
    # D6-2: copy the publishable Go module payload OUT of the checkout — exactly
    # what a tagged module (github.com/femboy2112/libgibson/bindings/go@vX.Y.Z)
    # would expose — and build the external consumer against THAT copy. The
    # consumer's `replace` therefore points at a staged artifact, never at
    # $REPO_ROOT, proving the Go wrapper has no dependency on repo-relative layout.
    gomod="$WORK/gomod"; mkdir -p "$gomod"
    cp -a bindings/go/. "$gomod/"

    # D6-3: that tagged module must carry the project's license terms at its root
    # (a subdirectory module does NOT inherit the repository-root license).
    golic=1
    for lf in LICENSE LICENSE-MIT LICENSE-APACHE; do
        [ -f "$gomod/$lf" ] || { echo "  Go module payload missing $lf"; golic=0; }
    done
    if [ "$golic" = 1 ]; then pass "Go module payload carries dual-license files"; else bad "Go module license payload"; fi

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
    gook=1
    ( cd "$godir" \
        && GOPROXY=off GOFLAGS=-mod=mod go mod init cleanroomconsumer >/dev/null 2>&1 \
        && go mod edit -require=github.com/femboy2112/libgibson/bindings/go@v0.0.0 \
        && go mod edit -replace=github.com/femboy2112/libgibson/bindings/go="$gomod" \
        && GOPROXY=off GOFLAGS=-mod=mod go build -o go_app . >/dev/null 2>&1 \
        && ./go_app >/dev/null 2>&1 ) || gook=0
    # The consumer's go.mod must reference the staged copy, never the checkout.
    if [ "$gook" = 1 ] && ! grep -q "$REPO_ROOT/bindings/go" "$godir/go.mod"; then
        pass "Go (built from staged copy; no checkout path in go.mod)"
    else
        [ "$gook" = 1 ] && grep -q "$REPO_ROOT/bindings/go" "$godir/go.mod" && echo "  checkout path leaked into go.mod"
        bad "Go"
    fi
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
for b in "$cdir/c_app" "$xdir/cpp_app" ${godir:+"$godir/go_app"}; do
    [ -f "$b" ] || continue
    if strings "$b" 2>/dev/null | grep -q "$REPO_ROOT"; then
        echo "  checkout path ($REPO_ROOT) leaked into $b"; leak=1
    fi
done
if [ "$leak" = 0 ]; then pass "no checkout path baked into binaries"; else bad "checkout path leak"; fi

echo
if [ "$fail" -ne 0 ]; then echo "cleanroom: FAIL"; exit 1; fi
echo "cleanroom: ALL CONSUMERS OK"
