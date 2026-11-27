#!/usr/bin/env python3
"""Write a small VALID `gibson.figure.tape.v1` test tape (pure python) so the render adapter can be smoke-tested before the
runtime produces real tapes.  It raises both arms (with a small spine sway, a pelvis bob and a thigh swing so the skirt moves).

    python3 -I tools/figure_assets/make_test_tape.py --export OUT/export --preset tools/figure_assets/presets/generic_neutral.json \
        --out OUT/test [--frames 12] [--fps 30] [--with-skirt] [--joint-frame world_aligned|bone] [--fit-status garment=Verdict ...]
    python3 -I tools/figure_assets/make_test_tape.py --selftest

Needs only the export directory (skeleton, deltas, landmarks).  The joint rest locals are re-derived from the SHAPED body exactly as
render_tape.py does (control values from the preset), so a rest frame is the identity pose.  This is NOT the runtime's tape: ids are
sha256 of the inputs, `provenance` says so, and nothing downstream may treat it as a runtime-evaluated performance.
"""
import argparse
import hashlib
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)
import fa_tape as ft  # noqa: E402


def sha(s):
    return hashlib.sha256(s if isinstance(s, bytes) else s.encode()).hexdigest()


def rest_locals(skel, wbind):
    out = []
    for i, b in enumerate(skel["bones"]):
        p = b["parent"]
        L = wbind[i] if p < 0 else ft.mul(ft.inv_affine(wbind[p]), wbind[i])
        out.append(L)
    return out


def build(export_dir, preset, frames, fps, joint_frame, with_skirt, fit_status_over):
    S, shift, warns = ft.shape_positions(export_dir, preset["controls"])
    skel = ft.load_json(os.path.join(export_dir, "skeleton.json"))
    wbind, heads = ft.bind_frames(skel, S, joint_frame, export_dir)
    L = rest_locals(skel, wbind)
    names = [b["name"] for b in skel["bones"]]
    pos = {n: i for i, n in enumerate(names)}
    for need in ("upperarm_l", "upperarm_r", "thigh_l", "thigh_r", "spine_02", "pelvis"):
        if need not in pos:
            raise SystemExit("rig has no bone %r (this generator knows the game_engine rig)" % need)
    rt = []
    for i in range(len(names)):
        m = L[i]
        rt.append(((m[0][3], m[1][3], m[2][3]), (0.0, 0.0, 0.0, 1.0), (1.0, 1.0, 1.0)))
    rot0 = {}
    if joint_frame == "bone":
        # bone-oriented joint frames: the rest rotation is part of the local; keep it by converting the matrix to a quaternion
        for i in range(len(names)):
            rt[i] = (rt[i][0], _quat_from_matrix(L[i]), (1.0, 1.0, 1.0))
    out_frames = []
    n = max(1, frames)
    for f in range(frames):
        ph = f / float(n)
        lift = 0.5 * (1.0 - math.cos(2.0 * math.pi * ph))
        th = math.radians(95.0) * lift
        sway = math.radians(2.0) * math.sin(2.0 * math.pi * ph)
        swing = math.radians(22.0) * math.sin(2.0 * math.pi * ph)
        jl = [list(x) for x in rt]
        for side in ("l", "r"):
            i = pos["upperarm_" + side]
            sx = 1.0 if heads[i][0] > heads[pos["pelvis"]][0] else -1.0
            jl[i][1] = _qmul(_qaxis((0, 1, 0), -sx * th), rt[i][1]) if joint_frame == "world_aligned" else _qmul(rt[i][1], _qaxis((0, 1, 0), -sx * th))
            t = pos["thigh_" + side]
            sg = 1.0 if side == "l" else -1.0
            jl[t][1] = _qmul(_qaxis((1, 0, 0), sg * swing), rt[t][1]) if joint_frame == "world_aligned" else _qmul(rt[t][1], _qaxis((1, 0, 0), sg * swing))
        s2 = pos["spine_02"]
        jl[s2][1] = _qmul(_qaxis((1, 0, 0), sway), rt[s2][1]) if joint_frame == "world_aligned" else _qmul(rt[s2][1], _qaxis((1, 0, 0), sway))
        root = (0.0, 0.0, 0.01 * math.sin(2.0 * math.pi * ph))
        out_frames.append((root, [(tuple(a), tuple(b), tuple(c)) for a, b, c in jl], []))
    garments = list(preset.get("render", {}).get("garments", []))
    if with_skirt and "skirt_a_line" not in garments:
        garments.append("skirt_a_line")
    fit = {g: "Fitted" for g in garments}
    fit.update(fit_status_over)
    for g in fit_status_over:
        if g not in garments:
            garments.append(g)
    mats = preset.get("render", {}).get("materials", {})
    ctl = dict(preset["controls"])
    body_rev = sha(json.dumps([os.path.basename(export_dir), ctl], sort_keys=True))
    manifest = os.path.join(export_dir, "manifest.json")
    man_sha = sha(open(manifest, "rb").read()) if os.path.isfile(manifest) else sha("no manifest")
    fit_id = sha(body_rev + json.dumps([garments, fit], sort_keys=True))
    present = sha(fit_id + json.dumps(mats, sort_keys=True))
    header = {"body_rev_id": body_rev, "fit_id": fit_id, "present_id": present, "asset_manifest_sha256": man_sha,
              "controls": ctl, "garments": garments, "fit_status": fit, "materials": mats,
              "style": {"preset": "restrained", "amplitude": 0.55, "tempo": 0.85, "settle_ms": 320},
              "fps": fps, "joints": names, "face": [],
              "events": [{"frame": 0, "kind": "test", "label": "arms rise"}],
              "camera_cuts": [{"frame": 0, "camera_id": "main"}, {"frame": max(1, frames // 2), "camera_id": "alt"}],
              "provenance": "make_test_tape.py: a SYNTHETIC test pose, not a runtime-evaluated performance (joint_frame=%s)" % joint_frame}
    return header, out_frames, warns


def _qaxis(axis, ang):
    return ft.quat_axis_angle(axis, ang)


def _qmul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz)


def _quat_from_matrix(m):
    r = [[m[i][j] for j in range(3)] for i in range(3)]
    # strip scale
    for j in range(3):
        n = math.sqrt(sum(r[i][j] ** 2 for i in range(3)))
        for i in range(3):
            r[i][j] /= n
    tr = r[0][0] + r[1][1] + r[2][2]
    if tr > 0:
        s = math.sqrt(tr + 1.0) * 2
        return ((r[2][1] - r[1][2]) / s, (r[0][2] - r[2][0]) / s, (r[1][0] - r[0][1]) / s, 0.25 * s)
    if r[0][0] > r[1][1] and r[0][0] > r[2][2]:
        s = math.sqrt(1.0 + r[0][0] - r[1][1] - r[2][2]) * 2
        return (0.25 * s, (r[0][1] + r[1][0]) / s, (r[0][2] + r[2][0]) / s, (r[2][1] - r[1][2]) / s)
    if r[1][1] > r[2][2]:
        s = math.sqrt(1.0 + r[1][1] - r[0][0] - r[2][2]) * 2
        return ((r[0][1] + r[1][0]) / s, 0.25 * s, (r[1][2] + r[2][1]) / s, (r[0][2] - r[2][0]) / s)
    s = math.sqrt(1.0 + r[2][2] - r[0][0] - r[1][1]) * 2
    return ((r[0][2] + r[2][0]) / s, (r[1][2] + r[2][1]) / s, 0.25 * s, (r[1][0] - r[0][1]) / s)


def _selftest():
    """Synthetic export in a temp dir (a 3-bone skeleton is not the game_engine rig, so only the writer/reader and quaternion helpers)."""
    fails = []

    def check(ok, msg):
        print(("PASS " if ok else "FAIL ") + msg)
        if not ok:
            fails.append(msg)

    q = _qaxis((0, 0, 1), 0.9)
    m = ft.from_trs((0, 0, 0), q, (1, 1, 1))
    q2 = _quat_from_matrix(m)
    sgn = 1.0 if q2[3] * q[3] > 0 else -1.0
    check(all(abs(sgn * a - b) < 1e-9 for a, b in zip(q2, q)), "quaternion <-> matrix round trip")
    a, b = _qaxis((0, 1, 0), 0.3), _qaxis((1, 0, 0), 0.5)
    ab = _qmul(a, b)
    ma = ft.mul(ft.from_trs((0, 0, 0), a, (1, 1, 1)), ft.from_trs((0, 0, 0), b, (1, 1, 1)))
    mb = ft.from_trs((0, 0, 0), ab, (1, 1, 1))
    check(all(abs(ma[i][j] - mb[i][j]) < 1e-12 for i in range(3) for j in range(3)), "quaternion product matches the matrix product")
    print("SELFTEST_FAILS %d" % len(fails))
    return 1 if fails else 0


def main():
    if "--selftest" in sys.argv:
        return _selftest()
    ap = argparse.ArgumentParser(prog="make_test_tape.py")
    ap.add_argument("--export", required=True)
    ap.add_argument("--preset", required=True)
    ap.add_argument("--out", required=True, help="output prefix directory; writes <out>/test.tape.json + test.tape.bin")
    ap.add_argument("--name", default="test")
    ap.add_argument("--frames", type=int, default=12)
    ap.add_argument("--fps", type=float, default=30.0)
    ap.add_argument("--with-skirt", action="store_true")
    ap.add_argument("--joint-frame", default="world_aligned", choices=("world_aligned", "bone"))
    ap.add_argument("--fit-status", action="append", default=[], help="GARMENT=Verdict override, e.g. female_casualsuit01=Failed")
    a = ap.parse_args()
    with open(a.preset, "r", encoding="utf-8") as f:
        preset = json.load(f)
    over = dict(kv.split("=", 1) for kv in a.fit_status)
    header, frames, warns = build(a.export, preset, a.frames, a.fps, a.joint_frame, a.with_skirt, over)
    os.makedirs(a.out, exist_ok=True)
    prefix = os.path.join(a.out, a.name)
    h = ft.write_tape(prefix, header, frames)
    t = ft.read_tape(prefix + ".tape.json")          # round trip through the validating reader
    print("TEST_TAPE frames=%d joints=%d bin_sha256=%s" % (t.frame_count, t.n_joints, h["bin_sha256"][:16]))
    for w in warns:
        print("TEST_TAPE_WARN " + w)
    return 0


if __name__ == "__main__":
    sys.exit(main())
