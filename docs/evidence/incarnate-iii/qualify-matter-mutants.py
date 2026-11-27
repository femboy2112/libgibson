"""Compile exact scalar modules and their tests with three single-site source mutants."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

CASES = {
    "gravity_ignored": ("particles.rs", ".add(gravity.scale(dt))", ".add(Vec3::ZERO.scale(dt))",
                        "analytic_gravity_history_and_timestep_mutation_controls"),
    "fabric_aliased": ("mod.rs", "match self {", "match Self::Medium {",
                       "identical_cut_different_fabric_and_alias_mutant"),
    "contact_disabled": ("contact.rs", "let Some(ids) = self.bins.get(&index(p)) else {",
                         "if thickness >= 0. { return p; }\n        let Some(ids) = self.bins.get(&index(p)) else {",
                         "real_surface_contact_and_pose_relative_support"),
}

def once(text,old,new):
    if text.count(old)!=1: raise ValueError(f"mutation site is not unique: {old}")
    return text.replace(old,new)

def main():
    if not os.environ.get("HEAVY_CLASS"): raise SystemExit("Run under a GOD heavy lease.")
    ap=argparse.ArgumentParser();ap.add_argument("out",type=Path);args=ap.parse_args()
    args.out.mkdir(parents=True,exist_ok=True)
    source=Path(__file__).resolve().parents[3]/"src"/"figure"
    root=source/"living_matter"
    modules={p.name:p.read_text() for p in root.glob("*.rs") if p.name not in ("actor.rs","checkpoint.rs")}
    # The integrating actor boundary has its own real mesh tests; this executable runs its
    # dependency-free physics and the complete original physics module test definitions.
    modules["mod.rs"]=once(modules["mod.rs"],"mod actor;","")
    line=next(l for l in modules["mod.rs"].splitlines() if l.startswith("pub use actor::"))
    modules["mod.rs"]=once(modules["mod.rs"],line,"")
    summary={}
    with tempfile.TemporaryDirectory() as tmp:
        d=Path(tmp)
        dest=d/"figure"/"living_matter"
        dest.mkdir(parents=True)
        shutil.copy2(source/"math.rs", d/"figure"/"math.rs")
        (d/"root.rs").write_text("mod figure { pub mod math; pub use math::Quat; pub mod living_matter; }")
        for name in ["original",*CASES]:
            files=modules.copy()
            if name!="original":
                file,old,new,expected=CASES[name];files[file]=once(files[file],old,new)
            for file,data in files.items():(dest/file).write_text(data)
            compile=subprocess.run(["rustc","+1.85","--edition","2021","-A","dead_code","--test","-O",str(d/"root.rs"),"-o",str(d/"run")],
                capture_output=True,text=True)
            (args.out/(name+"-build.log")).write_text(compile.stdout+compile.stderr)
            if compile.returncode: raise RuntimeError(f"{name} failed to compile")
            trial=subprocess.run([str(d/"run"),"--test-threads=1","--nocapture"],capture_output=True,text=True)
            evidence=trial.stdout+trial.stderr
            (args.out/(name+".log")).write_text(evidence)
            killed=[line.split()[1] for line in evidence.splitlines() if line.startswith("test ") and line.endswith(" FAILED")]
            if name=="original":
                if trial.returncode or "7 passed" not in evidence: raise RuntimeError("unaltered source failed")
            elif not trial.returncode or not any(t.endswith("::"+expected) for t in killed):
                # Rust's nocapture output may interleave println! between the test name and verdict.
                if not trial.returncode or ("    figure::living_matter::lab::"+expected+"\n") not in evidence:
                    raise RuntimeError(f"expected detector did not kill {name}")
                killed=[v.strip() for v in evidence.splitlines() if v.startswith("    figure::")]
            summary[name]={"exit":trial.returncode,"killed":killed,"sha256":hashlib.sha256(evidence.encode()).hexdigest()}
    print(json.dumps(summary,indent=2))
if __name__=="__main__":main()
