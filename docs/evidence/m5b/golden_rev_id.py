# REGRESSION PIN (not independent).
#
# This script is a numpy float32 REPLICATION of the Slice-1 implementation, for the RICH
# synthetic fixture (tests/figure_mechanics.rs: `rich_family`, `rich_recipe`). It mirrors the
# implementation's float32 EVALUATION ORDER ON PURPOSE (quaternion normalize then to_mat, then
# a per-column scale, the 4-term matrix product summed k = 0..3, the adjugate affine inverse,
# morph weights accumulated in control order). It is NOT an independent bearing on the maths:
# a formula error in the implementation would be replicated here. Independent bearings on the
# maths are the f64 oracle (tests/figure_oracle/mod.rs) and the hand-derived tests. What this
# script adds is (a) a cross-language check of float32 arithmetic order and (b) a pin of the
# documented canonical byte layout and value content (FIGURE_CONTRACT.md, "Canonical
# serialization gibson.figure.rev.v0 (NORMATIVE)"). A drift in the pinned hash means "the
# bytes changed"; it does not say which side is right.
#
# Every float is a numpy float32 scalar: each operation rounds to f32, no FMA, no f64
# accumulation. Fixture literals are converted from their DECIMAL text with exact
# round-to-nearest-even (Fraction), matching how rustc rounds a float literal, not through a
# double. Run: python3 -I docs/evidence/m5b/golden_rev_id.py
import hashlib
import struct
from fractions import Fraction

import numpy as np

f = np.float32


def F(text):
    """Decimal text -> float32, correctly rounded (ties to even), like a Rust literal."""
    x = Fraction(text)
    c = np.float32(float(x))
    best = None
    for cand in (np.nextafter(c, np.float32(-np.inf)), c, np.nextafter(c, np.float32(np.inf))):
        d = abs(Fraction(float(cand)) - x)
        key = (d, int(np.float32(cand).view(np.uint32)) & 1)
        if best is None or key < best[0]:
            best = (key, cand)
    return np.float32(best[1])


def pf(x):
    """f32 bit pattern, little-endian; -0.0 is canonicalized to +0.0 (ruling #59)."""
    x = np.float32(x)
    if x == f(0):
        x = f(0)
    return struct.pack('<I', int(x.view(np.uint32)))


def pu(x):
    return struct.pack('<I', x)


def pstr(s):
    b = s.encode()
    return pu(len(b)) + b


# ---------------------------------------------------------------------------------------
# Fixture: must stay value-for-value identical to `rich_*` in tests/figure_mechanics.rs.
# ---------------------------------------------------------------------------------------
FAMILY_ID, FAMILY_VERSION = "synthetic.tube.rich", 1

RING_Y = [F("0.0"), F("1.1"), F("2.2"), F("3.3")]
RING_HX = [F("0.30"), F("0.27"), F("0.24"), F("0.21")]
RING_HZ = [F("0.20"), F("0.18"), F("0.16"), F("0.14")]
pos = []
for r in range(4):
    for k in range(4):
        sx = -RING_HX[r] if k in (0, 3) else RING_HX[r]
        sz = -RING_HZ[r] if k < 2 else RING_HZ[r]
        pos.append([sx, RING_Y[r], sz])

idx = []
for r in range(3):
    for k in range(4):
        a = r * 4 + k
        b = r * 4 + (k + 1) % 4
        c = a + 4
        d = b + 4
        idx += [a, b, c, b, d, c]

SKIN_TEXT = [
    [(0, "1.0")],
    [(0, "1.0")],
    [(0, "0.7"), (1, "0.3")],
    [(0, "1.0")],
    [(0, "0.6"), (1, "0.4")],
    [(1, "0.3"), (2, "0.70005")],  # f64 sum 1.00005 (inside 1e-4, not renormalized)
    [(0, "0.45"), (1, "0.55")],
    [(1, "0.3"), (3, "0.7")],
    [(3, "0.2"), (0, "0.15"), (2, "0.35"), (1, "0.29995")],  # 4 influences, sum 0.99995
    [(2, "0.65"), (1, "0.35")],
    [(3, "0.4"), (1, "0.6")],
    [(1, "0.25"), (2, "0.5"), (3, "0.25")],
    [(2, "1.0")],
    [(2, "0.8"), (3, "0.2")],
    [(2, "1.0")],
    [(2, "0.55"), (3, "0.45")],
]
skin = [[(j, F(w)) for j, w in s] for s in SKIN_TEXT]

# girth: vertices 4..12, dx = +-0.12 (sign as the ring), dz = +-0.08
girth = []
for v in range(4, 12):
    k = v % 4
    dx = F("-0.12") if k in (0, 3) else F("0.12")
    dz = F("-0.08") if k < 2 else F("0.08")
    girth.append((v, [dx, F("0.0"), dz]))
# taper: declared order is NOT sorted by vertex; vertex 14 is a ZERO delta (kept)
taper = [
    (13, [F("0.05"), F("-0.04"), F("0.0")]),
    (12, [F("-0.03"), F("0.02"), F("0.01")]),
    (14, [F("0.0"), F("0.0"), F("0.0")]),
    (15, [F("-0.06"), F("0.0"), F("0.03")]),
]
morphs = [("girth", girth), ("taper", taper)]

# (name, parent, t, q xyzw, s)
JOINTS = [
    ("root", -1, ["0.1", "-0.05", "0.2"], ["0.0", "0.1", "0.0", "0.99498744"], ["1.0", "1.0", "1.0"]),
    ("upper", 0, ["0.05", "1.05", "-0.03"], ["0.06", "0.0", "-0.08", "0.99498744"], ["1.0", "1.0", "1.0"]),
    ("lower", 1, ["-0.02", "0.95", "0.04"], ["-0.1", "0.05", "0.0", "0.99373035"], ["1.2", "0.8", "1.1"]),
    ("arm", 1, ["0.6", "0.3", "0.05"], ["0.2", "0.4", "0.4", "0.8"], ["1.0", "1.1", "0.9"]),
]
joints = [
    (n, p, [F(c) for c in t], [F(c) for c in q], [F(c) for c in s])
    for n, p, t, q, s in JOINTS
]
landmarks = [
    ("head_top", 2, [F("0.05"), F("1.1"), F("-0.07")]),
    ("upper_mid", 1, [F("0.0"), F("0.55"), F("0.0")]),
    ("hand", 3, [F("0.12"), F("0.31"), F("-0.04")]),
]
# controls in family order: (id, effect, target)
CONTROLS = [
    ("upper_length", "bone", 1),
    ("torso_length", "bone", 0),
    ("girth", "morph", 0),
    ("girth_fine", "morph", 0),
    ("taper", "morph", 1),
]
RECIPE = {
    "upper_length": F("1.3"),
    "torso_length": F("0.85"),
    "girth": F("0.35"),
    "girth_fine": F("0.15"),
    "taper": F("-0.4"),
}

# ---------------------------------------------------------------------------------------
# Shaping (compile steps 9-11), in family-declared control order.
# ---------------------------------------------------------------------------------------
sj = [[n, p, list(t), list(q), list(s)] for n, p, t, q, s in joints]
w_morph = [f(0), f(0)]
for cid, kind, target in CONTROLS:
    v = RECIPE[cid]
    if kind == "bone":
        for j in sj:
            if j[1] == target:
                j[2] = [j[2][0] * v, j[2][1] * v, j[2][2] * v]
    else:
        w_morph[target] = w_morph[target] + v
mpos = [list(p) for p in pos]
for (name, deltas), w in zip(morphs, w_morph):
    for v, d in deltas:
        mpos[v] = [mpos[v][c] + w * d[c] for c in range(3)]


# ---------------------------------------------------------------------------------------
# float32 matrices, COLUMN-major cols[c][r] (mirrors the implementation's layout on purpose)
# ---------------------------------------------------------------------------------------
def quat_to_mat(x, y, z, w):
    two = f(2)
    xx = x * x
    yy = y * y
    zz = z * z
    xy = x * y
    xz = x * z
    yz = y * z
    wx = w * x
    wy = w * y
    wz = w * z
    return [
        [f(1) - two * (yy + zz), two * (xy + wz), two * (xz - wy), f(0)],
        [two * (xy - wz), f(1) - two * (xx + zz), two * (yz + wx), f(0)],
        [two * (xz + wy), two * (yz - wx), f(1) - two * (xx + yy), f(0)],
        [f(0), f(0), f(0), f(1)],
    ]


def from_trs(t, q, s):
    l2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]
    inv = f(1) / np.sqrt(l2)
    r = quat_to_mat(q[0] * inv, q[1] * inv, q[2] * inv, q[3] * inv)
    cols = [[f(0)] * 4 for _ in range(4)]
    for c in range(3):
        for rr in range(3):
            cols[c][rr] = r[c][rr] * s[c]
    cols[3] = [t[0], t[1], t[2], f(1)]
    return cols


def mul(a, b):
    out = [[f(0)] * 4 for _ in range(4)]
    for c in range(4):
        for r in range(4):
            out[c][r] = a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3]
    return out


def inv_affine(m):
    a00, a01, a02 = m[0][0], m[1][0], m[2][0]
    a10, a11, a12 = m[0][1], m[1][1], m[2][1]
    a20, a21, a22 = m[0][2], m[1][2], m[2][2]
    c00 = a11 * a22 - a12 * a21
    c01 = a12 * a20 - a10 * a22
    c02 = a10 * a21 - a11 * a20
    c10 = a02 * a21 - a01 * a22
    c11 = a00 * a22 - a02 * a20
    c12 = a01 * a20 - a00 * a21
    c20 = a01 * a12 - a02 * a11
    c21 = a02 * a10 - a00 * a12
    c22 = a00 * a11 - a01 * a10
    det = a00 * c00 + a01 * c01 + a02 * c02
    i00 = c00 / det
    i01 = c10 / det
    i02 = c20 / det
    i10 = c01 / det
    i11 = c11 / det
    i12 = c21 / det
    i20 = c02 / det
    i21 = c12 / det
    i22 = c22 / det
    tx, ty, tz = m[3][0], m[3][1], m[3][2]
    # Ordinary negation: yields -0.0 for a zero sum; the serializer canonicalizes it.
    t0 = -(i00 * tx + i01 * ty + i02 * tz)
    t1 = -(i10 * tx + i11 * ty + i12 * tz)
    t2 = -(i20 * tx + i21 * ty + i22 * tz)
    return [[i00, i10, i20, f(0)], [i01, i11, i21, f(0)], [i02, i12, i22, f(0)], [t0, t1, t2, f(1)]]


world = []
for n, p, t, q, s in sj:
    l = from_trs(t, q, s)
    world.append(l if p < 0 else mul(world[p], l))
ibs = [inv_affine(w_) for w_ in world]

# ---------------------------------------------------------------------------------------
# Canonical bytes: FIGURE_CONTRACT.md "Layout, in order" 1..9.
# ---------------------------------------------------------------------------------------
o = b"gibson.figure.rev.v0\0"
o += pstr(FAMILY_ID) + pu(FAMILY_VERSION)
o += pu(len(mpos))
for p in mpos:
    o += b''.join(pf(c) for c in p)
o += pu(len(idx)) + b''.join(pu(i) for i in idx)
o += pu(len(sj))
for n, p, t, q, s in sj:
    o += pstr(n) + struct.pack('<i', p)
    o += b''.join(pf(c) for c in t) + b''.join(pf(c) for c in q) + b''.join(pf(c) for c in s)
o += pu(len(landmarks))
for n, j, off in landmarks:
    o += pstr(n) + pu(j) + b''.join(pf(c) for c in off)
o += pu(len(ibs))
for m in ibs:
    for c in range(4):
        for r in range(4):
            o += pf(m[c][r])
o += pu(len(skin))
for s in skin:
    o += bytes([len(s)])
    for j, wt in s:
        o += pu(j) + pf(wt)
o += pu(len(morphs))
for name, deltas in morphs:
    o += pstr(name) + pu(len(deltas))
    for v, d in deltas:
        o += pu(v) + b''.join(pf(c) for c in d)

if __name__ == "__main__":
    print(hashlib.sha256(o).hexdigest())
