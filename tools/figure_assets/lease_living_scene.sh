#!/usr/bin/env bash
# Run inside a GOD heavy class-s lease. FPS and SPP control the film, independent of the baked tape clock.
set -euo pipefail
[[ ${HEAVY_CLASS:-} == s ]] || { echo 'Run inside heavy run --class s.' >&2; exit 2; }
[[ $# == 5 ]] || { echo 'usage: lease_living_scene.sh render|encode EXPORT PRESET TAPE_DIRECTORY OUTPUT_DIRECTORY' >&2; exit 2; }
phase=$1
export_dir=$(realpath "$2")
preset=$(realpath "$3")
performance=$(realpath "$4")
out=$(realpath -m "$5")
here=$(cd "$(dirname "$0")" && pwd)
blender=${BLENDER:-/home/leah/opt/blender/blender}
export MPFB_DATA="${MPFB_DATA:-$HOME/.config/blender/4.5/extensions/user_default/mpfb/data}"
export MPFB_USER_DATA="${MPFB_USER_DATA:-$HOME/.config/blender/4.5/extensions/.user/user_default/mpfb/data}"
mkdir -p "$out/logs"
fps=${FILM_FPS:-12}
if ! [[ $fps =~ ^[1-9][0-9]*$ ]] || (( fps > 60 )); then
    echo 'FILM_FPS must be 1..60.' >&2
    exit 2
fi
frames=$(jq --argjson fps "$fps" '(.frame_count * $fps / .fps) | ceil' "$performance/generic.tape.json")
if [[ $phase == render ]]; then
    jq --argjson fps "$fps" --argjson frames "$frames" '.clips[0].fps=$fps | .clips[0].frames=$frames' \
        "$performance/shots.json" > "$out/film-shots.json"
    lo=${FILM_FROM:-0}
    num=${FILM_COUNT:-0}
    "$blender" -b -t 4 --python-exit-code 3 --python "$here/render_tape.py" -- \
        --export "$export_dir" --preset "$preset" --joint-frame bone --tape "$performance/generic.tape.json" \
        --fitted "$performance/generic.fit.json" --matter "$performance/matter.json" --scene "$performance/scene.json" \
        --shots "$out/film-shots.json" --engine eevee --spp "${FILM_SPP:-4}" --no-stills --no-oracle \
        --clip-from "$lo" --clip-frames "$num" --out "$out" 2>&1 | tee "$out/logs/render-from-$lo.log"
elif [[ $phase == encode ]]; then
    for ((i=0; i<frames; i++)); do
        printf -v path '%s/frames/performance/frame_%05d.png' "$out" "$i"
        [[ -s $path ]] || { echo "Missing rendered frame $i" >&2; exit 1; }
    done
    tmp="$out/performance.tmp.mp4"
    ffmpeg -hide_banner -loglevel error -y -framerate "$fps" -i "$out/frames/performance/frame_%05d.png" \
        -frames:v "$frames" -c:v libx264 -preset fast -crf 18 -pix_fmt yuv420p -movflags +faststart "$tmp"
    [[ $(ffprobe -v error -select_streams v:0 -count_frames -show_entries stream=nb_read_frames \
        -of default=noprint_wrappers=1:nokey=1 "$tmp") == "$frames" ]]
    mv "$tmp" "$out/performance.mp4"
else
    echo "Unknown phase $phase" >&2
    exit 2
fi
