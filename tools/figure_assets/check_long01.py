"""Check long01 against the two actual Ex1 camera sightlines and an opaque obstruction.

Run under heavy with blender -b --python-exit-code 1 --python check_long01.py --
    --export EXPORT --tape GENERIC.tape.json --report render.json
"""
import argparse
import math
import sys
from pathlib import Path

from mathutils import Vector
from mathutils.bvhtree import BVHTree

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fa_common as fc
import fa_hair
import fa_stage as stage
import fa_tape as ft


def iris_targets(eye, posed):
    """Sample both eyes' actual triangle surfaces inside their blue_eye.png iris discs."""
    centers = ((0.705, 0.70), (0.30, 0.30))
    targets = [set(), set()]
    for face, uv_face in zip(eye["polys"], eye["uv_polys"]):
        for i in range(6):
            for j in range(6 - i):
                bary = (i / 5, j / 5, (5 - i - j) / 5)
                uv = [sum(w * eye["uvs"][k][d] for k, w in zip(uv_face, bary)) for d in (0, 1)]
                for side, center in enumerate(centers):
                    if math.dist(uv, center) < 0.109:
                        point = tuple(round(sum(w * posed[k][d] for k, w in zip(face, bary)), 7) for d in range(3))
                        targets[side].add(point)
    if not all(targets):
        raise ValueError("Missing samples from a left or right iris")
    return [Vector(p) for side in targets for p in sorted(side)]


def count_blocked(vertices, faces, targets, camera):
    tree = BVHTree.FromPolygons(vertices, faces, all_triangles=True)
    blocked = 0
    for point in targets:
        ray = point - camera
        if tree.ray_cast(camera, ray.normalized(), ray.length - 1e-5)[0] is not None:
            blocked += 1
    return blocked


def max_edge_stretch(faces, before, after):
    edges = {tuple(sorted((a, b))) for face in faces for a, b in zip(face, face[1:] + face[:1])}
    return max(math.dist(after[a], after[b]) / math.dist(before[a], before[b]) for a, b in edges
               if math.dist(before[a], before[b]) > 1e-8)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--export", required=True, type=Path)
    parser.add_argument("--tape", required=True)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])
    tape = ft.read_tape(args.tape)
    report = stage.load_json(args.report)
    if fc.sha256_file(args.export / "manifest.json") != tape.header["asset_manifest_sha256"]:
        raise ValueError("The tape belongs to another export")
    for key in ("present_id", "body_rev_id", "fit_id", "controls", "frame_count"):
        if report["tape"][key] != tape.header[key]:
            raise ValueError("The render report differs from the tape at " + key)
    if "bin_sha256" in report["tape"] and report["tape"]["bin_sha256"] != tape.header["bin_sha256"]:
        raise ValueError("The reported binary tape digest differs")
    if "hair_long01_diffuse" not in report["figure"]["textures"]:
        raise ValueError("The report does not use long01")

    shaped, _, _ = ft.shape_positions(args.export, tape.header["controls"])
    skeleton = stage.load_json(args.export / "skeleton.json")
    bind, _ = ft.bind_frames(skeleton, shaped, report["joint_frame"], args.export)
    joints = stage.load_json(args.export / "landmarks.json")["joints"]
    hair = stage.read_att(args.export, "long01")
    native = fc.fit_binding(hair["bind"], hair["offset_spec"], shaped)
    eye = stage.read_att(args.export, "low-poly")
    eye_base = fc.fit_binding(eye["bind"], eye["offset_spec"], shaped)
    centers = (fc.centroid(shaped, joints[key]) for key in ("joint-l-eye", "joint-r-eye"))
    remapped, info = fa_hair.part_hair(native, *centers)
    print("HAIR_FRINGE_CONTROL moved=%d max_displacement_m=%.7f max_edge_stretch=%.3f" %
          (info["moved_vertices"], max(map(math.dist, native, remapped)), max_edge_stretch(hair["polys"], native, remapped)))

    failures = []
    for name in ("still:front", "still:face_closeup"):
        shot = report["stage"]["cameras"][name]
        matrices = ft.skin_matrices(skeleton, bind, tape, shot["tape_frame"])
        az, el = math.radians(shot["az"]), math.radians(shot["el"])
        offset = Vector((math.sin(az) * math.cos(el), -math.cos(az) * math.cos(el), math.sin(el)))
        camera = Vector(shot["target"]) + shot["dist"] * offset
        posed_eyes = [Vector(p) for p in ft.lbs_positions(eye_base, eye["skin_idx"], eye["skin_w"], matrices)]
        targets = iris_targets(eye, posed_eyes)
        for label, points in (("native", native), ("bob_remap", remapped)):
            posed = ft.lbs_positions(points, hair["skin_idx"], hair["skin_w"], matrices)
            hits = count_blocked(posed, hair["polys"], targets, camera)
            print("HAIR_RAYS %s %s %d/%d" % (name, label, hits, len(targets)))
            if label == "native" and hits:
                failures.append(name + " hair covers an iris")
        # Move a duplicate of the two opaque eyes 3 cm towards the camera.
        toward_camera = (camera - targets[0]).normalized() * 0.03
        control = count_blocked([p + toward_camera for p in posed_eyes], eye["polys"], targets, camera)
        print("HAIR_RAYS %s opaque_eye_control %d/%d" % (name, control, len(targets)))
        if control != len(targets):
            failures.append(name + " opaque control missed an iris sample")
    if report["figure"]["hair_part"]["moved_vertices"] != 0:
        failures.append("the rendered long01 was moved by the fringe remap")
    if failures:
        raise ValueError(", ".join(failures))
    print("LONG01_CLEARANCE PASS")


if __name__ == "__main__":
    main()
