#!/usr/bin/env bash
# Rebuild one generic Ex1 wardrobe and the two offline reference stills.
# Run each phase inside GOD's heavy class s lease.
set -euo pipefail

[[ ${HEAVY_CLASS:-} == s ]] || { echo 'Run inside heavy run --class s.' >&2; exit 2; }
here=$(cd "$(dirname "$0")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
out="${FP_OUTPUT_ROOT:-$HOME/.local/state/first-performance/codex-fp}"
blender=${BLENDER:-/home/leah/opt/blender/blender}
preset="$here/presets/generic_neutral.json"
licence="$repo/docs/evidence/m5c/LICENCE_MANIFEST.json"
export MPFB_DATA="${MPFB_DATA:-$HOME/.config/blender/4.5/extensions/user_default/mpfb/data}"
export MPFB_USER_DATA="${MPFB_USER_DATA:-$HOME/.config/blender/4.5/extensions/.user/user_default/mpfb/data}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$out/cargo-target}"
export TMPDIR="$out/tmp"
mkdir -p "$out/logs" "$TMPDIR"
cd "$repo"

adapter=("$blender" -b -t 4 --python-exit-code 3 --python "$here/render_tape.py" --
         --export "$out/export" --tape "$out/tape/generic.tape.json" --fitted "$out/tape/generic.fit.json"
         --joint-frame bone --preset "$preset" --shots "$here/shots/ex1_character.json"
         --no-clips --no-oracle --engine eevee --spp 48)

case ${1:-} in
    export)
        python3 -I "$here/fa_skirt.py" --selftest
        "$blender" -b -t 4 --python-exit-code 3 --python "$here/export_mpfb.py" -- \
            --out "$out/export" --preset "$preset" --licence-json "$licence" \
            2>&1 | tee "$out/logs/export.log"
        grep -qx 'EXPORT_RC 0' "$out/logs/export.log"
        jq -e '.licence.sources_not_cc0 == []' "$out/export/manifest.json" >/dev/null
        python3 -I "$here/licence_audit.py" --check-export "$out/export/manifest.json"
        python3 -I "$here/convert_to_gibson.py" convert --export "$out/export" --out "$out/asset"
        ;;
    tape)
        cargo +1.98.1 run --offline --locked --manifest-path "$here/still_recipe/Cargo.toml" -- \
            "$out/asset" "$preset" "$out/tape" 2>&1 | tee "$out/logs/tape.log"
        python3 -I "$here/check_tape.py" "$out/tape/generic.tape.json" --require-fa-tape
        ;;
    render)
        "${adapter[@]}" --still front --still face_closeup --out "$out/render" 2>&1 | tee "$out/logs/render.log"
        grep -qx 'RENDER_TAPE_RC 0' "$out/logs/render.log"
        ;;
    controls)
        "$blender" -b -t 4 --python-exit-code 1 --python "$here/check_long01.py" -- \
            --export "$out/export" --tape "$out/tape/generic.tape.json" --fitted "$out/tape/generic.fit.json" \
            --report "$out/render/render.json" \
            2>&1 | tee "$out/logs/eye-control.log"
        rc=0
        "${adapter[@]}" --out "$out/bad_bounds" --mutant bad_bounds --checks-only > "$out/logs/bad-bounds.log" 2>&1 || rc=$?
        [[ $rc == 5 ]]
        grep -qx 'RENDER_TAPE_RC 5' "$out/logs/bad-bounds.log"
        grep '^RT_CHECK FAIL contact ' "$out/logs/bad-bounds.log"
        grep '^RT_CHECK FAIL shadow ' "$out/logs/bad-bounds.log"
        ;;
    *) echo "usage: $0 {export|tape|render|controls}" >&2; exit 2 ;;
esac
