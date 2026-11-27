#!/usr/bin/env python3
"""Blender script: the OFFLINE RENDER ADAPTER of M5-C.  It renders a runtime-evaluated performance tape
(`gibson.figure.tape.v1`, docs/design/m5c-studio-api.md section 5) with the exported body.  UNRUN when committed
(heavy-only): written against the Blender 4.5 API and the MPFB 2.0.18 services that export_mpfb.py already uses.

    "$BLENDER" -b --python tools/figure_assets/render_tape.py -- \
        --export OUT/export --tape PATH.tape.json --preset tools/figure_assets/presets/generic_neutral.json \
        --shots tools/figure_assets/shots/ex1_character.json --out OUT/render [--spp 32] [--engine auto|eevee|cycles]
        [--checks-only] [--mutant bad_bounds] [--clip-frames N] [--no-stills] [--no-clips] [--oracle-every 30]
        [--joint-frame world_aligned|bone]

What it does
  1. rebuilds the MPFB human (HumanService, same macro as the export), puts the SHAPED positions on it (neutral + the tape
     header's control values through the export's deltas, unclamped, re-grounded), adds the rig exactly like the exporter,
     deletes the helper geometry;
  2. fits only the garments whose `fit_status` verdict is Fitted (the exported mhclo binding evaluated on the shaped body);
     any other garment gets a visible text label on every image saying why it is not drawn;
  3. builds the self-authored skirt (fa_skirt.py) when the tape lists `skirt_a_line` as Fitted: smooth subdivided panels,
     bound to pelvis/thigh bones, with a deterministic bounded lag term;
  4. sets every pose bone for every frame from the tape locals (pose bone matrix_basis, no keyframes, no interpolation);
  5. runs the self-checks (contact, contact shadow, framing, eye clearance, brow/lash visibility, pose-math) and renders stills
     and clips per the shots file; writes render.json and oracle_posed.json (+ .f32.bin) for the Blender-vs-libgibson oracle.

Output lines (grep 'RT '): RT_CHECK PASS|FAIL <name> ..., RT_STEP ..., RT_DONE ..., and the final `RENDER_TAPE_RC <n>`.
Exit codes: 0 all checks passed, 2 bad arguments / inputs, 3 exception, 5 renders/outputs written but a self-check FAILED.

Honest limits: the joint-frame convention of the tape (world-aligned vs bone-oriented rest rotations) is an assumption
documented in fa_tape.py; the oracle is what can falsify it.  Thresholds of the checks are first guesses.
"""
import argparse
import hashlib
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
import bmesh  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402

import fa_checks as ck  # noqa: E402
import fa_common as fc  # noqa: E402
import fa_hair  # noqa: E402
import fa_skirt  # noqa: E402
import fa_stage as st  # noqa: E402
import fa_tape as ft  # noqa: E402

SKIRT_ID = "skirt_a_line"
LAG_FRAMES = 6
say = st.say


def parse_args(argv):
    ap = argparse.ArgumentParser(prog="render_tape.py")
    ap.add_argument("--export", default=None, help="export directory written by export_mpfb.py (for THIS preset's macro)")
    ap.add_argument("--tape", default=None, help="PATH.tape.json (the .tape.bin sits next to it)")
    ap.add_argument("--preset", default=None, help="preset JSON: macro/controls/materials/garments/hair")
    ap.add_argument("--shots", default=None, help="shots JSON (tools/figure_assets/shots/*.json)")
    ap.add_argument("--out", default=None)
    ap.add_argument("--spp", type=int, default=32)
    ap.add_argument("--engine", default="auto", choices=("auto", "eevee", "cycles"))
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--mutant", default=None, choices=(None, "bad_bounds"))
    ap.add_argument("--checks-only", action="store_true", help="build, run the self-checks, write render.json; no stills/clips")
    ap.add_argument("--no-stills", action="store_true")
    ap.add_argument("--no-clips", action="store_true")
    ap.add_argument("--clip-frames", type=int, default=0, help="render only N frames of every clip (smoke runs, split leases); 0 = all")
    ap.add_argument("--clip-from", type=int, default=0, help="first clip frame index to render (with --clip-frames, to split a clip over leases)")
    ap.add_argument("--oracle-every", type=int, default=30)
    ap.add_argument("--no-oracle", action="store_true")
    ap.add_argument("--joint-frame", default="world_aligned", choices=("world_aligned", "bone"))
    ap.add_argument("--child-smoke", action="store_true", help="internal: EEVEE smoke test")
    return ap.parse_args(argv)


def load_json(p):
    with open(p, "r", encoding="utf-8") as f:
        return json.load(f)


def verdict_of(v):
    """fit_status values may be a string or a one-key object; -> (is_fitted, text)."""
    if isinstance(v, dict) and v:
        k = sorted(v)[0]
        txt = "%s %s" % (k, json.dumps(v[k], sort_keys=True)[:60]) if v[k] not in (None, {}, []) else k
    else:
        txt = str(v)
    return txt.strip().lower().startswith("fitted"), txt


# =========================================================================== figure

class Fig:
    pass


def read_att(export_dir, name):
    p = os.path.join(export_dir, "attachments", name + ".json")
    if not os.path.isfile(p):
        return None
    h = load_json(p)
    b = h["blobs"]
    rd = lambda key, dt: ft.read_blob(export_dir, b[key], dt)
    rec = {"hdr": h, "name": name, "kind": h["kind"]}
    n = h["n_vertices"]
    bi, bw, bo = rd("bind_idx", "u32"), rd("bind_w", "f32"), rd("bind_off", "f32")
    rec["bind"] = [(bi[3 * i], bi[3 * i + 1], bi[3 * i + 2], bw[3 * i], bw[3 * i + 1], bw[3 * i + 2], bo[3 * i], bo[3 * i + 1], bo[3 * i + 2]) for i in range(n)]
    tr = rd("tris", "u32")
    rec["polys"] = [(tr[3 * i], tr[3 * i + 1], tr[3 * i + 2]) for i in range(len(tr) // 3)]
    if "tri_uv" in b:
        tu, uv = rd("tri_uv", "u32"), rd("uv", "f32")
        rec["uv_polys"] = [(tu[3 * i], tu[3 * i + 1], tu[3 * i + 2]) for i in range(len(tu) // 3)]
        rec["uvs"] = [(uv[2 * i], uv[2 * i + 1]) for i in range(len(uv) // 2)]
    else:
        rec["uv_polys"], rec["uvs"] = None, None
    si, sw = rd("skin_idx", "u16"), rd("skin_w", "f32")
    rec["skin_idx"] = [(si[4 * i], si[4 * i + 1], si[4 * i + 2], si[4 * i + 3]) for i in range(n)]
    rec["skin_w"] = [(sw[4 * i], sw[4 * i + 1], sw[4 * i + 2], sw[4 * i + 3]) for i in range(n)]
    dv = rd("delete_verts", "u32")
    rec["delete_verts"] = [i for i in dv if i < fc.N_BODY]
    rec["offset_spec"] = h["offset_spec"]
    return rec


def build_figure(a, tape, preset, mutant, notes):
    import export_mpfb as ex
    export_dir = a.export
    hdr = load_json(os.path.join(export_dir, "figure_asset.json"))
    skel = load_json(os.path.join(export_dir, "skeleton.json"))
    landmarks = load_json(os.path.join(export_dir, "landmarks.json"))
    names = [b["name"] for b in skel["bones"]]
    fig = Fig()
    fig.names, fig.skel, fig.hdr = names, skel, hdr
    controls = dict(tape.header["controls"])
    hp = hdr["preset"].get("controls", {})
    S, shift, warns = ft.shape_positions(export_dir, controls)
    for w in warns:
        notes.append(w)
    fig.S, fig.shift = S, shift
    fig.controls = controls

    B = ex.Blender(None, None)
    macro = B.default_macro()
    for k, v in hdr["preset"].get("macro", {}).items():
        if k == "race":
            macro["race"].update(v)
        else:
            macro[k] = v
    rp_macro = preset.get("macro")
    if rp_macro is not None and rp_macro != hdr["preset"].get("macro"):
        raise RuntimeError("the --preset macro differs from the macro this --export was made with; export with this preset first")
    B.clear_scene()
    obj = B.build_human(macro)
    P0 = B.positions(obj)
    base = ft.read_blob(export_dir, hdr["blobs"]["base.positions"]["file"], "f32")
    sh0 = hdr["ground_shift_z_neutral"]
    err = max(max(abs(P0[i][0] - base[3 * i]), abs(P0[i][1] - base[3 * i + 1]), abs(P0[i][2] + sh0 - base[3 * i + 2])) for i in range(0, len(P0), 7))
    fig.neutral_match_err_m = err
    me = obj.data
    if len(me.vertices) != len(S):
        raise RuntimeError("MPFB human has %d vertices, shaped positions have %d" % (len(me.vertices), len(S)))
    if me.shape_keys:
        for kb in reversed(list(me.shape_keys.key_blocks)):
            obj.shape_key_remove(kb)
    me.vertices.foreach_set("co", [c for p in S for c in p])
    me.update()
    arm = B.add_rig(obj, hdr["rig"]["name"])
    arm.data.pose_position = "POSE"
    mw = arm.matrix_world
    if any(abs(mw[r][c] - (1.0 if r == c else 0.0)) > 1e-6 for r in range(4) for c in range(4)):
        raise RuntimeError("armature object has a non-identity world matrix; the adapter assumes armature space == world")
    bn = {b.name for b in arm.data.bones}
    if bn != set(names):
        raise RuntimeError("armature bones differ from skeleton.json: only-in-blender=%s only-in-export=%s" % (sorted(bn - set(names))[:6], sorted(set(names) - bn)[:6]))
    miss = [j for j in tape.header["joints"] if j not in bn]
    if miss:
        raise RuntimeError("tape joints not in the rig: %s" % miss[:8])
    # delete helper geometry (the bounds must never see it), keep vertex groups
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.verts.ensure_lookup_table()
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if v.index >= fc.N_BODY], context="VERTS")
    bm.to_mesh(me)
    bm.free()
    me.update()
    for p in me.polygons:
        p.use_smooth = True
    obj.name = "Body"
    obj["fa_role"] = "body"
    for m in list(obj.modifiers):
        if m.type != "ARMATURE":
            notes.append("removed unexpected modifier %s (%s) from the body" % (m.name, m.type))
            obj.modifiers.remove(m)
    if not any(m.type == "ARMATURE" for m in obj.modifiers):
        st.add_armature_modifier(obj, arm)
    fig.body, fig.arm = obj, arm
    fig.rest_M = {b.name: b.matrix_local.copy() for b in arm.data.bones}
    wbind, heads = ft.bind_frames(skel, S, a.joint_frame, export_dir)
    fig.wbind = wbind
    hd = max(math.dist(heads[i], tuple(arm.data.bones[names[i]].head_local)) for i in range(len(names)))
    fig.rig_head_err_m = hd
    fig.pos = {n: i for i, n in enumerate(names)}
    # foot vertices: dominant bone is a foot/ball/toe bone
    si = ft.read_blob(export_dir, hdr["blobs"]["skin.idx"]["file"], "u16")
    sw = ft.read_blob(export_dir, hdr["blobs"]["skin.w"]["file"], "f32")
    fig.skin_idx = [(si[4 * i], si[4 * i + 1], si[4 * i + 2], si[4 * i + 3]) for i in range(fc.N_BODY)]
    fig.skin_w = [(sw[4 * i], sw[4 * i + 1], sw[4 * i + 2], sw[4 * i + 3]) for i in range(fc.N_BODY)]
    dom = [names[fig.skin_idx[i][max(range(4), key=lambda k: fig.skin_w[i][k])]] for i in range(fc.N_BODY)]
    fig.foot_idx = [i for i in range(fc.N_BODY) if re.match(st.FOOT_RE, dom[i])]
    fig.arm_mask = [bool(re.search(st.ARM_RE, dom[i])) for i in range(fc.N_BODY)]
    if not fig.foot_idx:
        raise RuntimeError("no foot vertices found by dominant bone (rig names %s ...)" % names[:6])
    cen = lambda n: fc.centroid(S, landmarks["joints"][n])
    fig.eye_l, fig.eye_r = cen("joint-l-eye"), cen("joint-r-eye")
    fig.landmarks = landmarks
    fig.objs = {"body": [obj], "hair": [], "garment": [], "eyes": [], "brow": [], "lash": [], "skirt": []}
    fig.labels = []
    fig.garment_report = {}
    rd = preset.get("render", {})
    # ---- materials (tape header wins over the preset)
    slots = dict(rd.get("materials", {}))
    for k, v in tape.header.get("materials", {}).items():
        slots[k] = v
    fig.slot_specs = slots
    bind = rd.get("bindings", {"garment": "base", "skirt": "panel"})
    skin_mat, sss = st.skin_material(rd.get("skin", {}))
    fig.sss_inputs = sss
    obj.data.materials.clear()
    obj.data.materials.append(skin_mat)
    fig.skin_rgb = rd.get("skin", {}).get("albedo", [0.62, 0.40, 0.31])
    hair_cfg = rd.get("hair", {})
    hair_rgb = hair_cfg.get("color", [0.09, 0.05, 0.03])
    brow_rgb = rd.get("brows", {}).get("color", [0.025, 0.015, 0.01])
    iris_spec = slots.get("iris", {})
    iris_rgb = iris_spec.get("albedo") or iris_spec.get("color") or rd.get("eyes", {}).get("iris", [0.20, 0.30, 0.42])
    fig.hair_rgb, fig.brow_rgb = hair_rgb, brow_rgb

    def attach(name, role_key, mat, post=None):
        rec = read_att(export_dir, name)
        if rec is None:
            return None
        pos = fc.fit_binding(rec["bind"], rec["offset_spec"], S)
        info = {}
        if post is not None:
            pos, info = post(pos)
        o = st.mesh_object(name, pos, rec["polys"], rec["uvs"], rec["uv_polys"], role=role_key)
        st.set_weights(o, [[names[i] for i in row] for row in rec["skin_idx"]], rec["skin_w"])
        st.add_armature_modifier(o, arm)
        o.data.materials.append(mat)
        return o, rec, pos, info

    # hair (parted away from the eyes)
    hid = hair_cfg.get("id")
    if hid:
        r = attach(hid, "hair", st.hair_material(hair_rgb), lambda p: fa_hair.part_hair(p, fig.eye_l, fig.eye_r))
        if r is None:
            notes.append("hair %s is not in the export attachments (export with this preset)" % hid)
            fig.garment_report["hair:" + hid] = "NOT DRAWN: not in export"
        else:
            fig.objs["hair"].append(r[0])
            fig.hair_part = r[3]
            fig.hair_pos_violations = fa_hair.clearance_violations(r[2], fig.eye_l, fig.eye_r)
    for kind, key, mat in (("eyes", "eyes", st.eye_material(iris_rgb)), ("eyebrows", "brow", st.brow_material(brow_rgb, "brow")),
                           ("eyelashes", "lash", st.brow_material([0.012, 0.01, 0.01], "lash"))):
        nm = rd.get(kind, {}).get("id") or {"eyes": "low-poly", "eyebrows": "eyebrow001", "eyelashes": "eyelashes01"}[kind]
        r = attach(nm, key, mat)
        if r is None:
            notes.append("%s %s is not in the export attachments" % (kind, nm))
            continue
        o, rec, pos, _ = r
        fig.objs[key].append(o)
        if key == "eyes":
            fig.iris_verts = st.add_iris_attribute(o, pos)
    # garments: only Fitted verdicts are drawn
    fit_status = tape.header.get("fit_status", {})
    hide = set()
    for gid in tape.header.get("garments", []):
        fitted, why = verdict_of(fit_status.get(gid, "MissingVerdict"))
        if gid not in fit_status:
            fig.garment_report[gid] = "NOT DRAWN: no fit_status verdict in the tape (a missing verdict means not drawn)"
        elif not fitted:
            fig.garment_report[gid] = "NOT DRAWN: FitStatus %s" % why
        elif gid == SKIRT_ID:
            fig.garment_report[gid] = "DRAWN (self-authored skirt)"
        else:
            slot = bind.get("garment:" + gid, bind.get("garment", "base"))
            spec = slots.get(slot, {"albedo": [0.20, 0.38, 0.42], "roughness": 0.7})
            r = attach(gid, "garment", st.slot_material(slot, spec))
            if r is None:
                fig.garment_report[gid] = "NOT DRAWN: not in the export attachments (export it with this preset)"
            else:
                fig.objs["garment"].append(r[0])
                fig.garment_report[gid] = "DRAWN (fit %s, slot %s)" % (why, slot)
                if preset.get("hide_body_under_garment", True):
                    hide.update(r[1]["delete_verts"])
    if hide:
        vg = obj.vertex_groups.new(name="fa_under_garment")
        vg.add(sorted(hide), 1.0, "REPLACE")
        md = obj.modifiers.new("HideUnderGarment", "MASK")
        md.vertex_group = "fa_under_garment"
        md.invert_vertex_group = True
        md.show_viewport = False        # the depsgraph (bounds, ray casts, oracle) sees the whole body; the render hides it
    fig.hidden_body_vertices = len(hide)
    # skirt
    fig.skirt = None
    if SKIRT_ID in tape.header.get("garments", []) and fig.garment_report.get(SKIRT_ID, "").startswith("DRAWN"):
        sk = fa_skirt.make_skirt(S, landmarks["joints"], exclude=fig.arm_mask)
        o = st.mesh_object("Skirt", sk["positions"], sk["quads"], sk["uvs"], sk["quad_uv"], role="skirt")
        slot = bind.get("skirt", "panel")
        o.data.materials.append(st.slot_material(slot, slots.get(slot, {"albedo": [0.55, 0.18, 0.24], "roughness": 0.75})))
        so = o.modifiers.new("Solidify", "SOLIDIFY")
        so.thickness = 0.004
        so.offset = -1.0
        sd = o.modifiers.new("Subsurf", "SUBSURF")
        sd.levels = 1
        sd.render_levels = 2
        fig.objs["skirt"].append(o)
        fig.skirt = {"obj": o, "data": sk, "cache": {}}
    if fig.garment_report:
        for gid, txt in sorted(fig.garment_report.items()):
            if txt.startswith("NOT DRAWN"):
                lab = st.build_label("%s: %s" % (gid, txt))
                fig.labels.append(lab)
    return fig


def figure_objects(fig):
    out = []
    for k in ("body", "hair", "garment", "eyes", "brow", "lash", "skirt"):
        out.extend(fig.objs[k])
    return out


# =========================================================================== pose

_S_CACHE = {}


def skin_for(fig, tape, f):
    if f not in _S_CACHE:
        if len(_S_CACHE) > 64:
            _S_CACHE.clear()
        _S_CACHE[f] = ft.skin_matrices(fig.skel, fig.wbind, tape, f)
    return _S_CACHE[f]


def set_rest_pose(fig):
    for pb in fig.arm.pose.bones:
        pb.matrix_basis = Matrix.Identity(4)
    if fig.skirt is not None:
        st.set_vertex_positions(fig.skirt["obj"], fig.skirt["data"]["positions"])
    bpy.context.view_layer.update()


def set_pose(fig, tape, f):
    Sm = skin_for(fig, tape, f)
    for i, name in enumerate(fig.names):
        par = fig.skel["bones"][i]["parent"]
        Sb = Matrix(Sm[i])
        Sp = Matrix(Sm[par]) if par >= 0 else Matrix.Identity(4)
        M = fig.rest_M[name]
        fig.arm.pose.bones[name].matrix_basis = M.inverted() @ Sp.inverted() @ Sb @ M
    if fig.skirt is not None:
        hist = []
        for k in range(LAG_FRAMES + 1):
            g = max(0, f - LAG_FRAMES + k)
            if g not in fig.skirt["cache"]:
                Sg = skin_for(fig, tape, g)
                three = {b: Sg[fig.pos[b]] for b in fa_skirt.BONES}
                fig.skirt["cache"][g] = fa_skirt.pose(fig.skirt["data"]["positions"], fig.skirt["data"]["weights"], three)
                if len(fig.skirt["cache"]) > 40:
                    fig.skirt["cache"].pop(min(fig.skirt["cache"]))
            hist.append(fig.skirt["cache"][g])
        st.set_vertex_positions(fig.skirt["obj"], fa_skirt.swing(hist, fig.skirt["data"]["t"]))
    bpy.context.view_layer.update()


def pelvis_world(fig, tape, f):
    Sm = skin_for(fig, tape, f)
    i = fig.pos.get("pelvis", 0)
    h = fig.wbind[i]
    return ft.apply(Sm[i], (h[0][3], h[1][3], h[2][3]))


# =========================================================================== checks

class Checks:
    def __init__(self):
        self.items = []

    def add(self, r, detail=""):
        self.items.append(r)
        say("RT_CHECK %s %s %s %s" % ("PASS" if r["ok"] else "FAIL", r["name"],
                                      " ".join("%s=%s" % (k, ("%.4g" % v) if isinstance(v, float) else v) for k, v in sorted(r.items()) if k not in ("name", "ok") and not isinstance(v, (list, dict))),
                                      detail))
        return r["ok"]

    def failed(self):
        return [r["name"] for r in self.items if not r["ok"]]


def body_ndc(cam, fig, pts=None):
    return st.project(cam, pts if pts is not None else st.eval_positions(fig.body))


def eye_clearance(cam, fig, name="eye_clearance"):
    blocked, total = 0, 0
    for e in (fig.eye_l, fig.eye_r):
        for dx, dz in ((0, 0), (0.008, 0.0), (-0.008, 0.0), (0.0, 0.006)):
            total += 1
            role, hitd, d = st.ray_first_hit(cam.location, (e[0] + dx, e[1] - 0.012, e[2] + dz))
            if role in ("hair", "garment", "skirt"):
                blocked += 1
    return ck.clearance_check(blocked, total, name)


def brow_lash_visibility(cam, fig):
    vis, total = 0, 0
    for key in ("brow", "lash"):
        for o in fig.objs[key]:
            pts = st.eval_positions(o)
            for i in range(0, len(pts), max(1, len(pts) // 24)):
                total += 1
                role, hitd, d = st.ray_first_hit(cam.location, pts[i])
                if role in (None, "brow", "lash") or hitd >= d - 0.004:
                    vis += 1
    return ck.visibility_check(vis, total, 0.6, "brows_lashes_visible")


def run_checks(a, fig, tape, cams, stage, checks, front_rec, res_check, info, out_dir):
    ground = stage["ground"]
    set_rest_pose(fig)
    pts_rest = st.eval_positions(fig.body)
    foot = [pts_rest[i] for i in fig.foot_idx]
    checks.add(ck.contact_check(min(p[2] for p in foot), ground.location.z))
    # contact shadow on a dedicated check render of the front camera
    sc = bpy.context.scene
    cam = front_rec["obj"]
    sc.camera = cam
    for lab in fig.labels:
        st.attach_label_to_camera(lab, cam, front_rec["lens"])
    fpath = os.path.join(out_dir, "check_front_beauty.png")
    mpath = os.path.join(out_dir, "check_front_mask.png")
    spp_old = None
    try:
        spp_old = sc.eevee.taa_render_samples
        sc.eevee.taa_render_samples = min(spp_old, 16)
    except Exception:  # noqa: BLE001
        pass
    st.render_to(fpath, res_check)
    if spp_old is not None:
        sc.eevee.taa_render_samples = spp_old
    st.render_mask(mpath, res_check, figure_objects(fig))
    sc.render.resolution_x, sc.render.resolution_y = res_check
    inner, outer = st.ground_rects(cam, [(p[0], p[1]) for p in foot], ground.location.z)
    r = ck.shadow_check_files(fpath, mpath, inner, outer)
    r["inner_rect"], r["outer_rect"] = list(inner), list(outer)
    checks.add(r)
    checks.add(ck.contrast_check(st.luminance_linear(fig.brow_rgb), st.luminance_linear(fig.skin_rgb), name="brow_vs_skin_contrast"))
    checks.add(eye_clearance(cam, fig, "eye_clearance:front"))
    face = info.get("face_cam")
    if face is not None:
        checks.add(eye_clearance(face["obj"], fig, "eye_clearance:face_closeup"))
        checks.add(brow_lash_visibility(face["obj"], fig))
    # pose math: Blender evaluated skinning vs pure LBS of the exported top-4 weights, on up to 3 frames
    P_all = fig.S
    worst, mean_acc, n_acc = 0.0, 0.0, 0
    frames = sorted({0, tape.frame_count // 2, tape.frame_count - 1})
    for f in frames:
        set_pose(fig, tape, f)
        bl = st.eval_positions(fig.body)
        ref = ft.lbs_positions(P_all[:fc.N_BODY], fig.skin_idx, fig.skin_w, skin_for(fig, tape, f))
        errs = [math.dist(x, y) for x, y in zip(bl, ref)]
        worst = max(worst, max(errs))
        mean_acc += sum(errs)
        n_acc += len(errs)
    set_rest_pose(fig)
    checks.add({"name": "pose_math", "ok": worst < 0.03 and mean_acc / n_acc < 0.004, "max_err_m": worst, "mean_err_m": mean_acc / n_acc,
                "frames": frames, "note": "Blender armature skinning vs pure top-4 LBS from the same skin matrices"})
    checks.add({"name": "garments_drawn_or_labelled", "ok": all((t.startswith("DRAWN") or t.startswith("NOT DRAWN")) for t in fig.garment_report.values()),
                "report": dict(fig.garment_report), "labels": len(fig.labels)})


# =========================================================================== main

def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    a = parse_args(argv)
    if a.child_smoke:
        return st.eevee_smoke()
    for k in ("export", "tape", "preset", "shots", "out"):
        if not getattr(a, k):
            say("ERROR: --%s is required" % k)
            return 2
    for k in ("export", "tape", "preset", "shots"):
        if not os.path.exists(getattr(a, k)):
            say("ERROR: --%s path does not exist: %s" % (k, os.path.basename(getattr(a, k))))
            return 2
    out = a.out
    os.makedirs(out, exist_ok=True)
    t_all = time.time()
    notes = []
    tape = ft.read_tape(a.tape)
    preset = load_json(a.preset)
    shots = load_json(a.shots)
    mutant = a.mutant == "bad_bounds"
    timings = {}

    want = a.engine
    engine_note = ""
    if want in ("auto", "eevee"):
        ok, why = st.eevee_usable(__file__)
        engine_note = "eevee smoke: " + why
        want = "EEVEE" if ok else ("CYCLES" if a.engine == "auto" else "EEVEE")
    else:
        want = "CYCLES"

    t = time.time()
    fig = build_figure(a, tape, preset, mutant, notes)
    timings["build_figure_s"] = time.time() - t
    say("RT_STEP figure built neutral_match_err_m=%.3g rig_head_err_m=%.3g foot_vertices=%d hidden_under_garment=%d" % (
        fig.neutral_match_err_m, fig.rig_head_err_m, len(fig.foot_idx), fig.hidden_body_vertices))

    # ---- bounds (the probe bug lives here) and stage
    set_rest_pose(fig)
    rest_pts = st.eval_positions(fig.body)
    lo_t = Vector((min(p[0] for p in rest_pts), min(p[1] for p in rest_pts), min(p[2] for p in rest_pts)))
    hi_t = Vector((max(p[0] for p in rest_pts), max(p[1] for p in rest_pts), max(p[2] for p in rest_pts)))
    if mutant:
        st.add_hidden_helper_proxy(lo_t.z)
        lo, hi = st.mesh_bounds_all(include_hidden=True)
        notes.append("MUTANT bad_bounds: bounds taken over every mesh object including a hidden helper proxy (%.2f..%.2f m)" % (lo.z, hi.z))
    else:
        lo, hi = lo_t, hi_t
    height = hi.z - lo.z
    center = (0.5 * (lo.x + hi.x), 0.5 * (lo.y + hi.y), 0.5 * (lo.z + hi.z))
    ground = st.make_ground(lo.z)
    st.make_world()
    lights = st.add_lights((0.0, 0.0, 0.5 * (lo.z + hi.z)), hi.z - lo.z if not mutant else (hi_t.z - lo_t.z))
    stage = {"ground": ground, "lights": lights}
    info = engine_info = st.set_engine(want, a.spp, a.threads)
    sc = bpy.context.scene
    ctx = {"points": rest_pts[::5], "center": (0.5 * (lo_t.x + hi_t.x), 0.5 * (lo_t.y + hi_t.y), 0.5 * (lo_t.z + hi_t.z)), "height": hi_t.z - lo_t.z,
           "bounds": (lo, hi), "mutant": mutant}
    ez = 0.5 * (fig.eye_l[2] + fig.eye_r[2])
    ctx["face_center"] = (0.0, 0.5 * (fig.eye_l[1] + fig.eye_r[1]), ez - 0.02)
    ctx["face_points"] = [p for p in rest_pts if p[2] > ez - 0.14 and abs(p[0]) < 0.14]

    # ---- cameras from the shots file
    cams = {}
    still_specs = [] if a.no_stills else shots.get("stills", [])
    clip_specs = [] if a.no_clips else shots.get("clips", [])
    res_default = tuple(shots.get("still_res", [1920, 1080]))
    for s in shots.get("stills", []):
        r = tuple(s.get("res", res_default))
        cams["still:" + s["name"]] = st.fit_camera("cam_still_" + s["name"], s["camera"], ctx, r)
        cams["still:" + s["name"]]["tape_frame"] = int(s.get("tape_frame", 0))
    for c in shots.get("clips", []):
        r = tuple(c.get("res", [1280, 720]))
        cmap = c.get("cameras") or {"main": c["camera"]}
        for cid, spec in cmap.items():
            cams["clip:%s:%s" % (c["name"], cid)] = st.fit_camera("cam_clip_%s_%s" % (c["name"], cid), spec, ctx, r)
            cams["clip:%s:%s" % (c["name"], cid)]["tape_frame"] = int(c.get("tape_start", 0))
    front_spec = {"kind": "front"}
    # the check render frames like the front still, at the check resolution
    front_rec = st.fit_camera("cam_check_front", next((s["camera"] for s in shots.get("stills", []) if s["camera"].get("kind") == "front"), front_spec), ctx, (960, 540))
    face_rec = next((v for k, v in cams.items() if v["kind"] == "face_closeup"), None)
    if face_rec is None:
        face_rec = st.fit_camera("cam_check_face", {"kind": "face_closeup"}, ctx, (960, 540))
    info_c = {"face_cam": face_rec}

    checks = Checks()
    t = time.time()
    run_checks(a, fig, tape, cams, stage, checks, front_rec, (960, 540), info_c, out)
    timings["checks_s"] = time.time() - t
    # framing of every shot camera (at the pose of its still / first clip frame)
    for key, rec in cams.items():
        set_pose(fig, tape, rec["tape_frame"])
        sc.render.resolution_x, sc.render.resolution_y = rec["res"]
        if rec["target_kind"] == "face":
            r = ck.framing_check(st.project(rec["obj"], ctx["face_points"]), min_frac=0.45)
        else:
            r = ck.framing_check(st.project(rec["obj"], st.eval_positions(fig.body)))
        r["name"] = "framing:" + key
        checks.add(r)
    set_rest_pose(fig)

    results = {"stills": [], "clips": [], "oracle": None}
    if not a.checks_only:
        # ---- stills
        for s in still_specs:
            rec = cams["still:" + s["name"]]
            r = tuple(s.get("res", res_default))
            f = int(s.get("tape_frame", 0))
            set_pose(fig, tape, f)
            sc.camera = rec["obj"]
            st.cam_pose_at(rec, 0, 1)
            for lab in fig.labels:
                st.attach_label_to_camera(lab, rec["obj"], rec["lens"])
            path = os.path.join(out, "stills", "%s_%s_%dx%d.png" % (shots.get("id", "shots"), s["name"], r[0], r[1]))
            os.makedirs(os.path.dirname(path), exist_ok=True)
            dt = st.render_to(path, r)
            results["stills"].append({"name": s["name"], "path": os.path.relpath(path, out), "res": list(r), "tape_frame": f, "seconds": dt})
            say("RT_STEP still %s %.2fs" % (s["name"], dt))
        # ---- clips
        for c in clip_specs:
            r = tuple(c.get("res", [1280, 720]))
            fps = float(c.get("fps", tape.fps))
            start = int(c.get("tape_start", 0))
            hold = bool(c.get("hold", False))
            n = int(c.get("frames") or max(1, int((tape.frame_count - start) * fps / tape.fps)))
            n_total = n
            lo_i = max(0, a.clip_from)
            hi_i = min(n_total, lo_i + a.clip_frames) if a.clip_frames else n_total
            if c.get("duration_s") and not a.clip_frames:
                dur = n_total / fps
                lo_d, hi_d = c["duration_s"]
                checks.add({"name": "duration:" + c["name"], "ok": lo_d <= dur <= hi_d, "seconds": dur, "min": lo_d, "max": hi_d})
            cmap = c.get("cameras") or {"main": c["camera"]}
            default_id = c.get("default_camera", sorted(cmap)[0])
            cuts = list(c.get("cuts", []))
            fdir = os.path.join(out, "frames", c["name"])
            os.makedirs(fdir, exist_ok=True)
            times, used = [], []
            ema = None
            for i in range(lo_i, hi_i):
                f = start if hold else min(tape.frame_count - 1, start + int(round(i * tape.fps / fps)))
                cid = tape.camera_id_at(f) if c.get("use_tape_camera_cuts", True) else None
                for cut in sorted(cuts, key=lambda q: q["frame"]):
                    if cut["frame"] <= i:
                        cid = cut["camera"]
                if cid not in cmap:
                    cid = default_id
                rec = cams["clip:%s:%s" % (c["name"], cid)]
                set_pose(fig, tape, f)
                pw = None
                if rec["track"]:
                    acc, wsum = [0.0, 0.0], 0.0
                    for k in range(8):
                        g = max(0, f - k)
                        p = pelvis_world(fig, tape, g)
                        w = 0.7 ** k
                        acc[0] += w * p[0]
                        acc[1] += w * p[1]
                        wsum += w
                    pw = (acc[0] / wsum, acc[1] / wsum)
                st.cam_pose_at(rec, i, n_total, pw)
                sc.camera = rec["obj"]
                for lab in fig.labels:
                    st.attach_label_to_camera(lab, rec["obj"], rec["lens"])
                path = os.path.join(fdir, "frame_%05d.png" % i)
                dt = st.render_to(path, r)
                times.append(dt)
                used.append(cid)
                if i % 10 == 0:
                    say("RT_FRAME %s %d/%d tape_frame=%d camera=%s %.2fs" % (c["name"], i, n_total, f, cid, dt))
            switches = sum(1 for x, y in zip(used, used[1:]) if x != y)
            results["clips"].append({"name": c["name"], "frames": hi_i - lo_i, "frames_total": n_total, "first_index": lo_i, "fps": fps, "res": list(r), "dir": os.path.relpath(fdir, out), "seconds_total": sum(times),
                                     "seconds_mean": sum(times) / len(times) if times else 0.0, "camera_switches": switches, "cameras_used": sorted(set(used)),
                                     "tape_start": start, "hold": hold})
            if c.get("min_cuts") is not None and switches < int(c["min_cuts"]):
                checks.add({"name": "camera_cuts:" + c["name"], "ok": False, "switches": switches, "min": int(c["min_cuts"])})
            elif c.get("min_cuts") is not None:
                checks.add({"name": "camera_cuts:" + c["name"], "ok": True, "switches": switches, "min": int(c["min_cuts"])})
    # ---- oracle: posed body vertices for sampled frames
    if not a.no_oracle:
        fr = list(range(0, tape.frame_count, max(1, a.oracle_every)))
        if (tape.frame_count - 1) not in fr:
            fr.append(tape.frame_count - 1)
        buf = bytearray()
        import array
        arr = array.array("f")
        for f in fr:
            set_pose(fig, tape, f)
            for p in st.eval_positions(fig.body):
                arr.extend(p)
        if sys.byteorder == "big":
            arr.byteswap()
        binp = os.path.join(out, "oracle_posed.f32.bin")
        with open(binp, "wb") as fh:
            fh.write(arr.tobytes())
        meta = {"format": "figure-oracle-posed/1", "frames": fr, "n_vertices": fc.N_BODY, "layout": "f32 little-endian [len(frames), n_vertices, 3], Blender frame, metres, armature space == world",
                "bin": "oracle_posed.f32.bin", "bin_sha256": hashlib.sha256(arr.tobytes()).hexdigest(), "joint_frame": a.joint_frame,
                "tape_present_id": tape.header.get("present_id"), "tape_body_rev_id": tape.header.get("body_rev_id"),
                "note": "Body vertices only (indices 0..13379 of the MPFB base mesh), skinned by Blender's armature from the tape locals; compare with Compiled::perform posed vertices."}
        with open(os.path.join(out, "oracle_posed.json"), "w", encoding="utf-8") as fh:
            json.dump(meta, fh, indent=1, sort_keys=True)
            fh.write("\n")
        results["oracle"] = {"frames": fr, "bin_sha256": meta["bin_sha256"]}
        set_rest_pose(fig)
    timings["total_s"] = time.time() - t_all
    failed = checks.failed()
    rj = {"format": "figure-render/1", "engine": engine_info, "engine_note": engine_note, "mutant": a.mutant, "spp": a.spp, "joint_frame": a.joint_frame,
          "tape": {"path_name": os.path.basename(a.tape), "frame_count": tape.frame_count, "fps": tape.fps, "present_id": tape.header.get("present_id"),
                   "body_rev_id": tape.header.get("body_rev_id"), "fit_id": tape.header.get("fit_id"), "controls": tape.header.get("controls"),
                   "fit_status": tape.header.get("fit_status"), "style": tape.header.get("style")},
          "preset": preset.get("name"), "shots": shots.get("id"), "timings": timings, "checks": checks.items, "failed_checks": failed,
          "garments": fig.garment_report, "stage": {"ground_z": ground.location.z, "bounds_lo": list(lo), "bounds_hi": list(hi), "body_height_m": hi_t.z - lo_t.z},
          "figure": {"neutral_match_err_m": fig.neutral_match_err_m, "rig_head_err_m": fig.rig_head_err_m, "hidden_body_vertices": fig.hidden_body_vertices,
                     "sss_inputs": fig.sss_inputs, "hair_part": getattr(fig, "hair_part", None), "hair_violations_in_eye_box": getattr(fig, "hair_pos_violations", None),
                     "iris_vertices": getattr(fig, "iris_verts", None)},
          "results": results, "notes": notes}
    with open(os.path.join(out, "render.json"), "w", encoding="utf-8") as fh:
        json.dump(rj, fh, indent=1, sort_keys=True, default=str)
        fh.write("\n")
    say("RT_DONE checks=%d failed=%d (%s) total_s=%.1f" % (len(checks.items), len(failed), ",".join(failed) or "none", timings["total_s"]))
    return 5 if failed else 0


if __name__ == "__main__":
    try:
        rc = main()
    except SystemExit as e:
        rc = int(e.code or 0)
    except Exception:  # noqa: BLE001
        traceback.print_exc()
        say("RT_FAILED see traceback above")
        rc = 3
    say("RENDER_TAPE_RC %d" % rc)
    sys.exit(rc)
