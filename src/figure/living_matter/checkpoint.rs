//! Little-endian checkpoint v1. Restore into an already compiled, matching actor.
use super::*;
use crate::figure::living_matter::particles::Particles;

struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], &'static str> {
        let (v, rest) = self.0.split_at_checked(N).ok_or("truncated checkpoint")?;
        self.0 = rest;
        Ok(v.try_into().unwrap())
    }
    fn scalar(&mut self) -> Result<f32, &'static str> {
        let n = f32::from_le_bytes(self.bytes()?);
        if n.is_finite() {
            Ok(n)
        } else {
            Err("nonfinite checkpoint")
        }
    }
    fn vector(&mut self) -> Result<Vec3, &'static str> {
        Ok(Vec3::new(self.scalar()?, self.scalar()?, self.scalar()?))
    }
}
fn vec_to(out: &mut Vec<u8>, v: Vec3) {
    for n in [v.x, v.y, v.z] {
        out.extend(n.to_le_bytes());
    }
}
fn particles_to(out: &mut Vec<u8>, p: &Particles) {
    out.extend((p.positions.len() as u32).to_le_bytes());
    for group in [&p.positions, &p.velocities, &p.previous_pins] {
        for &v in group {
            vec_to(out, v);
        }
    }
}
fn particles_from(r: &mut Reader<'_>, p: &mut Particles) -> Result<(), &'static str> {
    if u32::from_le_bytes(r.bytes()?) as usize != p.positions.len() {
        return Err("checkpoint topology mismatch");
    }
    for group in [&mut p.positions, &mut p.velocities, &mut p.previous_pins] {
        for v in group {
            *v = r.vector()?;
        }
    }
    Ok(())
}
impl FigureDynamics {
    /// Serializable continuation for this solver version, actor and configuration. Includes all
    /// inertial history; constraint multipliers are reset at each fixed substep.
    pub fn checkpoint_bytes(&self) -> Vec<u8> {
        let mut out = b"GLMCKP01".to_vec();
        out.extend(self.id);
        out.extend(self.present);
        out.extend(self.tick.to_le_bytes());
        out.extend(self.rate.unwrap_or(0).to_le_bytes());
        for r in &self.regions {
            for v in [r.displacement, r.speed, r.frame.origin, r.last_velocity] {
                vec_to(&mut out, v);
            }
            let q = r.frame.rotation;
            for n in [q.x, q.y, q.z, q.w] {
                out.extend(n.to_le_bytes());
            }
        }
        if let Some((_, c)) = &self.cloth {
            particles_to(&mut out, &c.particles);
        }
        if let Some((_, h)) = &self.hair {
            particles_to(&mut out, &h.particles);
        }
        out.extend(sha256::digest(&out));
        out
    }
    /// Restore atomically. Unsupported versions, damage, wrong identities and invalid state fail.
    pub fn restore_bytes(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        if bytes.len() < 32 || bytes.len() > 1024 * 1024 {
            return Err("invalid checkpoint length");
        }
        let (data, checksum) = bytes.split_at(bytes.len() - 32);
        if sha256::digest(data) != checksum {
            return Err("checkpoint digest mismatch");
        }
        let mut r = Reader(data);
        if &r.bytes::<8>()? != b"GLMCKP01"
            || r.bytes::<32>()? != self.id
            || r.bytes::<32>()? != self.present
        {
            return Err("checkpoint identity or version mismatch");
        }
        let mut next = self.clone();
        next.tick = u64::from_le_bytes(r.bytes()?);
        let rate = u32::from_le_bytes(r.bytes()?);
        next.rate = match (next.tick, rate) {
            (0, 0) => None,
            (1.., 10..=240) => Some(rate),
            _ => return Err("invalid checkpoint clock"),
        };
        for reg in &mut next.regions {
            reg.displacement = r.vector()?;
            reg.speed = r.vector()?;
            reg.frame.origin = r.vector()?;
            reg.last_velocity = r.vector()?;
            let q = Quat {
                x: r.scalar()?,
                y: r.scalar()?,
                z: r.scalar()?,
                w: r.scalar()?,
            };
            if (q.dot(q) - 1.).abs() > 0.01 || reg.displacement.length() > 0.0551 {
                return Err("invalid regional checkpoint");
            }
            reg.frame.rotation = q;
        }
        if let Some((_, c)) = &mut next.cloth {
            particles_from(&mut r, &mut c.particles)?;
        }
        if let Some((_, h)) = &mut next.hair {
            particles_from(&mut r, &mut h.particles)?;
        }
        if !r.0.is_empty() {
            return Err("trailing checkpoint data");
        }
        *self = next;
        Ok(())
    }
}
