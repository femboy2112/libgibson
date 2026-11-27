#!/usr/bin/env bash
# M5-C preview gate (software-raster preview, feature `figure`).
# Run only inside heavy, in a detached worktree; nobody commits there.
# Any unexpected compile/test failure => non-zero exit; per-step rc logged; every test target must
# run N>0 tests; the preview_stale mutant must compile and be killed by a non-golden assertion.
# Usage: REPO=<main clone> W=<detached gate worktree> LOGDIR=<log dir> OUT=<png dir> \
#        gate_preview.sh <head sha>
set -euo pipefail
HEAD_SHA=${1:?head sha}
REPO=${REPO:?main clone path}; W=${W:?detached gate worktree path}
L=${LOGDIR:?log directory}; OUT=${OUT:?png output dir}; mkdir -p "$L" "$OUT"
cd "$REPO"
git fetch -q origin feat/m5c-preview
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

echo "== HEAD @ $HEAD_SHA"; git checkout -q -f --detach "$HEAD_SHA"
run fmt $T fmt --all -- --check; [ "$RC" -eq 0 ] || fail "fmt rc=$RC"
for t in figure_preview figure_motion figure_sha256 figure_clip figure_mechanics figure_canonical_bytes; do
  [ -f "tests/$t.rs" ] || { echo "skip missing tests/$t.rs"; continue; }
  run "head_$t" $T test --features figure --test "$t"
  n=$(ran "head_$t"); echo "head_${t}_ran=$n"
  grep -E "^test result" "$L/head_$t.log" | tail -1 || true
  [ "$RC" -eq 0 ] || fail "head $t rc=$RC"
  [ "$n" -gt 0 ] || fail "head $t ran 0 tests"
done
if grep -rqs 'cfg(test)' src/figure src/figure.rs; then fail "src/figure gained unit tests; add a lib step"; fi

for mu in preview_stale; do
  name="mut_$mu"
  run "$name" env CARGO_TARGET_DIR=$W/target-mutant RUSTFLAGS="--cfg figure_mutant=\"$mu\"" \
    $T test --features figure --test figure_preview --no-fail-fast
  if compile_err "$name"; then fail "mutant $mu did not compile"; echo "KILL[$mu]=COMPILE_ERROR"; continue; fi
  n=$(ran "$name"); [ "$n" -gt 0 ] || fail "mutant $mu ran 0 tests"
  killed=$(grep -E "^test .* FAILED$" "$L/$name.log" | sed -E 's/^test (.*) \.\.\. FAILED$/\1/' | sort -u | tr '\n' ' ' || true)
  k=$(grep -cE "^test .* FAILED$" "$L/$name.log" || true)
  echo "KILL[$mu]($k)=${killed:-none}"
  [ "$k" -gt 0 ] || fail "mutant $mu not killed"
  # kills must be assertion failures in the test, not panics from src/
  srcp=$(grep -cE "panicked at src/" "$L/$name.log" || true); echo "mutant_${mu}_src_panics=$srcp"
  [ "$srcp" -eq 0 ] || fail "mutant $mu killed by a src panic"
  case "$killed" in *control_change_changes_subject_pixels*) ;; *) fail "mutant $mu not killed by the control-change test";; esac
done

run clippy $T clippy --features figure --lib --tests --examples -- -D warnings; tail -5 "$L/clippy.log"; [ "$RC" -eq 0 ] || fail "clippy rc=$RC"
run msrv_figure cargo +1.85 check --lib --features figure; tail -3 "$L/msrv_figure.log"; [ "$RC" -eq 0 ] || fail "msrv rc=$RC"
run default_check $T check --lib; tail -3 "$L/default_check.log"; [ "$RC" -eq 0 ] || fail "default check rc=$RC"

# Example: write the preview PNG (viewed by the session afterwards; this gate does not judge it).
run example $T run --release --features figure --example figure_preview_png -- "$OUT/figure_preview.png" 1280 720
cat "$L/example.log" | tail -3; [ "$RC" -eq 0 ] || fail "example rc=$RC"
[ -s "$OUT/figure_preview.png" ] || fail "no PNG written"
python3 -I - "$OUT/figure_preview.png" <<'PY' || fail "PNG header check"
import struct, sys
b = open(sys.argv[1], 'rb').read()
assert b[:8] == b'\x89PNG\r\n\x1a\n'
w, h = struct.unpack('>II', b[16:24])
print('png', w, h, len(b))
assert w >= 960 and h >= 540
PY

echo "== box at end $(date -u +%H:%M:%SZ)"; cat /proc/pressure/memory; free -m | sed -n "1,3p"
echo "GATE_PREVIEW_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
echo "GATE_PREVIEW=GREEN"
