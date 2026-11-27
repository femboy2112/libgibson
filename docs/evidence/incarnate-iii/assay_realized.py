"""Evaluate actual full render surfaces after seeking the authenticated bake in a shuffled order."""
import argparse
from itertools import combinations
import json
from pathlib import Path
import struct
import sys
from types import SimpleNamespace

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_unmasked import world_tree
import fa_matter
import fa_tape
import render_tape


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("export", type=Path)
    ap.add_argument("preset", type=Path)
    ap.add_argument("run", type=Path)
    ap.add_argument("out", type=Path)
    a = ap.parse_args(sys.argv[sys.argv.index("--") + 1:])
    path = a.run / "matter.json"
    tape = fa_tape.read_tape(str(a.run / "generic.tape.json"))
    h = json.loads(path.read_text())
    params = SimpleNamespace(export=a.export, fitted=str(a.run / "generic.fit.json"), matter=path, joint_frame="bone")
    fig = render_tape.build_figure(params,tape,json.loads(a.preset.read_text()),False,[])
    fig.matter = fa_matter.MatterPlayback(path,tape,fig,a.export)
    objects = {"body":fig.body}
    objects.update({o.name:o for role in ("garment","hair") for o in fig.objs[role]})
    if fig.skirt:
        objects["skirt_a_line"] = fig.skirt["obj"]
    # Begin from the last frame to falsify implicit simulation-on-seek; visit the beginning twice.
    order = [h["frames"]-1,0,h["frames"]//6,h["frames"]//3,h["frames"]*2//3,0]
    records = []
    with (a.run / h["binary"]).open("rb") as binary:
        for f in order:
            render_tape.set_pose(fig,tape,f)
            names = list(objects)
            trees = [world_tree(objects[name],False) for name in names]
            crossings = {f"{names[i]} / {names[j]}":len(trees[i].overlap(trees[j])) for i,j in combinations(range(len(names)),2)}
            max_error = 0.
            for m in h["meshes"]:
                obj = objects[m["id"]]
                for i in (0,m["vertices"]//4,m["vertices"]//2,m["vertices"]-1):
                    binary.seek(f*h["stride_bytes"]+m["offset_bytes"]+12*i)
                    expected = struct.unpack("<fff",binary.read(12))
                    actual = obj.data.vertices[i].co
                    max_error=max(max_error,max(abs(v-w) for v,w in zip(expected,actual)))
            records.append({"frame":f,"full_surface_crossing_polygon_pairs":crossings,"world_cage_max_sampled_error_m":max_error})
    result={"source_tape":h["tape_bin_sha256"],"simulation":h["sim_config_id"],"bake":h["bin_sha256"],"backend":h["backend"],"records":records}
    a.out.write_text(json.dumps(result,indent=2)+"\n")
    print("CAGE_SEEK",len(records),max(r["world_cage_max_sampled_error_m"] for r in records),records[1]==records[-1])
    if not (all(r["world_cage_max_sampled_error_m"]<1e-6 for r in records) and records[1]==records[-1]):
        raise RuntimeError("physical bake differs after random access")
    # Intentional intersections of sewn trim/panels must be reviewed by the asset owner.
    # Count and preserve every crossing, with no blanket fit pass.

if __name__=="__main__":
    main()
