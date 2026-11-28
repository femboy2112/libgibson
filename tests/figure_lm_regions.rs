//! Declared Living Matter regions on the SYNTHETIC studio fixture (`tests/fixtures/figure_studio`,
//! no private data): calibration at the first admitted frame, a held pose is exactly the
//! sculpt, the identity of a configuration without declared regions is unchanged, specs are part
//! of the identity, an inert spec is inert, stale or malformed specs are refused, and a declared
//! actor resumes exactly from a checkpoint.
//!
//! Controls: every positive claim has a case that must fail (a moving region does move the body;
//! a spec measured on a body with a different vertex count is refused, not ignored).
#![cfg(feature = "figure")]

use gibson::figure::living_matter::{
    DynamicsConfig, Falloff, FigureDynamics, Partition, RegionSpec, RegionSpecError,
};
use gibson::figure::math::{Quat, Transform, Vec3};
use gibson::figure::sha256::Sha256;
use gibson::figure::studio::{
    compile, AssetSet, BodyRevId, Compiled, MeshRole, RecipeV1, TapeFrame,
};

const BODY: &[u8] = include_bytes!("fixtures/figure_studio/body.gfa");
const CAP: &[u8] = include_bytes!("fixtures/figure_studio/part.cap.gfa");
const PLUG: &[u8] = include_bytes!("fixtures/figure_studio/part.plug.gfa");
const SHELL: &[u8] = include_bytes!("fixtures/figure_studio/part.shell.gfa");
const TABARD: &[u8] = include_bytes!("fixtures/figure_studio/part.tabard.gfa");
const TUNIC: &[u8] = include_bytes!("fixtures/figure_studio/part.tunic.gfa");

const FPS: u32 = 30;

fn compiled() -> Compiled {
    let assets =
        AssetSet::from_bytes(BODY, &[CAP, PLUG, SHELL, TABARD, TUNIC]).expect("the fixture loads");
    compile(&RecipeV1::new(), &assets).expect("compiles")
}

fn spine(c: &Compiled) -> usize {
    c.skeleton().joint_index("spine").expect("spine joint")
}

/// A spec over the three upper rings (z 0.5 .. 1.5) in the frame of the spine joint.
fn spec(c: &Compiled, name: &str, scale: f32, partition: Option<Partition>) -> RegionSpec {
    let positions = c.rest_render_meshes()[0].positions.clone();
    let j = spine(c);
    let verts: Vec<u32> = (8..32).collect();
    let falloff = Falloff { scale, partition };
    RegionSpec::measure(name, j, &verts, &positions, &c.inverse_binds()[j], falloff)
        .expect("a valid region")
}

/// The rest frame with the spine's local rotation turned by `angle` about x.
fn tilted(c: &Compiled, joint: usize, angle: f32) -> TapeFrame {
    let mut f = c.rest_frame();
    let t = f.locals[joint];
    f.locals[joint] = Transform::new(
        t.translation,
        Quat::from_axis_angle(Vec3::new(1., 0., 0.), angle).mul(t.rotation),
        t.scale,
    );
    f
}

/// The configuration identity of an actor with no declared regions, rebuilt by hand from the
/// byte layout `gibson.living-matter.config.v3` (the layout that existed before declared
/// regions): a change to those bytes is a change to every saved checkpoint identity.
fn legacy_config_id(c: &Compiled, cfg: DynamicsConfig) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"gibson.living-matter.config.v3\0");
    h.update(&c.body_rev_id().0);
    h.update(&c.fit_id().0);
    h.update(&[cfg.fabric as u8, cfg.substeps as u8, cfg.iterations as u8]);
    let fabric = cfg.fabric.properties();
    for coefficient in [
        fabric.density,
        fabric.stretch,
        fabric.bend,
        fabric.drag,
        fabric.friction,
    ] {
        h.update(&coefficient.to_le_bytes());
    }
    for n in [
        cfg.gravity.x,
        cfg.gravity.y,
        cfg.gravity.z,
        cfg.firmness,
        cfg.damping,
        cfg.support,
        cfg.intensity,
    ] {
        h.update(&n.to_le_bytes());
    }
    for m in c
        .rest_render_meshes()
        .iter()
        .filter(|m| m.role == MeshRole::Hair)
    {
        h.update(&(m.id.len() as u64).to_le_bytes());
        h.update(m.id.as_bytes());
        h.update(&(m.positions.len() as u64).to_le_bytes());
        for p in &m.positions {
            for n in [p.x, p.y, p.z] {
                h.update(&n.to_le_bytes());
            }
        }
    }
    let inner = h.finalize();
    let mut outer = Sha256::new();
    outer.update(&inner);
    outer.finalize()
}

#[test]
fn no_declared_regions_leaves_the_identity_byte_identical() {
    let c = compiled();
    let cfg = DynamicsConfig::default();
    let plain = FigureDynamics::new(&c, cfg).unwrap().config_id();
    assert_eq!(plain, legacy_config_id(&c, cfg));
    let hex: String = plain.iter().map(|b| format!("{b:02x}")).collect();
    println!("no-region config_id on the fixture: {hex}");
    let empty = FigureDynamics::with_regions(&c, cfg, Vec::new())
        .unwrap()
        .config_id();
    assert_eq!(empty, plain);
    // control: a declared region changes the identity, and so does every field of the declaration
    let one = FigureDynamics::with_regions(&c, cfg, vec![spec(&c, "a", 1.25, None)])
        .unwrap()
        .config_id();
    assert_ne!(one, plain);
    let other_scale = FigureDynamics::with_regions(&c, cfg, vec![spec(&c, "a", 1.5, None)])
        .unwrap()
        .config_id();
    let other_name = FigureDynamics::with_regions(&c, cfg, vec![spec(&c, "b", 1.25, None)])
        .unwrap()
        .config_id();
    assert!(one != other_scale && one != other_name);
    assert_eq!(
        FigureDynamics::with_regions(&c, cfg, vec![spec(&c, "a", 1.25, None)])
            .unwrap()
            .region_names(),
        vec!["a".to_string()]
    );
    assert_eq!(
        FigureDynamics::new(&c, cfg).unwrap().region_names(),
        vec!["left".to_string(), "right".to_string()]
    );
}

#[test]
fn a_held_pose_is_exactly_the_sculpt() {
    let c = compiled();
    let cfg = DynamicsConfig::default();
    let mut d = FigureDynamics::with_regions(&c, cfg, vec![spec(&c, "r", 1.25, None)]).unwrap();
    let frame = c.rest_frame();
    let rest = c.render_meshes(&frame).unwrap();
    for _ in 0..120 {
        let out = d.step(&c, &frame, FPS).unwrap();
        assert_eq!(out[0].positions, rest[0].positions);
    }
    assert_eq!(d.region_displacements(), vec![Vec3::ZERO]);
}

#[test]
fn calibration_is_taken_at_the_first_frame_whatever_its_orientation() {
    let c = compiled();
    let cfg = DynamicsConfig::default();
    let j = spine(&c);
    let mut d = FigureDynamics::with_regions(&c, cfg, vec![spec(&c, "r", 1.25, None)]).unwrap();
    let frame = tilted(&c, j, 0.26); // about 15 degrees off the bind pose, held
    let rest = c.render_meshes(&frame).unwrap();
    for _ in 0..120 {
        let out = d.step(&c, &frame, FPS).unwrap();
        assert_eq!(out[0].positions, rest[0].positions);
    }
    assert_eq!(d.region_displacements(), vec![Vec3::ZERO]);
}

#[test]
fn upright_stand_with_a_tiny_lean_settles_below_a_micrometre() {
    let c = compiled();
    let cfg = DynamicsConfig::default();
    let j = spine(&c);
    let mut d = FigureDynamics::with_regions(&c, cfg, vec![spec(&c, "r", 1.25, None)]).unwrap();
    d.step(&c, &c.rest_frame(), FPS).unwrap();
    let lean = tilted(&c, j, 2.0e-5);
    let mut out = Vec::new();
    for _ in 0..120 {
        out = d.step(&c, &lean, FPS).unwrap();
    }
    let rest = c.render_meshes(&lean).unwrap();
    let worst = out[0]
        .positions
        .iter()
        .zip(&rest[0].positions)
        .map(|(a, b)| a.sub(*b).length())
        .fold(0.0f32, f32::max);
    assert!(worst <= 1.0e-6, "settled {worst} m from the sculpt");
    let u = d.region_displacements()[0].length();
    assert!(u <= 1.0e-6, "region displacement {u} m");
}

#[test]
fn an_inert_spec_moves_nothing_and_a_live_spec_does() {
    let c = compiled();
    let cfg = DynamicsConfig::default();
    let j = spine(&c);
    let mut inert_spec = spec(&c, "r", 1.25, None);
    inert_spec.half_extent = Vec3::ZERO; // zero radius
    let mut inert = FigureDynamics::with_regions(&c, cfg, vec![inert_spec]).unwrap();
    let mut live = FigureDynamics::with_regions(&c, cfg, vec![spec(&c, "r", 1.25, None)]).unwrap();
    let mut last_inert = Vec::new();
    let mut last_live = Vec::new();
    let mut last_frame = c.rest_frame();
    for i in 0..30 {
        let angle = 0.3 * (i as f32 / 29.0);
        last_frame = tilted(&c, j, angle);
        last_inert = inert.step(&c, &last_frame, FPS).unwrap();
        last_live = live.step(&c, &last_frame, FPS).unwrap();
    }
    let reference = c.render_meshes(&last_frame).unwrap();
    // the region itself still responds ...
    assert!(inert.region_displacements()[0].length() > 1.0e-4);
    // ... but an inert declaration displaces no vertex, exactly
    assert_eq!(last_inert[0].positions, reference[0].positions);
    // control: the same motion with a live declaration does move the surface
    let moved = last_live[0]
        .positions
        .iter()
        .zip(&reference[0].positions)
        .map(|(a, b)| a.sub(*b).length())
        .fold(0.0f32, f32::max);
    assert!(moved > 1.0e-5, "live region moved the surface by {moved} m");
}

#[test]
fn stale_or_malformed_specs_are_refused() {
    let c = compiled();
    let cfg = DynamicsConfig::default();
    let n = c.rest_render_meshes()[0].positions.len();
    // measured on another body: the L1.1 (13380 vertices) against a different compiled body
    let mut old = spec(&c, "old", 1.25, None);
    old.vertex_count = 13380;
    match FigureDynamics::with_regions(&c, cfg, vec![old]) {
        Err(RegionSpecError::VertexCountMismatch { spec, body, .. }) => {
            assert_eq!((spec, body), (13380, n));
        }
        other => panic!("expected a vertex count refusal, got {other:?}"),
    }
    // bound to another compiled body revision
    let stale = spec(&c, "stale", 1.25, None).bound_to(BodyRevId([7; 32]));
    assert!(matches!(
        FigureDynamics::with_regions(&c, cfg, vec![stale]),
        Err(RegionSpecError::StaleBody { .. })
    ));
    // control: bound to this body it is accepted
    let fresh = spec(&c, "fresh", 1.25, None).bound_to(c.body_rev_id());
    assert!(FigureDynamics::with_regions(&c, cfg, vec![fresh]).is_ok());
    // a joint outside the skeleton
    let mut joint = spec(&c, "joint", 1.25, None);
    joint.joint = c.skeleton().joint_count();
    assert!(matches!(
        FigureDynamics::with_regions(&c, cfg, vec![joint]),
        Err(RegionSpecError::JointOutOfRange { .. })
    ));
    // a duplicate name, and a non-finite extent
    assert!(matches!(
        FigureDynamics::with_regions(
            &c,
            cfg,
            vec![spec(&c, "x", 1.25, None), spec(&c, "x", 1.25, None)]
        ),
        Err(RegionSpecError::DuplicateName { .. })
    ));
    let mut nan = spec(&c, "nan", 1.25, None);
    nan.half_extent.x = f32::NAN;
    assert!(matches!(
        FigureDynamics::with_regions(&c, cfg, vec![nan]),
        Err(RegionSpecError::BadFalloff { .. })
    ));
}

#[test]
fn a_declared_actor_resumes_exactly_from_its_checkpoint() {
    let c = compiled();
    let cfg = DynamicsConfig::default();
    let j = spine(&c);
    let make = || {
        FigureDynamics::with_regions(
            &c,
            cfg,
            vec![
                spec(&c, "a", 1.25, None),
                spec(
                    &c,
                    "b",
                    1.0,
                    Some(Partition {
                        normal: Vec3::new(1., 0., 0.),
                        offset: 0.0,
                        width: 0.03,
                    }),
                ),
            ],
        )
        .unwrap()
    };
    let frame = |i: usize| tilted(&c, j, 0.2 * (i as f32 * 0.4).sin());
    let mut a = make();
    for i in 0..20 {
        a.step(&c, &frame(i), FPS).unwrap();
    }
    let bytes = a.checkpoint_bytes();
    let mut b = make(); // fresh: tick 0, uncalibrated
    b.restore_bytes(&bytes).unwrap();
    for i in 20..40 {
        let out_a = a.step(&c, &frame(i), FPS).unwrap();
        let out_b = b.step(&c, &frame(i), FPS).unwrap();
        assert_eq!(out_a[0].positions, out_b[0].positions);
    }
    assert_eq!(a.region_displacements(), b.region_displacements());
    assert_eq!(a.checkpoint_bytes(), b.checkpoint_bytes());
    // control: a checkpoint of an actor with different declarations is refused
    let mut other = FigureDynamics::with_regions(&c, cfg, vec![spec(&c, "a", 1.25, None)]).unwrap();
    assert!(other.restore_bytes(&bytes).is_err());
}
