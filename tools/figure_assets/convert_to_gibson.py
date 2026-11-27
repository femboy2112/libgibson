#!/usr/bin/env python3
"""Convert the M5-C body export (figure-asset/1) into the libgibson rev.v1 asset input format.

Pure python, standard library only. Run with `python3 -I` (isolated mode: the script directory is
NOT on sys.path, so this file is self-contained on purpose).

    python3 -I tools/figure_assets/convert_to_gibson.py convert --export EXPORT_DIR --out OUT_DIR \
            [--asset-id ID]
    python3 -I tools/figure_assets/convert_to_gibson.py fixture --out DIR
    python3 -I tools/figure_assets/convert_to_gibson.py --selftest

`convert` reads the export that `export_mpfb.py` wrote (manifest.json, figure_asset.json,
skeleton.json, landmarks.json, controls_export.json, attachments/*.json, blobs/*.bin) and writes
OUT_DIR/{index.txt, body.gfa, part.<id>.gfa}. The output is a pure function of the input bytes (no
timestamps, no paths, fixed ordering), so two runs give identical sha256s. The sha256 of every
output file is printed as `SHA256 <hex>  <file>`.

`fixture` writes a tiny SYNTHETIC body (no MakeHuman bytes, self-authored geometry) for the Rust
tests of `figure::studio`; the committed copy lives in tests/fixtures/figure_studio/.

The format is `docs/FIGURE_CONTRACT.md`, section "rev.v1 asset input format
(gibson.figure.asset.v1)". Summary (little-endian; `str` = u32 byte length + UTF-8; `f32` = IEEE
binary32 bits):

body.gfa
    prefix `gibson.figure.asset.body.v1\\0`; str asset_id; u32 version; u8 grounded;
    u32 n_vertices (full index space, helpers included); u32 n_body (indices 0..n_body are the
    body, the rest are helper vertices); n_vertices x (f32 x3) neutral positions;
    u32 n_indices, n_indices x u32 (triangles, body vertices only);
    u32 n_joints; per joint: str name, i32 parent, f32 x3 declared rest translation (parent frame),
    f32 x4 rest rotation (xyzw, parent frame), f32 x3 rest scale, u32 n_anchor, n_anchor x u32
    (anchor group: the joint is the mean of these SHAPED vertices);
    n_body x skin: u8 count (1..4), count x (u32 joint, f32 weight), normalised, descending;
    u32 n_controls; per control: str name, f32 min, f32 max, f32 default, u32 n_neg,
    n_neg x (u32 vertex, f32 x3), u32 n_pos, n_pos x (u32 vertex, f32 x3);
    u32 n_landmarks; per landmark: str name, u32 count, count x u32 vertex index.

part.<id>.gfa  (a garment, hair, eyes, brows, ... bound to the body)
    prefix `gibson.figure.asset.part.v1\\0`; str id; str kind; u32 n_vertices;
    n_vertices x (f32 x3) neutral fit (reference only); u32 n_indices, n_indices x u32;
    n_vertices x binding: u32 v0, v1, v2, f32 w0, w1, w2, f32 ox, oy, oz;
    u32 n_spec (0 or 3); per spec: u8 kind (0 const: f32 value | 1 pair: u32 a, u32 b, u8 axis,
    f32 ref);
    n_vertices x skin (as the body's); u32 n_hidden, n_hidden x u32 body vertex.
    Fit rule (MakeHuman .mhclo, as exported): position = w0 P[v0] + w1 P[v1] + w2 P[v2]
    + (ox sx, oy sy, oz sz) with s_k = value (const) or |P[a][axis] - P[b][axis]| / ref (pair), P the
    SHAPED vertex positions in the full index space.
"""

import argparse
import array
import hashlib
import json
import math
import os
import struct
import sys

BODY_MAGIC = b"gibson.figure.asset.body.v1\0"
PART_MAGIC = b"gibson.figure.asset.part.v1\0"
INDEX_FORMAT = "gibson.figure.asset.v1"
MAX_INFLUENCES = 4


class Writer:
    """Minimal deterministic little-endian writer."""

    def __init__(self):
        self.b = bytearray()

    def u8(self, v):
        self.b += struct.pack("<B", v)

    def u32(self, v):
        if not (0 <= v < 2 ** 32):
            raise ValueError("u32 out of range: %r" % (v,))
        self.b += struct.pack("<I", v)

    def i32(self, v):
        self.b += struct.pack("<i", v)

    def f32(self, v):
        self.b += struct.pack("<f", v)

    def str(self, s):
        e = s.encode("utf-8")
        self.u32(len(e))
        self.b += e

    def vec3(self, v):
        for c in v:
            self.f32(c)

    def raw(self, data):
        self.b += data

    def bytes(self):
        return bytes(self.b)


def write_skin(w, influences):
    if not (1 <= len(influences) <= MAX_INFLUENCES):
        raise ValueError("skin influence count %d" % len(influences))
    w.u8(len(influences))
    for j, wt in influences:
        w.u32(j)
        w.f32(wt)


def encode_body(m):
    """Encode the body model dict (see module doc) into body.gfa bytes."""
    n_v = len(m["positions"])
    n_body = m["n_body"]
    if not (0 < n_body <= n_v):
        raise ValueError("n_body out of range")
    w = Writer()
    w.raw(BODY_MAGIC)
    w.str(m["id"])
    w.u32(m["version"])
    w.u8(1 if m["grounded"] else 0)
    w.u32(n_v)
    w.u32(n_body)
    for p in m["positions"]:
        w.vec3(p)
    w.u32(len(m["indices"]))
    for i in m["indices"]:
        if not (0 <= i < n_body):
            raise ValueError("triangle index %d outside the body" % i)
        w.u32(i)
    w.u32(len(m["joints"]))
    for j in m["joints"]:
        w.str(j["name"])
        w.i32(j["parent"])
        w.vec3(j["t"])
        for c in j["q"]:
            w.f32(c)
        w.vec3(j["s"])
        w.u32(len(j["anchor"]))
        for a in j["anchor"]:
            if not (0 <= a < n_v):
                raise ValueError("anchor vertex out of range")
            w.u32(a)
    if len(m["skin"]) != n_body:
        raise ValueError("skin must cover exactly the body vertices")
    for infl in m["skin"]:
        write_skin(w, infl)
    w.u32(len(m["controls"]))
    for c in m["controls"]:
        w.str(c["name"])
        w.f32(c["min"])
        w.f32(c["max"])
        w.f32(c["default"])
        for side in ("neg", "pos"):
            w.u32(len(c[side]))
            for v, d in c[side]:
                if not (0 <= v < n_v):
                    raise ValueError("delta vertex out of range")
                w.u32(v)
                w.vec3(d)
    w.u32(len(m["landmarks"]))
    for name, idx in m["landmarks"]:
        w.str(name)
        w.u32(len(idx))
        for i in idx:
            w.u32(i)
    return w.bytes()


def encode_part(m):
    n = len(m["positions"])
    w = Writer()
    w.raw(PART_MAGIC)
    w.str(m["id"])
    w.str(m["kind"])
    w.u32(n)
    for p in m["positions"]:
        w.vec3(p)
    w.u32(len(m["indices"]))
    for i in m["indices"]:
        if not (0 <= i < n):
            raise ValueError("part triangle index out of range")
        w.u32(i)
    if len(m["bind"]) != n:
        raise ValueError("one binding per part vertex")
    for (a, b, c, wa, wb, wc, ox, oy, oz) in m["bind"]:
        w.u32(a)
        w.u32(b)
        w.u32(c)
        w.f32(wa)
        w.f32(wb)
        w.f32(wc)
        w.f32(ox)
        w.f32(oy)
        w.f32(oz)
    spec = m["spec"]
    if len(spec) not in (0, 3):
        raise ValueError("offset spec must have 0 or 3 entries")
    w.u32(len(spec))
    for s in spec:
        if s[0] == "const":
            w.u8(0)
            w.f32(s[1])
        else:
            w.u8(1)
            w.u32(s[1])
            w.u32(s[2])
            w.u8(s[3])
            w.f32(s[4])
    if len(m["skin"]) != n:
        raise ValueError("one skin entry per part vertex")
    for infl in m["skin"]:
        write_skin(w, infl)
    w.u32(len(m["hidden"]))
    for h in m["hidden"]:
        w.u32(h)
    return w.bytes()


def sha256_bytes(b):
    return hashlib.sha256(b).hexdigest()


def write_set(out, body_bytes, parts, source_sha=None):
    """Write index.txt, body.gfa and part files; return [(name, sha)] in a fixed order."""
    os.makedirs(out, exist_ok=True)
    written = []
    lines = ["format " + INDEX_FORMAT]
    if source_sha:
        lines.append("source_manifest_sha256 " + source_sha)
    with open(os.path.join(out, "body.gfa"), "wb") as f:
        f.write(body_bytes)
    written.append(("body.gfa", sha256_bytes(body_bytes)))
    lines.append("body body.gfa " + sha256_bytes(body_bytes))
    for pid, kind, data in sorted(parts, key=lambda t: t[0]):
        fn = "part.%s.gfa" % pid
        with open(os.path.join(out, fn), "wb") as f:
            f.write(data)
        written.append((fn, sha256_bytes(data)))
        lines.append("part %s %s %s %s" % (pid, kind, fn, sha256_bytes(data)))
    idx = ("\n".join(lines) + "\n").encode("utf-8")
    with open(os.path.join(out, "index.txt"), "wb") as f:
        f.write(idx)
    written.insert(0, ("index.txt", sha256_bytes(idx)))
    return written


# --------------------------------------------------------------------------- real export

def read_blob(root, rel, code, count):
    a = array.array(code)
    with open(os.path.join(root, rel), "rb") as f:
        raw = f.read()
    if len(raw) != count * a.itemsize:
        raise ValueError("%s: %d bytes, expected %d" % (rel, len(raw), count * a.itemsize))
    a.frombytes(raw)
    if sys.byteorder != "little":
        a.byteswap()
    return a


def jload(root, rel):
    with open(os.path.join(root, rel), "r", encoding="utf-8") as f:
        return json.load(f)


def quat_from_rot(r):
    """Unit quaternion (x, y, z, w) of a 3x3 rotation matrix r[row][col], w >= 0."""
    tr = r[0][0] + r[1][1] + r[2][2]
    if tr > 0.0:
        s = math.sqrt(tr + 1.0) * 2.0
        w = 0.25 * s
        x = (r[2][1] - r[1][2]) / s
        y = (r[0][2] - r[2][0]) / s
        z = (r[1][0] - r[0][1]) / s
    elif r[0][0] > r[1][1] and r[0][0] > r[2][2]:
        s = math.sqrt(1.0 + r[0][0] - r[1][1] - r[2][2]) * 2.0
        w = (r[2][1] - r[1][2]) / s
        x = 0.25 * s
        y = (r[0][1] + r[1][0]) / s
        z = (r[0][2] + r[2][0]) / s
    elif r[1][1] > r[2][2]:
        s = math.sqrt(1.0 + r[1][1] - r[0][0] - r[2][2]) * 2.0
        w = (r[0][2] - r[2][0]) / s
        x = (r[0][1] + r[1][0]) / s
        y = 0.25 * s
        z = (r[1][2] + r[2][1]) / s
    else:
        s = math.sqrt(1.0 + r[2][2] - r[0][0] - r[1][1]) * 2.0
        w = (r[1][0] - r[0][1]) / s
        x = (r[0][2] + r[2][0]) / s
        y = (r[1][2] + r[2][1]) / s
        z = 0.25 * s
    n = math.sqrt(x * x + y * y + z * z + w * w)
    x, y, z, w = x / n, y / n, z / n, w / n
    if w < 0.0:
        x, y, z, w = -x, -y, -z, -w
    return (x, y, z, w)


def quat_mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def quat_conj(a):
    return (-a[0], -a[1], -a[2], a[3])


def canon_q(q):
    return q if q[3] >= 0.0 else (-q[0], -q[1], -q[2], -q[3])


def mat_t_vec(r, v):
    """r^T v for a 3x3 r[row][col]."""
    return tuple(r[0][i] * v[0] + r[1][i] * v[1] + r[2][i] * v[2] for i in range(3))


def skin_from_top4(idx, wt, n):
    out = []
    for v in range(n):
        pairs = [(int(idx[v * 4 + k]), float(wt[v * 4 + k])) for k in range(4) if wt[v * 4 + k] > 0.0]
        if not pairs:
            raise ValueError("body vertex %d has no positive skin weight" % v)
        pairs.sort(key=lambda p: -p[1])  # stable: ties keep slot order
        seen = set()
        for j, _ in pairs:
            if j in seen:
                raise ValueError("body vertex %d repeats joint %d" % (v, j))
            seen.add(j)
        out.append(pairs)
    return out


def triples(a, n):
    return [(a[3 * i], a[3 * i + 1], a[3 * i + 2]) for i in range(n)]


def convert_export(export, asset_id):
    fa = jload(export, "figure_asset.json")
    if fa.get("format") != "figure-asset/1":
        raise ValueError("unexpected export format %r" % (fa.get("format"),))
    counts = fa["counts"]
    n_v = counts["n_vertices_all"]
    n_body = counts["n_vertices_body"]
    blobs = fa["blobs"]
    pos = read_blob(export, blobs["base.positions"]["file"], "f", n_v * 3)
    positions = triples(pos, n_v)
    tris = read_blob(export, blobs["base.tris"]["file"], "I", counts["n_tris_body"] * 3)
    indices = [int(i) for i in tris]

    sk = jload(export, "skeleton.json")
    bones = sk["bones"]
    rest = read_blob(export, sk["rest_matrix_blob"], "f", len(bones) * 16)
    world_r, world_h, world_q = [], [], []
    for b in range(len(bones)):
        m = [[rest[b * 16 + r * 4 + c] for c in range(4)] for r in range(4)]
        world_r.append([[m[r][c] for c in range(3)] for r in range(3)])
        world_h.append(tuple(bones[b]["head"]))
        # the rest matrix translation is the head; it must agree with the head field
        for a in range(3):
            if abs(m[a][3] - bones[b]["head"][a]) > 1e-5:
                raise ValueError("bone %s: rest matrix translation != head" % bones[b]["name"])
        world_q.append(quat_from_rot(world_r[-1]))
    joints = []
    for b, bone in enumerate(bones):
        p = bone["parent"]
        if p >= b:
            raise ValueError("bones are not parent-first")
        if p < 0:
            t = world_h[b]
            q = world_q[b]
        else:
            d = tuple(world_h[b][a] - world_h[p][a] for a in range(3))
            t = mat_t_vec(world_r[p], d)
            q = canon_q(quat_mul(quat_conj(world_q[p]), world_q[b]))
            n = math.sqrt(sum(c * c for c in q))
            q = tuple(c / n for c in q)
        hb = bone["head_binding"]
        if hb.get("mode") != "mean":
            raise ValueError("bone %s head binding is not a mean" % bone["name"])
        joints.append({
            "name": bone["name"], "parent": p, "t": t, "q": q, "s": (1.0, 1.0, 1.0),
            "anchor": [int(i) for i in hb["indices"]],
        })

    sidx = read_blob(export, blobs["skin.idx"]["file"], "H", n_body * 4)
    swt = read_blob(export, blobs["skin.w"]["file"], "f", n_body * 4)
    skin = skin_from_top4(sidx, swt, n_body)

    cx = jload(export, "controls_export.json")
    lo, hi = cx["value_range"]
    controls = []
    for c in cx["controls"]:
        entry = {"name": c["name"], "min": float(lo), "max": float(hi), "default": 0.0}
        for side in ("neg", "pos"):
            cnt = c[side]["count"]
            idx = read_blob(export, c[side]["idx"], "I", cnt)
            xyz = read_blob(export, c[side]["xyz"], "f", cnt * 3)
            if list(idx) != sorted(set(idx)):
                raise ValueError("control %s %s: indices not sorted and unique" % (c["name"], side))
            entry[side] = [(int(idx[k]), (xyz[3 * k], xyz[3 * k + 1], xyz[3 * k + 2])) for k in range(cnt)]
        controls.append(entry)

    lm = jload(export, "landmarks.json")
    landmarks = []
    for name in sorted(lm["face"]):
        landmarks.append(("face." + name, [int(i) for i in lm["face"][name]]))
    for name in sorted(lm["joints"]):
        landmarks.append((name, [int(i) for i in lm["joints"][name]]))
    for name, idx in landmarks:
        if len(set(idx)) != len(idx):
            raise ValueError("landmark %s repeats a vertex" % name)

    body = {
        "id": asset_id, "version": 1, "grounded": True, "positions": positions, "n_body": n_body,
        "indices": indices, "joints": joints, "skin": skin, "controls": controls, "landmarks": landmarks,
    }

    parts = []
    for rel in sorted(fa["files"]["attachments"]):
        a = jload(export, rel)
        n = a["n_vertices"]
        bl = a["blobs"]
        bidx = read_blob(export, bl["bind_idx"], "I", n * 3)
        bw = read_blob(export, bl["bind_w"], "f", n * 3)
        bo = read_blob(export, bl["bind_off"], "f", n * 3)
        ppos = read_blob(export, bl["positions"], "f", n * 3)
        ptri = read_blob(export, bl["tris"], "I", a["n_tris"] * 3)
        pskin_i = read_blob(export, bl["skin_idx"], "H", n * 4)
        pskin_w = read_blob(export, bl["skin_w"], "f", n * 4)
        with open(os.path.join(export, bl["delete_verts"]), "rb") as f:
            hid = array.array("I")
            hid.frombytes(f.read())
        bind = []
        for v in range(n):
            b3 = [int(bidx[3 * v + k]) for k in range(3)]
            w3 = [bw[3 * v + k] for k in range(3)]
            o3 = [bo[3 * v + k] for k in range(3)]
            bind.append((b3[0], b3[1], b3[2], w3[0], w3[1], w3[2], o3[0], o3[1], o3[2]))
        spec = []
        for s in a["offset_spec"]:
            if "const" in s:
                spec.append(("const", float(s["const"])))
            else:
                spec.append(("pair", int(s["a"]), int(s["b"]), int(s["axis"]), float(s["ref"])))
        part = {
            "id": a["name"], "kind": a["kind"], "positions": triples(ppos, n),
            "indices": [int(i) for i in ptri], "bind": bind, "spec": spec,
            "skin": skin_from_top4(pskin_i, pskin_w, n), "hidden": [int(i) for i in hid],
        }
        parts.append((a["name"], a["kind"], encode_part(part)))
    return encode_body(body), parts


def cmd_convert(args):
    export = args.export
    man = os.path.join(export, "manifest.json")
    with open(man, "rb") as f:
        source_sha = sha256_bytes(f.read())
    body_bytes, parts = convert_export(export, args.asset_id)
    written = write_set(args.out, body_bytes, parts, source_sha)
    for name, sha in written:
        print("SHA256 %s  %s" % (sha, name))
    print("CONVERT_OK files=%d source_manifest_sha256=%s" % (len(written), source_sha))
    return 0


# --------------------------------------------------------------------------- synthetic fixture

def _rot_x(deg):
    a = math.radians(deg)
    return [[1.0, 0.0, 0.0], [0.0, math.cos(a), -math.sin(a)], [0.0, math.sin(a), math.cos(a)]]


def _rot_y(deg):
    a = math.radians(deg)
    return [[math.cos(a), 0.0, math.sin(a)], [0.0, 1.0, 0.0], [-math.sin(a), 0.0, math.cos(a)]]


def _eye3():
    return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]


RING_Z = [0.0, 0.5, 1.0, 1.5]
RING_R = [0.15, 0.20, 0.16, 0.10]
SEG = 8
SHELL_RADIUS = 0.172


def synthetic_fixture():
    """A tiny synthetic body and four parts. Every number is a hand-designed round value so the
    tests can state the expected geometry analytically (tests/figure_studio.rs).

    Body: 4 rings x 8 vertices (indices 0..31, ring a at z = RING_Z[a], radius RING_R[a]), then 5 joint
    helper groups of 4 vertices (32..51) and 8 FIXED helper vertices (52..59, moved by no control).
    Joints: root (0,0,0), pelvis (0,0,0.5), spine (0,0,1.0), neck (0,0,1.4), arm_l (0.25,0,1.25);
    rest world orientations: spine 20 deg about x, arm_l 90 deg about y, others identity.
    Controls (value c in [-1, 1], side by sign, weight |c|):
      stature   dz = +0.25 z (pos) / -0.20 z (neg)           on every non-fixed vertex
      hip_breadth (dx, dy) = +0.5 (x, y) / -0.5 (x, y)       on non-fixed vertices with |z - 0.5| <= 0.25
      bulge     (dx, dy) = +0.2 (x, y) / -0.1 (x, y)         on ring 2 (indices 16..23)
      lean      dy = +0.1 z / -0.1 z                          on every non-fixed vertex
      sink      dz = -0.05 (pos) / +0.05 (neg)                on every non-fixed vertex
    Parts: tunic (clothes, bound to ring 2 + ring 1, offset 0.05 outward, const scales), shell
    (clothes, bound to the 8 fixed helpers at radius 0.172 on z = 1.0: it does NOT follow the
    body, so a bulge pushes the body through it), tabard (clothes, bound 1:1 to ring 1, offset
    0.05 outward scaled by the hip breadth), cap (hair, bound to ring 3, offset +0.03 up scaled by
    the stature).
    """
    positions = []
    for a in range(4):
        for k in range(SEG):
            th = 2.0 * math.pi * k / SEG
            positions.append((RING_R[a] * math.cos(th), RING_R[a] * math.sin(th), RING_Z[a]))
    indices = []
    for a in range(3):
        for k in range(SEG):
            v00 = a * SEG + k
            v01 = a * SEG + (k + 1) % SEG
            v10 = (a + 1) * SEG + k
            v11 = (a + 1) * SEG + (k + 1) % SEG
            indices += [v00, v01, v11, v00, v11, v10]

    heads = [(0.0, 0.0, 0.0), (0.0, 0.0, 0.5), (0.0, 0.0, 1.0), (0.0, 0.0, 1.4), (0.25, 0.0, 1.25)]
    names = ["root", "pelvis", "spine", "neck", "arm_l"]
    parents = [-1, 0, 1, 2, 2]
    # parent chain: root <- pelvis <- spine <- (neck, arm_l); spine has world rotation Rx(20)
    world_rot = [_eye3(), _eye3(), _rot_x(20.0), _eye3(), _rot_y(90.0)]
    world_q = [quat_from_rot(r) for r in world_rot]
    anchors = []
    for j in range(5):
        h = heads[j]
        group = []
        for sx, sy in ((1, 1), (1, -1), (-1, 1), (-1, -1)):
            group.append(len(positions))
            positions.append((h[0] + 0.01 * sx, h[1] + 0.01 * sy, h[2]))
        anchors.append(group)
    fixed_first = len(positions)
    for k in range(SEG):
        th = 2.0 * math.pi * k / SEG
        positions.append((SHELL_RADIUS * math.cos(th), SHELL_RADIUS * math.sin(th), 1.0))
    n_v = len(positions)
    fixed = set(range(fixed_first, n_v))

    joints = []
    for j in range(5):
        p = parents[j]
        if p < 0:
            t = heads[j]
            q = canon_q(world_q[j])
        else:
            d = tuple(heads[j][a] - heads[p][a] for a in range(3))
            t = mat_t_vec(world_rot[p], d)
            q = canon_q(quat_mul(quat_conj(world_q[p]), world_q[j]))
        joints.append({"name": names[j], "parent": p, "t": t, "q": q, "s": (1.0, 1.0, 1.0),
                       "anchor": anchors[j]})

    def delta_set(fn):
        out = []
        for v in range(n_v):
            if v in fixed:
                continue
            d = fn(v, positions[v])
            if d is not None:
                out.append((v, d))
        return out

    controls = [
        {"name": "stature", "min": -1.0, "max": 1.0, "default": 0.0,
         "neg": delta_set(lambda v, p: (0.0, 0.0, -0.20 * p[2])),
         "pos": delta_set(lambda v, p: (0.0, 0.0, 0.25 * p[2]))},
        {"name": "hip_breadth", "min": -1.0, "max": 1.0, "default": 0.0,
         "neg": delta_set(lambda v, p: (-0.5 * p[0], -0.5 * p[1], 0.0) if abs(p[2] - 0.5) <= 0.25 else None),
         "pos": delta_set(lambda v, p: (0.5 * p[0], 0.5 * p[1], 0.0) if abs(p[2] - 0.5) <= 0.25 else None)},
        {"name": "bulge", "min": -1.0, "max": 1.0, "default": 0.0,
         "neg": delta_set(lambda v, p: (-0.1 * p[0], -0.1 * p[1], 0.0) if 16 <= v < 24 else None),
         "pos": delta_set(lambda v, p: (0.2 * p[0], 0.2 * p[1], 0.0) if 16 <= v < 24 else None)},
        {"name": "lean", "min": -1.0, "max": 1.0, "default": 0.0,
         "neg": delta_set(lambda v, p: (0.0, -0.1 * p[2], 0.0)),
         "pos": delta_set(lambda v, p: (0.0, 0.1 * p[2], 0.0))},
        {"name": "sink", "min": -1.0, "max": 1.0, "default": 0.0,
         "neg": delta_set(lambda v, p: (0.0, 0.0, 0.05)),
         "pos": delta_set(lambda v, p: (0.0, 0.0, -0.05))},
    ]

    # skin: root 0, pelvis 1, spine 2, neck 3, arm_l 4
    skin = []
    for a in range(4):
        for k in range(SEG):
            if a == 0:
                infl = [(0, 0.5), (1, 0.5)]
            elif a == 1:
                infl = [(1, 0.7), (0, 0.3)]
            elif a == 2:
                infl = [(2, 0.6), (1, 0.2), (4, 0.2)] if k < 3 else [(2, 0.7), (1, 0.3)]
            else:
                infl = [(3, 0.4), (2, 0.3), (4, 0.2), (1, 0.1)] if k % 2 == 0 else [(3, 0.6), (2, 0.4)]
            skin.append(infl)

    landmarks = [
        ("foot", list(range(0, 8))),
        ("head_top", list(range(24, 32))),
        ("hip_l", [8]),
        ("hip_r", [12]),
        ("chest_front", [16 + 6]),
        ("chest_back", [16 + 2]),
    ]
    body = {"id": "synthetic-puppet", "version": 1, "grounded": True, "positions": positions,
            "n_body": 32, "indices": indices, "joints": joints, "skin": skin, "controls": controls,
            "landmarks": landmarks}

    def ring_dir(k, half):
        th = 2.0 * math.pi * (k + (0.5 if half else 0.0)) / SEG
        return (math.cos(th), math.sin(th))

    def transfer(v0):
        return list(skin[v0]) if v0 < 32 else [(0, 1.0)]

    def fit_const(bind, spec):
        out = []
        sx, sy, sz = [s[1] if s[0] == "const" else 1.0 for s in spec]
        for (a, b, c, wa, wb, wc, ox, oy, oz) in bind:
            pa, pb, pc = positions[a], positions[b], positions[c]
            out.append(tuple(wa * pa[i] + wb * pb[i] + wc * pc[i] + o * s
                             for i, (o, s) in enumerate(((ox, sx), (oy, sy), (oz, sz)))))
        return out

    # tunic: 8 vertices on ring 2 mid-edges, bound to ring2[k], ring2[k+1], ring1[k]
    tun_bind = []
    for k in range(SEG):
        dx, dy = ring_dir(k, True)
        tun_bind.append((16 + k, 16 + (k + 1) % SEG, 8 + k, 0.5, 0.5, 0.0, 0.05 * dx, 0.05 * dy, 0.0))
    spec1 = [("const", 1.0), ("const", 1.0), ("const", 1.0)]
    tun_tris = []
    for k in range(1, SEG - 1):
        tun_tris += [0, k, k + 1]
    tunic = {"id": "tunic", "kind": "clothes", "bind": tun_bind, "spec": spec1, "indices": tun_tris,
             "skin": [transfer(b[0]) for b in tun_bind], "hidden": [16, 17, 18, 19, 20, 21, 22, 23]}
    tunic["positions"] = fit_const(tun_bind, spec1)

    # shell: bound 1:1 to the fixed helpers (zero offset, weight 1 on the first slot)
    sh_bind = [(fixed_first + k, fixed_first + k, fixed_first + k, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0)
               for k in range(SEG)]
    shell = {"id": "shell", "kind": "clothes", "bind": sh_bind, "spec": spec1, "indices": tun_tris,
             "skin": [transfer(16 + k) for k in range(SEG)], "hidden": []}
    shell["positions"] = fit_const(sh_bind, spec1)

    # tabard: ring 1 vertex k, offset 0.05 outward (x, y) scaled by the hip breadth ratio
    tab_bind = []
    for k in range(SEG):
        dx, dy = ring_dir(k, False)
        tab_bind.append((8 + k, 8 + k, 8 + k, 1.0, 0.0, 0.0, 0.05 * dx, 0.05 * dy, 0.0))
    # s_x = |P[8].x - P[12].x| / 0.4 (hip breadth ratio), s_y = const 1, s_z = const 1
    spec_hip = [("pair", 8, 12, 0, 0.4), ("const", 1.0), ("const", 1.0)]
    tabard = {"id": "tabard", "kind": "clothes", "bind": tab_bind, "spec": spec_hip, "indices": tun_tris,
              "skin": [transfer(8 + k) for k in range(SEG)], "hidden": []}
    tabard["positions"] = [
        tuple(positions[b[0]][i] + (b[6 + i] if i < 2 else 0.0) for i in range(3)) for b in tab_bind
    ]

    # cap: ring 3 vertex k, offset +0.03 up scaled by the stature ratio |z24 - z0| / 1.5
    cap_bind = [(24 + k, 24 + k, 24 + k, 1.0, 0.0, 0.0, 0.0, 0.0, 0.03) for k in range(SEG)]
    spec_stat = [("const", 1.0), ("const", 1.0), ("pair", 24, 0, 2, 1.5)]
    cap = {"id": "cap", "kind": "hair", "bind": cap_bind, "spec": spec_stat, "indices": tun_tris,
           "skin": [transfer(24 + k) for k in range(SEG)], "hidden": []}
    cap["positions"] = [(positions[b[0]][0], positions[b[0]][1], positions[b[0]][2] + 0.03) for b in cap_bind]

    parts = [(p["id"], p["kind"], encode_part(p)) for p in (tunic, shell, tabard, cap)]
    return encode_body(body), parts


def cmd_fixture(args):
    body_bytes, parts = synthetic_fixture()
    written = write_set(args.out, body_bytes, parts, None)
    for name, sha in written:
        print("SHA256 %s  %s" % (sha, name))
    print("FIXTURE_OK files=%d" % len(written))
    return 0


# --------------------------------------------------------------------------- selftest

def selftest():
    # 1. the fixture is deterministic
    a = synthetic_fixture()
    b = synthetic_fixture()
    assert a[0] == b[0] and [p[2] for p in a[1]] == [p[2] for p in b[1]], "fixture not deterministic"
    # 2. quaternion helpers: Rx(90) maps y to z
    q = quat_from_rot(_rot_x(90.0))
    assert abs(q[0] - math.sin(math.pi / 4)) < 1e-12 and abs(q[3] - math.cos(math.pi / 4)) < 1e-12, q
    qq = quat_mul(quat_conj(q), q)
    assert abs(qq[3] - 1.0) < 1e-12 and abs(qq[0]) < 1e-12, qq
    # 3. the parent-frame translation: chain pelvis -> spine with spine's parent rotated is a pure
    #    rotation of the world offset (R^T d), norm preserved
    d = (0.0, 0.0, 0.5)
    t = mat_t_vec(_rot_x(20.0), d)
    assert abs(math.sqrt(sum(c * c for c in t)) - 0.5) < 1e-12
    # 4. a writer round trip of the integer widths
    w = Writer()
    w.u32(7)
    w.i32(-1)
    w.str("ab")
    assert w.bytes() == b"\x07\x00\x00\x00\xff\xff\xff\xff\x02\x00\x00\x00ab"
    print("SELFTEST_OK")
    return 0


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--selftest", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    c = sub.add_parser("convert")
    c.add_argument("--export", required=True)
    c.add_argument("--out", required=True)
    c.add_argument("--asset-id", default="figure-body")
    f = sub.add_parser("fixture")
    f.add_argument("--out", required=True)
    args = ap.parse_args(argv)
    if args.selftest:
        return selftest()
    if args.cmd == "convert":
        return cmd_convert(args)
    if args.cmd == "fixture":
        return cmd_fixture(args)
    ap.print_help()
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
