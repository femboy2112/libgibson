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
import fa_surface  # noqa: E402
import fa_stage as st  # noqa: E402
import fa_tape as ft  # noqa: E402

LAG_FRAMES = 6
say = st.say


def parse_args(argv):
    ap = argparse.ArgumentParser(prog="render_tape.py")
    ap.add_argument("--export", default=None, help="export directory written by export_mpfb.py (for THIS preset's macro)")
    ap.add_argument("--tape", default=None, help="PATH.tape.json (the .tape.bin sits next to it)")
    ap.add_argument("--preset", default=None, help="preset JSON: macro/controls/materials/garments/hair")
    ap.add_argument("--shots", default=None, help="shots JSON (tools/figure_assets/shots/*.json)")
    ap.add_argument("--out", default=None)
    ap.add_argument("--matter", help="versioned Living Matter world-space bake manifest")
    ap.add_argument("--scene", help="versioned scene and prop manifest")
    ap.add_argument("--fitted", help="rest garment positions from the same compiled recipe as the tape")
    ap.add_argument("--still", action="append", help="render just this named still (repeatable); check every camera")
    ap.add_argument("--spp", type=int, default=32)
    ap.add_argument("--engine", default="auto", choices=("auto", "eevee", "cycles"))
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--mutant", default=None, choices=(None, "bad_bounds", "overexposed"))
    ap.add_argument("--checks-only", action="store_true", help="build, run the self-checks, write render.json; no stills/clips")
    ap.add_argument("--no-stills", action="store_true")
    ap.add_argument("--no-clips", action="store_true")
    ap.add_argument("--batch-clips", action="store_true", help="render each requested clip slice as a single timed Blender animation")
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


def load_fitted(path, tape, export_dir):
    """Refuse fitted geometry from another recipe or another version of the body export."""
    if path is None:
        return None
    fit = load_json(path)
    manifest_sha = fc.sha256_file(os.path.join(export_dir, "manifest.json"))
    if manifest_sha != tape.header["asset_manifest_sha256"]:
        raise ValueError("the tape was compiled against a different body export")
    for key in ("asset_manifest_sha256", "fit_id", "body_rev_id", "present_id"):
        if fit[key] != tape.header[key]:
            raise ValueError(f"fitted garment {key} differs from tape")
    if fit["tape_bin_sha256"] != tape.header["bin_sha256"]:
        raise ValueError("fitted geometry receipt refers to another tape")
    permitted = {g for g in tape.header["garments"] if st.verdict_of(tape.header["fit_status"].get(g))[0]}
    if set(fit["positions"]) != permitted:
        raise ValueError("garment positions do not match the tape's Fitted set")
    return fit["positions"]


class Fig:
    pass


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
    rd = preset.get("render", {})
    slots = dict(rd.get("materials", {}))
    for k, v in tape.header.get("materials", {}).items():       # the tape header wins over the preset
        slots[k] = v
    fig.slot_specs = slots
    D = st.dress(export_dir, S, landmarks, arm, names, obj, rd, slots, tape.header.get("garments", []), tape.header.get("fit_status", {}),
                 preset.get("hide_body_under_garment", True), fig.arm_mask, fig.eye_l, fig.eye_r, notes,
                 fitted_positions=load_fitted(a.fitted, tape, export_dir), baked_hair=bool(getattr(a, "matter", None)))
    fig.objs, fig.labels, fig.garment_report, fig.skirt = D["objs"], D["labels"], D["report"], D["skirt"]
    fig.sss_inputs, fig.skin_rgb, fig.hair_rgb, fig.brow_rgb = D["sss_inputs"], D["skin_rgb"], D["hair_rgb"], D["brow_rgb"]
    fig.hair_part, fig.hair_pos_violations = D.get("hair_part"), D.get("hair_violations")
    fig.iris_verts, fig.hidden_body_vertices = D.get("iris_verts"), D["hidden_body_vertices"]
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
    if getattr(fig, "matter", None):
        fig.matter.restore()
    for pb in fig.arm.pose.bones:
        pb.matrix_basis = Matrix.Identity(4)
    if fig.skirt is not None:
        st.set_vertex_positions(fig.skirt["obj"], fig.skirt["data"]["positions"])
    bpy.context.view_layer.update()


def set_pose(fig, tape, f, realized=True):
    matter = getattr(fig, "matter", None)
    if matter and not realized:
        matter.restore()
    Sm = skin_for(fig, tape, f)
    for i, name in enumerate(fig.names):
        par = fig.skel["bones"][i]["parent"]
        Sb = Matrix(Sm[i])
        Sp = Matrix(Sm[par]) if par >= 0 else Matrix.Identity(4)
        M = fig.rest_M[name]
        fig.arm.pose.bones[name].matrix_basis = M.inverted() @ Sp.inverted() @ Sb @ M
    if fig.skirt is not None and not (realized and matter):
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
    if realized and matter:
        matter.apply(f)
    if realized and getattr(fig, "frame_update", None):
        fig.frame_update(fig, tape, f)
    bpy.context.view_layer.update()


def pelvis_world(fig, tape, f):
    Sm = skin_for(fig, tape, f)
    i = fig.pos.get("pelvis", 0)
    h = fig.wbind[i]
    return ft.apply(Sm[i], (h[0][3], h[1][3], h[2][3]))


# =========================================================================== checks

def body_ndc(cam, fig, pts=None):
    return st.project(cam, pts if pts is not None else st.eval_positions(fig.body))


def run_checks(a, fig, tape, cams, stage, checks, front_rec, res_check, info, out_dir):
    ground = stage["ground"]
    set_pose(fig, tape, stage["reference_frame"])
    ctx = stage["ctx"]
    eye_l, eye_r = ctx["eye_centers"] or (fig.eye_l, fig.eye_r)
    st.check_contact_shadow(checks, fig.body, fig.foot_idx, ground, front_rec, res_check, out_dir, figure_objects(fig), fig.labels, foot_points=ctx["foot_points"])
    checks.add(ck.contrast_check(st.luminance_linear(fig.brow_rgb), st.luminance_linear(fig.skin_rgb), name="brow_vs_skin_contrast"))
    checks.add(st.eye_clearance(front_rec["obj"], eye_l, eye_r, "eye_clearance:front"))
    face = info.get("face_cam")
    if face is not None:
        checks.add(st.eye_clearance(face["obj"], eye_l, eye_r, "eye_clearance:face_closeup"))
        checks.add(st.brow_lash_visibility(face["obj"], fig.objs["brow"], fig.objs["lash"]))
    if fig.skirt and fig.objs["garment"]:
        overlaps = fa_surface.layer_intersections(fig.skirt["obj"], fig.objs["garment"])
        checks.add({"name": "skirt_suit_clear", "ok": not any(overlaps.values()), "face_pairs": overlaps,
                    "frame": stage["reference_frame"]})
    # pose math: Blender evaluated skinning vs pure LBS of the exported top-4 weights, on up to 3 frames
    P_all = fig.S
    worst, mean_acc, n_acc = 0.0, 0.0, 0
    frames = sorted({0, tape.frame_count // 2, tape.frame_count - 1})
    for f in frames:
        set_pose(fig, tape, f, realized=False)
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

def main(figure_setup=None, frame_update=None, stage_setup=None):
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
    over = a.mutant == "overexposed"
    timings = {}

    want = a.engine
    engine_note = ""
    if want == "auto":
        ok, why = st.eevee_usable(__file__)
        engine_note = "eevee smoke: " + why
        want = "EEVEE" if ok else "CYCLES"
    elif want == "eevee":
        want = "EEVEE"
        engine_note = "explicit EEVEE"
    else:
        want = "CYCLES"

    t = time.time()
    fig = build_figure(a, tape, preset, mutant, notes)
    if figure_setup:
        figure_setup(fig, preset, tape)
    fig.frame_update = frame_update
    if a.matter:
        import fa_matter
        fig.matter = fa_matter.MatterPlayback(a.matter, tape, fig, a.export)
    if a.scene:
        import fa_scene
        fa_scene.stage_scene(fig, tape, a.scene)
    timings["build_figure_s"] = time.time() - t
    say("RT_STEP figure built neutral_match_err_m=%.3g rig_head_err_m=%.3g foot_vertices=%d hidden_under_garment=%d" % (
        fig.neutral_match_err_m, fig.rig_head_err_m, len(fig.foot_idx), fig.hidden_body_vertices))

    # Fix the stage at the first requested reference pose so every camera sees the same floor and lighting.
    reference_frame = int(shots.get("reference_frame", 0))
    if not 0 <= reference_frame < tape.frame_count:
        raise ValueError("stage reference_frame is outside the tape")
    set_pose(fig, tape, reference_frame)
    ctx, ground_z = st.reference_stage(fig.body, fig.objs["eyes"], fig.foot_idx,
                                       figure_objs=figure_objects(fig), mutant=mutant)
    lo, hi = ctx["bounds"]
    lo_t, hi_t = ctx["body_bounds"]
    ground = st.make_ground(ground_z, albedo=(0.95, 0.95, 0.95) if over else (0.35, 0.35, 0.36),
                            center_xy=ctx["center"][:2])
    st.make_world()
    lights = st.add_lights(ctx["center"], ctx["height"], gain=12.0 if over else 1.0)
    stage = {"ground": ground, "lights": lights, "reference_frame": reference_frame, "ctx": ctx}
    light_setup = stage_setup(stage, preset) if stage_setup else None
    engine_info = st.set_engine(want, a.spp, a.threads)
    sc = bpy.context.scene

    # ---- cameras from the shots file
    cams = {}
    still_specs = [] if a.no_stills else shots.get("stills", [])
    if a.still:
        requested = set(a.still)
        unknown = requested - {s["name"] for s in still_specs}
        if unknown:
            raise ValueError("unknown or disabled stills: " + ", ".join(sorted(unknown)))
        still_specs = [s for s in still_specs if s["name"] in requested]
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

    checks = st.Checks()
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
            points = (ctx["points"] if rec["tape_frame"] == reference_frame
                      else st.visible_figure_points(figure_objects(fig)))
            r = ck.framing_check(st.project(rec["obj"], points))
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
            if a.scene:
                fa_scene.track_close(fig, rec)
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
            def apply_clip_frame(i):
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
                if a.scene:
                    fa_scene.track_close(fig, rec)
                sc.camera = rec["obj"]
                for lab in fig.labels:
                    st.attach_label_to_camera(lab, rec["obj"], rec["lens"])
                return f, cid

            times, used = [], []
            if a.batch_clips:
                import fa_batch
                records = fa_batch.render_frames(sc, fdir, r, lo_i, hi_i, apply_clip_frame)
                for i, f, cid, dt in records:
                    times.append(dt)
                    used.append(cid)
                    say("RT_BATCH_FRAME %s %d/%d tape_frame=%d camera=%s %.2fs" % (c["name"], i, n_total, f, cid, dt))
            else:
                for i in range(lo_i, hi_i):
                    f, cid = apply_clip_frame(i)
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
          "tape": {"path_name": os.path.basename(a.tape), "bin_sha256": tape.header.get("bin_sha256"),
                   "asset_manifest_sha256": tape.header.get("asset_manifest_sha256"),
                   "fitted_file_sha256": fc.sha256_file(a.fitted) if a.fitted else None, "frame_count": tape.frame_count, "fps": tape.fps, "present_id": tape.header.get("present_id"),
                   "body_rev_id": tape.header.get("body_rev_id"), "fit_id": tape.header.get("fit_id"), "controls": tape.header.get("controls"),
                   "fit_status": tape.header.get("fit_status"), "style": tape.header.get("style")},
          "preset": preset.get("name"), "shots": shots.get("id"), "timings": timings, "checks": checks.items, "failed_checks": failed,
          "garments": fig.garment_report, "stage": {"reference_frame": reference_frame, "ground_z": ground.location.z,
                       "cameras": {k: {p: v[p] for p in ("target", "lens", "az", "el", "dist", "mode", "tape_frame")}
                                   for k, v in cams.items()}, "bounds_lo": list(lo), "bounds_hi": list(hi), "body_height_m": hi_t.z - lo_t.z},
          "figure": {"neutral_match_err_m": fig.neutral_match_err_m, "rig_head_err_m": fig.rig_head_err_m, "hidden_body_vertices": fig.hidden_body_vertices,
                     "sss_inputs": fig.sss_inputs,
                     "textures": {m["fa_texture_id"]: m["fa_texture_sha256"] for obj in figure_objects(fig)
                                  for m in obj.data.materials if m and "fa_texture_id" in m},
                     "skirt": fig.skirt["data"]["params"] if fig.skirt else None, "hair_part": getattr(fig, "hair_part", None), "hair_violations_in_eye_box": getattr(fig, "hair_pos_violations", None),
                     "iris_vertices": getattr(fig, "iris_verts", None)},
          "results": results, "notes": notes}
    if light_setup is not None:
        rj["stage"]["light_setup"] = light_setup
    if a.matter:
        rj["living_matter"] = {k: fig.matter.header[k] for k in ("format", "backend", "sim_config_id", "bin_sha256", "preroll_frames")}
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
