//! Maps physical proxies onto a compiled, posed character. The authored mesh stays immutable.
use super::{
    cloth::Cloth,
    contact::{Collider, Reach, Source},
    hair::Hair,
    region::{Region, RegionFrame},
    regions::{RegionSpec, RegionSpecError},
    DynamicsConfig, Vec3,
};
use crate::figure::{
    sha256,
    studio::{Compiled, MeshRole, RenderMesh, TapeFrame},
    DualQuat, Quat,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Elapsed wall time for one real actor step. Constraint times include contact queries.
#[derive(Default, Debug)]
pub struct StepTiming {
    pub skin_and_regions: Duration,
    pub contact_build: Duration,
    pub cloth_and_contacts: Duration,
    pub hair_and_contacts: Duration,
}
enum Phase {
    Pose,
    Surface,
    Cloth,
    Hair,
}

/// The gravity every region is calibrated against: fixed, in the world frame, never the laboratory gravity.
const REFERENCE_GRAVITY: Vec3 = Vec3::new(0., 0., -9.80665);

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
    rate: Option<u32>,
    chest: usize,
    centers: Vec<Vec3>,
    regions: Vec<Region>,
    /// Declared regions ([`FigureDynamics::with_regions`]); `None` keeps the two built-in chest regions.
    specs: Option<Vec<RegionSpec>>,
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
    normals_of(&mesh.positions, &mesh.triangles, &mut mesh.normals);
}
fn normals_of(positions: &[Vec3], triangles: &[[u32; 3]], normals: &mut [Vec3]) {
    normals.fill(Vec3::ZERO);
    for tri in triangles {
        let [a, b, c] = tri.map(|i| i as usize);
        let n = positions[b]
            .sub(positions[a])
            .cross(positions[c].sub(positions[a]));
        for i in [a, b, c] {
            normals[i] = normals[i].add(n);
        }
    }
    for n in normals {
        *n = n.try_normalize().unwrap_or(Vec3::new(0., 0., 1.));
    }
}

/// Margin around the simulated particles' cells: queries beyond it stay exact through the
/// collider's full-surface fallback, so this only sets how often that slower path runs.
const REACH_MARGIN: f32 = 0.06;
fn digest(cfg: DynamicsConfig, c: &Compiled, meshes: &[RenderMesh]) -> [u8; 32] {
    let mut h = sha256::Sha256::new();
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
        Self::build(c, config, props, &[]).map_err(|e| match e {
            RegionSpecError::Dynamics(message) => message,
            _ => "invalid region declaration",
        })
    }

    /// Declared compliant regions in place of the two built-in chest regions. Each region has its
    /// own frame joint, its weight ellipsoid comes from the spec (tag centroid and half extent),
    /// and it is calibrated at the first admitted frame: its origin, rotation and the reference
    /// gravity in that frame are taken from the first pose, so a held pose, whatever its
    /// orientation, is the equilibrium and the sculpt is untouched there. Specs are part of the
    /// configuration identity. An empty list is exactly [`FigureDynamics::new`].
    ///
    /// A spec measured on another body (vertex count), bound to another compiled body revision,
    /// naming a missing joint or malformed is refused with a [`RegionSpecError`].
    pub fn with_regions(
        c: &Compiled,
        config: DynamicsConfig,
        specs: Vec<RegionSpec>,
    ) -> Result<Self, RegionSpecError> {
        Self::build(c, config, &[], &specs)
    }

    /// Like [`FigureDynamics::with_regions`] with stationary scene boxes.
    pub fn with_regions_and_props(
        c: &Compiled,
        config: DynamicsConfig,
        props: &[(Vec3, Vec3)],
        specs: Vec<RegionSpec>,
    ) -> Result<Self, RegionSpecError> {
        Self::build(c, config, props, &specs)
    }

    /// Per-region displacement in the region frame, in declaration order (left, right for the built-in regions).
    pub fn region_displacements(&self) -> Vec<Vec3> {
        self.regions.iter().map(|r| r.displacement).collect()
    }

    /// Per-region frame (origin, rotation) at the last step.
    pub fn region_frames(&self) -> Vec<(Vec3, Quat)> {
        self.regions
            .iter()
            .map(|r| (r.frame.origin, r.frame.rotation))
            .collect()
    }

    /// Region names: the specs' names, or `left`, `right` for the built-in regions.
    pub fn region_names(&self) -> Vec<String> {
        match &self.specs {
            Some(specs) => specs.iter().map(|s| s.name.clone()).collect(),
            None => vec!["left".to_string(), "right".to_string()],
        }
    }

    fn build(
        c: &Compiled,
        config: DynamicsConfig,
        props: &[(Vec3, Vec3)],
        specs: &[RegionSpec],
    ) -> Result<Self, RegionSpecError> {
        config.validate()?;
        if props.len() > 16
            || props.iter().any(|(p, s)| {
                !p.is_finite() || !s.is_finite() || s.x <= 0. || s.y <= 0. || s.z <= 0.
            })
        {
            return Err("invalid scene collider".into());
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
            return Err("unsupported figure scale".into());
        }
        for (i, spec) in specs.iter().enumerate() {
            spec.validate(
                body.positions.len(),
                c.skeleton().joint_count(),
                c.body_rev_id(),
            )?;
            if specs[..i].iter().any(|o| o.name == spec.name) {
                return Err(RegionSpecError::DuplicateName {
                    region: spec.name.clone(),
                });
            }
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
                if matches!(m.role, MeshRole::Body | MeshRole::Garment) && !specs.is_empty() {
                    for (vertex, &p) in m.positions.iter().enumerate() {
                        for (j, spec) in specs.iter().enumerate() {
                            if let Some((w, gradient, r)) = spec.field_at(p) {
                                weights.push(Weight {
                                    j,
                                    vertex,
                                    w,
                                    gradient,
                                    r,
                                });
                            }
                        }
                    }
                } else if matches!(m.role, MeshRole::Body | MeshRole::Garment) {
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
                    return Err("only one outer panel grid is supported".into());
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
        let (centers, regions): (Vec<Vec3>, Vec<Region>) = if specs.is_empty() {
            (
                centers.to_vec(),
                centers
                    .map(|origin| {
                        Region::new(
                            RegionFrame {
                                origin,
                                rotation: Quat::IDENTITY,
                            },
                            REFERENCE_GRAVITY,
                        )
                    })
                    .to_vec(),
            )
        } else {
            // Placeholders: every declared region is calibrated at the first admitted frame.
            (
                specs.iter().map(|s| s.centroid).collect(),
                specs
                    .iter()
                    .map(|s| {
                        Region::new(
                            RegionFrame {
                                origin: s.centroid,
                                rotation: Quat::IDENTITY,
                            },
                            REFERENCE_GRAVITY,
                        )
                    })
                    .collect(),
            )
        };
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
                if !specs.is_empty() {
                    // Declared regions only; no declared regions leaves the identity bytes untouched.
                    h.update(b"gibson.living-matter.regions.v1\0");
                    h.update(&(specs.len() as u64).to_le_bytes());
                    for spec in specs {
                        spec.write_digest(&mut h);
                    }
                }
                h.finalize()
            },
            props: props.to_vec(),
            present: c.present_id().0,
            tick: 0,
            rate: None,
            chest,
            centers,
            regions,
            specs: if specs.is_empty() {
                None
            } else {
                Some(specs.to_vec())
            },
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
        self.step_observed(c, frame, fps, |_| {})
    }

    /// Run the identical solver with wall-clock measurements for an exclusive host lease.
    pub fn step_profiled(
        &mut self,
        c: &Compiled,
        frame: &TapeFrame,
        fps: u32,
    ) -> Result<(Vec<RenderMesh>, StepTiming), &'static str> {
        let mut times = StepTiming::default();
        let mut last = Instant::now();
        let result = self.step_observed(c, frame, fps, |phase| {
            let now = Instant::now();
            let dt = now.duration_since(last);
            match phase {
                Phase::Pose => times.skin_and_regions = dt,
                Phase::Surface => times.contact_build = dt,
                Phase::Cloth => times.cloth_and_contacts = dt,
                Phase::Hair => times.hair_and_contacts = dt,
            }
            last = now;
        })?;
        Ok((result, times))
    }

    fn step_observed(
        &mut self,
        c: &Compiled,
        frame: &TapeFrame,
        fps: u32,
        done: impl FnMut(Phase),
    ) -> Result<Vec<RenderMesh>, &'static str> {
        if !(10..=240).contains(&fps)
            || self.rate.is_some_and(|r| r != fps)
            || self.present != c.present_id().0
        {
            return Err("invalid rate or compiled actor");
        }
        let mut next = self.clone();
        next.rate = Some(fps);
        let meshes = next.advance(c, frame, fps, done)?;
        *self = next;
        Ok(meshes)
    }
    fn advance(
        &mut self,
        c: &Compiled,
        frame: &TapeFrame,
        fps: u32,
        mut done: impl FnMut(Phase),
    ) -> Result<Vec<RenderMesh>, &'static str> {
        let mut meshes = c
            .render_meshes(frame)
            .map_err(|_| "invalid kinematic pose")?;
        let world = c
            .skeleton()
            .world_from_locals(&frame.locals)
            .map_err(|_| "joint frame failed")?;
        let dt = 1. / fps as f32;
        let mut shifts = vec![Vec3::ZERO; self.regions.len()];
        let mut rots = Vec::with_capacity(self.regions.len());
        if let Some(specs) = &self.specs {
            // Declared regions: one frame per region from its own joint.
            let mut frames = Vec::with_capacity(specs.len());
            for spec in specs {
                let skin = world[spec.joint].mul(&c.inverse_binds()[spec.joint]);
                let rotation = DualQuat::from_mat4(&skin)
                    .ok_or("regional anchor requires rigid joint")?
                    .real;
                frames.push(RegionFrame {
                    origin: skin.transform_point(spec.centroid).add(frame.root_t),
                    rotation,
                });
            }
            if self.tick == 0 {
                // First admitted frame: origin, rotation and the reference gravity in that frame.
                for (region, f) in self.regions.iter_mut().zip(frames.iter()) {
                    *region = Region::new(*f, REFERENCE_GRAVITY);
                }
            }
            for (j, f) in frames.iter().enumerate() {
                self.regions[j].advance(
                    *f,
                    self.config.gravity,
                    dt,
                    self.config.firmness,
                    self.config.damping,
                    self.config.support,
                    self.config.intensity,
                )?;
                shifts[j] = self.regions[j].displacement();
                rots.push(f.rotation);
            }
        } else {
            let skin = world[self.chest].mul(&c.inverse_binds()[self.chest]);
            let rot = DualQuat::from_mat4(&skin)
                .ok_or("regional anchor requires rigid joint")?
                .real;
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
                rots.push(rot);
            }
        }
        for (m, field) in meshes.iter_mut().zip(self.fields.iter()) {
            for w in field {
                let u = shifts[w.j];
                // curl(0.5*w*(u cross r)): a compact, divergence-free first-order volume mode.
                let d = u.scale(w.w).add(w.gradient.cross(u.cross(w.r)).scale(0.5));
                m.positions[w.vertex] = m.positions[w.vertex].add(rots[w.j].rotate(d));
            }
            if !field.is_empty() {
                normals(m);
            }
        }
        done(Phase::Pose);
        // The simulated meshes are detached so the collider can borrow the rest; hair is never
        // a collider and the simulated cloth is excluded, exactly as in the full surface.
        let cloth_at = self.cloth.as_ref().map(|(i, _)| *i);
        let hair_at = self.hair.as_ref().map(|(i, _)| *i);
        let mut detach = |at: Option<usize>| {
            at.map(|i| {
                (
                    std::mem::take(&mut meshes[i].positions),
                    std::mem::take(&mut meshes[i].normals),
                )
            })
        };
        let mut cloth_pos = detach(cloth_at);
        let mut hair_pos = detach(hair_at);
        {
            let collision = if cloth_at.is_some() || hair_at.is_some() {
                let mut sources = vec![Source::Flat(&meshes[0].positions, c.indices())];
                for (i, m) in meshes.iter().enumerate() {
                    if m.role == MeshRole::Garment && cloth_at != Some(i) {
                        sources.push(Source::Tris(&m.positions, &m.triangles));
                    }
                }
                let reach = cloth_pos
                    .iter()
                    .chain(hair_pos.iter())
                    .flat_map(|(p, _)| p.iter().copied())
                    .chain(
                        self.cloth
                            .iter()
                            .flat_map(|(_, s)| s.particles.positions().iter().copied()),
                    )
                    .chain(
                        self.hair
                            .iter()
                            .flat_map(|(_, s)| s.particles.positions().iter().copied()),
                    );
                Some(Collider::build(
                    sources,
                    Reach::around(reach, REACH_MARGIN),
                )?)
            } else {
                None
            };
            done(Phase::Surface);
            let contact = |p: Vec3, _t: f32| {
                let mut q = collision.as_ref().map_or(p, |s| s.project(p, 0.004));
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
            if let (Some((i, cloth)), Some((pos, nrm))) = (&mut self.cloth, &mut cloth_pos) {
                cloth.step(pos, self.config, dt, contact)?;
                normals_of(pos, &meshes[*i].triangles, nrm);
            }
            done(Phase::Cloth);
            if let (Some((i, hair)), Some((pos, nrm))) = (&mut self.hair, &mut hair_pos) {
                hair.step(pos, self.config, dt, contact)?;
                normals_of(pos, &meshes[*i].triangles, nrm);
            }
            done(Phase::Hair);
        }
        for (at, detached) in [(cloth_at, cloth_pos), (hair_at, hair_pos)] {
            if let (Some(i), Some((pos, nrm))) = (at, detached) {
                meshes[i].positions = pos;
                meshes[i].normals = nrm;
            }
        }
        self.tick = self.tick.checked_add(1).ok_or("simulation tick overflow")?;
        Ok(meshes)
    }
}

#[path = "checkpoint.rs"]
mod checkpoint;
