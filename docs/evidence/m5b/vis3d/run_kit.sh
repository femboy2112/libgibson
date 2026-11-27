#!/usr/bin/env bash
# VIS-3D kit runner. Builds and runs examples/figure_vis3d_kit, converts, and stages the small
# committed subset. The coordinator wraps this whole script in the heavy lock (cargo, ffmpeg and
# ImageMagick all run here).
#
# usage: run_kit.sh LOCAL_DIR
#   LOCAL_DIR     scratch output dir for PPM frames, full-size PNGs and MP4s. NOT committed.
# env:
#   CARGO         cargo command (default: cargo), e.g. CARGO="cargo +1.98.1"
#   KIT_BIN       prebuilt example binary; skips the build
#   KIT_COMMIT_DIR  where the committed subset goes (default: <this dir>/out)
#   KIT_SKIP_DETERMINISM=1  skip the second run that proves bit-identical output
set -euo pipefail

LOCAL="${1:?usage: run_kit.sh LOCAL_DIR}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
COMMIT="${KIT_COMMIT_DIR:-$HERE/out}"
CARGO="${CARGO:-cargo}"

for tool in convert montage ffmpeg sha256sum; do
  command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 1; }
done

if [ -n "${KIT_BIN:-}" ]; then
  BIN="$KIT_BIN"
else
  (cd "$ROOT" && $CARGO build --release --features figure --example figure_vis3d_kit)
  TD="${CARGO_TARGET_DIR:-$ROOT/target}"
  BIN="$TD/release/examples/figure_vis3d_kit"
fi
[ -x "$BIN" ] || { echo "example binary not found: $BIN" >&2; exit 1; }

rm -rf "$LOCAL/run1" "$LOCAL/run2" "$LOCAL/png" "$LOCAL/mp4"
mkdir -p "$LOCAL/run1" "$LOCAL/png" "$LOCAL/mp4"
R1="$LOCAL/run1"

echo "== render + analysis =="
"$BIN" "$R1" all | tee "$LOCAL/run.log"

tree_hash() { # tree_hash DIR  (hash of every .ppm under DIR, path-sorted)
  (cd "$1" && find . -name '*.ppm' | LC_ALL=C sort | xargs sha256sum | sha256sum | cut -d' ' -f1)
}

if [ "${KIT_SKIP_DETERMINISM:-0}" != "1" ]; then
  echo "== determinism: second run =="
  mkdir -p "$LOCAL/run2"
  "$BIN" "$LOCAL/run2" all >"$LOCAL/run2.log"
  [ "$(tree_hash "$R1")" = "$(tree_hash "$LOCAL/run2")" ] || { echo "DETERMINISM FAIL: ppm trees differ" >&2; exit 1; }
  cmp "$R1/analysis/motion_analysis.json" "$LOCAL/run2/analysis/motion_analysis.json" \
    || { echo "DETERMINISM FAIL: analysis json differs" >&2; exit 1; }
  echo "determinism: ppm tree and analysis json bit-identical across two runs"
fi

echo "== local PNG + MP4 (not committed) =="
for f in "$R1"/stills/*.ppm; do
  convert "$f" "$LOCAL/png/$(basename "${f%.ppm}").png"
done
enc() { # enc PATTERN OUT
  ffmpeg -y -loglevel error -framerate 30 -i "$1" \
    -vf "scale=640:640:flags=neighbor" -pix_fmt yuv420p -c:v libx264 -crf 18 -preset medium \
    -movflags +faststart -r 30 "$2"
}
for s in loop preempt look; do enc "$R1/motion/$s/f_%03d.ppm" "$LOCAL/mp4/$s.mp4"; done
enc "$R1/turntable/t_%03d.ppm" "$LOCAL/mp4/turntable.mp4"

echo "== committed subset =="
rm -rf "$COMMIT/stills" "$COMMIT/motion" "$COMMIT/contact_sheet.png" "$COMMIT/motion_analysis.json" "$COMMIT/manifest.txt"
mkdir -p "$COMMIT/stills" "$COMMIT/motion"
small() { convert "$1" -resize 160x160 +dither -colors 48 -strip "PNG8:$2"; }
for n in front side three_quarter back morph_front; do
  small "$R1/stills/$n.ppm" "$COMMIT/stills/$n.png"
done
for s in loop preempt look; do
  for k in 000 054 107; do
    small "$R1/motion/$s/f_$k.ppm" "$COMMIT/motion/${s}_f$k.png"
  done
done
TILES=()
for n in front side three_quarter back morph_front; do TILES+=("$R1/stills/$n.ppm"); done
for k in 000 006 012 018 024 030 036 042 048 054 060 066; do TILES+=("$R1/turntable/t_$k.ppm"); done
montage "${TILES[@]}" -tile 6x -geometry 128x128+2+2 -background '#202020' +dither -colors 48 -strip \
  "PNG8:$COMMIT/contact_sheet.png"
cp "$R1/analysis/motion_analysis.json" "$COMMIT/motion_analysis.json"

{
  echo "# VIS-3D kit manifest. sha256 of every committed output, then tree hashes of local PPMs."
  grep '^humanoid:' "$LOCAL/run.log" || true
  (cd "$COMMIT" && find . -type f ! -name manifest.txt ! -name .gitignore | LC_ALL=C sort | xargs sha256sum)
  for d in stills turntable motion/loop motion/preempt motion/look; do
    echo "tree-sha256 ppm/$d $(tree_hash "$R1/$d")"
  done
} >"$COMMIT/manifest.txt"

KB="$(du -sk "$COMMIT" | cut -f1)"
echo "committed subset: ${KB} KB in $COMMIT"
if [ "$KB" -gt 400 ]; then echo "WARNING: committed subset exceeds 400 KB" >&2; exit 3; fi
echo "local outputs: $LOCAL/run1 (ppm), $LOCAL/png, $LOCAL/mp4"
