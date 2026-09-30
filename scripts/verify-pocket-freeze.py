#!/usr/bin/env python3
"""Compare a regenerated pocket lab with committed R17 evidence, without fitting it.

Run pocket_music_lab with --render into a NEW directory, then pass that directory here.
The normal 48 full/stem renders are required. Factorial audio is optional; every
regenerated diagnostic/ledger text receipt is required and compared byte for byte.
"""
import argparse
import hashlib
import difflib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "fixtures"))
import humanmusic_archive as ha  # noqa: E402


def verify(root: Path, generated: Path) -> dict:
    frozen = root / "docs/fixtures/humanmusic-r17/final"
    expected_wavs = {}
    for line in (frozen / "SHA256SUMS").read_text().splitlines():
        digest, filename = line.split(maxsplit=1)
        expected_wavs[Path(filename).name] = digest
    required = [name for name in expected_wavs if any(
        f"_{arm}." in name for arm in ("r14", "r16", "r17")
    )]
    failures = []
    hashes = {}
    for name in required:
        path = generated / name
        if not path.is_file():
            failures.append(f"missing WAV: {name}")
            continue
        digest = hashlib.file_digest(path.open("rb"), "sha256").hexdigest()
        hashes[name] = digest
        if digest != expected_wavs[name]:
            failures.append(f"changed WAV: {name}")
    receipt_count = 0
    expected_receipts = {}
    for line in (frozen / "RECEIPTS_SHA256SUMS").read_text().splitlines():
        digest, filename = line.split("  ", 1)
        expected_receipts[filename] = digest
    rows = blobs = None
    for name, digest in sorted(expected_receipts.items()):
        if name in ("render.txt", "historical-controls.txt"):
            continue
        actual = generated / name
        receipt_count += 1
        if not actual.is_file():
            failures.append(f"changed/missing receipt: {name}")
        elif hashlib.sha256(actual.read_bytes()).hexdigest() != digest:
            failures.append(f"changed/missing receipt: {name}")
            # Show WHAT changed: recover the frozen original (tree, else archive) and diff.
            rel = f"docs/fixtures/humanmusic-r17/final/{name}"
            if rows is None:
                rows = ha.read_manifest(root)
            original = ha.original_bytes(root, rel, rows, blobs)
            diff = list(difflib.unified_diff(
                original.decode(errors="replace").splitlines(),
                actual.read_text(errors="replace").splitlines(),
                "frozen/" + name, "generated/" + name, lineterm="", n=0))
            failures.append(f"  diff head for {name}: " + " | ".join(diff[:6]))
    return {
        "schema": "humanmusic-pocket-freeze/v1",
        "reference_head": "207f0ab092e3d0a274998d84a245e7b78d096f97",
        "generated_directory": str(generated),
        "required_wavs": len(required),
        "compared_receipts": receipt_count,
        "wav_sha256": hashes,
        "failures": failures,
        "pass": not failures,
        "boundary": "Byte equivalence to frozen evidence; not new listening acceptance.",
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("generated", type=Path)
    args = parser.parse_args()
    result = verify(Path(__file__).resolve().parents[1], args.generated)
    print(json.dumps(result, indent=2, sort_keys=True))
    raise SystemExit(0 if result["pass"] else 1)
