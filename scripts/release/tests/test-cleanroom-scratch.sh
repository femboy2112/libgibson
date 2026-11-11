#!/usr/bin/env bash
# Exercise the real cleanroom setup/cleanup with intercepted deletion; never compile.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-cleanroom-scratch-test.XXXXXXXX")
echo "cleanroom scratch fixtures retained: $fixture"
mkdir -p "$fixture/repo/scripts/release" "$fixture/bin" "$fixture/allocated" "$fixture/other"
printf '[package]\nversion = "0.7.0"\n' > "$fixture/repo/Cargo.toml"
# This is the production initialization and EXIT trap, ending before any compiler.
awk '/^echo "### stage native SDK/{exit} {print}' \
    "$root/scripts/release/cleanroom.sh" > "$fixture/repo/scripts/release/setup.sh"
grep -q '^trap cleanup EXIT$' "$fixture/repo/scripts/release/setup.sh"
cat >> "$fixture/repo/scripts/release/setup.sh" <<'PROBE'
printf '%s\n' "$WORK" > "$PROBE_WORK_LOG"
if [[ ${PROBE_REPLACE_WITH_SYMLINK:-0} == 1 ]]; then
    rmdir -- "$WORK"
    ln -s "$PROBE_OTHER" "$WORK"
fi
PROBE
cat > "$fixture/bin/mktemp" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
case "$PROBE_MKTEMP" in
    fail) exit 19 ;;
    empty) exit 0 ;;
    missing) printf '%s\n' "$PROBE_ALLOCATED/missing" ;;
    repo) printf '%s\n' "$PROBE_REPO" ;;
    symlink) printf '%s\n' "$PROBE_SYMLINK" ;;
    normal) printf '%s\n' "$PROBE_ALLOCATED" ;;
    *) exit 20 ;;
esac
FAKE
# All destructive attempts are captured, including on the vulnerable baseline.
cat > "$fixture/bin/rm" <<'FAKE'
#!/usr/bin/env bash
printf '%s\0' "$@" >> "$PROBE_RM_LOG"
FAKE
chmod +x "$fixture/bin/mktemp" "$fixture/bin/rm"
ln -s "$fixture/other" "$fixture/allocated-link"
export PATH="$fixture/bin:$PATH" PROBE_ALLOCATED="$fixture/allocated"
export PROBE_REPO="$fixture/repo" PROBE_OTHER="$fixture/other" PROBE_SYMLINK="$fixture/allocated-link"
# Fault only canonicalization of the allocated directory, not source discovery.
pwd() {
    if [[ "$PWD" == "$PROBE_ALLOCATED" ]]; then
        case ${PROBE_CANONICAL:-normal} in
            fail) return 21 ;;
            empty) return 0 ;;
            other) printf '%s\n' "$PROBE_OTHER"; return 0 ;;
        esac
    fi
    builtin pwd "$@"
}
cd() {
    if [[ ${PROBE_CANONICAL:-normal} == cd && ${*: -1} == "$PROBE_ALLOCATED" ]]; then
        return 22
    fi
    builtin cd "$@"
}
export -f pwd cd
run_probe() {
    local name=$1
    export PROBE_WORK_LOG="$fixture/$name.work" PROBE_RM_LOG="$fixture/$name.rm"
    bash "$fixture/repo/scripts/release/setup.sh" > "$fixture/$name.log" 2>&1
}
for fault in fail empty missing repo symlink; do
    if PROBE_MKTEMP="$fault" run_probe "$fault"; then
        echo "FAIL: $fault scratch allocation was accepted" >&2; exit 1
    fi
    [[ ! -e "$fixture/$fault.rm" ]] || {
        echo "FAIL: $fault scratch allocation attempted cleanup" >&2; exit 1;
    }
    [[ ! -e "$fixture/$fault.work" ]] || {
        echo "FAIL: $fault scratch allocation reached the work stage" >&2; exit 1;
    }
done
for fault in fail empty other cd; do
    if PROBE_MKTEMP=normal PROBE_CANONICAL="$fault" run_probe "canonical-$fault"; then
        echo "FAIL: $fault scratch canonicalization was accepted" >&2; exit 1
    fi
    [[ ! -e "$fixture/canonical-$fault.rm" && ! -e "$fixture/canonical-$fault.work" ]] || {
        echo "FAIL: $fault scratch canonicalization reached work or cleanup" >&2; exit 1;
    }
done
PROBE_MKTEMP=normal run_probe normal || { cat "$fixture/normal.log"; exit 1; }
[[ $(cat "$fixture/normal.work") == "$fixture/allocated" ]]
python3 - "$fixture/normal.rm" "$fixture/allocated" <<'PY'
from pathlib import Path
import os, sys
actual = Path(sys.argv[1]).read_bytes().split(b'\0')[:-1]
assert actual == [b'-rf', b'--', os.fsencode(sys.argv[2])], actual
PY
PROBE_MKTEMP=normal PROBE_REPLACE_WITH_SYMLINK=1 run_probe replaced || {
    cat "$fixture/replaced.log"; exit 1;
}
[[ ! -e "$fixture/replaced.rm" ]] || { echo 'FAIL: cleanup followed a replaced scratch symlink' >&2; exit 1; }
[[ -d "$fixture/other" && -f "$fixture/repo/Cargo.toml" ]]
echo 'PASS: failed/empty/invalid allocation rejected; cleanup limited to verified scratch'
