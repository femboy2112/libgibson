#!/usr/bin/env bash
# Exercise cleanroom's real Rust-consumer block without compiling a crate.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-cleanroom-test.XXXXXXXX")
echo "cleanroom package fixtures retained: $fixture"
repo="$fixture/repo"
mkdir -p "$repo/target/package" "$repo/scripts/release" "$fixture/fresh/libgibson-0.7.0" \
    "$fixture/stale/libgibson-0.7.0" "$fixture/installed" "$fixture/bin" "$fixture/foreign target"
cp "$root/scripts/release/cleanroom.sh" "$repo/scripts/release/"
if [[ -f "$root/scripts/release/package-cleanroom-crate.sh" ]]; then
    cp "$root/scripts/release/package-cleanroom-crate.sh" "$repo/scripts/release/"
fi
printf 'fresh\n' > "$fixture/fresh/libgibson-0.7.0/marker"
printf 'stale\n' > "$fixture/stale/libgibson-0.7.0/marker"
printf '[package]\nname="libgibson"\nversion="0.7.0"\n' > "$fixture/fresh/libgibson-0.7.0/Cargo.toml"
cp "$fixture/fresh/libgibson-0.7.0/Cargo.toml" "$fixture/stale/libgibson-0.7.0/Cargo.toml"
tar czf "$fixture/fresh.crate" -C "$fixture/fresh" libgibson-0.7.0
tar czf "$repo/target/package/libgibson-0.7.0.crate" -C "$fixture/stale" libgibson-0.7.0
cat > "$fixture/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "$1" in
    package)
        mkdir -p "$CARGO_TARGET_DIR/package"
        if [[ ${FAKE_CORRUPT:-0} == 1 ]]; then
            printf corrupt > "$CARGO_TARGET_DIR/package/libgibson-0.7.0.crate"
        elif [[ ${FAKE_NO_PACKAGE:-0} != 1 ]]; then
            cp "$FRESH_CRATE" "$CARGO_TARGET_DIR/package/libgibson-0.7.0.crate"
        fi
        ;;
    run) ;;
    *) exit 2 ;;
esac
EOF
chmod +x "$fixture/bin/cargo"
awk '/^echo "### Rust consumer/{copy=1} /^echo "### source-tree/{copy=0} copy' \
    "$repo/scripts/release/cleanroom.sh" > "$fixture/rust-block.sh"
export CARGO_TARGET_DIR="$fixture/foreign target" FRESH_CRATE="$fixture/fresh.crate"
export CARGO="$fixture/bin/cargo" VERSION=0.7.0 WORK="$fixture/installed" REPO_ROOT="$repo"
cd "$repo"
pass() { echo "PASS: $*"; }
bad() { echo "FAIL: $*"; fail=1; }
dump() { cat "$1"; }
fail=0
source "$fixture/rust-block.sh"
[[ $fail == 0 ]] || { echo 'FAIL: cleanroom Rust block failed' >&2; exit 1; }
[[ $(cat "$WORK/libgibson-0.7.0/marker") == fresh ]] || {
    echo 'FAIL: cleanroom accepted stale repo-local crate' >&2; exit 1;
}
for fault in FAKE_NO_PACKAGE FAKE_CORRUPT; do
    mkdir "$fixture/$fault-dest"
    if env "$fault=1" CARGO_TARGET_DIR="$fixture/$fault-target" \
        "$repo/scripts/release/package-cleanroom-crate.sh" "$fixture/$fault-dest" "$VERSION"; then
        echo "FAIL: $fault was accepted" >&2; exit 1
    fi
done
if CARGO_TARGET_DIR="$WORK/package-target" "$repo/scripts/release/package-cleanroom-crate.sh" "$WORK" "$VERSION"; then
    echo 'FAIL: existing target or extracted crate was accepted' >&2; exit 1
fi
echo 'PASS: cleanroom used the package created by the current Cargo invocation'
