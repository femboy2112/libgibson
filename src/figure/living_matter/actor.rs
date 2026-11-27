//! Maps physical proxies onto a compiled, posed character. The authored mesh stays immutable.
use super::{
    cloth::Cloth,
    contact::Surface,
    hair::Hair,
    region::{Region, RegionFrame},
    DynamicsConfig, Vec3,
};
use crate::figure::{
    sha256,
    studio::{Compiled, MeshRole, RenderMesh, TapeFrame},
    DualQuat, Quat,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
struct Weight {
    j: usize,
    vertex: usize,
    w: f32,
    gradient: Vec3,
    r: Vec3,
}

#[derive(Clone, Debug)]
pub struct FigureDynamics {
    config: DynamicsConfig,
    id: [u8; 32],
    props: Vec<(Vec3, Vec3)>,
    present: [u8; 32],
    tick: u64,
    chest: usize,
    centers: [Vec3; 2],
    regions: [Region; 2],
    fields: Arc<Vec<Vec<Weight>>>,
    cloth: Option<(usize, Cloth)>,
    hair: Option<(usize, Hair)>,
}

/// An explicit same-build resume point, including velocity, previous anchors and region acceleration history.
#[derive(Clone, Debug)]
pub struct MatterCheckpoint {
    state: FigureDynamics,
}

fn normals(mesh: &mut RenderMesh) {
    mesh.normals.fill(Vec3::ZERO);
    for tri in &mesh.triangles {
        let [a, b, c] = tri.map(|i| i as usize);
        let n = mesh.positions[b]
            .sub(mesh.positions[a])
            .cross(mesh.positions[c].sub(mesh.positions[a]));
        for i in [a, b, c] {
            mesh.normals[i] = mesh.normals[i].add(n);
        }
    }
    for n in &mut mesh.normals {
        *n = n.try_normalize().unwrap_or(Vec3::new(0., 0., 1.));
    }
}
fn digest(cfg: DynamicsConfig, c: &Compiled, meshes: &[RenderMesh]) -> [u8; 32] {
    let mut h = sha256::Sha256::new();
    h.update(b"gibson.living-matter.config.v1\0");
    h.update(&c.body_rev_id().0);
    h.update(&c.fit_id().0);
    h.update(&[cfg.fabric as u8, cfg.substeps as u8, cfg.iterations as u8]);
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
    for m in meshes.iter().filter(|m| m.role == MeshRole::Hair) {
        h.update(&(m.id.len() as u64).to_le_bytes());
        h.update(m.id.as_bytes());
        h.update(&(m.positions.len() as u64).to_le_bytes());
        for p in &m.positions {
            for n in [p.x, p.y, p.z] {
                h.update(&n.to_le_bytes());
            }
        }
    }
    h.finalize()
}

impl FigureDynamics {
    pub fn new(c: &Compiled, config: DynamicsConfig) -> Result<Self, &'static str> {
        Self::with_props(c, config, &[])
    }

    /// Stationary scene-local boxes (centre and full dimensions in world metres).
    pub fn with_props(
        c: &Compiled,
        config: DynamicsConfig,
        props: &[(Vec3, Vec3)],
    ) -> Result<Self, &'static str> {
        config.validate()?;
        if props.len() > 16
            || props.iter().any(|(p, s)| {
                !p.is_finite() || !s.is_finite() || s.x <= 0. || s.y <= 0. || s.z <= 0.
            })
        {
            return Err("invalid scene collider");
        }

        let meshes = c.rest_render_meshes();
        let body = meshes
            .first()
            .filter(|m| m.role == MeshRole::Body)
            .ok_or("missing body")?;
        let top = body
            .positions
            .iter()
            .map(|p| p.z)
            .fold(f32::NEG_INFINITY, f32::max);
        let lo = body
            .positions
            .iter()
            .map(|p| p.z)
            .fold(f32::INFINITY, f32::min);
        let height = top - lo;
        if !(0.1..=5.).contains(&height) {
            return Err("unsupported figure scale");
        }
        let chest = c
            .skeleton()
            .joint_index("spine_03")
            .or_else(|| c.skeleton().joint_index("chest"))
            .unwrap_or(0);
        let center = c.skeleton().world_bind()[chest].transform_point(Vec3::ZERO);
        let z = lo + 0.718 * height;
        let centers = [-1., 1.].map(|s| {
            let x = center.x + s * 0.052 * height;
            let y = body
                .positions
                .iter()
                .filter(|p| (p.z - z).abs() < 0.035 * height && (p.x - x).abs() < 0.025 * height)
                .map(|p| p.y)
                .fold(center.y, f32::min);
            Vec3::new(x, y + 0.012 * height, z)
        });
        let fields = meshes
            .iter()
            .map(|m| {
                let mut weights = Vec::new();
                if matches!(m.role, MeshRole::Body | MeshRole::Garment) {
                    for (vertex, &p) in m.positions.iter().enumerate() {
                        for (j, &origin) in centers.iter().enumerate() {
                            let r = p.sub(origin);
                            let radius = [0.053 * height, 0.063 * height, 0.066 * height];
                            let grad = Vec3::new(
                                r.x / radius[0].powi(2),
                                r.y / radius[1].powi(2),
                                r.z / radius[2].powi(2),
                            );
                            let q = r.dot(grad);
                            if q < 1. {
                                let w = (1. - q).powi(3);
                                weights.push(Weight {
                                    j,
                                    vertex,
                                    w,
                                    gradient: grad.scale(-6. * (1. - q).powi(2)),
                                    r,
                                });
                            }
                        }
                    }
                }
                weights
            })
            .collect();
        let mut cloth = None;
        let mut hair = None;
        for (i, m) in meshes.iter().enumerate() {
            if m.role == MeshRole::Garment
                && c.drawable_garments()
                    .iter()
                    .any(|g| g.id == m.id && g.kind == "skirt")
            {
                if cloth.is_some() {
                    return Err("only one outer panel grid is supported");
                }
                cloth = Some((i, Cloth::new(&m.positions, &m.triangles, config)?));
            }
            if m.role == MeshRole::Hair && m.positions.len() > 2 {
                let h = m
                    .positions
                    .iter()
                    .map(|p| p.z)
                    .fold(f32::NEG_INFINITY, f32::max)
                    - m.positions
                        .iter()
                        .map(|p| p.z)
                        .fold(f32::INFINITY, f32::min);
                if h >= 0.02 {
                    hair = Some((i, Hair::new(&m.positions)?));
                }
            }
        }
        let regions = centers.map(|origin| {
            Region::new(
                RegionFrame {
                    origin,
                    rotation: Quat::IDENTITY,
                },
                Vec3::new(0., 0., -9.80665),
            )
        });
        Ok(Self {
            config,
            id: {
                let mut h = sha256::Sha256::new();
                h.update(&digest(config, c, &meshes));
                for (p, s) in props {
                    for x in [p.x, p.y, p.z, s.x, s.y, s.z] {
                        h.update(&x.to_le_bytes());
                    }
                }
                h.finalize()
            },
            props: props.to_vec(),
            present: c.present_id().0,
            tick: 0,
            chest,
            centers,
            regions,
            fields: Arc::new(fields),
            cloth,
            hair,
        })
    }
    pub fn config_id(&self) -> [u8; 32] {
        self.id
    }
    pub fn tick(&self) -> u64 {
        self.tick
    }
    pub fn checkpoint(&self) -> MatterCheckpoint {
        MatterCheckpoint {
            state: self.clone(),
        }
    }
    pub fn restore(&mut self, c: &MatterCheckpoint) -> Result<(), &'static str> {
        if self.id != c.state.id || self.present != c.state.present {
            return Err("checkpoint identity mismatch");
        }
        *self = c.state.clone();
        Ok(())
    }
    /// Advance one fixed-rate input pose. A failure leaves the previous coherent state usable.
    pub fn step(
        &mut self,
        c: &Compiled,
        frame: &TapeFrame,
        fps: u32,
    ) -> Result<Vec<RenderMesh>, &'static str> {
        if !(10..=240).contains(&fps) || self.present != c.present_id().0 {
            return Err("invalid rate or compiled actor");
        }
        let mut next = self.clone();
        let meshes = next.advance(c, frame, fps)?;
        *self = next;
        Ok(meshes)
    }
    fn advance(
        &mut self,
        c: &Compiled,
        frame: &TapeFrame,
        fps: u32,
    ) -> Result<Vec<RenderMesh>, &'static str> {
        let mut meshes = c
            .render_meshes(frame)
            .map_err(|_| "invalid kinematic pose")?;
        let world = c
            .skeleton()
            .world_from_locals(&frame.locals)
            .map_err(|_| "joint frame failed")?;
        let skin = world[self.chest].mul(&c.inverse_binds()[self.chest]);
        let rot = DualQuat::from_mat4(&skin)
            .ok_or("regional anchor requires rigid joint")?
            .real;
        let dt = 1. / fps as f32;
        let mut shifts = [Vec3::ZERO; 2];
        for (j, shift) in shifts.iter_mut().enumerate() {
            let f = RegionFrame {
                origin: skin.transform_point(self.centers[j]).add(frame.root_t),
                rotation: rot,
            };
            self.regions[j].advance(
                f,
                self.config.gravity,
                dt,
                self.config.firmness,
                self.config.damping,
                self.config.support,
                self.config.intensity,
            )?;
            *shift = self.regions[j].displacement();
        }
        for (m, field) in meshes.iter_mut().zip(self.fields.iter()) {
            for w in field {
                let u = shifts[w.j];
                // curl(0.5*w*(u cross r)): a compact, divergence-free first-order volume mode.
                let d = u.scale(w.w).add(w.gradient.cross(u.cross(w.r)).scale(0.5));
                m.positions[w.vertex] = m.positions[w.vertex].add(rot.rotate(d));
            }
            if !field.is_empty() {
                normals(m);
            }
        }
        let mut collision = Surface::new();
        if self.cloth.is_some() || self.hair.is_some() {
            let body = &meshes[0];
            collision.insert(
                &body.positions,
                c.indices().chunks_exact(3).map(|t| [t[0], t[1], t[2]]),
            );
            for (i, m) in meshes.iter().enumerate() {
                if m.role == MeshRole::Garment
                    && !self.cloth.as_ref().is_some_and(|(ci, _)| *ci == i)
                {
                    collision.insert(&m.positions, m.triangles.iter().copied());
                }
            }
        }
        let contact = |p: Vec3, _t: f32| {
            let mut q = collision.project(p, 0.004);
            for &(center, size) in &self.props {
                let d = q.sub(center);
                let e = size.scale(0.5).add(Vec3::new(0.004, 0.004, 0.004));
                let depth = [e.x - d.x.abs(), e.y - d.y.abs(), e.z - d.z.abs()];
                if depth.iter().all(|x| *x > 0.) {
                    let k = (0..3)
                        .min_by(|&a, &b| depth[a].total_cmp(&depth[b]))
                        .unwrap();
                    match k {
                        0 => q.x = center.x + e.x.copysign(d.x),
                        1 => q.y = center.y + e.y.copysign(d.y),
                        _ => q.z = center.z + e.z.copysign(d.z),
                    }
                }
            }
            q.z = q.z.max(0.004);
            q
        };
        if let Some((i, cloth)) = &mut self.cloth {
            cloth.step(&mut meshes[*i].positions, self.config, dt, contact)?;
            normals(&mut meshes[*i]);
        }
        if let Some((i, hair)) = &mut self.hair {
            hair.step(&mut meshes[*i].positions, self.config, dt, contact)?;
            normals(&mut meshes[*i]);
        }
        self.tick = self.tick.checked_add(1).ok_or("simulation tick overflow")?;
        Ok(meshes)
    }
}
