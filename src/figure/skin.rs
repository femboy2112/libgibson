//! Skin weights (up to 4 influences per vertex) and linear-blend skinning.

use std::fmt;

use super::math::{Mat4, Vec3};

/// Maximum influences per vertex.
pub const MAX_INFLUENCES: usize = 4;

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
            (0..usize::from(vs.count))
                .map(|k| (vs.joints[k] as usize, vs.weights[k]))
                .collect(),
        )
    }

    /// Append the canonical byte form of these weights: per vertex, the influence count as one
    /// byte, then each influence as `u32` joint (LE) and the weight's `f32` bits (LE).
    pub fn write_canonical(&self, out: &mut Vec<u8>) {
        for vs in &self.vertices {
            out.push(vs.count);
            for k in 0..usize::from(vs.count) {
                out.extend_from_slice(&vs.joints[k].to_le_bytes());
                out.extend_from_slice(&vs.weights[k].to_bits().to_le_bytes());
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
            for k in 0..usize::from(vs.count) {
                let m = &skin_mats[vs.joints[k] as usize];
                acc = acc.add(m.transform_point(*p).scale(vs.weights[k]));
            }
            out.push(acc);
        }
        Ok(out)
    }
}
