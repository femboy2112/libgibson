#!/usr/bin/env python3
"""Round XIIIb independent sounding-tension grader (second implementation, not the gate's code).

Reads the lab's `--dump-notes` TSVs ({world}_{arm}.notes.tsv + .chords.tsv). Stricter than the
Rust audit in one way: a note's heard window is its full patch-envelope audible length (the
`audible_secs` column), never cut at its role's next attack. Same law otherwise: a minor second /
augmented unison or minor ninth heard together is a clash; the less structural note owes it; it
is justified when Transient (< 0.20 weighted s), Foreshadowing (< 0.75 and steps 1-2 semitones to
a new pitch within 0.35 s, not onto the partner a semitone away), Suspension (prepared and
stepping), or Anticipation (< 0.75, a tone of the next chord, sounding into it).

Usage: tension_audit.py DIR/swiss_mass.notes.tsv DIR/swiss_tension.notes.tsv ...
"""
import collections, csv, sys

KEY = {"swiss": (0, {0, 2, 4, 5, 7, 9, 11}, 118), "black_ice": (9, {9, 11, 0, 2, 4, 5, 7}, 88)}
NAMES = "C C# D D# E F F# G G# A A# B".split()
FLEET, ASSERT, GAP = 0.20, 0.75, 0.35


def nm(m):
    return f"{NAMES[m % 12]}{m // 12 - 1}"


def audit(path):
    world = "swiss" if "swiss" in path.split("/")[-1] else "black_ice"
    _, keypcs, tempo = KEY[world]
    spb = 60 / tempo
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    chords = [(float(c["beat"]), float(c["beat"]) + float(c["dur"]), eval(c["pcs"]), c["label"])
              for c in csv.DictReader(open(path.replace("notes", "chords")), delimiter="\t")]
    for r in rows:
        r["b0"] = float(r["beat"]); r["b1"] = r["b0"] + float(r["dur_beats"])
        r["t0"] = r["b0"] * spb; r["t1"] = r["t0"] + float(r["audible_secs"]); r["m"] = int(r["midi"])

    def chord_at(b):
        return next((c for c in chords if c[0] <= b + 1e-6 < c[1]), chords[-1])

    def sal(b):
        p = round(b, 6) % 4
        return 1.5 if p == 0 else 1.25 if p == 2 else 1.0

    def rank(n, b):
        pcs = chord_at(b)[2]; pc = n["m"] % 12
        return pcs.index(pc) if pc in pcs else 8 if pc in keypcs else 9

    out = []
    for i, a in enumerate(rows):
        for b in rows[i + 1:]:
            d = abs(a["m"] - b["m"])
            if d not in (1, 13):
                continue
            if a["role"] == b["role"] and (a["b1"] <= b["b0"] + 1e-6 or b["b1"] <= a["b0"] + 1e-6):
                continue  # one voice's legato tail
            s, e = max(a["t0"], b["t0"]), min(a["t1"], b["t1"])
            if e - s <= 1e-3:
                continue
            sb = s / spb
            exp = (e - s) * sal(sb)
            ra, rb = rank(a, sb), rank(b, sb)
            if ra != rb:
                t, c = (a, b) if ra > rb else (b, a)
            elif a["b0"] != b["b0"]:
                t, c = (a, b) if a["b0"] > b["b0"] else (b, a)
            else:
                t, c = (a, b) if a["m"] > b["m"] else (b, a)
            if exp < FLEET:
                v = "Transient"
            else:
                nxt = [r for r in rows if r["role"] == t["role"] and r["b0"] > t["b0"] + 1e-6
                       and r["t0"] <= t["t1"] + GAP]
                r = None
                if nxt:
                    first = min(x["b0"] for x in nxt)
                    r = min((x for x in nxt if abs(x["b0"] - first) < 1e-6),
                            key=lambda x: (abs(x["m"] - t["m"]), x["m"]))
                step = r is not None and 1 <= abs(r["m"] - t["m"]) <= 2
                pre = step and abs(t["m"] - c["m"]) == 1 and r["m"] == c["m"]
                prepared = t["b0"] < c["b0"] - 1e-6
                nc = next((x for x in chords if x[0] > t["b0"] + 1e-6), None)
                antic = nc is not None and t["m"] % 12 in nc[2] and nc[0] * spb <= t["t1"] + 1e-6
                if step and not pre and (exp < ASSERT or prepared):
                    v = "Suspension" if prepared else "Foreshadowing"
                elif antic and exp < ASSERT:
                    v = "Anticipation"
                else:
                    v = "UNJUSTIFIED"
            out.append((s, t, c, exp, v, d, chord_at(sb)[3]))
    return out


def chromatic(path):
    """Out-of-key non-chord notes (no clash partner needed), judged by the same clauses on their
    own heard length: Transient, Foreshadowing (steps on), Anticipation, else UNJUSTIFIED."""
    world = "swiss" if "swiss" in path.split("/")[-1] else "black_ice"
    _, keypcs, tempo = KEY[world]
    spb = 60 / tempo
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    chords = [(float(c["beat"]), float(c["beat"]) + float(c["dur"]), eval(c["pcs"]))
              for c in csv.DictReader(open(path.replace("notes", "chords")), delimiter="\t")]
    out = collections.Counter()
    for r in rows:
        b, m = float(r["beat"]), int(r["midi"])
        pcs = next((c[2] for c in chords if c[0] <= b + 1e-6 < c[1]), chords[-1][2])
        if m % 12 in pcs or m % 12 in keypcs:
            continue
        p = round(b, 6) % 4
        exp = float(r["audible_secs"]) * (1.5 if p == 0 else 1.25 if p == 2 else 1.0)
        nxt = sorted((x for x in rows if x["role"] == r["role"] and float(x["beat"]) > b + 1e-6),
                     key=lambda x: float(x["beat"]))
        step = False
        if nxt and (float(nxt[0]["beat"]) - b) * spb <= float(r["audible_secs"]) + GAP:
            first = float(nxt[0]["beat"])
            q = min((x for x in nxt if abs(float(x["beat"]) - first) < 1e-6),
                    key=lambda x: (abs(int(x["midi"]) - m), int(x["midi"])))
            step = 1 <= abs(int(q["midi"]) - m) <= 2
        nc = next((c for c in chords if c[0] > b + 1e-6), None)
        antic = nc is not None and m % 12 in nc[2] and nc[0] <= b + float(r["audible_secs"]) / spb
        out["Transient" if exp < FLEET else "Foreshadowing" if step and exp < ASSERT
            else "Anticipation" if antic and exp < ASSERT else "UNJUSTIFIED"] += 1
    return out


if __name__ == "__main__":
    for path in sys.argv[1:]:
        out = audit(path)
        print(f"== {path.split('/')[-1]}: {dict(sorted(collections.Counter(o[4] for o in out).items()))}")
        for s, t, c, exp, v, d, ch in sorted((o for o in out if o[4] == "UNJUSTIFIED"), key=lambda o: o[0]):
            print(f"   {s:6.2f}s b{t['b0']:.2f} {ch:6s} exposure={exp:.2f} {t['role']}:{t['role_note']}:{nm(t['m'])} "
                  f"vs {c['role']}:{c['role_note']}:{nm(c['m'])} {'m2' if d == 1 else 'm9'}")
        print(f"   out-of-key non-chord notes: {dict(sorted(chromatic(path).items()))}")
