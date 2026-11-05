#!/usr/bin/env bash
# sdk_forcing_consumer.sh — RXIII Checkpoint C / Gate E.
#
# Build the LibGibson native SDK, STAGE it into a prefix OUTSIDE the checkout, then compile and run
# the Doom-shaped forcing consumer (bindings/sdk_forcing/doom_forcing_consumer.c) against ONLY that
# staged prefix via pkg-config — no repo include/lib paths. This is the Gate E evidence that a third
# party who INSTALLED the SDK can drive the heavyweight Gate I/J/K readback ABI, not merely that the
# repo-relative parity harnesses link.
#
# Usage: scripts/dev/sdk_forcing_consumer.sh
#   The library is pre-built single-lane (CARGO_BUILD_JOBS=1) so a warm worktree on a memory-tight
#   box does not OOM on a wide release build; stage-sdk then runs with --no-build.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-forcing-sdk.XXXXXX")"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-forcing-app.XXXXXX")"
cleanup() { rm -rf "$STAGE" "$WORK"; }
trap cleanup EXIT

echo "== 1. build the shipped library (cdylib + staticlib), single lane =="
CARGO_BUILD_JOBS=1 cargo build --release --lib

echo "== 2. stage the SDK into a prefix OUTSIDE the checkout: $STAGE =="
scripts/release/stage-sdk.sh --prefix "$STAGE" --force --no-build

echo "== 3. copy ONLY the consumer source out of the checkout (must build with no repo paths) =="
cp bindings/sdk_forcing/doom_forcing_consumer.c "$WORK/main.c"

echo "== 4. compile + link against the staged prefix via pkg-config (no -Iinclude, no -Ltarget) =="
export PKG_CONFIG_PATH="$STAGE/lib/pkgconfig"
pkg-config --exists libgibson || { echo "staged libgibson.pc not found under $PKG_CONFIG_PATH"; exit 1; }
CFLAGS_PC="$(pkg-config --cflags libgibson)"
LIBS_PC="$(pkg-config --libs libgibson)"
RPATH="$(pkg-config --variable=libdir libgibson)"
echo "   cflags: $CFLAGS_PC"
echo "   libs:   $LIBS_PC"
( cd "$WORK" && cc -std=c11 -Wall -Wextra main.c $CFLAGS_PC $LIBS_PC -Wl,-rpath,"$RPATH" -o doom_app )

echo "== 5. run the forcing consumer against the staged .so =="
"$WORK/doom_app"

echo "== Gate E: installed-SDK forcing consumer PASSED =="
