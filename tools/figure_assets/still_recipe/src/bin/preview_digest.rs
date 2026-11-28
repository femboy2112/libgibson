//! Bit-level record of the native preview over a whole scene tape: the living-matter meshes of
//! each frame are drawn by `Compiled::preview_meshes` with the studio's default view camera
//! (rest-shaped body bounds, fill 0.78, azimuth 0, elevation 4) at each requested
//! size, and one SHA-256 of the RGB8 bytes is printed per frame and size with the draw time.
//! Two builds draw identical frames when their digest lines match.
use gibson::figure::{
    living_matter::{DynamicsConfig, Fabric, FigureDynamics},
    preview::{frame_bounds_view, PreviewOptions},
    sha256::Sha256,
    studio::{compile_shared, AssetSet, TapeReader},
};
use serde_json::Value;
use std::{fs, path::Path, sync::Arc, time::Instant};
#[path = "../recipe.rs"]
mod recipe;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn hex(d: [u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [asset, preset, run, rest @ ..] = args.as_slice() else {
        return Err("usage: preview_digest ASSET PRESET RUN [SSAA [WxH ...]]".into());
    };
    let ssaa: u8 = rest.first().map_or(Ok(1), |s| s.parse())?;
    let sizes: Vec<(u32, u32)> = if rest.len() > 1 {
        rest[1..]
            .iter()
            .map(|s| {
                let (w, h) = s.split_once('x').ok_or("size is WxH")?;
                Ok((w.parse()?, h.parse()?))
            })
            .collect::<Result<_>>()?
    } else {
        vec![(180, 220), (120, 147)]
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
    let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for p in comp.shaped_positions().iter().take(comp.body_vertex_count()) {
        for (i, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            low[i] = low[i].min(v);
            high[i] = high[i].max(v);
        }
    }
    let cams: Vec<_> = sizes
        .iter()
        .map(|&(w, h)| frame_bounds_view((low, high), w as f32 / h as f32, 0.78, 0.0, 4.0))
        .collect();
    let mut dyns = FigureDynamics::new(&comp, cfg)?;
    let mut chain = Sha256::new();
    for i in 0..tape.header().frame_count {
        let frame = comp.tape_frame(&tape, i)?;
        let meshes = dyns.step(&comp, &frame, 30)?;
        if i == 0 {
            let tris: usize = meshes.iter().map(|m| m.triangles.len()).sum();
            let verts: usize = meshes.iter().map(|m| m.positions.len()).sum();
            println!("scene meshes {} verts {verts} tris {tris} ssaa {ssaa}", meshes.len());
        }
        let mut line = format!("frame {i}");
        for (&(width, height), cam) in sizes.iter().zip(&cams) {
            let opts = PreviewOptions {
                width,
                height,
                ssaa,
                ..Default::default()
            };
            let t = Instant::now();
            let img = comp.preview_meshes(&meshes, Some(cam), opts);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            let mut h = Sha256::new();
            h.update(&img.rgb8);
            let d = h.finalize();
            chain.update(&d);
            line += &format!(" {width}x{height} {} ms {ms:.3}", hex(d));
        }
        println!("{line}");
    }
    println!("chain {}", hex(chain.finalize()));
    Ok(())
}
