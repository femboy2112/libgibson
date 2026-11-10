#!/usr/bin/env bash
# Isolated no-compiler controls for explicit-library and candidate ABI checks.
set -euo pipefail
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
WORK=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-abi-test.XXXXXXXX")
echo "ABI checker test fixtures: $WORK"
mkdir -p "$WORK/repo/scripts/release" "$WORK/repo/abi" "$WORK/repo/target/release" "$WORK/bin"
cp "$ROOT/scripts/release/check-abi.sh" "$WORK/repo/scripts/release/"
printf '%s\n' gibson_required > "$WORK/repo/abi/gibson-abi-v1.symbols"
printf '%s\n' gibson_required > "$WORK/repo/target/release/libgibson.so"
printf '%s\n' gibson_other > "$WORK/wrong.so"
cat > "$WORK/bin/nm" <<'EOF'
#!/usr/bin/env bash
set -e
[[ ${FAKE_NM_FAIL:-0} == 0 ]] || exit 12
for arg in "$@"; do path=$arg; done
awk '{ print "000000 T " $0 }' "$path"
EOF
chmod +x "$WORK/bin/nm"
export PATH="$WORK/bin:$PATH"
checker="$WORK/repo/scripts/release/check-abi.sh"

"$checker"  # Backward-compatible local auto-selection.
"$checker" --so "$WORK/repo/target/release/libgibson.so"
( cd "$WORK" && "$checker" --so repo/target/release/libgibson.so )
if "$checker" --bad-option; then
    echo 'FAIL: unknown option passed' >&2; exit 1
fi
if "$checker" --so "$WORK/wrong.so"; then
    echo 'FAIL: explicit bad library was ignored in favor of stale release' >&2; exit 1
fi
if "$checker" --so "$WORK/missing.so"; then
    echo 'FAIL: missing explicit library passed' >&2; exit 1
fi
if FAKE_NM_FAIL=1 "$checker" --so "$WORK/repo/target/release/libgibson.so"; then
    echo 'FAIL: nm failure passed' >&2; exit 1
fi

cp "$ROOT/scripts/release/check-candidate-abi.sh" "$WORK/repo/scripts/release/"
printf '%s\n' committed > "$WORK/repo/source-marker"
git -C "$WORK/repo" init -q
git -C "$WORK/repo" -c user.name='ABI fixture' -c user.email='abi-fixture@example.invalid' \
    add scripts/release/check-abi.sh scripts/release/check-candidate-abi.sh abi/gibson-abi-v1.symbols source-marker
git -C "$WORK/repo" -c user.name='ABI fixture' -c user.email='abi-fixture@example.invalid' \
    -c commit.gpgsign=false -c core.hooksPath=/dev/null commit -qm 'fixture candidate'
printf '%s\n' dirty > "$WORK/repo/source-marker"
cat > "$WORK/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ ${FAKE_CARGO_FAIL:-0} == 0 ]] || exit 13
[[ $(cat source-marker) == committed ]] || { echo 'not built from committed source' >&2; exit 14; }
[[ ${FAKE_NO_OUTPUT:-0} == 0 ]] || exit 0
target=''
while [[ $# -gt 0 ]]; do
    if [[ $1 == --target-dir ]]; then shift; target=$1; fi
    shift
done
[[ -n $target && ! -e "$target/release/libgibson.so" ]]
mkdir -p "$target/release"
printf '%s\n' "${FAKE_ABI_SYMBOL:-gibson_required}" > "$target/release/libgibson.so"
EOF
chmod +x "$WORK/bin/cargo"
candidate="$WORK/repo/scripts/release/check-candidate-abi.sh"
"$candidate"
if FAKE_ABI_SYMBOL=gibson_other "$candidate"; then
    echo 'FAIL: stale good local .so masked bad candidate .so' >&2; exit 1
fi
if FAKE_CARGO_FAIL=1 "$candidate"; then
    echo 'FAIL: candidate build failure passed' >&2; exit 1
fi
if FAKE_NO_OUTPUT=1 "$candidate"; then
    echo 'FAIL: missing candidate library passed' >&2; exit 1
fi

echo 'PASS: explicit library, clean committed-source build, stale, missing, build and nm controls'
