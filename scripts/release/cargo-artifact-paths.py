#!/usr/bin/env python3
"""Extract this checkout's native-library paths from Cargo's JSON build messages."""
import json
import os
from pathlib import Path
import sys

if len(sys.argv) != 3:
    raise SystemExit("usage: cargo-artifact-paths.py BUILD_MESSAGES ROOT")
want = os.path.realpath(os.path.join(sys.argv[2], "src", "lib.rs"))
found = {"libgibson.so": set(), "libgibson.a": set()}
with open(sys.argv[1], encoding="utf-8") as messages:
    for line in messages:
        try:
            record = json.loads(line)
        except ValueError as exc:
            raise SystemExit(f"invalid Cargo JSON build message: {exc}")
        target = record.get("target", {})
        if (record.get("reason") != "compiler-artifact" or target.get("name") != "gibson"
                or os.path.realpath(target.get("src_path", "")) != want):
            continue
        for path in record.get("filenames", []):
            name = Path(path).name
            if name in found:
                found[name].add(path)
for name, paths in found.items():
    if len(paths) != 1:
        raise SystemExit(f"expected exactly one Cargo artifact {name}; got {sorted(paths)!r}")
    path = next(iter(paths))
    if not os.path.isfile(path):
        raise SystemExit(f"Cargo artifact missing: {path}")
    sys.stdout.buffer.write(os.fsencode(path) + b"\0")
