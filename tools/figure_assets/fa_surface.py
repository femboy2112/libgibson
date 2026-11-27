"""Presentation-only smoothing and occluded garment layers for the Blender adapters."""
import math

import bpy
from mathutils.bvhtree import BVHTree

import fa_skirt


def smooth_surface(obj, render_only=False):
    for face in obj.data.polygons:
        face.use_smooth = True
    sub = obj.modifiers.new("FabricSubdivision" if obj.get("fa_role") != "body" else "SkinSubdivision", "SUBSURF")
    sub.levels = sub.render_levels = 1
    sub.show_viewport = not render_only


def visible_positions(obj):
    """Render-surface world positions. Restore the original indexed cage used by the pose oracle."""
    from fa_stage import eval_positions

    toggled = [m for m in obj.modifiers if m.show_render and not m.show_viewport]
    try:
        for m in toggled:
            m.show_viewport = True
        return eval_positions(obj)
    finally:
        for m in toggled:
            m.show_viewport = False
        if toggled:
            bpy.context.view_layer.update()


def supplied_fit(gid, expected_count, fitted_positions):
    """Read the compiler's indexed rest positions without substituting a different topology."""
    if fitted_positions is None or gid not in fitted_positions:
        return None
    points = fitted_positions[gid]
    if len(points) != expected_count:
        raise ValueError(f"{gid}: compiled fit has {len(points)} vertices; expected {expected_count}")
    if any(len(p) != 3 or any(not math.isfinite(c) for c in p) for p in points):
        raise ValueError(f"{gid}: compiled fit contains invalid vertex coordinates")
    return points


def fit_skirt(body_positions, joints, body_arm_mask, inner_layers, fitted_positions=None):
    """Fit over all drawn inner layers and remove only their fully occluded middle surfaces."""
    extra = [(pos, mask) for _, pos, mask in inner_layers]
    skirt = fa_skirt.make_skirt(body_positions, joints, exclude=body_arm_mask, extra=extra)
    params = skirt["params"]
    fit = supplied_fit("skirt_a_line", len(skirt["positions"]), fitted_positions)
    if fit is not None:
        skirt["positions"] = fit
    # The compiler may move the waistband or hem. Keep overlap strips inside their boundaries
    # at every angle when masking the inner clothes.
    n_seg = params["n_seg"]
    z_top = min(p[2] for p in skirt["positions"][:n_seg])
    z_hem = max(p[2] for p in skirt["positions"][-n_seg:])
    params["masked_inner_vertices"] = {}
    for obj, pos, arm_mask in inner_layers:
        interior = set(fa_skirt.under_skirt_hide(pos, arm_mask, z_top, z_hem))
        # A Mask modifier discards incident faces too. Keep the boundary's one-ring so even
        # a long trouser or jacket triangle spanning the waistband/hem stays visible.
        boundary = set()
        for face in obj.data.polygons:
            if any(v not in interior for v in face.vertices):
                boundary.update(face.vertices)
        covered = sorted(interior - boundary)
        params["masked_inner_vertices"][obj.name] = len(covered)
        if covered:
            vg = obj.vertex_groups.new(name="fa_under_skirt")
            vg.add(covered, 1.0, "REPLACE")
            mask = obj.modifiers.new("HideUnderSkirt", "MASK")
            mask.vertex_group = vg.name
            mask.invert_vertex_group = True
    return skirt


def finish_skirt(obj):
    """Round the sewn panels before giving the smoothed fabric an inward-facing lining."""
    smooth_surface(obj)
    solid = obj.modifiers.new("FabricThickness", "SOLIDIFY")
    solid.thickness = 0.003
    solid.offset = -1.0
    solid.use_even_offset = True
    bevel = obj.modifiers.new("HemAndWaistRims", "BEVEL")
    bevel.width = 0.0007
    bevel.segments = 2
    bevel.limit_method = "ANGLE"


def fabric_material(mat):
    """Fine matte weave on the caller's authoritative colour and roughness."""
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    bsdf = nodes.get("Principled BSDF")
    bsdf.inputs["Sheen Weight"].default_value = 0.25
    coord = nodes.new("ShaderNodeTexCoord")
    noise = nodes.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = 850.0
    noise.inputs["Detail"].default_value = 1.0
    links.new(coord.outputs["Object"], noise.inputs["Vector"])
    bump = nodes.new("ShaderNodeBump")
    bump.inputs["Strength"].default_value = 0.12
    bump.inputs["Distance"].default_value = 0.00025
    links.new(noise.outputs["Fac"], bump.inputs["Height"])
    links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    return mat


def layer_intersections(skirt, garments):
    """Count crossing polygons on the final posed garment meshes, including lining and rims."""
    bpy.context.view_layer.update()
    dg = bpy.context.evaluated_depsgraph_get()

    def tree(obj):
        ev = obj.evaluated_get(dg)
        mesh = ev.to_mesh()
        try:
            return BVHTree.FromPolygons([ev.matrix_world @ v.co for v in mesh.vertices],
                                        [tuple(p.vertices) for p in mesh.polygons])
        finally:
            ev.to_mesh_clear()

    surface = tree(skirt)
    return {obj.name: len(surface.overlap(tree(obj))) for obj in garments}
