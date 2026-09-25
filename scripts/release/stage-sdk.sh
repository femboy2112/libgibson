#!/usr/bin/env bash
#
# stage-sdk.sh — stage a relocatable LibGibson native SDK into a prefix.
#
# Builds the release artifacts and copies them, with pkg-config metadata and the
# license/doc files, into a fresh staging prefix laid out as a standard native SDK:
#
#     <prefix>/
#       include/  gibson.h  gibson.hpp  termframe.h
#       lib/      libgibson.so.1  libgibson.so -> libgibson.so.1  libgibson.a
#       lib/pkgconfig/  libgibson.pc     (relocatable via ${pcfiledir})
#       share/doc/libgibson/  README.md  LICENSE-MIT  LICENSE-APACHE  LICENSES-THIRD-PARTY.md
#       SHA256SUMS
#
# Publication-free and non-destructive to the host: it never installs into system
# paths, never uses sudo, and never touches anything outside <prefix>. The pkg-config
# file resolves its prefix relative to its own location, so the staged tree can be
# moved or copied anywhere and still work (no baked-in checkout path).
#
# Usage:
#   scripts/release/stage-sdk.sh --prefix /tmp/libgibson-sdk [--force] [--no-build]
#
#   --prefix DIR   destination prefix (required); created if absent.
#   --force        if <prefix> already exists and is non-empty, remove it first.
#   --no-build     skip `cargo build --release` (use existing target/release artifacts).

set -euo pipefail

die() { printf 'stage-sdk: error: %s\n' "$*" >&2; exit 1; }
note() { printf 'stage-sdk: %s\n' "$*" >&2; }

PREFIX=""
FORCE=0
NO_BUILD=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix) PREFIX="${2:-}"; shift 2 ;;
        --prefix=*) PREFIX="${1#*=}"; shift ;;
        --force) FORCE=1; shift ;;
        --no-build) NO_BUILD=1; shift ;;
        -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

[[ -n "$PREFIX" ]] || die "--prefix is required (e.g. --prefix /tmp/libgibson-sdk)"

# Sanity guard against a mistyped staging path being handed to `rm -rf` under
# --force. This is NOT a security sandbox; it just refuses the obviously
# catastrophic targets. Normalize first so a trailing slash, "./", or a relative
# path cannot slip a dangerous prefix past the case match below.
case "$PREFIX" in
    /*) _norm="$PREFIX" ;;
    *)  _norm="$PWD/$PREFIX" ;;
esac
while [[ "$_norm" == */ && "$_norm" != "/" ]]; do _norm="${_norm%/}"; done
case "$_norm" in
    /)
        die "refusing to stage into the filesystem root" ;;
    /usr | /usr/* | /bin | /bin/* | /sbin | /sbin/* \
    | /lib | /lib/* | /lib64 | /lib64/* \
    | /etc | /etc/* | /boot | /boot/* \
    | /dev | /dev/* | /proc | /proc/* | /sys | /sys/*)
        die "refusing to stage into a system path: $PREFIX (normalized: $_norm)" ;;
esac
if [[ -n "${HOME:-}" && "$_norm" == "${HOME%/}" ]]; then
    die "refusing to stage directly into \$HOME ($HOME); use a subdirectory such as \$HOME/libgibson-sdk"
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# Single source of truth for the version: Cargo.toml [package] version.
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
[[ -n "$VERSION" ]] || die "could not read version from Cargo.toml"
note "LibGibson version: $VERSION"

if [[ "$NO_BUILD" -eq 0 ]]; then
    note "building release artifacts (cargo build --release)..."
    cargo build --release
fi

SO="$REPO_ROOT/target/release/libgibson.so"
A="$REPO_ROOT/target/release/libgibson.a"
[[ -f "$SO" ]] || die "missing $SO (run without --no-build, or build release first)"
[[ -f "$A" ]]  || die "missing $A"

# Best-effort discovery of the native system libraries a STATIC consumer must add.
# Dynamic (.so) consumers do not need these; they go into Libs.private for --static.
NATIVE_STATIC_LIBS="$(cargo rustc --release --crate-type staticlib -- --print native-static-libs 2>&1 \
    | sed -n 's/^note: native-static-libs: //p' | head -1 || true)"
[[ -n "$NATIVE_STATIC_LIBS" ]] || NATIVE_STATIC_LIBS="-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc"
note "native-static-libs: $NATIVE_STATIC_LIBS"

# Fresh prefix.
if [[ -e "$PREFIX" ]]; then
    if [[ -n "$(ls -A "$PREFIX" 2>/dev/null)" ]]; then
        [[ "$FORCE" -eq 1 ]] || die "$PREFIX exists and is non-empty (use --force to replace)"
        note "--force: removing existing $PREFIX"
        rm -rf -- "$PREFIX"
    fi
fi
mkdir -p "$PREFIX"/{include,lib,lib/pkgconfig,share/doc/libgibson}

# Headers (the canonical public surface + the documented termframe compat alias).
install -m 0644 include/gibson.h    "$PREFIX/include/gibson.h"
install -m 0644 include/gibson.hpp  "$PREFIX/include/gibson.hpp"
install -m 0644 include/termframe.h "$PREFIX/include/termframe.h"

# Libraries. The shared object is installed VERSIONED as libgibson.so.1 (its ELF
# SONAME — see build.rs), with a development symlink libgibson.so -> libgibson.so.1
# that `-lgibson` links against. A dynamic consumer then records
# NEEDED=libgibson.so.1 at runtime; the symlink is excluded from SHA256SUMS by the
# `-type f` filter below and preserved by `tar` in bundle.sh.
install -m 0755 "$SO" "$PREFIX/lib/libgibson.so.1"
ln -sf libgibson.so.1 "$PREFIX/lib/libgibson.so"
install -m 0644 "$A"  "$PREFIX/lib/libgibson.a"

# License / doc payload.
install -m 0644 README.md              "$PREFIX/share/doc/libgibson/README.md"
install -m 0644 LICENSE-MIT            "$PREFIX/share/doc/libgibson/LICENSE-MIT"
install -m 0644 LICENSE-APACHE         "$PREFIX/share/doc/libgibson/LICENSE-APACHE"
install -m 0644 LICENSES-THIRD-PARTY.md "$PREFIX/share/doc/libgibson/LICENSES-THIRD-PARTY.md"

# Exhaustive third-party notices for the current dependency closure. Regenerated
# fresh so the staged SDK always matches the libraries it actually ships.
if [[ -x scripts/release/gen-third-party-notices.sh ]]; then
    scripts/release/gen-third-party-notices.sh "$PREFIX/share/doc/libgibson/THIRD-PARTY-NOTICES.md" >&2
elif [[ -f THIRD-PARTY-NOTICES.md ]]; then
    install -m 0644 THIRD-PARTY-NOTICES.md "$PREFIX/share/doc/libgibson/THIRD-PARTY-NOTICES.md"
fi

# Relocatable pkg-config metadata: prefix derives from the .pc file's own location
# (<prefix>/lib/pkgconfig/../.. == <prefix>), so the tree can be moved freely.
cat > "$PREFIX/lib/pkgconfig/libgibson.pc" <<PC
prefix=\${pcfiledir}/../..
exec_prefix=\${prefix}
libdir=\${prefix}/lib
includedir=\${prefix}/include

Name: libgibson
Description: Language-neutral terminal UI engine (cell framebuffer, differential renderer)
URL: https://github.com/femboy2112/libgibson
Version: $VERSION
Libs: -L\${libdir} -lgibson
Libs.private: $NATIVE_STATIC_LIBS
Cflags: -I\${includedir}
PC

# Deterministic checksum manifest (sorted, prefix-relative paths).
( cd "$PREFIX" && find . -type f ! -name SHA256SUMS -print0 \
    | sort -z | xargs -0 sha256sum > SHA256SUMS )

note "staged SDK at: $PREFIX"
echo "=== staged contents ==="
( cd "$PREFIX" && find . -type f | sort | sed 's/^\.\//  /' )
echo "=== libgibson.pc ==="
cat "$PREFIX/lib/pkgconfig/libgibson.pc"
