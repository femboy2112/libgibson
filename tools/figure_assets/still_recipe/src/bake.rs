//! Authoritative world-space cage tracks, with a digest-bound manifest published last.
use gibson::figure::{
    living_matter::{DynamicsConfig, FigureDynamics},
    math::Vec3,
    perform::Tape,
    sha256::Sha256,
    studio::{Compiled, MeshRole, TapeFrame},
};
use serde_json::{json, Value};
use std::{
    error::Error,
    fs,
    io::{BufWriter, Write},
    path::Path,
    time::Instant,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn bake(
    compiled: &Compiled,
    tape: &Tape,
    fit: &Value,
    out: &Path,
    config: DynamicsConfig,
    preroll_frames: u32,
) -> Result<(), Box<dyn Error>> {
    if preroll_frames > 600 || tape.frame_count > 1_000_000 {
        return Err("physical bake exceeds bounded timeline".into());
    }
    let mut props = Vec::new();
    let scene = out.join("scene.json");
    if scene.is_file() {
        let scene: Value = serde_json::from_slice(&fs::read(scene)?)?;
        if let Some(seat) = scene.get("seat").filter(|value| !value.is_null()) {
            let center: [f32; 3] = serde_json::from_value(seat["center"].clone())?;
            let size: [f32; 3] = serde_json::from_value(seat["size"].clone())?;
            let v = |a: [f32; 3]| Vec3::new(a[0], a[1], a[2]);
            props.push((v(center), v(size)));
        }
    }
    let mut sim = FigureDynamics::with_props(compiled, config, &props)?;
    let meshes = compiled.rest_render_meshes();
    let outputs: Vec<_> = meshes
        .iter()
        .enumerate()
        .filter(|(_, m)| matches!(m.role, MeshRole::Garment | MeshRole::Body | MeshRole::Hair))
        .collect();
    let mut stride = 0;
    let mut records = Vec::new();
    for (_, m) in &outputs {
        records.push(json!({"id":m.id,"vertices":m.positions.len(),"offset_bytes":stride}));
        stride += m.positions.len() * 12;
    }
    let bytes = stride
        .checked_mul(tape.frame_count)
        .ok_or("physical size overflow")?;
    if bytes > 2_000_000_000 {
        return Err("physical bake exceeds 2 GB".into());
    }
    let id = hex(&sim.config_id());
    let start = Instant::now();
    let kinematic = |i| TapeFrame::new(tape.root(i), tape.locals(i));
    let first = kinematic(0);
    for _ in 0..preroll_frames {
        sim.step(compiled, &first, tape.fps)?;
    }
    let temp = out.join(format!(".matter-{}.tmp", std::process::id()));
    let mut writer = BufWriter::new(fs::File::create(&temp)?);
    let mut digest = Sha256::new();
    let mut frame_bytes = Vec::with_capacity(stride);
    let mut checkpoints = Vec::new();
    for i in 0..tape.frame_count {
        // A checkpoint is the state immediately before next_frame, including pre-roll.
        let mut replay = None;
        if i % (tape.fps as usize * 2) == 0 {
            let state = sim.checkpoint_bytes();
            let state_sha = hex(&gibson::figure::sha256::digest(&state));
            let name = format!("checkpoint-{state_sha}.bin");
            let tmp = out.join(format!(".checkpoint-{}.tmp", std::process::id()));
            fs::write(&tmp, &state)?;
            fs::rename(tmp, out.join(&name))?;
            checkpoints
                .push(json!({"next_frame":i,"tick":sim.tick(),"file":name,"sha256":state_sha}));
            let mut resumed = FigureDynamics::with_props(compiled, config, &props)?;
            resumed.restore_bytes(&state)?;
            replay = Some(resumed);
        }
        let pose = kinematic(i);
        let dynamic = sim.step(compiled, &pose, tape.fps)?;
        if let Some(mut resumed) = replay {
            let actual = resumed.step(compiled, &pose, tape.fps)?;
            if actual != dynamic || resumed.checkpoint_bytes() != sim.checkpoint_bytes() {
                return Err("checkpoint replay differs from uninterrupted actor".into());
            }
        }
        frame_bytes.clear();
        for (idx, _) in &outputs {
            for p in &dynamic[*idx].positions {
                for x in [p.x, p.y, p.z] {
                    frame_bytes.extend_from_slice(&x.to_le_bytes());
                }
            }
        }
        writer.write_all(&frame_bytes)?;
        digest.update(&frame_bytes);
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    let sha = hex(&digest.finalize());
    let filename = format!("matter-{sha}.bin");
    fs::rename(temp, out.join(&filename))?;
    let header = json!({"format":"gibson.living-matter.bake.v1","backend":"rust-xpbd-region-v2","basis":"world-z-up-m",
        "binary":filename,"bin_sha256":sha,"tape_bin_sha256":tape.bin_sha256(),"asset_manifest_sha256":fit["asset_manifest_sha256"],
        "fit_id":compiled.fit_id().to_hex(),"body_rev_id":compiled.body_rev_id().to_hex(),"present_id":compiled.present_id().to_hex(),
        "sim_config_id":id,"fps":tape.fps,"frames":tape.frame_count,"preroll_frames":preroll_frames,
        "gravity":[config.gravity.x,config.gravity.y,config.gravity.z],"fabric":format!("{:?}",config.fabric),
        "firmness":config.firmness,"damping":config.damping,"support":config.support,"intensity":config.intensity,
        "checkpoints":checkpoints,"substeps":config.substeps,"iterations":config.iterations,"stride_bytes":stride,"meshes":records});
    let tmp_header = out.join(format!(".matter-{}.json.tmp", std::process::id()));
    fs::write(&tmp_header, serde_json::to_vec_pretty(&header)?)?;
    fs::rename(tmp_header, out.join("matter.json"))?;
    println!(
        "MATTER_BAKE frames={} preroll={} bytes={bytes} seconds={:.3} sha256={sha}",
        tape.frame_count,
        preroll_frames,
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
