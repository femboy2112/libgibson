#!/usr/bin/env python3
"""Pure-python end-to-end self test of the tape path WITHOUT Blender: builds a synthetic export directory (a made-up skeleton
with the real game_engine bone names, one vertex per joint, one control delta), writes a test tape with make_test_tape.py's
generator, reads it back, and checks the kinematics.  Run:  python3 -I tools/figure_assets/fa_selftest.py [--mpfb-data DIR]

It needs the MPFB rig definition (for bone names and parents) from --mpfb-data / $MPFB_DATA; without it the test is SKIPPED (rc 0,
prints SKIP) because bone names are third-party data and are never copied into this repo.
"""
import array
import json
import math
import os
import shutil
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)
import fa_common as fc  # noqa: E402
import fa_tape as ft  # noqa: E402
import make_test_tape as mt  # noqa: E402


def w32(path, code, vals):
    a = array.array(code, vals)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(a.tobytes())


def synth_export(root, rig):
    order = fc.bone_order({n: (v["parent"] or None) for n, v in rig.items()})
    idx = {n: i for i, n in enumerate(order)}
    head = {}
    for n in order:
        par = rig[n]["parent"]
        p = head[par] if par else (0.0, 0.0, 0.0)
        dx = 0.0
        if n.endswith("_l"):
            dx = 0.09
        if n.endswith("_r"):
            dx = -0.09
        up = 0.0 if n == "Root" else (0.12 if not n.startswith(("thigh", "calf", "foot", "ball")) else -0.2)
        if n == "pelvis":
            head[n] = (0.0, 0.0, 0.95)
        elif n.startswith(("upperarm", "lowerarm", "hand", "thumb", "index", "middle", "ring", "pinky")):
            side = 1.0 if n.endswith("_l") else -1.0
            head[n] = (p[0] + side * 0.12, p[1], p[2])
        else:
            head[n] = (p[0] + dx, p[1], p[2] + up)
    pos = [(0.0, 0.0, 0.0)] * fc.N_ALL
    pos = [list(p) for p in pos]
    for n in order:
        pos[idx[n]] = list(head[n])
    pos[200] = [0.0, 0.0, 0.0]            # the lowest body vertex (floor)
    for i in range(300, fc.N_BODY):
        pos[i] = [0.001 * (i % 17), 0.001 * (i % 13), 0.5 + 0.0001 * (i % 100)]
    w32(os.path.join(root, "blobs/base.positions.f32.bin"), "f", [c for p in pos for c in p])
    bones = []
    for n in order:
        par = rig[n]["parent"]
        bones.append({"name": n, "parent": idx[par] if par else -1, "head": list(head[n]), "tail": list(head[n]),
                      "head_binding": {"strategy": "VERTEX", "indices": [idx[n]], "mode": "mean"}, "head_default": list(head[n])})
    json.dump({"bones": bones, "rest_matrix_blob": None}, open(os.path.join(root, "skeleton.json"), "w"))
    w32(os.path.join(root, "blobs/d.idx.bin"), "I", [idx["spine_01"]])
    w32(os.path.join(root, "blobs/d.xyz.bin"), "f", [0.02, 0.0, 0.0])
    w32(os.path.join(root, "blobs/z.idx.bin"), "I", [])
    w32(os.path.join(root, "blobs/z.xyz.bin"), "f", [])
    empty = {"idx": "blobs/z.idx.bin", "xyz": "blobs/z.xyz.bin"}
    names = [c["name"] for c in json.load(open(os.path.join(HERE, "controls.json")))["controls"]]
    ctl = [{"name": n, "pos": ({"idx": "blobs/d.idx.bin", "xyz": "blobs/d.xyz.bin"} if n == "waist" else empty), "neg": empty} for n in names]
    json.dump({"controls": ctl}, open(os.path.join(root, "controls_export.json"), "w"))
    json.dump({"blobs": {"base.positions": {"file": "blobs/base.positions.f32.bin"}}, "rig": {"name": "game_engine"}, "preset": {}, "ground_shift_z_neutral": 0.0},
              open(os.path.join(root, "figure_asset.json"), "w"))
    return order, idx


def main():
    md = None
    for i, a in enumerate(sys.argv):
        if a == "--mpfb-data":
            md = sys.argv[i + 1]
    md = md or os.environ.get("MPFB_DATA")
    rigf = os.path.join(md, "rigs", "standard", "rig.game_engine.json") if md else None
    if not rigf or not os.path.isfile(rigf):
        print("SKIP fa_selftest: no MPFB rig definition (set MPFB_DATA or pass --mpfb-data); the bone names are third-party data")
        print("SELFTEST_FAILS 0")
        return 0
    rig = json.load(open(rigf))
    fails = []

    def check(ok, msg):
        print(("PASS " if ok else "FAIL ") + msg)
        if not ok:
            fails.append(msg)

    d = tempfile.mkdtemp(prefix="fa_selftest_")
    order, idx = synth_export(d, rig)
    preset = {"controls": {"waist": 0.5}, "render": {"garments": ["g1"], "materials": {"base": {"albedo": [0.1, 0.2, 0.3]}}}}
    header, frames, warns = mt.build(d, preset, 12, 30.0, "world_aligned", True, {"g2": "Failed"})
    ft.write_tape(os.path.join(d, "t"), header, frames)
    t = ft.read_tape(os.path.join(d, "t.tape.json"))
    check(t.frame_count == 12 and t.n_joints == len(order), "tape round trip: 12 frames, %d joints" % len(order))
    check(header["fit_status"] == {"g1": "Fitted", "skirt_a_line": "Fitted", "g2": "Failed"} and "g2" in header["garments"], "fit_status carries Fitted and a Failed verdict")
    check(t.camera_id_at(0) == "main" and t.camera_id_at(11) == "alt", "camera cuts resolve per frame")
    skel = ft.load_json(os.path.join(d, "skeleton.json"))
    S, shift, _ = ft.shape_positions(d, preset["controls"])
    check(abs(min(p[2] for p in S[:fc.N_BODY])) < 1e-9, "shaped body is re-grounded (min body z = 0)")
    check(abs(S[idx["spine_01"]][0] - 0.01) < 1e-6, "control delta applied at |c| = 0.5 (0.02 * 0.5)")
    wb, heads = ft.bind_frames(skel, S, "world_aligned")
    S0 = ft.skin_matrices(skel, wb, t, 0)
    # frame 0: arms at rest, thigh swing 0, bob 0 -> identity skin matrices
    err = max(abs(S0[i][r][c] - (1.0 if r == c else 0.0)) for i in range(len(order)) for r in range(4) for c in range(4))
    check(err < 1e-5, "frame 0 of the test tape is the rest pose (max skin-matrix deviation %.2e)" % err)
    S6 = ft.skin_matrices(skel, wb, t, 6)       # lift = 1 (peak): arms up
    ua = idx["upperarm_l"]
    hand = idx["hand_l"]
    p0 = (wb[hand][0][3], wb[hand][1][3], wb[hand][2][3])
    p1 = ft.apply(S6[hand], p0)
    rise = p1[2] - p0[2]
    check(rise > 0.05, "left hand rises by %.3f m at the peak frame" % rise)
    pr = ft.apply(S6[idx["hand_r"]], (wb[idx["hand_r"]][0][3], wb[idx["hand_r"]][1][3], wb[idx["hand_r"]][2][3]))
    check(pr[2] - wb[idx["hand_r"]][2][3] > 0.05, "right hand rises too (sign by lateral position)")
    pf = ft.apply(S6[idx["foot_l"]], (wb[idx["foot_l"]][0][3], wb[idx["foot_l"]][1][3], wb[idx["foot_l"]][2][3]))
    check(math.dist(pf, (wb[idx["foot_l"]][0][3], wb[idx["foot_l"]][1][3], wb[idx["foot_l"]][2][3])) > 0.0, "the thigh swing moves the foot")
    shutil.rmtree(d, ignore_errors=True)
    print("SELFTEST_FAILS %d" % len(fails))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
