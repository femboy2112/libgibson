"""Measured displacements for identically posed physical captures; no rendered masks enter the assay."""
import json
import sys
from pathlib import Path
import numpy as np

def track(root, profile):
    directory = root / profile
    h = json.loads((directory / "matter.json").read_text())
    raw = np.memmap(directory / h["binary"], dtype="<f4", mode="r", shape=(h["frames"], h["stride_bytes"] // 4))
    meshes = {}
    for m in h["meshes"]:
        j = m["offset_bytes"] // 4
        meshes[m["id"]] = raw[:,j:j+m["vertices"]*3].reshape(h["frames"], -1, 3)
    return h, meshes

def main(root):
    base, m = track(root,"medium")
    report = {"units":"metres", "baseline":"medium normal gravity", "frames":base["frames"], "trials":{}}
    for profile in ("zero","side","structured","light","supported"):
        h, t = track(root,profile)
        for key in ("body_rev_id","present_id","fit_id","tape_bin_sha256"):
            if h[key]!=base[key]:
                raise ValueError(f"mismatched {key} in {profile}")
        stats = {}
        for name, positions in t.items():
            delta = positions[-1].astype(np.float64)-m[name][-1]
            distances = np.linalg.norm(delta,axis=1)
            stats[name] = {"mean_delta_xyz":delta.mean(axis=0).tolist(),"rms":float(np.sqrt(np.mean(distances**2))),"max":float(distances.max())}
        report["trials"][profile] = {"sim_config_id":h["sim_config_id"],"mesh":stats}
    print(json.dumps(report,indent=2,allow_nan=False))

if __name__ == "__main__":
    main(Path(sys.argv[1]))
