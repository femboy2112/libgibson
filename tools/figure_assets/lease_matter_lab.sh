#!/usr/bin/env bash
# Run one phase per local heavy lease. Keep raw inputs, movies and failed logs under OUT.
set -euo pipefail
[[ ${HEAVY_CLASS:-} == s || ${HEAVY_CLASS:-} == x ]] || { echo 'A GOD heavy lease is required.' >&2; exit 2; }
: "${CARGO_TARGET_DIR:?set to an owned build directory}"
phase=${1:?build|physics|movies|assemble}
asset=${2:?converted generic asset directory}
out=${3:?private output directory}
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
cd "$repo"
mkdir -p "$out"
preset="$here/presets/generic_neutral.json"
producer="$CARGO_TARGET_DIR/release/gibson-still-recipe"
movie="$CARGO_TARGET_DIR/release/matter_movie"
export CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
case "$phase" in
build)
  rc=0
  cargo +1.98.1 test --offline --locked --features figure --lib living_matter -- --nocapture >"$out/fixtures.log" 2>&1 || rc=$?
  tail -24 "$out/fixtures.log"
  "${BLENDER:-$HOME/opt/blender/blender}" -b -t 3 --factory-startup --python-exit-code 3 --python "$repo/docs/evidence/incarnate-iii/blender_cloth_probe.py" >"$out/blender-cloth.log" 2>&1 || rc=$?
  rg 'BLENDER_CLOTH|Error|Assertion' "$out/blender-cloth.log" || true
  cargo +1.98.1 build --release --offline --locked --manifest-path "$here/still_recipe/Cargo.toml" >"$out/build.log" 2>&1 || rc=$?
  tail -6 "$out/build.log"
  exit "$rc";;
physics)
  for profile in medium zero side light structured supported; do
    opts=(--fabric medium --gravity normal)
    case $profile in zero|side) opts=(--gravity "$profile");; light|structured) opts=(--fabric "$profile");; supported) opts+=(--support 1);; esac
    /usr/bin/time -f 'elapsed=%e rss_kib=%M' "$producer" "$asset" "$preset" "$out/$profile" --performance idle --preroll-frames 30 "${opts[@]}" >"$out/$profile.log" 2>&1
  done
  python3 "$here/matter_measures.py" "$out" >"$out/measures.json";;
movies)
  for profile in medium zero side light structured supported; do
    "$movie" "$asset" "$preset" "$out/$profile" "$out/$profile.mp4" wide physical
  done
  "$movie" "$asset" "$preset" "$out/medium" "$out/kinematic.mp4" wide kinematic;;
assemble)
  # Two synchronized screens preserve the source camera, scale and time for each trial.
  encode_panel(){
    local title=$1 a=$2 b=$3 c=$4 d=$5
    local flt=""
    local i=0
    for label in "$a" "$b" "$c" "$d"; do
      flt+="[${i}:v]scale=640:360,drawtext=fontfile=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf:text='$label':fontcolor=white:fontsize=18:box=1:boxcolor=black@0.75:x=12:y=12[v$i];"
      i=$((i+1))
    done
    ffmpeg -nostdin -loglevel error -y -filter_complex_threads 2 -i "$out/$a.mp4" -i "$out/$b.mp4" -i "$out/$c.mp4" -i "$out/$d.mp4" -filter_complex "${flt}[v0][v1][v2][v3]xstack=inputs=4:layout=0_0|640_0|0_360|640_360[v]" -map '[v]' -an -c:v libx264 -threads 2 -crf 17 -pix_fmt yuv420p "$out/$title.mp4"
  }
  encode_panel gravity-survey kinematic medium zero side
  encode_panel material-survey light medium structured supported
  printf "file 'gravity-survey.mp4'\nfile 'material-survey.mp4'\n" >"$out/gravity-shots.txt"
  ffmpeg -nostdin -loglevel error -y -f concat -safe 1 -i "$out/gravity-shots.txt" -c copy -movflags +faststart "$out/gravity-lab.mp4";;
*) exit 2;;
esac
