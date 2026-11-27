//! Encode an actual native RGB frame sequence from a verified figure tape and optional physical bake.
use gibson::figure::{
    math::Vec3,
    preview::{frame_bounds_view, PreviewOptions},
    sha256::Sha256,
    studio::{compile, AssetSet, Compiled, RenderMesh, TapeReader},
};
use serde_json::Value;
use std::{
    error::Error,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
};
#[path = "../recipe.rs"]
mod recipe;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn normals(m: &mut RenderMesh) {
    m.normals = vec![Vec3::ZERO; m.positions.len()];
    for &[a, b, c] in &m.triangles {
        let (a, b, c) = (a as usize, b as usize, c as usize);
        let n = m.positions[b]
            .sub(m.positions[a])
            .cross(m.positions[c].sub(m.positions[a]));
        for i in [a, b, c] {
            m.normals[i] = m.normals[i].add(n);
        }
    }
    for n in &mut m.normals {
        *n = n.try_normalize().unwrap_or(Vec3::new(0., 0., 1.));
    }
}
struct Track {
    file: File,
    channels: Vec<(usize, usize)>,
}
impl Track {
    fn open(path: &Path, id: &Value, meshes: &[RenderMesh]) -> Result<Self> {
        let h = json(&path.join("matter.json"))?;
        if h["format"] != "gibson.living-matter.bake.v1"
            || h["basis"] != "world-z-up-m"
            || h["tape_bin_sha256"] != id["bin_sha256"]
            || h["frames"] != id["frame_count"]
            || h["fps"].as_f64() != id["fps"].as_f64()
        {
            return Err("physical track mismatch".into());
        }
        for k in [
            "present_id",
            "fit_id",
            "body_rev_id",
            "asset_manifest_sha256",
        ] {
            if h[k] != id[k] {
                return Err(format!("track {k} mismatch").into());
            }
        }
        let sha = h["bin_sha256"].as_str().ok_or("missing digest")?;
        if sha.len() != 64
            || !sha.bytes().all(|b| b.is_ascii_hexdigit())
            || h["binary"] != format!("matter-{sha}.bin")
        {
            return Err("invalid physical filename".into());
        }
        let bin = path.join(format!("matter-{sha}.bin"));
        let mut input = File::open(&bin)?;
        let mut hash = Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = input.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
        }
        if hex(&hash.finalize()) != sha {
            return Err("physical digest mismatch".into());
        }
        let mut channels = Vec::new();
        let mut stride = 0;
        for ch in h["meshes"].as_array().ok_or("missing channels")? {
            let idx = meshes
                .iter()
                .position(|m| Some(m.id.as_str()) == ch["id"].as_str())
                .ok_or("unknown mesh channel")?;
            let n = meshes[idx].positions.len();
            if channels.iter().any(|&(i, _)| i == idx)
                || ch["vertices"] != n
                || ch["offset_bytes"] != stride
            {
                return Err("physical topology mismatch".into());
            }
            channels.push((idx, n));
            stride += 12 * n;
        }
        if h["stride_bytes"] != stride
            || fs::metadata(&bin)?.len()
                != h["frames"].as_u64().ok_or("frame count")? * stride as u64
        {
            return Err("physical length mismatch".into());
        }
        Ok(Self {
            file: File::open(bin)?,
            channels,
        })
    }
    fn read(&mut self, meshes: &mut [RenderMesh]) -> Result<()> {
        for &(idx, n) in &self.channels {
            let mut data = vec![0; n * 12];
            self.file.read_exact(&mut data)?;
            let mesh = &mut meshes[idx];
            for (p, bytes) in mesh.positions.iter_mut().zip(data.chunks_exact(12)) {
                let get = |i| f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
                *p = Vec3::new(get(0), get(4), get(8));
                if !p.is_finite() {
                    return Err("nonfinite physical position".into());
                }
            }
            normals(mesh);
        }
        Ok(())
    }
}

fn box_mesh(template: &RenderMesh, center: [f32; 3], size: [f32; 3]) -> RenderMesh {
    let mut m = template.clone();
    m.id = "scene prop".into();
    m.positions = (0..8)
        .map(|i| {
            Vec3::new(
                center[0] + if i & 1 == 0 { -0.5 } else { 0.5 } * size[0],
                center[1] + if i & 2 == 0 { -0.5 } else { 0.5 } * size[1],
                center[2] + if i & 4 == 0 { -0.5 } else { 0.5 } * size[2],
            )
        })
        .collect();
    m.triangles = vec![
        [0, 2, 1],
        [1, 2, 3],
        [4, 5, 6],
        [5, 7, 6],
        [0, 1, 4],
        [1, 5, 4],
        [2, 6, 3],
        [3, 6, 7],
        [0, 4, 2],
        [2, 4, 6],
        [1, 3, 5],
        [3, 7, 5],
    ];
    normals(&mut m);
    m.slot_per_triangle = vec![2; m.triangles.len()];
    m
}
fn chair(scene: &Value, template: &RenderMesh) -> Result<Vec<RenderMesh>> {
    let mut result = Vec::new();
    if scene["chair"].is_null() {
        return Ok(result);
    }
    for key in ["seat", "back"] {
        let spec = &scene["chair"][key];
        let center: [f32; 3] = serde_json::from_value(spec["center"].clone())?;
        let size: [f32; 3] = serde_json::from_value(spec["size"].clone())?;
        result.push(box_mesh(template, center, size));
        if key == "seat" {
            let h = center[2] - size[2] / 2.;
            for x in [-0.43, 0.43] {
                for y in [-0.43, 0.43] {
                    result.push(box_mesh(
                        template,
                        [center[0] + size[0] * x, center[1] + size[1] * y, h * 0.5],
                        [0.024, 0.024, h],
                    ));
                }
            }
        }
    }
    Ok(result)
}

struct Shape {
    channel: usize,
    mesh: usize,
    sign: f32,
    offsets: Vec<(usize, Vec3)>,
}
fn shapes(path: Option<&Path>, id: &Value, meshes: &[RenderMesh]) -> Result<Vec<Shape>> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    let s = json(path)?;
    if s["coordinates"] != "rest_armature_metres" || s["format"] != "projectghost.face-basis/1" {
        return Err("unknown host shape basis".into());
    }
    for k in ["present_id", "fit_id", "body_rev_id"] {
        if s[k] != id[k] {
            return Err(format!("shape basis {k} mismatch").into());
        }
    }
    let mut result = Vec::new();
    for (name, targets) in s["channels"].as_object().ok_or("missing shape channels")? {
        let channel = id["face"]
            .as_array()
            .ok_or("missing tape channels")?
            .iter()
            .position(|c| c.as_str() == Some(name))
            .ok_or("unknown shape channel")?;
        for t in targets.as_array().ok_or("invalid shape list")? {
            let mesh = meshes
                .iter()
                .position(|m| Some(m.id.as_str()) == t["part"].as_str())
                .ok_or("missing shape part")?;
            let count = meshes[mesh].positions.len();
            if t["vertex_count"] != count {
                return Err("shape topology mismatch".into());
            }
            let sign = t["sign"]
                .as_i64()
                .filter(|s| *s == 1 || *s == -1)
                .ok_or("invalid shape direction")? as f32;
            let mut offsets = Vec::new();
            for d in t["deltas"].as_array().ok_or("invalid shape offsets")? {
                let (i, x, y, z): (usize, f32, f32, f32) = serde_json::from_value(d.clone())?;
                let v = Vec3::new(x, y, z);
                if i >= count || !v.is_finite() {
                    return Err("bad shape vertex".into());
                }
                offsets.push((i, v));
            }
            result.push(Shape {
                channel,
                mesh,
                sign,
                offsets,
            });
        }
    }
    Ok(result)
}

fn render(
    comp: &Compiled,
    dir: &Path,
    output: &Path,
    mode: &str,
    dynamic: bool,
    basis: Option<&Path>,
) -> Result<()> {
    let header = fs::read(dir.join("generic.tape.json"))?;
    let id: Value = serde_json::from_slice(&header)?;
    let bin = fs::read(dir.join("generic.tape.bin"))?;
    let tape = TapeReader::new(&header, &bin)?;
    let scene = json(&dir.join("scene.json"))?;
    if id["body_rev_id"] != comp.body_rev_id().to_hex()
        || id["present_id"] != comp.present_id().to_hex()
        || id["fit_id"] != comp.fit_id().to_hex()
        || scene["tape_bin_sha256"] != id["bin_sha256"]
    {
        return Err("recipe/scene/tape identity mismatch".into());
    }
    let rest = comp.rest_render_meshes();
    let props = chair(&scene, &rest[0])?;
    let shapes = shapes(basis, &id, &rest)?;
    let mut track = if dynamic {
        Some(Track::open(dir, &id, &rest)?)
    } else {
        None
    };
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for m in &rest {
        for p in &m.positions {
            for (j, n) in [p.x, p.y, p.z].into_iter().enumerate() {
                lo[j] = lo[j].min(n);
                hi[j] = hi[j].max(n);
            }
        }
    }
    let mut rootlo = [0f32; 3];
    let mut roothi = [0f32; 3];
    let frames = tape.header().frame_count;
    for i in 0..frames {
        let p = tape.frame(i)?.root_t;
        for (j, n) in [p.x, p.y, p.z].into_iter().enumerate() {
            rootlo[j] = rootlo[j].min(n);
            roothi[j] = roothi[j].max(n);
        }
    }
    for j in 0..3 {
        lo[j] += rootlo[j];
        hi[j] += roothi[j];
    }
    for m in &props {
        for p in &m.positions {
            for (j, n) in [p.x, p.y, p.z].into_iter().enumerate() {
                lo[j] = lo[j].min(n);
                hi[j] = hi[j].max(n);
            }
        }
    }
    let mut opts = PreviewOptions::default();
    opts.width = 1280;
    opts.height = 720;
    opts.ssaa = 1;
    let fps = tape.header().fps;
    let fps_string = fps.to_string();
    let temp = output.with_extension("part.mp4");
    let mut process = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-y",
            "-v",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-s",
            "1280x720",
            "-r",
            &fps_string,
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-threads",
            "2",
            "-crf",
            "18",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(&temp)
        .stdin(Stdio::piped())
        .spawn()?;
    let mut pipe = process.stdin.take().ok_or("encoder pipe")?;
    let wide = frame_bounds_view((lo, hi), 16. / 9., 0.84, -14., 5.);
    let rec = (|| -> Result<()> {
        for i in 0..frames {
            let pose = comp.tape_frame(&tape, i)?;
            let mut meshes = comp.render_meshes(&pose)?;
            if let Some(t) = &mut track {
                t.read(&mut meshes)?;
            }
            if !shapes.is_empty() {
                let values = tape.face_channels(i)?;
                let mut sums = vec![Vec::new(); meshes.len()];
                for s in &shapes {
                    let w = (values[s.channel] * s.sign).clamp(0., 1.);
                    if w > 0. {
                        sums[s.mesh].extend(s.offsets.iter().map(|&(j, d)| (j, d.scale(w))));
                    }
                }
                for (m, offsets) in meshes.iter_mut().zip(sums) {
                    if !offsets.is_empty() {
                        comp.apply_rest_offsets(&pose, m, &offsets)?;
                    }
                }
            }
            let mut camera = wide;
            let close = mode == "cut"
                && id["camera_cuts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|c| c["frame"].as_u64().unwrap_or(u64::MAX) <= i as u64)
                    .next_back()
                    .is_some_and(|c| c["camera_id"] == "close");
            if close {
                let body = &meshes[0];
                let peak = body
                    .positions
                    .iter()
                    .min_by(|a, b| b.z.total_cmp(&a.z))
                    .ok_or("empty actor")?;
                let p = [peak.x, peak.y - 0.012, peak.z - 0.11];
                camera = frame_bounds_view(
                    (
                        [p[0] - 0.109, p[1] - 0.07, p[2] - 0.155],
                        [p[0] + 0.109, p[1] + 0.07, p[2] + 0.155],
                    ),
                    16. / 9.,
                    0.86,
                    -7.,
                    0.,
                );
            } else if mode == "orbit" {
                camera = frame_bounds_view(
                    (lo, hi),
                    16. / 9.,
                    0.84,
                    360. * i as f32 / frames as f32,
                    4.,
                );
            }
            meshes.extend_from_slice(&props);
            pipe.write_all(&comp.preview_meshes(&meshes, Some(&camera), opts).rgb8)?;
        }
        Ok(())
    })();
    drop(pipe);
    let status = process.wait()?;
    rec?;
    if !status.success() {
        return Err("video encoder failed".into());
    }
    fs::rename(temp, output)?;
    eprintln!("MOVIE frames={frames} fps={fps} mode={mode} physical={dynamic}");
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let [_, asset, preset, run, out, mode, source, tail @ ..] = args.as_slice() else {
        return Err(
            "usage: matter_movie ASSET PRESET RUN OUT.mp4 wide|cut|orbit kinematic|physical".into(),
        );
    };
    if !["wide", "cut", "orbit"].contains(&mode.as_str())
        || !["kinematic", "physical"].contains(&source.as_str())
    {
        return Err("unsupported camera/source".into());
    }
    let basis = match tail {
        [] => None,
        [flag, path] if flag == "--shape-basis" => Some(Path::new(path)),
        _ => return Err("expected --shape-basis PATH".into()),
    };
    let p = json(Path::new(preset))?;
    let style = recipe::movement_style(&p)?;
    let c = compile(
        &recipe::build_recipe(&p, &style)?,
        &AssetSet::load_dir(Path::new(asset))?,
    )?;
    render(
        &c,
        Path::new(run),
        Path::new(out),
        mode,
        source == "physical",
        basis,
    )
}
