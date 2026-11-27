#!/usr/bin/env python3
"""Pure-python reproduction of the two C-1 failures of tests/figure_perform.rs and of their fix.
Run: python3 -I model_perform_cut.py [--phases] [--floor]

Adds to model_perform_solver.py (loaded by path) the beat plans the continuity script uses (look,
wave, nod, walk, settle, sit/stand, idle), BeatMotion's finite-difference rates, and the Preempt
quintic inertialization of motion.rs (scalar channels), then evaluates:

  A  every_beat_boundary_and_foot_lock_transition_is_c1   (the test's own time grid, all probes)
  B  live_reduced_motion_lowers_amplitude, the J part     (hand + head, 1 ms ticks, 900..1550)
  C  live preemption wave -> nod at 1200 ms               (a passing test, kept as a check)
  D  live wave expiry at 1500 ms                          (a passing test, kept as a check)
  --phases: a wave cut at every 37 ms of the plateau, both styles, min |target - shoulder|.

each once with the pre-fix constants (WAVE_FORWARD 0.05, wave point resting at the origin) and once
with the shipped ones (0.12, resting at the hang). Root cause: the hand target of a wave is the
straight blend hang + (wave - hang) w; in a cut blend it passes the shoulder nearer than the IK fold
limit (0.0587 m), and soft_reach clamps it to that sphere, a velocity step (J = 1.20 / 1.36).
Second mechanism, only visible once the first is gone: resting at the origin, the inertialized
wave point flies in from the floor during the first raise (J = 0.10 at t = 1.218 s).
Same author as perform.rs: a common-mode mirror, not an oracle; the Rust gate decides.
"""
import importlib.util, math, os, sys

_p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "model_perform_solver.py")
_spec = importlib.util.spec_from_file_location("model_perform_solver", _p)
S = importlib.util.module_from_spec(_spec); _spec.loader.exec_module(S)

D_ARM, D_SPINE, D_GAZE, D_NOD, D_HEAD = S.D_ARM, S.D_SPINE, S.D_GAZE, S.D_NOD, S.D_HEAD
D_PEL, D_PELROT, D_FOOT = S.D_PEL, S.D_PELROT, S.D_FOOT
ramp, smoother, pi = S.ramp, S.smoother, math.pi
H = 1e-3
PROBES = {1: "pelvis", 4: "spine3", 6: "head", 11: "hand_l", 14: "hand_r", 17: "foot_l", 20: "foot_r"}


class Plan:
    def __init__(s, dur, start, fn): s.dur = dur; s.start = list(start); s.fn = fn
    def eval(s, t): return s.fn(min(max(t, 0.0), s.dur), list(s.start))


def plan_look(start, target, amp, tempo):
    eye = S.add(S.add(start[0:3], S.add(S.rest_pos[S.HEAD], S.sc(S.UP, S.HEAD_C_UP))),
                S.add(S.add(S.sc(S.FWD, start[D_PEL]), S.sc(S.RIGHT, start[D_PEL+1])), S.sc(S.UP, start[D_PEL+2])))
    dr = S.sub(target, eye); df = S.dot(dr, S.FWD); drr = S.dot(dr, S.RIGHT); du = S.dot(dr, S.UP)
    g = .6+.4*min(amp, 1)
    gy = max(-1.3, min(1.3, math.atan2(-drr, df)*g)); gp = max(-.6, min(.6, math.atan2(-du, math.hypot(df, drr))*g))
    ease = .55/tempo; s0 = list(start)
    def fn(t, d):
        r = ramp(t, 0, ease)
        d[D_GAZE] = s0[D_GAZE]+(gy-s0[D_GAZE])*r; d[D_GAZE+1] = s0[D_GAZE+1]+(gp-s0[D_GAZE+1])*r
        d[D_SPINE+2] = s0[D_SPINE+2]+(.12*gy-s0[D_SPINE+2])*r; return d
    return Plan(ease+.35, start, fn)


def plan_wave(start, amp, tempo, k=1.0):
    _, dur = S.wave_drivers(0, amp, tempo, k, start)
    return Plan(dur, start, lambda t, d: S.wave_drivers(t, amp, tempo, k, d)[0])


def plan_nod(start, amp, tempo, k=1.0):
    period = .72/tempo; depth = .30*k*(.3+.7*min(amp, 1.5))
    def fn(t, d):
        v = depth*.5*(1-math.cos(2*pi*t/period)) if t < 2*period else 0.0
        d[D_NOD] += v; d[D_SPINE] += .3*v; return d
    return Plan(2*period+.15, start, fn)


def plan_idle(start, amp, tempo, secs):
    dur = min(max(secs, .1), 600)
    def fn(t, d):
        e = min(dur*.5, .6); env = ramp(t, 0, e)*(1-ramp(t, dur-e, e))
        br = math.sin(2*pi*t*tempo/4)
        d[D_SPINE] += .012*amp*env*br; d[D_PEL+1] += .005*amp*env*math.sin(2*pi*t*tempo/6.5)
        d[D_PEL+2] += .0015*amp*env*br; d[D_GAZE] += .03*amp*env*math.sin(2*pi*t*tempo/9.1); return d
    return Plan(dur, start, fn)


def plan_settle(start, seated, settle):
    tg = [(D_PELROT+i, 0.0) for i in range(3)]+[(D_SPINE+i, 0.0) for i in range(3)]
    tg += [(D_GAZE, 0.0), (D_GAZE+1, 0.0), (D_NOD, 0.0)]
    if not seated: tg += [(D_PEL, 0.0), (D_PEL+1, 0.0)]
    tg += [(D_ARM+side*8+k, 0.0) for side in range(2) for k in (0, 1, 2, 3, 7)]
    dur = 2*settle+.2; s0 = list(start)
    def fn(t, d):
        r = ramp(t, 0, dur)
        for i, v in tg: d[i] = s0[i]+(v-s0[i])*r
        return d
    return Plan(dur, start, fn)


def plan_move(start, to, lean, dur):
    s0 = list(start)
    def fn(t, d):
        u = t/dur; uh = smoother(u/.75); uv = smoother((u-.12)/.88)
        d[D_PEL] = s0[D_PEL]+(to[0]-s0[D_PEL])*uh; d[D_PEL+1] = s0[D_PEL+1]+(to[1]-s0[D_PEL+1])*uh
        d[D_PEL+2] = s0[D_PEL+2]+(to[2]-s0[D_PEL+2])*uv; d[D_SPINE] += lean*math.sin(pi*u)**2; return d
    return Plan(dur, start, fn)


def plan_walk(start, target, stride, speed, amp, tempo):
    P = S.plan_walk(start, target, stride, speed, amp, tempo); s = list(start)
    def fn(t, d):  # perform.rs Kind::Walk (with the gaze/nod/spine-yaw fade the solver mirror omits)
        tau = P['tau']; total = P['total']; dd = S.walk_dist(t, tau, P['v'], P['n'])
        for c in range(3): d[c] = P['p0'][c]+P['dir'][c]*dd
        d[D_HEAD] = P['psi0']+(P['psi1']-P['psi0'])*ramp(t, 0, P['turn'])
        env = ramp(t, 0, tau)*(1-ramp(t, total-tau, tau)); sp = math.sin(pi*t/tau); gz = 1-ramp(t, 0, 2*tau)
        d[D_PEL+1] = s[D_PEL+1]+P['sway']*env*sp
        d[D_PEL+2] = s[D_PEL+2]-P['extra']*env+P['bob']*env*sp*sp
        d[D_PELROT+2] = s[D_PELROT+2]-P['twist']*env*sp
        d[D_SPINE+2] = s[D_SPINE+2]*gz+.8*P['twist']*env*sp; d[D_SPINE] = s[D_SPINE]+P['lean']*env
        d[D_GAZE] = s[D_GAZE]*gz; d[D_GAZE+1] = s[D_GAZE+1]*gz; d[D_NOD] = s[D_NOD]*gz
        for side in range(2):
            sw = (-1 if side == 0 else 1)*P['arm_sw']*env*sp; f0, z0 = P['arm_fz'][side]
            d[D_ARM+side*8] = s[D_ARM+side*8]+sw
            d[D_ARM+side*8+2] = s[D_ARM+side*8+2]+abs(z0)-math.sqrt(max(f0*f0+z0*z0-(f0+sw)**2, 1e-9))
        for f in range(2):
            p, yaw, toe = S.foot_at(P, f, t)
            for c in range(3): d[D_FOOT+f*6+c] = p[c]
            d[D_FOOT+f*6+3] = yaw; d[D_FOOT+f*6+4] = toe
        return d
    pl = Plan(P['total'], start, fn); pl.P = P; return pl


class BeatMotion:  # perform.rs BeatMotion::sample: value + finite-difference rate
    def __init__(s, plan, start): s.p = plan; s.t0 = start
    def sample(s, t):
        tl = t-s.t0; h = 1e-4; P = s.p
        if tl <= h:
            t0 = max(tl, 0); a, b, c = P.eval(t0), P.eval(t0+h), P.eval(t0+2*h)
            return a, [(-3*a[i]+4*b[i]-c[i])/(2*h) for i in range(len(a))]
        if tl >= P.dur-h:
            a, b, c = P.eval(tl), P.eval(tl-h), P.eval(tl-2*h)
            return a, [(3*a[i]-4*b[i]+c[i])/(2*h) for i in range(len(a))]
        m, a, b = P.eval(tl), P.eval(tl-h), P.eval(tl+h)
        return m, [(b[i]-a[i])/(2*h) for i in range(len(a))]


class Const:
    def __init__(s, v): s.v = list(v)
    def sample(s, t): return list(s.v), [0.0]*len(s.v)


def h0(x): return 1-smoother(x)
def dh0(x): return -30*x*x*(1-x)**2
def h1(x): return x-6*x**3+8*x**4-3*x**5
def dh1(x): return 1-18*x**2+32*x**3-15*x**4


class Preempt:  # motion.rs Preempt + inertialize (Scalar)
    def __init__(s, old, new, at, ease): s.o = old; s.n = new; s.at = at; s.e = ease
    def sample(s, t):
        if t < s.at: return s.o.sample(t)
        if t-s.at >= s.e: return s.n.sample(t)
        T = s.e; x = (t-s.at)/T
        (vo, ro), (vn0, rn0), (vnt, rnt) = s.o.sample(s.at), s.n.sample(s.at), s.n.sample(t)
        return ([vnt[i]+(vo[i]-vn0[i])*h0(x)+(ro[i]-rn0[i])*T*h1(x) for i in range(len(vnt))],
                [rnt[i]+(vo[i]-vn0[i])*dh0(x)/T+(ro[i]-rn0[i])*dh1(x) for i in range(len(vnt))])


def j_at(m, t, joints):
    a, b, c = (S.solve(m.sample(t+k*H)[0])[0] for k in (-1, 0, 1))
    return max((S.ln([a[j][i]-2*b[j][i]+c[j][i] for i in range(3)])/H, j) for j in joints)


def shoulder_dist(d):
    """|hand target - right shoulder| before any reach limit (the fold limit acts on this)."""
    wpos, wrot, _ = S.solve_pass(d, True, None)
    yq = S.qaa(S.UP, d[D_HEAD]); hf = S.qrot(yq, S.FWD); hr = S.qrot(yq, S.RIGHT); b = D_ARM+8
    pel_off = S.add(S.add(S.sc(hf, d[D_PEL]), S.sc(hr, d[D_PEL+1])), S.sc(S.UP, d[D_PEL+2]))
    hs = S.add(S.add(S.qrot(yq, S.rest_pos[14]), pel_off),
               S.add(S.add(S.sc(hf, d[b]), S.sc(hr, d[b+1])), S.sc(S.UP, d[b+2])))
    wave = S.add(S.add(S.sc(hf, d[b+4]), S.sc(hr, d[b+5])), S.sc(S.UP, d[b+6]))
    tgt = S.add(hs, S.sc(S.sub(wave, hs), d[b+3]))
    return S.ln(S.sub(tgt, S.add(wpos[4], S.qrot(wrot[4], S.J[12][2]))))


def continuity():  # tests/figure_perform.rs continuity_script(), restrained, wave cut at 1.2 s
    amp, tempo, settle = .45, .85, .45
    carry = S.init_drivers(); seated = False; cursor = 0.0; chain = None; bnd = []
    for b in ["look", "wave", "ack", "walk", "stop", "sit", "idle1", "stand", "idle06"]:
        if b == "look": p = plan_look(carry, [-1.4, 1.4, 1.64], amp, tempo)
        elif b == "wave": p = plan_wave(carry, amp, tempo)
        elif b == "ack": p = plan_nod(carry, amp, tempo)
        elif b == "walk": p = plan_walk(carry, [0, 2.0, 0], .4, .9, amp, tempo)
        elif b == "stop": p = plan_settle(carry, seated, settle)
        elif b == "sit":
            off = S.sub([0, 1.72, .55], S.add(carry[0:3], S.rest_pos[S.PEL]))
            p = plan_move(carry, [S.dot(off, S.FWD), S.dot(off, S.RIGHT), S.dot(off, S.UP)],
                          .25*(.5+.5*min(amp, 1)), max(1.9/tempo, 1.0)); seated = True
        elif b == "stand":
            p = plan_move(carry, [0, 0, -S.stand_crouch()], .25*(.5+.5*min(amp, 1)), max(1.9/tempo, 1.0)); seated = False
        else: p = plan_idle(carry, amp, tempo, 1.0 if b == "idle1" else .6)
        bnd.append(cursor)
        if b == "walk":
            for (f, t0, t1, *_r) in p.P['swings']: bnd += [cursor+t0, cursor+t1]
        bm = BeatMotion(p, cursor)
        chain = bm if chain is None else Preempt(chain, bm, cursor, settle)
        carry = p.eval(p.dur); cursor += 1.2 if b == "wave" else p.dur
    dur = cursor+settle
    times = [t0-.04+i*H for t0 in bnd for i in range(81)]
    t = .01
    while t < dur-.01: times.append(t); t += .008
    return max((j_at(chain, t, PROBES)+(t,)) for t in times if t-H >= 0 and t+H <= dur)


def live(cut_at, nxt, ease):  # Live::new(expressive) + wave at 0, preempted at cut_at
    d0 = S.init_drivers(); wv = plan_wave(d0, 1.0, 1.15)
    m = Preempt(Const(d0), BeatMotion(wv, 0.0), 0.0, .28)
    return Preempt(m, BeatMotion(nxt(wv.eval(wv.dur)), cut_at), cut_at, ease)


def live_j(m, lo, hi):
    pts = {ms: S.solve(m.sample(ms/1000)[0])[0] for ms in range(lo-1, hi+2)}
    return max((S.ln([pts[i-1][j][c]-2*pts[i][j][c]+pts[i+1][j][c] for c in range(3)])/H, i)
               for i in range(lo, hi+1) for j in (14, 6))


def phases():
    for nm, amp, tempo, st, T in (("restrained", .45, .85, .45, .45), ("expressive", 1.0, 1.15, .28, .28),
                                  ("expressive", 1.0, 1.15, .28, .30)):
        d0 = S.init_drivers(); wv = plan_wave(d0, amp, tempo); t_r = .55/tempo; t_w = 3/(2.4*tempo)
        base = Preempt(Const(d0), BeatMotion(wv, 0.0), 0.0, st); worst = 9; cut = t_r+.02; n = 0
        while cut < t_r+t_w:
            m = Preempt(base, BeatMotion(plan_nod(wv.eval(wv.dur), amp, tempo), cut), cut, T)
            worst = min([worst]+[shoulder_dist(m.sample(cut+i*.002)[0]) for i in range(int(T/.002)+1)])
            cut += .037; n += 1
        print("    phases %-10s ease %.2f: %2d cuts, min |target - shoulder| %.4f m (fold limit 0.0587)" % (nm, T, n, worst))


def report(label):
    print("%s  (WAVE_FORWARD %.2f, wave point rests at the hang: %s)" % (label, S.WAVE_FORWARD, S.WAVE_REST_AT_HANG))
    j, jt, t = continuity()
    print("  A continuity grid       J = %.4f m/s at t = %.4f s (%s)   bound 0.09" % (j, t, PROBES[jt]))
    j, ms = live_j(live(1.0, lambda s: plan_idle(s, .25, 1.15, 1.0), .30), 900, 1550)
    print("  B live reduced @1000 ms J = %.4f at %d ms" % (j, ms))
    j, ms = live_j(live(1.2, lambda s: plan_nod(s, 1.0, 1.15), .28), 1100, 1650)
    print("  C live nod @1200 ms     J = %.4f at %d ms" % (j, ms))
    j, ms = live_j(live(1.5, lambda s: plan_idle(s, 1.0, 1.15, 1.0), .28), 1300, 2300)
    print("  D live expiry @1500 ms  J = %.4f at %d ms" % (j, ms))
    if "--phases" in sys.argv: phases()


def poses_j(m, t0, n):
    """J (hand_r, head) and min |hand - head centre| over n+1 poses at 1 ms from t0, each pose once."""
    sol = [S.solve(m.sample(t0+i*H)[0]) for i in range(n+1)]
    j = max(S.ln([sol[i-1][0][k][c]-2*sol[i][0][k][c]+sol[i+1][0][k][c] for c in range(3)])/H
            for i in range(1, n) for k in (14, 6))
    return j, min(S.ln(S.sub(w[14], hc)) for (w, _, hc) in sol)


def phase_test():
    """Mirror of tests/figure_perform.rs a_wave_cut_at_any_phase_and_the_clearance_floor_are_c1."""
    out = []
    for nm, amp, tempo, st in (("restrained", .45, .85, .45), ("expressive", 1.0, 1.15, .28)):
        t_r = .55/tempo; per = 1/(2.4*tempo); d0 = S.init_drivers(); wv = plan_wave(d0, amp, tempo)
        for mode in ("offline", "live"):
            first = BeatMotion(wv, 0.0) if mode == "offline" else Preempt(Const(d0), BeatMotion(wv, 0.0), 0.0, st)
            w = 0.0; dm = 9.0
            for k in range(7):
                c = round((t_r+.15+k*per/6)*1000)/1000
                m = Preempt(first, BeatMotion(plan_nod(wv.eval(wv.dur), amp, tempo), c), c, st)
                j, d = poses_j(m, c-.003, int(round(st*1000))+8); w = max(w, j); dm = min(dm, d)
            out.append("%s/%s J %.4f (min hand-head %.4f)" % (nm, mode, w, dm))
    # The clearance floor where a large slow wave meets it: amplitude 2.0, tempo 0.6, intensity 1.5.
    wv = plan_wave(S.init_drivers(), 2.0, 0.6, 1.5); m = BeatMotion(wv, 0.0)
    # its inner swing meets the clearance on the way down (3.21..3.36 s, 0.189 m with clearance off)
    j, d = poses_j(m, 3.15, 250)
    sol = [S.solve(m.sample(3.15+i*H)[0], True, False) for i in range(251)]
    off = min(S.ln(S.sub(w[14], hc)) for (w, _, hc) in sol)
    out.append("large wave J %.4f, min hand-head %.4f, clearance off %.4f" % (j, d, off))
    return out


def floor_report():
    for hard in (False, True):
        S.HARD_PUSH_OUT = hard
        print("%s  (WAVE_LATERAL %.2f, CLEARANCE_SOFT %.2f)" % ("MUTANT hard_push_out" if hard else "SHIPPED soft floor",
                                                               S.WAVE_LATERAL, S.CLEARANCE_SOFT))
        for line in phase_test(): print("  " + line)
    S.HARD_PUSH_OUT = False


if __name__ == "__main__":
    S.WAVE_FORWARD, S.WAVE_REST_AT_HANG, S.WAVE_LATERAL, S.HARD_PUSH_OUT = 0.05, False, 0.07, True
    report("PRE-FIX (2f3081c^)")
    S.WAVE_FORWARD, S.WAVE_REST_AT_HANG = 0.12, True
    report("2f3081c (fold + hang fixes, lateral 0.07, hard clearance)")
    S.WAVE_LATERAL, S.HARD_PUSH_OUT = 0.12, False
    report("SHIPPED (lateral 0.12, soft clearance floor)")
    if "--floor" in sys.argv: floor_report()
