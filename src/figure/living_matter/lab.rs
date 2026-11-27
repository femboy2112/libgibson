use super::particles::{Link, Particles};
use super::region::{Region, RegionFrame};
use super::*;
use crate::figure::Quat;

fn spring(rate: usize, steps: usize, g: Vec3, history: bool, collide: bool) -> (Vec3, Vec3, f32) {
    let p = [Vec3::ZERO, Vec3::new(0., 0., -0.4)];
    let mut m = Particles::new(
        p.to_vec(),
        vec![0., 2.],
        vec![Link {
            a: 0,
            b: 1,
            length: 0.4,
            compliance: 0.004,
        }],
    )
    .unwrap();
    let mut transient = Vec3::ZERO;
    for i in 0..rate * 6 {
        let mut pins = p;
        if history && (rate..rate + rate / 2).contains(&i) {
            pins[0].x = 0.08 * ((i - rate) as f32 * std::f32::consts::TAU / rate as f32).sin();
        }
        m.advance(1. / rate as f32, steps, 4, g, 5., 0., &pins, |mut v, _| {
            if collide {
                v.z = v.z.max(-0.410);
            }
            v
        })
        .unwrap();
        if i == rate + rate / 2 {
            transient = m.positions()[1];
        }
        assert_eq!(m.positions()[0], pins[0]);
    }
    (transient, m.positions()[1], m.velocities()[1].length())
}

#[test]
fn analytic_gravity_history_and_timestep_mutation_controls() {
    let earth = DynamicsConfig::default().gravity;
    let (_, normal, speed) = spring(30, 4, earth, false, false);
    let (_, zero, _) = spring(30, 4, Vec3::ZERO, false, false);
    assert!(speed < 0.001);
    let loaded = |v: Vec3| (v.z + 0.4196133).abs() < 0.002;
    assert!(loaded(normal));
    assert!(!loaded(zero));
    for rate in [30, 60] {
        for steps in [2, 4, 8] {
            assert!(loaded(spring(rate, steps, earth, false, false).1));
        }
    }
    let (_, side, _) = spring(60, 4, Vec3::new(9.80665, 0., 0.), false, false);
    assert!((side.x - 0.4196133).abs() < 0.002 && side.z.abs() < 0.002);
    let (fresh, _, _) = spring(60, 4, earth, false, false);
    let (moved, settled, _) = spring(60, 4, earth, true, false);
    let history_gate = |a: Vec3, b: Vec3| a.sub(b).length() > 0.001;
    assert!(history_gate(fresh, moved));
    assert!(!history_gate(fresh, fresh));
    assert!(settled.sub(normal).length() < 0.001);
    let (_, floor, _) = spring(30, 4, earth, false, true);
    assert!((floor.z + 0.410).abs() < 0.00001 && normal.z < -0.412);
    println!("gravity normal={normal:?} zero={zero:?} side={side:?} stop_delta_m={} settled_delta_m={} speed={speed}",fresh.sub(moved).length(),settled.sub(normal).length());
}
#[test]
fn identical_cut_different_fabric_and_alias_mutant() {
    // Same four-ring sewn flared panel, suspended by the top in all three runs.
    let (cols, rows) = (16, 5);
    let mut v = Vec::new();
    let mut faces = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let a = c as f32 * std::f32::consts::TAU / cols as f32;
            let rad = 0.12 + r as f32 * 0.028;
            v.push(Vec3::new(
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
    let run = |fabric| {
        let config = DynamicsConfig {
            fabric,
            ..Default::default()
        };
        let mut cloth = cloth::Cloth::new(&v, &faces, config).unwrap();
        let mut output = v.clone();
        for _ in 0..45 {
            output.clone_from(&v);
            cloth.step(&mut output, config, 1. / 30., |p, _| p).unwrap();
        }
        assert_eq!(&output[..cols], &v[..cols]);
        output[output.len() - cols..]
            .iter()
            .map(|p| p.z)
            .sum::<f32>()
            / cols as f32
    };
    let z = [Fabric::Light, Fabric::Medium, Fabric::Structured].map(run);
    let distinct = |values: [f32; 3]| {
        (values[0] - values[1]).abs() > 0.0005 && (values[1] - values[2]).abs() > 0.0005
    };
    println!("same-cut mean hem heights {z:?}");
    assert!(distinct(z));
    assert!(!distinct([z[1]; 3]));
}
#[test]
fn real_surface_contact_and_pose_relative_support() {
    let mut s = contact::Surface::new();
    s.insert(
        &[Vec3::ZERO, Vec3::new(1., 0., 0.), Vec3::new(0., 1., 0.)],
        [[0, 1, 2]].into_iter(),
    );
    let p = Vec3::new(0.2, 0.2, -0.004);
    let valid = |q: Vec3| (q.z - 0.004).abs() < 0.00001;
    assert!(valid(s.project(p, 0.004)));
    assert!(!valid(p));
    let g = DynamicsConfig::default().gravity;
    let upright = RegionFrame {
        origin: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };
    let lean = RegionFrame {
        origin: Vec3::ZERO,
        rotation: Quat::new(0.38268343, 0., 0., 0.9238795),
    };
    let mut r = Region::new(upright, g);
    for _ in 0..180 {
        r.advance(lean, g, 1. / 60., 0.4, 0.5, 0., 1.).unwrap();
    }
    let lean_d = r.displacement();
    for _ in 0..180 {
        r.advance(upright, g, 1. / 60., 0.4, 0.5, 1., 1.).unwrap();
    }
    let support = r.displacement();
    for _ in 0..180 {
        r.advance(upright, g, 1. / 60., 0.4, 0.5, 0., 1.).unwrap();
    }
    assert!(lean_d.y.abs() > 0.005 && support.z > 0.003 && r.displacement().length() < 0.00001);
    println!(
        "lean={lean_d:?} supported={support:?} restored={:?}",
        r.displacement()
    );
}
