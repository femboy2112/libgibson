//! Studio preview of a converted figure asset: loads an asset directory (`gibson.figure.asset.v1`,
//! produced by `tools/figure_assets/`), compiles a recipe built from the arguments and writes the
//! offscreen software preview of the rest frame as a PNG.
//!
//! ```text
//! cargo run --release --features figure --example figure_studio_preview -- \
//!     <asset dir> <out.png> <width> <height> [control=value ...] [garment=<id>[:<slot>] ...] \
//!     [hair=<id>] [adapt]
//! ```
//! No asset ships with the crate; pass a local directory.

use std::path::Path;
use std::process::ExitCode;

use gibson::figure::preview::PreviewOptions;
use gibson::figure::studio::{
    compile, AdaptationV1, AssetSet, GarmentV1, MaterialSlotV1, RecipeV1,
};

fn slot(name: &str, albedo: [f32; 3], roughness: f32) -> MaterialSlotV1 {
    let mut m = MaterialSlotV1::new(name);
    m.albedo = albedo;
    m.roughness = roughness;
    m
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!("usage: <asset dir> <out.png> <width> <height> [control=v] [garment=id[:slot]] [hair=id] [adapt]");
        return ExitCode::from(2);
    }
    let (Ok(w), Ok(h)) = (args[2].parse::<u32>(), args[3].parse::<u32>()) else {
        eprintln!("width and height must be integers");
        return ExitCode::from(2);
    };
    let assets = match AssetSet::load_dir(Path::new(&args[0])) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("asset load failed: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    let mut r = RecipeV1::new();
    // neutral linear-light defaults: skin on `base`, garments on `panel`, accents on `trim`
    r.materials = vec![
        slot("base", [0.56, 0.39, 0.31], 0.55),
        slot("panel", [0.10, 0.16, 0.30], 0.7),
        slot("trim", [0.45, 0.42, 0.38], 0.6),
    ];
    r.hair.color = [0.09, 0.05, 0.03];
    for a in &args[4..] {
        if a == "adapt" {
            r.fit_params.adaptation = Some(AdaptationV1::default());
        } else if let Some(id) = a.strip_prefix("hair=") {
            r.hair.style_id = Some(id.to_string());
        } else if let Some(g) = a.strip_prefix("garment=") {
            let (id, s) = g.split_once(':').unwrap_or((g, "panel"));
            let mut garment = GarmentV1::new(id);
            garment.slot_bindings.push((String::new(), s.to_string()));
            r.garments.push(garment);
        } else if let Some((name, v)) = a.split_once('=') {
            let Ok(v) = v.parse::<f32>() else {
                eprintln!("bad control value: {a}");
                return ExitCode::from(2);
            };
            r.controls.push((name.to_string(), v));
        } else {
            eprintln!("unknown argument: {a}");
            return ExitCode::from(2);
        }
    }
    let c = match compile(&r, &assets) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("compile failed: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    for rep in c.fit_status() {
        println!("fit {rep:?}");
    }
    for d in c.diagnostics() {
        println!("diag {d:?}");
    }
    let opts = PreviewOptions {
        width: w,
        height: h,
        ..PreviewOptions::default()
    };
    let t0 = std::time::Instant::now();
    let img = match c.preview(&c.rest_frame(), None, opts) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("preview failed: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    let ms = t0.elapsed().as_millis();
    if let Err(e) = std::fs::write(&args[1], img.to_png_bytes()) {
        eprintln!("write failed: {e}");
        return ExitCode::FAILURE;
    }
    println!(
        "wrote {} {}x{} render_ms={ms}",
        args[1], img.width, img.height
    );
    ExitCode::SUCCESS
}
