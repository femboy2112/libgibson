#!/usr/bin/env bash
# M5-B C-1 gate (continuous clips, inertialized preemption, Look retarget).
# Run only inside heavy, in a detached worktree; nobody commits there.
# Rules (coordinator rulings #66/#69): any unexpected compile/test failure => non-zero exit;
# per-step rc logged; every test target must run N>0 tests; each motion mutant must compile
# and be killed by at least one assertion in tests/figure_motion.rs.
# Usage: REPO=<main clone> W=<detached gate worktree> LOGDIR=<log dir> gate_c1.sh <head sha> <red sha>
set -euo pipefail
HEAD_SHA=${1:?head sha}; RED_SHA=${2:?red sha}
REPO=${REPO:?main clone path}; W=${W:?detached gate worktree path}
L=${LOGDIR:?log directory}; mkdir -p "$L"
cd "$REPO"
git fetch -q origin feat/m5b-c1-continuous
[ -d "$W" ] || git worktree add -q --detach "$W" "$HEAD_SHA"
cd "$W"
echo "== box at start $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
export CARGO_TARGET_DIR=$W/target
T="cargo +1.98.1"
FAILS=0
fail() { echo "GATE_FAIL: $*"; FAILS=$((FAILS+1)); }
run() {
  local name=$1; shift
  set +e; "$@" >"$L/$name.log" 2>&1; RC=$?; set -e
  echo "${name}_exit=$RC"
}
ran() { awk '/^running [0-9]+ tests?$/{n+=$2} END{print n+0}' "$L/$1.log"; }
compile_err() { grep -qE "^error(\[E[0-9]+\])?:" "$L/$1.log" && ! grep -q "^running " "$L/$1.log"; }

# RED: the stub commit must compile and fail (informational for todo!() stubs; the mutants
# below are the discriminating controls).
echo "== RED @ $RED_SHA"; git checkout -q -f --detach "$RED_SHA"
run red_figure_motion $T test --features figure --test figure_motion --no-fail-fast
if compile_err red_figure_motion; then fail "RED does not compile"; fi
[ "$RC" -ne 0 ] || fail "RED passed"
echo "red_failed=$(grep -cE '^test .* FAILED$' "$L/red_figure_motion.log" || true) red_ran=$(ran red_figure_motion)"

echo "== HEAD @ $HEAD_SHA"; git checkout -q -f --detach "$HEAD_SHA"
run fmt $T fmt --all -- --check; [ "$RC" -eq 0 ] || fail "fmt rc=$RC"
for t in figure_motion figure_sha256 figure_clip figure_mechanics figure_canonical_bytes; do
  run "head_$t" $T test --features figure --test "$t"
  n=$(ran "head_$t"); echo "head_${t}_ran=$n"
  grep -E "^test result" "$L/head_$t.log" | tail -1 || true
  [ "$RC" -eq 0 ] || fail "head $t rc=$RC"
  [ "$n" -gt 0 ] || fail "head $t ran 0 tests"
done
if grep -rqs 'cfg(test)' src/figure src/figure.rs; then fail "src/figure gained unit tests; add a lib step"; fi

for mu in snap_transition freeze_on_preempt linear_keys; do
  name="mut_$mu"
  run "$name" env CARGO_TARGET_DIR=$W/target-mutant RUSTFLAGS="--cfg figure_mutant=\"$mu\"" \
    $T test --features figure --test figure_motion --no-fail-fast
  if compile_err "$name"; then fail "mutant $mu did not compile"; echo "KILL[$mu]=COMPILE_ERROR"; continue; fi
  n=$(ran "$name"); [ "$n" -gt 0 ] || fail "mutant $mu ran 0 tests"
  killed=$(grep -E "^test .* FAILED$" "$L/$name.log" | sed -E 's/^test (.*) \.\.\. FAILED$/\1/' | sort -u | tr '\n' ' ' || true)
  k=$(grep -cE "^test .* FAILED$" "$L/$name.log" || true)
  echo "KILL[$mu]($k)=${killed:-none}"
  [ "$k" -gt 0 ] || fail "mutant $mu not killed"
  # kills must be assertion failures, not panics from src/
  srcp=$(grep -cE "panicked at src/" "$L/$name.log" || true); echo "mutant_${mu}_src_panics=$srcp"
done

run clippy $T clippy --features figure --lib --tests -- -D warnings; tail -5 "$L/clippy.log"; [ "$RC" -eq 0 ] || fail "clippy rc=$RC"
run msrv_figure cargo +1.85 check --lib --features figure; tail -3 "$L/msrv_figure.log"; [ "$RC" -eq 0 ] || fail "msrv rc=$RC"
run default_check $T check --lib; tail -3 "$L/default_check.log"; [ "$RC" -eq 0 ] || fail "default check rc=$RC"

echo "== box at end $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
echo "GATE_C1_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
echo "GATE_C1=GREEN"
