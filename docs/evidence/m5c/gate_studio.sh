#!/usr/bin/env bash
# M5-C slice 4 gate (figure::studio, the rev.v1 coupled-deformation compiler). Same shape as
# docs/evidence/m5c/gate_dqs.sh. Run only inside heavy, in a detached worktree; nobody commits there.
# Rules: any unexpected compile/test failure => non-zero exit; per-step rc logged; every test target
# must run N>0 tests (figure_studio at least STUDIO_MIN_TESTS); each studio mutant must compile and be
# killed by at least one assertion in tests/figure_studio.rs (there is no golden in that file), and by
# the specific tests predicted in KILL_MATRIX_STUDIO_PREDICTED.md. The python tools are checked first
# (they need no compiler): the converter selftest, the committed fixture is reproduced bit for bit,
# and the oracle script runs on the fixture.
# Usage: REPO=<main clone> W=<detached gate worktree> LOGDIR=<log dir> gate_studio.sh <head sha>
set -euo pipefail
HEAD_SHA=${1:?head sha}
REPO=${REPO:?main clone path}; W=${W:?detached gate worktree path}
L=${LOGDIR:?log directory}; mkdir -p "$L"
STUDIO_MIN_TESTS=${STUDIO_MIN_TESTS:-26}
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

# Python tools (python3 -I, no compiler).
run py_selftest python3 -I tools/figure_assets/convert_to_gibson.py --selftest; [ "$RC" -eq 0 ] || fail "converter selftest rc=$RC"
FX=$L/fixture_regen; rm -rf "$FX"
run py_fixture python3 -I tools/figure_assets/convert_to_gibson.py fixture --out "$FX"; [ "$RC" -eq 0 ] || fail "fixture regen rc=$RC"
if diff -r tests/fixtures/figure_studio "$FX" >"$L/fixture_diff.log" 2>&1; then echo "fixture_reproduced=1"; else fail "committed fixture differs from the generator output (see fixture_diff.log)"; fi
run py_oracle python3 -I docs/evidence/m5c/oracle_shape.py --asset tests/fixtures/figure_studio \
  --control stature=0.6 --control lean=-0.4 --control hip_breadth=0.5 --control bulge=0.3 \
  --garment tunic --garment shell --garment tabard --garment cap
[ "$RC" -eq 0 ] || fail "oracle rc=$RC"
grep -q "JOINT neck .*world=(0, -0.056, 1.61)" "$L/py_oracle.log" || fail "oracle neck position is not the designed (0, -0.056, 1.61)"
# Optional, only when a converted real asset and its export are present locally:
#   python3 -I docs/evidence/m5c/oracle_shape.py --asset $ASSET --preset $EXPORT/oracle_shaped.json \
#     --blender-oracle $EXPORT   (expected: BLENDER_ORACLE max_abs_err ~1e-7 OK)

run fmt $T fmt --all -- --check; [ "$RC" -eq 0 ] || fail "fmt rc=$RC"

# Every figure test target, figure_studio included. The v0 golden lives in figure_mechanics and
# figure_canonical_bytes: they must still pass unchanged (v0 bytes are untouched by this slice).
for t in figure_studio figure_dqs figure_refit figure_motion figure_sha256 figure_clip figure_mechanics figure_canonical_bytes; do
  run "head_$t" $T test --features figure --test "$t"
  n=$(ran "head_$t"); echo "head_${t}_ran=$n"
  grep -E "^test result" "$L/head_$t.log" | tail -1 || true
  [ "$RC" -eq 0 ] || fail "head $t rc=$RC"
  [ "$n" -gt 0 ] || fail "head $t ran 0 tests"
done
n=$(ran head_figure_studio)
[ "$n" -ge "$STUDIO_MIN_TESTS" ] || fail "figure_studio ran $n tests, expected >= $STUDIO_MIN_TESTS"
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
check_mutant ignore_skeleton_update_v1 figure_studio \
  refit_joints_are_the_anchor_centroids_of_the_shaped_mesh \
  bone_lengths_are_outputs_of_the_surface
check_mutant ignore_surface_update figure_studio \
  each_control_moves_its_measurement_monotonically_by_the_designed_amount \
  an_out_of_domain_control_is_applied_unclamped_and_reported \
  the_lowest_body_vertex_is_grounded \
  revision_isolation
check_mutant stale_fit figure_studio \
  a_garment_the_body_bulges_through_is_not_drawn \
  a_verdict_is_never_carried_across_a_body_edit \
  garments_follow_the_shaped_body_with_per_axis_offset_scales \
  a_bulge_outside_the_delete_set_still_rejects_and_inside_it_does_not
check_mutant stale_control figure_studio \
  with_control_is_bit_identical_to_a_full_compile_and_moves_the_measurement

run clippy $T clippy --features figure --lib --tests -- -D warnings; tail -5 "$L/clippy.log"; [ "$RC" -eq 0 ] || fail "clippy rc=$RC"
run msrv_figure cargo +1.85 check --lib --features figure; tail -3 "$L/msrv_figure.log"; [ "$RC" -eq 0 ] || fail "msrv rc=$RC"
run default_check $T check --lib; tail -3 "$L/default_check.log"; [ "$RC" -eq 0 ] || fail "default check rc=$RC"

echo "== box at end $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
echo "GATE_STUDIO_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
echo "GATE_STUDIO=GREEN"
