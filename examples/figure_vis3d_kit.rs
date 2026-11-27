//! VIS-3D visual checkpoint kit for `gibson::figure` (M5-B). EXPERIMENTAL, example only.
//!
//! A deterministic software rasterizer plus a motion analyzer. Everything is synthetic: a
//! procedural humanoid [`BodyFamily`] (17 joints, tapered 8-sided prisms skinned with one or two
//! influences so joints bend smoothly, two morph targets, two controls), a fixed perspective
//! camera, z-buffered flat Lambert shading, 320x320 binary PPM (P6) output. No threads, no
//! clock, no I/O except the files written under the output directory.
//!
//! ```text
//! figure_vis3d_kit <out_dir> [stills|turntable|motion|analysis|all]
//! ```
//!
//! Output layout under `<out_dir>`:
//! `stills/*.ppm`, `turntable/t_NNN.ppm`, `motion/<scene>/f_NNN.ppm` (scenes `loop`, `preempt`,
//! `look`, 30 fps), `analysis/motion_analysis.json` and CSV files.
//!
//! The library has no `MotionPose -> PoseInput` conversion; [`pose_from_motion`] is that
//! conversion, kept here so the library stays generic.
//!
//! ANALYSIS (owner ruling 17: acceptance uses frame-timing and displacement metrics). For every
//! tracked point (joint origins plus landmarks) the world position `p(t)` is sampled. The
//! velocity-jump estimate at an instant is `J = |p(t+h) - 2 p(t) + p(t-h)| / h` with `h = 1 ms`
//! (units m/s). For a C1 path with bounded acceleration `A`, `J <= A h`; a velocity jump of size
//! `V` shows up as about `V`. See `docs/evidence/m5b/vis3d/README.md` for the bound used at the
//! preempt and retarget instants.
#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gibson::figure::clip::{Channel, ChannelTarget, Clip, Keys};
use gibson::figure::math::Transform;
use gibson::figure::math::{Quat, Vec3};
use gibson::figure::morph::MorphTarget;
use gibson::figure::motion::{
    Look, Motion, MotionClip, MotionPose, Playing, Preempt, Repeat, Sample,
};
use gibson::figure::revision::{
    compile, BodyFamily, BodyRecipe, CharacterRevision, ControlDef, ControlEffect, FamilyRef,
    NeutralMesh, PoseInput, PosedMesh,
};
use gibson::figure::skeleton::{Joint, Landmark};

// ---------------------------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------------------------

const W: usize = 320;
const H: usize = 320;
const FPS: u64 = 30;
const SCENE_MS: u64 = 3600;
const SCENE_FRAMES: usize = 108;
const TURNTABLE_FRAMES: usize = 72;
const SIDES: usize = 8;
const RINGS: usize = 4;
/// Half-width of the sub-step analysis window around an event, in ms.
const WINDOW_MS: i64 = 25;
/// Millisecond sub-step.
const H_S: f64 = 1.0e-3;
/// f32 unit roundoff.
const U: f64 = 5.960_464_477_539_063e-8;
/// Magnitude scale of world positions, metres, for the roundoff term.
const P_MAG: f64 = 3.0;
/// Declared slack for the nonlinearity of forward kinematics (the derived channel-space bound is
/// applied to world positions). Declared before any run; not tuned.
const SAFETY: f64 = 2.0;
const MOTION_YAW: f32 = 55.0;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

// ---------------------------------------------------------------------------------------------
// Humanoid family
// ---------------------------------------------------------------------------------------------

/// (name, parent, rest translation). Parent before child. Identity rest rotations.
const JOINTS: [(&str, i32, [f32; 3]); 17] = [
    ("pelvis", -1, [0.0, 1.0, 0.0]),
    ("spine", 0, [0.0, 0.12, 0.0]),
    ("chest", 1, [0.0, 0.25, 0.0]),
    ("neck", 2, [0.0, 0.22, 0.0]),
    ("head", 3, [0.0, 0.10, 0.0]),
    ("l_upper_arm", 2, [0.20, 0.15, 0.0]),
    ("l_lower_arm", 5, [0.0, -0.28, 0.0]),
    ("l_hand", 6, [0.0, -0.26, 0.0]),
    ("r_upper_arm", 2, [-0.20, 0.15, 0.0]),
    ("r_lower_arm", 8, [0.0, -0.28, 0.0]),
    ("r_hand", 9, [0.0, -0.26, 0.0]),
    ("l_upper_leg", 0, [0.09, -0.05, 0.0]),
    ("l_lower_leg", 11, [0.0, -0.42, 0.0]),
    ("l_foot", 12, [0.0, -0.42, 0.0]),
    ("r_upper_leg", 0, [-0.09, -0.05, 0.0]),
    ("r_lower_leg", 14, [0.0, -0.42, 0.0]),
    ("r_foot", 15, [0.0, -0.42, 0.0]),
];

struct Seg {
    joint: usize,
    start: [f32; 3],
    tip: [f32; 3],
    r0: (f32, f32),
    r1: (f32, f32),
    prev: Option<usize>,
    next: Option<usize>,
    color: [f32; 3],
}

fn segs() -> Vec<Seg> {
    let torso = [0.30, 0.45, 0.75];
    let skin = [0.90, 0.72, 0.58];
    let la = [0.85, 0.45, 0.35];
    let ra = [0.35, 0.65, 0.45];
    let ll = [0.40, 0.40, 0.62];
    let rl = [0.62, 0.55, 0.35];
    let shoe = [0.25, 0.25, 0.28];
    let z = [0.0, 0.0, 0.0];
    #[allow(clippy::too_many_arguments)]
    fn s(
        joint: usize,
        start: [f32; 3],
        tip: [f32; 3],
        r0: (f32, f32),
        r1: (f32, f32),
        prev: Option<usize>,
        next: Option<usize>,
        color: [f32; 3],
    ) -> Seg {
        Seg {
            joint,
            start,
            tip,
            r0,
            r1,
            prev,
            next,
            color,
        }
    }
    let arm = |j: usize, sign: f32, c: [f32; 3]| {
        let _ = sign;
        vec![
            s(
                j,
                [0.0, 0.03, 0.0],
                [0.0, -0.28, 0.0],
                (0.05, 0.05),
                (0.04, 0.04),
                Some(2),
                Some(j + 1),
                c,
            ),
            s(
                j + 1,
                [0.0; 3],
                [0.0, -0.26, 0.0],
                (0.04, 0.04),
                (0.032, 0.032),
                Some(j),
                Some(j + 2),
                c,
            ),
            s(
                j + 2,
                [0.0; 3],
                [0.0, -0.18, 0.0],
                (0.033, 0.02),
                (0.03, 0.015),
                Some(j + 1),
                None,
                skin,
            ),
        ]
    };
    let leg = |j: usize, c: [f32; 3]| {
        vec![
            s(
                j,
                [0.0; 3],
                [0.0, -0.42, 0.0],
                (0.075, 0.075),
                (0.055, 0.055),
                Some(0),
                Some(j + 1),
                c,
            ),
            s(
                j + 1,
                [0.0; 3],
                [0.0, -0.42, 0.0],
                (0.055, 0.055),
                (0.04, 0.04),
                Some(j),
                Some(j + 2),
                c,
            ),
            s(
                j + 2,
                [0.0, 0.0, -0.05],
                [0.0, -0.09, 0.17],
                (0.04, 0.04),
                (0.035, 0.03),
                Some(j + 1),
                None,
                shoe,
            ),
        ]
    };
    let mut v = vec![
        s(
            0,
            [0.0, -0.06, 0.0],
            [0.0, 0.12, 0.0],
            (0.15, 0.09),
            (0.14, 0.08),
            None,
            Some(1),
            torso,
        ),
        s(
            1,
            z,
            [0.0, 0.25, 0.0],
            (0.14, 0.085),
            (0.16, 0.09),
            Some(0),
            Some(2),
            torso,
        ),
        s(
            2,
            z,
            [0.0, 0.22, 0.0],
            (0.16, 0.09),
            (0.19, 0.095),
            Some(1),
            Some(3),
            torso,
        ),
        s(
            3,
            z,
            [0.0, 0.10, 0.0],
            (0.045, 0.045),
            (0.04, 0.04),
            Some(2),
            Some(4),
            skin,
        ),
        s(
            4,
            z,
            [0.0, 0.22, 0.0],
            (0.07, 0.08),
            (0.085, 0.095),
            Some(3),
            None,
            skin,
        ),
    ];
    v.extend(arm(5, 1.0, la));
    v.extend(arm(8, -1.0, ra));
    v.extend(leg(11, ll));
    v.extend(leg(14, rl));
    v
}

fn bind_world() -> Vec<Vec3> {
    let mut w: Vec<Vec3> = Vec::new();
    for (_, p, o) in JOINTS.iter() {
        let off = Vec3::new(o[0], o[1], o[2]);
        let base = if *p < 0 { Vec3::ZERO } else { w[*p as usize] };
        w.push(base.add(off));
    }
    w
}

fn basis(axis: Vec3) -> (Vec3, Vec3) {
    let a = axis.scale(1.0 / axis.length());
    let helper = if a.z.abs() > 0.7 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    let v = a.cross(helper);
    let v = v.scale(1.0 / v.length());
    let w = a.cross(v);
    (v, w)
}

fn ring_weights(seg: &Seg, k: usize) -> Vec<(usize, f32)> {
    let wa = [0.5f32, 0.85, 1.0, 0.85, 0.5][k];
    let other = if k <= 1 {
        seg.prev
    } else if k >= RINGS - 1 {
        seg.next
    } else {
        None
    };
    match other {
        Some(o) if wa < 1.0 => vec![(seg.joint, wa), (o, 1.0 - wa)],
        _ => vec![(seg.joint, 1.0)],
    }
}

/// The family plus a per-triangle base colour table (same order as the index buffer).
fn build_family() -> (BodyFamily, Vec<[f32; 3]>) {
    let world = bind_world();
    let segs = segs();
    let mut positions: Vec<Vec3> = Vec::new();
    let mut skin: Vec<Vec<(usize, f32)>> = Vec::new();
    let mut vseg: Vec<usize> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut tri_colors: Vec<[f32; 3]> = Vec::new();
    for (si, seg) in segs.iter().enumerate() {
        let base_idx = positions.len();
        let jp = world[seg.joint];
        let start = Vec3::new(seg.start[0], seg.start[1], seg.start[2]);
        let tip = Vec3::new(seg.tip[0], seg.tip[1], seg.tip[2]);
        let axis = tip.sub(start);
        let (bv, bw) = basis(axis);
        for k in 0..=RINGS {
            let s = k as f32 / RINGS as f32;
            let centre = jp.add(start).add(axis.scale(s));
            let rv = seg.r0.0 + (seg.r1.0 - seg.r0.0) * s;
            let rw = seg.r0.1 + (seg.r1.1 - seg.r0.1) * s;
            let wts = ring_weights(seg, k);
            for i in 0..SIDES {
                let ang = std::f32::consts::TAU * i as f32 / SIDES as f32;
                let p = centre
                    .add(bv.scale(rv * ang.cos()))
                    .add(bw.scale(rw * ang.sin()));
                positions.push(p);
                skin.push(wts.clone());
                vseg.push(si);
            }
        }
        let cap0 = positions.len();
        positions.push(jp.add(start));
        skin.push(ring_weights(seg, 0));
        vseg.push(si);
        let cap1 = positions.len();
        positions.push(jp.add(tip));
        skin.push(ring_weights(seg, RINGS));
        vseg.push(si);
        let ring = |k: usize, i: usize| (base_idx + k * SIDES + (i % SIDES)) as u32;
        for k in 0..RINGS {
            for i in 0..SIDES {
                let (a, b) = (ring(k, i), ring(k, i + 1));
                let (c, d) = (ring(k + 1, i), ring(k + 1, i + 1));
                indices.extend_from_slice(&[a, c, b, b, c, d]);
                tri_colors.push(seg.color);
                tri_colors.push(seg.color);
            }
        }
        for i in 0..SIDES {
            indices.extend_from_slice(&[cap0 as u32, ring(0, i + 1), ring(0, i)]);
            tri_colors.push(seg.color);
            indices.extend_from_slice(&[cap1 as u32, ring(RINGS, i), ring(RINGS, i + 1)]);
            tri_colors.push(seg.color);
        }
    }

    // Morph "broad": torso and pelvis widen, upper arms move out.
    let mut broad: Vec<(u32, Vec3)> = Vec::new();
    // Morph "head_big": the head grows about its centre.
    let mut head_big: Vec<(u32, Vec3)> = Vec::new();
    let head_centre = world[4].add(Vec3::new(0.0, 0.11, 0.0));
    for (vi, p) in positions.iter().enumerate() {
        let j = segs[vseg[vi]].joint;
        if j <= 2 {
            let d = Vec3::new(0.18 * p.x, 0.0, 0.15 * (p.z));
            broad.push((vi as u32, d));
        } else if j == 5 || j == 8 {
            let sx = if p.x >= 0.0 { 1.0 } else { -1.0 };
            broad.push((vi as u32, Vec3::new(0.03 * sx, 0.0, 0.0)));
        }
        if j == 4 {
            head_big.push((vi as u32, p.sub(head_centre).scale(0.25)));
        }
    }

    let joints: Vec<Joint> = JOINTS
        .iter()
        .map(|(n, p, o)| {
            Joint::new(
                *n,
                if *p < 0 { None } else { Some(*p as usize) },
                Transform::from_translation(Vec3::new(o[0], o[1], o[2])),
            )
        })
        .collect();
    let landmarks = vec![
        Landmark::new("head_top", 4, Vec3::new(0.0, 0.22, 0.0)),
        Landmark::new("l_hand_tip", 7, Vec3::new(0.0, -0.18, 0.0)),
        Landmark::new("r_hand_tip", 10, Vec3::new(0.0, -0.18, 0.0)),
        Landmark::new("l_toe", 13, Vec3::new(0.0, -0.09, 0.17)),
        Landmark::new("r_toe", 16, Vec3::new(0.0, -0.09, 0.17)),
    ];
    let family = BodyFamily::new(
        FamilyRef::new("synthetic.humanoid", 1),
        NeutralMesh::new(positions, indices),
        joints,
        landmarks,
        skin,
    )
    .with_morphs(vec![
        MorphTarget::new("broad", broad),
        MorphTarget::new("head_big", head_big),
    ])
    .with_controls(vec![
        ControlDef::new(
            "bulk",
            0.0,
            1.0,
            0.0,
            vec![ControlEffect::MorphWeight { target: 0 }],
        ),
        ControlDef::new(
            "head_size",
            0.0,
            1.0,
            0.0,
            vec![ControlEffect::MorphWeight { target: 1 }],
        ),
    ]);
    (family, tri_colors)
}

fn build_revision(family: &BodyFamily, values: Vec<(String, f32)>) -> CharacterRevision {
    let recipe = BodyRecipe::new(FamilyRef::new("synthetic.humanoid", 1), values);
    compile(&recipe, family).expect("the synthetic humanoid family must compile")
}

// ---------------------------------------------------------------------------------------------
// MotionPose -> PoseInput
// ---------------------------------------------------------------------------------------------

/// Joints named by a track are overridden (starting from the shaped rest local, so unnamed
/// components keep their rest value); morph weights become named weights.
fn pose_from_motion(rev: &CharacterRevision, mp: &MotionPose) -> PoseInput {
    let joints = rev.skeleton.joints();
    let mut locals = rev.skeleton.rest_locals();
    let mut touched = vec![false; locals.len()];
    let mut morphs: Vec<(String, f32)> = Vec::new();
    let find = |n: &str| joints.iter().position(|j| j.name == n);
    for (target, sample) in mp.tracks() {
        match (target, sample) {
            (ChannelTarget::JointTranslation(n), Sample::Vec3 { value, .. }) => {
                if let Some(j) = find(n) {
                    locals[j].translation = *value;
                    touched[j] = true;
                }
            }
            (ChannelTarget::JointRotation(n), Sample::Rotation { value, .. }) => {
                if let Some(j) = find(n) {
                    locals[j].rotation = *value;
                    touched[j] = true;
                }
            }
            (ChannelTarget::JointScale(n), Sample::Vec3 { value, .. }) => {
                if let Some(j) = find(n) {
                    locals[j].scale = *value;
                    touched[j] = true;
                }
            }
            (ChannelTarget::MorphWeight(n), Sample::Scalar { value, .. }) => {
                if let Some(e) = morphs.iter_mut().find(|(m, _)| m == n) {
                    e.1 = *value;
                } else {
                    morphs.push((n.clone(), *value));
                }
            }
            _ => {}
        }
    }
    let mut pose = PoseInput::new();
    for (j, t) in touched.iter().enumerate() {
        if *t {
            pose = pose.with_joint(j, locals[j]);
        }
    }
    for (n, w) in morphs {
        pose = pose.with_morph(n, w);
    }
    pose
}

fn eval_at(rev: &CharacterRevision, m: &dyn Motion, t: Duration) -> PosedMesh {
    let pose = pose_from_motion(rev, &m.sample(t));
    rev.evaluate(&pose).expect("a finite pose must evaluate")
}

// ---------------------------------------------------------------------------------------------
// Motions
// ---------------------------------------------------------------------------------------------

fn rot_chan(joint: &str, axis: Vec3, times: &[u64], deg: &[f32]) -> Channel {
    let keys = times
        .iter()
        .zip(deg.iter())
        .map(|(t, d)| (ms(*t), Quat::from_axis_angle(axis, d.to_radians())))
        .collect();
    Channel {
        target: ChannelTarget::JointRotation(joint.to_string()),
        keys: Keys::Rotation(keys),
    }
}

fn walk_clip() -> MotionClip {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    let t = [0u64, 300, 600, 900, 1200];
    let bob: Vec<(Duration, Vec3)> = [-0.02f32, 0.0, -0.02, 0.0, -0.02]
        .iter()
        .zip(t.iter())
        .map(|(b, tm)| (ms(*tm), Vec3::new(0.0, 1.0 + b, 0.0)))
        .collect();
    let channels = vec![
        Channel {
            target: ChannelTarget::JointTranslation("pelvis".into()),
            keys: Keys::Translation(bob),
        },
        rot_chan("spine", y, &t, &[8.0, 0.0, -8.0, 0.0, 8.0]),
        rot_chan("l_upper_arm", x, &t, &[35.0, 0.0, -35.0, 0.0, 35.0]),
        rot_chan("r_upper_arm", x, &t, &[-35.0, 0.0, 35.0, 0.0, -35.0]),
        rot_chan("l_lower_arm", x, &t, &[-45.0, -25.0, -5.0, -25.0, -45.0]),
        rot_chan("r_lower_arm", x, &t, &[-5.0, -25.0, -45.0, -25.0, -5.0]),
        rot_chan("l_upper_leg", x, &t, &[-28.0, 0.0, 28.0, 0.0, -28.0]),
        rot_chan("r_upper_leg", x, &t, &[28.0, 0.0, -28.0, 0.0, 28.0]),
        rot_chan("l_lower_leg", x, &t, &[5.0, 45.0, 10.0, 15.0, 5.0]),
        rot_chan("r_lower_leg", x, &t, &[10.0, 15.0, 5.0, 45.0, 10.0]),
    ];
    let clip = Clip::new("walkish", ms(1200), channels).expect("walk clip is valid");
    MotionClip::new(clip).with_repeat(Repeat::Loop)
}

fn wave_clip() -> MotionClip {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let t = [0u64, 500, 1000, 1500, 2000];
    let channels = vec![
        rot_chan("r_upper_arm", x, &t, &[0.0, -150.0, -140.0, -150.0, -145.0]),
        rot_chan("r_lower_arm", x, &t, &[0.0, -30.0, -70.0, -30.0, -70.0]),
        rot_chan("l_upper_arm", x, &t, &[0.0, -10.0, -15.0, -10.0, -12.0]),
    ];
    let clip = Clip::new("wave", ms(2000), channels).expect("wave clip is valid");
    MotionClip::new(clip).with_repeat(Repeat::Once)
}

/// The pelvis rest translation, so that targets only the old motion names decay to the rest
/// pose instead of to the origin.
fn rest_pose() -> MotionPose {
    MotionPose::new(vec![(
        ChannelTarget::JointTranslation("pelvis".into()),
        Sample::Vec3 {
            value: Vec3::new(0.0, 1.0, 0.0),
            rate: Vec3::ZERO,
        },
    )])
}

struct Overlay<A: Motion, B: Motion> {
    base: A,
    over: B,
}

impl<A: Motion, B: Motion> Motion for Overlay<A, B> {
    fn sample(&self, t: Duration) -> MotionPose {
        self.base.sample(t).overlay(&self.over.sample(t))
    }
}

/// CONTROL ONLY: a hard cut from `old` to `new` at `at`. Not a library motion; used to prove the
/// analyzer detects a discontinuity (it must FAIL the same bound).
struct Snap {
    old: Box<dyn Motion>,
    new: Box<dyn Motion>,
    at: Duration,
}

impl Motion for Snap {
    fn sample(&self, t: Duration) -> MotionPose {
        if t < self.at {
            self.old.sample(t)
        } else {
            self.new.sample(t)
        }
    }
}

const PREEMPT_AT_MS: u64 = 1450;
const PREEMPT_EASE_MS: u64 = 400;
const LOOK_EASE_MS: u64 = 350;
const FORWARD: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const LOOK_DIR_1: Vec3 = Vec3::new(0.8, 0.25, 0.55);
const LOOK_DIR_2: Vec3 = Vec3::new(-0.7, -0.2, 0.68);

fn walk_playing() -> Box<dyn Motion> {
    Box::new(Playing::new(walk_clip(), Duration::ZERO))
}

fn wave_playing() -> Box<dyn Motion> {
    Box::new(Playing::new(wave_clip(), ms(PREEMPT_AT_MS)))
}

fn look_const(dir: Vec3) -> Box<dyn Motion> {
    let look = Look::new("head", FORWARD, dir, ms(LOOK_EASE_MS)).expect("valid look");
    Box::new(Overlay {
        base: walk_playing(),
        over: look,
    })
}

enum Kind {
    Seam,
    Transition {
        ease: Duration,
        old: Box<dyn Motion>,
        new: Box<dyn Motion>,
    },
}

struct EventSpec {
    label: String,
    at_ms: i64,
    kind: Kind,
}

struct Scene {
    name: &'static str,
    motion: Box<dyn Motion>,
    events: Vec<EventSpec>,
    render: bool,
    expected_fail: bool,
}

fn transition(
    label: &str,
    at_ms: u64,
    ease_ms: u64,
    old: Box<dyn Motion>,
    new: Box<dyn Motion>,
) -> EventSpec {
    EventSpec {
        label: label.to_string(),
        at_ms: at_ms as i64,
        kind: Kind::Transition {
            ease: ms(ease_ms),
            old,
            new,
        },
    }
}

fn scenes() -> Vec<Scene> {
    let loop_scene = Scene {
        name: "loop",
        motion: walk_playing(),
        events: vec![
            EventSpec {
                label: "loop_seam_1200ms".into(),
                at_ms: 1200,
                kind: Kind::Seam,
            },
            EventSpec {
                label: "loop_seam_2400ms".into(),
                at_ms: 2400,
                kind: Kind::Seam,
            },
        ],
        render: true,
        expected_fail: false,
    };
    let preempt = Preempt::new(
        walk_playing(),
        wave_playing(),
        ms(PREEMPT_AT_MS),
        ms(PREEMPT_EASE_MS),
    )
    .with_rest(rest_pose());
    let preempt_scene = Scene {
        name: "preempt",
        motion: Box::new(preempt),
        events: vec![transition(
            "preempt_1450ms",
            PREEMPT_AT_MS,
            PREEMPT_EASE_MS,
            walk_playing(),
            wave_playing(),
        )],
        render: true,
        expected_fail: false,
    };
    let look = Look::new("head", FORWARD, FORWARD, ms(LOOK_EASE_MS))
        .expect("valid look")
        .retarget(ms(1450), LOOK_DIR_1)
        .expect("retarget 1")
        .retarget(ms(2450), LOOK_DIR_2)
        .expect("retarget 2");
    let look_scene = Scene {
        name: "look",
        motion: Box::new(Overlay {
            base: walk_playing(),
            over: look,
        }),
        events: vec![
            transition(
                "retarget_1450ms",
                1450,
                LOOK_EASE_MS,
                look_const(FORWARD),
                look_const(LOOK_DIR_1),
            ),
            transition(
                "retarget_2450ms",
                2450,
                LOOK_EASE_MS,
                look_const(LOOK_DIR_1),
                look_const(LOOK_DIR_2),
            ),
        ],
        render: true,
        expected_fail: false,
    };
    let control = Scene {
        name: "control_snap",
        motion: Box::new(Snap {
            old: walk_playing(),
            new: wave_playing(),
            at: ms(PREEMPT_AT_MS),
        }),
        events: vec![transition(
            "snap_1450ms",
            PREEMPT_AT_MS,
            PREEMPT_EASE_MS,
            walk_playing(),
            wave_playing(),
        )],
        render: false,
        expected_fail: true,
    };
    vec![loop_scene, preempt_scene, look_scene, control]
}

// ---------------------------------------------------------------------------------------------
// Rasterizer
// ---------------------------------------------------------------------------------------------

struct Cam {
    pos: Vec3,
    r: Vec3,
    u: Vec3,
    f: Vec3,
    tan_half: f32,
}

fn unit(v: Vec3) -> Vec3 {
    v.scale(1.0 / v.length())
}

fn camera(yaw_deg: f32, elev_deg: f32) -> Cam {
    let target = Vec3::new(0.0, 0.92, 0.0);
    let dist = 4.4f32;
    let yaw = yaw_deg.to_radians();
    let el = elev_deg.to_radians();
    let pos = target.add(Vec3::new(
        dist * yaw.sin() * el.cos(),
        dist * el.sin(),
        dist * yaw.cos() * el.cos(),
    ));
    let f = unit(target.sub(pos));
    let r = unit(f.cross(Vec3::new(0.0, 1.0, 0.0)));
    let u = r.cross(f);
    Cam {
        pos,
        r,
        u,
        f,
        tan_half: (32.0f32.to_radians() * 0.5).tan(),
    }
}

struct Canvas {
    rgb: Vec<u8>,
    depth: Vec<f32>,
}

impl Canvas {
    fn new() -> Canvas {
        let mut rgb = vec![0u8; W * H * 3];
        for y in 0..H {
            let t = y as f32 / (H - 1) as f32;
            let c = [
                0.80 + (0.93 - 0.80) * t,
                0.86 + (0.93 - 0.86) * t,
                0.93 + (0.90 - 0.93) * t,
            ];
            for x in 0..W {
                for k in 0..3 {
                    rgb[(y * W + x) * 3 + k] = (c[k] * 255.0 + 0.5) as u8;
                }
            }
        }
        Canvas {
            rgb,
            depth: vec![0.0; W * H],
        }
    }

    fn tri(&mut self, cam: &Cam, a: Vec3, b: Vec3, c: Vec3, color: [f32; 3]) {
        let n = (b.sub(a)).cross(c.sub(a));
        let nl = n.length();
        if nl < 1.0e-12 || !nl.is_finite() {
            return;
        }
        let mut n = n.scale(1.0 / nl);
        if n.dot(cam.pos.sub(a)) < 0.0 {
            n = n.neg();
        }
        let light = unit(
            cam.r
                .scale(-0.45)
                .add(cam.u.scale(0.65))
                .add(cam.f.scale(-0.65)),
        );
        let shade = 0.28 + 0.72 * n.dot(light).max(0.0);
        let mut sp = [(0.0f32, 0.0f32, 0.0f32); 3];
        for (i, p) in [a, b, c].iter().enumerate() {
            let d = p.sub(cam.pos);
            let zc = d.dot(cam.f);
            if zc < 0.2 {
                return;
            }
            let x = d.dot(cam.r) / (zc * cam.tan_half);
            let y = d.dot(cam.u) / (zc * cam.tan_half);
            sp[i] = (
                (x * 0.5 + 0.5) * W as f32,
                (0.5 - y * 0.5) * H as f32,
                1.0 / zc,
            );
        }
        let edge = |p: (f32, f32, f32), q: (f32, f32, f32), x: f32, y: f32| {
            (x - p.0) * (q.1 - p.1) - (y - p.1) * (q.0 - p.0)
        };
        let area = edge(sp[0], sp[1], sp[2].0, sp[2].1);
        if area.abs() < 1.0e-9 {
            return;
        }
        let minx = sp
            .iter()
            .map(|p| p.0)
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as usize;
        let maxx = (sp
            .iter()
            .map(|p| p.0)
            .fold(f32::MIN, f32::max)
            .ceil()
            .min(W as f32 - 1.0))
        .max(0.0) as usize;
        let miny = sp
            .iter()
            .map(|p| p.1)
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as usize;
        let maxy = (sp
            .iter()
            .map(|p| p.1)
            .fold(f32::MIN, f32::max)
            .ceil()
            .min(H as f32 - 1.0))
        .max(0.0) as usize;
        let rgb = [
            ((color[0] * shade * 255.0 + 0.5).min(255.0)) as u8,
            ((color[1] * shade * 255.0 + 0.5).min(255.0)) as u8,
            ((color[2] * shade * 255.0 + 0.5).min(255.0)) as u8,
        ];
        for py in miny..=maxy {
            for px in minx..=maxx {
                let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
                let l0 = edge(sp[1], sp[2], x, y) / area;
                let l1 = edge(sp[2], sp[0], x, y) / area;
                let l2 = edge(sp[0], sp[1], x, y) / area;
                if l0 < 0.0 || l1 < 0.0 || l2 < 0.0 {
                    continue;
                }
                let w = l0 * sp[0].2 + l1 * sp[1].2 + l2 * sp[2].2;
                let idx = py * W + px;
                if w > self.depth[idx] {
                    self.depth[idx] = w;
                    self.rgb[idx * 3..idx * 3 + 3].copy_from_slice(&rgb);
                }
            }
        }
    }

    fn ground(&mut self, cam: &Cam) {
        let n = 8i32;
        let tile = 0.5f32;
        for i in 0..n {
            for j in 0..n {
                let x0 = (i - n / 2) as f32 * tile;
                let z0 = (j - n / 2) as f32 * tile;
                let color = if (i + j) % 2 == 0 {
                    [0.55, 0.58, 0.62]
                } else {
                    [0.45, 0.48, 0.52]
                };
                let p = |dx: f32, dz: f32| Vec3::new(x0 + dx, 0.0, z0 + dz);
                self.tri(cam, p(0.0, 0.0), p(0.0, tile), p(tile, 0.0), color);
                self.tri(cam, p(tile, 0.0), p(0.0, tile), p(tile, tile), color);
            }
        }
    }
}

fn render(tri_colors: &[[f32; 3]], posed: &PosedMesh, cam: &Cam) -> Vec<u8> {
    let mut c = Canvas::new();
    c.ground(cam);
    for (t, tri) in posed.indices.chunks_exact(3).enumerate() {
        let p = |i: u32| posed.positions[i as usize];
        c.tri(cam, p(tri[0]), p(tri[1]), p(tri[2]), tri_colors[t]);
    }
    c.rgb
}

fn write_ppm(path: &Path, rgb: &[u8]) {
    let mut f = fs::File::create(path).expect("create ppm");
    write!(f, "P6\n{W} {H}\n255\n").expect("ppm header");
    f.write_all(rgb).expect("ppm data");
}

fn frame_time(k: usize) -> Duration {
    Duration::from_nanos(k as u64 * 1_000_000_000 / FPS)
}

// ---------------------------------------------------------------------------------------------
// Stages: stills, turntable, motion frames
// ---------------------------------------------------------------------------------------------

fn stage_stills(out: &Path, family: &BodyFamily, tri_colors: &[[f32; 3]]) {
    let dir = out.join("stills");
    fs::create_dir_all(&dir).expect("mkdir stills");
    let rev = build_revision(family, vec![]);
    let bind = rev.evaluate(&PoseInput::new()).expect("bind pose");
    for (name, yaw) in [
        ("front", 0.0f32),
        ("side", 90.0),
        ("three_quarter", 40.0),
        ("back", 180.0),
    ] {
        write_ppm(
            &dir.join(format!("{name}.ppm")),
            &render(tri_colors, &bind, &camera(yaw, 8.0)),
        );
    }
    let big = build_revision(
        family,
        vec![("bulk".to_string(), 1.0), ("head_size".to_string(), 1.0)],
    );
    let posed = big.evaluate(&PoseInput::new()).expect("morph pose");
    write_ppm(
        &dir.join("morph_front.ppm"),
        &render(tri_colors, &posed, &camera(25.0, 8.0)),
    );
    println!("stills: 5 written (front side three_quarter back morph_front)");
}

fn stage_turntable(out: &Path, family: &BodyFamily, tri_colors: &[[f32; 3]]) {
    let dir = out.join("turntable");
    fs::create_dir_all(&dir).expect("mkdir turntable");
    let rev = build_revision(family, vec![]);
    let bind = rev.evaluate(&PoseInput::new()).expect("bind pose");
    for k in 0..TURNTABLE_FRAMES {
        let yaw = 360.0 * k as f32 / TURNTABLE_FRAMES as f32;
        write_ppm(
            &dir.join(format!("t_{k:03}.ppm")),
            &render(tri_colors, &bind, &camera(yaw, 8.0)),
        );
    }
    println!("turntable: {TURNTABLE_FRAMES} frames written");
}

fn stage_motion(out: &Path, family: &BodyFamily, tri_colors: &[[f32; 3]]) {
    let rev = build_revision(family, vec![]);
    let cam = camera(MOTION_YAW, 8.0);
    for scene in scenes().iter().filter(|s| s.render) {
        let dir = out.join("motion").join(scene.name);
        fs::create_dir_all(&dir).expect("mkdir motion");
        for k in 0..SCENE_FRAMES {
            let posed = eval_at(&rev, &*scene.motion, frame_time(k));
            write_ppm(
                &dir.join(format!("f_{k:03}.ppm")),
                &render(tri_colors, &posed, &cam),
            );
        }
        println!("motion {}: {SCENE_FRAMES} frames at {FPS} fps", scene.name);
    }
}

// ---------------------------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------------------------

type P3 = [f64; 3];

fn p3(v: Vec3) -> P3 {
    [f64::from(v.x), f64::from(v.y), f64::from(v.z)]
}

fn sub3(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len3(a: P3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// `|a - 2 b + c|`.
fn second_diff(a: P3, b: P3, c: P3) -> f64 {
    len3([
        a[0] - 2.0 * b[0] + c[0],
        a[1] - 2.0 * b[1] + c[1],
        a[2] - 2.0 * b[2] + c[2],
    ])
}

fn point_names(rev: &CharacterRevision) -> Vec<String> {
    let mut v: Vec<String> = rev
        .skeleton
        .joints()
        .iter()
        .map(|j| j.name.clone())
        .collect();
    for l in rev.skeleton.landmarks() {
        v.push(l.name.clone());
    }
    v
}

fn points_of(rev: &CharacterRevision, pm: &PosedMesh) -> Vec<P3> {
    let mut v: Vec<P3> = pm
        .joint_world
        .iter()
        .map(|m| {
            [
                f64::from(m.cols[3][0]),
                f64::from(m.cols[3][1]),
                f64::from(m.cols[3][2]),
            ]
        })
        .collect();
    for i in 0..rev.skeleton.landmarks().len() {
        let p = rev
            .skeleton
            .landmark_world(i, &pm.joint_world)
            .expect("landmark joint exists");
        v.push(p3(p));
    }
    v
}

fn max_vertex_step(a: &[Vec3], b: &[Vec3]) -> (f64, usize) {
    let mut best = (0.0f64, 0usize);
    for (i, (p, q)) in a.iter().zip(b.iter()).enumerate() {
        let d = len3(sub3(p3(*p), p3(*q)));
        if d > best.0 {
            best = (d, i);
        }
    }
    best
}

fn arr(v: &[f64]) -> String {
    let mut s = String::from("[");
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{x:.4}");
    }
    s.push(']');
    s
}

fn nearest_event(events: &[EventSpec], t_ms: f64) -> (String, f64) {
    let mut best = (String::from("none"), f64::INFINITY);
    for e in events {
        let d = t_ms - e.at_ms as f64;
        if d.abs() < best.1.abs() || best.1.is_infinite() {
            best = (e.label.clone(), d);
        }
    }
    if best.1.is_infinite() {
        best.1 = 0.0;
    }
    best
}

struct EventOut {
    json: String,
    pass: bool,
}

#[allow(clippy::too_many_arguments)]
fn analyse_event(
    rev: &CharacterRevision,
    names: &[String],
    ev: &EventSpec,
    sweep: &[Vec<P3>],
    baseline_j: f64,
    motion: &dyn Motion,
    out_dir: &Path,
    scene: &str,
) -> EventOut {
    let at = ev.at_ms;
    let np = names.len();
    let j_at = |i: i64, p: usize| -> f64 {
        let i = i as usize;
        second_diff(sweep[i + 1][p], sweep[i][p], sweep[i - 1][p]) / H_S
    };
    // Window of J, and of the full-mesh step per millisecond.
    let mut csv = String::from("offset_ms,max_vertex_step_mm,max_j_m_s,max_j_point\n");
    let mut verts_prev: Option<Vec<Vec3>> = None;
    let mut max_step = (0.0f64, 0i64);
    let mut max_j = (0.0f64, 0i64, 0usize);
    for d in -WINDOW_MS..=WINDOW_MS {
        let t = Duration::from_micros(((at + d) * 1000) as u64);
        let pm = eval_at(rev, motion, t);
        let mut step_mm = 0.0;
        if let Some(prev) = &verts_prev {
            step_mm = max_vertex_step(&pm.positions, prev).0 * 1000.0;
            if step_mm > max_step.0 {
                max_step = (step_mm, d);
            }
        }
        verts_prev = Some(pm.positions);
        let (mut jm, mut jp) = (0.0, 0usize);
        if d > -WINDOW_MS && d < WINDOW_MS {
            for p in 0..np {
                let j = j_at(at + d, p);
                if j > jm {
                    jm = j;
                    jp = p;
                }
            }
            if jm > max_j.0 {
                max_j = (jm, d, jp);
            }
        }
        let _ = writeln!(csv, "{d},{step_mm:.6},{jm:.6},{}", names[jp]);
    }
    let _ = fs::write(
        out_dir.join(format!("substep_{scene}_{}.csv", ev.label)),
        csv,
    );

    // J at the instant itself, worst point, and the 1 ms / 2 ms scaling ratio there.
    let (mut j0, mut j0p) = (0.0f64, 0usize);
    for p in 0..np {
        let j = j_at(at, p);
        if j > j0 {
            j0 = j;
            j0p = p;
        }
    }
    let i = at as usize;
    let j2 = second_diff(sweep[i + 2][j0p], sweep[i][j0p], sweep[i - 2][j0p]) / (2.0 * H_S);
    let scaling = if j0 > 1.0e-9 { j2 / j0 } else { 0.0 };

    let round = 4.0 * U * P_MAG / H_S;
    let (kind_name, bound_desc, worst_ratio, bound_max) = match &ev.kind {
        Kind::Seam => {
            let bound = baseline_j + round;
            ("seam", "baseline_outside_events", max_j.0 / bound, bound)
        }
        Kind::Transition { ease, old, new } => {
            let t_s = ease.as_secs_f64();
            let lo = (at - 30).max(1);
            let hi = at + (ease.as_millis() as i64) + 1;
            let grid = |m: &dyn Motion| -> Vec<Vec<P3>> {
                (lo..=hi)
                    .map(|i| {
                        let pm = eval_at(rev, m, Duration::from_micros((i * 1000) as u64));
                        points_of(rev, &pm)
                    })
                    .collect()
            };
            let go = grid(&**old);
            let gn = grid(&**new);
            let n = go.len();
            let c = (at - lo) as usize;
            let mut worst = 0.0f64;
            let mut bmax = 0.0f64;
            for p in 0..np {
                let acc = |g: &Vec<Vec<P3>>| -> f64 {
                    (1..n - 1)
                        .map(|i| second_diff(g[i + 1][p], g[i][p], g[i - 1][p]) / (H_S * H_S))
                        .fold(0.0, f64::max)
                };
                let (a_old, a_new) = (acc(&go), acc(&gn));
                let x0 = len3(sub3(go[c][p], gn[c][p]));
                let vo = sub3(go[c + 1][p], go[c - 1][p]);
                let vn = sub3(gn[c + 1][p], gn[c - 1][p]);
                let v0 = len3(sub3(vo, vn)) / (2.0 * H_S);
                let a_bound = a_old + a_new + 5.78 * x0 / (t_s * t_s) + 4.0 * v0 / t_s;
                let bound = SAFETY * a_bound * H_S + round;
                bmax = bmax.max(bound);
                for d in (-WINDOW_MS + 1)..WINDOW_MS {
                    worst = worst.max(j_at(at + d, p) / bound);
                }
            }
            ("transition", "derived_c1_offset_bound", worst, bmax)
        }
    };
    let pass = worst_ratio <= 1.0;
    let mut j = String::new();
    let _ = write!(
        j,
        "{{\"label\":\"{}\",\"kind\":\"{}\",\"at_ms\":{},\"window_ms\":{},\"substep_ms\":1,\
\"max_vertex_step_mm_per_ms\":{:.6},\"max_vertex_step_offset_ms\":{},\
\"max_velocity_jump_m_s\":{:.6},\"max_velocity_jump_offset_ms\":{},\"max_velocity_jump_point\":\"{}\",\
\"velocity_jump_at_instant_m_s\":{:.6},\"instant_point\":\"{}\",\"scaling_ratio_2ms_over_1ms\":{:.4},\
\"bound_kind\":\"{}\",\"bound_max_over_points_m_s\":{:.6},\"worst_ratio_j_over_bound\":{:.6},\
\"baseline_outside_events_m_s\":{:.6},\"pass\":{}}}",
        ev.label,
        kind_name,
        at,
        WINDOW_MS,
        max_step.0,
        max_step.1,
        max_j.0,
        max_j.1,
        names[max_j.2],
        j0,
        names[j0p],
        scaling,
        bound_desc,
        bound_max,
        worst_ratio,
        baseline_j,
        pass
    );
    println!(
        "  event {:<18} jump@instant {:.5} m/s ({}), window max {:.5} m/s at {:+} ms, \
step {:.4} mm/ms, ratio J/bound {:.4}, scaling {:.2} -> {}",
        ev.label,
        j0,
        names[j0p],
        max_j.0,
        max_j.1,
        max_step.0,
        worst_ratio,
        scaling,
        if pass { "PASS" } else { "FAIL" }
    );
    EventOut { json: j, pass }
}

/// Returns the scene JSON and whether the scene did what it should (a normal scene passes; the
/// control scene is "detected" when its event FAILS).
fn analyse_scene(rev: &CharacterRevision, scene: &Scene, out_dir: &Path) -> (String, bool) {
    println!("scene {}:", scene.name);
    let names = point_names(rev);
    let np = names.len();
    let dt = 1.0 / FPS as f64;
    let motion: &dyn Motion = &*scene.motion;

    // 30 fps series.
    let mut verts: Vec<Vec<Vec3>> = Vec::new();
    let mut pts: Vec<Vec<P3>> = Vec::new();
    for k in 0..SCENE_FRAMES {
        let pm = eval_at(rev, motion, frame_time(k));
        pts.push(points_of(rev, &pm));
        verts.push(pm.positions);
    }
    let mut disp = vec![0.0f64; SCENE_FRAMES];
    let mut speed_max = vec![0.0f64; SCENE_FRAMES];
    let mut dv_max = vec![0.0f64; SCENE_FRAMES];
    let mut dv_pt = vec![0usize; SCENE_FRAMES];
    let mut pt_speed = vec![0.0f64; np];
    let mut pt_dv = vec![0.0f64; np];
    let mut pcsv = String::from("frame,t_s,point,speed_m_s,dv_m_s\n");
    let mut vel: Vec<Vec<P3>> = vec![vec![[0.0; 3]; np]; SCENE_FRAMES];
    for k in 1..SCENE_FRAMES {
        disp[k] = max_vertex_step(&verts[k], &verts[k - 1]).0;
        for p in 0..np {
            let d = sub3(pts[k][p], pts[k - 1][p]);
            vel[k][p] = [d[0] / dt, d[1] / dt, d[2] / dt];
            let sp = len3(vel[k][p]);
            speed_max[k] = speed_max[k].max(sp);
            pt_speed[p] = pt_speed[p].max(sp);
            let dv = if k >= 2 {
                len3(sub3(vel[k][p], vel[k - 1][p]))
            } else {
                0.0
            };
            if dv > dv_max[k] {
                dv_max[k] = dv;
                dv_pt[k] = p;
            }
            pt_dv[p] = pt_dv[p].max(dv);
            let _ = writeln!(
                pcsv,
                "{k},{:.6},{},{sp:.6},{dv:.6}",
                k as f64 * dt,
                names[p]
            );
        }
    }
    let mut fcsv =
        String::from("frame,t_s,max_vertex_disp_m,max_speed_m_s,max_dv_m_s,max_dv_point\n");
    for k in 0..SCENE_FRAMES {
        let _ = writeln!(
            fcsv,
            "{k},{:.6},{:.6},{:.6},{:.6},{}",
            k as f64 * dt,
            disp[k],
            speed_max[k],
            dv_max[k],
            names[dv_pt[k]]
        );
    }
    let _ = fs::write(out_dir.join(format!("frames_{}.csv", scene.name)), fcsv);
    let _ = fs::write(out_dir.join(format!("points_{}.csv", scene.name)), pcsv);
    let argmax = |v: &[f64]| -> usize {
        let mut b = 0;
        for (i, x) in v.iter().enumerate() {
            if *x > v[b] {
                b = i;
            }
        }
        b
    };
    let kd = argmax(&disp);
    let kv = argmax(&dv_max);
    let (ld, od) = nearest_event(&scene.events, kd as f64 * dt * 1000.0);
    let (lv, ov) = nearest_event(&scene.events, kv as f64 * dt * 1000.0);
    println!(
        "  30fps: max vertex displacement {:.5} m at frame {} ({:+.1} ms from {}); max dv {:.4} m/s at frame {} ({}, {:+.1} ms from {})",
        disp[kd], kd, od, ld, dv_max[kv], kv, names[dv_pt[kv]], ov, lv
    );

    // 1 ms sweep over the whole scene (joint and landmark positions only are retained).
    let sweep: Vec<Vec<P3>> = (0..=SCENE_MS)
        .map(|i| points_of(rev, &eval_at(rev, motion, ms(i))))
        .collect();
    let mut baseline = 0.0f64;
    let mut gmax = (0.0f64, 0usize, 0usize);
    for i in 1..SCENE_MS as usize {
        let excluded = scene
            .events
            .iter()
            .any(|e| (i as i64 - e.at_ms).abs() <= WINDOW_MS);
        let triples = sweep[i + 1].iter().zip(&sweep[i]).zip(&sweep[i - 1]);
        for (p, ((&next, &cur), &prev)) in triples.enumerate().take(np) {
            let j = second_diff(next, cur, prev) / H_S;
            if j > gmax.0 {
                gmax = (j, i, p);
            }
            if !excluded && j > baseline {
                baseline = j;
            }
        }
    }
    let (gl, go) = nearest_event(&scene.events, gmax.1 as f64);
    println!(
        "  1 ms sweep: global max velocity-jump estimate {:.5} m/s at {} ms ({}, {:+.0} ms from {}); baseline outside event windows {:.5} m/s",
        gmax.0, gmax.1, names[gmax.2], go, gl, baseline
    );

    let mut ev_json: Vec<String> = Vec::new();
    let mut all_pass = true;
    for ev in &scene.events {
        let r = analyse_event(
            rev, &names, ev, &sweep, baseline, motion, out_dir, scene.name,
        );
        all_pass &= r.pass;
        ev_json.push(r.json);
    }
    let ok = if scene.expected_fail {
        !all_pass
    } else {
        all_pass
    };
    let mut pts_json = Vec::new();
    for p in 0..np {
        pts_json.push(format!(
            "{{\"name\":\"{}\",\"max_speed_m_s\":{:.4},\"max_frame_dv_m_s\":{:.4}}}",
            names[p], pt_speed[p], pt_dv[p]
        ));
    }
    let mut j = String::new();
    let _ = write!(
        j,
        "{{\"name\":\"{}\",\"frames\":{},\"expected_fail_control\":{},\"max_vertex_disp_m\":{:.6},\
\"max_vertex_disp_frame\":{},\"max_vertex_disp_nearest_event\":\"{}\",\"max_vertex_disp_offset_ms\":{:.1},\
\"max_frame_dv_m_s\":{:.6},\"max_frame_dv_frame\":{},\"max_frame_dv_point\":\"{}\",\
\"max_frame_dv_nearest_event\":\"{}\",\"max_frame_dv_offset_ms\":{:.1},\
\"global_1ms_max_velocity_jump_m_s\":{:.6},\"global_1ms_max_at_ms\":{},\"global_1ms_max_point\":\"{}\",\
\"baseline_outside_events_m_s\":{:.6},\
\"per_frame_max_vertex_disp_m\":{},\"per_frame_max_dv_m_s\":{},\"points\":[{}],\"events\":[{}],\
\"scene_ok\":{}}}",
        scene.name,
        SCENE_FRAMES,
        scene.expected_fail,
        disp[kd],
        kd,
        ld,
        od,
        dv_max[kv],
        kv,
        names[dv_pt[kv]],
        lv,
        ov,
        gmax.0,
        gmax.1,
        names[gmax.2],
        baseline,
        arr(&disp),
        arr(&dv_max),
        pts_json.join(","),
        ev_json.join(","),
        ok
    );
    (j, ok)
}

fn stage_analysis(out: &Path, family: &BodyFamily) {
    let dir = out.join("analysis");
    fs::create_dir_all(&dir).expect("mkdir analysis");
    let rev = build_revision(family, vec![]);
    let mut scenes_json = Vec::new();
    let mut all_ok = true;
    for scene in scenes().iter() {
        let (j, ok) = analyse_scene(&rev, scene, &dir);
        all_ok &= ok;
        scenes_json.push(j);
    }
    let json = format!(
        "{{\"kit\":\"vis3d\",\"fps\":{FPS},\"scene_ms\":{SCENE_MS},\"window_ms\":{WINDOW_MS},\
\"substep_ms\":1,\"safety\":{SAFETY},\"rev_id\":\"{}\",\"scenes\":[{}],\"all_ok\":{}}}\n",
        rev.rev_id,
        scenes_json.join(",\n"),
        all_ok
    );
    fs::write(dir.join("motion_analysis.json"), json).expect("write json");
    println!(
        "analysis: all_ok={all_ok} (the control scene counts as ok only when its snap is DETECTED)"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(out) = args.get(1) else {
        eprintln!("usage: figure_vis3d_kit <out_dir> [stills|turntable|motion|analysis|all]");
        std::process::exit(2);
    };
    let out = PathBuf::from(out);
    let stage = args.get(2).map(String::as_str).unwrap_or("all");
    let (family, tri_colors) = build_family();
    let rev = build_revision(&family, vec![]);
    println!(
        "humanoid: {} joints, {} vertices, {} triangles, rev {}",
        rev.skeleton.joint_count(),
        rev.neutral.positions.len(),
        rev.neutral.indices.len() / 3,
        rev.rev_id
    );
    let all = stage == "all";
    if all || stage == "stills" {
        stage_stills(&out, &family, &tri_colors);
    }
    if all || stage == "turntable" {
        stage_turntable(&out, &family, &tri_colors);
    }
    if all || stage == "motion" {
        stage_motion(&out, &family, &tri_colors);
    }
    if all || stage == "analysis" {
        stage_analysis(&out, &family);
    }
}
