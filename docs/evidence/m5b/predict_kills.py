# PREDICTION ONLY (not run against the Rust implementation; no compiler is available to the
# author of this file). Predicts, for the rich fixture of tests/figure_mechanics.rs, how far each
# planted mutant (A ignored_skeleton_update, B stale_bind, C noop_morph) moves the posed mesh, the
# inverse binds and the world bind away from an f64 row-major oracle, in the same units as TOL.
#
# - The "impl" column is a numpy float32 MODEL of the implementation (it imports golden_rev_id.py
#   for the matrix code, so it shares its formulas), with the mutants applied by hand. The
#   skinning blend order is an assumption. It is a model, not the implementation.
# - The f64 oracle here uses numpy (np.linalg.inv); the Rust oracle uses Gauss-Jordan. Same maths.
# - Each entry is max over coordinates of |impl - oracle| / max(1, |oracle|); a test with TOL = 1e-4
#   fails (kills the mutant) when the entry exceeds 1e-4.
# Run: python3 -I docs/evidence/m5b/predict_kills.py
import importlib.util, math, copy
import numpy as np
import os
spec = importlib.util.spec_from_file_location("g", os.path.join(os.path.dirname(os.path.abspath(__file__)), "golden_rev_id.py"))
g = importlib.util.module_from_spec(spec); spec.loader.exec_module(g)
f = np.float32
TOL = 1e-4

def aa(axis, ang):
    n = math.sqrt(sum(a*a for a in axis)); s, c = math.sin(ang/2), math.cos(ang/2)
    return [f(axis[0]/n*s), f(axis[1]/n*s), f(axis[2]/n*s), f(c)]

# ---------- f64 oracle (row-major, np inverse, different formulation) ----------
def rot(q):
    q = np.array(q, float); q /= np.linalg.norm(q)
    v = q[:3]; w = q[3]
    K = np.array([[0,-v[2],v[1]],[v[2],0,-v[0]],[-v[1],v[0],0]])
    R = (w*w - v@v)*np.eye(3) + 2*np.outer(v,v) + 2*w*K
    return R
def trs(t,q,s):
    M = np.eye(4); T=np.eye(4); T[:3,3]=np.array(t,float,); R=np.eye(4); R[:3,:3]=rot(q); S=np.diag(list(map(float,s))+[1.0])
    return T@R@S
def ctl_values(ul, torso, girth, fine, taper): return [ul,torso,girth,fine,taper]
def o_rest(ctl, scale_bones=True):
    rest=[[np.array([float(x) for x in t]),[float(x) for x in q],[float(x) for x in s]] for n,p,t,q,s in g.joints]
    if scale_bones:
        for c,(cid,kind,target) in zip(ctl,g.CONTROLS):
            if kind=="bone":
                for i,j in enumerate(g.joints):
                    if j[1]==target: rest[i][0]=rest[i][0]*c
    return rest
def o_world(rest):
    W=[]
    for i,j in enumerate(g.joints):
        L=trs(*rest[i]); W.append(L if j[1]<0 else W[j[1]]@L)
    return W
def o_pos(ctl, pose_w, apply=True):
    P=np.array([[float(c) for c in p] for p in g.pos])
    w=np.zeros(2)
    for c,(cid,kind,target) in zip(ctl,g.CONTROLS):
        if kind=="morph": w[target]+=c
    if apply:
        for (name,d),wt in zip(g.morphs,w):
            for v,dd in d: P[v]+=wt*np.array([float(x) for x in dd])
        for (name,d),wt in zip(g.morphs,pose_w):
            for v,dd in d: P[v]+=wt*np.array([float(x) for x in dd])
    return P
def oracle(ctl, ov, pose_w):
    rest=o_rest(ctl); Wb=o_world(rest); IB=[np.linalg.inv(w) for w in Wb]
    loc=copy.deepcopy(rest)
    for j,(t,q,s) in ov.items(): loc[j]=[np.array(t,float),[float(x) for x in q],[float(x) for x in s]]
    Wp=o_world(loc); P=o_pos(ctl,pose_w)
    out=[]
    for v,pv in enumerate(P):
        acc=np.zeros(3)
        for j,wt in g.skin[v]:
            acc+=float(wt)*((Wp[j]@(IB[j]@np.append(pv,1)))[:3])
        out.append(acc)
    return np.array(out),Wb,IB

# ---------- f32 mirror of impl (+ mutants) ----------
def impl(ctl, ov, pose_w, mut=None):
    sj=[[n,p,list(t),list(q),list(s)] for n,p,t,q,s in g.joints]
    wm=[f(0),f(0)]
    for v,(cid,kind,target) in zip(ctl,g.CONTROLS):
        v=f(v)
        if kind=="bone":
            if mut=="A": continue
            for j in sj:
                if j[1]==target: j[2]=[j[2][0]*v,j[2][1]*v,j[2][2]*v]
        else: wm[target]=wm[target]+v
    P=[list(p) for p in g.pos]
    def apply(P,ws):
        if mut=="C": return P
        P=[list(p) for p in P]
        for (name,d),w in zip(g.morphs,ws):
            for v,dd in d: P[v]=[P[v][c]+w*dd[c] for c in range(3)]
        return P
    P=apply(P,wm)
    def worlds(joints):
        W=[]
        for n,p,t,q,s in joints:
            l=g.from_trs(t,q,s); W.append(l if p<0 else g.mul(W[p],l))
        return W
    Wb=worlds(sj)
    base=[[n,p,list(t),list(q),list(s)] for n,p,t,q,s in g.joints]
    IB=[g.inv_affine(w) for w in (worlds(base) if mut=="B" else Wb)]
    loc=[list(j) for j in sj]
    for j,(t,q,s) in ov.items(): loc[j]=[loc[j][0],loc[j][1],[f(x) for x in t],[f(x) for x in q],[f(x) for x in s]]
    Wp=worlds(loc)
    P=apply(P,[f(x) for x in pose_w])
    out=[]
    for v,pv in enumerate(P):
        acc=[f(0)]*3
        for j,wt in g.skin[v]:
            M=g.mul(Wp[j],IB[j])
            x=[M[0][r]*pv[0]+M[1][r]*pv[1]+M[2][r]*pv[2]+M[3][r] for r in range(3)]
            acc=[acc[r]+wt*x[r] for r in range(3)]
        out.append([float(a) for a in acc])
    return np.array(out),Wb,IB

def cmp(a,b): return float(np.max(np.abs(a-b)/np.maximum(1,np.abs(b))))
def mat32(M): return np.array([[float(M[c][r]) for c in range(4)] for r in range(4)])


REC=[float(g.RECIPE[k]) for k in ["upper_length","torso_length","girth","girth_fine","taper"]]
q=lambda ax,an: aa(ax,an)
POSES={
 "bind pose": ({}, [0,0]),
 "root-only override (L_j cancel)": ({0:([0.12,-0.04,0.18],q([0.2,1,-0.1],0.35),[1,1,1])},[0,0]),
 "joints 1+2 overridden": ({1:([0.05,1.05,-0.03],q([1,.5,-.3],.5),[1,1,1]), 2:([0.0,1.1,0.05],q([.3,1,.2],-.6),[1.1,.9,1.05])},[0,0]),
 "joint 2 only": ({2:([0.0,1.1,0.05],q([.3,1,.2],-.6),[1.1,.9,1.05])},[0,0]),
 "root+arm overridden, pose morphs": ({0:([0.1,-0.05,0.2],q([0,1,0],.3),[1,1,1]),3:([0.6,0.3,0.05],q([1,0,1],.4),[1,1.1,.9])},[0.3,0.5]),
}
print("pinned recipe", REC, "TOL", TOL)
print("%-36s %-5s %10s %10s %10s"%("pose","impl","mesh","inv bind","world bind"))
for name,(ov,pw) in POSES.items():
    o,Wo,IBo=oracle(REC,ov,pw)
    for mut in (None,"A","B","C"):
        i,Wi,IBi=impl(REC,ov,pw,mut)
        print("%-36s %-5s %10.2e %10.2e %10.2e"%(name,mut or "none",cmp(i,o),max(cmp(mat32(IBi[k]),IBo[k]) for k in range(4)),max(cmp(mat32(Wi[k]),Wo[k]) for k in range(4))))
