#!/usr/bin/env bash
# Verify the C137_ACCEPTED_V1 acoustic lock: render each accepted level via the REAL c137_song
# binary and sha256sum -c against the committed, external-to-candidate SHA256SUMS.
#
#   scripts/verify-c137-accepted-v1.sh             # verify (exit 0 = all artifacts reproduce)
#   scripts/verify-c137-accepted-v1.sh --calibrate # red-first: corrupt 1 PCM byte, prove the check FAILS
#
# A mismatch is a STOP/triage, NOT permission to rebaseline. This script never writes SHA256SUMS.
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIX="$REPO/docs/fixtures/humanmusic-c137-accepted-v1"
SUMS="$FIX/SHA256SUMS"
[ -f "$SUMS" ] || { echo "missing $SUMS"; exit 2; }

CALIBRATE=0
[ "${1:-}" = "--calibrate" ] && CALIBRATE=1

BIN="${C137_BIN:-}"
if [ -z "$BIN" ]; then
  echo "building c137_song (release, pinned 1.99.0) ..."
  ( cd "$REPO" && CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
      cargo +1.99.0 build --release --example c137_song ) || { echo "BUILD FAILED"; exit 2; }
  BIN="$REPO/target/release/examples/c137_song"
fi
[ -x "$BIN" ] || { echo "binary not found: $BIN"; exit 2; }

WORK="$(mktemp -d)"
echo "rendering corpus into $WORK ..."
render() { # name nolead flags...
  local name="$1" nolead="$2"; shift 2
  local args=(--seeds=2112,7,99 --tempo=144 --out="$WORK/$name" "$@")
  [ "$nolead" = yes ] && args+=(--no-lead)
  "$BIN" "${args[@]}" >/dev/null 2>&1 || { echo "render $name FAILED"; exit 2; }
}
render A1 no  --jerry=short,plain
render A2 no  --jerry=plain
render B  yes
render C1 no  --form=song
render C2 no  --form=song-intro
render D1 yes --form=song-narrative
render D2 yes --form=song-intro-narrative

if [ "$CALIBRATE" = 1 ]; then
  victim="$WORK/B/seed-2112/full.normal.wav"
  printf '\xa5' | dd of="$victim" bs=1 seek=200 count=1 conv=notrunc status=none
  echo "[calibrate] flipped one byte of B/seed-2112/full.normal.wav — the check MUST now fail"
fi

cd "$WORK" || exit 2
echo "checking $(grep -c . "$SUMS") artifacts against the committed lock ..."
if sha256sum -c "$SUMS"; then
  if [ "$CALIBRATE" = 1 ]; then
    echo "CALIBRATION FAILED: a 1-byte corruption went UNDETECTED — the lock is not biting."
    exit 1
  fi
  echo "C137_ACCEPTED_V1 LOCK: OK — every accepted artifact reproduces byte-for-byte."
  exit 0
else
  if [ "$CALIBRATE" = 1 ]; then
    echo "CALIBRATION OK: the acoustic witness correctly rejected a 1-byte mutation."
    exit 0
  fi
  echo ""
  echo "LOCK DRIFT DETECTED. This is a STOP/triage, not a rebaseline."
  echo "  - Diff the score.tsv layer to tell an audible-content change from a synth/master change."
  echo "  - Re-freeze ONLY after the change is understood, intended, and (for an ear level) re-accepted."
  exit 1
fi
