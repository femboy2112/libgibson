"""Numeric MODEL of the C-1 motion design (docs/FIGURE_CONTRACT.md, "Continuous clips and
transitions (C-1)"). Run with:  python3 -I docs/evidence/m5b/c1/model_c1.py

WHAT THIS IS. A float64 re-statement of the algorithm in `src/figure/motion.rs` written
BEFORE that code could be compiled. It is a model and a PREDICTION, not a measurement of the
Rust implementation: it shares the design with the implementation (so it is not an independent
check of the Rust), and the Rust is f32 where this is f64. Its jobs:
  1. check the closed-form identities the design relies on (quintic profile, SO(3) Jacobian,
     the velocity-preservation algebra) against finite differences;
  2. check that the bounds the tests declare (derived from key data, see tests/figure_motion.rs)
     really are upper bounds on the model's behaviour (a bound that the model violates is a
     derivation error and must be fixed in the derivation, never tuned);
  3. predict, per test and per mutant, the size of the quantity the test asserts on.
It prints a table. The fixtures here are copied from tests/figure_motion.rs.
"""
import math
import numpy as np

DT = 1e-3
U = 2.0 ** -24  # f32 unit roundoff, used for the declared roundoff term

# ---------------------------------------------------------------- quaternion helpers (xyzw)
def qmul(a, b):
    ax, ay, az, aw = a; bx, by, bz, bw = b
    return np.array([aw*bx + ax*bw + ay*bz - az*by,
                     aw*by - ax*bz + ay*bw + az*bx,
                     aw*bz + ax*by - ay*bx + az*bw,
                     aw*bw - ax*bx - ay*by - az*bz])
def qconj(q): return np.array([-q[0], -q[1], -q[2], q[3]])
def qrot(q, v):
    u = q[:3]; t = 2*np.cross(u, v)
    return v + q[3]*t + np.cross(u, t)
def qlog(q):
    if q[3] < 0: q = -q
    s = np.linalg.norm(q[:3])
    if s < 1e-12: return 2*q[:3]
    return q[:3] * (2*math.atan2(s, q[3]) / s)
def qexp(r):
    th = np.linalg.norm(r)
    if th < 1e-12:
        q = np.array([*(0.5*r), 1.0]); return q/np.linalg.norm(q)
    return np.array([*(r*math.sin(th/2)/th), math.cos(th/2)])
def skew2(r, v): return np.cross(r, np.cross(r, v))
def J(r, v):
    th = np.linalg.norm(r)
    if th < 1e-3: a = 0.5 - th**2/24; b = 1/6 - th**2/120
    else: a = (1-math.cos(th))/th**2; b = (th-math.sin(th))/th**3
    return v + a*np.cross(r, v) + b*skew2(r, v)
def Jinv(r, v):
    th = np.linalg.norm(r)
    if th < 1e-3: c = 1/12 + th**2/720
    else:
        h = th/2; c = (1 - h*math.cos(h)/math.sin(h))/th**2
    return v - 0.5*np.cross(r, v) + c*skew2(r, v)
def omega_to_qdot(q, w): return 0.5*qmul(np.array([*w, 0.0]), q)
def qdot_to_omega(q, qd): return 2*qmul(qd, qconj(q))[:3]
def angle_z(q): return 2*math.atan2(q[2], q[3])

# ---------------------------------------------------------------- quintic profile
def w5(x): return x**3*(x*(x*6-15)+10)
def dw5(x): return 30*x*x*(1-x)**2
def h0(x): return 1 - w5(x)
def dh0(x): return -dw5(x)
def h1(x): return x - 6*x**3 + 8*x**4 - 3*x**5
def dh1(x): return 1 - 18*x**2 + 32*x**3 - 15*x**4
def ddh0(x): return -60*x*(1-x)*(1-2*x)
def ddh1(x): return -12*x*(1-x)*(3-5*x)

# ---------------------------------------------------------------- key sampling (mirror of motion.rs)
def align(a, ref): return -a if np.dot(a, ref) < 0 else a
def eval_keys(ts, vs, t, loop_p, smooth, quat):
    n = len(ts)
    if loop_p is not None:
        ne = n
        if ne > 1 and ts[0] == 0 and abs(ts[-1] - loop_p) < 1e-12: ne -= 1
        if ne == 1: return vs[0].copy(), np.zeros(4), True
        tt = math.fmod(t, loop_p)
        nxt = sum(1 for k in ts[:ne] if k <= tt + 1e-15)
        i = nxt - 1
        def get(j): return (ts[j % ne] + (j // ne)*loop_p, vs[j % ne])
    else:
        tt = t
        nxt = sum(1 for k in ts if k <= tt + 1e-15)
        if nxt == 0: return vs[0].copy(), np.zeros(4), True
        if nxt == n: return vs[-1].copy(), np.zeros(4), True
        i = nxt - 1
        def get(j): return (ts[j], vs[j]) if 0 <= j < n else None
    t1, p1 = get(i); t2, p2 = get(i+1)
    h = t2 - t1; s = (tt - t1)/h
    a1 = p1; a2 = align(p2, a1) if quat else p2
    g0 = get(i-1); g3 = get(i+2)
    a0 = (align(g0[1], a1) if quat else g0[1]) if g0 else None
    a3 = (align(g3[1], a2) if quat else g3[1]) if g3 else None
    if not smooth:
        return (1-s)*a1 + s*a2, (a2 - a1)/h, s == 0
    m1 = (a2 - a0)/(t2 - g0[0]) if g0 else np.zeros(4)
    m2 = (a3 - a1)/(g3[0] - t1) if g3 else np.zeros(4)
    s2 = s*s; s3 = s2*s
    h00 = 2*s3 - 3*s2 + 1; h10 = s3 - 2*s2 + s; h01 = -2*s3 + 3*s2; h11 = s3 - s2
    d00 = 6*s2 - 6*s; d10 = 3*s2 - 4*s + 1; d01 = -6*s2 + 6*s; d11 = 3*s2 - 2*s
    v = h00*a1 + h10*h*m1 + h01*a2 + h11*h*m2
    d = (d00*a1 + d10*h*m1 + d01*a2 + d11*h*m2)/h
    return v, d, s == 0

def finish_quat(v, d, exact):
    rho = np.linalg.norm(v); qh = v/rho
    dh = (d - qh*np.dot(qh, d))/rho
    return (v if exact else qh), qdot_to_omega(qh, dh)

# ---------------------------------------------------------------- fixtures (mirror of the tests)
def zq(deg):
    a = math.radians(deg); return np.array([0, 0, math.sin(a/2), math.cos(a/2)])
def v4(x, y, z): return np.array([x, y, z, 0.0])
def s4(x): return np.array([x, 0, 0, 0.0])
A = dict(dur=2.0,
  arm=([0, .5, 1, 1.5, 2.0], [v4(0,0,0), v4(1,2,0), v4(.2,-1,.5), v4(1.5,.5,-1), v4(0,0,0)]),
  spine=([0, .5, 1, 1.5, 2.0], [zq(d) for d in (0, 40, -30, 50, 0)]),
  smile=([0, .5, 1, 1.5, 2.0], [s4(x) for x in (0, 1, .2, .8, 0)]))
B = dict(dur=1.0,
  arm=([.1, .4, .7], [v4(0,0,0), v4(2,0,0), v4(1,1,0)]),
  spine=([.1, .4, .7], [zq(d) for d in (0, 30, -20)]),
  smile=([.1, .4, .7], [s4(x) for x in (0, 1, .3)]))
N = dict(dur=1.0,
  arm=([0, .6, 1.0], [v4(3,3,0), v4(-2,1,1), v4(0,0,0)]),
  spine=([0, .6, 1.0], [zq(d) for d in (10, -40, 0)]),
  smile=([0, .6, 1.0], [s4(x) for x in (.5, 0, .5)]))
KIND = dict(arm='vec', spine='quat', smile='scalar')

def sample_clip(fx, t, loop=False, smooth=True, mut=None):
    """-> {name: (value, rate)}; value: vec3 | quat | float; rate: vec3 | omega vec3 | float"""
    out = {}
    sm = smooth and mut != 'linear_keys'
    for nm in ('arm', 'spine', 'smile'):
        ts, vs = fx[nm]
        v, d, ex = eval_keys(ts, vs, t, fx['dur'] if loop else None, sm, KIND[nm] == 'quat')
        if KIND[nm] == 'vec': out[nm] = (v[:3].copy(), d[:3].copy())
        elif KIND[nm] == 'scalar': out[nm] = (float(v[0]), float(d[0]))
        else:
            q, w = finish_quat(v, d, ex); out[nm] = (q, w)
    return out

# ---------------------------------------------------------------- transitions
def blend_rot(qa, wa, qb, wb, w, dw):
    s = -1.0 if np.dot(qa, qb) < 0 else 1.0
    qb2 = s*qb
    qad = omega_to_qdot(qa, wa); qbd = s*omega_to_qdot(qb, wb)
    n = (1-w)*qa + w*qb2; nd = (1-w)*qad + w*qbd + dw*(qb2 - qa)
    rho = np.linalg.norm(n); qh = n/rho; dh = (nd - qh*np.dot(qh, nd))/rho
    return qh, qdot_to_omega(qh, dh)

def blend_state(a, b, w, dw):
    out = {}
    for k in a:
        (va, ra), (vb, rb) = a[k], b[k]
        if KIND[k] == 'quat': out[k] = blend_rot(va, ra, vb, rb, w, dw)
        else: out[k] = ((1-w)*va + w*vb, (1-w)*ra + w*rb + dw*(vb - va))
    return out

def crossfade(fa, fb, start, ease, t, mut=None):
    """fa, fb: callables time->state"""
    if mut == 'snap_transition': ease = 0
    if t < start: return fa(t)
    if ease == 0 or t - start >= ease: return fb(t)
    x = (t - start)/ease
    return blend_state(fa(t), fb(t), w5(x), dw5(x)/ease)

def preempt_rot(old, new0, newt, x, T, mut=None):
    (qo, wo), (qn0, wn0), (qnt, wnt) = old, new0, newt
    if mut == 'freeze_on_preempt':
        return blend_rot(qo, np.zeros(3), qnt, wnt, w5(x), dw5(x)/T)
    qoff0 = qmul(qo, qconj(qn0))
    if qoff0[3] < 0: qoff0 = -qoff0
    r0 = qlog(qoff0)
    woff0 = wo - qrot(qoff0, wn0)
    w0 = Jinv(r0, woff0)
    r = r0*h0(x) + w0*T*h1(x)
    rd = r0*dh0(x)/T + w0*dh1(x)
    E = qexp(r)
    q = qmul(E, qnt); q = q/np.linalg.norm(q)
    return q, J(r, rd) + qrot(E, wnt)

def preempt(fo, fn, at, ease, t, mut=None):
    if t < at: return fo(t)
    if mut == 'snap_transition': ease = 0
    if ease == 0 or t - at >= ease: return fn(t)
    x = (t - at)/ease; T = ease
    o0 = fo(at); n0 = fn(at); nt = fn(t)
    if mut == 'freeze_on_preempt':
        held = {k: (v, np.zeros_like(r) if not np.isscalar(r) else 0.0) for k, (v, r) in o0.items()}
        return blend_state(held, nt, w5(x), dw5(x)/T)
    out = {}
    for k in o0:
        if KIND[k] == 'quat': out[k] = preempt_rot(o0[k], n0[k], nt[k], x, T, mut); continue
        x0 = o0[k][0] - n0[k][0]; v0 = o0[k][1] - n0[k][1]
        off = x0*h0(x) + v0*T*h1(x); offd = x0*dh0(x)/T + v0*dh1(x)
        out[k] = (nt[k][0] + off, nt[k][1] + offd)
    return out

# ---------------------------------------------------------------- measurement helpers
def comps(k, st):
    v = st[k][0]
    if KIND[k] == 'vec': return list(v)
    if KIND[k] == 'quat': return [angle_z(v)]
    return [v]

def max_dv(f, k, t0, t1):
    """max over the 1 ms grid of |second difference|/dt (the FD velocity jump), l-inf over comps"""
    n = int(round((t1 - t0)/DT)) + 1
    P = np.array([comps(k, f(t0 + i*DT)) for i in range(n)])
    V = np.diff(P, axis=0)/DT
    dV = np.abs(np.diff(V, axis=0))
    i = int(np.argmax(dV.max(axis=1)))
    return float(dV.max()), t0 + (i+1)*DT

def round_term(P): return 4*U*P/DT

# ---------------------------------------------------------------- declared bounds (from key data)
def chord_S_hmin(ts, comp_vals, period=None):
    ts = list(ts); S = 0.0; hmin = 1e9
    pairs = list(zip(range(len(ts)-1), range(1, len(ts))))
    for a, b in pairs:
        h = ts[b] - ts[a]; hmin = min(hmin, h)
        S = max(S, max(abs(comp_vals[b][c] - comp_vals[a][c]) for c in range(len(comp_vals[a])))/h)
    if period is not None:
        h = ts[0] + period - ts[-1]
        if h > 1e-12:
            hmin = min(hmin, h)
            S = max(S, max(abs(comp_vals[0][c] - comp_vals[-1][c]) for c in range(len(comp_vals[0])))/h)
    return S, hmin
def VA(S, hmin): return 3.5*S, 14*S/hmin
RHO_MIN = 0.75
def rot_bound(ts, qs, period=None):
    S, hmin = chord_S_hmin(ts, [q for q in qs], period)
    V, A = VA(S, hmin)
    Vn, An = math.sqrt(2)*V, math.sqrt(2)*A
    # angle theta = 2 atan2(z, w): |theta'| <= 2|n'|/rho, |theta''| <= 2|n''|/rho + 4|n'|^2/rho^2
    return 2*An/RHO_MIN + 4*Vn**2/RHO_MIN**2, 2*Vn/RHO_MIN

def bound_clip(fx, nm, loop):
    ts, vs = fx[nm]
    per = fx['dur'] if loop else None
    if KIND[nm] == 'quat':
        A_, V_ = rot_bound(ts, vs, per); return A_, V_
    S, hmin = chord_S_hmin(ts, vs, per)
    V, A_ = VA(S, hmin); return A_, V

def P_of(fx, nm):
    ts, vs = fx[nm]
    if KIND[nm] == 'quat': return math.pi
    return max(float(np.max(np.abs(v))) for v in vs)

# ---------------------------------------------------------------- run
def report(rows):
    w = max(len(r[0]) for r in rows)
    print(f"{'check':<{w}}  {'normal':>10} {'bound':>10}   {'snap':>10} {'freeze':>10} {'linear':>10}")
    for r in rows:
        print(f"{r[0]:<{w}}  " + " ".join(f"{x:>10.4g}" if x is not None else f"{'-':>10}" for x in r[1:3])
              + "   " + " ".join(f"{x:>10.4g}" if x is not None else f"{'-':>10}" for x in r[3:6]))

def identities():
    print("== identities")
    # quintic boundary conditions
    for f, name in ((h0, 'h0'), (h1, 'h1')):
        pass
    chk = [(h0(0), 1), (dh0(0), 0), (ddh0(0), 0), (h0(1), 0), (dh0(1), 0), (ddh0(1), 0),
           (h1(0), 0), (dh1(0), 1), (ddh1(0), 0), (h1(1), 0), (dh1(1), 0), (ddh1(1), 0)]
    print("quintic end conditions max err", max(abs(a-b) for a, b in chk))
    xs = np.linspace(0, 1, 100001)
    print("max|h0''| =", max(abs(ddh0(x)) for x in xs[::10]), "(declared 5.78)  max|h1''| =",
          max(abs(ddh1(x)) for x in xs[::10]), "(declared 4)  max|h0'| =", max(abs(dh0(x)) for x in xs[::10]),
          "(declared 1.875)  max|h1'| =", max(abs(dh1(x)) for x in xs[::10]), "(declared 1)")
    # numeric derivative sanity of h0/h1
    e = 0
    for x in (0.1, 0.37, 0.8):
        e = max(e, abs((h0(x+1e-6)-h0(x-1e-6))/2e-6 - dh0(x)), abs((h1(x+1e-6)-h1(x-1e-6))/2e-6 - dh1(x)),
                abs((dh0(x+1e-6)-dh0(x-1e-6))/2e-6 - ddh0(x)), abs((dh1(x+1e-6)-dh1(x-1e-6))/2e-6 - ddh1(x)))
    print("quintic derivative formulas vs FD max err", e)
    # J relation: omega = J(r) r'  (q = exp(r) has q' = 1/2 omega q)
    rng = np.random.default_rng(12345)
    worst = 0; worst_inv = 0; worst_pi = 0
    for _ in range(200):
        r = rng.normal(size=3); r *= rng.uniform(0.01, 3.1)/np.linalg.norm(r); rd = rng.normal(size=3)
        e = 1e-6
        qd = (qexp(r + e*rd) - qexp(r - e*rd))/(2*e)
        w = qdot_to_omega(qexp(r), qd)
        worst = max(worst, np.linalg.norm(w - J(r, rd)))
        worst_inv = max(worst_inv, np.linalg.norm(Jinv(r, J(r, rd)) - rd))
    r = np.array([0, 0, math.pi - 1e-9]); v = np.array([.3, -.4, .5])
    worst_pi = np.linalg.norm(Jinv(r, J(r, v)) - v)
    print("omega - J(r) r' max err:", worst, " Jinv(J(v)) - v max err:", worst_inv, " at theta=pi:", worst_pi)
    # the Look bound uses |J(r) v| <= |v| and |dJ/dt| <= |r'|/2 (J = int_0^1 exp(s [r]x) ds)
    worst_J = 0; worst_dJ = 0
    for _ in range(2000):
        r = rng.normal(size=3); r *= rng.uniform(0.0, 4*math.pi)/np.linalg.norm(r)
        v = rng.normal(size=3); v /= np.linalg.norm(v); rd = rng.normal(size=3); rd /= np.linalg.norm(rd)
        worst_J = max(worst_J, np.linalg.norm(J(r, v)))
        e = 1e-6
        dJv = (J(r + e*rd, v) - J(r - e*rd, v))/(2*e)
        worst_dJ = max(worst_dJ, np.linalg.norm(dJv)/0.5)
    print("sup |J(r) v| / |v| =", worst_J, "(declared <= 1);  sup |dJ/dt v| / (|r'|/2 |v|) =", worst_dJ, "(declared <= 1)")

def velocity_preservation_3d():
    print("== velocity preservation, general 3-D axes (preempt_rot, no mutant)")
    rng = np.random.default_rng(7)
    worst = 0
    for _ in range(200):
        def randrot():
            ax = rng.normal(size=3); ax /= np.linalg.norm(ax)
            return qexp(ax*rng.uniform(0.0, math.pi)), rng.normal(size=3)*3
        old, new = randrot(), randrot()
        T = 0.3
        r = preempt_rot(old, new, new, 0.0, T)
        worst = max(worst, np.linalg.norm(r[1] - old[1]))
        # value at x=0 equals old rotation (up to sign)
        assert abs(abs(np.dot(r[0], old[0])) - 1) < 1e-9
    print("max |omega_out(t0) - omega_old(t0)| over 200 random cases =", worst)

def main():
    identities(); velocity_preservation_3d()
    rows = []
    # --- T1/T2: smooth clips, C1 across keys, seam
    for fx, nm_fx, loop, rng_ms in ((A, 'A', False, (0, 2500)), (A, 'A', True, (0, 6000)), (B, 'B', True, (0, 3000))):
        for nm in ('arm', 'spine', 'smile'):
            A_, V_ = bound_clip(fx, nm, loop)
            bnd = A_*DT + round_term(P_of(fx, nm))
            f = lambda t, fx=fx, loop=loop: sample_clip(fx, t, loop)
            fl = lambda t, fx=fx, loop=loop: sample_clip(fx, t, loop, mut='linear_keys')
            m0, _ = max_dv(f, nm, rng_ms[0]/1000, rng_ms[1]/1000)
            ml, _ = max_dv(fl, nm, rng_ms[0]/1000, rng_ms[1]/1000)
            rows.append((f"clip {nm_fx}{'/loop' if loop else '/once'} {nm}", m0, bnd, None, None, ml))
    # --- T3: crossfade A (once, start 0) -> N (start 600ms), ease 500 ms
    ease = 0.5; start = 0.6
    fa = lambda t: sample_clip(A, t); fb = lambda t: sample_clip(N, max(t - start, 0.0))
    for nm in ('arm', 'spine', 'smile'):
        Aa, Va = bound_clip(A, nm, False); Ab, Vb = bound_clip(N, nm, False)
        D = 2*P_of(A, nm) if KIND[nm] != 'quat' else 2.0
        if KIND[nm] == 'quat': Dq = 2*math.pi
        bnd = (max(Aa, Ab) + 2*1.875/ease*(Va+Vb) + 5.78/ease**2*(2*P_of(A, nm) if KIND[nm] != 'quat' else 2*math.pi))*DT + round_term(P_of(A, nm))
        f = lambda t, mut=None: crossfade(fa, fb, start, ease, t, mut)
        m0, _ = max_dv(f, nm, 0.0, 1.8)
        ms, _ = max_dv(lambda t: crossfade(fa, fb, start, ease, t, 'snap_transition'), nm, 0.0, 1.8)
        rows.append((f"crossfade {nm}", m0, bnd, ms, None, None))
    # --- T4: preempt A(once) -> N(start at t0), t0 = 750 ms, ease 400 ms
    at = 0.75; T = 0.4
    fo = lambda t: sample_clip(A, t); fn = lambda t: sample_clip(N, max(t - at, 0.0))
    for nm in ('arm', 'spine', 'smile'):
        Ao, Vo = bound_clip(A, nm, False); An, Vn = bound_clip(N, nm, False)
        o0 = fo(at); n0 = fn(at)
        if KIND[nm] == 'quat':
            x0 = abs(angle_z(qmul(o0[nm][0], qconj(n0[nm][0])))); v0 = Vo + Vn
        else:
            x0 = float(np.max(np.abs(np.atleast_1d(o0[nm][0] - n0[nm][0])))); v0 = Vo + Vn
        Apre = max(Ao, An + x0*5.78/T**2 + v0*4/T)
        bnd = Apre*DT + round_term(P_of(A, nm))
        m0, _ = max_dv(lambda t: preempt(fo, fn, at, T, t), nm, 0.0, 2.0)
        ms, _ = max_dv(lambda t: preempt(fo, fn, at, T, t, 'snap_transition'), nm, 0.0, 2.0)
        mf, _ = max_dv(lambda t: preempt(fo, fn, at, T, t, 'freeze_on_preempt'), nm, 0.0, 2.0)
        rows.append((f"preempt {nm}", m0, bnd, ms, mf, None))
    report(rows)

    print("== preempt at t0: |FD vel before - after| (arm, y comp) and premise |v| ; reported-rate check")
    k = 'arm'
    for mut in (None, 'snap_transition', 'freeze_on_preempt'):
        f = lambda t: preempt(fo, fn, at, T, t, mut)[k][0]
        before = (f(at) - f(at - DT))/DT; after = (f(at + DT) - f(at))/DT
        rate0 = preempt(fo, fn, at, T, at, mut)[k][1]
        print(f"  mut={mut}: v_before={before}, v_after={after}, jump={np.max(np.abs(after-before)):.4g}, "
              f"reported rate(t0)={rate0}, old rate(t0)={fo(at)[k][1]}")
    print("== preempt offset closed form at tau=1/2 (arm): measured - predicted (normal / freeze / snap)")
    for mut in (None, 'freeze_on_preempt', 'snap_transition'):
        tm = at + T/2
        got = preempt(fo, fn, at, T, tm, mut)['arm'][0] - fn(tm)['arm'][0]
        x0 = fo(at)['arm'][0] - fn(at)['arm'][0]
        e = 1e-5
        vo = (fo(at+e)['arm'][0] - fo(at-e)['arm'][0])/(2*e); vn = (fn(at+e)['arm'][0] - fn(at-e)['arm'][0])/(2*e)
        pred = x0*0.5 + (vo - vn)*T*0.15625
        print(f"  mut={mut}: |got - pred|_inf = {np.max(np.abs(got - pred)):.3g}")

    # --- T5 look: nested retargets
    print("== look: forward +Z, initial +Z, retarget 500 ms -> +X, 700 ms -> +Y, ease 400 ms")
    ease = 0.4
    def dirs(d): d = np.array(d, float); return d/np.linalg.norm(d)
    def rbt(a, b):
        d = float(np.dot(a, b))
        if d < -0.999999:
            ax = np.array([1., 0, 0]) if abs(a[0]) < 0.9 else np.array([0., 1, 0])
            p = np.cross(a, ax); p /= np.linalg.norm(p); return np.array([*p, 0.0])
        c = np.cross(a, b); q = np.array([*c, 1 + d]); return q/np.linalg.norm(q)
    fwd = dirs((0, 0, 1)); sched = [(0.5, dirs((1, 0, 0))), (0.7, dirs((0, 1, 0)))]
    tg = [rbt(fwd, dirs((0, 0, 1)))] + [rbt(fwd, d) for _, d in sched]
    def aim(count, t, mut):
        if count == 0: return tg[0], np.zeros(3)
        at_, _ = sched[count-1]
        e_ = 0.0 if mut == 'snap_transition' else ease
        if e_ == 0 or t - at_ >= e_: return tg[count], np.zeros(3)
        x = (t - at_)/e_
        old = aim(count-1, at_, mut)
        n = (tg[count], np.zeros(3))
        return preempt_rot(old, n, n, x, e_, mut)
    def look(t, mut=None):
        cnt = sum(1 for a, _ in sched if a <= t)
        return aim(cnt, t, mut)
    def dvec(t, mut=None): return qrot(look(t, mut)[0], fwd)
    for mut in (None, 'snap_transition', 'freeze_on_preempt'):
        n = int(2.0/DT) + 1
        D = np.array([dvec(i*DT, mut) for i in range(n)])
        V = np.diff(D, axis=0)/DT; dV = np.linalg.norm(np.diff(V, axis=0), axis=1)
        jump = np.linalg.norm(np.diff(D, axis=0), axis=1)
        print(f"  mut={mut}: max|d jump per ms|={jump.max():.4g}  max FD-velocity jump={dV.max():.4g} at t={(np.argmax(dV)+1)*DT:.3f}")
        # converged
        fin = dvec(1.5, mut); print(f"    final direction {np.round(fin, 6)}")
    # declared look bound (see tests)
    R1a = math.pi*1.875/ease; Om1 = R1a
    R1b = R1a + (math.pi/2)*Om1; R2b = math.pi*5.78/ease**2 + (math.pi/2)*Om1*4/ease
    Ad = R2b + 1.5*R1b**2
    print(f"  declared look bound: R1={R1b:.4g} R2={R2b:.4g}  A_d={Ad:.4g}  bound*dt={Ad*DT:.4g}")

    # --- key facts for the tests
    ts, vs = A['spine']
    mn = 9
    for i in range(len(ts)-1):
        for s in np.linspace(0, 1, 101):
            t = ts[i] + s*(ts[i+1]-ts[i]); v, d, ex = eval_keys(ts, vs, t, None, True, True)
            mn = min(mn, np.linalg.norm(v))
    print("== min |n| over fixture A spine segments =", mn, "(declared rho_min =", RHO_MIN, ")")
    for nm_fx, fx, loop in (('A', A, True), ('B', B, True), ('N', N, False)):
        ts, vs = fx['spine']; mn = 9
        for t in np.linspace(0.0, 2.0 if nm_fx != 'N' else 1.0, 4001):
            v, d, ex = eval_keys(ts, vs, t, fx['dur'] if loop else None, True, True); mn = min(mn, np.linalg.norm(v))
        print(f"   {nm_fx} spine min|n| = {mn:.4f}")

if __name__ == '__main__':
    main()

def premises():
    """Facts the tests assert as PREMISES (so that a mutant can be detected at all)."""
    print("== premises used by tests/figure_motion.rs")
    fo = lambda t: sample_clip(A, t)
    o = fo(0.75)
    print("   old rates at 750 ms: arm", np.round(o['arm'][1], 3), " spine omega_z", round(o['spine'][1][2], 3),
          " smile rate", round(o['smile'][1], 3), "  (tests need |omega_z| > 0.5, |smile rate| > 0.5)")
    ax = lambda v: np.array(v)/np.linalg.norm(v)
    def tilt(axv, degs, t):
        ts = [0, .5, 1.0]
        vs = [np.array([*(ax(axv)*math.sin(math.radians(d)/2)), math.cos(math.radians(d)/2)]) for d in degs]
        v, d, ex = eval_keys(ts, vs, t, None, True, True); return finish_quat(v, d, ex)
    qo, wo = tilt((1, 2, 2), (0, 100, 60), 0.25)
    qn, wn = tilt((-2, 1, 2), (-80, 120, 10), 0.0)
    print("   tilted case: |dot(q_old,q_new)| =", abs(float(np.dot(qo, qn))), "(test needs < 0.9);  |omega_old| =",
          float(np.linalg.norm(wo)), "(test needs > 0.5)")
    # closed form with the new clip already running (started at 600 ms), as in the test
    fo = lambda t: sample_clip(A, t); fn = lambda t: sample_clip(N, max(t - 0.6, 0.0))
    for mut in (None, 'freeze_on_preempt', 'snap_transition'):
        tm = 0.75 + 0.2
        got = preempt(fo, fn, 0.75, 0.4, tm, mut)['arm'][0] - fn(tm)['arm'][0]
        x0 = fo(.75)['arm'][0] - fn(.75)['arm'][0]
        e = 1e-3
        vo = (fo(.75+e)['arm'][0] - fo(.75-e)['arm'][0])/(2*e); vn = (fn(.75+e)['arm'][0] - fn(.75-e)['arm'][0])/(2*e)
        pred = x0*0.5 + (vo - vn)*0.4*(5/32)
        print(f"   closed form (new started at 600 ms, 1 ms central differences), mut={mut}: |got-pred|_inf = {np.max(np.abs(got-pred)):.3g} (TOL_CF 2e-4)")

if __name__ == '__main__':
    premises()
