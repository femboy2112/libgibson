#!/usr/bin/env python3
"""Pure-python mirror of the perform.rs solver and the walk / wave / sit / gaze plans, on the
synthetic rig of tests/figure_perform.rs. Run: python3 -I model_perform_solver.py

It exists because perform.rs was written without a compiler: the geometry (basis, quaternion and
yaw/pitch signs, IK, soft reach, planned footfalls) is checked here first, with the same formulas,
so the first heavy run is not also the first time the numbers meet reality. It does NOT model the
Preempt inertialization (that is motion.rs, already measured) and it is not the oracle of the Rust
code: it is a second implementation of the same design by the same author (common-mode risk).
"""
import math, sys

# ---- vectors / quaternions (x y z w, Hamilton, same formulas as figure::math) ----
def add(a, b): return [a[0]+b[0], a[1]+b[1], a[2]+b[2]]
def sub(a, b): return [a[0]-b[0], a[1]-b[1], a[2]-b[2]]
def sc(a, s): return [a[0]*s, a[1]*s, a[2]*s]
def dot(a, b): return a[0]*b[0]+a[1]*b[1]+a[2]*b[2]
def cross(a, b): return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
def ln(a): return math.sqrt(dot(a, a))
def unit(a):
    l = ln(a); return None if l < 1e-12 else sc(a, 1/l)
QI = [0, 0, 0, 1]
def qmul(a, b):
    return [a[3]*b[0]+a[0]*b[3]+a[1]*b[2]-a[2]*b[1],
            a[3]*b[1]-a[0]*b[2]+a[1]*b[3]+a[2]*b[0],
            a[3]*b[2]+a[0]*b[1]-a[1]*b[0]+a[2]*b[3],
            a[3]*b[3]-a[0]*b[0]-a[1]*b[1]-a[2]*b[2]]
def qconj(a): return [-a[0], -a[1], -a[2], a[3]]
def qnorm(a):
    l = math.sqrt(sum(x*x for x in a)); return [x/l for x in a]
def qrot(q, v):
    u = q[:3]; t = sc(cross(u, v), 2.0)
    return add(add(v, sc(t, q[3])), cross(u, t))
def qaa(axis, ang):
    l = ln(axis); s = math.sin(ang/2)/l
    return [axis[0]*s, axis[1]*s, axis[2]*s, math.cos(ang/2)]
def qdot(a, b): return sum(x*y for x, y in zip(a, b))

SOFT = 0.98
def soft_reach(d, l1, l2, ss):
    L = l1+l2; lo = abs(l1-l2)+0.05*L
    if d < lo: return lo
    d0 = ss*L
    if d <= d0: return d
    sp = L-d0
    return d0+sp*math.tanh((d-d0)/sp)
def limit_reach(root, tgt, l1, l2):
    to = sub(tgt, root); u = unit(to)
    return tgt if u is None else add(root, sc(u, soft_reach(ln(to), l1, l2, SOFT)))
def perp_any(u):
    ax = [1, 0, 0] if abs(u[0]) < 0.9 else [0, 1, 0]
    return unit(cross(u, ax))
def arc(a, b):
    a = unit(a); b = unit(b)
    if a is None or b is None: return QI
    d = dot(a, b)
    if d < -0.999999: return qaa(perp_any(a), math.pi)
    c = cross(a, b)
    return qnorm([c[0], c[1], c[2], 1+d])
def two_bone(root, tgt, pole, l1, l2):
    to = sub(tgt, root); u = unit(to)
    d = soft_reach(ln(to), l1, l2, SOFT)
    end = add(root, sc(u, d))
    a = (l1*l1-l2*l2+d*d)/(2*max(d, 1e-9)); h2 = l1*l1-a*a; h = math.sqrt(h2) if h2 > 0 else 0
    pp = sub(pole, sc(u, dot(pole, u))); pd = unit(pp) if ln(pp) > 1e-6*(1+ln(pole)) else perp_any(u)
    return add(add(root, sc(u, a)), sc(pd, h)), end
def solve_limb(parent_w, up_rest, lo_rest, u2l, l2e, root, tgt, pole):
    l1 = ln(u2l); l2 = ln(l2e)
    mid, end = two_bone(root, tgt, pole, l1, l2)
    inh_u = qmul(parent_w, up_rest)
    uw = qnorm(qmul(arc(qrot(inh_u, u2l), sub(mid, root)), inh_u))
    inh_l = qmul(uw, lo_rest)
    lw = qnorm(qmul(arc(qrot(inh_l, l2e), sub(end, mid)), inh_l))
    return uw, lw, qnorm(qmul(qconj(parent_w), uw)), mid, end

# ---- synthetic rig (tests/figure_perform.rs) ----
J = [("root", None, [0, 0, 0]), ("pelvis", 0, [0, 0, 1.0]), ("spine1", 1, [0, 0, .12]), ("spine2", 2, [0, 0, .14]),
     ("spine3", 3, [0, 0, .16]), ("neck", 4, [0, 0, .10]), ("head", 5, [0, 0, .08]), ("eye_l", 6, [-.03, .09, .07]),
     ("eye_r", 6, [.03, .09, .07]), ("upperarm_l", 4, [-.19, 0, .05]), ("forearm_l", 9, [0, -.05, -.27]),
     ("hand_l", 10, [0, .10, -.22]), ("upperarm_r", 4, [.19, 0, .05]), ("forearm_r", 12, [0, -.05, -.27]),
     ("hand_r", 13, [0, .10, -.22]), ("thigh_l", 1, [-.09, 0, 0]), ("calf_l", 15, [0, .01, -.46]),
     ("foot_l", 16, [0, -.01, -.46]), ("thigh_r", 1, [.09, 0, 0]), ("calf_r", 18, [0, .01, -.46]),
     ("foot_r", 19, [0, -.01, -.46])]
N = len(J)
UP = [0, 0, 1.0]; FWD = [0, 1.0, 0]; RIGHT = cross(FWD, UP)
rest_pos = []; rest_rot = []
for (_, p, t) in J:
    if p is None: rest_pos.append(t); rest_rot.append(QI)
    else: rest_pos.append(add(rest_pos[p], qrot(rest_rot[p], t))); rest_rot.append(rest_rot[p])
PEL, HEAD, NECK = 1, 6, 5; SPINE = [2, 3, 4]; EYES = [7, 8]
LIMB = [[9, 10, 11], [12, 13, 14], [15, 16, 17], [18, 19, 20]]
HEAD_C_UP = .09; CLEAR = .20
# perform.rs WAVE_FORWARD and the wave point resting at the hang (initial_drivers); the pre-fix
# values (0.05, False) reproduce the two C-1 failures, see model_perform_cut.py.
WAVE_FORWARD = 0.12; WAVE_REST_AT_HANG = True
# perform.rs CLEARANCE_SOFT: the C1 soft floor of push_out; HARD_PUSH_OUT = mutant hard_push_out.
CLEARANCE_SOFT = 0.03; HARD_PUSH_OUT = False
WAVE_LATERAL = 0.12  # perform.rs WAVE_LATERAL (lateral margin of the wave beyond the clearance)
def push_out(p, c, r):
    v = sub(p, c); l = ln(v)
    if HARD_PUSH_OUT:
        if l >= r: return p
        return add(c, sc(v, r/l)) if l > 1e-6 else add(c, [0, 0, r])
    s = min(CLEARANCE_SOFT, r)
    if l >= r+s: return p
    lp = r+s*math.exp((l-r-s)/s)
    return add(c, sc(v, lp/l)) if l > 1e-6 else add(c, [0, 0, lp])
ROLE = {PEL: "pel", NECK: "neck", HEAD: "head"}
for s in SPINE: ROLE[s] = "spine"
for e in EYES: ROLE[e] = "eye"
for l, c in enumerate(LIMB): ROLE[c[0]] = ("up", l); ROLE[c[1]] = ("lo", l); ROLE[c[2]] = ("end", l)
ARM_POLE = []
for side in range(2):
    c = LIMB[side]; h = rest_pos[c[0]]; m = rest_pos[c[1]]; e = rest_pos[c[2]]
    ax = unit(sub(e, h)); mv = sub(m, h); perp = sub(mv, sc(ax, dot(mv, ax)))
    ARM_POLE.append(unit(perp))
D_ROOT, D_HEAD, D_PEL, D_PELROT, D_SPINE, D_GAZE, D_NOD, D_FOOT, D_ARM, NB = 0, 3, 4, 7, 10, 13, 15, 16, 28, 44
LEG = 2*math.hypot(.01, .46)
def leg_len(): return LEG
def stand_crouch():
    best = 0
    for f in range(2):
        c = LIMB[2+f]; d = sub(rest_pos[c[0]], rest_pos[c[2]]); vert = dot(d, UP)
        horiz = ln(sub(d, sc(UP, vert))); need = math.sqrt(max((0.95*LEG)**2-horiz**2, 0))
        best = max(best, vert-need)
    return best
def walk_crouch(step):
    best = stand_crouch()
    for f in range(2):
        c = LIMB[2+f]; d = sub(rest_pos[c[0]], rest_pos[c[2]]); vert = dot(d, UP)
        hf = abs(dot(d, FWD))+0.6*step; hr = dot(d, RIGHT); v2 = (0.95*LEG)**2-hf*hf-hr*hr
        need = math.sqrt(v2) if v2 > 0 else 0.3*LEG
        best = max(best, vert-need)
    return best
def init_drivers():
    d = [0.0]*NB; d[D_PEL+2] = -stand_crouch()
    for f in range(2):
        a = rest_pos[LIMB[2+f][2]]
        for c in range(3): d[D_FOOT+f*6+c] = a[c]
        d[D_FOOT+f*6+5] = 1.0
    if WAVE_REST_AT_HANG:
        for side in range(2):
            hp = rest_pos[LIMB[side][2]]; b = D_ARM+side*8
            d[b+4] = dot(hp, FWD)+d[D_PEL]; d[b+5] = dot(hp, RIGHT)+d[D_PEL+1]; d[b+6] = dot(hp, UP)+d[D_PEL+2]
    return d

def solve_pass(d, foot_lock=True, hc_in=None):
    yq = qaa(UP, d[D_HEAD]); hf = qrot(yq, FWD); hr = qrot(yq, RIGHT)
    root_t = d[0:3]
    def tilt(p, r, y): return qmul(qmul(qaa(UP, y), qaa(hr, -p)), qaa(hf, r))
    pel_off = add(add(sc(hf, d[D_PEL]), sc(hr, d[D_PEL+1])), sc(UP, d[D_PEL+2]))
    pel_pos = add(qrot(yq, rest_pos[PEL]), pel_off)
    pel_rot = qmul(qmul(tilt(d[D_PELROT], d[D_PELROT+1], d[D_PELROT+2]), yq), rest_rot[PEL])
    wy = [.35, .50, .15]; wp = [.30, .55, .15]
    gy, gp, nod = d[D_GAZE], d[D_GAZE+1], d[D_NOD]
    wpos = [None]*N; wrot = [None]*N; loc = [None]*N; lw = {}
    for j in range(N):
        _, par, tr = J[j]
        ppos, prot = (wpos[par], wrot[par]) if par is not None else ([0, 0, 0], QI)
        t = list(tr); q = QI
        if par is None: t = qrot(yq, t); q = yq
        pc = qconj(prot); inh = qmul(prot, QI)
        r = ROLE.get(j)
        if r == "pel":
            t = qrot(pc, sub(pel_pos, ppos)); q = qmul(pc, pel_rot)
        elif r == "spine":
            f = 1/3; q = qmul(pc, qmul(tilt(d[D_SPINE]*f, d[D_SPINE+1]*f, d[D_SPINE+2]*f), inh))
        elif r == "neck": q = qmul(pc, qmul(tilt(gp*wp[0], 0, gy*wy[0]), inh))
        elif r == "head": q = qmul(pc, qmul(tilt(gp*wp[1]+nod, 0, gy*wy[1]), inh))
        elif r == "eye": q = qmul(pc, qmul(tilt(gp*wp[2], 0, gy*wy[2]), inh))
        elif isinstance(r, tuple) and r[0] == "up":
            l = r[1]; c = LIMB[l]; lo = J[c[1]][2]; en = J[c[2]][2]
            root_pos = add(ppos, qrot(prot, t))
            fk_end = add(add(root_pos, qrot(inh, lo)), qrot(inh, en))
            if l >= 2:
                f = l-2; ikw = d[D_FOOT+f*6+5] if foot_lock else 0.0
                want = sub(d[D_FOOT+f*6:D_FOOT+f*6+3], root_t)
                tgt = add(fk_end, sc(sub(want, fk_end), ikw)); pole = hf
            else:
                s = l; b = D_ARM+s*8
                hang = add(qrot(yq, rest_pos[c[2]]), pel_off)
                swing = add(add(sc(hf, d[b]), sc(hr, d[b+1])), sc(UP, d[b+2]))
                w = d[b+3]
                wave = add(add(sc(hf, d[b+4]), sc(hr, d[b+5])), sc(UP, d[b+6]))
                hs = add(hang, swing); tgt = add(hs, sc(sub(wave, hs), w))
                if hc_in is not None:
                    l1 = ln(lo); l2 = ln(en)
                    t1 = limit_reach(root_pos, tgt, l1, l2)
                    t2 = push_out(t1, hc_in, CLEAR)
                    tgt = limit_reach(root_pos, t2, l1, l2)
                out = sc(hr, -1) if s == 0 else hr
                pole = add(qrot(yq, ARM_POLE[s]), sc(out, d[b+7]))
            uw, lwr, ulocal, mid, end = solve_limb(prot, QI, QI, lo, en, root_pos, tgt, pole)
            lw[l] = lwr; q = ulocal
        elif isinstance(r, tuple) and r[0] == "lo": q = qmul(pc, lw[r[1]])
        elif isinstance(r, tuple) and r[0] == "end" and r[1] >= 2:
            f = r[1]-2; ikw = d[D_FOOT+f*6+5] if foot_lock else 0.0
            yf = qaa(UP, d[D_FOOT+f*6+3]); rf = qrot(yf, RIGHT)
            want = qmul(qmul(qaa(rf, -d[D_FOOT+f*6+4]), yf), rest_rot[j])
            b2 = want if qdot(inh, want) >= 0 else [-x for x in want]
            mix = qnorm([x*(1-ikw)+y*ikw for x, y in zip(inh, b2)])
            q = qmul(pc, mix)
        q = qnorm(q); loc[j] = (t, q)
        wpos[j] = add(ppos, qrot(prot, t)); wrot[j] = qnorm(qmul(prot, q))
    up_h = qrot(wrot[HEAD], qrot(qconj(rest_rot[HEAD]), UP))
    hc = add(wpos[HEAD], sc(up_h, HEAD_C_UP))
    return wpos, wrot, hc
def solve(d, foot_lock=True, clearance=True):
    wpos, wrot, hc = solve_pass(d, foot_lock, None)
    if clearance: wpos, wrot, hc = solve_pass(d, foot_lock, hc)
    return [add(p, d[0:3]) for p in wpos], wrot, add(hc, d[0:3])

# ---- plans ----
def smoother(x):
    x = min(1.0, max(0.0, x)); return x*x*x*(x*(x*6-15)+10)
def ramp(t, t0, dd): return smoother((t-t0)/dd) if dd > 0 else (1.0 if t >= t0 else 0.0)
def walk_dist(t, tau, v, n):
    total = (n+1)*tau; t = min(max(t, 0), total); ec = n*tau
    if t <= tau: u = t/tau; return v*tau*(u**3-0.5*u**4)
    if t <= ec: return 0.5*v*tau+v*(t-tau)
    u = (t-ec)/tau; return 0.5*v*tau+v*(ec-tau)+v*tau*(u-(u**3-0.5*u**4))
def plan_walk(start, target, stride, speed, amp, tempo):
    p0 = start[0:3]; delta = sub(target, p0); dpl = sub(delta, sc(UP, dot(delta, UP))); dist = ln(dpl)
    dr = sc(dpl, 1/dist); left0 = sc(RIGHT, -1)
    psi0 = start[D_HEAD]; psi1 = math.atan2(dot(dr, left0), dot(dr, FWD))
    n = max(1, round(dist/stride)); step = dist/n; v = min(3, max(.2, speed*tempo)); tau = step/v; total = (n+1)*tau
    turn = min(2*tau, .6*total)
    psi_at = lambda t: psi0+(psi1-psi0)*ramp(t, 0, turn)
    def vrot(vv, k, ang):
        s, c = math.sin(ang), math.cos(ang); kv = cross(k, vv); kd = dot(k, vv)*(1-c)
        return [vv[i]*c+kv[i]*s+k[i]*kd for i in range(3)]
    feet0 = [start[D_FOOT+f*6:D_FOOT+f*6+3] for f in range(2)]; yaw0 = [start[D_FOOT+f*6+3] for f in range(2)]
    rest_ank = [rest_pos[LIMB[2+f][2]] for f in range(2)]
    swings = []; lp = list(feet0); ly = list(yaw0)
    for k in range(n+1):
        f = k % 2; t0 = k*tau+.1*tau; t1 = k*tau+.9*tau
        if k+2 <= n:
            dl = walk_dist(t1, tau, v, n); dn = walk_dist((k+2.1)*tau, tau, v, n); fk = dl+.5*(dn-dl)
        else: fk = dist
        ps = psi_at(t1)
        to = add(add(p0, sc(dr, fk)), vrot(rest_ank[f], UP, ps))
        swings.append((f, t0, t1, lp[f], to, ly[f], ps)); lp[f] = to; ly[f] = ps
    extra = max(walk_crouch(step)-stand_crouch(), 0)
    P = dict(p0=p0, dir=dr, psi0=psi0, psi1=psi1, turn=turn, tau=tau, v=v, n=n, swings=swings, feet0=feet0, yaw0=yaw0,
             extra=extra, lift=.04+.05*amp, bob=.008+.010*amp, sway=.012+.012*amp, twist=.04+.06*amp, arm_sw=.08+.12*amp,
             lean=.02+.03*amp, toe=.30*amp, total=total, start=start)
    arm_fz = []
    for side in range(2):
        rel = sub(rest_pos[LIMB[side][2]], rest_pos[LIMB[side][0]]); arm_fz.append((dot(rel, FWD), dot(rel, UP)))
    P['arm_fz'] = arm_fz
    return P
def foot_at(P, f, t):
    pos = P['feet0'][f]; yaw = P['yaw0'][f]
    for (ff, t0, t1, a, b, y0, y1) in P['swings']:
        if ff != f: continue
        if t < t0: break
        if t < t1:
            u = (t-t0)/(t1-t0); s = smoother(u); s2 = math.sin(math.pi*u)
            p = [a[i]+(b[i]-a[i])*s+UP[i]*P['lift']*s2*s2 for i in range(3)]
            return p, y0+(y1-y0)*s, P['toe']*s2*s2*(1-2*u)
        pos = b; yaw = y1
    return pos, yaw, 0.0
def eval_walk(P, t):
    t = min(max(t, 0), P['total']); s = P['start']; d = list(s)
    dd = walk_dist(t, P['tau'], P['v'], P['n'])
    for c in range(3): d[c] = P['p0'][c]+P['dir'][c]*dd
    d[D_HEAD] = P['psi0']+(P['psi1']-P['psi0'])*ramp(t, 0, P['turn'])
    tau = P['tau']; env = ramp(t, 0, tau)*(1-ramp(t, P['total']-tau, tau)); sp = math.sin(math.pi*t/tau)
    d[D_PEL+1] = s[D_PEL+1]+P['sway']*env*sp
    d[D_PEL+2] = s[D_PEL+2]-P['extra']*env+P['bob']*env*sp*sp
    d[D_PELROT+2] = s[D_PELROT+2]-P['twist']*env*sp
    d[D_SPINE+2] = s[D_SPINE+2]+.8*P['twist']*env*sp
    d[D_SPINE] = s[D_SPINE]+P['lean']*env
    for side in range(2):
        sgn = -1 if side == 0 else 1; swv = sgn*P['arm_sw']*env*sp
        f0, z0 = P['arm_fz'][side]; q = f0*f0+z0*z0
        lift = abs(z0)-math.sqrt(max(q-(f0+swv)**2, 1e-9))
        d[D_ARM+side*8] = s[D_ARM+side*8]+swv; d[D_ARM+side*8+2] = s[D_ARM+side*8+2]+lift
    for f in range(2):
        p, yaw, toe = foot_at(P, f, t)
        for c in range(3): d[D_FOOT+f*6+c] = p[c]
        d[D_FOOT+f*6+3] = yaw; d[D_FOOT+f*6+4] = toe
    return d
def stance_windows(P):
    out = [[], []]
    for f in range(2):
        mv = sorted((t0, t1) for (ff, t0, t1, *_r) in P['swings'] if ff == f); t = 0.0
        for a, b in mv:
            if a > t: out[f].append((t, a))
            t = max(t, b)
        if P['total'] > t: out[f].append((t, P['total']))
    return out

def run_walk(target, stride, speed, amp, tempo, foot_lock=True):
    d0 = init_drivers(); P = plan_walk(d0, target, stride, speed, amp, tempo)
    sw = stance_windows(P); res = {}
    rest_z = rest_pos[17][2]
    for f in range(2):
        foot = 17 if f == 0 else 20; worst = 0; wh = 0; chk = 0; dt = 1/30
        for (a, b) in sw[f]:
            a += .02; b -= .02
            t = a
            while t+dt <= b:
                p = solve(eval_walk(P, t), foot_lock)[0][foot]; q = solve(eval_walk(P, t+dt), foot_lock)[0][foot]
                worst = max(worst, ln(sub(p, q))/dt); wh = max(wh, abs(p[2]-rest_z)); chk += 1; t += dt
        res[foot] = (worst, wh, chk)
    # J on probes
    h = 1e-3; probes = [1, 4, 6, 11, 14, 17, 20]; wj = 0; t = h
    while t < P['total']-h:
        a = solve(eval_walk(P, t-h))[0]; b = solve(eval_walk(P, t))[0]; c = solve(eval_walk(P, t+h))[0]
        for j in probes:
            v = [a[j][i]-2*b[j][i]+c[j][i] for i in range(3)]; wj = max(wj, ln(v)/h)
        t += 0.007
    end = solve(eval_walk(P, P['total']))[0]
    return res, wj, P, end

if __name__ == "__main__":
    for (style, amp, tempo) in [("restrained", .45, .85), ("expressive", 1.0, 1.15)]:
        res, wj, P, end = run_walk([0, 3.0, 0], .5, 1.0, amp, tempo)
        print(style, "stance (speed m/s, height err m, checks):", {k: (round(v[0], 5), round(v[1], 6), v[2]) for k, v in res.items()})
        print(style, "worst J over probes (m/s):", round(wj, 4), " steps", P['n'], " total", round(P['total'], 3))
        print(style, "end ankle y", [round(end[j][1], 4) for j in (17, 20)], "pelvis", [round(x, 4) for x in end[1]])
    res, wj, P, end = run_walk([0, 3.0, 0], .5, 1.0, .45, .85, foot_lock=False)
    print("control foot_lock=False stance speed:", {k: round(v[0], 3) for k, v in res.items()})
    res, wj, P, end = run_walk([1.2, 2.4, 0], .45, 1.0, 1.0, 1.15)
    print("turn stance:", {k: (round(v[0], 5), round(v[1], 6), v[2]) for k, v in res.items()}, "J", round(wj, 4), "psi1", round(P['psi1'], 3))

# ---- wave / gaze / sit checks ----
def wave_drivers(t, amp, tempo, intensity, start):
    k = intensity; hc = add(rest_pos[HEAD], sc(UP, HEAD_C_UP))
    base = [dot(hc, FWD)+WAVE_FORWARD, dot(hc, RIGHT)+(CLEAR+WAVE_LATERAL), dot(hc, UP)-stand_crouch()+.04+.06*min(amp, 1.5)]
    freq = 2.4*tempo; t_r = .55/tempo; t_w = 3/freq; t_l = .6/tempo; dur = t_r+t_w+t_l
    t = min(max(t, 0), dur); d = list(start)
    w = ramp(t, 0, t_r)*(1-ramp(t, t_r+t_w, t_l)); o = .07*(.35+.65*amp)*k*w*math.sin(2*math.pi*freq*t)
    b = D_ARM+8; d[b+3] = w; d[b+4] = base[0]; d[b+5] = base[1]+o; d[b+6] = base[2]; d[b+7] = .5*w; d[D_SPINE+1] += .03*amp*w
    return d, dur
def wave_check(amp, tempo, intensity, clearance=True):
    start = init_drivers(); _, dur = wave_drivers(0, amp, tempo, intensity, start)
    mn = 9; top = -9; xs = []
    t = 0
    while t <= dur:
        d, _ = wave_drivers(t, amp, tempo, intensity, start)
        wp, wr, hc = solve(d, True, clearance)
        mn = min(mn, ln(sub(wp[14], hc))); top = max(top, wp[14][2])
        if .25*dur <= t <= .75*dur: xs.append(wp[14][0])
        t += .005
    return mn, top, max(xs)-min(xs)
def gaze_check(amp, target):
    d = init_drivers(); psi = 0
    eye = add(rest_pos[HEAD], sc(UP, HEAD_C_UP)); eye = add(eye, [0, 0, d[D_PEL+2]])
    dr = sub(target, eye); df = dot(dr, FWD); drr = dot(dr, RIGHT); du = dot(dr, UP)
    yaw = math.atan2(-drr, df); pitch = math.atan2(-du, math.hypot(df, drr)); g = .6+.4*min(amp, 1)
    gy = max(-1.3, min(1.3, yaw*g)); gp = max(-.6, min(.6, pitch*g))
    d[D_GAZE] = gy; d[D_GAZE+1] = gp; d[D_SPINE+2] = .12*gy
    wp, wr, hc = solve(d)
    f = qrot(wr[HEAD], [0, 1, 0]); return math.degrees(math.atan2(-f[0], f[1]))
def sit_check():
    d = init_drivers(); p1 = [0, -.28, .55]
    base = rest_pos[PEL]; off = sub(p1, base)
    d2 = list(d); d2[D_PEL] = dot(off, FWD); d2[D_PEL+1] = dot(off, RIGHT); d2[D_PEL+2] = dot(off, UP)
    wp, wr, hc = solve(d2)
    wp0, _, _ = solve(d)
    return wp[1], [ln(sub(wp[j], wp0[j])) for j in (17, 20)], wp[16]
if __name__ == "__main__":
    for (nm, amp, tempo, k) in [("restrained", .45, .85, 1.5), ("expressive", 1.0, 1.15, 1.5), ("huge", 3.0, 1.15, 1.5)]:
        print("wave", nm, "min hand-head %.4f top z %.3f p2p x %.4f" % wave_check(amp, tempo, k))
    print("wave huge, clearance OFF min hand-head %.4f" % wave_check(3.0, 1.15, 1.5, False)[0])
    a = wave_check(.45, .85, 1.0)[2]; b = wave_check(1.0, 1.15, 1.0)[2]; print("p2p ratio expressive/restrained %.3f" % (b/a))
    for amp in (.45, 1.0, .25):
        print("gaze yaw deg amp", amp, round(gaze_check(amp, [-1.414, 1.414, 1.644]), 2))
    print("gaze straight ahead", round(gaze_check(.45, [0, 2, 1.55]), 2))
    print("sit pelvis, feet moved, left knee:", sit_check())
