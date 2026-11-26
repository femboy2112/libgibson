#!/usr/bin/env bash
# M5-B gate RUN B (HEAD tests, mutant kill matrix, clippy, MSRV, default check).
# Run only inside heavy, in a detached worktree; nobody commits there.
# RULING #66: any unexpected compile/test failure => non-zero script exit; per-step rc logged;
# every test target must run N>0 tests (0 matched = FAIL); --features figure everywhere.
set -euo pipefail
# Usage: REPO=<main clone> W=<detached gate worktree> LOGDIR=<log dir> gate_b.sh <head sha>
HEAD_SHA=${1:?head sha}
REPO=${REPO:?main clone path}; W=${W:?detached gate worktree path}
L=${LOGDIR:?log directory}; mkdir -p "$L"
cd "$REPO"
git fetch -q origin feat/m5b-figure-mechanics
[ -d "$W" ] || git worktree add -q --detach "$W" "$HEAD_SHA"
cd "$W"
echo "== box at start $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
export CARGO_TARGET_DIR=$W/target
T="cargo +1.98.1"
FAILS=0
fail() { echo "GATE_FAIL: $*"; FAILS=$((FAILS+1)); }

# run NAME CMD... : capture rc without tripping set -e; log to $L/NAME.log
run() {
  local name=$1; shift
  set +e; "$@" >"$L/$name.log" 2>&1; RC=$?; set -e
  echo "${name}_exit=$RC"
}
# passed: sum of passed counts in test-result lines (a filter matching nothing gives 0)
passed() { awk '/^test result:/{for(i=1;i<=NF;i++) if($(i+1)=="passed;") n+=$i} END{print n+0}' "$L/$1.log"; }
# canary: N>0 tests actually ran in this log (sum of "running N tests" lines)
ran() { awk '/^running [0-9]+ tests?$/{n+=$2} END{print n+0}' "$L/$1.log"; }

echo "== HEAD @ $HEAD_SHA"; git checkout -q -f --detach "$HEAD_SHA"
run fmt $T fmt --all -- --check; [ "$RC" -eq 0 ] || fail "fmt rc=$RC"

TARGETS="figure_sha256 figure_clip figure_mechanics figure_canonical_bytes"
for t in $TARGETS; do
  run "head_$t" $T test --features figure --test "$t"
  n=$(ran "head_$t"); echo "head_${t}_ran=$n"
  grep -E "^test result" "$L/head_$t.log" | tail -1 || true
  [ "$RC" -eq 0 ] || fail "head $t rc=$RC"
  [ "$n" -gt 0 ] || fail "head $t ran 0 tests"
done
run head_lib_figure $T test --features figure --lib figure::
n=$(passed head_lib_figure); echo "head_lib_figure_passed=$n"
[ "$RC" -eq 0 ] || fail "lib figure rc=$RC"
[ "$n" -gt 0 ] || fail "lib figure:: matched 0 tests"

# Mutant kill matrix: each mutant MUST compile and fail on assertions (not compile errors).
for mu in ignored_skeleton_update stale_bind noop_morph; do
  for t in $TARGETS; do
    name="mut_${mu}_$t"
    run "$name" env CARGO_TARGET_DIR=$W/target-mutant RUSTFLAGS="--cfg figure_mutant=\"$mu\"" \
      $T test --features figure --test "$t" --no-fail-fast
    if grep -qE "^error(\[E[0-9]+\])?:" "$L/$name.log" && ! grep -q "^running " "$L/$name.log"; then
      fail "mutant $mu / $t did not compile"; echo "KILL[$mu][$t]=COMPILE_ERROR"; continue
    fi
    n=$(ran "$name"); [ "$n" -gt 0 ] || fail "mutant $mu / $t ran 0 tests"
    killed=$(grep -E "^test .* FAILED$" "$L/$name.log" | sed -E 's/^test (.*) \.\.\. FAILED$/\1/' | sort -u | tr '\n' ' ' || true)
    echo "KILL[$mu][$t]=${killed:-none}"
  done
  # must be killed by at least one non-golden test
  ng=$(grep -hE "^test .* FAILED$" "$L"/mut_${mu}_*.log 2>/dev/null | grep -vi golden | wc -l || true)
  echo "mutant_${mu}_nongolden_kills=$ng"
  [ "$ng" -gt 0 ] || fail "mutant $mu not killed by any non-golden test"
done

run clippy $T clippy --features figure --lib --tests -- -D warnings; tail -5 "$L/clippy.log"; [ "$RC" -eq 0 ] || fail "clippy rc=$RC"
run msrv_figure cargo +1.85 check --lib --features figure; tail -3 "$L/msrv_figure.log"; [ "$RC" -eq 0 ] || fail "msrv rc=$RC"
run default_check $T check --lib; tail -3 "$L/default_check.log"; [ "$RC" -eq 0 ] || fail "default check rc=$RC"

echo "== box at end $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
echo "GATE_B_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
echo "GATE_B=GREEN"
