#!/usr/bin/env python3
"""Synthetic timing controls (§6) — calibrate and self-test the groove instrument before it is
trusted on real songs. Each control is a deterministic, known-intent symbolic performance; the
analyzer must classify it SENSIBLY, distinguishing intentional shared structure from error:

  A perfectly quantized            -> RIGID   (dead grid)
  B whole band +8 mb together      -> PASS    (coordinated LEAN, not rigid, not sloppy)
  C shared swung offbeats          -> PASS    (lawful groove, not jitter)
  D small stable role leans        -> PASS    (pocket / bounded differential lean)
  E independent +/-40 mb per note  -> FAIL    (sloppy / voices tripping)
  F mixed 16th + triplet voices    -> not FAIL (lawful rhythmic vocabulary, triplets recognised)

Run directly: prints the table and exits non-zero if any control is misclassified.
    python3 synthetic_controls.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # this (trusted) script dir, for -I
import numpy as np  # noqa: E402

from hm_corpus import decompose  # noqa: E402

RNG_SEED = 2112


def _band(subdiv=0.25, bars=16, beats_per_bar=4):
    """Four generic voices on a shared grid, as beat positions. Pure provenance names (no roles)."""
    nbeats = bars * beats_per_bar
    quarters = np.arange(0, nbeats, 1.0)
    eighths = np.arange(0, nbeats, 0.5)
    sixteenths = np.arange(0, nbeats, subdiv)
    return {
        "t0c0": quarters.copy(),  # a sparse voice (think: bass on the beat)
        "t1c1": eighths.copy(),  # an 8th voice (think: comping)
        "t2c2": sixteenths.copy(),  # a busy 16th voice (think: lead/hat)
        "t3c3": np.sort(np.concatenate([quarters, quarters + 0.5])).copy(),  # backbeat-ish 8ths
    }


def control_A():
    return _band()


def control_B(lean=0.008):
    return {k: v + lean for k, v in _band().items()}


def control_C(swing=0.060):
    """Shared swung transport: every voice pushes the 'and' of each beat late by the same amount."""
    out = {}
    for k, v in _band().items():
        w = v.copy()
        offbeat = np.isclose(w % 1.0, 0.5)
        w[offbeat] += swing
        out[k] = w
    return out


def control_D():
    """Small, stable, DIFFERENT lean per voice — a pocket of complementary leans (bounded)."""
    leans = {"t0c0": 0.005, "t1c1": 0.000, "t2c2": -0.003, "t3c3": 0.009}
    return {k: v + leans[k] for k, v in _band().items()}


def control_E(noise=0.040):
    rng = np.random.default_rng(RNG_SEED)
    return {k: v + rng.normal(0.0, noise, size=v.shape) for k, v in _band().items()}


def control_F():
    """Mixed lattice: two voices straight 16ths, two voices perfect 8th-triplets. Lawful vocab."""
    nbeats = 16 * 4
    straight = np.arange(0, nbeats, 0.25)
    triplets = np.arange(0, nbeats, 1.0 / 3.0)
    return {
        "t0c0": straight.copy(),
        "t1c1": np.arange(0, nbeats, 0.5),
        "t2c2": triplets.copy(),
        "t3c3": triplets.copy(),
    }


CONTROLS = [
    ("A perfect-quantized", control_A, {"RIGID"}),
    ("B whole-band +8mb", control_B, {"PASS"}),
    ("C shared swing", control_C, {"PASS"}),
    ("D role leans (pocket)", control_D, {"PASS"}),
    ("E independent +/-40mb", control_E, {"FAIL"}),
    ("F mixed triplets", control_F, {"RIGID", "PASS"}),  # must NOT be FAIL; triplets recognised
]


def run():
    ok = True
    print(f"{'control':24} {'verdict':7} {'B':>7} {'scatter':>8} {'jitter':>7} {'swing':>7}  grids")
    for name, make, allowed in CONTROLS:
        voices = {k: np.asarray(v, dtype=float) for k, v in make().items()}
        s, _ = decompose(voices)
        good = s["verdict"] in allowed
        if name.startswith("F") and "trip8" not in s["grids"]:
            good = False  # triplet vocabulary must be recognised, not charged as jitter
        ok = ok and good
        flag = "" if good else "  <-- MISCLASSIFIED"
        print(
            f"{name:24} {s['verdict']:7} {s['band_lean']:+7.1f} {s['scatter']:8.1f} "
            f"{s['jitter']:7.1f} {s['swing']:7.1f}  {s['grids']}{flag}"
        )
    print("\nSELF-TEST:", "PASS — every control classified as intended" if ok else "FAIL — see above")
    return ok


if __name__ == "__main__":
    sys.exit(0 if run() else 1)
