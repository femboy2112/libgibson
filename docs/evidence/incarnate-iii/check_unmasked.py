"""Measure all fitted surfaces, then force a hidden suit patch through the real skirt."""
import argparse
import json
import sys
from pathlib import Path

import bpy
from mathutils.bvhtree import BVHTree

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "tools" / "figure_assets"))
import fa_common
import fa_tape
import render_tape


def world_tree(obj, masked):
    """Use the render modifier stack. Toggle only visibility masks between measurements."""
    state = [(m, m.show_viewport) for m in obj.modifiers]
    try:
        for modifier, _ in state:
            modifier.show_viewport = modifier.show_render and (masked or modifier.type != "MASK")
        bpy.context.view_layer.update()
        ev = obj.evaluated_get(bpy.context.evaluated_depsgraph_get())
        mesh = ev.to_mesh()
        try:
            return BVHTree.FromPolygons([ev.matrix_world @ v.co for v in mesh.vertices],
                                        [tuple(face.vertices) for face in mesh.polygons])
        finally:
            ev.to_mesh_clear()
    finally:
        for modifier, enabled in state:
            modifier.show_viewport = enabled
        bpy.context.view_layer.update()


def measurements(body, suit, skirt, masked):
    trees = [world_tree(obj, masked) for obj in (body, suit, skirt)]
    return {name: len(trees[i].overlap(trees[j]))
            for name, i, j in (("body_suit", 0, 1), ("body_skirt", 0, 2), ("suit_skirt", 1, 2))}


def inject_crossing(suit):
    """Pull a connected neighborhood of actual hidden cage vertices outward by 0.22 m."""
    group = suit.vertex_groups["fa_under_skirt"].index
    hidden = [v for v in suit.data.vertices if any(g.group == group and g.weight > 0.5 for g in v.groups)]
    zs = sorted(v.co.z for v in hidden)
    zmid = zs[len(zs) // 2]
    band = [v for v in hidden if abs(v.co.z - zmid) < 0.04]
    seed = max(band, key=lambda v: v.co.x)
    center = seed.co.copy()
    selected = [v for v in hidden if (v.co - center).length < 0.09]
    for v in selected:
        v.co.x += 0.22 * (1.0 - (v.co - center).length / 0.09)
    suit.data.update()
    bpy.context.view_layer.update()
    return len(selected)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--export", type=Path, required=True)
    parser.add_argument("--tape", required=True)
    parser.add_argument("--fitted", required=True)
    parser.add_argument("--inject", action="store_true")
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])
    args.joint_frame = "bone"
    tape = fa_tape.read_tape(args.tape)
    if fa_common.sha256_file(args.export / "manifest.json") != tape.header["asset_manifest_sha256"]:
        raise ValueError("Tape/export manifest identity mismatch")
    preset = json.loads((args.export / "figure_asset.json").read_text())["preset"]
    figure = render_tape.build_figure(args, tape, preset, False, [])
    render_tape.set_pose(figure, tape, 0)
    suit, = figure.objs["garment"]
    skirt, = figure.objs["skirt"]
    meshes = figure.body, suit, skirt
    if args.inject:
        print("HIDDEN_SUIT_VERTICES_DISPLACED", inject_crossing(suit))
    masked = measurements(*meshes, masked=True)
    physical = measurements(*meshes, masked=False)
    print("DISPLAY_SURFACE_CROSSINGS", json.dumps(masked, sort_keys=True))
    print("UNMASKED_SURFACE_CROSSINGS", json.dumps(physical, sort_keys=True))
    # The pair on which the retained adapter enforced zero crossings qualifies
    # only on the independently evaluated unmasked geometry. Body intersection
    # counts are also retained because the MPFB proxy fit excludes deleted skin.
    if physical["suit_skirt"]:
        print("UNMASKED_SUIT_SKIRT FAIL")
        raise SystemExit(5)
    print("UNMASKED_SUIT_SKIRT PASS")


if __name__ == "__main__":
    main()
