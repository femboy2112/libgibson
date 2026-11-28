//! Prints what the studio compile of a converted asset directory produces, so a python reference can
//! compare a body family against it: the f32 bits of every rest body position and the bind-world
//! translation of every joint. Inputs come from the environment, never from the repository:
//!
//! ```text
//! LM_FAMILY_ASSETS=<converted asset dir (gibson.figure.asset.v1)>
//! LM_FAMILY_CONTROLS="morphos_minerva=1.0 ..."   (name=value, space separated)
//! cargo run --release --features figure --example figure_family_dump
//! ```
//!
//! Output (stdout): `#positions,<count>,<sha256 of the little-endian f32 bytes>`, then one
//! `v,<index>,<x bits>,<y bits>,<z bits>` line per body vertex (u32 bit patterns in hex), then
//! one `j,<name>,<x>,<y>,<z>` line per joint.

use std::path::PathBuf;
use std::process::ExitCode;

use gibson::figure::math::Vec3;
use gibson::figure::sha256;
use gibson::figure::studio::{compile, AssetSet, RecipeV1};

fn run() -> Result<(), String> {
    let dir = std::env::var_os("LM_FAMILY_ASSETS")
        .map(PathBuf::from)
        .ok_or("set LM_FAMILY_ASSETS")?;
    let mut recipe = RecipeV1::new();
    for pair in std::env::var("LM_FAMILY_CONTROLS")
        .unwrap_or_default()
        .split_whitespace()
    {
        let (name, value) = pair
            .split_once('=')
            .ok_or_else(|| format!("bad control {pair:?}"))?;
        let value = value
            .parse::<f32>()
            .map_err(|e| format!("bad value {value:?}: {e}"))?;
        recipe.set_control(name, value);
    }
    let assets = AssetSet::load_dir(&dir).map_err(|e| format!("assets: {e:?}"))?;
    let c = compile(&recipe, &assets).map_err(|e| format!("compile: {e:?}"))?;
    let positions = &c.rest_render_meshes()[0].positions;
    let mut bytes = Vec::with_capacity(positions.len() * 12);
    for p in positions {
        for n in [p.x, p.y, p.z] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
    }
    let hex: String = sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    println!("#positions,{},{hex}", positions.len());
    for (i, p) in positions.iter().enumerate() {
        println!(
            "v,{i},{:08x},{:08x},{:08x}",
            p.x.to_bits(),
            p.y.to_bits(),
            p.z.to_bits()
        );
    }
    let world = c.skeleton().world_bind();
    for (j, joint) in c.skeleton().joints().iter().enumerate() {
        let t = world[j].transform_point(Vec3::ZERO);
        println!("j,{},{:e},{:e},{:e}", joint.name, t.x, t.y, t.z);
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("figure_family_dump: {e}");
            ExitCode::FAILURE
        }
    }
}
