#!/usr/bin/env python3
"""Pure-python (no bpy, no numpy) tape reader and kinematics for the M5-C offline render adapter.

    python3 -I tools/figure_assets/fa_tape.py --selftest

Tape format: docs/design/m5c-studio-api.md section 5, `gibson.figure.tape.v1`:
`<name>.tape.json` header + `<name>.tape.bin` (little-endian f32, no padding).  One frame is
    root_t3 (3), then per joint in header `joints` order: local t3, rotation quaternion xyzw, scale s3 (10),
    then len(face) face channels.

ASSUMPTIONS (the section does not spell these out; they follow docs/FIGURE_CONTRACT.md on the perform
branch, where a pose REPLACES a joint's rest local transform and `W[i] = W[parent] * L[i]`):
  * L[i] = T(local_t3) * R(local_r4) * S(local_s3) is the joint's full local transform relative to its parent
    (absolute, not a delta from rest);
  * root_t3 is an extra model-space translation applied on top of every root joint: W[root] = T(root_t3) * L[root];
  * the joint frames at bind have translation = the joint position re-derived from the SHAPED body and a rotation that
    is either identity ("world_aligned", the default) or the exported bone rotation ("bone").  The mode is a
    command-line option of the adapter and is echoed in render.json, because it is the one assumption the oracle
    can falsify (a mismatch shows up as a large oracle error on a posed frame).

Matrices are 4x4 row-major lists of lists, column vectors (p' = M p).  The skin matrix of joint i is
S[i] = W[i] * inv(Wbind[i]).
"""
import array
import hashlib
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)
import fa_common as fc  # noqa: E402

FORMAT = "gibson.figure.tape.v1"
CHANNELS = ["root_t3", "local_t3", "local_r4", "local_s3"]


# --------------------------------------------------------------------------- small matrix algebra

def ident():
    return [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]


def mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def from_trs(t, q, s):
    """T * R * S.  q = (x, y, z, w), normalised here (a zero quaternion is rejected)."""
    x, y, z, w = q
    n = math.sqrt(x * x + y * y + z * z + w * w)
    if not n > 1e-12:
        raise ValueError("zero quaternion in a tape frame")
    x, y, z, w = x / n, y / n, z / n, w / n
    r = [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
         [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
         [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]
    m = ident()
    for i in range(3):
        for j in range(3):
            m[i][j] = r[i][j] * s[j]
        m[i][3] = t[i]
    return m


def translation(t):
    m = ident()
    m[0][3], m[1][3], m[2][3] = t
    return m


def inv_affine(m):
    """Inverse of an affine matrix with an invertible 3x3 block (cofactor form)."""
    a = [[m[i][j] for j in range(3)] for i in range(3)]
    c = [[0.0] * 3 for _ in range(3)]
    for i in range(3):
        for j in range(3):
            r = [k for k in range(3) if k != i]
            q = [k for k in range(3) if k != j]
            c[i][j] = ((-1) ** (i + j)) * (a[r[0]][q[0]] * a[r[1]][q[1]] - a[r[0]][q[1]] * a[r[1]][q[0]])
    det = sum(a[0][j] * c[0][j] for j in range(3))
    if abs(det) < 1e-14:
        raise ValueError("singular joint matrix")
    ai = [[c[j][i] / det for j in range(3)] for i in range(3)]
    t = [m[i][3] for i in range(3)]
    out = ident()
    for i in range(3):
        for j in range(3):
            out[i][j] = ai[i][j]
        out[i][3] = -sum(ai[i][j] * t[j] for j in range(3))
    return out


def apply(m, p):
    return (m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
            m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3])


def quat_axis_angle(axis, ang):
    n = math.sqrt(sum(a * a for a in axis))
    s = math.sin(ang / 2.0) / n
    return (axis[0] * s, axis[1] * s, axis[2] * s, math.cos(ang / 2.0))


# --------------------------------------------------------------------------- tape files

class Tape:
    def __init__(self, header, floats, path):
        self.header = header
        self.floats = floats
        self.path = path
        self.n_joints = len(header["joints"])
        self.n_face = len(header.get("face", []))
        self.stride = 3 + 10 * self.n_joints + self.n_face
        self.frame_count = int(header["frame_count"])
        self.fps = float(header["fps"])

    def frame(self, f):
        """-> (root_t3, [(t3, q_xyzw, s3) per joint], face list).  f is clamped to the tape."""
        f = max(0, min(self.frame_count - 1, int(f)))
        o = f * self.stride
        a = self.floats
        root = tuple(a[o:o + 3])
        joints = []
        p = o + 3
        for _ in range(self.n_joints):
            joints.append((tuple(a[p:p + 3]), tuple(a[p + 3:p + 7]), tuple(a[p + 7:p + 10])))
            p += 10
        return root, joints, list(a[p:p + self.n_face])

    def camera_id_at(self, f):
        cid = None
        for cut in sorted(self.header.get("camera_cuts", []), key=lambda c: c["frame"]):
            if cut["frame"] <= f:
                cid = cut["camera_id"]
        return cid


def read_tape(json_path, verify_sha=True):
    if not json_path.endswith(".tape.json"):
        raise ValueError("tape header path must end in .tape.json: %s" % os.path.basename(json_path))
    with open(json_path, "r", encoding="utf-8") as f:
        header = json.load(f)
    if header.get("format") != FORMAT:
        raise ValueError("tape format is %r, expected %r" % (header.get("format"), FORMAT))
    if header.get("channels") != CHANNELS:
        raise ValueError("tape channels %r != %r" % (header.get("channels"), CHANNELS))
    bin_path = json_path[:-len(".tape.json")] + ".tape.bin"
    with open(bin_path, "rb") as f:
        raw = f.read()
    if verify_sha and header.get("bin_sha256"):
        got = hashlib.sha256(raw).hexdigest()
        if got != header["bin_sha256"]:
            raise ValueError("tape.bin sha256 %s does not match header bin_sha256 %s" % (got, header["bin_sha256"]))
    a = array.array("f")
    if len(raw) % 4:
        raise ValueError("tape.bin size %d is not a multiple of 4" % len(raw))
    a.frombytes(raw)
    if sys.byteorder == "big":
        a.byteswap()
    t = Tape(header, a, json_path)
    want = t.stride * t.frame_count
    if len(a) != want:
        raise ValueError("tape.bin has %d floats, header implies %d (frames=%d stride=%d)" % (len(a), want, t.frame_count, t.stride))
    for v in a:
        if v != v or v in (float("inf"), float("-inf")):
            raise ValueError("non-finite value in tape.bin")
    return t


def write_tape(prefix, header, frames):
    """frames: list of (root_t3, [(t3, q, s3)...], face_list).  header must carry joints/face/fps/etc.
    Writes prefix.tape.bin then prefix.tape.json (with frame_count, channels, format, bin_sha256 set)."""
    a = array.array("f")
    for root, joints, face in frames:
        a.extend(root)
        for t, q, s in joints:
            a.extend(t)
            a.extend(q)
            a.extend(s)
        a.extend(face)
    if sys.byteorder == "big":
        a.byteswap()
    raw = a.tobytes()
    h = dict(header)
    h["format"] = FORMAT
    h["channels"] = list(CHANNELS)
    h["frame_count"] = len(frames)
    h["bin_sha256"] = hashlib.sha256(raw).hexdigest()
    with open(prefix + ".tape.bin", "wb") as f:
        f.write(raw)
    with open(prefix + ".tape.json", "w", encoding="utf-8") as f:
        json.dump(h, f, indent=1, sort_keys=True)
        f.write("\n")
    return h


# --------------------------------------------------------------------------- export access

def read_blob(export_dir, rel, dtype):
    code = {"f32": "f", "u32": "I", "u16": "H"}[dtype]
    a = array.array(code)
    with open(os.path.join(export_dir, rel), "rb") as f:
        a.frombytes(f.read())
    if sys.byteorder == "big":
        a.byteswap()
    return a


def load_json(path):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def shape_positions(export_dir, controls):
    """Neutral positions + sum |c| * delta[side], full index space, lifted so min body z = 0 (the runtime's re-grounding).

    -> (S list of (x,y,z) [19158], ground_shift_z, warnings).  Values outside [-1,1] are applied unclamped (studio API section 1)
    and reported in `warnings`; an unknown control name is an error."""
    hdr = load_json(os.path.join(export_dir, "figure_asset.json"))
    base = read_blob(export_dir, hdr["blobs"]["base.positions"]["file"], "f32")
    n = len(base) // 3
    S = [[base[3 * i], base[3 * i + 1], base[3 * i + 2]] for i in range(n)]
    ce = load_json(os.path.join(export_dir, "controls_export.json"))
    by = {c["name"]: c for c in ce["controls"]}
    warns = []
    for name, val in controls.items():
        if name not in by:
            raise ValueError("control %r is not in the export (known: %s)" % (name, sorted(by)))
        if not -1.0 <= val <= 1.0:
            warns.append("ControlOutOfDomain %s=%s (applied unclamped)" % (name, val))
        if val == 0.0:
            continue
        side = by[name]["pos" if val > 0 else "neg"]
        idx = read_blob(export_dir, side["idx"], "u32")
        xyz = read_blob(export_dir, side["xyz"], "f32")
        a = abs(val)
        for k, i in enumerate(idx):
            S[i][0] += a * xyz[3 * k]
            S[i][1] += a * xyz[3 * k + 1]
            S[i][2] += a * xyz[3 * k + 2]
    shift = -min(S[i][2] for i in range(fc.N_BODY))
    S = [(p[0], p[1], p[2] + shift) for p in S]
    return S, shift, warns


def bind_frames(skel, S, mode="world_aligned", export_dir=None):
    """Bind world matrices per bone (skeleton.json order) from the SHAPED positions S.
    mode world_aligned: identity rotation; mode bone: the exported rest_matrix rotation."""
    if mode not in ("world_aligned", "bone"):
        raise ValueError("joint frame mode %r" % mode)
    rest = None
    if mode == "bone":
        raw = read_blob(export_dir, skel["rest_matrix_blob"], "f32")
        rest = [[[raw[16 * i + 4 * r + c] for c in range(4)] for r in range(4)] for i in range(len(skel["bones"]))]
    out = []
    heads = []
    for i, b in enumerate(skel["bones"]):
        ep = dict(b["head_binding"])
        ep["default"] = b["head_default"]
        h = fc.derive_endpoint(ep, S)
        if h is None:
            h = tuple(b["head"])
        heads.append(h)
        m = ident() if rest is None else [row[:] for row in rest[i]]
        m[0][3], m[1][3], m[2][3] = h
        out.append(m)
    return out, heads


def skin_matrices(skel, wbind, tape, f):
    """Per-bone skin matrices S[i] = W[i] * inv(Wbind[i]) for tape frame f.  A bone missing from the tape keeps its
    rest local transform (it rides its parent).  -> list of 4x4 in skeleton.json order."""
    joints = tape.header["joints"]
    names = [b["name"] for b in skel["bones"]]
    pos = {n: i for i, n in enumerate(names)}
    for j in joints:
        if j not in pos:
            raise ValueError("tape joint %r is not in the exported rig" % j)
    root, jl, _face = tape.frame(f)
    local = {}
    for jn, (t, q, s) in zip(joints, jl):
        local[pos[jn]] = from_trs(t, q, s)
    W = [None] * len(names)
    S = [None] * len(names)
    for i, b in enumerate(skel["bones"]):
        p = b["parent"]
        if i in local:
            L = local[i]
        elif p < 0:
            L = wbind[i]
        else:
            L = mul(inv_affine(wbind[p]), wbind[i])
        if p < 0:
            W[i] = mul(translation(root), L)
        else:
            W[i] = mul(W[p], L)
        S[i] = mul(W[i], inv_affine(wbind[i]))
    return S


def lbs_positions(P, idx4, w4, S):
    """Reference linear blend skinning of positions (pure python), idx4/w4 per vertex (top-4)."""
    out = []
    for p, ii, ww in zip(P, idx4, w4):
        x = y = z = 0.0
        for b, w in zip(ii, ww):
            if w == 0.0:
                continue
            q = apply(S[b], p)
            x += w * q[0]
            y += w * q[1]
            z += w * q[2]
        out.append((x, y, z))
    return out


# --------------------------------------------------------------------------- selftest

def _selftest():
    fails = []

    def check(ok, msg):
        print(("PASS " if ok else "FAIL ") + msg)
        if not ok:
            fails.append(msg)

    # matrix algebra
    q = quat_axis_angle((0, 0, 1), math.pi / 2)
    m = from_trs((1, 2, 3), q, (1, 1, 1))
    p = apply(m, (1, 0, 0))
    check(abs(p[0] - 1) < 1e-12 and abs(p[1] - 3) < 1e-12 and abs(p[2] - 3) < 1e-12, "from_trs rotates +x to +y then translates")
    mi = mul(m, inv_affine(m))
    check(all(abs(mi[i][j] - (1.0 if i == j else 0.0)) < 1e-12 for i in range(4) for j in range(4)), "inv_affine is the inverse (with scale)")
    m2 = from_trs((0.5, -1, 2), quat_axis_angle((1, 2, 3), 0.7), (1.1, 0.9, 1.3))
    mi = mul(m2, inv_affine(m2))
    check(all(abs(mi[i][j] - (1.0 if i == j else 0.0)) < 1e-12 for i in range(4) for j in range(4)), "inv_affine on a general TRS")
    # kinematics: 3-joint chain along +z, bind world positions (0,0,0), (0,0,1), (0,0,2)
    skel = {"bones": [{"name": "a", "parent": -1, "head_binding": {"mode": "mean", "indices": [0]}, "head_default": None, "head": [0, 0, 0]},
                      {"name": "b", "parent": 0, "head_binding": {"mode": "mean", "indices": [1]}, "head_default": None, "head": [0, 0, 1]},
                      {"name": "c", "parent": 1, "head_binding": {"mode": "mean", "indices": [2]}, "head_default": None, "head": [0, 0, 2]}]}
    Sx = [(0, 0, 0), (0, 0, 1), (0, 0, 2)]
    wb, heads = bind_frames(skel, Sx)
    check(heads == [(0, 0, 0), (0, 0, 1), (0, 0, 2)], "bind heads re-derived from positions")
    hdr = {"joints": ["a", "b", "c"], "face": [], "fps": 30, "frame_count": 2}
    rest_local = [((0, 0, 0), (0, 0, 0, 1), (1, 1, 1)), ((0, 0, 1), (0, 0, 0, 1), (1, 1, 1)), ((0, 0, 1), (0, 0, 0, 1), (1, 1, 1))]
    bent = [rest_local[0], ((0, 0, 1), quat_axis_angle((0, 1, 0), math.pi / 2), (1, 1, 1)), rest_local[2]]
    prefix = os.path.join(os.environ.get("TMPDIR", "/tmp"), "fa_tape_selftest_%d" % os.getpid())
    write_tape(prefix, hdr, [((0, 0, 0), rest_local, []), ((0.25, 0, 0), bent, [])])
    t = read_tape(prefix + ".tape.json")
    for ext in (".tape.json", ".tape.bin"):
        os.remove(prefix + ext)
    S0 = skin_matrices(skel, wb, t, 0)
    ok = all(abs(S0[i][r][c] - (1.0 if r == c else 0.0)) < 1e-6 for i in range(3) for r in range(4) for c in range(4))
    check(ok, "rest tape frame gives identity skin matrices")
    S1 = skin_matrices(skel, wb, t, 1)
    # bone b rotates +90 deg about +y: the point at c (0,0,2) goes to (+1,0,1) then root offset x+0.25
    pc = apply(S1[2], (0, 0, 2))
    check(abs(pc[0] - 1.25) < 1e-6 and abs(pc[1]) < 1e-6 and abs(pc[2] - 1.0) < 1e-6, "child joint follows the bent parent plus root_t3: %s" % (tuple(round(v, 6) for v in pc),))
    pa = apply(S1[0], (0, 0, 0.5))
    check(abs(pa[0] - 0.25) < 1e-6 and abs(pa[2] - 0.5) < 1e-6, "root joint only translates by root_t3")
    # tampering is caught
    write_tape(prefix, hdr, [((0, 0, 0), rest_local, [])])
    with open(prefix + ".tape.bin", "r+b") as f:
        f.seek(0)
        f.write(b"\x00\x00\x80\x3f")
    try:
        read_tape(prefix + ".tape.json")
        check(False, "tampered tape.bin must be rejected")
    except ValueError as exc:
        check("sha256" in str(exc), "tampered tape.bin rejected (%s)" % str(exc)[:40])
    for ext in (".tape.json", ".tape.bin"):
        os.remove(prefix + ext)
    print("SELFTEST_FAILS %d" % len(fails))
    return 1 if fails else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(_selftest())
    print(__doc__)
