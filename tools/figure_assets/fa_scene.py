"""Build the chair from the native performance's measured seat in metres."""
import json
import math
from pathlib import Path

import bpy
from mathutils import Vector
import fa_stage


def box(name, center, size, material, radius):
    if len(center) != 3 or len(size) != 3 or not all(math.isfinite(v) for v in (*center, *size)):
        raise ValueError(f"invalid chair box: {name}")
    if min(size) <= 0:
        raise ValueError(f"nonpositive chair dimension: {name}")
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=center)
    obj = bpy.context.object
    obj.name = name
    for v in obj.data.vertices:
        for a, length in enumerate(size):
            v.co[a] *= length
    obj.data.materials.append(material)
    edge = obj.modifiers.new("Rounded edges", "BEVEL")
    edge.width = radius
    edge.segments = 3
    obj.modifiers.new("Face normals", "WEIGHTED_NORMAL")
    obj["fa_role"] = "chair"
    return obj


def stage_scene(fig, tape, scene_path):
    if scene_path is None:
        return []
    data = json.loads(Path(scene_path).read_text())
    if data.get("format") != "gibson.figure.scene.v1":
        raise ValueError("unsupported native performance scene")
    for source, header in (("tape_bin_sha256", "bin_sha256"), ("body_rev_id", "body_rev_id"),
                           ("fit_id", "fit_id"), ("present_id", "present_id")):
        if data.get(source) != tape.header[header]:
            raise ValueError(f"scene/tape identity mismatch: {source}")
    fig.scene_eye_origin = eye_center(fig)
    if data["chair"] is None:
        if data.get("seat") is not None:
            raise ValueError("seat without a chair")
        return []
    chair = data["chair"]
    if data["seat"] != chair["seat"]:
        raise ValueError("stage and solver seat geometry disagree")
    cx, cy, cz = chair["seat"]["center"]
    w, d, t = chair["seat"]["size"]
    if abs(cz + t / 2 - chair["contact_top_m"]) > 1e-5 or chair["walk_to_chair_gap_m"] < 0.02:
        raise ValueError("unqualified walk clearance or seated surface")
    wood = fa_stage.slot_material("chair_wood", {"albedo": [0.20, 0.095, 0.054], "roughness": 0.47})
    frame = fa_stage.slot_material("chair_frame", {"albedo": [0.052, 0.063, 0.069], "roughness": 0.41})
    objects = [box("Seat", [cx, cy, cz], [w, d, t], wood, 0.008),
               box("Back", chair["back"]["center"], chair["back"]["size"], wood, 0.009)]
    height = cz - t / 2
    for x in (-1, 1):
        for y in (-1, 1):
            objects.append(box(f"Chair leg {x} {y}",
                               [cx + x * (w / 2 - 0.031), cy + y * (d / 2 - 0.031), height / 2],
                               [0.031, 0.031, height], frame, 0.003))
    print("SCENE_CHAIR", len(objects), "walk_gap_m", chair["walk_to_chair_gap_m"], "seat_top_m", cz + t / 2)
    return objects


def eye_center(fig):
    eye, = fig.objs["eyes"]
    points = fa_stage.eval_positions(eye)
    return sum((Vector(p) for p in points), Vector((0, 0, 0))) / len(points)


def track_close(fig, rec):
    if getattr(fig, "scene_eye_origin", None) is None or rec["kind"] != "face_closeup":
        return
    target = Vector(rec["target"]) + eye_center(fig) - fig.scene_eye_origin
    fa_stage.place_camera(rec["obj"], target, rec["az"], rec["el"], rec["dist"])
