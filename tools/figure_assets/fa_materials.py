"""Local CC0 image maps for the generic Blender adapter. Materials are built here or in fa_stage."""
import json
from pathlib import Path

import bpy

import fa_common as fc


class PackTextures:
    """Resolve only pinned, usable per-file licence rows; keep the image bytes in the installed pack."""

    def __init__(self, user_data):
        manifest = Path(__file__).resolve().parents[2] / "docs/evidence/m5c/LICENCE_MANIFEST.json"
        with manifest.open(encoding="utf-8") as file:
            self.rows = {r["id"]: r for r in json.load(file)["rows"]}
        self.root = Path(user_data)

    def node(self, material, asset_id):
        row = self.rows.get(asset_id)
        if not row or row["root"] != "user" or len(row["files"]) != 1:
            raise ValueError(f"Missing per-file licence row for {asset_id}")
        entry = row["files"][0]
        if not row["status"].startswith("CC0-") or not entry["status"].startswith("CC0-"):
            raise ValueError(f"Unlicensed texture {asset_id}")
        path = self.root / entry["path"]
        if fc.sha256_file(path) != entry["sha256"]:
            raise ValueError(f"Texture differs from its CC0 licence row: {entry['path']}")
        image = bpy.data.images.load(str(path), check_existing=False)
        image.alpha_mode = "STRAIGHT"
        tex = material.node_tree.nodes.new("ShaderNodeTexImage")
        tex.image = image
        tex.label = asset_id
        material["fa_texture_id"] = asset_id
        material["fa_texture_sha256"] = entry["sha256"]
        return tex


def tinted_albedo(material, tex, rgb, neutral, iris_only=False):
    """Keep original pigment at the reference palette and allow recipe/tape colour changes."""
    nt = material.node_tree
    color = tex.outputs["Color"]
    if any(abs(c - n) > 1e-6 for c, n in zip(rgb, neutral)):
        tint = nt.nodes.new("ShaderNodeMixRGB")
        tint.blend_type = "MULTIPLY"
        tint.inputs[0].default_value = 1.0
        tint.inputs[2].default_value = tuple(c / n for c, n in zip(rgb, neutral)) + (1.0,)
        nt.links.new(color, tint.inputs[1])
        if iris_only:
            # The blue source iris has high saturation, the sclera and pupil have little.
            hsv = nt.nodes.new("ShaderNodeSeparateColor")
            hsv.mode = "HSV"
            nt.links.new(color, hsv.inputs[0])
            nt.links.new(hsv.outputs[1], tint.inputs[0])
        color = tint.outputs[0]
    nt.links.new(color, nt.nodes["Principled BSDF"].inputs["Base Color"])


def alpha_cards(material, tex):
    material.surface_render_method = "DITHERED"
    material.use_backface_culling = False
    material.node_tree.links.new(tex.outputs["Alpha"], material.node_tree.nodes["Principled BSDF"].inputs["Alpha"])


def skin_detail(material):
    """A shallow pore relief in local metre coordinates, independent of UV seams."""
    nt = material.node_tree
    coords = nt.nodes.new("ShaderNodeTexCoord")
    pores = nt.nodes.new("ShaderNodeTexNoise")
    pores.inputs["Scale"].default_value = 1800.0
    pores.inputs["Detail"].default_value = 2.0
    nt.links.new(coords.outputs["Object"], pores.inputs["Vector"])
    bump = nt.nodes.new("ShaderNodeBump")
    bump.inputs["Strength"].default_value = 0.14
    bump.inputs["Distance"].default_value = 0.00008
    nt.links.new(pores.outputs["Fac"], bump.inputs["Height"])
    nt.links.new(bump.outputs["Normal"], nt.nodes["Principled BSDF"].inputs["Normal"])
