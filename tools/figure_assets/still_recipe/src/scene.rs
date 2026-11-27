//! Shot, chair and native beat definitions for an offline performance.
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use gibson::figure::living_matter::{DynamicsConfig, Fabric};
use gibson::figure::math::Vec3;
use gibson::figure::perform::{Beat, GestureKind, PerformanceScript, Rig, Style, Tape};
use gibson::figure::studio::{Compiled, TapeFrame};
use serde_json::{json, Value};

use super::Result;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sequence {
    Idle,
    Scene,
    Reference,
}

pub struct Options {
    pub sequence: Sequence,
    pub config: DynamicsConfig,
    pub kinematic_only: bool,
    pub preroll_frames: u32,
}

impl Options {
    pub fn parse(args: &[OsString]) -> Result<Option<Self>> {
        if args.is_empty() {
            return Ok(None);
        }
        let mut a = args.iter().map(|v| v.to_str().ok_or("non-UTF8 option"));
        let mut sequence = None;
        let mut config = DynamicsConfig::default();
        let mut kinematic_only = false;
        let mut preroll_frames = 30;
        while let Some(key) = a.next() {
            let key = key?;
            if key == "--kinematic-only" {
                kinematic_only = true;
                continue;
            }
            let v = a
                .next()
                .ok_or_else(|| format!("missing argument to {key}"))??;
            match key {
                "--performance" => {
                    sequence = Some(match v {
                        "idle" => Sequence::Idle,
                        "scene" => Sequence::Scene,
                        "reference" => Sequence::Reference,
                        _ => return Err("expected idle, scene or reference".into()),
                    })
                }
                "--fabric" => {
                    config.fabric = match v {
                        "light" => Fabric::Light,
                        "medium" => Fabric::Medium,
                        "structured" => Fabric::Structured,
                        _ => return Err("expected light, medium or structured".into()),
                    }
                }
                "--gravity" => {
                    config.gravity = match v {
                        "normal" => Vec3::new(0.0, 0.0, -9.80665),
                        "zero" => Vec3::ZERO,
                        "side" => Vec3::new(9.80665, 0.0, 0.0),
                        _ => return Err("expected normal, zero or side".into()),
                    }
                }
                "--support" => config.support = v.parse()?,
                "--firmness" => config.firmness = v.parse()?,
                "--damping" => config.damping = v.parse()?,
                "--intensity" => config.intensity = v.parse()?,
                "--preroll-frames" => preroll_frames = v.parse()?,
                _ => return Err(format!("unknown option {key}").into()),
            }
        }
        config.validate()?;
        Ok(Some(Self {
            sequence: sequence
                .ok_or("--performance idle|scene|reference is required with physics options")?,
            config,
            kinematic_only,
            preroll_frames,
        }))
    }
}

fn express(script: &mut PerformanceScript, values: &[(&str, f32)]) {
    script
        .beats
        .extend(values.iter().map(|(channel, value)| Beat::Express {
            channel: channel.to_string(),
            value: *value,
        }));
}

fn blink(script: &mut PerformanceScript) {
    for (value, ease_ms, hold) in [(1.0, 55, 0.14), (0.0, 80, 0.10)] {
        for eye in ["blink_l", "blink_r"] {
            script.beats.push(Beat::ExpressTimed {
                channel: eye.into(),
                value,
                ease_ms,
            });
        }
        script.beats.push(Beat::Idle { secs: hold });
    }
}

pub fn reference_rig(rig: Rig, compiled: &Compiled) -> Result<Rig> {
    let pelvis = rig.rest_position(rig.map().pelvis);
    let mut wrists = [Vec3::ZERO; 2];
    for (i, &chain) in rig.map().arms.iter().enumerate() {
        let [shoulder, elbow, wrist] = chain.map(|j| rig.rest_position(j));
        let length = shoulder.sub(elbow).length() + elbow.sub(wrist).length();
        let sign = (shoulder.x - pelvis.x).signum();
        let z = shoulder.z - 0.89 * length;
        let side = compiled.shaped_positions()[..compiled.body_vertex_count()]
            .iter()
            .filter(|p| {
                (p.z - z).abs() < 0.045
                    && (p.y - pelvis.y).abs() < 0.12
                    && (p.x - pelvis.x).abs() < (shoulder.x - pelvis.x).abs() + 0.065
            })
            .map(|p| sign * (p.x - pelvis.x))
            .fold(0.0_f32, f32::max);
        wrists[i] = Vec3::new(pelvis.x + sign * (side + 0.032), shoulder.y - 0.035, z);
    }
    println!(
        "reference_wrist_height_m={:.4},{:.4} reference_wrist_span_m={:.4}",
        wrists[0].z,
        wrists[1].z,
        (wrists[0].x - wrists[1].x).abs()
    );
    Ok(rig.with_rest_wrist_targets(wrists)?)
}

pub fn performance(
    rig: &Rig,
    style: Style,
    sequence: Sequence,
) -> Result<(PerformanceScript, Option<Vec3>)> {
    let pelvis = rig.rest_position(rig.map().pelvis);
    let mut script = PerformanceScript::new(style);
    if sequence == Sequence::Reference {
        return Ok((script.with(Beat::Idle { secs: 0.25 }), None));
    }
    express(
        &mut script,
        &[
            ("blink_l", 0.0),
            ("blink_r", 0.0),
            ("smile", 0.0),
            ("gaze_yaw", 0.0),
            ("gaze_pitch", 0.0),
            ("brow_raise_l", 0.0),
            ("brow_raise_r", 0.0),
            ("jaw_open", 0.0),
        ],
    );
    if sequence == Sequence::Idle {
        script.beats.push(Beat::Idle { secs: 0.7 });
        blink(&mut script);
        express(
            &mut script,
            &[("gaze_yaw", 0.21), ("gaze_pitch", 0.09), ("smile", 0.18)],
        );
        script.beats.push(Beat::Idle { secs: 1.2 });
        express(
            &mut script,
            &[("gaze_yaw", 0.0), ("gaze_pitch", 0.0), ("smile", 0.0)],
        );
        script.beats.push(Beat::Idle { secs: 1.2 });
        return Ok((script, None));
    }
    // Walk alongside the chair, then sit with the pelvis displaced from the walking corridor.
    // The cooker places the near edge from the posed wardrobe's measured walk envelope.
    let destination = Vec3::new(0.0, -0.70, 0.0);
    let seat = Vec3::new(
        pelvis.x + 0.43,
        pelvis.y + destination.y + 0.17,
        pelvis.z * 0.56,
    );
    script.beats.push(Beat::Idle { secs: 0.65 });
    script.beats.push(Beat::WalkTo {
        target: destination,
        stride: 0.29,
        speed: 0.54,
    });
    script.beats.push(Beat::Stop);
    express(
        &mut script,
        &[
            ("gaze_yaw", 0.32),
            ("gaze_pitch", 0.11),
            ("brow_raise_l", 0.26),
            ("brow_raise_r", 0.22),
        ],
    );
    script.beats.push(Beat::LookAt {
        target: Vec3::new(-0.65, -2.0, pelvis.z + 0.64),
    });
    blink(&mut script);
    express(&mut script, &[("smile", 0.48), ("jaw_open", 0.18)]);
    script.beats.push(Beat::Gesture {
        kind: GestureKind::Acknowledge,
        intensity: 0.85,
    });
    express(
        &mut script,
        &[
            ("jaw_open", 0.0),
            ("brow_raise_l", 0.0),
            ("brow_raise_r", 0.0),
        ],
    );
    script.beats.push(Beat::Sit {
        seat_height: seat.z,
        seat_pos: seat,
    });
    express(
        &mut script,
        &[("smile", 0.14), ("gaze_yaw", 0.0), ("gaze_pitch", 0.0)],
    );
    script.beats.push(Beat::Idle { secs: 2.0 });
    Ok((script, Some(seat)))
}

pub fn cuts(tape: &Tape) -> Vec<(u32, String)> {
    let mut cuts = vec![(0, "wide".to_string())];
    for e in &tape.events {
        match e.kind.as_str() {
            "look" => cuts.push((e.frame, "close".into())),
            "sit" => cuts.push((e.frame, "wide".into())),
            _ => {}
        }
    }
    cuts
}

pub fn verify_blink(tape: &Tape) -> Result<()> {
    let face_start = tape.stride() - tape.face.len();
    for eye in ["blink_l", "blink_r"] {
        let index = face_start
            + tape
                .face
                .iter()
                .position(|s| s == eye)
                .ok_or("missing eyelid channel")?;
        let samples: Vec<_> = (0..tape.frame_count)
            .map(|i| tape.frame(i)[index])
            .collect();
        let (peak_frame, peak) = samples
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap();
        let first = samples
            .iter()
            .position(|&v| v > 0.05)
            .ok_or("blink never started")?;
        let reopened = samples[peak_frame..]
            .iter()
            .position(|&v| v < 0.05)
            .ok_or("eyelid stayed closed")?
            + peak_frame;
        println!(
            "native_blink eye={eye} peak={peak:.4} peak_frame={peak_frame} active_ms={:.1}",
            (reopened - first) as f64 * 1000.0 / f64::from(tape.fps)
        );
        if *peak < 0.9 || (reopened - first) as f64 / f64::from(tape.fps) > 0.25 {
            return Err("native eyelid did not complete the authored close/open interval".into());
        }
    }
    Ok(())
}

pub fn seated_chair(compiled: &Compiled, tape: &Tape, seat: Vec3) -> Result<(Value, f32)> {
    let frame = |f| TapeFrame::new(tape.root(f), tape.locals(f));
    let settled = compiled.pose_vertices(&frame(tape.frame_count - 1))?;
    // Use the lowest contact vertex in a posterior ischial patch, avoiding legs and soles.
    let (contact_vertex, contact) = settled
        .body
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            (p.x - seat.x).abs() < 0.08
                && p.y >= seat.y
                && p.y < seat.y + 0.12
                && p.z > seat.z - 0.25
                && p.z < seat.z + 0.02
        })
        .min_by(|(_, a), (_, b)| a.z.total_cmp(&b.z))
        .ok_or("no seated posterior body vertices")?;
    let top = contact.z;
    let stop = tape
        .events
        .iter()
        .find(|e| e.kind == "stop")
        .ok_or("missing stop event")?
        .frame;
    let width = 0.43;
    let depth = 0.40;
    let thick = 0.035;
    let back_width = 0.26;
    let back_height = 0.33;
    let back_offset = 0.07;
    let margin = 0.025;
    let mut seat_max = f32::NEG_INFINITY;
    let mut back_max = f32::NEG_INFINITY;
    let back_y = seat.y + depth / 2.0 - 0.0175;
    let check = |p: &[Vec3], indices: &[u32], seat_max: &mut f32, back_max: &mut f32| {
        // A triangle must overlap both slab intervals before its maximum X can restrict
        // the chair. Using its entire X extent overestimates possible collision.
        for tri in indices.chunks_exact(3) {
            let [a, b, c] = [p[tri[0] as usize], p[tri[1] as usize], p[tri[2] as usize]];
            let ymin = a.y.min(b.y).min(c.y);
            let ymax = a.y.max(b.y).max(c.y);
            let zmin = a.z.min(b.z).min(c.z);
            let zmax = a.z.max(b.z).max(c.z);
            let xmax = a.x.max(b.x).max(c.x);
            if ymin <= seat.y + depth / 2.0
                && ymax >= seat.y - depth / 2.0
                && zmin <= top
                && zmax >= 0.0
            {
                *seat_max = seat_max.max(xmax);
            }
            if ymin <= back_y + 0.0175
                && ymax >= back_y - 0.0175
                && zmin <= top + back_height
                && zmax >= top
            {
                *back_max = back_max.max(xmax);
            }
        }
    };
    for f in 0..=stop as usize {
        let mesh = compiled.pose_vertices(&frame(f))?;
        check(&mesh.body, compiled.indices(), &mut seat_max, &mut back_max);
        for (g, source) in mesh.garments.iter().zip(compiled.drawable_garments()) {
            check(&g.positions, &source.indices, &mut seat_max, &mut back_max);
        }
    }
    if !seat_max.is_finite() {
        return Err("empty walking corridor sample".into());
    }
    let x = seat
        .x
        .max(seat_max + margin + width / 2.0)
        .max(back_max + margin + back_width / 2.0 - back_offset);
    let gap = (x - width / 2.0 - seat_max).min(x + back_offset - back_width / 2.0 - back_max);
    let min_pelvis_x = x - width / 2.0 + 0.085;
    Ok((
        json!({"pelvis_target":[seat.x,seat.y,seat.z],
        "walk_frames_measured":stop+1, "walk_to_chair_gap_m":gap,
        "contact_body_vertex":contact_vertex, "contact_top_m":top,
        "seat":{"center":[x,seat.y,top-thick/2.0],"size":[width,depth,thick]},
        "back":{"center":[x+back_offset,back_y,top+back_height/2.0],"size":[back_width,0.035,back_height]}}),
        min_pelvis_x,
    ))
}

pub fn write_setup(tape: &Tape, fit: &Value, out: &Path, chair: Option<Value>) -> Result<()> {
    let mut props = json!({"format":"gibson.figure.scene.v1","tape_bin_sha256":tape.bin_sha256(),
        "body_rev_id":fit["body_rev_id"],"fit_id":fit["fit_id"],"present_id":fit["present_id"],
        "seat":chair.as_ref().map(|c| &c["seat"]), "chair":chair});
    props["basis"] = json!("world-z-up-m");
    props["clock"] = json!({"frames_per_second":tape.fps,"frames":tape.frame_count});
    props["actors"] = json!([{"id":"actor-01","present_id":fit["present_id"],"root_track":"generic.tape.bin","origin":[0,0,0]}]);
    props["ground"] = json!({"id":"stage-floor","normal":[0,0,1],"offset":0});
    props["camera_cuts"] = json!(cuts(tape));
    if let Some(target) = props["chair"]["pelvis_target"][2].as_f64() {
        props["title"] = json!("THE GODDESS AWAKENS");
        props["targets"] = json!([{"id":"attention-01","position":[-0.65,-2.0,target/0.56+0.64]},
            {"id":"seat-contact-01","prop":"chair-01","position":props["chair"]["pelvis_target"]}]);
        props["chair"]["id"] = json!("chair-01");
    }
    if props["chair"].is_null() {
        props.as_object_mut().unwrap().remove("seat");
    }
    fs::write(out.join("scene.json"), serde_json::to_vec_pretty(&props)?)?;
    let shots = json!({"id":"living_performance", "reference_frame":0, "still_res":[1920,1080],
        "stills":[{"name":"front","camera":{"kind":"front"},"tape_frame":0},
            {"name":"face_closeup","camera":{"kind":"face_closeup"},"tape_frame":0}],
        "clips":[{"name":"performance","res":[1280,720],"fps":tape.fps,"frames":tape.frame_count,
            "use_tape_camera_cuts":true,"default_camera":"wide","cameras":{
                "wide":{"kind":"three_quarter","az":-14,"fill":0.70},
                "close":{"kind":"face_closeup","az":-9,"fill":0.65}}}]});
    fs::write(out.join("shots.json"), serde_json::to_vec_pretty(&shots)?)?;
    Ok(())
}
