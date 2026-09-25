#!/usr/bin/env bash
#
# msrv-consumer.sh — the DECLARED-RANGE consumer MSRV experiment.
#
# The MSRV job elsewhere proves our COMMITTED Cargo.lock builds on the floor
# (`cargo +1.85 check --locked --lib`) — that is the *resolved-release* MSRV.
# But a library does not hand consumers its lockfile. A fresh crates.io consumer
# resolves our *declared dependency ranges* from scratch, and could land on newer
# dependency versions whose own rust-version exceeds our floor.
#
# This experiment builds an external consumer of the PACKAGED crate with NO
# inherited lockfile and a FRESH resolution on the MSRV toolchain, and reports
# whether the declared ranges resolve to a graph that builds on the floor:
#
#   - MSRV-aware resolver ("fallback", stable since Rust 1.84): what a consumer
#     using rust-version-aware resolution sees — a compatible floor if one exists.
#   - default resolver ("allow"): what a plain `edition = "2021"` consumer sees by
#     default (newest in range), reported as additional evidence.
#
# The gate PASSES iff the MSRV-aware resolution yields a floor-buildable graph
# (i.e. a sound declared-range MSRV exists). It additionally reports whether the
# naive default resolution also builds on the floor.
#
# Env:
#   MSRV_TOOLCHAIN   default "+1.85"
#   CARGO            default "cargo"

set -uo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
CARGO="${CARGO:-cargo}"
MSRV="${MSRV_TOOLCHAIN:-+1.85}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-msrv.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
export CARGO_TARGET_DIR="$WORK/target"

echo "msrv-consumer: declared-range fresh-resolution experiment (toolchain $MSRV, crate $VERSION)"

# Package the crate and unpack it, so the consumer depends on exactly the payload
# a crates.io consumer would receive (declared ranges, no lockfile).
# --no-verify: we only need the produced payload; the consumer builds below ARE
# the verification (and on the MSRV toolchain, which is the point of the test).
# NOTE: CARGO_TARGET_DIR is redirected to $WORK, so `cargo package` writes the
# .crate under $CARGO_TARGET_DIR/package — read it from THERE, not the repo tree
# (a stale repo-tree .crate must never mask a clean run).
if ! $CARGO package --no-verify --allow-dirty >/dev/null 2>&1; then
    echo "  [FAIL] cargo package"; exit 1
fi
CRATE="$CARGO_TARGET_DIR/package/libgibson-$VERSION.crate"
if [ ! -f "$CRATE" ]; then
    echo "  [FAIL] packaged crate not found at $CRATE"; exit 1
fi
tar xzf "$CRATE" -C "$WORK"
CRATE_DIR="$WORK/libgibson-$VERSION"
[ -d "$CRATE_DIR" ] || { echo "  [FAIL] crate payload did not unpack to $CRATE_DIR"; exit 1; }

mkcons() {
    local dir="$1"; mkdir -p "$dir/src"
    cat > "$dir/Cargo.toml" <<EOF
[package]
name = "msrv-declared-range-consumer"
version = "0.0.0"
edition = "2021"
rust-version = "1.85"

[dependencies]
libgibson = { path = "$CRATE_DIR" }
EOF
    cat > "$dir/src/lib.rs" <<'EOF'
// Touch the CORE surface so the whole declared-range graph must build.
pub fn touch() {
    let _ = std::mem::size_of::<gibson::cell::Cell>();
    let _ = std::mem::size_of::<gibson::surface::Surface>();
}
EOF
}

run_experiment() { # name [extra cargo pre-subcommand args...]
    local name="$1"; shift
    local dir="$WORK/$name"; mkcons "$dir"
    ( cd "$dir" \
        && rm -f Cargo.lock \
        && $CARGO $MSRV "$@" generate-lockfile >/dev/null 2>&1 \
        && $CARGO $MSRV "$@" check --lib >/dev/null 2>&1 )
}

# 1) MSRV-aware ("fallback") resolution — the sound-floor answer.
if run_experiment fallback --config 'resolver.incompatible-rust-versions="fallback"'; then
    fb=OK; echo "  [PASS] MSRV-aware (fallback) resolution of declared ranges builds on $MSRV"
else
    fb=FAIL; echo "  [FAIL] MSRV-aware (fallback) resolution of declared ranges does NOT build on $MSRV"
fi

# 2) Default ("allow") resolution — what a naive fresh consumer sees by default.
if run_experiment allow; then
    echo "  [INFO] default (allow) resolution ALSO builds on $MSRV — floor is clean without MSRV-aware resolution"
else
    echo "  [INFO] default (allow) resolution does NOT build on $MSRV — a naive fresh consumer must use the MSRV-aware resolver (Rust >= 1.84) or pin the offending dependency"
fi

echo
if [ "$fb" = OK ]; then
    echo "msrv-consumer: PASS — declared dependency ranges have a sound $MSRV floor"
    exit 0
fi
echo "msrv-consumer: FAIL — no $MSRV-compatible resolution of the declared ranges"
exit 1
