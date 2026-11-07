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
# Surface the cause of a failed consumer: this script's steps previously sent all build/run output
# to /dev/null, so a red run reported WHICH consumer failed but never WHY — the exact gap that left
# the release-preflight cleanroom failure "cause unknown". Dumped ONLY on failure. The cause can sit
# far above the end of a long build log, so surface the failure-signature lines from the WHOLE log
# first, then a bounded tail for context — both capped so a long compile can't bury the summary.
dump() {
    local sig
    sig="$(grep -nE 'error(\[|:)|FAILED|panicked|assertion|\[FAIL\]|cannot find|unresolved|undefined (reference|symbol)|mismatch' "$1" | head -n 30)"
    [ -n "$sig" ] && printf '%s\n' "$sig" | sed 's/^/    !| /'
    tail -n 40 "$1" | sed 's/^/    | /'
}

# D6-3 license-payload checkers for the ACTUAL built Python artifacts.
# The wheel checker accepts BOTH the legacy flat dist-info layout and the PEP 639 /
# Metadata-2.4 `licenses/` layout that setuptools >= 77 emits, and verifies the
# license BYTES match the repository text (see check-wheel-license.py). It lives in
# its own script so the three-wheel red-first fixture can exercise it in isolation
# (scripts/release/tests/test-wheel-license.sh).
py_wheel_license_ok() {
    python3 "$REPO_ROOT/scripts/release/check-wheel-license.py" "$1" "$REPO_ROOT/bindings/python"
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
slog="$(mktemp)"
if scripts/release/stage-sdk.sh --prefix "$SDK" --force >"$slog" 2>&1; then pass "SDK staged"; else bad "SDK staging"; dump "$slog"; fi
rm -f "$slog"
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
clog="$(mktemp)"
if ( cd "$cdir" && cc -std=c11 main.c $(pkg-config --cflags --libs libgibson) -o c_app && ./c_app >/dev/null ) >"$clog" 2>&1; then pass "C"; else bad "C"; dump "$clog"; fi
rm -f "$clog"

echo "### C installed-SDK forcing consumer (bulk readback loop, pkg-config) ###"
# RXIV Gate G1: not just "it links" — the Doom-shaped forcing consumer drives the readback ABI from
# the staged prefix (240-frame bulk readback, bulk == single-cell, mid-session depth restyle,
# long-glyph recovery, ANSI output) and self-checks each invariant (nonzero exit on violation).
fdir="$WORK/forcing"; mkdir -p "$fdir"
cp bindings/sdk_forcing/doom_forcing_consumer.c "$fdir/main.c"
flog="$(mktemp)"
if ( cd "$fdir" && cc -std=c11 -Wall -Wextra main.c $(pkg-config --cflags --libs libgibson) -o forcing_app && ./forcing_app | grep -q '^doom_forcing_consumer: OK' ) >"$flog" 2>&1; then pass "C forcing consumer"; else bad "C forcing consumer"; dump "$flog"; fi
rm -f "$flog"

echo "### C installed-SDK headless input boundary (#259, pkg-config) ###"
# A headless handle is output-only: gibson_poll_event must return GIBSON_OK + GIBSON_EVENT_NONE at
# once, without opening the process terminal. Run with stdin from /dev/null; before the fix the
# poll tried to open a terminal and returned GIBSON_ERR_IO.
hdir="$WORK/headless_input"; mkdir -p "$hdir"
cp bindings/sdk_forcing/headless_input_consumer.c "$hdir/main.c"
hlog="$(mktemp)"
if ( cd "$hdir" && cc -std=c11 -Wall -Wextra main.c $(pkg-config --cflags --libs libgibson) -o headless_input_app && ./headless_input_app </dev/null | grep -q '^headless_input_consumer: OK' ) >"$hlog" 2>&1; then pass "C headless input boundary"; else bad "C headless input boundary"; dump "$hlog"; fi
rm -f "$hlog"

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
xlog="$(mktemp)"
if ( cd "$xdir" && c++ -std=c++17 main.cpp $(pkg-config --cflags --libs libgibson) -o cpp_app && ./cpp_app >/dev/null ) >"$xlog" 2>&1; then pass "C++"; else bad "C++"; dump "$xlog"; fi
rm -f "$xlog"

echo "### Python consumer (wheel + sdist -> fresh venv) ###"
if command -v python3 >/dev/null; then
    # Build both artifacts from a COPY so the source tree is never littered.
    pysrc="$WORK/pysrc"; cp -a bindings/python "$pysrc"
    rm -rf "$pysrc/build" "$pysrc/dist" "$pysrc"/*.egg-info "$pysrc/__pycache__"
    vr="$WORK/vr"; dist="$WORK/dist"; sdout="$WORK/sdist"; mkdir -p "$dist" "$sdout"
    ok=1
    plog="$(mktemp)"
    ( cd "$pysrc" && python3 -c "from setuptools import build_meta as b; b.build_wheel('$dist')" ) >>"$plog" 2>&1 || ok=0
    ( cd "$pysrc" && python3 -c "from setuptools import build_meta as b; b.build_sdist('$sdout')" ) >>"$plog" 2>&1 || ok=0
    whl="$(ls "$dist"/*.whl 2>/dev/null | head -1)"
    sdist="$(ls "$sdout"/*.tar.gz 2>/dev/null | head -1)"
    # D6-3: the ACTUAL built artifacts must physically carry the dual-license text.
    if [ -n "$whl" ] && py_wheel_license_ok "$whl"; then pass "Python wheel carries license text + metadata"; else bad "Python wheel license payload"; fi
    if [ -n "$sdist" ] && py_sdist_license_ok "$sdist"; then pass "Python sdist carries license text"; else bad "Python sdist license payload"; fi
    # Install the wheel into a fresh venv and exercise it against the staged .so.
    python3 -m venv "$vr" >>"$plog" 2>&1 || ok=0
    [ -n "$whl" ] && "$vr/bin/pip" install -q "$whl" >>"$plog" 2>&1 || ok=0
    pyroom="$WORK/pyroom"; mkdir -p "$pyroom"
    if [ "$ok" = 1 ] && ( cd "$pyroom" && PYTHONPATH= LIBGIBSON_LIBRARY="$SDK/lib/libgibson.so" "$vr/bin/python" -c '
import gibson
assert "site-packages" in gibson.__file__, "not from installed wheel"
assert gibson._loaded_abi_version == gibson.EXPECTED_ABI_VERSION
gibson.Node.text("x"); gibson.Line().add_span("y")
' ) >>"$plog" 2>&1; then pass "Python"; else bad "Python"; dump "$plog"; fi
    rm -f "$plog"
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
    glog="$(mktemp)"
    ( cd "$godir" \
        && GOPROXY=off GOFLAGS=-mod=mod go mod init cleanroomconsumer \
        && go mod edit -require=github.com/femboy2112/libgibson/bindings/go@v0.0.0 \
        && go mod edit -replace=github.com/femboy2112/libgibson/bindings/go="$gomod" \
        && GOPROXY=off GOFLAGS=-mod=mod go build -o go_app . \
        && ./go_app ) >"$glog" 2>&1 || gook=0
    # The consumer's go.mod must reference the staged copy, never the checkout.
    if [ "$gook" = 1 ] && ! grep -q "$REPO_ROOT/bindings/go" "$godir/go.mod"; then
        pass "Go (built from staged copy; no checkout path in go.mod)"
    else
        [ "$gook" = 1 ] && grep -q "$REPO_ROOT/bindings/go" "$godir/go.mod" && echo "  checkout path leaked into go.mod"
        [ "$gook" = 0 ] && dump "$glog"
        bad "Go"
    fi
    rm -f "$glog"
else
    echo "  [SKIP] go not installed"
fi

echo "### Rust consumer (packaged crate) ###"
rpkg="$(mktemp)"
if $CARGO package --allow-dirty >"$rpkg" 2>&1; then
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
    rrun="$(mktemp)"
    if ( cd "$rdir" && $CARGO run -q >/dev/null 2>&1 ) >"$rrun" 2>&1; then pass "Rust"; else bad "Rust"; dump "$rrun"; fi
    rm -f "$rrun"
else
    bad "Rust (cargo package failed)"
    dump "$rpkg"
fi
rm -f "$rpkg"

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
