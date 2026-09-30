#!/usr/bin/env python3
"""Compare a regenerated pocket lab with committed R17 evidence, without fitting it.

Run pocket_music_lab with --render into a NEW directory, then pass that directory here.
The normal 48 full/stem renders are required. Factorial audio is optional; every
regenerated diagnostic/ledger text receipt is required and compared byte for byte.
"""
import argparse
import hashlib
import json
from pathlib import Path


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
    for path in sorted(frozen.glob("*.txt")):
        if path.name in ("render.txt", "historical-controls.txt"):
            continue
        actual = generated / path.name
        receipt_count += 1
        if not actual.is_file() or actual.read_bytes() != path.read_bytes():
            failures.append(f"changed/missing receipt: {path.name}")
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
