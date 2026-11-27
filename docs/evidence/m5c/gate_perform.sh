#!/usr/bin/env bash
# M5-C performance gate (two-bone IK, procedural performance, tape, Live, loop-seam probe).
# Modelled on docs/evidence/m5b/c1/gate_c1.sh and gate_dqs.sh. Run only inside heavy, in a detached
# worktree; nobody commits there.
# Rules: any unexpected compile/test failure => non-zero exit; per-step rc logged; every test target
# must run N>0 tests (figure_perform at least PERFORM_MIN_TESTS); each performance mutant must
# compile and be killed by at least one assertion in tests/figure_perform.rs (there is no golden in
# that file) and by the specific tests predicted in KILL_MATRIX_PERFORM_PREDICTED.md.
# Usage: REPO=<main clone> W=<detached gate worktree> LOGDIR=<log dir> gate_perform.sh <head sha>
set -euo pipefail
HEAD_SHA=${1:?head sha}
REPO=${REPO:?main clone path}; W=${W:?detached gate worktree path}
L=${LOGDIR:?log directory}; mkdir -p "$L"
PERFORM_MIN_TESTS=${PERFORM_MIN_TESTS:-24}
cd "$REPO"
git fetch -q origin feat/m5c-perform
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
killed_set() { grep -E "^test .* FAILED$" "$L/$1.log" | sed -E 's/^test (.*) \.\.\. FAILED$/\1/' | sort -u | tr '\n' ' ' || true; }

echo "== HEAD @ $HEAD_SHA"; git checkout -q -f --detach "$HEAD_SHA"
run fmt $T fmt --all -- --check; [ "$RC" -eq 0 ] || fail "fmt rc=$RC"

# Every figure test target. The v0 golden (figure_mechanics, figure_canonical_bytes) must still
# pass unchanged: this slice adds modules only.
for t in figure_perform figure_motion figure_sha256 figure_clip figure_mechanics figure_canonical_bytes; do
  run "head_$t" $T test --features figure --test "$t"
  n=$(ran "head_$t"); echo "head_${t}_ran=$n"
  grep -E "^test result" "$L/head_$t.log" | tail -1 || true
  [ "$RC" -eq 0 ] || fail "head $t rc=$RC"
  [ "$n" -gt 0 ] || fail "head $t ran 0 tests"
done
n=$(ran head_figure_perform)
[ "$n" -ge "$PERFORM_MIN_TESTS" ] || fail "figure_perform ran $n tests, expected >= $PERFORM_MIN_TESTS"
if grep -rqs 'cfg(test)' src/figure src/figure.rs; then fail "src/figure gained unit tests; add a lib step"; fi

# Mutants. Expected killers (from the predicted matrix) must be among the failures.
check_mutant() {
  local mu=$1 target=$2; shift 2
  local name="mut_$mu"
  run "$name" env CARGO_TARGET_DIR=$W/target-mutant RUSTFLAGS="--cfg figure_mutant=\"$mu\"" \
    $T test --features figure --test "$target" --no-fail-fast
  if compile_err "$name"; then fail "mutant $mu did not compile"; echo "KILL[$mu]=COMPILE_ERROR"; return; fi
  local nn; nn=$(ran "$name"); [ "$nn" -gt 0 ] || fail "mutant $mu ran 0 tests"
  local killed k; killed=$(killed_set "$name"); k=$(grep -cE "^test .* FAILED$" "$L/$name.log" || true)
  echo "KILL[$mu]($k)=${killed:-none}"
  [ "$k" -gt 0 ] || fail "mutant $mu not killed"
  local srcp; srcp=$(grep -cE "panicked at src/" "$L/$name.log" || true); echo "mutant_${mu}_src_panics=$srcp"
  local want
  for want in "$@"; do
    case " $killed " in *" $want "*) ;; *) fail "mutant $mu: predicted killer $want did not fail";; esac
  done
}
check_mutant contact_ignored figure_perform \
  stance_feet_are_world_locked_and_on_the_ground \
  walk_with_a_turn_keeps_the_feet_planted \
  sit_settles_the_pelvis_at_the_seat_with_planted_feet_and_stand_returns \
  live_walk_reaches_the_target_planted
check_mutant snap_beats figure_perform \
  every_beat_boundary_and_foot_lock_transition_is_c1 \
  control_a_snapped_beat_cut_breaks_the_same_metric \
  the_cut_blend_takes_the_settle_ease_and_lowers_the_hand_monotonically \
  live_preemption_blends_from_the_current_pose_and_a_snap_does_not \
  live_intents_queue_by_priority_and_an_expired_one_eases_out
check_mutant no_root_motion figure_perform \
  walk_root_displacement_is_stride_times_steps_and_the_body_follows \
  stance_feet_are_world_locked_and_on_the_ground \
  the_tape_is_deterministic_well_formed_and_hashes_its_body
# The loop-seam probe is killed by the existing motion mutant linear_keys (a velocity step at the
# seam); informational, not a perform mutant.
check_mutant linear_keys figure_perform \
  loop_seam_matches_a_seamless_reference_and_the_velocity_gap_closes

run clippy $T clippy --features figure --lib --tests -- -D warnings; tail -5 "$L/clippy.log"; [ "$RC" -eq 0 ] || fail "clippy rc=$RC"
run msrv_figure cargo +1.85 check --lib --features figure; tail -3 "$L/msrv_figure.log"; [ "$RC" -eq 0 ] || fail "msrv rc=$RC"
run default_check $T check --lib; tail -3 "$L/default_check.log"; [ "$RC" -eq 0 ] || fail "default check rc=$RC"

echo "== box at end $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
echo "GATE_PERFORM_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
echo "GATE_PERFORM=GREEN"
