#!/usr/bin/env bash
# No-compiler staging provenance controls. Fixtures are retained for audited cleanup.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-stage-test.XXXXXXXX")
echo "SDK staging fixtures retained: $work"
mkdir -p "$work/repo/scripts/release" "$work/repo/include" "$work/repo/target/release" "$work/repo/src" "$work/bin" "$work/custom target"
cp "$root/scripts/release/stage-sdk.sh" "$work/repo/scripts/release/"
cp "$root/scripts/release/cargo-artifact-paths.py" "$work/repo/scripts/release/"
printf '[package]\nversion = "0.7.0"\n' > "$work/repo/Cargo.toml"
for f in gibson.h gibson.hpp termframe.h; do printf header > "$work/repo/include/$f"; done
for f in README.md LICENSE-MIT LICENSE-APACHE LICENSES-THIRD-PARTY.md; do printf document > "$work/repo/$f"; done
printf stale > "$work/repo/target/release/libgibson.so"
printf stale > "$work/repo/target/release/libgibson.a"
cat > "$work/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ $1 == rustc ]]; then
    case ${FAKE_MUTATE_SELECTED:-none} in
        so) printf mutated > "$CARGO_TARGET_DIR/release/libgibson.so" ;;
        a) printf mutated > "$CARGO_TARGET_DIR/release/libgibson.a" ;;
    esac
    echo 'note: native-static-libs: -lc' >&2; exit 0
fi
[[ $1 == build ]]
target=$CARGO_TARGET_DIR
if [[ ${FAKE_TRIPLE:-0} == 1 ]]; then target="$target/fake-triple"; fi
mkdir -p "$target/release"
printf fresh > "$target/release/libgibson.so"
printf fresh > "$target/release/libgibson.a"
if [[ ${FAKE_MISSING:-0} == 1 ]]; then rm "$target/release/libgibson.so"; fi
if [[ ${FAKE_BADJSON:-0} == 1 ]]; then echo not-json; exit 0; fi
if [[ ${FAKE_DUPLICATE:-0} == 1 ]]; then
    mkdir -p "$target/duplicate"; printf wrong > "$target/duplicate/libgibson.so"
fi
python3 - "$target" <<'PY'
import json,sys
import os
t=os.path.abspath(sys.argv[1])
files=[t+'/release/libgibson.so',t+'/release/libgibson.a']
if os.environ.get('FAKE_DUPLICATE') == '1': files.append(t+'/duplicate/libgibson.so')
print(json.dumps({'reason':'compiler-artifact','target':{'name':'gibson','kind':['rlib','cdylib','staticlib'],'src_path':os.getcwd()+'/src/lib.rs'},'filenames':files}))
PY
EOF
chmod +x "$work/bin/cargo"
export PATH="$work/bin:$PATH" CARGO_TARGET_DIR="$work/custom target"
"$work/repo/scripts/release/stage-sdk.sh" --prefix "$work/sdk" >/dev/null
[[ $(cat "$work/sdk/lib/libgibson.so.1") == fresh ]] || { echo 'FAIL: staged stale shared library' >&2; exit 1; }
[[ $(cat "$work/sdk/lib/libgibson.a") == fresh ]] || { echo 'FAIL: staged stale static library' >&2; exit 1; }
for changed in so a; do
    FAKE_MUTATE_SELECTED="$changed" "$work/repo/scripts/release/stage-sdk.sh" --prefix "$work/mutated-$changed" >/dev/null || {
        echo "FAIL: staging rejected mutation of the original $changed artifact" >&2; exit 1;
    }
    [[ $(cat "$CARGO_TARGET_DIR/release/libgibson.$changed") == mutated ]] || {
        echo "FAIL: $changed mutation control did not run" >&2; exit 1;
    }
    [[ $(cat "$work/mutated-$changed/lib/libgibson.so.1") == fresh ]] || {
        echo 'FAIL: changed shared library reached staged SDK' >&2; exit 1;
    }
    [[ $(cat "$work/mutated-$changed/lib/libgibson.a") == fresh ]] || {
        echo 'FAIL: changed static library reached staged SDK' >&2; exit 1;
    }
done
if FAKE_MISSING=1 "$work/repo/scripts/release/stage-sdk.sh" --prefix "$work/missing" >/dev/null; then
    echo 'FAIL: missing fresh artifact fell back to stale library' >&2; exit 1
fi
for fault in FAKE_BADJSON FAKE_DUPLICATE; do
    if env "$fault=1" "$work/repo/scripts/release/stage-sdk.sh" --prefix "$work/$fault" >/dev/null; then
        echo "FAIL: $fault was accepted" >&2; exit 1
    fi
done
FAKE_TRIPLE=1 "$work/repo/scripts/release/stage-sdk.sh" --prefix "$work/triple" >/dev/null
[[ $(cat "$work/triple/lib/libgibson.so.1") == fresh ]]
printf fresh > "$CARGO_TARGET_DIR/release/libgibson.so"
"$work/repo/scripts/release/stage-sdk.sh" --no-build --prefix "$work/prebuilt" >/dev/null
[[ $(cat "$work/prebuilt/lib/libgibson.so.1") == fresh ]]
echo 'PASS: staged selected cargo artifacts and rejected missing fresh output'
