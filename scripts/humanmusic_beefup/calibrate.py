#!/usr/bin/env python3
"""Calibrate the groove bands against the ORACLE: the calibration references SOUND coherent, so
the ruler must AGREE with the ear. Runs the corrected decomposition on the 7 calibration references
+ the declared negative and prints the readings, so the acceptance band can be set to admit every
reference and reject only the negative. The 3 HOLDOUT files are NEVER touched here (§3/§24).

    python3 -I calibrate.py [corpus_dir]      (default: $HUMANMUSIC_MIDI_DIR or ~/Downloads/GOOD_MIDI)

Reads local third-party MIDI (never committed); emits only aggregate per-reference numbers.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # trusted script dir, for -I
from hm_corpus import SmfError, decompose, parse_smf, smf_timing_voices  # noqa: E402

CALIBRATION = [
    ("ABBA — SOS", "ABBA_-_SOS.mid"),
    ("Hall & Oates — I Can't Go For That", "I_CanT_Go_For_That_Hall__Oats.mid"),
    ("Men at Work — Overkill", "Men_at_WorkOverkill.mid"),
    ("MJ — Off The Wall", "Rod_Temperton_Michael_Jackson_Off_The_Wall.mid"),
    ("Rush — Limelight", "Rush_-_Limelight.mid"),
    ("Rush — Subdivisions", "Rush_-_Subdivisions.mid"),
    ("Rush — Tom Sawyer", "Rush_-_Tom_Sawyer.mid"),
]
NEGATIVE = [("Charlie Puth — Attention (NEG)", "AUD_DW0160.mid")]
# Holdout — listed ONLY to assert we never read them here.
HOLDOUT = {"Herbie_Hancock__Chameleon.mid", "Rush_-_Fly_by_Night.mid", "The_Rembrandts_-_I'll_Be_There_For_You.mid"}


def main():
    d = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("HUMANMUSIC_MIDI_DIR") or os.path.expanduser("~/Downloads/GOOD_MIDI")
    print(f"corpus: {d}\n")
    print(f"{'reference':40} {'verdict':6} {'B':>7} {'scatter':>8} {'jitter':>7} {'swing':>7} {'voices':>7}")
    cal_rows = []
    for label, fn in CALIBRATION + NEGATIVE:
        assert fn not in HOLDOUT, f"REFUSED: {fn} is holdout"
        try:
            voices = smf_timing_voices(parse_smf(os.path.join(d, fn)))
            s, _ = decompose(voices)
        except (SmfError, FileNotFoundError) as e:
            print(f"{label:40} ERROR: {e}")
            continue
        tag = "NEG" if (label, fn) in NEGATIVE else "cal"
        print(
            f"{label:40} {s['verdict']:6} {s['band_lean']:+7.1f} {s['scatter']:8.1f} "
            f"{s['jitter']:7.1f} {s['swing']:7.1f} {s['n_voices']:7}"
        )
        if tag == "cal":
            cal_rows.append((label, s))
    if cal_rows:
        max_sc = max(s["scatter"] for _, s in cal_rows)
        max_ji = max(s["jitter"] for _, s in cal_rows)
        print(f"\ncalibration envelope: max scatter={max_sc:.1f} mb, max jitter={max_ji:.1f} mb")
        print("(acceptance ceilings must sit ABOVE this envelope so every heard-coherent reference passes)")


if __name__ == "__main__":
    main()
