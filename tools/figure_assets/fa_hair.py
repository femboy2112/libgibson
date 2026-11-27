#!/usr/bin/env python3
"""Hair parting away from the eyes (pure python, no bpy).  Fixes viewed defect 5 (bob hair covered one eye).

    python3 -I tools/figure_assets/fa_hair.py --selftest

`part_hair` remaps the lateral coordinate of hair vertices that sit IN FRONT of the face plane within the eye/brow height
window, so a central gap |x| < gap is cleared (the fringe is pushed toward the temples, monotone, no vertex crosses another).
It is a no-op for styles with no fringe (a ponytail).  The renderer ALSO ray-casts from the camera to the eyes; that
check, not this function, is the arbiter.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)


def _ramp(x, lo, hi):
    if hi == lo:
        return 1.0 if x >= hi else 0.0
    t = min(1.0, max(0.0, (x - lo) / (hi - lo)))
    return t * t * (3.0 - 2.0 * t)


def part_hair(positions, eye_l, eye_r, below=0.045, above=0.075, ramp=0.02, half_width=0.115, clearance=0.032, forward=0.012, y_ramp=0.02):
    """-> (new positions, info).  Eye centres eye_l/eye_r are (x, y, z); the character faces -Y (front = smaller y).

    window: z in [eye_z - below, eye_z + above] (smooth ramp `ramp` outside), y < eye_y + forward (ramp `y_ramp`),
    |x| < half_width.  Inside, |x| maps linearly from [0, half_width] to [gap, half_width], gap = |eye_x| + clearance."""
    ex = 0.5 * (abs(eye_l[0]) + abs(eye_r[0]))
    ez = 0.5 * (eye_l[2] + eye_r[2])
    ey = 0.5 * (eye_l[1] + eye_r[1])
    gap = ex + clearance
    out = []
    moved = 0
    for (x, y, z) in positions:
        bz = _ramp(z, ez - below - ramp, ez - below) * (1.0 - _ramp(z, ez + above, ez + above + ramp))
        by = 1.0 - _ramp(y, ey + forward, ey + forward + y_ramp)
        b = bz * by
        ax = abs(x)
        if b > 0.0 and ax < half_width:
            new = gap + ax * (half_width - gap) / half_width
            sgn = 1.0 if x >= 0.0 else -1.0
            nx = sgn * (ax + b * (new - ax))
            if abs(nx - x) > 1e-9:
                moved += 1
            x = nx
        out.append((x, y, z))
    return out, {"gap_m": gap, "moved_vertices": moved, "window_z": [ez - below, ez + above]}


def clearance_violations(positions, eye_l, eye_r, below=0.025, above=0.055, clearance=0.032, forward=0.0, tol=1e-6):
    """Hair vertices still inside the fully-cleared box (should be 0 after part_hair)."""
    ex = 0.5 * (abs(eye_l[0]) + abs(eye_r[0]))
    ez = 0.5 * (eye_l[2] + eye_r[2])
    ey = 0.5 * (eye_l[1] + eye_r[1])
    gap = ex + clearance
    return sum(1 for (x, y, z) in positions if abs(x) < gap - tol and ez - below <= z <= ez + above and y < ey + forward)


def long01_quads(tris):
    """Undo fa_common.triangulate's ordered fan for the all-quad long01 OBJ (also for loop UVs)."""
    if len(tris) % 2:
        raise ValueError("long01 has an incomplete pair of source triangles")
    quads = []
    for first, second in zip(tris[::2], tris[1::2]):
        if (first[0], first[2]) != tuple(second[:2]):
            raise ValueError("long01 triangle pair differs from its source quad")
        quads.append((*first, second[2]))
    return quads


def finish_long01(obj, body):
    """Round the original card edges, then keep the crown above its fitted scalp.

    Bake before the armature so rig weights subdivide with the mesh in Blender and glTF.
    The crown's disconnected ribbon normals are blended with the nearby scalp normal.
    Lower locks retain their authored sheet directions.
    """
    import bpy
    from mathutils.bvhtree import BVHTree

    # Body and hair share the fitted armature's local metre coordinates.
    scalp = BVHTree.FromPolygons([v.co for v in body.data.vertices],
                                 [p.vertices[:] for p in body.data.polygons])
    bpy.context.view_layer.objects.active = obj
    smooth = obj.modifiers.new("Smooth long01 cards", 'SUBSURF')
    smooth.levels = smooth.render_levels = 2
    bpy.ops.object.modifier_move_to_index(modifier=smooth.name, index=0)
    bpy.ops.object.modifier_apply(modifier=smooth.name)
    mesh = obj.data
    crown = max(v.co.z for v in mesh.vertices)
    near_scalp = {}
    for v in mesh.vertices:
        blend = _ramp(v.co.z, crown - 0.125, crown - 0.065)
        if blend == 0:
            continue
        point, normal, _, _ = scalp.find_nearest(v.co)
        # Lift a card only if rounding its rest edge would put it under skin.
        clearance = (v.co - point).dot(normal)
        if clearance < 0.001:
            v.co += normal * (0.001 - clearance) * blend
        near_scalp[v.index] = (normal, blend)
    mesh.update()
    normals = [v.normal.copy() for v in mesh.vertices]
    for i, (normal, blend) in near_scalp.items():
        normals[i] = normals[i].lerp(normal, 0.88 * blend).normalized()
    mesh.normals_split_custom_set_from_vertices(normals)


def _selftest():
    fails = []

    def check(ok, msg):
        print(("PASS " if ok else "FAIL ") + msg)
        if not ok:
            fails.append(msg)

    el, er = (0.032, -0.07, 1.60), (-0.032, -0.07, 1.60)
    # a bob with a full fringe: a sheet of vertices in front of the face from z 1.50 to 1.75, x -0.11..0.11
    fringe = [(-0.11 + 0.22 * i / 20.0, -0.085, 1.50 + 0.25 * j / 20.0) for i in range(21) for j in range(21)]
    back = [(x, 0.05, z) for x in (-0.1, 0.0, 0.1) for z in (1.5, 1.6, 1.7)]       # behind the eyes: untouched
    side = [(0.14, -0.08, 1.6), (-0.14, -0.08, 1.6)]                               # temples beyond half_width: untouched
    pos = fringe + back + side
    before = clearance_violations(pos, el, er)
    new, info = part_hair(pos, el, er)
    after = clearance_violations(new, el, er)
    check(before > 50 and after == 0, "fringe covering the eyes: %d vertices in the eye box before, %d after" % (before, after))
    check(new[len(fringe):] == pos[len(fringe):], "hair behind the eyes and at the temples is untouched")
    # monotone: ordering along x preserved on each row of the fringe
    ok = True
    for j in range(21):
        row = [new[i * 21 + j][0] for i in range(21)]
        ok = ok and all(a <= b + 1e-12 for a, b in zip(row, row[1:]))
    check(ok, "the remap is monotone in x (no vertex crosses another)")
    ponytail = [(0.0, 0.08, 1.6 + 0.01 * k) for k in range(10)] + [(0.05, 0.02, 1.8)]
    p2, i2 = part_hair(ponytail, el, er)
    check(p2 == ponytail and i2["moved_vertices"] == 0, "a style with no fringe is a no-op")
    print("SELFTEST_FAILS %d" % len(fails))
    return 1 if fails else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(_selftest())
    print(__doc__)
