#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Real-PTY harness for fb_adapter_demo (v0.7 node J prototype).

Runs the interactive demo on a pseudo-terminal it owns, types keys into the master, resizes the
terminal with TIOCSWINSZ, and checks from the outside:

  P3  input flows terminal -> simulation: typed keys change the final state hash; the run's own
      recorded trace, replayed headlessly, reproduces that hash; an unkeyed run's hash equals a
      headless run with no input.
  P5  a mid-run resize changes the rendered geometry (frame size readback, map rebuilds) without
      changing the simulation (hash equals the no-input replay at the same tic).
  P6  termios (tcgetattr on the slave, held by this harness) is identical before and after a
      normal quit, after SIGTERM; after a hangup (master closed, child ignoring SIGHUP) the
      program exits on its own, promptly and without spinning.
  P7  present cost and memory on a real PTY (no headless capture): 2000 frames at several sizes,
      and a real-clock 35 Hz run at each size.

This is an independent route to fb_adapter_demo --selfcheck's P3/P5/P6/P7 (written separately, in
another language, and observing only the terminal and the demo's report file).

usage: pty_harness.py --demo PATH [--control NAME] [--only P3|P5|P6|P7]
Prints `PTY <prop> <name>: PASS|FAIL <evidence>` lines and `pty_harness: OK` (exit 0) when all pass.
"""
import argparse
import fcntl
import os
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

CONTROLS = {
    "p2-wire-wrong-tic": ("P2", "the reference is the replay 5 tics before the live run's last frame"),
    "p3-ignore-input": ("P3", "the producer ignores every key"),
    "p5-resize-steps": ("P5", "mutant adapter: a resize runs an extra producer tic"),
    "p5-stale-map": ("P5", "mutant adapter: the map is not rebuilt on resize"),
    "p6-no-restore": ("P6", "the program exits without gibson_destroy_context"),
    "p6-no-signals": ("P6", "no SIGTERM handler"),
    "p6-retry-io": ("P6", "mutant adapter: GIBSON_ERR_IO from a poll is ignored"),
}


class Pty:
    """A child process on a pty whose master AND slave this harness holds."""

    def __init__(self, argv, cols=80, rows=24, ignore_hup=False, capture=False):
        self.captured = bytearray() if capture else None
        self.master, self.slave = os.openpty()
        self.resize(cols, rows)
        self.before = termios.tcgetattr(self.slave)
        pid = os.fork()
        if pid == 0:  # child
            try:
                os.setsid()
                fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)
                for fd in (0, 1, 2):
                    os.dup2(self.slave, fd)
                if self.slave > 2:
                    os.close(self.slave)
                os.close(self.master)
                signal.signal(signal.SIGHUP, signal.SIG_IGN if ignore_hup else signal.SIG_DFL)
                signal.signal(signal.SIGTERM, signal.SIG_DFL)
                signal.signal(signal.SIGINT, signal.SIG_DFL)
                env = dict(os.environ, TERM="xterm-256color", COLORTERM="truecolor")
                os.execve(argv[0], argv, env)
            finally:
                os._exit(127)
        self.pid = pid
        self.bytes = 0
        self._stop = False
        self._thread = threading.Thread(target=self._drain, daemon=True)
        self._thread.start()

    def _drain(self):
        while not self._stop:
            try:
                ready, _, _ = select.select([self.master], [], [], 0.02)
                if ready:
                    data = os.read(self.master, 1 << 16)
                    if not data:
                        return
                    self.bytes += len(data)
                    if self.captured is not None:
                        self.captured += data
            except OSError:
                return

    def resize(self, cols, rows):
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))

    def is_raw(self):
        return not (termios.tcgetattr(self.slave)[3] & termios.ICANON)

    def wait_raw(self, timeout=8.0):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if self.bytes > 2000 and self.is_raw():
                return True
            time.sleep(0.02)
        return False

    def send(self, data):
        os.write(self.master, data)

    def wait_exit(self, timeout):
        t0 = time.monotonic()
        while time.monotonic() - t0 < timeout:
            pid, status, ru = os.wait4(self.pid, os.WNOHANG)
            if pid == self.pid:
                return status, ru, time.monotonic() - t0, False
            time.sleep(0.01)
        os.kill(self.pid, signal.SIGKILL)
        _, status, ru = os.wait4(self.pid, 0)
        return status, ru, time.monotonic() - t0, True

    def close_master(self):
        self._stop = True
        self._thread.join()
        os.close(self.master)
        self.master = -1

    def close(self):
        self._stop = True
        self._thread.join()
        if self.master >= 0:
            os.close(self.master)
        os.close(self.slave)


def parse_report(path):
    """key=value tokens of every line of a report file, merged."""
    out = {}
    try:
        with open(path) as f:
            for line in f:
                for tok in line.split():
                    if "=" in tok:
                        k, v = tok.split("=", 1)
                        out[k] = v
    except OSError:
        pass
    return out


def describe(status):
    if os.WIFSIGNALED(status):
        return "signal %d" % os.WTERMSIG(status)
    return "code %d" % os.WEXITSTATUS(status)


class Harness:
    def __init__(self, demo, control, tmp):
        self.demo = demo
        self.control = control
        self.tmp = tmp
        self.n = 0

    def path(self, stem):
        self.n += 1
        return os.path.join(self.tmp, "%s_%d" % (stem, self.n))

    def session(self, tics, extra=(), schedule=(), cols=80, rows=24, capture=False):
        """Runs the demo for `tics` with a timed schedule of keys / resizes; returns (report, trace, info)."""
        rep, trace = self.path("report"), self.path("trace")
        argv = [self.demo, "--play", "--exit-after-tics", str(tics), "--report", rep, "--trace", trace] + list(extra)
        p = Pty(argv, cols, rows, capture=capture)
        raw = p.wait_raw()
        t0 = time.monotonic()
        for delay, action in schedule:
            while time.monotonic() - t0 < delay:
                time.sleep(0.005)
            if action[0] == "keys":
                p.send(action[1])
            else:
                p.resize(action[1], action[2])
        status, ru, secs, killed = p.wait_exit(tics / 35.0 + 15.0)
        p.close()
        return parse_report(rep), trace, {"raw": raw, "status": status, "killed": killed, "bytes": p.bytes,
                                          "captured": p.captured}

    def replay(self, trace, tics):
        r = subprocess.run([self.demo, "--replay", trace, "--tics", str(tics)], capture_output=True, text=True, timeout=120)
        out = {}
        for tok in r.stdout.split():
            if "=" in tok:
                k, v = tok.split("=", 1)
                out[k] = v
        return out

    # ------------------------------------------------------------------ P2 on the wire
    def p2(self):
        """The emitted ANSI, parsed by an independent VT emulator (pyte), equals the headless readback."""
        try:
            import pyte
        except ImportError:
            return None, "SKIP: pyte is not installed (the readback route, selfcheck P2, still holds)"
        ok, evs = True, []
        tics = 70
        for cols, rows in [(80, 24), (40, 15)]:
            sched = [(0.30, ("keys", b"\x1b[C")), (0.80, ("keys", b" "))]
            rep, trace, info = self.session(tics, (), sched, cols, rows, capture=True)
            data = bytes(info["captured"])
            cut = data.rfind(b"\x1b[?1049l")  # stop at the restore (leaving the alternate screen)
            screen = pyte.Screen(cols, rows)
            pyte.ByteStream(screen).feed(data[:cut] if cut > 0 else data)
            dump = self.path("cells")
            ref_tics = tics - 5 if self.control == "p2-wire-wrong-tic" else tics
            rr = subprocess.run([self.demo, "--replay", trace, "--tics", str(ref_tics), "--grid", "%dx%d" % (cols, rows),
                                 "--dump-cells", dump], capture_output=True, text=True, timeout=120)
            want = []
            try:
                with open(dump) as f:
                    head = f.readline().split()
                    want = [line.split() for line in f]
            except OSError:
                head = []
            bad, first, img, bars = 0, "", 0, 0
            if head != [str(cols), str(rows)] or len(want) != cols * rows or rr.returncode != 0:
                bad, first = -1, "replay dump failed: %s" % rr.stdout.strip()
            else:
                for y in range(rows - 1):  # the status row differs by design (live fps vs replay)
                    for x in range(cols):
                        kind, fg, bg = want[y * cols + x]
                        c = screen.buffer[y][x]
                        if kind == "H":
                            good = c.data == "\u2580" and c.fg == fg and c.bg == bg
                            img += 1
                        else:
                            good = c.data == " " and c.bg == bg
                            bars += 1
                        if not good:
                            if not bad:
                                first = "(%d,%d) wire %r fg %s bg %s, readback %s fg %s bg %s" % (
                                    x, y, c.data, c.fg, c.bg, kind, fg, bg)
                            bad += 1
            same = rep.get("hash") is not None and ("hash=" + rep.get("hash", "")) in rr.stdout
            ok = ok and bad == 0 and same and info["raw"] and rep.get("tics") == str(tics)
            evs.append("%dx%d: %d image + %d bar cells of the last frame on the wire (%d bytes) vs headless readback of "
                       "the replayed trace at tic %d: %s mismatches%s; hash %s" % (
                           cols, rows, img, bars, len(data), ref_tics, bad, (" first " + first) if first else "",
                           "match" if same else "MISMATCH"))
        try:
            from importlib.metadata import version
            pv = version("pyte")
        except Exception:
            pv = "?"
        return ok, "; ".join(evs) + " (pyte %s)" % pv

    # ------------------------------------------------------------------ P3
    def p3(self):
        tics = 140
        keys = [(0.30, ("keys", b"\x1b[C")), (0.36, ("keys", b"\x1b[C")), (0.42, ("keys", b"\x1b[C")),
                (1.00, ("keys", b"w")), (1.50, ("keys", b" ")), (2.00, ("keys", b"a")), (2.05, ("keys", b"a"))]
        extra = ["--control-ignore-input"] if self.control == "p3-ignore-input" else []
        keyed, ktrace, kinfo = self.session(tics, extra, keys)
        quiet, qtrace, qinfo = self.session(tics, extra)
        kr = self.replay(ktrace, tics)
        qr = self.replay(qtrace, tics)
        ok = (kinfo["raw"] and qinfo["raw"] and not kinfo["killed"] and not qinfo["killed"]
              and keyed.get("exit") == "tic_limit" and quiet.get("exit") == "tic_limit"
              and keyed.get("tics") == str(tics) and quiet.get("tics") == str(tics)
              and int(keyed.get("keys_observed", "0")) >= 7
              and keyed.get("hash") != quiet.get("hash")
              and kr.get("hash") == keyed.get("hash") and qr.get("hash") == quiet.get("hash"))
        ev = ("typed 3x Right, w, space, 2x a into the pty: keyed hash=%s %s unkeyed hash=%s (%s tics each, exit %s/%s); "
              "keys observed %s, delivered %s; keyed trace replayed headless: hash=%s (%s); unkeyed replay hash=%s (%s)" % (
                  keyed.get("hash"), "!=" if keyed.get("hash") != quiet.get("hash") else "==", quiet.get("hash"),
                  keyed.get("tics"), keyed.get("exit"), quiet.get("exit"), keyed.get("keys_observed"),
                  keyed.get("keys_delivered"), kr.get("hash"), "match" if kr.get("hash") == keyed.get("hash") else "MISMATCH",
                  qr.get("hash"), "match" if qr.get("hash") == quiet.get("hash") else "MISMATCH"))
        return ok, ev

    # ------------------------------------------------------------------ P5
    def p5(self):
        tics = 175
        extra = []
        if self.control == "p5-resize-steps":
            extra = ["--fault", "resize-steps"]
        elif self.control == "p5-stale-map":
            extra = ["--fault", "stale-map"]
        sched = [(1.2, ("resize", 120, 40)), (2.4, ("resize", 40, 15))]
        rep, _, info = self.session(tics, extra, sched)
        empty = self.path("empty")
        open(empty, "w").close()
        ref = self.replay(empty, tics)
        ok = (info["raw"] and not info["killed"] and rep.get("exit") == "tic_limit" and rep.get("tics") == str(tics)
              and rep.get("hash") == ref.get("hash") and rep.get("frame") == "40x15" and rep.get("grid") == "40x15"
              and rep.get("map_builds") == "3" and int(rep.get("resizes", "0")) >= 1)
        ev = ("TIOCSWINSZ 80x24 -> 120x40 (1.2 s) -> 40x15 (2.4 s), no keys: hash=%s %s no-input headless hash=%s at %s tics; "
              "final frame readback %s, map grid %s, map builds %s, resize events %s, frames %s, dropped %s" % (
                  rep.get("hash"), "==" if rep.get("hash") == ref.get("hash") else "!=", ref.get("hash"), rep.get("tics"),
                  rep.get("frame"), rep.get("grid"), rep.get("map_builds"), rep.get("resizes"), rep.get("frames"),
                  rep.get("dropped")))
        return ok, ev

    # ------------------------------------------------------------------ P6
    def p6_case(self, which):
        rep = self.path("p6")
        argv = [self.demo, "--play", "--report", rep, "--exit-after-tics", "3500"]
        if which == "quit" and self.control == "p6-no-restore":
            argv.append("--control-no-restore")
        if which == "sigterm" and self.control == "p6-no-signals":
            argv.append("--control-no-signals")
        if which == "hangup" and self.control == "p6-retry-io":
            argv += ["--fault", "retry-io"]
        p = Pty(argv, 80, 24, ignore_hup=(which == "hangup"))
        before = p.before
        raw = p.wait_raw()
        time.sleep(1.0)  # steady state
        during = termios.tcgetattr(p.slave)
        if which == "quit":
            p.send(b"q")
        elif which == "sigterm":
            os.kill(p.pid, signal.SIGTERM)
        else:
            p.close_master()
        status, ru, secs, killed = p.wait_exit(5.0)
        r = parse_report(rep)
        cpu = ru.ru_utime + ru.ru_stime
        if which == "hangup":
            try:
                termios.tcgetattr(p.slave)
                after = "readable"
            except termios.error as e:
                after = "unreadable (errno %d: every slave fd is hung up)" % e.args[0]
            ok = (raw and during != before and not killed and r.get("exit") == "hangup" and os.WIFEXITED(status)
                  and os.WEXITSTATUS(status) == 0 and secs < 2.0 and cpu < 3.0)
            ev = "hangup: exit=%s (%s) %.2fs after the master closed, child cpu %.2fs, termios after %s" % (
                r.get("exit", "(no report)"), describe(status), secs, cpu, after)
        else:
            after = termios.tcgetattr(p.slave)
            restored = after == before
            if which == "quit":
                exit_ok = r.get("exit") == "quit" and os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0
            else:
                exit_ok = r.get("exit") == "signal" and os.WIFSIGNALED(status) and os.WTERMSIG(status) == signal.SIGTERM
            ok = raw and during != before and not killed and restored and exit_ok
            ev = "%s: termios after %s before (raw during run: %s), exit=%s (%s) in %.2fs after %s tics" % (
                which, "==" if restored else "!=", "yes" if during != before else "no", r.get("exit", "(no report)"),
                describe(status), secs, r.get("tics", "?"))
        p.close()
        return ok, ev

    def p6(self):
        cases = ["quit", "sigterm", "hangup"]
        if self.control == "p6-no-restore":
            cases = ["quit"]
        elif self.control == "p6-no-signals":
            cases = ["sigterm"]
        elif self.control == "p6-retry-io":
            cases = ["hangup"]
        ok, evs = True, []
        for c in cases:
            o, e = self.p6_case(c)
            ok = ok and o
            evs.append(e)
        return ok, "; ".join(evs)

    # ------------------------------------------------------------------ P7
    def p7(self):
        ok, evs = True, []
        for cols, rows in [(80, 24), (160, 50), (200, 60), (320, 100)]:
            rep = self.path("bench")
            argv = [self.demo, "--play", "--bench-frames", "2000", "--report", rep]
            p = Pty(argv, cols, rows)
            status, ru, secs, killed = p.wait_exit(300.0)
            p.close()
            bench_bytes = p.bytes
            r = parse_report(rep)
            try:
                growth = int(r["hwm_end_kb"]) - int(r["hwm_warm_kb"])
            except (KeyError, ValueError):
                growth = None
            size_ok = (not killed and r.get("bench_frames") == "2000" and r.get("frame") == "%dx%d" % (cols, rows)
                       and growth is not None and growth <= 1024)
            ok = ok and size_ok
            # a real-clock run at the same size: does the loop hold 35 Hz?
            live, _, info = self.session(175, cols=cols, rows=rows)
            try:
                fps = int(live["frames"]) / (float(live["wall_ms"]) / 1000.0)
            except (KeyError, ValueError, ZeroDivisionError):
                fps = 0.0
            ok = ok and live.get("exit") == "tic_limit" and not info["killed"]
            evs.append("%dx%d: bench 2000 frames, %.0f KB/frame on the pty: lower mean %s p99 %s us, render (incl. pty write) mean %s p99 %s us, "
                       "total mean %s p99 %s us (35 Hz budget 28571 us: %s); VmHWM %s->%s KiB (+%s); "
                       "real clock 175 tics: %s frames in %s ms = %.1f fps, dropped %s tics, max %s tics/iteration" % (
                           cols, rows, bench_bytes / 2000.0 / 1000.0, r.get("lower_mean_us"), r.get("lower_p99_us"), r.get("render_mean_us"),
                           r.get("render_p99_us"), r.get("total_mean_us"), r.get("total_p99_us"),
                           "fits" if float(r.get("total_p99_us", "1e9")) < 28571 else "MISSES",
                           r.get("hwm_warm_kb"), r.get("hwm_end_kb"), growth, live.get("frames"), live.get("wall_ms"), fps,
                           live.get("dropped"), live.get("max_tics_in_iteration")))
        return ok, "; ".join(evs) + " (pty drained by this harness as fast as it can read)"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--demo", required=True)
    ap.add_argument("--control", choices=sorted(CONTROLS))
    ap.add_argument("--only", choices=["P2", "P3", "P5", "P6", "P7"])
    a = ap.parse_args()
    demo = os.path.abspath(a.demo)
    props = [("P2", "ansi_wire_equals_readback"), ("P3", "input_to_simulation"), ("P5", "resize_preserves_state"),
             ("P6", "terminal_restoration"), ("P7", "pty_frame_cost_and_memory")]
    if a.control:
        props = [p for p in props if p[0] == CONTROLS[a.control][0]]
    elif a.only:
        props = [p for p in props if p[0] == a.only]
    all_ok = True
    with tempfile.TemporaryDirectory(prefix="fb_pty_harness.") as tmp:
        h = Harness(demo, a.control, tmp)
        for key, name in props:
            ok, ev = getattr(h, key.lower())()
            if ok is None:  # an optional bearing whose tool is missing
                print("PTY %s %s: %s" % (key, name, ev), flush=True)
                continue
            print("PTY %s %s: %s %s" % (key, name, "PASS" if ok else "FAIL", ev), flush=True)
            all_ok = all_ok and ok
    if a.control:
        print("pty_harness: %s (control %s: %s)" % ("OK" if all_ok else "FAIL", a.control, CONTROLS[a.control][1]))
    else:
        print("pty_harness: %s" % ("OK" if all_ok else "FAIL"))
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())
