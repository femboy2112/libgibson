//! Emit the generic still's real libgibson performance and the compiler's fitted garment cages.
use std::error::Error;
use std::fs;
use std::path::Path;

use gibson::figure::math::Vec3;
use gibson::figure::perform::{
    perform, Beat, PerformanceScript, Rig, RigGeometry, RigMap, RigRoles, Style, TapeIdentity,
};
use gibson::figure::studio::{
    compile, AdaptationV1, AssetSet, Compiled, FitStatus, GarmentV1, MaterialSlotV1, RecipeV1,
    BODY_FRONT,
};
use serde_json::{from_value, json, Map, Value};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn generic_recipe(preset: &Value, style: &Style) -> Result<RecipeV1> {
    if preset["name"] != "generic_neutral" || preset["rig"] != "game_engine" {
        return Err("the still requires the generic_neutral preset and the game_engine rig".into());
    }
    let mut recipe = RecipeV1::new();
    let controls = preset["controls"].as_object().ok_or("missing controls")?;
    for (name, value) in controls {
        recipe.set_control(name, from_value(value.clone())?);
    }
    let render = &preset["render"];
    let materials = render["materials"].as_object().ok_or("missing materials")?;
    for (slot, spec) in materials {
        let mut m = MaterialSlotV1::new(slot);
        m.albedo = from_value(spec["albedo"].clone())?;
        m.roughness = from_value(spec["roughness"].clone())?;
        m.metalness = from_value(spec["metalness"].clone())?;
        if let Some(color) = spec.get("emissive") {
            m.emissive = Some((
                from_value(color.clone())?,
                from_value(spec["emissive_intensity"].clone())?,
            ));
        }
        recipe.materials.push(m);
    }
    recipe.hair.style_id = Some(from_value(render["hair"]["id"].clone())?);
    recipe.hair.color = from_value(render["hair"]["color"].clone())?;
    let garment_ids: Vec<String> = from_value(render["garments"].clone())?;
    let bindings = render["bindings"]
        .as_object()
        .ok_or("missing garment bindings")?;
    for id in garment_ids {
        let kind = if id == "skirt_a_line" {
            "skirt"
        } else {
            "garment"
        };
        let slot = bindings
            .get(&format!("garment:{id}"))
            .or_else(|| bindings.get(kind))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing slot binding for {id}"))?;
        if !materials.contains_key(slot) {
            return Err(format!("undefined slot {slot} on {id}").into());
        }
        let mut garment = GarmentV1::new(id);
        garment.slot_bindings.push((String::new(), slot.to_owned()));
        recipe.garments.push(garment);
    }
    recipe.fit_params.adaptation = Some(AdaptationV1::default());
    recipe.motion_style.preset = style.preset.clone();
    recipe.motion_style.amplitude = style.amplitude;
    recipe.motion_style.tempo = style.tempo;
    recipe.motion_style.settle_ease_ms = style.settle_ms as f32;
    Ok(recipe)
}

fn game_engine_rig(compiled: &Compiled) -> Result<Rig> {
    let skeleton = compiled.skeleton();
    let roles = RigRoles {
        pelvis: "pelvis",
        spine: &["spine_01", "spine_02", "spine_03"],
        neck: Some("neck_01"),
        head: "head",
        eyes: &[],
        arm_l: ["upperarm_l", "lowerarm_l", "hand_l"],
        arm_r: ["upperarm_r", "lowerarm_r", "hand_r"],
        leg_l: ["thigh_l", "calf_l", "foot_l"],
        leg_r: ["thigh_r", "calf_r", "foot_r"],
    };
    let map = RigMap::from_names(
        skeleton,
        &roles,
        RigGeometry::new(Vec3::new(0.0, 0.0, 1.0), BODY_FRONT),
    )?;
    Ok(Rig::new(skeleton.clone(), map)?)
}

fn source_manifest(asset_dir: &Path) -> Result<String> {
    let index = fs::read_to_string(asset_dir.join("index.txt"))?;
    let mut hashes = index
        .lines()
        .filter_map(|l| l.strip_prefix("source_manifest_sha256 "));
    let sha = hashes
        .next()
        .ok_or("asset index lacks source_manifest_sha256")?;
    if hashes.next().is_some() || sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid source_manifest_sha256 in asset index".into());
    }
    Ok(sha.to_owned())
}

fn run(asset_dir: &Path, preset_path: &Path, out: &Path) -> Result<()> {
    let preset: Value = serde_json::from_slice(&fs::read(preset_path)?)?;
    let style = Style::restrained();
    let recipe = generic_recipe(&preset, &style)?;
    let manifest_sha = source_manifest(asset_dir)?;
    let assets = AssetSet::load_dir(asset_dir)?;
    let compiled = compile(&recipe, &assets)?;
    let reports = compiled.fit_status();
    for report in reports {
        println!("fit {report:?}");
    }
    for diagnostic in compiled.diagnostics() {
        println!("diagnostic {diagnostic:?}");
    }
    if recipe.garments.is_empty()
        || reports.len() != recipe.garments.len()
        || !reports
            .iter()
            .zip(&recipe.garments)
            .all(|(r, g)| r.id == g.id && r.status == FitStatus::Fitted)
    {
        return Err("one or more requested garments lack a Fitted verdict".into());
    }
    let mut positions = Map::new();
    for mesh in compiled.drawable_garments() {
        let xyz: Vec<_> = mesh.positions.iter().map(|v| [v.x, v.y, v.z]).collect();
        positions.insert(mesh.id.clone(), serde_json::to_value(xyz)?);
    }
    if !reports.iter().all(|r| positions.contains_key(&r.id)) {
        return Err("compiled output is missing a fitted garment mesh".into());
    }
    let rig = game_engine_rig(&compiled)?;
    let fps = 30;
    let script = PerformanceScript::new(style).with(Beat::Idle {
        secs: 1.0 / fps as f32,
    });
    let tape = perform(&rig, &script, fps)?;
    let id = TapeIdentity {
        body_rev_id: compiled.body_rev_id().to_hex(),
        fit_id: compiled.fit_id().to_hex(),
        present_id: compiled.present_id().to_hex(),
        asset_manifest_sha256: manifest_sha,
        controls: recipe.controls.clone(),
        garments: reports
            .iter()
            .map(|r| (r.id.clone(), format!("{:?}", r.status)))
            .collect(),
        materials_json: serde_json::to_string(&preset["render"]["materials"])?,
        camera_cuts: vec![],
    };
    let fit = json!({
        "body_rev_id": id.body_rev_id,
        "fit_id": id.fit_id,
        "present_id": id.present_id,
        "asset_manifest_sha256": id.asset_manifest_sha256,
        "tape_bin_sha256": tape.bin_sha256(),
        "positions": positions,
    });
    let files = tape.to_files(&id);
    fs::create_dir_all(out)?;
    fs::write(out.join("generic.fit.json"), serde_json::to_vec(&fit)?)?;
    fs::write(out.join("generic.tape.bin"), files.bin)?;
    fs::write(out.join("generic.tape.json"), files.header_json)?;
    println!(
        "fps={fps} frames={} requested_idle_s=1/30 still_frame=0 fit_id={}",
        tape.frame_count, id.fit_id
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [asset, preset, out] = args.as_slice() else {
        return Err("usage: gibson-still-recipe ASSET_DIR GENERIC_PRESET_JSON OUT_DIR".into());
    };
    run(Path::new(asset), Path::new(preset), Path::new(out))
}
