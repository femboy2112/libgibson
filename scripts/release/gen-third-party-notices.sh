#!/usr/bin/env bash
#
# gen-third-party-notices.sh — generate THIRD-PARTY-NOTICES.md from Cargo metadata.
#
# Lists every crate in the RELEASE (normal) dependency closure with its version,
# SPDX license, and repository, so a distributed binary's third-party attribution is
# exhaustive and reproducible rather than hand-maintained. Re-run after any
# dependency change. LibGibson itself is MIT OR Apache-2.0 (LICENSE-MIT,
# LICENSE-APACHE), and every listed dependency is permissively licensed.
#
# Usage: scripts/release/gen-third-party-notices.sh [OUTPUT.md]
#        CARGO=cargo+1.98.1 scripts/release/gen-third-party-notices.sh   # pin toolchain

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
# Deterministic, locale-independent text processing: `sort` collation is
# locale-sensitive, so without this the generated row order differs between an
# en_US.UTF-8 developer box and a C/C.UTF-8 CI runner, which would make the
# (now fatal) freshness check in preflight.sh spuriously fail. Pin to C so the
# output is byte-identical everywhere.
export LC_ALL=C
OUT="${1:-$REPO_ROOT/THIRD-PARTY-NOTICES.md}"
CARGO="${CARGO:-cargo}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

# Normal (non-dev, non-build) closure. --no-dedupe surfaces every occurrence;
# stripping the "(proc-macro)"/"(*)" annotations then sort -u collapses repeats.
$CARGO tree -e normal -f "{p}@@@{l}@@@{r}" --prefix none --no-dedupe 2>/dev/null \
    | sed 's/ (proc-macro)//; s/ (\*)$//' \
    | sort -u \
    | grep -v '^libgibson ' > "$tmp"

{
    echo "# Third-Party Notices"
    echo
    echo "LibGibson $VERSION is distributed under \`MIT OR Apache-2.0\` (see"
    echo "\`LICENSE-MIT\` and \`LICENSE-APACHE\`). Its distributed native artifacts"
    echo "incorporate code from the third-party crates listed below."
    echo
    echo "This is the exhaustive **normal (release) dependency closure**, generated"
    echo "reproducibly from Cargo metadata by"
    echo "\`scripts/release/gen-third-party-notices.sh\`; regenerate it after any"
    echo "dependency change. Every crate is permissively licensed (MIT / Apache-2.0 /"
    echo "Zlib / Unicode-3.0 and dual combinations); the full text of the two primary"
    echo "licenses ships as \`LICENSE-MIT\` and \`LICENSE-APACHE\`. A few listed crates"
    echo "are build-time proc-macros that are not linked into the shipped binary; they"
    echo "are included here for completeness."
    echo
    echo "| Package | License | Repository |"
    echo "|---------|---------|------------|"
    awk -F'@@@' '{printf "| %s | %s | %s |\n", $1, $2, $3}' "$tmp"
} > "$OUT"

echo "gen-third-party-notices: wrote $OUT ($(grep -c '^| .* v' "$OUT") crates)"
