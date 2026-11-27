#!/usr/bin/env bash
# M5-C ADAPTER SMOKE lease (budget <= 10 minutes): pure self tests, export of the generic_neutral body, a synthetic test tape (arms rise,
# thighs swing, skirt), then render_tape.py renders 1 still + 12 frames WITH the self-checks, and the `bad_bounds` mutant must FAIL them.
#
#   BLENDER=/path/to/blender heavy run --class s --max-min 12 -- bash tools/figure_assets/lease_m5c_adapter_smoke.sh OUT
#
# Environment: BLENDER (required), MPFB_DATA, MPFB_USER_DATA (default: derived from the Blender user config dir),
#   LEASE_BUDGET_S (default 600), SMOKE_REUSE_EXPORT=1 (skip the export when OUT/export/manifest.json exists),
#   SMOKE_PROBE=1 (also run render_probe.py --checks-only, normal and mutant; needs the glTF, adds ~1 min), SMOKE_SPP (default 32).
# Output under OUT: export/ tape/ render/ mutant/ logs/ steps.txt.  Prints "STEP name rc=.. seconds=.. max_rss_kb=.." per step, the
# RT_CHECK lines, "SMOKE_RESULT ..." lines, and "SMOKE_FAILS=<n>".  Exit 1 if SMOKE_FAILS > 0, 2 on bad usage.
# The pass condition of the whole lease: normal render rc 0 with every check PASS; mutant render rc 5 with `RT_CHECK FAIL contact` and
# `RT_CHECK FAIL shadow` present (a must-FAIL control: if the mutant passes, the checks are blind and the lease fails).
set -u
OUT="${1:-}"
if [ -z "$OUT" ]; then echo "usage: $0 OUT_DIR" >&2; exit 2; fi
if [ -z "${BLENDER:-}" ] || [ ! -x "$BLENDER" ]; then echo "ERROR: BLENDER must point at the Blender binary" >&2; exit 2; fi
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUDGET="${LEASE_BUDGET_S:-600}"
SPP="${SMOKE_SPP:-32}"
CFG="${XDG_CONFIG_HOME:-$HOME/.config}"
BLVER="$("$BLENDER" --version 2>/dev/null | sed -n 's/^Blender \([0-9]*\.[0-9]*\).*/\1/p' | head -n1)"
BLVER="${BLVER:-4.5}"
export MPFB_DATA="${MPFB_DATA:-$CFG/blender/$BLVER/extensions/user_default/mpfb/data}"
export MPFB_USER_DATA="${MPFB_USER_DATA:-$CFG/blender/$BLVER/extensions/.user/user_default/mpfb/data}"
PRESET="$HERE/presets/generic_neutral.json"
mkdir -p "$OUT/logs" "$OUT/export" "$OUT/tape" "$OUT/render" "$OUT/mutant" || { echo "ERROR: cannot create $OUT" >&2; exit 2; }
: > "$OUT/steps.txt"
START="$(date +%s)"
FAILS=0

# run_step NAME LOGFILE MARKER CMD...   (MARKER: a line "<MARKER> <n>" the script prints before exiting; "-" = none)
run_step() {
  local name="$1" log="$2" marker="$3"; shift 3
  local now rem rc t0 t1 rss mrc eff
  now="$(date +%s)"; rem=$(( BUDGET - (now - START) ))
  if [ "$rem" -le 5 ]; then
    echo "STEP $name rc=124 seconds=0 max_rss_kb=NA (budget exhausted before start)" | tee -a "$OUT/steps.txt"
    FAILS=$((FAILS + 1)); return 0
  fi
  t0="$(date +%s)"
  if [ -x /usr/bin/time ]; then
    timeout -k 15 "$rem" /usr/bin/time -v -o "$log.time" "$@" > "$log" 2>&1
  else
    timeout -k 15 "$rem" "$@" > "$log" 2>&1
  fi
  rc=$?
  t1="$(date +%s)"
  rss="$(sed -n 's/.*Maximum resident set size (kbytes): *\([0-9]*\).*/\1/p' "$log.time" 2>/dev/null | head -n1)"
  eff="$rc"
  LAST_MARKER_RC=""
  if [ "$marker" != "-" ]; then
    mrc="$(sed -n "s/^${marker} \([0-9][0-9]*\)\$/\1/p" "$log" | tail -n1)"
    LAST_MARKER_RC="$mrc"
    if [ -z "$mrc" ]; then
      [ "$eff" -eq 0 ] && eff=97
      echo "NOTE $name: no '$marker' line in the log (crash or script did not run)" | tee -a "$OUT/steps.txt"
    else
      eff="$mrc"
    fi
  fi
  LAST_EFF="$eff"
  echo "STEP $name rc=$eff proc_rc=$rc seconds=$((t1 - t0)) max_rss_kb=${rss:-NA}" | tee -a "$OUT/steps.txt"
  return 0
}
need_ok() { if [ "$LAST_EFF" -ne 0 ]; then FAILS=$((FAILS + 1)); fi; }

# 1. pure self tests (python3 -I, no Blender)
for t in fa_tape fa_png fa_checks fa_skirt fa_hair make_test_tape; do
  run_step "selftest_$t" "$OUT/logs/selftest_$t.log" - python3 -I "$HERE/$t.py" --selftest; need_ok
done
run_step selftest_fa_selftest "$OUT/logs/selftest_fa_selftest.log" - python3 -I "$HERE/fa_selftest.py" --mpfb-data "$MPFB_DATA"; need_ok
for t in fa_tape fa_png fa_checks fa_skirt fa_hair make_test_tape fa_selftest; do
  if ! grep -q '^SELFTEST_FAILS 0$' "$OUT/logs/selftest_$t.log" 2>/dev/null; then echo "SMOKE_RESULT selftest_$t FAIL"; FAILS=$((FAILS + 1)); fi
done

# 2. licence audit json + export of the generic_neutral body (the adapter reads attachments, deltas, skeleton, skin weights, landmarks)
run_step licence_audit "$OUT/logs/licence.log" - python3 -I "$HERE/licence_audit.py" --out-json "$OUT/licence.json"; need_ok
if [ "${SMOKE_REUSE_EXPORT:-0}" = "1" ] && [ -f "$OUT/export/manifest.json" ]; then
  echo "STEP export_mpfb rc=0 (reused $OUT/export)" | tee -a "$OUT/steps.txt"
else
  EXTRA="--skip-gltf"; [ "${SMOKE_PROBE:-0}" = "1" ] && EXTRA=""
  run_step export_mpfb "$OUT/logs/export.log" EXPORT_RC \
    "$BLENDER" -b --python "$HERE/export_mpfb.py" -- --out "$OUT/export" --preset "$PRESET" --licence-json "$OUT/licence.json" $EXTRA
  if [ "$LAST_EFF" -ne 0 ] && [ "$LAST_EFF" -ne 4 ]; then FAILS=$((FAILS + 1)); fi   # rc 4 = a built-in XCHECK failed: reported, not fatal here
  [ "$LAST_EFF" -eq 4 ] && echo "SMOKE_RESULT export_xcheck FAIL (rc 4; see logs/export.log XCHECK FAIL lines)"
fi

# 3. synthetic test tape
if [ -f "$OUT/export/figure_asset.json" ]; then
  run_step make_test_tape "$OUT/logs/make_test_tape.log" - python3 -I "$HERE/make_test_tape.py" --export "$OUT/export" --preset "$PRESET" --out "$OUT/tape" --frames 12 --with-skirt; need_ok
else
  echo "STEP make_test_tape rc=98 (no export)" | tee -a "$OUT/steps.txt"; FAILS=$((FAILS + 1))
fi

# 4. the real render with self-checks: 1 still + 12 frames
if [ -f "$OUT/tape/test.tape.json" ]; then
  run_step render_tape "$OUT/logs/render_tape.log" RENDER_TAPE_RC \
    "$BLENDER" -b --python "$HERE/render_tape.py" -- --export "$OUT/export" --tape "$OUT/tape/test.tape.json" --preset "$PRESET" \
      --shots "$HERE/shots/smoke.json" --out "$OUT/render" --spp "$SPP"
  need_ok
  # 5. the must-FAIL control: the probe's bounds bug
  run_step render_tape_mutant "$OUT/logs/render_tape_mutant.log" RENDER_TAPE_RC \
    "$BLENDER" -b --python "$HERE/render_tape.py" -- --export "$OUT/export" --tape "$OUT/tape/test.tape.json" --preset "$PRESET" \
      --shots "$HERE/shots/smoke.json" --out "$OUT/mutant" --spp "$SPP" --mutant bad_bounds --checks-only --no-oracle
  # the mutant step is expected to exit 5 (checks failed); anything else is a lease failure
  MR="$LAST_EFF"
  if [ "$MR" -ne 5 ]; then echo "SMOKE_RESULT mutant_rc FAIL (expected 5, got $MR)"; FAILS=$((FAILS + 1)); else echo "SMOKE_RESULT mutant_rc ok (5)"; fi
  for c in contact shadow; do
    if grep -q "^RT_CHECK FAIL $c " "$OUT/logs/render_tape_mutant.log"; then echo "SMOKE_RESULT mutant_fails_$c ok"; else echo "SMOKE_RESULT mutant_fails_$c FAIL (the $c check did not fail on the bad_bounds mutant: it is blind)"; FAILS=$((FAILS + 1)); fi
    if grep -q "^RT_CHECK PASS $c " "$OUT/logs/render_tape.log"; then echo "SMOKE_RESULT normal_passes_$c ok"; else echo "SMOKE_RESULT normal_passes_$c FAIL"; FAILS=$((FAILS + 1)); fi
  done
  # the exposure must-FAIL control: a blown-out ground
  run_step render_tape_overexposed "$OUT/logs/render_tape_overexposed.log" RENDER_TAPE_RC \
    "$BLENDER" -b --python "$HERE/render_tape.py" -- --export "$OUT/export" --tape "$OUT/tape/test.tape.json" --preset "$PRESET" \
      --shots "$HERE/shots/smoke.json" --out "$OUT/overexposed" --spp "$SPP" --mutant overexposed --checks-only --no-oracle
  if [ "$LAST_EFF" -ne 5 ]; then echo "SMOKE_RESULT overexposed_rc FAIL (expected 5, got $LAST_EFF)"; FAILS=$((FAILS + 1)); else echo "SMOKE_RESULT overexposed_rc ok (5)"; fi
  if grep -q "^RT_CHECK FAIL ground_not_clipped " "$OUT/logs/render_tape_overexposed.log"; then echo "SMOKE_RESULT overexposed_fails_ground_not_clipped ok"; else echo "SMOKE_RESULT overexposed_fails_ground_not_clipped FAIL (the exposure check is blind)"; FAILS=$((FAILS + 1)); fi
  if grep -q "^RT_CHECK PASS ground_not_clipped " "$OUT/logs/render_tape.log"; then echo "SMOKE_RESULT normal_passes_ground_not_clipped ok"; else echo "SMOKE_RESULT normal_passes_ground_not_clipped FAIL"; FAILS=$((FAILS + 1)); fi
  # outputs of the normal run
  NPNG="$(ls "$OUT"/render/frames/smoke/frame_*.png 2>/dev/null | wc -l)"
  NST="$(ls "$OUT"/render/stills/*.png 2>/dev/null | wc -l)"
  [ "$NPNG" -eq 12 ] && [ "$NST" -ge 1 ] && echo "SMOKE_RESULT outputs ok (stills=$NST frames=$NPNG)" || { echo "SMOKE_RESULT outputs FAIL (stills=$NST frames=$NPNG)"; FAILS=$((FAILS + 1)); }
  for f in render/render.json render/oracle_posed.json render/oracle_posed.f32.bin; do
    [ -s "$OUT/$f" ] || { echo "SMOKE_RESULT missing $f FAIL"; FAILS=$((FAILS + 1)); }
  done
  grep -h '^RT_CHECK\|^RT_STEP\|^RT_DONE' "$OUT/logs/render_tape.log" "$OUT/logs/render_tape_mutant.log" "$OUT/logs/render_tape_overexposed.log" 2>/dev/null
else
  echo "STEP render_tape rc=98 (no tape)" | tee -a "$OUT/steps.txt"; FAILS=$((FAILS + 1))
fi

# 6. optional: the probe, normal then mutant (checks only)
if [ "${SMOKE_PROBE:-0}" = "1" ] && [ -f "$OUT/export/body_shaped.glb" ]; then
  run_step probe_checks "$OUT/logs/probe.log" PROBE_RC "$BLENDER" -b --python "$HERE/render_probe.py" -- --export "$OUT/export" --out "$OUT/probe" --checks-only --spp "$SPP"; need_ok
  run_step probe_mutant "$OUT/logs/probe_mutant.log" PROBE_RC "$BLENDER" -b --python "$HERE/render_probe.py" -- --export "$OUT/export" --out "$OUT/probe_mutant" --checks-only --mutant bad_bounds --spp "$SPP"
  if [ "$LAST_EFF" -ne 5 ]; then echo "SMOKE_RESULT probe_mutant_rc FAIL (expected 5, got $LAST_EFF)"; FAILS=$((FAILS + 1)); fi
  for c in contact shadow; do grep -q "^RT_CHECK FAIL $c " "$OUT/logs/probe_mutant.log" || { echo "SMOKE_RESULT probe_mutant_fails_$c FAIL"; FAILS=$((FAILS + 1)); }; done
fi

echo "ELAPSED_S=$(( $(date +%s) - START )) BUDGET_S=$BUDGET"
echo "SMOKE_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
exit 0
