#!/usr/bin/env python3
"""Reproducible coherence analysis (§8) — the durable implementation behind the acceptance doc's
`selfsim_peak`, `longrange_recur`, and opening-motif-restated claims, which previously lived only
in session scratch. Works on a local MIDI (reference) or a human_music_probe / gap_dump TSV (engine).

    python3 -I coherence_metrics.py <file.mid | file.notes.tsv>

Reports:
  selfsim_peak     : strongest bar-to-bar self-similarity of the pitched TEXTURE (harmonic recurrence)
  longrange_recur  : fraction of bars that recur near-identically more than 8 beats later
  opening_literal  : does the lead's opening 4-note PITCH figure return later (literal recurrence)?
  opening_transpos : does its INTERVAL figure return later (transposed recurrence)?
  vel_std (lead)   : within-lead velocity spread (gap-1 dynamics signature)
  distinct_ioi/dur : rhythmic vocabulary size (gap-3)

Aggregate, non-reconstructive signatures only — never commit note content from copyrighted sources.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # trusted script dir, for -I
import numpy as np  # noqa: E402

from hm_corpus import SmfError, parse_smf, read_probe_rows  # noqa: E402

SIM_RECUR = 0.90  # cosine >= this between two bars = "the same material returns"


def _bar_beats(parsed_meta):
    num, den = parsed_meta["time_sigs"][0][1], parsed_meta["time_sigs"][0][2]
    return num * 4.0 / den


def voices_from_midi(path):
    p = parse_smf(path)
    bar = _bar_beats(p["meta"])
    voices = {}
    for n in p["notes"]:
        if n["is_drum"]:
            continue
        key = f"t{n['track']}c{n['channel']}"
        voices.setdefault(key, []).append((n["start_beat"], n["pitch"], n["velocity"], n["dur_beat"]))
    return voices, bar


def voices_from_tsv(path):
    rows, _ = read_probe_rows(path)
    voices = {}
    for r in rows:
        if r.get("kind") != "note" or r.get("pitch") is None:
            continue
        key = r["voice"]
        voices.setdefault(key, []).append((r["start_beat"], r["pitch"], r.get("velocity") or 0.0, r.get("dur_beats") or 0.0))
    return voices, 4.0  # HumanMusic is 4/4


def pick_lead(voices):
    """Lead = explicit 'lead' voice if present, else the non-trivial voice with highest mean pitch."""
    if "lead" in voices and len(voices["lead"]) >= 8:
        return "lead"
    cand = {k: v for k, v in voices.items() if len(v) >= 12}
    if not cand:
        cand = {k: v for k, v in voices.items() if len(v) >= 6} or voices
    return max(cand, key=lambda k: np.mean([p for _, p, _, _ in cand[k]]))


def bar_pc_matrix(all_notes, bar, total_beats):
    """Per-bar normalized 12-bin pitch-class histogram of the whole pitched texture."""
    nbars = max(1, int(np.ceil(total_beats / bar)))
    M = np.zeros((nbars, 12))
    for start, pitch, _vel, _dur in all_notes:
        b = int(start // bar)
        if 0 <= b < nbars:
            M[b, pitch % 12] += 1.0
    norms = np.linalg.norm(M, axis=1, keepdims=True)
    norms[norms == 0] = 1.0
    return M / norms


def selfsim(M):
    S = M @ M.T
    nbars = S.shape[0]
    peak, recurs = 0.0, 0
    for i in range(nbars):
        offdiag = [S[i, j] for j in range(nbars) if abs(i - j) >= 2]  # ignore self + adjacent
        if offdiag:
            peak = max(peak, max(offdiag))
        # long-range: a near-identical bar starting MORE THAN 8 beats (2 bars) later
        far = [S[i, j] for j in range(i + 2, nbars)]
        if any(s >= SIM_RECUR for s in far):
            recurs += 1
    longrange = recurs / max(1, M.shape[0])
    return float(peak), float(longrange)


def opening_return(lead, bar):
    """Does the lead's opening 4-note figure return > 8 beats later, literally and/or transposed?"""
    seq = sorted(lead)
    if len(seq) < 8:
        return False, False
    pitches = [p for _, p, _, _ in seq]
    starts = [s for s, _, _, _ in seq]
    head = pitches[:4]
    head_iv = [b - a for a, b in zip(head, head[1:])]
    lit = trn = False
    for i in range(1, len(pitches) - 3):
        if starts[i] - starts[0] <= 8.0:
            continue
        win = pitches[i : i + 4]
        if win == head:
            lit = True
        if [b - a for a, b in zip(win, win[1:])] == head_iv:
            trn = True
        if lit and trn:
            break
    return lit, trn


def rhythmic_vocab(all_notes):
    starts = sorted(s for s, _, _, _ in all_notes)
    iois = [round(b - a, 3) for a, b in zip(starts, starts[1:]) if 0 < b - a < 8]
    durs = [round(d, 3) for _, _, _, d in all_notes if d > 0]
    return len(set(iois)), len(set(durs))


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    path = sys.argv[1]
    try:
        voices, bar = voices_from_tsv(path) if path.endswith(".tsv") else voices_from_midi(path)
    except SmfError as e:
        print(f"REFUSED: {e}")
        sys.exit(3)
    all_notes = [n for v in voices.values() for n in v]
    if not all_notes:
        print("no pitched notes")
        sys.exit(1)
    total = max(s + d for s, _, _, d in all_notes)
    M = bar_pc_matrix(all_notes, bar, total)
    peak, longrange = selfsim(M)
    lead_key = pick_lead(voices)
    lead = voices[lead_key]
    lit, trn = opening_return(lead, bar)
    vel = [vl for _, _, vl, _ in lead if vl > 0]
    vel_std = float(np.std(vel)) if len(vel) > 3 else float("nan")
    n_ioi, n_dur = rhythmic_vocab(all_notes)
    print(f"{os.path.basename(path)}: {len(voices)} pitched voices, {len(all_notes)} notes, "
          f"{M.shape[0]} bars @ {bar:g} beats; lead={lead_key}")
    print(f"  selfsim_peak      : {peak:.3f}   (acceptance target >= 0.65)")
    print(f"  longrange_recur   : {longrange:.3f}   (acceptance target >= 0.70)")
    print(f"  opening_literal   : {'YES' if lit else 'no'}   (lead opening 4-note figure returns >8 beats later)")
    print(f"  opening_transposed: {'YES' if trn else 'no'}   (same interval figure returns)")
    print(f"  lead vel_std      : {vel_std:.3f}   (reference central tendency ~0.075; flat ~0.025)")
    print(f"  distinct IOIs/durs: {n_ioi} / {n_dur}")


if __name__ == "__main__":
    main()
