#!/usr/bin/env bash
# Runs the M5-A kit on a private Xvfb (never touches your display). Hard cap 480 s.
# usage: run_kit.sh --example-bin PATH [--out DIR] [--only=stills|strip|all] [--display :N]
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
DISP=":97"; ARGS=()
for a in "$@"; do case "$a" in --display=*) DISP="${a#--display=}";; *) ARGS+=("$a");; esac; done
XPID=""; KPID=""
cleanup() {
  trap - EXIT INT TERM
  [ -n "$KPID" ] && kill "$KPID" 2>/dev/null && wait "$KPID" 2>/dev/null
  pkill -KILL -f "xterm -display $DISP " 2>/dev/null
  [ -n "$XPID" ] && kill "$XPID" 2>/dev/null && wait "$XPID" 2>/dev/null
  rm -f "/tmp/.X${DISP#:}-lock" 2>/dev/null
}
trap cleanup EXIT INT TERM
Xvfb "$DISP" -screen 0 1280x1024x24 -nolisten tcp >/dev/null 2>&1 &
XPID=$!
sleep 1
kill -0 "$XPID" 2>/dev/null || { echo "Xvfb $DISP failed to start" >&2; exit 1; }
timeout -s TERM 480 python3 "$HERE/make_kit.py" --display "$DISP" "${ARGS[@]}" &
KPID=$!
wait "$KPID"; RC=$?
KPID=""
exit "$RC"
