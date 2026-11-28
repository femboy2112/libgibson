//! Declared compliant regions for Living Matter: a [`RegionSpec`] per soft region (bust, glute,
//! thigh, ...) built from the family's [`CompliantTag`]s, with a compact smooth falloff and an
//! optional smooth side partition, instead of the two hard-wired chest regions.
//!
//! # Falloff
//!
//! With `a = scale * half_extent` (per axis) and `r = p - centroid` (bind world), `q = sum (r_i / a_i)^2`.
//! The ellipsoid weight is `w = (1 - q)^3` for `q < 1` (zero beyond), exactly the weight the two
//! built-in regions use. An optional [`Partition`] multiplies it by `h = s^2 (3 - 2 s)` with
//! `s = clamp((n . p - offset) / width, 0, 1)`: the weight goes to zero smoothly at the partition
//! plane, so a left and a right region (normals `+n` and `-n`) never overlap and have no hard
//! edge. `W = w h` and the displacement of a vertex for a region displacement `u` is
//! `d = W u + 0.5 grad(W) x (u x r)`, which is `curl(0.5 W (u x r))`: divergence free for any
//! smooth `W`, so the local volume change is zero to first order.
//!
//! A spec with a zero semi-axis moves no vertex (an inert declaration is inert, not an error).
//!
//! # Stale-proxy refusal
//!
//! A spec records the vertex count of the body its tag was measured on, optionally the family
//! body revision it came from and the compiled body revision it is bound to. A spec whose vertex
//! count differs from the compiled body, or that is bound to another compiled body revision, is
//! refused with a [`RegionSpecError`] by [`super::FigureDynamics::with_regions`].
use std::fmt;

use super::Vec3;
use crate::figure::family::{
    BodyRevId as FamilyBodyRevId, BoundRig, CompliantTag, RebindError, ShapedBody,
};
use crate::figure::sha256::Sha256;
use crate::figure::studio::BodyRevId;
use crate::figure::Mat4;

/// A smooth side partition: the region's weight is multiplied by `smoothstep((normal . p - offset) / width)`
/// and is zero on the far side of the plane `normal . p = offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Partition {
    /// Unit normal pointing into the region's side.
    pub normal: Vec3,
    /// Plane offset along `normal` (bind world metres).
    pub offset: f32,
    /// Fade width in metres; the weight reaches its full ellipsoid value at `offset + width`.
    pub width: f32,
}

/// The falloff of one region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Falloff {
    /// Semi-axes of the weight ellipsoid are `scale * half_extent`.
    pub scale: f32,
    /// Optional smooth side partition.
    pub partition: Option<Partition>,
}

impl Falloff {
    /// The compact ellipsoid falloff with no partition.
    pub const fn compact(scale: f32) -> Falloff {
        Falloff {
            scale,
            partition: None,
        }
    }

    /// The same falloff with a smooth side partition.
    pub const fn partitioned(scale: f32, partition: Partition) -> Falloff {
        Falloff {
            scale,
            partition: Some(partition),
        }
    }
}

/// Why a region declaration was refused.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RegionSpecError {
    /// The spec was measured on a body with a different vertex count (stale proxy or other remap).
    VertexCountMismatch {
        region: String,
        spec: usize,
        body: usize,
    },
    /// The spec is bound to a different compiled body revision.
    StaleBody {
        region: String,
        bound_to: BodyRevId,
        body: BodyRevId,
    },
    /// The frame joint is not a joint of the compiled skeleton.
    JointOutOfRange {
        region: String,
        joint: usize,
        joints: usize,
    },
    /// A non-finite number, a negative extent, a non-positive scale or a non-positive partition width.
    BadFalloff { region: String },
    /// Two specs share a name.
    DuplicateName { region: String },
    /// A measured region has no vertices.
    EmptyRegion { region: String },
    /// A measured region names a vertex outside the body.
    VertexOutOfRange { region: String, vertex: u32 },
    /// A refusal raised by the dynamics itself.
    Dynamics(&'static str),
}

impl From<&'static str> for RegionSpecError {
    fn from(message: &'static str) -> RegionSpecError {
        RegionSpecError::Dynamics(message)
    }
}

impl fmt::Display for RegionSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegionSpecError::VertexCountMismatch { region, spec, body } => write!(
                f,
                "region {region:?} was measured on a body with {spec} vertices, the body has {body}"
            ),
            RegionSpecError::StaleBody {
                region,
                bound_to,
                body,
            } => write!(
                f,
                "region {region:?} is bound to body revision {bound_to}, the body is {body}"
            ),
            RegionSpecError::JointOutOfRange {
                region,
                joint,
                joints,
            } => write!(
                f,
                "region {region:?} names joint {joint} of a skeleton with {joints} joints"
            ),
            RegionSpecError::BadFalloff { region } => {
                write!(f, "region {region:?} has an invalid extent or falloff")
            }
            RegionSpecError::DuplicateName { region } => {
                write!(f, "region name {region:?} is declared twice")
            }
            RegionSpecError::EmptyRegion { region } => {
                write!(f, "region {region:?} has no vertices")
            }
            RegionSpecError::VertexOutOfRange { region, vertex } => {
                write!(
                    f,
                    "region {region:?} names vertex {vertex} outside the body"
                )
            }
            RegionSpecError::Dynamics(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for RegionSpecError {}

/// One declared compliant region: a frame joint, a bind-world centre and extent measured on the
/// shaped surface (the family's compliant tag), and a falloff.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionSpec {
    pub name: String,
    /// Frame joint (index into the compiled skeleton).
    pub joint: usize,
    /// Region centroid in bind world space.
    pub centroid: Vec3,
    /// The centroid in the joint's bind frame (informational; the frame uses `centroid`).
    pub local_centroid: Vec3,
    /// Per-axis maximum `|p - centroid|` over the region (bind world axes).
    pub half_extent: Vec3,
    /// Largest `|p - centroid|` over the region (informational).
    pub radius: f32,
    pub falloff: Falloff,
    /// Vertex count of the body the tag was measured on.
    pub vertex_count: usize,
    /// The family body revision the tag came from, when it came from a rig.
    pub family_body_rev: Option<FamilyBodyRevId>,
    /// The compiled body revision this spec is bound to, when the caller knows it.
    pub bound_body: Option<BodyRevId>,
}

impl RegionSpec {
    /// A spec from a family compliant tag. `vertex_count` is the shaped body's vertex count.
    pub fn from_tag(tag: &CompliantTag, vertex_count: usize, falloff: Falloff) -> RegionSpec {
        RegionSpec {
            name: tag.name.clone(),
            joint: tag.joint,
            centroid: tag.centroid,
            local_centroid: tag.local_centroid,
            half_extent: tag.half_extent,
            radius: tag.radius,
            falloff,
            vertex_count,
            family_body_rev: None,
            bound_body: None,
        }
    }

    /// Every compliant tag of `rig` as a spec, refused with [`RebindError::StaleSkeleton`] unless
    /// the rig was derived from exactly `body`. `falloff` chooses the falloff by tag name.
    pub fn from_rig(
        rig: &BoundRig,
        body: &ShapedBody,
        falloff: impl Fn(&str) -> Falloff,
    ) -> Result<Vec<RegionSpec>, RebindError> {
        let tags = rig.compliant_for(body)?;
        let n = body.positions().len();
        Ok(tags
            .iter()
            .map(|t| {
                let mut s = RegionSpec::from_tag(t, n, falloff(&t.name));
                s.family_body_rev = Some(rig.body_rev());
                s
            })
            .collect())
    }

    /// Bind this spec to a compiled body revision.
    pub fn bound_to(mut self, body: BodyRevId) -> RegionSpec {
        self.bound_body = Some(body);
        self
    }

    /// Measure a spec on `positions` exactly as the rebind does for a compliant tag: centroid,
    /// radius and per-axis half extent over the (sorted) region vertices, and the centroid in the
    /// frame of `inverse_bind` (the frame joint's inverse bind matrix).
    pub fn measure(
        name: &str,
        joint: usize,
        vertices: &[u32],
        positions: &[Vec3],
        inverse_bind: &Mat4,
        falloff: Falloff,
    ) -> Result<RegionSpec, RegionSpecError> {
        if vertices.is_empty() {
            return Err(RegionSpecError::EmptyRegion {
                region: name.to_string(),
            });
        }
        let mut sorted = vertices.to_vec();
        sorted.sort_unstable();
        if let Some(&vertex) = sorted.iter().find(|&&v| (v as usize) >= positions.len()) {
            return Err(RegionSpecError::VertexOutOfRange {
                region: name.to_string(),
                vertex,
            });
        }
        let mut s = Vec3::ZERO;
        for &v in &sorted {
            s = s.add(positions[v as usize]);
        }
        let cnt = sorted.len() as f32;
        let centroid = Vec3::new(s.x / cnt, s.y / cnt, s.z / cnt);
        let mut radius = 0.0f32;
        let mut half_extent = Vec3::ZERO;
        for &v in &sorted {
            let o = positions[v as usize].sub(centroid);
            radius = radius.max(o.length());
            half_extent = Vec3::new(
                half_extent.x.max(o.x.abs()),
                half_extent.y.max(o.y.abs()),
                half_extent.z.max(o.z.abs()),
            );
        }
        Ok(RegionSpec {
            name: name.to_string(),
            joint,
            centroid,
            local_centroid: inverse_bind.transform_point(centroid),
            half_extent,
            radius,
            falloff,
            vertex_count: positions.len(),
            family_body_rev: None,
            bound_body: None,
        })
    }

    /// Validate against a compiled body with `vertices` body vertices, `joints` joints and the
    /// compiled body revision `body`.
    pub(super) fn validate(
        &self,
        vertices: usize,
        joints: usize,
        body: BodyRevId,
    ) -> Result<(), RegionSpecError> {
        let region = || self.name.clone();
        if self.vertex_count != vertices {
            return Err(RegionSpecError::VertexCountMismatch {
                region: region(),
                spec: self.vertex_count,
                body: vertices,
            });
        }
        if let Some(bound_to) = self.bound_body {
            if bound_to != body {
                return Err(RegionSpecError::StaleBody {
                    region: region(),
                    bound_to,
                    body,
                });
            }
        }
        if self.joint >= joints {
            return Err(RegionSpecError::JointOutOfRange {
                region: region(),
                joint: self.joint,
                joints,
            });
        }
        let h = self.half_extent;
        let finite = self.centroid.is_finite()
            && self.local_centroid.is_finite()
            && h.is_finite()
            && self.radius.is_finite();
        let extents_ok = h.x >= 0.0 && h.y >= 0.0 && h.z >= 0.0;
        let scale_ok = self.falloff.scale.is_finite() && self.falloff.scale >= 0.0;
        let partition_ok = self.falloff.partition.is_none_or(|p| {
            p.normal.is_finite()
                && p.offset.is_finite()
                && p.width.is_finite()
                && p.width > 0.0
                && (p.normal.length() - 1.0).abs() < 1.0e-3
        });
        if !(finite && extents_ok && scale_ok && partition_ok) {
            return Err(RegionSpecError::BadFalloff { region: region() });
        }
        Ok(())
    }

    /// The weight `W`, its gradient and `r = p - centroid` at a bind-world point, or `None`
    /// outside the support (and always `None` for a zero semi-axis).
    pub(super) fn field_at(&self, p: Vec3) -> Option<(f32, Vec3, Vec3)> {
        let s = self.falloff.scale;
        let a = Vec3::new(
            self.half_extent.x * s,
            self.half_extent.y * s,
            self.half_extent.z * s,
        );
        if !(a.x > 0.0 && a.y > 0.0 && a.z > 0.0) {
            return None;
        }
        let r = p.sub(self.centroid);
        let grad = Vec3::new(r.x / a.x.powi(2), r.y / a.y.powi(2), r.z / a.z.powi(2));
        let q = r.dot(grad);
        if q >= 1.0 {
            return None;
        }
        let om = 1.0 - q;
        let w = om.powi(3);
        let gw = grad.scale(-6.0 * om.powi(2));
        let (h, gh) = match self.falloff.partition {
            None => (1.0, Vec3::ZERO),
            Some(pt) => {
                let t = pt.normal.dot(p) - pt.offset;
                if t <= 0.0 {
                    return None;
                }
                let x = (t / pt.width).min(1.0);
                let h = x * x * (3.0 - 2.0 * x);
                let gh = if x < 1.0 {
                    pt.normal.scale(6.0 * x * (1.0 - x) / pt.width)
                } else {
                    Vec3::ZERO
                };
                (h, gh)
            }
        };
        if h <= 0.0 {
            return None;
        }
        Some((w * h, gw.scale(h).add(gh.scale(w)), r))
    }

    /// Append this spec's canonical bytes to a digest.
    pub(super) fn write_digest(&self, h: &mut Sha256) {
        h.update(&(self.name.len() as u64).to_le_bytes());
        h.update(self.name.as_bytes());
        h.update(&(self.joint as u64).to_le_bytes());
        for v in [self.centroid, self.local_centroid, self.half_extent] {
            for n in [v.x, v.y, v.z] {
                h.update(&n.to_le_bytes());
            }
        }
        h.update(&self.radius.to_le_bytes());
        h.update(&self.falloff.scale.to_le_bytes());
        match self.falloff.partition {
            None => h.update(&[0]),
            Some(p) => {
                h.update(&[1]);
                for n in [p.normal.x, p.normal.y, p.normal.z, p.offset, p.width] {
                    h.update(&n.to_le_bytes());
                }
            }
        }
        h.update(&(self.vertex_count as u64).to_le_bytes());
        match self.family_body_rev {
            None => h.update(&[0]),
            Some(r) => {
                h.update(&[1]);
                h.update(&r.0);
            }
        }
        match self.bound_body {
            None => h.update(&[0]),
            Some(r) => {
                h.update(&[1]);
                h.update(&r.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(partition: Option<Partition>) -> RegionSpec {
        RegionSpec {
            name: "r".to_string(),
            joint: 0,
            centroid: Vec3::new(0.1, -0.05, 0.3),
            local_centroid: Vec3::ZERO,
            half_extent: Vec3::new(0.08, 0.1, 0.12),
            radius: 0.2,
            falloff: Falloff {
                scale: 1.25,
                partition,
            },
            vertex_count: 100,
            family_body_rev: None,
            bound_body: None,
        }
    }

    fn mirrored(spec: &RegionSpec) -> RegionSpec {
        let mut m = spec.clone();
        m.centroid.x = -m.centroid.x;
        if let Some(p) = &mut m.falloff.partition {
            p.normal = p.normal.neg();
        }
        m
    }

    fn side(sign: f32) -> Option<Partition> {
        Some(Partition {
            normal: Vec3::new(sign, 0.0, 0.0),
            offset: 0.0,
            width: 0.03,
        })
    }

    #[test]
    fn gradient_matches_finite_differences_and_vanishes_at_the_edges() {
        let s = spec(side(1.0));
        let h = 1.0e-3f32;
        let mut checked = 0;
        for &p in &[
            Vec3::new(0.12, -0.02, 0.28),
            Vec3::new(0.02, -0.05, 0.3),
            Vec3::new(0.16, -0.1, 0.33),
        ] {
            let (w, g, _) = s.field_at(p).expect("inside the support");
            assert!(w > 0.0);
            for axis in 0..3 {
                let mut hi = p;
                let mut lo = p;
                match axis {
                    0 => {
                        hi.x += h;
                        lo.x -= h;
                    }
                    1 => {
                        hi.y += h;
                        lo.y -= h;
                    }
                    _ => {
                        hi.z += h;
                        lo.z -= h;
                    }
                }
                let wh = s.field_at(hi).map_or(0.0, |v| v.0);
                let wl = s.field_at(lo).map_or(0.0, |v| v.0);
                let fd = (wh - wl) / (2.0 * h);
                let an = [g.x, g.y, g.z][axis];
                assert!((fd - an).abs() < 2.0e-3 * (1.0 + an.abs()), "{fd} vs {an}");
                checked += 1;
            }
        }
        assert_eq!(checked, 9);
        // no hard edge: the weight is tiny next to the partition plane and next to the ellipsoid surface
        let near_plane = s
            .field_at(Vec3::new(1.0e-4, -0.05, 0.3))
            .map_or(0.0, |v| v.0);
        assert!(near_plane < 1.0e-4);
        assert!(s.field_at(Vec3::new(-0.01, -0.05, 0.3)).is_none());
        let edge = Vec3::new(0.1 + 0.101, -0.05, 0.3); // just beyond the x semi-axis 0.1
        assert!(s.field_at(edge).is_none());
        assert!(
            s.field_at(Vec3::new(0.1 + 0.0999, -0.05, 0.3))
                .map_or(0.0, |v| v.0)
                < 1.0e-6
        );
    }

    #[test]
    fn left_and_right_never_overlap_and_a_zero_radius_spec_is_inert() {
        let l = spec(side(1.0));
        let r = mirrored(&l);
        let mut seen = 0;
        for i in -20..=20 {
            for j in -4..=4 {
                let p = Vec3::new(i as f32 * 0.01, -0.05 + j as f32 * 0.02, 0.3);
                let (a, b) = (l.field_at(p), r.field_at(p));
                assert!(!(a.is_some() && b.is_some()), "overlap at {p:?}");
                seen += usize::from(a.is_some()) + usize::from(b.is_some());
            }
        }
        assert!(seen > 40);
        let mut zero = l.clone();
        zero.half_extent = Vec3::ZERO;
        assert!(zero.field_at(l.centroid).is_none());
        let mut flat = l.clone();
        flat.falloff.scale = 0.0;
        assert!(flat.field_at(l.centroid).is_none());
    }

    #[test]
    fn displacement_is_divergence_free_with_a_partition() {
        let s = spec(side(1.0));
        let u = Vec3::new(0.004, -0.006, 0.01);
        let d_at = |p: Vec3| -> Vec3 {
            match s.field_at(p) {
                Some((w, g, r)) => u.scale(w).add(g.cross(u.cross(r)).scale(0.5)),
                None => Vec3::ZERO,
            }
        };
        let h = 1.0e-3f32;
        for &p in &[Vec3::new(0.12, -0.02, 0.28), Vec3::new(0.09, -0.08, 0.34)] {
            let dx = d_at(Vec3::new(p.x + h, p.y, p.z)).x - d_at(Vec3::new(p.x - h, p.y, p.z)).x;
            let dy = d_at(Vec3::new(p.x, p.y + h, p.z)).y - d_at(Vec3::new(p.x, p.y - h, p.z)).y;
            let dz = d_at(Vec3::new(p.x, p.y, p.z + h)).z - d_at(Vec3::new(p.x, p.y, p.z - h)).z;
            let div = (dx + dy + dz) / (2.0 * h);
            assert!(div.abs() < 5.0e-3 * u.length() / 0.1, "div {div}");
        }
    }

    #[test]
    fn validation_refuses_stale_and_malformed_specs() {
        let body = BodyRevId([1; 32]);
        let ok = spec(None);
        let mut s = ok.clone();
        s.vertex_count = 13380;
        assert!(matches!(
            s.validate(21787, 53, body),
            Err(RegionSpecError::VertexCountMismatch {
                spec: 13380,
                body: 21787,
                ..
            })
        ));
        let mut stale = ok.clone().bound_to(BodyRevId([2; 32]));
        stale.vertex_count = 21787;
        assert!(matches!(
            stale.validate(21787, 53, body),
            Err(RegionSpecError::StaleBody { .. })
        ));
        let mut j = ok.clone();
        j.vertex_count = 21787;
        j.joint = 53;
        assert!(matches!(
            j.validate(21787, 53, body),
            Err(RegionSpecError::JointOutOfRange { .. })
        ));
        let mut bad = ok.clone();
        bad.vertex_count = 21787;
        bad.half_extent.x = -1.0;
        assert!(matches!(
            bad.validate(21787, 53, body),
            Err(RegionSpecError::BadFalloff { .. })
        ));
        let mut good = ok;
        good.vertex_count = 21787;
        assert!(good.validate(21787, 53, body).is_ok());
    }

    #[test]
    fn measure_follows_the_rebind_rule() {
        let p = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
            Vec3::new(0.0, 0.0, 6.0),
        ];
        let s = RegionSpec::measure(
            "m",
            1,
            &[3, 0, 1, 2],
            &p,
            &Mat4::IDENTITY,
            Falloff::compact(1.0),
        )
        .unwrap();
        assert_eq!((s.centroid.x, s.centroid.y, s.centroid.z), (0.5, 1.0, 1.5));
        assert_eq!(
            (s.half_extent.x, s.half_extent.y, s.half_extent.z),
            (1.5, 3.0, 4.5)
        );
        assert!((s.radius - (0.25f32 + 1.0 + 20.25).sqrt()).abs() < 1.0e-6);
        assert_eq!(s.vertex_count, 4);
        assert!(
            RegionSpec::measure("e", 0, &[], &p, &Mat4::IDENTITY, Falloff::compact(1.0)).is_err()
        );
        assert!(
            RegionSpec::measure("o", 0, &[9], &p, &Mat4::IDENTITY, Falloff::compact(1.0)).is_err()
        );
    }
}
