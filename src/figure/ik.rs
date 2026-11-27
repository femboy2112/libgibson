//! Analytic two-bone inverse kinematics with a pole vector and a soft reach limit. EXPERIMENTAL.
//!
//! The solver is the closed form of the law of cosines. For a limb `root -> mid -> end` with bone
//! lengths `l1 = |mid - root|` and `l2 = |end - mid|`, and a target at distance `d` from `root`
//! along the unit axis `u`:
//!
//! ```text
//!   a  = (l1^2 - l2^2 + d^2) / (2 d)          distance from root to the foot of the mid joint
//!   h  = sqrt(l1^2 - a^2)                      height of the mid joint above the axis
//!   mid = root + a u + h p                     p = the pole direction, made perpendicular to u
//! ```
//!
//! so the mid joint always lies in the plane through `root`, `target` and the pole vector (the
//! knee or elbow bends toward the pole), and `|end - target| = 0` whenever the target is inside
//! the reachable annulus.
//!
//! # Soft reach limit (no snap at full extension)
//!
//! A hard clamp `d' = min(d, l1 + l2)` is continuous in the end position but not in the mid joint:
//! `h = sqrt(l1^2 - a^2)` has infinite slope as `d' -> l1 + l2`, so the knee pops as the target
//! crosses full extension. [`soft_reach`] instead leaves `d` unchanged up to
//! `soft_start * (l1 + l2)` and then approaches `l1 + l2` asymptotically,
//! `d' = d0 + (L - d0) tanh((d - d0) / (L - d0))`, which is `C1` at `d0` (slope 1 on both sides)
//! and never reaches full extension, so `h` stays smooth. A target nearer than
//! `|l1 - l2| + MIN_FOLD * (l1 + l2)` is clamped to that distance (the fold limit; continuous,
//! with a slope kink).
//!
//! The pole vector must not be parallel to the limb axis; when it is (or is zero) a deterministic
//! perpendicular is used, and the bend direction may flip there. Callers keep the pole away from
//! the axis (a knee pole is "forward", a limb axis is "down").
//!
//! Pure scalar `f32`, no `unsafe`, no allocation, no clock.

use super::math::{Quat, Vec3};

/// Reach ratio (of `l1 + l2`) up to which the end effector reaches the target exactly.
pub const DEFAULT_SOFT_START: f32 = 0.98;
/// Fold limit as a fraction of `l1 + l2`, added to `|l1 - l2|`.
pub const MIN_FOLD: f32 = 0.05;

/// The geometric solution of one two-bone problem, in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoBoneSolution {
    /// The mid joint (knee or elbow).
    pub mid: Vec3,
    /// The end effector actually reached (equals the target when it is reachable).
    pub end: Vec3,
    /// The root-to-end distance used, after the soft limit.
    pub reach: f32,
    /// True when the soft or fold limit changed the requested distance.
    pub limited: bool,
}

/// Local and world rotations of a solved limb.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimbPose {
    /// World rotation of the upper joint.
    pub upper_world: Quat,
    /// World rotation of the lower joint.
    pub lower_world: Quat,
    /// Local rotation of the upper joint (relative to its parent).
    pub upper_local: Quat,
    /// Local rotation of the lower joint (relative to the upper joint).
    pub lower_local: Quat,
    /// The positions the rotations realize.
    pub solution: TwoBoneSolution,
}

/// Everything [`solve_limb`] needs. Rotations are unit quaternions, positions world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimbInput {
    /// World rotation of the upper joint's parent.
    pub parent_world: Quat,
    /// Rest local rotation of the upper joint.
    pub upper_rest_rot: Quat,
    /// Rest local rotation of the lower joint.
    pub lower_rest_rot: Quat,
    /// Rest local translation of the lower joint (the upper bone, in the upper frame).
    pub upper_to_lower: Vec3,
    /// Rest local translation of the end joint (the lower bone, in the lower frame).
    pub lower_to_end: Vec3,
    /// World position of the upper joint.
    pub root: Vec3,
    /// World position the end joint should reach.
    pub target: Vec3,
    /// A direction (world) the mid joint bends toward.
    pub pole: Vec3,
    /// See [`soft_reach`]; [`DEFAULT_SOFT_START`] is a sensible value.
    pub soft_start: f32,
}

fn unit_or(v: Vec3, eps: f32) -> Option<Vec3> {
    let l = v.length();
    if l.is_finite() && l > eps {
        Some(v.scale(1.0 / l))
    } else {
        None
    }
}

/// A deterministic unit vector perpendicular to the unit vector `u`.
fn any_perp(u: Vec3) -> Vec3 {
    let axis = if u.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    u.cross(axis)
        .try_normalize()
        .unwrap_or(Vec3::new(0.0, 0.0, 1.0))
}

/// The shortest-arc rotation taking direction `a` to direction `b` (both need not be unit).
/// A zero vector gives the identity; antipodal vectors give a half turn about a fixed
/// perpendicular of `a`.
pub fn shortest_arc(a: Vec3, b: Vec3) -> Quat {
    let (a, b) = match (unit_or(a, 1.0e-9), unit_or(b, 1.0e-9)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Quat::IDENTITY,
    };
    let d = a.dot(b);
    if d < -0.999_999 {
        return Quat::from_axis_angle(any_perp(a), std::f32::consts::PI);
    }
    let c = a.cross(b);
    Quat::new(c.x, c.y, c.z, 1.0 + d).normalize()
}

/// The root-to-end distance the limb will use for a requested distance `d`: unchanged inside
/// `[fold, soft_start * (l1 + l2)]`, asymptotic to `l1 + l2` beyond (see the module docs).
/// `soft_start >= 1` is a hard clamp (kept only as the control of the soft limit's test).
pub fn soft_reach(d: f32, l1: f32, l2: f32, soft_start: f32) -> f32 {
    let lmax = l1 + l2;
    let lo = (l1 - l2).abs() + MIN_FOLD * lmax;
    if d < lo {
        return lo;
    }
    if soft_start >= 1.0 {
        return d.min(lmax);
    }
    let d0 = soft_start.max(0.0) * lmax;
    if d <= d0 {
        return d;
    }
    let span = lmax - d0;
    d0 + span * ((d - d0) / span).tanh()
}

/// The point on the ray from `root` toward `target` the limb can actually reach.
pub fn limit_reach(root: Vec3, target: Vec3, l1: f32, l2: f32, soft_start: f32) -> Vec3 {
    let to = target.sub(root);
    let d = to.length();
    match unit_or(to, 1.0e-9) {
        Some(u) => root.add(u.scale(soft_reach(d, l1, l2, soft_start))),
        None => target,
    }
}

/// Solve the positions of a two-bone limb. See the module docs.
pub fn solve_two_bone(
    root: Vec3,
    target: Vec3,
    pole: Vec3,
    l1: f32,
    l2: f32,
    soft_start: f32,
) -> TwoBoneSolution {
    let to = target.sub(root);
    let d_raw = to.length();
    let u = unit_or(to, 1.0e-9)
        .or_else(|| unit_or(pole, 1.0e-9))
        .unwrap_or(Vec3::new(0.0, 0.0, -1.0));
    let d = soft_reach(d_raw, l1, l2, soft_start);
    let end = root.add(u.scale(d));
    let a = (l1 * l1 - l2 * l2 + d * d) / (2.0 * d.max(1.0e-9));
    let h2 = l1 * l1 - a * a;
    let h = if h2 > 0.0 { h2.sqrt() } else { 0.0 };
    let pp = pole.sub(u.scale(pole.dot(u)));
    let pdir = unit_or(pp, 1.0e-6 * (1.0 + pole.length())).unwrap_or_else(|| any_perp(u));
    let mid = root.add(u.scale(a)).add(pdir.scale(h));
    TwoBoneSolution {
        mid,
        end,
        reach: d,
        limited: (d - d_raw).abs() > 1.0e-7,
    }
}

/// Solve a limb and return the rotations that realize it. Each bone is rotated by the shortest
/// arc from its inherited direction (the rest local rotation under the parent's world rotation),
/// which keeps the twist about the bone as close to the parent's as possible.
pub fn solve_limb(i: &LimbInput) -> LimbPose {
    let l1 = i.upper_to_lower.length();
    let l2 = i.lower_to_end.length();
    let sol = solve_two_bone(i.root, i.target, i.pole, l1, l2, i.soft_start);

    let inh_u = i.parent_world.mul(i.upper_rest_rot);
    let dir_u = inh_u.rotate(i.upper_to_lower);
    let upper_world = shortest_arc(dir_u, sol.mid.sub(i.root))
        .mul(inh_u)
        .normalize();

    let inh_l = upper_world.mul(i.lower_rest_rot);
    let dir_l = inh_l.rotate(i.lower_to_end);
    let lower_world = shortest_arc(dir_l, sol.end.sub(sol.mid))
        .mul(inh_l)
        .normalize();

    LimbPose {
        upper_world,
        lower_world,
        upper_local: i.parent_world.conjugate().mul(upper_world).normalize(),
        lower_local: upper_world.conjugate().mul(lower_world).normalize(),
        solution: sol,
    }
}

/// Forward kinematics of a solved limb: `(mid, end)` world positions from the rotations
/// [`solve_limb`] returned. Used by tests as the independent check of the solver.
pub fn limb_fk(i: &LimbInput, pose: &LimbPose) -> (Vec3, Vec3) {
    let mid = i.root.add(pose.upper_world.rotate(i.upper_to_lower));
    let end = mid.add(pose.lower_world.rotate(i.lower_to_end));
    (mid, end)
}
