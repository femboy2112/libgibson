"""Compare corresponding RGB PNGs and adjacent-frame alternatives for an animation slice."""
import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


def errors(a, b):
    diff = ImageChops.difference(a, b)
    stat = ImageStat.Stat(diff)
    values = sorted(x[1] for x in diff.getextrema())
    return {"mean_abs_rgb":sum(stat.mean) / 3, "max_abs_rgb":values[-1],
            "rms_rgb":(sum(v*v for v in stat.rms) / 3)**0.5}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("reference", type=Path)
    p.add_argument("candidate", type=Path)
    p.add_argument("first", type=int)
    p.add_argument("count", type=int)
    a = p.parse_args()
    frames = range(a.first, a.first+a.count)
    refs = {}
    for i in frames:
        with Image.open(a.reference / f"frame_{i:05d}.png") as im:
            refs[i] = im.convert("RGB")
    order_ok = True
    for i in frames:
        with Image.open(a.candidate / f"frame_{i:05d}.png") as im:
            rgb = im.convert("RGB")
        if rgb.size != refs[i].size:
            raise ValueError("resolution mismatch")
        by_frame = {j:errors(rgb, refs[j]) for j in frames if abs(j-i) <= 1}
        nearest = min(by_frame, key=lambda j: by_frame[j]["rms_rgb"])
        order_ok &= nearest == i
        print(json.dumps({"frame":i, "nearest_reference":nearest, "distance_by_reference":by_frame,
                          "reference_rgb_sha256":hashlib.sha256(refs[i].tobytes()).hexdigest(),
                          "candidate_rgb_sha256":hashlib.sha256(rgb.tobytes()).hexdigest()}, sort_keys=True))
    if not order_ok:
        raise ValueError("one or more candidate frames matched an adjacent reference more closely")


if __name__ == "__main__":
    main()
