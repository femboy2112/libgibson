#!/usr/bin/env bash
# LibGibson v0.8 parity slice: build and run the five consumers of bindings/parity/v08/script.txt
# against an INSTALLED (staged) native SDK, diff every transcript against C (the ABI reference) and
# Rust, check replay determinism and resource bounds, run the negative controls, print the matrix.
#
#   scripts/dev/parity_v08.sh SDK_PREFIX            # or: LIBGIBSON_SDK=SDK_PREFIX scripts/dev/parity_v08.sh
#
# SDK_PREFIX is a staged SDK (scripts/release/stage-sdk.sh --prefix ...): include/, lib/,
# lib/pkgconfig/libgibson.pc. C and C++ compile against it via pkg-config and link with an rpath to
# its libdir; Python imports a copy of bindings/python/gibson.py (the module the wheel ships) from a
# temp dir and loads the SDK's libgibson.so via LIBGIBSON_LIBRARY; Go builds with cgo + pkg-config
# (GOPROXY=off); the Rust consumer is the gibson crate API (cargo example parity_v08).
#
# Environment:
#   PARITY_V08_REPS      in-process repetitions for the resource run (default 500)
#   PARITY_V08_WORK      work dir (default: a fresh mktemp dir; transcripts land in $WORK/out)
#   PARITY_V08_CARGO     cargo command (default: "cargo +1.98.1")
#   PARITY_V08_RUST_BIN  prebuilt examples/parity_v08 binary (skips the cargo build)
#   PARITY_V08_SKIP_CONTROLS=1  skip the negative controls
#
# Exit status: 0 = no regression against bindings/parity/v08/expected.json (documented gaps are
# not regressions); 1 = regression (a new transcript difference, an order change, a determinism or
# resource-bound failure, a consumer that failed to build/run, or a negative control the comparator
# did not catch); 2 = usage / environment error.
set -euo pipefail

die() { echo "parity_v08: $*" >&2; exit 2; }

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SDK=${1:-${LIBGIBSON_SDK:-}}
[ -n "$SDK" ] || die "usage: $0 SDK_PREFIX  (or set LIBGIBSON_SDK)"
SDK=$(cd "$SDK" && pwd)
[ -f "$SDK/lib/pkgconfig/libgibson.pc" ] || die "$SDK has no lib/pkgconfig/libgibson.pc"
REPS=${PARITY_V08_REPS:-500}
# The resource gate's bound is a fixed number of KB over the second half of the run, so its
# sensitivity depends on the repetition count: it detects a leak of about 4096 / (REPS / 2) KB per
# repetition (16 KB at 500). Below 500 the planted-leak control can slip under the bound (it did at
# 100: 3712 KB), so a shorter run is refused rather than reported with a weaker gate.
if [ "$REPS" -lt 500 ]; then
    echo "parity_v08: PARITY_V08_REPS=$REPS is below 500, the minimum at which the resource gate has its stated sensitivity" >&2
    exit 2
fi
WORK=${PARITY_V08_WORK:-$(mktemp -d "${TMPDIR:-/tmp}/parity_v08.XXXXXX")}
mkdir -p "$WORK/bin" "$WORK/out" "$WORK/pysite" "$WORK/ctl"
V08=$REPO/bindings/parity/v08
SCRIPT=$V08/script.txt
EXPECTED=$V08/expected.json
COMPARE=(python3 "$V08/compare.py")
read -r -a CARGO_CMD <<<"${PARITY_V08_CARGO:-cargo +1.98.1}"

# The main runs use the script exactly as committed.
unset PARITY_V08_PERTURB PARITY_V08_RUST_RESIZE

export PKG_CONFIG_PATH=$SDK/lib/pkgconfig
LIBDIR=$(pkg-config --variable=libdir libgibson)
read -r -a PC <<<"$(pkg-config --cflags --libs libgibson)"
echo "parity_v08: SDK=$SDK (libgibson $(pkg-config --modversion libgibson)) WORK=$WORK REPS=$REPS"
echo "parity_v08: libgibson.so sha256 $(sha256sum "$LIBDIR/libgibson.so" | cut -d' ' -f1)"

# ---------------------------------------------------------------------------------------- build
BUILD_FAIL=0
build() { # name, command...
    local name=$1; shift
    if "$@" >"$WORK/out/build_$name.log" 2>&1; then echo "  built $name"; else
        echo "  BUILD FAILED: $name (see $WORK/out/build_$name.log)"; BUILD_FAIL=1; fi
}
echo "== build =="
build c cc -std=c11 -O2 -Wall -Wextra -o "$WORK/bin/parity_v08_c" "$V08/parity_v08.c" "${PC[@]}" -Wl,-rpath,"$LIBDIR"
build cpp c++ -std=c++17 -O2 -Wall -Wextra -o "$WORK/bin/parity_v08_cpp" "$V08/parity_v08.cpp" "${PC[@]}" -Wl,-rpath,"$LIBDIR"
build py cp "$REPO/bindings/python/gibson.py" "$WORK/pysite/gibson.py"
build go env -C "$V08/go" GOFLAGS=-mod=mod GOPROXY=off CGO_LDFLAGS="-Wl,-rpath,$LIBDIR" go build -o "$WORK/bin/parity_v08_go" .
if [ -n "${PARITY_V08_RUST_BIN:-}" ]; then
    RUST_BIN=$PARITY_V08_RUST_BIN
    echo "  rust: using prebuilt $RUST_BIN"
else
    build rust env -C "$REPO" "${CARGO_CMD[@]}" build --release --example parity_v08
    RUST_BIN=${CARGO_TARGET_DIR:-$REPO/target}/release/examples/parity_v08
fi

run_lang() { # lang, extra args...  (stdout = transcript, stderr = meta lines)
    local lang=$1; shift
    case $lang in
        c) "$WORK/bin/parity_v08_c" "$SCRIPT" "$@" ;;
        cpp) "$WORK/bin/parity_v08_cpp" "$SCRIPT" "$@" ;;
        py) (cd "$WORK" && PYTHONPATH="$WORK/pysite" LIBGIBSON_LIBRARY="$SDK/lib/libgibson.so" \
                python3 "$V08/parity_v08.py" "$SCRIPT" "$@") ;;
        go) "$WORK/bin/parity_v08_go" "$SCRIPT" "$@" ;;
        rust) "$RUST_BIN" "$SCRIPT" "$@" ;;
    esac
}

# ------------------------------------------------------------------------------------------ run
# A and B: two separate processes (cross-process replay determinism).
# rep: one process repeating the whole script REPS times (resources + in-process determinism).
echo "== run (A, B, and $REPS in-process repetitions per language) =="
RUN_FAIL=0
for lang in c cpp py go rust; do
    for pass in a b; do
        if ! run_lang "$lang" >"$WORK/out/$lang.$pass.jsonl" 2>"$WORK/out/$lang.$pass.err"; then
            echo "  RUN FAILED: $lang ($pass)"; RUN_FAIL=1; fi
    done
    if ! run_lang "$lang" "$REPS" >"$WORK/out/$lang.rep.jsonl" 2>"$WORK/out/$lang.rep.err"; then
        echo "  RUN FAILED: $lang (reps)"; RUN_FAIL=1; fi
    echo "  ran $lang: $(wc -l <"$WORK/out/$lang.a.jsonl") records"
done
echo "== wrapper-surface observations (stderr of run A; not part of the comparable transcript) =="
cat "$WORK/out/"*.a.err | grep '"k":"wrapper"' | sed 's/^/  /' || true

# -------------------------------------------------------------------------------------- compare
MATRIX_RC=0
"${COMPARE[@]}" matrix --script "$SCRIPT" --expected "$EXPECTED" --dir "$WORK/out" || MATRIX_RC=$?

# ---------------------------------------------------------------------------- negative controls
CTL_FAIL=0
if [ "${PARITY_V08_SKIP_CONTROLS:-0}" != 1 ]; then
    echo "== negative controls (each must be CAUGHT as a regression, with exactly the planted ids) =="
    control() { # label, lang, want, env assignment
        local label=$1 lang=$2 want=$3 assign=$4 out rc line
        out="$WORK/ctl/$label.$lang.jsonl"
        ( export "${assign?}"; run_lang "$lang" ) >"$out" 2>/dev/null || true
        rc=0
        line=$("${COMPARE[@]}" check --script "$SCRIPT" --expected "$EXPECTED" --ref "$WORK/out/c.a.jsonl" \
            --lang "$lang" --transcript "$out" --want "$want" | tail -n 1) || rc=$?
        if [ "$rc" = 1 ] && [[ $line == *EXACT ]]; then
            echo "  CAUGHT   $label/$lang: $line"
        else
            echo "  MISSED   $label/$lang (rc=$rc): $line"; CTL_FAIL=1
        fi
    }
    for lang in c cpp py go rust; do
        control cell "$lang" "r03,s16" PARITY_V08_PERTURB=cell
        control events "$lang" "ORDER,e09.1,e09.2" PARITY_V08_PERTURB=events
    done
    # Gap witness (not a perturbation): without the explicit set_terminal_size step, the Rust
    # consumer's polled Resize leaves the headless geometry at 40x12.
    control rust-resize-native rust "r02,r03,r04" PARITY_V08_RUST_RESIZE=native
    grep '"id":"r02"' "$WORK/ctl/rust-resize-native.rust.jsonl" | sed 's/^/    witness r02: /' || true
    # Resource-gate control: the C consumer leaks its context every repetition.
    ( export PARITY_V08_PERTURB=leak; run_lang c "$REPS" ) >/dev/null 2>"$WORK/ctl/leak.c.err" || true
    rc=0
    line=$("${COMPARE[@]}" resource --expected "$EXPECTED" --err "$WORK/ctl/leak.c.err") || rc=$?
    if [ "$rc" = 1 ] && [[ $line == *EXCEEDED ]]; then echo "  CAUGHT   leak/c: $line"
    else echo "  MISSED   leak/c (rc=$rc): $line"; CTL_FAIL=1; fi
fi

echo "== summary =="
echo "  transcripts: $WORK/out"
echo "  build_fail=$BUILD_FAIL run_fail=$RUN_FAIL matrix_rc=$MATRIX_RC control_fail=$CTL_FAIL"
if [ "$BUILD_FAIL" != 0 ] || [ "$RUN_FAIL" != 0 ] || [ "$MATRIX_RC" != 0 ] || [ "$CTL_FAIL" != 0 ]; then
    echo "parity_v08: REGRESSION"
    exit 1
fi
echo "parity_v08: OK (no regression against expected.json)"
