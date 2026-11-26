# Independent float32 replication of the documented canonical bytes for the fixture at
# upper_length=1.25, girth=0.5. numpy float32 scalars: every op rounds to f32, no FMA.
import hashlib, struct, sys
import numpy as np
f = np.float32
def pf(x): return struct.pack('<I', int(np.float32(x).view(np.uint32)))
def pu(x): return struct.pack('<I', x)
def pstr(s): b=s.encode(); return pu(len(b))+b

UL = f(float(sys.argv[1])) if len(sys.argv)>1 else f(1.25)
G  = f(float(sys.argv[2])) if len(sys.argv)>2 else f(0.5)

# fixture
pos=[]
for r in range(4):
    for k in range(4):
        sx = f(-0.25) if k in (0,3) else f(0.25)
        sz = f(-0.25) if k<2 else f(0.25)
        pos.append([sx, f(r), sz])
idx=[]
for r in range(3):
    for k in range(4):
        a=r*4+k; b=r*4+(k+1)%4; c=a+4; d=b+4
        idx += [a,b,c,b,d,c]
skin=[]
for r in range(4):
    for k in range(4):
        if r==0: s=[(0,1.0)]
        elif r==1: s=[(0,0.5),(1,0.5)]
        elif r==2:
            s={0:[(0,0.125),(1,0.375),(2,0.5)],1:[(2,0.75),(1,0.25)],2:[(1,0.5),(2,0.5)],3:[(1,0.25),(2,0.75)]}[k]
        else: s=[(2,1.0)]
        skin.append(s)
deltas=[]
for v in range(4,12):
    k=v%4
    sx = f(-0.1) if k in (0,3) else f(0.1)
    sz = f(-0.1) if k<2 else f(0.1)
    deltas.append((v,[sx,f(0.0),sz]))
joints=[("root",-1,[f(0),f(0),f(0)]),("upper",0,[f(0),f(1),f(0)]),("lower",1,[f(0),f(1),f(0)])]
landmarks=[("head_top",2,[f(0),f(1),f(0)]),("upper_mid",1,[f(0),f(0.5),f(0)])]

# shaped skeleton: children of joint 1 get translation scaled by UL
sj=[]
for n,p,t in joints:
    t=list(t)
    if p==1: t=[t[0]*UL,t[1]*UL,t[2]*UL]
    sj.append((n,p,t))
# morph: weight = 0.0 + G ; positions
w = f(0.0)+G
mpos=[list(p) for p in pos]
for v,d in deltas:
    for c in range(3):
        mpos[v][c] = mpos[v][c] + w*d[c]

# matrices column-major cols[c][r]
def ident():
    return [[f(1 if r==c else 0) for r in range(4)] for c in range(4)]
def from_t(t):
    m=ident(); m[3]=[t[0],t[1],t[2],f(1)]; return m
def mul(a,b):
    out=[[f(0)]*4 for _ in range(4)]
    for c in range(4):
        for r in range(4):
            out[c][r]=a[0][r]*b[c][0]+a[1][r]*b[c][1]+a[2][r]*b[c][2]+a[3][r]*b[c][3]
    return out
def inv(m):
    a00,a01,a02=m[0][0],m[1][0],m[2][0]
    a10,a11,a12=m[0][1],m[1][1],m[2][1]
    a20,a21,a22=m[0][2],m[1][2],m[2][2]
    c00=a11*a22-a12*a21; c01=a12*a20-a10*a22; c02=a10*a21-a11*a20
    c10=a02*a21-a01*a22; c11=a00*a22-a02*a20; c12=a01*a20-a00*a21
    c20=a01*a12-a02*a11; c21=a02*a10-a00*a12; c22=a00*a11-a01*a10
    det=a00*c00+a01*c01+a02*c02
    i00=c00/det;i01=c10/det;i02=c20/det
    i10=c01/det;i11=c11/det;i12=c21/det
    i20=c02/det;i21=c12/det;i22=c22/det
    tx,ty,tz=m[3][0],m[3][1],m[3][2]
    t0=-(i00*tx+i01*ty+i02*tz); t1=-(i10*tx+i11*ty+i12*tz); t2=-(i20*tx+i21*ty+i22*tz)
    return [[i00,i10,i20,f(0)],[i01,i11,i21,f(0)],[i02,i12,i22,f(0)],[t0,t1,t2,f(1)]]
world=[]
for n,p,t in sj:
    l=from_t(t)
    world.append(l if p<0 else mul(world[p],l))
ibs=[inv(w_) for w_ in world]

o=b"gibson.figure.rev.v0\0"
o+=pstr("synthetic.tube")+pu(1)
o+=pu(len(mpos))
for p in mpos: o+=b''.join(pf(c) for c in p)
o+=pu(len(idx))+b''.join(pu(i) for i in idx)
o+=pu(len(sj))
for n,p,t in sj:
    o+=pstr(n)+struct.pack('<i',p)+b''.join(pf(c) for c in t)+b''.join(pf(c) for c in (0,0,0,1))+b''.join(pf(c) for c in (1,1,1))
o+=pu(len(landmarks))
for n,j,off in landmarks: o+=pstr(n)+pu(j)+b''.join(pf(c) for c in off)
o+=pu(len(ibs))
for m in ibs:
    for c in range(4):
        for r in range(4): o+=pf(m[c][r])
o+=pu(len(skin))
for s in skin:
    o+=bytes([len(s)])
    for j,wt in s: o+=pu(j)+pf(wt)
o+=pu(1)+pstr("girth")+pu(len(deltas))
for v,d in deltas: o+=pu(v)+b''.join(pf(c) for c in d)
print(hashlib.sha256(o).hexdigest())
