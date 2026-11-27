#!/usr/bin/env python3
"""f64 model of the DQS test fixtures (PREDICTION instrument, not a bearing on the Rust).

Run with `python3 -I model_dqs.py`. It restates the fixtures of `tests/figure_dqs.rs` (same
LCG, same joint tables, same poses) and prints the quantities the test premises depend on:
the largest linear-vs-dual-quaternion difference, the number of sign flips in the blend, the
smallest |dot| to the pivot (how close any vertex is to a sign decision), and the
cross-section ratios of the elbow cylinder. Pure python, standard library only. It shares the
design with the Rust (it is the same maths written a second time), so agreement with the Rust
is a check of the code against the design, not an independent proof of the design.
"""
import math

# ---- quaternions as (w, x, y, z) -------------------------------------------------------------
def qmul(a, b):
    return (a[0]*b[0]-a[1]*b[1]-a[2]*b[2]-a[3]*b[3],
            a[0]*b[1]+a[1]*b[0]+a[2]*b[3]-a[3]*b[2],
            a[0]*b[2]-a[1]*b[3]+a[2]*b[0]+a[3]*b[1],
            a[0]*b[3]+a[1]*b[2]-a[2]*b[1]+a[3]*b[0])
def qconj(a): return (a[0], -a[1], -a[2], -a[3])
def qadd(a, b): return tuple(x+y for x, y in zip(a, b))
def qscale(a, s): return tuple(x*s for x in a)
def qdot(a, b): return sum(x*y for x, y in zip(a, b))
def axis_angle(axis, deg):
    n = math.sqrt(sum(c*c for c in axis)); h = math.radians(deg)/2
    s = math.sin(h)/n
    return (math.cos(h), axis[0]*s, axis[1]*s, axis[2]*s)

# ---- dual quaternions (r, d) -----------------------------------------------------------------
def dq_from(q, t):
    return (q, qscale(qmul((0.0, t[0], t[1], t[2]), q), 0.5))
def dq_mul(a, b):
    return (qmul(a[0], b[0]), qadd(qmul(a[0], b[1]), qmul(a[1], b[0])))
def dq_inv(a): return (qconj(a[0]), qconj(a[1]))
def dq_blend(terms, align=True):
    pivot = terms[0][1][0]
    r = (0.0,)*4; d = (0.0,)*4; flips = 0; mind = 9.0
    for w, dq in terms:
        dd = qdot(dq[0], pivot)
        mind = min(mind, abs(dd))
        sg = -1.0 if (dd < 0 and align) else 1.0
        flips += sg < 0
        r = qadd(r, qscale(dq[0], w*sg)); d = qadd(d, qscale(dq[1], w*sg))
    a = math.sqrt(qdot(r, r)); b = qdot(r, d)/a
    ia = 1.0/a; ib = -b/(a*a)
    return (qscale(r, ia), qadd(qscale(d, ia), qscale(r, ib))), flips, mind
def dq_apply(q, p):
    P = ((1.0, 0.0, 0.0, 0.0), (0.0, p[0], p[1], p[2]))
    qc = (qconj(q[0]), qscale(qconj(q[1]), -1.0))
    res = dq_mul(dq_mul(q, P), qc)
    return res[1][1:]

# ---- matrices (row-major 4x4) for the linear control -----------------------------------------
def mat_from(q, t):
    w, x, y, z = q
    R = [[1-2*(y*y+z*z), 2*(x*y-w*z), 2*(x*z+w*y)],
         [2*(x*y+w*z), 1-2*(x*x+z*z), 2*(y*z-w*x)],
         [2*(x*z-w*y), 2*(y*z+w*x), 1-2*(x*x+y*y)]]
    return [R[0]+[t[0]], R[1]+[t[1]], R[2]+[t[2]], [0, 0, 0, 1]]
def mmul(a, b): return [[sum(a[i][k]*b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]
def mrigid_inv(m):
    R = [[m[j][i] for j in range(3)] for i in range(3)]
    t = [-sum(R[i][k]*m[k][3] for k in range(3)) for i in range(3)]
    return [R[0]+[t[0]], R[1]+[t[1]], R[2]+[t[2]], [0, 0, 0, 1]]
def mpoint(m, p): return tuple(sum(m[i][k]*p[k] for k in range(3))+m[i][3] for i in range(3))

# ---- the deterministic generator (identical to the Rust LCG) ---------------------------------
class Lcg:
    def __init__(self, s): self.s = s
    def next(self):
        self.s = (self.s*1664525 + 1013904223) & 0xFFFFFFFF
        return self.s >> 8
    def unit(self): return self.next()/16777216.0

def make_mesh(seed, nverts=32, njoints=4):
    g = Lcg(seed); pos = []; skin = []
    for i in range(nverts):
        x = g.unit()*3.0-1.5; y = g.unit()*4.0-0.5; z = g.unit()*3.0-1.5
        pos.append((x, y, z))
        order = list(range(njoints))
        for k in range(njoints-1, 0, -1):
            j = g.next() % (k+1); order[k], order[j] = order[j], order[k]
        cnt = 1 + (i % 4)
        raw = [g.unit()+0.1 for _ in range(cnt)]
        s = sum(raw)
        skin.append([(order[k], raw[k]/s) for k in range(cnt)])
    return pos, skin

A_JOINTS = [  # parent, t, (axis, deg)
    (-1, (0.0, 0.0, 0.0), ((0, 1, 0), 10.0)),
    (0, (0.1, 1.1, -0.05), ((1, 2, 3), 20.0)),
    (1, (-0.05, 0.9, 0.1), ((0, 1, 1), -30.0)),
    (0, (0.3, 0.2, 0.1), ((1, 0, 0), 45.0))]
B_JOINTS = [
    (-1, (0.0, 0.0, 0.0), ((0, 0, 1), 15.0)),
    (0, (0.05, 1.0, 0.02), ((1, 0, 0), -25.0)),
    (1, (-0.04, 0.95, 0.06), ((0, 1, 0), 35.0)),
    (2, (0.03, 0.85, -0.07), ((1, 1, 0), -40.0))]
A_POSES = [
    [((0, 1, 0), 25.0, (0, 0, 0)), ((1, 2, 3), 40.0, (0, 0, 0)), ((0, 1, 1), -55.0, (0, 0, 0)), ((1, 0, 0), 70.0, (0, 0, 0))],
    [((0, 1, 0), -35.0, (0, 0, 0)), ((1, 2, 3), 110.0, (0.05, -0.03, 0.02)), ((0, 1, 1), -130.0, (0, 0, 0)), ((1, 0, 0), 100.0, (0, 0, 0))],
    [((0, 1, 0), 60.0, (0, 0, 0)), ((1, 2, 3), -95.0, (0, 0, 0)), ((0, 1, 1), 170.0, (-0.04, 0.02, 0.0)), ((1, 0, 0), -105.0, (0, 0, 0))]]
B_POSES = [
    [((0, 0, 1), 20.0, (0, 0, 0)), ((1, 0, 0), 50.0, (0, 0, 0)), ((0, 1, 0), 60.0, (0, 0, 0)), ((1, 1, 0), -70.0, (0, 0, 0))],
    [((0, 0, 1), -40.0, (0, 0, 0)), ((1, 0, 0), 120.0, (0.02, 0.04, 0.0)), ((0, 1, 0), -100.0, (0, 0, 0)), ((1, 1, 0), 130.0, (0, 0, 0))],
    [((0, 0, 1), 75.0, (0, 0, 0)), ((1, 0, 0), -110.0, (0, 0, 0)), ((0, 1, 0), 140.0, (0, 0, 0.03)), ((1, 1, 0), -150.0, (0, 0, 0))]]

def world(joints, locals_):
    W = []
    for i, (par, _, _) in enumerate(joints):
        q, t = locals_[i]
        W.append(dq_from(q, t) if par < 0 else dq_mul(W[par], dq_from(q, t)))
    return W

def run_fixture(name, joints, poses, seed):
    pos, skin = make_mesh(seed)
    rest = [(axis_angle(*j[2]), j[1]) for j in joints]
    Wb = world(joints, rest)
    four = sum(1 for s in skin if len(s) == 4)
    for pi, pose in enumerate(poses):
        loc = [(axis_angle(a, d), tuple(r+o for r, o in zip(joints[i][1], off)))
               for i, (a, d, off) in enumerate(pose)]
        Wp = world(joints, loc)
        skin_dq = [dq_mul(Wp[j], dq_inv(Wb[j])) for j in range(len(joints))]
        # linear control through matrices
        def to_m(dq):
            r = dq[0]; c = qmul(dq[1], qconj(r)); t = (2*c[1], 2*c[2], 2*c[3])
            return mat_from(r, t)
        skin_m = [to_m(d) for d in skin_dq]
        maxd = 0.0; flips = 0; mind = 9.0; max_noalign = 0.0
        for p, infl in zip(pos, skin):
            terms = [(w, skin_dq[j]) for j, w in infl]
            blended, f, md = dq_blend(terms)
            flips += f; mind = min(mind, md)
            a = dq_apply(blended, p)
            bn, _, _ = dq_blend(terms, align=False)
            an = dq_apply(bn, p)
            max_noalign = max(max_noalign, max(abs(a[k]-an[k]) for k in range(3)))
            lbs = [0.0, 0.0, 0.0]
            for j, w in infl:
                q = mpoint(skin_m[j], p)
                lbs = [lbs[k]+w*q[k] for k in range(3)]
            maxd = max(maxd, max(abs(a[k]-lbs[k]) for k in range(3)))
        print(f"{name} pose{pi+1}: four_influence_vertices={four} max|LBS-DQS|={maxd:.4f} "
              f"sign_flips={flips} min|dot_to_pivot|={mind:.4f} max|DQS-DQS_no_align|={max_noalign:.4f}")

# ---- elbow cylinder ---------------------------------------------------------------------------
def vec_area(ring):
    ax = ay = az = 0.0
    n = len(ring)
    for i in range(n):
        p = ring[i]; q = ring[(i+1) % n]
        ax += p[1]*q[2]-p[2]*q[1]; ay += p[2]*q[0]-p[0]*q[2]; az += p[0]*q[1]-p[1]*q[0]
    return 0.5*math.sqrt(ax*ax+ay*ay+az*az)

def elbow(deg, ring_ys=(0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0), n=16, r=0.2):
    Rq = axis_angle((0, 0, 1), deg)
    pivot = (0.0, 1.0, 0.0)
    out = {}
    for ri, y in enumerate(ring_ys):
        t = min(max((y-0.5)/1.0, 0.0), 1.0)
        ring_rest = [(r*math.cos(2*math.pi*k/n), y, r*math.sin(2*math.pi*k/n)) for k in range(n)]
        # skin matrix of joint 1: rotation about the pivot
        M1 = mmul(mmul(mat_from((1, 0, 0, 0), pivot), mat_from(Rq, (0, 0, 0))), mat_from((1, 0, 0, 0), (0, -1.0, 0)))
        dq1 = dq_from(Rq, (0.0, 0.0, 0.0))
        dq1 = dq_mul(dq_mul(dq_from((1, 0, 0, 0), pivot), dq1), dq_from((1, 0, 0, 0), (0, -1.0, 0)))
        dq0 = dq_from((1, 0, 0, 0), (0, 0, 0))
        lin, dqs = [], []
        for p in ring_rest:
            a = mpoint(M1, p)
            lin.append(tuple((1-t)*p[k]+t*a[k] for k in range(3)))
            blended, _, _ = dq_blend([(1-t, dq0), (t, dq1)])
            dqs.append(dq_apply(blended, p))
        a0 = vec_area(ring_rest)
        out[ri] = (vec_area(lin)/a0, vec_area(dqs)/a0)
    return out

if __name__ == "__main__":
    run_fixture("two_bone", A_JOINTS, A_POSES, 12345)
    run_fixture("three_bone", B_JOINTS, B_POSES, 54321)
    for deg in (20.0, 120.0):
        e = elbow(deg)
        blend_rings = (3, 4, 5)
        print(f"elbow {deg:5.1f} deg: min ratio over rings 3..5  linear={min(e[i][0] for i in blend_rings):.4f}"
              f"  dualquat={min(e[i][1] for i in blend_rings):.4f}   mid ring linear={e[4][0]:.4f} dualquat={e[4][1]:.4f}")
