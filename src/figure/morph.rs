//! Sparse morph targets and their application.

use std::fmt;

use super::math::Vec3;

/// Hard cap on morph targets in one family or revision.
pub const MAX_MORPH_TARGETS: usize = 256;

/// Hard cap on deltas in one target (a target cannot usefully exceed the vertex cap).
pub const MAX_MORPH_DELTAS: usize = 1 << 18;

/// A named sparse morph target: a list of `(vertex, delta)`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MorphTarget {
    pub name: String,
    pub deltas: Vec<(u32, Vec3)>,
}

impl MorphTarget {
    /// Construct a target. Validation is [`MorphTarget::validate`].
    pub fn new(name: impl Into<String>, deltas: Vec<(u32, Vec3)>) -> MorphTarget {
        MorphTarget {
            name: name.into(),
            deltas,
        }
    }

    /// Check against `vertex_count`: delta count within the cap, no out-of-range vertex, no
    /// duplicate vertex, finite values. `target` is only used to label the error.
    pub fn validate(&self, target: usize, vertex_count: usize) -> Result<(), MorphError> {
        if self.deltas.len() > MAX_MORPH_DELTAS {
            return Err(MorphError::LimitExceeded {
                what: "morph deltas",
                count: self.deltas.len(),
                max: MAX_MORPH_DELTAS,
            });
        }
        let mut seen = vec![false; vertex_count];
        for &(v, d) in &self.deltas {
            let vi = v as usize;
            if vi >= vertex_count {
                return Err(MorphError::IndexOutOfRange {
                    target,
                    vertex: v,
                    vertex_count,
                });
            }
            if seen[vi] {
                return Err(MorphError::DuplicateVertex { target, vertex: v });
            }
            seen[vi] = true;
            if !d.is_finite() {
                return Err(MorphError::NonFinite { target });
            }
        }
        Ok(())
    }

    /// Append the canonical byte form: name (u32 length + UTF-8), delta count (u32), then per
    /// delta the vertex (u32) and three `f32` bit patterns, all little-endian.
    pub fn write_canonical(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.name.len() as u32).to_le_bytes());
        out.extend_from_slice(self.name.as_bytes());
        out.extend_from_slice(&(self.deltas.len() as u32).to_le_bytes());
        for &(v, d) in &self.deltas {
            out.extend_from_slice(&v.to_le_bytes());
            for c in [d.x, d.y, d.z] {
                out.extend_from_slice(
                    &crate::figure::revision::canonical_f32_bits(c).to_le_bytes(),
                );
            }
        }
    }
}

/// Why a morph target or application was rejected.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum MorphError {
    /// Too many targets or deltas.
    LimitExceeded {
        what: &'static str,
        count: usize,
        max: usize,
    },
    /// A delta addresses a vertex that does not exist.
    IndexOutOfRange {
        target: usize,
        vertex: u32,
        vertex_count: usize,
    },
    /// A target lists a vertex twice.
    DuplicateVertex { target: usize, vertex: u32 },
    /// A delta or a weight is NaN or infinite.
    NonFinite { target: usize },
    /// The weight list does not have one entry per target.
    WeightCount { expected: usize, got: usize },
}

impl fmt::Display for MorphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MorphError::LimitExceeded { what, count, max } => {
                write!(f, "too many {what}: {count} (max {max})")
            }
            MorphError::IndexOutOfRange {
                target,
                vertex,
                vertex_count,
            } => write!(
                f,
                "morph target {target} addresses vertex {vertex} of {vertex_count}"
            ),
            MorphError::DuplicateVertex { target, vertex } => {
                write!(f, "morph target {target} lists vertex {vertex} twice")
            }
            MorphError::NonFinite { target } => {
                write!(f, "morph target {target} has a non-finite delta or weight")
            }
            MorphError::WeightCount { expected, got } => {
                write!(f, "expected {expected} morph weights, got {got}")
            }
        }
    }
}

impl std::error::Error for MorphError {}

/// Validate a whole target list: target cap, then each target against `vertex_count`.
pub fn validate_targets(targets: &[MorphTarget], vertex_count: usize) -> Result<(), MorphError> {
    if targets.len() > MAX_MORPH_TARGETS {
        return Err(MorphError::LimitExceeded {
            what: "morph targets",
            count: targets.len(),
            max: MAX_MORPH_TARGETS,
        });
    }
    for (i, t) in targets.iter().enumerate() {
        t.validate(i, vertex_count)?;
    }
    Ok(())
}

/// `G0 + Σ wᵢ·Δᵢ`, accumulated per vertex in DECLARED target order (target 0 first). `weights`
/// must hold one finite weight per target. Targets are re-checked against `base.len()` so this
/// never panics on a mismatched input.
pub fn apply_morphs(
    base: &[Vec3],
    targets: &[MorphTarget],
    weights: &[f32],
) -> Result<Vec<Vec3>, MorphError> {
    if weights.len() != targets.len() {
        return Err(MorphError::WeightCount {
            expected: targets.len(),
            got: weights.len(),
        });
    }
    validate_targets(targets, base.len())?;
    for (i, w) in weights.iter().enumerate() {
        if !w.is_finite() {
            return Err(MorphError::NonFinite { target: i });
        }
    }
    // Fault injection (mutation control `noop_morph`): morph application is skipped.
    if cfg!(figure_mutant = "noop_morph") {
        return Ok(base.to_vec());
    }
    let mut out = base.to_vec();
    for (t, &w) in targets.iter().zip(weights.iter()) {
        for &(v, d) in &t.deltas {
            let p = out[v as usize];
            out[v as usize] = Vec3::new(p.x + w * d.x, p.y + w * d.y, p.z + w * d.z);
        }
    }
    Ok(out)
}
