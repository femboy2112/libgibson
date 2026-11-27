//! Skeleton re-derivation from the shaped surface (the `rev.v1` coupled derivation, contract
//! "Coupled derivation" steps 3 to 5). DRAFT, EXPERIMENTAL.
//!
//! A joint may carry an ANCHOR: an ordered group of vertex indices. After the body is shaped
//! (morph targets applied), an anchored joint's bind world position is the centroid of its
//! group's SHAPED positions, so a control that moves the surface moves the skeleton with it, and
//! the bone lengths and inverse binds follow. This is the coupling the v0 `BoneLengthScale`
//! effect lacks (it moves no vertices). Nothing here is wired into `compile`: v0 is untouched,
//! and the v1 compile that will call this is later work.
//!
//! # Derivation, evaluated in this exact order (scalar `f32`, no FMA, no reassociation)
//!
//! For each joint `i`, parent before child:
//! 1. if anchored, the centroid `c` is the sum of the group's shaped positions in the group's
//!    DECLARED order (starting from zero), each component then divided by the count converted to
//!    `f32`; the derived rest translation is `IB_parent * c` (the point transform by the parent's
//!    already derived inverse bind), or `c` for a root. An unanchored joint keeps its declared
//!    rest translation, so it rides with its parent;
//! 2. rest rotation and rest scale are the declared ones (orientation is NOT re-derived);
//! 3. `W_i = W_parent * M(rest)`, `IB_i = W_i^-1`.

use std::fmt;

use super::math::{Mat4, Vec3};
use super::skeleton::{Joint, Skeleton, SkeletonError};

/// Hard cap on the vertices of one anchor group.
pub const MAX_GROUP_VERTICES: usize = 4096;

/// Hard cap on the vertex indices of all anchor groups together.
pub const MAX_GROUP_TOTAL: usize = 65_536;

/// A joint's anchor group: the vertices whose shaped centroid is the joint's bind position.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct JointAnchor {
    pub joint: usize,
    pub vertices: Vec<u32>,
}

impl JointAnchor {
    /// Construct an anchor. Validation happens in [`refit_skeleton`].
    pub fn new(joint: usize, vertices: Vec<u32>) -> JointAnchor {
        JointAnchor { joint, vertices }
    }
}

/// Why [`refit_skeleton`] refused its input.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RefitError {
    /// An anchor names a joint that does not exist.
    JointOutOfRange { joint: usize },
    /// Two anchors name the same joint.
    DuplicateAnchor { joint: usize },
    /// An anchor group has no vertices.
    EmptyGroup { joint: usize },
    /// A group names a vertex outside the shaped positions.
    VertexOutOfRange { joint: usize, vertex: u32 },
    /// A group names one vertex twice.
    DuplicateVertex { joint: usize, vertex: u32 },
    /// A hard limit was exceeded.
    LimitExceeded {
        what: &'static str,
        count: usize,
        max: usize,
    },
    /// A shaped position in a group is not finite.
    NonFinitePosition { vertex: u32 },
    /// The derived skeleton is invalid (a non-finite or non-invertible bind, say).
    Skeleton(SkeletonError),
}

impl fmt::Display for RefitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefitError::JointOutOfRange { joint } => {
                write!(f, "anchor names joint {joint}, out of range")
            }
            RefitError::DuplicateAnchor { joint } => {
                write!(f, "joint {joint} is anchored more than once")
            }
            RefitError::EmptyGroup { joint } => write!(f, "anchor of joint {joint} is empty"),
            RefitError::VertexOutOfRange { joint, vertex } => {
                write!(
                    f,
                    "anchor of joint {joint} names vertex {vertex}, out of range"
                )
            }
            RefitError::DuplicateVertex { joint, vertex } => {
                write!(f, "anchor of joint {joint} names vertex {vertex} twice")
            }
            RefitError::LimitExceeded { what, count, max } => {
                write!(f, "too many {what}: {count} (max {max})")
            }
            RefitError::NonFinitePosition { vertex } => {
                write!(f, "shaped position of vertex {vertex} is not finite")
            }
            RefitError::Skeleton(e) => write!(f, "derived skeleton: {e}"),
        }
    }
}

impl std::error::Error for RefitError {}

/// The re-derived skeleton and its inverse binds. Complete or an error, never partial.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Refit {
    /// The declared joints with anchored joints' rest translations replaced; the declared
    /// landmarks, names, parents, rotations and scales are carried over unchanged.
    pub skeleton: Skeleton,
    /// One per joint: `(W_j at bind)^-1` of the derived skeleton.
    pub inverse_binds: Vec<Mat4>,
}

impl Refit {
    /// The bone length of `joint`: the length of its rest translation in its parent's frame.
    /// `None` for a root or an index out of range.
    pub fn bone_length(&self, joint: usize) -> Option<f32> {
        let j = self.skeleton.joints().get(joint)?;
        j.parent?;
        Some(j.rest.translation.length())
    }
}

/// The centroid of a validated group: component sums in the group's declared order starting from
/// zero, then one division per component by the count as `f32`.
fn centroid(group: &[u32], shaped: &[Vec3]) -> Vec3 {
    let mut s = Vec3::ZERO;
    for &v in group {
        s = s.add(shaped[v as usize]);
    }
    let n = group.len() as f32;
    Vec3::new(s.x / n, s.y / n, s.z / n)
}

/// Re-derive `skeleton` from the shaped surface: joints named by `anchors` move to the centroid
/// of their group in `shaped`, everything else follows (module docs give the exact order).
///
/// `shaped` are the vertex positions AFTER the body's morph controls. The result for unshaped
/// positions is whatever those positions say; this function never reads the declared rest
/// translation of an anchored joint. Validation precedes any derived data.
pub fn refit_skeleton(
    skeleton: &Skeleton,
    anchors: &[JointAnchor],
    shaped: &[Vec3],
) -> Result<Refit, RefitError> {
    let joints = skeleton.joints();
    let mut total = 0usize;
    let mut centroids: Vec<Option<Vec3>> = vec![None; joints.len()];
    for a in anchors {
        if a.joint >= joints.len() {
            return Err(RefitError::JointOutOfRange { joint: a.joint });
        }
        if centroids[a.joint].is_some() {
            return Err(RefitError::DuplicateAnchor { joint: a.joint });
        }
        if a.vertices.is_empty() {
            return Err(RefitError::EmptyGroup { joint: a.joint });
        }
        if a.vertices.len() > MAX_GROUP_VERTICES {
            return Err(RefitError::LimitExceeded {
                what: "anchor group vertices",
                count: a.vertices.len(),
                max: MAX_GROUP_VERTICES,
            });
        }
        total += a.vertices.len();
        if total > MAX_GROUP_TOTAL {
            return Err(RefitError::LimitExceeded {
                what: "anchor vertices",
                count: total,
                max: MAX_GROUP_TOTAL,
            });
        }
        for &v in &a.vertices {
            if (v as usize) >= shaped.len() {
                return Err(RefitError::VertexOutOfRange {
                    joint: a.joint,
                    vertex: v,
                });
            }
            if !shaped[v as usize].is_finite() {
                return Err(RefitError::NonFinitePosition { vertex: v });
            }
        }
        let mut sorted = a.vertices.clone();
        sorted.sort_unstable();
        if let Some(w) = sorted.windows(2).find(|w| w[0] == w[1]) {
            return Err(RefitError::DuplicateVertex {
                joint: a.joint,
                vertex: w[0],
            });
        }
        centroids[a.joint] = Some(centroid(&a.vertices, shaped));
    }

    let mut out_joints: Vec<Joint> = Vec::with_capacity(joints.len());
    let mut world: Vec<Mat4> = Vec::with_capacity(joints.len());
    let mut inverse_binds: Vec<Mat4> = Vec::with_capacity(joints.len());
    for (i, j) in joints.iter().enumerate() {
        let mut rest = j.rest;
        // Fault injection (mutation control `ignored_refit`): the surface is ignored and the
        // declared rest translation is kept.
        let anchored = centroids[i].filter(|_| !cfg!(figure_mutant = "ignored_refit"));
        if let Some(c) = anchored {
            rest.translation = match j.parent {
                Some(p) => inverse_binds[p].transform_point(c),
                None => c,
            };
        }
        let local = Mat4::from_transform(&rest);
        let w = match j.parent {
            Some(p) => world[p].mul(&local),
            None => local,
        };
        let ib = w
            .inverse()
            .ok_or(RefitError::Skeleton(SkeletonError::DegenerateBind {
                joint: i,
            }))?;
        out_joints.push(Joint::new(j.name.clone(), j.parent, rest));
        world.push(w);
        inverse_binds.push(ib);
    }
    let derived =
        Skeleton::new(out_joints, skeleton.landmarks().to_vec()).map_err(RefitError::Skeleton)?;
    Ok(Refit {
        skeleton: derived,
        inverse_binds,
    })
}
