#!/usr/bin/env python3
"""Blender-side stage helpers shared by render_tape.py and render_probe.py (imports bpy; never run on its own).

The tape renderer measures the posed, dressed surface for its floor and matched full-body cameras.
The close-up follows the eyes. Three area lights illuminate the figure and physical receiver;
the beauty and silhouette passes supply the near-foot shadow check.
`mutant=True` reproduces the probe bug (bounds over every mesh object including a hidden helper proxy that is 0.40 m
below the feet and 2.80 m tall) so the self-checks can be shown to FAIL.
"""
import math
import os
import subprocess
import sys
import time

import json

import bpy
from bpy_extras.object_utils import world_to_camera_view
from mathutils import Matrix, Vector
from mathutils.kdtree import KDTree

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)
import fa_checks as ck  # noqa: E402
import fa_common as fc  # noqa: E402
import fa_hair  # noqa: E402
import fa_materials  # noqa: E402
import fa_skirt  # noqa: E402
import fa_surface  # noqa: E402
import fa_tape as ft  # noqa: E402

SKIRT_ID = "skirt_a_line"

SLOTS = ("base", "panel", "trim", "ornament", "gem_emissive", "mask", "iris", "accessory_emissive")
SENSOR_H = 24.0
FOOT_RE = r"^(foot|ball|toe)"
ARM_RE = r"(clavicle|upperarm|lowerarm|hand|thumb|index|middle|ring|pinky)"


def say(*a):
    print(*a, flush=True)


def load_json(p):
    with open(p, "r", encoding="utf-8") as f:
        return json.load(f)


def verdict_of(v):
    """fit_status values may be a string or a one-key object; -> (is_fitted, text)."""
    if isinstance(v, dict) and len(v) == 1:
        key, detail = next(iter(v.items()))
        text = f"{key} {json.dumps(detail, sort_keys=True)[:60]}" if detail not in (None, {}, []) else key
        return key == "Fitted", text
    return v == "Fitted", str(v)


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


def skin_material(spec, pack=None):
    """Skin with subsurface scattering (EEVEE Next supports the Principled SSS inputs)."""
    m, b = new_principled("skin")
    alb = spec.get("albedo", [0.62, 0.40, 0.31])
    set_input(b, ("Base Color",), rgba(alb))
    set_input(b, ("Roughness",), float(spec.get("roughness", 0.55)))
    got = {}
    got["sss_weight"] = set_input(b, ("Subsurface Weight", "Subsurface"), float(spec.get("subsurface", 0.15)))
    got["sss_radius"] = set_input(b, ("Subsurface Radius",), tuple(spec.get("subsurface_radius", [1.0, 0.35, 0.2])))
    got["sss_scale"] = set_input(b, ("Subsurface Scale",), float(spec.get("subsurface_scale", 0.01)))
    set_input(b, ("Specular IOR Level", "Specular"), float(spec.get("specular", 0.22)))
    m.diffuse_color = rgba(alb)
    m["fa_sss_inputs"] = str(got)
    if pack is not None:
        tex = pack.node(m, "skin_diffuse")
        fa_materials.tinted_albedo(m, tex, alb, (0.68, 0.46, 0.34))
        fa_materials.skin_detail(m)
    return m, got


def hair_material(color, rough=0.38, pack=None):
    m, b = new_principled("hair")
    set_input(b, ("Base Color",), rgba(color))
    set_input(b, ("Roughness",), rough)
    set_input(b, ("Specular IOR Level", "Specular"), 0.6)
    m.diffuse_color = rgba(color)
    if pack is not None:
        tex = pack.node(m, "hair_long01_diffuse")
        # Use the recipe pigment to dye the measured light-brown source fibres.
        # Direct texture colour washes out the painted highlight at the crown.
        mix = m.node_tree.nodes.new("ShaderNodeMix")
        mix.data_type = "RGBA"
        mix.blend_type = "MULTIPLY"
        inputs = {s.identifier: s for s in mix.inputs}
        inputs["Factor_Float"].default_value = 1.0
        inputs["B_Color"].default_value = rgba([min(1.0, c / ref) for c, ref in zip(color, (0.36, 0.28, 0.20))])
        m.node_tree.links.new(tex.outputs["Color"], inputs["A_Color"])
        result = next(s for s in mix.outputs if s.identifier == "Result_Color")
        m.node_tree.links.new(result, b.inputs["Base Color"])
        fa_materials.alpha_cards(m, tex)
        set_input(b, ("Roughness",), 0.58)
        set_input(b, ("Anisotropic IOR Level",), 0.15)
        set_input(b, ("Specular IOR Level",), 0.15)
    return m


def brow_material(color, name="brow", pack=None):
    m, b = new_principled(name)
    set_input(b, ("Base Color",), rgba(color))
    set_input(b, ("Roughness",), 0.55)
    m.diffuse_color = rgba(color)
    if pack is not None:
        asset_id = "eyebrow001_diffuse" if name == "brow" else "eyelashes01_diffuse"
        fa_materials.alpha_cards(m, pack.node(m, asset_id))
    return m


def eye_material(iris_rgb, pack=None):
    """Pupil / iris / sclera from the MPFB low-poly eye UVs: the left eyeball (x > 0) is mapped around UV (0.705, 0.70) and the right one
    around (0.30, 0.30) (measured from the pack's low-poly.obj: the front vertex rings at 20 and 38 degrees sit 0.05 and 0.10 from those
    centres).  Distance to the nearer centre drives a constant colour ramp; the cornea is a glossy coat."""
    m, b = new_principled("eye")
    if pack is not None:
        tex = pack.node(m, "eyes_blue_diffuse")
        fa_materials.tinted_albedo(m, tex, iris_rgb, (0.2, 0.3, 0.42), iris_only=True)
        set_input(b, ("Roughness",), 0.24)
        fa_materials.corneal_coat(m)
        m.diffuse_color = rgba(iris_rgb)
        return m
    nt = m.node_tree
    tc = nt.nodes.new("ShaderNodeTexCoord")
    dists = []
    for cx, cy in ((0.705, 0.70), (0.30, 0.30)):
        vm = nt.nodes.new("ShaderNodeVectorMath")
        vm.operation = "DISTANCE"
        vm.inputs[1].default_value = (cx, cy, 0.0)
        nt.links.new(tc.outputs["UV"], vm.inputs[0])
        dists.append(vm)
    mn = nt.nodes.new("ShaderNodeMath")
    mn.operation = "MINIMUM"
    nt.links.new(dists[0].outputs["Value"], mn.inputs[0])
    nt.links.new(dists[1].outputs["Value"], mn.inputs[1])
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.interpolation = "CONSTANT"
    els = ramp.color_ramp.elements
    els[0].position, els[0].color = 0.0, (0.008, 0.008, 0.010, 1.0)           # pupil
    els[1].position, els[1].color = 0.058, rgba(iris_rgb)                      # iris
    e3 = els.new(0.112)
    e3.color = (0.80, 0.78, 0.74, 1.0)                                         # sclera (slightly off-white)
    nt.links.new(mn.outputs[0], ramp.inputs[0])
    nt.links.new(ramp.outputs[0], b.inputs["Base Color"])
    set_input(b, ("Roughness",), 0.28)
    set_input(b, ("Specular IOR Level", "Specular"), 0.5)
    set_input(b, ("Coat Weight", "Clearcoat"), 1.0)
    set_input(b, ("Coat Roughness", "Clearcoat Roughness"), 0.03)
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


def bounds_of(points):
    return (Vector([min(p[i] for p in points) for i in range(3)]),
            Vector([max(p[i] for p in points) for i in range(3)]))


def visible_figure_points(figure_objs):
    return [p for obj in figure_objs if not obj.hide_render for p in fa_surface.visible_positions(obj)]


def reference_stage(body, eye_objs, ref_foot_idx, *, figure_objs=None, mutant=False):
    """Measure the posed, dressed render surface once, before adding scenery.

    Foot indices belong to the armature cage; subdivision and garment masks change its
    topology. Associate the dressed contact surface with its closest posed cage vertex.
    This check stays independent of the minimum used to place the floor, even for a
    wide-legged pose with another body part or a piece of fabric between the feet.
    """
    objects = figure_objs if figure_objs is not None else [body, *eye_objs]
    surfaces = {obj: fa_surface.visible_positions(obj) for obj in objects if not obj.hide_render}
    points = [p for surface in surfaces.values() for p in surface]
    body_points = surfaces[body]
    lo, hi = bounds_of(points)
    body_bounds = bounds_of(body_points)
    height = hi.z - lo.z
    center = (lo + hi) * 0.5
    stage_bounds = (lo, hi)
    if mutant:
        add_hidden_helper_proxy(lo.z)
        stage_bounds = mesh_bounds_all(include_hidden=True)

    cage = eval_positions(body)
    tree = KDTree(len(cage))
    for i, point in enumerate(cage):
        tree.insert(point, i)
    tree.balance()
    foot_ids = set(ref_foot_idx)
    foot = [p for obj, surface in surfaces.items() if obj.get("fa_role") in ("body", "garment")
            for p in surface if tree.find(p)[1] in foot_ids]
    if not foot:
        raise RuntimeError("no visible sole or foot surface in the reference pose")

    # Use the actual posed eyeballs. The MPFB attachment is one mesh spanning
    # both eyes; partition in bind space so root translation/rotation do not swap sides.
    eye_points = [[], []]
    for obj in eye_objs:
        if obj.hide_render:
            continue
        verts = obj.data.vertices
        posed = eval_positions(obj)
        if len(verts) != len(posed):
            continue
        middle_x = (max(v.co.x for v in verts) + min(v.co.x for v in verts)) * 0.5
        for v, p in zip(verts, posed):
            eye_points[int(v.co.x < middle_x)].append(p)
    eye_centers = []
    if all(eye_points):
        for eye in eye_points:
            lower, upper = bounds_of(eye)
            eye_centers.append(tuple((lower + upper) * 0.5))
        eye_mid = (Vector(eye_centers[0]) + Vector(eye_centers[1])) * 0.5
    else:
        eye_mid = Vector((center.x, center.y, body_bounds[1].z - 0.105))
    ex, ey, ez = eye_mid
    face_points = [p for p in body_points if abs(p[0] - ex) <= 0.14
                   and abs(p[1] - ey) <= 0.23 and ez - 0.14 <= p[2] <= ez + 0.19]
    if not face_points:
        raise RuntimeError("no visible head surface at the reference-pose eye position")
    ctx = {"points": points, "bounds": stage_bounds, "body_bounds": body_bounds, "height": height, "center": tuple(center),
           "mutant": mutant, "foot_points": foot, "eye_centers": eye_centers,
           "face_center": (ex, ey, ez - 0.02), "face_points": face_points}
    return ctx, stage_bounds[0].z


def make_ground(z, size=40.0, albedo=(0.35, 0.35, 0.36), center_xy=(0.0, 0.0)):
    """An actual flat receiver around the subject sweeps up outside the camera orbit."""
    radius, bend = size * 0.22, size * 0.08
    profile = [(radius * 0.8, 0.0)]
    for i in range(13):
        a = 0.5 * math.pi * i / 12
        profile.append((radius + bend * math.sin(a), bend * (1 - math.cos(a))))
    profile.append((radius + bend, size * 0.4))
    sides = 96
    ring = [(math.cos(math.tau * i / sides), math.sin(math.tau * i / sides)) for i in range(sides)]
    verts = [(r * x, r * y, h) for r, h in profile for x, y in ring]
    polys = [tuple(range(sides))]
    for j in range(len(profile) - 1):
        for i in range(sides):
            k = (i + 1) % sides
            polys.append((j * sides + i, (j + 1) * sides + i, (j + 1) * sides + k, j * sides + k))
    o = mesh_object("Ground", verts, polys, role="ground")
    o.location = (center_xy[0], center_xy[1], z)
    m, b = new_principled("ground")
    set_input(b, ("Base Color",), rgba(albedo))
    set_input(b, ("Roughness",), 0.85)
    set_input(b, ("Specular IOR Level", "Specular"), 0.1)
    o.data.materials.append(m)
    return o


def make_world(color=(0.30, 0.32, 0.37), strength=0.25):
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


def add_lights(center, height, gain=1.0):
    """Three softboxes. The high backlight falls toward the soles, the off-axis key models the face,
    and the broad fill lifts the unlit eye. Scale power with distance squared when the subject changes."""
    dist = 2.7 * height
    k = (dist / 4.5) ** 2 * gain
    cx, cy, cz = center
    rig = (("key", -38.0, 44.0, 390.0, 2.1, (1.0, 0.91, 0.81)),
           ("fill", 42.0, 22.0, 120.0, 3.0, (0.82, 0.89, 1.0)),
           ("rim", 180.0, 77.0, 850.0, 2.3, (0.88, 0.94, 1.0)))
    made = {}
    for name, az, el, energy, size, col in rig:
        a, e = math.radians(az), math.radians(el)
        pos = (cx + dist * math.sin(a) * math.cos(e), cy - dist * math.cos(a) * math.cos(e), cz + dist * math.sin(e))
        ld = bpy.data.lights.new(name, "AREA")
        ld.shape = "DISK"
        ld.energy = energy * k
        ld.size = size * height / 1.7
        ld.color = col
        if name == "rim":
            # The overhead box defines the silhouette and floor shadow. Its reflection
            # spans several long01 cards at once and reads as a silver band across the part.
            ld.specular_factor = 0.0
        ld.use_shadow = True
        if hasattr(ld, "use_shadow_jitter"):
            ld.use_shadow_jitter = True
        o = bpy.data.objects.new(name, ld)
        link(o)
        o.location = pos
        aim(o, (cx, cy, cz + 0.1 * height))
        made[name] = o
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
    "front": {"az": 0.0, "el": 5.0},
    "side": {"az": 90.0, "el": 5.0},
    "three_quarter": {"az": 35.0, "el": 5.0},
    "back": {"az": 180.0, "el": 5.0},
    "face_closeup": {"az": 5.0, "el": 0.0, "target": "face", "lens": 85.0, "fill": 0.74},
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
    # Compute the physical camera projection once per mesh, including sensor fit,
    # shift and pixel aspect; subdivided silhouettes have tens of thousands of points.
    projection = cam.calc_matrix_camera(bpy.context.evaluated_depsgraph_get(),
                                        x=sc.render.resolution_x, y=sc.render.resolution_y,
                                        scale_x=sc.render.pixel_aspect_x, scale_y=sc.render.pixel_aspect_y)
    world_to_clip = projection @ cam.matrix_world.normalized().inverted()
    out = []
    for p in pts:
        c = world_to_clip @ Vector((*p, 1.0))
        if c.w == 0.0:
            out.append((0.5, 0.5))
        else:
            out.append((0.5 + 0.5 * c.x / c.w, 0.5 + 0.5 * c.y / c.w))
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
    lens = float(base.get("lens", 60.0))
    az, el = float(base["az"]), float(base["el"])
    fill = float(base.get("fill", 0.82))
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
        # Every full-body view uses the same target, lens, elevation and orbit
        # radius. Refitting to each azimuth makes side/front comparison drift.
        mode = "matched_orbit"
    place_camera(cam, target, az, el, dist)
    if mode == "face_analytic+fit":
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
    try:
        sc.view_settings.view_transform = "AgX"          # honest exposure: highlights roll off instead of clipping to white
    except TypeError:
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
            sc.eevee.taa_render_samples = int(spp)
            for attr, val in (("use_shadows", True), ("use_raytracing", False), ("use_gtao", True)):
                try:
                    setattr(sc.eevee, attr, val)
                except Exception:  # noqa: BLE001
                    notes.append(attr)
            return {"engine": "EEVEE_NEXT" if ident.endswith("NEXT") else "EEVEE", "samples": sc.eevee.taa_render_samples, "unsupported_settings": notes}
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


# =========================================================================== attachments, garments, skirt (shared by render_tape and render_probe)

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


def place_like(o, arm):
    """New objects live in ARMATURE space: give them the armature's world matrix (identity for MPFB rigs, possibly not for a glTF import)."""
    o.matrix_world = arm.matrix_world.copy()
    return o


def attach_object(export_dir, name, role, mat, S, arm, names, post=None):
    rec = read_att(export_dir, name)
    if rec is None:
        return None
    if mat.get("fa_texture_id") and not rec["uv_polys"]:
        raise ValueError(f"{name} has a material map but no exported face UVs")
    pos = fc.fit_binding(rec["bind"], rec["offset_spec"], S)
    info = {}
    if post is not None:
        pos, info = post(pos)
    faces, uv_faces = rec["polys"], rec["uv_polys"]
    if name == "long01":
        faces, uv_faces = fa_hair.long01_quads(faces), fa_hair.long01_quads(uv_faces)
    o = mesh_object(name, pos, faces, rec["uvs"], uv_faces, role=role)
    place_like(o, arm)
    set_weights(o, [[names[i] for i in row] for row in rec["skin_idx"]], rec["skin_w"])
    add_armature_modifier(o, arm)
    o.data.materials.append(mat)
    return o, rec, pos, info


def dress(export_dir, S, landmarks, arm, names, body, rd, slots, garments, fit_status, hide_under, arm_mask, eye_l, eye_r, notes,
          fitted_positions=None, baked_hair=False):
    """Skin, hair (parted away from the eyes), eyes, brows, lashes, the garments whose verdict is Fitted, the skirt, and a visible
    label for every garment that is NOT drawn.  -> dict with objs / labels / report / skirt / diagnostics."""
    out = {"objs": {"body": [body], "hair": [], "garment": [], "eyes": [], "brow": [], "lash": [], "skirt": []}, "labels": [], "report": {}, "skirt": None}
    bind = rd.get("bindings", {"garment": "base", "skirt": "panel"})
    pack = None
    if rd.get("texture_set") == "makehuman_system":
        from export_mpfb import Blender
        pack = fa_materials.PackTextures(Blender().user_data)
        if not body.data.uv_layers.active:
            raise ValueError("The skin texture requires the exported body UV map")
    skin_mat, sss = skin_material(rd.get("skin", {}), pack)
    out["sss_inputs"] = sss
    body.data.materials.clear()
    body.data.materials.append(skin_mat)
    out["skin_rgb"] = rd.get("skin", {}).get("albedo", [0.62, 0.40, 0.31])
    hair_cfg = rd.get("hair", {})
    hair_rgb = hair_cfg.get("color", [0.09, 0.05, 0.03])
    brow_rgb = rd.get("brows", {}).get("color", [0.025, 0.015, 0.01])
    iris_spec = slots.get("iris", {})
    iris_rgb = iris_spec.get("albedo") or iris_spec.get("color") or rd.get("eyes", {}).get("iris", [0.20, 0.30, 0.42])
    out["hair_rgb"], out["brow_rgb"] = hair_rgb, brow_rgb
    hid = hair_cfg.get("id")
    if hid:
        mat = hair_material(hair_rgb, pack=pack if hid == "long01" else None)
        # Long01 already has a centre part. Applying the bob fringe remap
        # stretches triangles spanning that part into a shelf above the brow.
        post = None if hid == "long01" else lambda p: fa_hair.part_hair(p, eye_l, eye_r)
        r = attach_object(export_dir, hid, "hair", mat, S, arm, names, post)
        if r is None:
            notes.append("hair %s is not in the export attachments (export with this preset)" % hid)
            out["report"]["hair:" + hid] = "NOT DRAWN: not in export"
        else:
            if hid == "long01":
                if baked_hair:
                    # The physics track addresses source cage indices. Subdivide after playback.
                    smooth = r[0].modifiers.new("Smooth realized hair", "SUBSURF")
                    smooth.levels = smooth.render_levels = 2
                else:
                    fa_hair.finish_long01(r[0], body)
            out["objs"]["hair"].append(r[0])
            out["hair_part"] = r[3] if post else {"method": "native centre part", "moved_vertices": 0}
            out["hair_violations"] = fa_hair.clearance_violations(r[2], eye_l, eye_r)
    for kind, key, default_id in (("eyes", "eyes", "low-poly"), ("eyebrows", "brow", "eyebrow001"),
                                  ("eyelashes", "lash", "eyelashes01")):
        nm = rd.get(kind, {}).get("id") or default_id
        maps = pack if nm == default_id else None
        if key == "eyes":
            mat = eye_material(iris_rgb, maps)
        else:
            mat = brow_material(brow_rgb if key == "brow" else [0.012, 0.01, 0.01], key, maps)
        r = attach_object(export_dir, nm, key, mat, S, arm, names)
        if r is None:
            notes.append("%s %s is not in the export attachments" % (kind, nm))
            continue
        o, rec, pos, _ = r
        out["objs"][key].append(o)
        if key == "eyes":
            out["iris_verts"] = add_iris_attribute(o, pos)
    hide = set()
    inner_layers = []
    for gid in garments:
        fitted, why = verdict_of(fit_status.get(gid, "MissingVerdict"))
        if gid not in fit_status:
            out["report"][gid] = "NOT DRAWN: no fit_status verdict in the tape (a missing verdict means not drawn)"
        elif not fitted:
            out["report"][gid] = "NOT DRAWN: FitStatus %s" % why
        elif gid == SKIRT_ID:
            out["report"][gid] = "DRAWN (self-authored skirt)"
        else:
            slot = bind.get("garment:" + gid, bind.get("garment", "base"))
            spec = slots.get(slot, {"albedo": [0.20, 0.38, 0.42], "roughness": 0.7})
            cloth = fa_surface.garment_material(slot_material(slot, spec), gid, pack)
            r = attach_object(export_dir, gid, "garment", cloth, S, arm, names)
            if r is None:
                out["report"][gid] = "NOT DRAWN: not in the export attachments (export it with this preset)"
            else:
                obj, rec, pos, _ = r
                fit = fa_surface.supplied_fit(gid, len(obj.data.vertices), fitted_positions)
                if fit is not None:
                    set_vertex_positions(obj, fit)
                    pos = fit
                inner_layers.append((obj, pos, fa_skirt.arm_mask(names, rec["skin_idx"], rec["skin_w"])))
                out["objs"]["garment"].append(obj)
                out["report"][gid] = "DRAWN (fit %s, slot %s)" % (why, slot)
                if hide_under:
                    hide.update(rec["delete_verts"])
    if hide:
        vg = body.vertex_groups.new(name="fa_under_garment")
        vg.add(sorted(hide), 1.0, "REPLACE")
        md = body.modifiers.new("HideUnderGarment", "MASK")
        md.vertex_group = "fa_under_garment"
        md.invert_vertex_group = True
        md.show_viewport = False        # the depsgraph (bounds, ray casts, oracle) sees the whole body; the render hides it
    out["hidden_body_vertices"] = len(hide)
    fa_surface.smooth_surface(body, render_only=True)
    if out["report"].get(SKIRT_ID, "").startswith("DRAWN"):
        sk = fa_surface.fit_skirt(S, landmarks["joints"], arm_mask, inner_layers, fitted_positions)
        o = mesh_object("Skirt", sk["positions"], sk["quads"], sk["uvs"], sk["quad_uv"], role="skirt")
        place_like(o, arm)
        slot = bind.get("skirt", "panel")
        skirt_mat = slot_material(slot, slots.get(slot, {"albedo": [0.55, 0.18, 0.24], "roughness": 0.75}))
        o.data.materials.append(fa_surface.fabric_material(skirt_mat))
        fa_surface.finish_skirt(o)
        out["objs"]["skirt"].append(o)
        out["skirt"] = {"obj": o, "data": sk, "cache": {}}
    for obj in out["objs"]["garment"]:
        fa_surface.smooth_surface(obj)
    for gid, txt in sorted(out["report"].items()):
        if txt.startswith("NOT DRAWN"):
            out["labels"].append(build_label("%s: %s" % (gid, txt)))
    return out


# =========================================================================== shared self-checks

class Checks:
    def __init__(self):
        self.items = []

    def add(self, r, detail=""):
        self.items.append(r)
        say("RT_CHECK %s %s %s %s" % ("PASS" if r["ok"] else "FAIL", r["name"],
                                      " ".join("%s=%s" % (k, ("%.4g" % v) if isinstance(v, float) else v) for k, v in sorted(r.items())
                                               if k not in ("name", "ok") and not isinstance(v, (list, dict))), detail))
        return r["ok"]

    def failed(self):
        return [r["name"] for r in self.items if not r["ok"]]


def eye_clearance(cam, eye_l, eye_r, name="eye_clearance"):
    """Rays from the camera to each eye (centre and three offsets); a hair / garment / skirt hit in front of the eye blocks it."""
    blocked, total = 0, 0
    for e in (eye_l, eye_r):
        for dx, dz in ((0, 0), (0.008, 0.0), (-0.008, 0.0), (0.0, 0.006)):
            total += 1
            role, hitd, d = ray_first_hit(cam.location, (e[0] + dx, e[1] - 0.012, e[2] + dz))
            if role in ("hair", "garment", "skirt"):
                blocked += 1
    return ck.clearance_check(blocked, total, name)


def brow_lash_visibility(cam, brow_objs, lash_objs):
    vis, total = 0, 0
    for objs in (brow_objs, lash_objs):
        for o in objs:
            pts = eval_positions(o)
            for i in range(0, len(pts), max(1, len(pts) // 24)):
                total += 1
                role, hitd, d = ray_first_hit(cam.location, pts[i])
                if role in (None, "brow", "lash") or hitd >= d - 0.004:
                    vis += 1
    return ck.visibility_check(vis, total, 0.6, "brows_lashes_visible")


def check_contact_shadow(checks, body, foot_idx, ground, cam_rec, res, out_dir, figure_objs, labels, foot_points=None):
    """CONTACT: min z of the evaluated foot vertices in the reference pose within 5 mm of the ground OBJECT's z.
    SHADOW: coverage of shadowed pixels on the ground around the feet in a check render of `cam_rec` (beauty + figure mask)."""
    if foot_points is None:
        pts = eval_positions(body)
        foot_points = [pts[i] for i in foot_idx]
    foot = foot_points
    checks.add(ck.contact_check(min(p[2] for p in foot), ground.location.z))
    sc = bpy.context.scene
    cam = cam_rec["obj"]
    sc.camera = cam
    for lab in labels:
        attach_label_to_camera(lab, cam, cam_rec["lens"])
    fpath = os.path.join(out_dir, "check_front_beauty.png")
    mpath = os.path.join(out_dir, "check_front_mask.png")
    spp_old = None
    try:
        spp_old = sc.eevee.taa_render_samples
        sc.eevee.taa_render_samples = min(spp_old, 16)
    except Exception:  # noqa: BLE001
        spp_old = None
    render_to(fpath, res)
    if spp_old is not None:
        sc.eevee.taa_render_samples = spp_old
    render_mask(mpath, res, figure_objs)
    sc.render.resolution_x, sc.render.resolution_y = res
    inner, outer = ground_rects(cam, [(p[0], p[1]) for p in foot], ground.location.z)
    r = ck.shadow_check_files(fpath, mpath, inner, outer)
    r["inner_rect"], r["outer_rect"] = list(inner), list(outer)
    checks.add(r)
    checks.add(ck.ground_clip_check_files(fpath, mpath, outer))
    return foot
