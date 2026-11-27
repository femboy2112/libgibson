#!/usr/bin/env bash
# M5-C MEASURE lease instrument: export the body with MPFB, then render the probe, each under
# /usr/bin/time -v, with a Tctl sampler and an 18-minute overall budget.
#
#   BLENDER=/path/to/blender heavy run --class s --max-min 20 -- bash tools/figure_assets/lease_m5c_measure.sh OUT
#
# Environment:
#   BLENDER           (required) the Blender 4.5 binary.  Never written into the repo.
#   MPFB_DATA         MPFB extension data dir      (default: derived from the Blender user config dir)
#   MPFB_USER_DATA    MakeHuman asset pack data dir (default: derived likewise)
#   LEASE_STRICT_LICENCE=1   count a failed licence cross-check (DROP/unknown sources) as a lease failure
#   LEASE_BUDGET_S    overall budget in seconds (default 1080 = 18 min)
#
# Output (under OUT): export/ render/ logs/{licence,export,probe}.log tctl.log licence.json steps.txt
# Prints "STEP <name> rc=<n> seconds=<s> max_rss_kb=<k>" per step, the PROBE lines, TCTL_MAX_C and
# "LEASE_FAILS=<n>".  Exit status is 1 if LEASE_FAILS > 0, 2 on bad usage.
set -u

OUT="${1:-}"
if [ -z "$OUT" ]; then echo "usage: $0 OUT_DIR" >&2; exit 2; fi
if [ -z "${BLENDER:-}" ] || [ ! -x "$BLENDER" ]; then echo "ERROR: BLENDER must point at the Blender binary" >&2; exit 2; fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUDGET="${LEASE_BUDGET_S:-1080}"
CFG="${XDG_CONFIG_HOME:-$HOME/.config}"
BLVER="$("$BLENDER" --version 2>/dev/null | sed -n 's/^Blender \([0-9]*\.[0-9]*\).*/\1/p' | head -n1)"
BLVER="${BLVER:-4.5}"
export MPFB_DATA="${MPFB_DATA:-$CFG/blender/$BLVER/extensions/user_default/mpfb/data}"
export MPFB_USER_DATA="${MPFB_USER_DATA:-$CFG/blender/$BLVER/extensions/.user/user_default/mpfb/data}"

mkdir -p "$OUT/export" "$OUT/render" "$OUT/logs" || { echo "ERROR: cannot create $OUT" >&2; exit 2; }
: > "$OUT/steps.txt"
: > "$OUT/tctl.log"

# ---- Tctl sampler (every 5 s): `sensors` if present, else k10temp temp1_input
read_tctl() {
  local t=""
  if command -v sensors >/dev/null 2>&1; then
    t="$(sensors 2>/dev/null | awk '/^Tctl:/ { gsub(/[+°C]/, "", $2); print $2; exit }')"
  fi
  if [ -z "$t" ]; then
    local d
    for d in /sys/class/hwmon/hwmon*; do
      if [ "$(cat "$d/name" 2>/dev/null)" = "k10temp" ] && [ -r "$d/temp1_input" ]; then
        t="$(awk '{ printf "%.1f", $1 / 1000 }' "$d/temp1_input")"
        break
      fi
    done
  fi
  echo "${t:-NA}"
}
(
  while true; do
    echo "$(date +%s) $(read_tctl)" >> "$OUT/tctl.log"
    sleep 5
  done
) &
SAMPLER=$!
trap 'kill "$SAMPLER" 2>/dev/null; wait "$SAMPLER" 2>/dev/null' EXIT

START="$(date +%s)"
FAILS=0

# run_step NAME LOGFILE MARKER_REGEX CMD...   (MARKER: a line like "EXPORT_RC 0" the script prints before exiting; "-" = none)
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
  if [ "$marker" != "-" ]; then
    mrc="$(sed -n "s/^${marker} \([0-9][0-9]*\)\$/\1/p" "$log" | tail -n1)"
    if [ -z "$mrc" ]; then
      [ "$eff" -eq 0 ] && eff=97   # exited 0 without the script's own result line: it never ran to its end
      echo "NOTE $name: no '$marker' line in the log (crash or script did not run)" | tee -a "$OUT/steps.txt"
    elif [ "$mrc" -ne 0 ]; then
      eff="$mrc"
    fi
  fi
  echo "STEP $name rc=$eff proc_rc=$rc seconds=$((t1 - t0)) max_rss_kb=${rss:-NA}" | tee -a "$OUT/steps.txt"
  if [ "$eff" -ne 0 ]; then FAILS=$((FAILS + 1)); fi
  return 0
}

# 1. licence audit (pure python, also writes the json the export records)
run_step licence_audit "$OUT/logs/licence.log" - \
  python3 -I "$HERE/licence_audit.py" --out-json "$OUT/licence.json"

# 2. export from MPFB
run_step export_mpfb "$OUT/logs/export.log" EXPORT_RC \
  "$BLENDER" -b --python "$HERE/export_mpfb.py" -- --out "$OUT/export" --licence-json "$OUT/licence.json"

# 3. render probe (needs the glTF from step 2)
if [ -f "$OUT/export/body_shaped.glb" ]; then
  run_step render_probe "$OUT/logs/probe.log" PROBE_RC \
    "$BLENDER" -b --python "$HERE/render_probe.py" -- --export "$OUT/export" --out "$OUT/render"
else
  echo "STEP render_probe rc=98 proc_rc=NA seconds=0 max_rss_kb=NA (no export/body_shaped.glb)" | tee -a "$OUT/steps.txt"
  FAILS=$((FAILS + 1))
fi

# 4. licence cross-check of what the export actually read (informational unless strict)
if [ -f "$OUT/export/manifest.json" ]; then
  python3 -I "$HERE/licence_audit.py" --check-export "$OUT/export/manifest.json" > "$OUT/logs/licence_check.log" 2>&1
  lrc=$?
  echo "STEP licence_check rc=$lrc (informational unless LEASE_STRICT_LICENCE=1) $(grep '^LICENCE_CHECK' "$OUT/logs/licence_check.log" | tail -n1)" | tee -a "$OUT/steps.txt"
  if [ "$lrc" -ne 0 ] && [ "${LEASE_STRICT_LICENCE:-0}" = "1" ]; then FAILS=$((FAILS + 1)); fi
fi

# ---- summary
echo "--- PROBE lines"
grep -h '^PROBE \|^PROBE_NOTE\|^XCHECK\|^EXPORT_STEP\|^EXPORT_DONE' "$OUT/logs/export.log" "$OUT/logs/probe.log" 2>/dev/null
TMAX="$(awk '$2 != "NA" && $2 + 0 > m { m = $2 + 0 } END { if (m > 0) printf "%.1f", m; else print "NA" }' "$OUT/tctl.log")"
echo "TCTL_MAX_C=$TMAX samples=$(wc -l < "$OUT/tctl.log")"
echo "ELAPSED_S=$(( $(date +%s) - START )) BUDGET_S=$BUDGET"
echo "LEASE_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
exit 0
