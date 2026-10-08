#!/usr/bin/env python3
"""Shared, honest instrument core for the HumanMusic beef-up measurement rig.

Three jobs, one place so every tool is the SAME instrument:

  1. `parse_smf`  — a Standard MIDI File parser that is honest about what it can and
     cannot read (format 0/1, running status, note on/off incl. vel-0-is-off, tempo and
     time-signature meta, sysex skip). It **refuses SMPTE division explicitly** rather than
     silently pretending it is tpq=480 (the old rig's bug: a parser error would have become a
     musical "law"). Robust to truncation — raises a clear error, never a bogus reading.

  2. `read_probe_tsv` — reads a `human_music_probe` / gap_dump TSV (engine output) into the
     same voice structure, so the engine is judged by the same instrument as the corpus.

  3. the **lattice-aware timing decomposition** (`decompose`) that separates *intentional
     shared structure* from *unexplained independent error*:

         performed = structural_lattice_position
                   + common_band_lean      (B : whole-ensemble, phase-independent)
                   + groove/swing shape    (per metric-phase systematic, e.g. late offbeats)
                   + role differential lean (seen as cross-voice scatter of B_v)
                   + residual jitter        (eps : the irreducible per-note wobble)

     Crucially it (a) chooses each voice's rhythmic grid (16th vs 8th/16th-triplet) BEFORE
     measuring deviation, so triplets/swing are not mislabelled as jitter; (b) exposes the
     common band lean as its own first-class number, so a band sitting +8 mb behind TOGETHER
     reads as a COORDINATED LEAN, not "rigid"; (c) measures jitter as the wobble left AFTER
     the voice's own groove shape is removed.

Units: everything timing is in BEATS internally; reported in MILIBEAT (1 beat = 1000 mb,
a sixteenth = 250 mb). Dependencies: python3 stdlib + numpy only — nothing enters the shipped
library closure. Reference MIDIs are third-party and never committed; point tools at a local file.
"""
import struct
import numpy as np

# ---------------------------------------------------------------------------
# 1. Honest Standard MIDI File parser
# ---------------------------------------------------------------------------


class SmfError(Exception):
    """Malformed, truncated, or unsupported (e.g. SMPTE) MIDI — raised, never papered over."""


def _varlen(data, i, end):
    v = 0
    for _ in range(4):  # a VLQ is at most 4 bytes
        if i >= end:
            raise SmfError("truncated variable-length quantity")
        b = data[i]
        i += 1
        v = (v << 7) | (b & 0x7F)
        if not (b & 0x80):
            return v, i
    raise SmfError("variable-length quantity too long")


def parse_smf(path):
    """Parse an SMF into {"meta": {...}, "voices": {voice_key: {...}}, "notes": [...]}.

    A *voice_key* is `t{track}c{channel}` — pure provenance (track + channel). Drums (channel 9,
    General MIDI percussion) keep their own voice_key AND carry `is_drum=True` + the GM drum lane.
    Never a musical-role label (bass/keys/lead): track+channel is an observation, not a guarantee.
    """
    data = open(path, "rb").read()
    if data[:4] != b"MThd":
        raise SmfError(f"{path}: not a MIDI file (no MThd)")
    if len(data) < 14:
        raise SmfError(f"{path}: truncated header")
    hlen = struct.unpack(">I", data[4:8])[0]
    fmt, ntrks, division = struct.unpack(">HHH", data[8:14])
    if division & 0x8000:
        # SMPTE time division (negative frames/sec + ticks/frame). The old rig silently used 480.
        fps = 256 - (division >> 8)
        tpf = division & 0xFF
        raise SmfError(
            f"{path}: SMPTE division (fps={fps}, ticks/frame={tpf}) is not supported — refused "
            "rather than silently treated as tpq=480. Convert to metrical (tpq) division first."
        )
    tpq = division
    if tpq <= 0:
        raise SmfError(f"{path}: non-positive tpq {tpq}")

    i = 8 + hlen
    notes = []  # dicts: track, channel, pitch, start_beat, dur_beat, velocity, is_drum
    tempos = []  # (beat, usec_per_quarter)
    time_sigs = []  # (beat, numerator, denominator)
    track = 0
    while i + 8 <= len(data) and data[i : i + 4] == b"MTrk":
        trklen = struct.unpack(">I", data[i + 4 : i + 8])[0]
        i += 8
        end = min(i + trklen, len(data))
        abstick = 0
        status = None
        active = {}  # (channel, pitch) -> [start_tick, ...] (stack, for overlapping same pitch)
        while i < end:
            dt, i = _varlen(data, i, end)
            abstick += dt
            if i >= end:
                break
            if data[i] & 0x80:
                status = data[i]
                i += 1
            if status is None:
                raise SmfError(f"{path}: running status with no prior status byte")
            hi = status & 0xF0
            ch = status & 0x0F
            if hi in (0x80, 0x90, 0xA0, 0xB0, 0xE0):
                if i + 2 > end:
                    raise SmfError(f"{path}: truncated channel message")
                d1, d2 = data[i], data[i + 1]
                i += 2
                if hi == 0x90 and d2 > 0:  # note-on
                    active.setdefault((ch, d1), []).append((abstick, d2))
                elif hi == 0x80 or (hi == 0x90 and d2 == 0):  # note-off (incl. vel-0 note-on)
                    st = active.get((ch, d1))
                    if st:
                        start, vel = st.pop(0)  # FIFO: pair with the oldest held onset
                        notes.append(
                            {
                                "track": track,
                                "channel": ch,
                                "pitch": d1,
                                "start_beat": start / tpq,
                                "dur_beat": max(1, abstick - start) / tpq,
                                "velocity": vel / 127.0,
                                "is_drum": ch == 9,
                            }
                        )
            elif hi in (0xC0, 0xD0):  # program change / channel pressure: one data byte
                i += 1
            elif status == 0xFF:  # meta
                if i >= end:
                    break
                meta_type = data[i]
                i += 1
                mlen, i = _varlen(data, i, end)
                payload = data[i : i + mlen]
                if meta_type == 0x51 and mlen == 3:  # set tempo
                    tempos.append((abstick / tpq, struct.unpack(">I", b"\x00" + payload)[0]))
                elif meta_type == 0x58 and mlen >= 2:  # time signature
                    time_sigs.append((abstick / tpq, payload[0], 1 << payload[1]))
                i += mlen
            elif status in (0xF0, 0xF7):  # sysex
                mlen, i = _varlen(data, i, end)
                i += mlen
            else:
                raise SmfError(f"{path}: unknown status byte {status:#04x}")
        i = end
        track += 1

    if not time_sigs:
        time_sigs = [(0.0, 4, 4)]
    if not tempos:
        tempos = [(0.0, 500000)]
    return {
        "meta": {"format": fmt, "ntrks": ntrks, "tpq": tpq, "tempos": tempos, "time_sigs": time_sigs},
        "notes": notes,
        "voices": _group_voices(notes),
    }


def _group_voices(notes):
    voices = {}
    for n in notes:
        key = f"t{n['track']}c{n['channel']}"
        v = voices.setdefault(
            key, {"onsets": [], "pitches": [], "velocities": [], "durations": [], "is_drum": n["channel"] == 9}
        )
        v["onsets"].append(n["start_beat"])
        v["pitches"].append(n["pitch"])
        v["velocities"].append(n["velocity"])
        v["durations"].append(n["dur_beat"])
    return voices


# General MIDI percussion → coarse lane families (kick / snare-clap / hats-cymbals / toms / perc).
# Split so the band-vs-drum and backbeat timing relations survive; NOT collapsed to one pseudo-voice.
GM_DRUM_LANES = {
    "kick": {35, 36},
    "snare": {37, 38, 40, 39},  # snares + hand clap (39) + side stick (37)
    "hats": {42, 44, 46, 49, 51, 52, 53, 55, 57, 59},  # hats + cymbals/rides
    "toms": {41, 43, 45, 47, 48, 50},
    "perc": set(),  # everything else falls here
}


def drum_lane(pitch):
    for lane, pitches in GM_DRUM_LANES.items():
        if pitch in pitches:
            return lane
    return "perc"


def smf_timing_voices(parsed, min_n=12, split_drums=True):
    """From a parsed SMF, build {voice_key: onsets[]} for the timing decomposition.

    Pitched voices keep their `t{trk}c{ch}` provenance key. The drum channel is split by GM lane
    (kick/snare/hats/toms/perc) so the backbeat-vs-kick-vs-band relation is measurable — collapsing
    the whole kit into one voice would destroy exactly the coordination we are trying to see.
    """
    out = {}
    for n in parsed["notes"]:
        if n["is_drum"] and split_drums:
            key = f"drum:{drum_lane(n['pitch'])}"
        elif n["is_drum"]:
            key = "drum:all"
        else:
            key = f"t{n['track']}c{n['channel']}"
        out.setdefault(key, []).append(n["start_beat"])
    return {k: sorted(v) for k, v in out.items() if len(v) >= min_n}


# ---------------------------------------------------------------------------
# 2. Engine-output (probe / gap_dump) TSV reader — same voice structure
# ---------------------------------------------------------------------------


def read_probe_tsv(path, min_n=12, include_drums=True):
    """Read a human_music_probe / gap_dump TSV into {voice_key: onsets[]} for the decomposition.

    Handles BOTH the legacy gap_dump schema (source voice kind start_beat dur_beats pitch velocity)
    and the enriched human_music_probe schema (extra provenance columns). Drums are INCLUDED
    (the old reader dropped `kind=drum`), keyed `drum:<lane>` from the `voice` column.
    """
    voices = {}
    with open(path) as f:
        header = f.readline().rstrip("\n").split("\t")
        idx = {k: n for n, k in enumerate(header)}
        for line in f:
            p = line.rstrip("\n").split("\t")
            if len(p) < len(header):
                continue
            kind = p[idx["kind"]]
            if kind == "drum":
                if not include_drums:
                    continue
                key = f"drum:{p[idx['voice']]}"
            else:
                key = p[idx["voice"]]
            voices.setdefault(key, []).append(float(p[idx["start_beat"]]))
    return {k: sorted(v) for k, v in voices.items() if len(v) >= min_n}


def read_probe_rows(path):
    """Full typed rows for coherence analysis (keeps pitch/velocity/provenance where present)."""
    rows = []
    with open(path) as f:
        header = f.readline().rstrip("\n").split("\t")
        idx = {k: n for n, k in enumerate(header)}
        for line in f:
            p = line.rstrip("\n").split("\t")
            if len(p) < len(header):
                continue
            row = {k: p[idx[k]] for k in idx}
            for num in ("start_beat", "dur_beats", "velocity"):
                if num in row:
                    try:
                        row[num] = float(row[num])
                    except ValueError:
                        row[num] = None
            if "pitch" in row:
                try:
                    row["pitch"] = int(row["pitch"])
                except ValueError:
                    row["pitch"] = None
            rows.append(row)
    return rows, header


# ---------------------------------------------------------------------------
# 3. Lattice-aware timing decomposition
# ---------------------------------------------------------------------------

# candidate subdivision grids, in beats: straight 16th, 8th-triplet, 16th-triplet.
_GRIDS = {"q16": 0.25, "trip8": 1.0 / 3.0, "trip16": 1.0 / 6.0}


def _choose_grid(onsets):
    """Pick the subdivision grid that best fits a voice BEFORE measuring deviation, so triplet or
    swung material is not charged as jitter. Returns (grid_name, grid_beats, mean_abs_resid_beats)."""
    best = None
    for name, g in _GRIDS.items():
        resid = onsets - np.round(onsets / g) * g
        score = float(np.mean(np.abs(resid)))
        if best is None or score < best[2]:
            best = (name, g, score)
    return best


def decompose_voice(onsets):
    """Decompose one voice's onsets into (common lean B_v, groove/swing shape, residual jitter).

    Returns a dict in MILIBEAT:
      grid        : chosen rhythmic class (q16 / trip8 / trip16)
      lean_mb     : B_v, the voice's phase-independent systematic offset (mean residual)
      swing_mb    : spread of the per-metric-phase systematic displacement (late offbeats etc.)
      jitter_mb   : std of the residual AFTER removing the voice's own per-phase groove shape
      n           : note count
    """
    s = np.asarray(sorted(onsets), dtype=float)
    name, g, _ = _choose_grid(s)
    snapped_idx = np.round(s / g)
    resid = s - snapped_idx * g  # beats
    per_beat = int(round(1.0 / g))  # metric phases per beat: 4 (q16), 3 (trip8), 6 (trip16)
    phase = (snapped_idx.astype(int)) % per_beat
    occupied = [p for p in range(per_beat) if (phase == p).any()]
    sys = np.zeros(per_beat)
    for p in occupied:
        sys[p] = np.median(resid[phase == p])
    # The LEAN is where the pulse sits — the DOWNBEAT (phase 0) displacement, phase-independent.
    # Anchoring to the downbeat keeps a *shared* swing (phase-dependent, weak-beats late) out of
    # the common-lean and out of the cross-voice scatter — swing is lawful groove, not a lean.
    lean = sys[0] if (phase == 0).any() else float(np.mean([sys[p] for p in occupied]))
    occ = [sys[p] for p in occupied]
    swing = (max(occ) - min(occ)) if occ else 0.0  # groove shape, independent of constant lean
    groove_removed = resid - sys[phase]  # jitter = wobble after the voice's own groove is removed
    return {
        "grid": name,
        "lean_mb": float(lean * 1000.0),
        "swing_mb": float(swing * 1000.0),
        "jitter_mb": float(np.std(groove_removed) * 1000.0),
        "n": int(s.size),
    }


# Recalibrated acceptance band — set by the ORACLE principle (CALIBRATION.md): the ceilings sit
# ABOVE the heard-coherent calibration envelope (max scatter 18.1 mb = I Can't Go For That's
# dragged-backbeat pocket; max jitter 15.2 mb = Tom Sawyer) and BELOW the declared negative
# (Attention: scatter 37.1, jitter 21.7). Scatter is the strong discriminator — Attention's
# pathology is independent per-voice offsets, which this decomposition reads as cross-voice scatter.
RIGID_FLOOR_MB = 3.0  # below this much *structure* AND jitter = dead grid
SCATTER_CEIL_MB = 26.0  # cross-voice lean scatter above this = voices pulling apart
JITTER_CEIL_MB = 20.0  # within-voice residual wobble above this = sloppy


def decompose(voices):
    """Decompose a whole take. Returns (summary dict, per-voice rows).

    summary keys (all milibeat unless noted):
      band_lean   : B, mean of per-voice leans — the ensemble's shared pocket lean
      scatter     : std of per-voice leans — coordinated (small) vs pulling-apart (large)
      jitter      : median per-voice residual jitter — the irreducible wobble
      swing       : median per-voice groove/swing spread — lawful phase displacement
      verdict     : RIGID / PASS / FAIL  (see thresholds above)
    """
    rows = []
    for v, onsets in voices.items():
        d = decompose_voice(onsets)
        d["voice"] = v
        rows.append(d)
    if not rows:
        return {"band_lean": 0.0, "scatter": 0.0, "jitter": 0.0, "swing": 0.0, "verdict": "EMPTY", "n_voices": 0}, []
    leans = np.array([r["lean_mb"] for r in rows])
    jits = np.array([r["jitter_mb"] for r in rows])
    swings = np.array([r["swing_mb"] for r in rows])
    band_lean = float(np.mean(leans))
    scatter = float(np.std(leans))
    jitter = float(np.median(jits))
    swing = float(np.median(swings))
    structure = max(abs(band_lean), scatter, swing)
    if jitter > JITTER_CEIL_MB or scatter > SCATTER_CEIL_MB:
        verdict = "FAIL"  # independent error dominates — voices trip over each other
    elif structure < RIGID_FLOOR_MB and jitter < RIGID_FLOOR_MB:
        verdict = "RIGID"  # dead grid, no pocket at all
    else:
        verdict = "PASS"  # coordinated: shared lean / swing / complementary role leans, bounded wobble
    grids = {}
    for r in rows:
        grids[r["grid"]] = grids.get(r["grid"], 0) + 1
    summary = {
        "band_lean": band_lean,
        "scatter": scatter,
        "jitter": jitter,
        "swing": swing,
        "structure": structure,
        "verdict": verdict,
        "n_voices": len(rows),
        "grids": grids,
    }
    rows.sort(key=lambda r: -r["n"])
    return summary, rows
