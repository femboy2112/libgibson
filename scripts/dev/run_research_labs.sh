#!/usr/bin/env bash
#
# run_research_labs.sh — one-shot runner for the two research labs on the
# research/temporal-braille-persistence-2026-10-03 branch.
#
#   * examples/temporal_braille_persistence_lab.rs        (temporal PDM)
#   * examples/terminal_to_1080p_lab.rs                   (minimal glyph basis)
#
# It builds both examples in release, runs every non-interactive mode, writes each
# output to a file, and runs the two example test suites. It does NOT run
# --mode=live (that needs an interactive TTY and is the only human-perception arm),
# and it never opens a PR or pushes.
#
# Usage:
#   scripts/dev/run_research_labs.sh            # build, run everything, run tests
#   scripts/dev/run_research_labs.sh --quick    # smaller grids / fewer seeds
#   SKIP_TESTS=1 scripts/dev/run_research_labs.sh
#   SKIP_BUILD=1 scripts/dev/run_research_labs.sh
#   OUT_DIR=/tmp/my-run scripts/dev/run_research_labs.sh
#
# Env:
#   OUT_DIR      output directory (default: target/research-labs/<timestamp>)
#   SKIP_TESTS   set to 1 to skip the example test suites
#   SKIP_BUILD   set to 1 to reuse already-built release examples
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

QUICK=0
if [[ "${1:-}" == "--quick" ]]; then
    QUICK=1
fi

STAMP="$(date +%Y%m%d-%H%M%S)"
OUT="${OUT_DIR:-$REPO_ROOT/target/research-labs/$STAMP}"
mkdir -p "$OUT"

T=./target/release/examples/temporal_braille_persistence_lab
A=./target/release/examples/terminal_to_1080p_lab

# ---------------------------------------------------------------- pretty output
if [[ -t 1 ]]; then
    BOLD=$'\033[1m'; DIM=$'\033[2m'; RESET=$'\033[0m'
else
    BOLD=''; DIM=''; RESET=''
fi
say()  { printf '\n%s== %s ==%s\n' "$BOLD" "$*" "$RESET"; }
note() { printf '%s%s%s\n' "$DIM" "$*" "$RESET"; }

# run <name> <cmd...>  -> tee to $OUT/<name>.txt
run() {
    local name="$1"; shift
    say "$name"
    note "\$ $*"
    "$@" 2>&1 | tee "$OUT/$name.txt"
}

# ---------------------------------------------------------------- build
if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
    say "build (release)"
    cargo build --release \
        --example temporal_braille_persistence_lab \
        --example terminal_to_1080p_lab
else
    note "SKIP_BUILD=1: reusing existing release examples"
fi

# ---------------------------------------------------------------- temporal lab
T_ARGS=()
if [[ "$QUICK" == "1" ]]; then
    T_ARGS+=(--quick)
    MC_SEEDS=16
    PARETO_N=128
else
    MC_SEEDS=48
    PARETO_N=512
fi

run matrix       "$T" --mode=matrix        "${T_ARGS[@]}"
run montecarlo   "$T" --mode=montecarlo    "${T_ARGS[@]}" --k=8 --seeds="$MC_SEEDS"
run spectrum     "$T" --mode=spectrum      "${T_ARGS[@]}"
run pareto       "$T" --mode=pareto        "${T_ARGS[@]}" --k=8 --n="$PARETO_N"
run loss         "$T" --mode=loss          "${T_ARGS[@]}"
run framelocal   "$T" --mode=framelocal    "${T_ARGS[@]}"
run reach        "$T" --mode=reach         "${T_ARGS[@]}"
run decompose    "$T" --mode=decompose     "${T_ARGS[@]}"
run color        "$T" --mode=color         "${T_ARGS[@]}"
run livediag     "$T" --mode=livediag      "${T_ARGS[@]}"

# ---------------------------------------------------------------- glyph lab
run rank            "$A" --mode=rank
run wall            "$A" --mode=wall
run search          "$A" --mode=search
run search-portable "$A" --mode=search --portable
run cv              "$A" --mode=cv
run cv-portable     "$A" --mode=cv --portable
run ablations       "$A" --mode=ablations
run lattice         "$A" --mode=lattice
run shape           "$A" --mode=shape

# ---------------------------------------------------------------- tests
if [[ "${SKIP_TESTS:-0}" != "1" ]]; then
    say "tests"
    cargo test --example temporal_braille_persistence_lab 2>&1 | tee "$OUT/test-temporal.txt" | tail -2
    cargo test --example terminal_to_1080p_lab 2>&1 | tee "$OUT/test-1080p.txt" | tail -2
else
    note "SKIP_TESTS=1: skipping example test suites"
fi

# ---------------------------------------------------------------- summary
say "done"
echo "outputs: $OUT"
ls -1 "$OUT"
echo
note "Read: docs/research/TEMPORAL_BRAILLE_PERSISTENCE_2026-10-03.md"
note "      docs/research/TERMINAL_TO_1080P_GLYPH_BASIS_2026-10-03.md"
note "Perception is HUMAN UNVERIFIED until --mode=live is run on a real TTY behind"
note "temporal_cadence_beacon with a camera/photodiode."
