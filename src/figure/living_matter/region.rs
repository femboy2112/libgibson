//! A bounded local mode referenced to its gravity-loaded authored shape.
use super::Vec3;

/// A reduced-order region's moving anatomical frame. Rotation maps local into world.
#[derive(Clone, Copy, Debug)]
pub struct RegionFrame {
    pub origin: Vec3,
    pub rotation: crate::figure::Quat,
}

#[derive(Clone, Debug)]
pub struct Region {
    pub(super) displacement: Vec3,
    pub(super) speed: Vec3,
    pub(super) frame: RegionFrame,
    pub(super) last_velocity: Vec3,
    pub(super) reference_force: Vec3,
}

impl Region {
    /// The reference gravity is part of the rest calibration and stays fixed when laboratory gravity changes.
    pub fn new(frame: RegionFrame, reference_gravity: Vec3) -> Self {
        Self {
            displacement: Vec3::ZERO,
            speed: Vec3::ZERO,
            frame,
            last_velocity: Vec3::ZERO,
            reference_force: frame.rotation.conjugate().rotate(reference_gravity),
        }
    }

    pub fn displacement(&self) -> Vec3 {
        self.displacement
    }

    /// Backward Euler for three damped modes. Support adds a parallel spring and a reversible lift.
    /// The unloaded offset of each mode is -reference_force / omega^2.
    #[allow(clippy::too_many_arguments)]
    pub fn advance(
        &mut self,
        frame: RegionFrame,
        gravity: Vec3,
        dt: f32,
        firmness: f32,
        damping: f32,
        support: f32,
        intensity: f32,
    ) -> Result<Vec3, &'static str> {
        if !(1. / 241. ..=0.1).contains(&dt)
            || !gravity.is_finite()
            || !frame.origin.is_finite()
            || !frame.rotation.dot(frame.rotation).is_finite()
            || (frame.rotation.dot(frame.rotation) - 1.).abs() > 0.01
            || ![firmness, damping, support, intensity]
                .iter()
                .all(|v| (0. ..=1.).contains(v))
        {
            return Err("invalid region input");
        }
        let velocity = frame.origin.sub(self.frame.origin).scale(1. / dt);
        let acceleration = velocity.sub(self.last_velocity).scale(1. / dt);
        let force = frame
            .rotation
            .conjugate()
            .rotate(gravity.sub(acceleration))
            .sub(self.reference_force);
        let omega = 12. + 24. * firmness;
        let integrate = |x: f32, v: f32, a: f32, scale: f32, lift: f32| {
            let k = omega * omega * scale;
            let supported = k * (1. + 2.5 * support);
            let c = 2. * supported.sqrt() * (0.18 + 0.82 * damping);
            let v = (v + dt * (a * intensity + k * support * lift - supported * x))
                / (1. + dt * c + dt * dt * supported);
            (x + dt * v, v)
        };
        let x = integrate(self.displacement.x, self.speed.x, force.x, 1.7, 0.);
        let y = integrate(self.displacement.y, self.speed.y, force.y, 1., 0.002);
        let z = integrate(self.displacement.z, self.speed.z, force.z, 0.85, 0.014);
        let mut displacement = Vec3::new(x.0, y.0, z.0);
        let mut speed = Vec3::new(x.1, y.1, z.1);
        if !displacement.is_finite() || !speed.is_finite() {
            return Err("divergent region");
        }
        if displacement.length() > 0.055 {
            let n = displacement.try_normalize().ok_or("invalid displacement")?;
            displacement = n.scale(0.055);
            speed = speed.sub(n.scale(speed.dot(n).max(0.)));
        }
        self.displacement = displacement;
        self.speed = speed;
        self.last_velocity = velocity;
        self.frame = frame;
        Ok(frame.rotation.rotate(displacement))
    }
}
