"""Shared reader for the compacted HumanMusic fixture archive (stdlib only).

Layout (all under docs/fixtures/humanmusic-archive/):
  MANIFEST.tsv    path <TAB> bytes <TAB> sha256 <TAB> storage <TAB> ref <TAB> class
                  storage=archive : bytes live in blobs.tar.xz as member <sha256>
                  storage=inplace : identical bytes live at path `ref` (still in the tree)
  blobs.tar.xz    solid, deterministic tar.xz of the unique compacted contents,
                  one member per distinct sha256, member name == sha256 hex.

Plain-shell equivalent for one blob:  tar -xJOf blobs.tar.xz <sha256> > original
"""
import hashlib
import io
import lzma
import tarfile
from pathlib import Path

ARCHIVE_DIR = "docs/fixtures/humanmusic-archive"
MANIFEST = f"{ARCHIVE_DIR}/MANIFEST.tsv"
BLOBS = f"{ARCHIVE_DIR}/blobs.tar.xz"
HEADER = "path\tbytes\tsha256\tstorage\tref\tclass"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_manifest(root: Path):
    rows = []
    lines = (root / MANIFEST).read_text().splitlines()
    assert lines[0] == HEADER, "unexpected MANIFEST.tsv header"
    for line in lines[1:]:
        path, size, digest, storage, ref, klass = line.split("\t")
        rows.append(dict(path=path, bytes=int(size), sha256=digest,
                         storage=storage, ref=ref, klass=klass))
    return rows


def load_blobs(root: Path, wanted=None):
    """Return {sha256: bytes}; every member is re-hashed against its own name."""
    out = {}
    with tarfile.open(root / BLOBS, "r:xz") as tar:
        for member in tar:
            if wanted is not None and member.name not in wanted:
                continue
            data = tar.extractfile(member).read()
            if sha256(data) != member.name:
                raise SystemExit(f"corrupt archive member {member.name}")
            out[member.name] = data
    return out


def original_bytes(root: Path, path: str, rows=None, blobs=None) -> bytes:
    """Bytes of the ORIGINAL file at `path`, from the tree or (if compacted) the archive."""
    rows = rows if rows is not None else read_manifest(root)
    row = next((r for r in rows if r["path"] == path), None)
    if row is None:
        return (root / path).read_bytes()
    if row["storage"] == "inplace":
        return (root / row["ref"]).read_bytes()
    blobs = blobs if blobs is not None else load_blobs(root, {row["sha256"]})
    return blobs[row["sha256"]]


def ensure_expanded(root: Path, prefix: str) -> int:
    """Materialize every compacted file under `prefix` (repo-relative) in place. Returns count written."""
    rows = [r for r in read_manifest(root) if r["path"].startswith(prefix)]
    need = [r for r in rows if not (root / r["path"]).is_file()]
    if not need:
        return 0
    blobs = load_blobs(root, {r["sha256"] for r in need if r["storage"] == "archive"})
    for r in need:
        data = blobs[r["sha256"]] if r["storage"] == "archive" else (root / r["ref"]).read_bytes()
        assert sha256(data) == r["sha256"], r["path"]
        (root / r["path"]).parent.mkdir(parents=True, exist_ok=True)
        (root / r["path"]).write_bytes(data)
    return len(need)


def write_blobs(dest: Path, blobs: dict, order):
    """Deterministic solid tar.xz: GNU format, sorted by `order`, mtime 0, root owner, xz -9e."""
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.GNU_FORMAT) as tar:
        for name in order:
            info = tarfile.TarInfo(name)
            info.size = len(blobs[name])
            info.mode = 0o644
            info.mtime = 0
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            tar.addfile(info, io.BytesIO(blobs[name]))
    dest.write_bytes(lzma.compress(raw.getvalue(), format=lzma.FORMAT_XZ,
                                   check=lzma.CHECK_CRC64,
                                   preset=9 | lzma.PRESET_EXTREME))
