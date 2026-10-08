#!/usr/bin/env python3
"""Groove / timing-coordination metrics for the HumanMusic beef-up acceptance suite.

Measures the two numbers that decide whether a multi-voice take holds a POCKET or
"trips over itself": cross-voice offset scatter and within-voice jitter (both in
milibeat; 1 beat = 1000 mb, a sixteenth = 250 mb). Works on either a Standard MIDI
File (reference songs) or a HumanMusic gap_dump TSV (engine output), so the engine is
judged by the same instrument as the corpus.

    python3 groove_metrics.py <file.mid | file.notes.tsv>

Calibrated bands (from docs/HUMANMUSIC_BEEFUP_ACCEPTANCE.md):
  PASS  : cross-voice scatter <= 12 mb AND within-voice jitter <= 16 mb   (funk-ref pocket)
  FAIL  : the NEGATIVE REFERENCE "Attention" (AUD_DW0160) sits at ~27 mb scatter /
          ~47 mb jitter, floating between the 16th and triplet grids — the engine's
          feel layer must NEVER score in that region.

Dependencies: python3 + numpy (no mido/music21). Reference MIDIs are NOT committed
(third-party copyright); point this at a local file.
"""
import sys
import os
import struct
import numpy as np

# ---- calibrated acceptance band (see the acceptance doc) ----
# A real pocket lives in a BAND, not at zero: the funk references carry a small, COORDINATED
# lean (one anchor voice + complementary leans), so scatter is low but not nil and jitter is
# bounded but nonzero. There are TWO failure modes:
#   - too LOOSE  (scatter > CEIL or jitter > CEIL): voices trip over each other  (Attention)
#   - too RIGID  (scatter ~0 AND jitter ~0): dead grid, no pocket at all          (HumanMusic today)
SCATTER_CEIL_MB = 12.0   # above this = scattered / tripping
JITTER_CEIL_MB = 16.0    # above this = sloppy / tripping
RIGID_FLOOR_MB = 1.0     # scatter AND jitter both below this = robotic, no pocket
NEGATIVE_REFERENCE = {"name": "Attention (AUD_DW0160)", "scatter_mb": 26.0, "jitter_mb": 45.0}


# ---- Standard MIDI File parse (format 0/1, running status, note on/off) ----
def _varlen(data, i):
    v = 0
    while True:
        b = data[i]
        i += 1
        v = (v << 7) | (b & 0x7F)
        if not (b & 0x80):
            return v, i


def parse_smf(path):
    data = open(path, "rb").read()
    assert data[:4] == b"MThd", f"{path}: not a MIDI file"
    hlen = struct.unpack(">I", data[4:8])[0]
    _, _, division = struct.unpack(">HHH", data[8:14])
    tpq = division if not (division & 0x8000) else 480
    i = 8 + hlen
    notes = []  # (track, channel, start_tick, dur_tick)
    track = 0
    while i + 8 <= len(data) and data[i : i + 4] == b"MTrk":
        trklen = struct.unpack(">I", data[i + 4 : i + 8])[0]
        i += 8
        end = i + trklen
        abstick = 0
        status = None
        active = {}
        while i < end:
            dt, i = _varlen(data, i)
            abstick += dt
            if data[i] & 0x80:
                status = data[i]
                i += 1
            if status is None:
                i += 1
                continue
            hi = status & 0xF0
            ch = status & 0x0F
            if hi in (0x80, 0x90, 0xA0, 0xB0, 0xE0):
                d1, d2 = data[i], data[i + 1]
                i += 2
                if hi == 0x90 and d2 > 0:
                    active.setdefault((ch, d1), []).append(abstick)
                elif hi == 0x80 or (hi == 0x90 and d2 == 0):
                    st = active.get((ch, d1))
                    if st:
                        start = st.pop(0)
                        notes.append((track, ch, start, max(1, abstick - start)))
            elif hi in (0xC0, 0xD0):
                i += 1
            elif status == 0xFF:
                i += 1
                mlen, i = _varlen(data, i)
                i += mlen
            elif status in (0xF0, 0xF7):
                mlen, i = _varlen(data, i)
                i += mlen
            else:
                i += 1
        i = end
        track += 1
    # voice = track+channel; drop channel 9 (drums analyzed separately would need its own call)
    voices = {}
    for trk, ch, st, _dur in notes:
        voices.setdefault(f"t{trk}c{ch}", []).append(st / tpq)
    return voices


def read_tsv(path):
    """HumanMusic gap_dump TSV -> {voice: [start_beat,...]} for pitched notes."""
    voices = {}
    with open(path) as f:
        header = f.readline().rstrip("\n").split("\t")
        idx = {k: n for n, k in enumerate(header)}
        for line in f:
            p = line.rstrip("\n").split("\t")
            if len(p) < len(header) or p[idx["kind"]] != "note":
                continue
            voices.setdefault(p[idx["voice"]], []).append(float(p[idx["start_beat"]]))
    return voices


def groove_metrics(voices, min_n=15):
    """Returns (cross_voice_scatter_mb, within_voice_jitter_mb, per_voice_rows)."""
    rows = []
    for v, st in voices.items():
        if len(st) < min_n:
            continue
        s = np.array(sorted(st))
        resid = (s - np.round(s * 4) / 4) * 1000.0  # signed residual to nearest 16th, mb
        rows.append((v, len(s), float(resid.mean()), float(resid.std())))
    if not rows:
        return 0.0, 0.0, []
    means = np.array([r[2] for r in rows])
    stds = np.array([r[3] for r in rows])
    return float(means.std()), float(np.median(stds)), rows


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    path = sys.argv[1]
    voices = read_tsv(path) if path.endswith(".tsv") else parse_smf(path)
    scatter, jitter, rows = groove_metrics(voices)
    rows.sort(key=lambda r: -r[1])
    print(f"{os.path.basename(path)}: {len(rows)} voices >= 15 notes")
    print(f"  cross-voice offset scatter : {scatter:6.1f} mb  (pocket band: {RIGID_FLOOR_MB}..{SCATTER_CEIL_MB})")
    print(f"  within-voice jitter        : {jitter:6.1f} mb  (pocket band: >{RIGID_FLOOR_MB}, <= {JITTER_CEIL_MB})")
    for v, n, mean, std in rows[:10]:
        print(f"    {v:8} n={n:5} offset={mean:+6.1f} jitter={std:5.1f}")
    if scatter > SCATTER_CEIL_MB or jitter > JITTER_CEIL_MB:
        verdict = "FAIL — loose/scattered (voices trip over each other)"
    elif scatter < RIGID_FLOOR_MB and jitter < RIGID_FLOOR_MB:
        verdict = "RIGID — dead grid, no pocket (robotic)"
    else:
        verdict = "PASS — coordinated pocket"
    neg = NEGATIVE_REFERENCE
    print(f"  VERDICT: {verdict}")
    print(f"  (negative reference {neg['name']}: ~{neg['scatter_mb']} mb scatter / ~{neg['jitter_mb']} mb jitter = too loose)")


if __name__ == "__main__":
    main()
