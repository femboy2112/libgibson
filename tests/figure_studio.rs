//! M5-C slice 4: `figure::studio` (the `rev.v1` coupled-deformation compiler), tested on the
//! SYNTHETIC fixture only (`tests/fixtures/figure_studio`, written by
//! `tools/figure_assets/convert_to_gibson.py fixture`; no MakeHuman bytes anywhere in the repo).
//!
//! Every expectation below is stated analytically in f64 from the fixture's design (its docstring
//! in the converter), never read back from the code under test, and there is no golden id in this
//! file. Evidence, each with a control that must fail:
//! - control effectiveness: each control moves its landmark measurement monotonically and by the
//!   designed amount; controls do not compensate each other; an out-of-domain value is applied
//!   (not clamped) and reported as a diagnostic;
//! - skeleton and surface consistency: every anchored joint sits on the f64 centroid of its
//!   SHAPED anchor group, at the designed world position, with bone lengths and inverse binds to
//!   match, and the rest orientation chain of the asset survives the refit;
//! - revision isolation: a garment, material, hair, attachment, style, provenance or motion-style
//!   edit leaves the neutral body id bit-identical; a material or style edit leaves the fit id
//!   identical; a body edit changes all three; the fit id and presentation id bytes are rebuilt by
//!   hand from the contract text;
//! - fit gating: a garment the body bulges through goes NeedsAdaptation then Rejected and is not
//!   drawn, and a verdict is never carried across a body edit; garments bound to the shaped body
//!   follow it (analytic positions, per-axis offset scales);
//! - determinism: compile twice, identical ids and bytes; recipe order does not matter;
//! - the pose path: a rigid root motion moves the body and every drawn garment exactly.
//!
//! - garment adaptation (opt-in): a garment the body penetrates is pushed to the clearance and
//!   Fitted, vertices outside the moved region and its 1-ring are bit-identical, the displacement
//!   stays under the cap or the adaptation is refused and the unadapted verdict stands, and the fit
//!   id covers the adaptation parameters;
//!
//! Mutation controls (`RUSTFLAGS='--cfg figure_mutant="..."'`): `ignore_skeleton_update_v1`,
//! `ignore_surface_update`, `stale_fit`, `stale_control`, `no_adapt`. None is inspected here. Predicted kill matrix:
//! `docs/evidence/m5c/KILL_MATRIX_STUDIO_PREDICTED.md`.
#![cfg(feature = "figure")]
#![allow(clippy::needless_range_loop)]

use std::path::Path;

use gibson::figure::math::{Quat, Transform, Vec3};
use gibson::figure::sha256;
use gibson::figure::skin::SkinningMode;
use gibson::figure::studio::{
    compile, AdaptationV1, AnchorV1, AssetError, AssetSet, AttachmentV1, CompileError, Compiled,
    Diagnostic, FitReason, FitStatus, GarmentV1, MaterialSlotV1, MeshRole, PoseError, ProvenanceV1,
    RecipeV1, TapeError, TapeFrame, TapeHeader, TapeReader, BODY_MAGIC, FIT_PREFIX, PRESENT_PREFIX,
};

const BODY: &[u8] = include_bytes!("fixtures/figure_studio/body.gfa");
const CAP: &[u8] = include_bytes!("fixtures/figure_studio/part.cap.gfa");
const PLUG: &[u8] = include_bytes!("fixtures/figure_studio/part.plug.gfa");
const SHELL: &[u8] = include_bytes!("fixtures/figure_studio/part.shell.gfa");
const TABARD: &[u8] = include_bytes!("fixtures/figure_studio/part.tabard.gfa");
const TUNIC: &[u8] = include_bytes!("fixtures/figure_studio/part.tunic.gfa");
const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/figure_studio");

fn assets() -> AssetSet {
    AssetSet::from_bytes(BODY, &[CAP, PLUG, SHELL, TABARD, TUNIC]).expect("the fixture loads")
}

fn recipe(controls: &[(&str, f32)]) -> RecipeV1 {
    let mut r = RecipeV1::new();
    for (name, v) in controls {
        r.set_control(name, *v);
    }
    r
}

fn with_garments(mut r: RecipeV1, ids: &[&str]) -> RecipeV1 {
    for id in ids {
        r.garments.push(GarmentV1::new(*id));
    }
    r
}

fn build(r: &RecipeV1) -> Compiled {
    compile(r, &assets()).expect("compiles")
}

fn near(a: f32, b: f64, tol: f64) -> bool {
    (f64::from(a) - b).abs() <= tol
}

fn assert_near(label: &str, a: f32, b: f64, tol: f64) {
    assert!(near(a, b, tol), "{label}: got {a}, want {b} (tol {tol})");
}

fn assert_near3(label: &str, a: Vec3, b: [f64; 3], tol: f64) {
    assert!(
        near(a.x, b[0], tol) && near(a.y, b[1], tol) && near(a.z, b[2], tol),
        "{label}: got ({}, {}, {}), want ({}, {}, {}) (tol {tol})",
        a.x,
        a.y,
        a.z,
        b[0],
        b[1],
        b[2]
    );
}

// ---- the fixture's designed responses (see the converter docstring), in f64 ----

/// Stature scales z: +0.25 z on the positive side, -0.20 z on the negative side.
fn f_stature(c: f64) -> f64 {
    1.0 + if c > 0.0 { 0.25 * c } else { 0.2 * c }
}

/// Hip breadth scales (x, y) of the hip rings by 1 + 0.5 c on both sides.
fn f_hip(c: f64) -> f64 {
    1.0 + 0.5 * c
}

/// Bulge scales (x, y) of ring 2: +0.2 c on the positive side, +0.1 c on the negative side.
fn f_bulge(c: f64) -> f64 {
    1.0 + if c > 0.0 { 0.2 * c } else { 0.1 * c }
}

type Measure = fn(&Compiled) -> f64;
type Expect = fn(f64) -> f64;

fn centroid(c: &Compiled, name: &str) -> Vec3 {
    c.landmark_centroid(name).expect("landmark exists")
}

fn m_height(c: &Compiled) -> f64 {
    f64::from(centroid(c, "head_top").z) - f64::from(centroid(c, "foot").z)
}

fn m_hip(c: &Compiled) -> f64 {
    f64::from(c.landmark_distance("hip_l", "hip_r").expect("landmarks"))
}

fn m_chest_depth(c: &Compiled) -> f64 {
    f64::from(
        c.landmark_distance("chest_front", "chest_back")
            .expect("landmarks"),
    )
}

fn m_lean(c: &Compiled) -> f64 {
    f64::from(centroid(c, "head_top").y) - f64::from(centroid(c, "foot").y)
}

fn e_height(v: f64) -> f64 {
    1.5 * f_stature(v)
}

fn e_hip(v: f64) -> f64 {
    0.4 * f_hip(v)
}

fn e_chest_depth(v: f64) -> f64 {
    0.32 * f_bulge(v)
}

fn e_lean(v: f64) -> f64 {
    0.15 * v
}

fn measures() -> Vec<(&'static str, Measure, Expect)> {
    vec![
        ("stature", m_height as Measure, e_height as Expect),
        ("hip_breadth", m_hip as Measure, e_hip as Expect),
        ("bulge", m_chest_depth as Measure, e_chest_depth as Expect),
        ("lean", m_lean as Measure, e_lean as Expect),
    ]
}

// ---- control effectiveness ----

/// Each control moves its own landmark measurement strictly monotonically, by the designed
/// amount (hip breadth `0.4 (1 + 0.5 c)`, and so on). Kills `ignore_surface_update` (nothing
/// moves). The sweep includes both sides of zero, where the two sides have different slopes.
#[test]
fn each_control_moves_its_measurement_monotonically_by_the_designed_amount() {
    let a = assets();
    let values = [-1.0f32, -0.5, 0.0, 0.5, 1.0];
    for (name, measure, expect) in measures() {
        let mut last: Option<f64> = None;
        for &v in &values {
            let c = compile(&recipe(&[(name, v)]), &a).expect("compiles");
            let got = measure(&c);
            let want = expect(f64::from(v));
            assert!(
                (got - want).abs() <= 2.0e-5,
                "{name} at {v}: measured {got}, designed {want}"
            );
            if let Some(prev) = last {
                assert!(
                    got > prev + 1.0e-4,
                    "{name}: measurement did not increase from {prev} to {got} at {v}"
                );
            }
            last = Some(got);
        }
    }
}

/// No downward compensation: a control at 1.0 leaves every OTHER control's measurement exactly at
/// its neutral value. Kills nothing by itself under the mutants (it holds with deltas ignored) but
/// pins the "no compensation" rule of the studio API.
#[test]
fn a_control_does_not_move_the_other_controls_measurements() {
    let a = assets();
    let neutral = compile(&recipe(&[]), &a).expect("compiles");
    for (moved, _, _) in measures() {
        let c = compile(&recipe(&[(moved, 1.0)]), &a).expect("compiles");
        for (other, measure, _) in measures() {
            if other == moved {
                continue;
            }
            let (got, base) = (measure(&c), measure(&neutral));
            assert!(
                (got - base).abs() <= 2.0e-5,
                "{moved} = 1 moved the {other} measurement: {base} -> {got}"
            );
        }
    }
}

/// A value outside the control's declared domain [-1, 1] is applied as given (not clamped) and
/// reported as a diagnostic; in-domain values give no diagnostic. Kills `ignore_surface_update`.
#[test]
fn an_out_of_domain_control_is_applied_unclamped_and_reported() {
    let a = assets();
    let c = compile(&recipe(&[("stature", 1.5)]), &a).expect("compiles");
    // 1.5 * (1 + 0.25 * 1.5) = 2.0625; a clamp to 1.0 would give 1.875
    assert_near("unclamped height", m_height(&c) as f32, 2.0625, 2.0e-5);
    assert!(
        c.diagnostics().iter().any(|d| matches!(
            d,
            Diagnostic::ControlOutOfDomain { id, value, min, max }
                if id == "stature" && *value == 1.5 && *min == -1.0 && *max == 1.0
        )),
        "missing ControlOutOfDomain diagnostic: {:?}",
        c.diagnostics()
    );
    let low = compile(&recipe(&[("stature", -2.0)]), &a).expect("compiles");
    // 1.5 * (1 - 0.2 * 2) = 0.9
    assert_near("unclamped low height", m_height(&low) as f32, 0.9, 2.0e-5);
    let ok = compile(&recipe(&[("stature", 1.0)]), &a).expect("compiles");
    assert!(
        !ok.diagnostics()
            .iter()
            .any(|d| matches!(d, Diagnostic::ControlOutOfDomain { .. })),
        "an in-domain value produced a domain diagnostic"
    );
    // the id of the out-of-domain body is not the id of the clamped body
    let clamped = compile(&recipe(&[("stature", 1.0)]), &a).expect("compiles");
    assert_ne!(c.body_rev_id(), clamped.body_rev_id());
}

/// Bad control input is a typed error, never a silent fix.
#[test]
fn bad_control_input_is_a_typed_error() {
    let a = assets();
    let mut r = RecipeV1::new();
    r.controls.push(("no_such_control".to_string(), 0.1));
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::UnknownControl { .. })
    ));
    let mut r = RecipeV1::new();
    r.controls.push(("stature".to_string(), 0.1));
    r.controls.push(("stature".to_string(), 0.2));
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::DuplicateControl { .. })
    ));
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let r = recipe(&[("stature", bad)]);
        assert!(matches!(
            compile(&r, &a),
            Err(CompileError::NonFinite { .. })
        ));
    }
    let mut r = RecipeV1::new();
    r.footwear.heel_height_m = f32::NAN;
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::NonFinite { .. })
    ));
    let mut r = RecipeV1::new();
    r.fit_params.tolerance_m = -0.1;
    assert!(matches!(compile(&r, &a), Err(CompileError::BadFitParams)));
    let mut r = RecipeV1::new();
    r.fit_params.adapt_depth_m = 0.0001; // below the tolerance
    assert!(matches!(compile(&r, &a), Err(CompileError::BadFitParams)));
}

/// Grounding: the whole figure is lifted so the lowest body vertex is at z = 0. The `sink`
/// control moves everything down by 0.05 |c|; after the lift the body is where it was.
#[test]
fn the_lowest_body_vertex_is_grounded() {
    let a = assets();
    let neutral = compile(&recipe(&[]), &a).expect("compiles");
    for v in [-1.0f32, 0.4, 1.0] {
        let c = compile(&recipe(&[("sink", v)]), &a).expect("compiles");
        assert_near("foot z", centroid(&c, "foot").z, 0.0, 1.0e-6);
        assert_near("lift", c.ground_lift_m(), 0.05 * f64::from(v), 1.0e-6);
        for i in 0..c.body_vertex_count() {
            let (p, q) = (c.shaped_positions()[i], neutral.shaped_positions()[i]);
            assert_near3(
                "sunk then lifted",
                p,
                [f64::from(q.x), f64::from(q.y), f64::from(q.z)],
                1.0e-6,
            );
        }
    }
    // the neutral body is already grounded
    assert_near("neutral lift", neutral.ground_lift_m(), 0.0, 1.0e-7);
}

// ---- skeleton and surface consistency ----

fn joint_world(c: &Compiled, j: usize) -> [f64; 3] {
    let m = c.skeleton().world_bind()[j];
    [
        f64::from(m.cols[3][0]),
        f64::from(m.cols[3][1]),
        f64::from(m.cols[3][2]),
    ]
}

/// The designed world head of joint `j` under stature `s` and lean `l`: heads are the centroids
/// of ±0.01 helper squares, which scale in z with the stature and shift in y with the lean.
fn designed_joint(j: usize, s: f64, l: f64) -> [f64; 3] {
    let heads = [
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.5],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.4],
        [0.25, 0.0, 1.25],
    ];
    let h = heads[j];
    [h[0], h[1] + 0.1 * l * h[2], h[2] * f_stature(s)]
}

/// Skeleton/surface consistency, three ways. (1) every joint's world bind position is the f64
/// centroid of its anchor group in the SHAPED positions (recomputed here from the compiled
/// surface and the asset's groups); (2) the same positions equal the DESIGNED ones (stature 0.6,
/// lean -0.4), which are independent of the compiler; (3) the inverse binds map each joint to the
/// origin and the bone lengths are the designed distances. Kills `ignore_skeleton_update_v1`
/// (the declared skeleton does not follow the shaped surface).
#[test]
fn refit_joints_are_the_anchor_centroids_of_the_shaped_mesh() {
    let a = assets();
    let (s, l) = (0.6f32, -0.4f32);
    let c = compile(
        &recipe(&[
            ("stature", s),
            ("lean", l),
            ("hip_breadth", 0.5),
            ("bulge", 0.3),
        ]),
        &a,
    )
    .expect("compiles");
    let pos = c.shaped_positions();
    for j in 0..a.joint_count() {
        let group = a.joint_anchor(j).expect("joint exists");
        assert!(!group.is_empty());
        let mut sum = [0.0f64; 3];
        for &v in group {
            let p = pos[v as usize];
            sum[0] += f64::from(p.x);
            sum[1] += f64::from(p.y);
            sum[2] += f64::from(p.z);
        }
        let n = group.len() as f64;
        let centroid = [sum[0] / n, sum[1] / n, sum[2] / n];
        let w = joint_world(&c, j);
        for k in 0..3 {
            assert!(
                (w[k] - centroid[k]).abs() <= 2.0e-5,
                "joint {j} axis {k}: world {} vs shaped centroid {}",
                w[k],
                centroid[k]
            );
            let designed = designed_joint(j, f64::from(s), f64::from(l));
            assert!(
                (w[k] - designed[k]).abs() <= 2.0e-5,
                "joint {j} axis {k}: world {} vs designed {}",
                w[k],
                designed[k]
            );
        }
        // the inverse bind maps the joint's own world position to the origin
        let ib = c.inverse_binds()[j];
        let o = ib.transform_point(Vec3::new(w[0] as f32, w[1] as f32, w[2] as f32));
        assert_near3("inverse bind at the joint", o, [0.0, 0.0, 0.0], 2.0e-5);
    }
    // bone lengths follow the surface: |R_parent^T (head - parent head)| = |head - parent head|
    for (j, parent) in [(1usize, 0usize), (2, 1), (3, 2), (4, 2)] {
        let (h, p) = (
            designed_joint(j, f64::from(s), f64::from(l)),
            designed_joint(parent, f64::from(s), f64::from(l)),
        );
        let want = ((h[0] - p[0]).powi(2) + (h[1] - p[1]).powi(2) + (h[2] - p[2]).powi(2)).sqrt();
        let got = c.skeleton().joints()[j].rest.translation.length();
        assert_near("bone length", got, want, 2.0e-5);
    }
}

/// Bone lengths are an OUTPUT of the surface (contract, coupled derivation step 4): the
/// root-to-pelvis and pelvis-to-spine bones are both `0.5 f_stature(c)` long (the pelvis and spine
/// helper squares are at z = 0.5 and 1.0 and both parents have an identity world rotation).
/// Kills `ignore_skeleton_update_v1` (the declared 0.5 would never change).
#[test]
fn bone_lengths_are_outputs_of_the_surface() {
    let a = assets();
    let mut last = 0.0f32;
    for v in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
        let c = compile(&recipe(&[("stature", v)]), &a).expect("compiles");
        let want = 0.5 * f_stature(f64::from(v));
        let j = c.skeleton().joints();
        assert_near(
            "root-pelvis bone",
            j[1].rest.translation.length(),
            want,
            2.0e-5,
        );
        assert_near(
            "pelvis-spine bone",
            j[2].rest.translation.length(),
            want,
            2.0e-5,
        );
        let len = j[1].rest.translation.length();
        assert!(
            len > last,
            "bone length did not grow with the stature at {v}"
        );
        last = len;
    }
}

/// The asset's rest orientation chain survives the refit: the spine's world rotation is 20 degrees
/// about x and the left arm's is 90 degrees about y, whatever the shape. (Orientation is not
/// re-derived in v1; it is carried.)
#[test]
fn the_rest_orientation_chain_survives_the_refit() {
    let a = assets();
    for controls in [vec![], vec![("stature", 0.7f32), ("lean", 0.5)]] {
        let c = compile(&recipe(&controls), &a).expect("compiles");
        let w = c.skeleton().world_bind();
        let (s20, c20) = (20.0f64.to_radians().sin(), 20.0f64.to_radians().cos());
        // spine (2): columns of Rx(20): x = (1,0,0), y = (0,c,s), z = (0,-s,c)
        assert_near("spine y.y", w[2].cols[1][1], c20, 2.0e-6);
        assert_near("spine y.z", w[2].cols[1][2], s20, 2.0e-6);
        assert_near("spine z.y", w[2].cols[2][1], -s20, 2.0e-6);
        // arm (4): columns of Ry(90): x = (0,0,-1), z = (1,0,0)
        assert_near("arm x.z", w[4].cols[0][2], -1.0, 2.0e-6);
        assert_near("arm z.x", w[4].cols[2][0], 1.0, 2.0e-6);
        // the neck inherits the spine's world rotation (its own rest world rotation is identity,
        // so the local one is the spine's inverse and the product is the identity)
        assert_near("neck x.x", w[3].cols[0][0], 1.0, 2.0e-6);
        assert_near("neck z.z", w[3].cols[2][2], 1.0, 2.0e-6);
    }
}

// ---- identities ----

fn base_recipe() -> RecipeV1 {
    let mut r = with_garments(
        recipe(&[("stature", 0.3), ("hip_breadth", -0.2), ("bulge", 0.1)]),
        &["tunic", "tabard"],
    );
    r.materials.push(MaterialSlotV1::new("base"));
    r.hair.style_id = Some("cap".to_string());
    r.hair.color = [0.1, 0.2, 0.3];
    r
}

fn ids(r: &RecipeV1) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let c = build(r);
    (
        c.body_rev_id().0.to_vec(),
        c.fit_id().0.to_vec(),
        c.present_id().0.to_vec(),
    )
}

/// Revision isolation, edit by edit. A garment edit (add, remove, reorder) or a fit-parameter
/// edit changes fit and presentation ids and never the body id; a material, slot binding, hair,
/// attachment or lower-face-cover edit changes only the presentation id; provenance and motion
/// style change nothing; a body edit (control, footwear, skinning mode) changes all three.
/// Kills `ignore_surface_update` (its control edit leaves the body id unchanged).
#[test]
fn revision_isolation() {
    let r0 = base_recipe();
    let (b0, f0, p0) = ids(&r0);

    // garment edits: body same, fit and presentation different
    let mut add = r0.clone();
    add.garments.push(GarmentV1::new("shell"));
    let mut remove = r0.clone();
    remove.garments.pop();
    let mut reorder = r0.clone();
    reorder.garments.reverse();
    let mut params = r0.clone();
    params.fit_params.tolerance_m = 0.003;
    for (what, r) in [
        ("add", add),
        ("remove", remove),
        ("reorder", reorder),
        ("fit params", params),
    ] {
        let (b, f, p) = ids(&r);
        assert_eq!(b, b0, "{what}: the body id moved");
        assert_ne!(f, f0, "{what}: the fit id did not move");
        assert_ne!(p, p0, "{what}: the presentation id did not move");
    }

    // appearance edits: body and fit same, presentation different
    let mut mat = r0.clone();
    mat.materials[0].albedo = [0.9, 0.1, 0.1];
    let mut mat_new = r0.clone();
    mat_new.materials.push(MaterialSlotV1::new("trim"));
    let mut slot = r0.clone();
    slot.garments[0]
        .slot_bindings
        .push(("body".to_string(), "panel".to_string()));
    let mut hair_color = r0.clone();
    hair_color.hair.color = [0.9, 0.9, 0.1];
    let mut hair_off = r0.clone();
    hair_off.hair.style_id = None;
    let mut att = r0.clone();
    att.attachments.push(AttachmentV1::new(
        "pin",
        AnchorV1::Landmark("head_top".to_string()),
    ));
    let mut face = r0.clone();
    face.lower_face_covered = true;
    let mut style = r0.clone();
    style.style_params.push(("skin_tone".to_string(), 0.4));
    for (what, r) in [
        ("material", mat),
        ("new material slot", mat_new),
        ("garment slot binding", slot),
        ("hair color", hair_color),
        ("hair removed", hair_off),
        ("attachment", att),
        ("lower face cover", face),
        ("style parameter", style),
    ] {
        let (b, f, p) = ids(&r);
        assert_eq!(b, b0, "{what}: the body id moved");
        assert_eq!(f, f0, "{what}: the fit id moved");
        assert_ne!(p, p0, "{what}: the presentation id did not move");
    }

    // provenance and motion style are in no id
    let mut prov = r0.clone();
    prov.provenance = Some(ProvenanceV1::new("consumer-key", [7u8; 32]));
    prov.motion_style.preset = "expressive".to_string();
    prov.motion_style.amplitude = 0.5;
    let (b, f, p) = ids(&prov);
    assert_eq!(
        (b, f, p),
        (b0.clone(), f0.clone(), p0.clone()),
        "provenance or motion style rewrote an id"
    );
    let c = build(&prov);
    assert_eq!(c.provenance().expect("echoed").key, "consumer-key");
    assert_eq!(c.motion_style().preset, "expressive");

    // body edits: all three move
    let mut ctl = r0.clone();
    ctl.set_control("stature", 0.31);
    let mut shoe = r0.clone();
    shoe.footwear.heel_height_m = 0.05;
    let mut mode = r0.clone();
    mode.skinning = SkinningMode::Linear;
    for (what, r) in [
        ("control", ctl),
        ("footwear", shoe),
        ("skinning mode", mode),
    ] {
        let (b, f, p) = ids(&r);
        assert_ne!(b, b0, "{what}: the body id did not move");
        assert_ne!(f, f0, "{what}: the fit id did not move");
        assert_ne!(p, p0, "{what}: the presentation id did not move");
    }

    // the three ids of one compile are pairwise distinct
    assert_ne!(b0, f0);
    assert_ne!(f0, p0);
    assert_ne!(b0, p0);
}

/// Two recipes that resolve to the same body give one id: an explicit default equals an omitted
/// control, and the recipe's control order does not matter (the family order does).
#[test]
fn resolved_content_not_recipe_text_is_identified() {
    let a = assets();
    let plain = compile(&recipe(&[]), &a).expect("compiles");
    let explicit = compile(&recipe(&[("stature", 0.0), ("lean", 0.0)]), &a).expect("compiles");
    assert_eq!(plain.body_rev_id(), explicit.body_rev_id());
    let ab = compile(&recipe(&[("stature", 0.4), ("lean", 0.2)]), &a).expect("compiles");
    let ba = compile(&recipe(&[("lean", 0.2), ("stature", 0.4)]), &a).expect("compiles");
    assert_eq!(ab.body_rev_id(), ba.body_rev_id());
    assert_eq!(ab.body_bytes(), ba.body_bytes());
    assert_ne!(plain.body_rev_id(), ab.body_rev_id());
}

fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

fn f32_at(b: &[u8], p: usize) -> f32 {
    f32::from_bits(u32_at(b, p))
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

/// The head of the canonical body bytes, read back by hand from the contract's layout: the
/// 21-byte prefix, the mode byte, the asset id and version, the 32-byte asset sha, the vertex
/// count and the first vertex; and the tail (the footwear floats). The digest of the bytes is the
/// body id.
#[test]
fn the_body_bytes_follow_the_contract_layout() {
    let a = assets();
    let mut r = recipe(&[("stature", 0.5)]);
    r.footwear.heel_height_m = 0.05;
    r.footwear.toe_pitch_rad = -0.25;
    let c = compile(&r, &a).expect("compiles");
    let b = c.body_bytes();
    assert_eq!(&b[..21], b"gibson.figure.rev.v1\0");
    assert_eq!(b[21], 1, "DualQuat is mode byte 1");
    assert_eq!(u32_at(b, 22), 16);
    assert_eq!(&b[26..42], b"synthetic-puppet");
    assert_eq!(u32_at(b, 42), 1, "asset version");
    assert_eq!(&b[46..78], &a.body_sha256()[..]);
    assert_eq!(u32_at(b, 78), 60, "all vertices, helpers included");
    // vertex 0 is (0.15, 0, 0): the stature does not move a vertex at z = 0
    assert_near("v0.x", f32_at(b, 82), 0.15, 1.0e-6);
    assert_near("v0.y", f32_at(b, 86), 0.0, 1.0e-6);
    assert_near("v0.z", f32_at(b, 90), 0.0, 1.0e-6);
    let n = b.len();
    assert_eq!(f32_at(b, n - 8), 0.05f32);
    assert_eq!(f32_at(b, n - 4), -0.25f32);
    assert_eq!(&sha256::digest(b), &c.body_rev_id().0);
    let mut lin = r.clone();
    lin.skinning = SkinningMode::Linear;
    let l = compile(&lin, &a).expect("compiles");
    assert_eq!(l.body_bytes()[21], 0, "Linear is mode byte 0");
    assert_eq!(l.skinning_mode(), SkinningMode::Linear);
    assert_eq!(c.skinning_mode(), SkinningMode::DualQuat);
}

/// The fit id and the presentation id, rebuilt by hand from the contract text. A missing field,
/// a different order or a different prefix would change the digest.
#[test]
fn the_fit_id_and_presentation_id_are_rebuilt_by_hand() {
    let a = assets();
    let mut r = with_garments(recipe(&[("hip_breadth", 0.3)]), &["tunic", "tabard"]);
    let c = compile(&r, &a).expect("compiles");

    let mut fit = Vec::new();
    fit.extend_from_slice(b"gibson.figure.fit.v1\0");
    assert_eq!(FIT_PREFIX, b"gibson.figure.fit.v1\0");
    fit.extend_from_slice(&c.body_rev_id().0);
    put_u32(&mut fit, 2);
    put_str(&mut fit, "tunic");
    fit.extend_from_slice(&sha256::digest(TUNIC));
    put_str(&mut fit, "tabard");
    fit.extend_from_slice(&sha256::digest(TABARD));
    put_str(
        &mut fit,
        "gibson.fit.vertex-normal-penetration.v2+layers.v1",
    );
    put_u32(&mut fit, 2);
    put_str(&mut fit, "tolerance_m");
    fit.extend_from_slice(&0.002f32.to_le_bytes());
    put_str(&mut fit, "adapt_depth_m");
    fit.extend_from_slice(&0.01f32.to_le_bytes());
    assert_eq!(sha256::digest(&fit), c.fit_id().0, "fit id");

    // presentation: one material, one garment slot binding, hair, one attachment, one style value
    let mut m = MaterialSlotV1::new("trim");
    m.material_id = "brass".to_string();
    m.texture_sha256 = [3u8; 32];
    m.albedo = [0.25, 0.5, 0.75];
    m.roughness = 0.125;
    m.metalness = 1.0;
    m.emissive = Some(([0.5, 0.25, 0.125], 2.0));
    r.materials.push(m);
    r.garments[0]
        .slot_bindings
        .push(("collar".to_string(), "trim".to_string()));
    r.hair.style_id = Some("cap".to_string());
    r.hair.color = [0.5, 0.25, 0.125];
    let mut att = AttachmentV1::new("pin", AnchorV1::Bone("neck".to_string()));
    att.emissive = Some([1.0, 0.5, 0.25]);
    r.attachments.push(att);
    r.style_params.push(("skin_tone".to_string(), 0.75));
    r.lower_face_covered = true;
    let c2 = compile(&r, &a).expect("compiles");
    assert_eq!(
        c2.fit_id(),
        c.fit_id(),
        "appearance must not move the fit id"
    );

    let mut p = Vec::new();
    p.extend_from_slice(b"gibson.figure.presentation.v1\0");
    assert_eq!(PRESENT_PREFIX, b"gibson.figure.presentation.v1\0");
    p.extend_from_slice(&c2.fit_id().0);
    put_u32(&mut p, 1);
    put_str(&mut p, "trim");
    put_str(&mut p, "brass");
    p.extend_from_slice(&[3u8; 32]);
    put_u32(&mut p, 10);
    for v in [0.25f32, 0.5, 0.75, 0.125, 1.0, 1.0, 0.5, 0.25, 0.125, 2.0] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    put_u32(&mut p, 2);
    put_str(&mut p, "tunic");
    put_u32(&mut p, 1);
    put_str(&mut p, "collar");
    put_str(&mut p, "trim");
    put_str(&mut p, "tabard");
    put_u32(&mut p, 0);
    p.push(1);
    put_str(&mut p, "cap");
    p.extend_from_slice(&sha256::digest(CAP));
    for v in [0.5f32, 0.25, 0.125] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    put_u32(&mut p, 1);
    put_str(&mut p, "pin");
    p.push(1);
    put_str(&mut p, "neck");
    p.push(1);
    for v in [1.0f32, 0.5, 0.25] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    put_u32(&mut p, 2);
    put_str(&mut p, "skin_tone");
    p.extend_from_slice(&0.75f32.to_le_bytes());
    put_str(&mut p, "lower_face_covered");
    p.extend_from_slice(&1.0f32.to_le_bytes());
    assert_eq!(sha256::digest(&p), c2.present_id().0, "presentation id");
}

/// Compiling the same recipe twice gives identical ids, bytes, surface and posed vertices.
#[test]
fn compile_is_deterministic() {
    let a = assets();
    let r = base_recipe();
    let c1 = compile(&r, &a).expect("compiles");
    let c2 = compile(&r, &a).expect("compiles");
    assert_eq!(c1.body_rev_id(), c2.body_rev_id());
    assert_eq!(c1.fit_id(), c2.fit_id());
    assert_eq!(c1.present_id(), c2.present_id());
    assert_eq!(c1.body_bytes(), c2.body_bytes());
    assert_eq!(c1.shaped_positions(), c2.shaped_positions());
    assert_eq!(c1.fit_status(), c2.fit_status());
    let f = c1.rest_frame();
    assert_eq!(c1.pose_vertices(&f), c2.pose_vertices(&f));
    // an independently loaded copy of the same asset gives the same ids
    let b = AssetSet::load_dir(Path::new(FIXTURE_DIR)).expect("loads from disk");
    let c3 = compile(&r, &b).expect("compiles");
    assert_eq!(c1.body_rev_id(), c3.body_rev_id());
    assert_eq!(c1.present_id(), c3.present_id());
}

// ---- garments: fit, gating, geometry ----

fn status_of(c: &Compiled, id: &str) -> FitStatus {
    c.fit_status()
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no verdict for {id}"))
        .status
        .clone()
}

fn drawn(c: &Compiled, id: &str) -> bool {
    c.drawable_garments().iter().any(|g| g.id == id)
}

fn depth_of(c: &Compiled, id: &str) -> f32 {
    c.fit_status()
        .iter()
        .find(|r| r.id == id)
        .expect("verdict")
        .metrics
        .max_depth_m
}

/// FitStatus gating. The `shell` garment is bound to fixed helpers, so it does not follow the
/// body; the `bulge` control pushes ring 2 out through it (gap 0.012 at rest, body radius
/// `0.16 (1 + 0.2 c)` against the shell's 0.172). Verdicts, by design: c = 0.2 Fitted (radius
/// 0.1664), c = 0.5 NeedsAdaptation (penetration 0.004 along a near-radial normal), c = 1.0
/// Rejected (0.02), and only Fitted garments are drawn. The `tunic` and `tabard`, bound to the
/// shaped body, stay Fitted and drawn throughout. Kills `stale_fit` (the neutral body is used, so
/// the shell stays Fitted) and `ignore_surface_update`.
#[test]
fn a_garment_the_body_bulges_through_is_not_drawn() {
    let a = assets();
    for (bulge, want) in [
        (-1.0f32, 0u8),
        (0.0, 0),
        (0.2, 0),
        (0.5, 1),
        (1.0, 2),
        (1.5, 2),
    ] {
        let r = with_garments(recipe(&[("bulge", bulge)]), &["tunic", "shell", "tabard"]);
        let c = compile(&r, &a).expect("compiles");
        let st = status_of(&c, "shell");
        let ok = match want {
            0 => st == FitStatus::Fitted,
            1 => matches!(
                st,
                FitStatus::NeedsAdaptation(FitReason::Penetration { .. })
            ),
            _ => matches!(st, FitStatus::Rejected(FitReason::Penetration { .. })),
        };
        assert!(ok, "shell at bulge {bulge}: {st:?}");
        assert_eq!(
            drawn(&c, "shell"),
            want == 0,
            "shell drawn at bulge {bulge}"
        );
        // the metric is the penetration along a near-radial normal: 0.9..=1.0 of the gap
        if want != 0 {
            let gap = 0.16 * (1.0 + 0.2 * f64::from(bulge)) - 0.172;
            let d = f64::from(depth_of(&c, "shell"));
            assert!(
                d <= gap + 1.0e-5 && d >= 0.9 * gap,
                "bulge {bulge}: depth {d} vs gap {gap}"
            );
        }
        for id in ["tunic", "tabard"] {
            assert_eq!(
                status_of(&c, id),
                FitStatus::Fitted,
                "{id} at bulge {bulge}"
            );
            assert!(drawn(&c, id), "{id} drawn at bulge {bulge}");
        }
        // the diagnostic names every garment that is not drawn
        let not_drawn = c
            .diagnostics()
            .iter()
            .filter(|d| matches!(d, Diagnostic::GarmentNotDrawn { id, .. } if id == "shell"))
            .count();
        assert_eq!(not_drawn, usize::from(want != 0));
        // every verdict is reported, drawn or not, in set order
        let order: Vec<&str> = c.fit_status().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(order, ["tunic", "shell", "tabard"]);
    }
}

/// A fit result is never carried across body ids: the same garment set on two different bodies
/// has two fit ids and (here) two different verdicts. Kills `stale_fit`.
#[test]
fn a_verdict_is_never_carried_across_a_body_edit() {
    let a = assets();
    let at = |b: f32| {
        compile(&with_garments(recipe(&[("bulge", b)]), &["shell"]), &a).expect("compiles")
    };
    let (flat, bulged) = (at(0.0), at(1.0));
    assert_ne!(flat.body_rev_id(), bulged.body_rev_id());
    assert_ne!(flat.fit_id(), bulged.fit_id());
    assert_eq!(status_of(&flat, "shell"), FitStatus::Fitted);
    assert!(matches!(
        status_of(&bulged, "shell"),
        FitStatus::Rejected(_)
    ));
    assert!(drawn(&flat, "shell") && !drawn(&bulged, "shell"));
}

/// Garments bound to the shaped body are re-fitted on every compile and follow it. The tabard's
/// vertex 0 is `ring1[0] + 0.05 * (sx, 0, 0)` with `sx` the hip breadth ratio (so
/// `0.25 (1 + 0.5 c)`), and vertex 2 is `ring1[2] + (0, 0.05, 0)` with sy a constant 1 (so
/// `y = 0.2 (1 + 0.5 c) + 0.05`: the offset scale is per axis). The tunic's vertex 0 is
/// `0.8 P16 + 0.1 P17 + 0.1 P23 + (0.05, 0, 0)` on ring 2 (`P = 0.16 (cos, sin)`), so
/// `0.1506274 f_bulge + 0.05`. The cap (hair) vertex 0 is `ring3[0] + (0, 0, 0.03 sz)` with sz the
/// stature ratio, so `z = 1.53 f_stature`. Kills `stale_fit` (the neutral body is used) and
/// `ignore_surface_update`.
#[test]
fn garments_follow_the_shaped_body_with_per_axis_offset_scales() {
    let a = assets();
    for c in [-1.0f32, 0.0, 0.5, 1.0] {
        let cc = f64::from(c);
        // hip breadth: the tabard
        let r = with_garments(recipe(&[("hip_breadth", c)]), &["tabard"]);
        let comp = compile(&r, &a).expect("compiles");
        let tab = comp
            .drawable_garments()
            .iter()
            .find(|g| g.id == "tabard")
            .expect("drawn");
        assert_near("tabard v0.x", tab.positions[0].x, 0.25 * f_hip(cc), 2.0e-5);
        assert_near("tabard v0.y", tab.positions[0].y, 0.0, 2.0e-5);
        assert_near(
            "tabard v2.y",
            tab.positions[2].y,
            0.2 * f_hip(cc) + 0.05,
            2.0e-5,
        );
        assert_near("tabard v2.z", tab.positions[2].z, 0.5, 2.0e-5);
        // bulge: the tunic
        let r = with_garments(recipe(&[("bulge", c)]), &["tunic"]);
        let comp = compile(&r, &a).expect("compiles");
        let tun = comp
            .drawable_garments()
            .iter()
            .find(|g| g.id == "tunic")
            .expect("drawn");
        let base = 0.8 * 0.16 + 0.2 * 0.16 * std::f64::consts::FRAC_1_SQRT_2;
        assert_near(
            "tunic v0.x",
            tun.positions[0].x,
            base * f_bulge(cc) + 0.05,
            2.0e-5,
        );
        assert_near("tunic v0.y", tun.positions[0].y, 0.0, 2.0e-5);
        assert_near("tunic v0.z", tun.positions[0].z, 1.0, 2.0e-5);
        // stature: the cap
        let mut r = recipe(&[("stature", c)]);
        r.hair.style_id = Some("cap".to_string());
        let comp = compile(&r, &a).expect("compiles");
        let cap = comp.hair().expect("hair fitted");
        assert_near("cap v0.x", cap.positions[0].x, 0.1, 2.0e-5);
        assert_near("cap v0.z", cap.positions[0].z, 1.53 * f_stature(cc), 2.0e-5);
    }
}

/// Delete sets (MakeHuman `delete_verts`). A body vertex a garment hides is masked out of that
/// garment's penetration measure (a garment vertex whose nearest body vertex is hidden is not
/// measured); a body vertex it does NOT hide still counts. The `shell` hides ring 0 only, so the
/// ring-2 bulge still rejects it; the `plug` is the same geometry hiding ring 2, so the same bulge
/// does not reject it. Kills `ignore_surface_update` and `stale_fit` (the bulge never reaches the
/// shell's measure).
#[test]
fn a_bulge_outside_the_delete_set_still_rejects_and_inside_it_does_not() {
    let a = assets();
    let r = with_garments(recipe(&[("bulge", 1.0)]), &["shell", "plug"]);
    let c = compile(&r, &a).expect("compiles");
    assert!(
        matches!(status_of(&c, "shell"), FitStatus::Rejected(_)),
        "{:?}",
        status_of(&c, "shell")
    );
    assert_eq!(status_of(&c, "plug"), FitStatus::Fitted);
    assert!(!drawn(&c, "shell") && drawn(&c, "plug"));
    let plug = c
        .drawable_garments()
        .iter()
        .find(|g| g.id == "plug")
        .expect("drawn");
    assert_eq!(plug.hidden_body_vertices, (16u32..24).collect::<Vec<u32>>());
    // the measured vertices are masked, not forgiven: the shell's metrics see the full bulge
    let shell_depth = f64::from(depth_of(&c, "shell"));
    assert!(shell_depth > 0.015, "shell depth {shell_depth}");
    assert_eq!(
        c.fit_status()
            .iter()
            .find(|r| r.id == "plug")
            .unwrap()
            .metrics
            .penetrating,
        0
    );
}

// ---- garment adaptation (opt-in, deterministic) ----

/// The shell's fixed helper vertices in the full index space: 32 body vertices, then 5 joint
/// helper groups of 4, then the 8 fixed helpers the shell is bound to 1:1 (zero offset).
const SHELL_HELPER_FIRST: usize = 52;
const SHELL_VERTICES: usize = 8;
/// The shell hides body ring 0 (vertices 0..8): those body vertices are not measured.
const SHELL_HIDDEN_BODY: usize = 8;

fn shell_recipe(bulge: f32, lean: f32, adaptation: Option<AdaptationV1>) -> RecipeV1 {
    let mut r = with_garments(recipe(&[("bulge", bulge), ("lean", lean)]), &["shell"]);
    r.fit_params.adaptation = adaptation;
    r
}

/// The shell's vertices as fitted (before any adaptation): its fixed helpers, read from the body.
fn shell_fitted(c: &Compiled) -> Vec<Vec3> {
    (0..SHELL_VERTICES)
        .map(|k| c.shaped_positions()[SHELL_HELPER_FIRST + k])
        .collect()
}

fn drawn_positions(c: &Compiled, id: &str) -> Vec<Vec3> {
    c.drawable_garments()
        .iter()
        .find(|g| g.id == id)
        .unwrap_or_else(|| panic!("{id} is not drawn"))
        .positions
        .clone()
}

fn bits(v: Vec3) -> [u32; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    let d = [
        f64::from(a.x) - f64::from(b.x),
        f64::from(a.y) - f64::from(b.y),
        f64::from(a.z) - f64::from(b.z),
    ];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The clearance of a garment vertex to the shaped body, measured in f64 with the rule of the
/// fit check: the nearest body vertex (ring 0 is hidden by the shell and skipped), and the signed
/// distance along that vertex's normal.
fn clearance_of(c: &Compiled, g: Vec3) -> f64 {
    let body = &c.shaped_positions()[..c.body_vertex_count()];
    let mut best = 0usize;
    let mut best_d = f64::INFINITY;
    for (i, b) in body.iter().enumerate() {
        let d = dist(g, *b);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    assert!(
        best >= SHELL_HIDDEN_BODY,
        "nearest body vertex {best} is hidden"
    );
    let n = c.body_normals()[best];
    let b = body[best];
    (f64::from(g.x) - f64::from(b.x)) * f64::from(n.x)
        + (f64::from(g.y) - f64::from(b.y)) * f64::from(n.y)
        + (f64::from(g.z) - f64::from(b.z)) * f64::from(n.z)
}

/// Adaptation, partial case. `bulge 0.3` with `lean 0.05` pushes the +y side of the body ring
/// through the shell: vertices 1, 2 and 3 are closer than the 2 mm clearance (vertex 2 by 4.6 mm,
/// 2.6 mm of it penetration, over the 2 mm tolerance), vertices 0 and 4 are close but clear. Without
/// adaptation the shell is `NeedsAdaptation` and not drawn. With it: the three are pushed to the
/// clearance, their 1-ring (0 and 4; the shell is a fan around vertex 0) is feathered, vertices 5..8
/// are bit-identical, every vertex keeps the clearance (measured here in f64), the displacement is
/// under the cap, the report and the diagnostic carry the statistics, and the same fit check now
/// says `Fitted`. Kills `no_adapt` (the garment stays `NeedsAdaptation`).
#[test]
fn adaptation_makes_a_penetrating_garment_fit_and_leaves_the_rest_untouched() {
    let a = assets();
    let plain = compile(&shell_recipe(0.3, 0.05, None), &a).expect("compiles");
    assert!(
        matches!(
            status_of(&plain, "shell"),
            FitStatus::NeedsAdaptation(FitReason::Penetration { .. })
        ),
        "{:?}",
        status_of(&plain, "shell")
    );
    assert!(!drawn(&plain, "shell"));
    assert!(plain.fit_status()[0].adaptation.is_none());

    let ad = AdaptationV1::default();
    let (eps, cap) = (f64::from(ad.clearance_m), f64::from(ad.max_displacement_m));
    let r = shell_recipe(0.3, 0.05, Some(ad));
    let c = compile(&r, &a).expect("compiles");
    // the same body, a different fit (the parameters are in the fit id)
    assert_eq!(c.body_rev_id(), plain.body_rev_id());
    assert_ne!(c.fit_id(), plain.fit_id());
    assert_eq!(status_of(&c, "shell"), FitStatus::Fitted);
    assert!(drawn(&c, "shell"));
    let report = &c.fit_status()[0];
    assert_eq!(report.metrics.penetrating, 0);
    let ar = report.adaptation.as_ref().expect("adaptation report");
    assert!(ar.applied);
    assert_eq!(
        ar.pushed, 3,
        "vertices 1, 2, 3 are closer than the clearance"
    );
    assert_eq!(ar.moved, 5, "the pushed vertices and their 1-ring (0, 4)");
    assert!(
        ar.max_displacement_m > 0.004 && ar.max_displacement_m < 0.006,
        "max displacement {}",
        ar.max_displacement_m
    );
    assert!(c.diagnostics().iter().any(|d| matches!(
        d,
        Diagnostic::GarmentAdapted { id, pushed: 3, moved: 5, .. } if id == "shell"
    )));
    assert!(!c
        .diagnostics()
        .iter()
        .any(|d| matches!(d, Diagnostic::GarmentNotDrawn { .. })));

    let before = shell_fitted(&c);
    let after = drawn_positions(&c, "shell");
    assert_eq!(after.len(), SHELL_VERTICES);
    // untouched vertices are bit-identical; the pushed ones and the 1-ring moved
    for k in 5..8 {
        assert_eq!(bits(after[k]), bits(before[k]), "vertex {k} must not move");
    }
    for k in 0..5 {
        assert_ne!(bits(after[k]), bits(before[k]), "vertex {k} must move");
    }
    // clearance everywhere, displacement under the cap, and the report is the true maximum
    let mut max_disp = 0.0f64;
    for k in 0..SHELL_VERTICES {
        let clr = clearance_of(&c, after[k]);
        assert!(clr >= eps - 1.0e-5, "vertex {k}: clearance {clr}");
        let d = dist(after[k], before[k]);
        assert!(d <= cap, "vertex {k}: displacement {d}");
        max_disp = max_disp.max(d);
    }
    assert!(
        (max_disp - f64::from(ar.max_displacement_m)).abs() < 1.0e-6,
        "{max_disp} vs {}",
        ar.max_displacement_m
    );
    // the feathered 1-ring moves less than the pushed vertex it borders (no crease: the
    // displacement falls off away from vertex 2)
    let d_of = |k: usize| dist(after[k], before[k]);
    assert!(d_of(2) > d_of(4) && d_of(4) > 0.0 && d_of(0) > 0.0);

    // deterministic: the same recipe gives the same bits and the same id
    let again = compile(&r, &a).expect("compiles");
    assert_eq!(again.fit_id(), c.fit_id());
    let after2 = drawn_positions(&again, "shell");
    for k in 0..SHELL_VERTICES {
        assert_eq!(bits(after2[k]), bits(after[k]));
    }
}

/// Adaptation, whole garment. `bulge 0.5` pushes all eight shell vertices 4 mm through the body:
/// every vertex is pushed to the clearance (about 6 mm in all), the garment is `Fitted` and drawn,
/// and the clearance holds everywhere. Kills `no_adapt`.
#[test]
fn adaptation_pushes_a_uniformly_penetrating_garment_out_to_the_clearance() {
    let a = assets();
    let ad = AdaptationV1::default();
    let eps = f64::from(ad.clearance_m);
    let c = compile(&shell_recipe(0.5, 0.0, Some(ad)), &a).expect("compiles");
    assert_eq!(status_of(&c, "shell"), FitStatus::Fitted);
    assert!(drawn(&c, "shell"));
    let ar = c.fit_status()[0].adaptation.clone().expect("report");
    assert!(ar.applied);
    assert_eq!((ar.pushed, ar.moved), (8, 8));
    let before = shell_fitted(&c);
    let after = drawn_positions(&c, "shell");
    for k in 0..SHELL_VERTICES {
        let clr = clearance_of(&c, after[k]);
        assert!(clr >= eps - 1.0e-5, "vertex {k}: clearance {clr}");
        let d = dist(after[k], before[k]);
        assert!(d > 0.005 && d < 0.008, "vertex {k}: displacement {d}");
    }
}

/// Garments that already clear the body are not touched: with adaptation on, the tunic, the shell
/// and the tabard at the neutral body keep their `Fitted` verdicts and their positions bit for bit
/// (nothing is within the clearance), the report says so, and no diagnostic is added.
#[test]
fn adaptation_leaves_a_garment_that_already_clears_the_body_bit_identical() {
    let a = assets();
    let ids = ["tunic", "shell", "tabard"];
    let plain = compile(&with_garments(RecipeV1::new(), &ids), &a).expect("compiles");
    let mut r = with_garments(RecipeV1::new(), &ids);
    r.fit_params.adaptation = Some(AdaptationV1::default());
    let c = compile(&r, &a).expect("compiles");
    assert_eq!(c.body_rev_id(), plain.body_rev_id());
    for id in ids {
        assert_eq!(status_of(&c, id), FitStatus::Fitted);
        let p = drawn_positions(&plain, id);
        let q = drawn_positions(&c, id);
        assert_eq!(p.len(), q.len());
        for k in 0..p.len() {
            assert_eq!(bits(p[k]), bits(q[k]), "{id} vertex {k}");
        }
    }
    for rep in c.fit_status() {
        let ar = rep.adaptation.as_ref().expect("report");
        assert!(ar.applied && ar.pushed == 0 && ar.moved == 0 && ar.max_displacement_m == 0.0);
    }
    assert_eq!(c.diagnostics(), plain.diagnostics());
}

/// The cap. Adaptation never silently warps a garment past `max_displacement_m`: `bulge 1.0` needs
/// about 22 mm (20 mm of penetration plus the clearance), over the default 20 mm cap, so the shell
/// stays `Rejected` with the metrics and the verdict of the unadapted fit, is not drawn, and the
/// refusal is reported; `bulge 0.5` needs about 6 mm, over a 3 mm cap, so it stays `NeedsAdaptation`
/// the same way. Both verdicts equal the ones without adaptation. Kills `no_adapt` (no refusal is
/// reported) via the report and diagnostic assertions.
#[test]
fn an_adaptation_over_the_cap_is_refused_and_the_unadapted_verdict_stands() {
    let a = assets();
    let mut tight = AdaptationV1::default();
    tight.max_displacement_m = 0.003;
    let cases: [(f32, AdaptationV1); 2] = [(1.0, AdaptationV1::default()), (0.5, tight)];
    for (bulge, ad) in cases {
        let cap = ad.max_displacement_m;
        let plain = compile(&shell_recipe(bulge, 0.0, None), &a).expect("compiles");
        let c = compile(&shell_recipe(bulge, 0.0, Some(ad)), &a).expect("compiles");
        let st = status_of(&c, "shell");
        assert_eq!(st, status_of(&plain, "shell"), "bulge {bulge}");
        assert_ne!(st, FitStatus::Fitted);
        assert!(!drawn(&c, "shell"), "bulge {bulge}");
        assert_eq!(c.fit_status()[0].metrics, plain.fit_status()[0].metrics);
        let ar = c.fit_status()[0].adaptation.clone().expect("report");
        assert!(!ar.applied, "bulge {bulge}");
        assert!(ar.max_displacement_m > cap, "bulge {bulge}: {ar:?}");
        assert!(c.diagnostics().iter().any(|d| matches!(
            d,
            Diagnostic::GarmentAdaptationRefused { id, cap_m, .. } if id == "shell" && *cap_m == cap
        )));
        assert!(c
            .diagnostics()
            .iter()
            .any(|d| matches!(d, Diagnostic::GarmentNotDrawn { id, .. } if id == "shell")));
        assert!(!c
            .diagnostics()
            .iter()
            .any(|d| matches!(d, Diagnostic::GarmentAdapted { .. })));
    }
}

/// A garment that is `Rejected` before adaptation stays `Rejected` even when its adaptation would
/// fit under the cap: `bulge 1.0` needs about 22 mm, under a 50 mm cap, yet the verdict, metrics and
/// positions are the unadapted ones, the refusal is reported and the garment is not drawn.
/// Adaptation therefore never widens the `adapt_depth_m` bound. Kills `adapt_rejected`.
#[test]
fn a_rejected_garment_is_never_adapted_even_under_the_cap() {
    let a = assets();
    let mut wide = AdaptationV1::default();
    wide.max_displacement_m = 0.05;
    let plain = compile(&shell_recipe(1.0, 0.0, None), &a).expect("compiles");
    assert!(
        matches!(status_of(&plain, "shell"), FitStatus::Rejected(_)),
        "{:?}",
        status_of(&plain, "shell")
    );
    let c = compile(&shell_recipe(1.0, 0.0, Some(wide)), &a).expect("compiles");
    assert_eq!(status_of(&c, "shell"), status_of(&plain, "shell"));
    assert!(!drawn(&c, "shell"));
    assert_eq!(c.fit_status()[0].metrics, plain.fit_status()[0].metrics);
    let ar = c.fit_status()[0].adaptation.clone().expect("report");
    assert!(!ar.applied, "{ar:?}");
    assert!(
        ar.max_displacement_m <= 0.05,
        "the cap alone would have applied it: {ar:?}"
    );
    assert!(c.diagnostics().iter().any(|d| matches!(
        d,
        Diagnostic::GarmentAdaptationRefused { id, .. } if id == "shell"
    )));
    assert!(!c
        .diagnostics()
        .iter()
        .any(|d| matches!(d, Diagnostic::GarmentAdapted { .. })));
}

/// The fit id covers the adaptation parameters. Without adaptation the id is the v2 id of old (the
/// hand-rebuilt test above); with it the algorithm id is `...v2+adapt.v1`, the parameter count is 5
/// and the three adaptation parameters follow in order. Each parameter moves the id. Invalid
/// parameters are a typed error.
#[test]
fn the_fit_id_covers_the_adaptation_parameters() {
    let a = assets();
    let at = |f: &dyn Fn(&mut AdaptationV1)| {
        let mut ad = AdaptationV1::default();
        f(&mut ad);
        let mut r = with_garments(RecipeV1::new(), &["tunic"]);
        r.fit_params.adaptation = Some(ad);
        compile(&r, &a)
    };
    let base = at(&|_| {}).expect("compiles");

    let mut fit = Vec::new();
    fit.extend_from_slice(b"gibson.figure.fit.v1\0");
    fit.extend_from_slice(&base.body_rev_id().0);
    put_u32(&mut fit, 1);
    put_str(&mut fit, "tunic");
    fit.extend_from_slice(&sha256::digest(TUNIC));
    put_str(&mut fit, "gibson.fit.vertex-normal-penetration.v2+adapt.v1");
    put_u32(&mut fit, 5);
    put_str(&mut fit, "tolerance_m");
    fit.extend_from_slice(&0.002f32.to_le_bytes());
    put_str(&mut fit, "adapt_depth_m");
    fit.extend_from_slice(&0.01f32.to_le_bytes());
    put_str(&mut fit, "adapt_clearance_m");
    fit.extend_from_slice(&0.002f32.to_le_bytes());
    put_str(&mut fit, "adapt_max_displacement_m");
    fit.extend_from_slice(&0.02f32.to_le_bytes());
    put_str(&mut fit, "adapt_smooth_iterations");
    put_u32(&mut fit, 2);
    assert_eq!(sha256::digest(&fit), base.fit_id().0, "adaptation fit id");

    let off = compile(&with_garments(RecipeV1::new(), &["tunic"]), &a).expect("compiles");
    assert_ne!(off.fit_id(), base.fit_id());
    for (what, c) in [
        ("clearance", at(&|x| x.clearance_m = 0.003)),
        ("cap", at(&|x| x.max_displacement_m = 0.03)),
        ("iterations", at(&|x| x.smooth_iterations = 3)),
    ] {
        let c = c.expect("compiles");
        assert_ne!(c.fit_id(), base.fit_id(), "{what} must move the fit id");
        assert_eq!(c.body_rev_id(), base.body_rev_id());
    }

    assert!(matches!(
        at(&|x| x.clearance_m = -0.001),
        Err(CompileError::BadFitParams)
    ));
    assert!(matches!(
        at(&|x| x.max_displacement_m = f32::NAN),
        Err(CompileError::BadFitParams)
    ));
    assert!(matches!(
        at(&|x| x.smooth_iterations = 17),
        Err(CompileError::BadFitParams)
    ));
}

/// Typed errors for garment, hair, slot, anchor and attachment input.
#[test]
fn bad_garment_hair_and_slot_input_is_a_typed_error() {
    let a = assets();
    let mut r = RecipeV1::new();
    r.garments.push(GarmentV1::new("nope"));
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::UnknownGarment { .. })
    ));
    let r = with_garments(RecipeV1::new(), &["tunic", "tunic"]);
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::DuplicateGarment { .. })
    ));
    let r = with_garments(RecipeV1::new(), &["cap"]);
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::NotAGarment { .. })
    ));
    let mut r = RecipeV1::new();
    r.hair.style_id = Some("tunic".to_string());
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::UnknownHair { .. })
    ));
    let mut r = RecipeV1::new();
    r.materials.push(MaterialSlotV1::new("not_a_slot"));
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::UnknownMaterialSlot { .. })
    ));
    let mut r = RecipeV1::new();
    r.materials.push(MaterialSlotV1::new("base"));
    r.materials.push(MaterialSlotV1::new("base"));
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::DuplicateMaterialSlot { .. })
    ));
    let mut r = with_garments(RecipeV1::new(), &["tunic"]);
    r.garments[0]
        .slot_bindings
        .push(("collar".to_string(), "nope".to_string()));
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::UnknownMaterialSlot { .. })
    ));
    let mut r = RecipeV1::new();
    r.attachments.push(AttachmentV1::new(
        "x",
        AnchorV1::Bone("no_such_bone".to_string()),
    ));
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::UnknownAnchor { .. })
    ));
    let mut r = RecipeV1::new();
    r.attachments.push(AttachmentV1::new(
        "x",
        AnchorV1::Landmark("foot".to_string()),
    ));
    r.attachments
        .push(AttachmentV1::new("x", AnchorV1::Bone("neck".to_string())));
    assert!(matches!(
        compile(&r, &a),
        Err(CompileError::DuplicateAttachment { .. })
    ));
    let mut r = RecipeV1::new();
    r.attachments.push(AttachmentV1::new(
        "x",
        AnchorV1::Landmark("foot".to_string()),
    ));
    r.attachments
        .push(AttachmentV1::new("y", AnchorV1::Bone("neck".to_string())));
    assert!(compile(&r, &a).is_ok());
}

/// Jaw and mouth channels are inert (and reported when driven) only while the lower face is
/// covered.
#[test]
fn a_covered_lower_face_reports_driven_jaw_and_mouth_channels() {
    let a = assets();
    let open = compile(&RecipeV1::new(), &a).expect("compiles");
    assert!(open
        .face_channel_diagnostics(&["jaw_open", "brow_up"])
        .is_empty());
    let mut r = RecipeV1::new();
    r.lower_face_covered = true;
    let c = compile(&r, &a).expect("compiles");
    assert!(c.lower_face_covered());
    let d = c.face_channel_diagnostics(&["jaw_open", "mouth_smile", "brow_up", "lid_close"]);
    assert_eq!(d.len(), 2);
    assert!(d
        .iter()
        .all(|x| matches!(x, Diagnostic::FaceChannelInert { channel } if channel.starts_with("jaw") || channel.starts_with("mouth"))));
}

// ---- the pose path ----

fn rot_z90(p: [f64; 3]) -> [f64; 3] {
    [-p[1], p[0], p[2]]
}

/// An independent GFA part. Each vertex is bound to fixture vertex 16 with its
/// specified offset and to the root bone with unit weight.
fn bound_patch(id: &str, kind: &str, offsets: &[[f32; 3]], triangles: &[u32]) -> Vec<u8> {
    let mut out = b"gibson.figure.asset.part.v1\0".to_vec();
    put_str(&mut out, id);
    put_str(&mut out, kind);
    put_u32(&mut out, offsets.len() as u32);
    for p in offsets.iter().flatten() {
        out.extend_from_slice(&p.to_le_bytes());
    }
    put_u32(&mut out, triangles.len() as u32);
    for &i in triangles {
        put_u32(&mut out, i);
    }
    for p in offsets {
        for _ in 0..3 {
            put_u32(&mut out, 16);
        }
        for f in [1.0_f32, 0.0, 0.0].iter().chain(p) {
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    put_u32(&mut out, 0);
    for _ in offsets {
        out.push(1);
        put_u32(&mut out, 0);
        out.extend_from_slice(&1.0_f32.to_le_bytes());
    }
    put_u32(&mut out, 0);
    out
}

#[test]
fn forward_eye_regions_and_face_materials_follow_the_posed_triangles() {
    // The first three triangles have centroid directions 0°, 23° and >90°
    // relative to the forward axis. Shared edges connect them into one eye.
    let points = [
        [0.0, -1.0, -0.05],
        [0.04, -1.0, 0.04],
        [-0.04, -1.0, 0.04],
        [0.4, -1.0, 0.0],
        [0.48, -1.0, 0.02],
        [0.42, -1.0, 0.05],
        [0.0, 1.0, 0.0],
        [-0.5, 0.0, 0.0],
        [0.5, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
    ]
    .map(|p| p.map(|v| v * 0.01));
    let triangles = [
        0, 1, 2, 3, 4, 5, 6, 8, 10, 0, 2, 3, 2, 3, 4, 5, 6, 8, 0, 9, 7, 6, 7, 9, 3, 5, 6,
    ];
    let eyes = bound_patch("eye", "eyes", &points, &triangles);
    let brow = bound_patch("brow", "eyebrows", &points[..3], &[0, 1, 2]);
    let lash = bound_patch("lash", "eyelashes", &points[..3], &[0, 1, 2]);
    let teeth = bound_patch("teeth", "teeth", &points[..3], &[0, 1, 2]);
    let a = AssetSet::from_bytes(BODY, &[&eyes, &brow, &lash, &teeth]).unwrap();
    let c = compile(&recipe(&[]), &a).unwrap();
    let rest = c.rest_render_meshes();
    let mut frame = c.rest_frame();
    frame.locals[0].rotation =
        Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), std::f32::consts::FRAC_PI_2);
    let posed = c.render_meshes(&frame).unwrap();
    for meshes in [&rest, &posed] {
        for (id, expected) in [
            ("eye", &[9, 6, 8][..]),
            ("brow", &[10]),
            ("lash", &[10]),
            ("teeth", &[11]),
        ] {
            let m = meshes.iter().find(|m| m.id == id).unwrap();
            assert_eq!(m.role, MeshRole::Face);
            assert_eq!(&m.slot_per_triangle[..expected.len()], expected, "{id}");
        }
    }
    let before = rest.iter().find(|m| m.id == "eye").unwrap();
    let after = posed.iter().find(|m| m.id == "eye").unwrap();
    assert_ne!(before.positions, after.positions);
    assert_eq!(before.slot_per_triangle, after.slot_per_triangle);
}

#[test]
fn the_face_selection_and_source_bytes_change_only_presentation_identity() {
    let points = [[0.0, -0.02, 0.0], [0.01, 0.0, 0.0], [0.0, 0.0, 0.01]];
    let original = bound_patch("brow", "eyebrows", &points, &[0, 1, 2]);
    let edited = bound_patch("brow", "eyebrows", &points, &[0, 2, 1]);
    let mut r = recipe(&[]);
    let a = AssetSet::from_bytes(BODY, &[&original]).unwrap();
    let b = AssetSet::from_bytes(BODY, &[&edited]).unwrap();
    let on = compile(&r, &a).unwrap();
    let changed = compile(&r, &b).unwrap();
    r.face_parts = gibson::figure::studio::FacePartsV1::Off;
    let off = compile(&r, &a).unwrap();
    assert_eq!(on.face_parts().len(), 1);
    assert!(off.face_parts().is_empty());
    for other in [changed, off] {
        assert_eq!(on.body_rev_id(), other.body_rev_id());
        assert_eq!(on.fit_id(), other.fit_id());
        assert_ne!(on.present_id(), other.present_id());
    }
}

#[test]
fn the_outer_garment_is_measured_against_the_fitted_inner_garment() {
    let patch = |id: &str, x| {
        bound_patch(
            id,
            "clothes",
            &[[x, -0.001, -0.001], [x, 0.001, -0.001], [x, 0.0, 0.001]],
            &[0, 1, 2],
        )
    };
    let deeper = patch("deeper", 0.025);
    let shallower = patch("shallower", 0.030);
    let a = AssetSet::from_bytes(BODY, &[&deeper, &shallower]).unwrap();
    let outward = compile(&with_garments(recipe(&[]), &["deeper", "shallower"]), &a).unwrap();
    assert_eq!(status_of(&outward, "deeper"), FitStatus::Fitted);
    assert_eq!(status_of(&outward, "shallower"), FitStatus::Fitted);
    let inward = compile(&with_garments(recipe(&[]), &["shallower", "deeper"]), &a).unwrap();
    assert_eq!(status_of(&inward, "shallower"), FitStatus::Fitted);
    assert!(matches!(
        status_of(&inward, "deeper"),
        FitStatus::NeedsAdaptation(_)
    ));
    assert_near(
        "independent panel depth",
        depth_of(&inward, "deeper"),
        0.005,
        1e-6,
    );
}

/// At rest the posed body equals the shaped body and the posed garments equal their fits (the
/// skin matrices are the identity). A rigid root motion (the root rotated 90 degrees about z
/// about its own world position, plus a root translation) moves the body and every drawn garment
/// by exactly that rigid motion, in f64, and a garment without a verdict is not posed.
#[test]
fn garments_follow_the_pose_and_only_drawn_garments_are_posed() {
    let a = assets();
    let r = with_garments(
        recipe(&[("bulge", 1.0), ("stature", 0.3)]),
        &["tunic", "shell", "tabard"],
    );
    let c = compile(&r, &a).expect("compiles");
    assert!(!drawn(&c, "shell"), "the shell is rejected in this body");

    let rest = c.pose_vertices(&c.rest_frame()).expect("rest pose");
    assert_eq!(rest.body.len(), c.body_vertex_count());
    for i in 0..rest.body.len() {
        let (p, q) = (rest.body[i], c.shaped_positions()[i]);
        assert_near3(
            "rest body",
            p,
            [f64::from(q.x), f64::from(q.y), f64::from(q.z)],
            2.0e-5,
        );
    }
    let posed_ids: Vec<&str> = rest.garments.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(
        posed_ids,
        ["tunic", "tabard"],
        "the rejected shell must not be posed"
    );
    for g in &rest.garments {
        let fit = c
            .drawable_garments()
            .iter()
            .find(|d| d.id == g.id)
            .expect("drawn");
        for (p, q) in g.positions.iter().zip(fit.positions.iter()) {
            assert_near3(
                "rest garment",
                *p,
                [f64::from(q.x), f64::from(q.y), f64::from(q.z)],
                2.0e-5,
            );
        }
    }

    // the root's world position: the f64 centroid of its anchor group in the shaped surface
    let group = a.joint_anchor(0).expect("root");
    let mut ctr = [0.0f64; 3];
    for &v in group {
        let p = c.shaped_positions()[v as usize];
        ctr[0] += f64::from(p.x) / group.len() as f64;
        ctr[1] += f64::from(p.y) / group.len() as f64;
        ctr[2] += f64::from(p.z) / group.len() as f64;
    }
    let mut frame = c.rest_frame();
    frame.root_t = Vec3::new(1.0, 2.0, 3.0);
    frame.locals[0].rotation =
        Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), std::f32::consts::FRAC_PI_2);
    let moved = c.pose_vertices(&frame).expect("posed");
    let expect = |v: Vec3| -> [f64; 3] {
        let rel = [
            f64::from(v.x) - ctr[0],
            f64::from(v.y) - ctr[1],
            f64::from(v.z) - ctr[2],
        ];
        let r = rot_z90(rel);
        [
            r[0] + ctr[0] + 1.0,
            r[1] + ctr[1] + 2.0,
            r[2] + ctr[2] + 3.0,
        ]
    };
    for i in 0..moved.body.len() {
        assert_near3(
            "moved body",
            moved.body[i],
            expect(c.shaped_positions()[i]),
            3.0e-5,
        );
    }
    for g in &moved.garments {
        let fit = c
            .drawable_garments()
            .iter()
            .find(|d| d.id == g.id)
            .expect("drawn");
        for (p, q) in g.positions.iter().zip(fit.positions.iter()) {
            assert_near3("moved garment", *p, expect(*q), 3.0e-5);
        }
    }
    // posed normals are the rest normals rotated by the same rigid motion (translation ignored)
    for i in 0..moved.body_normals.len() {
        let n = c.body_normals()[i];
        let w = rot_z90([f64::from(n.x), f64::from(n.y), f64::from(n.z)]);
        assert_near3("moved body normal", moved.body_normals[i], w, 1.0e-4);
        let l = moved.body_normals[i];
        assert_near("unit normal", l.length(), 1.0, 1.0e-4);
    }
    for g in &moved.garments {
        let fit = c
            .drawable_garments()
            .iter()
            .find(|d| d.id == g.id)
            .expect("drawn");
        for (p, q) in g.normals.iter().zip(fit.normals.iter()) {
            let w = rot_z90([f64::from(q.x), f64::from(q.y), f64::from(q.z)]);
            assert_near3("moved garment normal", *p, w, 1.0e-4);
        }
    }
    // an outward ring-2 vertex at angle 0 has a normal pointing along +x
    assert!(c.body_normals()[16].x > 0.7, "{:?}", c.body_normals()[16]);
    // the linear mode agrees on a rigid motion
    let mut lin = r.clone();
    lin.skinning = SkinningMode::Linear;
    let cl = compile(&lin, &a).expect("compiles");
    let ml = cl.pose_vertices(&frame).expect("posed");
    for i in 0..ml.body.len() {
        assert_near3(
            "linear body",
            ml.body[i],
            expect(cl.shaped_positions()[i]),
            3.0e-5,
        );
    }
}

/// Bad frames are typed errors.
#[test]
fn a_bad_frame_is_a_typed_error() {
    let c = build(&RecipeV1::new());
    let mut short = c.rest_frame();
    short.locals.pop();
    assert!(matches!(
        c.pose_vertices(&short),
        Err(PoseError::JointCount {
            expected: 5,
            got: 4
        })
    ));
    let mut nan = c.rest_frame();
    nan.root_t = Vec3::new(f32::NAN, 0.0, 0.0);
    assert!(matches!(
        c.pose_vertices(&nan),
        Err(PoseError::InvalidTransform { joint: None })
    ));
    let mut bad = c.rest_frame();
    bad.locals[2] = Transform::new(
        Vec3::new(f32::INFINITY, 0.0, 0.0),
        Quat::IDENTITY,
        Vec3::new(1.0, 1.0, 1.0),
    );
    assert!(matches!(
        c.pose_vertices(&bad),
        Err(PoseError::InvalidTransform { joint: Some(2) })
    ));
    let ok = TapeFrame::new(Vec3::ZERO, c.skeleton().rest_locals());
    assert!(c.pose_vertices(&ok).is_ok());
}

// ---- incremental control edits and drawable meshes ----

/// Everything observable of two compiles, compared bit for bit (floats by their `==`, which the
/// same code path makes exact): ids, bytes, surface, normals, verdicts, drawable garments, hair,
/// diagnostics and the rest pose.
fn assert_same(label: &str, a: &Compiled, b: &Compiled) {
    assert_eq!(a.body_rev_id(), b.body_rev_id(), "{label}: body id");
    assert_eq!(a.fit_id(), b.fit_id(), "{label}: fit id");
    assert_eq!(a.present_id(), b.present_id(), "{label}: presentation id");
    assert_eq!(a.body_bytes(), b.body_bytes(), "{label}: body bytes");
    assert_eq!(
        a.shaped_positions(),
        b.shaped_positions(),
        "{label}: surface"
    );
    assert_eq!(a.body_normals(), b.body_normals(), "{label}: normals");
    assert_eq!(a.fit_status(), b.fit_status(), "{label}: verdicts");
    assert_eq!(
        a.drawable_garments(),
        b.drawable_garments(),
        "{label}: garments"
    );
    assert_eq!(a.hair(), b.hair(), "{label}: hair");
    assert_eq!(a.diagnostics(), b.diagnostics(), "{label}: diagnostics");
    assert_eq!(a.ground_lift_m(), b.ground_lift_m(), "{label}: lift");
    assert_eq!(
        a.pose_vertices(&a.rest_frame()),
        b.pose_vertices(&b.rest_frame()),
        "{label}: rest pose"
    );
}

/// `with_control` is bit-identical to a full compile of the edited recipe: one edit, a chain of
/// edits, an out-of-domain edit (with its diagnostic), an edit that flips a garment verdict, and
/// an edit back to the start (which returns the first compile's bytes). Its result also MOVES the
/// control's landmark measurement (`hip_breadth` 1.0 gives a hip distance of 0.6, from 0.4); a
/// `with_control` that returned the old shape fails here. Kills `stale_control` (and
/// `ignore_surface_update`).
#[test]
fn with_control_is_bit_identical_to_a_full_compile_and_moves_the_measurement() {
    let a = std::sync::Arc::new(assets());
    let mut r = with_garments(recipe(&[("bulge", 0.3)]), &["tunic", "shell", "tabard"]);
    r.hair.style_id = Some("cap".to_string());
    r.materials.push(MaterialSlotV1::new("base"));
    let base = gibson::figure::studio::compile_shared(&r, &a).expect("compiles");
    assert_near("start hip", m_hip(&base) as f32, 0.4, 2.0e-5);

    // one edit
    let edited = base.with_control("hip_breadth", 1.0).expect("edit");
    assert_near("edited hip", m_hip(&edited) as f32, 0.6, 2.0e-5);
    assert_ne!(edited.body_rev_id(), base.body_rev_id());
    let mut r1 = r.clone();
    r1.set_control("hip_breadth", 1.0);
    assert_same("one edit", &edited, &compile(&r1, &a).expect("compiles"));
    assert_eq!(edited.recipe(), &r1);

    // a chain
    let chained = edited.with_control("stature", -0.5).expect("edit");
    let mut r2 = r1.clone();
    r2.set_control("stature", -0.5);
    assert_same("chain", &chained, &compile(&r2, &a).expect("compiles"));
    assert_near("chained height", m_height(&chained) as f32, 1.35, 2.0e-5);

    // an edit that flips a verdict: the shell is NeedsAdaptation at 0.5 and drawn at 0.0
    let flat = base.with_control("bulge", 0.0).expect("edit");
    let bulged = flat.with_control("bulge", 0.5).expect("edit");
    assert_eq!(status_of(&flat, "shell"), FitStatus::Fitted);
    assert!(matches!(
        status_of(&bulged, "shell"),
        FitStatus::NeedsAdaptation(_)
    ));
    let mut r3 = r.clone();
    r3.set_control("bulge", 0.5);
    assert_same(
        "verdict flip",
        &bulged,
        &compile(&r3, &a).expect("compiles"),
    );

    // out of domain: applied, reported, identical to the full compile
    let wild = base.with_control("stature", 1.5).expect("edit");
    let mut r4 = r.clone();
    r4.set_control("stature", 1.5);
    let full = compile(&r4, &a).expect("compiles");
    assert_same("out of domain", &wild, &full);
    assert!(wild
        .diagnostics()
        .iter()
        .any(|d| matches!(d, Diagnostic::ControlOutOfDomain { .. })));

    // back to the start: the first compile's bytes again
    let back = edited.with_control("hip_breadth", 0.0).expect("edit");
    assert_eq!(back.body_bytes(), base.body_bytes());
    assert_eq!(back.body_rev_id(), base.body_rev_id());

    // typed errors, as in a full compile
    assert!(matches!(
        base.with_control("no_such_control", 0.1),
        Err(CompileError::UnknownControl { .. })
    ));
    assert!(matches!(
        base.with_control("stature", f32::NAN),
        Err(CompileError::NonFinite { .. })
    ));
}

/// The drawable meshes. The body is drawn minus the triangles that touch a vertex a drawn garment
/// hides (the plug hides ring 2, so the 32 triangles around it go and 16 remain); a garment with no
/// `Fitted` verdict is absent; garments take the slot of their first slot binding (trim = 2) and
/// the body and hair take `base` (0); normals are unit; the posed meshes have the rest meshes'
/// shape and move with the pose.
#[test]
fn drawable_meshes_hide_deleted_body_triangles_and_carry_slots() {
    let a = assets();
    let plain = compile(&recipe(&[]), &a).expect("compiles");
    let meshes = plain.rest_render_meshes();
    assert_eq!(meshes.len(), 1);
    assert_eq!(meshes[0].role, MeshRole::Body);
    assert_eq!(meshes[0].triangles.len(), 48, "no garment hides anything");
    assert_eq!(meshes[0].positions.len(), 32);
    assert_eq!(meshes[0].normals.len(), 32);
    assert_eq!(meshes[0].slot_per_triangle, vec![0u8; 48]);

    let mut r = with_garments(recipe(&[("bulge", 1.0)]), &["plug", "shell", "tabard"]);
    r.garments[0]
        .slot_bindings
        .push(("collar".to_string(), "trim".to_string()));
    r.hair.style_id = Some("cap".to_string());
    r.materials.push({
        let mut m = MaterialSlotV1::new("trim");
        m.albedo = [0.9, 0.1, 0.2];
        m
    });
    let c = compile(&r, &a).expect("compiles");
    let rest = c.rest_render_meshes();
    let ids: Vec<&str> = rest.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        ["body", "plug", "tabard", "cap"],
        "the rejected shell is absent"
    );
    let roles: Vec<MeshRole> = rest.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        [
            MeshRole::Body,
            MeshRole::Garment,
            MeshRole::Garment,
            MeshRole::Hair
        ]
    );
    // the plug hides ring 2 (vertices 16..24): every body triangle touching it is gone
    assert_eq!(rest[0].triangles.len(), 16);
    assert!(rest[0]
        .triangles
        .iter()
        .all(|t| t.iter().all(|&v| !(16..24).contains(&v))));
    for m in &rest {
        assert_eq!(m.slot_per_triangle.len(), m.triangles.len());
        assert_eq!(m.positions.len(), m.normals.len());
        for n in &m.normals {
            assert_near("unit normal", n.length(), 1.0, 1.0e-4);
        }
    }
    assert_eq!(rest[1].slot_per_triangle, vec![2u8; 6], "plug: trim");
    assert_eq!(rest[2].slot_per_triangle, vec![0u8; 6], "tabard: base");
    assert_eq!(rest[3].slot_per_triangle, vec![0u8; 6], "hair: base");
    assert_eq!(c.material(2).expect("trim").albedo, [0.9, 0.1, 0.2]);
    assert!(c.material(0).is_none(), "no base material was bound");
    assert!(c.material(200).is_none());

    // posed: same set and shape, positions move with the pose
    let mut frame = c.rest_frame();
    frame.root_t = Vec3::new(1.0, 0.0, 0.0);
    let posed = c.render_meshes(&frame).expect("posed");
    assert_eq!(posed.len(), rest.len());
    for (p, q) in posed.iter().zip(rest.iter()) {
        assert_eq!(p.id, q.id);
        assert_eq!(p.triangles, q.triangles);
        assert_eq!(p.slot_per_triangle, q.slot_per_triangle);
        assert_eq!(p.positions.len(), q.positions.len());
        for (a2, b2) in p.positions.iter().zip(q.positions.iter()) {
            assert_near3(
                "posed vertex",
                *a2,
                [f64::from(b2.x) + 1.0, f64::from(b2.y), f64::from(b2.z)],
                2.0e-5,
            );
        }
    }
    let mut bad = c.rest_frame();
    bad.locals.pop();
    assert!(c.render_meshes(&bad).is_err());
}

// ---- reading gibson.figure.tape.v1 ----

fn tape_header(body_id: &str, joints: &[&str], face: &[&str], frames: usize, sha: &str) -> String {
    let q = |v: &[&str]| {
        v.iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<String>>()
            .join(", ")
    };
    format!(
        "{{\"format\": \"gibson.figure.tape.v1\", \"body_rev_id\": \"{body_id}\", \"fit_id\": \"00\", \
         \"present_id\": \"11\", \"asset_manifest_sha256\": \"ab\", \"controls\": {{\"stature\": 0.5}}, \
         \"style\": {{\"preset\": \"a\\u0062c\"}}, \"fps\": 30, \"frame_count\": {frames}, \
         \"joints\": [{}], \"channels\": [\"root_t3\", \"local_t3\", \"local_r4\", \"local_s3\"], \
         \"face\": [{}], \"events\": [], \"camera_cuts\": [], \"bin_sha256\": \"{sha}\"}}",
        q(joints),
        q(face)
    )
}

fn tape_bin(frames: &[Vec<f32>]) -> Vec<u8> {
    let mut out = Vec::new();
    for f in frames {
        for v in f {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The tape reader: a hand-built `gibson.figure.tape.v1` (JSON with an escape, 2 frames, joints in
/// REVERSED order, 2 face channels) is read back, its joint order is mapped to the rig's, and every
/// refusal is typed: wrong sha, wrong length, wrong format, frame out of range, a joint the rig does
/// not have, a rig joint the tape lacks, a tape made for another body, a non-finite value.
#[test]
fn a_tape_frame_is_read_and_mapped_to_the_rig() {
    let c = build(&RecipeV1::new());
    let rig = ["root", "pelvis", "spine", "neck", "arm_l"];
    let rev: Vec<&str> = rig.iter().rev().copied().collect();
    // per frame: root t3, then per tape joint (neck-first order): t3 r4 s3, then 2 face values
    let mut frames = Vec::new();
    for f in 0..2 {
        let mut v = vec![0.5 + f as f32, 0.0, 2.0];
        for (k, _) in rev.iter().enumerate() {
            let k = k as f32;
            v.extend_from_slice(&[k, f as f32, 0.25, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
        }
        v.extend_from_slice(&[0.125, 0.75]);
        frames.push(v);
    }
    let bin = tape_bin(&frames);
    let body = c.body_rev_id().to_hex();
    let header = tape_header(
        &body,
        &rev,
        &["brow_up", "lid_close"],
        2,
        &hex(&sha256::digest(&bin)),
    );
    let reader = TapeReader::new(header.as_bytes(), &bin).expect("valid tape");
    let h: &TapeHeader = reader.header();
    assert_eq!(h.joints, rev);
    assert_eq!(h.face, ["brow_up", "lid_close"]);
    assert_eq!(h.frame_count, 2);
    assert_eq!(h.fps, 30.0);
    assert_eq!(h.frame_floats(), 3 + 50 + 2);
    assert_eq!(reader.face_channels(1).expect("face"), [0.125, 0.75]);

    // tape order: the first tape joint is arm_l (k = 0), the last is root (k = 4)
    let raw = reader.frame(1).expect("frame");
    assert_eq!(raw.root_t, Vec3::new(1.5, 0.0, 2.0));
    assert_eq!(raw.locals[0].translation, Vec3::new(0.0, 1.0, 0.25));
    assert_eq!(raw.locals[4].translation, Vec3::new(4.0, 1.0, 0.25));
    // rig order: root first, so its local is the tape's LAST joint's
    let f = c.tape_frame(&reader, 1).expect("mapped");
    assert_eq!(f.root_t, Vec3::new(1.5, 0.0, 2.0));
    for (j, name) in rig.iter().enumerate() {
        let want_k = (rig.len() - 1 - j) as f32;
        assert_eq!(
            f.locals[j].translation,
            Vec3::new(want_k, 1.0, 0.25),
            "joint {name}"
        );
    }
    assert!(c.pose_vertices(&f).is_ok(), "a mapped frame poses");
    let one = TapeFrame::from_tape_bytes(header.as_bytes(), &bin, 0).expect("convenience");
    assert_eq!(one.locals.len(), 5);

    // refusals
    let bad_sha = tape_header(&body, &rev, &["a", "b"], 2, &hex(&[0u8; 32]));
    assert_eq!(
        TapeReader::new(bad_sha.as_bytes(), &bin).unwrap_err(),
        TapeError::BinSha
    );
    let more = tape_header(&body, &rev, &["a", "b"], 3, &hex(&sha256::digest(&bin)));
    assert!(matches!(
        TapeReader::new(more.as_bytes(), &bin),
        Err(TapeError::BinLength { .. })
    ));
    let wrong_format = header.replace("gibson.figure.tape.v1", "gibson.figure.tape.v2");
    assert_eq!(
        TapeReader::new(wrong_format.as_bytes(), &bin).unwrap_err(),
        TapeError::Format
    );
    assert!(matches!(
        TapeReader::new(b"{not json", &bin),
        Err(TapeError::Json(_))
    ));
    assert!(matches!(
        TapeReader::new(b"{}", &bin),
        Err(TapeError::MissingField(_))
    ));
    assert!(matches!(
        reader.frame(2),
        Err(TapeError::FrameOutOfRange { frame: 2, count: 2 })
    ));
    let other_body = tape_header(
        &hex(&[9u8; 32]),
        &rev,
        &["brow_up", "lid_close"],
        2,
        &hex(&sha256::digest(&bin)),
    );
    let other = TapeReader::new(other_body.as_bytes(), &bin).expect("valid tape");
    assert_eq!(
        c.tape_frame(&other, 0).unwrap_err(),
        TapeError::BodyMismatch
    );

    // a joint the rig lacks, and a rig joint the tape lacks
    let mut names = rev.clone();
    names[0] = "tail";
    let unk = tape_header(&body, &names, &["a", "b"], 2, &hex(&sha256::digest(&bin)));
    let ur = TapeReader::new(unk.as_bytes(), &bin).expect("valid tape");
    assert!(matches!(
        c.tape_frame(&ur, 0),
        Err(TapeError::UnknownJoint { .. })
    ));
    let short_names = &rev[1..];
    let mut sf = vec![0.0f32, 0.0, 0.0];
    for _ in 0..4 {
        sf.extend_from_slice(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
    }
    sf.extend_from_slice(&[0.0, 0.0]);
    let sbin = tape_bin(&[sf]);
    let sh = tape_header(
        &body,
        short_names,
        &["a", "b"],
        1,
        &hex(&sha256::digest(&sbin)),
    );
    let sr = TapeReader::new(sh.as_bytes(), &sbin).expect("valid tape");
    assert!(matches!(
        c.tape_frame(&sr, 0),
        Err(TapeError::MissingJoint { .. })
    ));

    // a non-finite value is refused when the frame is read
    let mut nan = frames.clone();
    nan[0][5] = f32::NAN;
    let nbin = tape_bin(&nan);
    let nh = tape_header(&body, &rev, &["a", "b"], 2, &hex(&sha256::digest(&nbin)));
    let nr = TapeReader::new(nh.as_bytes(), &nbin).expect("valid tape");
    assert_eq!(nr.frame(0).unwrap_err(), TapeError::NonFinite);
    assert!(nr.frame(1).is_ok());
}

// ---- the asset loader ----

/// Hostile bytes are typed errors, never panics: every proper prefix of the body file fails, a
/// wrong prefix fails, trailing bytes fail, and a count that cannot fit the file fails before any
/// allocation.
#[test]
fn hostile_asset_bytes_are_typed_errors() {
    for cut in (0..BODY.len()).step_by(41) {
        assert!(
            AssetSet::from_bytes(&BODY[..cut], &[]).is_err(),
            "body prefix {cut} loaded"
        );
    }
    assert!(AssetSet::from_bytes(&BODY[..BODY.len() - 1], &[]).is_err());
    for cut in (0..TUNIC.len()).step_by(13) {
        assert!(
            AssetSet::from_bytes(BODY, &[&TUNIC[..cut]]).is_err(),
            "part prefix {cut} loaded"
        );
    }
    let mut bad = BODY.to_vec();
    bad[0] ^= 0xff;
    assert!(matches!(
        AssetSet::from_bytes(&bad, &[]),
        Err(AssetError::BadMagic { .. })
    ));
    let mut more = BODY.to_vec();
    more.push(0);
    assert!(matches!(
        AssetSet::from_bytes(&more, &[]),
        Err(AssetError::TrailingBytes { extra: 1 })
    ));
    let mut more = TUNIC.to_vec();
    more.push(0);
    assert!(matches!(
        AssetSet::from_bytes(BODY, &[more.as_slice()]),
        Err(AssetError::TrailingBytes { extra: 1 })
    ));
    // n_vertices (offset: prefix, id string, version, grounded flag) set to u32::MAX
    let off = BODY_MAGIC.len() + 4 + "synthetic-puppet".len() + 4 + 1;
    assert_eq!(
        u32_at(BODY, off),
        60,
        "the offset points at the vertex count"
    );
    let mut huge = BODY.to_vec();
    huge[off..off + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        AssetSet::from_bytes(&huge, &[]),
        Err(AssetError::LimitExceeded { .. })
    ));
    // the same part twice
    assert!(matches!(
        AssetSet::from_bytes(BODY, &[TUNIC, TUNIC]),
        Err(AssetError::DuplicatePart { .. })
    ));
    // a part is not a body and a body is not a part
    assert!(AssetSet::from_bytes(TUNIC, &[]).is_err());
    assert!(AssetSet::from_bytes(BODY, &[BODY]).is_err());
}

/// The asset set exposes what the tests and a host need, and `load_dir` verifies the index: a
/// flipped byte in a part file is a sha mismatch, and an index naming a path outside the
/// directory is refused.
#[test]
fn the_asset_set_loads_from_disk_and_verifies_the_index() {
    let a = AssetSet::load_dir(Path::new(FIXTURE_DIR)).expect("loads");
    assert_eq!(a.asset_id(), "synthetic-puppet");
    assert_eq!(a.vertex_count(), 60);
    assert_eq!(a.body_vertex_count(), 32);
    assert_eq!(
        a.control_names(),
        ["stature", "hip_breadth", "bulge", "lean", "sink"]
    );
    assert_eq!(a.control_domain("stature"), Some((-1.0, 1.0, 0.0)));
    assert_eq!(a.joint_count(), 5);
    assert_eq!(a.joint_name(3), Some("neck"));
    assert_eq!(a.part_ids(), ["cap", "plug", "shell", "tabard", "tunic"]);
    assert_eq!(a.part_kind("cap"), Some("hair"));
    assert_eq!(a.part_sha256("tunic"), Some(sha256::digest(TUNIC)));
    assert_eq!(a.body_sha256(), sha256::digest(BODY));
    assert!(a.landmark("hip_l").is_some() && a.landmark("nope").is_none());

    let dir = std::env::temp_dir().join(format!("gibson_figure_studio_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    for f in [
        "index.txt",
        "body.gfa",
        "part.cap.gfa",
        "part.plug.gfa",
        "part.shell.gfa",
        "part.tabard.gfa",
        "part.tunic.gfa",
    ] {
        std::fs::copy(Path::new(FIXTURE_DIR).join(f), dir.join(f)).expect("copy");
    }
    assert!(AssetSet::load_dir(&dir).is_ok());
    let mut tampered = std::fs::read(dir.join("part.cap.gfa")).expect("read");
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    std::fs::write(dir.join("part.cap.gfa"), &tampered).expect("write");
    assert!(matches!(
        AssetSet::load_dir(&dir),
        Err(AssetError::ShaMismatch { .. })
    ));
    let idx = std::fs::read_to_string(Path::new(FIXTURE_DIR).join("index.txt")).expect("index");
    std::fs::write(
        dir.join("index.txt"),
        idx.replace("body.gfa", "../body.gfa"),
    )
    .expect("write");
    assert!(matches!(
        AssetSet::load_dir(&dir),
        Err(AssetError::Index(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The studio preview (ORDER #105): `Compiled::preview` renders the compiled figure offscreen at
/// ≥960×540 with posed smooth normals and the ground shadow, deterministically, and a control
/// change is visible. Control: the same compile renders byte-identically twice, so the difference
/// below is caused by the edit. Must-FAIL: `preview_stale` (a preview that ignores the moved
/// vertices) is killed by the pixel-difference assertion.
#[test]
fn the_studio_preview_shows_a_control_change() {
    use gibson::figure::preview::{bounds_of, frame_bounds, PreviewMesh, PreviewOptions};
    let a = assets();
    let base = compile(&recipe(&[]), &a).expect("compiles");
    let opts = PreviewOptions::default();
    assert!(opts.width >= 960 && opts.height >= 540);
    // one fixed camera for both renders, framed on the base figure
    let rest = base.rest_render_meshes();
    let flat: Vec<(Vec<[f32; 3]>, Vec<u8>)> = rest
        .iter()
        .map(|m| {
            (
                m.positions.iter().map(|v| [v.x, v.y, v.z]).collect(),
                m.slot_per_triangle.clone(),
            )
        })
        .collect();
    let pm: Vec<PreviewMesh<'_>> = rest
        .iter()
        .zip(flat.iter())
        .map(|(m, (p, s))| PreviewMesh {
            positions: p,
            normals: None,
            triangles: &m.triangles,
            slot_per_triangle: s,
        })
        .collect();
    let cam = frame_bounds(
        bounds_of(&pm).expect("bounds"),
        opts.width as f32 / opts.height as f32,
        0.6,
    );
    let img = base
        .preview(&base.rest_frame(), Some(&cam), opts)
        .expect("preview");
    assert_eq!((img.width, img.height), (opts.width, opts.height));
    assert_eq!(img.rgb8.len(), (opts.width * opts.height * 3) as usize);
    let again = base
        .preview(&base.rest_frame(), Some(&cam), opts)
        .expect("preview");
    assert_eq!(img.rgb8, again.rgb8, "the preview is deterministic");
    // the auto-framed preview renders too and is not blank
    let auto = base.preview(&base.rest_frame(), None, opts).expect("auto");
    let first = &auto.rgb8[..3];
    assert!(
        auto.rgb8.chunks(3).any(|p| p != first),
        "auto preview is blank"
    );

    let edited = base.with_control("stature", 1.0).expect("edit");
    let moved = edited
        .preview(&edited.rest_frame(), Some(&cam), opts)
        .expect("preview");
    let changed = img
        .rgb8
        .chunks(3)
        .zip(moved.rgb8.chunks(3))
        .filter(|(p, q)| p != q)
        .count();
    assert!(
        changed > 2000,
        "a control change must change the preview: {changed} pixels"
    );
}
