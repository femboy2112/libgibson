#!/usr/bin/env python3
"""Round XIIIb audio-domain witness: how long one pitch actually sounds. Level (dB re the file's
peak) of the partial at a given MIDI note, in 20 ms frames (Hann-windowed single-bin DFT), over
a time span of a rendered WAV; prints the frames and the seconds spent above -24 and -30 dB
(the floor around -38 dB is the neighbouring pad C5 leaking into a 40 ms window).

Usage: tone_energy.py FILE.wav MIDI T0 T1
"""
import sys, wave
import numpy as np

w = wave.open(sys.argv[1])
x = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float64) / 32768.0
x = x.reshape(-1, w.getnchannels()).mean(axis=1); sr = w.getframerate()
midi, t0, t1 = int(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4])
f = 440.0 * 2 ** ((midi - 69) / 12)
n = int(0.04 * sr); hop = int(0.02 * sr); win = np.hanning(n)
ph = np.exp(-2j * np.pi * f * np.arange(n) / sr)
lv = []
for s in range(int(t0 * sr), int(t1 * sr), hop):
    lv.append((s / sr, 20 * np.log10(abs(np.sum(x[s:s + n] * win * ph)) / (n / 4) + 1e-12)))
print(" ".join(f"{t:.2f}:{v:.0f}" for t, v in lv))
for th in (-24, -30):
    print(f"seconds above {th} dB: {sum(0.02 for _, v in lv if v > th):.2f}")
