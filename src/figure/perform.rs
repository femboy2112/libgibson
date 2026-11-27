//! Procedural performance: scripts, root-motion walking with world-locked feet, gaze, wave,
//! sit/stand, a deterministic tape, and an incremental `Live` evaluator. EXPERIMENTAL.
//!
//! Owner ruling 17 applies to everything here: motion is a pure function of explicit time with
//! `C1` velocity, and every transition is blended, never snapped.
//!
//! # Architecture
//!
//! A [`PerformanceScript`] is compiled beat by beat into *driver plans*. A driver is one scalar
//! of a small fixed vector (root position, heading, pelvis offset and tilt, spine bend, gaze,
//! foot targets, arm targets, ...; see the `D_*` constants). Each beat is a closed-form function
//! of beat-local time that returns the whole driver vector. Consecutive beats are joined with the
//! existing C-1 [`Preempt`] (quintic inertialization from the *current value and velocity*) over
//! the style's settle ease, so a beat that is cut short, or a beat that starts from a different
//! pose than the previous one ended in, never snaps. Face channels are a separate [`Preempt`]
//! chain of constant targets.
//!
//! The drivers are mapped to joint locals by one solver ([`Rig::solve`]): the pelvis and spine are
//! posed by world-space deltas, feet and hands by analytic two-bone IK ([`super::ik`]) with a pole
//! vector, so the solver works for any rest orientation. Joint roles are data ([`RigRoles`]); the
//! library never learns a body.
//!
//! # Walking without a treadmill
//!
//! `WalkTo` moves the *root* along the path with a smooth distance profile (velocity ramps up
//! over one step period, cruises, ramps down over one step period) and plans footfalls in world
//! space: each foot is planted (target velocity exactly zero) from landing to lift-off and flies
//! on a smootherstep path with a `sin^2` lift in between, so the foot target is `C2` at contact.
//! A planted foot is held by two-bone IK, so the foot does not slide while the pelvis passes over
//! it. The walk ends planted with both feet at their rest offsets from the root.
//! [`Performance::stance_windows`] reports when each foot is planted.
//!
//! One *stride* in a `WalkTo` is the root advance per step (the distance between successive
//! footprints), so the root travels `stride * steps`; the planner rounds the step count and
//! adjusts the step length so the path length is covered exactly.
//!
//! Zero dependencies, scalar `f32` in the solver, `f64` in the plans, no `unsafe`, no clock.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use std::f64::consts::PI;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use super::clip::ChannelTarget;
use super::ik;
use super::math::{Mat4, Quat, Transform, Vec3};
use super::motion::{Motion, MotionPose, Preempt, Sample};
use super::sha256;
use super::skeleton::Skeleton;

/// Most beats in one script.
pub const MAX_BEATS: usize = 4096;
/// Longest performance accepted, in seconds.
pub const MAX_SECONDS: f64 = 600.0;
/// Most frames one tape holds.
pub const MAX_FRAMES: usize = 200_000;
/// Most intents a [`Live`] holds queued.
pub const MAX_QUEUED: usize = 64;
/// Amplitude cap of reduced motion.
pub const REDUCED_AMPLITUDE: f32 = 0.25;

/// Reach ratio the planner designs the standing and walking pelvis height for (the soft limit of
/// the IK starts at 0.98, so planted feet stay exactly on their targets).
const DESIGN_RATIO: f64 = 0.95;
const MIN_WALK: f64 = 0.05;
/// How far in front of the head-centre plane the waving hand is held (metres, heading frame).
///
/// The hand target of a wave is the straight blend `hang + (wave - hang) w`, so on the way up
/// and down (and through every cut blend, where `w` decays while the inherited wave oscillation
/// still pulls the hand toward the head) it passes the shoulder at this forward distance. The
/// two-bone IK clamps a target nearer than the fold limit (`|l1 - l2| + 0.05 (l1 + l2)`, 0.059 m
/// on the synthetic arm) to that sphere: continuous, but a velocity step. At the former 0.05 m the
/// blend dipped to 0.041 m from the shoulder and the hand jumped by 1.2 m/s of velocity
/// (`docs/evidence/m5c/model_perform_cut.py`). The forward offset is not touched by the lateral
/// wave oscillation, so it keeps the chord clear of the fold for every cut phase.
const WAVE_FORWARD: f64 = 0.12;
/// How far beyond the hand clearance the waving hand is held to the side of the head centre
/// (metres, heading frame), before the wave oscillation.
///
/// Clearance is a safety net, not a path: deflecting a hand that moves at walking-arm speed
/// toward the head within a few centimetres takes more acceleration than the C-1 bound allows,
/// hard or soft. At the former 0.07 m an expressive wave cut at an unlucky phase drove the hand
/// onto the sphere (J = 0.24 hard, 0.19 with the soft floor); at 0.12 m no cut of a wave of
/// intensity 1 in either preset reaches the soft band (closest 0.240 m).
const WAVE_LATERAL: f64 = 0.12;
/// Width of the soft band of the hand clearance (metres); see [`push_out`].
const CLEARANCE_SOFT: f32 = 0.03;

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

/// Why a rig, a script or an intent was rejected.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PerformError {
    /// A role names a joint the skeleton does not have.
    UnknownJoint(String),
    /// A limb's three joints are not a parent-child chain.
    BadLimb {
        /// Which limb (`arm_l`, `arm_r`, `leg_l`, `leg_r`).
        limb: &'static str,
    },
    /// A joint has a scale other than one; the solver does not support it.
    NonUnitScale {
        /// The joint index.
        joint: usize,
    },
    /// The up or forward vector of the rig is zero, non-finite or parallel.
    BadBasis,
    /// A beat is invalid.
    BadBeat {
        /// The beat index (0 for an intent).
        beat: usize,
        /// What is wrong.
        what: &'static str,
    },
    /// A posture rule was broken (walk while seated, sit while seated).
    Posture(&'static str),
    /// The seat is out of reach of the planted feet.
    SeatUnreachable {
        /// Requested hip-to-ankle distance over leg length.
        ratio: f32,
    },
    /// Only non-walk beats can be cut.
    CannotCutWalk {
        /// The beat index.
        beat: usize,
    },
    /// A hard limit was exceeded.
    LimitExceeded {
        /// What.
        what: &'static str,
    },
    /// The frame rate is outside `1..=240`.
    BadFps,
    /// The live queue is full.
    QueueFull,
    /// A tape value was NaN or infinite (a tape reader refuses such a body).
    NonFinite,
}

impl fmt::Display for PerformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PerformError::UnknownJoint(n) => write!(f, "unknown joint `{n}`"),
            PerformError::BadLimb { limb } => write!(f, "{limb} is not a parent-child chain"),
            PerformError::NonUnitScale { joint } => write!(f, "joint {joint} has a non-unit scale"),
            PerformError::BadBasis => write!(f, "bad up/forward basis"),
            PerformError::BadBeat { beat, what } => write!(f, "beat {beat}: {what}"),
            PerformError::Posture(w) => write!(f, "posture: {w}"),
            PerformError::SeatUnreachable { ratio } => {
                write!(f, "seat out of reach of the planted feet (ratio {ratio})")
            }
            PerformError::CannotCutWalk { beat } => write!(f, "beat {beat}: a walk cannot be cut"),
            PerformError::LimitExceeded { what } => write!(f, "limit exceeded: {what}"),
            PerformError::BadFps => write!(f, "fps must be in 1..=240"),
            PerformError::QueueFull => write!(f, "live queue is full"),
            PerformError::NonFinite => write!(f, "the performance produced a non-finite value"),
        }
    }
}

impl std::error::Error for PerformError {}

// ---------------------------------------------------------------------------------------------
// Small f64 vector helpers (plans are f64; the solver is f32)
// ---------------------------------------------------------------------------------------------

type V = [f64; 3];

fn vadd(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn vsub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn vscale(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn vdot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn vcross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn vlen(a: V) -> f64 {
    vdot(a, a).sqrt()
}
fn v64(v: Vec3) -> V {
    [f64::from(v.x), f64::from(v.y), f64::from(v.z)]
}
fn v32(v: V) -> Vec3 {
    Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32)
}
/// Rotate `v` about the unit axis `k` by `ang` (Rodrigues).
fn vrot(v: V, k: V, ang: f64) -> V {
    let (s, c) = ang.sin_cos();
    let kv = vcross(k, v);
    let kd = vdot(k, v) * (1.0 - c);
    [
        v[0] * c + kv[0] * s + k[0] * kd,
        v[1] * c + kv[1] * s + k[1] * kd,
        v[2] * c + kv[2] * s + k[2] * kd,
    ]
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
/// Smootherstep, `C2` at both ends.
fn smoother(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}
/// Smootherstep from `t0` over `d` seconds.
fn ramp(t: f64, t0: f64, d: f64) -> f64 {
    if d > 0.0 {
        smoother((t - t0) / d)
    } else if t >= t0 {
        1.0
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------------------------
// Style, options, script
// ---------------------------------------------------------------------------------------------

/// How a performance is played: the amplitude of every gesture, the tempo, and the settle ease
/// (the blend time of every beat transition).
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// A name for the tape header (`restrained`, `expressive`, ...).
    pub preset: String,
    /// Amplitude multiplier of gestures, swing, sway and lift; about 0.2 to 1.5.
    pub amplitude: f32,
    /// Tempo multiplier of walking speed, wave frequency and beat durations.
    pub tempo: f32,
    /// Settle ease in milliseconds (the blend duration of every transition).
    pub settle_ms: u32,
}

impl Style {
    /// Low amplitude, slower tempo, a long settle.
    pub fn restrained() -> Style {
        Style {
            preset: "restrained".into(),
            amplitude: 0.45,
            tempo: 0.85,
            settle_ms: 450,
        }
    }

    /// Full amplitude, brisker tempo, a short settle.
    pub fn expressive() -> Style {
        Style {
            preset: "expressive".into(),
            amplitude: 1.0,
            tempo: 1.15,
            settle_ms: 280,
        }
    }

    /// This style under reduced motion: amplitude clamped to [`REDUCED_AMPLITUDE`] and a settle of
    /// at least 300 ms.
    pub fn reduced(&self) -> Style {
        Style {
            preset: self.preset.clone(),
            amplitude: self.amplitude.min(REDUCED_AMPLITUDE),
            tempo: self.tempo,
            settle_ms: self.settle_ms.max(300),
        }
    }

    fn settle_secs(&self) -> f64 {
        f64::from(self.settle_ms) / 1000.0
    }
}

/// Switches. The default is the shipping behaviour; each `false` is the *control* of one
/// guarantee, used by tests to prove the guarantee is not vacuous.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerformOptions {
    /// Hold planted feet with IK. `false`: the feet follow the pelvis (control).
    pub foot_lock: bool,
    /// Join beats with inertialization. `false`: a beat switches at once (control).
    pub inertialize: bool,
    /// Translate the root. `false`: the figure walks in place (control).
    pub root_motion: bool,
    /// Keep the hand out of the head sphere.
    pub arm_clearance: bool,
    /// Reduced motion: amplitude clamp and locomotion as a cross-dissolve.
    pub reduced_motion: bool,
}

impl Default for PerformOptions {
    fn default() -> PerformOptions {
        PerformOptions {
            foot_lock: true,
            inertialize: true,
            root_motion: true,
            arm_clearance: true,
            reduced_motion: false,
        }
    }
}

/// A gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GestureKind {
    /// Raise the right hand beside the head and wave.
    Wave,
    /// Nod twice.
    Acknowledge,
}

/// One beat of a script.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Beat {
    /// Walk to a ground point (the up component of `target` is ignored). `stride` is the root
    /// advance per step in metres, `speed` the cruise speed in metres per second.
    WalkTo {
        /// World target.
        target: Vec3,
        /// Root advance per step.
        stride: f32,
        /// Cruise speed.
        speed: f32,
    },
    /// Ease every posture driver (gaze, lean, arms) back to neutral with both feet planted.
    Stop,
    /// Turn the gaze (head chain and eyes) toward a world point. Persists until changed.
    LookAt {
        /// World point.
        target: Vec3,
    },
    /// A gesture of the given intensity (about 0 to 1.5).
    Gesture {
        /// Which.
        kind: GestureKind,
        /// How strong.
        intensity: f32,
    },
    /// Sit: the pelvis joint settles at `seat_height` above the ground plane, horizontally at
    /// `seat_pos`, with the feet planted.
    Sit {
        /// Height of the pelvis joint when seated.
        seat_height: f32,
        /// Horizontal position of the pelvis (the up component is ignored).
        seat_pos: Vec3,
    },
    /// Stand up from a seat (a stop if already standing).
    Stand,
    /// Hold with a breathing and sway micro motion.
    Idle {
        /// Seconds.
        secs: f32,
    },
    /// Ease a face channel to a value. Takes no script time; runs alongside the next beat.
    Express {
        /// Channel name.
        channel: String,
        /// Target value.
        value: f32,
    },
}

/// A list of beats and a style.
#[derive(Debug, Clone, PartialEq)]
pub struct PerformanceScript {
    /// The beats, in order.
    pub beats: Vec<Beat>,
    /// The style.
    pub style: Style,
    /// `(beat index, seconds)`: beat `index` is cut after `seconds` and preempted by the next
    /// beat (inertialized). Walks cannot be cut.
    pub cuts: Vec<(usize, f32)>,
}

impl PerformanceScript {
    /// An empty script.
    pub fn new(style: Style) -> PerformanceScript {
        PerformanceScript {
            beats: Vec::new(),
            style,
            cuts: Vec::new(),
        }
    }

    /// Append a beat.
    pub fn with(mut self, beat: Beat) -> PerformanceScript {
        self.beats.push(beat);
        self
    }

    /// Cut beat `beat` after `secs` seconds.
    pub fn with_cut(mut self, beat: usize, secs: f32) -> PerformanceScript {
        self.cuts.push((beat, secs));
        self
    }
}

// ---------------------------------------------------------------------------------------------
// Rig description
// ---------------------------------------------------------------------------------------------

/// Joint names for each role. Everything is data the host supplies.
#[derive(Debug, Clone, Copy)]
pub struct RigRoles<'a> {
    /// The pelvis (hip) joint.
    pub pelvis: &'a str,
    /// The spine chain, pelvis-most first.
    pub spine: &'a [&'a str],
    /// The neck, if the rig has one.
    pub neck: Option<&'a str>,
    /// The head.
    pub head: &'a str,
    /// Eye joints (may be empty).
    pub eyes: &'a [&'a str],
    /// Left arm: upper arm, forearm, hand.
    pub arm_l: [&'a str; 3],
    /// Right arm: upper arm, forearm, hand.
    pub arm_r: [&'a str; 3],
    /// Left leg: thigh, calf, foot (the foot joint is the ankle).
    pub leg_l: [&'a str; 3],
    /// Right leg: thigh, calf, foot.
    pub leg_r: [&'a str; 3],
}

/// World axes and body sizes the solver needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigGeometry {
    /// The up axis.
    pub up: Vec3,
    /// The direction the rest pose faces.
    pub forward: Vec3,
    /// Head sphere radius (metres).
    pub head_radius: f32,
    /// Head sphere centre above the head joint, along the head's up (metres).
    pub head_center_up: f32,
    /// Minimum distance of a hand from the head sphere centre (metres).
    pub hand_clearance: f32,
}

impl RigGeometry {
    /// Axes with a 0.10 m head sphere centred 0.09 m above the head joint and a 0.20 m hand
    /// clearance.
    pub fn new(up: Vec3, forward: Vec3) -> RigGeometry {
        RigGeometry {
            up,
            forward,
            head_radius: 0.10,
            head_center_up: 0.09,
            hand_clearance: 0.20,
        }
    }
}

/// The roles resolved to joint indices.
#[derive(Debug, Clone, PartialEq)]
pub struct RigMap {
    /// Pelvis joint.
    pub pelvis: usize,
    /// Spine chain.
    pub spine: Vec<usize>,
    /// Neck.
    pub neck: Option<usize>,
    /// Head.
    pub head: usize,
    /// Eyes.
    pub eyes: Vec<usize>,
    /// `[left, right]` arms, each `[upper, lower, hand]`.
    pub arms: [[usize; 3]; 2],
    /// `[left, right]` legs, each `[thigh, calf, foot]`.
    pub legs: [[usize; 3]; 2],
    /// Axes and sizes.
    pub geometry: RigGeometry,
}

impl RigMap {
    /// Resolve names against a skeleton. Every limb must be a parent-child chain.
    pub fn from_names(
        skel: &Skeleton,
        roles: &RigRoles<'_>,
        geometry: RigGeometry,
    ) -> Result<RigMap, PerformError> {
        let find = |n: &str| -> Result<usize, PerformError> {
            skel.joint_index(n)
                .ok_or_else(|| PerformError::UnknownJoint(n.to_string()))
        };
        let triple = |names: &[&str; 3]| -> Result<[usize; 3], PerformError> {
            Ok([find(names[0])?, find(names[1])?, find(names[2])?])
        };
        let mut spine = Vec::new();
        for n in roles.spine {
            spine.push(find(n)?);
        }
        let mut eyes = Vec::new();
        for n in roles.eyes {
            eyes.push(find(n)?);
        }
        let neck = match roles.neck {
            Some(n) => Some(find(n)?),
            None => None,
        };
        let map = RigMap {
            pelvis: find(roles.pelvis)?,
            spine,
            neck,
            head: find(roles.head)?,
            eyes,
            arms: [triple(&roles.arm_l)?, triple(&roles.arm_r)?],
            legs: [triple(&roles.leg_l)?, triple(&roles.leg_r)?],
            geometry,
        };
        let names = ["arm_l", "arm_r", "leg_l", "leg_r"];
        let chains = [map.arms[0], map.arms[1], map.legs[0], map.legs[1]];
        for (i, c) in chains.iter().enumerate() {
            let j = skel.joints();
            if j[c[1]].parent != Some(c[0]) || j[c[2]].parent != Some(c[1]) {
                return Err(PerformError::BadLimb { limb: names[i] });
            }
        }
        Ok(map)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    None,
    Pelvis,
    Spine,
    Neck,
    Head,
    Eye,
    Upper(usize),
    Lower(usize),
    End(usize),
}

/// A skeleton, its role map and the rest data the solver and planners need.
#[derive(Debug, Clone)]
pub struct Rig {
    skeleton: Skeleton,
    map: RigMap,
    rest_pos: Vec<Vec3>,
    rest_rot: Vec<Quat>,
    roles: Vec<Role>,
    limb: [[usize; 3]; 4],
    up: Vec3,
    fwd: Vec3,
    right: Vec3,
    up64: V,
    fwd64: V,
    right64: V,
    arm_pole: [Vec3; 2],
    rest_wrists: [Vec3; 2],
}

impl Rig {
    /// Build the rest data. Rejects non-unit scales and a degenerate basis.
    pub fn new(skeleton: Skeleton, map: RigMap) -> Result<Rig, PerformError> {
        let n = skeleton.joint_count();
        for (i, j) in skeleton.joints().iter().enumerate() {
            let s = j.rest.scale;
            if (s.x - 1.0).abs() > 1.0e-4
                || (s.y - 1.0).abs() > 1.0e-4
                || (s.z - 1.0).abs() > 1.0e-4
            {
                return Err(PerformError::NonUnitScale { joint: i });
            }
        }
        let mut rest_pos: Vec<Vec3> = Vec::with_capacity(n);
        let mut rest_rot: Vec<Quat> = Vec::with_capacity(n);
        for j in skeleton.joints() {
            let rot_here = j.rest.rotation.normalize();
            match j.parent {
                Some(p) => {
                    let pr = rest_rot[p];
                    let pp: Vec3 = rest_pos[p];
                    rest_pos.push(pp.add(pr.rotate(j.rest.translation)));
                    rest_rot.push(pr.mul(rot_here).normalize());
                }
                None => {
                    rest_pos.push(j.rest.translation);
                    rest_rot.push(rot_here);
                }
            }
        }
        let up = map
            .geometry
            .up
            .try_normalize()
            .ok_or(PerformError::BadBasis)?;
        let fwd_raw = map.geometry.forward;
        let fwd = fwd_raw
            .sub(up.scale(fwd_raw.dot(up)))
            .try_normalize()
            .ok_or(PerformError::BadBasis)?;
        let right = fwd
            .cross(up)
            .try_normalize()
            .ok_or(PerformError::BadBasis)?;
        let mut roles = vec![Role::None; n];
        roles[map.pelvis] = Role::Pelvis;
        for &s in &map.spine {
            roles[s] = Role::Spine;
        }
        if let Some(nk) = map.neck {
            roles[nk] = Role::Neck;
        }
        roles[map.head] = Role::Head;
        for &e in &map.eyes {
            roles[e] = Role::Eye;
        }
        let limb = [map.arms[0], map.arms[1], map.legs[0], map.legs[1]];
        for (l, c) in limb.iter().enumerate() {
            roles[c[0]] = Role::Upper(l);
            roles[c[1]] = Role::Lower(l);
            roles[c[2]] = Role::End(l);
        }
        // Elbow pole: the rest elbow's offset from the shoulder-hand line, else back-and-out.
        let mut arm_pole = [Vec3::ZERO; 2];
        for side in 0..2 {
            let c = limb[side];
            let h = rest_pos[c[0]];
            let m = rest_pos[c[1]];
            let e = rest_pos[c[2]];
            let axis = e.sub(h).try_normalize();
            let mut pole = None;
            if let Some(a) = axis {
                let mv = m.sub(h);
                let perp = mv.sub(a.scale(mv.dot(a)));
                if perp.length() > 0.01 {
                    pole = perp.try_normalize();
                }
            }
            let out = if side == 0 { right.neg() } else { right };
            arm_pole[side] = pole.unwrap_or_else(|| fwd.scale(-0.8).add(out.scale(0.6)));
        }
        let rest_wrists = [rest_pos[map.arms[0][2]], rest_pos[map.arms[1][2]]];
        Ok(Rig {
            skeleton,
            map,
            rest_pos,
            rest_rot,
            roles,
            limb,
            up,
            fwd,
            right,
            up64: v64(up),
            fwd64: v64(fwd),
            right64: v64(right),
            arm_pole,
            rest_wrists,
        })
    }

    /// The skeleton.
    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    /// The role map.
    pub fn map(&self) -> &RigMap {
        &self.map
    }

    /// Override the two neutral wrist IK targets in rest model coordinates, left then right.
    /// Their frame follows the pelvis translation and heading. Limb lengths and reach limits
    /// still come from the skeleton. By default each target is its bind-pose wrist position.
    pub fn with_rest_wrist_targets(mut self, wrists: [Vec3; 2]) -> Result<Self, PerformError> {
        if wrists.iter().any(|p| !p.is_finite()) {
            return Err(PerformError::NonFinite);
        }
        self.rest_wrists = wrists;
        Ok(self)
    }

    /// Rest world position of joint `j` (model space, before any root translation).
    pub fn rest_position(&self, j: usize) -> Vec3 {
        self.rest_pos[j]
    }

    /// World matrices of every joint for a pose, in model space (without `root_t`).
    pub fn world_matrices(&self, pose: &Pose) -> Vec<Mat4> {
        self.skeleton
            .world_from_locals(&pose.locals)
            .unwrap_or_default()
    }

    /// World positions of every joint for a pose, including the root translation.
    pub fn joint_world(&self, pose: &Pose) -> Vec<Vec3> {
        self.world_matrices(pose)
            .iter()
            .map(|m| m.transform_point(Vec3::ZERO).add(pose.root_t))
            .collect()
    }

    /// The head sphere centre of a pose, in world space.
    pub fn head_center(&self, pose: &Pose) -> Vec3 {
        let w = self.world_matrices(pose);
        let hm = match w.get(self.map.head) {
            Some(m) => *m,
            None => return Vec3::ZERO,
        };
        let local_up = self.rest_rot[self.map.head].conjugate().rotate(self.up);
        hm.transform_point(Vec3::ZERO)
            .add(
                hm.transform_vector(local_up)
                    .scale(self.map.geometry.head_center_up),
            )
            .add(pose.root_t)
    }

    fn leg_len(&self, f: usize) -> f64 {
        let c = self.limb[2 + f];
        let l1 = self.skeleton.joints()[c[1]].rest.translation.length();
        let l2 = self.skeleton.joints()[c[2]].rest.translation.length();
        f64::from(l1 + l2)
    }

    /// How far the pelvis is lowered when standing so both legs are bent to the design ratio.
    fn stand_crouch(&self) -> f64 {
        let mut best = 0.0f64;
        for f in 0..2 {
            let c = self.limb[2 + f];
            let hip = v64(self.rest_pos[c[0]]);
            let ank = v64(self.rest_pos[c[2]]);
            let d = vsub(hip, ank);
            let vert = vdot(d, self.up64);
            let horiz = vlen(vsub(d, vscale(self.up64, vert)));
            let l = self.leg_len(f);
            let need = ((DESIGN_RATIO * l).powi(2) - horiz * horiz).max(0.0).sqrt();
            best = best.max(vert - need);
        }
        best
    }

    /// The pelvis lowering for a walk of the given step length (feet 0.6 steps ahead and behind).
    fn walk_crouch(&self, step: f64) -> f64 {
        let mut best = self.stand_crouch();
        for f in 0..2 {
            let c = self.limb[2 + f];
            let hip = v64(self.rest_pos[c[0]]);
            let ank = v64(self.rest_pos[c[2]]);
            let d = vsub(hip, ank);
            let vert = vdot(d, self.up64);
            let hf = vdot(d, self.fwd64).abs() + 0.6 * step;
            let hr = vdot(d, self.right64);
            let h2 = hf * hf + hr * hr;
            let l = self.leg_len(f);
            let v2 = (DESIGN_RATIO * l).powi(2) - h2;
            let need = if v2 > 0.0 { v2.sqrt() } else { 0.3 * l };
            best = best.max(vert - need);
        }
        best
    }
}

// ---------------------------------------------------------------------------------------------
// Drivers
// ---------------------------------------------------------------------------------------------

// Drivers 0..3: world ground position of the root (indexed by literal; a `0 + c` offset constant
// would trip clippy::identity_op).
const D_HEAD: usize = 3; // heading yaw about up, radians, left positive
const D_PEL: usize = 4; // 3: pelvis offset in the heading frame (forward, right, up)
const D_PELROT: usize = 7; // 3: pelvis pitch (forward +), roll (right +), yaw (left +)
const D_SPINE: usize = 10; // 3: total spine bend, distributed over the chain
const D_GAZE: usize = 13; // 2: gaze yaw (left +), pitch (down +)
const D_NOD: usize = 15; // head nod pitch, additive
const D_FOOT: usize = 16; // 2 x 6: target x y z (world), yaw, toe pitch, ik weight
const D_ARM: usize = 28; // 2 x 8: swing f r u, wave weight, wave f r u (rest: the hang), elbow out
const N_BASE: usize = 44;

fn initial_drivers(rig: &Rig) -> Vec<f64> {
    let mut d = vec![0.0; N_BASE];
    d[D_PEL + 2] = -rig.stand_crouch();
    for f in 0..2 {
        let a = v64(rig.rest_pos[rig.limb[2 + f][2]]);
        d[D_FOOT + f * 6..D_FOOT + f * 6 + 3].copy_from_slice(&a);
        d[D_FOOT + f * 6 + 5] = 1.0;
    }
    // The wave point rests at the hang (the pelvis offset included, as in `solve_pass`), so the
    // first wave's raise blends it out of the hand's own position. Resting at the origin, the
    // inertialized wave point flew in from the floor while the wave weight rose: the target dipped
    // away from the shoulder into the soft reach limit, J = 0.10 m/s at 1 ms.
    for side in 0..2 {
        let hand = v64(rig.rest_pos[rig.limb[side][2]]);
        let h = heading_coords(rig, hand, 0.0);
        let b = D_ARM + side * 8;
        for c in 0..3 {
            d[b + 4 + c] = h[c] + d[D_PEL + c];
        }
    }
    d
}

#[derive(Clone, Debug)]
struct Carry {
    d: Vec<f64>,
    seated: bool,
}

// ---------------------------------------------------------------------------------------------
// Pose and the solver
// ---------------------------------------------------------------------------------------------

/// One evaluated pose: the root translation (world), every joint's local transform in skeleton
/// order, and the face channel values.
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    /// World translation of the rig origin (the ground point under the character).
    pub root_t: Vec3,
    /// Local transform of every joint.
    pub locals: Vec<Transform>,
    /// Face channel values, in the performance's face-channel order.
    pub face: Vec<f32>,
}

fn nlerp(a: Quat, b: Quat, w: f32) -> Quat {
    let b = if a.dot(b) < 0.0 { b.neg() } else { b };
    a.scale(1.0 - w).add(b.scale(w)).normalize()
}

/// Keep `p` at least `r` from `c`. Points farther than `r + s` (`s` = [`CLEARANCE_SOFT`]) are
/// unchanged; nearer ones are moved out along the ray from `c` to the distance
/// `r + s exp((l - r - s) / s)`, which has value and slope 1 at `l = r + s` (C1, no velocity
/// step) and stays above `r` for every `l`. A hard projection onto the sphere is continuous but a
/// velocity step at the moment of contact (J = 0.44 m/s on a large slow wave, against 0.04 soft).
fn push_out(p: Vec3, c: Vec3, r: f32) -> Vec3 {
    let v = p.sub(c);
    let l = v.length();
    // Fault injection (mutation control `hard_push_out`): the hard projection onto the sphere.
    let (reach, band) = if cfg!(figure_mutant = "hard_push_out") {
        (r, 0.0)
    } else {
        let s = CLEARANCE_SOFT.min(r).max(1.0e-4);
        (r + s * ((l - r - s) / s).exp(), s)
    };
    if l >= r + band {
        return p;
    }
    match v.try_normalize() {
        Some(u) if l > 1.0e-6 => c.add(u.scale(reach)),
        _ => c.add(Vec3::new(0.0, 0.0, reach)),
    }
}

/// Map a driver vector to joint locals. `head_center` (model space) enables hand clearance.
/// Returns the locals and the head sphere centre of this pass.
fn solve_pass(
    rig: &Rig,
    d: &[f64],
    foot_lock: bool,
    head_center: Option<Vec3>,
) -> (Vec<Transform>, Vec3) {
    let n = rig.skeleton.joint_count();
    let g = |i: usize| -> f32 { d[i] as f32 };
    let up = rig.up;
    let yawq = Quat::from_axis_angle(up, g(D_HEAD));
    let hf = yawq.rotate(rig.fwd);
    let hr = yawq.rotate(rig.right);
    let root_t = Vec3::new(g(0), g(1), g(2));
    let tilt = |pitch: f32, roll: f32, yaw: f32| -> Quat {
        let qy = Quat::from_axis_angle(up, yaw);
        let qp = Quat::from_axis_angle(hr, -pitch);
        let qr = Quat::from_axis_angle(hf, roll);
        qy.mul(qp).mul(qr)
    };
    let pel_off = hf
        .scale(g(D_PEL))
        .add(hr.scale(g(D_PEL + 1)))
        .add(up.scale(g(D_PEL + 2)));
    let pel_pos = yawq.rotate(rig.rest_pos[rig.map.pelvis]).add(pel_off);
    let pel_rot = tilt(g(D_PELROT), g(D_PELROT + 1), g(D_PELROT + 2))
        .mul(yawq)
        .mul(rig.rest_rot[rig.map.pelvis]);

    // Gaze weights (yaw, pitch) for neck, head, eyes; absent parts hand their share to the head.
    let has_neck = rig.map.neck.is_some();
    let has_eyes = !rig.map.eyes.is_empty();
    let mut wy = [0.35f32, 0.50, 0.15];
    let mut wp = [0.30f32, 0.55, 0.15];
    if !has_neck {
        wy[1] += wy[0];
        wy[0] = 0.0;
        wp[1] += wp[0];
        wp[0] = 0.0;
    }
    if !has_eyes {
        wy[1] += wy[2];
        wy[2] = 0.0;
        wp[1] += wp[2];
        wp[2] = 0.0;
    }
    let (gy, gp, nod) = (g(D_GAZE), g(D_GAZE + 1), g(D_NOD));
    let ns = rig.map.spine.len().max(1) as f32;

    let joints = rig.skeleton.joints();
    let mut loc: Vec<Transform> = rig.skeleton.rest_locals();
    let mut wpos = vec![Vec3::ZERO; n];
    let mut wrot = vec![Quat::IDENTITY; n];
    let mut lower_world = [Quat::IDENTITY; 4];

    for j in 0..n {
        let joint = &joints[j];
        let (ppos, prot) = match joint.parent {
            Some(p) => (wpos[p], wrot[p]),
            None => (Vec3::ZERO, Quat::IDENTITY),
        };
        let rest = joint.rest;
        let mut t = rest.translation;
        let mut q = rest.rotation;
        if joint.parent.is_none() {
            t = yawq.rotate(t);
            q = yawq.mul(q);
        }
        let pc = prot.conjugate();
        let inherited = prot.mul(rest.rotation);
        match rig.roles[j] {
            Role::None => {}
            Role::Pelvis => {
                t = pc.rotate(pel_pos.sub(ppos));
                q = pc.mul(pel_rot);
            }
            Role::Spine => {
                let f = 1.0 / ns;
                let delta = tilt(g(D_SPINE) * f, g(D_SPINE + 1) * f, g(D_SPINE + 2) * f);
                q = pc.mul(delta.mul(inherited));
            }
            Role::Neck => {
                let delta = tilt(gp * wp[0], 0.0, gy * wy[0]);
                q = pc.mul(delta.mul(inherited));
            }
            Role::Head => {
                let delta = tilt(gp * wp[1] + nod, 0.0, gy * wy[1]);
                q = pc.mul(delta.mul(inherited));
            }
            Role::Eye => {
                let delta = tilt(gp * wp[2], 0.0, gy * wy[2]);
                q = pc.mul(delta.mul(inherited));
            }
            Role::Upper(l) => {
                let c = rig.limb[l];
                let lo = joints[c[1]].rest;
                let en = joints[c[2]].rest;
                let root_pos = ppos.add(prot.rotate(t));
                let fk_end = root_pos
                    .add(inherited.rotate(lo.translation))
                    .add(inherited.mul(lo.rotation).rotate(en.translation));
                let (target, pole) = if l >= 2 {
                    let f = l - 2;
                    let ikw = if foot_lock {
                        g(D_FOOT + f * 6 + 5)
                    } else {
                        0.0
                    };
                    let want = Vec3::new(
                        g(D_FOOT + f * 6),
                        g(D_FOOT + f * 6 + 1),
                        g(D_FOOT + f * 6 + 2),
                    )
                    .sub(root_t);
                    (fk_end.add(want.sub(fk_end).scale(ikw)), hf)
                } else {
                    let s = l;
                    let base = D_ARM + s * 8;
                    let hand_rest = rig.rest_wrists[s];
                    let hang = yawq.rotate(hand_rest).add(pel_off);
                    let swing = hf
                        .scale(g(base))
                        .add(hr.scale(g(base + 1)))
                        .add(up.scale(g(base + 2)));
                    let w = g(base + 3);
                    let wave = hf
                        .scale(g(base + 4))
                        .add(hr.scale(g(base + 5)))
                        .add(up.scale(g(base + 6)));
                    let hang_s = hang.add(swing);
                    let mut tgt = hang_s.add(wave.sub(hang_s).scale(w));
                    if let Some(hc) = head_center {
                        let l1 = lo.translation.length();
                        let l2 = en.translation.length();
                        let t1 = ik::limit_reach(root_pos, tgt, l1, l2, ik::DEFAULT_SOFT_START);
                        let t2 = push_out(t1, hc, rig.map.geometry.hand_clearance);
                        tgt = ik::limit_reach(root_pos, t2, l1, l2, ik::DEFAULT_SOFT_START);
                    }
                    let out = if s == 0 { hr.neg() } else { hr };
                    let pole = yawq.rotate(rig.arm_pole[s]).add(out.scale(g(base + 7)));
                    (tgt, pole)
                };
                let lp = ik::solve_limb(&ik::LimbInput {
                    parent_world: prot,
                    upper_rest_rot: rest.rotation,
                    lower_rest_rot: lo.rotation,
                    upper_to_lower: lo.translation,
                    lower_to_end: en.translation,
                    root: root_pos,
                    target,
                    pole,
                    soft_start: ik::DEFAULT_SOFT_START,
                });
                lower_world[l] = lp.lower_world;
                q = lp.upper_local;
            }
            Role::Lower(l) => {
                q = pc.mul(lower_world[l]);
            }
            Role::End(l) => {
                if l >= 2 {
                    let f = l - 2;
                    let ikw = if foot_lock {
                        g(D_FOOT + f * 6 + 5)
                    } else {
                        0.0
                    };
                    let yawf = Quat::from_axis_angle(up, g(D_FOOT + f * 6 + 3));
                    let rf = yawf.rotate(rig.right);
                    let pitch = Quat::from_axis_angle(rf, -g(D_FOOT + f * 6 + 4));
                    let want = pitch.mul(yawf).mul(rig.rest_rot[j]);
                    q = pc.mul(nlerp(inherited, want, ikw));
                }
            }
        }
        let q = q.normalize();
        loc[j] = Transform::new(t, q, rest.scale);
        wpos[j] = ppos.add(prot.rotate(t));
        wrot[j] = prot.mul(q).normalize();
    }
    let hd = rig.map.head;
    let head_up = wrot[hd].rotate(rig.rest_rot[hd].conjugate().rotate(up));
    let hc = wpos[hd].add(head_up.scale(rig.map.geometry.head_center_up));
    (loc, hc)
}

impl Rig {
    /// Map a driver vector (at least the base drivers) to a pose. `foot_lock` false makes the
    /// feet follow the pelvis (the control of the planting guarantee).
    pub fn solve(&self, d: &[f64], foot_lock: bool, arm_clearance: bool) -> Pose {
        let root_t = Vec3::new(d[0] as f32, d[1] as f32, d[2] as f32);
        let locals = if arm_clearance {
            let (_, hc0) = solve_pass(self, d, foot_lock, None);
            solve_pass(self, d, foot_lock, Some(hc0)).0
        } else {
            solve_pass(self, d, foot_lock, None).0
        };
        Pose {
            root_t,
            locals,
            face: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Beat plans
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Swing {
    foot: usize,
    t0: f64,
    t1: f64,
    from: V,
    to: V,
    yaw0: f64,
    yaw1: f64,
}

#[derive(Clone, Debug)]
struct WalkPlan {
    p0: V,
    dir: V,
    up: V,
    psi0: f64,
    psi1: f64,
    turn: f64,
    tau: f64,
    v: f64,
    n: usize,
    swings: Vec<Swing>,
    feet0: [V; 2],
    yaw0: [f64; 2],
    extra: f64,
    lift: f64,
    bob: f64,
    sway: f64,
    twist: f64,
    arm_sw: f64,
    lean: f64,
    toe: f64,
    /// Per arm: forward and up components of the rest shoulder-to-hand vector.
    arm_fz: [[f64; 2]; 2],
}

#[derive(Clone, Debug)]
struct DissolvePlan {
    p0: V,
    p1: V,
    psi0: f64,
    psi1: f64,
    feet0: [V; 2],
    feet1: [V; 2],
    yaw0: [f64; 2],
    yaw1: [f64; 2],
}

#[derive(Clone, Debug)]
enum Kind {
    Settle {
        targets: Vec<(usize, f64)>,
    },
    Idle {
        tempo: f64,
    },
    Look {
        gaze: [f64; 2],
        spine_yaw: f64,
        ease: f64,
    },
    Wave {
        t_r: f64,
        t_w: f64,
        t_l: f64,
        freq: f64,
        osc: f64,
        base: V,
        out: f64,
    },
    Nod {
        period: f64,
        depth: f64,
        lean: f64,
    },
    Walk(Box<WalkPlan>),
    Dissolve(Box<DissolvePlan>),
    Move {
        to: [f64; 3],
        lean: f64,
    },
}

#[derive(Clone, Debug)]
struct BeatPlan {
    dur: f64,
    start: Vec<f64>,
    amp: f64,
    kind: Kind,
}

/// Distance travelled by the root `t` seconds into a walk of `n` cruise steps.
fn walk_dist(t: f64, tau: f64, v: f64, n: usize) -> f64 {
    let total = (n as f64 + 1.0) * tau;
    let t = t.clamp(0.0, total);
    let ec = n as f64 * tau;
    if t <= tau {
        let u = t / tau;
        v * tau * (u * u * u - 0.5 * u * u * u * u)
    } else if t <= ec {
        0.5 * v * tau + v * (t - tau)
    } else {
        let u = (t - ec) / tau;
        0.5 * v * tau + v * (ec - tau) + v * tau * (u - (u * u * u - 0.5 * u * u * u * u))
    }
}

impl WalkPlan {
    fn foot_at(&self, f: usize, t: f64) -> (V, f64, f64) {
        let mut pos = self.feet0[f];
        let mut yaw = self.yaw0[f];
        for sw in self.swings.iter().filter(|s| s.foot == f) {
            if t < sw.t0 {
                break;
            }
            if t < sw.t1 {
                let u = (t - sw.t0) / (sw.t1 - sw.t0);
                let s = smoother(u);
                let s2 = (PI * u).sin();
                let mut p = [0.0; 3];
                for c in 0..3 {
                    p[c] =
                        sw.from[c] + (sw.to[c] - sw.from[c]) * s + self.up[c] * self.lift * s2 * s2;
                }
                let toe = self.toe * s2 * s2 * (1.0 - 2.0 * u);
                return (p, lerp(sw.yaw0, sw.yaw1, s), toe);
            }
            pos = sw.to;
            yaw = sw.yaw1;
        }
        (pos, yaw, 0.0)
    }
}

impl BeatPlan {
    /// The driver vector `t` seconds into the beat (held outside `0..=dur`).
    fn eval(&self, t: f64) -> Vec<f64> {
        let t = t.clamp(0.0, self.dur);
        let s = &self.start;
        let mut d = s.clone();
        let amp = self.amp;
        match &self.kind {
            Kind::Settle { targets } => {
                let r = ramp(t, 0.0, self.dur);
                for &(i, v) in targets {
                    d[i] = lerp(s[i], v, r);
                }
            }
            Kind::Idle { tempo } => {
                let e = (self.dur * 0.5).min(0.6);
                let env = ramp(t, 0.0, e) * (1.0 - ramp(t, self.dur - e, e));
                let breath = (2.0 * PI * t * tempo / 4.0).sin();
                let sway = (2.0 * PI * t * tempo / 6.5).sin();
                let drift = (2.0 * PI * t * tempo / 9.1).sin();
                d[D_SPINE] += 0.012 * amp * env * breath;
                d[D_PEL + 1] += 0.005 * amp * env * sway;
                d[D_PEL + 2] += 0.0015 * amp * env * breath;
                d[D_GAZE] += 0.03 * amp * env * drift;
            }
            Kind::Look {
                gaze,
                spine_yaw,
                ease,
            } => {
                let r = ramp(t, 0.0, *ease);
                d[D_GAZE] = lerp(s[D_GAZE], gaze[0], r);
                d[D_GAZE + 1] = lerp(s[D_GAZE + 1], gaze[1], r);
                d[D_SPINE + 2] = lerp(s[D_SPINE + 2], *spine_yaw, r);
            }
            Kind::Wave {
                t_r,
                t_w,
                t_l,
                freq,
                osc,
                base,
                out,
            } => {
                let w = ramp(t, 0.0, *t_r) * (1.0 - ramp(t, t_r + t_w, *t_l));
                let o = osc * w * (2.0 * PI * freq * t).sin();
                let b = D_ARM + 8;
                d[b + 3] = w;
                d[b + 4] = base[0];
                d[b + 5] = base[1] + o;
                d[b + 6] = base[2];
                d[b + 7] = out * w;
                d[D_SPINE + 1] += 0.03 * amp * w;
            }
            Kind::Nod {
                period,
                depth,
                lean,
            } => {
                let v = if t < 2.0 * period {
                    depth * 0.5 * (1.0 - (2.0 * PI * t / period).cos())
                } else {
                    0.0
                };
                d[D_NOD] += v;
                d[D_SPINE] += lean * v;
            }
            Kind::Walk(w) => {
                let total = self.dur;
                let dd = walk_dist(t, w.tau, w.v, w.n);
                for c in 0..3 {
                    d[c] = w.p0[c] + w.dir[c] * dd;
                }
                d[D_HEAD] = w.psi0 + (w.psi1 - w.psi0) * ramp(t, 0.0, w.turn);
                let env = ramp(t, 0.0, w.tau) * (1.0 - ramp(t, total - w.tau, w.tau));
                let sp = (PI * t / w.tau).sin();
                d[D_PEL + 1] = s[D_PEL + 1] + w.sway * env * sp;
                d[D_PEL + 2] = s[D_PEL + 2] - w.extra * env + w.bob * env * sp * sp;
                d[D_PELROT + 2] = s[D_PELROT + 2] - w.twist * env * sp;
                let gz = 1.0 - ramp(t, 0.0, 2.0 * w.tau);
                d[D_SPINE + 2] = s[D_SPINE + 2] * gz + 0.8 * w.twist * env * sp;
                d[D_SPINE] = s[D_SPINE] + w.lean * env;
                d[D_GAZE] = s[D_GAZE] * gz;
                d[D_GAZE + 1] = s[D_GAZE + 1] * gz;
                d[D_NOD] = s[D_NOD] * gz;
                for side in 0..2 {
                    let sgn = if side == 0 { -1.0 } else { 1.0 };
                    let sw = sgn * w.arm_sw * env * sp;
                    // The hand swings on a circle about the shoulder, so it rises as it swings.
                    let [f0, z0] = w.arm_fz[side];
                    let q = f0 * f0 + z0 * z0;
                    let lift = z0.abs() - (q - (f0 + sw) * (f0 + sw)).max(1.0e-9).sqrt();
                    d[D_ARM + side * 8] = s[D_ARM + side * 8] + sw;
                    d[D_ARM + side * 8 + 2] = s[D_ARM + side * 8 + 2] + lift;
                }
                for f in 0..2 {
                    let (p, yaw, toe) = w.foot_at(f, t);
                    d[D_FOOT + f * 6..D_FOOT + f * 6 + 3].copy_from_slice(&p);
                    d[D_FOOT + f * 6 + 3] = yaw;
                    d[D_FOOT + f * 6 + 4] = toe;
                }
            }
            Kind::Dissolve(p) => {
                let r = ramp(t, 0.0, self.dur);
                for c in 0..3 {
                    d[c] = lerp(p.p0[c], p.p1[c], r);
                }
                d[D_HEAD] = lerp(p.psi0, p.psi1, r);
                for f in 0..2 {
                    for c in 0..3 {
                        d[D_FOOT + f * 6 + c] = lerp(p.feet0[f][c], p.feet1[f][c], r);
                    }
                    d[D_FOOT + f * 6 + 3] = lerp(p.yaw0[f], p.yaw1[f], r);
                }
            }
            Kind::Move { to, lean } => {
                let u = if self.dur > 0.0 { t / self.dur } else { 1.0 };
                let uh = smoother(u / 0.75);
                let uv = smoother((u - 0.12) / 0.88);
                d[D_PEL] = lerp(s[D_PEL], to[0], uh);
                d[D_PEL + 1] = lerp(s[D_PEL + 1], to[1], uh);
                d[D_PEL + 2] = lerp(s[D_PEL + 2], to[2], uv);
                let sn = (PI * u).sin();
                d[D_SPINE] += lean * sn * sn;
            }
        }
        d
    }
}

/// Public summary of one walk beat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WalkSummary {
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Path length in metres.
    pub distance: f32,
    /// Root advance per step in metres.
    pub stride: f32,
    /// Number of cruise steps (the root travels `stride * steps`).
    pub steps: usize,
}

struct Ctx<'a> {
    rig: &'a Rig,
    amp: f64,
    tempo: f64,
    settle: f64,
    reduced: bool,
}

fn neutral_targets(seated: bool) -> Vec<(usize, f64)> {
    let mut v: Vec<(usize, f64)> = Vec::new();
    for i in 0..3 {
        v.push((D_PELROT + i, 0.0));
        v.push((D_SPINE + i, 0.0));
    }
    v.push((D_GAZE, 0.0));
    v.push((D_GAZE + 1, 0.0));
    v.push((D_NOD, 0.0));
    if !seated {
        v.push((D_PEL, 0.0));
        v.push((D_PEL + 1, 0.0));
    }
    for side in 0..2 {
        for k in [0usize, 1, 2, 3, 7] {
            v.push((D_ARM + side * 8 + k, 0.0));
        }
    }
    v
}

fn plan_settle(cx: &Ctx<'_>, c: &Carry) -> BeatPlan {
    BeatPlan {
        dur: 2.0 * cx.settle + 0.2,
        start: c.d.clone(),
        amp: cx.amp,
        kind: Kind::Settle {
            targets: neutral_targets(c.seated),
        },
    }
}

fn root_of(d: &[f64]) -> V {
    [d[0], d[1], d[2]]
}

fn plan_walk(
    cx: &Ctx<'_>,
    c: &Carry,
    target: Vec3,
    stride: f32,
    speed: f32,
) -> Result<BeatPlan, PerformError> {
    if c.seated {
        return Err(PerformError::Posture("walk while seated"));
    }
    let rig = cx.rig;
    let (up, fw0) = (rig.up64, rig.fwd64);
    let left0 = vscale(rig.right64, -1.0);
    let s = &c.d;
    let p0 = root_of(s);
    let delta = vsub(v64(target), p0);
    let dplan = vsub(delta, vscale(up, vdot(delta, up)));
    let dist = vlen(dplan);
    if !dist.is_finite() {
        return Err(PerformError::BadBeat {
            beat: 0,
            what: "walk target is not finite",
        });
    }
    if dist < MIN_WALK {
        return Ok(plan_settle(cx, c));
    }
    let dir = vscale(dplan, 1.0 / dist);
    let psi0 = s[D_HEAD];
    let mut psi1 = vdot(dir, left0).atan2(vdot(dir, fw0));
    while psi1 - psi0 > PI {
        psi1 -= 2.0 * PI;
    }
    while psi1 - psi0 < -PI {
        psi1 += 2.0 * PI;
    }
    let speed = if speed > 0.0 { f64::from(speed) } else { 1.0 };
    let p1 = vadd(p0, dplan);
    let mut feet0 = [[0.0; 3]; 2];
    let mut yaw0 = [0.0; 2];
    let mut rest_ank = [[0.0; 3]; 2];
    for f in 0..2 {
        feet0[f].copy_from_slice(&s[D_FOOT + f * 6..D_FOOT + f * 6 + 3]);
        yaw0[f] = s[D_FOOT + f * 6 + 3];
        rest_ank[f] = v64(rig.rest_pos[rig.limb[2 + f][2]]);
    }
    if cx.reduced {
        let total = (dist / (speed * 0.9)).clamp(0.8, 3.0);
        let mut feet1 = [[0.0; 3]; 2];
        for f in 0..2 {
            feet1[f] = vadd(p1, vrot(rest_ank[f], up, psi1));
        }
        let plan = DissolvePlan {
            p0,
            p1,
            psi0,
            psi1,
            feet0,
            feet1,
            yaw0,
            yaw1: [psi1, psi1],
        };
        return Ok(BeatPlan {
            dur: total,
            start: s.clone(),
            amp: cx.amp,
            kind: Kind::Dissolve(Box::new(plan)),
        });
    }

    let stride = f64::from(stride).clamp(0.15, 1.2);
    let n = ((dist / stride).round() as usize).clamp(1, 2000);
    let step = dist / n as f64;
    let v = (speed * cx.tempo).clamp(0.2, 3.0);
    let tau = step / v;
    let total = (n as f64 + 1.0) * tau;
    let turn = (2.0 * tau).min(0.6 * total);
    let psi_at = |t: f64| lerp(psi0, psi1, ramp(t, 0.0, turn));
    let mut swings: Vec<Swing> = Vec::new();
    let mut last_pos = feet0;
    let mut last_yaw = yaw0;
    for k in 0..=n {
        let f = k % 2;
        let t0 = k as f64 * tau + 0.1 * tau;
        let t1 = k as f64 * tau + 0.9 * tau;
        let fk = if k + 2 <= n {
            let dl = walk_dist(t1, tau, v, n);
            let dn = walk_dist((k as f64 + 2.1) * tau, tau, v, n);
            dl + 0.5 * (dn - dl)
        } else {
            dist
        };
        let ps = psi_at(t1);
        let to = vadd(vadd(p0, vscale(dir, fk)), vrot(rest_ank[f], up, ps));
        swings.push(Swing {
            foot: f,
            t0,
            t1,
            from: last_pos[f],
            to,
            yaw0: last_yaw[f],
            yaw1: ps,
        });
        last_pos[f] = to;
        last_yaw[f] = ps;
    }
    let amp = cx.amp;
    let mut arm_fz = [[0.0; 2]; 2];
    for side in 0..2 {
        let rel = vsub(
            v64(rig.rest_pos[rig.limb[side][2]]),
            v64(rig.rest_pos[rig.limb[side][0]]),
        );
        arm_fz[side] = [vdot(rel, fw0), vdot(rel, up)];
    }
    let extra = (rig.walk_crouch(step) - rig.stand_crouch()).max(0.0);
    let plan = WalkPlan {
        p0,
        dir,
        up,
        psi0,
        psi1,
        turn,
        tau,
        v,
        n,
        swings,
        feet0,
        yaw0,
        extra,
        lift: 0.04 + 0.05 * amp,
        bob: 0.008 + 0.010 * amp,
        sway: 0.012 + 0.012 * amp,
        twist: 0.04 + 0.06 * amp,
        arm_sw: 0.08 + 0.12 * amp,
        lean: 0.02 + 0.03 * amp,
        toe: 0.30 * amp,
        arm_fz,
    };
    Ok(BeatPlan {
        dur: total,
        start: s.clone(),
        amp,
        kind: Kind::Walk(Box::new(plan)),
    })
}

/// Heading-frame coordinates `(forward, right, up)` of a model-space vector at yaw `psi`.
fn heading_coords(rig: &Rig, v: V, psi: f64) -> V {
    let f = vrot(rig.fwd64, rig.up64, psi);
    let r = vrot(rig.right64, rig.up64, psi);
    [vdot(v, f), vdot(v, r), vdot(v, rig.up64)]
}

fn plan_look(cx: &Ctx<'_>, c: &Carry, target: Vec3) -> Result<BeatPlan, PerformError> {
    let rig = cx.rig;
    let s = &c.d;
    let psi = s[D_HEAD];
    let root = root_of(s);
    let head_rest = v64(rig.rest_pos[rig.map.head]);
    let eye_rest = vadd(
        head_rest,
        vscale(rig.up64, f64::from(rig.map.geometry.head_center_up)),
    );
    let off = heading_to_world(rig, [s[D_PEL], s[D_PEL + 1], s[D_PEL + 2]], psi);
    let eye = vadd(vadd(root, vrot(eye_rest, rig.up64, psi)), off);
    let dir = vsub(v64(target), eye);
    if vlen(dir) < 0.05 || !vlen(dir).is_finite() {
        return Err(PerformError::BadBeat {
            beat: 0,
            what: "look target too close to the head",
        });
    }
    let [df, dr, du] = heading_coords(rig, dir, psi);
    let yaw = (-dr).atan2(df);
    let pitch = (-du).atan2((df * df + dr * dr).sqrt());
    let g = 0.6 + 0.4 * cx.amp.min(1.0);
    let gy = (yaw * g).clamp(-1.3, 1.3);
    let gp = (pitch * g).clamp(-0.6, 0.6);
    let ease = 0.55 / cx.tempo;
    Ok(BeatPlan {
        dur: ease + 0.35,
        start: s.clone(),
        amp: cx.amp,
        kind: Kind::Look {
            gaze: [gy, gp],
            spine_yaw: 0.12 * gy,
            ease,
        },
    })
}

/// World vector of heading-frame coordinates `(forward, right, up)`.
fn heading_to_world(rig: &Rig, c: V, psi: f64) -> V {
    let f = vrot(rig.fwd64, rig.up64, psi);
    let r = vrot(rig.right64, rig.up64, psi);
    vadd(
        vadd(vscale(f, c[0]), vscale(r, c[1])),
        vscale(rig.up64, c[2]),
    )
}

fn plan_gesture(cx: &Ctx<'_>, c: &Carry, kind: GestureKind, intensity: f32) -> BeatPlan {
    let k = f64::from(intensity).clamp(0.0, 1.5);
    let s = &c.d;
    match kind {
        GestureKind::Wave => {
            let rig = cx.rig;
            let hc = vadd(
                v64(rig.rest_pos[rig.map.head]),
                vscale(rig.up64, f64::from(rig.map.geometry.head_center_up)),
            );
            let [hf, hr, hu] = heading_coords(rig, hc, 0.0);
            let clear = f64::from(rig.map.geometry.hand_clearance);
            let base = [
                hf + WAVE_FORWARD,
                hr + (clear + WAVE_LATERAL),
                hu - rig.stand_crouch() + 0.04 + 0.06 * cx.amp.min(1.5),
            ];
            let freq = 2.4 * cx.tempo;
            let t_r = 0.55 / cx.tempo;
            let t_w = 3.0 / freq;
            let t_l = 0.6 / cx.tempo;
            BeatPlan {
                dur: t_r + t_w + t_l,
                start: s.clone(),
                amp: cx.amp,
                kind: Kind::Wave {
                    t_r,
                    t_w,
                    t_l,
                    freq,
                    osc: 0.07 * (0.35 + 0.65 * cx.amp) * k,
                    base,
                    out: 0.5,
                },
            }
        }
        GestureKind::Acknowledge => {
            let period = 0.72 / cx.tempo;
            BeatPlan {
                dur: 2.0 * period + 0.15,
                start: s.clone(),
                amp: cx.amp,
                kind: Kind::Nod {
                    period,
                    depth: 0.30 * k * (0.3 + 0.7 * cx.amp.min(1.5)),
                    lean: 0.3,
                },
            }
        }
    }
}

fn plan_sit(
    cx: &Ctx<'_>,
    c: &Carry,
    seat_height: f32,
    seat_pos: Vec3,
) -> Result<BeatPlan, PerformError> {
    if c.seated {
        return Err(PerformError::Posture("sit while seated"));
    }
    let rig = cx.rig;
    let s = &c.d;
    let up = rig.up64;
    let psi = s[D_HEAD];
    let sp = v64(seat_pos);
    let planar = vsub(sp, vscale(up, vdot(sp, up)));
    let p1 = vadd(planar, vscale(up, f64::from(seat_height)));
    let base = vadd(root_of(s), vrot(v64(rig.rest_pos[rig.map.pelvis]), up, psi));
    let off = heading_coords(rig, vsub(p1, base), psi);
    // Reach of each leg from the seated hip to the planted ankle.
    for f in 0..2 {
        let cl = rig.limb[2 + f];
        let hip_rel = vsub(v64(rig.rest_pos[cl[0]]), v64(rig.rest_pos[rig.map.pelvis]));
        let hip = vadd(p1, vrot(hip_rel, up, psi));
        let ank = [
            s[D_FOOT + f * 6],
            s[D_FOOT + f * 6 + 1],
            s[D_FOOT + f * 6 + 2],
        ];
        let ratio = vlen(vsub(hip, ank)) / rig.leg_len(f);
        if !ratio.is_finite() || ratio > 0.985 {
            return Err(PerformError::SeatUnreachable {
                ratio: ratio as f32,
            });
        }
    }
    Ok(BeatPlan {
        dur: (1.9 / cx.tempo).max(1.0),
        start: s.clone(),
        amp: cx.amp,
        kind: Kind::Move {
            to: off,
            lean: 0.25 * (0.5 + 0.5 * cx.amp.min(1.0)),
        },
    })
}

fn plan_stand(cx: &Ctx<'_>, c: &Carry) -> BeatPlan {
    if !c.seated {
        return plan_settle(cx, c);
    }
    BeatPlan {
        dur: (1.9 / cx.tempo).max(1.0),
        start: c.d.clone(),
        amp: cx.amp,
        kind: Kind::Move {
            to: [0.0, 0.0, -cx.rig.stand_crouch()],
            lean: 0.25 * (0.5 + 0.5 * cx.amp.min(1.0)),
        },
    }
}

fn plan_idle(cx: &Ctx<'_>, c: &Carry, secs: f32) -> BeatPlan {
    BeatPlan {
        dur: f64::from(secs).clamp(0.1, MAX_SECONDS),
        start: c.d.clone(),
        amp: cx.amp,
        kind: Kind::Idle { tempo: cx.tempo },
    }
}

/// Compile one non-face beat. `Express` is handled by the caller.
fn plan_beat(cx: &Ctx<'_>, c: &Carry, beat: &Beat) -> Result<Option<BeatPlan>, PerformError> {
    let plan = match beat {
        Beat::WalkTo {
            target,
            stride,
            speed,
        } => plan_walk(cx, c, *target, *stride, *speed)?,
        Beat::Stop => plan_settle(cx, c),
        Beat::LookAt { target } => plan_look(cx, c, *target)?,
        Beat::Gesture { kind, intensity } => plan_gesture(cx, c, *kind, *intensity),
        Beat::Sit {
            seat_height,
            seat_pos,
        } => plan_sit(cx, c, *seat_height, *seat_pos)?,
        Beat::Stand => plan_stand(cx, c),
        Beat::Idle { secs } => plan_idle(cx, c, *secs),
        Beat::Express { .. } => return Ok(None),
    };
    Ok(Some(plan))
}

// ---------------------------------------------------------------------------------------------
// Motions (the C-1 trait objects)
// ---------------------------------------------------------------------------------------------

fn driver_names(prefix: &str, n: usize) -> Arc<Vec<ChannelTarget>> {
    Arc::new(
        (0..n)
            .map(|i| ChannelTarget::MorphWeight(format!("{prefix}{i}")))
            .collect(),
    )
}

#[derive(Clone)]
struct BeatMotion {
    plan: BeatPlan,
    start: f64,
    names: Arc<Vec<ChannelTarget>>,
}

impl Motion for BeatMotion {
    fn sample(&self, t: Duration) -> MotionPose {
        let tl = t.as_secs_f64() - self.start;
        let h = 1.0e-4;
        let n = self.names.len();
        let mut value = vec![0.0f64; n];
        let mut rate = vec![0.0f64; n];
        if tl <= h {
            // Start of the beat: second-order one-sided difference.
            let t0 = tl.max(0.0);
            let (a, b, c) = (
                self.plan.eval(t0),
                self.plan.eval(t0 + h),
                self.plan.eval(t0 + 2.0 * h),
            );
            for i in 0..n {
                value[i] = a[i];
                rate[i] = (-3.0 * a[i] + 4.0 * b[i] - c[i]) / (2.0 * h);
            }
        } else if tl >= self.plan.dur - h {
            // End of the beat (and the hold after it): backward difference.
            let (a, b, c) = (
                self.plan.eval(tl),
                self.plan.eval(tl - h),
                self.plan.eval(tl - 2.0 * h),
            );
            for i in 0..n {
                value[i] = a[i];
                rate[i] = (3.0 * a[i] - 4.0 * b[i] + c[i]) / (2.0 * h);
            }
        } else {
            let (m, a, b) = (
                self.plan.eval(tl),
                self.plan.eval(tl - h),
                self.plan.eval(tl + h),
            );
            for i in 0..n {
                value[i] = m[i];
                rate[i] = (b[i] - a[i]) / (2.0 * h);
            }
        }
        let tracks = (0..n)
            .map(|i| {
                (
                    self.names[i].clone(),
                    Sample::Scalar {
                        value: value[i] as f32,
                        rate: rate[i] as f32,
                    },
                )
            })
            .collect();
        MotionPose::new(tracks)
    }
}

/// A constant motion over a fixed channel list.
#[derive(Clone)]
struct ConstMotion {
    names: Arc<Vec<ChannelTarget>>,
    values: Vec<f64>,
}

impl Motion for ConstMotion {
    fn sample(&self, _t: Duration) -> MotionPose {
        MotionPose::new(
            self.names
                .iter()
                .zip(self.values.iter())
                .map(|(n, v)| {
                    (
                        n.clone(),
                        Sample::Scalar {
                            value: *v as f32,
                            rate: 0.0,
                        },
                    )
                })
                .collect(),
        )
    }
}

fn secs(t: f64) -> Duration {
    if t.is_finite() && t > 0.0 {
        Duration::from_secs_f64(t)
    } else {
        Duration::ZERO
    }
}

fn read_drivers(p: &MotionPose, n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    for (i, (_, s)) in p.tracks().iter().enumerate().take(n) {
        if let Sample::Scalar { value, .. } = s {
            out[i] = f64::from(*value);
        }
    }
    out
}

fn read_face(p: &MotionPose, names: &[ChannelTarget]) -> Vec<f32> {
    names
        .iter()
        .map(|n| match p.get(n) {
            Some(Sample::Scalar { value, .. }) => *value,
            _ => 0.0,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Performance (offline)
// ---------------------------------------------------------------------------------------------

/// A scripted event on the timeline, for the tape header.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineEvent {
    /// Time in seconds.
    pub time: f64,
    /// Kind (`walk`, `stop`, `look`, `wave`, `acknowledge`, `sit`, `stand`, `idle`, `express`).
    pub kind: &'static str,
    /// Free label (the channel name of an `express`).
    pub label: String,
}

/// A compiled script: a pure function from time to [`Pose`].
pub struct Performance {
    rig: Rig,
    opts: PerformOptions,
    style: Style,
    main: Box<dyn Motion>,
    face: Option<Box<dyn Motion>>,
    face_targets: Vec<ChannelTarget>,
    face_names: Vec<String>,
    duration: f64,
    root0: V,
    walks: Vec<WalkSummary>,
    stance: [Vec<(f64, f64)>; 2],
    events: Vec<TimelineEvent>,
}

fn ease_for(style: &Style, opts: &PerformOptions) -> Duration {
    // Fault injection (mutation control `snap_beats`): beats switch at once.
    if !opts.inertialize || cfg!(figure_mutant = "snap_beats") {
        Duration::ZERO
    } else {
        Duration::from_millis(u64::from(style.settle_ms))
    }
}

impl Performance {
    /// Compile a script against a rig.
    pub fn compile(
        rig: &Rig,
        script: &PerformanceScript,
        opts: &PerformOptions,
    ) -> Result<Performance, PerformError> {
        if script.beats.len() > MAX_BEATS {
            return Err(PerformError::LimitExceeded { what: "beats" });
        }
        let style = if opts.reduced_motion {
            script.style.reduced()
        } else {
            script.style.clone()
        };
        let cx = Ctx {
            rig,
            amp: f64::from(style.amplitude).clamp(0.0, 3.0),
            tempo: f64::from(style.tempo).clamp(0.3, 3.0),
            settle: style.settle_secs(),
            reduced: opts.reduced_motion,
        };
        let mut cut_at: Vec<Option<f64>> = vec![None; script.beats.len()];
        for &(i, s) in &script.cuts {
            if i >= script.beats.len() {
                return Err(PerformError::BadBeat {
                    beat: i,
                    what: "cut of a beat that does not exist",
                });
            }
            if matches!(script.beats[i], Beat::WalkTo { .. }) {
                return Err(PerformError::CannotCutWalk { beat: i });
            }
            if !s.is_finite() || s <= 0.0 {
                return Err(PerformError::BadBeat {
                    beat: i,
                    what: "cut time must be positive",
                });
            }
            cut_at[i] = Some(f64::from(s));
        }
        let mut carry = Carry {
            d: initial_drivers(rig),
            seated: false,
        };
        let root0 = root_of(&carry.d);
        let mut face_names: Vec<String> = Vec::new();
        for b in &script.beats {
            if let Beat::Express { channel, .. } = b {
                if !face_names.contains(channel) {
                    face_names.push(channel.clone());
                }
            }
        }
        let mut plans: Vec<(f64, BeatPlan)> = Vec::new();
        let mut face_events: Vec<(f64, usize, f64)> = Vec::new();
        let mut events: Vec<TimelineEvent> = Vec::new();
        let mut walks: Vec<WalkSummary> = Vec::new();
        let mut swings_global: [Vec<(f64, f64)>; 2] = [Vec::new(), Vec::new()];
        let mut moving_global: Vec<(f64, f64)> = Vec::new();
        let mut cursor = 0.0f64;
        for (i, beat) in script.beats.iter().enumerate() {
            if let Beat::Express { channel, value } = beat {
                let idx = face_names.iter().position(|n| n == channel).unwrap_or(0);
                face_events.push((cursor, idx, f64::from(*value)));
                events.push(TimelineEvent {
                    time: cursor,
                    kind: "express",
                    label: channel.clone(),
                });
                continue;
            }
            let plan = match plan_beat(&cx, &carry, beat) {
                Ok(Some(p)) => p,
                Ok(None) => continue,
                Err(PerformError::BadBeat { what, .. }) => {
                    return Err(PerformError::BadBeat { beat: i, what })
                }
                Err(e) => return Err(e),
            };
            let nominal_end = plan.eval(plan.dur);
            let dur_eff = match cut_at[i] {
                Some(c) => c.min(plan.dur),
                None => plan.dur,
            };
            let kind_name = match beat {
                Beat::WalkTo { .. } => "walk",
                Beat::Stop => "stop",
                Beat::LookAt { .. } => "look",
                Beat::Gesture {
                    kind: GestureKind::Wave,
                    ..
                } => "wave",
                Beat::Gesture { .. } => "acknowledge",
                Beat::Sit { .. } => "sit",
                Beat::Stand => "stand",
                Beat::Idle { .. } => "idle",
                Beat::Express { .. } => "express",
            };
            events.push(TimelineEvent {
                time: cursor,
                kind: kind_name,
                label: String::new(),
            });
            match &plan.kind {
                Kind::Walk(w) => {
                    for sw in &w.swings {
                        swings_global[sw.foot].push((cursor + sw.t0, cursor + sw.t1));
                    }
                    walks.push(WalkSummary {
                        start: cursor,
                        end: cursor + plan.dur,
                        distance: (w.v * w.tau * w.n as f64) as f32,
                        stride: (w.v * w.tau) as f32,
                        steps: w.n,
                    });
                }
                Kind::Dissolve(_) => moving_global.push((cursor, cursor + plan.dur)),
                _ => {}
            }
            if matches!(beat, Beat::Sit { .. }) {
                carry.seated = true;
            }
            if matches!(beat, Beat::Stand) {
                carry.seated = false;
            }
            carry.d = nominal_end;
            plans.push((cursor, plan));
            cursor += dur_eff;
            if cursor > MAX_SECONDS {
                return Err(PerformError::LimitExceeded { what: "duration" });
            }
        }
        let duration = if plans.is_empty() && face_events.is_empty() {
            0.5
        } else {
            cursor + cx.settle
        };

        // Main chain: beat 0, then each next beat preempts the chain so far.
        let names = driver_names("d", N_BASE);
        let ease = ease_for(&style, opts);
        let mut it = plans.into_iter();
        let mut main: Box<dyn Motion> = match it.next() {
            Some((start, plan)) => Box::new(BeatMotion {
                plan,
                start,
                names: names.clone(),
            }),
            None => Box::new(ConstMotion {
                names: names.clone(),
                values: initial_drivers(rig),
            }),
        };
        for (start, plan) in it {
            let next = BeatMotion {
                plan,
                start,
                names: names.clone(),
            };
            main = Box::new(Preempt::new(main, Box::new(next), secs(start), ease));
        }

        // Face chain.
        let face_targets: Vec<ChannelTarget> = face_names
            .iter()
            .map(|n| ChannelTarget::MorphWeight(format!("f:{n}")))
            .collect();
        let face = if face_names.is_empty() {
            None
        } else {
            let fnames = Arc::new(face_targets.clone());
            let mut vals = vec![0.0f64; face_names.len()];
            let mut chain: Box<dyn Motion> = Box::new(ConstMotion {
                names: fnames.clone(),
                values: vals.clone(),
            });
            let face_ease = Duration::from_millis(u64::from(style.settle_ms));
            for (t, ch, v) in face_events {
                vals[ch] = v;
                let next = ConstMotion {
                    names: fnames.clone(),
                    values: vals.clone(),
                };
                chain = Box::new(Preempt::new(chain, Box::new(next), secs(t), face_ease));
            }
            Some(chain)
        };

        // Stance windows: the complement of the swing (or dissolve) intervals.
        let mut stance: [Vec<(f64, f64)>; 2] = [Vec::new(), Vec::new()];
        for f in 0..2 {
            let mut moving = swings_global[f].clone();
            moving.extend(moving_global.iter().copied());
            moving.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut t = 0.0;
            for (a, b) in moving {
                if a > t {
                    stance[f].push((t, a));
                }
                if b > t {
                    t = b;
                }
            }
            if duration > t {
                stance[f].push((t, duration));
            }
        }

        Ok(Performance {
            rig: rig.clone(),
            opts: *opts,
            style,
            main,
            face,
            face_targets,
            face_names,
            duration,
            root0,
            walks,
            stance,
            events,
        })
    }

    /// Length in seconds (the last beat plus one settle).
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// The rig.
    pub fn rig(&self) -> &Rig {
        &self.rig
    }

    /// The style actually played (reduced motion already applied).
    pub fn style(&self) -> &Style {
        &self.style
    }

    /// Face channel names, in pose order.
    pub fn face_names(&self) -> &[String] {
        &self.face_names
    }

    /// One summary per `WalkTo`.
    pub fn walks(&self) -> &[WalkSummary] {
        &self.walks
    }

    /// Timeline events.
    pub fn events(&self) -> &[TimelineEvent] {
        &self.events
    }

    /// The `(start, end)` seconds during which foot `foot` (0 left, 1 right) is planted: its IK
    /// target is world-fixed.
    pub fn stance_windows(&self, foot: usize) -> &[(f64, f64)] {
        &self.stance[foot.min(1)]
    }

    /// The driver vector at `t` seconds.
    fn drivers_at(&self, t: f64) -> (Vec<f64>, Vec<f32>) {
        let at = secs(t);
        let mut d = read_drivers(&self.main.sample(at), N_BASE);
        // Fault injection (mutation control `no_root_motion`): the figure walks in place.
        if !self.opts.root_motion || cfg!(figure_mutant = "no_root_motion") {
            let mut shift = [0.0; 3];
            for c in 0..3 {
                shift[c] = d[c] - self.root0[c];
                d[c] = self.root0[c];
            }
            for f in 0..2 {
                for c in 0..3 {
                    d[D_FOOT + f * 6 + c] -= shift[c];
                }
            }
        }
        let face = match &self.face {
            Some(chain) => read_face(&chain.sample(at), &self.face_targets),
            None => Vec::new(),
        };
        (d, face)
    }

    /// The pose at `t` seconds (held before 0 and after the end).
    pub fn pose_at(&self, t: f64) -> Pose {
        let (d, face) = self.drivers_at(t);
        // Fault injection (mutation control `contact_ignored`): feet are never locked.
        let lock = self.opts.foot_lock && !cfg!(figure_mutant = "contact_ignored");
        let mut p = self.rig.solve(&d, lock, self.opts.arm_clearance);
        p.face = face;
        p
    }

    /// Frame count at `fps`.
    pub fn frame_count(&self, fps: u32) -> usize {
        (self.duration * f64::from(fps)).floor() as usize + 1
    }
}

// ---------------------------------------------------------------------------------------------
// Tape
// ---------------------------------------------------------------------------------------------

/// An event of the tape header.
#[derive(Debug, Clone, PartialEq)]
pub struct TapeEvent {
    /// Frame index.
    pub frame: u32,
    /// Kind.
    pub kind: String,
    /// Label.
    pub label: String,
}

/// Identity and presentation fields only the caller knows; written into the header verbatim.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TapeIdentity {
    /// Hex body revision id.
    pub body_rev_id: String,
    /// Hex fit id.
    pub fit_id: String,
    /// Hex present id.
    pub present_id: String,
    /// Hex sha256 of the asset manifest.
    pub asset_manifest_sha256: String,
    /// Controls in exact recipe order.
    pub controls: Vec<(String, f32)>,
    /// `(garment id, fit verdict)`.
    pub garments: Vec<(String, String)>,
    /// A JSON object text for `materials` (empty means `{}`). Written verbatim: the caller makes
    /// sure it is valid JSON.
    pub materials_json: String,
    /// `(frame, camera id)`.
    pub camera_cuts: Vec<(u32, String)>,
}

/// The two files of a tape.
#[derive(Debug, Clone, PartialEq)]
pub struct TapeFiles {
    /// Contents of `<name>.tape.json`.
    pub header_json: String,
    /// Contents of `<name>.tape.bin`.
    pub bin: Vec<u8>,
}

/// `gibson.figure.tape.v1`: per-frame root translation and joint locals.
#[derive(Debug, Clone, PartialEq)]
pub struct Tape {
    /// Frames per second.
    pub fps: u32,
    /// Number of frames.
    pub frame_count: usize,
    /// Joint names, skeleton order.
    pub joints: Vec<String>,
    /// Face channel names.
    pub face: Vec<String>,
    /// Events.
    pub events: Vec<TapeEvent>,
    /// The style played.
    pub style: Style,
    data: Vec<f32>,
}

impl Tape {
    /// Floats per frame: `3 + 10 * joints + face`.
    pub fn stride(&self) -> usize {
        3 + 10 * self.joints.len() + self.face.len()
    }

    /// The floats of frame `i`.
    pub fn frame(&self, i: usize) -> &[f32] {
        let s = self.stride();
        &self.data[i * s..(i + 1) * s]
    }

    /// Root translation of frame `i`.
    pub fn root(&self, i: usize) -> Vec3 {
        let f = self.frame(i);
        Vec3::new(f[0], f[1], f[2])
    }

    /// Local transforms of frame `i`, in joint order.
    pub fn locals(&self, i: usize) -> Vec<Transform> {
        let f = self.frame(i);
        (0..self.joints.len())
            .map(|j| {
                let o = 3 + j * 10;
                Transform::new(
                    Vec3::new(f[o], f[o + 1], f[o + 2]),
                    Quat::new(f[o + 3], f[o + 4], f[o + 5], f[o + 6]),
                    Vec3::new(f[o + 7], f[o + 8], f[o + 9]),
                )
            })
            .collect()
    }

    /// The body bytes: every float little-endian, frame after frame, no padding.
    pub fn bin_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len() * 4);
        for v in &self.data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Lowercase hex sha256 of [`Tape::bin_bytes`].
    pub fn bin_sha256(&self) -> String {
        hex(&sha256::digest(&self.bin_bytes()))
    }

    /// Write the header (hand-built JSON, fixed key order and number formatting) and the body.
    /// The same tape and identity always give the same bytes.
    pub fn to_files(&self, id: &TapeIdentity) -> TapeFiles {
        let bin = self.bin_bytes();
        let mut j = String::new();
        j.push_str("{\n");
        j.push_str("\"format\":\"gibson.figure.tape.v1\",\n");
        field_str(&mut j, "body_rev_id", &id.body_rev_id);
        field_str(&mut j, "fit_id", &id.fit_id);
        field_str(&mut j, "present_id", &id.present_id);
        field_str(&mut j, "asset_manifest_sha256", &id.asset_manifest_sha256);
        j.push_str("\"controls\":{");
        for (i, (k, v)) in id.controls.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            json_str(&mut j, k);
            j.push(':');
            j.push_str(&json_num(*v));
        }
        j.push_str("},\n\"garments\":[");
        for (i, (g, _)) in id.garments.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            json_str(&mut j, g);
        }
        j.push_str("],\n\"fit_status\":{");
        for (i, (g, v)) in id.garments.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            json_str(&mut j, g);
            j.push(':');
            json_str(&mut j, v);
        }
        j.push_str("},\n\"materials\":");
        if id.materials_json.trim().is_empty() {
            j.push_str("{}");
        } else {
            j.push_str(id.materials_json.trim());
        }
        j.push_str(",\n\"style\":{\"preset\":");
        json_str(&mut j, &self.style.preset);
        j.push_str(",\"amplitude\":");
        j.push_str(&json_num(self.style.amplitude));
        j.push_str(",\"tempo\":");
        j.push_str(&json_num(self.style.tempo));
        j.push_str(",\"settle_ms\":");
        j.push_str(&self.style.settle_ms.to_string());
        j.push_str("},\n\"fps\":");
        j.push_str(&self.fps.to_string());
        j.push_str(",\n\"frame_count\":");
        j.push_str(&self.frame_count.to_string());
        j.push_str(",\n\"joints\":[");
        for (i, n) in self.joints.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            json_str(&mut j, n);
        }
        j.push_str(
            "],\n\"channels\":[\"root_t3\",\"local_t3\",\"local_r4\",\"local_s3\"],\n\"face\":[",
        );
        for (i, n) in self.face.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            json_str(&mut j, n);
        }
        j.push_str("],\n\"events\":[");
        for (i, e) in self.events.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            j.push_str("{\"frame\":");
            j.push_str(&e.frame.to_string());
            j.push_str(",\"kind\":");
            json_str(&mut j, &e.kind);
            j.push_str(",\"label\":");
            json_str(&mut j, &e.label);
            j.push('}');
        }
        j.push_str("],\n\"camera_cuts\":[");
        for (i, (fr, cam)) in id.camera_cuts.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            j.push_str("{\"frame\":");
            j.push_str(&fr.to_string());
            j.push_str(",\"camera_id\":");
            json_str(&mut j, cam);
            j.push('}');
        }
        j.push_str("],\n\"bin_sha256\":");
        json_str(&mut j, &hex(&sha256::digest(&bin)));
        j.push_str("\n}\n");
        TapeFiles {
            header_json: j,
            bin,
        }
    }
}

fn hex(b: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push(H[(x >> 4) as usize] as char);
        s.push(H[(x & 15) as usize] as char);
    }
    s
}

fn json_str(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_num(v: f32) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "0".to_string()
    }
}

fn field_str(out: &mut String, key: &str, val: &str) {
    json_str(out, key);
    out.push(':');
    json_str(out, val);
    out.push_str(",\n");
}

/// Evaluate a script at `fps` into a tape, with default options.
pub fn perform(rig: &Rig, script: &PerformanceScript, fps: u32) -> Result<Tape, PerformError> {
    perform_with(rig, script, fps, &PerformOptions::default())
}

/// Evaluate a script at `fps` into a tape. Frame `i` is the pose at `i / fps` seconds.
pub fn perform_with(
    rig: &Rig,
    script: &PerformanceScript,
    fps: u32,
    opts: &PerformOptions,
) -> Result<Tape, PerformError> {
    if !(1..=240).contains(&fps) {
        return Err(PerformError::BadFps);
    }
    let perf = Performance::compile(rig, script, opts)?;
    let frames = perf.frame_count(fps);
    if frames > MAX_FRAMES {
        return Err(PerformError::LimitExceeded { what: "frames" });
    }
    let nj = rig.skeleton.joint_count();
    let nf = perf.face_names.len();
    let stride = 3 + 10 * nj + nf;
    let mut data: Vec<f32> = Vec::with_capacity(frames * stride);
    let mut prev_q: Vec<Option<Quat>> = vec![None; nj];
    for i in 0..frames {
        let t = i as f64 / f64::from(fps);
        let pose = perf.pose_at(t);
        data.extend_from_slice(&[pose.root_t.x, pose.root_t.y, pose.root_t.z]);
        for (j, l) in pose.locals.iter().enumerate() {
            let mut q = l.rotation;
            if let Some(p) = prev_q[j] {
                if q.dot(p) < 0.0 {
                    q = q.neg();
                }
            }
            prev_q[j] = Some(q);
            data.extend_from_slice(&[
                l.translation.x,
                l.translation.y,
                l.translation.z,
                q.x,
                q.y,
                q.z,
                q.w,
                l.scale.x,
                l.scale.y,
                l.scale.z,
            ]);
        }
        data.extend_from_slice(&pose.face);
    }
    // A tape reader refuses NaN and infinity anywhere in the body, so refuse to write one.
    if data.iter().any(|v| !v.is_finite()) {
        return Err(PerformError::NonFinite);
    }
    // An event frame is an index into the tape: rounding must not land one past the last frame.
    let last_frame = (frames - 1) as u32;
    let events = perf
        .events
        .iter()
        .map(|e| TapeEvent {
            frame: ((e.time * f64::from(fps)).round() as u32).min(last_frame),
            kind: e.kind.to_string(),
            label: e.label.clone(),
        })
        .collect();
    Ok(Tape {
        fps,
        frame_count: frames,
        joints: rig
            .skeleton
            .joints()
            .iter()
            .map(|j| j.name.clone())
            .collect(),
        face: perf.face_names.clone(),
        events,
        style: perf.style.clone(),
        data,
    })
}

// ---------------------------------------------------------------------------------------------
// Live
// ---------------------------------------------------------------------------------------------

/// The closed set of live intents.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum IntentKind {
    /// Gaze at `target`.
    Look,
    /// Nod.
    Acknowledge,
    /// Wave.
    Wave,
    /// Sit: `target` is the pelvis position, its up component the seat height.
    Sit,
    /// Stand.
    Stand,
    /// Walk to `target` at `intensity` metres per second.
    WalkTo,
    /// Hold with micro motion.
    Idle,
    /// Ease a face channel to `intensity`.
    Express {
        /// Channel name.
        channel: String,
    },
}

/// A request to the live evaluator.
#[derive(Debug, Clone, PartialEq)]
pub struct Intent {
    /// What.
    pub kind: IntentKind,
    /// Strength (gesture intensity, walk speed, face value).
    pub intensity: f32,
    /// World target, where the kind needs one.
    pub target: Option<Vec3>,
    /// Blend-in duration in ms; 0 uses the style's settle.
    pub ease_ms: u32,
    /// Higher preempts lower; only non-walk beats are preempted.
    pub priority: u8,
    /// Absolute time (ms) after which the intent eases out; a queued intent is dropped.
    pub expires_ms: Option<u64>,
}

impl Intent {
    /// An intent of `kind` with intensity 1, priority 1 and no expiry.
    pub fn new(kind: IntentKind) -> Intent {
        Intent {
            kind,
            intensity: 1.0,
            target: None,
            ease_ms: 0,
            priority: 1,
            expires_ms: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Running {
    end_ms: u64,
    priority: u8,
    is_walk: bool,
    expires_ms: Option<u64>,
}

#[derive(Clone, Debug)]
struct Queued {
    beat: Beat,
    ease_ms: u32,
    priority: u8,
    expires_ms: Option<u64>,
}

/// Incremental interactive evaluator. Every `submit` blends from the current evaluated pose with
/// value and velocity matched (the same inertialization as an offline beat boundary).
///
/// Walks are not preempted mid-stride: an intent submitted during a walk waits for it to plant.
/// [`Live::stop`] is the immediate path.
pub struct Live {
    rig: Rig,
    base_style: Style,
    opts: PerformOptions,
    reduced: bool,
    carry: Carry,
    names: Arc<Vec<ChannelTarget>>,
    main: Box<dyn Motion>,
    face: Option<Box<dyn Motion>>,
    face_vals: Vec<f64>,
    face_names: Vec<String>,
    face_targets: Vec<ChannelTarget>,
    running: Option<Running>,
    queue: Vec<Queued>,
    last_ms: u64,
    last_drivers: Vec<f64>,
    last_pose: Pose,
    quiescent: bool,
    stopped: bool,
    work: u64,
}

impl Live {
    /// A figure standing at the rest pose with its feet planted.
    pub fn new(rig: &Rig, style: Style, opts: PerformOptions) -> Live {
        let names = driver_names("d", N_BASE);
        let d0 = initial_drivers(rig);
        let pose = rig.solve(&d0, opts.foot_lock, opts.arm_clearance);
        Live {
            rig: rig.clone(),
            base_style: style,
            opts,
            reduced: opts.reduced_motion,
            carry: Carry {
                d: d0.clone(),
                seated: false,
            },
            names: names.clone(),
            main: Box::new(ConstMotion {
                names,
                values: d0.clone(),
            }),
            face: None,
            face_vals: Vec::new(),
            face_names: Vec::new(),
            face_targets: Vec::new(),
            running: None,
            queue: Vec::new(),
            last_ms: 0,
            last_drivers: d0,
            last_pose: pose,
            quiescent: false,
            stopped: false,
            work: 0,
        }
    }

    fn style(&self) -> Style {
        if self.reduced {
            self.base_style.reduced()
        } else {
            self.base_style.clone()
        }
    }

    /// Number of pose evaluations so far (`tick` on a quiescent or stopped figure adds none).
    pub fn work_count(&self) -> u64 {
        self.work
    }

    /// True after [`Live::quiesce`] until the next `submit`.
    pub fn is_quiescent(&self) -> bool {
        self.quiescent
    }

    /// True after [`Live::stop`] until the next `submit`.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Face channel names, in `Pose::face` order.
    pub fn face_names(&self) -> &[String] {
        &self.face_names
    }

    /// The last evaluated pose.
    pub fn pose(&self) -> &Pose {
        &self.last_pose
    }

    /// Reduced motion: amplitude is clamped from the next beat on and locomotion becomes a
    /// cross-dissolve. Turning it on also eases any running non-walk beat out.
    pub fn set_reduced_motion(&mut self, on: bool) {
        if self.reduced == on {
            return;
        }
        self.reduced = on;
        if on {
            if let Some(r) = self.running {
                if !r.is_walk && self.last_ms < r.end_ms {
                    let at = self.last_ms;
                    self.begin(Beat::Idle { secs: 1.0 }, 0, 255, None, at);
                }
            }
        }
    }

    /// Hidden presentation: `tick` returns the held pose and evaluates nothing until the next
    /// `submit`.
    pub fn quiesce(&mut self) {
        self.quiescent = true;
    }

    /// Immediate stop: clears the queue and holds the last evaluated pose; `tick` evaluates
    /// nothing until the next `submit`.
    pub fn stop(&mut self) {
        self.stopped = true;
        self.queue.clear();
        self.running = None;
        self.carry.d = self.last_drivers.clone();
        self.main = Box::new(ConstMotion {
            names: self.names.clone(),
            values: self.last_drivers.clone(),
        });
    }

    fn beat_of(&self, intent: &Intent) -> Result<Beat, PerformError> {
        let need = |what: &'static str| -> Result<Vec3, PerformError> {
            intent.target.ok_or(PerformError::BadBeat { beat: 0, what })
        };
        Ok(match &intent.kind {
            IntentKind::Look => Beat::LookAt {
                target: need("look needs a target")?,
            },
            IntentKind::Acknowledge => Beat::Gesture {
                kind: GestureKind::Acknowledge,
                intensity: intent.intensity,
            },
            IntentKind::Wave => Beat::Gesture {
                kind: GestureKind::Wave,
                intensity: intent.intensity,
            },
            IntentKind::Sit => {
                let t = need("sit needs a target")?;
                let h = v64(t);
                let height = vdot(h, self.rig.up64);
                let planar = vsub(h, vscale(self.rig.up64, height));
                Beat::Sit {
                    seat_height: height as f32,
                    seat_pos: v32(planar),
                }
            }
            IntentKind::Stand => Beat::Stand,
            IntentKind::WalkTo => {
                let speed = if intent.intensity > 0.0 {
                    intent.intensity
                } else {
                    1.0
                };
                Beat::WalkTo {
                    target: need("walk needs a target")?,
                    stride: 0.45 + 0.1 * speed.clamp(0.5, 2.0),
                    speed,
                }
            }
            IntentKind::Idle => Beat::Idle { secs: 4.0 },
            IntentKind::Express { channel } => Beat::Express {
                channel: channel.clone(),
                value: intent.intensity,
            },
        })
    }

    /// Submit an intent at presentation time `now_ms`. A higher-priority intent preempts a
    /// running non-walk beat immediately (inertialized); otherwise it queues. An error means the
    /// intent was malformed and nothing changed.
    pub fn submit(&mut self, intent: Intent, now_ms: u64) -> Result<(), PerformError> {
        let beat = self.beat_of(&intent)?;
        self.quiescent = false;
        self.stopped = false;
        let now = now_ms.max(self.last_ms);
        self.pump(now);
        if let Beat::Express { channel, value } = &beat {
            let ease = if intent.ease_ms > 0 {
                intent.ease_ms
            } else {
                self.style().settle_ms
            };
            self.express(channel, f64::from(*value), now, ease);
            return Ok(());
        }
        let active = match self.running {
            Some(r) if now < r.end_ms => Some(r),
            _ => None,
        };
        let preempt = match active {
            Some(r) => !r.is_walk && intent.priority > r.priority,
            None => true,
        };
        if preempt {
            // Validate against the current carry before changing anything.
            let style = self.style();
            let cx = Ctx {
                rig: &self.rig,
                amp: f64::from(style.amplitude).clamp(0.0, 3.0),
                tempo: f64::from(style.tempo).clamp(0.3, 3.0),
                settle: style.settle_secs(),
                reduced: self.reduced,
            };
            plan_beat(&cx, &self.carry, &beat)?;
            self.begin(
                beat,
                intent.ease_ms,
                intent.priority,
                intent.expires_ms,
                now,
            );
            Ok(())
        } else {
            if self.queue.len() >= MAX_QUEUED {
                return Err(PerformError::QueueFull);
            }
            self.queue.push(Queued {
                beat,
                ease_ms: intent.ease_ms,
                priority: intent.priority,
                expires_ms: intent.expires_ms,
            });
            Ok(())
        }
    }

    fn express(&mut self, channel: &str, value: f64, now: u64, ease_ms: u32) {
        let idx = match self.face_names.iter().position(|n| n == channel) {
            Some(i) => i,
            None => {
                self.face_names.push(channel.to_string());
                self.face_vals.push(0.0);
                self.face_targets
                    .push(ChannelTarget::MorphWeight(format!("f:{channel}")));
                self.face_names.len() - 1
            }
        };
        let fnames = Arc::new(self.face_targets.clone());
        let old: Box<dyn Motion> = match self.face.take() {
            Some(c) => c,
            None => Box::new(ConstMotion {
                names: fnames.clone(),
                values: vec![0.0; self.face_names.len()],
            }),
        };
        self.face_vals[idx] = value;
        let next = ConstMotion {
            names: fnames,
            values: self.face_vals.clone(),
        };
        self.face = Some(Box::new(Preempt::new(
            old,
            Box::new(next),
            Duration::from_millis(now),
            Duration::from_millis(u64::from(ease_ms)),
        )));
    }

    /// Start `beat` at `start_ms`, preempting the chain with inertialization. A beat that does not
    /// compile against the current posture is dropped.
    fn begin(
        &mut self,
        beat: Beat,
        ease_ms: u32,
        priority: u8,
        expires: Option<u64>,
        start_ms: u64,
    ) {
        let style = self.style();
        let cx = Ctx {
            rig: &self.rig,
            amp: f64::from(style.amplitude).clamp(0.0, 3.0),
            tempo: f64::from(style.tempo).clamp(0.3, 3.0),
            settle: style.settle_secs(),
            reduced: self.reduced,
        };
        let plan = match plan_beat(&cx, &self.carry, &beat) {
            Ok(Some(p)) => p,
            _ => return,
        };
        match beat {
            Beat::Sit { .. } => self.carry.seated = true,
            Beat::Stand => self.carry.seated = false,
            _ => {}
        }
        let is_walk = matches!(plan.kind, Kind::Walk(_) | Kind::Dissolve(_));
        let end_ms = start_ms + (plan.dur * 1000.0).ceil() as u64;
        self.carry.d = plan.eval(plan.dur);
        let ease = if !self.opts.inertialize || cfg!(figure_mutant = "snap_beats") {
            Duration::ZERO
        } else if ease_ms > 0 {
            Duration::from_millis(u64::from(ease_ms))
        } else {
            Duration::from_millis(u64::from(style.settle_ms))
        };
        let next = BeatMotion {
            plan,
            start: start_ms as f64 / 1000.0,
            names: self.names.clone(),
        };
        let old = std::mem::replace(
            &mut self.main,
            Box::new(ConstMotion {
                names: self.names.clone(),
                values: Vec::new(),
            }),
        );
        self.main = Box::new(Preempt::new(
            old,
            Box::new(next),
            Duration::from_millis(start_ms),
            ease,
        ));
        self.running = Some(Running {
            end_ms,
            priority,
            is_walk,
            expires_ms: expires,
        });
    }

    fn pump(&mut self, now: u64) {
        if let Some(r) = self.running {
            if let Some(exp) = r.expires_ms {
                if now >= exp && now < r.end_ms && !r.is_walk {
                    let at = exp.max(self.last_ms);
                    self.begin(Beat::Idle { secs: 1.0 }, 0, 0, None, at);
                }
            }
        }
        loop {
            let active = matches!(self.running, Some(r) if now < r.end_ms);
            if active || self.queue.is_empty() {
                break;
            }
            let start = self
                .running
                .map(|r| r.end_ms)
                .unwrap_or(now)
                .max(self.last_ms);
            let mut best = 0usize;
            for (i, q) in self.queue.iter().enumerate() {
                if q.priority > self.queue[best].priority {
                    best = i;
                }
            }
            let q = self.queue.remove(best);
            if let Some(exp) = q.expires_ms {
                if start >= exp {
                    continue;
                }
            }
            self.begin(q.beat, q.ease_ms, q.priority, q.expires_ms, start);
            if self.running.map(|r| r.end_ms <= start).unwrap_or(true) {
                // The beat was dropped (it did not compile); `running` still holds the old one.
                self.running = None;
            }
        }
    }

    /// Evaluate the pose at `now_ms`. A quiescent or stopped figure returns the held pose and
    /// does no work.
    pub fn tick(&mut self, now_ms: u64) -> Pose {
        if self.quiescent || self.stopped {
            return self.last_pose.clone();
        }
        let now = now_ms.max(self.last_ms);
        self.pump(now);
        self.last_ms = now;
        let at = Duration::from_millis(now);
        let d = read_drivers(&self.main.sample(at), N_BASE);
        let face = match &self.face {
            Some(c) => read_face(&c.sample(at), &self.face_targets),
            None => Vec::new(),
        };
        let lock = self.opts.foot_lock && !cfg!(figure_mutant = "contact_ignored");
        let mut p = self.rig.solve(&d, lock, self.opts.arm_clearance);
        p.face = face;
        self.last_drivers = d;
        self.last_pose = p.clone();
        self.work += 1;
        p
    }
}
