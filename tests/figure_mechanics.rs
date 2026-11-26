//! M5-B Slice 1 geometry core: skeleton, skinning, morphs and `CharacterRevision`.
//!
//! Two synthetic fixtures live here.
//! - The SIMPLE fixture (`fixture_family`, 3 joints, dyadic values, identity rest rotations) is
//!   hand-derivable. The hand-computed tests and the validation tests use it.
//! - The RICH fixture (`rich_family`, 4 joints, 5 controls) is the strength fixture that the
//!   oracle, the direct world-bind / inverse-bind comparisons, the landmark checks and the
//!   golden use: non-identity rest rotations on every chain, a non-uniformly scaled joint,
//!   non-dyadic lengths, weights and control values, a 4-influence vertex, skin weight sums
//!   that are inside the 1e-4 tolerance but not exact, an unsorted influence order, an
//!   unsorted morph delta list with a zero delta, and two controls on one morph target.
//!   `rich_fixture_meets_the_required_strength` fails if any of that is weakened.
//!
//! The independent f64 oracle lives in `tests/figure_oracle` and shares no code with
//! `gibson::figure::math`.
//!
//! Mutation controls (`RUSTFLAGS='--cfg figure_mutant="..."'`) are caught by these ordinary
//! behavioural tests, none of which inspect the cfg. Each test's doc comment lists the mutants
//! it is designed to kill: A = `ignored_skeleton_update`, B = `stale_bind`, C = `noop_morph`.
//! Every mutant is killed by at least one test that is NOT the golden. The predicted kill
//! matrix is `docs/evidence/m5b/KILL_MATRIX_PREDICTED.md` (a prediction until it is run under
//! heavy). The `golden_rev_id` test is a REGRESSION PIN (not independent).
#![cfg(feature = "figure")]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::excessive_precision)]

mod figure_oracle;

use std::f64::consts::PI;

use figure_oracle as oracle;
use gibson::figure::math::{Mat4, Quat, Transform, Vec3};
use gibson::figure::morph::{apply_morphs, MorphError, MorphTarget};
use gibson::figure::revision::{
    compile, BodyFamily, BodyRecipe, CharacterRevision, CompileError, ControlDef, ControlEffect,
    EvalError, FamilyRef, FitStatus, GarmentFit, NeutralMesh, PoseInput, SupportClass,
};
use gibson::figure::skeleton::{Joint, Landmark, Skeleton, SkeletonError, MAX_JOINTS};
use gibson::figure::skin::{SkinError, SkinWeights};
use sha2::Digest as _;

/// Comparison tolerance, DECLARED BEFORE any comparison and never tuned to results.
///
/// Absolute-or-relative: `|impl - oracle| <= TOL * max(1, |oracle|)`.
///
/// Error bound. The f32 unit roundoff is `u = 2^-24 = 5.96e-8` (epsilon = 2u = 1.19e-7 is the
/// spacing at 1, not the per-operation error). Each rounded operation adds a relative error of
/// at most `u`, so a dependent chain of `N` operations over quantities whose magnitudes sum to
/// at most `S` has absolute error at most about `N * u * S` (first order, worst case). A
/// matrix-entry product is an inner product of length 4: 4 multiplications and 3 additions =
/// 7 flops (not 12). The longest dependent chain, for a vertex skinned to joint 2 or 3
/// (world = L0 * L1 * Lj), counted along the critical path:
///   - `from_transform`: 4 (squared length) + 1 (sqrt) + 1 (reciprocal) + 1 (normalize)
///     + 4 (matrix entry from products, sum, `2*`, `1-`) + 1 (column scale) = 12
///   - two matrix products for the world chain: 2 * 7 = 14            -> 26 so far
///   - affine inverse: 3x3 block 9 (cofactor 3, determinant 5, divide 1) + translation 6 = 15
///     -> 41 on the inverse-bind path
///   - `W * IB`: 7 -> 48; point transform: 3 mul + 3 add = 6 -> 54
///   - blend of up to 4 influences (1 mul + 3 add) = 4 -> 58; the morphed input position
///     (weight accumulation, mul, add) adds 4 -> 62, rounded up to N = 64.
/// Fixture magnitudes: |coordinate| <= 3.4, |inverse-bind entry| <= 3.2 (largest over the
/// control sets used), world entries <= 2.6 and a 3-term product-and-sum, so S <= 12.
/// Then `N * u * S = 64 * 5.96e-8 * 12 = 4.6e-5`. TOL = 1e-4 is 2.2x above that worst case and
/// is 500x below the smallest effect any test relies on (0.05).
///
/// What TOL cannot see: a skin weight sum of `1 +- 5e-5` (inside the 1e-4 validation
/// tolerance) changes a coordinate by at most `5e-5 * |x|`, below `TOL * max(1, |x|)`. So
/// "no renormalization" is NOT proved by an oracle comparison; the dedicated test
/// `weights_are_applied_as_declared_never_renormalized` uses an absolute tolerance instead.
const TOL: f64 = 1.0e-4;

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

// ---------------------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------------------

fn fam_ref() -> FamilyRef {
    FamilyRef::new("synthetic.tube", 1)
}

/// Rings at y = 0, 1, 2, 3; four vertices per ring: (-,-), (+,-), (+,+), (-,+) in (x, z).
fn ring_pos(r: usize, k: usize) -> [f32; 3] {
    let sx = if k == 0 || k == 3 { -0.25 } else { 0.25 };
    let sz = if k < 2 { -0.25 } else { 0.25 };
    [sx, r as f32, sz]
}

fn fixture_positions() -> Vec<[f32; 3]> {
    (0..4)
        .flat_map(|r| (0..4).map(move |k| ring_pos(r, k)))
        .collect()
}

fn fixture_indices() -> Vec<u32> {
    let mut idx = Vec::new();
    for r in 0..3u32 {
        for k in 0..4u32 {
            let a = r * 4 + k;
            let b = r * 4 + (k + 1) % 4;
            let c = a + 4;
            let d = b + 4;
            idx.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    idx
}

/// Weights are exact binary fractions summing to exactly 1.
fn fixture_skin() -> Vec<Vec<(usize, f32)>> {
    let mut skin: Vec<Vec<(usize, f32)>> = Vec::new();
    for r in 0..4 {
        for k in 0..4 {
            skin.push(match (r, k) {
                (0, _) => vec![(0, 1.0)],
                (1, _) => vec![(0, 0.5), (1, 0.5)],
                (2, 0) => vec![(0, 0.125), (1, 0.375), (2, 0.5)],
                (2, 1) => vec![(2, 0.75), (1, 0.25)],
                (2, 2) => vec![(1, 0.5), (2, 0.5)],
                (2, _) => vec![(1, 0.25), (2, 0.75)],
                _ => vec![(2, 1.0)],
            });
        }
    }
    skin
}

/// "girth": push the middle rings (vertices 4..12) radially by 0.1 in x and z.
fn fixture_girth_deltas() -> Vec<(u32, [f32; 3])> {
    (4..12u32)
        .map(|v| {
            let k = (v % 4) as usize;
            let sx = if k == 0 || k == 3 { -0.1 } else { 0.1 };
            let sz = if k < 2 { -0.1 } else { 0.1 };
            (v, [sx, 0.0, sz])
        })
        .collect()
}

fn fixture_family() -> BodyFamily {
    fixture_family_from(fixture_positions())
}

fn fixture_family_from(neutral_positions: Vec<[f32; 3]>) -> BodyFamily {
    let joints = vec![
        Joint::new(
            "root",
            None,
            Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
        ),
        Joint::new(
            "upper",
            Some(0),
            Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        ),
        Joint::new(
            "lower",
            Some(1),
            Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        ),
    ];
    let landmarks = vec![
        Landmark::new("head_top", 2, Vec3::new(0.0, 1.0, 0.0)),
        Landmark::new("upper_mid", 1, Vec3::new(0.0, 0.5, 0.0)),
    ];
    let positions: Vec<Vec3> = neutral_positions
        .iter()
        .map(|p| Vec3::new(p[0], p[1], p[2]))
        .collect();
    let girth = MorphTarget::new(
        "girth",
        fixture_girth_deltas()
            .iter()
            .map(|&(v, d)| (v, Vec3::new(d[0], d[1], d[2])))
            .collect(),
    );
    BodyFamily::new(
        fam_ref(),
        NeutralMesh::new(positions, fixture_indices()),
        joints,
        landmarks,
        fixture_skin(),
    )
    .with_morphs(vec![girth])
    .with_controls(vec![
        ControlDef::new(
            "upper_length",
            0.5,
            2.0,
            1.0,
            vec![ControlEffect::BoneLengthScale { joint: 1 }],
        ),
        ControlDef::new(
            "girth",
            -1.0,
            2.0,
            0.0,
            vec![ControlEffect::MorphWeight { target: 0 }],
        ),
    ])
}

fn widen3(p: [f32; 3]) -> [f64; 3] {
    [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
}

fn identity_trs(t: [f64; 3]) -> oracle::OTrs {
    oracle::OTrs {
        t,
        q: [0.0, 0.0, 0.0, 1.0],
        s: [1.0, 1.0, 1.0],
    }
}

/// The oracle's view of the same fixture, built from the same plain arrays.
fn fixture_oracle() -> oracle::ORig {
    oracle::ORig {
        joints: vec![
            oracle::OJoint {
                parent: None,
                rest: identity_trs([0.0, 0.0, 0.0]),
            },
            oracle::OJoint {
                parent: Some(0),
                rest: identity_trs([0.0, 1.0, 0.0]),
            },
            oracle::OJoint {
                parent: Some(1),
                rest: identity_trs([0.0, 1.0, 0.0]),
            },
        ],
        positions: fixture_positions().into_iter().map(widen3).collect(),
        skin: fixture_skin()
            .into_iter()
            .map(|l| l.into_iter().map(|(j, w)| (j, f64::from(w))).collect())
            .collect(),
        morphs: vec![fixture_girth_deltas()
            .into_iter()
            .map(|(v, d)| (v as usize, widen3(d)))
            .collect()],
        controls: vec![
            oracle::OControl {
                default: 1.0,
                bone_scale_of_children_of: Some(1),
                morph_target: None,
            },
            oracle::OControl {
                default: 0.0,
                bone_scale_of_children_of: None,
                morph_target: Some(0),
            },
        ],
        landmarks: vec![(2, [0.0, 1.0, 0.0]), (1, [0.0, 0.5, 0.0])],
    }
}

fn recipe(upper_length: f32, girth: f32) -> BodyRecipe {
    BodyRecipe::new(
        fam_ref(),
        vec![
            ("upper_length".to_string(), upper_length),
            ("girth".to_string(), girth),
        ],
    )
}

fn build(upper_length: f32, girth: f32) -> CharacterRevision {
    compile(&recipe(upper_length, girth), &fixture_family()).expect("fixture compiles")
}

/// Unit quaternion `[x, y, z, w]` for an axis and angle, computed in f64 (test-local, not the
/// library routine) and rounded to f32.
fn axis_angle(axis: [f64; 3], angle: f64) -> [f32; 4] {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let (s, c) = (angle / 2.0).sin_cos();
    [
        (axis[0] / n * s) as f32,
        (axis[1] / n * s) as f32,
        (axis[2] / n * s) as f32,
        c as f32,
    ]
}

const NO_ROT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const UNIT: [f32; 3] = [1.0, 1.0, 1.0];

/// One pose override: joint, translation, quaternion xyzw, scale.
type Ovr = (usize, [f32; 3], [f32; 4], [f32; 3]);

struct Case {
    name: &'static str,
    upper_length: f32,
    girth: f32,
    overrides: Vec<Ovr>,
    pose_girth: Option<f32>,
}

fn to_pose(case: &Case) -> PoseInput {
    let mut p = PoseInput::new();
    for &(j, t, q, s) in &case.overrides {
        p = p.with_joint(
            j,
            Transform::new(
                Vec3::new(t[0], t[1], t[2]),
                Quat::new(q[0], q[1], q[2], q[3]),
                Vec3::new(s[0], s[1], s[2]),
            ),
        );
    }
    if let Some(w) = case.pose_girth {
        p = p.with_morph("girth", w);
    }
    p
}

fn to_oracle_pose(case: &Case) -> oracle::OPose {
    oracle::OPose {
        locals: case
            .overrides
            .iter()
            .map(|&(j, t, q, s)| {
                (
                    j,
                    oracle::OTrs {
                        t: widen3(t),
                        q: [
                            f64::from(q[0]),
                            f64::from(q[1]),
                            f64::from(q[2]),
                            f64::from(q[3]),
                        ],
                        s: widen3(s),
                    },
                )
            })
            .collect(),
        morph_weights: case
            .pose_girth
            .map(|w| vec![(0usize, f64::from(w))])
            .unwrap_or_default(),
    }
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "rest",
            upper_length: 1.0,
            girth: 0.0,
            overrides: vec![],
            pose_girth: None,
        },
        Case {
            name: "upper z90",
            upper_length: 1.0,
            girth: 0.0,
            overrides: vec![(
                1,
                [0.0, 1.0, 0.0],
                axis_angle([0.0, 0.0, 1.0], PI / 2.0),
                UNIT,
            )],
            pose_girth: None,
        },
        // Cases below were written alongside the implementation.
        Case {
            name: "long upper, mixed rotations, non-uniform scale",
            upper_length: 1.5,
            girth: 0.5,
            overrides: vec![
                (0, [0.0, 0.0, 0.0], axis_angle([0.0, 1.0, 0.0], 0.3), UNIT),
                (1, [0.0, 1.0, 0.0], axis_angle([1.0, 0.0, 0.0], 0.7), UNIT),
                (
                    2,
                    [0.0, 1.5, 0.0],
                    axis_angle([1.0, 1.0, 0.0], -0.4),
                    [1.2, 0.9, 1.1],
                ),
            ],
            pose_girth: None,
        },
        // HELD-OUT cases: chosen after the implementation, not used while writing it.
        Case {
            name: "held-out: short upper, negative girth, pose morph, root translation",
            upper_length: 0.6,
            girth: -0.7,
            overrides: vec![
                (0, [0.2, -0.4, 1.0], NO_ROT, UNIT),
                (1, [0.0, 1.0, 0.0], axis_angle([0.0, 0.0, 1.0], -1.1), UNIT),
                (2, [0.0, 0.6, 0.0], axis_angle([0.3, -0.5, 0.8], 2.0), UNIT),
            ],
            pose_girth: Some(0.3),
        },
        Case {
            name: "held-out: long upper, big girth, lower flipped 180",
            upper_length: 1.9,
            girth: 1.75,
            overrides: vec![(2, [0.0, 1.9, 0.0], axis_angle([1.0, 0.0, 0.0], PI), UNIT)],
            pose_girth: None,
        },
        Case {
            name: "held-out: range edges, twist, scaled upper",
            upper_length: 0.5,
            girth: -1.0,
            overrides: vec![(
                1,
                [0.0, 1.0, 0.0],
                axis_angle([0.0, 1.0, 0.0], 1.3),
                [0.8, 1.3, 0.8],
            )],
            pose_girth: None,
        },
    ]
}

fn mat_close(label: &str, m: &Mat4, o: &oracle::M) {
    for c in 0..4 {
        for r in 0..4 {
            assert!(
                close(m.cols[c][r], o[r][c]),
                "{label}: cols[{c}][{r}] = {} but oracle[{r}][{c}] = {}",
                m.cols[c][r],
                o[r][c]
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// RICH fixture (the strength fixture; see the module doc)
// ---------------------------------------------------------------------------------------
//
// Values are decimal literals on purpose (non-dyadic). `docs/evidence/m5b/golden_rev_id.py`
// carries the same literals; keep the two value-for-value identical.
//
// Skeleton (parent -> child): root(0) -> upper(1) -> lower(2), and upper(1) -> arm(3).
// Controls (family order):
//   0 upper_length  BoneLengthScale { joint: 1 }  -> scales the rest translation of joints 2, 3
//   1 torso_length  BoneLengthScale { joint: 0 }  -> scales the rest translation of joint 1
//   2 girth         MorphWeight { target: 0 }
//   3 girth_fine    MorphWeight { target: 0 }     (accumulates with girth)
//   4 taper         MorphWeight { target: 1 }

const RICH_CONTROL_IDS: [&str; 5] = [
    "upper_length",
    "torso_length",
    "girth",
    "girth_fine",
    "taper",
];
/// The recipe the golden pins: non-dyadic values, all inside their ranges.
const RICH_CTL: [f32; 5] = [1.3, 0.85, 0.35, 0.15, -0.4];
const RICH_DEFAULTS: [f32; 5] = [1.0, 1.0, 0.0, 0.0, 0.0];

const RICH_RING_Y: [f32; 4] = [0.0, 1.1, 2.2, 3.3];
const RICH_RING_HX: [f32; 4] = [0.30, 0.27, 0.24, 0.21];
const RICH_RING_HZ: [f32; 4] = [0.20, 0.18, 0.16, 0.14];

/// `(name, parent, translation, rotation xyzw, scale)`. The quaternions are unit to within
/// f32 rounding; joint 2 is non-uniformly scaled and joint 3 mildly so.
type RichJoint = (&'static str, Option<usize>, [f32; 3], [f32; 4], [f32; 3]);
const RICH_JOINTS: [RichJoint; 4] = [
    (
        "root",
        None,
        [0.1, -0.05, 0.2],
        [0.0, 0.1, 0.0, 0.99498744],
        [1.0, 1.0, 1.0],
    ),
    (
        "upper",
        Some(0),
        [0.05, 1.05, -0.03],
        [0.06, 0.0, -0.08, 0.99498744],
        [1.0, 1.0, 1.0],
    ),
    (
        "lower",
        Some(1),
        [-0.02, 0.95, 0.04],
        [-0.1, 0.05, 0.0, 0.99373035],
        [1.2, 0.8, 1.1],
    ),
    (
        "arm",
        Some(1),
        [0.6, 0.3, 0.05],
        [0.2, 0.4, 0.4, 0.8],
        [1.0, 1.1, 0.9],
    ),
];

const RICH_LANDMARKS: [(&str, usize, [f32; 3]); 3] = [
    ("head_top", 2, [0.05, 1.1, -0.07]),
    ("upper_mid", 1, [0.0, 0.55, 0.0]),
    ("hand", 3, [0.12, 0.31, -0.04]),
];

fn rich_fam_ref() -> FamilyRef {
    FamilyRef::new("synthetic.tube.rich", 1)
}

fn rich_positions() -> Vec<[f32; 3]> {
    let mut out = Vec::new();
    for r in 0..4 {
        for k in 0..4 {
            let sx = if k == 0 || k == 3 {
                -RICH_RING_HX[r]
            } else {
                RICH_RING_HX[r]
            };
            let sz = if k < 2 {
                -RICH_RING_HZ[r]
            } else {
                RICH_RING_HZ[r]
            };
            out.push([sx, RICH_RING_Y[r], sz]);
        }
    }
    out
}

/// Non-dyadic weights. Vertex 5 sums to 1 + 5.0e-5 and vertex 8 (four influences, declared
/// out of joint order) to 1 - 5.0e-5 in f64: inside the 1e-4 tolerance, not exact, never
/// renormalized. Several vertices declare their influences out of joint order.
fn rich_skin() -> Vec<Vec<(usize, f32)>> {
    vec![
        vec![(0, 1.0)],
        vec![(0, 1.0)],
        vec![(0, 0.7), (1, 0.3)],
        vec![(0, 1.0)],
        vec![(0, 0.6), (1, 0.4)],
        vec![(1, 0.3), (2, 0.70005)],
        vec![(0, 0.45), (1, 0.55)],
        vec![(1, 0.3), (3, 0.7)],
        vec![(3, 0.2), (0, 0.15), (2, 0.35), (1, 0.29995)],
        vec![(2, 0.65), (1, 0.35)],
        vec![(3, 0.4), (1, 0.6)],
        vec![(1, 0.25), (2, 0.5), (3, 0.25)],
        vec![(2, 1.0)],
        vec![(2, 0.8), (3, 0.2)],
        vec![(2, 1.0)],
        vec![(2, 0.55), (3, 0.45)],
    ]
}

/// "girth": vertices 4..12, `dx = +-0.12` and `dz = +-0.08` with the ring's signs.
fn rich_girth_deltas() -> Vec<(u32, [f32; 3])> {
    (4..12u32)
        .map(|v| {
            let k = (v % 4) as usize;
            let dx = if k == 0 || k == 3 { -0.12 } else { 0.12 };
            let dz = if k < 2 { -0.08 } else { 0.08 };
            (v, [dx, 0.0, dz])
        })
        .collect()
}

/// "taper": declared in an order that is NOT sorted by vertex, and vertex 14 is a zero delta
/// (kept, per the canonical serialization).
fn rich_taper_deltas() -> Vec<(u32, [f32; 3])> {
    vec![
        (13, [0.05, -0.04, 0.0]),
        (12, [-0.03, 0.02, 0.01]),
        (14, [0.0, 0.0, 0.0]),
        (15, [-0.06, 0.0, 0.03]),
    ]
}

fn widen4(q: [f32; 4]) -> [f64; 4] {
    [
        f64::from(q[0]),
        f64::from(q[1]),
        f64::from(q[2]),
        f64::from(q[3]),
    ]
}

fn rich_family() -> BodyFamily {
    let joints: Vec<Joint> = RICH_JOINTS
        .iter()
        .map(|&(name, parent, t, q, s)| {
            Joint::new(
                name,
                parent,
                Transform::new(
                    Vec3::new(t[0], t[1], t[2]),
                    Quat::new(q[0], q[1], q[2], q[3]),
                    Vec3::new(s[0], s[1], s[2]),
                ),
            )
        })
        .collect();
    let landmarks: Vec<Landmark> = RICH_LANDMARKS
        .iter()
        .map(|&(name, j, o)| Landmark::new(name, j, Vec3::new(o[0], o[1], o[2])))
        .collect();
    let positions: Vec<Vec3> = rich_positions()
        .iter()
        .map(|p| Vec3::new(p[0], p[1], p[2]))
        .collect();
    let to_deltas = |d: Vec<(u32, [f32; 3])>| -> Vec<(u32, Vec3)> {
        d.into_iter()
            .map(|(v, x)| (v, Vec3::new(x[0], x[1], x[2])))
            .collect()
    };
    BodyFamily::new(
        rich_fam_ref(),
        NeutralMesh::new(positions, fixture_indices()),
        joints,
        landmarks,
        rich_skin(),
    )
    .with_morphs(vec![
        MorphTarget::new("girth", to_deltas(rich_girth_deltas())),
        MorphTarget::new("taper", to_deltas(rich_taper_deltas())),
    ])
    .with_controls(vec![
        ControlDef::new(
            "upper_length",
            0.5,
            2.0,
            1.0,
            vec![ControlEffect::BoneLengthScale { joint: 1 }],
        ),
        ControlDef::new(
            "torso_length",
            0.5,
            2.0,
            1.0,
            vec![ControlEffect::BoneLengthScale { joint: 0 }],
        ),
        ControlDef::new(
            "girth",
            -1.0,
            2.0,
            0.0,
            vec![ControlEffect::MorphWeight { target: 0 }],
        ),
        ControlDef::new(
            "girth_fine",
            -0.5,
            0.5,
            0.0,
            vec![ControlEffect::MorphWeight { target: 0 }],
        ),
        ControlDef::new(
            "taper",
            -1.0,
            1.0,
            0.0,
            vec![ControlEffect::MorphWeight { target: 1 }],
        ),
    ])
}

fn rich_recipe(ctl: [f32; 5]) -> BodyRecipe {
    BodyRecipe::new(
        rich_fam_ref(),
        RICH_CONTROL_IDS
            .iter()
            .zip(ctl.iter())
            .map(|(id, v)| ((*id).to_string(), *v))
            .collect(),
    )
}

fn rich_build(ctl: [f32; 5]) -> CharacterRevision {
    compile(&rich_recipe(ctl), &rich_family()).expect("rich fixture compiles")
}

fn ctl64(ctl: &[f32; 5]) -> Vec<f64> {
    ctl.iter().map(|&v| f64::from(v)).collect()
}

/// The oracle's view of the rich fixture, built from the same plain arrays.
fn rich_oracle() -> oracle::ORig {
    let to_morph = |d: Vec<(u32, [f32; 3])>| -> Vec<(usize, [f64; 3])> {
        d.into_iter()
            .map(|(v, x)| (v as usize, widen3(x)))
            .collect()
    };
    let bone = |of: usize| oracle::OControl {
        default: 1.0,
        bone_scale_of_children_of: Some(of),
        morph_target: None,
    };
    let morph = |t: usize| oracle::OControl {
        default: 0.0,
        bone_scale_of_children_of: None,
        morph_target: Some(t),
    };
    oracle::ORig {
        joints: RICH_JOINTS
            .iter()
            .map(|&(_, parent, t, q, s)| oracle::OJoint {
                parent,
                rest: oracle::OTrs {
                    t: widen3(t),
                    q: widen4(q),
                    s: widen3(s),
                },
            })
            .collect(),
        positions: rich_positions().into_iter().map(widen3).collect(),
        skin: rich_skin()
            .into_iter()
            .map(|l| l.into_iter().map(|(j, w)| (j, f64::from(w))).collect())
            .collect(),
        morphs: vec![to_morph(rich_girth_deltas()), to_morph(rich_taper_deltas())],
        controls: vec![bone(1), bone(0), morph(0), morph(0), morph(1)],
        landmarks: RICH_LANDMARKS
            .iter()
            .map(|&(_, j, o)| (j, widen3(o)))
            .collect(),
    }
}

/// Control sets the direct comparisons run over: the pinned recipe, all defaults, and two
/// range-edge mixes.
fn rich_control_sets() -> Vec<[f32; 5]> {
    vec![
        RICH_CTL,
        RICH_DEFAULTS,
        [2.0, 0.5, -1.0, -0.5, 1.0],
        [0.5, 2.0, 2.0, 0.5, -1.0],
    ]
}

/// A rich pose case. Angles are small and the values smooth (no snapping between poses).
struct RichCase {
    name: &'static str,
    ctl: [f32; 5],
    overrides: Vec<Ovr>,
    /// `(morph name, morph target index, weight)`.
    pose_morphs: Vec<(&'static str, usize, f32)>,
}

fn rich_pose(case: &RichCase) -> PoseInput {
    pose_from(&case.overrides, &case.pose_morphs)
}

fn pose_from(overrides: &[Ovr], pose_morphs: &[(&'static str, usize, f32)]) -> PoseInput {
    let mut p = PoseInput::new();
    for &(j, t, q, s) in overrides {
        p = p.with_joint(
            j,
            Transform::new(
                Vec3::new(t[0], t[1], t[2]),
                Quat::new(q[0], q[1], q[2], q[3]),
                Vec3::new(s[0], s[1], s[2]),
            ),
        );
    }
    for &(name, _, w) in pose_morphs {
        p = p.with_morph(name, w);
    }
    p
}

fn oracle_pose_from(
    overrides: &[Ovr],
    pose_morphs: &[(&'static str, usize, f32)],
) -> oracle::OPose {
    oracle::OPose {
        locals: overrides
            .iter()
            .map(|&(j, t, q, s)| {
                (
                    j,
                    oracle::OTrs {
                        t: widen3(t),
                        q: widen4(q),
                        s: widen3(s),
                    },
                )
            })
            .collect(),
        morph_weights: pose_morphs
            .iter()
            .map(|&(_, t, w)| (t, f64::from(w)))
            .collect(),
    }
}

fn rich_cases() -> Vec<RichCase> {
    vec![
        RichCase {
            name: "bind pose, pinned recipe",
            ctl: RICH_CTL,
            overrides: vec![],
            pose_morphs: vec![],
        },
        // Control case for mutant A, NOT a detector of it. Joint 0 is not the child of any
        // bone-length control, so it is the only joint whose local this pose replaces. For
        // every other joint j the skinning matrix is W'_j * IB_j = W'_0 * (L_1 .. L_j)
        // * (L_1 .. L_j)^-1 * W_0^-1 = W'_0 * W_0^-1, in which every shaped child local L_j
        // cancels exactly. The posed mesh is therefore identical whether or not the bone
        // length control was applied (PASS1 finding 5). Only the direct world-bind and
        // inverse-bind comparisons, the landmark check and rev_id can see mutant A with this pose.
        RichCase {
            name: "root-only override (L_j cancels in W' * IB)",
            ctl: RICH_CTL,
            overrides: vec![(
                0,
                [0.12, -0.04, 0.18],
                axis_angle([0.2, 1.0, -0.1], 0.35),
                UNIT,
            )],
            pose_morphs: vec![],
        },
        // The pose that makes mutant A visible in the MESH: it replaces the local of joint 2,
        // the child scaled by `upper_length`, with a transform that does not depend on the
        // control. Then W'_2 = W'_1 * L'_2 no longer contains the scaled L_2, but IB_2 still
        // does (it is the inverse of the SHAPED bind). The product W'_2 * IB_2 therefore
        // depends on the bone length, and an implementation that ignored the control would
        // use a different IB_2. Joint 1 is overridden too, which does the same for
        // `torso_length`.
        RichCase {
            name: "joints 1 and 2 overridden (scaled children)",
            ctl: RICH_CTL,
            overrides: vec![
                (
                    1,
                    [0.05, 1.05, -0.03],
                    axis_angle([1.0, 0.5, -0.3], 0.5),
                    UNIT,
                ),
                (
                    2,
                    [0.0, 1.1, 0.05],
                    axis_angle([0.3, 1.0, 0.2], -0.6),
                    [1.1, 0.9, 1.05],
                ),
            ],
            pose_morphs: vec![],
        },
        RichCase {
            name: "scaled child joint 2 only, rotated with non-uniform scale",
            ctl: RICH_CTL,
            overrides: vec![(
                2,
                [0.0, 1.1, 0.05],
                axis_angle([0.3, 1.0, 0.2], -0.6),
                [1.1, 0.9, 1.05],
            )],
            pose_morphs: vec![],
        },
        RichCase {
            name: "root and arm overridden with pose-time morph weights",
            ctl: RICH_CTL,
            overrides: vec![
                (0, [0.1, -0.05, 0.2], axis_angle([0.0, 1.0, 0.0], 0.3), UNIT),
                (
                    3,
                    [0.6, 0.3, 0.05],
                    axis_angle([1.0, 0.0, 1.0], 0.4),
                    [1.0, 1.1, 0.9],
                ),
            ],
            pose_morphs: vec![("girth", 0, 0.3), ("taper", 1, 0.5)],
        },
        RichCase {
            name: "range-edge controls, joints 1 and 2 overridden",
            ctl: [2.0, 0.5, -1.0, -0.5, 1.0],
            overrides: vec![
                (
                    1,
                    [0.05, 0.5, -0.03],
                    axis_angle([0.0, 1.0, 0.3], 0.4),
                    [1.05, 1.0, 0.95],
                ),
                (
                    2,
                    [-0.02, 1.9, 0.04],
                    axis_angle([1.0, 0.0, 0.0], -0.5),
                    UNIT,
                ),
            ],
            pose_morphs: vec![("girth", 0, -0.4)],
        },
        RichCase {
            name: "default controls, joints 2 and 3 overridden",
            ctl: RICH_DEFAULTS,
            overrides: vec![
                (
                    2,
                    [-0.02, 0.95, 0.04],
                    axis_angle([0.0, 0.0, 1.0], 0.45),
                    [1.2, 0.8, 1.1],
                ),
                (
                    3,
                    [0.6, 0.3, 0.05],
                    axis_angle([0.0, 1.0, 0.0], -0.35),
                    [1.0, 1.1, 0.9],
                ),
            ],
            pose_morphs: vec![],
        },
    ]
}

// ---------------------------------------------------------------------------------------
// Math
// ---------------------------------------------------------------------------------------

#[test]
fn quaternion_axis_angle_matches_reference_formula() {
    for (axis, ang) in [
        ([0.0, 0.0, 1.0], PI / 2.0),
        ([1.0, 2.0, -0.5], 1.234),
        ([0.0, 1.0, 0.0], -2.5),
    ] {
        let want = axis_angle(axis, ang);
        let got = Quat::from_axis_angle(
            Vec3::new(axis[0] as f32, axis[1] as f32, axis[2] as f32),
            ang as f32,
        );
        for (g, w) in [got.x, got.y, got.z, got.w].into_iter().zip(want) {
            assert!(
                (g - w).abs() < 1e-6,
                "axis {axis:?} angle {ang}: {g} vs {w}"
            );
        }
    }
    // A zero axis is the identity, not NaN.
    assert_eq!(Quat::from_axis_angle(Vec3::ZERO, 1.0), Quat::IDENTITY);
}

#[test]
fn quaternion_matrix_matches_oracle_and_mul_composes_right_to_left() {
    let qa = axis_angle([1.0, 0.0, 0.0], 0.9);
    let qb = axis_angle([0.0, 1.0, 0.0], -0.4);
    let a = Quat::new(qa[0], qa[1], qa[2], qa[3]);
    let b = Quat::new(qb[0], qb[1], qb[2], qb[3]);
    let ab = a.mul(b);
    // (a * b) applied to a point equals a applied after b.
    let p = Vec3::new(0.3, -1.2, 0.8);
    let step = a.to_mat().transform_point(b.to_mat().transform_point(p));
    let at_once = ab.to_mat().transform_point(p);
    assert!((step.x - at_once.x).abs() < 1e-6);
    assert!((step.y - at_once.y).abs() < 1e-6);
    assert!((step.z - at_once.z).abs() < 1e-6);
    // The matrix itself matches the oracle's rotation formula.
    let om = oracle::rotation([
        f64::from(ab.x),
        f64::from(ab.y),
        f64::from(ab.z),
        f64::from(ab.w),
    ]);
    mat_close("quat to_mat", &ab.to_mat(), &om);
    // Normalizing a scaled quaternion recovers the unit one; zero has no normalization.
    let big = Quat::new(2.0 * a.x, 2.0 * a.y, 2.0 * a.z, 2.0 * a.w).normalize();
    assert!((big.w - a.w).abs() < 1e-6);
    assert!(Quat::new(0.0, 0.0, 0.0, 0.0).try_normalize().is_none());
}

#[test]
fn transform_is_translation_rotation_scale_in_that_order() {
    // p = (1,0,0): S=(2,1,1) -> (2,0,0); R = +90 about Z -> (0,2,0); T=(0,0,5) -> (0,2,5).
    let q = axis_angle([0.0, 0.0, 1.0], PI / 2.0);
    let t = Transform::new(
        Vec3::new(0.0, 0.0, 5.0),
        Quat::new(q[0], q[1], q[2], q[3]),
        Vec3::new(2.0, 1.0, 1.0),
    );
    let p = Mat4::from_transform(&t).transform_point(Vec3::new(1.0, 0.0, 0.0));
    assert_close3("T*R*S", p, [0.0, 2.0, 5.0]);
}

#[test]
fn affine_inverse_round_trips_and_matches_gauss_jordan() {
    let q = axis_angle([0.2, -0.7, 0.4], 1.1);
    let t = Transform::new(
        Vec3::new(0.5, -2.0, 1.5),
        Quat::new(q[0], q[1], q[2], q[3]),
        Vec3::new(1.5, 0.75, 2.0),
    );
    let m = Mat4::from_transform(&t);
    let inv = m.inverse().expect("invertible");
    let prod = m.mul(&inv);
    mat_close("M * M^-1", &prod, &oracle::identity());
    let om = oracle::trs_matrix(&oracle::OTrs {
        t: [0.5, -2.0, 1.5],
        q: [
            f64::from(q[0]),
            f64::from(q[1]),
            f64::from(q[2]),
            f64::from(q[3]),
        ],
        s: [1.5, 0.75, 2.0],
    });
    mat_close("forward matrix vs oracle", &m, &om);
    mat_close(
        "inverse vs Gauss-Jordan",
        &inv,
        &oracle::invert(&om).unwrap(),
    );
}

#[test]
fn singular_or_non_affine_matrices_have_no_inverse() {
    let flat = Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::new(1.0, 0.0, 1.0));
    assert!(Mat4::from_transform(&flat).inverse().is_none());
    let mut proj = Mat4::IDENTITY;
    proj.cols[0][3] = 0.5;
    assert!(proj.inverse().is_none());
    let mut nan = Mat4::IDENTITY;
    nan.cols[3][1] = f32::NAN;
    assert!(nan.inverse().is_none());
}

// ---------------------------------------------------------------------------------------
// Skeleton / skin / morph validation
// ---------------------------------------------------------------------------------------

fn j(name: &str, parent: Option<usize>) -> Joint {
    Joint::new(name, parent, Transform::IDENTITY)
}

#[test]
fn skeleton_validation_errors() {
    assert_eq!(
        Skeleton::new(vec![], vec![]).unwrap_err(),
        SkeletonError::Empty
    );
    assert!(matches!(
        Skeleton::new(vec![j("a", None), j("b", Some(2)), j("c", Some(0))], vec![]),
        Err(SkeletonError::ParentOrder {
            joint: 1,
            parent: 2
        })
    ));
    assert!(matches!(
        Skeleton::new(vec![j("a", Some(0))], vec![]),
        Err(SkeletonError::ParentOrder {
            joint: 0,
            parent: 0
        })
    ));
    assert!(matches!(
        Skeleton::new(vec![j("a", None), j("a", Some(0))], vec![]),
        Err(SkeletonError::DuplicateName { kind: "joint", .. })
    ));
    let mut bad = j("a", None);
    bad.rest.translation.y = f32::INFINITY;
    assert!(matches!(
        Skeleton::new(vec![bad], vec![]),
        Err(SkeletonError::NonFinite { joint: 0 })
    ));
    let mut zero_rot = j("a", None);
    zero_rot.rest.rotation = Quat::new(0.0, 0.0, 0.0, 0.0);
    assert!(matches!(
        Skeleton::new(vec![zero_rot], vec![]),
        Err(SkeletonError::NonFinite { joint: 0 })
    ));
    let many: Vec<Joint> = (0..=MAX_JOINTS)
        .map(|i| j(&format!("j{i}"), if i == 0 { None } else { Some(i - 1) }))
        .collect();
    assert!(matches!(
        Skeleton::new(many, vec![]),
        Err(SkeletonError::LimitExceeded { what: "joints", .. })
    ));
    assert!(matches!(
        Skeleton::new(vec![j("a", None)], vec![Landmark::new("l", 5, Vec3::ZERO)]),
        Err(SkeletonError::LandmarkJoint {
            landmark: 0,
            joint: 5
        })
    ));
    let cap: Vec<Joint> = (0..MAX_JOINTS)
        .map(|i| j(&format!("j{i}"), if i == 0 { None } else { Some(i - 1) }))
        .collect();
    assert!(
        Skeleton::new(cap, vec![]).is_ok(),
        "exactly the cap is allowed"
    );
}

#[test]
fn inverse_binds_degenerate_is_a_typed_error() {
    let mut flat = j("b", Some(0));
    flat.rest.scale = Vec3::new(0.0, 1.0, 1.0);
    let sk = Skeleton::new(vec![j("a", None), flat], vec![]).unwrap();
    assert_eq!(
        sk.inverse_binds().unwrap_err(),
        SkeletonError::DegenerateBind { joint: 1 }
    );
}

#[test]
fn skin_validation_errors_and_no_renormalisation() {
    let ok = |l: Vec<(usize, f32)>| SkinWeights::new(&[l], 3);
    assert!(ok(vec![(0, 0.25), (1, 0.75)]).is_ok());
    assert!(matches!(
        ok(vec![]),
        Err(SkinError::InfluenceCount {
            vertex: 0,
            count: 0
        })
    ));
    assert!(matches!(
        ok(vec![(0, 0.2), (1, 0.2), (2, 0.2), (0, 0.2), (1, 0.2)]),
        Err(SkinError::InfluenceCount { count: 5, .. })
    ));
    assert!(matches!(
        ok(vec![(3, 1.0)]),
        Err(SkinError::JointOutOfRange { joint: 3, .. })
    ));
    assert!(matches!(
        ok(vec![(1, 0.5), (1, 0.5)]),
        Err(SkinError::DuplicateJoint { joint: 1, .. })
    ));
    assert!(matches!(
        ok(vec![(0, f32::NAN)]),
        Err(SkinError::NonFinite { .. })
    ));
    assert!(matches!(
        ok(vec![(0, 1.5), (1, -0.5)]),
        Err(SkinError::NegativeWeight { .. })
    ));
    // Sum 0.5 is an error, not silently rescaled; 1 +/- 5e-5 is inside the tolerance, 1 + 2e-4 is not.
    assert!(matches!(
        ok(vec![(0, 0.5)]),
        Err(SkinError::WeightSum { .. })
    ));
    assert!(ok(vec![(0, 1.00005)]).is_ok());
    assert!(matches!(
        ok(vec![(0, 1.0002)]),
        Err(SkinError::WeightSum { .. })
    ));
    // The stored weights are exactly what was given.
    let w = ok(vec![(0, 1.00005)]).unwrap();
    assert_eq!(w.influences(0).unwrap(), vec![(0usize, 1.00005f32)]);
}

#[test]
fn skinning_checks_lengths_and_finiteness() {
    let w = SkinWeights::new(&[vec![(0, 1.0)]], 1).unwrap();
    let id = [Mat4::IDENTITY];
    assert!(matches!(
        w.skin(&[], &id, &id),
        Err(SkinError::LengthMismatch {
            what: "positions",
            ..
        })
    ));
    assert!(matches!(
        w.skin(&[Vec3::ZERO], &[], &id),
        Err(SkinError::LengthMismatch { .. })
    ));
    assert!(matches!(
        w.skin(&[Vec3::new(f32::NAN, 0.0, 0.0)], &id, &id),
        Err(SkinError::NonFiniteInput { .. })
    ));
    assert_eq!(
        w.skin(&[Vec3::new(1.0, 2.0, 3.0)], &id, &id).unwrap(),
        vec![Vec3::new(1.0, 2.0, 3.0)]
    );
}

#[test]
fn morph_validation_and_application_order() {
    let base = vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)];
    let t0 = MorphTarget::new("a", vec![(1, Vec3::new(0.0, 1.0, 0.0))]);
    let t1 = MorphTarget::new(
        "b",
        vec![(1, Vec3::new(0.0, 0.0, 2.0)), (0, Vec3::new(1.0, 1.0, 1.0))],
    );
    let out = apply_morphs(&base, &[t0.clone(), t1.clone()], &[0.5, 0.25]).unwrap();
    assert_eq!(out[0], Vec3::new(0.25, 0.25, 0.25));
    assert_eq!(out[1], Vec3::new(1.0, 0.5, 0.5));
    assert!(matches!(
        apply_morphs(&base, &[t0.clone()], &[]),
        Err(MorphError::WeightCount {
            expected: 1,
            got: 0
        })
    ));
    assert!(matches!(
        apply_morphs(&base, &[t0.clone()], &[f32::INFINITY]),
        Err(MorphError::NonFinite { .. })
    ));
    let oob = MorphTarget::new("o", vec![(2, Vec3::ZERO)]);
    assert!(matches!(
        apply_morphs(&base, &[oob], &[1.0]),
        Err(MorphError::IndexOutOfRange {
            vertex: 2,
            vertex_count: 2,
            ..
        })
    ));
    let dup = MorphTarget::new("d", vec![(0, Vec3::ZERO), (0, Vec3::ZERO)]);
    assert!(matches!(
        apply_morphs(&base, &[dup], &[1.0]),
        Err(MorphError::DuplicateVertex { vertex: 0, .. })
    ));
    let nan = MorphTarget::new("n", vec![(0, Vec3::new(f32::NAN, 0.0, 0.0))]);
    assert!(matches!(
        apply_morphs(&base, &[nan], &[1.0]),
        Err(MorphError::NonFinite { .. })
    ));
}

// ---------------------------------------------------------------------------------------
// Compile and evaluate
// ---------------------------------------------------------------------------------------

#[test]
fn compiled_revision_shape() {
    let rev = build(1.0, 0.0);
    assert_eq!(rev.support, SupportClass::Supported);
    assert!(rev.colliders.is_empty());
    assert!(rev.garment_fits.is_empty());
    assert_eq!(rev.neutral.positions.len(), 16);
    assert_eq!(rev.neutral.indices.len(), 72);
    assert_eq!(rev.inverse_binds.len(), 3);
    assert_eq!(rev.skeleton.joint_count(), 3);
    assert_eq!(rev.skin.vertex_count(), 16);
    assert_eq!(rev.morphs.len(), 1);
    assert_eq!(rev.family, fam_ref());
    assert_eq!(rev.rev_id, rev.content_id());
    // A renderer gets positions, indices and joint worlds from the posed mesh alone.
    let posed = rev.evaluate(&PoseInput::new()).unwrap();
    assert_eq!(posed.indices, rev.neutral.indices);
    assert_eq!(posed.joint_world.len(), 3);
    // The typed per-garment outcome exists. Slice 1 runs no fit checks, so `FitReason` has no
    // variants and only `Fit` is constructible; the list is empty.
    let _typed: &[GarmentFit] = &rev.garment_fits;
    assert!(rev.garment_fits.is_empty());
    let fit = GarmentFit::new("coat", FitStatus::Fit);
    assert_eq!(fit.garment_id, "coat");
    assert_eq!(fit.status, FitStatus::Fit);
}

/// Designed to kill: Mutant A (`ignored_skeleton_update`): the rest translation and landmark stay unscaled.
#[test]
fn bone_length_control_reshapes_skeleton() {
    let rev = build(1.5, 0.0);
    let joints = rev.skeleton.joints();
    // upper_length scales the bone upper -> lower: the child's rest translation, and only it.
    assert_eq!(joints[0].rest.translation, Vec3::new(0.0, 0.0, 0.0));
    assert_eq!(joints[1].rest.translation, Vec3::new(0.0, 1.0, 0.0));
    assert_eq!(joints[2].rest.translation, Vec3::new(0.0, 1.5, 0.0));
    // The lower joint's world bind is therefore (0, 2.5, 0) and the head landmark (0, 3.5, 0).
    let world = rev.skeleton.world_bind();
    assert_close3(
        "lower world",
        world[2].transform_point(Vec3::ZERO),
        [0.0, 2.5, 0.0],
    );
    assert_close3(
        "head_top landmark",
        rev.skeleton.landmark_world(0, &world).unwrap(),
        [0.0, 3.5, 0.0],
    );
    // Default value leaves the family skeleton alone.
    let plain = build(1.0, 0.0);
    assert_eq!(
        plain.skeleton.joints()[2].rest.translation,
        Vec3::new(0.0, 1.0, 0.0)
    );
}

/// Designed to kill: Mutant C (`noop_morph`): the compile-time morph moves no vertex.
#[test]
fn morph_control_moves_vertices() {
    let rev = build(1.0, 0.5);
    // Vertex 4 is ring 1, k = 0: (-0.25, 1, -0.25) + 0.5 * (-0.1, 0, -0.1).
    assert_close3("v4", rev.neutral.positions[4], [-0.30, 1.0, -0.30]);
    // Vertex 6 is ring 1, k = 2: (+0.25, 1, +0.25) + 0.5 * (+0.1, 0, +0.1).
    assert_close3("v6", rev.neutral.positions[6], [0.30, 1.0, 0.30]);
    // Vertices outside the morph are untouched.
    assert_eq!(rev.neutral.positions[0], Vec3::new(-0.25, 0.0, -0.25));
    assert_eq!(rev.neutral.positions[12], Vec3::new(-0.25, 3.0, -0.25));
    // Negative weight inverts the delta.
    let thin = build(1.0, -0.5);
    assert_close3("thin v4", thin.neutral.positions[4], [-0.20, 1.0, -0.20]);
}

/// Designed to kill: Mutant C (`noop_morph`), POSE-time path: the compiled shape has weight 0, so only a working
/// `apply_morphs` in `evaluate` can move vertex 4.
#[test]
fn pose_morph_weight_moves_vertices() {
    let rev = build(1.0, 0.0);
    let posed = rev
        .evaluate(&PoseInput::new().with_morph("girth", 1.0))
        .unwrap();
    assert_close3("pose-morph v4", posed.positions[4], [-0.35, 1.0, -0.35]);
    // Pose morphs act on top of the compiled shape: 0.5 (control) + 1.0 (pose) = 1.5.
    let shaped = build(1.0, 0.5);
    let both = shaped
        .evaluate(&PoseInput::new().with_morph("girth", 1.0))
        .unwrap();
    assert_close3(
        "stacked v4",
        both.positions[4],
        [-0.25 - 0.15, 1.0, -0.25 - 0.15],
    );
}

/// Designed to kill: Mutant B (`stale_bind`): W' is shaped but IB is from the unshaped skeleton, so the identity
/// pose no longer reproduces the shaped neutral for any body with `upper_length != 1`.
/// Mutant C also (the oracle shaped neutral moves with girth). Mutant A is NOT killed here:
/// W' and IB both ignore the control, so they cancel (PASS1 finding 5).
#[test]
fn identity_pose_equals_bind_for_shaped_bodies() {
    for (ul, g) in [(1.0, 0.0), (1.5, 0.5), (0.6, -0.7), (2.0, 2.0)] {
        let rev = build(ul, g);
        let posed = rev.evaluate(&PoseInput::new()).unwrap();
        for (v, (p, n)) in posed
            .positions
            .iter()
            .zip(&rev.neutral.positions)
            .enumerate()
        {
            assert_close3(
                &format!("rest vertex {v} (ul {ul}, girth {g})"),
                *p,
                [f64::from(n.x), f64::from(n.y), f64::from(n.z)],
            );
        }
        // And the same against the independent oracle's shaped neutral.
        let want = oracle::shaped_neutral(&fixture_oracle(), &[f64::from(ul), f64::from(g)]);
        for (v, (p, w)) in posed.positions.iter().zip(&want).enumerate() {
            assert_close3(&format!("oracle rest vertex {v}"), *p, *w);
        }
    }
}

#[test]
fn single_joint_rotated_90_degrees_hand_computed() {
    let rev = build(1.0, 0.0);
    let q = axis_angle([0.0, 0.0, 1.0], PI / 2.0);
    let pose = PoseInput::new().with_joint(
        1,
        Transform::new(
            Vec3::new(0.0, 1.0, 0.0),
            Quat::new(q[0], q[1], q[2], q[3]),
            Vec3::new(1.0, 1.0, 1.0),
        ),
    );
    let out = rev.evaluate(&pose).unwrap();
    // Rotation of +90 degrees about Z maps (x, y) to (-y, x). Hand-derived:
    // - root-only vertex 0 does not move.
    // - lower-only vertex 12 (-0.25, 3, -0.25): relative to the lower joint (0,2,0) it is
    //   (-0.25, 1, -0.25); the posed lower frame is T(0,1,0) Rz T(0,1,0), giving (-2, 0.75, -0.25).
    // - lower-only vertex 13 (0.25, 3, -0.25) goes to (-2, 1.25, -0.25).
    // - vertex 4 (ring 1, root .5 / upper .5): x/2 = -0.125, 1 + x/2 = 0.875, z = -0.25.
    // - vertex 8 (root .125, upper .375, lower .5): (-0.90625, 0.90625, -0.25).
    // - vertex 9 (upper .25, lower .75) is rigid with the chain at its height: (-1, 1.25, -0.25).
    let expect: [(usize, [f64; 3]); 6] = [
        (0, [-0.25, 0.0, -0.25]),
        (12, [-2.0, 0.75, -0.25]),
        (13, [-2.0, 1.25, -0.25]),
        (4, [-0.125, 0.875, -0.25]),
        (8, [-0.90625, 0.90625, -0.25]),
        (9, [-1.0, 1.25, -0.25]),
    ];
    for (v, want) in expect {
        assert_close3(&format!("vertex {v}"), out.positions[v], want);
    }
    // Landmark "head_top" sits (0,1,0) above the lower joint: lower world is (-2, 1, 0) + R.
    let head = rev.skeleton.landmark_world(0, &out.joint_world).unwrap();
    assert_close3("head_top", head, [-2.0, 1.0, 0.0]);
}

/// Designed to kill: Mutants A (rest translations and inverse binds compared directly), B (inverse binds, and the
/// pose-time vertices) and C (case with `pose_girth`, and compile-time girth).
#[test]
fn oracle_matches_implementation() {
    let rig = fixture_oracle();
    for case in cases() {
        let rev = build(case.upper_length, case.girth);
        let ctl = [f64::from(case.upper_length), f64::from(case.girth)];
        let posed = rev.evaluate(&to_pose(&case)).unwrap();
        let opose = to_oracle_pose(&case);

        // Shaped skeleton and inverse binds against the oracle's own derivation.
        let rest = oracle::shaped_rest(&rig, &ctl);
        for (i, jt) in rev.skeleton.joints().iter().enumerate() {
            let t = jt.rest.translation;
            assert_close3(&format!("{}: rest t[{i}]", case.name), t, rest[i].t);
        }
        let ibs = oracle::inverse_binds(&rig, &ctl);
        for (i, ib) in rev.inverse_binds.iter().enumerate() {
            mat_close(&format!("{}: inverse bind {i}", case.name), ib, &ibs[i]);
        }
        // Posed joint worlds.
        let ow = oracle::posed_world(&rig, &ctl, &opose);
        for (i, w) in posed.joint_world.iter().enumerate() {
            mat_close(&format!("{}: world {i}", case.name), w, &ow[i]);
        }
        // Posed vertices.
        let want = oracle::posed(&rig, &ctl, &opose);
        assert_eq!(posed.positions.len(), want.len());
        for (v, (p, w)) in posed.positions.iter().zip(&want).enumerate() {
            assert_close3(&format!("{}: vertex {v}", case.name), *p, *w);
        }
    }
}

/// Designed to kill: Mutant B only (IB from the unshaped skeleton times the shaped W is not the identity). It
/// cannot see A: IB and W are both unscaled, so their product is still the identity.
#[test]
fn inverse_binds_times_world_bind_is_identity() {
    for (ul, g) in [(1.0, 0.0), (1.5, 0.5), (0.5, -1.0), (2.0, 2.0)] {
        let rev = build(ul, g);
        let world = rev.skeleton.world_bind();
        for (i, (w, ib)) in world.iter().zip(&rev.inverse_binds).enumerate() {
            mat_close(
                &format!("IB{i} * W{i} (ul {ul})"),
                &ib.mul(w),
                &oracle::identity(),
            );
            mat_close(
                &format!("W{i} * IB{i} (ul {ul})"),
                &w.mul(ib),
                &oracle::identity(),
            );
        }
    }
}

#[test]
fn every_compile_error_variant_is_reachable() {
    let ok_recipe = recipe(1.0, 0.0);
    let fam = fixture_family();

    // FamilyMismatch
    let mut r = ok_recipe.clone();
    r.family = FamilyRef::new("synthetic.tube", 2);
    assert!(matches!(
        compile(&r, &fam),
        Err(CompileError::FamilyMismatch { .. })
    ));

    // UnknownControl
    let r = BodyRecipe::new(fam_ref(), vec![("nope".to_string(), 0.0)]);
    assert!(
        matches!(compile(&r, &fam), Err(CompileError::UnknownControl { ref id }) if id == "nope")
    );

    // DuplicateControl
    let r = BodyRecipe::new(
        fam_ref(),
        vec![("girth".to_string(), 0.1), ("girth".to_string(), 0.2)],
    );
    assert!(matches!(
        compile(&r, &fam),
        Err(CompileError::DuplicateControl { .. })
    ));

    // ControlOutOfRange (both sides) and NonFinite control value
    for bad in [2.5, 0.4] {
        let r = BodyRecipe::new(fam_ref(), vec![("upper_length".to_string(), bad)]);
        assert!(
            matches!(
                compile(&r, &fam),
                Err(CompileError::ControlOutOfRange { .. })
            ),
            "{bad}"
        );
    }
    let r = BodyRecipe::new(fam_ref(), vec![("girth".to_string(), f32::NAN)]);
    assert!(matches!(
        compile(&r, &fam),
        Err(CompileError::NonFinite {
            part: "control value"
        })
    ));

    // SkeletonCycle: a parent that is not earlier (here a forward reference).
    let mut f = fam.clone();
    f.joints[1].parent = Some(2);
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::SkeletonCycle {
            joint: 1,
            parent: 2
        })
    ));

    // NonFinite: vertex position, joint transform.
    let mut f = fam.clone();
    f.neutral.positions[3].x = f32::NAN;
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::NonFinite { part: "positions" })
    ));
    let mut f = fam.clone();
    f.joints[2].rest.translation.y = f32::INFINITY;
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::NonFinite {
            part: "joint transform"
        })
    ));

    // DegenerateBind: a collapsed scale.
    let mut f = fam.clone();
    f.joints[2].rest.scale = Vec3::new(0.0, 1.0, 1.0);
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::DegenerateBind { joint: 2 })
    ));

    // SkinWeightInvalid: weights sum to 0.5 (not renormalised), and a wrong vertex count.
    let mut f = fam.clone();
    f.skin[3] = vec![(0, 0.5)];
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::SkinWeightInvalid(SkinError::WeightSum {
            vertex: 3,
            ..
        }))
    ));
    let mut f = fam.clone();
    f.skin.pop();
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::SkinWeightInvalid(
            SkinError::LengthMismatch { .. }
        ))
    ));

    // MorphTopologyMismatch: delta addresses a vertex that does not exist; duplicate vertex.
    let mut f = fam.clone();
    f.morphs[0].deltas.push((99, Vec3::ZERO));
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::MorphTopologyMismatch(
            MorphError::IndexOutOfRange { vertex: 99, .. }
        ))
    ));
    let mut f = fam.clone();
    f.morphs[0].deltas.push((4, Vec3::ZERO));
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::MorphTopologyMismatch(
            MorphError::DuplicateVertex { vertex: 4, .. }
        ))
    ));

    // LimitExceeded: one joint over the cap.
    let mut f = fam.clone();
    f.joints = (0..=MAX_JOINTS)
        .map(|i| j(&format!("j{i}"), if i == 0 { None } else { Some(i - 1) }))
        .collect();
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::LimitExceeded { what: "joints", count, max }) if count == MAX_JOINTS + 1 && max == MAX_JOINTS
    ));

    // DuplicateName: joints, morph targets, controls.
    let mut f = fam.clone();
    f.joints[2].name = "upper".to_string();
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::DuplicateName { kind: "joint", .. })
    ));
    let mut f = fam.clone();
    let again = f.morphs[0].clone();
    f.morphs.push(again);
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::DuplicateName {
            kind: "morph target",
            ..
        })
    ));
    let mut f = fam.clone();
    f.controls[1].id = "upper_length".to_string();
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::DuplicateName {
            kind: "control",
            ..
        })
    ));

    // InvalidFamily: bad index buffer; an effect that points at nothing; an empty range.
    let mut f = fam.clone();
    f.neutral.indices[0] = 99;
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::InvalidFamily { .. })
    ));
    let mut f = fam.clone();
    f.neutral.indices.pop();
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::InvalidFamily { .. })
    ));
    let mut f = fam.clone();
    f.controls[0].effects = vec![ControlEffect::BoneLengthScale { joint: 9 }];
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::InvalidFamily { .. })
    ));
    let mut f = fam.clone();
    f.controls[1].effects = vec![ControlEffect::MorphWeight { target: 4 }];
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::InvalidFamily { .. })
    ));
    let mut f = fam.clone();
    f.controls[0].min = 3.0;
    assert!(matches!(
        compile(&ok_recipe, &f),
        Err(CompileError::InvalidFamily { .. })
    ));

    // Display and Error are implemented and never empty.
    let e: Box<dyn std::error::Error> = Box::new(CompileError::UnknownControl { id: "x".into() });
    assert!(!e.to_string().is_empty());
}

#[test]
fn compile_is_all_or_nothing() {
    // An error in the LAST validation stage (recipe value) still yields only an Err: nothing
    // is returned partially, and the family is not modified.
    let fam = fixture_family();
    let before = fam.clone();
    let r = BodyRecipe::new(
        fam_ref(),
        vec![
            ("upper_length".to_string(), 1.2),
            ("girth".to_string(), 99.0),
        ],
    );
    assert!(compile(&r, &fam).is_err());
    assert_eq!(fam, before);
}

/// Normative v0 behaviour (FIGURE_CONTRACT, canonical serialization; ruling #59): the
/// serialization canonicalizes `-0.0` to `+0.0`. So a body whose data differ only in the sign
/// of a zero, which ordinary negation produces, has the same `rev_id`.
#[test]
fn negative_zero_canonicalized_same_rev_id() {
    let mut neg = fixture_positions();
    assert_eq!(
        neg[0][1].to_bits(),
        0.0f32.to_bits(),
        "fixture vertex 0 has y = +0.0"
    );
    neg[0][1] = -0.0;
    assert_ne!(
        neg[0][1].to_bits(),
        0.0f32.to_bits(),
        "the input really is -0.0"
    );
    let plus = compile(&recipe(1.0, 0.0), &fixture_family()).unwrap();
    let minus = compile(&recipe(1.0, 0.0), &fixture_family_from(neg)).unwrap();
    assert_eq!(plus.rev_id, minus.rev_id, "-0.0 is canonicalized to +0.0");
    assert_eq!(plus.canonical_bytes(), minus.canonical_bytes());
    // Positive control: a real non-zero change in the same slot does change rev_id.
    let mut moved = fixture_positions();
    moved[0][1] = 1.0e-3;
    let other = compile(&recipe(1.0, 0.0), &fixture_family_from(moved)).unwrap();
    assert_ne!(plus.rev_id, other.rev_id);
}

#[test]
fn deterministic_rev_id() {
    let a = build(1.25, 0.5);
    let b = build(1.25, 0.5);
    assert_eq!(a.rev_id, b.rev_id);
    assert_eq!(a, b);
    // Omitted controls equal their explicit defaults; recipe order does not matter.
    let fam = fixture_family();
    let explicit = compile(&recipe(1.0, 0.0), &fam).unwrap();
    let omitted = compile(&BodyRecipe::new(fam_ref(), vec![]), &fam).unwrap();
    assert_eq!(explicit.rev_id, omitted.rev_id);
    let swapped = BodyRecipe::new(
        fam_ref(),
        vec![
            ("girth".to_string(), 0.5),
            ("upper_length".to_string(), 1.25),
        ],
    );
    assert_eq!(compile(&swapped, &fam).unwrap().rev_id, a.rev_id);
}

#[test]
fn evaluate_does_not_change_the_revision() {
    let rev = build(1.5, 0.5);
    let snapshot = rev.clone();
    let id = rev.rev_id;
    for case in cases() {
        let _ = rev.evaluate(&to_pose(&case));
    }
    // Failed evaluations also leave it alone.
    assert!(rev
        .evaluate(&PoseInput::new().with_morph("nope", 1.0))
        .is_err());
    assert_eq!(rev.rev_id, id);
    assert_eq!(rev, snapshot);
    assert_eq!(
        rev.content_id(),
        id,
        "content still hashes to the stored id"
    );
}

#[test]
fn evaluate_rejects_bad_poses_with_typed_errors() {
    let rev = build(1.0, 0.0);
    let t = Transform::IDENTITY;
    assert!(matches!(
        rev.evaluate(&PoseInput::new().with_morph("nope", 1.0)),
        Err(EvalError::UnknownMorph { .. })
    ));
    assert!(matches!(
        rev.evaluate(
            &PoseInput::new()
                .with_morph("girth", 1.0)
                .with_morph("girth", 2.0)
        ),
        Err(EvalError::DuplicateMorph { .. })
    ));
    assert!(matches!(
        rev.evaluate(&PoseInput::new().with_morph("girth", f32::NAN)),
        Err(EvalError::NonFinite { .. })
    ));
    assert!(matches!(
        rev.evaluate(&PoseInput::new().with_joint(3, t)),
        Err(EvalError::JointOutOfRange { joint: 3 })
    ));
    assert!(matches!(
        rev.evaluate(&PoseInput::new().with_joint(1, t).with_joint(1, t)),
        Err(EvalError::DuplicateJoint { joint: 1 })
    ));
    let mut bad = t;
    bad.scale.x = f32::NAN;
    assert!(matches!(
        rev.evaluate(&PoseInput::new().with_joint(0, bad)),
        Err(EvalError::InvalidTransform { joint: 0 })
    ));
}

/// Designed to kill: Mutants A (a bone-length control changes no `rev_id`) and C (the girth control then changes
/// no vertex, so `rev_id` does not move). Mutant B is not designed for here.
#[test]
fn each_control_changes_rev_id() {
    let base = build(1.0, 0.0).rev_id;
    let bone_only = build(1.5, 0.0).rev_id;
    let morph_only = build(1.0, 0.5).rev_id;
    let both = build(1.5, 0.5).rev_id;
    let ids = [base, bone_only, morph_only, both];
    for (a, ida) in ids.iter().enumerate() {
        for (b, idb) in ids.iter().enumerate() {
            if a != b {
                assert_ne!(ida, idb, "recipes {a} and {b} must hash differently");
            }
        }
    }
    // A different family version hashes differently even with identical geometry.
    let mut fam = fixture_family();
    fam.family = FamilyRef::new("synthetic.tube", 2);
    let r = BodyRecipe::new(fam.family.clone(), vec![]);
    assert_ne!(compile(&r, &fam).unwrap().rev_id, base);
}

/// The canonical serialization written out independently from its documented layout.
fn independent_canonical_bytes(rev: &CharacterRevision) -> Vec<u8> {
    fn u32le(o: &mut Vec<u8>, v: u32) {
        o.extend_from_slice(&v.to_le_bytes());
    }
    fn f32le(o: &mut Vec<u8>, v: f32) {
        // rev.v0 canonicalizes -0.0 to +0.0 (ruling #59).
        let bits = if v == 0.0 { 0 } else { v.to_bits() };
        o.extend_from_slice(&bits.to_le_bytes());
    }
    fn string(o: &mut Vec<u8>, s: &str) {
        u32le(o, s.len() as u32);
        o.extend_from_slice(s.as_bytes());
    }
    let mut o = Vec::new();
    o.extend_from_slice(b"gibson.figure.rev.v0\0");
    string(&mut o, &rev.family.id);
    u32le(&mut o, rev.family.version);
    u32le(&mut o, rev.neutral.positions.len() as u32);
    for p in &rev.neutral.positions {
        for c in [p.x, p.y, p.z] {
            f32le(&mut o, c);
        }
    }
    u32le(&mut o, rev.neutral.indices.len() as u32);
    for &i in &rev.neutral.indices {
        u32le(&mut o, i);
    }
    u32le(&mut o, rev.skeleton.joints().len() as u32);
    for jt in rev.skeleton.joints() {
        string(&mut o, &jt.name);
        let parent: i32 = jt.parent.map_or(-1, |p| p as i32);
        o.extend_from_slice(&parent.to_le_bytes());
        let (t, q, s) = (jt.rest.translation, jt.rest.rotation, jt.rest.scale);
        for c in [t.x, t.y, t.z, q.x, q.y, q.z, q.w, s.x, s.y, s.z] {
            f32le(&mut o, c);
        }
    }
    u32le(&mut o, rev.skeleton.landmarks().len() as u32);
    for l in rev.skeleton.landmarks() {
        string(&mut o, &l.name);
        u32le(&mut o, l.joint as u32);
        for c in [l.offset.x, l.offset.y, l.offset.z] {
            f32le(&mut o, c);
        }
    }
    u32le(&mut o, rev.inverse_binds.len() as u32);
    for m in &rev.inverse_binds {
        for col in 0..4 {
            for row in 0..4 {
                f32le(&mut o, m.cols[col][row]);
            }
        }
    }
    u32le(&mut o, rev.skin.vertex_count() as u32);
    for v in 0..rev.skin.vertex_count() {
        let infl = rev.skin.influences(v).unwrap();
        o.push(infl.len() as u8);
        for (jn, w) in infl {
            u32le(&mut o, jn as u32);
            f32le(&mut o, w);
        }
    }
    u32le(&mut o, rev.morphs.len() as u32);
    for m in &rev.morphs {
        string(&mut o, &m.name);
        u32le(&mut o, m.deltas.len() as u32);
        for &(v, d) in &m.deltas {
            u32le(&mut o, v);
            for c in [d.x, d.y, d.z] {
                f32le(&mut o, c);
            }
        }
    }
    o
}

#[test]
fn rev_id_is_sha256_of_the_documented_canonical_bytes() {
    for (ul, g) in [(1.0, 0.0), (1.25, 0.5)] {
        let rev = build(ul, g);
        let bytes = independent_canonical_bytes(&rev);
        assert_eq!(
            bytes,
            rev.canonical_bytes(),
            "serialization layout drifted from the doc"
        );
        let want = sha2::Sha256::digest(&bytes);
        assert_eq!(&rev.rev_id.0[..], want.as_slice());
    }
}

// ---------------------------------------------------------------------------------------
// RICH fixture tests
// ---------------------------------------------------------------------------------------

fn v3(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

/// Guards the fixture itself: fails if the strength the oracle, the mutation controls and the
/// golden depend on is weakened. Not a mutant killer.
#[test]
fn rich_fixture_meets_the_required_strength() {
    let fam = rich_family();
    // 1. A non-identity rest rotation on every joint, so on every chain; and a non-uniform
    //    rest scale on at least one joint.
    for (i, jt) in fam.joints.iter().enumerate() {
        assert_ne!(jt.rest.rotation, Quat::IDENTITY, "joint {i} has a rotation");
    }
    assert!(
        fam.joints.iter().any(|jt| {
            let s = jt.rest.scale;
            s.x != s.y || s.y != s.z
        }),
        "a joint is scaled non-uniformly"
    );
    // 2. Non-dyadic values: the low 8 mantissa bits are not all zero (0.3 is 0x3E99999A).
    let non_dyadic = |v: f32| v.to_bits() & 0xFF != 0;
    for v in RICH_CTL {
        assert!(non_dyadic(v), "control value {v}");
    }
    for jt in &fam.joints {
        for v in [
            jt.rest.translation.x,
            jt.rest.translation.y,
            jt.rest.translation.z,
        ] {
            assert!(v == 0.0 || non_dyadic(v), "joint translation {v}");
        }
    }
    assert!(fam.skin.iter().flatten().any(|&(_, w)| w == 0.3));
    assert!(fam.skin.iter().flatten().any(|&(_, w)| w == 0.7));
    // 3. A full 4-influence vertex.
    assert!(fam.skin.iter().any(|s| s.len() == 4));
    // 4. Weight sums that are inside the 1e-4 tolerance but not exact (f64 sums of the f32
    //    weights), one above and one below 1.
    let dev = |s: &Vec<(usize, f32)>| s.iter().map(|&(_, w)| f64::from(w)).sum::<f64>() - 1.0;
    assert!(fam.skin.iter().all(|s| dev(s).abs() < 1.0e-4));
    assert!(fam.skin.iter().any(|s| (dev(s) - 5.0e-5).abs() < 1.0e-6));
    assert!(fam.skin.iter().any(|s| (dev(s) + 5.0e-5).abs() < 1.0e-6));
    // 5. Declared influence order is not sorted by joint, and the taper deltas are not sorted
    //    by vertex and include a zero delta.
    assert!(fam
        .skin
        .iter()
        .any(|s| s.windows(2).any(|p| p[0].0 > p[1].0)));
    let taper = &fam.morphs[1].deltas;
    assert!(taper.windows(2).any(|p| p[0].0 > p[1].0));
    assert!(taper.iter().any(|&(_, d)| d == Vec3::ZERO));
    // 6. Two controls feed one morph target, and a bone control has two children.
    let girth_controls = fam
        .controls
        .iter()
        .filter(|c| c.effects == vec![ControlEffect::MorphWeight { target: 0 }])
        .count();
    assert_eq!(girth_controls, 2);
    assert_eq!(
        fam.joints.iter().filter(|jt| jt.parent == Some(1)).count(),
        2
    );
}

/// The shaped rest translations: a bone-length control scales the DIRECT children of its
/// joint, and nothing else, against the oracle's own shaping.
///
/// Designed to kill: mutant A (`ignored_skeleton_update`).
#[test]
fn rich_bone_length_controls_scale_direct_children_only() {
    let rev = rich_build(RICH_CTL);
    let joints = rev.skeleton.joints();
    // Joint 0 has no parent: its translation is exactly the declared one.
    assert_eq!(
        joints[0].rest.translation,
        Vec3::new(0.1, -0.05, 0.2),
        "the root is untouched"
    );
    // Joint 1 is the child of joint 0: scaled by torso_length (0.85) only.
    // Joints 2 and 3 are the children of joint 1: scaled by upper_length (1.3) only.
    for (j, ci) in [(1usize, 1usize), (2, 0), (3, 0)] {
        let t = RICH_JOINTS[j].2;
        let kk = f64::from(RICH_CTL[ci]);
        assert_close3(
            &format!("rest translation of joint {j}"),
            joints[j].rest.translation,
            [
                f64::from(t[0]) * kk,
                f64::from(t[1]) * kk,
                f64::from(t[2]) * kk,
            ],
        );
        // The rotation and scale are not touched by a bone-length control.
        assert_eq!(joints[j].rest.rotation.w, RICH_JOINTS[j].3[3]);
        assert_eq!(joints[j].rest.scale.y, RICH_JOINTS[j].4[1]);
    }
}

/// DIRECT comparison of the shaped skeleton's world bind and the inverse binds with the
/// oracle, beside (not instead of) the posed-mesh check, because the posed mesh cannot see a
/// skipped bone-length update unless the pose replaces a scaled child (PASS1 finding 5).
///
/// Designed to kill: mutant A (world bind and IB use the unscaled bone), mutant B (IB from the
/// unshaped skeleton; the world bind still matches, the IB does not).
#[test]
fn rich_world_bind_and_inverse_binds_match_oracle_directly() {
    let rig = rich_oracle();
    for ctl in rich_control_sets() {
        let rev = rich_build(ctl);
        let c = ctl64(&ctl);
        let rest = oracle::shaped_rest(&rig, &c);
        let ow = oracle::world_bind(&rig, &c);
        let oib = oracle::inverse_binds(&rig, &c);
        let world = rev.skeleton.world_bind();
        assert_eq!(world.len(), 4);
        assert_eq!(rev.inverse_binds.len(), 4);
        for i in 0..4 {
            let ctx = format!("ctl {ctl:?} joint {i}");
            assert_close3(
                &format!("{ctx}: rest t"),
                rev.skeleton.joints()[i].rest.translation,
                rest[i].t,
            );
            mat_close(&format!("{ctx}: world bind"), &world[i], &ow[i]);
            mat_close(
                &format!("{ctx}: inverse bind"),
                &rev.inverse_binds[i],
                &oib[i],
            );
        }
    }
}

/// Landmarks evaluated through the skeleton and compared with the oracle, at the bind pose
/// for every control set and in every posed case.
///
/// Designed to kill: mutant A (landmarks ride on joints whose world bind is wrong).
#[test]
fn rich_landmarks_match_oracle_at_bind_and_posed() {
    let rig = rich_oracle();
    for ctl in rich_control_sets() {
        let rev = rich_build(ctl);
        let ow = oracle::world_bind(&rig, &ctl64(&ctl));
        let world = rev.skeleton.world_bind();
        for l in 0..RICH_LANDMARKS.len() {
            assert_close3(
                &format!("ctl {ctl:?} landmark {l} at bind"),
                rev.skeleton.landmark_world(l, &world).unwrap(),
                oracle::landmark_world(&rig, &ow, l),
            );
        }
    }
    for case in rich_cases() {
        let rev = rich_build(case.ctl);
        let posed = rev.evaluate(&rich_pose(&case)).unwrap();
        let opose = oracle_pose_from(&case.overrides, &case.pose_morphs);
        let ow = oracle::posed_world(&rig, &ctl64(&case.ctl), &opose);
        for l in 0..RICH_LANDMARKS.len() {
            assert_close3(
                &format!("{}: landmark {l}", case.name),
                rev.skeleton.landmark_world(l, &posed.joint_world).unwrap(),
                oracle::landmark_world(&rig, &ow, l),
            );
        }
    }
}

/// The full posed mesh, posed joint worlds and posed landmarks against the f64 oracle, over
/// the rich cases: non-identity rest rotations, a non-uniform scale, 4-influence and
/// off-by-5e-5 weight vertices, morph-before-skin with rotated joints, and compile-time plus
/// pose-time morph weights together.
///
/// Designed to kill:
/// - mutant A, but ONLY through the cases that replace the local of a joint that a bone-length
///   control scaled, with a non-rest local: "joints 1 and 2 overridden", "scaled child joint 2
///   only", "root and arm overridden" (joint 3 is a scaled child) and "range-edge controls".
///   The bind-pose, root-only and default-control cases cannot see A in the mesh;
/// - mutant B in every case, even the bind pose (W' is shaped, IB is not);
/// - mutant C in every case with a non-zero compile-time or pose-time morph weight.
#[test]
fn rich_posed_mesh_matches_oracle() {
    let rig = rich_oracle();
    for case in rich_cases() {
        let rev = rich_build(case.ctl);
        let posed = rev.evaluate(&rich_pose(&case)).unwrap();
        let opose = oracle_pose_from(&case.overrides, &case.pose_morphs);
        let c = ctl64(&case.ctl);
        let ow = oracle::posed_world(&rig, &c, &opose);
        for (i, w) in posed.joint_world.iter().enumerate() {
            mat_close(&format!("{}: world {i}", case.name), w, &ow[i]);
        }
        let want = oracle::posed(&rig, &c, &opose);
        assert_eq!(posed.positions.len(), want.len());
        for (v, (p, w)) in posed.positions.iter().zip(&want).enumerate() {
            assert_close3(&format!("{}: vertex {v}", case.name), *p, *w);
        }
    }
}

/// At the bind pose the posed mesh is the shaped neutral, for every control set.
///
/// Designed to kill: mutant B (the skinning matrices W' * IB are not the identity), mutant C
/// (the shaped neutral against the oracle).
#[test]
fn rich_identity_pose_reproduces_the_shaped_neutral() {
    let rig = rich_oracle();
    for ctl in rich_control_sets() {
        let rev = rich_build(ctl);
        let posed = rev.evaluate(&PoseInput::new()).unwrap();
        let want = oracle::shaped_neutral(&rig, &ctl64(&ctl));
        for (v, (p, w)) in posed.positions.iter().zip(&want).enumerate() {
            assert_close3(&format!("ctl {ctl:?} vertex {v}"), *p, *w);
        }
        for (v, (p, n)) in posed
            .positions
            .iter()
            .zip(&rev.neutral.positions)
            .enumerate()
        {
            assert_close3(
                &format!("ctl {ctl:?} vs stored neutral, vertex {v}"),
                *p,
                [f64::from(n.x), f64::from(n.y), f64::from(n.z)],
            );
        }
    }
}

/// Compile-time morph weights: `girth` and `girth_fine` accumulate on one target, the zero
/// delta and the unsorted delta order do not matter to the geometry, and vertices outside a
/// target are untouched.
///
/// Designed to kill: mutant C (`noop_morph`) at compile time.
#[test]
fn rich_morph_controls_accumulate_on_one_target() {
    let rig = rich_oracle();
    let rev = rich_build(RICH_CTL);
    let want = oracle::shaped_neutral(&rig, &ctl64(&RICH_CTL));
    for (v, (p, w)) in rev.neutral.positions.iter().zip(&want).enumerate() {
        assert_close3(&format!("shaped neutral vertex {v}"), *p, *w);
    }
    // Vertex 4 moves by (0.35 + 0.15) * (-0.12, 0, -0.08) from its declared position.
    let base = rich_positions()[4];
    assert_close3(
        "vertex 4",
        rev.neutral.positions[4],
        [
            f64::from(base[0]) - 0.5 * 0.12,
            f64::from(base[1]),
            f64::from(base[2]) - 0.5 * 0.08,
        ],
    );
    // The zero delta and the vertices outside every target stay exactly where they were.
    assert_eq!(rev.neutral.positions[14], v3(rich_positions()[14]));
    assert_eq!(rev.neutral.positions[0], v3(rich_positions()[0]));
}

/// A POSE-time morph weight, on a body whose compile-time morph weights are all zero, moves
/// the vertices, and it moves them BEFORE skinning: the same rotated pose with and without the
/// pose morph is compared with the oracle, which morphs first and skins second. A morph applied
/// after skinning would differ under the rotations here.
///
/// Designed to kill: mutant C (`noop_morph`) on the pose-time path specifically.
#[test]
fn rich_pose_time_morph_moves_vertices_before_skinning() {
    let rig = rich_oracle();
    let rev = rich_build(RICH_DEFAULTS);
    let c = ctl64(&RICH_DEFAULTS);
    let ovr: Vec<Ovr> = vec![
        (0, [0.1, -0.05, 0.2], axis_angle([0.0, 1.0, 0.0], 0.3), UNIT),
        (
            3,
            [0.6, 0.3, 0.05],
            axis_angle([1.0, 0.0, 1.0], 0.4),
            [1.0, 1.1, 0.9],
        ),
    ];
    let morphs: Vec<(&'static str, usize, f32)> = vec![("girth", 0, 0.8), ("taper", 1, -0.6)];
    let without = rev.evaluate(&pose_from(&ovr, &[])).unwrap();
    let with = rev.evaluate(&pose_from(&ovr, &morphs)).unwrap();
    let o_without = oracle::posed(&rig, &c, &oracle_pose_from(&ovr, &[]));
    let o_with = oracle::posed(&rig, &c, &oracle_pose_from(&ovr, &morphs));
    for v in 0..16 {
        assert_close3(
            &format!("without morph, vertex {v}"),
            without.positions[v],
            o_without[v],
        );
        assert_close3(
            &format!("with morph, vertex {v}"),
            with.positions[v],
            o_with[v],
        );
        // How far the oracle says the pose morph moves this vertex, and how far we moved it.
        let want: f64 = (0..3)
            .map(|k| (o_with[v][k] - o_without[v][k]).powi(2))
            .sum::<f64>()
            .sqrt();
        let got: f64 = [
            with.positions[v].x - without.positions[v].x,
            with.positions[v].y - without.positions[v].y,
            with.positions[v].z - without.positions[v].z,
        ]
        .iter()
        .map(|d| f64::from(*d).powi(2))
        .sum::<f64>()
        .sqrt();
        assert!(
            (got - want).abs() <= 1.0e-4,
            "vertex {v}: moved {got}, oracle {want}"
        );
        // Girth moves vertices 4..12 by about 0.1 even after the rotations; the taper moves
        // 12, 13 and 15 (not the zero-delta vertex 14).
        if (4..12).contains(&v) {
            assert!(
                got > 0.05,
                "girth pose morph must move vertex {v} (moved {got})"
            );
        }
        if v == 12 || v == 13 || v == 15 {
            assert!(
                got > 0.01,
                "taper pose morph must move vertex {v} (moved {got})"
            );
        }
        if v == 14 || v < 4 {
            assert!(got < 1.0e-4, "vertex {v} has no delta (moved {got})");
        }
    }
}

/// Weights are used exactly as declared. A sum of `1 + 5e-5` is inside the validation
/// tolerance and is NOT renormalized. This needs its own absolute tolerance: the oracle
/// comparisons cannot see it, because `5e-5 * |x|` is below `TOL * max(1, |x|)` at every
/// coordinate of the rich fixture. The vertex sits at x = 20: an unrenormalized result is
/// `20 * 1.00005 = 20.001`, a renormalized one is `20.0`, a gap of 1e-3 against the absolute
/// tolerance 1e-4 (f32 rounding at 20 is about 2e-6).
///
/// Designed to kill: a renormalizing implementation (not one of the three planted mutants).
#[test]
fn weights_are_applied_as_declared_never_renormalized() {
    let id = FamilyRef::new("synthetic.weights", 1);
    let fam = BodyFamily::new(
        id.clone(),
        NeutralMesh::new(vec![Vec3::new(20.0, 0.0, 0.0); 3], vec![0, 1, 2]),
        vec![Joint::new("only", None, Transform::IDENTITY)],
        vec![],
        vec![vec![(0usize, 1.00005f32)]; 3],
    );
    let rev = compile(&BodyRecipe::new(id, vec![]), &fam).expect("compiles");
    // Stored exactly as declared.
    assert_eq!(rev.skin.influences(0).unwrap(), vec![(0usize, 1.00005f32)]);
    let posed = rev.evaluate(&PoseInput::new()).unwrap();
    for p in &posed.positions {
        assert!(
            (f64::from(p.x) - 20.001).abs() < 1.0e-4,
            "posed x = {}, want 20.001 (20.0 would mean renormalization)",
            p.x
        );
        assert!(p.y.abs() < 1.0e-6 && p.z.abs() < 1.0e-6);
    }
}

/// Each of the five controls, moved alone from the defaults, mints a different `rev_id`, and
/// the explicit defaults equal the omitted controls.
///
/// Designed to kill: mutant A (a bone-length control that changes no byte), mutant C (a girth
/// control that moves no vertex, so the shaped positions in the bytes do not change).
#[test]
fn rich_each_control_changes_rev_id() {
    let base = rich_build(RICH_DEFAULTS).rev_id;
    let omitted = compile(&BodyRecipe::new(rich_fam_ref(), vec![]), &rich_family())
        .unwrap()
        .rev_id;
    assert_eq!(base, omitted, "explicit defaults equal omitted controls");
    let moved = [1.3f32, 0.85, 0.35, 0.15, -0.4];
    let mut ids = vec![base];
    for (i, v) in moved.iter().enumerate() {
        let mut ctl = RICH_DEFAULTS;
        ctl[i] = *v;
        let id = rich_build(ctl).rev_id;
        assert_ne!(
            id, base,
            "control {} alone must change rev_id",
            RICH_CONTROL_IDS[i]
        );
        ids.push(id);
    }
    for a in 0..ids.len() {
        for b in a + 1..ids.len() {
            assert_ne!(ids[a], ids[b], "ids {a} and {b} must differ");
        }
    }
}

/// REGRESSION PIN (not independent). The hash was produced by
/// `docs/evidence/m5b/golden_rev_id.py`, a numpy float32 replication that mirrors the
/// implementation's float evaluation order ON PURPOSE and shares its formulas (adjugate
/// inverse, column-scaled `to_mat`). It pins the documented byte layout and the float32
/// arithmetic order for the rich fixture and the pinned recipe, and it detects drift. It is
/// NOT an independent bearing on correctness: if the implementation had a formula error, the
/// script would carry it too. The independent bearings are the f64 oracle tests above and
/// `tests/figure_canonical_bytes.rs`. A change here means "the bytes changed", not "which side
/// is right". Drift requires a minor release; x86_64 only.
///
/// Mutants A, B and C all change this hash, but it is never the only test that kills them.
#[test]
fn golden_rev_id() {
    let rev = rich_build(RICH_CTL);
    assert_eq!(
        rev.rev_id.to_hex(),
        "0042bc1826da28bce4f31b222b6b86382f60da56e0c762076d8e2edb98063ed8"
    );
}
