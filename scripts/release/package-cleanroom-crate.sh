#!/usr/bin/env bash
# Package and unpack this source tree into a cleanroom using a fresh, explicit target.
set -euo pipefail
[[ $# == 2 ]] || { echo 'usage: package-cleanroom-crate.sh DEST VERSION' >&2; exit 2; }
dest=$1
version=$2
[[ "$version" =~ ^[0-9A-Za-z.+-]+$ ]] || { echo 'invalid crate version' >&2; exit 2; }
[[ -n ${CARGO_TARGET_DIR:-} && "$CARGO_TARGET_DIR" = /* ]] || {
    echo 'an absolute, fresh CARGO_TARGET_DIR is required' >&2; exit 2;
}
[[ ! -e "$CARGO_TARGET_DIR" ]] || { echo 'package target already exists' >&2; exit 2; }
[[ -d "$dest" && ! -e "$dest/libgibson-$version" ]] || {
    echo 'destination is missing or already contains the crate' >&2; exit 2;
}
"${CARGO:-cargo}" package --allow-dirty
crate="$CARGO_TARGET_DIR/package/libgibson-$version.crate"
[[ -f "$crate" ]] || { echo "fresh packaged crate missing: $crate" >&2; exit 1; }
tar xzf "$crate" -C "$dest"
[[ -f "$dest/libgibson-$version/Cargo.toml" ]] || {
    echo 'unpacked crate is missing Cargo.toml' >&2; exit 1;
}
