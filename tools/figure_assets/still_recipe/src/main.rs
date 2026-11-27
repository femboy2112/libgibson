//! Emit the generic still's real libgibson performance and the compiler's fitted garment cages.
use std::error::Error;
use std::fs;
use std::path::Path;

mod bake;
mod recipe;
mod scene;

use gibson::figure::math::Vec3;
use gibson::figure::perform::{
    perform, Beat, PerformanceScript, Rig, RigGeometry, RigMap, RigRoles, Style, TapeIdentity,
};
use gibson::figure::studio::{compile, AssetSet, Compiled, FitStatus, BODY_FRONT};
use recipe::build_recipe;
use serde_json::{json, Map, Value};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

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

fn run(
    asset_dir: &Path,
    preset_path: &Path,
    out: &Path,
    options: Option<scene::Options>,
) -> Result<()> {
    let preset: Value = serde_json::from_slice(&fs::read(preset_path)?)?;
    if options.is_none() && preset["name"] != "generic_neutral" {
        return Err("the reference still requires generic_neutral".into());
    }
    let style = if options.is_some() {
        recipe::movement_style(&preset)?
    } else {
        Style::restrained()
    };
    let recipe = build_recipe(&preset, &style)?;
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
    let mut rig = game_engine_rig(&compiled)?;
    if options
        .as_ref()
        .is_some_and(|o| o.sequence == scene::Sequence::Reference)
    {
        rig = scene::reference_rig(rig, &compiled)?;
    }
    let fps = 30;
    let (mut script, seat) = match options.as_ref() {
        Some(o) => scene::performance(&rig, style, o.sequence)?,
        None => (
            PerformanceScript::new(style).with(Beat::Idle {
                secs: 1.0 / fps as f32,
            }),
            None,
        ),
    };
    let mut tape = perform(&rig, &script, fps)?;
    let chair = if let Some(mut position) = seat {
        let (mut data, min_x) = scene::seated_chair(&compiled, &tape, position)?;
        if position.x < min_x {
            println!("seat_lateral_adjustment_m={}", min_x - position.x);
            position.x = min_x + 0.002;
            for beat in &mut script.beats {
                if let Beat::Sit { seat_pos, .. } = beat {
                    *seat_pos = position;
                }
            }
            tape = perform(&rig, &script, fps)?;
            let (updated, required_x) = scene::seated_chair(&compiled, &tape, position)?;
            if position.x < required_x {
                return Err(format!(
                    "walk clearance requires pelvis x {required_x}, got {}",
                    position.x
                )
                .into());
            }
            data = updated;
        }
        Some(data)
    } else {
        None
    };
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
        camera_cuts: if options.is_some() {
            scene::cuts(&tape)
        } else {
            Vec::new()
        },
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
    if let Some(o) = options {
        scene::write_setup(&tape, &fit, out, chair)?;
        if !o.kinematic_only {
            bake::bake(&compiled, &tape, &fit, out, o.config, o.preroll_frames)?;
        }
        println!(
            "fps={fps} frames={} sequence={:?} fit_id={}",
            tape.frame_count, o.sequence, id.fit_id
        );
        return Ok(());
    }
    println!(
        "fps={fps} frames={} requested_idle_s=1/30 still_frame=0 fit_id={}",
        tape.frame_count, id.fit_id
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [asset, preset, out, flags @ ..] = args.as_slice() else {
        return Err("usage: gibson-still-recipe ASSET_DIR PRESET_JSON OUT_DIR [--performance idle|scene|reference --kinematic-only | physics options]".into());
    };
    run(
        Path::new(asset),
        Path::new(preset),
        Path::new(out),
        scene::Options::parse(flags)?,
    )
}
