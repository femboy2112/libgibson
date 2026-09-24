//! Distinct architecture remains a pure realization of the battle model.
#[allow(dead_code)]
#[path = "../examples/acid_vs_crash.rs"]
mod encounter;
use encounter::battle::{EncounterModel, NodeId};
use encounter::world_geom::draw;
use gibson::raster3d::{Camera, Rasterizer};
use gibson::Vec3;

fn image(world: &EncounterModel) -> Rasterizer {
    let mut raster = Rasterizer::new(120, 64);
    let camera = Camera {
        position: Vec3::new(8.0, 9.0, -13.0),
        target: Vec3::new(0.0, 1.0, 1.0),
        ..Camera::default()
    };
    draw(&mut raster, &camera, world, 2.5, Some(NodeId::Auth), 0.8);
    raster
}

#[test]
fn architecture_is_deterministic_filled_and_bounded() {
    let world = EncounterModel::new("first-breach", 71);
    let a = image(&world);
    let b = image(&world);
    assert_eq!(a.raster, b.raster);
    assert!(a.stats.triangles_drawn > 50);
    assert!(a.stats.triangles_submitted < 1200);
    assert!(
        a.raster
            .pixels()
            .iter()
            .filter(|&&p| p != (0, 0, 0))
            .count()
            > 100
    );
    assert!((0..64).any(|y| (0..120).any(|x| a.depth(x, y).is_some_and(f32::is_finite))));
}

#[test]
fn damage_removes_vault_and_storage_pieces_without_changing_control() {
    let intact = EncounterModel::new("quiet", 71);
    let mut damaged = intact.clone();
    damaged.graph.nodes[NodeId::Auth.index()].integrity = 600;
    damaged.graph.nodes[NodeId::Files.index()].integrity = 600;
    let a = image(&intact);
    let b = image(&damaged);
    assert_ne!(a.raster, b.raster);
    assert_ne!(b.stats.triangles_submitted, a.stats.triangles_submitted);
    assert_eq!(
        intact.graph.node(NodeId::Auth).influence,
        damaged.graph.node(NodeId::Auth).influence
    );
}

#[test]
fn mirror_and_isolation_reconfigure_the_same_architecture() {
    let base = EncounterModel::new("contest", 71);
    let mut mirror = base.clone();
    mirror.command("decoy");
    let mut isolated = base.clone();
    isolated.command("isolate");
    assert!(mirror.graph.node(NodeId::Decoy).visible);
    let a = image(&base);
    assert_ne!(a.raster, image(&mirror).raster);
    assert_ne!(a.raster, image(&isolated).raster);
}
