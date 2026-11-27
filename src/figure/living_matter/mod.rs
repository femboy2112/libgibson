//! Experimental Living Matter. Metres, seconds, kilograms and Z-up world forces.
//! State is separate from compiled morphology, garment fit and presentation identity.
//! A presentation frame is driven by a fixed-rate sequence of admitted poses.
pub mod particles;
pub mod region;
use super::math::Vec3;

/// Material properties for one cut. Density is kg/m², stretch and bend are m/N,
/// drag is s⁻¹, and friction is a dimensionless contact parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fabric {
    Light,
    Medium,
    Structured,
}

#[derive(Clone, Copy, Debug)]
pub struct FabricProperties {
    pub density: f32,
    pub stretch: f32,
    pub bend: f32,
    pub drag: f32,
    pub friction: f32,
}

impl Fabric {
    pub fn properties(self) -> FabricProperties {
        let (density, stretch, bend, drag, friction) = match self {
            Self::Light => (0.085, 0.00012, 0.50, 1.8, 0.12),
            Self::Medium => (0.19, 0.00004, 0.045, 3.5, 0.24),
            Self::Structured => (0.36, 0.00001, 0.0015, 6.5, 0.36),
        };
        FabricProperties {
            density,
            stretch,
            bend,
            drag,
            friction,
        }
    }
}

/// Candidate solver parameters. Configuration changes require a new physical state.
#[derive(Clone, Copy, Debug)]
pub struct DynamicsConfig {
    pub gravity: Vec3,
    pub fabric: Fabric,
    pub firmness: f32,
    pub damping: f32,
    pub support: f32,
    pub intensity: f32,
    pub substeps: usize,
    pub iterations: usize,
}

impl Default for DynamicsConfig {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0., 0., -9.80665),
            fabric: Fabric::Medium,
            firmness: 0.45,
            damping: 0.45,
            support: 0.,
            intensity: 1.,
            substeps: 4,
            iterations: 6,
        }
    }
}

impl DynamicsConfig {
    pub fn validate(self) -> Result<(), &'static str> {
        if !self.gravity.is_finite()
            || self.gravity.length() > 100.
            || ![self.firmness, self.damping, self.support, self.intensity]
                .iter()
                .all(|v| (0. ..=1.).contains(v))
            || !(1..=16).contains(&self.substeps)
            || !(1..=32).contains(&self.iterations)
        {
            return Err("invalid Living Matter configuration");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::particles::{Capsule, Link, Particles};
    use super::region::{Region, RegionFrame};
    use super::*;
    use crate::figure::Quat;
    #[test]
    fn hanging_mass_matches_hooke_equilibrium_and_can_replay_from_checkpoint() {
        let pins = [Vec3::ZERO, Vec3::new(0., 0., -0.4)];
        let mut model = Particles::new(
            pins.to_vec(),
            vec![0., 2.],
            vec![Link {
                a: 0,
                b: 1,
                length: 0.4,
                compliance: 0.004,
            }],
        )
        .unwrap();
        let run = |state: &mut Particles, g| {
            state
                .advance(
                    1. / 30.,
                    4,
                    4,
                    Vec3::new(0., 0., g),
                    5.,
                    0.,
                    &pins,
                    |p, _| p,
                )
                .unwrap();
        };
        let mut zero = model.clone();
        for _ in 0..180 {
            run(&mut model, -9.80665);
            run(&mut zero, 0.);
        }
        assert!((model.positions()[1].z - (-0.4 - 0.5 * 9.80665 * 0.004)).abs() < 0.002);
        assert_eq!(zero.positions()[1], pins[1]);
        assert_eq!(model.positions()[0], pins[0]);
        let mut resume = model.clone();
        run(&mut model, 5.);
        run(&mut resume, 5.);
        assert_eq!(model.positions(), resume.positions());
        let before = model.positions().to_vec();
        assert!(model
            .advance(1. / 30., 4, 4, Vec3::ZERO, 5., 0., &pins, |_, _| Vec3::new(
                f32::NAN,
                0.,
                0.
            ))
            .is_err());
        assert_eq!(before, model.positions());
    }

    #[test]
    fn region_loaded_equilibrium_rotated_gravity_and_reversible_support() {
        let g = DynamicsConfig::default().gravity;
        let pose = RegionFrame {
            origin: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        };
        let mut mode = Region::new(pose, g);
        let settle = |m: &mut Region, gravity, support| {
            for _ in 0..240 {
                m.advance(pose, gravity, 1. / 60., 0.4, 0.5, support, 1.)
                    .unwrap();
            }
            m.displacement()
        };
        assert_eq!(settle(&mut mode, g, 0.), Vec3::ZERO);
        let side = settle(&mut mode, Vec3::new(9.80665, 0., 0.), 0.);
        assert!(side.x > 0.008 && side.z > 0.01);
        let supported = settle(&mut mode, g, 1.);
        assert!(supported.z > 0.003 && supported.y > 0.);
        assert!(settle(&mut mode, g, 0.).length() < 1e-5);
        let collider = Capsule {
            a: Vec3::ZERO,
            b: Vec3::new(0., 0., 1.),
            radius: 0.12,
        };
        let inside = Vec3::new(0.05, 0., 0.5);
        assert!((collider.project(inside, 0.003).x - 0.123).abs() < 1e-6);
    }
}
