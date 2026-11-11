#!/usr/bin/env bash
# Exercise the real cleanroom Go block with a fake Go tool; no compiler needed.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-cleanroom-go-test.XXXXXXXX")
echo "cleanroom Go fixtures retained: $fixture"
mkdir -p "$fixture/bin" "$fixture/source/bindings/go" "$fixture/work"
for name in LICENSE LICENSE-MIT LICENSE-APACHE; do
    printf 'fixture\n' > "$fixture/source/bindings/go/$name"
done
cat > "$fixture/bin/go" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
case "$1 $2" in
    'mod init') printf 'module fixture\n' > go.mod ;;
    'mod edit') : ;;
    'build -o')
        [[ ${FAKE_GO_BUILD_FAIL:-0} == 0 ]] || exit 7
        printf '#!/bin/sh\nexit 0\n' > "$3"
        chmod +x "$3"
        ;;
    *) echo "unexpected fake Go command: $*" >&2; exit 2 ;;
esac
FAKE
chmod +x "$fixture/bin/go"
awk '/^echo "### Go consumer/{copy=1} /^echo "### Rust consumer/{copy=0} copy' \
    "$root/scripts/release/cleanroom.sh" > "$fixture/go-block.sh"
[[ -s "$fixture/go-block.sh" ]] || { echo 'FAIL: Go block missing' >&2; exit 1; }
run_block() (
    export PATH="$fixture/bin:$PATH"
    export WORK="$fixture/work" REPO_ROOT="$fixture/source"
    cd "$fixture/source"
    fail=0
    command() {
        if [[ $1 == -v && $2 == go && ${FAKE_GO_MISSING:-0} == 1 ]]; then return 1; fi
        builtin command "$@"
    }
    pass() { echo "PASS: $*"; }
    bad() { echo "FAIL: $*"; fail=1; }
    dump() { cat "$1"; }
    source "$fixture/go-block.sh"
    exit "$fail"
)
run_block > "$fixture/present.log" 2>&1 || { cat "$fixture/present.log"; echo 'FAIL: present Go rejected' >&2; exit 1; }
grep -q 'PASS: Go (built from staged copy' "$fixture/present.log" || { cat "$fixture/present.log"; echo 'FAIL: Go consumer did not execute' >&2; exit 1; }
if FAKE_GO_MISSING=1 run_block > "$fixture/missing.log" 2>&1; then
    cat "$fixture/missing.log"; echo 'FAIL: missing Go passed' >&2; exit 1
fi
if FAKE_GO_BUILD_FAIL=1 run_block > "$fixture/build-fail.log" 2>&1; then
    cat "$fixture/build-fail.log"; echo 'FAIL: failed Go build passed' >&2; exit 1
fi
echo 'PASS: present Go consumer executed; missing Go and failing Go build rejected'
