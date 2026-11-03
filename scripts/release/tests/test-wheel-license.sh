#!/usr/bin/env bash
#
# test-wheel-license.sh — red-first fixture for check-wheel-license.py.
#
# Builds one real wheel from bindings/python with a MODERN setuptools (>= 77, the
# PEP 639 / Metadata-2.4 `licenses/` layout that broke the old flat-only checker),
# then repacks it into four fixtures and asserts the checker's verdict on each:
#
#   A. modern correct   (licenses/ layout, matching bytes)  -> PASS
#   B. legacy flat      (dist-info/LICENSE, matching bytes)  -> PASS  (back-compat)
#   C. missing payload  (license files removed, METADATA still lists them) -> FAIL
#   D. wrong bytes      (LICENSE-MIT body replaced)          -> FAIL
#
# The content checks (B/C/D) are what make this more than a presence test: a wheel
# that merely *names* its licenses in metadata, or carries a file with the right
# name but the wrong text, must be rejected.
#
# Requires: python3 with venv + network access to pip-install setuptools/wheel.
# Usage: scripts/release/tests/test-wheel-license.sh

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CHECKER="$REPO_ROOT/scripts/release/check-wheel-license.py"
REF="$REPO_ROOT/bindings/python"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-wheel-license-test.XXXXXX")"
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

fail=0
want_pass() { # <label> <wheel>
    if python3 "$CHECKER" "$2" "$REF" 2>/dev/null; then
        printf '  [PASS] %s (checker accepted, as required)\n' "$1"
    else
        printf '  [FAIL] %s (checker REJECTED a valid wheel)\n' "$1"; fail=1
    fi
}
want_fail() { # <label> <wheel>
    if python3 "$CHECKER" "$2" "$REF" 2>/dev/null; then
        printf '  [FAIL] %s (checker ACCEPTED an invalid wheel)\n' "$1"; fail=1
    else
        printf '  [PASS] %s (checker rejected, as required)\n' "$1"
    fi
}

echo "wheel-license fixture: work dir $WORK"

echo "### build a modern (PEP 639) wheel from bindings/python ###"
pysrc="$WORK/pysrc"; cp -a "$REF" "$pysrc"
rm -rf "$pysrc/build" "$pysrc/dist" "$pysrc"/*.egg-info "$pysrc/__pycache__"
python3 -m venv "$WORK/venv" >/dev/null 2>&1 || { echo "  [FAIL] venv"; exit 1; }
"$WORK/venv/bin/pip" install -q -U "setuptools>=77" wheel >/dev/null 2>&1 \
    || { echo "  [FAIL] pip install setuptools>=77 wheel"; exit 1; }
mkdir -p "$WORK/dist"
( cd "$pysrc" && "$WORK/venv/bin/python" -c \
    "from setuptools import build_meta as b; b.build_wheel('$WORK/dist')" ) >/dev/null 2>&1 \
    || { echo "  [FAIL] build_wheel"; exit 1; }
base="$(ls "$WORK/dist"/*.whl 2>/dev/null | head -1)"
[ -n "$base" ] || { echo "  [FAIL] no wheel produced"; exit 1; }

# Confirm the base really is the modern layout, else the fixture proves nothing.
if ! python3 -c "import zipfile,sys; sys.exit(0 if any('.dist-info/licenses/' in n for n in zipfile.ZipFile(sys.argv[1]).namelist()) else 1)" "$base"; then
    echo "  [FAIL] base wheel is not PEP 639 layout (setuptools too old?) — fixture inconclusive"; exit 1
fi
echo "  built $(basename "$base") (PEP 639 licenses/ layout confirmed)"

# Repack the base wheel into each fixture variant.
repack() { # <mode> <out.whl>
    python3 - "$base" "$2" "$1" <<'PY'
import sys, zipfile, posixpath
src, dst, mode = sys.argv[1], sys.argv[2], sys.argv[3]
LIC = ("LICENSE", "LICENSE-MIT", "LICENSE-APACHE")
zin = zipfile.ZipFile(src)
zout = zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED)
for info in zin.infolist():
    name = info.filename
    data = zin.read(name)
    base = posixpath.basename(name)
    is_lic = ".dist-info/licenses/" in name and base in LIC
    if mode == "flat" and is_lic:
        name = name.split(".dist-info/")[0] + ".dist-info/" + base
    elif mode == "missing" and is_lic:
        continue  # drop the payload; METADATA License-File lines are left intact
    elif mode == "wrong" and is_lic and base == "LICENSE-MIT":
        data = b"NOT THE REAL MIT LICENSE\n"
    zout.writestr(name, data)
zout.close()
PY
}

echo "### fixtures ###"
want_pass "A modern correct" "$base"
repack flat    "$WORK/B-flat.whl"    && want_pass "B legacy flat correct" "$WORK/B-flat.whl"
repack missing "$WORK/C-missing.whl" && want_fail "C missing payload"     "$WORK/C-missing.whl"
repack wrong   "$WORK/D-wrong.whl"   && want_fail "D wrong license bytes"  "$WORK/D-wrong.whl"

echo
if [ "$fail" -ne 0 ]; then echo "wheel-license fixture: FAIL"; exit 1; fi
echo "wheel-license fixture: ALL CASES OK"
