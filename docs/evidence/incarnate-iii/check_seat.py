"""Measure final physical cage distance to the recorded chair contact point."""
import argparse
import hashlib
import json
import struct
from pathlib import Path


def measure(root):
    matter = json.loads((root / "matter.json").read_text())
    scene = json.loads((root / "scene.json").read_text())
    tape = json.loads((root / "generic.tape.json").read_text())
    if not scene.get("chair"):
        raise ValueError("a seated scene is required")
    for key in ("body_rev_id", "fit_id", "present_id"):
        if len({matter[key], scene[key], tape[key]}) != 1:
            raise ValueError("identity mismatch at " + key)
    if matter["tape_bin_sha256"] != tape["bin_sha256"] or scene["tape_bin_sha256"] != tape["bin_sha256"]:
        raise ValueError("tape digest mismatch")
    chair = scene["chair"]
    seat = chair["seat"]
    center, size = seat["center"], seat["size"]
    top = center[2] + size[2] / 2
    mesh = next(m for m in matter["meshes"] if m["id"] == "body")
    index = chair["contact_body_vertex"]
    if not 0 <= index < mesh["vertices"]:
        raise ValueError("recorded contact vertex outside body mesh")
    binary = root / matter["binary"]
    if binary.stat().st_size != matter["frames"] * matter["stride_bytes"]:
        raise ValueError("wrong physical byte count")
    gaps = []
    in_footprint = []
    with binary.open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != matter["bin_sha256"]:
            raise ValueError("wrong physical byte digest")
        for f in range(max(0, matter["frames"] - matter["fps"] // 2), matter["frames"]):
            stream.seek(f * matter["stride_bytes"] + mesh["offset_bytes"] + 12 * index)
            p = struct.unpack("<fff", stream.read(12))
            gaps.append(p[2] - top)
            in_footprint.append(all(abs(p[a] - center[a]) <= size[a] / 2 for a in (0, 1)))
    if not all(in_footprint):
        raise ValueError("physical contact witness left the chair footprint")
    print(json.dumps({"frames_near_settle":len(gaps), "final_body_seat_gap_m":gaps[-1],
                      "min_gap_m":min(gaps), "max_gap_m":max(gaps),
                      "walk_triangle_lower_bound_m":chair["walk_to_chair_gap_m"]}, sort_keys=True))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("performance", type=Path)
    measure(parser.parse_args().performance)
