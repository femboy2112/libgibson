#!/usr/bin/env python3
"""Numeric model behind src/figure/perform.rs (walk plan, reach ratios, kinematic bounds).

Pure python, no third-party modules. Run: python3 -I model_perform.py
It mirrors the walk planner of perform.rs on the synthetic test rig of tests/figure_perform.rs
(Z up, +Y forward, legs 0.46011 + 0.46011 m, hip height 1.0 m, ankle height 0.08 m) and prints:
  * the walk crouch the planner derives, the worst hip-to-ankle reach ratio over a dense sweep,
  * the largest foot and pelvis accelerations (the declared A bound of the continuity test),
  * the stance-window foot speed of the TARGET (must be exactly 0).
These are properties of the DRIVERS (before IK); the IK adds only a smooth map.
"""
import math, sys

def smoother(x):
    x = min(1.0, max(0.0, x))
    return x * x * x * (x * (x * 6 - 15) + 10)

def ramp(t, t0, d):
    return smoother((t - t0) / d) if d > 0 else (1.0 if t >= t0 else 0.0)

L1 = math.hypot(0.01, 0.46); L2 = L1; LEG = L1 + L2
HIP = [(-0.09, 0.0, 1.0), (0.09, 0.0, 1.0)]           # L, R hip in model space at rest
ANK = [(-0.09, 0.0, 0.08), (0.09, 0.0, 0.08)]         # rest ankle
SOFT = 0.98

def walk_dist(t, tau, v, n):
    total = (n + 1) * tau
    t = min(max(t, 0.0), total)
    ec = n * tau
    if t <= tau:
        u = t / tau; return v * tau * (u**3 - 0.5 * u**4)
    if t <= ec:
        return 0.5 * v * tau + v * (t - tau)
    u = (t - ec) / tau
    return 0.5 * v * tau + v * (ec - tau) + v * tau * (u - (u**3 - 0.5 * u**4))

def stand_crouch():
    best = 0.0
    for f in range(2):
        h = math.hypot(HIP[f][0] - ANK[f][0], HIP[f][1] - ANK[f][1])
        vrest = HIP[f][2] - ANK[f][2]
        need = math.sqrt(max((0.95 * LEG) ** 2 - h * h, 0.0))
        best = max(best, vrest - need)
    return best

def walk_crouch(s, cs):
    best = cs
    a = 0.6 * s
    for f in range(2):
        hf = HIP[f][1] - ANK[f][1]; hr = HIP[f][0] - ANK[f][0]
        h = math.hypot(abs(hf) + a, hr)
        vrest = HIP[f][2] - ANK[f][2]
        v2 = (0.95 * LEG) ** 2 - h * h
        need = math.sqrt(v2) if v2 > 0 else 0.3 * LEG
        best = max(best, vrest - need)
    return best

def run(D, stride, speed, amp, tempo=1.0):
    n = max(1, round(D / stride)); s = D / n
    v = min(3.0, max(0.2, speed * tempo)); tau = s / v; total = (n + 1) * tau
    cs = stand_crouch(); cw = walk_crouch(s, cs)
    extra = cw - cs
    lift = 0.04 + 0.05 * amp
    bob = 0.008 + 0.01 * amp; sway = 0.012 + 0.012 * amp
    fp = []
    last = [None, None]
    swings = []
    for k in range(n + 1):
        f = k % 2
        t0 = k * tau + 0.1 * tau; t1 = k * tau + 0.9 * tau
        if k + 2 <= n:
            dl = walk_dist(t1, tau, v, n); dn = walk_dist((k + 2 + 0.1) * tau, tau, v, n)
            F = dl + 0.5 * (dn - dl)
        else:
            F = D
        frm = last[f] if last[f] is not None else 0.0
        swings.append((f, t0, t1, frm, F)); last[f] = F
    def foot(f, t):
        pos = 0.0; z = 0.0
        for (ff, t0, t1, a, b) in swings:
            if ff != f: continue
            if t < t0: break
            if t < t1:
                u = (t - t0) / (t1 - t0)
                return a + (b - a) * smoother(u), lift * math.sin(math.pi * u) ** 2
            pos = b
        else:
            pass
        # not in swing: planted at last landing or initial
        p = 0.0
        for (ff, t0, t1, a, b) in swings:
            if ff == f and t >= t1: p = b
        return p, 0.0
    h = 1e-3
    ts = [i * h for i in range(int(total / h) + 1)]
    maxratio = 0.0; maxfa = 0.0; maxpa = 0.0
    prev = {}
    def state(t):
        env = ramp(t, 0, tau) * (1 - ramp(t, total - tau, tau))
        ph = math.pi * t / tau
        d = walk_dist(t, tau, v, n)
        pel = (sway * env * math.sin(ph) + 0.0, d, 1.0 - cs - extra * env + bob * env * math.sin(ph) ** 2)
        feet = []
        for f in range(2):
            y, z = foot(f, t)
            feet.append((ANK[f][0], y + ANK[f][1], ANK[f][2] + z))
        return pel, feet
    series = []
    for t in ts:
        pel, feet = state(t)
        series.append((pel, feet))
        for f in range(2):
            hp = (HIP[f][0] + pel[0], HIP[f][1] + pel[1], pel[2] - 1.0 + HIP[f][2] - (HIP[f][2] - 1.0) * 0 + 0.0)
            hp = (HIP[f][0] + pel[0], HIP[f][1] + pel[1], pel[2])
            dd = math.dist(hp, feet[f])
            maxratio = max(maxratio, dd / LEG)
    def acc(idx):
        m = 0.0
        for i in range(1, len(series) - 1):
            a = [(series[i + 1][idx[0]][idx[1]][c] if len(idx) > 2 else 0) for c in range(3)]
            p1 = series[i + 1]; p0 = series[i]; pm = series[i - 1]
            def g(p):
                return p[idx[0]][idx[1]] if idx[0] == 1 else p[0]
            A = g(p1); B = g(p0); C = g(pm)
            m = max(m, math.dist([A[c] - 2 * B[c] + C[c] for c in range(3)], [0, 0, 0]) / h / h)
        return m
    maxfa = max(acc((1, 0)), acc((1, 1)))
    maxpa = acc((0, 0))
    return dict(n=n, s=s, tau=tau, total=total, stand_crouch=cs, walk_crouch=cw, max_reach_ratio=maxratio,
                foot_acc=maxfa, pelvis_acc=maxpa)

if __name__ == "__main__":
    for (D, stride, speed, amp) in [(3.0, 0.5, 1.0, 0.45), (3.0, 0.5, 1.0, 1.0), (2.0, 0.4, 0.9, 1.0), (1.0, 0.5, 1.0, 0.45)]:
        r = run(D, stride, speed, amp)
        print("D=%.1f stride=%.2f speed=%.1f amp=%.2f ->" % (D, stride, speed, amp), {k: (round(v, 4) if isinstance(v, float) else v) for k, v in r.items()})
