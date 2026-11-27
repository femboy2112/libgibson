#!/usr/bin/env bash
# Assemble a viewing kit from a render_tape.py output directory (run it after the render lease; it is NOT a heavy job: ffmpeg + ImageMagick
# on a few hundred PNGs, run it niced).  Per clip: an H.264 MP4 (yuv420p, crf 18) and a contact sheet; the frame PNGs are deleted only after
# BOTH exist and the MP4 holds exactly as many frames as there were PNGs; their sha256 values stay in MANIFEST.md.
#
#   bash tools/figure_assets/make_kit.sh RENDER_DIR KIT_ID [--fps N] [--keep-frames]
#
# Kit layout (default root ~/.local/state/first-performance, override with KIT_ROOT):
#   <KIT_ROOT>/<KIT_ID>/{examples/*.mp4, stills/*.png, contact/*.png, MANIFEST.md, render.json, oracle_posed.*}
# RENDER_DIR layout (written by render_tape.py): stills/*.png, frames/<clip>/frame_<NNNNN>.png, render.json.
# Exit status: 0 ok, 1 a clip failed (its frames are kept), 2 bad usage.
set -u
RD="${1:-}"; KID="${2:-}"
if [ -z "$RD" ] || [ -z "$KID" ] || [ ! -d "$RD" ]; then echo "usage: $0 RENDER_DIR KIT_ID [--fps N] [--keep-frames]" >&2; exit 2; fi
shift 2
FPS_OVERRIDE=""; KEEP=0
while [ $# -gt 0 ]; do
  case "$1" in
    --fps) FPS_OVERRIDE="${2:-}"; shift 2 ;;
    --keep-frames) KEEP=1; shift ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done
case "$KID" in *[!A-Za-z0-9._-]*|"") echo "KIT_ID must be [A-Za-z0-9._-]+" >&2; exit 2 ;; esac
for t in ffmpeg ffprobe montage sha256sum python3; do command -v "$t" >/dev/null 2>&1 || { echo "ERROR: $t not found" >&2; exit 2; }; done

KIT="${KIT_ROOT:-$HOME/.local/state/first-performance}/$KID"
mkdir -p "$KIT/examples" "$KIT/stills" "$KIT/contact" || exit 2
MAN="$KIT/MANIFEST.md"
FAILS=0
{
  echo "# Kit $KID"
  echo
  echo "Offline reference render of a runtime-evaluated performance (render_tape.py)."
  echo "Built by make_kit.sh. Frame PNGs are deleted after the MP4 and contact sheet exist; their sha256 values are listed below."
  echo
} > "$MAN"

if [ -f "$RD/render.json" ]; then
  cp "$RD/render.json" "$KIT/render.json"
  for f in oracle_posed.json oracle_posed.f32.bin; do [ -f "$RD/$f" ] && cp "$RD/$f" "$KIT/$f"; done
fi
clip_fps() {   # $1 clip name -> fps from render.json, else 30
  if [ -n "$FPS_OVERRIDE" ]; then echo "$FPS_OVERRIDE"; return; fi
  python3 -I - "$RD/render.json" "$1" <<'PY' 2>/dev/null || echo 30
import json, sys
try:
    d = json.load(open(sys.argv[1]))
    for c in d["results"]["clips"]:
        if c["name"] == sys.argv[2]:
            print(c["fps"]); sys.exit(0)
except Exception:
    pass
print(30)
PY
}

# ---- stills
if [ -d "$RD/stills" ]; then
  echo "## Stills" >> "$MAN"
  for p in "$RD"/stills/*.png; do
    [ -f "$p" ] || continue
    cp "$p" "$KIT/stills/"
    echo "- stills/$(basename "$p") sha256 $(sha256sum "$p" | cut -d' ' -f1)" >> "$MAN"
  done
  echo >> "$MAN"
fi

# ---- clips
if [ -d "$RD/frames" ]; then
  for cdir in "$RD"/frames/*/; do
    [ -d "$cdir" ] || continue
    clip="$(basename "$cdir")"
    mapfile -t PNGS < <(ls "$cdir"frame_*.png 2>/dev/null | sort)
    n="${#PNGS[@]}"
    if [ "$n" -eq 0 ]; then echo "NOTE clip $clip has no frames, skipped"; continue; fi
    fps="$(clip_fps "$clip")"
    mp4="$KIT/examples/${KID}_${clip}.mp4"
    sheet="$KIT/contact/${KID}_${clip}.png"
    echo "KIT clip=$clip frames=$n fps=$fps"
    nice -n 10 ffmpeg -nostdin -loglevel error -y -framerate "$fps" -pattern_type glob -i "${cdir}frame_*.png" \
      -c:v libx264 -preset medium -crf 18 -pix_fmt yuv420p -movflags +faststart "$mp4" || { echo "FAIL ffmpeg clip=$clip"; FAILS=$((FAILS + 1)); continue; }
    # contact sheet: 12 evenly spaced frames (fewer when the clip is shorter)
    k=12; [ "$n" -lt "$k" ] && k="$n"
    SEL=()
    for ((i = 0; i < k; i++)); do SEL+=("${PNGS[$(( i * (n - 1) / (k > 1 ? k - 1 : 1) ))]}"); done
    cols=4; [ "$k" -lt 4 ] && cols="$k"
    rows=$(( (k + cols - 1) / cols ))
    nice -n 10 montage "${SEL[@]}" -tile "${cols}x${rows}" -geometry 320x180+3+3 -background '#202020' "$sheet" || { echo "FAIL montage clip=$clip"; FAILS=$((FAILS + 1)); continue; }
    got="$(ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 "$mp4" 2>/dev/null)"
    if [ ! -s "$mp4" ] || [ ! -s "$sheet" ] || [ "$got" != "$n" ]; then
      echo "FAIL clip=$clip mp4_frames=${got:-NA} pngs=$n (frames kept)"; FAILS=$((FAILS + 1)); continue
    fi
    {
      echo "## Clip $clip"
      echo "- examples/${KID}_${clip}.mp4 sha256 $(sha256sum "$mp4" | cut -d' ' -f1) (H.264 yuv420p crf 18, $fps fps, $n frames)"
      echo "- contact/${KID}_${clip}.png sha256 $(sha256sum "$sheet" | cut -d' ' -f1)"
      echo "- frame sha256 (deleted after assembly unless --keep-frames):"
      for p in "${PNGS[@]}"; do echo "  - $(basename "$p") $(sha256sum "$p" | cut -d' ' -f1)"; done
      echo
    } >> "$MAN"
    if [ "$KEEP" -eq 0 ]; then rm -f -- "${PNGS[@]}"; rmdir "$cdir" 2>/dev/null; fi
  done
fi
echo "KIT_DIR_NAME=$KID KIT_FAILS=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
exit 0
