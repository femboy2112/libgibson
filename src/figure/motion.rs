//! Continuous clips, inertialized transitions and a Look retarget (C-1). EXPERIMENTAL.
//!
//! RED STUB: the public surface only. Every behavioural function is `todo!()`; the
//! implementation lands in the next commit, so the tests in `tests/figure_motion.rs` show
//! RED to GREEN. See `docs/FIGURE_CONTRACT.md`, "Continuous clips and transitions (C-1)".

use std::fmt;
use std::time::Duration;

use super::clip::{ChannelTarget, Clip};
use super::math::{Quat, Vec3};

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
    /// A unit rotation and its angular velocity (radians per second, parent-space axis).
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

    /// This pose with every track of `over` replacing the same target, and new targets appended.
    pub fn overlay(&self, _over: &MotionPose) -> MotionPose {
        todo!("C-1 RED stub")
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
}

impl fmt::Display for MotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownChannel { channel } => write!(f, "clip has no channel {channel}"),
            Self::BadDirection => write!(f, "direction is zero-length or not finite"),
            Self::RetargetNotIncreasing => write!(f, "retarget times are not strictly increasing"),
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
        self,
        _channel: usize,
        _mode: Interpolation,
    ) -> Result<MotionClip, MotionError> {
        todo!("C-1 RED stub")
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

    /// Evaluate every channel at local time `t`.
    pub fn sample_at(&self, _t: Duration) -> MotionPose {
        todo!("C-1 RED stub")
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
    fn sample(&self, _t: Duration) -> MotionPose {
        todo!("C-1 RED stub")
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
    fn sample(&self, _t: Duration) -> MotionPose {
        todo!("C-1 RED stub")
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
    fn sample(&self, _t: Duration) -> MotionPose {
        todo!("C-1 RED stub")
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
    /// Aim `joint`'s `forward` axis along `initial`; retargets blend over `ease`.
    pub fn new(
        _joint: impl Into<String>,
        _forward: Vec3,
        _initial: Vec3,
        _ease: Duration,
    ) -> Result<Look, MotionError> {
        todo!("C-1 RED stub")
    }

    /// From `at`, aim along `direction`. Times must be strictly increasing.
    pub fn retarget(self, _at: Duration, _direction: Vec3) -> Result<Look, MotionError> {
        todo!("C-1 RED stub")
    }
}

impl Motion for Look {
    fn sample(&self, _t: Duration) -> MotionPose {
        todo!("C-1 RED stub")
    }
}
