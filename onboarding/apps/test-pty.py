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


run("chat-console", b"Offline echo console", b"hi\r", (b"Echo", b"(local):", b"hi"), b"\x1b")
run("status-dashboard", b"Status dashboard", b"p", (b"paused",), b"q")
run("temperature-instrument", b"Temperature instrument", b"+", (b"21.5",), b"q")
reject_dumb("chat-console")
reject_dumb("status-dashboard")
reject_dumb("temperature-instrument")
