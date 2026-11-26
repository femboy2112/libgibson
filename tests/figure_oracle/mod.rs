//! Independent f64 reference for the figure geometry core.
//!
//! Deliberately shares NO code with `gibson::figure::math`: row-major `[[f64; 4]; 4]`
//! matrices, a different quaternion-to-matrix formula (`(w^2 - |v|^2) I + 2 v v^T + 2 w [v]x`),
//! a general Gauss-Jordan inverse with partial pivoting (not the affine shortcut), and LBS as
//! an explicit per-vertex sum applying `IB` then `W` to the point (the library premultiplies
//! `W * IB`). Inputs are plain arrays; nothing from `gibson` appears in this file.
#![allow(dead_code)]
#![allow(clippy::needless_range_loop)]

pub type M = [[f64; 4]; 4];

/// A joint's local TRS; `q` is `[x, y, z, w]`.
#[derive(Clone, Copy, Debug)]
pub struct OTrs {
    pub t: [f64; 3],
    pub q: [f64; 4],
    pub s: [f64; 3],
}

#[derive(Clone, Debug)]
pub struct OJoint {
    pub parent: Option<usize>,
    pub rest: OTrs,
}

/// A control: its default and what it does (by index into joints / morph targets).
#[derive(Clone, Debug)]
pub struct OControl {
    pub default: f64,
    pub bone_scale_of_children_of: Option<usize>,
    pub morph_target: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct ORig {
    pub joints: Vec<OJoint>,
    pub positions: Vec<[f64; 3]>,
    /// Per vertex `(joint, weight)`.
    pub skin: Vec<Vec<(usize, f64)>>,
    /// Per target, sparse `(vertex, delta)`.
    pub morphs: Vec<Vec<(usize, [f64; 3])>>,
    pub controls: Vec<OControl>,
    /// Per landmark `(joint, offset in that joint's frame)`.
    pub landmarks: Vec<(usize, [f64; 3])>,
}

/// A pose: local-TRS overrides (replacing the shaped rest local) and extra morph weights.
#[derive(Clone, Debug, Default)]
pub struct OPose {
    pub locals: Vec<(usize, OTrs)>,
    pub morph_weights: Vec<(usize, f64)>,
}

pub fn identity() -> M {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

pub fn matmul(a: &M, b: &M) -> M {
    let mut out = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[i][k] * b[k][j];
            }
            out[i][j] = s;
        }
    }
    out
}

pub fn matvec_point(m: &M, p: [f64; 3]) -> [f64; 3] {
    let mut out = [0.0; 3];
    for i in 0..3 {
        out[i] = m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3];
    }
    out
}

/// Rotation matrix (3x3 embedded in a 4x4) of quaternion `[x, y, z, w]`, normalized here:
/// `R = (w^2 - |v|^2) I + 2 v v^T + 2 w [v]x`.
pub fn rotation(q: [f64; 4]) -> M {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let v = [q[0] / n, q[1] / n, q[2] / n];
    let w = q[3] / n;
    let vv = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    let cross = [[0.0, -v[2], v[1]], [v[2], 0.0, -v[0]], [-v[1], v[0], 0.0]];
    let mut m = identity();
    for i in 0..3 {
        for j in 0..3 {
            let id = if i == j { 1.0 } else { 0.0 };
            m[i][j] = (w * w - vv) * id + 2.0 * v[i] * v[j] + 2.0 * w * cross[i][j];
        }
    }
    m
}

/// `T * R * S` as `T`, then `R`, then `S` matrix products (not a column-scaling shortcut).
pub fn trs_matrix(t: &OTrs) -> M {
    let mut tm = identity();
    for i in 0..3 {
        tm[i][3] = t.t[i];
    }
    let mut sm = identity();
    for i in 0..3 {
        sm[i][i] = t.s[i];
    }
    matmul(&matmul(&tm, &rotation(t.q)), &sm)
}

/// General 4x4 inverse by Gauss-Jordan elimination with partial pivoting.
pub fn invert(m: &M) -> Option<M> {
    let mut a = [[0.0f64; 8]; 4];
    for i in 0..4 {
        for j in 0..4 {
            a[i][j] = m[i][j];
        }
        a[i][4 + i] = 1.0;
    }
    for col in 0..4 {
        let mut piv = col;
        for r in col + 1..4 {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, piv);
        let d = a[col][col];
        for j in 0..8 {
            a[col][j] /= d;
        }
        for r in 0..4 {
            if r != col {
                let f = a[r][col];
                if f != 0.0 {
                    for j in 0..8 {
                        a[r][j] -= f * a[col][j];
                    }
                }
            }
        }
    }
    let mut out = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            out[i][j] = a[i][4 + j];
        }
    }
    Some(out)
}

fn world_matrices(joints: &[OJoint], locals: &[OTrs]) -> Vec<M> {
    let mut world: Vec<M> = Vec::new();
    for (i, j) in joints.iter().enumerate() {
        let l = trs_matrix(&locals[i]);
        let w = match j.parent {
            Some(p) => matmul(&world[p], &l),
            None => l,
        };
        world.push(w);
    }
    world
}

/// The shaped rest TRS list for `control_values` (one per control): each bone-length control
/// multiplies the rest translation of every direct child of its joint, controls in order.
pub fn shaped_rest(rig: &ORig, control_values: &[f64]) -> Vec<OTrs> {
    assert_eq!(
        control_values.len(),
        rig.controls.len(),
        "one value per control"
    );
    let mut rest: Vec<OTrs> = rig.joints.iter().map(|j| j.rest).collect();
    for (c, &v) in rig.controls.iter().zip(control_values) {
        if let Some(pj) = c.bone_scale_of_children_of {
            for (i, j) in rig.joints.iter().enumerate() {
                if j.parent == Some(pj) {
                    for k in 0..3 {
                        rest[i].t[k] *= v;
                    }
                }
            }
        }
    }
    rest
}

/// The shaped neutral: `G0 + sum_i w_i * delta_i` with `w_i` the sum of control values mapped
/// to target `i`.
pub fn shaped_neutral(rig: &ORig, control_values: &[f64]) -> Vec<[f64; 3]> {
    assert_eq!(
        control_values.len(),
        rig.controls.len(),
        "one value per control"
    );
    let mut w = vec![0.0; rig.morphs.len()];
    for (c, &v) in rig.controls.iter().zip(control_values) {
        if let Some(t) = c.morph_target {
            w[t] += v;
        }
    }
    apply_morph_weights(&rig.positions, &rig.morphs, &w)
}

pub fn apply_morph_weights(
    base: &[[f64; 3]],
    morphs: &[Vec<(usize, [f64; 3])>],
    w: &[f64],
) -> Vec<[f64; 3]> {
    let mut out = base.to_vec();
    for (t, &wt) in morphs.iter().zip(w) {
        for &(v, d) in t {
            for k in 0..3 {
                out[v][k] += wt * d[k];
            }
        }
    }
    out
}

/// Inverse bind matrices of the SHAPED skeleton, via the general inverse.
pub fn inverse_binds(rig: &ORig, control_values: &[f64]) -> Vec<M> {
    let rest = shaped_rest(rig, control_values);
    world_matrices(&rig.joints, &rest)
        .iter()
        .map(|w| invert(w).expect("oracle bind matrix must be invertible"))
        .collect()
}

pub fn world_bind(rig: &ORig, control_values: &[f64]) -> Vec<M> {
    world_matrices(&rig.joints, &shaped_rest(rig, control_values))
}

/// Posed joint world matrices.
pub fn posed_world(rig: &ORig, control_values: &[f64], pose: &OPose) -> Vec<M> {
    let mut locals = shaped_rest(rig, control_values);
    for &(j, t) in &pose.locals {
        locals[j] = t;
    }
    world_matrices(&rig.joints, &locals)
}

/// World position of landmark `index`: its offset carried by joint world matrix `world[joint]`.
pub fn landmark_world(rig: &ORig, world: &[M], index: usize) -> [f64; 3] {
    let (joint, offset) = rig.landmarks[index];
    matvec_point(&world[joint], offset)
}

/// The full reference: posed vertex positions for `control_values` and `pose`.
pub fn posed(rig: &ORig, control_values: &[f64], pose: &OPose) -> Vec<[f64; 3]> {
    let neutral = shaped_neutral(rig, control_values);
    let mut pw = vec![0.0; rig.morphs.len()];
    for &(t, w) in &pose.morph_weights {
        pw[t] += w;
    }
    let geometry = apply_morph_weights(&neutral, &rig.morphs, &pw);
    let world = posed_world(rig, control_values, pose);
    let ibs = inverse_binds(rig, control_values);
    let mut out = Vec::new();
    for (v, p) in geometry.iter().enumerate() {
        let mut acc = [0.0; 3];
        for &(j, w) in &rig.skin[v] {
            let bound = matvec_point(&ibs[j], *p);
            let q = matvec_point(&world[j], bound);
            for k in 0..3 {
                acc[k] += w * q[k];
            }
        }
        out.push(acc);
    }
    out
}
