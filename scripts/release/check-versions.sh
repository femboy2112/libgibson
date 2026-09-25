#!/usr/bin/env bash
#
# check-versions.sh — verify the version and ABI constants agree across sources.
#
# Cargo.toml is the canonical package version. The Python wrapper declares its own
# version and ABI constant; the Go wrapper derives its ABI from the C header at
# compile time (so it cannot diverge) and takes its version from the release tag.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
fail=0

cargo_ver="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
py_ver="$(sed -n 's/^version = "\(.*\)"/\1/p' bindings/python/pyproject.toml | head -1)"
echo "Cargo.toml version:         $cargo_ver"
echo "pyproject.toml version:     $py_ver"
if [ "$cargo_ver" != "$py_ver" ]; then echo "  MISMATCH: crate vs Python wrapper version"; fail=1; fi

hdr_abi="$(sed -n 's/^#define GIBSON_ABI_VERSION \([0-9]*\).*/\1/p' include/gibson.h | head -1)"
py_abi="$(sed -n 's/^EXPECTED_ABI_VERSION = \([0-9]*\).*/\1/p' bindings/python/gibson.py | head -1)"
echo "gibson.h GIBSON_ABI_VERSION: $hdr_abi"
echo "python EXPECTED_ABI_VERSION: $py_abi"
if [ "$hdr_abi" != "$py_abi" ]; then echo "  MISMATCH: C header ABI vs Python wrapper ABI"; fail=1; fi

if [ "$fail" -ne 0 ]; then echo "check-versions: FAIL"; exit 1; fi
echo "check-versions: OK (version $cargo_ver, ABI $hdr_abi)"
