#!/usr/bin/env bash
# Compile tiny real C/C++ clients through the production cleanroom blocks.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-cleanroom-pkgconfig-test.XXXXXXXX")
echo "cleanroom pkg-config fixtures retained: $fixture"
for tool in cc c++ pkg-config python3; do
    command -v "$tool" >/dev/null || { echo "required test tool missing: $tool" >&2; exit 1; }
done
real_pkgconfig=$(command -v pkg-config)
mkdir -p "$fixture/bin" "$fixture/work"
# Extract actual production flags/C/C++ code, retaining compiler/run checks.
awk '/^echo "### SDK compiler flags/{copy=1} /^echo "### C consumer/{copy=1} /^echo "### C installed/{copy=0} copy' \
    "$root/scripts/release/cleanroom.sh" > "$fixture/client-blocks.sh"
awk '/^echo "### C\+\+ consumer/{copy=1} /^echo "### Python consumer/{copy=0} copy' \
    "$root/scripts/release/cleanroom.sh" >> "$fixture/client-blocks.sh"
grep -q 'cc -std=c11' "$fixture/client-blocks.sh"
grep -q 'c++ -std=c++17' "$fixture/client-blocks.sh"
cat > "$fixture/lib.c" <<'C'
unsigned int gibson_abi_version(void) { return 1; }
C
# Explicitly quoted .pc flags also cover literal apostrophes and backslashes.
python3 - "$fixture" <<'PY'
from pathlib import Path
import shlex, sys
root = Path(sys.argv[1])
for name in ('sdk with spaces', "sdk quote's back\\slash"):
    sdk = root / name
    (sdk / 'include').mkdir(parents=True)
    (sdk / 'lib/pkgconfig').mkdir(parents=True)
    (sdk / 'include/gibson.h').write_text('''#define GIBSON_ABI_VERSION 1
#ifdef __cplusplus
extern "C" {
#endif
unsigned int gibson_abi_version(void);
#ifdef __cplusplus
}
#endif
''')
    (sdk / 'include/gibson.hpp').write_text('''#include "gibson.h"
namespace gibson { struct Context {
static unsigned int abi_version() { return gibson_abi_version(); }
static bool abi_compatible() { return abi_version() == GIBSON_ABI_VERSION; }
}; }
''')
    (sdk / 'lib/pkgconfig/libgibson.pc').write_text(
        'Name: libgibson\nDescription: small real ABI fixture\nVersion: 0.7.0\n'
        + 'Cflags: ' + shlex.quote('-I' + str(sdk / 'include')) + '\n'
        + 'Libs: ' + shlex.quote('-L' + str(sdk / 'lib')) + ' -lgibson\n')
PY
for sdk in "$fixture/sdk with spaces" "$fixture/sdk quote's back\\slash"; do
    cc -shared -fPIC "$fixture/lib.c" -o "$sdk/lib/libgibson.so"
done
cat > "$fixture/bin/pkg-config" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
case ${PROBE_PKGCONFIG:-normal} in
    fail) exit 23 ;;
    missing) exit 127 ;;
    malformed) printf "unclosed'\n" ;;
    empty) : ;;
    nul) printf 'invalid\0argument\n' ;;
    shell)
        "$REAL_PKGCONFIG" "$@"
        python3 - "$PROBE_SHELL_MARKER" <<'PY'
import shlex, sys
print(shlex.quote('-DINERT=$(touch ' + sys.argv[1] + ')'))
PY
        ;;
    *) exec "$REAL_PKGCONFIG" "$@" ;;
esac
FAKE
chmod +x "$fixture/bin/pkg-config"
export PATH="$fixture/bin:$PATH" REAL_PKGCONFIG="$real_pkgconfig"
export PROBE_SHELL_MARKER="$fixture/SHOULD_NOT_EXIST"
run_clients() (
    export SDK=$1 WORK="$fixture/work" PKG_CONFIG_PATH="$1/lib/pkgconfig" LD_LIBRARY_PATH="$1/lib"
    fail=0
    pass() { printf 'PASS: %s\n' "$*"; }
    bad() { printf 'FAIL: %s\n' "$*"; fail=1; }
    dump() { cat "$1"; }
    source "$fixture/client-blocks.sh"
    exit "$fail"
)
index=0
for sdk in "$fixture/sdk with spaces" "$fixture/sdk quote's back\\slash"; do
    index=$((index + 1))
    run_clients "$sdk" > "$fixture/native-$index.log" 2>&1 || {
        cat "$fixture/native-$index.log"; echo 'FAIL: correctly quoted SDK rejected' >&2; exit 1;
    }
    grep -q '^PASS: C$' "$fixture/native-$index.log"
    grep -q '^PASS: C++$' "$fixture/native-$index.log"
done
for fault in fail missing malformed empty nul; do
    if PROBE_PKGCONFIG="$fault" run_clients "$fixture/sdk with spaces" > "$fixture/$fault.log" 2>&1; then
        cat "$fixture/$fault.log"; echo "FAIL: $fault pkg-config output passed" >&2; exit 1
    fi
    grep -q '^FAIL: SDK compiler flags' "$fixture/$fault.log" || {
        cat "$fixture/$fault.log"; echo "FAIL: $fault did not reach the flags gate" >&2; exit 1;
    }
done
PROBE_PKGCONFIG=shell run_clients "$fixture/sdk with spaces" > "$fixture/shell.log" 2>&1 || {
    cat "$fixture/shell.log"; echo 'FAIL: literal shell-like compiler flag was corrupted' >&2; exit 1;
}
[[ ! -e "$PROBE_SHELL_MARKER" ]] || { echo 'FAIL: compiler flags were evaluated as shell code' >&2; exit 1; }
echo 'PASS: real C/C++ spaced and quoted prefixes; failed/malformed/empty flags rejected without evaluation'
