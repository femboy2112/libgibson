#!/usr/bin/env python3
"""Deterministically compact docs/fixtures/humanmusic-* (builder for the archive + manifest).

  compact-humanmusic-fixtures.py plan            # print what would be compacted, touch nothing
  compact-humanmusic-fixtures.py build           # write MANIFEST.tsv, blobs.tar.xz, r17 RECEIPTS_SHA256SUMS
  compact-humanmusic-fixtures.py build --apply   # ...and delete the compacted originals from the tree
  compact-humanmusic-fixtures.py report [--before-rev 5ad153e]   # markdown before/after table

Input must be the EXPANDED tree (run expand-humanmusic-archives.py first if files were already
compacted). The build is a pure function of the file contents, so re-running it on an expanded
tree reproduces the committed archive and manifest byte for byte.

Policy (see docs/fixtures/HUMANMUSIC_FIXTURE_MANIFEST.md for the rationale):
  * PROTECTED, never compacted: holdout v1 (consolidation/), hardening/, the archive itself,
    symbolic sources (cover/ode-import, cover/sources), everything Rust reads (golden set),
    first-contact / minimal-falsifier files, every hash manifest, *.md, *.py, *.gz, *.meta.
  * Eligible text (.txt .tsv .json .log .jsonl) is compacted when >= 16 KiB, or when it is an
    exact duplicate (same sha256 at >= 2 paths, non-empty).
  * Duplicates keep ONE canonical copy: a protected/in-tree member if any (manifest row
    storage=inplace), otherwise a single blob in the archive (storage=archive).
"""
import argparse
import os
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import humanmusic_archive as ha  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
FIX = "docs/fixtures"
RECEIPTS = f"{FIX}/humanmusic-r17/final/RECEIPTS_SHA256SUMS"
UNIQUE_MIN = 16384
DUP_MIN = 1
ELIGIBLE_EXT = (".txt", ".tsv", ".json", ".log", ".jsonl")

PROTECTED_PREFIXES = (
    f"{FIX}/humanmusic-consolidation/",   # includes IMMUTABLE holdout v1 (fresh/)
    f"{FIX}/humanmusic-hardening/",       # IMMUTABLE this round
    f"{FIX}/humanmusic-archive/",
    f"{FIX}/humanmusic-cover/ode-import/",
    f"{FIX}/humanmusic-cover/sources/",
)
# Read by Rust: tests/audio_consolidation_characterization.rs -> r17/final/{world}_r17.{suffix}.txt
GOLDEN_SUFFIXES = ["notes", "drums", "continuity_links", "phrase_plans", "expression_decisions",
                   "occupancy", "hearings", "actions", "pocket", "phrase", "conformance"]
GOLDEN = {f"{FIX}/humanmusic-r17/final/{w}_r17.{s}.txt"
          for w in ("black_ice", "swiss") for s in GOLDEN_SUFFIXES}
# First-contact failures / minimal counterexamples / control sweeps: stay greppable in the tree.
FALSIFIERS = {
    f"{FIX}/humanmusic-r13/baseline-failures.txt",
    f"{FIX}/humanmusic-r14/baseline-failures.txt",
    f"{FIX}/humanmusic-r15/r14-control-sweep.txt",
    f"{FIX}/humanmusic-r16/first-holdout-sweep.txt",
    f"{FIX}/humanmusic-r16/holdout-sweep.txt",
    f"{FIX}/humanmusic-r16/historical-r14-sweep.txt",
    f"{FIX}/humanmusic-r17/diagnostic-failures/",
    f"{FIX}/humanmusic-r17/support-no-go/candidates.tsv",
}


def is_protected(path: str) -> bool:
    name = path.rsplit("/", 1)[-1]
    if path.startswith(PROTECTED_PREFIXES) or path in GOLDEN or path == RECEIPTS:
        return True
    if any(path == f or (f.endswith("/") and path.startswith(f)) for f in FALSIFIERS):
        return True
    if not name.endswith(ELIGIBLE_EXT):
        return True
    return "SHA256SUMS" in name or "manifest" in name.lower()


def tree_files(root: Path):
    out = {}
    for top in sorted((root / FIX).glob("humanmusic-*")):
        for d, _, fs in os.walk(top):
            for f in fs:
                rel = str((Path(d) / f).relative_to(root))
                if rel.startswith(f"{FIX}/humanmusic-archive/") or rel == RECEIPTS:
                    continue
                out[rel] = (Path(d) / f).read_bytes()
    return out


def plan(root: Path):
    files = tree_files(root)
    by_sha = defaultdict(list)
    for p, data in files.items():
        by_sha[ha.sha256(data)].append(p)
    rows, blobs = [], {}
    compact = set()
    for digest, paths in by_sha.items():
        size = len(files[paths[0]])
        dup = len(paths) > 1
        for p in paths:
            if not is_protected(p) and (size >= UNIQUE_MIN or (dup and size >= DUP_MIN)):
                compact.add(p)
    for digest, paths in sorted(by_sha.items(), key=lambda kv: kv[1][0]):
        stay = sorted(p for p in paths if p not in compact)
        canonical = stay[0] if stay else None
        for p in sorted(paths):
            if p not in compact:
                continue
            data = files[p]
            if canonical:
                rows.append((p, len(data), digest, "inplace", canonical, "duplicate"))
            else:
                blobs[digest] = data
                klass = "derived-receipt" if "/humanmusic-r17/final/" in p else "archival"
                if len(paths) > 1:
                    klass += "+dup"
                rows.append((p, len(data), digest, "archive", digest, klass))
    rows.sort()
    return files, rows, blobs


def order_blobs(rows, blobs):
    first = {}
    for p, _, digest, storage, _, _ in rows:
        if storage == "archive" and digest not in first:
            first[digest] = p
    # group similar dumps (same kind of receipt) next to each other for the solid xz stream
    def key(digest):
        name = first[digest].rsplit("/", 1)[-1]
        return (".".join(name.split(".")[-2:]), first[digest])
    return sorted(blobs, key=key)


def receipts_text(files, rows):
    base = f"{FIX}/humanmusic-r17/final/"
    sums = {}
    for p, data in files.items():
        if p.startswith(base) and p.count("/") == base.count("/") and p.endswith(".txt"):
            sums[p[len(base):]] = ha.sha256(data)
    return "".join(f"{d}  {n}\n" for n, d in sorted(sums.items()))


def cmd_build(root: Path, apply: bool):
    files, rows, blobs = plan(root)
    out = root / ha.ARCHIVE_DIR
    out.mkdir(parents=True, exist_ok=True)
    (out / "MANIFEST.tsv").write_text(ha.HEADER + "\n" + "".join("\t".join(map(str, r)) + "\n" for r in rows))
    ha.write_blobs(out / "blobs.tar.xz", blobs, order_blobs(rows, blobs))
    (root / RECEIPTS).write_text(receipts_text(files, rows))
    print(f"{len(rows)} files compacted ({sum(r[1] for r in rows)} bytes), "
          f"{len(blobs)} unique blobs ({sum(map(len, blobs.values()))} bytes) -> "
          f"{(out / 'blobs.tar.xz').stat().st_size} bytes xz")
    if apply:
        for p, *_ in rows:
            (root / p).unlink()
        for d, dirs, fs in os.walk(root / FIX, topdown=False):
            if not dirs and not fs:
                os.rmdir(d)
        print("removed compacted originals from the tree")


def cmd_plan(root: Path):
    _, rows, blobs = plan(root)
    print(f"{len(rows)} files, {sum(r[1] for r in rows)} bytes would leave the tree; "
          f"{len(blobs)} unique blobs ({sum(map(len, blobs.values()))} bytes)")


def cmd_report(root: Path, before_rev: str):
    """Per-directory table: files/bytes at `before_rev` (ls-tree) vs the current working tree."""
    import subprocess
    before = {}
    listing = subprocess.check_output(["git", "ls-tree", "-r", "-l", before_rev, FIX], cwd=root, text=True)
    for line in listing.splitlines():
        meta, path = line.split("\t", 1)
        if path.startswith(f"{FIX}/humanmusic-"):
            before[path] = int(meta.split()[3])
    after = {}
    for top in sorted((root / FIX).glob("humanmusic-*")):
        for d, _, fs in os.walk(top):
            for f in fs:
                after[str((Path(d) / f).relative_to(root))] = (Path(d) / f).stat().st_size
    dirs = defaultdict(lambda: [0, 0, 0, 0])
    for p, b in before.items():
        e = dirs[p.rsplit("/", 1)[0]]
        e[0] += 1
        e[1] += b
    for p, b in after.items():
        e = dirs[p.rsplit("/", 1)[0]]
        e[2] += 1
        e[3] += b
    print("| directory | files before | bytes before | files after | bytes after |")
    print("|---|---:|---:|---:|---:|")
    tot = [0, 0, 0, 0]
    for d in sorted(dirs, key=lambda k: (-dirs[k][1], k)):
        e = dirs[d]
        tot = [a + b for a, b in zip(tot, e)]
        print(f"| `{d.replace(FIX + '/', '')}` | {e[0]} | {e[1]} | {e[2]} | {e[3]} |")
    print(f"| **total** | {tot[0]} | {tot[1]} | {tot[2]} | {tot[3]} |")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", choices=["plan", "build", "report"])
    ap.add_argument("--root", type=Path, default=ROOT)
    ap.add_argument("--apply", action="store_true")
    ap.add_argument("--before-rev", default="5ad153e", help="report: revision holding the pre-compaction tree")
    a = ap.parse_args()
    if a.mode == "plan":
        cmd_plan(a.root)
    elif a.mode == "build":
        cmd_build(a.root, a.apply)
    else:
        cmd_report(a.root, a.before_rev)


if __name__ == "__main__":
    main()
