//! Cross-check harness for declared Living Matter regions: steps `FigureDynamics::with_regions`
//! through a baked tape and prints, per frame and region, the region frame (origin, rotation),
//! the region displacement and the largest and summed vertex displacement of the body mesh, as
//! CSV on stdout. A python reference (`lm_ref.py`, kept outside this repository) consumes the
//! same lines. All inputs are private and come from the environment, never from the repository:
//!
//! ```text
//! LM_XCHECK_ASSETS=<converted asset dir (gibson.figure.asset.v1)>
//! LM_XCHECK_TAPE=<dir holding generic.tape.json and generic.tape.bin>
//! LM_XCHECK_REGIONS=<dir holding regions.txt and the region blobs it names>
//! cargo run --release --features figure --example figure_lm_regions_xcheck
//! ```
//!
//! `regions.txt`, one record per line, `#` starts a comment:
//! `control <name> <value>` and
//! `region <name> <joint> <scale> <nx> <ny> <nz> <offset> <width> <blob.u32.bin>`
//! (a zero normal means no side partition). The region is measured on the compiled body's rest
//! positions and the joint's inverse bind, as the family rebind measures a compliant tag.
//! Optional `LM_XCHECK_FRAMES=<first>:<last>` (inclusive) limits the frames and `LM_XCHECK_HOLD=<n>` repeats the last
//! frame `n` more times (a held pose; the printed frame index keeps counting).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gibson::figure::living_matter::{
    DynamicsConfig, Falloff, FigureDynamics, Partition, RegionSpec,
};
use gibson::figure::math::Vec3;
use gibson::figure::studio::{compile, AssetSet, RecipeV1, TapeFrame, TapeReader};

fn env_path(name: &str) -> Result<PathBuf, String> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("set {name}"))
}

fn read_u32s(path: &Path) -> Result<Vec<u32>, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() % 4 != 0 {
        return Err(format!("{}: length is not a multiple of 4", path.display()));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect())
}

fn number(token: &str) -> Result<f32, String> {
    token
        .parse::<f32>()
        .map_err(|e| format!("bad number {token:?}: {e}"))
}

struct RegionLine {
    name: String,
    joint: String,
    scale: f32,
    partition: Option<Partition>,
    blob: PathBuf,
}

/// Body controls `(id, value)` and the declared region lines.
type ParsedRegions = (Vec<(String, f32)>, Vec<RegionLine>);

fn parse_regions(dir: &Path) -> Result<ParsedRegions, String> {
    let text =
        fs::read_to_string(dir.join("regions.txt")).map_err(|e| format!("regions.txt: {e}"))?;
    let mut controls = Vec::new();
    let mut regions = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.as_slice() {
            ["control", name, value] => controls.push(((*name).to_string(), number(value)?)),
            ["region", name, joint, scale, nx, ny, nz, offset, width, blob] => {
                let normal = Vec3::new(number(nx)?, number(ny)?, number(nz)?);
                let partition = if normal.length() == 0.0 {
                    None
                } else {
                    Some(Partition {
                        normal,
                        offset: number(offset)?,
                        width: number(width)?,
                    })
                };
                regions.push(RegionLine {
                    name: (*name).to_string(),
                    joint: (*joint).to_string(),
                    scale: number(scale)?,
                    partition,
                    blob: dir.join(blob),
                });
            }
            _ => return Err(format!("unrecognised line {line:?}")),
        }
    }
    Ok((controls, regions))
}

fn run() -> Result<(), String> {
    let assets_dir = env_path("LM_XCHECK_ASSETS")?;
    let tape_dir = env_path("LM_XCHECK_TAPE")?;
    let regions_dir = env_path("LM_XCHECK_REGIONS")?;
    let (controls, lines) = parse_regions(&regions_dir)?;

    let mut recipe = RecipeV1::new();
    for (name, value) in &controls {
        recipe.set_control(name, *value);
    }
    let assets = AssetSet::load_dir(&assets_dir).map_err(|e| format!("assets: {e:?}"))?;
    let c = compile(&recipe, &assets).map_err(|e| format!("compile: {e:?}"))?;

    let positions = c.rest_render_meshes()[0].positions.clone();
    let mut specs = Vec::new();
    for l in &lines {
        let joint = c
            .skeleton()
            .joint_index(&l.joint)
            .ok_or_else(|| format!("no joint {}", l.joint))?;
        let vertices = read_u32s(&l.blob)?;
        let falloff = Falloff {
            scale: l.scale,
            partition: l.partition,
        };
        let spec = RegionSpec::measure(
            &l.name,
            joint,
            &vertices,
            &positions,
            &c.inverse_binds()[joint],
            falloff,
        )
        .map_err(|e| format!("{}: {e}", l.name))?
        .bound_to(c.body_rev_id());
        println!(
            "#spec,{},{},{},{:e},{:e},{:e},{:e},{:e},{:e},{:e},{}",
            spec.name,
            l.joint,
            vertices.len(),
            spec.centroid.x,
            spec.centroid.y,
            spec.centroid.z,
            spec.half_extent.x,
            spec.half_extent.y,
            spec.half_extent.z,
            spec.radius,
            spec.vertex_count
        );
        specs.push(spec);
    }
    let names: Vec<String> = specs.iter().map(|s| s.name.clone()).collect();

    let header =
        fs::read(tape_dir.join("generic.tape.json")).map_err(|e| format!("tape json: {e}"))?;
    let bin = fs::read(tape_dir.join("generic.tape.bin")).map_err(|e| format!("tape bin: {e}"))?;
    let tape = TapeReader::new(&header, &bin).map_err(|e| format!("tape: {e:?}"))?;
    let h = tape.header();
    let fps = h.fps.round() as u32;
    let count = h.frame_count;
    // joint order of the tape -> joint order of the compiled skeleton (the body revision of the
    // tape is not required to match: only region frames are read, not skin)
    let slot: BTreeMap<usize, usize> = h
        .joints
        .iter()
        .enumerate()
        .map(|(k, name)| {
            c.skeleton()
                .joint_index(name)
                .map(|j| (k, j))
                .ok_or_else(|| format!("tape joint {name} is not in the skeleton"))
        })
        .collect::<Result<_, _>>()?;
    let (first, last) = match std::env::var("LM_XCHECK_FRAMES") {
        Ok(v) => {
            let (a, b) = v
                .split_once(':')
                .ok_or("LM_XCHECK_FRAMES wants first:last")?;
            (
                a.parse::<usize>().map_err(|e| e.to_string())?,
                b.parse::<usize>().map_err(|e| e.to_string())?,
            )
        }
        Err(_) => (0, count.saturating_sub(1)),
    };
    if last >= count {
        return Err(format!("frame {last} outside 0..{count}"));
    }

    let hold: usize = match std::env::var("LM_XCHECK_HOLD") {
        Ok(v) => v.parse::<usize>().map_err(|e| e.to_string())?,
        Err(_) => 0,
    };
    let mut dynamics = FigureDynamics::with_regions(&c, DynamicsConfig::default(), specs)
        .map_err(|e| format!("with_regions: {e}"))?;
    println!("#config_id,{}", hex(&dynamics.config_id()));
    println!("frame,region,ox,oy,oz,qx,qy,qz,qw,ux,uy,uz,max_disp,sum_disp");
    for f in first..=last + hold {
        let raw = tape
            .frame(f.min(last))
            .map_err(|e| format!("frame {f}: {e:?}"))?;
        let mut locals = c.skeleton().rest_locals();
        for (k, t) in raw.locals.iter().enumerate() {
            locals[slot[&k]] = *t;
        }
        let frame = TapeFrame {
            root_t: raw.root_t,
            locals,
        };
        let rest = c
            .render_meshes(&frame)
            .map_err(|e| format!("pose {f}: {e:?}"))?;
        let out = dynamics
            .step(&c, &frame, fps)
            .map_err(|e| format!("step {f}: {e}"))?;
        let (mut max_disp, mut sum_disp) = (0.0f32, 0.0f32);
        for (a, b) in out[0].positions.iter().zip(&rest[0].positions) {
            let d = a.sub(*b).length();
            max_disp = max_disp.max(d);
            sum_disp += d;
        }
        let u = dynamics.region_displacements();
        let fr = dynamics.region_frames();
        for (i, name) in names.iter().enumerate() {
            let (o, q) = fr[i];
            println!(
                "{f},{name},{:e},{:e},{:e},{:e},{:e},{:e},{:e},{:e},{:e},{:e},{:e},{:e}",
                o.x, o.y, o.z, q.x, q.y, q.z, q.w, u[i].x, u[i].y, u[i].z, max_disp, sum_disp
            );
        }
    }
    Ok(())
}

fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("figure_lm_regions_xcheck: {e}");
            ExitCode::FAILURE
        }
    }
}
