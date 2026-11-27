//! A ribbon is driven by its nearest rooted guide in the rest hairstyle. The guides
//! collide with the posed body, and vertices above the guide roots stay attached.
use super::{
    particles::{Link, Particles},
    DynamicsConfig, Vec3,
};

#[derive(Clone, Debug)]
pub(super) struct Hair {
    particles: Particles,
    ids: Vec<usize>,
    map: Vec<(usize, usize, f32, f32)>,
}

impl Hair {
    pub(super) fn new(rest: &[Vec3]) -> Result<Self, &'static str> {
        let top = rest.iter().map(|v| v.z).fold(f32::NEG_INFINITY, f32::max);
        let bottom = rest.iter().map(|v| v.z).fold(f32::INFINITY, f32::min);
        let height = top - bottom;
        if height < 0.02 {
            return Err("hair too short for a hanging guide");
        }
        let center = rest
            .iter()
            .copied()
            .fold(Vec3::ZERO, Vec3::add)
            .scale(1. / rest.len() as f32);
        let root_z = top - height * 0.32;
        let mut guides = Vec::new();
        let mut ids = Vec::new();
        let mut links = Vec::new();
        let mut mass = Vec::new();
        const LEVELS: usize = 5;
        const SECTORS: usize = 8;
        for sector in 0..SECTORS {
            let ang = std::f32::consts::TAU * sector as f32 / SECTORS as f32;
            let axis = Vec3::new(ang.cos(), ang.sin(), 0.);
            let mut sector_min = f32::INFINITY;
            for p in rest {
                let xy = Vec3::new(p.x - center.x, p.y - center.y, 0.);
                if xy.try_normalize().is_some_and(|u| u.dot(axis) > 0.7) {
                    sector_min = sector_min.min(p.z);
                }
            }
            let end = if sector_min.is_finite() {
                sector_min.min(root_z - 0.01)
            } else {
                bottom
            };
            for level in 0..LEVELS {
                let target_z = root_z + (end - root_z) * level as f32 / (LEVELS - 1) as f32;
                let target = Vec3::new(
                    center.x + 0.105 * axis.x,
                    center.y + 0.105 * axis.y,
                    target_z,
                );
                let idx = (0..rest.len())
                    .min_by(|&a, &b| {
                        let score = |p: Vec3| {
                            let d = p.sub(target);
                            d.dot(d) + 2. * d.z * d.z
                        };
                        score(rest[a]).total_cmp(&score(rest[b]))
                    })
                    .ok_or("empty hair")?;
                let i = guides.len();
                guides.push(rest[idx]);
                ids.push(idx);
                mass.push(if level == 0 { 0. } else { 300. });
                if level >= 1 {
                    for (back, soft) in [(1, 0.0006), (2, 0.015)] {
                        if level >= back {
                            let length = guides[i].sub(guides[i - back]).length();
                            if length > 1e-6 {
                                links.push(Link {
                                    a: i - back,
                                    b: i,
                                    length,
                                    compliance: soft,
                                });
                            }
                        }
                    }
                }
            }
        }
        let mut map = Vec::with_capacity(rest.len());
        for p in rest {
            let angle = (p.y - center.y)
                .atan2(p.x - center.x)
                .rem_euclid(std::f32::consts::TAU);
            let start = (((angle / std::f32::consts::TAU * SECTORS as f32).round() as usize)
                % SECTORS)
                * LEVELS;
            let range = (root_z - guides[start + LEVELS - 1].z).max(0.01);
            let t = ((root_z - p.z) / range).clamp(0., 1.) * (LEVELS - 1) as f32;
            let k = (t.floor() as usize).min(LEVELS - 2);
            map.push((
                start + k,
                start + k + 1,
                t - k as f32,
                ((root_z - p.z) / 0.06).clamp(0., 1.),
            ));
        }
        Ok(Self {
            particles: Particles::new(guides, mass, links)?,
            ids,
            map,
        })
    }
    pub(super) fn step(
        &mut self,
        vertices: &mut [Vec3],
        cfg: DynamicsConfig,
        dt: f32,
        project: impl Fn(Vec3, f32) -> Vec3,
    ) -> Result<(), &'static str> {
        let pins: Vec<_> = self.ids.iter().map(|&i| vertices[i]).collect();
        self.particles.advance(
            dt,
            cfg.substeps,
            cfg.iterations,
            cfg.gravity,
            6.,
            0.18,
            &pins,
            project,
        )?;
        let delta: Vec<_> = self
            .particles
            .positions()
            .iter()
            .zip(pins)
            .map(|(a, b)| a.sub(b))
            .collect();
        for (p, &(a, b, t, w)) in vertices.iter_mut().zip(&self.map) {
            let d = delta[a].scale(1. - t).add(delta[b].scale(t));
            *p = p.add(d.scale(w * cfg.intensity));
        }
        Ok(())
    }
}
