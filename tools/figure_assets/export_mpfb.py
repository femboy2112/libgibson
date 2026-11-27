#!/usr/bin/env python3
"""Blender script: export the M5-C body (neutral mesh, control deltas, rig, weights,
landmarks, garments, glTF) from MPFB.  UNRUN when committed: written against the MPFB
2.0.18 source and the Blender 4.5 API, with precise diagnostics instead of silent fallbacks.

Run (the user preferences must be loaded so the MPFB extension is enabled, so do NOT pass
--factory-startup):

    "$BLENDER" -b --python tools/figure_assets/export_mpfb.py -- \
        --out "$OUT" [--preset tools/figure_assets/presets/default_female.json] \
        [--rig game_engine] [--licence-json LICENCE_MANIFEST.json] [--no-oracle] [--skip-gltf]

Asset roots: --mpfb-data / env MPFB_DATA (MPFB extension data dir) and --user-data / env
MPFB_USER_DATA (MakeHuman system asset pack data dir); both default to what MPFB itself reports.

Exit codes: 0 ok, 2 bad arguments / environment, 3 exception, 4 files written but a built-in
cross-check failed (see the XCHECK lines).  Output format: see README.md.  Deterministic:
fixed ordering, no timestamps, hashes in manifest.json.

This script only CALLS MPFB's API inside Blender (HumanService, TargetService, ClothesService
for the oracle) and reads MakeHuman data files with our own parsers in fa_common.py.  No MPFB
or MakeHuman python is copied.
"""
import argparse
import importlib
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
import fa_common as fc  # noqa: E402
import fa_skirt  # noqa: E402

try:
    import bpy  # noqa: E402
    from mathutils import Vector  # noqa: E402
except ImportError:  # allows the pure assembly path to be exercised with a stub
    bpy = None
    Vector = None

FORMAT = "figure-asset/1"
XCHECK_FAILS = []


def say(*a):
    print(*a, flush=True)


def xcheck(ok, msg):
    say(("XCHECK PASS " if ok else "XCHECK FAIL ") + msg)
    if not ok:
        XCHECK_FAILS.append(msg)


class Timer:
    def __init__(self):
        self.t = time.time()
        self.t0 = self.t

    def lap(self, name):
        now = time.time()
        say("EXPORT_STEP %s seconds=%.2f" % (name, now - self.t))
        self.t = now


# ===================================================================== Blender layer

class Blender:
    """Everything that touches bpy / MPFB.  Replaced by a stub in offline tests."""

    def __init__(self, mpfb_data=None, user_data=None):
        self.pkg = self._find_package()
        self.HumanService = self._imp(".services.humanservice", "HumanService")
        self.TargetService = self._imp(".services.targetservice", "TargetService")
        self.LocationService = self._imp(".services.locationservice", "LocationService")
        self.Mhclo = self._imp(".entities.clothes.mhclo", "Mhclo")
        self.ClothesService = self._imp(".services.clothesservice", "ClothesService")
        self.mpfb_data = mpfb_data or os.environ.get("MPFB_DATA") or self.LocationService.get_mpfb_data()
        self.user_data = user_data or os.environ.get("MPFB_USER_DATA") or self.LocationService.get_user_data()
        self.versions = {"blender": bpy.app.version_string, "mpfb": self._mpfb_version()}

    def _find_package(self):
        cands = [k for k in sys.modules if k.startswith("bl_ext.") and k.endswith(".mpfb")]
        if not cands:
            try:
                importlib.import_module("bl_ext.user_default.mpfb")
            except ImportError:
                try:
                    bpy.ops.preferences.addon_enable(module="bl_ext.user_default.mpfb")
                    importlib.import_module("bl_ext.user_default.mpfb")
                except Exception as exc:  # noqa: BLE001
                    say("ERROR: the MPFB extension is not importable (%s)." % exc)
                    say("       modules mentioning mpfb: %s" % sorted(k for k in sys.modules if "mpfb" in k)[:20])
                    say("       Run Blender with the normal user preferences (no --factory-startup).")
                    raise SystemExit(2)
            cands = [k for k in sys.modules if k.startswith("bl_ext.") and k.endswith(".mpfb")]
        return sorted(cands)[0]

    def _imp(self, rel, cls):
        mod = importlib.import_module(self.pkg + rel)
        return getattr(mod, cls)

    def _mpfb_version(self):
        try:
            mod = sys.modules[self.pkg]
            toml = os.path.join(os.path.dirname(mod.__file__), "blender_manifest.toml")
            with open(toml, "r", encoding="utf-8") as f:
                m = re.search(r'^version\s*=\s*"([^"]+)"', f.read(), re.M)
            return m.group(1) if m else "unknown"
        except Exception:  # noqa: BLE001
            return "unknown"

    # ---- scene helpers
    def clear_scene(self):
        for o in list(bpy.data.objects):
            bpy.data.objects.remove(o, do_unlink=True)
        for coll in (bpy.data.meshes, bpy.data.armatures, bpy.data.materials):
            for d in list(coll):
                if d.users == 0:
                    coll.remove(d)

    def default_macro(self):
        return self.TargetService.get_default_macro_info_dict()

    def build_human(self, macro):
        obj = self.HumanService.create_human(mask_helpers=False, detailed_helpers=True, extra_vertex_groups=True,
                                             feet_on_ground=False, scale=0.1, macro_detail_dict=macro)
        bpy.context.view_layer.update()
        return obj

    def positions(self, obj):
        bpy.context.view_layer.update()
        dg = bpy.context.evaluated_depsgraph_get()
        ev = obj.evaluated_get(dg)
        me = ev.to_mesh()
        try:
            n = len(me.vertices)
            buf = [0.0] * (3 * n)
            me.vertices.foreach_get("co", buf)
        finally:
            ev.to_mesh_clear()
        pts = [(buf[3 * i], buf[3 * i + 1], buf[3 * i + 2]) for i in range(n)]
        mw = obj.matrix_world
        if any(abs(mw[r][c] - (1.0 if r == c else 0.0)) > 1e-6 for r in range(4) for c in range(4)):
            say("NOTE: object %s has a non-identity world matrix; applying it to the positions" % obj.name)
            pts = [tuple(mw @ Vector(p)) for p in pts]
        return pts

    def delete_object(self, obj):
        bpy.data.objects.remove(obj, do_unlink=True)

    # ---- oracles (MPFB's own result)
    def oracle_target_delta(self, obj, rel_target, base_pos):
        path = os.path.join(self.mpfb_data, "targets", rel_target + ".target.gz")
        key = self.TargetService.load_target(obj, path, weight=1.0, name="zz_xcheck")
        try:
            p2 = self.positions(obj)
        finally:
            obj.shape_key_remove(key)
        return [(a[0] - b[0], a[1] - b[1], a[2] - b[2]) for a, b in zip(p2, base_pos)]

    def oracle_garment_fit(self, obj, mhclo_path):
        mh = self.Mhclo()
        mh.load(mhclo_path)
        clothes = mh.load_mesh(bpy.context)
        try:
            self.ClothesService.fit_clothes_to_human(clothes, obj, mh, set_parent=False)
            pts = [tuple(clothes.matrix_world @ v.co) for v in clothes.data.vertices]
        finally:
            bpy.data.objects.remove(clothes, do_unlink=True)
        return pts

    # ---- rig
    def add_rig(self, obj, rig_name):
        arm = self.HumanService.add_builtin_rig(obj, rig_name, import_weights=True)
        if arm is None:
            raise RuntimeError("add_builtin_rig returned None for rig '%s'" % rig_name)
        arm.data.pose_position = "REST"
        bpy.context.view_layer.update()
        return arm

    def read_rig(self, obj, arm, n_body):
        """-> dict(bones=[{name,parent,head,tail,matrix(16),roll}], weights=[[(bone_name, w)...] per body vertex])."""
        mw = arm.matrix_world
        bones = []
        for b in arm.data.bones:
            m = mw @ b.matrix_local
            head = mw @ b.head_local
            tail = mw @ b.tail_local
            roll = None
            try:
                _axis, roll = bpy.types.Bone.AxisRollFromMatrix(b.matrix_local.to_3x3())
                roll = float(roll)
            except Exception as exc:  # noqa: BLE001
                say("NOTE: AxisRollFromMatrix failed for %s (%s); roll left null" % (b.name, exc))
            bones.append({"name": b.name, "parent": b.parent.name if b.parent else None,
                          "head": tuple(head), "tail": tuple(tail),
                          "matrix": [m[r][c] for r in range(4) for c in range(4)], "roll": roll})
        gname = {vg.index: vg.name for vg in obj.vertex_groups}
        bone_names = {b["name"] for b in bones}
        weights = []
        for v in obj.data.vertices:
            if v.index >= n_body:
                break
            weights.append([(gname[g.group], g.weight) for g in v.groups if gname.get(g.group) in bone_names])
        return {"bones": bones, "weights": weights}

    # ---- glTF of the shaped, rigged body (+ attachments)
    def build_and_export_gltf(self, macro, S_all, rig_name, attachments, hide_idx, out_path, n_body, render=None):
        self.clear_scene()
        obj = self.build_human(macro)
        me = obj.data
        if len(me.vertices) != len(S_all):
            raise RuntimeError("shaped build has %d vertices, expected %d" % (len(me.vertices), len(S_all)))
        if me.shape_keys:
            for kb in reversed(list(me.shape_keys.key_blocks)):
                obj.shape_key_remove(kb)
        me.vertices.foreach_set("co", [c for p in S_all for c in p])
        me.update()
        arm = self.add_rig(obj, rig_name)
        import bmesh
        bm = bmesh.new()
        bm.from_mesh(me)
        bm.verts.ensure_lookup_table()
        kill = [v for v in bm.verts if v.index >= n_body or v.index in hide_idx]
        bmesh.ops.delete(bm, geom=kill, context="VERTS")
        bm.to_mesh(me)
        bm.free()
        me.update()
        for p in me.polygons:
            p.use_smooth = True
        obj.data.materials.clear()
        # Use the same audited image maps in the optional interchange export.
        pack = None
        if render and render.get("texture_set") == "makehuman_system":
            import fa_materials
            import fa_stage as stage
            pack = fa_materials.PackTextures(self.user_data)
            skin, _ = stage.skin_material(render["skin"], pack)
        else:
            skin = self._material("body_skin", (0.78, 0.56, 0.46), 0.55)
        obj.data.materials.append(skin)
        obj.name = "Body"
        made = [obj]
        for a in attachments:
            material = None
            if pack is not None:
                slots = render.get("materials", {})
                if a["name"] == "long01":
                    material = stage.hair_material(render["hair"]["color"], pack=pack)
                elif a["name"] == "low-poly":
                    material = stage.eye_material(slots.get("iris", {}).get("albedo", render["eyes"]["iris"]), pack)
                elif a["name"] == "eyebrow001":
                    material = stage.brow_material(render["brows"]["color"], "brow", pack)
                elif a["name"] == "eyelashes01":
                    material = stage.brow_material([0.012, 0.01, 0.01], "lash", pack)
                elif a["kind"] in ("clothes", "skirt"):
                    key, default = ("skirt", "panel") if a["kind"] == "skirt" else ("garment", "base")
                    slot = render.get("bindings", {}).get(key, default)
                    material = stage.slot_material(slot, slots.get(slot, {}))
                    if a["kind"] == "clothes":
                        material = stage.fa_surface.garment_material(material, a["name"], pack)
            made.append(self._attachment_object(a, arm, material))
        bpy.ops.object.select_all(action="DESELECT")
        for o in [arm] + made:
            o.select_set(True)
        bpy.context.view_layer.objects.active = arm
        try:
            bpy.ops.export_scene.gltf(filepath=out_path, export_format="GLB", use_selection=True,
                                      export_apply=False, export_skins=True, export_animations=False, export_yup=True)
        except TypeError as exc:
            say("NOTE: glTF exporter rejected an argument (%s); retrying with minimal arguments" % exc)
            bpy.ops.export_scene.gltf(filepath=out_path, export_format="GLB", use_selection=True)
        info = {"objects": [o.name for o in [arm] + made], "n_body_vertices": len(me.vertices)}
        if pack is not None:
            info["texture_sources"] = {
                m["fa_texture_id"]: pack.rows[m["fa_texture_id"]]["files"][0]["path"]
                for o in made for m in o.data.materials if m and "fa_texture_id" in m}
        return info

    def _material(self, name, rgb, rough):
        m = bpy.data.materials.new(name)
        m.use_nodes = True
        bsdf = m.node_tree.nodes.get("Principled BSDF")
        if bsdf is not None:
            bsdf.inputs["Base Color"].default_value = (rgb[0], rgb[1], rgb[2], 1.0)
            bsdf.inputs["Roughness"].default_value = rough
        m.diffuse_color = (rgb[0], rgb[1], rgb[2], 1.0)
        return m

    def _attachment_object(self, a, arm, material=None):
        if material and material.get("fa_texture_id") and not a.get("uv_polys"):
            raise ValueError(f"{a['name']} has a material map but no exported face UVs")
        me = bpy.data.meshes.new(a["name"])
        me.from_pydata([tuple(p) for p in a["positions"]], [], [tuple(f) for f in a["polys"]])
        me.update()
        if a.get("uv_polys") is not None and a.get("uvs"):
            layer = me.uv_layers.new(name="UVMap")
            li = 0
            for poly, upoly in zip(a["polys"], a["uv_polys"]):
                for ui in upoly:
                    layer.data[li].uv = a["uvs"][ui]
                    li += 1
        for p in me.polygons:
            p.use_smooth = True
        me.materials.append(material or self._material(a["name"], a["color"], a.get("rough", 0.6)))
        o = bpy.data.objects.new(a["name"], me)
        bpy.context.scene.collection.objects.link(o)
        groups = {}
        for vi, (bi, wi) in enumerate(zip(a["skin_names"], a["skin_w"])):
            for bn, w in zip(bi, wi):
                if w > 0.0:
                    groups.setdefault(bn, []).append((vi, w))
        for bn in sorted(groups):
            vg = o.vertex_groups.new(name=bn)
            for vi, w in groups[bn]:
                vg.add([vi], w, "REPLACE")
        mod = o.modifiers.new("Armature", "ARMATURE")
        mod.object = arm
        o.parent = arm
        return o


# ===================================================================== assembly (pure python)

def load_json(path):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def side_deltas(control, mpfb_data, sources):
    """Sparse delta dicts {idx: [dx,dy,dz]} for a kind=targets control, per side, at |value|=1."""
    out = {}
    for side in ("neg", "pos"):
        acc = {}
        for rel, w in control[side]:
            path = os.path.join(mpfb_data, "targets", rel + ".target.gz")
            sources.add("target:" + rel, "mpfb", "targets/" + rel + ".target.gz", path)
            for i, dx, dy, dz in fc.parse_target(path):
                a = acc.setdefault(i, [0.0, 0.0, 0.0])
                a[0] += w * dx
                a[1] += w * dy
                a[2] += w * dz
        out[side] = acc
    return out


class Sources:
    """Third-party files actually read, with sha256, for the licence cross-check."""

    def __init__(self):
        self.rows = {}

    def add(self, sid, root, rel, full):
        key = (root, rel)
        if key not in self.rows:
            self.rows[key] = {"id": sid, "root": root, "path": rel, "sha256": fc.sha256_file(full),
                              "size": os.path.getsize(full)}

    def add_aggregate(self, sid, root, rel_glob, base_dir, suffix):
        import hashlib
        lines = []
        total = 0
        for dp, dn, fn in os.walk(base_dir):
            dn.sort()
            for n in fn:
                if n.endswith(suffix):
                    full = os.path.join(dp, n)
                    rel = os.path.relpath(full, self.root_for(root)).replace(os.sep, "/")
                    lines.append((rel, fc.sha256_file(full)))
                    total += os.path.getsize(full)
        lines.sort()
        h = hashlib.sha256()
        for rel, sha in lines:
            h.update(("%s:%s\n" % (rel, sha)).encode())
        self.rows[(root, rel_glob)] = {"id": sid, "root": root, "path": rel_glob, "sha256": h.hexdigest(),
                                       "size": total, "n_files": len(lines),
                                       "sha256_kind": "sha256 of sorted 'relpath:sha256' lines"}

    root_dirs = {}

    def root_for(self, root):
        return self.root_dirs[root]

    def as_list(self):
        return [self.rows[k] for k in sorted(self.rows)]


ATTACH_DIRS = {"clothes": "clothes", "hair": "hair", "eyes": "eyes", "eyebrows": "eyebrows",
               "eyelashes": "eyelashes", "teeth": "teeth"}
ATTACH_COLORS = {"clothes": ((0.20, 0.38, 0.42), 0.7), "hair": ((0.18, 0.11, 0.07), 0.5),
                 "eyes": ((0.10, 0.06, 0.04), 0.15), "eyebrows": ((0.14, 0.09, 0.06), 0.6),
                 "eyelashes": ((0.05, 0.04, 0.04), 0.6), "teeth": ((0.92, 0.90, 0.84), 0.3),
                 "skirt": ((0.55, 0.20, 0.25), 0.8)}


def load_attachment(spec, user_data, P, groups, body_idx, body_w, sources, bone_names, inner_layers=()):
    """spec like 'clothes:female_casualsuit01' or 'skirt'.  Returns (record, blender_view_dict)."""
    kind, _, name = spec.partition(":")
    panel_weights = None
    if kind in ("skirt", "skirt_a_line"):
        if kind == "skirt":
            sk = fc.make_skirt(P, groups)
            polys, uv_polys = sk["tris"], sk["tri_uv"]
        else:
            body_mask = fa_skirt.arm_mask(bone_names, body_idx, body_w)
            layers = [(g["positions"], fa_skirt.arm_mask(bone_names, g["skin_idx"], g["skin_w"]))
                      for g in inner_layers if g["kind"] == "clothes"]
            sk = fa_skirt.make_skirt(P, groups, exclude=body_mask, extra=layers)
            polys, uv_polys = sk["quads"], sk["quad_uv"]
            sk["tris"], sk["tri_uv"], _ = fc.triangulate((v, uv, None) for v, uv in zip(polys, uv_polys))
            params = sk["params"]
            sk["bind"], sk["offset_spec"] = fc.skirt_binding(
                P, sk["positions"], fc.centroid(P, groups["joint-pelvis"])[2],
                params["z_top"], params["stature"], params["lateral_limit"])
            panel_weights = sk["weights"]
        licence = "CC0-1.0 (generated geometry)" if kind == "skirt_a_line" else "self-authored, repo licence"
        rec = {"name": kind, "kind": "skirt", "source": "self-authored", "licence": licence,
               "positions": sk["positions"], "tris": sk["tris"], "tri_uv": sk["tri_uv"], "uvs": sk["uvs"],
               "bind": sk["bind"], "offset_spec": sk["offset_spec"], "delete_verts": [], "params": sk["params"],
               "polys": polys, "uv_polys": uv_polys}
    else:
        if kind not in ATTACH_DIRS:
            raise ValueError("unknown attachment kind '%s' in '%s'" % (kind, spec))
        rel = "%s/%s/%s" % (ATTACH_DIRS[kind], name, name)
        mh_path = os.path.join(user_data, rel + ".mhclo")
        ob_path = os.path.join(user_data, rel + ".obj")
        for p in (mh_path, ob_path):
            if not os.path.isfile(p):
                raise FileNotFoundError("attachment file missing: %s (user data root %s)" % (os.path.relpath(p, user_data), "MPFB_USER_DATA"))
        sources.add("%s:%s.mhclo" % (kind, name), "user", rel + ".mhclo", mh_path)
        sources.add("%s:%s.obj" % (kind, name), "user", rel + ".obj", ob_path)
        info = fc.parse_mhclo(mh_path)
        obj = fc.parse_obj(ob_path)
        if len(info["bind"]) != len(obj["verts"]):
            raise ValueError("%s: %d bindings vs %d obj vertices" % (spec, len(info["bind"]), len(obj["verts"])))
        spec_off = fc.offset_scale_spec(info)
        tris, tri_uv, _ = fc.triangulate(obj["faces"])
        rec = {"name": name, "kind": kind, "source": "MakeHuman system assets (CC0, see LICENCE_MANIFEST)",
               "licence": "CC0 (per-file evidence in docs/evidence/m5c/LICENCE_MANIFEST.md)",
               "positions": None, "tris": tris, "tri_uv": tri_uv, "uvs": obj["uvs"], "bind": info["bind"],
               "offset_spec": spec_off, "delete_verts": sorted(set(info["delete_verts"])), "params": {"z_depth": info["z_depth"]},
               "polys": [f[0] for f in obj["faces"]],
               "uv_polys": [f[1] for f in obj["faces"]] if all(f[1] is not None for f in obj["faces"]) else None}
        rec["positions"] = fc.fit_binding(rec["bind"], spec_off, P)
    if panel_weights is None:
        si, sw, bad = fc.transfer_weights(rec["bind"], body_idx, body_w, P)
    else:
        indices = [bone_names.index(b) for b in fa_skirt.BONES] + [0]
        si = [indices for _ in rec["positions"]]
        sw = [[panel_weights[b][i] for b in fa_skirt.BONES] + [0.0] for i in range(len(si))]
        bad = 0
    rec["skin_idx"], rec["skin_w"], rec["n_unweighted"] = si, sw, bad
    rec["skin_names"] = [[bone_names[i] for i in row] for row in si]
    c, r = ATTACH_COLORS[rec["kind"]]
    rec["color"], rec["rough"] = c, r
    return rec


def write_attachment(out, rec):
    n = rec["name"]
    b = "att.%s" % n
    out.blob(b + ".positions", "f32", fc.flat3(rec["positions"]), [len(rec["positions"]), 3])
    out.blob(b + ".tris", "u32", (i for t in rec["tris"] for i in t), [len(rec["tris"]), 3])
    if rec["tri_uv"] is not None:
        out.blob(b + ".tri_uv", "u32", (i for t in rec["tri_uv"] for i in t), [len(rec["tri_uv"]), 3])
        out.blob(b + ".uv", "f32", (c for uv in rec["uvs"] for c in uv), [len(rec["uvs"]), 2])
    out.blob(b + ".bind_idx", "u32", (i for r in rec["bind"] for i in r[0:3]), [len(rec["bind"]), 3])
    out.blob(b + ".bind_w", "f32", (w for r in rec["bind"] for w in r[3:6]), [len(rec["bind"]), 3])
    out.blob(b + ".bind_off", "f32", (o for r in rec["bind"] for o in r[6:9]), [len(rec["bind"]), 3])
    out.blob(b + ".skin_idx", "u16", (i for r in rec["skin_idx"] for i in r), [len(rec["skin_idx"]), 4])
    out.blob(b + ".skin_w", "f32", (w for r in rec["skin_w"] for w in r), [len(rec["skin_w"]), 4])
    out.blob(b + ".delete_verts", "u32", rec["delete_verts"], [len(rec["delete_verts"])])
    hdr = {"name": n, "kind": rec["kind"], "source": rec["source"], "licence": rec["licence"],
           "n_vertices": len(rec["positions"]), "n_tris": len(rec["tris"]),
           "blobs": {k: out.blobs[b + "." + k]["file"] for k in
                     ("positions", "tris", "bind_idx", "bind_w", "bind_off", "skin_idx", "skin_w", "delete_verts")
                     + (("tri_uv", "uv") if rec["tri_uv"] is not None else ())},
           "offset_spec": rec["offset_spec"], "params": rec["params"], "n_unweighted_bound_vertices": rec["n_unweighted"],
           "fit_rule": "position[i] = w0*P[v0] + w1*P[v1] + w2*P[v2] + (ox*sx, oy*sy, oz*sz); sk = |P[a][axis]-P[b][axis]|/ref per offset_spec[k]; P = body positions in the full index space (helper vertices included).",
           "uv_convention": "OBJ (v up)", "hides_body_vertices": "delete_verts (body indices under the garment)"}
    out.json("attachments/%s.json" % n, hdr)
    return "attachments/%s.json" % n


def assemble(args, B, say=say):
    tm = Timer()
    preset = load_json(args.preset)
    rig_name = args.rig or preset.get("rig", "game_engine")
    mpfb_data, user_data = B.mpfb_data, B.user_data
    for p in (os.path.join(mpfb_data, "3dobjs", "base.obj"), os.path.join(user_data, "clothes")):
        if not os.path.exists(p):
            say("ERROR: expected asset path missing: <root>/%s (mpfb_data / user_data roots)" % os.path.relpath(p, os.path.dirname(os.path.dirname(p))))
            return 2
    sources = Sources()
    Sources.root_dirs = {"mpfb": mpfb_data, "user": user_data}
    controls = load_json(os.path.join(HERE, "controls.json"))["controls"]
    out = fc.Output(args.out)

    # ---- base mesh topology + uv (pure)
    base_obj_path = os.path.join(mpfb_data, "3dobjs", "base.obj")
    sources.add("base_mesh", "mpfb", "3dobjs/base.obj", base_obj_path)
    base_obj = fc.parse_obj(base_obj_path)
    if len(base_obj["verts"]) != fc.N_ALL:
        say("ERROR: base.obj has %d vertices, expected %d" % (len(base_obj["verts"]), fc.N_ALL))
        return 2
    tris, tri_uv, _ = fc.triangulate(base_obj["faces"], only_group="body")
    if tri_uv is None:
        say("ERROR: base.obj body faces lack texture coordinates")
        return 2
    vg_path = os.path.join(mpfb_data, "mesh_metadata", "basemesh_vertex_groups.json")
    sources.add("vertex_groups", "mpfb", "mesh_metadata/basemesh_vertex_groups.json", vg_path)
    groups = fc.vertex_groups(vg_path)
    if groups["body"] != list(range(fc.N_BODY)):
        say("ERROR: vertex group 'body' is not 0..%d" % (fc.N_BODY - 1))
        return 2
    tm.lap("parse_base_topology")

    # ---- neutral human (macro only, no controls)
    macro = B.default_macro()
    pm = preset.get("macro", {})
    for k, v in pm.items():
        if k == "race":
            macro["race"].update(v)
        else:
            macro[k] = v
    B.clear_scene()
    base = B.build_human(macro)
    P0 = B.positions(base)
    if len(P0) != fc.N_ALL:
        say("ERROR: MPFB human has %d vertices, expected %d (helpers must be kept: mask_helpers=False)" % (len(P0), fc.N_ALL))
        return 2
    shift = -min(P0[i][2] for i in range(fc.N_BODY))
    P = [(p[0], p[1], p[2] + shift) for p in P0]
    sources.add_aggregate("macro_targets", "mpfb", "targets/macrodetails/**/*.target.gz",
                          os.path.join(mpfb_data, "targets", "macrodetails"), ".target.gz")
    sources.add("macro_json", "mpfb", "targets/macrodetails/macro.json", os.path.join(mpfb_data, "targets", "macrodetails", "macro.json"))
    tm.lap("build_neutral_human")

    # ---- control deltas
    deltas = {}
    ctl_json = []
    for c in controls:
        entry = {"name": c["name"], "kind": c["kind"], "desc": c["desc"]}
        if c["kind"] == "targets":
            sd = side_deltas(c, mpfb_data, sources)
            entry["neg_targets"] = c["neg"]
            entry["pos_targets"] = c["pos"]
        elif c["kind"] == "macro":
            sd = {}
            for side, val in (("neg", c["neg_value"]), ("pos", c["pos_value"])):
                m2 = json.loads(json.dumps(macro))
                m2[c["macro"]] = val
                tmp = B.build_human(m2)
                p2 = B.positions(tmp)
                B.delete_object(tmp)
                acc = {}
                for i in range(fc.N_ALL):
                    d = (p2[i][0] - P0[i][0], p2[i][1] - P0[i][1], p2[i][2] - P0[i][2])
                    if abs(d[0]) > 1e-7 or abs(d[1]) > 1e-7 or abs(d[2]) > 1e-7:
                        acc[i] = [d[0], d[1], d[2]]
                sd[side] = acc
            entry["macro"] = {"param": c["macro"], "baseline": macro[c["macro"]], "neg_value": c["neg_value"], "pos_value": c["pos_value"],
                              "note": "delta = positions(param=value) - positions(baseline); linear in |c| per side"}
        else:
            raise ValueError("unknown control kind %s" % c["kind"])
        for side in ("neg", "pos"):
            acc = sd[side]
            idx = sorted(acc)
            out.blob("delta.%s.%s.idx" % (c["name"], side), "u32", idx, [len(idx)])
            out.blob("delta.%s.%s.xyz" % (c["name"], side), "f32", (v for i in idx for v in acc[i]), [len(idx), 3])
            entry[side] = {"count": len(idx), "idx": out.blobs["delta.%s.%s.idx" % (c["name"], side)]["file"],
                           "xyz": out.blobs["delta.%s.%s.xyz" % (c["name"], side)]["file"],
                           "max_abs_m": max([abs(x) for i in idx for x in acc[i]] or [0.0])}
        deltas[c["name"]] = sd
        ctl_json.append(entry)
    tm.lap("control_deltas")

    # ---- oracles for targets and garment fit (MPFB's own result), before the rig exists
    if not args.no_oracle:
        try:
            tgt = next(c for c in controls if c["kind"] == "targets" and len(c["pos"]) == 1)
            rel, w1 = tgt["pos"][0]
            od = B.oracle_target_delta(base, rel, P0)
            mine = {i: (w1 * dx, w1 * dy, w1 * dz) for i, dx, dy, dz in fc.parse_target(os.path.join(mpfb_data, "targets", rel + ".target.gz"))}
            err = 0.0
            for i in range(fc.N_ALL):
                m = mine.get(i, (0.0, 0.0, 0.0))
                err = max(err, abs(od[i][0] - m[0]), abs(od[i][1] - m[1]), abs(od[i][2] - m[2]))
            xcheck(err < 1e-5, "own .target parser vs MPFB shape key for %s: max abs diff %.3e m" % (rel, err))
        except Exception as exc:  # noqa: BLE001
            xcheck(False, "target oracle raised %r" % (exc,))
            traceback.print_exc()
        try:
            rel = "clothes/female_casualsuit01/female_casualsuit01.mhclo"
            info = fc.parse_mhclo(os.path.join(user_data, rel))
            mine = fc.fit_binding(info["bind"], fc.offset_scale_spec(info), P0)
            theirs = B.oracle_garment_fit(base, os.path.join(user_data, rel))
            err = max(math.dist(a, b) for a, b in zip(mine, theirs))
            xcheck(len(mine) == len(theirs) and err < 1e-4, "own mhclo fit vs MPFB ClothesService for female_casualsuit01: max err %.3e m" % err)
        except Exception as exc:  # noqa: BLE001
            xcheck(False, "garment oracle raised %r" % (exc,))
            traceback.print_exc()
        tm.lap("oracles")

    # ---- rig + weights
    rig_file = os.path.join(mpfb_data, "rigs", "standard", "rig.%s.json" % rig_name)
    if not os.path.isfile(rig_file):
        say("ERROR: no such rig definition: rigs/standard/rig.%s.json" % rig_name)
        return 2
    sources.add("rig_" + rig_name, "mpfb", "rigs/standard/rig.%s.json" % rig_name, rig_file)
    for cand in (rig_name, "default"):
        wf = os.path.join(mpfb_data, "rigs", "standard", "weights.%s.json" % cand)
        if os.path.isfile(wf):
            sources.add("weights_" + cand, "mpfb", "rigs/standard/weights.%s.json" % cand, wf)
            weights_name = cand
            break
    else:
        say("ERROR: no weights file for rig %s" % rig_name)
        return 2
    rig_def = load_json(rig_file)
    arm = B.add_rig(base, rig_name)
    rig = B.read_rig(base, arm, fc.N_BODY)
    names_rig = {b["name"] for b in rig["bones"]}
    if names_rig != set(rig_def):
        say("ERROR: armature bones differ from the rig file: only-in-blender=%s only-in-file=%s" % (
            sorted(names_rig - set(rig_def))[:8], sorted(set(rig_def) - names_rig)[:8]))
        return 3
    order = fc.bone_order({b["name"]: b["parent"] for b in rig["bones"]})
    bidx = {n: i for i, n in enumerate(order)}
    by_name = {b["name"]: b for b in rig["bones"]}
    skel = []
    mats = []
    max_err = 0.0
    for n in order:
        b = by_name[n]
        head = (b["head"][0], b["head"][1], b["head"][2] + shift)
        tail = (b["tail"][0], b["tail"][1], b["tail"][2] + shift)
        mat = list(b["matrix"])
        mat[11] += shift
        mats.append(mat)
        hs = fc.rig_endpoint_spec(rig_def[n]["head"], groups)
        ts = fc.rig_endpoint_spec(rig_def[n]["tail"], groups)
        dh, dt = fc.derive_endpoint(hs, P), fc.derive_endpoint(ts, P)
        e = max(math.dist(dh, head), math.dist(dt, tail))
        max_err = max(max_err, e)
        skel.append({"name": n, "parent": bidx[b["parent"]] if b["parent"] else -1, "head": list(head), "tail": list(tail),
                     "roll": b["roll"], "file_roll": rig_def[n].get("roll"), "head_binding": {k: v for k, v in hs.items() if k != "default"},
                     "tail_binding": {k: v for k, v in ts.items() if k != "default"}, "head_default": hs["default"], "tail_default": ts["default"],
                     "bone_max_rederive_err_m": e})
    xcheck(max_err < 2e-3, "bone endpoints re-derived from landmark-group centroids vs Blender armature: max err %.3e m over %d bones" % (max_err, len(order)))
    out.blob("skel.rest_matrix", "f32", (v for m in mats for v in m), [len(order), 16])
    out.blob("skel.head", "f32", fc.flat3([tuple(s["head"]) for s in skel]), [len(order), 3])
    out.blob("skel.tail", "f32", fc.flat3([tuple(s["tail"]) for s in skel]), [len(order), 3])
    fingers = [n for n in order if re.search(r"thumb|index|middle|ring|pinky|finger", n)]
    side_of = lambda n: "l" if re.search(r"(_l|\.L)$", n) else ("r" if re.search(r"(_r|\.R)$", n) else "?")
    skeleton = {"rig": rig_name, "weights_file": weights_name, "n_bones": len(order), "n_finger_bones": len(fingers),
                "finger_bones_per_side": {s: len([n for n in fingers if side_of(n) == s]) for s in ("l", "r")},
                "bones": skel, "rest_matrix_blob": out.blobs["skel.rest_matrix"]["file"],
                "rest_matrix_layout": "row-major 4x4, armature space == model space, bone Y axis head->tail",
                "rederive": "joint = mean of body+helper vertices listed in {head,tail}_binding.indices (strategy mean); orientation: use roll with the standard head/tail/roll -> matrix rule and check against rest_matrix"}
    out.json("skeleton.json", skeleton)
    tm.lap("rig")

    # weights, top-4 normalised
    wi, ww, unw = [], [], 0
    for pairs in rig["weights"]:
        i4, w4, ok = fc.top4([(bidx[n], w) for n, w in pairs])
        unw += 0 if ok else 1
        wi.append(i4)
        ww.append(w4)
    xcheck(unw == 0, "every body vertex has skin weights (%d without)" % unw)
    out.blob("skin.idx", "u16", (i for r in wi for i in r), [fc.N_BODY, 4])
    out.blob("skin.w", "f32", (w for r in ww for w in r), [fc.N_BODY, 4])

    # ---- neutral mesh blobs
    nrm = fc.vertex_normals(P, tris)
    out.blob("base.positions", "f32", fc.flat3(P), [fc.N_ALL, 3])
    out.blob("base.normals", "f32", fc.flat3(nrm[:fc.N_BODY]), [fc.N_BODY, 3])
    out.blob("base.uv", "f32", (c for uv in base_obj["uvs"] for c in uv), [len(base_obj["uvs"]), 2])
    out.blob("base.tris", "u32", (i for t in tris for i in t), [len(tris), 3])
    out.blob("base.tri_uv", "u32", (i for t in tri_uv for i in t), [len(tri_uv), 3])

    # ---- landmarks
    face, rules = fc.derive_face_landmarks(P, groups)
    joints = {k: v for k, v in groups.items() if k.startswith("joint-")}
    landmarks = {"index_space": "base mesh full index space (0..%d body, %d..%d helper geometry)" % (fc.N_BODY - 1, fc.N_BODY, fc.N_ALL - 1),
                 "joints": joints, "face": face, "face_rules": rules,
                 "helper_groups": {k: groups[k] for k in sorted(groups) if k.startswith("helper-")}}
    out.json("landmarks.json", landmarks)
    tm.lap("landmarks_and_mesh")

    # ---- attachments
    att_specs = preset.get("export_attachments", [])
    atts = []
    att_files = []
    bone_names = order
    for s in att_specs:
        rec = load_attachment(s, user_data, P, groups, wi, ww, sources, bone_names, atts)
        atts.append(rec)
        att_files.append(write_attachment(out, rec))
    tm.lap("attachments")

    # ---- glTF of the shaped body
    ctl_vals = preset.get("controls", {})
    S = [list(p) for p in P0]
    for cname, val in sorted(ctl_vals.items()):
        if cname not in deltas:
            say("ERROR: preset control '%s' is not defined in controls.json" % cname)
            return 2
        if not -1.0 <= val <= 1.0:
            # No silent clamp and no rejection (studio API section 1; owner bust rule): out-of-domain
            # values extrapolate the target delta linearly and are reported as a diagnostic.
            say("DIAG ControlOutOfDomain %s=%s (documented domain [-1,1]; applied unclamped)" % (cname, val))
        side = "pos" if val > 0 else "neg"
        for i, d in deltas[cname][side].items():
            a = abs(val)
            S[i][0] += a * d[0]
            S[i][1] += a * d[1]
            S[i][2] += a * d[2]
    s_shift = -min(S[i][2] for i in range(fc.N_BODY))
    S = [(p[0], p[1], p[2] + s_shift) for p in S]
    out.blob("oracle.shaped_positions", "f32", fc.flat3(S), [fc.N_ALL, 3])
    out.json("oracle_shaped.json", {"controls": ctl_vals, "ground_shift_z": s_shift,
                                    "note": "positions = neutral + sum(|c| * delta[side]) then lifted so min body z = 0; the runtime's shaped body must match this within float tolerance"})
    gltf_info = None
    if not args.skip_gltf:
        gl_specs = preset.get("gltf_attachments", [])
        gl_atts = []
        hide = set()
        for s in gl_specs:
            rec = load_attachment(s, user_data, P, groups, wi, ww, sources, bone_names, gl_atts)
            # refit to the shaped body S (binding is the same code the runtime uses)
            rec["positions"] = fc.fit_binding(rec["bind"], rec["offset_spec"], S)
            if preset.get("hide_body_under_garment", True) and rec["kind"] == "clothes":
                hide.update(i for i in rec["delete_verts"] if i < fc.N_BODY)
            gl_atts.append(rec)
        glb = os.path.join(args.out, "body_shaped.glb")
        gltf_info = B.build_and_export_gltf(macro, S, rig_name, gl_atts, hide, glb, fc.N_BODY, preset.get("render"))
        for asset_id, relpath in gltf_info.get("texture_sources", {}).items():
            sources.add(asset_id, "user", relpath, os.path.join(user_data, relpath))
        gltf_info["hidden_body_vertices"] = len(hide)
        gltf_info["attachments"] = gl_specs
        if not os.path.isfile(glb) or os.path.getsize(glb) < 1000:
            say("ERROR: glTF export produced no usable file")
            return 3
        tm.lap("gltf")

    # ---- headers
    out.json("controls_export.json", {"format": "figure-controls-export/1", "value_range": [-1.0, 1.0],
             "semantics": "delta_total = sum_c |c| * delta[c][side(c)], side = pos if c>0 else neg; deltas in the neutral frame, metres, full index space; sparse (idx u32 + xyz f32)",
             "controls": ctl_json})
    header = {"format": FORMAT, "frame": "Blender frame: right-handed, +Z up, character faces -Y; metres",
              "uv_convention": "OBJ (v up)", "ground_shift_z_neutral": shift,
              "counts": {"n_vertices_all": fc.N_ALL, "n_vertices_body": fc.N_BODY, "n_tris_body": len(tris), "n_bones": len(order),
                         "n_controls": len(controls)},
              "preset": {k: preset[k] for k in sorted(preset)}, "rig": {"name": rig_name, "n_bones": len(order),
                                                                          "n_finger_bones": len(fingers)},
              "files": {"skeleton": "skeleton.json", "landmarks": "landmarks.json", "controls": "controls_export.json",
                        "attachments": att_files, "gltf": "body_shaped.glb" if gltf_info else None,
                        "oracle": "oracle_shaped.json"},
              "blobs": {k: out.blobs[k] for k in sorted(out.blobs) if k.startswith(("base.", "skin.", "skel.", "oracle."))},
              "tools": B.versions, "gltf": gltf_info}
    out.json("figure_asset.json", header)
    extra = {"format": FORMAT + "-manifest", "tools": B.versions, "sources": sources.as_list(),
             "checks": {"xcheck_failures": list(XCHECK_FAILS)}}
    if args.licence_json:
        lic = load_json(args.licence_json)
        by_sha = {}
        for r in lic["rows"]:
            by_sha[r["sha256"]] = r["status"]
            for f in r["files"]:
                by_sha[f["sha256"]] = f["status"]
        extra["licence"] = {"audit_json_sha256": fc.sha256_file(args.licence_json),
                            "sources_not_cc0": [s["path"] for s in sources.as_list() if by_sha.get(s["sha256"], "UNKNOWN") in ("DROP", "UNKNOWN")]}
    out.manifest(extra)
    tm.lap("write_manifest")
    say("EXPORT_DONE out_blobs=%d files_in_manifest=%d total_seconds=%.1f" % (len(out.blobs), len(load_json(os.path.join(args.out, "manifest.json"))["files"]), time.time() - tm.t0))
    return 4 if XCHECK_FAILS else 0


def parse_args(argv):
    ap = argparse.ArgumentParser(prog="export_mpfb.py")
    ap.add_argument("--out", required=True)
    ap.add_argument("--preset", default=os.path.join(HERE, "presets", "default_female.json"))
    ap.add_argument("--rig", default=None, help="override the preset's rig (game_engine | default_no_toes | default ...)")
    ap.add_argument("--mpfb-data", default=None)
    ap.add_argument("--user-data", default=None)
    ap.add_argument("--licence-json", default=None, help="LICENCE_MANIFEST.json from licence_audit.py")
    ap.add_argument("--no-oracle", action="store_true", help="skip the MPFB cross-checks (target + garment fit)")
    ap.add_argument("--skip-gltf", action="store_true")
    return ap.parse_args(argv)


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[1:]
    args = parse_args(argv)
    if bpy is None:
        say("ERROR: bpy is not importable; run this file with blender -b --python")
        return 2
    try:
        B = Blender(args.mpfb_data, args.user_data)
        say("EXPORT_START blender=%s mpfb=%s" % (B.versions["blender"], B.versions["mpfb"]))
        return assemble(args, B)
    except SystemExit as e:
        return int(e.code or 0)
    except Exception:  # noqa: BLE001
        traceback.print_exc()
        say("EXPORT_FAILED see traceback above")
        return 3


if __name__ == "__main__":
    rc = main()
    say("EXPORT_RC %d" % rc)
    sys.exit(rc)
