#!/usr/bin/env bash
# Real native archive + cgo controls, without building LibGibson or invoking Cargo.
# GO may name a toolchain executable. Its cgo must reject embedded LTO sections
# (Go 1.27.4 / 1.26.11 security behavior); an older permissive tool is not a witness.
set -euo pipefail
export LC_ALL=C
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
GO=${GO:-go}
OBJCOPY=${OBJCOPY:-objcopy}
for tool in cc ar nm objdump readelf rustc pkg-config python3 "$GO" "$OBJCOPY"; do
    command -v -- "$tool" >/dev/null 2>&1 || { echo "native archive controls: missing $tool" >&2; exit 1; }
done
work=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-native-archive.XXXXXXXX")
work=$(cd "$work" && pwd -P)
echo "native archive controls: $work"
cleanup() {
    status=$?
    if [[ $status -eq 0 && ${KEEP_NATIVE_ARCHIVE_FIXTURE:-0} == 0 ]]; then
        python3 - "$work" <<'PY'
from pathlib import Path
import shutil, sys
p = Path(sys.argv[1])
if p.is_symlink() or p.resolve() != p.absolute() or not p.name.startswith("libgibson-native-archive."):
    raise SystemExit("refusing unexpected native archive fixture cleanup")
shutil.rmtree(p)
PY
    else
        echo "native archive fixtures retained: $work" >&2
    fi
}
trap cleanup EXIT
repo="$work/repo"
input="$work/input target"
mkdir -p "$repo/scripts/release" "$repo/include" "$input/release" "$work/bin" "$work/go"
cp "$root/scripts/release/stage-sdk.sh" "$repo/scripts/release/"
printf '[package]\nversion = "0.7.0"\n' > "$repo/Cargo.toml"
printf 'int gibson_archive_probe(void);\n' > "$repo/include/gibson.h"
for header in gibson.hpp termframe.h; do printf fixture > "$repo/include/$header"; done
for document in README.md LICENSE-MIT LICENSE-APACHE LICENSES-THIRD-PARTY.md THIRD-PARTY-NOTICES.md; do
    printf fixture > "$repo/$document"
done
# Only the packaging metadata query is stubbed: the source, archive, object
# transformation, C clients and Go toolchain below are all real.
cat > "$work/bin/cargo" <<'STUB'
#!/bin/sh
[ "$1" = rustc ] || exit 99
printf 'note: native-static-libs: -lc\n' >&2
STUB
chmod +x "$work/bin/cargo"
cat > "$work/probe.rs" <<'RUST'
#![no_std]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! { loop {} }
#[no_mangle]
pub extern "C" fn gibson_archive_probe() -> i32 { 42 }
RUST
rustc --edition=2021 --crate-type staticlib -C panic=abort -C embed-bitcode=yes -C opt-level=1 \
    "$work/probe.rs" -o "$input/release/libgibson.a"
cat > "$work/probe.c" <<'C'
int gibson_archive_probe(void) { return 42; }
C
cc -fPIC -c "$work/probe.c" -o "$work/probe.o"
cc -shared -Wl,-soname,libgibson.so.1 "$work/probe.o" -o "$input/release/libgibson.so"
sha256sum "$input/release/libgibson.a" > "$work/input-before.sha256"
( cd "$input/release" && ar t libgibson.a ) > "$work/members-before"
( cd "$input/release" && nm -g --defined-only libgibson.a ) > "$work/symbols-before"
( cd "$input/release" && objdump -d libgibson.a ) > "$work/code-before"
PATH="$work/bin:$PATH" CARGO_TARGET_DIR="$input" OBJCOPY="$OBJCOPY" \
    "$repo/scripts/release/stage-sdk.sh" --no-build --prefix "$work/sdk prefix" > "$work/stage.log" 2>&1
sdk="$work/sdk prefix"
sha256sum -c "$work/input-before.sha256" >/dev/null
cmp "$input/release/libgibson.so" "$sdk/lib/libgibson.so.1"
( cd "$sdk/lib" && ar t libgibson.a ) > "$work/members-after"
( cd "$sdk/lib" && nm -g --defined-only libgibson.a ) > "$work/symbols-after"
( cd "$sdk/lib" && objdump -d libgibson.a ) > "$work/code-after"
for content in members symbols code; do cmp "$work/$content-before" "$work/$content-after"; done
( cd "$sdk" && sha256sum -c SHA256SUMS ) > "$work/checksums.log"
cat > "$work/client.c" <<'C'
#include <gibson.h>
int main(void) { return gibson_archive_probe() != 42; }
C
cc "$work/client.c" -I"$sdk/include" "$input/release/libgibson.a" -o "$work/static-before"
cc "$work/client.c" -I"$sdk/include" "$sdk/lib/libgibson.a" -o "$work/static-after"
"$work/static-before"
"$work/static-after"

# Build an unstripped sibling SDK for the red control. It has a usable .so, but
# cgo still examines the co-located unselected .a in its -L search directory.
cp -a "$sdk" "$work/original sdk"
cp "$input/release/libgibson.a" "$work/original sdk/lib/libgibson.a"
printf 'module example.invalid/nativearchive\n\ngo 1.18\n' > "$work/go/go.mod"
cat > "$work/go/source" <<'GO'
package main
/*
#cgo pkg-config: libgibson
#include <gibson.h>
*/
import "C"
func main() { if C.gibson_archive_probe() != 42 { panic("wrong native function") } }
GO
"$GO" version
check_go() {
    local label=$1 prefix=$2 expectation=$3
    # Different source bytes force the package containing #cgo through the check;
    # an already cached successful package must not hide an archive mutation.
    cat "$work/go/source" > "$work/go/main.go"
    printf '\n// native archive case: %s\n' "$label" >> "$work/go/main.go"
    if ( cd "$work/go" && env GOTOOLCHAIN=local GOWORK=off GOFLAGS= GOMAXPROCS=2 GOPROXY=off \
        CGO_ENABLED=1 CGO_CFLAGS= CGO_CPPFLAGS= CGO_LDFLAGS= CGO_CFLAGS_ALLOW= CGO_LDFLAGS_ALLOW= \
        GOCACHE="$work/go-cache" GOMODCACHE="$work/go-modules" \
        PKG_CONFIG_PATH="$prefix/lib/pkgconfig" LD_LIBRARY_PATH="$prefix/lib" \
        "$GO" build -p=1 -o "$work/$label" . ) > "$work/$label.log" 2>&1; then
        [[ $expectation == pass ]] || { echo "FAIL: $label accepted forbidden compiler IR (toolchain too old?)" >&2; return 1; }
        LD_LIBRARY_PATH="$prefix/lib" "$work/$label"
        readelf -d "$work/$label" > "$work/$label.dynamic"
        grep -Fq 'Shared library: [libgibson.so.1]' "$work/$label.dynamic" || {
            echo 'FAIL: Go control did not select the shared library' >&2; return 1;
        }
    else
        [[ $expectation == fail ]] || { cat "$work/$label.log" >&2; return 1; }
        grep -Fq 'disallowed LTO section ".llvmbc"' "$work/$label.log" || {
            cat "$work/$label.log" >&2; echo 'FAIL: unrelated Go failure is not a valid IR rejection' >&2; return 1;
        }
    fi
}
check_go original "$work/original sdk" fail
check_go packaged "$sdk" pass
printf 'synthetic forbidden compiler IR' > "$work/ir.bin"
"$OBJCOPY" --add-section=.llvmbc="$work/ir.bin" "$work/probe.o" "$work/injected.o"
ar q "$sdk/lib/libgibson.a" "$work/injected.o"
check_go injected "$sdk" fail
echo 'PASS: native members/symbols/code and static clients unchanged; input preserved; cgo rejects original/injected IR and accepts packaged shared SDK'
