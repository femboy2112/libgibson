#!/usr/bin/env bash
# VIS-3D lease, one heavy s run in a detached worktree where nobody commits.
#   1. gate_c1.sh on the VIS-3D head: RED, all figure test targets (including the A4 id test),
#      the 3 motion mutants, whole-crate clippy, MSRV and the default check.
#   2. clippy -D warnings on the figure_vis3d_kit example.
#   3. run_kit.sh: release build, render, determinism double run, PNG/MP4 conversion, staging.
# Any failure gives a non-zero exit. Each step's rc is printed.
# Usage: REPO=<main clone> W=<detached worktree> LOGDIR=<logs> KIT_LOCAL=<local output dir>
#        KIT_STAGE=<staging dir for the committed subset> lease_vis3d.sh <head sha> <red sha>
set -euo pipefail
HEAD_SHA=${1:?head sha}; RED_SHA=${2:?red sha}
REPO=${REPO:?}; W=${W:?}; L=${LOGDIR:?}; KL=${KIT_LOCAL:?}; KS=${KIT_STAGE:?}
mkdir -p "$L" "$KL" "$KS"
cd "$REPO"; git fetch -q origin
[ -d "$W" ] || git worktree add -q --detach "$W" "$HEAD_SHA"
cd "$W"; git checkout -q -f --detach "$HEAD_SHA"
G=$W/docs/evidence/m5b/c1/gate_c1.sh
FAILS=0
set +e
REPO="$REPO" W="$W" LOGDIR="$L/gate" "$G" "$HEAD_SHA" "$RED_SHA" > "$L/gate.out" 2>&1; rc=$?
set -e
grep -E '_exit=|_ran=|KILL|GATE' "$L/gate.out" | cut -c1-200
echo "step1_gate_exit=$rc"; [ "$rc" -eq 0 ] || FAILS=$((FAILS+1))
# gate_c1 leaves the worktree at HEAD.
export CARGO_TARGET_DIR=$W/target
set +e
cargo +1.98.1 clippy --features figure --example figure_vis3d_kit -- -D warnings > "$L/clippy_example.log" 2>&1; rc=$?
set -e
tail -5 "$L/clippy_example.log"; echo "step2_clippy_example_exit=$rc"; [ "$rc" -eq 0 ] || FAILS=$((FAILS+1))
set +e
CARGO="cargo +1.98.1" KIT_COMMIT_DIR="$KS" bash "$W/docs/evidence/m5b/vis3d/run_kit.sh" "$KL" > "$L/kit.log" 2>&1; rc=$?
set -e
tail -25 "$L/kit.log"; echo "step3_kit_exit=$rc"; [ "$rc" -eq 0 ] || FAILS=$((FAILS+1))
echo "LEASE_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
echo "LEASE=GREEN"
