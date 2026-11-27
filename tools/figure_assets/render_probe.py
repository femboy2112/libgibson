#!/usr/bin/env python3
"""Blender script for the MEASURE lease: render one 1920x1080 still and 24 frames at 1280x720 of
the exported body doing a simple idle + arm-raise on its rig, and print machine-readable timings.
UNRUN when committed (heavy-only); written against the Blender 4.5 API.

    "$BLENDER" -b --python tools/figure_assets/render_probe.py -- \
        --export "$OUT/export" --out "$OUT/render" [--engine auto|eevee|cycles] [--spp 32] [--threads 4]

Input: --export DIR (uses DIR/body_shaped.glb written by export_mpfb.py) or --glb FILE.
Engine selection (auto): EEVEE (Next) is smoke-tested in a CHILD Blender process first, because a
headless EEVEE failure can kill the process; if the child fails, Cycles on the CPU is used
(--spp samples, OpenImageDenoise when the build has it).  An exception during the real EEVEE
render also falls back to Cycles.

Output lines (grep 'PROBE'):
    PROBE engine=<EEVEE_NEXT|CYCLES> still_s=<s> frame_s_mean=<s> frame_s_max=<s> ...
    PROBE_FRAME <i> seconds=<s>
Exit codes: 0 ok, 2 bad arguments / missing input, 3 failure.

Ground: an opaque light-grey plane that receives the key light's shadow (no shadow-catcher
transparency), so the shadow pixels under the feet are in the PNG.  Colours are flat Principled
materials from export_mpfb.py; no textures.
"""
import argparse
import json
import math
import os
import subprocess
import sys
import time
import traceback

import bpy
from mathutils import Matrix, Vector

STILL_RES = (1920, 1080)
FRAME_RES = (1280, 720)


def say(*a):
    print(*a, flush=True)


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
    ap.add_argument("--child-smoke", action="store_true", help="internal: EEVEE smoke test, renders 64x64 and exits")
    return ap.parse_args(argv)


def clear_scene():
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)


def import_glb(path):
    clear_scene()
    bpy.ops.import_scene.gltf(filepath=path)
    arms = [o for o in bpy.context.scene.objects if o.type == "ARMATURE"]
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if not arms or not meshes:
        raise RuntimeError("glTF import produced %d armatures and %d meshes" % (len(arms), len(meshes)))
    return arms[0], meshes


def world_bounds(meshes):
    lo = Vector((1e9, 1e9, 1e9))
    hi = Vector((-1e9, -1e9, -1e9))
    for o in meshes:
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector((min(lo[i], w[i]) for i in range(3)))
            hi = Vector((max(hi[i], w[i]) for i in range(3)))
    return lo, hi


def aim(obj, target):
    d = Vector(target) - obj.location
    obj.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()


def build_stage(meshes, res):
    sc = bpy.context.scene
    lo, hi = world_bounds(meshes)
    height = hi.z - lo.z
    zc = 0.5 * (lo.z + hi.z)
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.render.resolution_percentage = 100
    sc.view_settings.view_transform = "Standard"
    # ground
    bpy.ops.mesh.primitive_plane_add(size=24.0, location=(0.0, 0.0, lo.z))
    ground = bpy.context.active_object
    ground.name = "Ground"
    m = bpy.data.materials.new("ground")
    m.use_nodes = True
    b = m.node_tree.nodes.get("Principled BSDF")
    if b is not None:
        b.inputs["Base Color"].default_value = (0.55, 0.55, 0.58, 1.0)
        b.inputs["Roughness"].default_value = 0.9
    ground.data.materials.append(m)
    # world: soft ambient
    w = bpy.data.worlds.new("probe_world") if sc.world is None else sc.world
    sc.world = w
    w.use_nodes = True
    bg = w.node_tree.nodes.get("Background")
    if bg is not None:
        bg.inputs["Color"].default_value = (0.35, 0.37, 0.42, 1.0)
        bg.inputs["Strength"].default_value = 0.6
    # camera: full body in frame, character faces -Y so the camera sits on -Y
    cam_data = bpy.data.cameras.new("probe_cam")
    cam_data.lens = 50.0
    cam_data.sensor_fit = "VERTICAL"
    cam_data.sensor_height = 24.0
    cam = bpy.data.objects.new("probe_cam", cam_data)
    bpy.context.scene.collection.objects.link(cam)
    vfov = 2.0 * math.atan(12.0 / 50.0)
    dist = (height * 1.18 * 0.5) / math.tan(vfov * 0.5)
    cam.location = (0.0, -dist, zc)
    aim(cam, (0.0, 0.0, zc))
    sc.camera = cam
    # three-point lights, aimed at the chest
    chest = (0.0, 0.0, lo.z + 0.65 * height)
    for name, loc, power, size in (("key", (-0.9 * dist, -0.8 * dist, lo.z + 1.5 * height), 900.0, 1.5),
                                   ("fill", (0.9 * dist, -0.6 * dist, lo.z + 0.9 * height), 300.0, 2.5),
                                   ("rim", (0.3 * dist, 0.9 * dist, lo.z + 1.3 * height), 500.0, 1.0)):
        ld = bpy.data.lights.new(name, "AREA")
        ld.energy = power
        ld.size = size
        lo_ = bpy.data.objects.new(name, ld)
        bpy.context.scene.collection.objects.link(lo_)
        lo_.location = loc
        aim(lo_, chest)
    return {"height_m": height, "camera_distance_m": dist}


def find_bones(arm):
    import re
    res = {}
    for pb in arm.pose.bones:
        n = pb.name.lower()
        m = re.match(r"^upperarm(0?1)?[._]?(l|r)$", n)
        if m:
            res["arm_" + m.group(2)] = pb
        elif re.match(r"^spine[_]?0?3$", n) or n in ("spine03", "spine_03"):
            res["spine"] = pb
    return res


def keyframe_idle_and_arm_raise(arm, n_frames):
    """Armature-space rotations converted to bone-local quaternions (independent of the bone roll)."""
    arm.data.pose_position = "POSE"
    bones = find_bones(arm)
    if "arm_l" not in bones or "arm_r" not in bones:
        raise RuntimeError("no upperarm bones found; bones are: %s" % [b.name for b in arm.pose.bones][:20])
    rest = {}
    wm = arm.matrix_world.to_3x3().normalized()   # world -> armature-space rotation basis (identity if the importer baked it)
    for key, pb in bones.items():
        rest[key] = pb.bone.matrix_local.to_3x3()
        pb.rotation_mode = "QUATERNION"
    sc = bpy.context.scene
    sc.frame_start, sc.frame_end = 1, n_frames
    for f in range(1, n_frames + 1):
        t = (f - 1) / float(n_frames)          # 0..1, loops
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


def set_engine(name, spp, threads):
    sc = bpy.context.scene
    sc.render.threads_mode = "FIXED"
    sc.render.threads = threads
    if name == "EEVEE":
        last = None
        for ident in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"):
            try:
                sc.render.engine = ident
                try:
                    sc.eevee.taa_render_samples = 64
                except Exception:  # noqa: BLE001
                    pass
                return {"engine": "EEVEE_NEXT" if ident.endswith("NEXT") else "EEVEE", "samples": 64}
            except TypeError as exc:
                last = exc
        raise RuntimeError("no EEVEE engine id accepted: %s" % last)
    sc.render.engine = "CYCLES"
    cy = sc.cycles
    cy.device = "CPU"
    cy.samples = spp
    denoise = "none"
    try:
        cy.use_denoising = True
        cy.denoiser = "OPENIMAGEDENOISE"
        denoise = "OIDN"
    except Exception:  # noqa: BLE001
        try:
            cy.use_denoising = False
        except Exception:  # noqa: BLE001
            pass
    return {"engine": "CYCLES", "samples": spp, "denoise": denoise}


def render_to(path, res):
    sc = bpy.context.scene
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.render.image_settings.file_format = "PNG"
    sc.render.image_settings.color_mode = "RGB"
    sc.render.filepath = path
    t = time.time()
    bpy.ops.render.render(write_still=True)
    return time.time() - t


def eevee_smoke(glb):
    """Runs in a child Blender: render 64x64 with EEVEE. rc 0 and marker line mean EEVEE works headless here."""
    arm, meshes = import_glb(glb)
    build_stage(meshes, (64, 64))
    info = set_engine("EEVEE", 0, 2)
    out = os.path.join(os.environ.get("TMPDIR", "/tmp"), "probe_smoke_%d.png" % os.getpid())
    render_to(out, (64, 64))
    ok = os.path.isfile(out)
    if ok:
        os.remove(out)
    say("SMOKE_EEVEE %s engine=%s" % ("OK" if ok else "NO_FILE", info["engine"]))
    return 0 if ok else 1


def eevee_usable(glb):
    try:
        r = subprocess.run([bpy.app.binary_path, "-b", "--python", os.path.abspath(__file__), "--", "--child-smoke", "--glb", glb],
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=240, text=True)
    except (subprocess.TimeoutExpired, OSError) as exc:
        say("PROBE_NOTE eevee smoke child failed to run: %s" % exc)
        return False, "child failed: %s" % exc
    ok = r.returncode == 0 and "SMOKE_EEVEE OK" in r.stdout
    if not ok:
        tail = " | ".join(r.stdout.strip().splitlines()[-4:])
        say("PROBE_NOTE eevee smoke child rc=%d tail: %s" % (r.returncode, tail[:400]))
        return False, "child rc=%d" % r.returncode
    return True, "child ok"


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    a = parse_args(argv)
    glb = a.glb or (os.path.join(a.export, "body_shaped.glb") if a.export else None)
    if not glb or not os.path.isfile(glb):
        say("ERROR: need --export DIR containing body_shaped.glb, or --glb FILE (got %r)" % (glb and os.path.basename(glb)))
        return 2
    if a.child_smoke:
        return eevee_smoke(glb)
    if not a.out:
        say("ERROR: --out is required")
        return 2
    os.makedirs(a.out, exist_ok=True)

    engine_note = ""
    want = a.engine
    if want in ("auto", "eevee"):
        ok, why = eevee_usable(glb)
        engine_note = "eevee smoke: " + why
        want = "EEVEE" if ok else ("CYCLES" if a.engine == "auto" else "EEVEE")
        if not ok and a.engine == "eevee":
            say("PROBE_NOTE --engine eevee requested but the smoke test failed; trying anyway")
    else:
        want = "CYCLES"

    arm, meshes = import_glb(glb)
    stage = build_stage(meshes, STILL_RES)
    bones = keyframe_idle_and_arm_raise(arm, a.frames)
    info = set_engine(want, a.spp, a.threads)
    sc = bpy.context.scene
    fell_back = None
    still_s = 0.0
    if not a.no_still:
        sc.frame_set(1)
        try:
            still_s = render_to(os.path.join(a.out, "still_1920x1080.png"), STILL_RES)
        except Exception as exc:  # noqa: BLE001
            if info["engine"] == "CYCLES":
                raise
            fell_back = "EEVEE render raised %r" % (exc,)
            say("PROBE_NOTE %s; falling back to Cycles CPU" % fell_back)
            info = set_engine("CYCLES", a.spp, a.threads)
            still_s = render_to(os.path.join(a.out, "still_1920x1080.png"), STILL_RES)
    times = []
    for f in range(1, a.frames + 1):
        sc.frame_set(f)
        try:
            dt = render_to(os.path.join(a.out, "frame_%03d_1280x720.png" % f), FRAME_RES)
        except Exception as exc:  # noqa: BLE001
            if info["engine"] == "CYCLES":
                raise
            fell_back = "EEVEE frame %d raised %r" % (f, exc)
            say("PROBE_NOTE %s; falling back to Cycles CPU" % fell_back)
            info = set_engine("CYCLES", a.spp, a.threads)
            dt = render_to(os.path.join(a.out, "frame_%03d_1280x720.png" % f), FRAME_RES)
        times.append(dt)
        say("PROBE_FRAME %d seconds=%.2f" % (f, dt))
    mean = sum(times) / len(times) if times else 0.0
    mx = max(times) if times else 0.0
    res = {"engine": info["engine"], "still_s": still_s, "frame_s_mean": mean, "frame_s_max": mx, "frames": len(times),
           "still_res": list(STILL_RES), "frame_res": list(FRAME_RES), "settings": info, "engine_note": engine_note,
           "fell_back": fell_back, "rig_bones_animated": bones, "stage": stage}
    with open(os.path.join(a.out, "probe.json"), "w", encoding="utf-8") as f:
        json.dump(res, f, indent=1, sort_keys=True)
        f.write("\n")
    say("PROBE engine=%s still_s=%.2f frame_s_mean=%.2f frame_s_max=%.2f frames=%d samples=%s fell_back=%s" % (
        info["engine"], still_s, mean, mx, len(times), info.get("samples"), "yes" if fell_back else "no"))
    return 0


if __name__ == "__main__":
    try:
        rc = main()
    except Exception:  # noqa: BLE001
        traceback.print_exc()
        say("PROBE_FAILED see traceback above")
        rc = 3
    say("PROBE_RC %d" % rc)
    sys.exit(rc)
