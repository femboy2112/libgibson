#!/usr/bin/env python3
"""Round XIIIb audio-domain witness: the strongest spectral peaks (as note names, dB relative to
the frame maximum, 50 Hz - 2.5 kHz) of a rendered WAV at given times.

Usage: peaks.py FILE.wav WINDOW_SECS T1 [T2 ...]
"""
import sys, wave
import numpy as np
from scipy.signal import find_peaks

NAMES = "C C# D D# E F F# G G# A A# B".split()


def name(f):
    m = 69 + 12 * np.log2(f / 440.0); r = int(round(m))
    return f"{NAMES[r % 12]}{r // 12 - 1}"


w = wave.open(sys.argv[1])
x = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float64) / 32768.0
x = x.reshape(-1, w.getnchannels()).mean(axis=1); sr = w.getframerate()
win = float(sys.argv[2])
for t in map(float, sys.argv[3:]):
    n = int(win * sr); seg = x[int(t * sr):int(t * sr) + n] * np.hanning(n)
    X = np.abs(np.fft.rfft(seg, 8 * n)); f = np.fft.rfftfreq(8 * n, 1 / sr)
    band = (f > 50) & (f < 2500); X, f = X[band], f[band]
    pk, _ = find_peaks(X, height=X.max() * 10 ** (-30 / 20), distance=8)
    top = sorted(sorted(pk, key=lambda i: -X[i])[:14], key=lambda i: f[i])
    print(f"t={t:6.2f}s  " + " ".join(f"{name(f[i])}({20 * np.log10(X[i] / X.max()):.0f})" for i in top))
