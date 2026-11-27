//! Held-out trajectory assay on a real cooked actor. Both histories end at the SAME taped pose.
use gibson::figure::{
    living_matter::{DynamicsConfig, FigureDynamics},
    studio::{compile, AssetSet, Compiled, RenderMesh, TapeFrame, TapeReader},
};
use serde_json::{json, Value};
use std::{fs, path::Path};
#[path = "../recipe.rs"]
mod recipe;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn sample(sim: &mut FigureDynamics, c: &Compiled, pose: &TapeFrame) -> Result<Vec<RenderMesh>> {
    Ok(sim.step(c, pose, 30)?)
}
fn rms(a: &[gibson::figure::Vec3], b: &[gibson::figure::Vec3]) -> f32 {
    (a.iter()
        .zip(b)
        .map(|(a, b)| {
            let v = a.sub(*b);
            v.dot(v)
        })
        .sum::<f32>()
        / a.len() as f32)
        .sqrt()
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [asset, preset, dir] = args.as_slice() else {
        return Err("usage: actor_history ASSET PRESET TAPE_DIRECTORY".into());
    };
    let p: Value = serde_json::from_slice(&fs::read(preset)?)?;
    let c = compile(
        &recipe::build_recipe(&p, &recipe::movement_style(&p)?)?,
        &AssetSet::load_dir(Path::new(asset))?,
    )?;
    let dir = Path::new(dir);
    let h = fs::read(dir.join("generic.tape.json"))?;
    let bin = fs::read(dir.join("generic.tape.bin"))?;
    let hj: Value = serde_json::from_slice(&h)?;
    if hj["present_id"] != c.present_id().to_hex() {
        return Err("candidate identity mismatch".into());
    }
    let t = TapeReader::new(&h, &bin)?;
    let pose = c.tape_frame(&t, 0)?;
    let mut a = FigureDynamics::new(&c, DynamicsConfig::default())?;
    for _ in 0..45 {
        sample(&mut a, &c, &pose)?;
    }
    let mut b = FigureDynamics::new(&c, DynamicsConfig::default())?;
    b.restore_bytes(&a.checkpoint_bytes())?;
    let mut records = vec![];
    let mut first = 0.;
    let mut final_amp = 0.;
    for i in 0..150 {
        let mut excursion = pose.clone();
        if i < 15 {
            excursion.root_t.x += 0.08 * (std::f32::consts::PI * (i + 1) as f32 / 16.).sin();
        }
        let base = sample(&mut a, &c, &pose)?;
        let other = sample(&mut b, &c, &excursion)?;
        if [15, 30, 60, 90, 149].contains(&i) {
            if pose != excursion {
                return Err("histories observed at different pose".into());
            }
            let mut rows = vec![];
            let mut greatest = 0f32;
            for (x, y) in base.iter().zip(other.iter()) {
                let delta = rms(&x.positions, &y.positions);
                if x.id == "skirt_a_line" || x.role == gibson::figure::studio::MeshRole::Hair {
                    greatest = greatest.max(delta);
                }
                rows.push(json!({"mesh":x.id,"rms_m":delta}));
            }
            if i == 15 {
                first = greatest;
            }
            if i == 149 {
                final_amp = greatest;
            }
            records.push(json!({"frame":i,"meshes":rows}));
        }
    }
    let result = json!({"body_rev_id":c.body_rev_id().to_hex(),"present_id":c.present_id().to_hex(),"config_id":a.config_id().iter().map(|b|format!("{b:02x}")).collect::<String>(),
        "fps":30,"common_preroll":45,"lateral_excursion_m":0.08,"held_pose_frames":135,"first_rms_m":first,"final_rms_m":final_amp,"records":records});
    println!("{}", serde_json::to_string_pretty(&result)?);
    if first <= 0.001 || final_amp >= first * 0.5 {
        return Err("real held-pose transient or half-amplitude recovery failed".into());
    }
    Ok(())
}
