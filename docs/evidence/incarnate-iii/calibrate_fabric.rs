//! Offline constitutive calibration. Compiles the actual panel, contact and XPBD implementation.
//! rustc --edition 2021 -O calibrate_fabric.rs -o /chosen/output/calibrate
#![allow(dead_code)]
#[path = "../../../src/figure/math.rs"]
mod math;
use math::Vec3;
#[path = "../../../src/figure/living_matter/cloth.rs"]
mod cloth;
#[path = "../../../src/figure/living_matter/contact.rs"]
mod contact;
#[path = "../../../src/figure/living_matter/particles.rs"]
mod particles;
#[derive(Clone, Copy)]
struct Fabric {
    density: f32,
    stretch: f32,
    bend: f32,
    drag: f32,
    friction: f32,
}
impl Fabric {
    fn properties(self) -> Self {
        self
    }
}
#[derive(Clone, Copy)]
struct DynamicsConfig {
    fabric: Fabric,
    gravity: Vec3,
    substeps: usize,
    iterations: usize,
}
fn sample(cols: usize, rows: usize, fabric: Fabric) -> (f32, f32) {
    let mut points = Vec::new();
    let mut faces = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let a = c as f32 * std::f32::consts::TAU / cols as f32;
            let rad = 0.12 + r as f32 * 0.028;
            points.push(Vec3::new(
                rad * a.cos(),
                rad * a.sin(),
                0.6 - r as f32 * 0.055,
            ));
            if r + 1 < rows {
                let k = (r * cols + c) as u32;
                let j = (r * cols + (c + 1) % cols) as u32;
                let n = cols as u32;
                faces.extend([[k, k + n, j + n], [k, j + n, j]]);
            }
        }
    }
    let cfg = DynamicsConfig {
        fabric,
        gravity: Vec3::new(0., 0., -9.80665),
        substeps: 4,
        iterations: 6,
    };
    let mut cloth = cloth::Cloth::new(&points, &faces, cfg).unwrap();
    let mut xyz = points.clone();
    for _ in 0..45 {
        xyz.clone_from(&points);
        cloth.step(&mut xyz, cfg, 1. / 30., |v, _| v).unwrap();
    }
    let hem = xyz[xyz.len() - cols..].iter().map(|p| p.z).sum::<f32>() / cols as f32;
    let max = xyz
        .iter()
        .zip(&points)
        .map(|(a, b)| a.sub(*b).length())
        .fold(0., f32::max);
    (hem, max)
}
fn main() {
    for (family, density, bend, drag, friction) in [
        ("light", 0.085, 0.5, 1.8, 0.12),
        ("medium", 0.19, 0.045, 3.5, 0.24),
        ("structured", 0.36, 0.0015, 6.5, 0.36),
    ] {
        for stretch in [0.00008, 0.002, 0.014, 0.03, 0.06, 0.12, 0.24, 0.48] {
            let (hem, max) = sample(
                16,
                5,
                Fabric {
                    density,
                    stretch,
                    bend,
                    drag,
                    friction,
                },
            );
            println!("{family},{density},{stretch},{hem:.8},{max:.8}");
        }
    }
}
