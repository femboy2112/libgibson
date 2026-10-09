#!/usr/bin/env python3
"""Groove / timing-coordination metric for the HumanMusic beef-up acceptance suite.

Decides whether a multi-voice take holds a POCKET or trips over itself, by SEPARATING intentional
shared structure from unexplained independent error (the whole point — a band leaning together is
not sloppy, and a triplet is not jitter). It reports, in MILIBEAT (1 beat = 1000 mb, a 16th = 250):

  band_lean (B) : the ensemble's shared, phase-independent pulse lean (anchored to the downbeat)
  scatter       : how much the per-voice leans disagree — small = coordinated, large = pulling apart
  jitter        : the median per-voice residual wobble AFTER each voice's own groove shape is removed
  swing         : the median per-voice groove/swing spread (lawful phase-dependent displacement)

Works on a Standard MIDI File (reference songs, drums INCLUDED and split by GM lane) or a
human_music_probe / gap_dump TSV (engine output), so the engine is judged by the same instrument.

    python3 groove_metrics.py <file.mid | file.notes.tsv>

------------------------------------------------------------------------------------------------
Calibration philosophy (the ORACLE principle): the reference corpus was chosen because it SOUNDS
coherent — the heard experience is ground truth; this metric is only a downstream lens. So if the
corrected metric ever scored a reference as out-of-pocket, that would refute the METRIC, not the
reference. The acceptance band below is set so EVERY calibration reference lands non-FAIL and only
the declared negative ("Attention", an audio->MIDI transcription with independent per-voice error)
lands FAIL. Thresholds live in hm_corpus.py and were set from the calibration corpus + the §6
synthetic controls; see CALIBRATION.md for the derivation and the per-reference readings.
------------------------------------------------------------------------------------------------
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # trusted script dir, for -I
from hm_corpus import (  # noqa: E402
    JITTER_CEIL_MB,
    RIGID_FLOOR_MB,
    SCATTER_CEIL_MB,
    SmfError,
    decompose,
    parse_smf,
    read_probe_tsv,
    smf_timing_voices,
)

NEGATIVE_REFERENCE = {"name": "Attention (AUD_DW0160)", "note": "independent per-voice error"}


def load_voices(path):
    if path.endswith(".tsv"):
        return read_probe_tsv(path)
    return smf_timing_voices(parse_smf(path))


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    path = sys.argv[1]
    try:
        voices = load_voices(path)
    except SmfError as e:
        print(f"REFUSED: {e}")
        sys.exit(3)
    s, rows = decompose(voices)
    print(f"{os.path.basename(path)}: {s['n_voices']} voices >= 12 notes   grids={s.get('grids', {})}")
    print(f"  band_lean (shared pulse lean) : {s['band_lean']:+7.1f} mb")
    print(f"  cross-voice scatter           : {s['scatter']:7.1f} mb   (ceil {SCATTER_CEIL_MB})")
    print(f"  within-voice jitter           : {s['jitter']:7.1f} mb   (ceil {JITTER_CEIL_MB})")
    print(f"  groove/swing spread           : {s['swing']:7.1f} mb")
    print(f"  structure (max|lean|,scatter,swing): {s['structure']:6.1f} mb   (rigid floor {RIGID_FLOOR_MB})")
    for r in rows[:12]:
        print(
            f"    {r['voice']:12} n={r['n']:5} grid={r['grid']:7} "
            f"lean={r['lean_mb']:+7.1f} swing={r['swing_mb']:6.1f} jitter={r['jitter_mb']:6.1f}"
        )
    verdict_long = {
        "RIGID": "RIGID — dead grid, no pocket (robotic)",
        "PASS": "PASS — coordinated pocket (shared lean / swing / complementary role leans)",
        "FAIL": "FAIL — independent error dominates (voices trip over each other)",
        "EMPTY": "EMPTY — no voices with enough notes",
    }
    print(f"  VERDICT: {verdict_long.get(s['verdict'], s['verdict'])}")
    print(
        f"  (negative reference {NEGATIVE_REFERENCE['name']}: {NEGATIVE_REFERENCE['note']} -> must land FAIL)"
    )


if __name__ == "__main__":
    main()
