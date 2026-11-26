//! Continuous clips, inertialized transitions and a Look retarget (C-1). EXPERIMENTAL.
//!
//! Owner ruling 17: model continuous motion, not jarring stop motion. Everything here is a
//! **pure function of explicit presentation time**: [`Motion::sample`] takes a [`Duration`] and
//! returns a pose and its rates. There is no wall clock and no state that a call advances, so
//! sampling twice, or in any order, gives bit-identical output. Velocity is at least C1
//! continuous, and every transition blends over an explicit `ease` (`Settle.ease`, a BLEND
//! duration) instead of snapping. `ease == 0` is the one documented exception: it is the
//! designed static realization (reduced motion) and switches at once.
//!
//! Contract text: `docs/FIGURE_CONTRACT.md`, "Continuous clips and transitions (C-1)". The
//! bounds the tests assert are derived in `tests/figure_motion.rs`; the numeric model is
//! `docs/evidence/m5b/c1/model_c1.py`.
//!
//! # Smooth keys
//!
//! [`Interpolation::Smooth`] is **time-parameterized Catmull-Rom**: a cubic Hermite per segment
//! with the three-point tangent `m_i = (p_{i+1} - p_{i-1}) / (t_{i+1} - t_{i-1})`, one value per
//! key shared by both neighbouring segments, so the velocity is continuous at every key. This is
//! the uniform-tension form made time-aware, not the centripetal form: centripetal Catmull-Rom
//! chooses the curve parameter by chord length to avoid cusps and self-intersections of a
//! *spatial* curve, but here the parameter is the explicit presentation time and each channel is
//! an independent function of it, so there is no spatial curve to cusp. The tangent is a
//! weighted mean of the two chord slopes, so a segment never exceeds `3.5 S` in speed for chord
//! slope `S` (the bound the tests use). At the ends of a one-shot clip the tangent is clamped to
//! zero, which is exactly the velocity of the hold outside the keys: the hold joins C1.
//!
//! Rotations interpolate **component-wise on hemisphere-aligned quaternions and are then
//! renormalized**. This is not exactly constant angular speed (nor a geodesic); it is C1 and
//! monotone enough for authored motion. Alignment is relative to the segment's start key, so an
//! authored pair more than 180 degrees apart (negative dot) can flip the output quaternion's
//! sign between segments; the rotation itself stays continuous. Values at keys are returned as
//! authored (not renormalized).
//!
//! # Loops
//!
//! [`Repeat::Loop`] wraps time exactly (integer nanoseconds) with period [`Clip::duration`] and
//! uses wrap tangents: neighbours across the seam are the keys of the previous or next period,
//! so velocity is continuous across it. If the first key is at 0 and the last at the duration,
//! they are the same instant; the last key is dropped from the cycle and the first key's value
//! wins. A clip with one key in the cycle is constant.
//!
//! # Transitions
//!
//! * [`Crossfade`]: `out = (1 - w) A + w B` with `w = 6x^5 - 15x^4 + 10x^3` (smootherstep, `w' =
//!   w'' = 0` at both ends, so it is C2 at the joins). Rates include `w' (B - A)`. Rotations use
//!   nlerp with the hemisphere fix, with an analytic rate.
//! * [`Preempt`]: inertialization. At the instant `t0` the offset `old(t0) - new(t0)` and its
//!   velocity `old'(t0) - new'(t0)` are captured; the offset then follows the quintic
//!   `x0 h0(x) + v0 T h1(x)`, `x = (t - t0) / T`, which starts at `(x0, v0)` and ends at zero
//!   value, velocity and acceleration at `t0 + T`. The old pose is NOT frozen: freezing it makes
//!   the velocity drop to zero at `t0`, a visible hitch. After `t0 + T` the output is the new
//!   motion bit for bit. For rotations the offset is the shortest-arc delta `q_old q_new^-1`
//!   expressed as a rotation vector `r`; its decay velocity is `J^-1(r) w` with `w` the delta
//!   angular velocity, so the angular velocity is preserved exactly (the SO(3) left Jacobian).
//!   The quintic does not clamp: a large `v0` against `x0` can overshoot.
//! * Targets only one side names blend through a caller-supplied `rest` pose (default values:
//!   translation 0, rotation identity, scale 1, weight 0). After a preemption ends, targets the
//!   new motion does not name leave the pose, which equals `rest` at that instant, so there is
//!   no snap if `rest` is the consumer's rest pose.
//!
//! # Look
//!
//! [`Look`] aims one joint's `forward` axis along a direction (expressed in the joint's parent
//! space) with the shortest-arc rotation. Each retarget is a [`Preempt`] between constant
//! rotations, so a retarget, even in the middle of an earlier retarget's blend, is inertialized.
//! The layer owns the joint's rotation: [`MotionPose::overlay`] replaces that track.
//!
//! Zero dependencies, scalar `f32` (the Jacobian coefficients are evaluated in `f64`), no
//! `unsafe`.

use std::fmt;
use std::time::Duration;

use super::clip::{ChannelTarget, Clip, Keys};
use super::math::{Quat, Vec3};

/// Most retargets one [`Look`] accepts (bounds the depth of a retarget chain).
pub const MAX_RETARGETS: usize = 4096;

/// How the keys of one channel are interpolated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    /// Piecewise linear (rotations: nlerp with the hemisphere fix). Velocity jumps at keys.
    Linear,
    /// Catmull-Rom: C1 velocity across keys.
    Smooth,
}

/// What a clip does after its last key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    /// Hold the last key.
    Once,
    /// Wrap with period [`Clip::duration`], with wrap tangents.
    Loop,
}

/// One target's value and its rate of change at an instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sample {
    /// Translation or scale, and its velocity per second.
    Vec3 {
        /// Value.
        value: Vec3,
        /// Per-second rate.
        rate: Vec3,
    },
    /// A unit rotation and its angular velocity in radians per second, in the parent frame:
    /// `q' = 1/2 omega q`.
    Rotation {
        /// Value.
        value: Quat,
        /// Angular velocity.
        omega: Vec3,
    },
    /// A morph weight and its rate per second.
    Scalar {
        /// Value.
        value: f32,
        /// Per-second rate.
        rate: f32,
    },
}

/// Values and rates of a set of targets at one instant, in a stable order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MotionPose {
    tracks: Vec<(ChannelTarget, Sample)>,
}

impl MotionPose {
    /// Build from tracks.
    pub fn new(tracks: Vec<(ChannelTarget, Sample)>) -> MotionPose {
        MotionPose { tracks }
    }

    /// The tracks, in order.
    pub fn tracks(&self) -> &[(ChannelTarget, Sample)] {
        &self.tracks
    }

    /// The sample of `target`, if present.
    pub fn get(&self, target: &ChannelTarget) -> Option<&Sample> {
        self.tracks
            .iter()
            .find(|(t, _)| t == target)
            .map(|(_, s)| s)
    }

    /// This pose with every track of `over` replacing the same target, and targets only `over`
    /// names appended.
    pub fn overlay(&self, over: &MotionPose) -> MotionPose {
        let mut tracks: Vec<(ChannelTarget, Sample)> = self
            .tracks
            .iter()
            .map(|(t, s)| (t.clone(), over.get(t).copied().unwrap_or(*s)))
            .collect();
        for (t, s) in &over.tracks {
            if self.get(t).is_none() {
                tracks.push((t.clone(), *s));
            }
        }
        MotionPose { tracks }
    }
}

/// Why a motion object could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MotionError {
    /// A channel index does not exist in the clip.
    UnknownChannel {
        /// The offending index.
        channel: usize,
    },
    /// A direction or axis is zero-length or not finite.
    BadDirection,
    /// Retarget times are not strictly increasing.
    RetargetNotIncreasing,
    /// More than [`MAX_RETARGETS`] retargets.
    TooManyRetargets,
}

impl fmt::Display for MotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownChannel { channel } => write!(f, "clip has no channel {channel}"),
            Self::BadDirection => write!(f, "direction is zero-length or not finite"),
            Self::RetargetNotIncreasing => write!(f, "retarget times are not strictly increasing"),
            Self::TooManyRetargets => write!(f, "more than {MAX_RETARGETS} retargets"),
        }
    }
}

impl std::error::Error for MotionError {}

/// A pure function of explicit presentation time: no clock, no hidden state.
pub trait Motion {
    /// The pose and its rates at `t`.
    fn sample(&self, t: Duration) -> MotionPose;
}

impl<M: Motion + ?Sized> Motion for &M {
    fn sample(&self, t: Duration) -> MotionPose {
        (**self).sample(t)
    }
}

impl<M: Motion + ?Sized> Motion for Box<M> {
    fn sample(&self, t: Duration) -> MotionPose {
        (**self).sample(t)
    }
}

/// A [`Clip`] with per-channel interpolation and a repeat mode.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionClip {
    clip: Clip,
    interpolation: Vec<Interpolation>,
    repeat: Repeat,
}

impl MotionClip {
    /// Wrap a clip: every channel [`Interpolation::Smooth`], [`Repeat::Once`].
    pub fn new(clip: Clip) -> MotionClip {
        let n = clip.channels().len();
        MotionClip {
            clip,
            interpolation: vec![Interpolation::Smooth; n],
            repeat: Repeat::Once,
        }
    }

    /// Set the interpolation of channel `channel` (index into [`Clip::channels`]).
    pub fn with_interpolation(
        mut self,
        channel: usize,
        mode: Interpolation,
    ) -> Result<MotionClip, MotionError> {
        if channel >= self.interpolation.len() {
            return Err(MotionError::UnknownChannel { channel });
        }
        self.interpolation[channel] = mode;
        Ok(self)
    }

    /// Set the repeat mode.
    pub fn with_repeat(mut self, repeat: Repeat) -> MotionClip {
        self.repeat = repeat;
        self
    }

    /// The wrapped clip.
    pub fn clip(&self) -> &Clip {
        &self.clip
    }

    /// Evaluate every channel at local time `t`, in channel order.
    pub fn sample_at(&self, t: Duration) -> MotionPose {
        let period = match self.repeat {
            Repeat::Loop => Some(self.clip.duration()),
            Repeat::Once => None,
        };
        let mut tracks = Vec::with_capacity(self.clip.channels().len());
        for (idx, ch) in self.clip.channels().iter().enumerate() {
            // Fault injection (mutation control `linear_keys`): smooth falls back to linear.
            let smooth = self.interpolation[idx] == Interpolation::Smooth
                && !cfg!(figure_mutant = "linear_keys");
            let sample = match &ch.keys {
                Keys::Translation(k) | Keys::Scale(k) => {
                    finish_vec3(&eval_keys(k, smooth, false, period, t))
                }
                Keys::Rotation(k) => finish_quat(&eval_keys(k, smooth, true, period, t)),
                Keys::Weight(k) => finish_scalar(&eval_keys(k, smooth, false, period, t)),
            };
            tracks.push((ch.target.clone(), sample));
        }
        MotionPose { tracks }
    }
}

/// A [`MotionClip`] that started at `start` on the presentation clock.
#[derive(Debug, Clone, PartialEq)]
pub struct Playing {
    clip: MotionClip,
    start: Duration,
}

impl Playing {
    /// `clip` started at `start`. Before `start` the pose is the clip's local time 0.
    pub fn new(clip: MotionClip, start: Duration) -> Playing {
        Playing { clip, start }
    }
}

impl Motion for Playing {
    fn sample(&self, t: Duration) -> MotionPose {
        self.clip.sample_at(t.saturating_sub(self.start))
    }
}

/// Crossfade from one motion to another over `ease`, with a C2 weight.
#[derive(Debug, Clone)]
pub struct Crossfade<A, B> {
    from: A,
    to: B,
    start: Duration,
    ease: Duration,
    rest: MotionPose,
}

impl<A: Motion, B: Motion> Crossfade<A, B> {
    /// Blend `from` into `to` over `[start, start + ease]`.
    pub fn new(from: A, to: B, start: Duration, ease: Duration) -> Crossfade<A, B> {
        Crossfade {
            from,
            to,
            start,
            ease,
            rest: MotionPose::default(),
        }
    }

    /// Values for targets that only one side names.
    pub fn with_rest(mut self, rest: MotionPose) -> Crossfade<A, B> {
        self.rest = rest;
        self
    }
}

impl<A: Motion, B: Motion> Motion for Crossfade<A, B> {
    fn sample(&self, t: Duration) -> MotionPose {
        if t < self.start {
            return self.from.sample(t);
        }
        // Fault injection (mutation control `snap_transition`): no blend.
        let ease = if cfg!(figure_mutant = "snap_transition") {
            Duration::ZERO
        } else {
            self.ease
        };
        let elapsed = t - self.start;
        if elapsed >= ease {
            return self.to.sample(t);
        }
        let tt = ease.as_secs_f32();
        let x = (elapsed.as_secs_f64() / ease.as_secs_f64()) as f32;
        let a = self.from.sample(t);
        let b = self.to.sample(t);
        let tracks = union_targets(&a, &b)
            .into_iter()
            .map(|tg| {
                let sa = sample_or_default(&a, &tg, &self.rest);
                let sb = sample_or_default(&b, &tg, &self.rest);
                let s = blend_sample(sa, sb, smoother(x), smoother_rate(x) / tt);
                (tg, s)
            })
            .collect();
        MotionPose { tracks }
    }
}

/// Preempt `old` by `new` at `at`, by inertialization over `ease`.
#[derive(Debug, Clone)]
pub struct Preempt<A, B> {
    old: A,
    new: B,
    at: Duration,
    ease: Duration,
    rest: MotionPose,
}

impl<A: Motion, B: Motion> Preempt<A, B> {
    /// `old` until `at`, then `new` plus a decaying offset that preserves `old`'s velocity.
    pub fn new(old: A, new: B, at: Duration, ease: Duration) -> Preempt<A, B> {
        Preempt {
            old,
            new,
            at,
            ease,
            rest: MotionPose::default(),
        }
    }

    /// Values for targets that only one side names.
    pub fn with_rest(mut self, rest: MotionPose) -> Preempt<A, B> {
        self.rest = rest;
        self
    }
}

impl<A: Motion, B: Motion> Motion for Preempt<A, B> {
    fn sample(&self, t: Duration) -> MotionPose {
        if t < self.at {
            return self.old.sample(t);
        }
        // Fault injection (mutation control `snap_transition`): no blend.
        let ease = if cfg!(figure_mutant = "snap_transition") {
            Duration::ZERO
        } else {
            self.ease
        };
        let elapsed = t - self.at;
        if elapsed >= ease {
            return self.new.sample(t);
        }
        let tt = ease.as_secs_f32();
        let x = (elapsed.as_secs_f64() / ease.as_secs_f64()) as f32;
        // The offset is a function of the state at the preemption instant, recomputed here, so
        // no state is carried between calls.
        let o0 = self.old.sample(self.at);
        let n0 = self.new.sample(self.at);
        let nt = self.new.sample(t);
        let tracks = union_targets(&o0, &nt)
            .into_iter()
            .map(|tg| {
                let so = sample_or_default(&o0, &tg, &self.rest);
                let sn0 = sample_or_default(&n0, &tg, &self.rest);
                let snt = sample_or_default(&nt, &tg, &self.rest);
                (tg, inertialize(so, sn0, snt, x, tt))
            })
            .collect();
        MotionPose { tracks }
    }
}

/// A generic aim layer: rotates one joint so that `forward` points along a direction, with
/// every retarget inertialized.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    joint: String,
    forward: Vec3,
    initial: Vec3,
    retargets: Vec<(Duration, Vec3)>,
    ease: Duration,
}

impl Look {
    /// Aim `joint`'s `forward` axis along `initial` (both in the joint's parent space; any
    /// non-zero length); retargets blend over `ease`.
    pub fn new(
        joint: impl Into<String>,
        forward: Vec3,
        initial: Vec3,
        ease: Duration,
    ) -> Result<Look, MotionError> {
        Ok(Look {
            joint: joint.into(),
            forward: unit(forward)?,
            initial: unit(initial)?,
            retargets: Vec::new(),
            ease,
        })
    }

    /// From `at`, aim along `direction`. Times must be strictly increasing.
    pub fn retarget(mut self, at: Duration, direction: Vec3) -> Result<Look, MotionError> {
        let d = unit(direction)?;
        if self.retargets.len() >= MAX_RETARGETS {
            return Err(MotionError::TooManyRetargets);
        }
        if let Some((last, _)) = self.retargets.last() {
            if at <= *last {
                return Err(MotionError::RetargetNotIncreasing);
            }
        }
        self.retargets.push((at, d));
        Ok(self)
    }

    /// The rotation that aims `forward` along the direction after `k` retargets.
    fn target(&self, k: usize) -> Quat {
        let d = if k == 0 {
            self.initial
        } else {
            self.retargets[k - 1].1
        };
        rotation_between(self.forward, d)
    }

    /// Rotation and angular velocity at `t`, given that exactly `count` retargets (those at or
    /// before `t`) have fired.
    fn aim_state(&self, count: usize, t: Duration) -> (Quat, Vec3) {
        let new = self.target(count);
        if count == 0 {
            return (new, Vec3::ZERO);
        }
        let at = self.retargets[count - 1].0;
        // Fault injection (mutation control `snap_transition`): no blend.
        let ease = if cfg!(figure_mutant = "snap_transition") {
            Duration::ZERO
        } else {
            self.ease
        };
        let elapsed = t.saturating_sub(at);
        if elapsed >= ease {
            return (new, Vec3::ZERO);
        }
        let tt = ease.as_secs_f32();
        let x = (elapsed.as_secs_f64() / ease.as_secs_f64()) as f32;
        // Retarget times are strictly increasing, so `count - 1` retargets have fired by `at`.
        let old = self.aim_state(count - 1, at);
        inertialize_rot(old, (new, Vec3::ZERO), (new, Vec3::ZERO), x, tt)
    }
}

impl Motion for Look {
    fn sample(&self, t: Duration) -> MotionPose {
        let count = self.retargets.partition_point(|(at, _)| *at <= t);
        let (value, omega) = self.aim_state(count, t);
        MotionPose {
            tracks: vec![(
                ChannelTarget::JointRotation(self.joint.clone()),
                Sample::Rotation { value, omega },
            )],
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Key evaluation.
// ---------------------------------------------------------------------------------------------

/// Four-component working vector: `f32` uses `[v, 0, 0, 0]`, `Vec3` `[x, y, z, 0]`, `Quat` `xyzw`.
type V4 = [f32; 4];

trait Chan: Copy {
    fn to4(self) -> V4;
}

impl Chan for f32 {
    fn to4(self) -> V4 {
        [self, 0.0, 0.0, 0.0]
    }
}

impl Chan for Vec3 {
    fn to4(self) -> V4 {
        [self.x, self.y, self.z, 0.0]
    }
}

impl Chan for Quat {
    fn to4(self) -> V4 {
        [self.x, self.y, self.z, self.w]
    }
}

struct Eval {
    value: V4,
    rate: V4,
    /// True at a key or in a hold: the value is returned as authored.
    exact: bool,
}

fn hold(v: V4) -> Eval {
    Eval {
        value: v,
        rate: [0.0; 4],
        exact: true,
    }
}

fn dot4(a: V4, b: V4) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// `a`, or `-a` if it lies in the opposite hemisphere from `reference`.
fn align(a: V4, reference: V4) -> V4 {
    if dot4(a, reference) < 0.0 {
        [-a[0], -a[1], -a[2], -a[3]]
    } else {
        a
    }
}

fn tangent(hi: V4, lo: V4, dt: f32) -> V4 {
    [
        (hi[0] - lo[0]) / dt,
        (hi[1] - lo[1]) / dt,
        (hi[2] - lo[2]) / dt,
        (hi[3] - lo[3]) / dt,
    ]
}

/// Value and per-second derivative of one non-empty, strictly increasing key list at `t`.
/// `period` is `Some` for a loop. Total: never panics, never returns NaN for finite keys.
fn eval_keys<T: Chan>(
    keys: &[(Duration, T)],
    smooth: bool,
    quat: bool,
    period: Option<Duration>,
    t: Duration,
) -> Eval {
    let n = keys.len();
    match period {
        None => {
            // Index of the first key strictly after `t`.
            let next = keys.partition_point(|(kt, _)| *kt <= t);
            if next == 0 {
                return hold(keys[0].1.to4());
            }
            if next == n {
                return hold(keys[n - 1].1.to4());
            }
            let get = |j: isize| -> Option<(f64, V4)> {
                if j < 0 || j as usize >= n {
                    None
                } else {
                    let k = &keys[j as usize];
                    Some((k.0.as_secs_f64(), k.1.to4()))
                }
            };
            segment(&get, next as isize - 1, t.as_secs_f64(), smooth, quat)
        }
        Some(p) => {
            // The first key at 0 and the last at the period are one instant: drop the last.
            let mut ne = n;
            if ne > 1 && keys[0].0.is_zero() && keys[ne - 1].0 == p {
                ne -= 1;
            }
            if ne == 1 {
                return hold(keys[0].1.to4());
            }
            let tt = Duration::from_nanos((t.as_nanos() % p.as_nanos()) as u64);
            let next = keys[..ne].partition_point(|(kt, _)| *kt <= tt);
            let ps = p.as_secs_f64();
            let m = ne as isize;
            // Neighbours across the seam are the keys of the adjacent periods.
            let get = |j: isize| -> Option<(f64, V4)> {
                let k = &keys[j.rem_euclid(m) as usize];
                Some((k.0.as_secs_f64() + j.div_euclid(m) as f64 * ps, k.1.to4()))
            };
            segment(&get, next as isize - 1, tt.as_secs_f64(), smooth, quat)
        }
    }
}

/// Evaluate the segment `[get(i), get(i + 1)]` at time `ts` (seconds).
#[allow(clippy::needless_range_loop)]
fn segment(
    get: &dyn Fn(isize) -> Option<(f64, V4)>,
    i: isize,
    ts: f64,
    smooth: bool,
    quat: bool,
) -> Eval {
    let (Some((t1, p1)), Some((t2, p2))) = (get(i), get(i + 1)) else {
        // Unreachable: callers pick `i` with both ends present.
        return hold([0.0; 4]);
    };
    let h = (t2 - t1) as f32;
    let s = ((ts - t1) / (t2 - t1)) as f32;
    let a1 = p1;
    let a2 = if quat { align(p2, a1) } else { p2 };
    let mut value = [0.0f32; 4];
    let mut rate = [0.0f32; 4];
    if !smooth {
        for c in 0..4 {
            value[c] = (1.0 - s) * a1[c] + s * a2[c];
            rate[c] = (a2[c] - a1[c]) / h;
        }
        return Eval {
            value,
            rate,
            exact: s == 0.0,
        };
    }
    // Clamped end tangent (zero) where a neighbour is missing.
    let m1 = match get(i - 1) {
        Some((t0, p0)) => {
            let a0 = if quat { align(p0, a1) } else { p0 };
            tangent(a2, a0, (t2 - t0) as f32)
        }
        None => [0.0; 4],
    };
    let m2 = match get(i + 2) {
        Some((t3, p3)) => {
            let a3 = if quat { align(p3, a2) } else { p3 };
            tangent(a3, a1, (t3 - t1) as f32)
        }
        None => [0.0; 4],
    };
    let s2 = s * s;
    let s3 = s2 * s;
    let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
    let h10 = s3 - 2.0 * s2 + s;
    let h01 = -2.0 * s3 + 3.0 * s2;
    let h11 = s3 - s2;
    let d00 = 6.0 * s2 - 6.0 * s;
    let d10 = 3.0 * s2 - 4.0 * s + 1.0;
    let d01 = -6.0 * s2 + 6.0 * s;
    let d11 = 3.0 * s2 - 2.0 * s;
    for c in 0..4 {
        value[c] = h00 * a1[c] + h10 * h * m1[c] + h01 * a2[c] + h11 * h * m2[c];
        rate[c] = (d00 * a1[c] + d10 * h * m1[c] + d01 * a2[c] + d11 * h * m2[c]) / h;
    }
    Eval {
        value,
        rate,
        exact: s == 0.0,
    }
}

fn finish_vec3(e: &Eval) -> Sample {
    Sample::Vec3 {
        value: Vec3::new(e.value[0], e.value[1], e.value[2]),
        rate: Vec3::new(e.rate[0], e.rate[1], e.rate[2]),
    }
}

fn finish_scalar(e: &Eval) -> Sample {
    Sample::Scalar {
        value: e.value[0],
        rate: e.rate[0],
    }
}

fn finish_quat(e: &Eval) -> Sample {
    let n = Quat::new(e.value[0], e.value[1], e.value[2], e.value[3]);
    let nd = Quat::new(e.rate[0], e.rate[1], e.rate[2], e.rate[3]);
    let (qh, omega) = normalize_with_rate(n, nd);
    Sample::Rotation {
        value: if e.exact { n } else { qh },
        omega,
    }
}

// ---------------------------------------------------------------------------------------------
// Quaternion and rotation-vector helpers.
// ---------------------------------------------------------------------------------------------

fn quat_conj(q: Quat) -> Quat {
    Quat::new(-q.x, -q.y, -q.z, q.w)
}

/// `q' = 1/2 omega q` as a quaternion.
fn omega_to_qdot(q: Quat, w: Vec3) -> Quat {
    let p = Quat::new(w.x, w.y, w.z, 0.0).mul(q);
    Quat::new(0.5 * p.x, 0.5 * p.y, 0.5 * p.z, 0.5 * p.w)
}

/// `omega = 2 q' q^-1` for a unit `q`.
fn qdot_to_omega(q: Quat, qd: Quat) -> Vec3 {
    let w = qd.mul(quat_conj(q));
    Vec3::new(2.0 * w.x, 2.0 * w.y, 2.0 * w.z)
}

/// Normalize `n` and carry its derivative `nd` through the normalization:
/// `q = n / |n|`, `q' = (n' - q (q . n')) / |n|`. Returns `q` and the angular velocity.
fn normalize_with_rate(n: Quat, nd: Quat) -> (Quat, Vec3) {
    let rho = n.length_squared().sqrt();
    if !(rho > 0.0 && rho.is_finite()) {
        return (Quat::IDENTITY, Vec3::ZERO);
    }
    let q = Quat::new(n.x / rho, n.y / rho, n.z / rho, n.w / rho);
    let k = q.x * nd.x + q.y * nd.y + q.z * nd.z + q.w * nd.w;
    let d = Quat::new(
        (nd.x - q.x * k) / rho,
        (nd.y - q.y * k) / rho,
        (nd.z - q.z * k) / rho,
        (nd.w - q.w * k) / rho,
    );
    (q, qdot_to_omega(q, d))
}

/// Rotate `v` by the unit quaternion `q`.
fn rotate(q: Quat, v: Vec3) -> Vec3 {
    let u = Vec3::new(q.x, q.y, q.z);
    let t = u.cross(v).scale(2.0);
    v.add(t.scale(q.w)).add(u.cross(t))
}

/// Shortest-arc rotation vector (axis times angle, angle in `[0, pi]`) of a unit quaternion.
fn quat_log(q: Quat) -> Vec3 {
    let q = if q.w < 0.0 {
        Quat::new(-q.x, -q.y, -q.z, -q.w)
    } else {
        q
    };
    let s = (q.x * q.x + q.y * q.y + q.z * q.z).sqrt();
    if s < 1.0e-7 {
        return Vec3::new(2.0 * q.x, 2.0 * q.y, 2.0 * q.z);
    }
    let k = 2.0 * s.atan2(q.w) / s;
    Vec3::new(q.x * k, q.y * k, q.z * k)
}

/// Unit quaternion of a rotation vector.
fn quat_exp(r: Vec3) -> Quat {
    let th = r.length();
    if th < 1.0e-7 {
        return Quat::new(0.5 * r.x, 0.5 * r.y, 0.5 * r.z, 1.0).normalize();
    }
    let half = 0.5 * th;
    let k = half.sin() / th;
    Quat::new(r.x * k, r.y * k, r.z * k, half.cos())
}

/// `(a, b)` with `J(r) = I + a [r]x + b [r]x^2`, the left Jacobian of SO(3), so that the
/// angular velocity of `exp(r(t))` is `J(r) r'`. Evaluated in `f64`, with a series for small
/// angles, because `1 - cos` and `theta - sin` cancel in `f32`.
fn jac_coeffs(theta: f32) -> (f32, f32) {
    let th = f64::from(theta);
    if th < 1.0e-3 {
        let t2 = th * th;
        return ((0.5 - t2 / 24.0) as f32, (1.0 / 6.0 - t2 / 120.0) as f32);
    }
    (
        ((1.0 - th.cos()) / (th * th)) as f32,
        ((th - th.sin()) / (th * th * th)) as f32,
    )
}

/// `c` with `J^-1(r) = I - 1/2 [r]x + c [r]x^2`, `c = (1 - (theta/2) cot(theta/2)) / theta^2`.
/// Finite at `theta = pi` (the largest shortest-arc angle).
fn jac_inv_coeff(theta: f32) -> f32 {
    let th = f64::from(theta);
    if th < 1.0e-3 {
        return (1.0 / 12.0 + th * th / 720.0) as f32;
    }
    let half = 0.5 * th;
    ((1.0 - half * half.cos() / half.sin()) / (th * th)) as f32
}

fn jac_apply(r: Vec3, v: Vec3) -> Vec3 {
    let (a, b) = jac_coeffs(r.length());
    let c1 = r.cross(v);
    let c2 = r.cross(c1);
    v.add(c1.scale(a)).add(c2.scale(b))
}

fn jac_inv_apply(r: Vec3, v: Vec3) -> Vec3 {
    let c = jac_inv_coeff(r.length());
    let c1 = r.cross(v);
    let c2 = r.cross(c1);
    v.sub(c1.scale(0.5)).add(c2.scale(c))
}

/// Normalize a direction; a zero-length or non-finite one is an error.
fn unit(v: Vec3) -> Result<Vec3, MotionError> {
    let len = v.length();
    if !v.is_finite() || !len.is_finite() || len <= 1.0e-6 {
        return Err(MotionError::BadDirection);
    }
    Ok(v.scale(1.0 / len))
}

/// The shortest-arc rotation taking unit `a` to unit `b`: `(a x b, 1 + a . b)` normalized. For
/// antipodal vectors the 180 degree axis is a fixed perpendicular of `a` (deterministic).
fn rotation_between(a: Vec3, b: Vec3) -> Quat {
    let d = a.dot(b);
    if d < -0.999_999 {
        let helper = if a.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let p = a.cross(helper);
        return Quat::new(p.x, p.y, p.z, 0.0).normalize();
    }
    let c = a.cross(b);
    Quat::new(c.x, c.y, c.z, 1.0 + d).normalize()
}

// ---------------------------------------------------------------------------------------------
// Transition profiles and blending.
// ---------------------------------------------------------------------------------------------

/// Smootherstep `6x^5 - 15x^4 + 10x^3`: C2 at both ends.
fn smoother(x: f32) -> f32 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// `d smoother / dx = 30 x^2 (1 - x)^2`.
fn smoother_rate(x: f32) -> f32 {
    let o = 1.0 - x;
    30.0 * x * x * o * o
}

/// Quintic with `h0(0) = 1` and `h0'(0) = h0''(0) = 0`, `h0(1) = h0'(1) = h0''(1) = 0`.
fn h0(x: f32) -> f32 {
    1.0 - smoother(x)
}

fn dh0(x: f32) -> f32 {
    -smoother_rate(x)
}

/// Quintic with `h1(0) = 0`, `h1'(0) = 1`, `h1''(0) = 0`, and zero value, slope and
/// curvature at 1.
fn h1(x: f32) -> f32 {
    let x2 = x * x;
    let x3 = x2 * x;
    x - 6.0 * x3 + 8.0 * x3 * x - 3.0 * x3 * x2
}

fn dh1(x: f32) -> f32 {
    let x2 = x * x;
    let x3 = x2 * x;
    1.0 - 18.0 * x2 + 32.0 * x3 - 15.0 * x3 * x
}

fn held(s: Sample) -> Sample {
    match s {
        Sample::Vec3 { value, .. } => Sample::Vec3 {
            value,
            rate: Vec3::ZERO,
        },
        Sample::Rotation { value, .. } => Sample::Rotation {
            value,
            omega: Vec3::ZERO,
        },
        Sample::Scalar { value, .. } => Sample::Scalar { value, rate: 0.0 },
    }
}

/// The value a target takes where a side does not name it: the supplied rest value (held), else
/// translation 0, rotation identity, scale 1, weight 0.
fn default_sample(target: &ChannelTarget, rest: &MotionPose) -> Sample {
    if let Some(s) = rest.get(target) {
        return held(*s);
    }
    match target {
        ChannelTarget::JointTranslation(_) => Sample::Vec3 {
            value: Vec3::ZERO,
            rate: Vec3::ZERO,
        },
        ChannelTarget::JointScale(_) => Sample::Vec3 {
            value: Vec3::new(1.0, 1.0, 1.0),
            rate: Vec3::ZERO,
        },
        ChannelTarget::JointRotation(_) => Sample::Rotation {
            value: Quat::IDENTITY,
            omega: Vec3::ZERO,
        },
        ChannelTarget::MorphWeight(_) => Sample::Scalar {
            value: 0.0,
            rate: 0.0,
        },
    }
}

fn sample_or_default(p: &MotionPose, target: &ChannelTarget, rest: &MotionPose) -> Sample {
    p.get(target)
        .copied()
        .unwrap_or_else(|| default_sample(target, rest))
}

/// Targets of `a` in order, then targets only `b` names.
fn union_targets(a: &MotionPose, b: &MotionPose) -> Vec<ChannelTarget> {
    let mut out: Vec<ChannelTarget> = a.tracks.iter().map(|(t, _)| t.clone()).collect();
    for (t, _) in &b.tracks {
        if !out.contains(t) {
            out.push(t.clone());
        }
    }
    out
}

fn lerp(a: f32, b: f32, w: f32) -> f32 {
    (1.0 - w) * a + w * b
}

/// Derivative of `(1 - w) a + w b` when `a`, `b` and `w` all vary.
fn lerp_rate(a: f32, b: f32, ra: f32, rb: f32, w: f32, dw: f32) -> f32 {
    (1.0 - w) * ra + w * rb + dw * (b - a)
}

/// Blend two samples of one target by weight `w` (rate `dw` per second). `w <= 0` is exactly
/// `a`, `w >= 1` exactly `b`.
fn blend_sample(a: Sample, b: Sample, w: f32, dw: f32) -> Sample {
    if w <= 0.0 {
        return a;
    }
    if w >= 1.0 {
        return b;
    }
    match (a, b) {
        (
            Sample::Vec3 {
                value: va,
                rate: ra,
            },
            Sample::Vec3 {
                value: vb,
                rate: rb,
            },
        ) => Sample::Vec3 {
            value: Vec3::new(
                lerp(va.x, vb.x, w),
                lerp(va.y, vb.y, w),
                lerp(va.z, vb.z, w),
            ),
            rate: Vec3::new(
                lerp_rate(va.x, vb.x, ra.x, rb.x, w, dw),
                lerp_rate(va.y, vb.y, ra.y, rb.y, w, dw),
                lerp_rate(va.z, vb.z, ra.z, rb.z, w, dw),
            ),
        },
        (
            Sample::Scalar {
                value: va,
                rate: ra,
            },
            Sample::Scalar {
                value: vb,
                rate: rb,
            },
        ) => Sample::Scalar {
            value: lerp(va, vb, w),
            rate: lerp_rate(va, vb, ra, rb, w, dw),
        },
        (
            Sample::Rotation {
                value: qa,
                omega: wa,
            },
            Sample::Rotation {
                value: qb,
                omega: wb,
            },
        ) => {
            let (value, omega) = blend_rot(qa, wa, qb, wb, w, dw);
            Sample::Rotation { value, omega }
        }
        // One target never changes kind; keep the destination if it somehow does.
        (_, other) => other,
    }
}

/// nlerp with the hemisphere fix, with the analytic angular velocity.
fn blend_rot(qa: Quat, wa: Vec3, qb: Quat, wb: Vec3, w: f32, dw: f32) -> (Quat, Vec3) {
    let dot = qa.x * qb.x + qa.y * qb.y + qa.z * qb.z + qa.w * qb.w;
    let sg = if dot < 0.0 { -1.0 } else { 1.0 };
    let qb2 = Quat::new(sg * qb.x, sg * qb.y, sg * qb.z, sg * qb.w);
    let da = omega_to_qdot(qa, wa);
    let db0 = omega_to_qdot(qb, wb);
    let db = Quat::new(sg * db0.x, sg * db0.y, sg * db0.z, sg * db0.w);
    let n = Quat::new(
        lerp(qa.x, qb2.x, w),
        lerp(qa.y, qb2.y, w),
        lerp(qa.z, qb2.z, w),
        lerp(qa.w, qb2.w, w),
    );
    let nd = Quat::new(
        lerp_rate(qa.x, qb2.x, da.x, db.x, w, dw),
        lerp_rate(qa.y, qb2.y, da.y, db.y, w, dw),
        lerp_rate(qa.z, qb2.z, da.z, db.z, w, dw),
        lerp_rate(qa.w, qb2.w, da.w, db.w, w, dw),
    );
    normalize_with_rate(n, nd)
}

/// The preempted sample of one target at `x = (t - t0) / T` in `[0, 1)`: the new motion plus the
/// quintic offset, built from the old and new samples at `t0` (`so`, `sn0`) and the new sample
/// at `t` (`snt`). `tt` is `T` in seconds.
fn inertialize(so: Sample, sn0: Sample, snt: Sample, x: f32, tt: f32) -> Sample {
    // Fault injection (mutation control `freeze_on_preempt`): the old pose is held static and
    // faded out, so the velocity at the preemption instant drops to zero.
    if cfg!(figure_mutant = "freeze_on_preempt") {
        return blend_sample(held(so), snt, smoother(x), smoother_rate(x) / tt);
    }
    let (a0, da0, a1, da1) = (h0(x), dh0(x) / tt, h1(x), dh1(x));
    match (so, sn0, snt) {
        (
            Sample::Vec3 {
                value: vo,
                rate: ro,
            },
            Sample::Vec3 {
                value: vn0,
                rate: rn0,
            },
            Sample::Vec3 {
                value: vnt,
                rate: rnt,
            },
        ) => {
            let x0 = vo.sub(vn0);
            let v0 = ro.sub(rn0);
            let off = x0.scale(a0).add(v0.scale(tt * a1));
            let offr = x0.scale(da0).add(v0.scale(da1));
            Sample::Vec3 {
                value: vnt.add(off),
                rate: rnt.add(offr),
            }
        }
        (
            Sample::Scalar {
                value: vo,
                rate: ro,
            },
            Sample::Scalar {
                value: vn0,
                rate: rn0,
            },
            Sample::Scalar {
                value: vnt,
                rate: rnt,
            },
        ) => {
            let x0 = vo - vn0;
            let v0 = ro - rn0;
            Sample::Scalar {
                value: vnt + (x0 * a0 + v0 * (tt * a1)),
                rate: rnt + (x0 * da0 + v0 * da1),
            }
        }
        (
            Sample::Rotation {
                value: qo,
                omega: wo,
            },
            Sample::Rotation {
                value: qn0,
                omega: wn0,
            },
            Sample::Rotation {
                value: qnt,
                omega: wnt,
            },
        ) => {
            let (value, omega) = inertialize_rot((qo, wo), (qn0, wn0), (qnt, wnt), x, tt);
            Sample::Rotation { value, omega }
        }
        // One target never changes kind; keep the destination if it somehow does.
        (_, _, other) => other,
    }
}

/// Rotation inertialization. `q_off = q_old q_new0^-1` (shortest arc) is held as a rotation
/// vector `r0`; `r(x) = r0 h0 + w0 T h1` with `w0 = J^-1(r0) (omega_old - R(q_off) omega_new0)`;
/// the output is `exp(r) q_new(t)`, with angular velocity `J(r) r' + R(exp r) omega_new(t)`. At
/// `x = 0` this is `q_old` and `omega_old` exactly; at `x = 1` the offset is the identity with
/// zero angular velocity and acceleration. The sign of the output follows `q_old` at `t0`.
fn inertialize_rot(
    old: (Quat, Vec3),
    new0: (Quat, Vec3),
    newt: (Quat, Vec3),
    x: f32,
    tt: f32,
) -> (Quat, Vec3) {
    // Fault injection (mutation control `freeze_on_preempt`), as in `inertialize`.
    if cfg!(figure_mutant = "freeze_on_preempt") {
        return blend_rot(
            old.0,
            Vec3::ZERO,
            newt.0,
            newt.1,
            smoother(x),
            smoother_rate(x) / tt,
        );
    }
    let off = old.0.mul(quat_conj(new0.0)).normalize();
    let flip = if off.w < 0.0 { -1.0 } else { 1.0 };
    let r0 = quat_log(off);
    let w_off0 = old.1.sub(rotate(off, new0.1));
    let w0 = jac_inv_apply(r0, w_off0);
    let r = r0.scale(h0(x)).add(w0.scale(tt * h1(x)));
    let rd = r0.scale(dh0(x) / tt).add(w0.scale(dh1(x)));
    let e = quat_exp(r);
    let p = e.mul(newt.0);
    let q = Quat::new(flip * p.x, flip * p.y, flip * p.z, flip * p.w).normalize();
    let omega = jac_apply(r, rd).add(rotate(e, newt.1));
    (q, omega)
}
