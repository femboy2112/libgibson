#!/usr/bin/env python3
"""Compile using pkg-config flags without losing quoted paths to shell splitting."""

import shlex
import subprocess
import sys

if len(sys.argv) < 2:
    sys.exit("usage: pkg-config-cc.py COMPILER [COMPILER ARGUMENTS...]")
try:
    flags = subprocess.check_output(
        ["pkg-config", "--cflags", "--libs", "libgibson"], text=True
    )
    sys.exit(subprocess.call(sys.argv[1:] + shlex.split(flags)))
except (OSError, subprocess.CalledProcessError, ValueError) as exc:
    sys.exit(f"pkg-config compiler invocation failed: {exc}")
