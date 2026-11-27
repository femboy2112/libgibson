//! Validated joint hierarchy with semantic landmarks, world transforms and inverse binds.

use std::fmt;

use super::math::{Mat4, Transform, Vec3};

/// Hard cap on joints per skeleton. Checked before any allocation that depends on the count.
pub const MAX_JOINTS: usize = 256;

/// Hard cap on landmarks per skeleton.
pub const MAX_LANDMARKS: usize = 1024;

/// One joint: a name, an optional parent (which must come EARLIER in the joint list) and its
/// rest (bind) local transform relative to the parent.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Joint {
    pub name: String,
    pub parent: Option<usize>,
    pub rest: Transform,
}

impl Joint {
    /// Construct a joint. Validation happens in [`Skeleton::new`].
    pub fn new(name: impl Into<String>, parent: Option<usize>, rest: Transform) -> Joint {
        Joint {
            name: name.into(),
            parent,
            rest,
        }
    }
}

/// A named semantic point attached to a joint: `offset` is in that joint's local frame.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Landmark {
    pub name: String,
    pub joint: usize,
    pub offset: Vec3,
}

impl Landmark {
    /// Construct a landmark. Validation happens in [`Skeleton::new`].
    pub fn new(name: impl Into<String>, joint: usize, offset: Vec3) -> Landmark {
        Landmark {
            name: name.into(),
            joint,
            offset,
        }
    }
}

/// Why a skeleton, pose or bind computation was rejected.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SkeletonError {
    /// The joint list is empty.
    Empty,
    /// More joints (or landmarks) than the hard cap.
    LimitExceeded {
        what: &'static str,
        count: usize,
        max: usize,
    },
    /// `parent` is not strictly less than `joint`: a forward or self reference, which is how a
    /// cycle or a child-before-parent order shows up.
    ParentOrder { joint: usize, parent: usize },
    /// A translation, rotation or scale is NaN or infinite, or the rotation has zero length.
    NonFinite { joint: usize },
    /// Two joints (or two landmarks) share a name.
    DuplicateName { kind: &'static str, name: String },
    /// A landmark names a joint that does not exist.
    LandmarkJoint { landmark: usize, joint: usize },
    /// A landmark offset is not finite.
    LandmarkNonFinite { landmark: usize },
    /// The world bind matrix of `joint` cannot be inverted (singular scale, say).
    DegenerateBind { joint: usize },
    /// A slice that must hold one entry per joint has the wrong length.
    LengthMismatch { expected: usize, got: usize },
}

impl fmt::Display for SkeletonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkeletonError::Empty => write!(f, "skeleton has no joints"),
            SkeletonError::LimitExceeded { what, count, max } => {
                write!(f, "too many {what}: {count} (max {max})")
            }
            SkeletonError::ParentOrder { joint, parent } => write!(
                f,
                "joint {joint} has parent {parent}, which is not earlier in the joint list"
            ),
            SkeletonError::NonFinite { joint } => {
                write!(f, "joint {joint} has a non-finite or zero-length transform")
            }
            SkeletonError::DuplicateName { kind, name } => {
                write!(f, "duplicate {kind} name {name:?}")
            }
            SkeletonError::LandmarkJoint { landmark, joint } => {
                write!(f, "landmark {landmark} refers to missing joint {joint}")
            }
            SkeletonError::LandmarkNonFinite { landmark } => {
                write!(f, "landmark {landmark} has a non-finite offset")
            }
            SkeletonError::DegenerateBind { joint } => {
                write!(f, "world bind matrix of joint {joint} is not invertible")
            }
            SkeletonError::LengthMismatch { expected, got } => {
                write!(f, "expected {expected} entries, got {got}")
            }
        }
    }
}

impl std::error::Error for SkeletonError {}

/// A validated skeleton: joints in parent-before-child order, unique names, finite transforms,
/// and landmarks. Fields are private so the validated invariants cannot be broken afterwards.
#[derive(Debug, Clone, PartialEq)]
pub struct Skeleton {
    joints: Vec<Joint>,
    landmarks: Vec<Landmark>,
}

impl Skeleton {
    /// Validate and build. Checks, in order: joint and landmark caps, then per joint parent
    /// ordering (`parent < index`, which also rules out cycles), finite transforms and unique
    /// names, then landmarks.
    pub fn new(joints: Vec<Joint>, landmarks: Vec<Landmark>) -> Result<Skeleton, SkeletonError> {
        if joints.is_empty() {
            return Err(SkeletonError::Empty);
        }
        if joints.len() > MAX_JOINTS {
            return Err(SkeletonError::LimitExceeded {
                what: "joints",
                count: joints.len(),
                max: MAX_JOINTS,
            });
        }
        if landmarks.len() > MAX_LANDMARKS {
            return Err(SkeletonError::LimitExceeded {
                what: "landmarks",
                count: landmarks.len(),
                max: MAX_LANDMARKS,
            });
        }
        for (i, j) in joints.iter().enumerate() {
            if let Some(p) = j.parent {
                if p >= i {
                    return Err(SkeletonError::ParentOrder {
                        joint: i,
                        parent: p,
                    });
                }
            }
            if !j.rest.is_valid() {
                return Err(SkeletonError::NonFinite { joint: i });
            }
            if joints[..i].iter().any(|o| o.name == j.name) {
                return Err(SkeletonError::DuplicateName {
                    kind: "joint",
                    name: j.name.clone(),
                });
            }
        }
        for (i, l) in landmarks.iter().enumerate() {
            if l.joint >= joints.len() {
                return Err(SkeletonError::LandmarkJoint {
                    landmark: i,
                    joint: l.joint,
                });
            }
            if !l.offset.is_finite() {
                return Err(SkeletonError::LandmarkNonFinite { landmark: i });
            }
            if landmarks[..i].iter().any(|o| o.name == l.name) {
                return Err(SkeletonError::DuplicateName {
                    kind: "landmark",
                    name: l.name.clone(),
                });
            }
        }
        Ok(Skeleton { joints, landmarks })
    }

    /// The joints, parent before child.
    pub fn joints(&self) -> &[Joint] {
        &self.joints
    }

    /// The landmarks.
    pub fn landmarks(&self) -> &[Landmark] {
        &self.landmarks
    }

    /// Number of joints.
    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    /// Index of the joint called `name`.
    pub fn joint_index(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|j| j.name == name)
    }

    /// The rest local transforms, one per joint.
    pub fn rest_locals(&self) -> Vec<Transform> {
        self.joints.iter().map(|j| j.rest).collect()
    }

    /// World matrices for explicit local transforms (one per joint): `W[i] = W[parent] * L[i]`,
    /// evaluated in joint order, so a parent is always complete before its child.
    pub fn world_from_locals(&self, locals: &[Transform]) -> Result<Vec<Mat4>, SkeletonError> {
        if locals.len() != self.joints.len() {
            return Err(SkeletonError::LengthMismatch {
                expected: self.joints.len(),
                got: locals.len(),
            });
        }
        let mut world: Vec<Mat4> = Vec::with_capacity(self.joints.len());
        for (i, j) in self.joints.iter().enumerate() {
            let local = Mat4::from_transform(&locals[i]);
            let w = match j.parent {
                Some(p) => world[p].mul(&local),
                None => local,
            };
            world.push(w);
        }
        Ok(world)
    }

    /// World bind matrices (from the rest locals).
    pub fn world_bind(&self) -> Vec<Mat4> {
        // The rest locals always have the right length, so this cannot fail.
        self.world_from_locals(&self.rest_locals())
            .unwrap_or_default()
    }

    /// The inverse of each joint's world bind matrix. A non-invertible matrix is a typed
    /// error naming the joint.
    pub fn inverse_binds(&self) -> Result<Vec<Mat4>, SkeletonError> {
        let world = self.world_bind();
        let mut out = Vec::with_capacity(world.len());
        for (i, w) in world.iter().enumerate() {
            match w.inverse() {
                Some(inv) => out.push(inv),
                None => return Err(SkeletonError::DegenerateBind { joint: i }),
            }
        }
        Ok(out)
    }

    /// World position of landmark `index` given joint world matrices.
    pub fn landmark_world(&self, index: usize, world: &[Mat4]) -> Option<Vec3> {
        let l = self.landmarks.get(index)?;
        let w = world.get(l.joint)?;
        Some(w.transform_point(l.offset))
    }
}
