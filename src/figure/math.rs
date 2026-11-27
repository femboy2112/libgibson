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

/// A rigid transform as a dual quaternion `real + e * dual`, where `real` is the rotation and
/// `dual = 1/2 * t * real` for a translation `t` (as a pure quaternion). Points transform as
/// `p' = R p + t`, the same convention as [`Mat4`]. Unit when `|real| = 1` and `real . dual = 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DualQuat {
    pub real: Quat,
    pub dual: Quat,
}

/// A skin matrix whose 3x3 block departs from a rotation by more than this is not rigid
/// (dual-quaternion skinning cannot represent it). The test is on `|c_i . c_j - delta_ij|` for
/// the three columns and on `|det - 1|`.
pub const RIGID_TOLERANCE: f32 = 1.0e-3;

/// A blended dual quaternion whose real part has squared length below this is degenerate.
pub const DEGENERATE_BLEND_NORM2: f32 = 1.0e-12;

// ---------------------------------------------------------------------------------------------
// Operations.
//
// FIXED EVALUATION ORDER (part of the determinism law; `rev_id` depends on it):
// - every expression is evaluated strictly left to right exactly as written in the source,
//   in scalar `f32`, with no fused multiply-add, no reassociation and no SIMD;
// - sums of products accumulate in ascending index order (`k = 0, 1, 2, [3]`);
// - `Mat4::mul(a, b)` is `a * b` (apply `b` first): `out[c][r] = a[0][r]*b[c][0] + a[1][r]*b[c][1]
//   + a[2][r]*b[c][2] + a[3][r]*b[c][3]`;
// - `Transform` composes as `T * R * S`: the rotation matrix columns are scaled by `S`, the
//   translation is the fourth column.
// ---------------------------------------------------------------------------------------------

/// Determinants whose magnitude is below this are treated as singular by [`Mat4::inverse`].
pub const SINGULAR_DET: f32 = 1.0e-12;

#[allow(clippy::should_implement_trait)]
impl Vec3 {
    /// The zero vector.
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Construct from components.
    pub const fn new(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3 { x, y, z }
    }

    /// Component-wise sum.
    pub fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    /// Component-wise difference.
    pub fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    /// Multiply every component by `s`.
    pub fn scale(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }

    /// Component-wise product.
    pub fn mul_elem(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }

    /// Negation.
    pub fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }

    /// Dot product `x*x' + y*y' + z*z'`.
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Cross product.
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// Euclidean length.
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// Unit-length copy, or `None` for a zero-length or non-finite vector. Evaluated as
    /// `self.scale(1.0 / length)`.
    pub fn try_normalize(self) -> Option<Vec3> {
        let l = self.length();
        if l <= 0.0 || !l.is_finite() {
            return None;
        }
        Some(self.scale(1.0 / l))
    }

    /// True when every component is finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[allow(clippy::should_implement_trait)]
impl Quat {
    /// The identity rotation.
    pub const IDENTITY: Quat = Quat {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Construct from components (x, y, z, w).
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Quat {
        Quat { x, y, z, w }
    }

    /// Rotation of `angle` radians about `axis` (right-handed). The axis is normalized; a
    /// zero or non-finite axis yields the identity.
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Quat {
        let len = axis.length();
        if len <= 0.0 || !len.is_finite() {
            return Quat::IDENTITY;
        }
        let half = angle * 0.5;
        let s = half.sin() / len;
        Quat::new(axis.x * s, axis.y * s, axis.z * s, half.cos())
    }

    /// Hamilton product `self * o` (apply `o` first, then `self`).
    pub fn mul(self, o: Quat) -> Quat {
        Quat::new(
            self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
            self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
        )
    }

    /// Conjugate `(-x, -y, -z, w)`; the inverse of a unit quaternion.
    pub fn conjugate(self) -> Quat {
        Quat::new(-self.x, -self.y, -self.z, self.w)
    }

    /// Component-wise negation. `-q` is the same rotation as `q`.
    pub fn neg(self) -> Quat {
        Quat::new(-self.x, -self.y, -self.z, -self.w)
    }

    /// Component-wise sum.
    pub fn add(self, o: Quat) -> Quat {
        Quat::new(self.x + o.x, self.y + o.y, self.z + o.z, self.w + o.w)
    }

    /// Multiply every component by `s`.
    pub fn scale(self, s: f32) -> Quat {
        Quat::new(self.x * s, self.y * s, self.z * s, self.w * s)
    }

    /// 4-D dot product `x*x' + y*y' + z*z' + w*w'`, summed in that order.
    pub fn dot(self, o: Quat) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z + self.w * o.w
    }

    /// Rotate `v` by this quaternion, assumed unit length: with `u = (x, y, z)` and
    /// `t = 2 (u x v)`, the result is `v + w t + u x t`. Evaluated in exactly that order.
    pub fn rotate(self, v: Vec3) -> Vec3 {
        let u = Vec3::new(self.x, self.y, self.z);
        let t = u.cross(v).scale(2.0);
        v.add(t.scale(self.w)).add(u.cross(t))
    }

    /// Squared length `x*x + y*y + z*z + w*w`.
    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    /// Normalized copy, or `None` for a zero-length or non-finite quaternion.
    pub fn try_normalize(self) -> Option<Quat> {
        let l2 = self.length_squared();
        if l2 <= 0.0 || !l2.is_finite() {
            return None;
        }
        let inv = 1.0 / l2.sqrt();
        Some(Quat::new(
            self.x * inv,
            self.y * inv,
            self.z * inv,
            self.w * inv,
        ))
    }

    /// Normalized copy; a zero-length or non-finite quaternion becomes the identity. Callers
    /// that must reject such input use [`Quat::try_normalize`].
    pub fn normalize(self) -> Quat {
        self.try_normalize().unwrap_or(Quat::IDENTITY)
    }

    /// True when every component is finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite() && self.w.is_finite()
    }

    /// The 4x4 rotation matrix. The quaternion is assumed to be unit length (not
    /// renormalized here).
    pub fn to_mat(self) -> Mat4 {
        let (x, y, z, w) = (self.x, self.y, self.z, self.w);
        let xx = x * x;
        let yy = y * y;
        let zz = z * z;
        let xy = x * y;
        let xz = x * z;
        let yz = y * z;
        let wx = w * x;
        let wy = w * y;
        let wz = w * z;
        Mat4 {
            cols: [
                [1.0 - 2.0 * (yy + zz), 2.0 * (xy + wz), 2.0 * (xz - wy), 0.0],
                [2.0 * (xy - wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz + wx), 0.0],
                [2.0 * (xz + wy), 2.0 * (yz - wx), 1.0 - 2.0 * (xx + yy), 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

#[allow(clippy::should_implement_trait)]
impl Mat4 {
    /// The identity matrix.
    pub const IDENTITY: Mat4 = Mat4 {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// Matrix product `self * o` (apply `o` first). Summation order is fixed: `k = 0..4`.
    #[allow(clippy::needless_range_loop)]
    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let a = &self.cols;
        let b = &o.cols;
        let mut out = [[0.0f32; 4]; 4];
        for c in 0..4 {
            for r in 0..4 {
                out[c][r] =
                    a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
            }
        }
        Mat4 { cols: out }
    }

    /// The matrix of `T * R * S`. The rotation is normalized first (a zero or non-finite
    /// rotation acts as the identity; validated callers reject it earlier).
    #[allow(clippy::needless_range_loop)]
    pub fn from_transform(t: &Transform) -> Mat4 {
        let r = t.rotation.normalize().to_mat();
        let s = [t.scale.x, t.scale.y, t.scale.z];
        let mut cols = [[0.0f32; 4]; 4];
        for c in 0..3 {
            cols[c][0] = r.cols[c][0] * s[c];
            cols[c][1] = r.cols[c][1] * s[c];
            cols[c][2] = r.cols[c][2] * s[c];
        }
        cols[3] = [t.translation.x, t.translation.y, t.translation.z, 1.0];
        Mat4 { cols }
    }

    /// Transform a point (`w = 1`) by this affine matrix; the bottom row is not consulted.
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let m = &self.cols;
        Vec3::new(
            m[0][0] * p.x + m[1][0] * p.y + m[2][0] * p.z + m[3][0],
            m[0][1] * p.x + m[1][1] * p.y + m[2][1] * p.z + m[3][1],
            m[0][2] * p.x + m[1][2] * p.y + m[2][2] * p.z + m[3][2],
        )
    }

    /// Transform a direction (`w = 0`) by the 3x3 linear part; the translation is ignored.
    pub fn transform_vector(&self, v: Vec3) -> Vec3 {
        let m = &self.cols;
        Vec3::new(
            m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z,
            m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z,
            m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z,
        )
    }

    /// True when every element is finite.
    pub fn is_finite(&self) -> bool {
        self.cols.iter().all(|c| c.iter().all(|v| v.is_finite()))
    }

    /// True when the bottom row is exactly `0 0 0 1` (an affine matrix).
    pub fn is_affine(&self) -> bool {
        let m = &self.cols;
        m[0][3] == 0.0 && m[1][3] == 0.0 && m[2][3] == 0.0 && m[3][3] == 1.0
    }

    /// Inverse of an affine matrix via the 3x3 adjugate and `-A^-1 t`.
    ///
    /// Returns `None` when the matrix is not affine, not finite, or its 3x3 determinant has
    /// magnitude below [`SINGULAR_DET`] (singular or numerically degenerate).
    pub fn inverse(&self) -> Option<Mat4> {
        if !self.is_finite() || !self.is_affine() {
            return None;
        }
        let m = &self.cols;
        // a_rc = m[c][r]
        let (a00, a01, a02) = (m[0][0], m[1][0], m[2][0]);
        let (a10, a11, a12) = (m[0][1], m[1][1], m[2][1]);
        let (a20, a21, a22) = (m[0][2], m[1][2], m[2][2]);
        let c00 = a11 * a22 - a12 * a21;
        let c01 = a12 * a20 - a10 * a22;
        let c02 = a10 * a21 - a11 * a20;
        let c10 = a02 * a21 - a01 * a22;
        let c11 = a00 * a22 - a02 * a20;
        let c12 = a01 * a20 - a00 * a21;
        let c20 = a01 * a12 - a02 * a11;
        let c21 = a02 * a10 - a00 * a12;
        let c22 = a00 * a11 - a01 * a10;
        let det = a00 * c00 + a01 * c01 + a02 * c02;
        if !det.is_finite() || det.abs() < SINGULAR_DET {
            return None;
        }
        // inv_ij = C_ji / det
        let i00 = c00 / det;
        let i01 = c10 / det;
        let i02 = c20 / det;
        let i10 = c01 / det;
        let i11 = c11 / det;
        let i12 = c21 / det;
        let i20 = c02 / det;
        let i21 = c12 / det;
        let i22 = c22 / det;
        let (tx, ty, tz) = (m[3][0], m[3][1], m[3][2]);
        let t0 = -(i00 * tx + i01 * ty + i02 * tz);
        let t1 = -(i10 * tx + i11 * ty + i12 * tz);
        let t2 = -(i20 * tx + i21 * ty + i22 * tz);
        let out = Mat4 {
            cols: [
                [i00, i10, i20, 0.0],
                [i01, i11, i21, 0.0],
                [i02, i12, i22, 0.0],
                [t0, t1, t2, 1.0],
            ],
        };
        if out.is_finite() {
            Some(out)
        } else {
            None
        }
    }
}

/// Dual-quaternion helpers for DQS. EVALUATION ORDER is fixed like the rest of this module:
/// every expression left to right as written, scalar `f32`, no fused multiply-add, no
/// reassociation; blends accumulate in the caller's term order.
#[allow(clippy::should_implement_trait)]
impl DualQuat {
    /// The identity: no rotation, no translation.
    pub const IDENTITY: DualQuat = DualQuat {
        real: Quat::IDENTITY,
        dual: Quat {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
    };

    /// From a rotation and a translation (`p' = R p + t`). The rotation is normalized; `None`
    /// for a zero-length or non-finite rotation or a non-finite translation. The dual part is
    /// `0.5 * (t_pure * real)`, with `t_pure = (tx, ty, tz, 0)` and the Hamilton product of
    /// [`Quat::mul`].
    pub fn from_rigid(rotation: Quat, translation: Vec3) -> Option<DualQuat> {
        let real = rotation.try_normalize()?;
        if !translation.is_finite() {
            return None;
        }
        let t = Quat::new(translation.x, translation.y, translation.z, 0.0);
        Some(DualQuat {
            real,
            dual: t.mul(real).scale(0.5),
        })
    }

    /// From the rigid part of an affine matrix. `None` when the matrix is not finite, not affine
    /// (bottom row exactly `0 0 0 1`) or not rigid: the three columns of the 3x3 block must be
    /// orthonormal and the determinant +1, each within [`RIGID_TOLERANCE`]. The rotation is read
    /// by Shepperd's method (largest of the trace and the three diagonal terms picks the
    /// branch; the first of a tie wins); its sign is whatever the branch gives, which is why
    /// blending aligns signs ([`DualQuat::aligned_to`]).
    #[allow(clippy::needless_range_loop)]
    pub fn from_mat4(m: &Mat4) -> Option<DualQuat> {
        if !m.is_finite() || !m.is_affine() {
            return None;
        }
        let c = &m.cols;
        let cols = [
            Vec3::new(c[0][0], c[0][1], c[0][2]),
            Vec3::new(c[1][0], c[1][1], c[1][2]),
            Vec3::new(c[2][0], c[2][1], c[2][2]),
        ];
        for i in 0..3 {
            for j in i..3 {
                let target = if i == j { 1.0 } else { 0.0 };
                if (cols[i].dot(cols[j]) - target).abs() > RIGID_TOLERANCE {
                    return None;
                }
            }
        }
        let det = cols[0].dot(cols[1].cross(cols[2]));
        if (det - 1.0).abs() > RIGID_TOLERANCE {
            return None;
        }
        // m_rc = c[col][row]
        let (m00, m01, m02) = (c[0][0], c[1][0], c[2][0]);
        let (m10, m11, m12) = (c[0][1], c[1][1], c[2][1]);
        let (m20, m21, m22) = (c[0][2], c[1][2], c[2][2]);
        let trace = m00 + m11 + m22;
        let q = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            Quat::new((m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s)
        } else if m00 > m11 && m00 > m22 {
            let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
            Quat::new(0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s)
        } else if m11 > m22 {
            let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
            Quat::new((m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s)
        } else {
            let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
            Quat::new((m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s)
        };
        DualQuat::from_rigid(q, Vec3::new(c[3][0], c[3][1], c[3][2]))
    }

    /// The same rigid transform with its sign chosen so that `real . reference.real >= 0`: when
    /// the 4-D dot product of the real parts is below zero, both parts are negated (`-q` is the
    /// same transform). Exactly zero is NOT flipped.
    pub fn aligned_to(self, reference: &DualQuat) -> DualQuat {
        // Fault injection (mutation control `dqs_no_align`): signs are never aligned.
        if cfg!(figure_mutant = "dqs_no_align") {
            return self;
        }
        if self.real.dot(reference.real) < 0.0 {
            DualQuat {
                real: self.real.neg(),
                dual: self.dual.neg(),
            }
        } else {
            self
        }
    }

    /// Multiply both parts by `s`.
    pub fn scale(self, s: f32) -> DualQuat {
        DualQuat {
            real: self.real.scale(s),
            dual: self.dual.scale(s),
        }
    }

    /// Component-wise sum of both parts.
    pub fn add(self, o: DualQuat) -> DualQuat {
        DualQuat {
            real: self.real.add(o.real),
            dual: self.dual.add(o.dual),
        }
    }

    /// Normalize by the dual-number norm: with `n = |real|`, `real' = real / n` and
    /// `dual' = (dual - real' * (real' . dual)) / n`, which makes `real' . dual' = 0`.
    /// `None` when `|real|^2` is below [`DEGENERATE_BLEND_NORM2`] or anything is non-finite.
    pub fn try_normalize(self) -> Option<DualQuat> {
        let n2 = self.real.length_squared();
        if n2 < DEGENERATE_BLEND_NORM2 || !n2.is_finite() {
            return None;
        }
        let n = n2.sqrt();
        let inv = 1.0 / n;
        let real = self.real.scale(inv);
        let proj = real.dot(self.dual);
        let dual = self.dual.add(real.scale(-proj)).scale(inv);
        if real.is_finite() && dual.is_finite() {
            Some(DualQuat { real, dual })
        } else {
            None
        }
    }

    /// Dual-quaternion linear blend of `(weight, dq)` terms: every term is sign-aligned to the
    /// FIRST term, `weight * dq` is summed in slice order starting from zero, and the sum is
    /// normalized. `None` for an empty slice or a degenerate (cancelled) sum. Weights are used as
    /// given; the normalization makes the result independent of their sum.
    pub fn blend(terms: &[(f32, DualQuat)]) -> Option<DualQuat> {
        let first = terms.first()?.1;
        let zero = Quat::new(0.0, 0.0, 0.0, 0.0);
        let mut acc = DualQuat {
            real: zero,
            dual: zero,
        };
        for &(w, dq) in terms {
            acc = acc.add(dq.aligned_to(&first).scale(w));
        }
        acc.try_normalize()
    }

    /// The translation `t = 2 * (dual * conjugate(real))`, vector part (unit `real`).
    pub fn translation(&self) -> Vec3 {
        let c = self.dual.mul(self.real.conjugate());
        Vec3::new(2.0 * c.x, 2.0 * c.y, 2.0 * c.z)
    }

    /// Transform a point: rotate by `real`, then add the translation.
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        self.real.rotate(p).add(self.translation())
    }

    /// Transform a direction or normal: rotation only.
    pub fn transform_vector(&self, v: Vec3) -> Vec3 {
        self.real.rotate(v)
    }
}

impl Default for DualQuat {
    fn default() -> DualQuat {
        DualQuat::IDENTITY
    }
}

impl Default for Quat {
    fn default() -> Quat {
        Quat::IDENTITY
    }
}

impl Default for Mat4 {
    fn default() -> Mat4 {
        Mat4::IDENTITY
    }
}

impl Transform {
    /// The identity transform.
    pub const IDENTITY: Transform = Transform {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
    };

    /// Construct from translation, rotation and scale.
    pub const fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Transform {
        Transform {
            translation,
            rotation,
            scale,
        }
    }

    /// A pure translation.
    pub const fn from_translation(translation: Vec3) -> Transform {
        Transform::new(translation, Quat::IDENTITY, Vec3::new(1.0, 1.0, 1.0))
    }

    /// True when translation, rotation and scale are finite and the rotation has non-zero
    /// length.
    pub fn is_valid(&self) -> bool {
        self.translation.is_finite()
            && self.scale.is_finite()
            && self.rotation.is_finite()
            && self.rotation.try_normalize().is_some()
    }

    /// The matrix of this transform (`T * R * S`).
    pub fn to_mat(&self) -> Mat4 {
        Mat4::from_transform(self)
    }
}

impl Default for Transform {
    fn default() -> Transform {
        Transform::IDENTITY
    }
}
