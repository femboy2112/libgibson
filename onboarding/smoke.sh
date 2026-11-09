#!/usr/bin/env bash
# Run copied consumers from a fresh directory against explicitly staged artifacts.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
: "${GIBSON_SDK:?Set GIBSON_SDK to the external staged native SDK prefix}"
: "${GIBSON_GO_MODULE:?Set GIBSON_GO_MODULE to an external copy of bindings/go}"
: "${GIBSON_RUST_CRATE:?Set GIBSON_RUST_CRATE to an externally unpacked libgibson crate}"
python="${PYTHON:-python3}"
for cmd in cc c++ pkg-config go cargo "$python"; do
    command -v "$cmd" >/dev/null || { printf 'missing toolchain: %s\n' "$cmd" >&2; exit 1; }
done
[ -f "$GIBSON_SDK/lib/pkgconfig/libgibson.pc" ] || { echo 'missing SDK or lib/pkgconfig/libgibson.pc' >&2; exit 1; }
[ -f "$GIBSON_SDK/lib/libgibson.so.1" ] || { echo 'missing native shared library in SDK lib/' >&2; exit 1; }
[ -f "$GIBSON_GO_MODULE/go.mod" ] || { echo 'missing staged Go module' >&2; exit 1; }
[ -f "$GIBSON_RUST_CRATE/Cargo.toml" ] || { echo 'missing unpacked Rust crate' >&2; exit 1; }
GIBSON_SDK="$(cd "$GIBSON_SDK" && pwd -P)"
GIBSON_GO_MODULE="$(cd "$GIBSON_GO_MODULE" && pwd -P)"
GIBSON_RUST_CRATE="$(cd "$GIBSON_RUST_CRATE" && pwd -P)"
export PKG_CONFIG_PATH="$GIBSON_SDK/lib/pkgconfig"
export LD_LIBRARY_PATH="$GIBSON_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LIBGIBSON_LIBRARY="$GIBSON_SDK/lib/libgibson.so.1"
pkg-config --exists libgibson || { echo 'pkg-config cannot locate libgibson; check PKG_CONFIG_PATH' >&2; exit 1; }
work="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-onboarding.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cp -R "$here/c" "$here/cpp" "$here/python" "$here/go" "$here/rust" "$work/"
# First isolate loader and ABI failures from example compile/runtime failures.
cat > "$work/abi.c" <<'C'
#include <gibson.h>
#include <stdio.h>
int main(void) {
    unsigned int actual = gibson_abi_version();
    if (actual != GIBSON_ABI_VERSION) {
        fprintf(stderr, "ABI mismatch: header %u, library %u\n", GIBSON_ABI_VERSION, actual);
        return 1;
    }
    return 0;
}
C
if ! "$python" "$here/pkg-config-cc.py" cc "$work/abi.c" -o "$work/abi"; then
    echo 'SDK link failed; check compiler, libgibson.pc and library installation' >&2; exit 1
fi
if ! "$work/abi"; then
    echo 'ABI/loader probe failed; check ABI version and LD_LIBRARY_PATH' >&2; exit 1
fi
if ! (cd "$work/c" && "$python" "$here/pkg-config-cc.py" cc -std=c11 -Wall -Wextra -Werror main.c -o main && ./main); then
    echo 'C example build or execution failed after SDK probe' >&2; exit 1
fi
if ! (cd "$work/cpp" && "$python" "$here/pkg-config-cc.py" c++ -std=c++17 -Wall -Wextra -Werror main.cpp -o main && ./main); then
    echo 'C++ example build or execution failed after SDK probe' >&2; exit 1
fi
if ! (cd "$work/python" && "$python" -I main.py); then
    echo 'Python example failed; install the matching wrapper into the selected Python environment' >&2; exit 1
fi
if ! (cd "$work/go" && go mod edit -replace="github.com/femboy2112/libgibson/bindings/go=$GIBSON_GO_MODULE" && GOPROXY=off GOFLAGS=-mod=mod go run .); then
    echo 'Go example failed; check the staged module, cgo and native toolchain' >&2; exit 1
fi
# Generate only this copied manifest; never modify the supplied crate.
GIBSON_RUST_CRATE="$GIBSON_RUST_CRATE" "$python" - "$work/rust/Cargo.toml" <<'PY'
import json, os, sys
path = os.path.abspath(os.environ['GIBSON_RUST_CRATE'])
with open(sys.argv[1], 'w', encoding='utf8') as f:
    f.write('[package]\nname = "libgibson-onboarding"\nversion = "0.1.0"\nedition = "2021"\n')
    f.write('[dependencies]\nlibgibson = { path = ' + json.dumps(path) + ' }\n')
PY
if ! (cd "$work/rust" && cargo run --quiet); then
    echo 'Rust example failed; check Cargo dependencies and the staged crate' >&2; exit 1
fi
if [ -t 0 ] && [ -t 1 ]; then
    echo 'TTY detected; interactive terminal support was not exercised by this headless smoke.'
else
    echo 'No TTY: headless examples passed; interactive terminal behavior was not exercised.'
fi
