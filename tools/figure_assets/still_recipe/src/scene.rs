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
                        _ => return Err("expected idle or scene".into()),
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
                .ok_or("--performance idle|scene is required with physics options")?,
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
    express(script, &[("blink_l", 1.0), ("blink_r", 1.0)]);
    script.beats.push(Beat::Idle { secs: 0.16 });
    express(script, &[("blink_l", 0.0), ("blink_r", 0.0)]);
}

pub fn performance(
    rig: &Rig,
    style: Style,
    sequence: Sequence,
) -> Result<(PerformanceScript, Option<Vec3>)> {
    let pelvis = rig.rest_position(rig.map().pelvis);
    let mut script = PerformanceScript::new(style);
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

pub fn seated_chair(compiled: &Compiled, tape: &Tape, seat: Vec3) -> Result<(Value, f32)> {
    let frame = |f| TapeFrame::new(tape.root(f), tape.locals(f));
    let settled = compiled.pose_vertices(&frame(tape.frame_count - 1))?;
    // Use the lowest contact vertex in a posterior ischial patch, avoiding legs and soles.
    let top = settled
        .body
        .iter()
        .filter(|p| {
            (p.x - seat.x).abs() < 0.09
                && p.y >= seat.y
                && p.y < seat.y + 0.12
                && p.z > seat.z - 0.25
                && p.z < seat.z + 0.02
        })
        .map(|p| p.z)
        .reduce(f32::min)
        .ok_or("no seated posterior body vertices")?;
    let stop = tape
        .events
        .iter()
        .find(|e| e.kind == "stop")
        .ok_or("missing stop event")?
        .frame;
    let mut xmax = f32::NEG_INFINITY;
    for f in 0..=stop as usize {
        let mesh = compiled.pose_vertices(&frame(f))?;
        for p in mesh
            .body
            .iter()
            .chain(mesh.garments.iter().flat_map(|g| &g.positions))
        {
            if (p.y - seat.y).abs() < 0.28 && p.z > 0.0 && p.z < top + 0.48 {
                xmax = xmax.max(p.x);
            }
        }
    }
    let width = 0.43;
    let depth = 0.40;
    let thick = 0.035;
    if !xmax.is_finite() {
        return Err("empty walking corridor sample".into());
    }
    // Includes the back uprights and all four legs in the same horizontal envelope.
    let x = seat.x.max(xmax + 0.025 + width / 2.0);
    let min_pelvis_x = x - width / 2.0 + 0.085;
    Ok((
        json!({"pelvis_target":[seat.x,seat.y,seat.z],
        "walk_frames_measured":stop+1, "walk_to_chair_gap_m":x-width/2.0-xmax,
        "contact_top_m":top, "seat":{"center":[x,seat.y,top-thick/2.0],"size":[width,depth,thick]},
        "back":{"center":[x,seat.y+depth/2.0-0.0175,top+0.245],"size":[width,0.035,0.49]}}),
        min_pelvis_x,
    ))
}

pub fn write_setup(tape: &Tape, fit: &Value, out: &Path, chair: Option<Value>) -> Result<()> {
    let mut props = json!({"format":"gibson.figure.scene.v1","tape_bin_sha256":tape.bin_sha256(),
        "body_rev_id":fit["body_rev_id"],"fit_id":fit["fit_id"],"present_id":fit["present_id"],
        "seat":chair.as_ref().map(|c| &c["seat"]), "chair":chair});
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
