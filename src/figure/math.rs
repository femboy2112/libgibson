//! Scalar `f32` geometry types for `figure`. No SIMD, no `unsafe`, fixed evaluation order.
//!
//! The field layouts below are FIXED for Slice 1 (other submodules depend on them).
//! Operations are added alongside.

/// A 3-vector.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// A rotation quaternion `x i + y j + z k + w` (glTF component order). Unit length
/// when used as a rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// A 4x4 affine matrix, COLUMN-major like glTF: `cols[c][r]`. Points are column vectors,
/// `p' = M * p`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

/// Translation, rotation, scale. Composed as `T * R * S` (glTF node semantics).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
