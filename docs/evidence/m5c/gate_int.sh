#!/usr/bin/env bash
# M5-C INTEGRATION gate (deform + studio + perform + preview + assets on one head).
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
STUDIO_MIN_TESTS=${STUDIO_MIN_TESTS:-36}
cd "$REPO"
HEAD_SHA=$(git rev-parse --verify "$HEAD_SHA^{commit}")
[ -d "$W" ] || git worktree add -q --detach "$W" "$HEAD_SHA"
cd "$W"
if git symbolic-ref -q HEAD >/dev/null || [[ -n $(git status --porcelain) ]]; then
  echo "Use a clean, detached gate worktree." >&2
  exit 2
fi
echo "== box at start $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
export CARGO_TARGET_DIR=${GATE_TARGET:-$W/target}
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

echo "== HEAD @ $HEAD_SHA"; git checkout -q --detach "$HEAD_SHA"
run fmt $T fmt --all -- --check; [ "$RC" -eq 0 ] || fail "fmt rc=$RC"

# Every figure test target. The v0 golden (figure_mechanics, figure_canonical_bytes) must still
# pass unchanged: this slice adds modules only.
for t in figure_studio figure_perform figure_preview figure_dqs figure_refit figure_motion figure_sha256 figure_clip figure_mechanics figure_canonical_bytes; do
  run "head_$t" $T test --features figure --test "$t"
  n=$(ran "head_$t"); echo "head_${t}_ran=$n"
  grep -E "^test result" "$L/head_$t.log" | tail -1 || true
  [ "$RC" -eq 0 ] || fail "head $t rc=$RC"
  [ "$n" -gt 0 ] || fail "head $t ran 0 tests"
done
n=$(ran head_figure_perform)
[ "$n" -ge "$PERFORM_MIN_TESTS" ] || fail "figure_perform ran $n tests, expected >= $PERFORM_MIN_TESTS"
n=$(ran head_figure_studio)
[ "$n" -ge "$STUDIO_MIN_TESTS" ] || fail "figure_studio ran $n tests, expected >= $STUDIO_MIN_TESTS"
if grep -rqs 'cfg(test)' src/figure src/figure.rs; then fail "src/figure gained unit tests; add a lib step"; fi

# Mutants. Expected killers (from the predicted matrix) must be among the failures.
check_mutant() {
  local mu=$1 target=$2; shift 2
  local name="mut_$mu"
  run "$name" env CARGO_TARGET_DIR=${GATE_TARGET:-$W/target}-mutant RUSTFLAGS="--cfg figure_mutant=\"$mu\"" \
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
# Integration mutants: the perform set (not yet gated on its own head), the preview mutant against
# the studio preview, and the two adaptation mutants. Other mutants were gated on their branches.
check_mutant contact_ignored figure_perform stance_feet_are_world_locked_and_on_the_ground
check_mutant snap_beats figure_perform every_beat_boundary_and_foot_lock_transition_is_c1
check_mutant hard_push_out figure_perform a_wave_cut_at_any_phase_and_the_clearance_floor_are_c1
check_mutant no_root_motion figure_perform walk_root_displacement_is_stride_times_steps_and_the_body_follows
check_mutant linear_keys figure_perform loop_seam_matches_a_seamless_reference_and_the_velocity_gap_closes
check_mutant preview_stale figure_studio the_studio_preview_shows_a_control_change
check_mutant no_contact_shadow figure_preview contact_shadow_darkens_ground_beside_each_foot contact_darkening_falls_off_with_distance
check_mutant shadow_detached figure_preview the_shadow_of_a_vertex_on_the_ground_lands_at_its_screen_position
check_mutant no_adapt figure_studio adaptation_makes_a_penetrating_garment_fit_and_leaves_the_rest_untouched
check_mutant adapt_rejected figure_studio a_rejected_garment_is_never_adapted_even_under_the_cap
check_mutant face_iris_backward figure_studio forward_eye_regions_and_face_materials_follow_the_posed_triangles
check_mutant face_unhashed figure_studio the_face_selection_and_source_bytes_change_only_presentation_identity
check_mutant no_layers figure_studio the_outer_garment_is_measured_against_the_fitted_inner_garment

run clippy $T clippy --features figure --lib --tests -- -D warnings; tail -5 "$L/clippy.log"; [ "$RC" -eq 0 ] || fail "clippy rc=$RC"
run msrv_figure cargo +1.85 check --lib --features figure; tail -3 "$L/msrv_figure.log"; [ "$RC" -eq 0 ] || fail "msrv rc=$RC"
run default_check $T check --lib; tail -3 "$L/default_check.log"; [ "$RC" -eq 0 ] || fail "default check rc=$RC"

# Examples: the synthetic preview, and (when ASSET_DIR is set) the studio preview of a real
# converted asset with the fitted suit (adapted), the skirt and hair. PNGs go to $OUT (local only).
OUT=${OUT:-$L/out}; mkdir -p "$OUT"
run example_synth $T run --release --features figure --example figure_preview_png -- "$OUT/synthetic.png" 1280 720
tail -2 "$L/example_synth.log"; [ "$RC" -eq 0 ] || fail "synthetic example rc=$RC"
if [ -n "${ASSET_DIR:-}" ]; then
  run example_real $T run --release --features figure --example figure_studio_preview -- \
    "$ASSET_DIR" "$OUT/studio_real.png" 1920 1080 ${REAL_ARGS:-}
  grep -E "^(fit|diag|wrote)" "$L/example_real.log" | cut -c1-300; [ "$RC" -eq 0 ] || fail "real example rc=$RC"
fi

echo "== box at end $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
echo "GATE_INT_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
echo "GATE_INT=GREEN"
