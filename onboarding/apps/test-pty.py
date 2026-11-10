#!/usr/bin/env python3
"""Linux/Unix PTY smoke for the two interactive example applications."""
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time

HERE = Path(__file__).resolve().parent


def run(name, initial, action, response, quit_key):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 40, 0, 0))
    before = termios.tcgetattr(slave)
    env = dict(os.environ, TERM="xterm-256color")
    proc = subprocess.Popen([sys.executable, str(HERE / name / "app.py")], stdin=slave,
                            stdout=slave, stderr=slave, env=env, start_new_session=True)
    data = bytearray()

    def wait_for(*needles):
        deadline = time.monotonic() + 5
        start = len(data)
        while time.monotonic() < deadline:
            # Diff rendering may place cursor-control bytes between words. Require
            # each marker in order after the previous one, not merely anywhere in
            # the output (which could mistake the echoed input for the response).
            cursor = start
            for needle in needles:
                found = data.find(needle, cursor)
                if found < 0:
                    break
                cursor = found + len(needle)
            else:
                return
            if proc.poll() is not None:
                raise AssertionError(f"{name}: exited {proc.returncode}, output={bytes(data)!r}")
            ready, _, _ = select.select([master], [], [], 0.1)
            if ready:
                data.extend(os.read(master, 65536))
        raise AssertionError(f"{name}: missing {needles!r}, output={bytes(data)!r}")

    try:
        wait_for(initial)
        # The first frame can precede the first poll, which is where raw mode
        # starts. Observe the termios transition before sending input.
        deadline = time.monotonic() + 5
        while termios.tcgetattr(slave)[3] & termios.ICANON:
            if proc.poll() is not None or time.monotonic() >= deadline:
                raise AssertionError(f"{name}: raw mode not enabled")
            time.sleep(0.01)
        os.write(master, action)
        wait_for(*response)
        # Require a fresh header after the live terminal changes size; the
        # headless tests separately inspect the resulting frame geometry.
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 8, 30, 0, 0))
        os.kill(proc.pid, signal.SIGWINCH)
        wait_for(initial)
        os.write(master, quit_key)
        proc.wait(timeout=5)
        assert proc.returncode == 0, f"{name}: exit {proc.returncode}, output={bytes(data)!r}"
        assert termios.tcgetattr(slave)[3] == before[3], f"{name}: terminal flags not restored"
        print(f"{name}: real PTY input, response, resize liveness, exit and termios restore OK")
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait()
        os.close(master)
        os.close(slave)


STARTUP_CHILD = r'''
import os, runpy, sys
path, gate_fd, first_key, attr, expected, after_attr, after_expected = sys.argv[1:]
gate_fd = int(gate_fd)
ns = runpy.run_path(path, run_name="startup_probe")
original_poll = ns["Context"].poll_event
first = True
def poll_with_first_event(self, *args, **kwargs):
    global first
    event = original_poll(self, *args, **kwargs)
    if first:
        first = False
        if event.event_type != ns["EventType"].NONE:
            raise AssertionError("unexpected real event during first poll")
        return ns["Event"].char(first_key)
    return event
ns["Context"].poll_event = poll_with_first_event
main = ns["main"]
app_globals = main.__globals__
original_paint = app_globals["paint"]
gated = False
reported = False
def gated_paint(ctx, app):
    global gated, reported
    if not gated:
        gated = True
        os.write(2, b"\nSTARTUP_GATE\n")
        if os.read(gate_fd, 1) != b"1":
            raise RuntimeError("startup gate closed")
        if str(getattr(app, attr)) != expected:
            raise AssertionError("the first polled event was not applied before painting")
    elif not reported and str(getattr(app, after_attr)) == after_expected:
        reported = True
        os.write(2, b"\nSTARTUP_TYPEAHEAD_APPLIED\n")
    return original_paint(ctx, app)
app_globals["paint"] = gated_paint
sys.argv = [path]
sys.exit(main())
'''


def startup_typeahead(name, first_key, attr, expected, action, after_attr, after_expected, quit_key):
    """Gate the first paint, then queue real input; inject one first-poll event for its oracle."""
    master, slave = pty.openpty()
    gate_read, gate_write = os.pipe()
    before = termios.tcgetattr(slave)
    proc = None
    data = bytearray()
    try:
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 40, 0, 0))
        env = dict(os.environ, TERM="xterm-256color")
        proc = subprocess.Popen(
            [sys.executable, "-c", STARTUP_CHILD, str(HERE / name / "app.py"),
             str(gate_read), first_key, attr, expected, after_attr, after_expected],
            stdin=slave, stdout=slave, stderr=slave, env=env, pass_fds=(gate_read,),
            start_new_session=True,
        )
        os.close(gate_read)
        gate_read = -1
        deadline = time.monotonic() + 5

        def collect_until(needles, start=0):
            while time.monotonic() < deadline:
                cursor = start
                for needle in needles:
                    found = data.find(needle, cursor)
                    if found < 0:
                        break
                    cursor = found + len(needle)
                else:
                    return
                if proc.poll() is not None:
                    raise AssertionError(f"{name}: startup exited {proc.returncode}: {bytes(data)!r}")
                ready, _, _ = select.select([master], [], [], 0.1)
                if ready:
                    data.extend(os.read(master, 65536))
            raise AssertionError(f"{name}: startup missing {needles!r}: {bytes(data)!r}")

        collect_until((b"STARTUP_GATE",))
        if termios.tcgetattr(slave)[3] & (termios.ICANON | termios.ECHO):
            raise AssertionError(f"{name}: first paint was reached before raw mode")
        # No application code can run until the gate is released. The real input is therefore
        # queued before the first frame, after the real public poll initialized the terminal.
        os.write(master, action)
        start = len(data)
        os.write(gate_write, b"1")
        collect_until((b"STARTUP_TYPEAHEAD_APPLIED",), start)
        os.write(master, quit_key)
        proc.wait(timeout=5)
        if proc.returncode != 0 or termios.tcgetattr(slave)[3] != before[3]:
            raise AssertionError(f"{name}: startup exit/restore failed: {bytes(data)!r}")
        print(f"{name}: first event preserved, raw before paint, queued input delivered OK")
    finally:
        if proc is not None and proc.poll() is None:
            proc.kill()
            proc.wait()
        if gate_read >= 0:
            os.close(gate_read)
        os.close(gate_write)
        os.close(master)
        os.close(slave)


def reject_dumb(name):
    master, slave = pty.openpty()
    proc = None
    try:
        env = dict(os.environ, TERM="dumb")
        proc = subprocess.Popen([sys.executable, str(HERE / name / "app.py")],
                                stdin=slave, stdout=slave, stderr=slave, env=env)
        proc.wait(timeout=5)
        output = os.read(master, 65536)
        assert proc.returncode == 2 and b"TERM=dumb is unsupported" in output, (name, output)
        print(f"{name}: TERM=dumb rejected with diagnostic OK")
    finally:
        if proc is not None and proc.poll() is None:
            proc.kill()
            proc.wait()
        os.close(master)
        os.close(slave)


startup_typeahead("chat-console", "X", "draft", "X", b"hi\r", "messages", "['You: Xhi', 'Echo (local): Xhi']", b"\x1b")
startup_typeahead("status-dashboard", "p", "paused", "True", b"p", "paused", "False", b"q")
startup_typeahead("temperature-instrument", "+", "setpoint_tenths", "215", b"-", "setpoint_tenths", "210", b"q")
run("chat-console", b"Offline echo console", b"hi\r", (b"Echo", b"(local):", b"hi"), b"\x1b")
run("status-dashboard", b"Status dashboard", b"p", (b"paused",), b"q")
run("temperature-instrument", b"Temperature instrument", b"+", (b"21.5",), b"q")
reject_dumb("chat-console")
reject_dumb("status-dashboard")
reject_dumb("temperature-instrument")
