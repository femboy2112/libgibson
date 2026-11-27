//! Body-family layer: authored origin, region-scoped controls, locks, rebind, stale detection.
//!
//! The fixture is a SYNTHETIC procedural body: 16 stacked ellipse rings of 16 vertices each
//! (a mannequin tube, no character data of any kind) with a nine-joint bone chain. Rings are
//! named by height: 0 foot (locked), 1-2 calf, 3 knee, 4-5 thigh, 6-7 hip/pelvis, 8-9 waist,
//! 10-12 ribcage/bust, 13 shoulder, 14-15 neck/head (locked).
//!
//! DISCRIMINATING CONTROLS (each is a test that must fail if the guarded behaviour is removed):
//! ignored parameter, wrong-region morph, stale skeleton (refused AND observably wrong if used),
//! no-op edit (same revision), locked-region change (refused at authoring, bit-identical at
//! evaluation), topology mismatch, distinct origins, symmetry pairing, corrective gating.
#![cfg(feature = "figure")]
#![allow(clippy::needless_range_loop)]

use std::f32::consts::PI;

use gibson::figure::family::{
    BodyFamily, Corrective, EvalError, FamilyControl, FamilyError, FamilyRegion, FamilySpec,
    JointAnchor, RebindError, RigSpec, ShapedBody, Side, REQUIRED_CONTROLS,
};
use gibson::figure::math::{Quat, Transform, Vec3};
use gibson::figure::skeleton::{Joint, Skeleton};
use gibson::figure::skin::SkinWeights;

const K: usize = 16;
const R: usize = 16;

// Per-ring ellipse radii (x lateral, y front/back), foot to head.
const RX: [f32; R] = [
    0.07, 0.06, 0.07, 0.06, 0.08, 0.10, 0.14, 0.17, 0.13, 0.12, 0.13, 0.15, 0.15, 0.17, 0.05, 0.09,
];
const RY: [f32; R] = [
    0.10, 0.06, 0.07, 0.06, 0.08, 0.10, 0.12, 0.13, 0.10, 0.09, 0.11, 0.12, 0.11, 0.09, 0.05, 0.09,
];

fn vid(r: usize, k: usize) -> u32 {
    (r * K + k) as u32
}
fn theta(k: usize) -> f32 {
    k as f32 * 2.0 * PI / K as f32
}
fn ring_z(r: usize) -> f32 {
    r as f32 * 0.12
}
fn is_left(k: usize) -> bool {
    !(4..=12).contains(&k) // k = 13, 14, 15, 0, 1, 2, 3
}
fn is_right(k: usize) -> bool {
    (5..=11).contains(&k)
}
fn is_front(k: usize) -> bool {
    theta(k).sin() > 0.3
}
fn is_back(k: usize) -> bool {
    theta(k).sin() < -0.3
}
fn is_side(k: usize, c: f32) -> bool {
    theta(k).cos().abs() > c
}

fn origin(xscale: f32) -> Vec<Vec3> {
    let mut p = Vec::new();
    for r in 0..R {
        for k in 0..K {
            p.push(Vec3::new(
                xscale * RX[r] * theta(k).cos(),
                RY[r] * theta(k).sin(),
                ring_z(r),
            ));
        }
    }
    p
}

fn indices() -> Vec<u32> {
    let mut ix = Vec::new();
    for r in 0..R - 1 {
        for k in 0..K {
            let k2 = (k + 1) % K;
            ix.extend([vid(r, k), vid(r, k2), vid(r + 1, k)]);
            ix.extend([vid(r, k2), vid(r + 1, k2), vid(r + 1, k)]);
        }
    }
    ix
}

fn mirror_map() -> Vec<u32> {
    let mut m = vec![0u32; R * K];
    for r in 0..R {
        for k in 0..K {
            m[r * K + k] = vid(r, (K / 2 + K - k) % K);
        }
    }
    m
}

fn verts(rings: std::ops::RangeInclusive<usize>, pred: impl Fn(usize) -> bool) -> Vec<u32> {
    let mut v = Vec::new();
    for r in rings {
        for k in 0..K {
            if pred(k) {
                v.push(vid(r, k));
            }
        }
    }
    v
}

fn all(_: usize) -> bool {
    true
}

/// A bipolar control: positive side is `f`, negative side is `-f`.
fn bipolar(id: &str, region: &[u32], f: impl Fn(usize, usize) -> Vec3) -> FamilyControl {
    let mut pos = Vec::new();
    let mut neg = Vec::new();
    for &v in region {
        let (r, k) = (v as usize / K, v as usize % K);
        let d = f(r, k);
        pos.push((v, d));
        neg.push((v, d.neg()));
    }
    FamilyControl::new(id, -1.0, 1.0, 0.0, id, neg, pos)
}

fn radial(r: usize, k: usize, s: f32) -> Vec3 {
    let _ = r;
    Vec3::new(theta(k).cos() * s, theta(k).sin() * s, 0.0)
}

/// The required control set, defined procedurally. Each control's region is its own vertex set.
fn required_controls() -> (Vec<FamilyRegion>, Vec<FamilyControl>) {
    type Def = (&'static str, Vec<u32>, Box<dyn Fn(usize, usize) -> Vec3>);
    let defs: Vec<Def> = vec![
        (
            "ribcage_width",
            verts(10..=12, all),
            Box::new(|_, k| Vec3::new(theta(k).cos() * 0.02, 0.0, 0.0)),
        ),
        (
            "ribcage_depth",
            verts(10..=12, all),
            Box::new(|_, k| Vec3::new(0.0, theta(k).sin() * 0.02, 0.0)),
        ),
        (
            "bust_volume",
            verts(10..=12, is_front),
            Box::new(|r, k| {
                let d = radial(r, k, 0.015);
                Vec3::new(d.x * 0.6, d.y, 0.0)
            }),
        ),
        (
            "bust_projection",
            verts(11..=11, is_front),
            Box::new(|_, k| Vec3::new(0.0, 0.03 * theta(k).sin(), 0.0)),
        ),
        (
            "bust_base",
            verts(10..=10, is_front),
            Box::new(|_, k| Vec3::new(0.0, 0.005 * theta(k).sin(), -0.015 * theta(k).sin())),
        ),
        (
            "bust_fullness",
            verts(12..=12, is_front),
            Box::new(|_, k| Vec3::new(0.0, 0.01 * theta(k).sin(), 0.02 * theta(k).sin())),
        ),
        (
            "waist_width",
            verts(8..=9, all),
            Box::new(|_, k| Vec3::new(theta(k).cos() * 0.02, 0.0, 0.0)),
        ),
        (
            "waist_depth",
            verts(8..=9, all),
            Box::new(|_, k| Vec3::new(0.0, theta(k).sin() * 0.02, 0.0)),
        ),
        (
            "pelvis",
            verts(6..=7, all),
            Box::new(|r, _| Vec3::new(0.0, 0.0, if r == 7 { -0.02 } else { -0.01 })),
        ),
        (
            "hip_flare",
            verts(6..=7, |k| is_side(k, 0.5)),
            Box::new(|_, k| Vec3::new(theta(k).cos() * 0.03, 0.0, 0.0)),
        ),
        (
            "glute_projection",
            verts(5..=7, is_back),
            Box::new(|_, k| Vec3::new(0.0, theta(k).sin() * 0.03, 0.0)),
        ),
        (
            "glute_fullness",
            verts(5..=6, is_back),
            Box::new(|r, k| radial(r, k, 0.015)),
        ),
        (
            "thigh_upper",
            verts(4..=5, all),
            Box::new(|r, k| radial(r, k, 0.02)),
        ),
        (
            "thigh_lower",
            verts(3..=4, all),
            Box::new(|r, k| radial(r, k, 0.015)),
        ),
        (
            "shoulder",
            verts(13..=13, all),
            Box::new(|_, k| Vec3::new(theta(k).cos() * 0.02, 0.0, 0.01)),
        ),
        (
            "upper_arm",
            verts(12..=13, |k| is_side(k, 0.7)),
            Box::new(|_, k| Vec3::new(theta(k).cos() * 0.02, 0.0, 0.0)),
        ),
        (
            "calf",
            verts(1..=2, all),
            Box::new(|r, k| radial(r, k, 0.02)),
        ),
        (
            "posture",
            verts(8..=13, all),
            Box::new(|r, _| Vec3::new(0.0, (r as f32 - 7.0) * 0.01, 0.0)),
        ),
    ];
    let mut regions = Vec::new();
    let mut controls = Vec::new();
    for (id, vs, f) in defs {
        controls.push(bipolar(id, &vs, f));
        regions.push(FamilyRegion::new(id, vs));
    }
    (regions, controls)
}

/// A mirrored left/right control pair (not part of the required set).
fn arm_pair() -> (Vec<FamilyRegion>, Vec<FamilyControl>) {
    let l = verts(12..=13, is_left);
    let r = verts(12..=13, is_right);
    let f = |_: usize, k: usize| Vec3::new(theta(k).cos() * 0.02, 0.0, 0.0);
    let cl = bipolar("arm_l", &l, f).with_mirror_partner("arm_r");
    let cr = bipolar("arm_r", &r, f).with_mirror_partner("arm_l");
    (
        vec![FamilyRegion::new("arm_l", l), FamilyRegion::new("arm_r", r)],
        vec![cl, cr],
    )
}

fn spec_with(name: &str, revision: u32, xscale: f32) -> FamilySpec {
    let mut spec = FamilySpec::new(name, revision, origin(xscale), indices());
    let (mut regions, mut controls) = required_controls();
    let (ar, ac) = arm_pair();
    regions.extend(ar);
    controls.extend(ac);
    regions.push(FamilyRegion::new("head_locked", verts(14..=15, all)));
    regions.push(FamilyRegion::new("feet_locked", verts(0..=0, all)));
    spec.regions = regions;
    spec.controls = controls;
    spec.locked = vec!["head_locked".to_string(), "feet_locked".to_string()];
    spec.mirror = mirror_map();
    spec
}

fn family() -> BodyFamily {
    BodyFamily::new(spec_with("synthetic_tube", 1, 1.0)).expect("synthetic family is valid")
}

fn rv(pairs: &[(&str, f32)]) -> Vec<(String, f32)> {
    pairs.iter().map(|&(k, v)| (k.to_string(), v)).collect()
}

// ---- rig ------------------------------------------------------------------------------------

const PELVIS: usize = 0;
const SPINE: usize = 1;
const CHEST: usize = 2;
const THIGH_L: usize = 5;

fn centroid(p: &[Vec3], vs: &[u32]) -> Vec3 {
    let mut s = Vec3::ZERO;
    for &v in vs {
        s = s.add(p[v as usize]);
    }
    s.scale(1.0 / vs.len() as f32)
}

fn anchors() -> Vec<JointAnchor> {
    vec![
        JointAnchor::new(0, verts(7..=7, all)),
        JointAnchor::new(1, verts(9..=9, all)),
        JointAnchor::new(2, verts(12..=12, all)),
        JointAnchor::new(3, verts(14..=14, all)),
        JointAnchor::new(4, verts(15..=15, all)),
        JointAnchor::new(5, verts(5..=5, is_left)),
        JointAnchor::new(6, verts(3..=3, is_left)),
        JointAnchor::new(7, verts(5..=5, is_right)),
        JointAnchor::new(8, verts(3..=3, is_right)),
    ]
}

fn skin_influences() -> Vec<Vec<(usize, f32)>> {
    let chain_rings = [7usize, 9, 12, 14, 15];
    let mut out = Vec::new();
    for r in 0..R {
        for k in 0..K {
            let mut w: Vec<(usize, f32)> = Vec::new();
            if r >= 7 {
                let top = chain_rings.len() - 1;
                if r >= chain_rings[top] {
                    w.push((top, 1.0));
                } else {
                    let i = (0..top).rev().find(|&i| chain_rings[i] <= r).unwrap();
                    let (a, b) = (chain_rings[i], chain_rings[i + 1]);
                    let t = (r - a) as f32 / (b - a) as f32;
                    if t == 0.0 {
                        w.push((i, 1.0));
                    } else {
                        w.push((i, 1.0 - t));
                        w.push((i + 1, t));
                    }
                }
            } else {
                // (pelvis, thigh, calf) weights by ring.
                let (wp, wt, wc) = match r {
                    6 => (0.5, 0.5, 0.0),
                    5 => (0.0, 1.0, 0.0),
                    4 => (0.0, 0.5, 0.5),
                    _ => (0.0, 0.0, 1.0),
                };
                let sides: Vec<(usize, usize, f32)> = if k == 4 || k == 12 {
                    vec![(5, 6, 0.5), (7, 8, 0.5)]
                } else if is_left(k) {
                    vec![(5, 6, 1.0)]
                } else {
                    vec![(7, 8, 1.0)]
                };
                if wp > 0.0 {
                    w.push((0, wp));
                }
                for (t, c, f) in sides {
                    if wt > 0.0 {
                        w.push((t, wt * f));
                    }
                    if wc > 0.0 {
                        w.push((c, wc * f));
                    }
                }
            }
            out.push(w);
        }
    }
    out
}

fn rig_spec(fam: &BodyFamily) -> RigSpec {
    let o = fam.origin();
    let parents = [
        None,
        Some(0),
        Some(1),
        Some(2),
        Some(3),
        Some(0),
        Some(5),
        Some(0),
        Some(7),
    ];
    let names = [
        "pelvis", "spine", "chest", "neck", "head", "thigh_l", "calf_l", "thigh_r", "calf_r",
    ];
    let anc = anchors();
    let world: Vec<Vec3> = anc.iter().map(|a| centroid(o, &a.vertices)).collect();
    let joints: Vec<Joint> = (0..9)
        .map(|i| {
            let t = match parents[i] {
                Some(p) => world[i].sub(world[p]),
                None => world[i],
            };
            Joint::new(
                names[i],
                parents[i],
                Transform::new(t, Quat::IDENTITY, Vec3::new(1.0, 1.0, 1.0)),
            )
        })
        .collect();
    let skel = Skeleton::new(joints, vec![]).unwrap();
    let skin = SkinWeights::new(&skin_influences(), 9).unwrap();
    RigSpec::new(
        skel,
        anc,
        skin,
        // One compliant (soft-tissue) tag per required control region.
        REQUIRED_CONTROLS
            .iter()
            .map(|id| (format!("tag_{id}"), (*id).to_string()))
            .collect(),
    )
}

fn bits(v: Vec3) -> [u32; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn close(a: Vec3, b: Vec3, eps: f32) -> bool {
    a.sub(b).length() <= eps
}

fn moved(fam: &BodyFamily, body: &ShapedBody) -> Vec<u32> {
    (0..fam.vertex_count() as u32)
        .filter(|&v| bits(body.positions()[v as usize]) != bits(fam.origin()[v as usize]))
        .collect()
}

/// Everything derived that a consumer reads, as numbers, excluding the body-revision stamp
/// (which differs for any edit by construction), so "dependents changed" is about content.
fn derived(fam: &BodyFamily, body: &ShapedBody, spec: &RigSpec) -> Vec<f32> {
    let rig = fam.rebind(body, spec).unwrap();
    let mut s = Vec::new();
    let v3 = |s: &mut Vec<f32>, v: Vec3| s.extend([v.x, v.y, v.z]);
    for j in rig.skeleton_for(body).unwrap().joints() {
        v3(&mut s, j.rest.translation);
    }
    for m in rig.inverse_binds_for(body).unwrap() {
        s.extend(m.cols.iter().flatten().copied());
    }
    for c in rig.colliders_for(body).unwrap() {
        v3(&mut s, c.a);
        v3(&mut s, c.b);
        s.push(c.radius);
    }
    for t in rig.compliant_for(body).unwrap() {
        v3(&mut s, t.centroid);
        v3(&mut s, t.local_centroid);
        v3(&mut s, t.half_extent);
        s.push(t.radius);
    }
    s
}

fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

// ---- tests ----------------------------------------------------------------------------------

#[test]
fn required_control_schema_is_complete_and_detects_a_missing_control() {
    let fam = family();
    assert_eq!(REQUIRED_CONTROLS.len(), 18);
    assert!(fam.check_required_controls().is_ok());
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    spec.controls
        .retain(|c| c.id != "calf" && c.id != "posture");
    let thin = BodyFamily::new(spec).unwrap();
    assert_eq!(
        thin.check_required_controls().unwrap_err(),
        vec!["calf", "posture"]
    );
}

#[test]
fn every_required_control_moves_geometry_inside_its_region_and_never_in_a_lock() {
    let fam = family();
    for id in REQUIRED_CONTROLS {
        let region = fam.region(id).unwrap();
        for v in [1.0f32, -1.0, 0.5, -0.5] {
            let body = fam.evaluate(&rv(&[(id, v)])).unwrap();
            let m = moved(&fam, &body);
            assert!(!m.is_empty(), "{id}={v} moved nothing");
            for &x in &m {
                assert!(
                    region.binary_search(&x).is_ok(),
                    "{id} moved {x} outside region"
                );
                assert!(!fam.is_locked(x), "{id} moved locked vertex {x}");
            }
        }
        // The positive and negative sides are different shapes (not one delta reused).
        let p = fam.evaluate(&rv(&[(id, 1.0)])).unwrap();
        let n = fam.evaluate(&rv(&[(id, -1.0)])).unwrap();
        assert_ne!(p.rev_id(), n.rev_id(), "{id}: pos and neg identical");
        // Linear in |c|: half value gives half displacement.
        let h = fam.evaluate(&rv(&[(id, 0.5)])).unwrap();
        for &x in &moved(&fam, &p) {
            let d1 = p.positions()[x as usize].sub(fam.origin()[x as usize]);
            let dh = h.positions()[x as usize].sub(fam.origin()[x as usize]);
            assert!(close(d1.scale(0.5), dh, 1e-6), "{id} not linear at {x}");
        }
    }
}

#[test]
fn dependents_joints_proxies_and_tags_update_for_every_required_control() {
    let fam = family();
    let spec = rig_spec(&fam);
    let base = derived(&fam, &fam.evaluate(&[]).unwrap(), &spec);
    for id in REQUIRED_CONTROLS {
        for v in [1.0f32, -1.0] {
            let body = fam.evaluate(&rv(&[(id, v)])).unwrap();
            let d = max_abs_diff(&derived(&fam, &body, &spec), &base);
            assert!(
                d > 1.0e-3,
                "{id}={v}: joints, inverse binds, proxies and tags barely moved ({d})"
            );
        }
    }
}

#[test]
fn specific_dependents_follow_the_surface() {
    let fam = family();
    let spec = rig_spec(&fam);
    let b0 = fam.evaluate(&[]).unwrap();
    let r0 = fam.rebind(&b0, &spec).unwrap();
    let pos = |rig: &gibson::figure::family::BoundRig, body: &ShapedBody, j: usize| {
        let w = rig.skeleton_for(body).unwrap().world_bind();
        w[j].transform_point(Vec3::ZERO)
    };

    // Neutral rebind reproduces the declared skeleton (anchors were taken from the origin).
    for j in 0..9 {
        let declared = spec.skeleton.world_bind()[j].transform_point(Vec3::ZERO);
        assert!(close(pos(&r0, &b0, j), declared, 1e-5), "joint {j}");
    }

    // `pelvis` -0.02 z on the pelvis ring: the pelvis joint drops by exactly that.
    let b1 = fam.evaluate(&rv(&[("pelvis", 1.0)])).unwrap();
    let r1 = fam.rebind(&b1, &spec).unwrap();
    let d = pos(&r1, &b1, PELVIS).sub(pos(&r0, &b0, PELVIS));
    assert!(close(d, Vec3::new(0.0, 0.0, -0.02), 1e-5), "{d:?}");
    // ... and the unanchored-by-this-control thigh joint stays in world space, so its LOCAL
    // translation (parent frame) changed.
    assert!(close(pos(&r1, &b1, THIGH_L), pos(&r0, &b0, THIGH_L), 1e-5));
    let lt = |r: &gibson::figure::family::BoundRig, b: &ShapedBody| {
        r.skeleton_for(b).unwrap().joints()[THIGH_L]
            .rest
            .translation
    };
    assert!(!close(lt(&r0, &b0), lt(&r1, &b1), 1e-4));

    // `posture` leans the spine and chest forward (+y): their joints move, in the shaped frame.
    let b2 = fam.evaluate(&rv(&[("posture", 1.0)])).unwrap();
    let r2 = fam.rebind(&b2, &spec).unwrap();
    assert!(pos(&r2, &b2, SPINE).y > pos(&r0, &b0, SPINE).y + 0.01);
    assert!(pos(&r2, &b2, CHEST).y > pos(&r0, &b0, CHEST).y + 0.03);

    // `hip_flare` widens the hips: the pelvis capsule grows, the neutral one is smaller.
    let b3 = fam.evaluate(&rv(&[("hip_flare", 1.0)])).unwrap();
    let r3 = fam.rebind(&b3, &spec).unwrap();
    let rad = |r: &gibson::figure::family::BoundRig, b: &ShapedBody, j: usize| {
        r.colliders_for(b)
            .unwrap()
            .iter()
            .find(|c| c.joint == j)
            .unwrap()
            .radius
    };
    assert!(rad(&r3, &b3, PELVIS) > rad(&r0, &b0, PELVIS) + 0.01);

    // Bust projection moves the bust tag forward by the mean of its deltas, in world space and
    // consistently in the dominant joint's bind frame.
    let b4 = fam.evaluate(&rv(&[("bust_projection", 1.0)])).unwrap();
    let r4 = fam.rebind(&b4, &spec).unwrap();
    let tag = |r: &gibson::figure::family::BoundRig, b: &ShapedBody| {
        r.compliant_for(b)
            .unwrap()
            .iter()
            .find(|t| t.name == "tag_bust_volume")
            .unwrap()
            .clone()
    };
    let (t0, t4) = (tag(&r0, &b0), tag(&r4, &b4));
    assert!(t4.centroid.y > t0.centroid.y + 0.001);
    let ibs = r4.inverse_binds_for(&b4).unwrap();
    assert!(close(
        ibs[t4.joint].transform_point(t4.centroid),
        t4.local_centroid,
        1e-6
    ));

    // The skin weights are carried over untouched.
    assert_eq!(r4.skin_for(&b4).unwrap(), &spec.skin);
    // Capsules enclose every dominantly bound vertex.
    for c in r4.colliders_for(&b4).unwrap() {
        assert!(c.radius.is_finite() && c.radius > 0.0 && c.bound_vertices > 0);
    }
}

#[test]
fn bind_pose_reproduces_the_shaped_mesh_and_pivots_follow_the_rebound_joint() {
    let fam = family();
    let spec = rig_spec(&fam);
    let body = fam
        .evaluate(&rv(&[("posture", 1.0), ("ribcage_depth", 0.7)]))
        .unwrap();
    let rig = fam.rebind(&body, &spec).unwrap();
    let skel = rig.skeleton_for(&body).unwrap();

    // Rest pose: identity skinning.
    let world = skel.world_bind();
    let at_rest = rig.skin_positions(&body, &world).unwrap();
    for (a, b) in at_rest.iter().zip(body.positions()) {
        assert!(close(*a, *b, 1e-5));
    }

    // Rotate the chest about its REBOUND position: ring 12 (weight 1 on chest) rotates about it.
    let rot = Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), 0.5);
    let mut locals = skel.rest_locals();
    locals[CHEST].rotation = rot;
    let posed_world = skel.world_from_locals(&locals).unwrap();
    let c = world[CHEST].transform_point(Vec3::ZERO);
    let rm = rot.to_mat();
    let posed = rig.skin_positions(&body, &posed_world).unwrap();
    for v in verts(12..=12, all) {
        let p = body.positions()[v as usize];
        let want = c.add(rm.transform_point(p.sub(c)));
        assert!(close(posed[v as usize], want, 1e-5), "vertex {v}");
    }

    // What a STALE rig would do: bind from the neutral body, apply to the shaped mesh. It is
    // observably wrong (pivot in the wrong place), which is why `skin_positions` refuses it.
    let b0 = fam.evaluate(&[]).unwrap();
    let r0 = fam.rebind(&b0, &spec).unwrap();
    let sk0 = r0.skeleton_for(&b0).unwrap();
    let mut l0 = sk0.rest_locals();
    l0[CHEST].rotation = rot;
    let w0 = sk0.world_from_locals(&l0).unwrap();
    let wrong = spec
        .skin
        .skin(body.positions(), &w0, r0.inverse_binds_for(&b0).unwrap())
        .unwrap();
    let worst = verts(12..=12, all)
        .iter()
        .map(|&v| wrong[v as usize].sub(posed[v as usize]).length())
        .fold(0.0f32, f32::max);
    assert!(
        worst > 5e-3,
        "stale binds should be visibly wrong, worst {worst}"
    );
}

#[test]
fn stale_skeleton_is_refused_everywhere_it_could_leak() {
    let fam = family();
    let spec = rig_spec(&fam);
    let b0 = fam.evaluate(&[]).unwrap();
    let r0 = fam.rebind(&b0, &spec).unwrap();
    let b1 = fam.evaluate(&rv(&[("bust_volume", 0.8)])).unwrap();
    assert_ne!(b0.rev_id(), b1.rev_id());

    // Current is accepted.
    assert!(r0.check_current(&b0).is_ok());
    // Stale is refused by every guarded accessor, with both revisions named.
    let want = RebindError::StaleSkeleton {
        bound_to: b0.rev_id(),
        got: b1.rev_id(),
    };
    assert_eq!(r0.check_current(&b1), Err(want.clone()));
    assert_eq!(r0.skeleton_for(&b1).unwrap_err(), want);
    assert_eq!(r0.inverse_binds_for(&b1).unwrap_err(), want);
    assert_eq!(r0.colliders_for(&b1).unwrap_err(), want);
    assert_eq!(r0.compliant_for(&b1).unwrap_err(), want);
    assert_eq!(r0.skin_for(&b1).unwrap_err(), want);
    let world = spec.skeleton.world_bind();
    assert_eq!(r0.skin_positions(&b1, &world).unwrap_err(), want);
    assert_eq!(r0.verify(&fam, &b1, &spec).unwrap_err(), want);

    // A fresh bind for the new revision is accepted and verifies against a fresh derivation.
    let r1 = fam.rebind(&b1, &spec).unwrap();
    assert!(r1.check_current(&b1).is_ok());
    assert!(r1.verify(&fam, &b1, &spec).is_ok());
    assert_ne!(r0.id(), r1.id());

    // Same rig contents, different spec (an anchor changed): verify catches the difference.
    let mut other = spec.clone();
    other.anchors[CHEST] = JointAnchor::new(CHEST, verts(11..=11, all));
    assert!(matches!(
        r1.verify(&fam, &b1, &other),
        Err(RebindError::Tampered { .. })
    ));
}

#[test]
fn no_op_edit_yields_the_same_revision_and_the_same_rig() {
    let fam = family();
    let spec = rig_spec(&fam);
    let a = fam.evaluate(&[]).unwrap();
    // Explicit defaults, explicit zeros, and a value set then reverted are all the same edit.
    let zeros: Vec<(String, f32)> = fam
        .spec()
        .controls
        .iter()
        .map(|c| (c.id.clone(), 0.0))
        .collect();
    let b = fam.evaluate(&zeros).unwrap();
    let c = fam.evaluate(&rv(&[("bust_volume", 0.0)])).unwrap();
    assert_eq!(a.rev_id(), b.rev_id());
    assert_eq!(a.rev_id(), c.rev_id());
    assert_eq!(a.positions(), b.positions());
    assert_eq!(
        fam.rebind(&a, &spec).unwrap().id(),
        fam.rebind(&b, &spec).unwrap().id()
    );
    // Recipe order never matters.
    let x = fam
        .evaluate(&rv(&[("calf", 0.3), ("posture", -0.4), ("pelvis", 0.2)]))
        .unwrap();
    let y = fam
        .evaluate(&rv(&[("pelvis", 0.2), ("calf", 0.3), ("posture", -0.4)]))
        .unwrap();
    assert_eq!(x.rev_id(), y.rev_id());
    // A real edit is a different revision, and the smallest real edit still is.
    assert_ne!(a.rev_id(), x.rev_id());
    let tiny = fam.evaluate(&rv(&[("calf", 1.0e-4)])).unwrap();
    assert_ne!(a.rev_id(), tiny.rev_id());
    // Evaluation is deterministic.
    let x2 = fam
        .evaluate(&rv(&[("calf", 0.3), ("posture", -0.4), ("pelvis", 0.2)]))
        .unwrap();
    assert_eq!(x, x2);
}

#[test]
fn locked_regions_are_bit_identical_under_a_sweep_of_recipes() {
    let fam = family();
    let mut seed = 0x9e37_79b9_u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    };
    for _ in 0..64 {
        let vals: Vec<(String, f32)> = fam
            .spec()
            .controls
            .iter()
            .map(|c| (c.id.clone(), next()))
            .collect();
        let body = fam.evaluate(&vals).unwrap();
        for v in 0..fam.vertex_count() as u32 {
            if fam.is_locked(v) {
                assert_eq!(
                    bits(body.positions()[v as usize]),
                    bits(fam.origin()[v as usize]),
                    "locked vertex {v} changed"
                );
            }
        }
    }
    // Sanity: the sweep did move free geometry.
    let any = fam.evaluate(&rv(&[("posture", 1.0)])).unwrap();
    assert!(!moved(&fam, &any).is_empty());
}

#[test]
fn a_control_that_touches_a_locked_vertex_is_refused() {
    // `shoulder`'s region is widened to include ring 14 (neck, locked) and a delta lands there.
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    let c = spec
        .controls
        .iter_mut()
        .find(|c| c.id == "shoulder")
        .unwrap();
    let v = vid(14, 3);
    c.pos.push((v, Vec3::new(0.01, 0.0, 0.0)));
    c.neg.push((v, Vec3::new(-0.01, 0.0, 0.0)));
    let r = spec
        .regions
        .iter_mut()
        .find(|r| r.name == "shoulder")
        .unwrap();
    r.vertices.push(v);
    match BodyFamily::new(spec) {
        Err(FamilyError::LockedVertexTouched {
            owner,
            vertex,
            region,
        }) => {
            assert_eq!(owner, "shoulder");
            assert_eq!(vertex, v);
            assert_eq!(region, "head_locked");
        }
        other => panic!("expected LockedVertexTouched, got {other:?}"),
    }
}

#[test]
fn a_morph_outside_its_declared_region_is_flagged() {
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    // `waist_width` (rings 8-9) also nudges a bust vertex: right idea, wrong place.
    let c = spec
        .controls
        .iter_mut()
        .find(|c| c.id == "waist_width")
        .unwrap();
    let v = vid(11, 2);
    c.pos.push((v, Vec3::new(0.01, 0.0, 0.0)));
    c.neg.push((v, Vec3::new(-0.01, 0.0, 0.0)));
    assert_eq!(
        BodyFamily::new(spec).unwrap_err(),
        FamilyError::WrongRegion {
            owner: "waist_width".to_string(),
            vertex: v
        }
    );
}

#[test]
fn ignored_parameters_are_flagged() {
    // Zero deltas on both sides.
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    let c = spec.controls.iter_mut().find(|c| c.id == "calf").unwrap();
    for (_, d) in c.pos.iter_mut().chain(c.neg.iter_mut()) {
        *d = Vec3::ZERO;
    }
    assert_eq!(
        BodyFamily::new(spec).unwrap_err(),
        FamilyError::IgnoredControl {
            control: "calf".to_string()
        }
    );
    // No deltas at all.
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    let c = spec.controls.iter_mut().find(|c| c.id == "calf").unwrap();
    c.pos.clear();
    c.neg.clear();
    assert!(matches!(
        BodyFamily::new(spec),
        Err(FamilyError::IgnoredControl { .. })
    ));
    // The range reaches the negative side but only the positive side does anything.
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    let c = spec.controls.iter_mut().find(|c| c.id == "calf").unwrap();
    c.neg.clear();
    assert_eq!(
        BodyFamily::new(spec).unwrap_err(),
        FamilyError::IgnoredSide {
            control: "calf".to_string(),
            side: Side::Neg
        }
    );
    // Narrowing the range to the live side is a legal one-sided control.
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    let c = spec.controls.iter_mut().find(|c| c.id == "calf").unwrap();
    c.neg.clear();
    c.min = 0.0;
    assert!(BodyFamily::new(spec).is_ok());
}

#[test]
fn hostile_authoring_data_gets_typed_errors() {
    let base = || spec_with("synthetic_tube", 1, 1.0);
    let mut s = base();
    s.origin[3] = Vec3::new(f32::NAN, 0.0, 0.0);
    assert_eq!(
        BodyFamily::new(s).unwrap_err(),
        FamilyError::NonFiniteOrigin { vertex: 3 }
    );
    let mut s = base();
    s.origin.clear();
    s.indices.clear();
    assert_eq!(BodyFamily::new(s).unwrap_err(), FamilyError::EmptyOrigin);
    let mut s = base();
    s.indices.push(0);
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::BadTopology { .. })
    ));
    let mut s = base();
    s.indices[0] = 9999;
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::BadTopology { .. })
    ));
    let mut s = base();
    let dup = s.controls[0].clone();
    s.controls.push(dup);
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::DuplicateName {
            kind: "control",
            ..
        })
    ));
    let mut s = base();
    s.controls[0].region = "nowhere".into();
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::UnknownRegion { .. })
    ));
    let mut s = base();
    s.locked.push("nowhere".into());
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::UnknownRegion { .. })
    ));
    let mut s = base();
    let first = s.controls[0].pos[0];
    s.controls[0].pos.push(first);
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::BadDeltas { .. })
    ));
    let mut s = base();
    s.controls[0].default = 5.0;
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::ControlDomain { .. })
    ));
    let mut s = base();
    s.controls[0].pos[0].1 = Vec3::new(f32::INFINITY, 0.0, 0.0);
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::BadDeltas { .. })
    ));
}

#[test]
fn recipe_values_are_validated() {
    let fam = family();
    assert_eq!(
        fam.evaluate(&rv(&[("nope", 0.1)])).unwrap_err(),
        EvalError::UnknownControl {
            control: "nope".into()
        }
    );
    assert_eq!(
        fam.evaluate(&rv(&[("calf", 0.1), ("calf", 0.2)]))
            .unwrap_err(),
        EvalError::DuplicateControl {
            control: "calf".into()
        }
    );
    assert!(matches!(
        fam.evaluate(&rv(&[("calf", f32::NAN)])),
        Err(EvalError::NonFinite { .. })
    ));
    assert!(matches!(
        fam.evaluate(&rv(&[("calf", f32::INFINITY)])),
        Err(EvalError::NonFinite { .. })
    ));
    // Out of range is refused, never clamped.
    assert_eq!(
        fam.evaluate(&rv(&[("calf", 1.5)])).unwrap_err(),
        EvalError::OutOfRange {
            control: "calf".into(),
            value: 1.5,
            min: -1.0,
            max: 1.0
        }
    );
    assert!(matches!(
        fam.evaluate(&rv(&[("calf", -1.0001)])),
        Err(EvalError::OutOfRange { .. })
    ));
    // The range ends are legal.
    assert!(fam
        .evaluate(&rv(&[("calf", 1.0), ("posture", -1.0)]))
        .is_ok());
}

#[test]
fn topology_mismatch_is_refused_at_rebind() {
    let fam = family();
    let body = fam.evaluate(&[]).unwrap();
    let spec = rig_spec(&fam);
    // Skin for a different vertex count.
    let mut short = skin_influences();
    short.truncate(R * K - 1);
    let bad = RigSpec::new(
        spec.skeleton.clone(),
        spec.anchors.clone(),
        SkinWeights::new(&short, 9).unwrap(),
        vec![],
    );
    assert_eq!(
        fam.rebind(&body, &bad).unwrap_err(),
        RebindError::TopologyMismatch {
            skin_vertices: R * K - 1,
            vertices: R * K
        }
    );
    // Anchors that do not exist.
    let mut s = spec.clone();
    s.anchors[0] = JointAnchor::new(0, vec![(R * K) as u32]);
    assert!(matches!(
        fam.rebind(&body, &s),
        Err(RebindError::BadAnchorVertex { .. })
    ));
    let mut s = spec.clone();
    s.anchors.push(JointAnchor::new(0, vec![1]));
    assert!(matches!(
        fam.rebind(&body, &s),
        Err(RebindError::BadAnchor { joint: 0 })
    ));
    let mut s = spec.clone();
    s.compliant.push(("x".into(), "no_such_region".into()));
    assert!(matches!(
        fam.rebind(&body, &s),
        Err(RebindError::BadCompliant { .. })
    ));
}

#[test]
fn a_body_from_another_family_is_refused() {
    let fam = family();
    let other = BodyFamily::new(spec_with("synthetic_tube", 1, 1.05)).unwrap();
    let foreign = other.evaluate(&[]).unwrap();
    let spec = rig_spec(&fam);
    assert!(matches!(
        fam.rebind(&foreign, &spec),
        Err(RebindError::ForeignBody { .. })
    ));
}

#[test]
fn distinct_authored_origins_have_distinct_ids() {
    let a = family();
    let again = family();
    assert_eq!(a.id(), again.id(), "same authored data, same id");
    assert_eq!(a.origin_id(), again.origin_id());
    assert_eq!(a.id().to_hex().len(), 64);

    // Same controls and topology on a different origin (a different authored body): different
    // family id AND origin id, and the same recipe gives a different body revision.
    let b = BodyFamily::new(spec_with("synthetic_tube", 1, 1.05)).unwrap();
    assert_ne!(a.id(), b.id());
    assert_ne!(a.origin_id(), b.origin_id());
    let rcp = rv(&[("bust_volume", 0.5)]);
    assert_ne!(
        a.evaluate(&rcp).unwrap().rev_id(),
        b.evaluate(&rcp).unwrap().rev_id()
    );
    assert_ne!(
        a.evaluate(&[]).unwrap().rev_id(),
        b.evaluate(&[]).unwrap().rev_id()
    );

    // Name and revision are part of the id; the origin id is not.
    let renamed = BodyFamily::new(spec_with("other_name", 1, 1.0)).unwrap();
    let bumped = BodyFamily::new(spec_with("synthetic_tube", 2, 1.0)).unwrap();
    assert_ne!(a.id(), renamed.id());
    assert_ne!(a.id(), bumped.id());
    assert_eq!(a.origin_id(), renamed.origin_id());

    // A one-delta edit of a control changes the family id but not the origin id.
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    spec.controls[0].pos[0].1.x += 0.001;
    spec.controls[0].neg[0].1.x -= 0.001;
    let edited = BodyFamily::new(spec).unwrap();
    assert_ne!(a.id(), edited.id());
    assert_eq!(a.origin_id(), edited.origin_id());

    // A lock is part of the definition.
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    spec.locked.pop();
    assert_ne!(a.id(), BodyFamily::new(spec).unwrap().id());
}

#[test]
fn coupling_detects_a_control_that_is_the_same_shape_under_another_name() {
    let fam = family();
    // The four bust controls are distinct shapes.
    let bust = [
        "bust_volume",
        "bust_projection",
        "bust_base",
        "bust_fullness",
    ];
    for (i, a) in bust.iter().enumerate() {
        for b in &bust[i + 1..] {
            let c = fam.coupling(a, b).unwrap();
            assert!(c.abs() < 0.8, "{a} vs {b}: {c}");
        }
        assert!((fam.coupling(a, a).unwrap() - 1.0).abs() < 1e-5);
    }
    // A control in a disjoint region has no overlap at all.
    assert_eq!(fam.coupling("calf", "bust_volume").unwrap(), 0.0);
    // A clone under a new name is coupling 1.
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    let mut dup = spec
        .controls
        .iter()
        .find(|c| c.id == "calf")
        .unwrap()
        .clone();
    dup.id = "calf_again".into();
    dup.region = "calf".into();
    spec.controls.push(dup);
    let f2 = BodyFamily::new(spec).unwrap();
    assert!((f2.coupling("calf", "calf_again").unwrap() - 1.0).abs() < 1e-5);
    assert!(f2.coupling("calf", "missing").is_none());
}

#[test]
fn mirror_pairs_are_validated_and_evaluate_symmetrically() {
    let fam = family();
    assert_eq!(fam.mirror_partner("arm_l"), Some("arm_r"));
    let m = mirror_map();
    let body = fam
        .evaluate(&rv(&[("arm_l", 0.6), ("arm_r", 0.6)]))
        .unwrap();
    for v in moved(&fam, &body) {
        let mv = m[v as usize];
        let d = body.positions()[v as usize].sub(fam.origin()[v as usize]);
        let dm = body.positions()[mv as usize].sub(fam.origin()[mv as usize]);
        assert!(close(Vec3::new(-d.x, d.y, d.z), dm, 1e-6), "vertex {v}");
    }
    // One side alone moves only its own side.
    let left = fam.evaluate(&rv(&[("arm_l", 0.6)])).unwrap();
    assert!(moved(&fam, &left).iter().all(|&v| fam
        .region("arm_l")
        .unwrap()
        .binary_search(&v)
        .is_ok()));

    // A partner whose delta is not the mirror image is refused.
    let mut s = spec_with("synthetic_tube", 1, 1.0);
    let c = s.controls.iter_mut().find(|c| c.id == "arm_r").unwrap();
    c.pos[0].1.x += 0.01;
    assert_eq!(
        BodyFamily::new(s).unwrap_err(),
        FamilyError::MirrorMismatch {
            control: "arm_l".into()
        }
    );
    // Non-reciprocal pairing.
    let mut s = spec_with("synthetic_tube", 1, 1.0);
    s.controls
        .iter_mut()
        .find(|c| c.id == "arm_r")
        .unwrap()
        .mirror_partner = None;
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::BadMirrorPair { .. })
    ));
    // A pair with no mirror map.
    let mut s = spec_with("synthetic_tube", 1, 1.0);
    s.mirror.clear();
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::BadMirrorPair { .. })
    ));
    // A mirror map that is not an involution.
    let mut s = spec_with("synthetic_tube", 1, 1.0);
    s.mirror[0] = 5;
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::BadMirror { .. })
    ));
    // Partners with different ranges.
    let mut s = spec_with("synthetic_tube", 1, 1.0);
    s.controls.iter_mut().find(|c| c.id == "arm_r").unwrap().max = 0.9;
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::MirrorMismatch { .. })
    ));
}

#[test]
fn combo_correctives_fire_only_when_both_controls_are_active() {
    let base = family();
    let v = vid(11, 3); // a bust vertex, inside bust_volume's and bust_projection's regions
    let corr = Vec3::new(0.0, 0.0, 0.05);
    let mut spec = spec_with("synthetic_tube", 1, 1.0);
    spec.correctives.push(Corrective::new(
        "bust_volume",
        "bust_projection",
        vec![(v, corr)],
    ));
    let fam = BodyFamily::new(spec).unwrap();
    assert_ne!(base.id(), fam.id());

    // One control alone: the corrective is off, result equals the family without it.
    for r in [
        rv(&[("bust_volume", 0.8)]),
        rv(&[("bust_projection", -0.9)]),
    ] {
        assert_eq!(
            fam.evaluate(&r).unwrap().positions(),
            base.evaluate(&r).unwrap().positions()
        );
    }
    // Both: the difference is exactly |a|*|b|*delta, whatever the signs.
    for (a, b) in [(0.8f32, 0.5f32), (-0.8, 0.5), (0.8, -0.5)] {
        let r = rv(&[("bust_volume", a), ("bust_projection", b)]);
        let with = fam.evaluate(&r).unwrap();
        let without = base.evaluate(&r).unwrap();
        let d = with.positions()[v as usize].sub(without.positions()[v as usize]);
        assert!(
            close(d, corr.scale(a.abs() * b.abs()), 1e-6),
            "{a} {b}: {d:?}"
        );
        assert_eq!(moved_between(&with, &without), vec![v]);
    }

    // Authoring errors.
    let mk = |k: Corrective| {
        let mut s = spec_with("synthetic_tube", 1, 1.0);
        s.correctives.push(k);
        BodyFamily::new(s)
    };
    // Outside the union of the pair's regions.
    assert!(matches!(
        mk(Corrective::new(
            "bust_volume",
            "bust_projection",
            vec![(vid(3, 3), corr)]
        )),
        Err(FamilyError::WrongRegion { .. })
    ));
    assert!(matches!(
        mk(Corrective::new("bust_volume", "nope", vec![(v, corr)])),
        Err(FamilyError::UnknownControl { .. })
    ));
    assert!(matches!(
        mk(Corrective::new("calf", "calf", vec![(vid(1, 0), corr)])),
        Err(FamilyError::CorrectiveSameControl { .. })
    ));
    assert!(matches!(
        mk(Corrective::new(
            "bust_volume",
            "bust_projection",
            vec![(v, Vec3::ZERO)]
        )),
        Err(FamilyError::IgnoredCorrective { .. })
    ));
}

fn dist_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b.sub(a);
    let l2 = ab.dot(ab);
    let t = if l2 > 0.0 {
        (p.sub(a).dot(ab) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.sub(a.add(ab.scale(t))).length()
}

fn moved_between(a: &ShapedBody, b: &ShapedBody) -> Vec<u32> {
    (0..a.positions().len() as u32)
        .filter(|&v| bits(a.positions()[v as usize]) != bits(b.positions()[v as usize]))
        .collect()
}

#[test]
fn a_corrective_may_not_reach_into_a_lock() {
    // Widen a control region over a locked vertex, but put the delta in a corrective.
    let mut s = spec_with("synthetic_tube", 1, 1.0);
    s.correctives.push(Corrective::new(
        "shoulder",
        "upper_arm",
        vec![(vid(14, 2), Vec3::new(0.0, 0.0, 0.01))],
    ));
    // vid(14, 2) is in head_locked, so it is refused as a lock violation before region gating.
    assert!(matches!(
        BodyFamily::new(s),
        Err(FamilyError::LockedVertexTouched { .. })
    ));
}

#[test]
fn three_representative_edits_rebind_cleanly_and_stay_self_consistent() {
    let fam = family();
    let spec = rig_spec(&fam);
    let edits = [
        rv(&[
            ("bust_volume", 0.9),
            ("bust_projection", 0.5),
            ("bust_base", -0.3),
        ]),
        rv(&[
            ("hip_flare", 0.8),
            ("glute_projection", 0.7),
            ("glute_fullness", 0.5),
        ]),
        rv(&[("posture", -0.6), ("shoulder", 0.4), ("ribcage_width", 0.5)]),
    ];
    let mut rigs = vec![fam.rebind(&fam.evaluate(&[]).unwrap(), &spec).unwrap().id()];
    for e in &edits {
        let body = fam.evaluate(e).unwrap();
        let rig = fam.rebind(&body, &spec).unwrap();
        assert!(rig.verify(&fam, &body, &spec).is_ok());
        // Bind pose reproduces the mesh.
        let skel = rig.skeleton_for(&body).unwrap();
        let out = rig.skin_positions(&body, &skel.world_bind()).unwrap();
        for (a, b) in out.iter().zip(body.positions()) {
            assert!(close(*a, *b, 1e-5));
        }
        // Inverse binds really invert the shaped world binds.
        let w = skel.world_bind();
        for (wj, ib) in w.iter().zip(rig.inverse_binds_for(&body).unwrap()) {
            let id = wj.mul(ib);
            for c in 0..4 {
                for r in 0..4 {
                    let want = if c == r { 1.0 } else { 0.0 };
                    assert!((id.cols[c][r] - want).abs() < 1e-4);
                }
            }
        }
        // Each vertex lies inside the capsule of its dominant joint.
        let caps = rig.colliders_for(&body).unwrap();
        for v in 0..R * K {
            let inf = spec.skin.influences(v).unwrap();
            let mut best = inf[0];
            for &(j, w) in &inf {
                if w > best.1 {
                    best = (j, w);
                }
            }
            let cap = caps.iter().find(|c| c.joint == best.0).unwrap();
            let d = dist_to_segment(body.positions()[v], cap.a, cap.b);
            assert!(d <= cap.radius + 1e-5, "vertex {v}: {d} > {}", cap.radius);
        }
        // Rebind is deterministic.
        assert_eq!(rig.id(), fam.rebind(&body, &spec).unwrap().id());
        rigs.push(rig.id());
    }
    rigs.sort_by_key(|r| r.0);
    rigs.dedup();
    assert_eq!(rigs.len(), 4, "every edit has its own rig");
}
