"""Independent Blender cloth comparison. Run with factory startup under a GOD heavy lease."""
import json
import bpy

def simulate(gravity, rotated=False):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.fps = 30
    scene.gravity = gravity
    n = 9
    verts = [(0.04 * i, 0.025 * (j - 4), 1.0) for j in range(n) for i in range(n)]
    if rotated:
        verts = [(x,z-1.0,1.0-y) for x,y,z in verts]
    faces = [(j*n+i, j*n+i+1, (j+1)*n+i+1, (j+1)*n+i) for j in range(n-1) for i in range(n-1)]
    mesh = bpy.data.meshes.new("identical patch")
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    obj = bpy.data.objects.new("pinned patch", mesh)
    scene.collection.objects.link(obj)
    pins = [j*n for j in range(n)]
    g = obj.vertex_groups.new(name="attachment")
    g.add(pins, 1.0, "REPLACE")
    cloth = obj.modifiers.new("independent solver", "CLOTH")
    cloth.settings.vertex_group_mass = g.name
    cloth.settings.quality = 6
    cloth.settings.mass = 0.002
    cloth.settings.bending_stiffness = 0.012
    cloth.settings.air_damping = 3
    cloth.point_cache.frame_end = 91
    for frame in range(1, 92):
        scene.frame_set(frame)
        deps = bpy.context.evaluated_depsgraph_get()
        evaluated = obj.evaluated_get(deps)
        pos = [v.co.copy() for v in evaluated.data.vertices]
    pin_error = max((pos[k] - mesh.vertices[k].co).length for k in pins)
    tip = [sum(pos[j*n+n-1][axis] for j in range(n))/n for axis in range(3)]
    return {"tip": tip, "pin_error_m": pin_error}

normal, zero, side = [simulate(g) for g in [(0,0,-9.80665),(0,0,0),(0,-9.80665,0)]]
print("BLENDER_CLOTH", json.dumps({"normal":normal,"zero":zero,"side":side,"blender":bpy.app.version_string}))
assert max(r["pin_error_m"] for r in [normal,zero,side]) < 0.0001
assert zero["tip"][2]-normal["tip"][2] > 0.015
# The in-plane load is resisted by the panel in shear. Rotate the entire experiment
# to preserve the load/patch relationship and measure rotational covariance independently.
rot = simulate((0,-9.80665,0), rotated=True)
rot0 = simulate((0,0,0), rotated=True)
dz = zero["tip"][2] - normal["tip"][2]
dy = rot0["tip"][1] - rot["tip"][1]
print("BLENDER_ROTATED", json.dumps({"loaded":rot,"fresh":rot0,"out_of_plane_native_m":dz,"out_of_plane_rotated_m":dy}))
assert dy > 0.015 and abs(dy-dz) < 0.015 and rot["pin_error_m"] < 0.0001
