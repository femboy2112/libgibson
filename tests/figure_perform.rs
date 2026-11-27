//! M5-C performance: analytic two-bone IK, the scripted performance (walk with world-locked
//! feet, gaze, wave, sit and stand), the tape, the `Live` evaluator, and an independent loop-seam
//! probe.
//!
//! WHAT THESE TESTS ASSERT. Owner ruling 17 ("model continuous motion and not just jarring stop
//! motion") governs the continuity assertions. They use the C-1 second-difference metric
//! `J = |p(t+h) - 2 p(t) + p(t-h)| / h` at `h = 1 ms` on WORLD joint positions. For a C1 path with
//! acceleration at most `A`, `J <= A h` plus roundoff; a velocity step of `v` shows up as `v` and a
//! position jump of `d` as `d / h`. The bound is DECLARED from the kinematic model, never from
//! observed output (`docs/evidence/m5c/model_perform.py` prints the worst foot acceleration of the
//! planned drivers, 36.7 m/s^2; the declared `A = 60 m/s^2` leaves room for the IK map):
//! `J_BOUND = A h + 4 e / h = 0.06 + 0.004 ~ 0.08`, with `e = 1e-6 m` the declared f32 error of a
//! world position assembled from a chain of ten matrix products.
//!
//! Every guarantee has a CONTROL that must fail (the guarantee switched off through
//! `PerformOptions`, or a deliberately broken construction). The controls prove the measurement
//! can see the defect; they run in the same build as the assertions.
//!
//! Mutation controls (`RUSTFLAGS='--cfg figure_mutant="..."'`) are caught by ordinary behavioural
//! assertions (no golden, no cfg inspection): `contact_ignored` (feet never locked),
//! `snap_beats` (no inertialization at beat boundaries), `no_root_motion` (the root never
//! translates). The prediction is `docs/evidence/m5c/KILL_MATRIX_PERFORM_PREDICTED.md`.
//!
//! The synthetic rig is Z-up and faces +Y (so the character's right is +X), the opposite handedness
//! of the exported MPFB rig on purpose, so a sign error in the basis handling cannot hide.
#![cfg(feature = "figure")]
#![allow(clippy::excessive_precision)]
#![allow(clippy::needless_range_loop)]
#![allow(dead_code)]

use std::time::Duration;

use gibson::figure::clip::{Channel, ChannelTarget, Clip, Keys};
use gibson::figure::ik::{self, LimbInput};
use gibson::figure::math::{Quat, Transform, Vec3};
use gibson::figure::motion::{Interpolation, Motion, MotionClip, MotionPose, Repeat, Sample};
use gibson::figure::perform::{
    perform, Beat, GestureKind, Intent, IntentKind, Live, PerformError, PerformOptions,
    Performance, PerformanceScript, Pose, Rig, RigGeometry, RigMap, RigRoles, Style, TapeIdentity,
};
use gibson::figure::sha256;
use gibson::figure::skeleton::{Joint, Skeleton};

/// Declared bound of the world-position second difference at h = 1 ms (see the module docs).
const J_BOUND: f64 = 0.08;
const H: f64 = 1.0e-3;

// Joint indices of the synthetic rig.
const PELVIS: usize = 1;
const SPINE3: usize = 4;
const HEAD: usize = 6;
const HAND_L: usize = 11;
const HAND_R: usize = 14;
const FOOT_L: usize = 17;
const FOOT_R: usize = 20;
const FEET: [usize; 2] = [FOOT_L, FOOT_R];
const PROBES: [usize; 7] = [PELVIS, SPINE3, HEAD, HAND_L, HAND_R, FOOT_L, FOOT_R];

fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    f64::from(a.sub(b).length())
}

fn joint(name: &str, parent: Option<usize>, t: [f32; 3]) -> Joint {
    Joint::new(
        name,
        parent,
        Transform::from_translation(Vec3::new(t[0], t[1], t[2])),
    )
}

/// 21 joints; legs 0.46011 + 0.46011 m, hips at 1.0 m, ankles at 0.08 m; arms bent at the elbow;
/// eyes under the head. Z up, +Y forward, right = +X.
fn synthetic_rig() -> Rig {
    let joints = vec![
        joint("root", None, [0.0, 0.0, 0.0]),
        joint("pelvis", Some(0), [0.0, 0.0, 1.0]),
        joint("spine1", Some(1), [0.0, 0.0, 0.12]),
        joint("spine2", Some(2), [0.0, 0.0, 0.14]),
        joint("spine3", Some(3), [0.0, 0.0, 0.16]),
        joint("neck", Some(4), [0.0, 0.0, 0.10]),
        joint("head", Some(5), [0.0, 0.0, 0.08]),
        joint("eye_l", Some(6), [-0.03, 0.09, 0.07]),
        joint("eye_r", Some(6), [0.03, 0.09, 0.07]),
        joint("upperarm_l", Some(4), [-0.19, 0.0, 0.05]),
        joint("forearm_l", Some(9), [0.0, -0.05, -0.27]),
        joint("hand_l", Some(10), [0.0, 0.10, -0.22]),
        joint("upperarm_r", Some(4), [0.19, 0.0, 0.05]),
        joint("forearm_r", Some(12), [0.0, -0.05, -0.27]),
        joint("hand_r", Some(13), [0.0, 0.10, -0.22]),
        joint("thigh_l", Some(1), [-0.09, 0.0, 0.0]),
        joint("calf_l", Some(15), [0.0, 0.01, -0.46]),
        joint("foot_l", Some(16), [0.0, -0.01, -0.46]),
        joint("thigh_r", Some(1), [0.09, 0.0, 0.0]),
        joint("calf_r", Some(18), [0.0, 0.01, -0.46]),
        joint("foot_r", Some(19), [0.0, -0.01, -0.46]),
    ];
    let skel = Skeleton::new(joints, Vec::new()).expect("synthetic skeleton is valid");
    let roles = RigRoles {
        pelvis: "pelvis",
        spine: &["spine1", "spine2", "spine3"],
        neck: Some("neck"),
        head: "head",
        eyes: &["eye_l", "eye_r"],
        arm_l: ["upperarm_l", "forearm_l", "hand_l"],
        arm_r: ["upperarm_r", "forearm_r", "hand_r"],
        leg_l: ["thigh_l", "calf_l", "foot_l"],
        leg_r: ["thigh_r", "calf_r", "foot_r"],
    };
    let geo = RigGeometry::new(v3(0.0, 0.0, 1.0), v3(0.0, 1.0, 0.0));
    let map = RigMap::from_names(&skel, &roles, geo).expect("roles resolve");
    Rig::new(skel, map).expect("rig builds")
}

fn compile(rig: &Rig, script: &PerformanceScript, opts: &PerformOptions) -> Performance {
    Performance::compile(rig, script, opts).expect("script compiles")
}

fn script(style: Style, beats: Vec<Beat>) -> PerformanceScript {
    let mut s = PerformanceScript::new(style);
    s.beats = beats;
    s
}

fn walk(x: f32, y: f32, stride: f32, speed: f32) -> Beat {
    Beat::WalkTo {
        target: v3(x, y, 0.0),
        stride,
        speed,
    }
}

fn joint_pos(rig: &Rig, perf: &Performance, t: f64, j: usize) -> Vec3 {
    rig.joint_world(&perf.pose_at(t))[j]
}

/// Largest `J` of the probe joints over `times`.
fn max_j(rig: &Rig, perf: &Performance, times: &[f64]) -> (f64, f64) {
    let mut worst = (0.0f64, 0.0f64);
    for &t in times {
        if t - H < 0.0 || t + H > perf.duration() {
            continue;
        }
        let a = rig.joint_world(&perf.pose_at(t - H));
        let b = rig.joint_world(&perf.pose_at(t));
        let c = rig.joint_world(&perf.pose_at(t + H));
        for &j in &PROBES {
            let d = a[j].sub(b[j].scale(2.0)).add(c[j]);
            let jj = f64::from(d.length()) / H;
            if jj > worst.0 {
                worst = (jj, t);
            }
        }
    }
    worst
}

/// Dense times (1 ms) in `[t0 - w, t0 + w]`.
fn window(t0: f64, w: f64) -> Vec<f64> {
    let n = (2.0 * w / H).round() as usize;
    (0..=n).map(|i| t0 - w + i as f64 * H).collect()
}

fn event_time(perf: &Performance, kind: &str) -> f64 {
    perf.events()
        .iter()
        .find(|e| e.kind == kind)
        .map(|e| e.time)
        .unwrap_or_else(|| panic!("no `{kind}` event"))
}

// ---------------------------------------------------------------------------------------------
// IK
// ---------------------------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
    }
    fn unit(&mut self) -> Vec3 {
        loop {
            let v = v3(
                (self.next() * 2.0 - 1.0) as f32,
                (self.next() * 2.0 - 1.0) as f32,
                (self.next() * 2.0 - 1.0) as f32,
            );
            let l = v.length();
            if l > 0.2 && l <= 1.0 {
                return v.scale(1.0 / l);
            }
        }
    }
    fn quat(&mut self) -> Quat {
        let a = self.unit();
        Quat::from_axis_angle(a, (self.next() * 6.0) as f32)
    }
}

const L1: f32 = 0.460_108_7; // sqrt(0.01^2 + 0.46^2)

fn limb(target: Vec3, pole: Vec3, soft: f32) -> LimbInput {
    LimbInput {
        parent_world: Quat::IDENTITY,
        upper_rest_rot: Quat::IDENTITY,
        lower_rest_rot: Quat::IDENTITY,
        upper_to_lower: v3(0.0, 0.01, -0.46),
        lower_to_end: v3(0.0, -0.01, -0.46),
        root: v3(0.0, 0.0, 1.0),
        target,
        pole,
        soft_start: soft,
    }
}

#[test]
fn ik_reaches_reachable_targets_to_1e4_and_keeps_bone_lengths() {
    let mut rng = Rng(7);
    let l = 2.0 * L1;
    let mut checked = 0;
    while checked < 600 {
        let dir = rng.unit();
        let d = (0.25 + 0.72 * rng.next()) as f32 * l;
        let pole = rng.unit();
        if f64::from(pole.dot(dir)).abs() > 0.9 {
            continue;
        }
        let mut inp = limb(
            v3(0.0, 0.0, 1.0).add(dir.scale(d)),
            pole,
            ik::DEFAULT_SOFT_START,
        );
        inp.parent_world = rng.quat();
        inp.upper_rest_rot = rng.quat();
        inp.lower_rest_rot = rng.quat();
        let lp = ik::solve_limb(&inp);
        assert!(
            !lp.solution.limited,
            "d/L = {} is inside the exact range",
            d / l
        );
        let (mid, end) = ik::limb_fk(&inp, &lp);
        assert!(
            dist(end, inp.target) <= 1.0e-4,
            "end error {}",
            dist(end, inp.target)
        );
        let l1 = inp.upper_to_lower.length();
        let l2 = inp.lower_to_end.length();
        assert!((f64::from(mid.sub(inp.root).length() - l1)).abs() <= 1.0e-4);
        assert!((f64::from(end.sub(mid).length() - l2)).abs() <= 1.0e-4);
        // The local rotations compose back to the world rotations.
        let up_back = inp.parent_world.mul(lp.upper_local);
        let lo_back = lp.upper_world.mul(lp.lower_local);
        assert!(f64::from(up_back.dot(lp.upper_world)).abs() >= 1.0 - 1.0e-5);
        assert!(f64::from(lo_back.dot(lp.lower_world)).abs() >= 1.0 - 1.0e-5);
        checked += 1;
    }
}

#[test]
fn ik_mid_joint_lies_in_the_pole_plane_on_the_pole_side() {
    let mut rng = Rng(11);
    let mut checked = 0;
    while checked < 300 {
        let dir = rng.unit();
        let d = (0.4 + 0.5 * rng.next()) as f32 * 2.0 * L1;
        let pole = rng.unit();
        let axis = dir;
        if f64::from(pole.dot(axis)).abs() > 0.85 {
            continue;
        }
        let inp = limb(
            v3(0.0, 0.0, 1.0).add(dir.scale(d)),
            pole,
            ik::DEFAULT_SOFT_START,
        );
        let sol = ik::solve_two_bone(
            inp.root,
            inp.target,
            inp.pole,
            inp.upper_to_lower.length(),
            inp.lower_to_end.length(),
            inp.soft_start,
        );
        let to = inp.target.sub(inp.root);
        let n = to.cross(pole);
        let off = sol.mid.sub(inp.root);
        assert!(
            f64::from(off.dot(n)).abs() <= 1.0e-4 * f64::from(n.length().max(1.0)),
            "mid is off the pole plane by {}",
            off.dot(n)
        );
        let u = to.scale(1.0 / to.length());
        let pp = pole.sub(u.scale(pole.dot(u)));
        assert!(off.dot(pp) > 0.0, "mid is on the wrong side of the axis");
        checked += 1;
    }
}

#[test]
fn ik_unreachable_targets_move_the_limb_continuously_and_a_hard_clamp_does_not() {
    let l = 2.0 * L1;
    let dir = v3(0.0, 0.3, -1.0).scale(1.0 / v3(0.0, 0.3, -1.0).length());
    let pole = v3(0.0, 1.0, 0.0);
    let run = |soft: f32| -> (f64, f64) {
        let mut prev: Option<(Vec3, Vec3, Vec3)> = None;
        let mut mid_ratio = 0.0f64;
        let mut end_ratio = 0.0f64;
        for k in 0..=800 {
            let d = l * (1.3 - 0.0005 * k as f32);
            let target = v3(0.0, 0.0, 1.0).add(dir.scale(d));
            let sol = ik::solve_two_bone(v3(0.0, 0.0, 1.0), target, pole, L1, L1, soft);
            assert!(sol.reach <= l + 1.0e-6, "the limb never overreaches");
            if let Some((pm, pe, pt)) = prev {
                let dt = dist(target, pt);
                mid_ratio = mid_ratio.max(dist(sol.mid, pm) / dt);
                end_ratio = end_ratio.max(dist(sol.end, pe) / dt);
            }
            prev = Some((sol.mid, sol.end, target));
        }
        (mid_ratio, end_ratio)
    };
    let (soft_mid, soft_end) = run(ik::DEFAULT_SOFT_START);
    assert!(
        soft_end <= 1.0 + 1.0e-3,
        "end effector is 1-Lipschitz: {soft_end}"
    );
    // Derived: with l1 = l2 the knee moves at most 2.73 mm per mm of target at the start of the
    // soft zone (model in the test notes of docs/evidence/m5c/model_perform.py).
    assert!(
        soft_mid <= 3.5,
        "knee pops near full extension: {soft_mid} mm/mm"
    );
    // CONTROL: a hard clamp at full extension. Must fail the same bound.
    let (hard_mid, _) = run(1.0);
    assert!(
        hard_mid >= 10.0,
        "the control (hard clamp) should snap the knee, saw {hard_mid} mm/mm"
    );
}

// ---------------------------------------------------------------------------------------------
// Walking
// ---------------------------------------------------------------------------------------------

fn restrained() -> Style {
    Style::restrained()
}

fn walk_script(style: Style, x: f32, y: f32, stride: f32, speed: f32) -> PerformanceScript {
    script(style, vec![walk(x, y, stride, speed)])
}

#[test]
fn walk_root_displacement_is_stride_times_steps_and_the_body_follows() {
    let rig = synthetic_rig();
    let s = walk_script(restrained(), 0.0, 3.0, 0.5, 1.0);
    let perf = compile(&rig, &s, &PerformOptions::default());
    let w = perf.walks()[0];
    assert_eq!(w.steps, 6);
    assert!((f64::from(w.stride) - 0.5).abs() < 1.0e-5);
    let expect = f64::from(w.stride) * w.steps as f64;
    let p0 = perf.pose_at(0.0);
    let p1 = perf.pose_at(perf.duration());
    let root = dist(p1.root_t, p0.root_t);
    assert!(
        (root - expect).abs() <= 2.0e-3,
        "root moved {root}, expected {expect}"
    );
    // The pelvis and both ankles travel with it (no treadmill: the pelvis does not stay put while
    // the feet shuffle).
    for &j in &[PELVIS, FOOT_L, FOOT_R] {
        let a = joint_pos(&rig, &perf, 0.0, j);
        let b = joint_pos(&rig, &perf, perf.duration(), j);
        let travelled = f64::from(b.y - a.y);
        assert!(
            (travelled - expect).abs() <= 5.0e-3,
            "joint {j} travelled {travelled}, expected {expect}"
        );
    }
    // CONTROL: walking in place has no root displacement, and the same measurement sees it.
    let off = PerformOptions {
        root_motion: false,
        ..PerformOptions::default()
    };
    let perf0 = compile(&rig, &s, &off);
    let q1 = perf0.pose_at(perf0.duration());
    assert!(dist(q1.root_t, perf0.pose_at(0.0).root_t) < 1.0e-4);
    let b = joint_pos(&rig, &perf0, perf0.duration(), PELVIS);
    let a = joint_pos(&rig, &perf0, 0.0, PELVIS);
    assert!(
        f64::from(b.y - a.y).abs() < 1.0e-3,
        "control pelvis must not travel"
    );
}

fn stance_speed_and_height(
    rig: &Rig,
    perf: &Performance,
    foot: usize,
    rest_z: f32,
) -> (f64, f64, usize) {
    let f = if foot == FOOT_L { 0 } else { 1 };
    let dt = 1.0 / 30.0;
    let mut worst_speed = 0.0f64;
    let mut worst_height = 0.0f64;
    let mut checks = 0usize;
    for &(a, b) in perf.stance_windows(f) {
        let a = a + 0.02;
        let b = b - 0.02;
        if b - a <= dt {
            continue;
        }
        let mut t = a;
        while t + dt <= b {
            let p = joint_pos(rig, perf, t, foot);
            let q = joint_pos(rig, perf, t + dt, foot);
            worst_speed = worst_speed.max(dist(p, q) / dt);
            worst_height = worst_height.max(f64::from((p.z - rest_z).abs()));
            checks += 1;
            t += dt;
        }
    }
    (worst_speed, worst_height, checks)
}

#[test]
fn stance_feet_are_world_locked_and_on_the_ground() {
    let rig = synthetic_rig();
    let rest_z = rig.rest_position(FOOT_L).z;
    for style in [Style::restrained(), Style::expressive()] {
        let s = walk_script(style, 0.0, 3.0, 0.5, 1.0);
        let perf = compile(&rig, &s, &PerformOptions::default());
        for &foot in &FEET {
            let (v, h, n) = stance_speed_and_height(&rig, &perf, foot, rest_z);
            assert!(n >= 25, "too few stance checks ({n}) for foot {foot}");
            assert!(v <= 2.0e-3, "foot {foot} slides at {v} m/s in stance");
            assert!(
                h <= 2.0e-3,
                "foot {foot} leaves the ground by {h} m in stance"
            );
        }
        // The feet really step: a swinging ankle rises well off the ground.
        let mut peak = 0.0f32;
        let mut t = 0.0;
        while t < perf.duration() {
            peak = peak.max(joint_pos(&rig, &perf, t, FOOT_L).z - rest_z);
            t += 0.01;
        }
        assert!(peak >= 0.03, "swing foot never lifts (peak {peak})");
    }
}

#[test]
fn stance_control_without_foot_ik_slides() {
    let rig = synthetic_rig();
    let rest_z = rig.rest_position(FOOT_L).z;
    let s = walk_script(restrained(), 0.0, 3.0, 0.5, 1.0);
    let off = PerformOptions {
        foot_lock: false,
        ..PerformOptions::default()
    };
    let perf = compile(&rig, &s, &off);
    let (v, _, n) = stance_speed_and_height(&rig, &perf, FOOT_L, rest_z);
    assert!(n >= 25);
    assert!(
        v > 0.2,
        "the control must slide at walking speed, saw {v} m/s"
    );
}

#[test]
fn walk_with_a_turn_keeps_the_feet_planted() {
    let rig = synthetic_rig();
    let rest_z = rig.rest_position(FOOT_L).z;
    let s = walk_script(Style::expressive(), 1.2, 2.4, 0.45, 1.0);
    let perf = compile(&rig, &s, &PerformOptions::default());
    for &foot in &FEET {
        let (v, h, n) = stance_speed_and_height(&rig, &perf, foot, rest_z);
        assert!(n >= 20);
        assert!(v <= 2.0e-3, "foot {foot} slides at {v} m/s while turning");
        assert!(h <= 2.0e-3, "foot {foot} floats by {h} m while turning");
    }
    // It arrives at the target, facing it.
    let end = perf.pose_at(perf.duration());
    assert!(dist(end.root_t, v3(1.2, 2.4, 0.0)) <= 2.0e-3);
}

// ---------------------------------------------------------------------------------------------
// Continuity
// ---------------------------------------------------------------------------------------------

fn continuity_script() -> PerformanceScript {
    // Beat 1 (the wave) is cut after 1.2 s while the hand is raised: the next beat starts from a
    // different pose than the wave was in, which only inertialization hides.
    script(
        Style::restrained(),
        vec![
            Beat::LookAt {
                target: v3(-1.4, 1.4, 1.64),
            },
            Beat::Gesture {
                kind: GestureKind::Wave,
                intensity: 1.0,
            },
            Beat::Gesture {
                kind: GestureKind::Acknowledge,
                intensity: 1.0,
            },
            walk(0.0, 2.0, 0.4, 0.9),
            Beat::Stop,
            Beat::Sit {
                seat_height: 0.55,
                seat_pos: v3(0.0, 1.72, 0.0),
            },
            Beat::Idle { secs: 1.0 },
            Beat::Stand,
            Beat::Express {
                channel: "smile".into(),
                value: 1.0,
            },
            Beat::Idle { secs: 0.6 },
        ],
    )
    .with_cut(1, 1.2)
}

fn boundary_times(perf: &Performance) -> Vec<f64> {
    let mut v: Vec<f64> = perf.events().iter().map(|e| e.time).collect();
    for f in 0..2 {
        for &(a, b) in perf.stance_windows(f) {
            v.push(a);
            v.push(b);
        }
    }
    v
}

#[test]
fn every_beat_boundary_and_foot_lock_transition_is_c1() {
    let rig = synthetic_rig();
    let s = continuity_script();
    let perf = compile(&rig, &s, &PerformOptions::default());
    // Dense around every boundary, sparse (8 ms) elsewhere.
    let mut times: Vec<f64> = Vec::new();
    for t0 in boundary_times(&perf) {
        times.extend(window(t0, 0.04));
    }
    let mut t = 0.01;
    while t < perf.duration() - 0.01 {
        times.push(t);
        t += 0.008;
    }
    let (worst, at) = max_j(&rig, &perf, &times);
    assert!(
        worst <= J_BOUND,
        "J = {worst} m/s at t = {at}s exceeds the declared bound {J_BOUND}"
    );
}

#[test]
fn control_a_snapped_beat_cut_breaks_the_same_metric() {
    let rig = synthetic_rig();
    let s = continuity_script();
    let off = PerformOptions {
        inertialize: false,
        ..PerformOptions::default()
    };
    let perf = compile(&rig, &s, &off);
    let cut = event_time(&perf, "acknowledge");
    let (worst, _) = max_j(&rig, &perf, &window(cut, 0.01));
    assert!(
        worst >= 10.0 * J_BOUND,
        "the control must exceed the bound by 10x, saw {worst}"
    );
    // And the shipping build is smooth at that very instant.
    let good = compile(&rig, &s, &PerformOptions::default());
    let (ok, _) = max_j(&rig, &good, &window(event_time(&good, "acknowledge"), 0.04));
    assert!(ok <= J_BOUND, "J = {ok} at the cut");
}

#[test]
fn the_cut_blend_takes_the_settle_ease_and_lowers_the_hand_monotonically() {
    // Position and velocity are matched, so the hand leaves the raised wave without a jump and
    // arrives at the hang within the settle ease (plus the nod that follows).
    let rig = synthetic_rig();
    let s = continuity_script();
    let perf = compile(&rig, &s, &PerformOptions::default());
    let cut = event_time(&perf, "acknowledge");
    let before = joint_pos(&rig, &perf, cut - 0.002, HAND_R);
    let after = joint_pos(&rig, &perf, cut + 0.002, HAND_R);
    assert!(
        dist(before, after) < 0.01,
        "hand jumped {}",
        dist(before, after)
    );
    let settle = f64::from(Style::restrained().settle_ms) / 1000.0;
    let late = joint_pos(&rig, &perf, cut + settle + 0.1, HAND_R);
    let shoulder = joint_pos(&rig, &perf, cut + settle + 0.1, 12);
    assert!(
        shoulder.z - late.z > 0.35,
        "after the ease the hand hangs below the shoulder"
    );
}

// ---------------------------------------------------------------------------------------------
// Gesture and gaze
// ---------------------------------------------------------------------------------------------

fn wave_script_i(style: Style, intensity: f32) -> PerformanceScript {
    script(
        style,
        vec![Beat::Gesture {
            kind: GestureKind::Wave,
            intensity,
        }],
    )
}

fn wave_script(style: Style) -> PerformanceScript {
    wave_script_i(style, 1.0)
}

fn min_hand_head(rig: &Rig, perf: &Performance) -> f64 {
    let mut m = f64::INFINITY;
    let mut t = 0.0;
    while t <= perf.duration() {
        let p = perf.pose_at(t);
        let hand = rig.joint_world(&p)[HAND_R];
        m = m.min(dist(hand, rig.head_center(&p)));
        t += 0.005;
    }
    m
}

#[test]
fn the_waving_hand_stays_out_of_the_head_sphere() {
    let rig = synthetic_rig();
    let clearance = 0.20;
    // A style so large that the authored wave swings the hand into the head.
    let mut huge = Style::expressive();
    huge.amplitude = 3.0;
    for style in [Style::restrained(), Style::expressive(), huge.clone()] {
        let perf = compile(&rig, &wave_script_i(style, 1.5), &PerformOptions::default());
        let m = min_hand_head(&rig, &perf);
        assert!(
            m >= clearance - 1.0e-3,
            "hand came within {m} of the head centre"
        );
        // The hand really goes up beside the head (not a vacuous pass with the arm down).
        let mut top = f32::MIN;
        let mut t = 0.0;
        while t <= perf.duration() {
            top = top.max(joint_pos(&rig, &perf, t, HAND_R).z);
            t += 0.01;
        }
        assert!(top > 1.55, "the wave never raised the hand (top {top})");
    }
    // CONTROL: clearance off. The huge style then drives the hand inside the radius.
    let off = PerformOptions {
        arm_clearance: false,
        ..PerformOptions::default()
    };
    let perf = compile(&rig, &wave_script_i(huge, 1.5), &off);
    let m = min_hand_head(&rig, &perf);
    assert!(
        m < clearance - 0.02,
        "the control must violate the clearance, saw {m}"
    );
}

fn head_yaw_deg(rig: &Rig, pose: &Pose) -> f64 {
    let m = rig.world_matrices(pose)[HEAD];
    let f = m.transform_vector(v3(0.0, 1.0, 0.0));
    // Heading is 0, forward +Y, left -X.
    f64::from(-f.x).atan2(f64::from(f.y)).to_degrees()
}

#[test]
fn a_target_forty_five_degrees_off_turns_the_head_legibly() {
    let rig = synthetic_rig();
    let eye_z = 1.60 + 0.09 - 0.05;
    for style in [
        Style::restrained(),
        Style::expressive(),
        Style::restrained().reduced(),
    ] {
        let target = v3(-1.414, 1.414, eye_z);
        let s = script(style, vec![Beat::LookAt { target }]);
        let perf = compile(&rig, &s, &PerformOptions::default());
        let yaw0 = head_yaw_deg(&rig, &perf.pose_at(0.0));
        let yaw1 = head_yaw_deg(&rig, &perf.pose_at(perf.duration()));
        assert!(yaw0.abs() < 1.0);
        assert!(yaw1 >= 15.0, "head yaw {yaw1} deg is not legible");
        assert!(yaw1 <= 60.0, "head overshoots the target: {yaw1}");
        // It turns smoothly (monotone) and the eyes lead the head.
        let mut prev = yaw0;
        let mut t = 0.0;
        while t <= perf.duration() {
            let y = head_yaw_deg(&rig, &perf.pose_at(t));
            assert!(y >= prev - 0.05, "yaw reversed at t = {t}");
            prev = y;
            t += 0.02;
        }
    }
    // CONTROL: a target straight ahead gives no turn, so the measurement is not vacuous.
    let s = script(
        Style::restrained(),
        vec![Beat::LookAt {
            target: v3(0.0, 2.0, 1.55),
        }],
    );
    let perf = compile(&rig, &s, &PerformOptions::default());
    assert!(head_yaw_deg(&rig, &perf.pose_at(perf.duration())).abs() < 3.0);
}

// ---------------------------------------------------------------------------------------------
// Sit and stand
// ---------------------------------------------------------------------------------------------

#[test]
fn sit_settles_the_pelvis_at_the_seat_with_planted_feet_and_stand_returns() {
    let rig = synthetic_rig();
    let s = script(
        Style::restrained(),
        vec![
            Beat::Sit {
                seat_height: 0.55,
                seat_pos: v3(0.0, -0.28, 0.0),
            },
            Beat::Idle { secs: 0.5 },
            Beat::Stand,
        ],
    );
    let perf = compile(&rig, &s, &PerformOptions::default());
    let stand_t = event_time(&perf, "stand");
    let seated = rig.joint_world(&perf.pose_at(stand_t - 0.05));
    let start = rig.joint_world(&perf.pose_at(0.0));
    assert!(
        (f64::from(seated[PELVIS].z) - 0.55).abs() <= 0.01,
        "pelvis at {} not 0.55",
        seated[PELVIS].z
    );
    assert!(f64::from(seated[PELVIS].y + 0.28).abs() <= 0.01);
    for &f in &FEET {
        assert!(
            dist(seated[f], start[f]) <= 1.0e-3,
            "foot {f} moved {} while sitting",
            dist(seated[f], start[f])
        );
    }
    // The knee bends forward (the calf is in front of the thigh line).
    let knee = seated[16];
    assert!(
        knee.y > seated[PELVIS].y + 0.1,
        "knee does not point forward"
    );
    // Standing again: pelvis back above the feet, feet still where they were.
    let end = rig.joint_world(&perf.pose_at(perf.duration()));
    let stand_z = 1.0 - 0.0458;
    assert!(
        (f64::from(end[PELVIS].z) - stand_z).abs() <= 0.01,
        "pelvis {}",
        end[PELVIS].z
    );
    assert!(f64::from(end[PELVIS].y).abs() <= 0.01);
    for &f in &FEET {
        assert!(dist(end[f], start[f]) <= 1.0e-3);
    }
    // The path is smooth.
    let mut times = Vec::new();
    let mut t = 0.02;
    while t < perf.duration() - 0.02 {
        times.push(t);
        t += 0.01;
    }
    let (worst, at) = max_j(&rig, &perf, &times);
    assert!(worst <= J_BOUND, "J = {worst} at {at}");
}

#[test]
fn an_unreachable_seat_and_a_walk_while_seated_are_typed_errors() {
    let rig = synthetic_rig();
    let far = script(
        Style::restrained(),
        vec![Beat::Sit {
            seat_height: 0.5,
            seat_pos: v3(0.0, -3.0, 0.0),
        }],
    );
    assert!(matches!(
        Performance::compile(&rig, &far, &PerformOptions::default()),
        Err(PerformError::SeatUnreachable { .. })
    ));
    let bad = script(
        Style::restrained(),
        vec![
            Beat::Sit {
                seat_height: 0.55,
                seat_pos: v3(0.0, -0.28, 0.0),
            },
            walk(0.0, 2.0, 0.5, 1.0),
        ],
    );
    assert!(matches!(
        Performance::compile(&rig, &bad, &PerformOptions::default()),
        Err(PerformError::Posture(_))
    ));
    let cut_walk = walk_script(Style::restrained(), 0.0, 2.0, 0.5, 1.0).with_cut(0, 1.0);
    assert!(matches!(
        Performance::compile(&rig, &cut_walk, &PerformOptions::default()),
        Err(PerformError::CannotCutWalk { beat: 0 })
    ));
}

// ---------------------------------------------------------------------------------------------
// Style
// ---------------------------------------------------------------------------------------------

fn range_of(rig: &Rig, perf: &Performance, j: usize, axis: usize, t0: f64, t1: f64) -> f64 {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let mut t = t0;
    while t <= t1 {
        let p = joint_pos(rig, perf, t, j);
        let v = f64::from([p.x, p.y, p.z][axis]);
        lo = lo.min(v);
        hi = hi.max(v);
        t += 0.004;
    }
    hi - lo
}

#[test]
fn restrained_and_expressive_differ_measurably_in_amplitude() {
    let rig = synthetic_rig();
    // Wave: peak-to-peak lateral travel of the hand over the middle half of the beat.
    let mut p2p = Vec::new();
    for style in [Style::restrained(), Style::expressive()] {
        let perf = compile(&rig, &wave_script(style), &PerformOptions::default());
        let d = perf.duration() - f64::from(perf.style().settle_ms) / 1000.0;
        p2p.push(range_of(&rig, &perf, HAND_R, 0, 0.25 * d, 0.75 * d));
    }
    assert!(p2p[1] >= 1.3 * p2p[0], "wave excursion {p2p:?}");
    // Nod: head pitch excursion (the head's forward vector tilts down).
    let mut nod = Vec::new();
    for style in [Style::restrained(), Style::expressive()] {
        let s = script(
            style,
            vec![Beat::Gesture {
                kind: GestureKind::Acknowledge,
                intensity: 1.0,
            }],
        );
        let perf = compile(&rig, &s, &PerformOptions::default());
        let mut lo = 0.0f32;
        let mut t = 0.0;
        while t <= perf.duration() {
            let m = rig.world_matrices(&perf.pose_at(t))[HEAD];
            lo = lo.min(m.transform_vector(v3(0.0, 1.0, 0.0)).z);
            t += 0.01;
        }
        nod.push(f64::from(-lo));
    }
    assert!(nod[0] > 0.05, "restrained nod is invisible: {nod:?}");
    assert!(nod[1] >= 1.3 * nod[0], "nod depth {nod:?}");
    // Walk: arm swing (the hand travels forward and back).
    let mut swing = Vec::new();
    for style in [Style::restrained(), Style::expressive()] {
        let perf = compile(
            &rig,
            &walk_script(style, 0.0, 3.0, 0.5, 1.0),
            &PerformOptions::default(),
        );
        // Subtract the root travel by looking at the hand relative to the pelvis.
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        let mut t = 0.0;
        while t <= perf.duration() {
            let w = rig.joint_world(&perf.pose_at(t));
            let rel = w[HAND_L].y - w[PELVIS].y;
            lo = lo.min(rel);
            hi = hi.max(rel);
            t += 0.01;
        }
        swing.push(f64::from(hi - lo));
    }
    assert!(swing[1] >= 1.3 * swing[0], "arm swing {swing:?}");
}

#[test]
fn reduced_motion_lowers_amplitude_and_turns_the_walk_into_a_dissolve() {
    let rig = synthetic_rig();
    let on = PerformOptions {
        reduced_motion: true,
        ..PerformOptions::default()
    };
    let full = compile(
        &rig,
        &wave_script(Style::expressive()),
        &PerformOptions::default(),
    );
    let calm = compile(&rig, &wave_script(Style::expressive()), &on);
    let d1 = full.duration() - 0.3;
    let d2 = calm.duration() - 0.3;
    let a = range_of(&rig, &full, HAND_R, 0, 0.25 * d1, 0.75 * d1);
    let b = range_of(&rig, &calm, HAND_R, 0, 0.25 * d2, 0.75 * d2);
    assert!(b <= 0.7 * a, "reduced wave excursion {b} vs {a}");
    // Locomotion: no steps (the feet slide as one pose), still arrives, still smooth.
    let s = walk_script(Style::expressive(), 0.0, 2.0, 0.5, 1.0);
    let w = compile(&rig, &s, &on);
    let rest_z = rig.rest_position(FOOT_L).z;
    let mut peak = 0.0f32;
    let mut t = 0.0;
    while t <= w.duration() {
        peak = peak.max(joint_pos(&rig, &w, t, FOOT_L).z - rest_z);
        t += 0.01;
    }
    assert!(peak < 0.01, "a dissolve has no foot lift, saw {peak}");
    assert!(dist(w.pose_at(w.duration()).root_t, v3(0.0, 2.0, 0.0)) <= 2.0e-3);
    let times: Vec<f64> = (1..(w.duration() / 0.01) as usize - 1)
        .map(|i| i as f64 * 0.01)
        .collect();
    let (worst, at) = max_j(&rig, &w, &times);
    assert!(worst <= J_BOUND, "dissolve J = {worst} at {at}");
}

// ---------------------------------------------------------------------------------------------
// Tape
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected `{}` at {}", c as char, self.i))
        }
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.b.get(self.i) {
            Some(b'{') => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Obj(out));
                }
                loop {
                    let k = match self.value()? {
                        Json::Str(s) => s,
                        _ => return Err("object key is not a string".into()),
                    };
                    self.eat(b':')?;
                    out.push((k, self.value()?));
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(out));
                        }
                        _ => return Err(format!("bad object at {}", self.i)),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Arr(out));
                }
                loop {
                    out.push(self.value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(out));
                        }
                        _ => return Err(format!("bad array at {}", self.i)),
                    }
                }
            }
            Some(b'"') => {
                self.i += 1;
                let mut s = String::new();
                loop {
                    match self.b.get(self.i) {
                        None => return Err("unterminated string".into()),
                        Some(b'"') => {
                            self.i += 1;
                            return Ok(Json::Str(s));
                        }
                        Some(b'\\') => {
                            self.i += 1;
                            match self.b.get(self.i) {
                                Some(b'n') => s.push('\n'),
                                Some(b't') => s.push('\t'),
                                Some(b'r') => s.push('\r'),
                                Some(b'"') => s.push('"'),
                                Some(b'\\') => s.push('\\'),
                                _ => return Err("unsupported escape".into()),
                            }
                            self.i += 1;
                        }
                        Some(&c) => {
                            s.push(c as char);
                            self.i += 1;
                        }
                    }
                }
            }
            Some(b't') if self.b[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(Json::Bool(true))
            }
            Some(b'f') if self.b[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(Json::Bool(false))
            }
            Some(b'n') if self.b[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(Json::Null)
            }
            Some(_) => {
                let st = self.i;
                while self.i < self.b.len()
                    && matches!(
                        self.b[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                std::str::from_utf8(&self.b[st..self.i])
                    .ok()
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(Json::Num)
                    .ok_or_else(|| format!("bad number at {st}"))
            }
            None => Err("unexpected end".into()),
        }
    }
}

fn parse_json(s: &str) -> Result<Json, String> {
    let mut p = P {
        b: s.as_bytes(),
        i: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i != s.len() {
        return Err(format!("trailing bytes at {}", p.i));
    }
    Ok(v)
}

fn field<'a>(j: &'a Json, k: &str) -> &'a Json {
    match j {
        Json::Obj(o) => {
            &o.iter()
                .find(|(n, _)| n == k)
                .unwrap_or_else(|| panic!("no key {k}"))
                .1
        }
        _ => panic!("not an object"),
    }
}

fn tape_script() -> PerformanceScript {
    script(
        Style::restrained(),
        vec![
            Beat::Express {
                channel: "smile".into(),
                value: 0.8,
            },
            Beat::LookAt {
                target: v3(-1.0, 1.5, 1.6),
            },
            walk(0.0, 1.5, 0.5, 1.0),
            Beat::Gesture {
                kind: GestureKind::Wave,
                intensity: 1.0,
            },
        ],
    )
}

fn identity() -> TapeIdentity {
    TapeIdentity {
        body_rev_id: "00ab".into(),
        fit_id: "11cd".into(),
        present_id: "22ef".into(),
        asset_manifest_sha256: "33".repeat(32),
        controls: vec![("stature".into(), 0.25), ("hip \"breadth\"".into(), -1.0)],
        garments: vec![("tee".into(), "Fit".into())],
        materials_json: "{\"base\":{\"albedo\":[0.5,0.5,0.5]}}".into(),
        camera_cuts: vec![(0, "front".into())],
    }
}

#[test]
fn the_tape_is_deterministic_well_formed_and_hashes_its_body() {
    let rig = synthetic_rig();
    let s = tape_script();
    let a = perform(&rig, &s, 30).expect("tape");
    let b = perform(&rig, &s, 30).expect("tape");
    let fa = a.to_files(&identity());
    let fb = b.to_files(&identity());
    assert_eq!(fa.bin, fb.bin, "bodies must be byte-identical");
    assert_eq!(
        fa.header_json, fb.header_json,
        "headers must be byte-identical"
    );
    assert_eq!(a, b);
    // Layout: 3 + 10 per joint + face channels, f32 little-endian, no padding.
    let stride = 3 + 10 * 21 + 1;
    assert_eq!(a.stride(), stride);
    assert_eq!(fa.bin.len(), a.frame_count * stride * 4);
    let h = parse_json(&fa.header_json).expect("the header is valid JSON");
    match field(&h, "format") {
        Json::Str(f) => assert_eq!(f, "gibson.figure.tape.v1"),
        other => panic!("format is {other:?}"),
    }
    match field(&h, "frame_count") {
        Json::Num(n) => assert_eq!(*n as usize, a.frame_count),
        other => panic!("frame_count is {other:?}"),
    }
    match field(&h, "fps") {
        Json::Num(n) => assert_eq!(*n as u32, 30),
        other => panic!("fps is {other:?}"),
    }
    match field(&h, "joints") {
        Json::Arr(j) => assert_eq!(j.len(), 21),
        other => panic!("joints is {other:?}"),
    }
    match field(&h, "channels") {
        Json::Arr(c) => assert_eq!(c.len(), 4),
        other => panic!("channels is {other:?}"),
    }
    match field(&h, "face") {
        Json::Arr(c) => assert_eq!(c.len(), 1),
        other => panic!("face is {other:?}"),
    }
    match field(field(&h, "controls"), "hip \"breadth\"") {
        Json::Num(n) => assert_eq!(*n, -1.0),
        other => panic!("{other:?}"),
    }
    let digest = sha256::digest(&fa.bin);
    let mut hex = String::new();
    for x in digest {
        hex.push_str(&format!("{x:02x}"));
    }
    match field(&h, "bin_sha256") {
        Json::Str(s) => assert_eq!(*s, hex),
        other => panic!("bin_sha256 is {other:?}"),
    }
    assert_eq!(a.bin_sha256(), hex);
    // Frame 0: the root is at the origin and every rotation is a unit quaternion. The face
    // channel eases toward 0.8 and lands there.
    let f0 = a.frame(0);
    assert_eq!([f0[0], f0[1], f0[2]], [0.0, 0.0, 0.0]);
    for i in 0..a.frame_count {
        for (j, l) in a.locals(i).iter().enumerate() {
            let q = l.rotation;
            let n = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
            assert!((n - 1.0).abs() < 1.0e-4, "frame {i} joint {j} not unit");
        }
    }
    let last = a.frame(a.frame_count - 1);
    assert!(
        (last[stride - 1] - 0.8).abs() < 0.02,
        "face channel ends at {}",
        last[stride - 1]
    );
    // Frame i is the pose at i / fps, so the tape root travels the walk.
    assert!(dist(a.root(a.frame_count - 1), v3(0.0, 1.5, 0.0)) <= 5.0e-3);
    // A different script gives different bytes (the hash is not a constant).
    let c = perform(
        &rig,
        &walk_script(Style::expressive(), 0.0, 2.0, 0.5, 1.0),
        30,
    )
    .unwrap();
    assert_ne!(c.to_files(&identity()).bin, fa.bin);
    // Bad inputs are typed errors.
    assert!(matches!(perform(&rig, &s, 0), Err(PerformError::BadFps)));
}

// ---------------------------------------------------------------------------------------------
// Live
// ---------------------------------------------------------------------------------------------

fn intent(kind: IntentKind) -> Intent {
    Intent::new(kind)
}

fn live(opts: PerformOptions) -> (Rig, Live) {
    let rig = synthetic_rig();
    let l = Live::new(&rig, Style::expressive(), opts);
    (rig, l)
}

#[test]
fn live_stop_is_immediate_and_schedules_no_work() {
    let (_, mut l) = live(PerformOptions::default());
    let mut walk_i = intent(IntentKind::WalkTo);
    walk_i.target = Some(v3(0.0, 3.0, 0.0));
    l.submit(walk_i, 0).expect("walk accepted");
    let mut now = 0;
    let mut last = l.tick(0);
    while now < 1000 {
        now += 33;
        last = l.tick(now);
    }
    let w = l.work_count();
    assert!(w > 20);
    assert!(last.root_t.y > 0.3, "the figure is walking");
    l.stop();
    let held = l.tick(now + 33);
    assert_eq!(held, last, "the next tick returns the held pose");
    assert_eq!(l.tick(now + 5000), last);
    assert_eq!(l.work_count(), w, "stop schedules no work");
    // A new intent after a stop starts again, from the held pose without a jump.
    let mut wave = intent(IntentKind::Wave);
    wave.priority = 3;
    l.submit(wave, now + 6000).unwrap();
    let after = l.tick(now + 6000);
    assert!(dist(after.root_t, last.root_t) < 1.0e-3);
}

#[test]
fn live_quiesce_returns_the_held_pose_with_zero_work() {
    let (_, mut l) = live(PerformOptions::default());
    l.submit(intent(IntentKind::Wave), 0).unwrap();
    let p = l.tick(500);
    let w = l.work_count();
    l.quiesce();
    assert!(l.is_quiescent());
    for k in 1..=100 {
        assert_eq!(l.tick(500 + 16 * k), p);
    }
    assert_eq!(l.work_count(), w, "quiesce is zero work");
    l.submit(intent(IntentKind::Acknowledge), 5000).unwrap();
    assert!(!l.is_quiescent());
    l.tick(5016);
    assert_eq!(l.work_count(), w + 1);
}

/// J of hand and head from recorded 1 ms poses, over indices `lo..=hi`.
fn j_of(pts: &[(Vec3, Vec3)], lo: usize, hi: usize) -> f64 {
    let mut worst = 0.0f64;
    for i in lo.max(1)..=hi.min(pts.len() - 2) {
        for k in 0..2 {
            let get = |q: &(Vec3, Vec3)| if k == 0 { q.0 } else { q.1 };
            let d = get(&pts[i - 1])
                .sub(get(&pts[i]).scale(2.0))
                .add(get(&pts[i + 1]));
            worst = worst.max(f64::from(d.length()) / H);
        }
    }
    worst
}

fn record(rig: &Rig, l: &mut Live, pts: &mut Vec<(Vec3, Vec3)>, ms: u64) {
    let w = rig.joint_world(&l.tick(ms));
    pts.push((w[HAND_R], w[HEAD]));
}

#[test]
fn live_preemption_blends_from_the_current_pose_and_a_snap_does_not() {
    let run = |opts: PerformOptions| -> f64 {
        let (rig, mut l) = live(opts);
        let mut pts = Vec::new();
        l.submit(intent(IntentKind::Wave), 0).unwrap();
        for ms in 0..1200 {
            record(&rig, &mut l, &mut pts, ms);
        }
        let mut nod = intent(IntentKind::Acknowledge);
        nod.priority = 5;
        l.submit(nod, 1200).unwrap();
        for ms in 1200..=1700 {
            record(&rig, &mut l, &mut pts, ms);
        }
        j_of(&pts, 1100, 1650)
    };
    let good = run(PerformOptions::default());
    assert!(good <= J_BOUND, "preempted pose has J = {good}");
    let off = PerformOptions {
        inertialize: false,
        ..PerformOptions::default()
    };
    let bad = run(off);
    assert!(
        bad >= 10.0 * J_BOUND,
        "the control must snap, saw J = {bad}"
    );
}

#[test]
fn live_intents_queue_by_priority_and_an_expired_one_eases_out() {
    let (rig, mut l) = live(PerformOptions::default());
    let mut wave = intent(IntentKind::Wave);
    wave.expires_ms = Some(1500);
    l.submit(wave, 0).unwrap();
    // Same priority queues behind the wave and is dropped if it expires first.
    let mut late = intent(IntentKind::Acknowledge);
    late.expires_ms = Some(400);
    l.submit(late, 100).unwrap();
    let mut pts = Vec::new();
    for ms in 0..=2400 {
        record(&rig, &mut l, &mut pts, ms);
    }
    let j = j_of(&pts, 1300, 2300);
    assert!(j <= J_BOUND, "the expiry at 1500 ms is not smooth: J = {j}");
    // After the ease the hand hangs again.
    let p = l.tick(2600);
    let w = rig.joint_world(&p);
    assert!(
        w[12].z - w[HAND_R].z > 0.35,
        "the hand is still raised after expiry"
    );
}

#[test]
fn live_reduced_motion_lowers_amplitude() {
    let hand_range = |reduced: bool| -> f64 {
        let (rig, mut l) = live(PerformOptions::default());
        l.set_reduced_motion(reduced);
        l.submit(intent(IntentKind::Wave), 0).unwrap();
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for ms in (800..1700).step_by(4) {
            let w = rig.joint_world(&l.tick(ms));
            lo = lo.min(w[HAND_R].x);
            hi = hi.max(w[HAND_R].x);
        }
        f64::from(hi - lo)
    };
    let full = hand_range(false);
    let calm = hand_range(true);
    assert!(calm <= 0.7 * full, "reduced {calm} vs full {full}");
    // Turning it on mid-wave eases the wave out without a jump.
    let (rig, mut l) = live(PerformOptions::default());
    l.submit(intent(IntentKind::Wave), 0).unwrap();
    let mut pts = Vec::new();
    for ms in 0..=1000 {
        record(&rig, &mut l, &mut pts, ms);
    }
    l.set_reduced_motion(true);
    for ms in 1001..=1600 {
        record(&rig, &mut l, &mut pts, ms);
    }
    let j = j_of(&pts, 900, 1550);
    assert!(j <= J_BOUND, "J = {j} when reduced motion is switched on");
}

#[test]
fn live_walk_reaches_the_target_planted() {
    let (rig, mut l) = live(PerformOptions::default());
    let mut w = intent(IntentKind::WalkTo);
    w.target = Some(v3(0.0, 2.0, 0.0));
    w.intensity = 1.0;
    l.submit(w, 0).unwrap();
    // A wave submitted during the walk waits for it to plant.
    let mut wave = intent(IntentKind::Wave);
    wave.priority = 9;
    l.submit(wave, 500).unwrap();
    let rest_z = rig.rest_position(FOOT_L).z;
    let mut p = l.tick(0);
    let mut peak = 0.0f32;
    for ms in (0..9000).step_by(10) {
        p = l.tick(ms);
        peak = peak.max(rig.joint_world(&p)[FOOT_L].z - rest_z);
    }
    assert!(dist(p.root_t, v3(0.0, 2.0, 0.0)) <= 2.0e-3);
    assert!(peak >= 0.03, "the live walk steps");
}

// ---------------------------------------------------------------------------------------------
// Independent loop-seam probe (VIEWED defect 8)
// ---------------------------------------------------------------------------------------------
//
// The C-1 test of the seam is `J = second difference / h <= bound` and the vis3d receipt put the
// seam at 0.98 of that bound: a thin margin, and a bound that cannot tell "the seam is one more
// smooth point" from "the seam is a mild defect". This probe does not use `J` and does not use a
// bound. It asks two different questions of the analytic samples (value AND rate) of the clip:
//
//  A. DIFFERENTIAL. Build the same keys over four periods as a one-shot clip. Its interior
//     is, by construction, a smooth curve with true neighbours at every key, so around t = T it is
//     the loop with no seam at all. The loop clip must equal it in value and in rate on a dense grid
//     across the seam (1e-5 and 1e-3 respectively; the two evaluate the same Hermite pieces, so
//     the real difference is 0 unless the wrap tangent is wrong).
//  B. VELOCITY MATCHING. The analytic rate just after the seam minus just before, at
//     h = 8, 4, 2, 1, 0.5 ms, must shrink with h: `gap(0.5 ms) <= 0.1 gap(8 ms)` (a C1 loop has gap
//     proportional to h, ratio 1/16; a velocity step has a constant gap, ratio 1).
//
// CONTROLS that must fail: (A) a one-shot clip wrapped by modulo (clamped end tangents, so the
// loop forgets its own momentum at the seam) fails the differential; (B) a linearly interpolated
// clip wrapped by modulo (a velocity step at the seam) fails the matching.

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// The vis3d walk clip's keys (examples/figure_vis3d_kit.rs `walk_clip`), as plain data:
/// `(joint, axis, degrees)` rotation channels and a pelvis bob, keys every 300 ms over 1200 ms.
fn vis3d_walk_channels(period_ms: u64, periods: u64) -> Vec<Channel> {
    let x = v3(1.0, 0.0, 0.0);
    let y = v3(0.0, 1.0, 0.0);
    let base_t = [0u64, 300, 600, 900];
    let rots: [(&str, Vec3, [f32; 4]); 6] = [
        ("spine", y, [8.0, 0.0, -8.0, 0.0]),
        ("l_upper_arm", x, [35.0, 0.0, -35.0, 0.0]),
        ("r_upper_arm", x, [-35.0, 0.0, 35.0, 0.0]),
        ("l_upper_leg", x, [-28.0, 0.0, 28.0, 0.0]),
        ("r_upper_leg", x, [28.0, 0.0, -28.0, 0.0]),
        ("l_lower_leg", x, [5.0, 45.0, 10.0, 15.0]),
    ];
    let bob = [-0.02f32, 0.0, -0.02, 0.0];
    let mut channels = Vec::new();
    for (name, axis, deg) in rots {
        let mut keys = Vec::new();
        for p in 0..periods {
            for (i, t) in base_t.iter().enumerate() {
                keys.push((
                    ms(t + p * period_ms),
                    Quat::from_axis_angle(axis, deg[i].to_radians()),
                ));
            }
        }
        channels.push(Channel {
            target: ChannelTarget::JointRotation(name.into()),
            keys: Keys::Rotation(keys),
        });
    }
    let mut keys = Vec::new();
    for p in 0..periods {
        for (i, t) in base_t.iter().enumerate() {
            keys.push((ms(t + p * period_ms), v3(0.0, 1.0 + bob[i], 0.0)));
        }
    }
    channels.push(Channel {
        target: ChannelTarget::JointTranslation("pelvis".into()),
        keys: Keys::Translation(keys),
    });
    channels
}

/// A wrap segment of positive length: keys at 100, 400, 700 ms over a 1000 ms period.
fn offset_channels(periods: u64) -> Vec<Channel> {
    let times = [100u64, 400, 700];
    let arm = [v3(0.0, 0.0, 0.0), v3(2.0, 0.0, 0.0), v3(1.0, 1.0, 0.0)];
    let spine = [0.0f32, 30.0, -20.0];
    let smile = [0.0f32, 1.0, 0.3];
    let mut a = Vec::new();
    let mut s = Vec::new();
    let mut w = Vec::new();
    for p in 0..periods {
        for i in 0..3 {
            let t = ms(times[i] + p * 1000);
            a.push((t, arm[i]));
            s.push((
                t,
                Quat::from_axis_angle(v3(0.0, 0.0, 1.0), spine[i].to_radians()),
            ));
            w.push((t, smile[i]));
        }
    }
    vec![
        Channel {
            target: ChannelTarget::JointTranslation("arm".into()),
            keys: Keys::Translation(a),
        },
        Channel {
            target: ChannelTarget::JointRotation("spine".into()),
            keys: Keys::Rotation(s),
        },
        Channel {
            target: ChannelTarget::MorphWeight("smile".into()),
            keys: Keys::Weight(w),
        },
    ]
}

fn clip_of(name: &str, dur: u64, channels: Vec<Channel>) -> Clip {
    Clip::new(name, ms(dur), channels).expect("fixture clip is valid")
}

/// Compare two motions over a grid by channel name, value and rate (rotation up to sign).
fn seam_diff(a: &dyn Motion, b: &dyn Motion, grid: &[f64], shift_b: f64) -> (f64, f64) {
    let mut wv = 0.0f64;
    let mut wr = 0.0f64;
    for &t in grid {
        let pa = a.sample(Duration::from_secs_f64(t));
        let pb = b.sample(Duration::from_secs_f64(t + shift_b));
        for (tg, sa) in pa.tracks() {
            let sb = pb.get(tg).expect("the same channel exists in both");
            match (sa, sb) {
                (
                    Sample::Vec3 {
                        value: va,
                        rate: ra,
                    },
                    Sample::Vec3 {
                        value: vb,
                        rate: rb,
                    },
                ) => {
                    wv = wv.max(f64::from(dist(*va, *vb)));
                    wr = wr.max(f64::from(dist(*ra, *rb)));
                }
                (
                    Sample::Rotation {
                        value: qa,
                        omega: oa,
                    },
                    Sample::Rotation {
                        value: qb,
                        omega: ob,
                    },
                ) => {
                    let qb = if qa.dot(*qb) < 0.0 { qb.neg() } else { *qb };
                    let d = Quat::new(qa.x - qb.x, qa.y - qb.y, qa.z - qb.z, qa.w - qb.w);
                    wv = wv.max(f64::from(d.dot(d)).sqrt());
                    wr = wr.max(f64::from(dist(*oa, *ob)));
                }
                (
                    Sample::Scalar {
                        value: va,
                        rate: ra,
                    },
                    Sample::Scalar {
                        value: vb,
                        rate: rb,
                    },
                ) => {
                    wv = wv.max(f64::from((va - vb).abs()));
                    wr = wr.max(f64::from((ra - rb).abs()));
                }
                _ => panic!("channel kinds differ"),
            }
        }
    }
    (wv, wr)
}

/// Rate gap across a seam at `t_seam`: `|rate(t + h) - rate(t - h)|`, worst over channels, as a
/// function of `h`.
fn rate_gap(m: &dyn Motion, t_seam: f64, h: f64) -> f64 {
    let before = m.sample(Duration::from_secs_f64(t_seam - h));
    let after = m.sample(Duration::from_secs_f64(t_seam + h));
    let mut worst = 0.0f64;
    for (tg, sb) in before.tracks() {
        let sa = after.get(tg).expect("channel");
        let g = match (sb, sa) {
            (Sample::Vec3 { rate: rb, .. }, Sample::Vec3 { rate: ra, .. }) => dist(*ra, *rb),
            (Sample::Rotation { omega: ob, .. }, Sample::Rotation { omega: oa, .. }) => {
                dist(*oa, *ob)
            }
            (Sample::Scalar { rate: rb, .. }, Sample::Scalar { rate: ra, .. }) => {
                f64::from((ra - rb).abs())
            }
            _ => panic!("channel kinds differ"),
        };
        worst = worst.max(g);
    }
    worst
}

const GAP_HS: [f64; 5] = [8.0e-3, 4.0e-3, 2.0e-3, 1.0e-3, 0.5e-3];

/// `gap(0.5 ms) / gap(8 ms)`.
fn gap_shrink(m: &dyn Motion, t_seam: f64) -> f64 {
    let a = rate_gap(m, t_seam, GAP_HS[0]);
    let b = rate_gap(m, t_seam, GAP_HS[4]);
    b / a.max(1.0e-12)
}

/// A one-shot clip over `periods` copies of the keys, wrapped by modulo: the wrong way to loop.
struct Modulo<'a> {
    inner: &'a MotionClip,
    period: f64,
}

impl Motion for Modulo<'_> {
    fn sample(&self, t: Duration) -> MotionPose {
        let s = t.as_secs_f64().rem_euclid(self.period);
        self.inner.sample_at(Duration::from_secs_f64(s))
    }
}

struct AsMotion<'a>(&'a MotionClip);

impl Motion for AsMotion<'_> {
    fn sample(&self, t: Duration) -> MotionPose {
        self.0.sample_at(t)
    }
}

#[test]
fn loop_seam_matches_a_seamless_reference_and_the_velocity_gap_closes() {
    // Two fixtures: the vis3d walk loop (keys every 300 ms, first key at 0 and the cycle closes on
    // it) and a loop whose wrap chord has positive length.
    struct Fix {
        name: &'static str,
        period: u64,
        looped: MotionClip,
        reference: MotionClip,
    }
    let walk_loop = {
        let c = clip_of("walk_loop", 1200, {
            let mut ch = vis3d_walk_channels(1200, 1);
            // Close the loop: the last key of the one-period clip repeats the first.
            for c in &mut ch {
                match &mut c.keys {
                    Keys::Rotation(k) => {
                        let first = k[0].1;
                        k.push((ms(1200), first));
                    }
                    Keys::Translation(k) => {
                        let first = k[0].1;
                        k.push((ms(1200), first));
                    }
                    _ => {}
                }
            }
            ch
        });
        MotionClip::new(c).with_repeat(Repeat::Loop)
    };
    let walk_ref = MotionClip::new(clip_of("walk_ref", 4800, vis3d_walk_channels(1200, 4)));
    let off_loop =
        MotionClip::new(clip_of("off_loop", 1000, offset_channels(1))).with_repeat(Repeat::Loop);
    let off_ref = MotionClip::new(clip_of("off_ref", 4000, offset_channels(4)));
    let fixtures = [
        Fix {
            name: "vis3d walk",
            period: 1200,
            looped: walk_loop,
            reference: walk_ref,
        },
        Fix {
            name: "offset wrap",
            period: 1000,
            looped: off_loop,
            reference: off_ref,
        },
    ];
    for f in &fixtures {
        let t_seam = f.period as f64 / 1000.0;
        // The reference is shifted one period into its interior (it holds four copies).
        let grid: Vec<f64> = (0..=400)
            .map(|i| t_seam - 0.1 + i as f64 * 0.0005)
            .collect();
        let looped = AsMotion(&f.looped);
        let reference = AsMotion(&f.reference);
        let (wv, wr) = seam_diff(&looped, &reference, &grid, t_seam);
        assert!(
            wv <= 1.0e-5,
            "{}: loop differs from the seamless reference by {wv}",
            f.name
        );
        assert!(
            wr <= 1.0e-3,
            "{}: loop rate differs from the reference by {wr}",
            f.name
        );
        // B. The velocity gap closes with h (checked at the seam and at the next one).
        for k in [1.0, 2.0] {
            let s = gap_shrink(&looped, k * t_seam);
            assert!(
                s <= 0.1,
                "{}: gap(0.5ms)/gap(8ms) = {s} at seam {k}",
                f.name
            );
        }
        // CONTROL A: wrapping a one-shot clip by modulo clamps the tangents at the seam, so it
        // forgets its momentum: it must fail the differential against the seamless reference.
        let once = MotionClip::new(clip_of(
            "once",
            f.period,
            match f.name {
                "vis3d walk" => {
                    let mut ch = vis3d_walk_channels(1200, 1);
                    for c in &mut ch {
                        match &mut c.keys {
                            Keys::Rotation(k) => {
                                let first = k[0].1;
                                k.push((ms(1200), first));
                            }
                            Keys::Translation(k) => {
                                let first = k[0].1;
                                k.push((ms(1200), first));
                            }
                            _ => {}
                        }
                    }
                    ch
                }
                _ => offset_channels(1),
            },
        ));
        let wrapped = Modulo {
            inner: &once,
            period: t_seam,
        };
        let (cv, cr) = seam_diff(&wrapped, &reference, &grid, t_seam);
        assert!(
            cv > 1.0e-3 || cr > 0.05,
            "{}: the control (modulo wrap) must fail the differential (saw {cv}, {cr})",
            f.name
        );
    }
    // CONTROL B: linear keys wrapped by modulo have a velocity step at the seam; the gap does not
    // close as h shrinks.
    let tri = Clip::new(
        "tri",
        ms(2000),
        vec![Channel {
            target: ChannelTarget::MorphWeight("w".into()),
            keys: Keys::Weight(vec![(ms(0), 0.0), (ms(1000), 1.0), (ms(2000), 0.0)]),
        }],
    )
    .unwrap();
    let tri_m = MotionClip::new(tri)
        .with_interpolation(0, Interpolation::Linear)
        .unwrap();
    let wrapped = Modulo {
        inner: &tri_m,
        period: 2.0,
    };
    let s = gap_shrink(&wrapped, 2.0);
    assert!(
        s > 0.5,
        "the control must keep a constant velocity gap, ratio {s}"
    );
}
