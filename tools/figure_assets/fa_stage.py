#!/usr/bin/env python3
"""Blender-side stage helpers shared by render_tape.py and render_probe.py (imports bpy; never run on its own).

UNRUN when committed (heavy-only): written against the Blender 4.5 API.  Everything here is generic stage craft:
materials (Principled BSDF, EEVEE Next), the 3-point + soft-top light rig, ground, cameras fitted from projected bounds,
the figure-mask pass used by the contact-shadow check, ray-cast clearance checks, text labels, engine selection.

Defects of the MEASURE probe this module fixes (docs/evidence/m5c/measure/RESULT.md, viewed items 1, 2, 5):
  1. the ground is placed at the min z of the EVALUATED body vertices (never helper or hidden geometry) and checked;
  2. the camera is fitted from the projected body vertices so the figure spans a target fraction of the frame height;
  5. key / fill / rim lighting, a skin material with subsurface, dark brows and lashes, eyes with an iris and pupil.
`mutant=True` reproduces the probe bug (bounds over every mesh object including a hidden helper proxy that is 0.40 m
below the feet and 2.80 m tall) so the self-checks can be shown to FAIL.
"""
import math
import os
import subprocess
import sys
import time

import bpy
from bpy_extras.object_utils import world_to_camera_view
from mathutils import Matrix, Vector

SLOTS = ("base", "panel", "trim", "ornament", "gem_emissive", "mask", "iris", "accessory_emissive")
SENSOR_H = 24.0
FOOT_RE = r"^(foot|ball|toe)"
ARM_RE = r"(clavicle|upperarm|lowerarm|hand|thumb|index|middle|ring|pinky)"


def say(*a):
    print(*a, flush=True)


# =========================================================================== materials

def set_input(bsdf, names, value):
    """Set the first Principled input in `names` that exists (the sockets were renamed in Blender 4.0)."""
    for n in names:
        sock = bsdf.inputs.get(n)
        if sock is not None:
            try:
                sock.default_value = value
                return n
            except (TypeError, ValueError):
                continue
    return None


def rgba(c, a=1.0):
    return (float(c[0]), float(c[1]), float(c[2]), a)


def new_principled(name):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    return m, m.node_tree.nodes.get("Principled BSDF")


def slot_material(slot, spec):
    """Studio API section 1: linear RGB albedo, roughness, metalness, optional emissive colour + intensity."""
    m, b = new_principled("slot_" + slot)
    alb = spec.get("albedo") or spec.get("base_color") or spec.get("color") or [0.5, 0.5, 0.5]
    set_input(b, ("Base Color",), rgba(alb))
    set_input(b, ("Roughness",), float(spec.get("roughness", 0.6)))
    set_input(b, ("Metallic",), float(spec.get("metalness", spec.get("metallic", 0.0))))
    em = spec.get("emissive") or spec.get("emission")
    if em is not None:
        set_input(b, ("Emission Color", "Emission"), rgba(em))
        set_input(b, ("Emission Strength",), float(spec.get("emissive_intensity", spec.get("emission_strength", 1.0))))
    m.diffuse_color = rgba(alb)
    return m


def skin_material(spec):
    """Skin with subsurface scattering (EEVEE Next supports the Principled SSS inputs)."""
    m, b = new_principled("skin")
    alb = spec.get("albedo", [0.62, 0.40, 0.31])
    set_input(b, ("Base Color",), rgba(alb))
    set_input(b, ("Roughness",), float(spec.get("roughness", 0.42)))
    got = {}
    got["sss_weight"] = set_input(b, ("Subsurface Weight", "Subsurface"), float(spec.get("subsurface", 0.35)))
    got["sss_radius"] = set_input(b, ("Subsurface Radius",), tuple(spec.get("subsurface_radius", [1.0, 0.35, 0.2])))
    got["sss_scale"] = set_input(b, ("Subsurface Scale",), float(spec.get("subsurface_scale", 0.02)))
    set_input(b, ("Specular IOR Level", "Specular"), 0.45)
    m.diffuse_color = rgba(alb)
    m["fa_sss_inputs"] = str(got)
    return m, got


def hair_material(color, rough=0.38):
    m, b = new_principled("hair")
    set_input(b, ("Base Color",), rgba(color))
    set_input(b, ("Roughness",), rough)
    set_input(b, ("Specular IOR Level", "Specular"), 0.6)
    m.diffuse_color = rgba(color)
    return m


def brow_material(color, name="brow"):
    m, b = new_principled(name)
    set_input(b, ("Base Color",), rgba(color))
    set_input(b, ("Roughness",), 0.55)
    m.diffuse_color = rgba(color)
    return m


def eye_material(iris_rgb):
    """Sclera / iris / pupil from a per-vertex float attribute `iris_mask` (0 sclera, 0.6 iris, 1.0 pupil), so it follows the
    deformation and does not depend on the eye mesh UV layout."""
    m, b = new_principled("eye")
    nt = m.node_tree
    attr = nt.nodes.new("ShaderNodeAttribute")
    attr.attribute_name = "iris_mask"
    attr.attribute_type = "GEOMETRY"
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.interpolation = "CONSTANT"
    els = ramp.color_ramp.elements
    els[0].position, els[0].color = 0.0, (0.88, 0.86, 0.82, 1.0)
    els[1].position, els[1].color = 0.30, rgba(iris_rgb)
    e3 = els.new(0.80)
    e3.color = (0.01, 0.01, 0.012, 1.0)
    nt.links.new(attr.outputs.get("Fac") or attr.outputs[2], ramp.inputs[0])
    nt.links.new(ramp.outputs[0], b.inputs["Base Color"])
    set_input(b, ("Roughness",), 0.12)
    set_input(b, ("Coat Weight", "Clearcoat"), 0.6)
    m.diffuse_color = rgba(iris_rgb)
    return m


def flat_material(name, rgb, emissive=False):
    m, b = new_principled(name)
    set_input(b, ("Base Color",), rgba(rgb))
    set_input(b, ("Roughness",), 0.9)
    if emissive:
        set_input(b, ("Emission Color", "Emission"), rgba(rgb))
        set_input(b, ("Emission Strength",), 3.0)
    m.diffuse_color = rgba(rgb)
    return m


def luminance_linear(rgb):
    return 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]


# =========================================================================== objects

def link(obj):
    bpy.context.scene.collection.objects.link(obj)
    return obj


def mesh_object(name, positions, polys, uvs=None, uv_polys=None, role=None, smooth=True):
    me = bpy.data.meshes.new(name)
    me.from_pydata([tuple(p) for p in positions], [], [tuple(f) for f in polys])
    me.update()
    if uvs and uv_polys is not None:
        layer = me.uv_layers.new(name="UVMap")
        li = 0
        for poly, upoly in zip(polys, uv_polys):
            for ui in upoly:
                layer.data[li].uv = uvs[ui]
                li += 1
    for p in me.polygons:
        p.use_smooth = smooth
    o = bpy.data.objects.new(name, me)
    link(o)
    if role:
        o["fa_role"] = role
    return o


def set_weights(obj, names_per_vertex, weights_per_vertex):
    groups = {}
    for vi, (bn, ws) in enumerate(zip(names_per_vertex, weights_per_vertex)):
        for n, w in zip(bn, ws):
            if w > 0.0:
                groups.setdefault(n, []).append((vi, w))
    for n in sorted(groups):
        vg = obj.vertex_groups.new(name=n)
        for vi, w in groups[n]:
            vg.add([vi], w, "REPLACE")


def add_armature_modifier(obj, arm):
    mod = obj.modifiers.new("Armature", "ARMATURE")
    mod.object = arm
    obj.parent = arm
    return mod


def eval_positions(obj):
    """Evaluated (modifier-applied, viewport depsgraph) vertex positions in world space."""
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
    mw = obj.matrix_world
    ident = all(abs(mw[r][c] - (1.0 if r == c else 0.0)) < 1e-9 for r in range(4) for c in range(4))
    pts = [(buf[3 * i], buf[3 * i + 1], buf[3 * i + 2]) for i in range(n)]
    if not ident:
        pts = [tuple(mw @ Vector(p)) for p in pts]
    return pts


def set_vertex_positions(obj, pts):
    me = obj.data
    me.vertices.foreach_set("co", [c for p in pts for c in p])
    me.update()


def add_iris_attribute(obj, pts, forward=(0.0, -1.0, 0.0), pupil_deg=15.0, iris_deg=40.0):
    """Per-vertex iris_mask from the angle between (vertex - eyeball centre) and the forward axis, one eyeball per x sign."""
    sides = {1: [i for i, p in enumerate(pts) if p[0] >= 0.0], -1: [i for i, p in enumerate(pts) if p[0] < 0.0]}
    vals = [0.0] * len(pts)
    fw = Vector(forward)
    for s, idx in sides.items():
        if not idx:
            continue
        c = Vector((sum(pts[i][0] for i in idx), sum(pts[i][1] for i in idx), sum(pts[i][2] for i in idx))) / len(idx)
        for i in idx:
            d = Vector(pts[i]) - c
            if d.length < 1e-9:
                continue
            ang = math.degrees(d.angle(fw))
            vals[i] = 1.0 if ang < pupil_deg else (0.6 if ang < iris_deg else 0.0)
    me = obj.data
    a = me.attributes.new(name="iris_mask", type="FLOAT", domain="POINT")
    a.data.foreach_set("value", vals)
    return sum(1 for v in vals if v > 0.0)


# =========================================================================== stage: ground, world, lights

def mesh_bounds_all(include_hidden=True):
    """The BUGGY bounds of the probe: bound boxes of every mesh object, hidden ones included."""
    lo = Vector((1e9, 1e9, 1e9))
    hi = Vector((-1e9, -1e9, -1e9))
    for o in bpy.data.objects:
        if o.type != "MESH":
            continue
        if not include_hidden and (o.hide_render or o.hide_get()):
            continue
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector((min(lo[i], w[i]) for i in range(3)))
            hi = Vector((max(hi[i], w[i]) for i in range(3)))
    return lo, hi


def add_hidden_helper_proxy(body_lo_z, height=2.80, drop=0.40):
    """Mutant `bad_bounds`: an invisible mesh whose box spans [lo - 0.40, lo - 0.40 + 2.80] (the viewed 2.80 m 'stage height')."""
    z0 = body_lo_z - drop
    verts = [(x, y, z) for z in (z0, z0 + height) for y in (-0.1, 0.1) for x in (-0.1, 0.1)]
    faces = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (1, 5, 7, 3), (3, 7, 6, 2), (2, 6, 4, 0)]
    o = mesh_object("fa_helper_proxy", verts, faces, role="helper")
    o.hide_render = True
    o.hide_set(True)
    return o


def make_ground(z, size=40.0, albedo=(0.46, 0.46, 0.48)):
    h = 0.5 * size
    o = mesh_object("Ground", [(-h, -h, 0.0), (h, -h, 0.0), (h, h, 0.0), (-h, h, 0.0)], [(0, 1, 2, 3)], role="ground", smooth=False)
    o.location = (0.0, 0.0, z)
    m, b = new_principled("ground")
    set_input(b, ("Base Color",), rgba(albedo))
    set_input(b, ("Roughness",), 0.9)
    set_input(b, ("Specular IOR Level", "Specular"), 0.1)
    o.data.materials.append(m)
    return o


def make_world(color=(0.30, 0.32, 0.37), strength=0.45):
    sc = bpy.context.scene
    w = sc.world if sc.world is not None else bpy.data.worlds.new("fa_world")
    sc.world = w
    w.use_nodes = True
    bg = w.node_tree.nodes.get("Background")
    if bg is not None:
        bg.inputs["Color"].default_value = rgba(color)
        bg.inputs["Strength"].default_value = strength
    return w


def aim(obj, target):
    d = Vector(target) - obj.location
    obj.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()


def add_lights(center, height):
    """Key / fill / rim (three-point) plus a soft top light that gives the figure a contact shadow.  Azimuth 0 is the camera
    side (-Y); +x is the camera's right.  Energies are first guesses scaled by distance squared (4.5 m, 1.7 m figure)."""
    sc = bpy.context.scene
    dist = 2.7 * height
    k = (dist / 4.5) ** 2
    cx, cy, cz = center
    rig = (("key", -42.0, 38.0, 1.0, 1100.0, 1.7, (1.0, 0.94, 0.88)),
           ("fill", 58.0, 14.0, 1.0, 380.0, 2.6, (0.86, 0.91, 1.0)),
           ("rim", 150.0, 34.0, 1.0, 900.0, 1.2, (0.92, 0.96, 1.0)))
    made = {}
    for name, az, el, rr, energy, size, col in rig:
        a, e = math.radians(az), math.radians(el)
        pos = (cx + dist * rr * math.sin(a) * math.cos(e), cy - dist * rr * math.cos(a) * math.cos(e), cz + dist * rr * math.sin(e))
        ld = bpy.data.lights.new(name, "AREA")
        ld.energy = energy * k
        ld.size = size * height / 1.7
        ld.color = col
        try:
            ld.use_shadow = True
        except Exception:  # noqa: BLE001
            pass
        o = bpy.data.objects.new(name, ld)
        link(o)
        o.location = pos
        aim(o, (cx, cy, cz + 0.1 * height))
        made[name] = o
    top = bpy.data.lights.new("top_soft", "AREA")
    top.energy = 380.0 * k
    top.size = 1.6 * height / 1.7
    top.color = (1.0, 1.0, 1.0)
    to = bpy.data.objects.new("top_soft", top)
    link(to)
    to.location = (cx, cy, cz + 1.9 * height)
    aim(to, (cx, cy, cz - 0.5 * height))
    made["top_soft"] = to
    return made


def build_label(text, size=0.035):
    cu = bpy.data.curves.new("fa_label", "FONT")
    cu.body = text
    cu.size = size
    cu.align_x = "LEFT"
    o = bpy.data.objects.new("fa_label", cu)
    link(o)
    o["fa_role"] = "label"
    cu.materials.append(flat_material("label", (1.0, 0.85, 0.2), emissive=True))
    return o


def attach_label_to_camera(label, cam_obj, lens):
    """Bottom-left of the frame at 1 m in front of the camera (frame half height at depth d is d * 12 / lens)."""
    d = 1.0
    half_h = d * (SENSOR_H / 2.0) / lens
    label.parent = cam_obj
    label.matrix_parent_inverse = Matrix.Identity(4)
    label.location = (-1.72 * half_h * 0.90, -0.86 * half_h, -d)
    label.rotation_euler = (0.0, 0.0, 0.0)
    label.data.size = 0.075 * half_h


# =========================================================================== cameras

KINDS = {
    "front": {"az": 0.0, "el": 4.0},
    "side": {"az": 90.0, "el": 4.0},
    "three_quarter": {"az": 35.0, "el": 6.0},
    "back": {"az": 180.0, "el": 4.0},
    "face_closeup": {"az": 8.0, "el": 2.0, "target": "face", "lens": 85.0, "fill": 0.70},
    "turntable": {"az": 0.0, "el": 5.0},
    "tracking": {"az": 22.0, "el": 4.0, "track": True},
}


def vfov(lens):
    return 2.0 * math.atan((SENSOR_H / 2.0) / lens)


def place_camera(cam, target, az_deg, el_deg, dist):
    a, e = math.radians(az_deg), math.radians(el_deg)
    t = Vector(target)
    cam.location = t + Vector((dist * math.sin(a) * math.cos(e), -dist * math.cos(a) * math.cos(e), dist * math.sin(e)))
    aim(cam, t)
    bpy.context.view_layer.update()


def project(cam, pts):
    sc = bpy.context.scene
    out = []
    for p in pts:
        c = world_to_camera_view(sc, cam, Vector(p))
        out.append((c.x, c.y))
    return out


def make_camera(name, lens):
    cd = bpy.data.cameras.new(name)
    cd.lens = lens
    cd.sensor_fit = "VERTICAL"
    cd.sensor_height = SENSOR_H
    cd.clip_start = 0.05
    cd.clip_end = 200.0
    o = bpy.data.objects.new(name, cd)
    link(o)
    return o


def fit_camera(name, spec, ctx, res):
    """Create and fit a camera.  ctx: dict(points=[body verts], face_points=[...], center=(x,y,z), height=H, face_center=(x,y,z),
    bounds=(lo,hi) from the bounds routine in force, mutant=bool).  Returns a record with the placement parameters."""
    base = dict(KINDS[spec.get("kind", "front")])
    base.update({k: v for k, v in spec.items() if k != "kind"})
    lens = float(base.get("lens", 50.0))
    az, el = float(base["az"]), float(base["el"])
    fill = float(base.get("fill", 0.80))
    sc = bpy.context.scene
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.render.resolution_percentage = 100
    cam = make_camera(name, lens)
    face = base.get("target") == "face"
    frame = base.get("frame")
    rec = {"name": name, "kind": spec.get("kind", "front"), "lens": lens, "az": az, "el": el, "fill": fill, "target_kind": "face" if face else "body",
           "track": bool(base.get("track")), "orbit_deg": float(base.get("orbit_deg", 0.0)), "obj": cam}
    if face:
        target = Vector(ctx["face_center"])
        pts = ctx["face_points"]
        height_m = float(base.get("face_height_m", 0.30))
        dist = (height_m / fill * 0.5) / math.tan(vfov(lens) / 2.0)
        mode = "face_analytic+fit"
    elif frame and frame.get("mode") == "fixed":
        c = frame["center"]
        target = Vector(c)
        pts = ctx["points"]
        dist = (float(frame["height_m"]) / fill * 0.5) / math.tan(vfov(lens) / 2.0)
        mode = "fixed"
    elif ctx.get("mutant"):
        lo, hi = ctx["bounds"]
        h = hi.z - lo.z                        # the probe's formula: distance from the (wrong) bounds height, no fit
        target = Vector((0.0, 0.0, 0.5 * (lo.z + hi.z)))
        dist = (h * 1.18 * 0.5) / math.tan(vfov(lens) / 2.0)
        pts = ctx["points"]
        mode = "bounds_analytic(mutant)"
    else:
        target = Vector(ctx["center"])
        pts = ctx["points"]
        dist = (ctx["height"] / fill * 0.5) / math.tan(vfov(lens) / 2.0)
        mode = "points_fit"
    place_camera(cam, target, az, el, dist)
    if mode in ("points_fit", "face_analytic+fit"):
        for _ in range(6):
            ndc = project(cam, pts)
            ys = [p[1] for p in ndc]
            xs = [p[0] for p in ndc]
            frac = max(ys) - min(ys)
            if frac <= 1e-6:
                break
            dist *= frac / fill
            half_w_world = dist * math.tan(vfov(lens) / 2.0)
            right = Vector(cam.matrix_world.to_3x3() @ Vector((1.0, 0.0, 0.0)))
            up = Vector(cam.matrix_world.to_3x3() @ Vector((0.0, 1.0, 0.0)))
            target = target + up * ((0.5 * (max(ys) + min(ys)) - 0.5) * 2.0 * half_w_world) + right * ((0.5 * (max(xs) + min(xs)) - 0.5) * 2.0 * half_w_world * res[0] / float(res[1]))
            place_camera(cam, target, az, el, dist)
    rec.update({"target": tuple(target), "dist": dist, "mode": mode, "res": tuple(res), "tape_frame": 0})
    return rec


def cam_pose_at(rec, frame_i, n_frames, tracking_target=None):
    """Update a camera record for frame i of a clip: orbit (turntable) and tracking move the camera, others stay."""
    az = rec["az"] + (rec["orbit_deg"] * frame_i / float(max(1, n_frames)) if rec["orbit_deg"] else 0.0)
    tgt = rec["target"]
    if rec["track"] and tracking_target is not None:
        tgt = (tracking_target[0], tracking_target[1], rec["target"][2])
    place_camera(rec["obj"], tgt, az, rec["el"], rec["dist"])


# =========================================================================== self-check glue

def ground_rects(cam, foot_pts, ground_z, inner_m=0.07, outer_m=0.30):
    """Pixel rects (x0, y0, x1, y1; y down) of the ground plane around the foot footprint, from the camera's projection."""
    sc = bpy.context.scene
    w, h = sc.render.resolution_x, sc.render.resolution_y
    xs = [p[0] for p in foot_pts]
    ys = [p[1] for p in foot_pts]
    out = []
    for m in (inner_m, outer_m):
        px, py = [], []
        for x in (min(xs) - m, max(xs) + m):
            for y in (min(ys) - m, max(ys) + m):
                c = world_to_camera_view(sc, cam, Vector((x, y, ground_z)))
                px.append(c.x * w)
                py.append((1.0 - c.y) * h)
        out.append((int(min(px)), int(min(py)), int(max(px)) + 1, int(max(py)) + 1))
    return out[0], out[1]


def ray_first_hit(origin, target, ignore_epsilon=0.0):
    sc = bpy.context.scene
    dg = bpy.context.evaluated_depsgraph_get()
    d = Vector(target) - Vector(origin)
    dist = d.length
    hit, loc, nor, idx, obj, mat = sc.ray_cast(dg, Vector(origin), d.normalized(), distance=dist + 0.05)
    if not hit:
        return None, dist, dist
    role = None
    try:
        role = obj.original.get("fa_role") if obj is not None else None
    except Exception:  # noqa: BLE001
        role = None
    return role, (loc - Vector(origin)).length, dist


# =========================================================================== mask pass (figure vs ground)

def render_mask(path, res, figure_objs, hide_extra=()):
    """Flat white figure on black ground via the Workbench engine (object colour, no lights, no AA): the figure silhouette."""
    sc = bpy.context.scene
    old_engine = sc.render.engine
    old_res = (sc.render.resolution_x, sc.render.resolution_y)
    old_vt = sc.view_settings.view_transform
    sh = sc.display.shading
    old_sh = (sh.light, sh.color_type, sc.display.render_aa)
    ground = [o for o in bpy.data.objects if o.get("fa_role") == "ground"]
    saved = []
    for o in bpy.data.objects:
        if o.type in ("MESH", "FONT"):
            saved.append((o, tuple(o.color), o.hide_render))
    w = sc.world
    old_world_col = tuple(w.color) if w is not None else None
    try:
        sc.render.engine = "BLENDER_WORKBENCH"
        sc.display.render_aa = "OFF"
        sh.light = "FLAT"
        sh.color_type = "OBJECT"
        sc.view_settings.view_transform = "Standard"
        sc.render.resolution_x, sc.render.resolution_y = res
        for o in bpy.data.objects:
            if o.type in ("MESH", "FONT"):
                if o in figure_objs:
                    o.color = (1.0, 1.0, 1.0, 1.0)
                elif o in ground:
                    o.color = (0.0, 0.0, 0.0, 1.0)
                else:
                    o.hide_render = True
        if w is not None:
            w.color = (0.0, 0.0, 0.0)
        sc.render.image_settings.file_format = "PNG"
        sc.render.image_settings.color_mode = "RGB"
        sc.render.image_settings.color_depth = "8"
        sc.render.filepath = path
        bpy.ops.render.render(write_still=True)
    finally:
        for o, col, hr in saved:
            o.color = col
            o.hide_render = hr
        if w is not None and old_world_col is not None:
            w.color = old_world_col
        sc.render.engine = old_engine
        sc.render.resolution_x, sc.render.resolution_y = old_res
        sc.view_settings.view_transform = old_vt
        sh.light, sh.color_type = old_sh[0], old_sh[1]
        sc.display.render_aa = old_sh[2]
    return path


# =========================================================================== engine

def set_engine(name, spp, threads):
    sc = bpy.context.scene
    sc.render.threads_mode = "FIXED"
    sc.render.threads = threads
    sc.view_settings.view_transform = "Standard"
    try:
        sc.view_settings.look = "None"
    except Exception:  # noqa: BLE001
        pass
    if name == "EEVEE":
        last = None
        for ident in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"):
            try:
                sc.render.engine = ident
            except TypeError as exc:
                last = exc
                continue
            notes = []
            for attr, val in (("taa_render_samples", int(spp)), ("use_shadows", True), ("use_raytracing", False), ("use_gtao", True)):
                try:
                    setattr(sc.eevee, attr, val)
                except Exception:  # noqa: BLE001
                    notes.append(attr)
            return {"engine": "EEVEE_NEXT" if ident.endswith("NEXT") else "EEVEE", "samples": int(spp), "unsupported_settings": notes}
        raise RuntimeError("no EEVEE engine id accepted: %s" % last)
    sc.render.engine = "CYCLES"
    cy = sc.cycles
    cy.device = "CPU"
    cy.samples = int(spp)
    denoise = "none"
    try:
        cy.use_denoising = True
        cy.denoiser = "OPENIMAGEDENOISE"
        denoise = "OIDN"
    except Exception:  # noqa: BLE001
        pass
    return {"engine": "CYCLES", "samples": int(spp), "denoise": denoise}


def render_to(path, res):
    sc = bpy.context.scene
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.render.resolution_percentage = 100
    sc.render.image_settings.file_format = "PNG"
    sc.render.image_settings.color_mode = "RGB"
    sc.render.image_settings.color_depth = "8"
    sc.render.filepath = path
    t = time.time()
    bpy.ops.render.render(write_still=True)
    return time.time() - t


def eevee_smoke():
    """Child Blender: render 64x64 of a cube with EEVEE; rc 0 plus the marker line mean EEVEE works headless here."""
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    verts = [(x, y, z) for z in (0.0, 1.0) for y in (-0.5, 0.5) for x in (-0.5, 0.5)]
    faces = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (1, 5, 7, 3), (3, 7, 6, 2), (2, 6, 4, 0)]
    mesh_object("cube", verts, faces)
    make_world()
    add_lights((0.0, 0.0, 0.5), 1.0)
    c = make_camera("c", 50.0)
    place_camera(c, (0.0, 0.0, 0.5), 20.0, 10.0, 4.0)
    bpy.context.scene.camera = c
    info = set_engine("EEVEE", 4, 2)
    out = os.path.join(os.environ.get("TMPDIR", "/tmp"), "fa_smoke_%d.png" % os.getpid())
    render_to(out, (64, 64))
    ok = os.path.isfile(out)
    if ok:
        os.remove(out)
    say("SMOKE_EEVEE %s engine=%s" % ("OK" if ok else "NO_FILE", info["engine"]))
    return 0 if ok else 1


def eevee_usable(script_path):
    try:
        r = subprocess.run([bpy.app.binary_path, "-b", "--python", os.path.abspath(script_path), "--", "--child-smoke"],
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=240, text=True)
    except (subprocess.TimeoutExpired, OSError) as exc:
        say("NOTE eevee smoke child failed to run: %s" % exc)
        return False, "child failed: %s" % exc
    ok = r.returncode == 0 and "SMOKE_EEVEE OK" in r.stdout
    if not ok:
        tail = " | ".join(r.stdout.strip().splitlines()[-4:])
        say("NOTE eevee smoke child rc=%d tail: %s" % (r.returncode, tail[:400]))
        return False, "child rc=%d" % r.returncode
    return True, "child ok"
