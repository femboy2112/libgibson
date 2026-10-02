#!/usr/bin/env python3
"""Round XIIIb audio-domain grader: Sethares/Plomp-Levelt sensory roughness of rendered WAVs.

Frames of 0.16 s every 0.05 s (Hann), spectral peaks below 4 kHz above ONE absolute threshold
per file (40 dB under the file's loudest partial), pairwise roughness summed with min-amplitude
weighting. The threshold is per file, not per frame: a per-frame threshold admits quiet partials
exactly where a loud note was removed and reports the removal as extra roughness (observed on the
first calibration: +2.1% where the lead's C#5 was shortened). Run on the lab's `--pitched`
renders (drums and SFX muted), comparing the mass (Round XIII) and tension (Round XIIIb) arms
over the windows the maintainer and the audit point at.

Usage: roughness.py DIR   (expects DIR/{swiss,black_ice}_{mass,tension}.pitched.wav)
"""
import sys, wave
import numpy as np
from scipy.signal import find_peaks


def load(p):
    w = wave.open(p)
    x = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float64) / 32768.0
    return x.reshape(-1, w.getnchannels()).mean(axis=1), w.getframerate()


def curve(x, sr, win=0.16, hop=0.05, fmax=4000.0, floor_db=40.0):
    n, h = int(win * sr), int(hop * sr)
    w, f = np.hanning(n), np.fft.rfftfreq(n, 1 / sr)
    band = f < fmax
    starts = range(0, len(x) - n, h)
    spectra = [(np.abs(np.fft.rfft(x[s:s + n] * w)) / (n / 4))[band] for s in starts]
    floor = max(m.max() for m in spectra) * 10 ** (-floor_db / 20)
    t, r = [], []
    for s, mag in zip(starts, spectra):
        rough = 0.0
        pk, _ = find_peaks(mag, height=floor)
        fa, aa = f[band][pk], mag[pk]
        for i in range(len(fa)):
            for j in range(i + 1, len(fa)):
                f1, f2 = min(fa[i], fa[j]), max(fa[i], fa[j])
                sx = 0.24 / (0.0207 * f1 + 18.96)
                rough += min(aa[i], aa[j]) * (np.exp(-3.5 * sx * (f2 - f1)) - np.exp(-5.75 * sx * (f2 - f1)))
        t.append(s / sr + win / 2); r.append(rough)
    return np.array(t), np.array(r)


if __name__ == "__main__":
    d = sys.argv[1]
    spans = {"swiss": [(14.3, 16.2, "Cmaj7 pad, the 16 s chord"), (18.4, 20.3, "Cmaj7 pad"),
                       (22.4, 24.3, "Cmaj7 pad"), (42.5, 45.5, "the 43-45 s window"), (0, 62, "whole song")],
             "black_ice": [(8.2, 10.9, "Am9 pad"), (19.2, 21.8, "Am9 pad + lead C5"),
                           (30.0, 32.7, "Am9 pad"), (16.6, 19.1, "Am6, lead C#5s"), (0, 82, "whole song")]}
    for world, windows in spans.items():
        c = {arm: curve(*load(f"{d}/{world}_{arm}.pitched.wav")) for arm in ("mass", "tension")}
        for a, b, what in windows:
            m = {arm: 1000 * r[(t >= a) & (t < b)].mean() for arm, (t, r) in c.items()}
            print(f"{world:9s} {a:5.1f}-{b:5.1f} s  roughness x1000 mass={m['mass']:6.2f} "
                  f"tension={m['tension']:6.2f} ({100 * (m['tension'] - m['mass']) / m['mass']:+5.1f}%)  {what}")
