//! Skin weights (up to 4 influences per vertex) and linear-blend skinning.

use std::fmt;

use super::math::{DualQuat, Mat4, Vec3};

/// Maximum influences per vertex.
pub const MAX_INFLUENCES: usize = 4;

/// How skin matrices are blended per vertex.
///
/// `Linear` is the contract's linear-blend skinning, `v' = sum(w_j * M_j * v)` with
/// `M_j = W_j * IB_j`. `DualQuat` blends the rigid part of each `M_j` as a unit dual quaternion
/// and refuses a skin matrix that is not rigid ([`SkinError::NonRigid`]). Linear is the default
/// and is bit-identical to [`SkinWeights::skin`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SkinningMode {
    #[default]
    Linear,
    DualQuat,
}

/// Hard cap on skinned vertices.
pub const MAX_SKIN_VERTICES: usize = 1 << 18;

/// A vertex's weights must sum to 1 within this tolerance. Weights are NEVER renormalized:
/// input outside the tolerance is an error.
pub const WEIGHT_SUM_TOLERANCE: f64 = 1.0e-4;

/// Why skin weights or a skinning call were rejected.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SkinError {
    /// More vertices than [`MAX_SKIN_VERTICES`].
    LimitExceeded { count: usize, max: usize },
    /// A vertex has no influences or more than [`MAX_INFLUENCES`].
    InfluenceCount { vertex: usize, count: usize },
    /// An influence names a joint outside `0..joint_count`.
    JointOutOfRange { vertex: usize, joint: usize },
    /// A vertex lists the same joint twice.
    DuplicateJoint { vertex: usize, joint: usize },
    /// A weight is NaN or infinite.
    NonFinite { vertex: usize },
    /// A weight is negative.
    NegativeWeight { vertex: usize },
    /// The weights of a vertex do not sum to 1 within [`WEIGHT_SUM_TOLERANCE`].
    WeightSum { vertex: usize, sum: f64 },
    /// A slice handed to skinning has the wrong length.
    LengthMismatch {
        what: &'static str,
        expected: usize,
        got: usize,
    },
    /// A skinning matrix or input position is not finite.
    NonFiniteInput { what: &'static str },
    /// Dual-quaternion skinning met a skin matrix `W_j * IB_j` of a referenced joint that is
    /// not rigid (scale, shear, reflection, not affine) within `figure::math::RIGID_TOLERANCE`.
    NonRigid { joint: usize },
    /// A vertex's blended dual quaternion (or normal) cancelled to nothing.
    DegenerateBlend { vertex: usize },
}

impl fmt::Display for SkinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkinError::LimitExceeded { count, max } => {
                write!(f, "too many skinned vertices: {count} (max {max})")
            }
            SkinError::InfluenceCount { vertex, count } => write!(
                f,
                "vertex {vertex} has {count} influences (must be 1..={MAX_INFLUENCES})"
            ),
            SkinError::JointOutOfRange { vertex, joint } => {
                write!(f, "vertex {vertex} references joint {joint}, out of range")
            }
            SkinError::DuplicateJoint { vertex, joint } => {
                write!(f, "vertex {vertex} lists joint {joint} twice")
            }
            SkinError::NonFinite { vertex } => write!(f, "vertex {vertex} has a non-finite weight"),
            SkinError::NegativeWeight { vertex } => {
                write!(f, "vertex {vertex} has a negative weight")
            }
            SkinError::WeightSum { vertex, sum } => write!(
                f,
                "vertex {vertex} weights sum to {sum}, not 1 within {WEIGHT_SUM_TOLERANCE}"
            ),
            SkinError::LengthMismatch {
                what,
                expected,
                got,
            } => write!(f, "{what}: expected {expected} entries, got {got}"),
            SkinError::NonFiniteInput { what } => write!(f, "{what} is not finite"),
            SkinError::NonRigid { joint } => write!(
                f,
                "skin matrix of joint {joint} is not rigid (dual-quaternion skinning refuses scale and shear)"
            ),
            SkinError::DegenerateBlend { vertex } => {
                write!(f, "vertex {vertex} blends to a degenerate transform")
            }
        }
    }
}

impl std::error::Error for SkinError {}

/// The influences of one vertex: `count` used slots of `joints` / `weights`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct VertexSkin {
    count: u8,
    joints: [u32; MAX_INFLUENCES],
    weights: [f32; MAX_INFLUENCES],
}

/// Validated skin weights for a mesh: per vertex, 1 to 4 `(joint, weight)` influences with
/// non-negative finite weights summing to 1 within [`WEIGHT_SUM_TOLERANCE`].
#[derive(Debug, Clone, PartialEq)]
pub struct SkinWeights {
    joint_count: usize,
    vertices: Vec<VertexSkin>,
}

impl SkinWeights {
    /// Validate `influences` (one list per vertex) against `joint_count`. Nothing is
    /// renormalized and nothing is clamped: any violation is an error naming the vertex.
    pub fn new(
        influences: &[Vec<(usize, f32)>],
        joint_count: usize,
    ) -> Result<SkinWeights, SkinError> {
        if influences.len() > MAX_SKIN_VERTICES {
            return Err(SkinError::LimitExceeded {
                count: influences.len(),
                max: MAX_SKIN_VERTICES,
            });
        }
        let mut vertices = Vec::with_capacity(influences.len());
        for (v, list) in influences.iter().enumerate() {
            if list.is_empty() || list.len() > MAX_INFLUENCES {
                return Err(SkinError::InfluenceCount {
                    vertex: v,
                    count: list.len(),
                });
            }
            let mut vs = VertexSkin {
                count: list.len() as u8,
                joints: [0; MAX_INFLUENCES],
                weights: [0.0; MAX_INFLUENCES],
            };
            let mut sum = 0.0f64;
            for (k, &(j, w)) in list.iter().enumerate() {
                if j >= joint_count {
                    return Err(SkinError::JointOutOfRange {
                        vertex: v,
                        joint: j,
                    });
                }
                if list[..k].iter().any(|&(pj, _)| pj == j) {
                    return Err(SkinError::DuplicateJoint {
                        vertex: v,
                        joint: j,
                    });
                }
                if !w.is_finite() {
                    return Err(SkinError::NonFinite { vertex: v });
                }
                if w < 0.0 {
                    return Err(SkinError::NegativeWeight { vertex: v });
                }
                sum += f64::from(w);
                vs.joints[k] = j as u32;
                vs.weights[k] = w;
            }
            if (sum - 1.0).abs() > WEIGHT_SUM_TOLERANCE {
                return Err(SkinError::WeightSum { vertex: v, sum });
            }
            vertices.push(vs);
        }
        Ok(SkinWeights {
            joint_count,
            vertices,
        })
    }

    /// Number of skinned vertices.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// The joint count these weights were validated against.
    pub fn joint_count(&self) -> usize {
        self.joint_count
    }

    /// The `(joint, weight)` influences of `vertex`, in declared order.
    pub fn influences(&self, vertex: usize) -> Option<Vec<(usize, f32)>> {
        let vs = self.vertices.get(vertex)?;
        Some(
            vs.joints
                .iter()
                .zip(vs.weights.iter())
                .take(usize::from(vs.count))
                .map(|(&j, &w)| (j as usize, w))
                .collect(),
        )
    }

    /// Append the canonical byte form of these weights: per vertex, the influence count as one
    /// byte, then each influence as `u32` joint (LE) and the weight's `f32` bits (LE).
    pub fn write_canonical(&self, out: &mut Vec<u8>) {
        for vs in &self.vertices {
            out.push(vs.count);
            for (j, w) in vs
                .joints
                .iter()
                .zip(vs.weights.iter())
                .take(usize::from(vs.count))
            {
                out.extend_from_slice(&j.to_le_bytes());
                out.extend_from_slice(
                    &crate::figure::revision::canonical_f32_bits(*w).to_le_bytes(),
                );
            }
        }
    }

    /// Linear-blend skinning: `v' = Σ wⱼ (Wⱼ · IBⱼ) v`, summed over a vertex's influences in
    /// declared order. `world` are the posed joint world matrices `Wⱼ`, `inverse_binds` the
    /// `IBⱼ`; both must have one entry per joint.
    pub fn skin(
        &self,
        positions: &[Vec3],
        world: &[Mat4],
        inverse_binds: &[Mat4],
    ) -> Result<Vec<Vec3>, SkinError> {
        if positions.len() != self.vertices.len() {
            return Err(SkinError::LengthMismatch {
                what: "positions",
                expected: self.vertices.len(),
                got: positions.len(),
            });
        }
        if world.len() != self.joint_count {
            return Err(SkinError::LengthMismatch {
                what: "joint world matrices",
                expected: self.joint_count,
                got: world.len(),
            });
        }
        if inverse_binds.len() != self.joint_count {
            return Err(SkinError::LengthMismatch {
                what: "inverse binds",
                expected: self.joint_count,
                got: inverse_binds.len(),
            });
        }
        if !world.iter().all(Mat4::is_finite) {
            return Err(SkinError::NonFiniteInput {
                what: "joint world matrix",
            });
        }
        if !inverse_binds.iter().all(Mat4::is_finite) {
            return Err(SkinError::NonFiniteInput {
                what: "inverse bind matrix",
            });
        }
        if !positions.iter().all(|p| p.is_finite()) {
            return Err(SkinError::NonFiniteInput { what: "position" });
        }
        let skin_mats: Vec<Mat4> = world
            .iter()
            .zip(inverse_binds.iter())
            .map(|(w, ib)| w.mul(ib))
            .collect();
        let mut out = Vec::with_capacity(positions.len());
        for (vs, p) in self.vertices.iter().zip(positions.iter()) {
            let mut acc = Vec3::ZERO;
            for (&j, &w) in vs
                .joints
                .iter()
                .zip(vs.weights.iter())
                .take(usize::from(vs.count))
            {
                let m = &skin_mats[j as usize];
                acc = acc.add(m.transform_point(*p).scale(w));
            }
            out.push(acc);
        }
        Ok(out)
    }

    /// Skin positions with an explicit [`SkinningMode`]. `Linear` is exactly [`SkinWeights::skin`].
    ///
    /// `DualQuat` converts each REFERENCED joint's `M_j = W_j * IB_j` to a unit dual quaternion
    /// (a non-rigid one is [`SkinError::NonRigid`]), blends each vertex's influences with
    /// [`DualQuat::blend`] (sign-aligned to the vertex's FIRST declared influence, summed in
    /// declared order, normalized; a cancelled blend is [`SkinError::DegenerateBlend`]) and
    /// applies the result to the point. Evaluation is scalar `f32` in the fixed order of
    /// `figure::math`. Unlike linear blending, the result does not depend on the weight sum.
    pub fn skin_with(
        &self,
        mode: SkinningMode,
        positions: &[Vec3],
        world: &[Mat4],
        inverse_binds: &[Mat4],
    ) -> Result<Vec<Vec3>, SkinError> {
        match mode {
            SkinningMode::Linear => self.skin(positions, world, inverse_binds),
            SkinningMode::DualQuat => {
                // Fault injection (mutation control `dqs_as_lbs`): DualQuat falls back to
                // linear blending.
                if cfg!(figure_mutant = "dqs_as_lbs") {
                    return self.skin(positions, world, inverse_binds);
                }
                if positions.len() != self.vertices.len() {
                    return Err(SkinError::LengthMismatch {
                        what: "positions",
                        expected: self.vertices.len(),
                        got: positions.len(),
                    });
                }
                if !positions.iter().all(|p| p.is_finite()) {
                    return Err(SkinError::NonFiniteInput { what: "position" });
                }
                let dqs = self.joint_dual_quats(world, inverse_binds)?;
                let mut out = Vec::with_capacity(positions.len());
                for (v, (vs, p)) in self.vertices.iter().zip(positions.iter()).enumerate() {
                    let blended = Self::blend_vertex(vs, &dqs)
                        .ok_or(SkinError::DegenerateBlend { vertex: v })?;
                    out.push(blended.transform_point(*p));
                }
                Ok(out)
            }
        }
    }

    /// Skin per-vertex normals (directions) with an explicit [`SkinningMode`].
    ///
    /// `DualQuat`: the normal is rotated by the vertex's blended rotation (a unit input stays
    /// unit; nothing is renormalized). `Linear`: `sum(w_j * L_j * n)` with `L_j` the 3x3 linear
    /// part of `M_j`, then normalized (a zero or cancelled result is
    /// [`SkinError::DegenerateBlend`]); that is the correct normal transform for rigid skin
    /// matrices only (the inverse transpose for scaled joints is LATER).
    pub fn skin_normals(
        &self,
        mode: SkinningMode,
        normals: &[Vec3],
        world: &[Mat4],
        inverse_binds: &[Mat4],
    ) -> Result<Vec<Vec3>, SkinError> {
        if normals.len() != self.vertices.len() {
            return Err(SkinError::LengthMismatch {
                what: "normals",
                expected: self.vertices.len(),
                got: normals.len(),
            });
        }
        if !normals.iter().all(|n| n.is_finite()) {
            return Err(SkinError::NonFiniteInput { what: "normal" });
        }
        let dual_quat = mode == SkinningMode::DualQuat && !cfg!(figure_mutant = "dqs_as_lbs");
        if dual_quat {
            let dqs = self.joint_dual_quats(world, inverse_binds)?;
            let mut out = Vec::with_capacity(normals.len());
            for (v, (vs, n)) in self.vertices.iter().zip(normals.iter()).enumerate() {
                let blended =
                    Self::blend_vertex(vs, &dqs).ok_or(SkinError::DegenerateBlend { vertex: v })?;
                out.push(blended.transform_vector(*n));
            }
            return Ok(out);
        }
        let skin_mats = self.skin_matrices(world, inverse_binds)?;
        let mut out = Vec::with_capacity(normals.len());
        for (v, (vs, n)) in self.vertices.iter().zip(normals.iter()).enumerate() {
            let mut acc = Vec3::ZERO;
            for (&j, &w) in vs
                .joints
                .iter()
                .zip(vs.weights.iter())
                .take(usize::from(vs.count))
            {
                let m = &skin_mats[j as usize];
                acc = acc.add(m.transform_vector(*n).scale(w));
            }
            out.push(
                acc.try_normalize()
                    .ok_or(SkinError::DegenerateBlend { vertex: v })?,
            );
        }
        Ok(out)
    }

    /// `W_j * IB_j` per joint after the length and finiteness checks shared by every mode.
    fn skin_matrices(
        &self,
        world: &[Mat4],
        inverse_binds: &[Mat4],
    ) -> Result<Vec<Mat4>, SkinError> {
        if world.len() != self.joint_count {
            return Err(SkinError::LengthMismatch {
                what: "joint world matrices",
                expected: self.joint_count,
                got: world.len(),
            });
        }
        if inverse_binds.len() != self.joint_count {
            return Err(SkinError::LengthMismatch {
                what: "inverse binds",
                expected: self.joint_count,
                got: inverse_binds.len(),
            });
        }
        if !world.iter().all(Mat4::is_finite) {
            return Err(SkinError::NonFiniteInput {
                what: "joint world matrix",
            });
        }
        if !inverse_binds.iter().all(Mat4::is_finite) {
            return Err(SkinError::NonFiniteInput {
                what: "inverse bind matrix",
            });
        }
        Ok(world
            .iter()
            .zip(inverse_binds.iter())
            .map(|(w, ib)| w.mul(ib))
            .collect())
    }

    /// One unit dual quaternion per joint. A joint no vertex references is not converted (so a
    /// scaled, unreferenced joint cannot fail the mode) and holds the identity; a referenced
    /// joint with a non-rigid skin matrix is [`SkinError::NonRigid`], reported for the lowest
    /// such joint index.
    fn joint_dual_quats(
        &self,
        world: &[Mat4],
        inverse_binds: &[Mat4],
    ) -> Result<Vec<DualQuat>, SkinError> {
        let skin_mats = self.skin_matrices(world, inverse_binds)?;
        let mut used = vec![false; self.joint_count];
        for vs in &self.vertices {
            for &j in vs.joints.iter().take(usize::from(vs.count)) {
                used[j as usize] = true;
            }
        }
        let mut dqs = Vec::with_capacity(skin_mats.len());
        for (j, (m, &is_used)) in skin_mats.iter().zip(used.iter()).enumerate() {
            if is_used {
                match DualQuat::from_mat4(m) {
                    Some(dq) => dqs.push(dq),
                    None => return Err(SkinError::NonRigid { joint: j }),
                }
            } else {
                dqs.push(DualQuat::IDENTITY);
            }
        }
        Ok(dqs)
    }

    /// The blended unit dual quaternion of one vertex (terms in declared influence order).
    fn blend_vertex(vs: &VertexSkin, dqs: &[DualQuat]) -> Option<DualQuat> {
        let mut terms = [(0.0f32, DualQuat::IDENTITY); MAX_INFLUENCES];
        let n = usize::from(vs.count);
        for ((slot, &j), &w) in terms
            .iter_mut()
            .zip(vs.joints.iter())
            .zip(vs.weights.iter())
            .take(n)
        {
            *slot = (w, dqs[j as usize]);
        }
        DualQuat::blend(&terms[..n])
    }
}
