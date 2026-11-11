#!/usr/bin/env python3
"""Reject Git index flags that can hide tracked modifications from git status."""
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
result = subprocess.run(
    ["git", "ls-files", "-v", "-z"], cwd=root, stdout=subprocess.PIPE
)
if result.returncode:
    print("release index check: git ls-files failed", file=sys.stderr)
    sys.exit(1)
hidden = []
for entry in result.stdout.split(b"\0"):
    if not entry:
        continue
    if len(entry) < 3 or entry[1:2] != b" ":
        print("release index check: malformed git ls-files output", file=sys.stderr)
        sys.exit(1)
    tag = entry[0]
    # Lowercase -v tags indicate assume-unchanged. S/s indicates skip-worktree.
    if tag == ord("S") or ord("a") <= tag <= ord("z"):
        hidden.append((chr(tag), os.fsdecode(entry[2:])))
if hidden:
    print("release index check: unsafe index flags (assume-unchanged/skip-worktree):", file=sys.stderr)
    for tag, path in hidden:
        print(f"  {tag} {path!r}", file=sys.stderr)
    sys.exit(1)
print("release index check: no assume-unchanged or skip-worktree entries")
