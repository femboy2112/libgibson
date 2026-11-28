//! Bit-level record of a living-matter run over a whole scene tape: one SHA-256 per frame of
//! every output mesh (positions and normals, as bits) plus the checkpoint bytes, and the
//! contact-index time of each frame. Two builds agree bit for bit when their digest lines match.
use gibson::figure::{
    living_matter::{DynamicsConfig, Fabric, FigureDynamics},
    sha256::Sha256,
    studio::{compile_shared, AssetSet, TapeReader},
};
use serde_json::Value;
use std::{fs, path::Path, sync::Arc};
#[path = "../recipe.rs"]
mod recipe;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn hex(d: [u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [asset, preset, run] = args.as_slice() else {
        return Err("usage: matter_tape_digest ASSET PRESET RUN".into());
    };
    let preset: Value = serde_json::from_slice(&fs::read(preset)?)?;
    let recipe = recipe::build_recipe(&preset, &recipe::movement_style(&preset)?)?;
    let assets = Arc::new(AssetSet::load_dir(Path::new(asset))?);
    let comp = compile_shared(&recipe, &assets)?;
    let base = Path::new(run);
    let header = fs::read(base.join("generic.tape.json"))?;
    let bin = fs::read(base.join("generic.tape.bin"))?;
    let tape = TapeReader::new(&header, &bin)?;
    let mut cfg = DynamicsConfig::default();
    cfg.fabric = Fabric::Medium;
    cfg.firmness = 0.5;
    cfg.damping = 0.5;
    cfg.support = 0.5;
    let mut dyns = FigureDynamics::new(&comp, cfg)?;
    let mut chain = Sha256::new();
    for i in 0..tape.header().frame_count {
        let frame = comp.tape_frame(&tape, i)?;
        let (meshes, t) = dyns.step_profiled(&comp, &frame, 30)?;
        let mut h = Sha256::new();
        for m in &meshes {
            h.update(m.id.as_bytes());
            for p in m.positions.iter().chain(m.normals.iter()) {
                for c in [p.x, p.y, p.z] {
                    h.update(&c.to_bits().to_le_bytes());
                }
            }
        }
        h.update(&dyns.checkpoint_bytes());
        let d = h.finalize();
        chain.update(&d);
        println!(
            "frame {i} {} contact_ms {:.3} hair_ms {:.3} step_ms {:.3}",
            hex(d),
            t.contact_build.as_secs_f64() * 1e3,
            t.hair_and_contacts.as_secs_f64() * 1e3,
            (t.skin_and_regions + t.contact_build + t.cloth_and_contacts + t.hair_and_contacts)
                .as_secs_f64()
                * 1e3
        );
    }
    println!("chain {}", hex(chain.finalize()));
    Ok(())
}
