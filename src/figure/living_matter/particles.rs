//! Fixed-step, scalar XPBD distance constraints. A callback supplies moving pin targets
//! and contact geometry in metres. The caller owns the wall clock and any hidden-state policy.
use super::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub a: usize,
    pub b: usize,
    pub length: f32,
    /// Inverse stiffness (m/N).
    pub compliance: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Capsule {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

impl Capsule {
    pub fn project(&self, point: Vec3, thickness: f32) -> Vec3 {
        let line = self.b.sub(self.a);
        let along = (point.sub(self.a).dot(line) / line.dot(line).max(1e-12)).clamp(0., 1.);
        let center = self.a.add(line.scale(along));
        let delta = point.sub(center);
        let radius = self.radius + thickness;
        if delta.dot(delta) >= radius * radius {
            return point;
        }
        center.add(
            delta
                .try_normalize()
                .unwrap_or(Vec3::new(0., -1., 0.))
                .scale(radius),
        )
    }
}

/// All order-sensitive arrays use their stored order. Lambdas start at zero per substep.
#[derive(Clone, Debug)]
pub struct Particles {
    inv_mass: Vec<f32>,
    links: Vec<Link>,
    pub(super) positions: Vec<Vec3>,
    pub(super) velocities: Vec<Vec3>,
    previous_pins: Vec<Vec3>,
}

impl Particles {
    pub fn new(
        rest: Vec<Vec3>,
        inv_mass: Vec<f32>,
        links: Vec<Link>,
    ) -> Result<Self, &'static str> {
        if rest.is_empty()
            || rest.len() > 8192
            || rest.len() != inv_mass.len()
            || links.len() > 65536
            || !rest.iter().all(|p| p.is_finite())
            || !inv_mass.iter().all(|m| m.is_finite() && *m >= 0.)
            || !links.iter().all(|l| {
                l.a < rest.len()
                    && l.b < rest.len()
                    && l.a != l.b
                    && l.length.is_finite()
                    && l.length > 0.
                    && l.compliance.is_finite()
                    && l.compliance >= 0.
            })
        {
            return Err("invalid particle model");
        }
        Ok(Self {
            previous_pins: rest.clone(),
            velocities: vec![Vec3::ZERO; rest.len()],
            positions: rest,
            inv_mass,
            links,
        })
    }

    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    pub fn velocities(&self) -> &[Vec3] {
        &self.velocities
    }

    /// Advance a single presentation frame with a bounded number of equal substeps.
    /// Results are committed atomically. Contact projects a predicted point to free space.
    #[allow(clippy::too_many_arguments)]
    pub fn advance<F>(
        &mut self,
        frame_dt: f32,
        substeps: usize,
        iterations: usize,
        gravity: Vec3,
        drag: f32,
        friction: f32,
        pins: &[Vec3],
        project: F,
    ) -> Result<(), &'static str>
    where
        F: Fn(Vec3, f32) -> Vec3,
    {
        if !(1..=16).contains(&substeps)
            || !(1..=32).contains(&iterations)
            || !(1. / 241. ..=0.1).contains(&frame_dt)
            || !gravity.is_finite()
            || gravity.length() > 100.
            || !(0. ..=80.).contains(&drag)
            || !(0. ..=1.).contains(&friction)
            || pins.len() != self.positions.len()
            || !pins.iter().all(|p| p.is_finite())
        {
            return Err("invalid solver input");
        }
        let dt = frame_dt / substeps as f32;
        let mut pos = self.positions.clone();
        let mut velocity = self.velocities.clone();
        let mut lambda = vec![0.; self.links.len()];
        let mut normals = vec![Vec3::ZERO; pos.len()];
        for step in 1..=substeps {
            let t = step as f32 / substeps as f32;
            let old = pos.clone();
            lambda.fill(0.);
            normals.fill(Vec3::ZERO);
            for (i, p) in pos.iter_mut().enumerate() {
                if self.inv_mass[i] == 0. {
                    *p = self.previous_pins[i].add(pins[i].sub(self.previous_pins[i]).scale(t));
                } else {
                    velocity[i] = velocity[i].scale((-drag * dt).exp()).add(gravity.scale(dt));
                    *p = p.add(velocity[i].scale(dt));
                }
            }
            for _ in 0..iterations {
                for (l, total) in self.links.iter().zip(&mut lambda) {
                    let delta = pos[l.b].sub(pos[l.a]);
                    let length = delta.length();
                    let weight = self.inv_mass[l.a] + self.inv_mass[l.b];
                    if length < 1e-8 || weight == 0. {
                        continue;
                    }
                    let alpha = l.compliance / (dt * dt);
                    let dl = (-(length - l.length) - alpha * *total) / (weight + alpha);
                    *total += dl;
                    let dx = delta.scale(dl / length);
                    pos[l.a] = pos[l.a].sub(dx.scale(self.inv_mass[l.a]));
                    pos[l.b] = pos[l.b].add(dx.scale(self.inv_mass[l.b]));
                }
                for (i, p) in pos.iter_mut().enumerate() {
                    if self.inv_mass[i] > 0. {
                        let next = project(*p, t);
                        if let Some(n) = next.sub(*p).try_normalize() {
                            normals[i] = n;
                        }
                        *p = next;
                    }
                }
            }
            for i in 0..pos.len() {
                let mut v = pos[i].sub(old[i]).scale(1. / dt);
                let n = normals[i];
                if n != Vec3::ZERO {
                    let vn = n.scale(v.dot(n));
                    v = v
                        .sub(vn)
                        .scale(1. - friction)
                        .add(n.scale(v.dot(n).max(0.)));
                }
                if !v.is_finite() || !pos[i].is_finite() || v.length() > 500. {
                    return Err("divergent particle step");
                }
                velocity[i] = v;
            }
        }
        self.positions = pos;
        self.velocities = velocity;
        self.previous_pins.clone_from_slice(pins);
        Ok(())
    }
}
