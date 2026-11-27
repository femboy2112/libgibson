#!/usr/bin/env python3
"""Self-authored A-line skirt for the M5-C offline render (pure python, no bpy).  Our own geometry, repo licence.

    python3 -I tools/figure_assets/fa_skirt.py --selftest

Fixes viewed defect 4 of the MEASURE lease (stiff flared hem, notch at the waist, did not follow the pose):
  * rings are SUPER-ELLIPSES (exponent 2.6) fitted around the torso/thigh vertices of each height band, then scaled
    until every band vertex is inside, so there is no notch by construction; arms and hands are excluded by skin weight;
  * the hem flare is gentle (a few cm) and the ring parameters are smoothed over height;
  * the mesh is a coarse quad panel grid; the renderer adds Solidify + Subdivision Surface (smooth panels);
  * skinning is kinematic: every vertex is bound to pelvis / thigh_l / thigh_r with weights that ramp from the pelvis at
    the waist to the thighs at the hem, split between the legs by lateral position, then Laplacian-smoothed;
  * a deterministic BOUNDED lag term: the posed hem is pulled toward an exponentially weighted average of the same
    vertices over the previous K frames (a pure function of the tape frames f-K..f, so any frame can be rendered alone),
    scaled by ring height and clamped to `max_swing_m`.  No cloth simulation, no state.
"""
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)
import fa_common as fc  # noqa: E402

BONES = ("pelvis", "thigh_l", "thigh_r")
POWER = 2.6


def _smooth3(vals, passes=2):
    v = list(vals)
    for _ in range(passes):
        w = v[:]
        for i in range(1, len(v) - 1):
            w[i] = 0.25 * v[i - 1] + 0.5 * v[i] + 0.25 * v[i + 1]
        v = w
    return v


def _smoothstep(x):
    x = min(1.0, max(0.0, x))
    return x * x * (3.0 - 2.0 * x)


def make_skirt(P, joints, exclude=None, n_seg=48, n_ring=14, hem_below_knee_frac=0.03, margin_m=0.012, flare_m=0.05,
               waist_above_pelvis_frac=0.06, left_is_plus_x=None):
    """P: shaped positions (full index space).  joints: {"joint-pelvis": [idx...], "joint-l-knee": ..., "joint-r-knee",
    "joint-l-upper-leg"}.  exclude: optional list[bool] per body vertex (True = ignore, e.g. arms/hands by skin weight).
    -> dict(positions, quads, uvs, quad_uv, t (per vertex 0 waist..1 hem), weights {bone: [per vertex]}, params)."""
    cen = lambda n: fc.centroid(P, joints[n])
    pel = cen("joint-pelvis")
    kl, kr = cen("joint-l-knee"), cen("joint-r-knee")
    lup = cen("joint-l-upper-leg")
    if left_is_plus_x is None:
        left_is_plus_x = lup[0] > 0.0
    body = [i for i in range(fc.N_BODY) if not (exclude and exclude[i])]
    zs = [P[i][2] for i in range(fc.N_BODY)]
    stature = max(zs) - min(zs)
    hipx = abs(lup[0])
    lim = min(max(2.6 * hipx, 0.20), 0.34)
    z_top = pel[2] + waist_above_pelvis_frac * stature
    z_hem = 0.5 * (kl[2] + kr[2]) - hem_below_knee_frac * stature
    zk = [z_top + (z_hem - z_top) * k / (n_ring - 1.0) for k in range(n_ring)]
    torso = [i for i in body if abs(P[i][0]) <= lim]

    def band(z):
        w = 0.012
        for _ in range(7):
            sel = [i for i in torso if abs(P[i][2] - z) <= w]
            if len(sel) >= 8:
                return sel
            w *= 2.0
        raise ValueError("skirt: no body vertices near z=%.3f" % z)

    bands = [band(z) for z in zk]
    cy = [0.5 * (min(P[i][1] for i in b) + max(P[i][1] for i in b)) for b in bands]
    a0 = [max(abs(P[i][0]) for i in b) for b in bands]
    b0 = [max(abs(P[i][1] - cy[k]) for i in b) for k, b in enumerate(bands)]
    cy, a0, b0 = _smooth3(cy), _smooth3(a0), _smooth3(b0)
    rings = []
    for k, b in enumerate(bands):
        s = 1.0
        for i in b:
            u = (abs(P[i][0]) / a0[k]) ** POWER + (abs(P[i][1] - cy[k]) / b0[k]) ** POWER
            s = max(s, u ** (1.0 / POWER))
        t = k / (n_ring - 1.0)
        rings.append((a0[k] * s + margin_m + flare_m * t ** 1.5, b0[k] * s + margin_m + flare_m * t ** 1.5, cy[k]))
    two_pi = 2.0 * math.pi
    pos, tt = [], []
    for k, (a, b, c) in enumerate(rings):
        for j in range(n_seg):
            th = two_pi * j / n_seg
            ca, sa = math.cos(th), math.sin(th)
            r = (abs(ca / a) ** POWER + abs(sa / b) ** POWER) ** (-1.0 / POWER)
            pos.append((r * ca, c + r * sa, zk[k]))
            tt.append(k / (n_ring - 1.0))
    quads, quv = [], []
    uvs = [(j / float(n_seg), 1.0 - k / (n_ring - 1.0)) for k in range(n_ring) for j in range(n_seg + 1)]
    for k in range(n_ring - 1):
        for j in range(n_seg):
            a_, b_ = k * n_seg + j, k * n_seg + (j + 1) % n_seg
            c_, d_ = (k + 1) * n_seg + (j + 1) % n_seg, (k + 1) * n_seg + j
            quads.append((a_, d_, c_, b_))
            quv.append((k * (n_seg + 1) + j, (k + 1) * (n_seg + 1) + j, (k + 1) * (n_seg + 1) + j + 1, k * (n_seg + 1) + j + 1))
    # weights: pelvis at the waist -> thighs at the hem, split by lateral position (smooth), then Laplacian-smoothed
    xb = max(0.5 * hipx, 0.04)
    wp, wl, wr = [], [], []
    for (x, _y, _z), t in zip(pos, tt):
        thigh = 0.92 * _smoothstep(t) ** 1.2
        side_l = _smoothstep(0.5 + 0.5 * (x if left_is_plus_x else -x) / (2.0 * xb))
        wp.append(1.0 - thigh)
        wl.append(thigh * side_l)
        wr.append(thigh * (1.0 - side_l))
    W = {"pelvis": wp, "thigh_l": wl, "thigh_r": wr}

    def nbrs(v):
        k, j = divmod(v, n_seg)
        out = [k * n_seg + (j - 1) % n_seg, k * n_seg + (j + 1) % n_seg]
        if k > 0:
            out.append((k - 1) * n_seg + j)
        if k < n_ring - 1:
            out.append((k + 1) * n_seg + j)
        return out

    for _ in range(2):
        for bn in BONES:
            old = W[bn]
            W[bn] = [0.5 * old[v] + 0.5 * sum(old[m] for m in nbrs(v)) / len(nbrs(v)) for v in range(len(pos))]
    for v in range(len(pos)):
        s = sum(W[bn][v] for bn in BONES)
        for bn in BONES:
            W[bn][v] /= s
    return {"positions": pos, "quads": quads, "quad_uv": quv, "uvs": uvs, "t": tt, "weights": W,
            "params": {"n_seg": n_seg, "n_ring": n_ring, "z_top": z_top, "z_hem": z_hem, "stature": stature, "lateral_limit": lim,
                       "flare_m": flare_m, "margin_m": margin_m, "power": POWER, "left_is_plus_x": left_is_plus_x}}


def pose(rest, W, S):
    """Linear blend of the three skin matrices S = {bone: 4x4} (armature space).  -> positions."""
    out = []
    for v, p in enumerate(rest):
        x = y = z = 0.0
        for bn in BONES:
            w = W[bn][v]
            if w == 0.0:
                continue
            m = S[bn]
            x += w * (m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3])
            y += w * (m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3])
            z += w * (m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3])
        out.append((x, y, z))
    return out


def swing(history, tt, decay=0.62, gain=0.55, max_swing_m=0.05):
    """history: posed position lists, OLDEST first, NEWEST last (the current frame).  Returns the current positions pulled toward
    their exponentially weighted past (weight decay^age), by gain * t^1.5 per vertex, with the pull clamped to max_swing_m.
    A constant history returns the current positions exactly.  Pure and deterministic."""
    now = history[-1]
    if len(history) == 1:
        return [tuple(p) for p in now]
    ws = [decay ** (len(history) - 1 - k) for k in range(len(history))]
    sw = sum(ws)
    out = []
    for v, p in enumerate(now):
        avg = [sum(ws[k] * history[k][v][a] for k in range(len(history))) / sw for a in range(3)]
        g = gain * tt[v] ** 1.5
        d = [g * (avg[a] - p[a]) for a in range(3)]
        n = math.sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
        if n > max_swing_m:
            d = [c * max_swing_m / n for c in d]
        out.append((p[0] + d[0], p[1] + d[1], p[2] + d[2]))
    return out


# --------------------------------------------------------------------------- selftest

def _selftest():
    import fa_tape as ft
    fails = []

    def check(ok, msg):
        print(("PASS " if ok else "FAIL ") + msg)
        if not ok:
            fails.append(msg)

    # synthetic body: stacked elliptic rings (torso 0.14x0.09 + a wide hip bulge), full index space padded with helpers
    P = [(0.0, 0.0, 0.0)] * fc.N_ALL
    n = 0
    for zi in range(60):
        z = 0.0 + zi * 0.03
        hip = 0.05 * math.exp(-((z - 0.95) / 0.12) ** 2)
        for ai in range(36):
            a = 2 * math.pi * ai / 36
            P[n] = ((0.16 + hip) * math.cos(a), (0.10 + 0.5 * hip) * math.sin(a), z)
            n += 1
    for i in range(n, fc.N_BODY):
        P[i] = P[i % n]
    J = {"joint-pelvis": [0], "joint-l-knee": [1], "joint-r-knee": [2], "joint-l-upper-leg": [3]}
    P[0], P[1], P[2], P[3] = (0, 0, 0.95), (0.1, 0, 0.50), (-0.1, 0, 0.50), (0.1, 0, 0.9)
    sk = make_skirt(P, J, n_seg=32, n_ring=10)
    pos = sk["positions"]
    check(len(pos) == 320 and len(sk["quads"]) == 32 * 9, "320 vertices, 288 quads")
    # containment: every body vertex in the skirt height range lies inside the ring at its height
    zt, zh = sk["params"]["z_top"], sk["params"]["z_hem"]
    rz = [pos[k * 32][2] for k in range(10)]
    bad = 0
    for i in range(1, n):
        z = P[i][2]
        if zh <= z <= zt:
            k = min(range(10), key=lambda kk: abs(rz[kk] - z))
            xs = [pos[k * 32 + j][0] for j in range(32)]
            if abs(P[i][0]) > max(xs) + 1e-9:
                bad += 1
    check(bad == 0, "no body vertex pokes out of the skirt (outside count %d)" % bad)
    # no notch: ring radius varies smoothly with angle (second difference bounded)
    ring = [math.hypot(pos[j][0], pos[j][1] - sk["positions"][0][1]) for j in range(32)]
    check(max(abs(ring[(j - 1) % 32] - 2 * ring[j] + ring[(j + 1) % 32]) for j in range(32)) < 0.02, "ring has no notch (bounded second difference)")
    ws = sum(abs(sum(sk["weights"][b][v] for b in BONES) - 1.0) for v in range(320))
    check(ws < 1e-9, "weights sum to 1 at every vertex")
    top = [v for v in range(32)]
    hem = [v for v in range(320 - 32, 320)]
    check(all(sk["weights"]["pelvis"][v] > 0.9 for v in top) and all(sk["weights"]["pelvis"][v] < 0.25 for v in hem),
          "waist rides the pelvis, hem rides the thighs")
    # +x vertices favour thigh_l (left is +x here), -x favour thigh_r
    vl = max(hem, key=lambda v: pos[v][0])
    vr = min(hem, key=lambda v: pos[v][0])
    check(sk["weights"]["thigh_l"][vl] > sk["weights"]["thigh_r"][vl] and sk["weights"]["thigh_r"][vr] > sk["weights"]["thigh_l"][vr], "hem weights split left/right by lateral position")
    I = ft.ident()
    S0 = {b: I for b in BONES}
    check(max(math.dist(a, b) for a, b in zip(pose(pos, sk["weights"], S0), pos)) < 1e-12, "identity skin matrices leave the skirt in place")
    # raising the left thigh (rotation about the hip) moves left hem vertices more than right
    hip_l = (0.1, 0.0, 0.9)
    rot = ft.mul(ft.mul(ft.translation(hip_l), ft.from_trs((0, 0, 0), ft.quat_axis_angle((1, 0, 0), -0.6), (1, 1, 1))), ft.translation((-hip_l[0], -hip_l[1], -hip_l[2])))
    Sl = {"pelvis": I, "thigh_l": rot, "thigh_r": I}
    moved = pose(pos, sk["weights"], Sl)
    dl = math.dist(moved[vl], pos[vl])
    dr = math.dist(moved[vr], pos[vr])
    check(dl > 0.08 and dr < dl * 0.5, "left thigh lift moves the left hem %.3f m, the right hem %.3f m" % (dl, dr))
    # smoothness of the posed hem: neighbouring hem vertices stay close (no tearing)
    gap = max(math.dist(moved[hem[j]], moved[hem[(j + 1) % 32]]) for j in range(32))
    check(gap < 0.12, "posed hem stays connected (max neighbour gap %.3f m)" % gap)
    # swing: constant history = exact, bounded, zero at the waist
    const = swing([pos, pos, pos], sk["t"])
    check(max(math.dist(a, b) for a, b in zip(const, pos)) < 1e-12, "constant history gives no swing")
    hist = [pos, pos, moved]
    sw = swing(hist, sk["t"])
    pull = max(math.dist(a, b) for a, b in zip(sw, moved))
    check(0.0 < pull <= 0.05 + 1e-12, "swing is bounded by max_swing_m (max pull %.4f m)" % pull)
    check(max(math.dist(sw[v], moved[v]) for v in top) < 1e-12, "no swing at the waist ring")
    check(swing(hist, sk["t"]) == sw, "swing is deterministic")
    print("SELFTEST_FAILS %d" % len(fails))
    return 1 if fails else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(_selftest())
    print(__doc__)
