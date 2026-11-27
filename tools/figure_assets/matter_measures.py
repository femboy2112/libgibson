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

def volume(positions, triangles):
    # Translation conditioning deliberately uses all body vertices, including under clothes.
    p = positions.astype(np.float64) - positions.mean(axis=0,dtype=np.float64)
    a,b,c = p[triangles].transpose(1,0,2)
    return abs(np.einsum("ij,ij->", a, np.cross(b,c)))/6.

def main(root, export=None):
    base, m = track(root,"medium")
    report = {"units":"metres", "baseline":"medium normal gravity", "frames":base["frames"], "trials":{}}
    triangles = None
    if export:
        rec = json.loads((export/"figure_asset.json").read_text())["blobs"]["base.tris"]
        if rec["dtype"] != "u32":
            raise ValueError("unknown body triangle format")
        triangles = np.fromfile(export/rec["file"], dtype="<u4").reshape(-1,3)
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
            if name=="skirt_a_line":
                # The original 18x192 periodic cut orders the waistband and hem along row 0 and -1.
                if positions.shape[1]!=18*192: raise ValueError("unknown skirt topology")
                stats[name]["waist_max_difference_m"]=float(distances[:192].max())
                stats[name]["hem_mean_difference_xyz_m"]=delta[-192:].mean(axis=0).tolist()
                sampled=np.arange(0,len(positions),8)
                stats[name]["hem_trajectory_xyz_m"]=[(positions[i,-192:]-m[name][i,-192:]).mean(axis=0,dtype=np.float64).tolist() for i in sampled]
                stats[name]["sample_frames"]=sampled.tolist()
            if name=="body" and triangles is not None:
                samples=[]
                for frame in range(0,len(positions),12):
                    baseline=volume(m[name][frame],triangles)
                    measured=volume(positions[frame],triangles)
                    samples.append(abs(measured-baseline)/max(baseline,1e-12))
                stats[name]["max_relative_whole_body_volume_change"]=max(samples)
                stats[name]["volume_sample_stride"]=12
        report["trials"][profile] = {"sim_config_id":h["sim_config_id"],"mesh":stats}
    print(json.dumps(report,indent=2,allow_nan=False))

if __name__ == "__main__":
    main(Path(sys.argv[1]),Path(sys.argv[2]) if len(sys.argv)>2 else None)
