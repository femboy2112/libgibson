#!/usr/bin/env bash
# kitty_key_release_e2e.sh — #275 end to end in REAL terminal emulators, on a private X display.
#
# Runs DEEP SIGNAL (release build) inside kitty and inside xterm on an Xvfb display, types real key
# downs and ups with xdotool (X key events: the emulator does its own encoding), records the game's
# own trace (`--record`), and checks:
#   kitty: the terminal confirmed key releases (`keys releases`); a 2 s hold of w is one press and
#          one release ~70 tics apart; d pressed and released while w stays down is released on its
#          own and w is not re-pressed (the chord autorepeat cannot carry); the HUD shows KEYS EXACT;
#          after the game exits, kitty's keyboard flags are 0 on both the main and the alternate
#          screen (the game's push was popped); the recorded trace replays.
#   xterm: no protocol: the answer is `keys presses`, and no release is recorded.
# With FB_DEMO=path to an fb_adapter_demo built against an SDK with the C ABI opt-in, the same in
# kitty through the C ABI: the answer and the releases are in the demo's input trace, the status
# row shows "keys exact", the trace replays headless to the live run's hash, and the flags are
# popped after exit.
# Not run in CI (needs Xvfb, kitty, xterm, xdotool). Never touches the caller's display.
#
# usage: [FB_DEMO=...] scripts/dev/kitty_key_release_e2e.sh [OUT_DIR]
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${1:-$(mktemp -d)}
mkdir -p "$OUT"
for t in Xvfb kitty xterm xdotool python3; do
    command -v "$t" >/dev/null || { echo "missing $t" >&2; exit 2; }
done
GAME="$ROOT/target/release/examples/deep_signal"
(cd "$ROOT" && cargo build -q --release --example deep_signal)

# kitty's control socket: a Unix socket path must stay under 108 bytes, so not inside OUT.
SOCK_DIR=$(mktemp -d /tmp/kre2e.XXXXXX)
SOCK="$SOCK_DIR/k.sock"
DISP=:$(( 90 + RANDOM % 9 ))
Xvfb "$DISP" -screen 0 1280x900x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
cleanup() { kill "$XVFB" 2>/dev/null || true; rm -rf "$SOCK_DIR"; }
trap cleanup EXIT
sleep 1
export DISPLAY=$DISP
unset WAYLAND_DISPLAY
xset r on
xset r rate 500 30   # the autorepeat of the box this was written on

# After the game: query kitty's flags on the main screen, then on the alternate screen.
cat >"$OUT/flags_after.py" <<'EOF'
import os, sys, termios, tty, select, time
fd = 0; old = termios.tcgetattr(fd); tty.setraw(fd)
out = open(sys.argv[1], "wb")
def ask(seq):
    os.write(1, seq); buf = b""; end = time.time() + 2
    while time.time() < end and buf.count(b"c") < 1:
        r, _, _ = select.select([fd], [], [], 0.1)
        if r: buf += os.read(fd, 1024)
    return buf
try:
    main = ask(b"\x1b[?u\x1b[c")
    alt = ask(b"\x1b[?1049h\x1b[?u\x1b[c"); os.write(1, b"\x1b[?1049l")
    out.write(b"main " + main + b"\nalt " + alt + b"\n")
finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
EOF

wait_file() { for _ in $(seq 150); do [ -e "$1" ] && return 0; sleep 0.1; done; return 1; }
key() { xdotool "$@"; }

run_in() { # run_in TERMINAL NAME : start the game in TERMINAL, recording NAME.trace
    local term=$1 name=$2
    rm -f "$OUT/$name".*
    local prog=${3:-"'$GAME' --record '$OUT/$name.trace' --max-seconds 25"}
    local cmd="$prog; python3 -I '$OUT/flags_after.py' '$OUT/$name.flags'; touch '$OUT/$name.done'"
    if [ "$term" = kitty ]; then
        kitty --config NONE -o allow_remote_control=yes --listen-on "unix:$SOCK" \
            -o initial_window_width=1000 -o initial_window_height=700 \
            sh -c "$cmd" >"$OUT/$name.term.log" 2>&1 &
    else
        xterm -geometry 120x40 -e sh -c "$cmd" >"$OUT/$name.term.log" 2>&1 &
    fi
    sleep 3
    local w; w=$(xdotool search --sync --onlyvisible --class "$term" | head -1)
    xdotool windowfocus --sync "$w" 2>/dev/null || true
    sleep 0.5
}

# ---------------------------------------------------------------- kitty
run_in kitty kitty
key keydown w; sleep 1.0
key keydown d; sleep 0.5; key keyup d    # chord: d while w stays down
sleep 0.5; key keyup w; sleep 0.4
key keydown a; sleep 0.06; key keyup a   # a tap
sleep 0.6
kitty @ --to "unix:$SOCK" get-text >"$OUT/kitty.screen" 2>/dev/null || true
key key q
wait_file "$OUT/kitty.done" || { echo "kitty run did not finish" >&2; exit 1; }

# ---------------------------------------------------------------- xterm
run_in xterm xterm
key keydown w; sleep 0.3; key keyup w; sleep 0.3
key key q
wait_file "$OUT/xterm.done" || { echo "xterm run did not finish" >&2; exit 1; }

# ---------------------------------------------------------------- fb_adapter demo (C ABI)
if [ -n "${FB_DEMO:-}" ]; then
    run_in kitty demo "'$FB_DEMO' --play --exit-after-tics 600 --trace '$OUT/demo.trace' --report '$OUT/demo.report'"
    key keydown Right; sleep 0.8
    key keydown w; sleep 0.4; key keyup w     # w while Right stays down
    sleep 0.4; key keyup Right; sleep 0.3
    key keydown a; sleep 0.06; key keyup a
    sleep 0.6
    kitty @ --to "unix:$OUT/kitty.sock" get-text >"$OUT/demo.screen" 2>/dev/null || true
    key key q
    wait_file "$OUT/demo.done" || { echo "demo run did not finish" >&2; exit 1; }
fi

FB_DEMO=${FB_DEMO:-} python3 -I - "$OUT" "$GAME" <<'EOF'
import sys, subprocess, re
out, game = sys.argv[1], sys.argv[2]
fails = []
def check(ok, what):
    print(("PASS " if ok else "FAIL ") + what)
    if not ok: fails.append(what)
def lines(name):
    return [l.split("\t") for l in open(f"{out}/{name}.trace").read().splitlines()[1:]]
W, D, A = "char:119", "char:100", "char:97"
k = lines("kitty")
check(any(l[1:] == ["keys", "releases"] for l in k), "kitty: the terminal confirmed key releases")
ev = [(int(l[0]), l[1], l[2]) for l in k if l[0] != "end" and l[1] in ("key", "release")]
wp = [t for t, kind, c in ev if kind == "key" and c == W]
wr = [t for t, kind, c in ev if kind == "release" and c == W]
dp = [t for t, kind, c in ev if kind == "key" and c == D]
dr = [t for t, kind, c in ev if kind == "release" and c == D]
check(len(wr) == 1 and len(dr) == 1, f"kitty: one release each of w and d (w {wr}, d {dr})")
if wr and dr and wp and dp:
    held_w = wr[0] - wp[0]
    check(60 <= held_w <= 80, f"kitty: w held {held_w} tics for a 2.0 s hold (70 expected)")
    check(wp[0] < dp[0] < dr[0] < wr[0], "kitty: d pressed and released inside the w hold")
    # In stream order (a last autorepeat of w can share d's tic): no press of w after d's.
    order = [(kind, c) for _, kind, c in ev]
    d_at = order.index(("key", D))
    check(("key", W) not in order[d_at:], f"kitty: w was not re-pressed once d took the autorepeat (last w press tic {wp[-1]}, d press tic {dp[0]})")
    check(10 <= dr[0] - dp[0] <= 25, f"kitty: d held {dr[0]-dp[0]} tics for 0.5 s (17.5 expected)")
ap = [t for t, kind, c in ev if kind == "key" and c == A]
ar = [t for t, kind, c in ev if kind == "release" and c == A]
check(len(ap) == 1 and len(ar) == 1 and 0 <= ar[0] - ap[0] <= 5, f"kitty: the tap of a is one press and one release ({ap}, {ar})")
screen = open(f"{out}/kitty.screen", errors="replace").read()
check("KEYS EXACT" in screen, "kitty: the HUD shows KEYS EXACT")
flags = open(f"{out}/kitty.flags", "rb").read()
check(b"main \x1b[?0u" in flags and b"alt \x1b[?0u" in flags, f"kitty: flags popped on both screens after exit: {flags!r}")
r = subprocess.run([game, "--replay", f"{out}/kitty.trace"], capture_output=True, text=True)
check(r.returncode == 0 and "deep_signal replay:" in r.stdout, f"kitty: the trace replays: {r.stdout.strip()}")
x = lines("xterm")
check(any(l[1:] == ["keys", "presses"] for l in x), "xterm: the answer is press-only")
check(not any(len(l) > 1 and l[1] == "release" for l in x), "xterm: no release recorded")
check(any(len(l) > 2 and l[1] == "key" and l[2] == W for l in x), "xterm: the press of w still arrives")
import os
demo = os.environ.get("FB_DEMO")
if demo:
    tr = [l.split() for l in open(f"{out}/demo.trace").read().splitlines()]
    kinds = [int(t[1]) for t in tr]
    check(0x200 in kinds, "demo (C ABI): the answer KEY_RELEASES_ACTIVE is in the input trace")
    rel = [t for t in tr if int(t[1]) & 0x100 and int(t[1]) < 0x200]
    names = {(int(t[1]) & 0xff, int(t[2])) for t in rel}
    check(len(rel) == 3 and (1, ord("w")) in names and (1, ord("a")) in names and (2, 0x1003) in names,
          f"demo (C ABI): one release each of Right, w and a ({len(rel)} release lines)")
    rp = dict(kv.split("=", 1) for kv in open(f"{out}/demo.report").read().split() if "=" in kv)
    r = subprocess.run([demo, "--replay", f"{out}/demo.trace", "--tics", rp["tics"]], capture_output=True, text=True)
    m = re.search(r"hash=([0-9a-f]+)", r.stdout)
    check(m is not None and m.group(1) == rp["hash"],
          f"demo (C ABI): the trace replays to the live hash (live {rp['hash']}, replay {m.group(1) if m else None})")
    check("keys exact" in open(f"{out}/demo.screen", errors="replace").read(), "demo (C ABI): the status row shows keys exact")
    fl = open(f"{out}/demo.flags", "rb").read()
    check(b"main \x1b[?0u" in fl and b"alt \x1b[?0u" in fl, f"demo (C ABI): flags popped on both screens after exit: {fl!r}")
print(f"evidence in {out}")
sys.exit(1 if fails else 0)
EOF
