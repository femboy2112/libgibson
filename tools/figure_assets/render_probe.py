#!/usr/bin/env python3
"""Blender script for the MEASURE lease and its re-runs: render one 1920x1080 still and N frames at 1280x720 of the exported body doing
a simple idle + arm-raise on its rig, print machine-readable timings, and run the defect self-checks.  UNRUN when committed (heavy-only);
written against the Blender 4.5 API.  The animation here is a MEASUREMENT stimulus only: Blender-authored animation never stands in
for the runtime tape (that is render_tape.py).

    "$BLENDER" -b --python tools/figure_assets/render_probe.py -- \
        --export "$OUT/export" --out "$OUT/render" [--engine auto|eevee|cycles] [--spp 32] [--threads 4] [--frames 24]
        [--mutant bad_bounds] [--checks-only] [--no-still]

Input: --export DIR (needs body_shaped.glb for the rigged Body and armature, plus the export's attachments, skeleton, landmarks and
oracle_shaped positions; every other glTF object is discarded and rebuilt from the export through fa_stage.dress, the same code the
adapter uses).  Output lines: PROBE ..., PROBE_FRAME ..., RT_CHECK PASS|FAIL <name> ..., PROBE_RC <n>.
Exit codes: 0 ok, 2 bad arguments / missing input, 3 failure, 5 rendered but a self-check failed.

Fixes of the viewed defects (docs/evidence/m5c/measure/RESULT.md): ground at the min z of the EVALUATED body vertices (the helper
geometry and hidden objects never count) with a contact check and a contact-shadow pixel test; the camera is fitted from the projected
body vertices (figure >= 60% of the frame height, asserted); the fitted garment renders or a visible label says why not; the skirt is
smooth panels skinned to pelvis/thighs with a bounded lag; hair is parted away from the eyes (eye ray check); key/fill/rim + soft top
light, skin with subsurface, dark brows and lashes.  `--mutant bad_bounds` reproduces the probe bug: both checks must then FAIL.
"""
import argparse
import json
import math
import os
import re
import sys
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

import bpy  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402

import fa_checks as ck  # noqa: E402
import fa_common as fc  # noqa: E402
import fa_skirt  # noqa: E402
import fa_stage as st  # noqa: E402
import fa_tape as ft  # noqa: E402

STILL_RES = (1920, 1080)
FRAME_RES = (1280, 720)
say = st.say


def parse_args(argv):
    ap = argparse.ArgumentParser(prog="render_probe.py")
    ap.add_argument("--export", default=None, help="export directory containing body_shaped.glb")
    ap.add_argument("--glb", default=None)
    ap.add_argument("--out", default=None)
    ap.add_argument("--engine", default="auto", choices=("auto", "eevee", "cycles"))
    ap.add_argument("--spp", type=int, default=32)
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--frames", type=int, default=24)
    ap.add_argument("--no-still", action="store_true")
    ap.add_argument("--checks-only", action="store_true")
    ap.add_argument("--mutant", default=None, choices=(None, "bad_bounds"))
    ap.add_argument("--child-smoke", action="store_true", help="internal: EEVEE smoke test, renders 64x64 and exits")
    return ap.parse_args(argv)


def import_glb(path):
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.import_scene.gltf(filepath=path)
    arms = [o for o in bpy.context.scene.objects if o.type == "ARMATURE"]
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if not arms or not meshes:
        raise RuntimeError("glTF import produced %d armatures and %d meshes" % (len(arms), len(meshes)))
    return arms[0], meshes


def pick_body(meshes, n_body):
    cands = [o for o in meshes if o.name.startswith("Body")]
    if not cands:
        cands = sorted(meshes, key=lambda o: -len(o.data.vertices))[:1]
    body = cands[0]
    if len(body.data.vertices) < n_body:
        raise RuntimeError("glTF Body has %d vertices, fewer than the %d body vertices of the export (the importer may duplicate vertices at UV seams, never drop them)" % (len(body.data.vertices), n_body))
    return body


def find_bones(arm):
    res = {}
    for pb in arm.pose.bones:
        n = pb.name.lower()
        m = re.match(r"^upperarm[._]?(l|r)$", n)
        if m:
            res["arm_" + m.group(1)] = pb
        elif n in ("spine_03", "spine03"):
            res["spine"] = pb
    return res


def keyframe_idle_and_arm_raise(arm, n_frames):
    """Armature-space rotations converted to bone-local quaternions (independent of the bone roll)."""
    arm.data.pose_position = "POSE"
    bones = find_bones(arm)
    if "arm_l" not in bones or "arm_r" not in bones:
        raise RuntimeError("no upperarm bones found; bones are: %s" % [b.name for b in arm.pose.bones][:20])
    rest = {}
    wm = arm.matrix_world.to_3x3().normalized()
    for key, pb in bones.items():
        rest[key] = pb.bone.matrix_local.to_3x3()
        pb.rotation_mode = "QUATERNION"
    sc = bpy.context.scene
    sc.frame_start, sc.frame_end = 1, n_frames
    for f in range(1, n_frames + 1):
        t = (f - 1) / float(n_frames)
        lift = 0.5 * (1.0 - math.cos(2.0 * math.pi * t))
        th = math.radians(95.0) * lift
        sway = math.radians(1.5) * math.sin(2.0 * math.pi * t)
        for key, pb in bones.items():
            if key == "arm_l":
                ra = Matrix.Rotation(-th, 3, "Y")
            elif key == "arm_r":
                ra = Matrix.Rotation(th, 3, "Y")
            else:
                ra = Matrix.Rotation(sway, 3, "X")
            ra = wm.inverted() @ ra @ wm
            local = rest[key].inverted() @ ra @ rest[key]
            pb.rotation_quaternion = local.to_quaternion()
            pb.keyframe_insert("rotation_quaternion", frame=f)
    return sorted(bones)


class Skirt:
    """Kinematic skirt for the probe: skin matrices come from the evaluated pose bones; bounded lag over the previous frames."""

    def __init__(self, arm, d):
        self.arm, self.d = arm, d
        self.rest_M = {b.name: b.matrix_local.copy() for b in arm.data.bones}
        self.hist = []

    def _posed(self):
        S = {}
        for b in fa_skirt.BONES:
            pb = self.arm.pose.bones[b]
            m = pb.matrix @ self.rest_M[b].inverted()
            S[b] = [[m[r][c] for c in range(4)] for r in range(4)]
        return fa_skirt.pose(self.d["data"]["positions"], self.d["data"]["weights"], S)

    def update(self, reset=False):
        bpy.context.view_layer.update()
        p = self._posed()
        if reset:
            self.hist = []
        self.hist.append(p)
        self.hist = self.hist[-7:]
        st.set_vertex_positions(self.d["obj"], fa_skirt.swing(self.hist, self.d["data"]["t"]))


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    a = parse_args(argv)
    if a.child_smoke:
        return st.eevee_smoke()
    glb = a.glb or (os.path.join(a.export, "body_shaped.glb") if a.export else None)
    if not a.export or not glb or not os.path.isfile(glb):
        say("ERROR: need --export DIR containing body_shaped.glb (got %r)" % (glb and os.path.basename(glb)))
        return 2
    if not a.out:
        say("ERROR: --out is required")
        return 2
    os.makedirs(a.out, exist_ok=True)
    mutant = a.mutant == "bad_bounds"
    notes = []

    want = a.engine
    engine_note = ""
    if want in ("auto", "eevee"):
        ok, why = st.eevee_usable(__file__)
        engine_note = "eevee smoke: " + why
        want = "EEVEE" if ok else ("CYCLES" if a.engine == "auto" else "EEVEE")
        if not ok and a.engine == "eevee":
            say("PROBE_NOTE --engine eevee requested but the smoke test failed; trying anyway")
    else:
        want = "CYCLES"

    hdr = st.load_json(os.path.join(a.export, "figure_asset.json"))
    skel = st.load_json(os.path.join(a.export, "skeleton.json"))
    landmarks = st.load_json(os.path.join(a.export, "landmarks.json"))
    names = [b["name"] for b in skel["bones"]]
    shaped = ft.read_blob(a.export, hdr["blobs"]["oracle.shaped_positions"]["file"], "f32")
    S = [(shaped[3 * i], shaped[3 * i + 1], shaped[3 * i + 2]) for i in range(len(shaped) // 3)]

    arm, meshes = import_glb(glb)
    body = pick_body(meshes, fc.N_BODY)
    for o in meshes:
        if o is not body:
            bpy.data.objects.remove(o, do_unlink=True)      # rebuilt from the export below, with the shared dressing code
    body["fa_role"] = "body"
    bn = {b.name for b in arm.data.bones}
    if bn != set(names):
        raise RuntimeError("glTF armature bones differ from skeleton.json: only-in-blender=%s only-in-export=%s" % (sorted(bn - set(names))[:6], sorted(set(names) - bn)[:6]))
    gname = {vg.index: vg.name for vg in body.vertex_groups}
    dom = []
    for v in body.data.vertices:
        g = max(v.groups, key=lambda x: x.weight) if v.groups else None
        dom.append(gname.get(g.group, "") if g else "")
    foot_idx = [i for i, n in enumerate(dom) if re.match(st.FOOT_RE, n)]
    si = ft.read_blob(a.export, hdr["blobs"]["skin.idx"]["file"], "u16")      # arm mask by the EXPORT's body vertex index space
    sw = ft.read_blob(a.export, hdr["blobs"]["skin.w"]["file"], "f32")
    arm_mask = [bool(re.search(st.ARM_RE, names[si[4 * i + max(range(4), key=lambda k: sw[4 * i + k])]])) for i in range(fc.N_BODY)]
    if not foot_idx:
        raise RuntimeError("no foot vertices found by dominant vertex group")
    cen = lambda n: fc.centroid(S, landmarks["joints"][n])
    eye_l, eye_r = cen("joint-l-eye"), cen("joint-r-eye")
    rd = dict(hdr["preset"].get("render", {}))
    if "hair" not in rd:
        hs = [x.split(":", 1)[1] for x in hdr["preset"].get("export_attachments", []) if x.startswith("hair:")]
        if hs:
            rd["hair"] = {"id": hs[0], "color": [0.10, 0.06, 0.035]}
    garments = list(rd.get("garments", [x.split(":", 1)[1] for x in hdr["preset"].get("export_attachments", []) if x.startswith("clothes:")]))
    if st.SKIRT_ID not in garments:
        garments.append(st.SKIRT_ID)
    fit_status = {g: "Fitted" for g in garments}
    slots = dict(rd.get("materials", {}))
    D = st.dress(a.export, S, landmarks, arm, names, body, rd, slots, garments, fit_status, False, arm_mask, eye_l, eye_r, notes)
    for n in notes:
        say("PROBE_NOTE " + n)
    keyed = keyframe_idle_and_arm_raise(arm, a.frames)
    sc = bpy.context.scene
    sc.frame_set(1)
    skirt = Skirt(arm, D["skirt"]) if D["skirt"] else None
    if skirt:
        skirt.update(reset=True)

    # ---- bounds, ground, stage (the probe bug lives in the bounds)
    pts = st.eval_positions(body)
    lo_t = Vector((min(p[0] for p in pts), min(p[1] for p in pts), min(p[2] for p in pts)))
    hi_t = Vector((max(p[0] for p in pts), max(p[1] for p in pts), max(p[2] for p in pts)))
    if mutant:
        st.add_hidden_helper_proxy(lo_t.z)
        lo, hi = st.mesh_bounds_all(include_hidden=True)
    else:
        lo, hi = lo_t, hi_t
    ground = st.make_ground(lo.z)
    st.make_world()
    st.add_lights((0.0, 0.0, 0.5 * (lo.z + hi.z)), hi_t.z - lo_t.z if mutant else hi.z - lo.z)
    info = st.set_engine(want, a.spp, a.threads)
    ez = 0.5 * (eye_l[2] + eye_r[2])
    ctx = {"points": pts[::5], "center": (0.5 * (lo_t.x + hi_t.x), 0.5 * (lo_t.y + hi_t.y), 0.5 * (lo_t.z + hi_t.z)), "height": hi_t.z - lo_t.z,
           "bounds": (lo, hi), "mutant": mutant, "face_center": (0.0, 0.5 * (eye_l[1] + eye_r[1]), ez - 0.02),
           "face_points": [p for p in pts if p[2] > ez - 0.14 and abs(p[0]) < 0.14]}
    cam_still = st.fit_camera("probe_cam_still", {"kind": "front"}, ctx, STILL_RES)
    cam_frame = st.fit_camera("probe_cam_frame", {"kind": "front"}, ctx, FRAME_RES)
    cam_check = st.fit_camera("probe_cam_check", {"kind": "front"}, ctx, (960, 540))
    cam_face = st.fit_camera("probe_cam_face", {"kind": "face_closeup"}, ctx, (960, 540))
    figure_objs = [o for k in ("body", "hair", "garment", "eyes", "brow", "lash", "skirt") for o in D["objs"][k]]

    checks = st.Checks()
    t0 = time.time()
    st.check_contact_shadow(checks, body, foot_idx, ground, cam_check, (960, 540), a.out, figure_objs, D["labels"])
    for key, rec in (("still", cam_still), ("frame", cam_frame)):
        sc.render.resolution_x, sc.render.resolution_y = rec["res"]
        r = ck.framing_check(st.project(rec["obj"], st.eval_positions(body)))
        r["name"] = "framing:" + key
        checks.add(r)
    sc.render.resolution_x, sc.render.resolution_y = (960, 540)
    r = ck.framing_check(st.project(cam_face["obj"], ctx["face_points"]), min_frac=0.45)
    r["name"] = "framing:face_closeup"
    checks.add(r)
    checks.add(ck.contrast_check(st.luminance_linear(D["brow_rgb"]), st.luminance_linear(D["skin_rgb"]), name="brow_vs_skin_contrast"))
    checks.add(st.eye_clearance(cam_check["obj"], eye_l, eye_r, "eye_clearance:front"))
    checks.add(st.eye_clearance(cam_face["obj"], eye_l, eye_r, "eye_clearance:face_closeup"))
    checks.add(st.brow_lash_visibility(cam_face["obj"], D["objs"]["brow"], D["objs"]["lash"]))
    checks.add({"name": "garments_drawn_or_labelled", "ok": all(t.startswith(("DRAWN", "NOT DRAWN")) for t in D["report"].values()),
                "report": dict(D["report"]), "labels": len(D["labels"])})
    check_s = time.time() - t0

    still_s = 0.0
    times = []
    if not a.checks_only:
        if not a.no_still:
            sc.frame_set(1)
            if skirt:
                skirt.update(reset=True)
            sc.camera = cam_still["obj"]
            for lab in D["labels"]:
                st.attach_label_to_camera(lab, cam_still["obj"], cam_still["lens"])
            still_s = st.render_to(os.path.join(a.out, "still_1920x1080.png"), STILL_RES)
        sc.camera = cam_frame["obj"]
        for lab in D["labels"]:
            st.attach_label_to_camera(lab, cam_frame["obj"], cam_frame["lens"])
        for f in range(1, a.frames + 1):
            sc.frame_set(f)
            if skirt:
                skirt.update(reset=(f == 1))
            dt = st.render_to(os.path.join(a.out, "frame_%03d_1280x720.png" % f), FRAME_RES)
            times.append(dt)
            say("PROBE_FRAME %d seconds=%.2f" % (f, dt))
    mean = sum(times) / len(times) if times else 0.0
    mx = max(times) if times else 0.0
    failed = checks.failed()
    res = {"engine": info["engine"], "still_s": still_s, "frame_s_mean": mean, "frame_s_max": mx, "frames": len(times),
           "still_res": list(STILL_RES), "frame_res": list(FRAME_RES), "settings": info, "engine_note": engine_note, "mutant": a.mutant,
           "rig_bones_animated": keyed, "checks": checks.items, "failed_checks": failed, "check_seconds": check_s, "garments": D["report"],
           "stage": {"ground_z": ground.location.z, "bounds_lo": list(lo), "bounds_hi": list(hi), "body_height_m": hi_t.z - lo_t.z},
           "notes": notes}
    with open(os.path.join(a.out, "probe.json"), "w", encoding="utf-8") as f:
        json.dump(res, f, indent=1, sort_keys=True, default=str)
        f.write("\n")
    say("PROBE engine=%s still_s=%.2f frame_s_mean=%.2f frame_s_max=%.2f frames=%d samples=%s failed_checks=%s" % (
        info["engine"], still_s, mean, mx, len(times), info.get("samples"), ",".join(failed) or "none"))
    return 5 if failed else 0


if __name__ == "__main__":
    try:
        rc = main()
    except SystemExit as e:
        rc = int(e.code or 0)
    except Exception:  # noqa: BLE001
        traceback.print_exc()
        say("PROBE_FAILED see traceback above")
        rc = 3
    say("PROBE_RC %d" % rc)
    sys.exit(rc)
