//! M5-C slice 3: skeleton re-derivation from the shaped surface (`figure::refit`).
//!
//! Evidence, each with a control that must fail:
//! - an analytic fixture with a rotated parent: joints land on hand-derived centroids, bone
//!   lengths and inverse binds follow;
//! - the SHAPED path: moving an anchor group moves its joint and everything unanchored below it,
//!   by exactly the move; with the unshaped surface the declared skeleton is reproduced;
//! - an independent f64 check (`tests/figure_oracle`, which shares no code with `figure::math`):
//!   the world bind of the derived skeleton, built in the oracle, sits on the f64 centroids, and
//!   the derived inverse binds equal the oracle's Gauss-Jordan inverses;
//! - the summation order is the declared order (f32 sums of `1e8, -1e8, 1` differ by order);
//! - typed errors, hard limits, and "no anchors changes nothing".
//!
//! Mutation control (`RUSTFLAGS='--cfg figure_mutant="ignored_refit"'`): the surface is ignored
//! and the declared rest translations are kept. Killed by the analytic, shaped, oracle and
//! order tests; none inspects the cfg. Predicted matrix:
//! `docs/evidence/m5c/KILL_MATRIX_REFIT_PREDICTED.md`.
#![cfg(feature = "figure")]
#![allow(clippy::needless_range_loop)]

mod figure_oracle;

use figure_oracle as oracle;
use gibson::figure::math::{Quat, Transform, Vec3};
use gibson::figure::refit::{
    refit_skeleton, JointAnchor, RefitError, MAX_GROUP_TOTAL, MAX_GROUP_VERTICES,
};
use gibson::figure::skeleton::{Joint, Landmark, Skeleton};

/// Tolerance, declared before any comparison: absolute-or-relative
/// `|got - want| <= TOL * max(1, |want|)`. The analytic fixtures are two or three products deep
/// with entries of magnitude at most 5: `N * u * S` with `u = 2^-24`, `N = 48` dependent
/// operations (`from_transform` 12, three products 21, inverse 15) and `S = 12` gives 3.4e-5.
/// TOL = 1e-4 is 2.9x above that and 2000x below the smallest displacement any test uses (0.1).
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

fn ones() -> Vec3 {
    Vec3::new(1.0, 1.0, 1.0)
}

fn rz(deg: f32) -> Quat {
    Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), deg.to_radians())
}

/// Root rotated 90 degrees about z, then two joints whose declared translations are junk (the
/// refit must never read them, which `ignored_refit` would do).
fn rotated_chain() -> Skeleton {
    Skeleton::new(
        vec![
            Joint::new(
                "root",
                None,
                Transform::new(Vec3::new(0.0, 0.0, 0.0), rz(90.0), ones()),
            ),
            Joint::new(
                "mid",
                Some(0),
                Transform::new(Vec3::new(9.0, 9.0, 9.0), Quat::IDENTITY, ones()),
            ),
            Joint::new(
                "tip",
                Some(1),
                Transform::new(Vec3::new(7.0, 7.0, 7.0), Quat::IDENTITY, ones()),
            ),
        ],
        vec![Landmark::new("tip_end", 2, Vec3::new(0.0, 0.5, 0.0))],
    )
    .expect("valid skeleton")
}

fn rotated_chain_surface() -> (Vec<Vec3>, Vec<JointAnchor>) {
    let shaped = vec![
        Vec3::new(-1.0, 0.0, 0.5),
        Vec3::new(1.0, 0.0, -0.5),
        Vec3::new(0.0, 1.5, 0.0),
        Vec3::new(0.0, 2.5, 0.0),
        Vec3::new(0.0, 3.0, 0.5),
        Vec3::new(0.0, 5.0, -0.5),
    ];
    let anchors = vec![
        JointAnchor::new(0, vec![0, 1]),
        JointAnchor::new(1, vec![2, 3]),
        JointAnchor::new(2, vec![4, 5]),
    ];
    (shaped, anchors)
}

/// Hand derivation. Group centroids (world): root (0,0,0), mid (0,2,0), tip (0,4,0). The root's
/// rotation maps x to y, so `Rz(-90)` maps (0,2,0) to (2,0,0): the mid joint's local translation
/// is (2,0,0), and the same for the tip below the mid. Bone lengths 2 and 2. World bind
/// translations are the centroids. Inverse-bind translations are `-Rz(-90) c`: (0,0,0),
/// (-2,0,0), (-4,0,0). Rotations, scales and landmarks are carried over untouched.
/// Kills `ignored_refit` (the declared junk (9,9,9) and (7,7,7) would survive).
#[test]
fn joints_land_on_the_group_centroids_in_a_rotated_frame() {
    let sk = rotated_chain();
    let (shaped, anchors) = rotated_chain_surface();
    let r = refit_skeleton(&sk, &anchors, &shaped).expect("refit");
    let js = r.skeleton.joints();
    assert_close3("root t", js[0].rest.translation, [0.0, 0.0, 0.0]);
    assert_close3("mid t", js[1].rest.translation, [2.0, 0.0, 0.0]);
    assert_close3("tip t", js[2].rest.translation, [2.0, 0.0, 0.0]);

    assert_eq!(r.bone_length(0), None, "a root has no bone");
    assert!(close(r.bone_length(1).expect("mid"), 2.0));
    assert!(close(r.bone_length(2).expect("tip"), 2.0));
    assert_eq!(r.bone_length(3), None, "out of range");

    let world = r.skeleton.world_bind();
    let want_world = [[0.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 4.0, 0.0]];
    let want_ib_t = [[0.0, 0.0, 0.0], [-2.0, 0.0, 0.0], [-4.0, 0.0, 0.0]];
    for i in 0..3 {
        let c = world[i].cols[3];
        assert_close3(
            &format!("world bind {i}"),
            Vec3::new(c[0], c[1], c[2]),
            want_world[i],
        );
        let t = r.inverse_binds[i].cols[3];
        assert_close3(
            &format!("inverse bind {i}"),
            Vec3::new(t[0], t[1], t[2]),
            want_ib_t[i],
        );
        assert_eq!(js[i].rest.rotation, sk.joints()[i].rest.rotation);
        assert_eq!(js[i].rest.scale, sk.joints()[i].rest.scale);
        assert_eq!(js[i].name, sk.joints()[i].name);
        assert_eq!(js[i].parent, sk.joints()[i].parent);
    }
    assert_eq!(r.skeleton.landmarks(), sk.landmarks());
    // Same call, same bits.
    assert_eq!(r, refit_skeleton(&sk, &anchors, &shaped).expect("again"));
}

/// Identity-rotation chain: root, arm (anchored), hand (NOT anchored, declared (0,1,0)).
fn straight_chain() -> Skeleton {
    Skeleton::new(
        vec![
            Joint::new("root", None, Transform::IDENTITY),
            Joint::new(
                "arm",
                Some(0),
                Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            ),
            Joint::new(
                "hand",
                Some(1),
                Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            ),
        ],
        Vec::new(),
    )
    .expect("valid skeleton")
}

fn straight_surface(arm_shift: Vec3) -> Vec<Vec3> {
    vec![
        Vec3::new(-0.5, 0.0, 0.0),
        Vec3::new(0.5, 0.0, 0.0),
        Vec3::new(-0.25, 1.0, 0.0).add(arm_shift),
        Vec3::new(0.25, 1.0, 0.0).add(arm_shift),
    ]
}

fn straight_anchors() -> Vec<JointAnchor> {
    vec![
        JointAnchor::new(0, vec![0, 1]),
        JointAnchor::new(1, vec![2, 3]),
    ]
}

/// The shaped path, both halves. With the NEUTRAL surface the declared skeleton comes back; with
/// the arm group moved by `d` the arm joint moves by exactly `d` in its parent's frame, the
/// unanchored hand rides with it (its local translation is kept, so its world position moves by
/// `d`), the bone length is the new length and the inverse binds follow. Kills `ignored_refit`
/// (the arm would not move).
#[test]
fn a_shaped_surface_moves_the_skeleton_and_the_unanchored_child_rides_along() {
    let sk = straight_chain();
    let anchors = straight_anchors();

    let neutral = refit_skeleton(&sk, &anchors, &straight_surface(Vec3::ZERO)).expect("neutral");
    let js = neutral.skeleton.joints();
    assert_close3("neutral arm", js[1].rest.translation, [0.0, 1.0, 0.0]);
    assert_close3("neutral hand", js[2].rest.translation, [0.0, 1.0, 0.0]);
    assert!(close(neutral.bone_length(1).expect("arm"), 1.0));

    let d = Vec3::new(0.0, 0.3, 0.1);
    let shaped = refit_skeleton(&sk, &anchors, &straight_surface(d)).expect("shaped");
    let js = shaped.skeleton.joints();
    assert_close3("shaped arm", js[1].rest.translation, [0.0, 1.3, 0.1]);
    assert_close3("hand local kept", js[2].rest.translation, [0.0, 1.0, 0.0]);
    let w = shaped.skeleton.world_bind();
    assert_close3(
        "hand world",
        Vec3::new(w[2].cols[3][0], w[2].cols[3][1], w[2].cols[3][2]),
        [0.0, 2.3, 0.1],
    );
    let want_len = (1.3f64 * 1.3 + 0.1 * 0.1).sqrt();
    assert!(close(shaped.bone_length(1).expect("arm"), want_len));
    let ib = shaped.inverse_binds[1].cols[3];
    assert_close3(
        "arm inverse bind",
        Vec3::new(ib[0], ib[1], ib[2]),
        [0.0, -1.3, -0.1],
    );
    let ib = shaped.inverse_binds[2].cols[3];
    assert_close3(
        "hand inverse bind",
        Vec3::new(ib[0], ib[1], ib[2]),
        [0.0, -2.3, -0.1],
    );
}

/// Five joints with rotations, a non-uniform scale and junk declared translations, 16 vertices
/// from a generator, five anchor groups of 3, 2, 4, 3 and 4 vertices. The oracle builds the world
/// bind of the derived rest transforms (its own TRS products) and its translation must be the
/// f64 centroid of the group; the derived inverse binds must equal the oracle's Gauss-Jordan
/// inverses. Kills `ignored_refit`.
#[test]
fn derived_skeleton_matches_the_f64_centroids_and_inverse_binds() {
    let rot = |axis: [f32; 3], deg: f32| {
        Quat::from_axis_angle(Vec3::new(axis[0], axis[1], axis[2]), deg.to_radians())
    };
    let spec: [(Option<usize>, [f32; 3], f32, Vec3); 5] = [
        (None, [1.0, 2.0, 3.0], 25.0, ones()),
        (Some(0), [0.0, 1.0, 1.0], -40.0, ones()),
        (Some(1), [1.0, 0.0, 2.0], 70.0, Vec3::new(1.2, 0.9, 1.1)),
        (Some(1), [1.0, 1.0, 1.0], 15.0, ones()),
        (Some(3), [2.0, -1.0, 0.0], -55.0, ones()),
    ];
    let names = ["j0", "j1", "j2", "j3", "j4"];
    let sk = Skeleton::new(
        spec.iter()
            .enumerate()
            .map(|(i, &(p, axis, deg, s))| {
                Joint::new(
                    names[i],
                    p,
                    Transform::new(Vec3::new(8.0, 8.0, 8.0), rot(axis, deg), s),
                )
            })
            .collect(),
        Vec::new(),
    )
    .expect("valid skeleton");

    // A deterministic, non-dyadic surface.
    let mut state = 2_463_534_242u32;
    let mut next = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) as f32 / 16_777_216.0
    };
    let shaped: Vec<Vec3> = (0..16)
        .map(|_| Vec3::new(next() * 3.0 - 1.5, next() * 4.0 - 0.5, next() * 3.0 - 1.5))
        .collect();
    let groups: [Vec<u32>; 5] = [
        vec![0, 1, 2],
        vec![3, 4],
        vec![5, 6, 7, 8],
        vec![9, 10, 11],
        vec![12, 13, 14, 15],
    ];
    let anchors: Vec<JointAnchor> = groups
        .iter()
        .enumerate()
        .map(|(j, g)| JointAnchor::new(j, g.clone()))
        .collect();
    let r = refit_skeleton(&sk, &anchors, &shaped).expect("refit");

    // The oracle's world chain over the derived rest transforms.
    let q4 = |q: Quat| {
        [
            f64::from(q.x),
            f64::from(q.y),
            f64::from(q.z),
            f64::from(q.w),
        ]
    };
    let mut worlds: Vec<oracle::M> = Vec::new();
    for (i, j) in r.skeleton.joints().iter().enumerate() {
        let local = oracle::trs_matrix(&oracle::OTrs {
            t: [
                f64::from(j.rest.translation.x),
                f64::from(j.rest.translation.y),
                f64::from(j.rest.translation.z),
            ],
            q: q4(j.rest.rotation),
            s: [
                f64::from(j.rest.scale.x),
                f64::from(j.rest.scale.y),
                f64::from(j.rest.scale.z),
            ],
        });
        let w = match j.parent {
            Some(p) => oracle::matmul(&worlds[p], &local),
            None => local,
        };
        // The f64 centroid of the group, from the same f32 inputs widened.
        let n = groups[i].len() as f64;
        let mut c = [0.0f64; 3];
        for &v in &groups[i] {
            let p = shaped[v as usize];
            c[0] += f64::from(p.x);
            c[1] += f64::from(p.y);
            c[2] += f64::from(p.z);
        }
        let want = [c[0] / n, c[1] / n, c[2] / n];
        let got = [w[0][3], w[1][3], w[2][3]];
        for k in 0..3 {
            assert!(
                (got[k] - want[k]).abs() <= TOL * want[k].abs().max(1.0),
                "joint {i}: oracle world position {got:?}, f64 centroid {want:?}"
            );
        }
        let inv = oracle::invert(&w).expect("oracle inverse");
        for c in 0..4 {
            for row in 0..4 {
                let have = r.inverse_binds[i].cols[c][row];
                assert!(
                    close(have, inv[row][c]),
                    "inverse bind {i} [{row}][{c}]: {have} vs {}",
                    inv[row][c]
                );
            }
        }
        worlds.push(w);
    }
}

fn single_root() -> Skeleton {
    Skeleton::new(
        vec![Joint::new("root", None, Transform::IDENTITY)],
        Vec::new(),
    )
    .expect("valid skeleton")
}

/// The centroid sums in the group's DECLARED order. In f32, `0 + 1e8 - 1e8 + 1` is 1 but
/// `0 + 1 + 1e8 - 1e8` is 0, so the two orders of the same three vertices give 1/3 and exactly 0.
/// Pins the evaluation order that the contract makes part of the bytes. Kills `ignored_refit`.
#[test]
fn the_centroid_sums_in_declared_order() {
    let shaped = vec![
        Vec3::new(1.0e8, 0.0, 0.0),
        Vec3::new(-1.0e8, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
    ];
    let sk = single_root();
    let a = refit_skeleton(&sk, &[JointAnchor::new(0, vec![0, 1, 2])], &shaped).expect("a");
    assert!(
        (a.skeleton.joints()[0].rest.translation.x - 1.0 / 3.0).abs() < 1.0e-6,
        "order 1e8, -1e8, 1 gives {}",
        a.skeleton.joints()[0].rest.translation.x
    );
    let b = refit_skeleton(&sk, &[JointAnchor::new(0, vec![2, 0, 1])], &shaped).expect("b");
    assert_eq!(
        b.skeleton.joints()[0].rest.translation.x,
        0.0,
        "order 1, 1e8, -1e8 must absorb the 1"
    );
}

/// With no anchors nothing is re-derived: the skeleton and the inverse binds are exactly the
/// declared ones, bit for bit.
#[test]
fn no_anchors_changes_nothing() {
    let sk = rotated_chain();
    let r = refit_skeleton(&sk, &[], &[]).expect("refit");
    assert_eq!(r.skeleton, sk);
    assert_eq!(r.inverse_binds, sk.inverse_binds().expect("binds"));
}

/// Every bad input is a typed error naming the part, never a panic, and nothing is returned.
#[test]
fn bad_anchors_are_typed_errors() {
    let sk = straight_chain();
    let shaped = straight_surface(Vec3::ZERO);
    let e = |anchors: &[JointAnchor], pos: &[Vec3]| {
        refit_skeleton(&sk, anchors, pos).expect_err("must be refused")
    };
    assert_eq!(
        e(&[JointAnchor::new(5, vec![0])], &shaped),
        RefitError::JointOutOfRange { joint: 5 }
    );
    assert_eq!(
        e(
            &[JointAnchor::new(1, vec![0]), JointAnchor::new(1, vec![1])],
            &shaped
        ),
        RefitError::DuplicateAnchor { joint: 1 }
    );
    assert_eq!(
        e(&[JointAnchor::new(1, vec![])], &shaped),
        RefitError::EmptyGroup { joint: 1 }
    );
    assert_eq!(
        e(&[JointAnchor::new(1, vec![0, 9])], &shaped),
        RefitError::VertexOutOfRange {
            joint: 1,
            vertex: 9
        }
    );
    assert_eq!(
        e(&[JointAnchor::new(1, vec![1, 2, 1])], &shaped),
        RefitError::DuplicateVertex {
            joint: 1,
            vertex: 1
        }
    );
    let mut bad = shaped.clone();
    bad[2] = Vec3::new(f32::NAN, 0.0, 0.0);
    assert_eq!(
        e(&[JointAnchor::new(1, vec![2, 3])], &bad),
        RefitError::NonFinitePosition { vertex: 2 }
    );
}

/// The group caps are checked before any derived data: one group over `MAX_GROUP_VERTICES`, and
/// 17 groups of exactly `MAX_GROUP_VERTICES` over `MAX_GROUP_TOTAL`.
#[test]
fn group_limits_are_enforced() {
    let shaped = vec![Vec3::ZERO; MAX_GROUP_VERTICES + 1];
    let sk = single_root();
    let big: Vec<u32> = (0..=MAX_GROUP_VERTICES as u32).collect();
    assert_eq!(
        refit_skeleton(&sk, &[JointAnchor::new(0, big)], &shaped).expect_err("limit"),
        RefitError::LimitExceeded {
            what: "anchor group vertices",
            count: MAX_GROUP_VERTICES + 1,
            max: MAX_GROUP_VERTICES
        }
    );

    let n_groups = MAX_GROUP_TOTAL / MAX_GROUP_VERTICES + 1;
    let chain = Skeleton::new(
        (0..n_groups)
            .map(|i| {
                Joint::new(
                    format!("j{i}"),
                    if i == 0 { None } else { Some(i - 1) },
                    Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
                )
            })
            .collect(),
        Vec::new(),
    )
    .expect("valid skeleton");
    let group: Vec<u32> = (0..MAX_GROUP_VERTICES as u32).collect();
    let anchors: Vec<JointAnchor> = (0..n_groups)
        .map(|i| JointAnchor::new(i, group.clone()))
        .collect();
    let positions = vec![Vec3::ZERO; MAX_GROUP_VERTICES];
    assert_eq!(
        refit_skeleton(&chain, &anchors, &positions).expect_err("limit"),
        RefitError::LimitExceeded {
            what: "anchor vertices",
            count: n_groups * MAX_GROUP_VERTICES,
            max: MAX_GROUP_TOTAL
        }
    );
}
