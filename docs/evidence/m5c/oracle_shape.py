#!/usr/bin/env python3
"""Independent f64 oracle for `figure::studio`: shaped neutral vertices, refit joints and garment
fits, recomputed from the CONVERTED asset files (`gibson.figure.asset.v1`) with its own reader and
its own formulation. Run with `python3 -I`; standard library only; no Blender, no Rust.

What it recomputes (the `rev.v1` coupled derivation, docs/FIGURE_CONTRACT.md):

  shaped_v = neutral_v + sum_c |c| * delta[c][side(c)]_v        (f64, the order of the sum is moot)
  lift     = -min over body vertices of shaped z                 (when the asset is grounded)
  shaped_v += (0, 0, lift)
  joint_j  = mean of the shaped positions of j's anchor group   (world, f64)
  local_t  = R_parent^T (joint_j - joint_parent)                (R_parent: world rest rotation, the
                                                                  product of the declared local rotations)
  bone     = |local_t|
  part_i   = w0 P[v0] + w1 P[v1] + w2 P[v2] + (ox sx, oy sy, oz sz)   (the MakeHuman .mhclo rule)
  penetration: nearest body vertex (ties: lowest index; a part vertex whose nearest body vertex is in its delete_verts is skipped) of each part vertex; signed distance along that
  vertex's area-weighted normal; depth = -signed if negative; count = #(depth > tolerance).

Usage:
  oracle_shape.py --asset DIR [--control name=value ...] [--preset FILE.json]
                  [--garment ID ...] [--tolerance 0.002] [--adapt-depth 0.01]
                  [--blender-oracle EXPORT_DIR] [--dump-positions FILE.f32] [--json-out FILE]
                  [--rust-positions FILE.f32] [--rust-tol 2e-5]

--blender-oracle EXPORT_DIR compares the shaped positions with `blobs/oracle.shaped_positions.f32.bin`
of the export (its preset must be the one given here; that blob is also lifted so the lowest body
vertex is at z = 0). --rust-positions compares with an f32 LE dump of `Compiled::shaped_positions()`.
Exit status 0 when every requested comparison is within tolerance, 1 otherwise.
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


class R:
    def __init__(self, data):
        self.d = data
        self.p = 0

    def take(self, n):
        if self.p + n > len(self.d):
            raise ValueError("truncated at %d" % self.p)
        s = self.d[self.p:self.p + n]
        self.p += n
        return s

    def u8(self):
        return self.take(1)[0]

    def u32(self):
        return struct.unpack("<I", self.take(4))[0]

    def i32(self):
        return struct.unpack("<i", self.take(4))[0]

    def f32(self):
        return struct.unpack("<f", self.take(4))[0]

    def vec3(self):
        return [self.f32(), self.f32(), self.f32()]

    def str(self):
        return self.take(self.u32()).decode("utf-8")


def read_skin(r):
    n = r.u8()
    return [(r.u32(), r.f32()) for _ in range(n)]


def parse_body(data):
    r = R(data)
    if r.take(len(BODY_MAGIC)) != BODY_MAGIC:
        raise ValueError("bad body magic")
    b = {"id": r.str(), "version": r.u32(), "grounded": r.u8() == 1}
    n_v = r.u32()
    b["n_body"] = r.u32()
    b["pos"] = [r.vec3() for _ in range(n_v)]
    n_idx = r.u32()
    b["tris"] = [r.u32() for _ in range(n_idx)]
    joints = []
    for _ in range(r.u32()):
        name = r.str()
        parent = r.i32()
        t = r.vec3()
        q = [r.f32() for _ in range(4)]
        s = r.vec3()
        anchor = [r.u32() for _ in range(r.u32())]
        joints.append({"name": name, "parent": parent, "t": t, "q": q, "s": s, "anchor": anchor})
    b["joints"] = joints
    b["skin"] = [read_skin(r) for _ in range(b["n_body"])]
    controls = []
    for _ in range(r.u32()):
        name = r.str()
        lo, hi, df = r.f32(), r.f32(), r.f32()
        sides = []
        for _ in range(2):
            n = r.u32()
            sides.append([(r.u32(), r.vec3()) for _ in range(n)])
        controls.append({"name": name, "min": lo, "max": hi, "default": df, "neg": sides[0], "pos": sides[1]})
    b["controls"] = controls
    b["landmarks"] = [(r.str(), [r.u32() for _ in range(r.u32())]) for _ in range(r.u32())]
    if r.p != len(data):
        raise ValueError("trailing bytes in body")
    return b


def parse_part(data):
    r = R(data)
    if r.take(len(PART_MAGIC)) != PART_MAGIC:
        raise ValueError("bad part magic")
    p = {"id": r.str(), "kind": r.str()}
    n = r.u32()
    p["ref"] = [r.vec3() for _ in range(n)]
    p["tris"] = [r.u32() for _ in range(r.u32())]
    p["bind"] = []
    for _ in range(n):
        v = [r.u32(), r.u32(), r.u32()]
        w = [r.f32(), r.f32(), r.f32()]
        o = r.vec3()
        p["bind"].append((v, w, o))
    spec = []
    for _ in range(r.u32()):
        if r.u8() == 0:
            spec.append(("const", r.f32()))
        else:
            a, b, axis, ref = r.u32(), r.u32(), r.u8(), r.f32()
            spec.append(("pair", a, b, axis, ref))
    p["spec"] = spec
    p["skin"] = [read_skin(r) for _ in range(n)]
    p["hidden"] = [r.u32() for _ in range(r.u32())]
    if r.p != len(data):
        raise ValueError("trailing bytes in part")
    return p


def load_asset(d):
    index = {}
    parts = []
    with open(os.path.join(d, "index.txt"), "r", encoding="utf-8") as f:
        for line in f:
            f_ = line.split()
            if not f_:
                continue
            if f_[0] == "body":
                index["body"] = (f_[1], f_[2])
            elif f_[0] == "part":
                parts.append((f_[1], f_[2], f_[3], f_[4]))
    body_file, body_sha = index["body"]
    with open(os.path.join(d, body_file), "rb") as f:
        raw = f.read()
    if hashlib.sha256(raw).hexdigest() != body_sha:
        raise ValueError("body sha mismatch")
    body = parse_body(raw)
    out = {}
    for pid, kind, fn, sha in parts:
        with open(os.path.join(d, fn), "rb") as f:
            raw = f.read()
        if hashlib.sha256(raw).hexdigest() != sha:
            raise ValueError("part sha mismatch: " + fn)
        out[pid] = parse_part(raw)
    return body, out


# ----------------------------------------------------------------------------- math (f64)

def qmul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw,
            aw * bw - ax * bx - ay * by - az * bz]


def qnorm(q):
    n = math.sqrt(sum(c * c for c in q))
    return [c / n for c in q]


def rot_from_q(q):
    x, y, z, w = qnorm(q)
    return [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]


def rt_v(r, v):
    """R^T v."""
    return [r[0][i] * v[0] + r[1][i] * v[1] + r[2][i] * v[2] for i in range(3)]


def sub(a, b):
    return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]


def norm(v):
    return math.sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])


# ----------------------------------------------------------------------------- shaping

def shape(body, values):
    names = [c["name"] for c in body["controls"]]
    for k in values:
        if k not in names:
            raise ValueError("unknown control " + k)
    p = [list(map(float, v)) for v in body["pos"]]
    for c in body["controls"]:
        v = values.get(c["name"], c["default"])
        if v == 0.0:
            continue
        side = c["pos"] if v > 0.0 else c["neg"]
        w = abs(v)
        for idx, d in side:
            p[idx][0] += w * d[0]
            p[idx][1] += w * d[1]
            p[idx][2] += w * d[2]
    lift = 0.0
    if body["grounded"]:
        lift = -min(p[i][2] for i in range(body["n_body"]))
        for v in p:
            v[2] += lift
    return p, lift


def joints_from(body, shaped):
    js = body["joints"]
    world_q = []
    world_r = []
    pos = []
    for j in js:
        parent = j["parent"]
        a = j["anchor"]
        if a:
            n = float(len(a))
            pos.append([sum(shaped[i][k] for i in a) / n for k in range(3)])
        elif parent < 0:
            pos.append(list(j["t"]))
        else:
            # unanchored: rides with its parent at the declared local offset
            r, t = world_r[parent], j["t"]
            pos.append([pos[parent][i] + sum(r[i][k] * t[k] for k in range(3)) for i in range(3)])
        q = qnorm(j["q"]) if parent < 0 else qnorm(qmul(world_q[parent], qnorm(j["q"])))
        world_q.append(q)
        world_r.append(rot_from_q(q))
    out = []
    for i, j in enumerate(js):
        parent = j["parent"]
        lt = list(pos[i]) if parent < 0 else rt_v(world_r[parent], sub(pos[i], pos[parent]))
        out.append({"name": j["name"], "world": pos[i], "local_t": lt,
                    "bone_length": None if parent < 0 else norm(lt)})
    return out


# ----------------------------------------------------------------------------- parts

def scales(spec, p):
    if len(spec) != 3:
        return [1.0, 1.0, 1.0]
    out = []
    for s in spec:
        if s[0] == "const":
            out.append(s[1])
        else:
            _, a, b, axis, ref = s
            out.append(abs(p[a][axis] - p[b][axis]) / ref)
    return out


def fit_part(part, p):
    s = scales(part["spec"], p)
    out = []
    for v, w, o in part["bind"]:
        out.append([w[0] * p[v[0]][k] + w[1] * p[v[1]][k] + w[2] * p[v[2]][k] + o[k] * s[k] for k in range(3)])
    return out


def body_normals(p, tris, n_body):
    acc = [[0.0, 0.0, 0.0] for _ in range(n_body)]
    for t in range(0, len(tris), 3):
        a, b, c = tris[t], tris[t + 1], tris[t + 2]
        e1, e2 = sub(p[b], p[a]), sub(p[c], p[a])
        n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
        for v in (a, b, c):
            for k in range(3):
                acc[v][k] += n[k]
    out = []
    for v in acc:
        l = norm(v)
        out.append([v[0] / l, v[1] / l, v[2] / l] if l > 0.0 else None)
    return out


def penetration(garment, p, normals, tolerance, hidden=()):
    hid = set(hidden)  # the body vertices the part hides (delete_verts) are not measured
    cand = [i for i, n in enumerate(normals) if n is not None]
    cx = [p[i][0] for i in cand]
    cy = [p[i][1] for i in cand]
    cz = [p[i][2] for i in cand]
    count = 0
    max_depth = 0.0
    rng = range(len(cand))
    for g in garment:
        gx, gy, gz = g
        best, best_d2 = -1, float("inf")
        for k in rng:
            dx, dy, dz = gx - cx[k], gy - cy[k], gz - cz[k]
            d2 = dx * dx + dy * dy + dz * dz
            if d2 < best_d2:
                best_d2, best = d2, k
        b = cand[best]
        if b in hid:
            continue  # nearest body vertex is hidden under the part: not measured
        n = normals[b]
        s = (gx - p[b][0]) * n[0] + (gy - p[b][1]) * n[1] + (gz - p[b][2]) * n[2]
        if s < 0.0:
            depth = -s
            if depth > tolerance:
                count += 1
            max_depth = max(max_depth, depth)
    return count, max_depth


def verdict(count, max_depth, adapt):
    if count == 0:
        return "Fitted"
    return "NeedsAdaptation" if max_depth <= adapt else "Rejected"


# ----------------------------------------------------------------------------- main

def read_f32_file(path):
    a = array.array("f")
    with open(path, "rb") as f:
        a.frombytes(f.read())
    if sys.byteorder != "little":
        a.byteswap()
    return a


def main(argv):
    ap = argparse.ArgumentParser(description="f64 oracle for figure::studio")
    ap.add_argument("--asset", required=True)
    ap.add_argument("--control", action="append", default=[])
    ap.add_argument("--preset")
    ap.add_argument("--garment", action="append", default=[])
    ap.add_argument("--tolerance", type=float, default=0.002)
    ap.add_argument("--adapt-depth", type=float, default=0.01)
    ap.add_argument("--blender-oracle")
    ap.add_argument("--dump-positions")
    ap.add_argument("--json-out")
    ap.add_argument("--rust-positions")
    ap.add_argument("--rust-tol", type=float, default=2e-5)
    ap.add_argument("--blender-tol", type=float, default=1e-5)
    args = ap.parse_args(argv)

    body, parts = load_asset(args.asset)
    values = {}
    if args.preset:
        with open(args.preset, "r", encoding="utf-8") as f:
            pj = json.load(f)
        values.update(pj.get("controls", pj))
    for kv in args.control:
        k, v = kv.split("=", 1)
        values[k] = float(v)
    for k, v in values.items():
        if not math.isfinite(float(v)):
            raise ValueError("non-finite control " + k)

    shaped, lift = shape(body, values)
    joints = joints_from(body, shaped)
    # f32 bytes of the shaped positions: what a Rust f32 dump is compared against
    flat = array.array("f", [c for v in shaped for c in v])
    if sys.byteorder != "little":
        flat.byteswap()
    raw = flat.tobytes()
    sha = hashlib.sha256(raw).hexdigest()
    zmin = min(shaped[i][2] for i in range(body["n_body"]))
    ok = True
    summary = {
        "asset_id": body["id"], "controls": {k: float(v) for k, v in sorted(values.items())},
        "ground_lift": lift, "body_min_z": zmin, "shaped_f32_sha256": sha,
        "joints": joints, "garments": {},
    }
    print("ORACLE asset=%s vertices=%d body=%d ground_lift=%.9g body_min_z=%.3g shaped_f32_sha256=%s"
          % (body["id"], len(shaped), body["n_body"], lift, zmin, sha))
    for j in joints:
        bl = "-" if j["bone_length"] is None else "%.9g" % j["bone_length"]
        print("JOINT %-14s world=(%.7g, %.7g, %.7g) bone=%s" % (j["name"], j["world"][0], j["world"][1], j["world"][2], bl))

    if args.garment:
        normals = body_normals(shaped, body["tris"], body["n_body"])
        for gid in args.garment:
            part = parts[gid]
            pos = fit_part(part, shaped)
            count, depth = penetration(pos, shaped, normals, args.tolerance, part["hidden"])
            v = verdict(count, depth, args.adapt_depth)
            summary["garments"][gid] = {"vertices": len(pos), "penetrating": count, "max_depth_m": depth, "verdict": v}
            print("GARMENT %-22s vertices=%d penetrating=%d max_depth_m=%.6g verdict=%s" % (gid, len(pos), count, depth, v))

    if args.dump_positions:
        with open(args.dump_positions, "wb") as f:
            f.write(raw)
    if args.json_out:
        with open(args.json_out, "w", encoding="utf-8") as f:
            json.dump(summary, f, indent=1, sort_keys=True)
            f.write("\n")

    if args.blender_oracle:
        ref = read_f32_file(os.path.join(args.blender_oracle, "blobs", "oracle.shaped_positions.f32.bin"))
        if len(ref) != len(shaped) * 3:
            print("BLENDER_ORACLE length mismatch")
            ok = False
        else:
            worst = max(abs(ref[3 * i + k] - shaped[i][k]) for i in range(len(shaped)) for k in range(3))
            good = worst <= args.blender_tol
            ok = ok and good
            print("BLENDER_ORACLE max_abs_err=%.3g tol=%.3g %s" % (worst, args.blender_tol, "OK" if good else "FAIL"))
    if args.rust_positions:
        ref = read_f32_file(args.rust_positions)
        if len(ref) != len(shaped) * 3:
            print("RUST_COMPARE length mismatch")
            ok = False
        else:
            worst = max(abs(ref[3 * i + k] - shaped[i][k]) for i in range(len(shaped)) for k in range(3))
            good = worst <= args.rust_tol
            ok = ok and good
            print("RUST_COMPARE max_abs_err=%.3g tol=%.3g %s" % (worst, args.rust_tol, "OK" if good else "FAIL"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
