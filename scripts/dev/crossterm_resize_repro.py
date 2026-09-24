#!/usr/bin/env python3
"""Linux upstream reproducer; builds an isolated Cargo project, never LibGibson.

Exit 1 means a key failed exact delivery, 2 means instrumentation/build failure.
The known Crossterm 0.29.0 default backend is expected to exit 1. No second key
is sent. --out-dir retains small logs, results and the resolved Cargo.lock.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time


def reap(process):
    """Terminate the entire child session, including a possible strace child."""
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(process.pid, sig)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=0.5)
        except subprocess.TimeoutExpired:
            continue
    process.wait(timeout=1)


def bounded(command, cwd, log):
    with log.open("wb") as output:
        process = subprocess.Popen(command, cwd=cwd, stdout=output,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        try:
            if process.wait(timeout=180) != 0:
                raise RuntimeError("Cargo build failed: " + log.read_text()[-6000:])
        finally:
            reap(process)


def trial(binary, output, mode, order, index, trace):
    label = f"{mode}-{order}-{index}"
    logfile, gate = output / f"{label}.log", output / f"{label}.gate"
    logfile.unlink(missing_ok=True)
    gate.unlink(missing_ok=True)
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)
    start = time.monotonic_ns()
    parent = []

    def record(kind, payload):
        parent.append({"seq": len(parent), "elapsed_us": (time.monotonic_ns() - start) // 1000,
                       "kind": kind, "payload": payload})

    def child_setup():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    command = [str(binary), mode, str(logfile), str(gate)]
    if trace:
        command = ["strace", "-f", "-r", "-yy", "-e",
                   "trace=read,write,ioctl,poll,ppoll,epoll_wait,epoll_ctl",
                   "-o", str(output / f"{label}.strace")] + command
    process = None
    try:
        process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave,
                                   preexec_fn=child_setup)
        ready_deadline = time.monotonic() + 2
        while not (logfile.exists() and logfile.read_text().startswith("READY\n")):
            if process.poll() is not None or time.monotonic() >= ready_deadline:
                raise RuntimeError(f"{label}: child READY deadline")
            time.sleep(0.002)
        record("CHILD_READY", mode)
        for operation in order.split("-"):
            if operation == "resize":
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
                record("RESIZE", "80x24")
            else:
                count = os.write(master, b"r")
                if count != 1:
                    raise RuntimeError("short key write")
                record("PTY_WRITE", "r")
            # Let the WINCH handler run before releasing application polling.
            time.sleep(0.005)
        gate.touch()
        record("GATE_RELEASE", "polling may resume")
        process.wait(timeout=3.8)
        record("CHILD_EXIT", process.returncode)
        restored = termios.tcgetattr(slave) == original
        lines = logfile.read_text().splitlines()
        keys = [int(line.split()[-1]) for line in lines if " KEY_BYTE " in line]
        return {"mode": mode, "order": order, "iteration": index, "keys": keys,
                "delivered": keys == [ord("r")], "restored": restored,
                "exit": process.returncode, "parent_trace": parent,
                "tty_nonempty_samples": sum("TTY_BYTES 1" in line for line in lines),
                "decoded": [line for line in lines if " DECODED " in line]}
    finally:
        if process is not None:
            reap(process)
        os.close(master)
        os.close(slave)
        gate.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeat", type=int, default=1, help="trials per mode/order (1..100)")
    parser.add_argument("--strace", action="store_true")
    parser.add_argument("--out-dir", type=Path)
    parser.add_argument("--toolchain", default="1.98.1")
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("this discriminating epoll probe is Linux-only")
    if not 1 <= args.repeat <= 100:
        parser.error("--repeat must be 1..100")
    if args.strace and not shutil.which("strace"):
        parser.error("strace is unavailable; omit --strace")
    with tempfile.TemporaryDirectory(prefix="crossterm-resize-probe-") as temporary:
        project = Path(temporary)
        output = args.out_dir.resolve() if args.out_dir else project / "evidence"
        output.mkdir(parents=True, exist_ok=True)
        (project / "src").mkdir()
        shutil.copyfile(Path(__file__).with_name("event_pressure_control.rs"),
                        project / "src/main.rs")
        (project / "Cargo.toml").write_text(
            '[package]\nname="crossterm-resize-control"\nversion="0.0.0"\n'
            'edition="2021"\npublish=false\n[dependencies]\n'
            'crossterm="=0.29.0"\nmio="=1.2.3"\nlibc="=0.2.189"\n')
        # A scratch project is independent of workspace feature unification.
        bounded(["cargo", f"+{args.toolchain}", "build", "--quiet", "--target-dir",
                 str(project / "target")], project, output / "build.log")
        shutil.copyfile(project / "Cargo.lock", output / "Cargo.lock")
        binary = project / "target/debug/crossterm-resize-control"
        results = []
        for mode in ("raw", "crossterm"):
            for order in ("resize-key", "key-resize"):
                for index in range(args.repeat):
                    result = trial(binary, output, mode, order, index, args.strace)
                    results.append(result)
                    print(json.dumps(result), flush=True)
        (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        if any(r["exit"] != 0 or not r["restored"] for r in results):
            raise RuntimeError("child exit/restoration failed; inspect results")
        return int(any(not r["delivered"] for r in results))


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f"instrument failure: {error}", file=sys.stderr)
        sys.exit(2)
