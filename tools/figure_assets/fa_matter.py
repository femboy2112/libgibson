"""Read an authenticated Rust Living Matter bake and seek directly to a realized cage frame."""

from array import array
import json
import math
from pathlib import Path
import sys

import fa_common as fc


class MatterPlayback:
    def __init__(self, path, tape, figure, export):
        path = Path(path)
        with path.open(encoding="utf-8") as source:
            h = json.load(source)
        if h.get("format") != "gibson.living-matter.bake.v1" or h.get("basis") != "world-z-up-m":
            raise ValueError("unsupported physical-state track")
        for field in ("body_rev_id", "fit_id", "present_id", "asset_manifest_sha256"):
            if h[field] != tape.header[field]:
                raise ValueError("physical track/tape identity differs at " + field)
        if h["asset_manifest_sha256"] != fc.sha256_file(Path(export) / "manifest.json"):
            raise ValueError("physical track belongs to another asset export")
        if (h["tape_bin_sha256"] != tape.header["bin_sha256"] or h["fps"] != tape.header["fps"]
                or h["frames"] != tape.frame_count):
            raise ValueError("physical track belongs to another performance")
        digest = h["bin_sha256"]
        if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest) or h["binary"] != f"matter-{digest}.bin":
            raise ValueError("invalid physical binary name")
        file = path.parent / h["binary"]
        objects = {o.name: o for group in ("garment", "hair") for o in figure.objs[group]}
        objects["body"] = figure.body
        if figure.skirt:
            objects["skirt_a_line"] = figure.skirt["obj"]
        self.channels = []
        offset, seen = 0, set()
        for record in h["meshes"]:
            name = record["id"]
            obj = objects.get(name)
            if obj is None or name in seen or len(obj.data.vertices) != record["vertices"] or record["offset_bytes"] != offset:
                raise ValueError("physical mesh binding differs at " + name)
            seen.add(name)
            count = len(obj.data.vertices)
            original = array("f", [0.0]) * (count * 3)
            obj.data.vertices.foreach_get("co", original)
            # A relative key adds the independent host face keys to the realized body.
            # The baked cage is in armature/world metres. Apply skinning exactly once.
            if obj.data.shape_keys:
                target = obj.shape_key_add(name="Living Matter world cage", from_mix=False)
                data = target.data
            else:
                target, data = None, obj.data.vertices
            mods = [(m, m.show_viewport, m.show_render) for m in obj.modifiers if m.type == "ARMATURE"]
            self.channels.append((obj, data, target, original, mods))
            offset += count * 12
        if seen != objects.keys() or not 0 < offset == h["stride_bytes"]:
            raise ValueError("physical track omits a drawable surface")
        if file.stat().st_size != offset * h["frames"] or fc.sha256_file(file) != digest:
            raise ValueError("physical binary length or digest mismatch")
        self.path, self.stride, self.frames = file, offset, h["frames"]
        self.header = h
        self.active = False

    def restore(self):
        if self.active:
            for obj, data, target, original, mods in self.channels:
                if target is None:
                    data.foreach_set("co", original)
                else:
                    target.value = 0.0
                for mod, viewport, render in mods:
                    mod.show_viewport, mod.show_render = viewport, render
                obj.data.update()
            self.active = False

    def apply(self, frame):
        if not 0 <= frame < self.frames:
            raise ValueError("physical frame outside baked range")
        with self.path.open("rb") as source:
            source.seek(frame * self.stride)
            for obj, data, target, original, mods in self.channels:
                coords = array("f")
                coords.fromfile(source, len(original))
                if sys.byteorder != "little":
                    coords.byteswap()
                if not all(math.isfinite(v) for v in coords):
                    raise ValueError("nonfinite coordinate in physical tape")
                for mod, _, _ in mods:
                    mod.show_viewport = mod.show_render = False
                data.foreach_set("co", coords)
                if target is not None:
                    target.value = 1.0
                obj.data.update()
        self.active = True
