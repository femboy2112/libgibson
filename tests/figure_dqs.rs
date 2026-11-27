//! M5-C slice 2: dual-quaternion skinning (`SkinningMode::DualQuat`).
//!
//! Evidence in this file, each with a control that must fail:
//! - (a) agreement with an independent f64 oracle on a 2-bone and a 3-bone fixture that have
//!   vertices with 4 influences;
//! - (b) the elbow cross-section check from the M5-C plan: a synthetic cylinder bent 120 degrees
//!   keeps at least 0.85 of its mid cross-section area under DualQuat, and the SAME check fails
//!   for Linear (the control);
//! - (c) the antipodal / sign-alignment case stays correct and continuous;
//! - plus the documented edge decisions: Linear is bit-identical to `evaluate`, a non-rigid skin
//!   matrix is rejected (never silently dropped), a cancelled blend is a typed failure.
//!
//! The oracle (`dq_oracle`, below) shares no code with `gibson::figure::math`. The runtime goes
//! TRS -> 4x4 matrices -> `W * IB` -> quaternion extraction (Shepperd) -> sign alignment ->
//! closed-form normalization -> rotate-plus-translation. The oracle never builds a matrix: it
//! composes dual quaternions from the TRS data directly (an 8-component Hamilton product),
//! inverts a unit dual quaternion by conjugation, normalizes by dual-NUMBER division, and applies
//! the result as the sandwich `q (1 + e p) q*` (not rotate-then-add). Quaternion component order
//! is `[w, x, y, z]` there and `x, y, z, w` in the runtime.
//!
//! Mutation controls (`RUSTFLAGS='--cfg figure_mutant="..."'`) are caught by ordinary behavioural
//! tests; none inspects the cfg. A = `dqs_as_lbs` (DualQuat falls back to linear blending),
//! B = `dqs_no_align` (blend signs are never aligned). The predicted kill matrix is
//! `docs/evidence/m5c/KILL_MATRIX_DQS_PREDICTED.md` (a prediction until run under heavy).
#![cfg(feature = "figure")]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::excessive_precision)]

use gibson::figure::math::{DualQuat, Mat4, Quat, Transform, Vec3};
use gibson::figure::revision::{
    compile, BodyFamily, BodyRecipe, CharacterRevision, EvalError, FamilyRef, NeutralMesh,
    PoseInput,
};
use gibson::figure::skeleton::Joint;
use gibson::figure::skin::{SkinError, SkinWeights, SkinningMode};

/// Comparison tolerance, DECLARED BEFORE any comparison and never tuned to results.
///
/// Absolute-or-relative: `|impl - oracle| <= TOL * max(1, |oracle|)`.
///
/// Error bound, in the style of `tests/figure_mechanics.rs`: `N * u * S` with `u = 2^-24` (the
/// f32 unit roundoff), `N` the longest dependent chain of rounded operations and `S` the sum of
/// magnitudes along it. Critical path for a vertex under a 4-joint chain: `from_transform` 12,
/// three matrix products 21, affine inverse 15, `W * IB` 7, quaternion extraction (sqrt,
/// divisions) 8, dual part `0.5 t q` 8, blend of 4 terms (align dot 7, scale 1, add 1) 36,
/// normalization (norm 8, projection 7, scale 8) 23, translation `2 d q*` 8, rotate 14, add 3:
/// 155, rounded up to N = 160. Fixture magnitudes (coordinates <= 3.5, translations <= 2.5,
/// rotation entries <= 1) keep `S <= 12`. Then `N * u * S = 160 * 5.96e-8 * 12 = 1.1e-4`.
/// TOL = 3e-4 is 2.7x above that worst case (the first-order worst case; the observed error of
/// such chains is far smaller) and is about 65x below the smallest effect any premise relies on
/// (`MIN_MODE_GAP`, 0.02, the Linear-versus-DualQuat difference that makes the oracle
/// comparison discriminate).
const TOL: f64 = 3.0e-4;

/// The smallest `max |Linear - DualQuat|` over a fixture pose that the oracle comparisons need
/// for the comparison to be able to tell the two modes apart (model: 0.045 at the weakest pose).
const MIN_MODE_GAP: f64 = 0.02;

fn close(a: f32, b: f64) -> bool {
    (f64::from(a) - b).abs() <= TOL * b.abs().max(1.0)
}

fn assert_close3(label: &str, got: Vec3, want: [f64; 3]) {
    assert!(
        close(got.x, want[0]) && close(got.y, want[1]) && close(got.z, want[2]),
        "{label}: got ({}, {}, {}), want ({}, {}, {})",
        got.x,
        got.y,
        got.z,
        want[0],
        want[1],
        want[2]
    );
}

fn widen3(p: [f32; 3]) -> [f64; 3] {
    [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
}

fn v3(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

fn quat_of(axis: [f32; 3], deg: f32) -> Quat {
    Quat::from_axis_angle(v3(axis), deg.to_radians())
}

fn ones() -> Vec3 {
    Vec3::new(1.0, 1.0, 1.0)
}

// ---------------------------------------------------------------------------------------------
// The independent f64 oracle.
// ---------------------------------------------------------------------------------------------

mod dq_oracle {
    /// Quaternion `[w, x, y, z]`.
    pub type Q = [f64; 4];

    /// Dual quaternion `r + e d`.
    #[derive(Clone, Copy, Debug)]
    pub struct Dq {
        pub r: Q,
        pub d: Q,
    }

    pub fn qmul(a: Q, b: Q) -> Q {
        [
            a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
            a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
            a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
            a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
        ]
    }

    pub fn qconj(a: Q) -> Q {
        [a[0], -a[1], -a[2], -a[3]]
    }

    pub fn qadd(a: Q, b: Q) -> Q {
        [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
    }

    pub fn qscale(a: Q, s: f64) -> Q {
        [a[0] * s, a[1] * s, a[2] * s, a[3] * s]
    }

    pub fn qdot(a: Q, b: Q) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
    }

    /// From a rotation given as `[x, y, z, w]` (normalized here) and a translation.
    pub fn from_trs(q_xyzw: [f64; 4], t: [f64; 3]) -> Dq {
        let n = qdot(
            [q_xyzw[3], q_xyzw[0], q_xyzw[1], q_xyzw[2]],
            [q_xyzw[3], q_xyzw[0], q_xyzw[1], q_xyzw[2]],
        )
        .sqrt();
        let r = [q_xyzw[3] / n, q_xyzw[0] / n, q_xyzw[1] / n, q_xyzw[2] / n];
        let d = qscale(qmul([0.0, t[0], t[1], t[2]], r), 0.5);
        Dq { r, d }
    }

    /// `(a.r + e a.d)(b.r + e b.d)` with `e^2 = 0`.
    pub fn dq_mul(a: Dq, b: Dq) -> Dq {
        Dq {
            r: qmul(a.r, b.r),
            d: qadd(qmul(a.r, b.d), qmul(a.d, b.r)),
        }
    }

    /// Inverse of a UNIT dual quaternion: the conjugate of both parts.
    pub fn dq_inv(a: Dq) -> Dq {
        Dq {
            r: qconj(a.r),
            d: qconj(a.d),
        }
    }

    /// World dual quaternions for local `(rotation xyzw, translation)` data, parent before child.
    pub fn world(parents: &[Option<usize>], locals: &[([f64; 4], [f64; 3])]) -> Vec<Dq> {
        let mut out: Vec<Dq> = Vec::new();
        for (i, p) in parents.iter().enumerate() {
            let l = from_trs(locals[i].0, locals[i].1);
            let w = match p {
                Some(pi) => dq_mul(out[*pi], l),
                None => l,
            };
            out.push(w);
        }
        out
    }

    /// Blend `(weight, dq)` terms: align every term to the first by the sign of the 4-D dot
    /// product of the real parts, sum, then normalize by DUAL-NUMBER division. Returns the unit
    /// dual quaternion and the number of terms that were sign-flipped.
    pub fn blend(terms: &[(f64, Dq)]) -> (Dq, usize) {
        let pivot = terms[0].1.r;
        let mut r = [0.0f64; 4];
        let mut d = [0.0f64; 4];
        let mut flips = 0usize;
        for &(w, dq) in terms {
            let sign = if qdot(dq.r, pivot) < 0.0 {
                flips += 1;
                -1.0
            } else {
                1.0
            };
            r = qadd(r, qscale(dq.r, w * sign));
            d = qadd(d, qscale(dq.d, w * sign));
        }
        // The dual number norm is a + e b with a = |r|, b = (r . d) / |r|. Its inverse is
        // 1/a - e b / a^2, multiplied in as a dual number.
        let a = qdot(r, r).sqrt();
        let b = qdot(r, d) / a;
        let inv_a = 1.0 / a;
        let inv_b = -b / (a * a);
        let r_out = qscale(r, inv_a);
        let d_out = qadd(qscale(d, inv_a), qscale(r, inv_b));
        (Dq { r: r_out, d: d_out }, flips)
    }

    /// The sandwich `q (1 + e p) q*c`, with `q*c = (conj r, -conj d)`; the point is the vector
    /// part of the dual component of the result.
    pub fn apply_point(q: Dq, p: [f64; 3]) -> [f64; 3] {
        let pt = Dq {
            r: [1.0, 0.0, 0.0, 0.0],
            d: [0.0, p[0], p[1], p[2]],
        };
        let qc = Dq {
            r: qconj(q.r),
            d: qscale(qconj(q.d), -1.0),
        };
        let res = dq_mul(dq_mul(q, pt), qc);
        [res.d[1], res.d[2], res.d[3]]
    }

    /// A direction: `r n r*`, no translation.
    pub fn apply_normal(q: Dq, n: [f64; 3]) -> [f64; 3] {
        let res = qmul(qmul(q.r, [0.0, n[0], n[1], n[2]]), qconj(q.r));
        [res[1], res[2], res[3]]
    }
}

use dq_oracle as oracle;

// ---------------------------------------------------------------------------------------------
// Fixtures: a deterministic generator, then a 2-bone and a 3-bone rig with 4-influence vertices.
// ---------------------------------------------------------------------------------------------

/// The deterministic generator of the fixtures (a 32-bit linear congruential generator; the
/// model `docs/evidence/m5c/model_dqs.py` restates it).
struct Lcg(u32);

impl Lcg {
    fn next_u24(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }

    /// A value in `[0, 1)`, exact in f32 (24 bits over 2^24).
    fn unit(&mut self) -> f32 {
        self.next_u24() as f32 / 16_777_216.0
    }
}

const NVERTS: usize = 32;
const NJOINTS: usize = 4;

type PoseJoint = ([f32; 3], f32, [f32; 3]); // axis, degrees, translation delta from the rest

struct Fixture {
    name: &'static str,
    parents: [Option<usize>; NJOINTS],
    rest_t: [[f32; 3]; NJOINTS],
    rest_rot: [([f32; 3], f32); NJOINTS],
    seed: u32,
    poses: Vec<[PoseJoint; NJOINTS]>,
}

/// Root, an upper and a lower bone in a chain, and a helper joint on the root: two bones plus
/// the root and a sibling, so that a vertex can have 4 distinct influences.
fn two_bone() -> Fixture {
    Fixture {
        name: "two_bone",
        parents: [None, Some(0), Some(1), Some(0)],
        rest_t: [
            [0.0, 0.0, 0.0],
            [0.1, 1.1, -0.05],
            [-0.05, 0.9, 0.1],
            [0.3, 0.2, 0.1],
        ],
        rest_rot: [
            ([0.0, 1.0, 0.0], 10.0),
            ([1.0, 2.0, 3.0], 20.0),
            ([0.0, 1.0, 1.0], -30.0),
            ([1.0, 0.0, 0.0], 45.0),
        ],
        seed: 12345,
        poses: vec![
            [
                ([0.0, 1.0, 0.0], 25.0, [0.0; 3]),
                ([1.0, 2.0, 3.0], 40.0, [0.0; 3]),
                ([0.0, 1.0, 1.0], -55.0, [0.0; 3]),
                ([1.0, 0.0, 0.0], 70.0, [0.0; 3]),
            ],
            [
                ([0.0, 1.0, 0.0], -35.0, [0.0; 3]),
                ([1.0, 2.0, 3.0], 110.0, [0.05, -0.03, 0.02]),
                ([0.0, 1.0, 1.0], -130.0, [0.0; 3]),
                ([1.0, 0.0, 0.0], 100.0, [0.0; 3]),
            ],
            [
                ([0.0, 1.0, 0.0], 60.0, [0.0; 3]),
                ([1.0, 2.0, 3.0], -95.0, [0.0; 3]),
                ([0.0, 1.0, 1.0], 170.0, [-0.04, 0.02, 0.0]),
                ([1.0, 0.0, 0.0], -105.0, [0.0; 3]),
            ],
        ],
    }
}

/// Root and three bones in a chain.
fn three_bone() -> Fixture {
    Fixture {
        name: "three_bone",
        parents: [None, Some(0), Some(1), Some(2)],
        rest_t: [
            [0.0, 0.0, 0.0],
            [0.05, 1.0, 0.02],
            [-0.04, 0.95, 0.06],
            [0.03, 0.85, -0.07],
        ],
        rest_rot: [
            ([0.0, 0.0, 1.0], 15.0),
            ([1.0, 0.0, 0.0], -25.0),
            ([0.0, 1.0, 0.0], 35.0),
            ([1.0, 1.0, 0.0], -40.0),
        ],
        seed: 54321,
        poses: vec![
            [
                ([0.0, 0.0, 1.0], 20.0, [0.0; 3]),
                ([1.0, 0.0, 0.0], 50.0, [0.0; 3]),
                ([0.0, 1.0, 0.0], 60.0, [0.0; 3]),
                ([1.0, 1.0, 0.0], -70.0, [0.0; 3]),
            ],
            [
                ([0.0, 0.0, 1.0], -40.0, [0.0; 3]),
                ([1.0, 0.0, 0.0], 120.0, [0.02, 0.04, 0.0]),
                ([0.0, 1.0, 0.0], -100.0, [0.0; 3]),
                ([1.0, 1.0, 0.0], 130.0, [0.0; 3]),
            ],
            [
                ([0.0, 0.0, 1.0], 75.0, [0.0; 3]),
                ([1.0, 0.0, 0.0], -110.0, [0.0; 3]),
                ([0.0, 1.0, 0.0], 140.0, [0.0, 0.0, 0.03]),
                ([1.0, 1.0, 0.0], -150.0, [0.0; 3]),
            ],
        ],
    }
}

type Mesh = (Vec<[f32; 3]>, Vec<Vec<(usize, f32)>>);

/// Positions and skin weights from the generator. Vertex `i` has `1 + i % 4` influences over a
/// shuffled joint order (so the first, pivot influence varies), with non-dyadic weights summing
/// to 1 within 1e-7.
fn make_mesh(seed: u32) -> Mesh {
    let mut g = Lcg(seed);
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut skin: Vec<Vec<(usize, f32)>> = Vec::new();
    for i in 0..NVERTS {
        let x = g.unit() * 3.0 - 1.5;
        let y = g.unit() * 4.0 - 0.5;
        let z = g.unit() * 3.0 - 1.5;
        pos.push([x, y, z]);
        let mut order = [0usize, 1, 2, 3];
        for k in (1..NJOINTS).rev() {
            let j = (g.next_u24() % (k as u32 + 1)) as usize;
            order.swap(k, j);
        }
        let cnt = 1 + (i % 4);
        let raw: Vec<f32> = (0..cnt).map(|_| g.unit() + 0.1).collect();
        let sum: f64 = raw.iter().map(|&r| f64::from(r)).sum();
        skin.push(
            (0..cnt)
                .map(|k| (order[k], (f64::from(raw[k]) / sum) as f32))
                .collect(),
        );
    }
    (pos, skin)
}

/// Unit normals from the generator.
fn make_normals(seed: u32) -> Vec<Vec3> {
    let mut g = Lcg(seed);
    (0..NVERTS)
        .map(|_| {
            let v = Vec3::new(
                g.unit() * 2.0 - 1.0,
                g.unit() * 2.0 - 1.0,
                g.unit() * 2.0 - 1.0,
            );
            v.try_normalize().expect("non-zero normal")
        })
        .collect()
}

fn fam_ref() -> FamilyRef {
    FamilyRef::new("synthetic.dqs", 1)
}

fn build_revision(fx: &Fixture) -> CharacterRevision {
    let (pos, skin) = make_mesh(fx.seed);
    let names = ["root", "b1", "b2", "b3"];
    let joints: Vec<Joint> = (0..NJOINTS)
        .map(|j| {
            Joint::new(
                names[j],
                fx.parents[j],
                Transform::new(
                    v3(fx.rest_t[j]),
                    quat_of(fx.rest_rot[j].0, fx.rest_rot[j].1),
                    ones(),
                ),
            )
        })
        .collect();
    let mut indices = Vec::new();
    for i in (0..30u32).step_by(3) {
        indices.extend_from_slice(&[i, i + 1, i + 2]);
    }
    let family = BodyFamily::new(
        fam_ref(),
        NeutralMesh::new(pos.iter().map(|&p| v3(p)).collect(), indices),
        joints,
        Vec::new(),
        skin,
    );
    compile(&BodyRecipe::new(fam_ref(), Vec::new()), &family).expect("fixture compiles")
}

fn pose_input(fx: &Fixture, pose: &[PoseJoint; NJOINTS]) -> PoseInput {
    let mut p = PoseInput::new();
    for (j, &(axis, deg, dt)) in pose.iter().enumerate() {
        let t = fx.rest_t[j];
        p = p.with_joint(
            j,
            Transform::new(
                Vec3::new(t[0] + dt[0], t[1] + dt[1], t[2] + dt[2]),
                quat_of(axis, deg),
                ones(),
            ),
        );
    }
    p
}

/// The oracle's per-joint skin dual quaternions `W_pose * W_bind^-1` for one pose.
fn oracle_skin_dqs(fx: &Fixture, pose: &[PoseJoint; NJOINTS]) -> Vec<oracle::Dq> {
    let q4 = |q: Quat| {
        [
            f64::from(q.x),
            f64::from(q.y),
            f64::from(q.z),
            f64::from(q.w),
        ]
    };
    let rest: Vec<([f64; 4], [f64; 3])> = (0..NJOINTS)
        .map(|j| {
            (
                q4(quat_of(fx.rest_rot[j].0, fx.rest_rot[j].1)),
                widen3(fx.rest_t[j]),
            )
        })
        .collect();
    let posed: Vec<([f64; 4], [f64; 3])> = (0..NJOINTS)
        .map(|j| {
            let (axis, deg, dt) = pose[j];
            let t = fx.rest_t[j];
            (
                q4(quat_of(axis, deg)),
                widen3([t[0] + dt[0], t[1] + dt[1], t[2] + dt[2]]),
            )
        })
        .collect();
    let wb = oracle::world(&fx.parents, &rest);
    let wp = oracle::world(&fx.parents, &posed);
    (0..NJOINTS)
        .map(|j| oracle::dq_mul(wp[j], oracle::dq_inv(wb[j])))
        .collect()
}

/// Oracle blend of vertex `v`'s influences.
fn oracle_vertex(skin_dqs: &[oracle::Dq], influences: &[(usize, f32)]) -> (oracle::Dq, usize) {
    let terms: Vec<(f64, oracle::Dq)> = influences
        .iter()
        .map(|&(j, w)| (f64::from(w), skin_dqs[j]))
        .collect();
    oracle::blend(&terms)
}

// ---------------------------------------------------------------------------------------------
// (a) Oracle agreement.
// ---------------------------------------------------------------------------------------------

/// Runs every pose of `fx` through the runtime and the oracle; returns the total number of
/// sign flips the oracle made (a premise: alignment must actually be exercised).
fn check_fixture(fx: &Fixture) -> usize {
    let rev = build_revision(fx);
    let (pos, skin) = make_mesh(fx.seed);
    let four = skin.iter().filter(|s| s.len() == 4).count();
    assert!(
        four >= 4,
        "{}: fixture must have vertices with 4 influences, has {four}",
        fx.name
    );
    let mut total_flips = 0usize;
    for (pi, pose) in fx.poses.iter().enumerate() {
        let input = pose_input(fx, pose);
        let dq = rev
            .evaluate_with(&input, SkinningMode::DualQuat)
            .unwrap_or_else(|e| panic!("{} pose {pi}: DualQuat failed: {e}", fx.name));
        let lin = rev
            .evaluate_with(&input, SkinningMode::Linear)
            .unwrap_or_else(|e| panic!("{} pose {pi}: Linear failed: {e}", fx.name));
        let sk = oracle_skin_dqs(fx, pose);
        let mut mode_gap = 0.0f64;
        for (v, infl) in skin.iter().enumerate() {
            let (blended, flips) = oracle_vertex(&sk, infl);
            total_flips += flips;
            let want = oracle::apply_point(blended, widen3(pos[v]));
            assert_close3(
                &format!("{} pose {pi} vertex {v} (DualQuat vs oracle)", fx.name),
                dq.positions[v],
                want,
            );
            let l = lin.positions[v];
            let d = dq.positions[v];
            for (a, b) in [(l.x, d.x), (l.y, d.y), (l.z, d.z)] {
                mode_gap = mode_gap.max((f64::from(a) - f64::from(b)).abs());
            }
        }
        assert!(
            mode_gap > MIN_MODE_GAP,
            "{} pose {pi}: Linear and DualQuat differ by only {mode_gap} (premise: > {MIN_MODE_GAP}); \
             the oracle comparison could not tell the modes apart",
            fx.name
        );
    }
    total_flips
}

/// (a) 2-bone fixture (plus root and a sibling joint), vertices with 4 influences, three poses
/// including rotations past 100 degrees. Kills A (`dqs_as_lbs`: the modes differ by more than
/// TOL on every pose, premise-checked). B (`dqs_no_align`) is killed here only if the extracted
/// quaternion signs of some influence pair are opposite, which depends on the extraction
/// branches and is not guaranteed; the deterministic B killers are the two alignment tests below.
#[test]
fn dualquat_matches_f64_oracle_on_two_bone_fixture() {
    let flips = check_fixture(&two_bone());
    assert!(
        flips > 0,
        "premise: the two-bone poses must exercise sign alignment"
    );
}

/// (a) 3-bone chain fixture, the same obligations (the oracle flips 5 terms in pose 2 under
/// the continuous-path sign convention). Kills A; B as above.
#[test]
fn dualquat_matches_f64_oracle_on_three_bone_fixture() {
    let flips = check_fixture(&three_bone());
    assert!(
        flips > 0,
        "premise: the three-bone poses must exercise sign alignment"
    );
}

/// Normals follow the blended rotation, stay unit length, and match the oracle. Kills A.
#[test]
fn dualquat_normals_match_oracle_and_stay_unit() {
    for fx in [two_bone(), three_bone()] {
        let rev = build_revision(&fx);
        let (_, skin) = make_mesh(fx.seed);
        let normals = make_normals(fx.seed ^ 0x5eed);
        // The last pose of each fixture is the strongest (flips, large angles).
        let pose = fx.poses.last().expect("poses");
        let input = pose_input(&fx, pose);
        let posed = rev
            .evaluate_with(&input, SkinningMode::DualQuat)
            .expect("DualQuat evaluates");
        let got = rev
            .skin
            .skin_normals(
                SkinningMode::DualQuat,
                &normals,
                &posed.joint_world,
                &rev.inverse_binds,
            )
            .expect("normals skin");
        let sk = oracle_skin_dqs(&fx, pose);
        for (v, infl) in skin.iter().enumerate() {
            let (blended, _) = oracle_vertex(&sk, infl);
            let n = normals[v];
            let want = oracle::apply_normal(blended, widen3([n.x, n.y, n.z]));
            assert_close3(&format!("{} normal {v}", fx.name), got[v], want);
            assert!(
                (f64::from(got[v].length()) - 1.0).abs() < 1.0e-4,
                "{} normal {v} is not unit length: {}",
                fx.name,
                got[v].length()
            );
        }
    }
}

/// `Linear` through `evaluate_with` is `evaluate` bit for bit, the default mode is Linear, and
/// evaluating never touches `rev_id`.
#[test]
fn linear_mode_is_evaluate_and_never_touches_rev_id() {
    assert_eq!(SkinningMode::default(), SkinningMode::Linear);
    let fx = two_bone();
    let rev = build_revision(&fx);
    let id = rev.rev_id;
    for pose in &fx.poses {
        let input = pose_input(&fx, pose);
        let a = rev.evaluate(&input).expect("evaluate");
        let b = rev
            .evaluate_with(&input, SkinningMode::Linear)
            .expect("evaluate_with Linear");
        assert_eq!(a, b, "Linear must be exactly evaluate");
        let _ = rev.evaluate_with(&input, SkinningMode::DualQuat);
    }
    assert_eq!(rev.rev_id, id);
    assert_eq!(rev.content_id(), id, "rev_id must stay the content id");
}

/// At the bind pose every skin matrix is the identity, so both modes return the neutral mesh.
#[test]
fn dualquat_at_bind_pose_is_the_neutral_mesh() {
    for fx in [two_bone(), three_bone()] {
        let rev = build_revision(&fx);
        let posed = rev
            .evaluate_with(&PoseInput::new(), SkinningMode::DualQuat)
            .expect("bind pose");
        for (v, (got, want)) in posed
            .positions
            .iter()
            .zip(rev.neutral.positions.iter())
            .enumerate()
        {
            assert_close3(
                &format!("{} bind vertex {v}", fx.name),
                *got,
                widen3([want.x, want.y, want.z]),
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// (b) The elbow cross-section check (plan: area ratio >= 0.85 at 120 degrees).
// ---------------------------------------------------------------------------------------------

const RING_N: usize = 16;
const RING_YS: [f32; 9] = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
/// Rings in the skin blend zone (weights 0.25, 0.5, 0.75 on the elbow joint); ring 4 is the mid
/// cross-section, at the joint.
const BLEND_RINGS: std::ops::RangeInclusive<usize> = 3..=5;
const MID_RING: usize = 4;
/// The plan's bar: the cross-section area ratio at 120 degrees of flexion.
const AREA_BAR: f64 = 0.85;

/// A cylinder of radius 0.2 along y in 9 rings of 16 vertices, skinned to a root and an elbow
/// joint at (0, 1, 0): rings at or below y = 0.5 follow the root, rings at or above 1.5 the
/// elbow, and the weight on the elbow ramps linearly between (dyadic weights, exact sums).
fn elbow_revision() -> CharacterRevision {
    let mut positions = Vec::new();
    let mut skin: Vec<Vec<(usize, f32)>> = Vec::new();
    for &y in &RING_YS {
        let t = ((y - 0.5) / 1.0).clamp(0.0, 1.0);
        for k in 0..RING_N {
            let a = 2.0 * std::f32::consts::PI * (k as f32) / (RING_N as f32);
            positions.push(Vec3::new(0.2 * a.cos(), y, 0.2 * a.sin()));
            skin.push(if t == 0.0 {
                vec![(0, 1.0)]
            } else if t == 1.0 {
                vec![(1, 1.0)]
            } else {
                vec![(0, 1.0 - t), (1, t)]
            });
        }
    }
    let mut indices = Vec::new();
    for r in 0..(RING_YS.len() - 1) {
        for k in 0..RING_N {
            let a = (r * RING_N + k) as u32;
            let b = (r * RING_N + (k + 1) % RING_N) as u32;
            let c = a + RING_N as u32;
            let d = b + RING_N as u32;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    let joints = vec![
        Joint::new("root", None, Transform::IDENTITY),
        Joint::new(
            "elbow",
            Some(0),
            Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        ),
    ];
    let family = BodyFamily::new(
        fam_ref(),
        NeutralMesh::new(positions, indices),
        joints,
        Vec::new(),
        skin,
    );
    compile(&BodyRecipe::new(fam_ref(), Vec::new()), &family).expect("elbow compiles")
}

/// Area of the polygon through `ring` (the magnitude of its vector area), in f64.
fn ring_area(ring: &[Vec3]) -> f64 {
    let mut a = [0.0f64; 3];
    let n = ring.len();
    for i in 0..n {
        let p = ring[i];
        let q = ring[(i + 1) % n];
        let (px, py, pz) = (f64::from(p.x), f64::from(p.y), f64::from(p.z));
        let (qx, qy, qz) = (f64::from(q.x), f64::from(q.y), f64::from(q.z));
        a[0] += py * qz - pz * qy;
        a[1] += pz * qx - px * qz;
        a[2] += px * qy - py * qx;
    }
    0.5 * (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// Per-ring area of the posed ring over the area of the same ring at rest.
fn ring_ratios(rev: &CharacterRevision, deg: f32, mode: SkinningMode) -> Vec<f64> {
    let pose = PoseInput::new().with_joint(
        1,
        Transform::new(
            Vec3::new(0.0, 1.0, 0.0),
            Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), deg.to_radians()),
            ones(),
        ),
    );
    let posed = rev
        .evaluate_with(&pose, mode)
        .unwrap_or_else(|e| panic!("elbow {deg} deg {mode:?}: {e}"));
    (0..RING_YS.len())
        .map(|r| {
            let lo = r * RING_N;
            let hi = lo + RING_N;
            ring_area(&posed.positions[lo..hi]) / ring_area(&rev.neutral.positions[lo..hi])
        })
        .collect()
}

fn min_blend_ratio(ratios: &[f64]) -> f64 {
    BLEND_RINGS.map(|r| ratios[r]).fold(f64::INFINITY, f64::min)
}

/// The plan's cross-section check. At 120 degrees the DualQuat blend zone keeps at least 0.85 of
/// the rest area (analytically exactly 1: each ring moves rigidly), and the SAME check applied to
/// Linear FAILS (the control; analytically cos 60 = 0.5 at the joint). At 20 degrees the check
/// passes for both, so it does not fail merely by existing. Kills A (`dqs_as_lbs`).
#[test]
fn elbow_cross_section_dualquat_keeps_area_and_linear_collapses() {
    let rev = elbow_revision();

    let dq = ring_ratios(&rev, 120.0, SkinningMode::DualQuat);
    let lin = ring_ratios(&rev, 120.0, SkinningMode::Linear);
    let dq_min = min_blend_ratio(&dq);
    let lin_min = min_blend_ratio(&lin);
    assert!(
        dq_min >= AREA_BAR,
        "DualQuat must keep the elbow cross-section: min ratio {dq_min} < {AREA_BAR}"
    );
    assert!(
        lin_min < AREA_BAR,
        "control: Linear must fail the same check: min ratio {lin_min} >= {AREA_BAR} \
         (the check cannot discriminate)"
    );
    // Analytic values: DualQuat moves every ring rigidly (ratio 1); Linear at the joint is
    // |0.5 + 0.5 e^{i 120deg}| = cos 60deg = 0.5.
    assert!(
        (dq[MID_RING] - 1.0).abs() < 1.0e-3,
        "DualQuat mid ratio {} should be 1",
        dq[MID_RING]
    );
    assert!(
        (lin[MID_RING] - 0.5).abs() < 1.0e-3,
        "Linear mid ratio {} should be 0.5",
        lin[MID_RING]
    );

    // Calibration at a small bend: both pass, and Linear is the analytic cos(10 deg).
    let dq20 = ring_ratios(&rev, 20.0, SkinningMode::DualQuat);
    let lin20 = ring_ratios(&rev, 20.0, SkinningMode::Linear);
    assert!(min_blend_ratio(&dq20) >= AREA_BAR);
    assert!(min_blend_ratio(&lin20) >= AREA_BAR);
    assert!(
        (lin20[MID_RING] - 10.0f64.to_radians().cos()).abs() < 1.0e-3,
        "Linear mid ratio at 20 deg {} should be cos 10deg",
        lin20[MID_RING]
    );
}

// ---------------------------------------------------------------------------------------------
// (c) Antipodal / sign alignment.
// ---------------------------------------------------------------------------------------------

fn rigid(axis: [f32; 3], deg: f32, t: [f32; 3]) -> DualQuat {
    DualQuat::from_rigid(quat_of(axis, deg), v3(t)).expect("rigid dual quaternion")
}

fn negated(d: DualQuat) -> DualQuat {
    DualQuat {
        real: d.real.neg(),
        dual: d.dual.neg(),
    }
}

fn oracle_of(axis: [f32; 3], deg: f32, t: [f32; 3]) -> oracle::Dq {
    let q = quat_of(axis, deg);
    oracle::from_trs(
        [
            f64::from(q.x),
            f64::from(q.y),
            f64::from(q.z),
            f64::from(q.w),
        ],
        widen3(t),
    )
}

/// `-q` is the same transform as `q`: negating any non-first term of a blend must not change the
/// result (it is exactly the sign alignment that guarantees this), including for pairs whose
/// quaternions are on opposite hemispheres (rotations more than 180 degrees apart as 4-D
/// vectors). Analytic anchor: blending 100 and -100 degrees about z equally gives the BISECTOR
/// of the shorter way round, a half turn, not the identity. Kills B (`dqs_no_align`).
#[test]
fn blend_is_invariant_to_the_sign_of_a_later_term_and_takes_the_short_way() {
    let z = [0.0, 0.0, 1.0];
    // (first, second, weights, true when the raw quaternion dot product must be negative)
    let cases: [(DualQuat, DualQuat, (f32, f32), bool, oracle::Dq, oracle::Dq); 3] = [
        (
            rigid(z, 100.0, [0.0; 3]),
            rigid(z, -100.0, [0.0; 3]),
            (0.5, 0.5),
            true,
            oracle_of(z, 100.0, [0.0; 3]),
            oracle_of(z, -100.0, [0.0; 3]),
        ),
        (
            rigid([1.0, 2.0, 3.0], 30.0, [0.2, -0.1, 0.3]),
            rigid([1.0, 2.0, 3.0], -170.0, [-0.3, 0.4, 0.1]),
            (0.3, 0.7),
            true,
            oracle_of([1.0, 2.0, 3.0], 30.0, [0.2, -0.1, 0.3]),
            oracle_of([1.0, 2.0, 3.0], -170.0, [-0.3, 0.4, 0.1]),
        ),
        (
            rigid([0.0, 1.0, 0.0], 80.0, [0.1, 0.2, 0.3]),
            rigid([0.0, 1.0, 0.0], 20.0, [0.0, -0.2, 0.4]),
            (0.6, 0.4),
            false,
            oracle_of([0.0, 1.0, 0.0], 80.0, [0.1, 0.2, 0.3]),
            oracle_of([0.0, 1.0, 0.0], 20.0, [0.0, -0.2, 0.4]),
        ),
    ];
    let probes = [[1.1f32, -0.7, 0.4], [-0.3, 0.9, -1.2], [0.0, 0.0, 0.0]];
    for (i, (a, b, (wa, wb), opposite, oa, ob)) in cases.iter().enumerate() {
        let dot = a.real.dot(b.real);
        assert_eq!(
            dot < 0.0,
            *opposite,
            "case {i}: premise on the raw quaternion dot product ({dot})"
        );
        let pos = DualQuat::blend(&[(*wa, *a), (*wb, *b)]).expect("blend");
        let neg = DualQuat::blend(&[(*wa, *a), (*wb, negated(*b))]).expect("blend");
        assert_eq!(
            pos, neg,
            "case {i}: negating the second term changed the blend"
        );
        let (ob_blend, flips) = oracle::blend(&[(f64::from(*wa), *oa), (f64::from(*wb), *ob)]);
        assert_eq!(flips > 0, *opposite, "case {i}: oracle flip count");
        for probe in probes {
            let got = pos.transform_point(v3(probe));
            let want = oracle::apply_point(ob_blend, widen3(probe));
            assert_close3(&format!("case {i} probe {probe:?}"), got, want);
        }
    }
    // Analytic anchor for case 0: the blended rotation is a half turn about z.
    let half = DualQuat::blend(&[
        (0.5, rigid(z, 100.0, [0.0; 3])),
        (0.5, rigid(z, -100.0, [0.0; 3])),
    ])
    .expect("blend");
    assert_close3(
        "half turn",
        half.transform_point(Vec3::new(1.0, 0.0, 0.0)),
        [-1.0, 0.0, 0.0],
    );
}

/// Sweeping one influence's rotation across the point where the extracted quaternion changes
/// sign (the trace crosses zero at 120 degrees about -z, where the extraction switches from
/// `w > 0` to the dominant-axis branch) keeps the blended point on the analytic curve: with
/// equal weights the blended rotation is the bisector, angle `(70 + theta) / 2` about -z. The
/// relative angle between the two influences stays under 100 degrees, so there is no genuine
/// 180-degree ambiguity in the sweep. Kills B (`dqs_no_align`: a jump of order 1 at the flip).
#[test]
fn sweep_across_a_quaternion_sign_flip_is_continuous() {
    let axis = Vec3::new(0.0, 0.0, -1.0);
    let rot = |deg: f32| {
        Transform::new(
            Vec3::ZERO,
            Quat::from_axis_angle(axis, deg.to_radians()),
            ones(),
        )
    };
    let skin = SkinWeights::new(&[vec![(0, 0.5), (1, 0.5)]], 2).expect("weights");
    let p = Vec3::new(1.5, 0.3, 0.2);
    let ibs = [Mat4::IDENTITY, Mat4::IDENTITY];
    let first_deg = 70.0f32;
    let step = 0.5f32;
    let mut prev_raw: Option<Quat> = None;
    let mut prev_out: Option<Vec3> = None;
    let mut saw_raw_flip = false;
    let mut theta = 100.0f32;
    while theta <= 140.0 {
        let world = [rot(first_deg).to_mat(), rot(theta).to_mat()];
        let raw = DualQuat::from_mat4(&world[1]).expect("rigid").real;
        if let Some(pr) = prev_raw {
            if raw.dot(pr) < 0.0 {
                saw_raw_flip = true;
            }
        }
        prev_raw = Some(raw);

        let out = skin
            .skin_with(SkinningMode::DualQuat, &[p], &world, &ibs)
            .expect("DualQuat skins")[0];
        // Analytic: rotation about -z by phi = (70 + theta) / 2 degrees.
        let phi = f64::from((first_deg + theta) / 2.0).to_radians();
        let (c, s) = (phi.cos(), phi.sin());
        let (x, y) = (f64::from(p.x), f64::from(p.y));
        assert_close3(
            &format!("theta {theta}"),
            out,
            [x * c + y * s, -x * s + y * c, f64::from(p.z)],
        );
        if let Some(po) = prev_out {
            // Theory: |step| <= 0.5 * |p| * dtheta = 0.0067; the bound allows 2x.
            let bound = f64::from(p.length()) * f64::from(step).to_radians();
            let d = out.sub(po);
            assert!(
                f64::from(d.length()) <= bound,
                "theta {theta}: jump {} exceeds {bound}",
                d.length()
            );
        }
        prev_out = Some(out);
        theta += step;
    }
    assert!(
        saw_raw_flip,
        "premise: the sweep must cross a sign change of the extracted quaternion"
    );
}

/// Blends that cancel to nothing are `None`, never a NaN or an identity in disguise. Exact
/// opposite real parts that are both at dot 0 with the (zero-weighted) first term are not
/// flipped, so they cancel.
#[test]
fn a_cancelled_blend_is_none() {
    let first = DualQuat::from_rigid(Quat::new(0.0, 0.0, 1.0, 0.0), Vec3::ZERO).expect("rigid");
    let b = DualQuat::from_rigid(Quat::new(1.0, 0.0, 0.0, 0.0), Vec3::ZERO).expect("rigid");
    let c = DualQuat::from_rigid(Quat::new(-1.0, 0.0, 0.0, 0.0), Vec3::ZERO).expect("rigid");
    assert!(DualQuat::blend(&[(0.0, first), (0.5, b), (0.5, c)]).is_none());
    assert!(DualQuat::blend(&[]).is_none());
}

// ---------------------------------------------------------------------------------------------
// The scale decision: reject, never drop.
// ---------------------------------------------------------------------------------------------

fn small_revision() -> CharacterRevision {
    let joints = vec![
        Joint::new("root", None, Transform::IDENTITY),
        Joint::new(
            "arm",
            Some(0),
            Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        ),
        Joint::new(
            "spare",
            Some(0),
            Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
        ),
    ];
    let positions = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.2),
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.3, 1.5, -0.1),
    ];
    let skin = vec![
        vec![(0, 1.0)],
        vec![(0, 0.5), (1, 0.5)],
        vec![(1, 1.0)],
        vec![(0, 0.25), (1, 0.75)],
    ];
    let family = BodyFamily::new(
        fam_ref(),
        NeutralMesh::new(positions, vec![0, 1, 2, 1, 2, 3]),
        joints,
        Vec::new(),
        skin,
    );
    compile(&BodyRecipe::new(fam_ref(), Vec::new()), &family).expect("small compiles")
}

/// A scaled, referenced joint makes its skin matrix non-rigid: DualQuat returns the typed
/// `NonRigid` error for that joint while Linear still evaluates; a scaled joint that no vertex
/// references does not fail the mode. Kills A (`dqs_as_lbs` would return Ok).
#[test]
fn a_non_rigid_skin_matrix_is_rejected_not_dropped() {
    let rev = small_revision();
    let arm_rest = Vec3::new(0.0, 1.0, 0.0);
    let scaled = PoseInput::new().with_joint(
        1,
        Transform::new(arm_rest, Quat::IDENTITY, Vec3::new(2.0, 2.0, 2.0)),
    );
    assert!(
        rev.evaluate_with(&scaled, SkinningMode::Linear).is_ok(),
        "Linear must still evaluate a scaled joint"
    );
    let err = rev
        .evaluate_with(&scaled, SkinningMode::DualQuat)
        .expect_err("DualQuat must refuse a scaled referenced joint");
    assert!(
        matches!(err, EvalError::Skin(SkinError::NonRigid { joint: 1 })),
        "wrong error: {err:?}"
    );

    // Joint 2 is referenced by no vertex: scaling it is not an error for DualQuat.
    let spare_scaled = PoseInput::new().with_joint(
        2,
        Transform::new(
            Vec3::new(1.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::new(3.0, 3.0, 3.0),
        ),
    );
    let posed = rev
        .evaluate_with(&spare_scaled, SkinningMode::DualQuat)
        .expect("an unreferenced scaled joint must not fail");
    for (v, (got, want)) in posed
        .positions
        .iter()
        .zip(rev.neutral.positions.iter())
        .enumerate()
    {
        assert_close3(
            &format!("spare-scaled vertex {v}"),
            *got,
            widen3([want.x, want.y, want.z]),
        );
    }
}

/// Hand-computed values for the helpers. Pure translation: exact. A quarter turn about z plus a
/// translation. A matrix round trip. Non-rigid and non-affine matrices have no dual quaternion.
#[test]
fn dual_quat_helpers_match_hand_values() {
    let t = DualQuat::from_rigid(Quat::IDENTITY, Vec3::new(1.0, 2.0, 3.0)).expect("rigid");
    assert_eq!(t.real, Quat::IDENTITY);
    assert_eq!(t.dual, Quat::new(0.5, 1.0, 1.5, 0.0));
    assert_eq!(t.translation(), Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(
        t.transform_point(Vec3::new(1.0, 1.0, 1.0)),
        Vec3::new(2.0, 3.0, 4.0)
    );
    assert_eq!(
        t.transform_vector(Vec3::new(1.0, 1.0, 1.0)),
        Vec3::new(1.0, 1.0, 1.0)
    );

    let q = rigid([0.0, 0.0, 1.0], 90.0, [0.0, 0.0, 5.0]);
    assert_close3(
        "quarter turn",
        q.transform_point(Vec3::new(1.0, 0.0, 0.0)),
        [0.0, 1.0, 5.0],
    );

    let tr = Transform::new(
        Vec3::new(0.3, -0.7, 1.1),
        quat_of([1.0, 2.0, 3.0], 77.0),
        ones(),
    );
    let m = tr.to_mat();
    let dq = DualQuat::from_mat4(&m).expect("rigid matrix");
    for probe in [[1.0f32, 0.5, -0.25], [0.0, 0.0, 0.0], [-2.0, 1.0, 3.0]] {
        let want = m.transform_point(v3(probe));
        assert_close3(
            "matrix round trip",
            dq.transform_point(v3(probe)),
            [f64::from(want.x), f64::from(want.y), f64::from(want.z)],
        );
    }
    // The unit constraint: |real| = 1 and real . dual = 0.
    assert!((f64::from(dq.real.length_squared()) - 1.0).abs() < 1.0e-6);
    assert!(f64::from(dq.real.dot(dq.dual)).abs() < 1.0e-6);

    let scaled = Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::new(1.0, 2.0, 1.0)).to_mat();
    assert!(DualQuat::from_mat4(&scaled).is_none(), "non-uniform scale");
    let mirrored = Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::new(-1.0, 1.0, 1.0)).to_mat();
    assert!(DualQuat::from_mat4(&mirrored).is_none(), "reflection");
    let mut projective = Mat4::IDENTITY;
    projective.cols[0][3] = 0.5;
    assert!(DualQuat::from_mat4(&projective).is_none(), "not affine");

    // Normalization makes the unit constraint hold for a raw sum.
    let sum = rigid([0.0, 1.0, 0.0], 40.0, [0.4, 0.0, 0.1])
        .scale(0.7)
        .add(rigid([0.0, 1.0, 0.0], 10.0, [0.0, 0.3, 0.2]).scale(0.3));
    let unit = sum.try_normalize().expect("normalizes");
    assert!((f64::from(unit.real.length_squared()) - 1.0).abs() < 1.0e-6);
    assert!(f64::from(unit.real.dot(unit.dual)).abs() < 1.0e-6);
}
