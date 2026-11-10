#!/usr/bin/env python3
"""Check wrapper focus request bytes on Linux PTYs; this does not certify terminal support."""
import errno
import os
import pty
import re
import select
import subprocess
import sys
import time

EXPECTED = [b"\x1b[?1004h", b"\x1b[?1004l", b"\x1b[?1004h", b"\x1b[?1004l"]
FOCUS = re.compile(rb"\x1b\[\?1004[hl]")


def run(command, mode):
    master, slave = pty.openpty()
    child = None
    try:
        child = subprocess.Popen(command + [mode], stdin=slave, stdout=slave, stderr=subprocess.PIPE,
                                 close_fds=True)
        os.close(slave)
        slave = -1
        output = bytearray()
        deadline = time.monotonic() + 10
        while True:
            left = deadline - time.monotonic()
            if left <= 0:
                child.kill()
                child.communicate()
                raise AssertionError(f"{command[0]} {mode}: timed out")
            readable, _, _ = select.select([master], [], [], min(left, 0.1))
            if readable:
                try:
                    chunk = os.read(master, 65536)
                except OSError as exc:
                    if exc.errno != errno.EIO:
                        raise
                    break  # PTY slave closed
                if not chunk:
                    break
                output.extend(chunk)
                if len(output) > 1024 * 1024:
                    child.kill()
                    child.communicate()
                    raise AssertionError(f"{command[0]} {mode}: excessive output")
        _, err = child.communicate(timeout=max(0.1, deadline - time.monotonic()))
        if child.returncode != 0:
            raise AssertionError(f"{command[0]} {mode}: exit {child.returncode}, stderr={err!r}")
        return FOCUS.findall(output), err
    finally:
        if child is not None and child.poll() is None:
            child.kill()
            child.communicate()
        os.close(master)
        if slave >= 0:
            os.close(slave)


def main():
    if len(sys.argv) != 5:
        raise SystemExit("usage: pty_check.py CPP_CLIENT GO_CLIENT PY_CLIENT PYTHON")
    commands = {"cpp": [sys.argv[1]], "go": [sys.argv[2]], "py": [sys.argv[4], sys.argv[3]]}
    for lang, command in commands.items():
        got, err = run(command, "normal")
        if b"COMPLETE" not in err or got != EXPECTED:
            raise AssertionError((lang, "unexpected focus bytes", got, err))
        mutant, err = run(command, "noop")
        if b"COMPLETE" not in err or mutant == EXPECTED:
            raise AssertionError((lang, "no-op control escaped", mutant, err))
        failed, err = run(command, "io_error")
        if b"EXPECTED_IO_ERROR" not in err or failed:
            raise AssertionError((lang, "I/O error not reported", failed, err))
        print(f"  {lang}: PTY bytes={got!r}; no-op={mutant!r}; ENOSPC=reported: PASS")


if __name__ == "__main__":
    main()
