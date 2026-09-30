#!/usr/bin/env python3
"""Restore compacted HumanMusic fixtures to their original paths and bytes, or verify them.

  expand-humanmusic-archives.py expand [--root REPO] [--dest DIR]
      Write every compacted file (docs/fixtures/humanmusic-archive/MANIFEST.tsv) to its
      original relative path under DIR (default: REPO itself, i.e. in place). Each byte string
      is re-hashed against the manifest before it is written. Existing files with the right
      bytes are left alone; existing files with different bytes are an error.
      Prefer --dest /some/tmp/dir if you only want to grep: expanded files are working
      copies and must not be committed.

  expand-humanmusic-archives.py check [--root REPO] [--expanded]
      Verify every manifest hash: the archive members re-hash to their names, every
      `inplace` ref exists with the recorded sha256, and every compacted path is recoverable.
      With --expanded, additionally require that every compacted path exists in the tree
      with the recorded bytes (what `sha256sum -c` style checks of old manifests need).

Stdlib only (tarfile + lzma). Plain-shell equivalent of the archive part:
  tar -xJf docs/fixtures/humanmusic-archive/blobs.tar.xz -C /some/dir   # members are named by sha256
"""
import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import humanmusic_archive as ha  # noqa: E402


def expand(root: Path, dest: Path) -> int:
    rows = ha.read_manifest(root)
    blobs = ha.load_blobs(root, {r["sha256"] for r in rows if r["storage"] == "archive"})
    wrote = same = 0
    for r in rows:
        data = blobs[r["sha256"]] if r["storage"] == "archive" else (root / r["ref"]).read_bytes()
        if ha.sha256(data) != r["sha256"] or len(data) != r["bytes"]:
            raise SystemExit(f"manifest mismatch for {r['path']}")
        target = dest / r["path"]
        if target.is_file():
            if target.read_bytes() != data:
                raise SystemExit(f"refusing to overwrite differing file: {target}")
            same += 1
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        target.chmod(0o644)
        wrote += 1
    print(f"expanded {wrote} files into {dest} ({same} already present, {len(rows)} in manifest)")
    return 0


def check(root: Path, expanded: bool) -> int:
    rows = ha.read_manifest(root)
    blobs = ha.load_blobs(root)
    problems = []
    used = set()
    for r in rows:
        if r["storage"] == "archive":
            used.add(r["sha256"])
            data = blobs.get(r["sha256"])
            if data is None or len(data) != r["bytes"]:
                problems.append(f"archive missing/short: {r['path']}")
        else:
            ref = root / r["ref"]
            if not ref.is_file() or ha.sha256(ref.read_bytes()) != r["sha256"]:
                problems.append(f"inplace ref bad: {r['path']} -> {r['ref']}")
        if expanded:
            target = root / r["path"]
            if not target.is_file() or ha.sha256(target.read_bytes()) != r["sha256"]:
                problems.append(f"not expanded / wrong bytes: {r['path']}")
    orphan = set(blobs) - used
    if orphan:
        problems.append(f"{len(orphan)} archive members referenced by no manifest row")
    for p in problems[:50]:
        print("FAIL", p)
    print(f"{len(rows)} manifest rows, {len(blobs)} archive blobs, "
          f"{'expanded tree checked, ' if expanded else ''}{len(problems)} problems")
    return 1 if problems else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", choices=["expand", "check"])
    ap.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    ap.add_argument("--dest", type=Path)
    ap.add_argument("--expanded", action="store_true")
    a = ap.parse_args()
    if a.mode == "expand":
        raise SystemExit(expand(a.root, a.dest or a.root))
    raise SystemExit(check(a.root, a.expanded))


if __name__ == "__main__":
    main()
