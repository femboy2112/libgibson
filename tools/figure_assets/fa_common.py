#!/usr/bin/env python3
"""Pure-python helpers for the M5-C body export (no bpy, no numpy).

Imported by export_mpfb.py inside Blender (which puts this directory on sys.path),
and runnable on its own as a self test:

    python3 -I tools/figure_assets/fa_common.py --selftest \
        --mpfb-data "$MPFB_DATA" --user-data "$MPFB_USER_DATA"

Everything here only READS data files (OBJ, .mhclo, .target, rig/weights/vertex
group JSON).  No MakeHuman or MPFB python is copied; the file formats are parsed
by the code below.

Frame and units of every exported array: the Blender frame, metres,
right-handed, +Z up, the character faces -Y.  MakeHuman data files are in
decimetres with +Y up and +Z toward the viewer, so a MakeHuman (x, y, z) becomes
(x, -z, y) * 0.1 (the same mapping MPFB applies when it loads the base mesh).
"""
import array
import gzip
import hashlib
import heapq
import json
import math
import os
import sys

SCALE = 0.1          # MakeHuman decimetres -> metres
N_BODY = 13380       # base mesh vertices 0..13379 are the body, the rest is helper geometry
N_ALL = 19158


def mh_to_bl(p):
    return (p[0] * SCALE, -p[2] * SCALE, p[1] * SCALE)


# --------------------------------------------------------------------------- file parsers

def parse_obj(path):
    """Return dict(verts=[(x,y,z) raw], uvs=[(u,v)], faces=[(vi_list, ti_list|None, group)])."""
    verts, uvs, faces = [], [], []
    group = ""
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for line in f:
            if line.startswith("v "):
                p = line.split()
                verts.append((float(p[1]), float(p[2]), float(p[3])))
            elif line.startswith("vt "):
                p = line.split()
                uvs.append((float(p[1]), float(p[2])))
            elif line.startswith("g "):
                group = line.split()[1]
            elif line.startswith("f "):
                vi, ti = [], []
                for tok in line.split()[1:]:
                    a = tok.split("/")
                    vi.append(int(a[0]) - 1)
                    ti.append(int(a[1]) - 1 if len(a) > 1 and a[1] else None)
                faces.append((vi, ti if all(t is not None for t in ti) else None, group))
    return {"verts": verts, "uvs": uvs, "faces": faces}


def triangulate(faces, only_group=None):
    """Fan-triangulate. Returns (tri_pos [(a,b,c)], tri_uv [(a,b,c)] or None-filled, src_face_index)."""
    tp, tu, src = [], [], []
    have_uv = True
    for fi, (vi, ti, g) in enumerate(faces):
        if only_group is not None and g != only_group:
            continue
        for k in range(1, len(vi) - 1):
            tp.append((vi[0], vi[k], vi[k + 1]))
            if ti is None:
                have_uv = False
                tu.append((0, 0, 0))
            else:
                tu.append((ti[0], ti[k], ti[k + 1]))
            src.append(fi)
    return tp, (tu if have_uv else None), src


def parse_target(path):
    """Sparse MakeHuman target -> list of (index, dx, dy, dz) in the Blender frame, metres."""
    op = gzip.open if path.endswith(".gz") else open
    out = []
    with op(path, "rt", encoding="utf-8", errors="replace") as f:
        for line in f:
            s = line.strip()
            if not s or s.startswith("#") or s.startswith('"'):
                continue
            p = s.split()
            x, y, z = float(p[1]), float(p[2]), float(p[3])
            out.append((int(p[0]), x * SCALE, -z * SCALE, y * SCALE))
    return out


def parse_mhclo(path):
    """Parse a MakeHuman .mhclo binding file.

    Returns dict(name, obj_file, scales={x_scale:(a,b,ref),...}, z_depth,
                 bind=[(v0,v1,v2,w0,w1,w2,ox,oy,oz)], delete_verts=[...]).
    The offset is converted to the Blender axis order (file d0,d1,d2 -> d0,-d2,d1),
    still in file units.
    """
    info = {"name": None, "obj_file": None, "scales": {}, "z_depth": None, "bind": [], "delete_verts": []}
    status = ""
    seq = False
    v0 = 0
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for line in f:
            words = line.split()
            if not words or words[0].startswith("#"):
                continue
            if words[0] in ("material", "vertexboneweights"):
                continue  # may appear inside the vertex block; not a vertex line
            if status == "v":
                if words[0].isdigit():
                    if len(words) == 1:
                        v = int(words[0])
                        info["bind"].append((v, v, v, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0))
                    else:
                        a = int(words[0]), int(words[1]), int(words[2])
                        w = float(words[3]), float(words[4]), float(words[5])
                        d = float(words[6]), float(words[7]), float(words[8])
                        info["bind"].append((a[0], a[1], a[2], w[0], w[1], w[2], d[0], -d[2], d[1]))
                    continue
                status = ""
            elif status == "d":
                if words[0].isdigit():
                    for tok in words:
                        if tok == "-":
                            seq = True
                        else:
                            v1 = int(tok)
                            if seq:
                                info["delete_verts"].extend(range(v0, v1 + 1))
                                seq = False
                            else:
                                info["delete_verts"].append(v1)
                            v0 = v1
                    continue
                status = ""
            key = words[0]
            if key == "name":
                info["name"] = " ".join(words[1:])
            elif key == "obj_file":
                info["obj_file"] = words[1]
            elif key in ("x_scale", "y_scale", "z_scale"):
                info["scales"][key] = (int(words[1]), int(words[2]), float(words[3]))
            elif key == "z_depth":
                info["z_depth"] = int(words[1])
            elif key == "verts":
                status = "v"
            elif key == "delete_verts":
                status = "d"
                seq = False
    return info


def offset_scale_spec(info):
    """Per Blender axis (x, y, z): how the stored offset is scaled at fit time.

    scale_k = |P[a][axis] - P[b][axis]| / ref   (a, b body vertex indices);
    MakeHuman's file x_scale -> Blender x, z_scale -> Blender y, y_scale -> Blender z.
    """
    sc = info["scales"]
    spec = []
    for key, axis in (("x_scale", 0), ("z_scale", 1), ("y_scale", 2)):
        if key in sc:
            a, b, ref = sc[key]
            spec.append({"a": a, "b": b, "axis": axis, "ref": ref})
        else:
            spec.append({"const": SCALE})
    return spec


def eval_scales(spec, P):
    out = []
    for s in spec:
        if "const" in s:
            out.append(s["const"])
        else:
            out.append(abs(P[s["a"]][s["axis"]] - P[s["b"]][s["axis"]]) / s["ref"])
    return out


def fit_binding(bind, spec, P):
    """Positions of the bound mesh for body positions P (list of (x,y,z))."""
    sx, sy, sz = eval_scales(spec, P)
    out = []
    for (a, b, c, wa, wb, wc, ox, oy, oz) in bind:
        pa, pb, pc = P[a], P[b], P[c]
        out.append((wa * pa[0] + wb * pb[0] + wc * pc[0] + ox * sx,
                    wa * pa[1] + wb * pb[1] + wc * pc[1] + oy * sy,
                    wa * pa[2] + wb * pb[2] + wc * pc[2] + oz * sz))
    return out


def vertex_groups(path):
    """basemesh_vertex_groups.json (inclusive index ranges) -> {name: sorted index list}."""
    with open(path, "r", encoding="utf-8") as f:
        d = json.load(f)
    out = {}
    for name, ranges in d.items():
        idx = []
        for r in ranges:
            if isinstance(r, list) and len(r) == 2:
                idx.extend(range(r[0], r[1] + 1))
            else:
                idx.append(int(r))
        out[name] = sorted(idx)
    return out


# --------------------------------------------------------------------------- geometry helpers

def centroid(P, idx):
    n = float(len(idx))
    return (sum(P[i][0] for i in idx) / n, sum(P[i][1] for i in idx) / n, sum(P[i][2] for i in idx) / n)


def vertex_normals(P, tris):
    n = [[0.0, 0.0, 0.0] for _ in P]
    for a, b, c in tris:
        pa, pb, pc = P[a], P[b], P[c]
        e1 = (pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2])
        e2 = (pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2])
        cx = (e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0])
        for v in (a, b, c):
            n[v][0] += cx[0]
            n[v][1] += cx[1]
            n[v][2] += cx[2]
    out = []
    for x, y, z in n:
        l = math.sqrt(x * x + y * y + z * z)
        out.append((x / l, y / l, z / l) if l > 1e-20 else (0.0, 0.0, 1.0))
    return out


def top4(pairs):
    """[(bone, weight)] -> ([4 bone idx], [4 weights]) top-4 by weight, normalised, padded with (0, 0.0)."""
    acc = {}
    for b, w in pairs:
        if w > 0.0:
            acc[b] = acc.get(b, 0.0) + w
    items = sorted(acc.items(), key=lambda kv: (-kv[1], kv[0]))[:4]
    s = sum(w for _, w in items)
    if s <= 0.0:
        return [0, 0, 0, 0], [0.0, 0.0, 0.0, 0.0], False
    idx = [b for b, _ in items] + [0] * (4 - len(items))
    wt = [w / s for _, w in items] + [0.0] * (4 - len(items))
    return idx, wt, True


def bone_order(parents):
    """Deterministic parents-first order. parents: {name: parent_name|None}. Returns list of names."""
    kids = {}
    for n, p in parents.items():
        kids.setdefault(p or None, []).append(n)  # the rig files use "" for the root's parent
    order = []
    stack = sorted(kids.get(None, []), reverse=True)
    while stack:
        n = stack.pop()
        order.append(n)
        stack.extend(sorted(kids.get(n, []), reverse=True))
    if len(order) != len(parents):
        raise ValueError("bone hierarchy is not a forest rooted at None: %d of %d reached" % (len(order), len(parents)))
    return order


# --------------------------------------------------------------------------- rig binding

def rig_endpoint_spec(spec, groups):
    """Resolve one rig head/tail strategy into a landmark description (indices, not Blender data)."""
    st = spec["strategy"]
    out = {"strategy": st, "default": spec.get("default_position")}
    if "offset" in spec:
        out["offset"] = spec["offset"]
    if st == "CUBE":
        out["group"] = spec["cube_name"]
        out["indices"] = groups[spec["cube_name"]]
        out["mode"] = "mean"
    elif st == "VERTEX":
        out["indices"] = [spec["vertex_index"]]
        out["mode"] = "mean"
    elif st == "MEAN":
        out["indices"] = list(spec["vertex_indices"])
        out["mode"] = "mean"
    else:
        out["indices"] = []
        out["mode"] = "unsupported"
    return out


def derive_endpoint(ep, P):
    if ep["mode"] == "mean" and ep["indices"]:
        return centroid(P, ep["indices"])
    return tuple(ep["default"]) if ep["default"] is not None else None


# --------------------------------------------------------------------------- face landmarks

def derive_face_landmarks(P, groups):
    """Rule-derived face landmark vertex sets on the (neutral) body (indices < N_BODY).

    The base mesh has no named nose / brow / mouth groups, so they are chosen by
    geometric rules relative to joint-group centroids.  The rules (all metres,
    Blender frame, face toward -Y) are recorded in the output so the runtime and
    a viewer can judge them.  Index sets are frozen at the neutral shape.
    """
    cen = lambda n: centroid(P, groups[n])
    el, er = cen("joint-l-eye"), cen("joint-r-eye")
    zeye = 0.5 * (el[2] + er[2])
    yeye = 0.5 * (el[1] + er[1])
    mouth = cen("joint-mouth")
    body = range(N_BODY)
    # nose tip: most frontal (min y) vertex near the midline between the eyes and the mouth
    cand = [i for i in body if abs(P[i][0]) < 0.02 and zeye - 0.075 < P[i][2] < zeye + 0.005]
    if not cand:
        raise ValueError("face landmark rule found no nose candidates")
    tip = min(cand, key=lambda i: (P[i][1], i))
    tp = P[tip]
    nose = [i for i in body if math.dist(P[i], tp) < 0.03 and P[i][1] < tp[1] + 0.035]
    rules = {
        "nose": "body verts within 0.03 m of the most frontal vertex (min y) among |x|<0.02, eye_z-0.075<z<eye_z+0.005; y < tip_y+0.035",
        "brow_l/r": "body verts with eye_z+0.012<z<eye_z+0.045, |x-eye_x|<0.04, y < eye_y-0.003 (on the eye's side of the midline)",
        "eye_l/r_region": "body verts with |x-eye_x|<0.022, |z-eye_z|<0.015, y < eye_y-0.002",
        "eye_l/r_helper": "the base mesh helper groups helper-l-eye / helper-r-eye (the eyeball helper geometry)",
        "mouth": "body verts with |x-mouth_x|<0.035, |z-mouth_z|<0.025, y < mouth_y",
        "lids": "base mesh groups joint-l/r-upperlid and joint-l/r-lowerlid",
    }
    out = {"nose": nose, "nose_tip": [tip]}
    for side, e in (("l", el), ("r", er)):
        out["eye_%s_region" % side] = [i for i in body if abs(P[i][0] - e[0]) < 0.022 and abs(P[i][2] - e[2]) < 0.015 and P[i][1] < e[1] - 0.002]
        out["brow_%s" % side] = [i for i in body if zeye + 0.012 < P[i][2] < zeye + 0.045 and abs(P[i][0] - e[0]) < 0.04 and P[i][1] < e[1] - 0.003]
        out["eye_%s_helper" % side] = groups["helper-%s-eye" % side]
        out["lid_%s_upper" % side] = groups["joint-%s-upperlid" % side]
        out["lid_%s_lower" % side] = groups["joint-%s-lowerlid" % side]
    out["mouth"] = [i for i in body if abs(P[i][0] - mouth[0]) < 0.035 and abs(P[i][2] - mouth[2]) < 0.025 and P[i][1] < mouth[1]]
    out["mouth_joint"] = groups["joint-mouth"]
    out["teeth_upper_helper"] = groups["helper-upper-teeth"]
    out["teeth_lower_helper"] = groups["helper-lower-teeth"]
    return out, rules


# --------------------------------------------------------------------------- skirt (self-authored)

def make_skirt(P, groups, body_w=None, n_seg=40, n_ring=12):
    """Procedural A-line skirt around the hips, bound to nearby body vertices.

    Our own geometry (rings of vertices around the body), no third-party data.
    P: neutral positions (full index space).  Returns a garment dict compatible
    with the mhclo-derived ones: positions, tris, uv, bind arrays, offset spec.
    """
    cen = lambda n: centroid(P, groups[n])
    pel = cen("joint-pelvis")
    kl, kr = cen("joint-l-knee"), cen("joint-r-knee")
    hipx = abs(cen("joint-l-upper-leg")[0])
    zs = [P[i][2] for i in range(N_BODY)]
    zmin, zmax = min(zs), max(zs)
    stature = zmax - zmin
    lim = min(max(2.6 * hipx, 0.20), 0.34)
    z_top = pel[2] + 0.06 * stature
    z_hem = 0.5 * (kl[2] + kr[2]) - 0.03 * stature
    zk = [z_top + (z_hem - z_top) * k / (n_ring - 1) for k in range(n_ring)]
    torso = [i for i in range(N_BODY) if abs(P[i][0]) <= lim]

    def band(z):
        w = 0.012
        for _ in range(6):
            sel = [i for i in torso if abs(P[i][2] - z) <= w]
            if len(sel) >= 8:
                return sel
            w *= 2.0
        raise ValueError("skirt: no body vertices near z=%.3f" % z)

    bands = [band(z) for z in zk]
    cys = [0.5 * (min(P[i][1] for i in b) + max(P[i][1] for i in b)) for b in bands]
    cy = sum(cys) / len(cys)
    two_pi = 2.0 * math.pi
    ang = [-math.pi + (j + 0.5) * two_pi / n_seg for j in range(n_seg)]
    radii = []
    for k, b in enumerate(bands):
        r = [None] * n_seg
        for i in b:
            x, y = P[i][0], P[i][1] - cy
            j = int(((math.atan2(y, x) + math.pi) / two_pi) * n_seg) % n_seg
            d = math.hypot(x, y)
            if r[j] is None or d > r[j]:
                r[j] = d
        filled = [j for j in range(n_seg) if r[j] is not None]
        if not filled:
            raise ValueError("skirt: empty angular bins at ring %d" % k)
        for j in range(n_seg):
            if r[j] is None:
                lo = next(((j - s) % n_seg for s in range(1, n_seg) if r[(j - s) % n_seg] is not None), None)
                hi = next(((j + s) % n_seg for s in range(1, n_seg) if r[(j + s) % n_seg] is not None), None)
                r[j] = 0.5 * (r[lo] + r[hi])
        for _ in range(2):
            sm = [0.25 * r[(j - 1) % n_seg] + 0.5 * r[j] + 0.25 * r[(j + 1) % n_seg] for j in range(n_seg)]
            r = [max(a, b2) for a, b2 in zip(r, sm)]
        t = k / (n_ring - 1.0)
        r = [v + 0.010 + 0.12 * t ** 1.6 for v in r]
        if radii:
            r = [max(a, b2) for a, b2 in zip(r, radii[-1])]
        radii.append(r)
    pos = []
    for k in range(n_ring):
        for j in range(n_seg):
            pos.append((radii[k][j] * math.cos(ang[j]), cy + radii[k][j] * math.sin(ang[j]), zk[k]))
    tris, tuv = [], []
    uvs = [((j) / float(n_seg), 1.0 - k / (n_ring - 1.0)) for k in range(n_ring) for j in range(n_seg + 1)]
    uvi = lambda k, j: k * (n_seg + 1) + j
    vi = lambda k, j: k * n_seg + (j % n_seg)
    for k in range(n_ring - 1):
        for j in range(n_seg):
            tris.append((vi(k, j), vi(k + 1, j), vi(k, j + 1)))
            tuv.append((uvi(k, j), uvi(k + 1, j), uvi(k, j + 1)))
            tris.append((vi(k, j + 1), vi(k + 1, j), vi(k + 1, j + 1)))
            tuv.append((uvi(k, j + 1), uvi(k + 1, j), uvi(k + 1, j + 1)))
    # binding: 3 nearest body vertices in the hip/waist band, inverse-distance weights
    cz0, cz1 = pel[2] - 0.08 * stature, z_top + 0.06 * stature
    cand = [i for i in range(N_BODY) if abs(P[i][0]) <= lim * 1.15 and cz0 <= P[i][2] <= cz1]
    bind = []
    for p in pos:
        near = heapq.nsmallest(3, ((math.dist(P[i], p), i) for i in cand))
        ws = [1.0 / (d + 1e-3) for d, _ in near]
        s = sum(ws)
        ws = [w / s for w in ws]
        ids = [i for _, i in near]
        base = [sum(ws[m] * P[ids[m]][a] for m in range(3)) for a in range(3)]
        bind.append((ids[0], ids[1], ids[2], ws[0], ws[1], ws[2], p[0] - base[0], p[1] - base[1], p[2] - base[2]))
    hipband = [i for i in range(N_BODY) if abs(P[i][2] - pel[2]) < 0.02 * stature]
    xa = max(hipband, key=lambda i: (P[i][0], -i))
    xb = min(hipband, key=lambda i: (P[i][0], i))
    ya = min(hipband, key=lambda i: (P[i][1], i))
    yb = max(hipband, key=lambda i: (P[i][1], -i))
    za = max(range(N_BODY), key=lambda i: (P[i][2], -i))
    zb = min(range(N_BODY), key=lambda i: (P[i][2], i))
    spec = [{"a": xa, "b": xb, "axis": 0, "ref": abs(P[xa][0] - P[xb][0])},
            {"a": ya, "b": yb, "axis": 1, "ref": abs(P[ya][1] - P[yb][1])},
            {"a": za, "b": zb, "axis": 2, "ref": abs(P[za][2] - P[zb][2])}]
    return {"name": "skirt", "positions": pos, "tris": tris, "tri_uv": tuv, "uvs": uvs, "bind": bind,
            "offset_spec": spec, "delete_verts": [],
            "params": {"n_seg": n_seg, "n_ring": n_ring, "z_top": z_top, "z_hem": z_hem, "stature": stature,
                       "lateral_limit": lim, "flare_m": 0.12, "margin_m": 0.010}}


def transfer_weights(bind, body_idx, body_w, P, n_body=N_BODY):
    """Garment skin weights by transfer from the bound body vertices.

    body_idx/body_w: per body vertex 4 bone indices / 4 weights (lists of lists).
    A binding that points at helper geometry (index >= n_body) borrows the weights of the
    nearest body vertex.  Returns (idx[n][4], w[n][4], n_unweighted).
    """
    cache = {}

    def src(v):
        if v < n_body:
            return v
        if v not in cache:
            cache[v] = min(range(n_body), key=lambda i: (math.dist(P[i], P[v]), i))
        return cache[v]

    oi, ow, bad = [], [], 0
    for (a, b, c, wa, wb, wc, _x, _y, _z) in bind:
        pairs = []
        for v, w in ((a, wa), (b, wb), (c, wc)):
            s = src(v)
            for bi, bw in zip(body_idx[s], body_w[s]):
                pairs.append((bi, bw * w))
        i4, w4, ok = top4(pairs)
        bad += 0 if ok else 1
        oi.append(i4)
        ow.append(w4)
    return oi, ow, bad


# --------------------------------------------------------------------------- binary output

def _arr(code, values):
    a = array.array(code, values)
    want = {"f": 4, "I": 4, "H": 2, "B": 1}[code]
    if a.itemsize != want:
        raise RuntimeError("array typecode %s has itemsize %d, expected %d" % (code, a.itemsize, want))
    if sys.byteorder == "big":
        a.byteswap()
    return a


class Output:
    """Writes little-endian blobs and JSON files under a directory, tracks them for the manifest."""

    def __init__(self, root):
        self.root = root
        self.blobs = {}
        os.makedirs(root, exist_ok=True)

    def _path(self, rel):
        p = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        return p

    def blob(self, name, dtype, flat, shape):
        """dtype in f32|u32|u16|u8; flat is any iterable of numbers; shape is the logical shape."""
        code = {"f32": "f", "u32": "I", "u16": "H", "u8": "B"}[dtype]
        a = _arr(code, flat)
        count = 1
        for s in shape:
            count *= s
        if len(a) != count:
            raise ValueError("blob %s: %d values for shape %s" % (name, len(a), shape))
        rel = "blobs/%s.%s.bin" % (name, dtype)
        with open(self._path(rel), "wb") as f:
            a.tofile(f)
        self.blobs[name] = {"file": rel, "dtype": dtype, "shape": list(shape)}
        return self.blobs[name]

    def json(self, rel, obj):
        with open(self._path(rel), "w", encoding="utf-8") as f:
            json.dump(obj, f, indent=1, sort_keys=True)
            f.write("\n")

    def manifest(self, extra):
        files = []
        for dp, dn, fn in os.walk(self.root):
            dn.sort()
            for n in sorted(fn):
                rel = os.path.relpath(os.path.join(dp, n), self.root).replace(os.sep, "/")
                if rel == "manifest.json":
                    continue
                files.append((rel, os.path.join(dp, n)))
        files.sort()
        ent = []
        for rel, full in files:
            h = hashlib.sha256()
            with open(full, "rb") as f:
                for ch in iter(lambda: f.read(1 << 20), b""):
                    h.update(ch)
            ent.append({"path": rel, "sha256": h.hexdigest(), "size": os.path.getsize(full)})
        m = dict(extra)
        m["files"] = ent
        self.json("manifest.json", m)
        return m


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for ch in iter(lambda: f.read(1 << 20), b""):
            h.update(ch)
    return h.hexdigest()


def flat3(seq):
    for t in seq:
        yield t[0]
        yield t[1]
        yield t[2]


# --------------------------------------------------------------------------- self test

def selftest(mpfb, user):
    import time
    fails = 0

    def check(cond, msg):
        nonlocal fails
        print(("PASS " if cond else "FAIL ") + msg)
        if not cond:
            fails += 1

    t0 = time.time()
    base = parse_obj(os.path.join(mpfb, "3dobjs", "base.obj"))
    check(len(base["verts"]) == N_ALL, "base.obj has %d vertices (want %d)" % (len(base["verts"]), N_ALL))
    P = [mh_to_bl(v) for v in base["verts"]]
    tp, tu, _ = triangulate(base["faces"], only_group="body")
    check(len(tp) == 2 * 13378 and tu is not None, "body triangles %d (13378 quads) with uv" % len(tp))
    check(max(max(t) for t in tp) < N_BODY, "body triangles reference only vertices < %d" % N_BODY)
    zs = [P[i][2] for i in range(N_BODY)]
    print("INFO raw base z range %.3f .. %.3f m, x range %.3f .. %.3f" % (min(zs), max(zs), min(P[i][0] for i in range(N_BODY)), max(P[i][0] for i in range(N_BODY))))
    groups = vertex_groups(os.path.join(mpfb, "mesh_metadata", "basemesh_vertex_groups.json"))
    check(groups["body"] == list(range(N_BODY)), "vertex group 'body' is exactly 0..%d" % (N_BODY - 1))
    with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "controls.json"), "r", encoding="utf-8") as f:
        ctl = json.load(f)
    nt = 0
    for c in ctl["controls"]:
        if c["kind"] != "targets":
            continue
        for side in ("neg", "pos"):
            tot = 0
            mx = 0.0
            for rel, _w in c[side]:
                d = parse_target(os.path.join(mpfb, "targets", rel + ".target.gz"))
                tot += len(d)
                mx = max([mx] + [math.sqrt(a * a + b * b + c2 * c2) for _, a, b, c2 in d])
                check(all(0 <= i < N_ALL for i, *_ in d), "%s indices in range" % rel)
            nt += 1
            check(tot > 0 and mx < 0.2, "control %s.%s: %d deltas, max |delta| %.4f m" % (c["name"], side, tot, mx))
    # garment fit: mhclo bound to the raw base should land near the garment obj (authored on the default human)
    for tag, rel in (("casualsuit", "clothes/female_casualsuit01/female_casualsuit01"), ("bob01", "hair/bob01/bob01"),
                     ("eyes", "eyes/low-poly/low-poly"), ("eyebrow", "eyebrows/eyebrow001/eyebrow001"),
                     ("teeth", "teeth/teeth_base/teeth_base")):
        info = parse_mhclo(os.path.join(user, rel + ".mhclo"))
        obj = parse_obj(os.path.join(user, rel + ".obj"))
        check(len(info["bind"]) == len(obj["verts"]), "%s: %d bindings == %d obj vertices" % (tag, len(info["bind"]), len(obj["verts"])))
        fit = fit_binding(info["bind"], offset_scale_spec(info), P)
        ref = [mh_to_bl(v) for v in obj["verts"]]
        errs = sorted(math.dist(a, b) for a, b in zip(fit, ref))
        print("INFO %s fit vs obj on RAW base: median %.4f m, p95 %.4f m, max %.4f m, delete_verts %d" % (
            tag, errs[len(errs) // 2], errs[int(len(errs) * 0.95)], errs[-1], len(info["delete_verts"])))
        # The assets were authored on MakeHuman's default (macro 0.5) human, not on the raw base, so a
        # few-cm offset is expected here; the real formula oracle is MPFB's own fit, run inside Blender.
        check(errs[len(errs) // 2] < 0.08, "%s fit median error < 8 cm on the raw base (gross plausibility only)" % tag)
    # rigs: endpoint re-derivation from landmark groups
    for rig in ("game_engine", "default_no_toes"):
        with open(os.path.join(mpfb, "rigs", "standard", "rig.%s.json" % rig), "r", encoding="utf-8") as f:
            rj = json.load(f)
        bad, dmax = 0, 0.0
        for n, b in rj.items():
            for e in ("head", "tail"):
                ep = rig_endpoint_spec(b[e], groups)
                p = derive_endpoint(ep, P)
                dflt = b[e]["default_position"]
                dmax = max(dmax, math.dist(p, dflt))
                bad += ep["mode"] == "unsupported" or "offset" in b[e]
        print("INFO rig %s: %d bones, max |derived(raw base) - default_position| = %.4f m (default_position uses another origin; fallback only)" % (rig, len(rj), dmax))
        check(bad == 0, "rig %s: all endpoints are mean-of-vertices strategies (no offset/unsupported)" % rig)
        order = bone_order({n: b["parent"] for n, b in rj.items()})
        check(len(order) == len(rj), "rig %s: forest order covers %d bones" % (rig, len(order)))
    fl, rules = derive_face_landmarks(P, groups)
    print("INFO face landmark sizes on raw base: " + ", ".join("%s=%d" % (k, len(v)) for k, v in sorted(fl.items())))
    check(all(len(fl[k]) > 0 for k in ("nose", "brow_l", "brow_r", "eye_l_region", "eye_r_region", "mouth")), "face landmark sets non-empty")
    check(abs(P[fl["nose_tip"][0]][0]) < 0.02, "nose tip near midline")
    sk = make_skirt(P, groups)
    fit = fit_binding(sk["bind"], sk["offset_spec"], P)
    e = max(math.dist(a, b) for a, b in zip(fit, sk["positions"]))
    check(e < 1e-9, "skirt binding reproduces its own positions (max err %.2e m)" % e)
    check(len(sk["tris"]) == 2 * 40 * 11 and len(sk["positions"]) == 480, "skirt has 480 vertices, 880 triangles")
    ns = vertex_normals(sk["positions"], sk["tris"])
    cyc = sum(p[1] for p in sk["positions"]) / len(sk["positions"])
    outw = sum(1 for p, n in zip(sk["positions"], ns) if (p[0] * n[0] + (p[1] - cyc) * n[1]) > 0)
    check(outw > 0.9 * len(ns), "skirt normals point outward (%d of %d)" % (outw, len(ns)))
    print("INFO skirt z_top %.3f z_hem %.3f stature %.3f lateral_limit %.3f" % (sk["params"]["z_top"], sk["params"]["z_hem"], sk["params"]["stature"], sk["params"]["lateral_limit"]))
    # top4 + bone order sanity
    i4, w4, ok = top4([(3, 0.5), (1, 0.2), (7, 0.2), (2, 0.05), (9, 0.05), (3, 0.1)])
    check(ok and i4 == [3, 1, 7, 2] and abs(sum(w4) - 1.0) < 1e-12, "top4 normalises and ties break by bone index: %s %s" % (i4, [round(x, 3) for x in w4]))
    # blob writer round trip
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        o = Output(td)
        o.blob("t", "f32", [1.0, 2.5, -3.0], [3])
        o.blob("u", "u32", [1, 2, 4000000000], [3])
        raw = open(os.path.join(td, o.blobs["t"]["file"]), "rb").read()
        check(raw == b"\x00\x00\x80\x3f\x00\x00\x20\x40\x00\x00\x40\xc0", "f32 blob is little-endian")
        m = o.manifest({"format": "test"})
        check(len(m["files"]) == 2 and all(len(f["sha256"]) == 64 for f in m["files"]), "manifest hashes every file")
    print("SELFTEST fails=%d seconds=%.1f" % (fails, time.time() - t0))
    return fails


if __name__ == "__main__":
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--mpfb-data", default=os.environ.get("MPFB_DATA"))
    ap.add_argument("--user-data", default=os.environ.get("MPFB_USER_DATA"))
    a = ap.parse_args()
    if not a.selftest:
        ap.error("nothing to do; use --selftest")
    sys.exit(1 if selftest(a.mpfb_data, a.user_data) else 0)
