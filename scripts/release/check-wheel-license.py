#!/usr/bin/env python3
"""Verify a built wheel physically carries the project's dual-license payload.

A distributed wheel must contain the actual license TEXT, not merely list it in
metadata. This checker accepts BOTH spec-valid dist-info license layouts:

  * legacy flat            <pkg>.dist-info/LICENSE
  * PEP 639 / Metadata 2.4 <pkg>.dist-info/licenses/<path>/LICENSE

setuptools >= 77 relocates license-files into the ``licenses/`` subdirectory and
emits ``Metadata-Version: 2.4``; older setuptools used the flat layout. Both are
standards-compliant and both must pass. (The previous cleanroom check only
recognised the flat form, so a wheel built by a modern runner failed its license
gate even though it carried every license file correctly.)

For each required file the checker verifies the bytes inside the wheel are
IDENTICAL to the authoritative repository license text — presence alone is not
accepted, because a file with the right name but wrong contents is not the
license. It also confirms ``METADATA`` still DECLARES each license file via a
``License-File:`` field, tolerating the bare and ``licenses/``-prefixed spellings.

Usage:
    check-wheel-license.py <wheel.whl> <reference-license-dir>

Exit 0 if the payload is correct and complete, 1 otherwise (reasons on stderr),
2 on a usage error.
"""

from __future__ import annotations

import os
import posixpath
import sys
import zipfile

# License files the distributable wheel must physically carry (with matching bytes).
REQUIRED = ["LICENSE", "LICENSE-MIT", "LICENSE-APACHE"]
# License files METADATA must additionally declare as License-File entries.
METADATA_REQUIRED = ["LICENSE-MIT", "LICENSE-APACHE"]


def fail(msg: str) -> None:
    print(f"  [wheel-license] {msg}", file=sys.stderr)


def dist_info_license_entries(names: list[str], basename: str) -> list[str]:
    """Archive entries named ``basename`` inside a ``.dist-info`` tree, in either
    the legacy flat layout or the PEP 639 ``licenses/`` subdirectory layout."""
    hits = []
    for n in names:
        parts = n.split("/")
        # flat: .../<pkg>.dist-info/<basename>
        if len(parts) >= 2 and parts[-2].endswith(".dist-info") and parts[-1] == basename:
            hits.append(n)
            continue
        # PEP 639 / Metadata 2.4: .../<pkg>.dist-info/licenses/.../<basename>
        if ".dist-info/licenses/" in n and posixpath.basename(n) == basename:
            hits.append(n)
    return hits


def main() -> int:
    if len(sys.argv) != 3:
        fail("usage: check-wheel-license.py <wheel.whl> <reference-license-dir>")
        return 2
    whl, refdir = sys.argv[1], sys.argv[2]

    try:
        zf = zipfile.ZipFile(whl)
    except (OSError, zipfile.BadZipFile) as exc:
        fail(f"cannot open wheel {whl}: {exc}")
        return 1

    names = zf.namelist()
    ok = True

    # (1) Each required license file must be present (either layout) AND its bytes
    #     must match the authoritative repository license text.
    for name in REQUIRED:
        hits = dist_info_license_entries(names, name)
        if not hits:
            fail(f"license file not found in wheel dist-info (flat or licenses/): {name}")
            ok = False
            continue
        try:
            with open(os.path.join(refdir, name), "rb") as fh:
                ref = fh.read()
        except OSError as exc:
            fail(f"cannot read reference license {name} from {refdir}: {exc}")
            ok = False
            continue
        if not any(zf.read(h) == ref for h in hits):
            fail(f"license bytes in wheel do not match repository {name} (at {', '.join(hits)})")
            ok = False

    # (2) METADATA must still declare the license files, tolerant of the bare and
    #     the licenses/-prefixed License-File spelling.
    metas = [n for n in names if n.rsplit("/", 1)[-1] == "METADATA"]
    if not metas:
        fail("wheel has no dist-info/METADATA")
        return 1
    body = zf.read(metas[0]).decode("utf-8", "replace")
    declared = {
        posixpath.basename(line.split(":", 1)[1].strip())
        for line in body.splitlines()
        if line.startswith("License-File:")
    }
    for name in METADATA_REQUIRED:
        if name not in declared:
            fail(f"METADATA does not declare License-File for {name}")
            ok = False

    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
