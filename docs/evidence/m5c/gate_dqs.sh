#!/usr/bin/env bash
# M5-C slice 2 gate (dual-quaternion skinning). Modelled on docs/evidence/m5b/c1/gate_c1.sh.
# Run only inside heavy, in a detached worktree; nobody commits there.
# Slice 3 (skeleton refit) is gated here too: figure_refit and the ignored_refit mutant.
# Rules: any unexpected compile/test failure => non-zero exit; per-step rc logged; every test
# target must run N>0 tests (figure_dqs must run at least DQS_MIN_TESTS); each DQS mutant must
# compile and be killed by at least one assertion in tests/figure_dqs.rs (there is no golden in
# that file), and by the specific tests predicted in KILL_MATRIX_DQS_PREDICTED.md.
# Usage: REPO=<main clone> W=<detached gate worktree> LOGDIR=<log dir> gate_dqs.sh <head sha>
set -euo pipefail
HEAD_SHA=${1:?head sha}
REPO=${REPO:?main clone path}; W=${W:?detached gate worktree path}
L=${LOGDIR:?log directory}; mkdir -p "$L"
DQS_MIN_TESTS=${DQS_MIN_TESTS:-11}
cd "$REPO"
git fetch -q origin feat/m5c-deform
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

# Every figure test target, figure_dqs included. The v0 golden lives in figure_mechanics and
# figure_canonical_bytes: they must still pass unchanged (v0 bytes are untouched by this slice).
for t in figure_dqs figure_refit figure_motion figure_sha256 figure_clip figure_mechanics figure_canonical_bytes; do
  run "head_$t" $T test --features figure --test "$t"
  n=$(ran "head_$t"); echo "head_${t}_ran=$n"
  grep -E "^test result" "$L/head_$t.log" | tail -1 || true
  [ "$RC" -eq 0 ] || fail "head $t rc=$RC"
  [ "$n" -gt 0 ] || fail "head $t ran 0 tests"
done
n=$(ran head_figure_refit)
[ "$n" -ge 7 ] || fail "figure_refit ran $n tests, expected >= 7"
n=$(ran head_figure_dqs)
[ "$n" -ge "$DQS_MIN_TESTS" ] || fail "figure_dqs ran $n tests, expected >= $DQS_MIN_TESTS"
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
check_mutant dqs_as_lbs figure_dqs \
  elbow_cross_section_dualquat_keeps_area_and_linear_collapses \
  a_non_rigid_skin_matrix_is_rejected_not_dropped \
  sweep_across_a_quaternion_sign_flip_is_continuous \
  dualquat_matches_f64_oracle_on_two_bone_fixture \
  dualquat_matches_f64_oracle_on_three_bone_fixture \
  dualquat_normals_match_oracle_and_stay_unit
check_mutant dqs_no_align figure_dqs \
  blend_is_invariant_to_the_sign_of_a_later_term_and_takes_the_short_way \
  sweep_across_a_quaternion_sign_flip_is_continuous
check_mutant ignored_refit figure_refit \
  joints_land_on_the_group_centroids_in_a_rotated_frame \
  a_shaped_surface_moves_the_skeleton_and_the_unanchored_child_rides_along \
  derived_skeleton_matches_the_f64_centroids_and_inverse_binds \
  the_centroid_sums_in_declared_order

run clippy $T clippy --features figure --lib --tests -- -D warnings; tail -5 "$L/clippy.log"; [ "$RC" -eq 0 ] || fail "clippy rc=$RC"
run msrv_figure cargo +1.85 check --lib --features figure; tail -3 "$L/msrv_figure.log"; [ "$RC" -eq 0 ] || fail "msrv rc=$RC"
run default_check $T check --lib; tail -3 "$L/default_check.log"; [ "$RC" -eq 0 ] || fail "default check rc=$RC"

echo "== box at end $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
echo "GATE_DQS_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
echo "GATE_DQS=GREEN"
