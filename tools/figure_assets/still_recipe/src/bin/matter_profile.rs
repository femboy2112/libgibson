//! Reproducible actor workload for a bounded exclusive hardware measurement.
use gibson::figure::{
    living_matter::{DynamicsConfig, FigureDynamics},
    preview::PreviewOptions,
    studio::{compile_shared, AssetSet, TapeReader},
};
use serde_json::{json, Value};
use std::{fs, path::Path, sync::Arc, time::Instant};
#[path = "../recipe.rs"]
mod recipe;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [asset, preset, run] = args.as_slice() else {
        return Err("usage: matter_profile ASSET PRESET RUN".into());
    };
    let preset: Value = serde_json::from_slice(&fs::read(preset)?)?;
    let recipe = recipe::build_recipe(&preset, &recipe::movement_style(&preset)?)?;
    let time = Instant::now();
    let assets = Arc::new(AssetSet::load_dir(Path::new(asset))?);
    let load_ms = time.elapsed().as_secs_f64() * 1e3;
    let mut body_recipe = recipe.clone();
    body_recipe.garments.clear();
    let time = Instant::now();
    let body = compile_shared(&body_recipe, &assets)?;
    let body_compile_ms = time.elapsed().as_secs_f64() * 1e3;
    let time = Instant::now();
    let comp = compile_shared(&recipe, &assets)?;
    let dressed_compile_ms = time.elapsed().as_secs_f64() * 1e3;
    if body.body_rev_id() != comp.body_rev_id() {
        return Err("wardrobe changed neutral body".into());
    }
    let base = Path::new(run);
    let header = fs::read(base.join("generic.tape.json"))?;
    let h: Value = serde_json::from_slice(&header)?;
    let bin = fs::read(base.join("generic.tape.bin"))?;
    let tape = TapeReader::new(&header, &bin)?;
    if h["present_id"] != comp.present_id().to_hex() {
        return Err("workload recipe mismatch".into());
    }
    let mut dyns = FigureDynamics::new(&comp, DynamicsConfig::default())?;
    let mut reference = FigureDynamics::new(&comp, DynamicsConfig::default())?;
    let mut rows = Vec::new();
    for i in 0..24.min(tape.header().frame_count) {
        let frame = comp.tape_frame(&tape, i)?;
        let time = Instant::now();
        let (meshes, t) = dyns.step_profiled(&comp, &frame, 30)?;
        let all = time.elapsed().as_secs_f64() * 1e3;
        let expected = reference.step(&comp, &frame, 30)?;
        if expected != meshes || dyns.checkpoint_bytes() != reference.checkpoint_bytes() {
            return Err("instrument changes physical result".into());
        }
        if i == 5 {
            let state = dyns.checkpoint_bytes();
            let mut swapped = state.clone();
            swapped[20] ^= 1;
            if dyns.restore_bytes(&swapped).is_ok()
                || dyns.checkpoint_bytes() != state
                || dyns.step(&comp, &frame, 60).is_ok()
                || dyns.checkpoint_bytes() != state
            {
                return Err("damaged state or changed rate admitted".into());
            }
        }
        let opts = PreviewOptions {
            width: 180,
            height: 220,
            ssaa: 1,
            ..Default::default()
        };
        let time = Instant::now();
        let preview = comp.preview_meshes(&meshes, None, opts);
        std::hint::black_box(preview);
        let draw_ms = time.elapsed().as_secs_f64() * 1e3;
        let ms = |dt: std::time::Duration| dt.as_secs_f64() * 1e3;
        rows.push(json!({"frame":i,"step_ms":all,"skin_region_ms":ms(t.skin_and_regions),"contact_index_ms":ms(t.contact_build),
                          "cloth_ms":ms(t.cloth_and_contacts),"hair_ms":ms(t.hair_and_contacts),"native_raster_ms":draw_ms}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"units":"milliseconds","asset_load_ms":load_ms,"body_compile_ms":body_compile_ms,
        "dressed_compile_ms":dressed_compile_ms,"body_rev_id":comp.body_rev_id().to_hex(),"present_id":comp.present_id().to_hex(),"rows":rows})
        )?
    );
    Ok(())
}
