//! Finite, declarative, data-only animation clips.
//!
//! A [`Clip`] is typed channels of keys over a finite [`Duration`]. It contains no scripts,
//! callbacks or closures: it is plain data that can be validated, compared and hashed.
//!
//! - [`Clip::sample`] evaluates at an **explicit** time `t`. There is no clock and no implicit
//!   loop. Before the first key a channel holds its first key; after the last it holds its last.
//!   [`ClipSample::settled`] is true exactly when `t >= duration`, meaning no further change.
//! - Vec3 (translation, scale) and `f32` (morph weight) keys interpolate linearly.
//! - Quaternion keys interpolate by **normalized lerp** (nlerp) with the hemisphere sign fix:
//!   if the dot product of the two keys is negative, the second is negated first, so the blend
//!   takes the short way round (`q` and `-q` are the same rotation). nlerp was chosen over slerp
//!   for determinism and simplicity: it needs only `+ * /` and one `sqrt`, with no `acos`/`sin`
//!   and no small-angle special cases. Its angular speed is not constant, which is acceptable
//!   for authored clips. Rotation keys are normalized once in [`Clip::new`], so every sampled
//!   rotation is unit length.
//! - Targets are named. [`Clip::bind`] resolves names to indices against a revision's joint and
//!   morph names and fails there, not silently at sample time, when a target is unknown.

use std::fmt;
use std::time::Duration;

use super::math::{Quat, Vec3};

/// Longest clip accepted: 10 minutes.
pub const MAX_DURATION: Duration = Duration::from_secs(600);
/// Most channels in one clip.
pub const MAX_CHANNELS: usize = 1024;
/// Most keys in one channel.
pub const MAX_KEYS_PER_CHANNEL: usize = 16_384;
/// Allowed deviation of a rotation key's length from 1.
pub const QUAT_UNIT_TOLERANCE: f32 = 1e-3;

/// What a channel animates, by name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChannelTarget {
    /// Local translation of the named joint.
    JointTranslation(String),
    /// Local rotation of the named joint.
    JointRotation(String),
    /// Local scale of the named joint.
    JointScale(String),
    /// Weight of the named morph target.
    MorphWeight(String),
}

/// The keys of one channel. The variant must match the channel's [`ChannelTarget`].
#[derive(Debug, Clone, PartialEq)]
pub enum Keys {
    /// For [`ChannelTarget::JointTranslation`].
    Translation(Vec<(Duration, Vec3)>),
    /// For [`ChannelTarget::JointRotation`].
    Rotation(Vec<(Duration, Quat)>),
    /// For [`ChannelTarget::JointScale`].
    Scale(Vec<(Duration, Vec3)>),
    /// For [`ChannelTarget::MorphWeight`].
    Weight(Vec<(Duration, f32)>),
}

/// One animated target and its keys.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    /// What is animated.
    pub target: ChannelTarget,
    /// Keys, strictly increasing in time, each at most the clip duration.
    pub keys: Keys,
}

/// Why a clip was rejected, or could not be bound.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClipError {
    /// The clip duration is zero.
    ZeroDuration,
    /// The clip duration exceeds [`MAX_DURATION`].
    DurationTooLong {
        /// The offending duration.
        duration: Duration,
    },
    /// More than [`MAX_CHANNELS`] channels.
    TooManyChannels {
        /// The channel count given.
        count: usize,
    },
    /// A channel has no keys.
    EmptyChannel {
        /// Index of the channel.
        channel: usize,
    },
    /// A channel has more than [`MAX_KEYS_PER_CHANNEL`] keys.
    TooManyKeys {
        /// Index of the channel.
        channel: usize,
        /// The key count given.
        count: usize,
    },
    /// The key variant does not match the channel target.
    KeyTypeMismatch {
        /// Index of the channel.
        channel: usize,
    },
    /// Two channels animate the same target.
    DuplicateTarget {
        /// Index of the second channel.
        channel: usize,
    },
    /// Key times are not strictly increasing.
    KeysNotIncreasing {
        /// Index of the channel.
        channel: usize,
        /// Index of the key that is not later than its predecessor.
        key: usize,
    },
    /// A key lies after the clip duration.
    KeyBeyondDuration {
        /// Index of the channel.
        channel: usize,
        /// Index of the key.
        key: usize,
    },
    /// A key value is NaN or infinite.
    NonFiniteValue {
        /// Index of the channel.
        channel: usize,
        /// Index of the key.
        key: usize,
    },
    /// A rotation key's length is not within [`QUAT_UNIT_TOLERANCE`] of 1.
    NonUnitQuaternion {
        /// Index of the channel.
        channel: usize,
        /// Index of the key.
        key: usize,
    },
    /// At bind time: a joint target names no joint.
    UnknownJoint {
        /// The unresolved name.
        name: String,
    },
    /// At bind time: a morph target names no morph.
    UnknownMorph {
        /// The unresolved name.
        name: String,
    },
}

impl fmt::Display for ClipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDuration => write!(f, "clip duration is zero"),
            Self::DurationTooLong { duration } => {
                write!(
                    f,
                    "clip duration {duration:?} exceeds the cap {MAX_DURATION:?}"
                )
            }
            Self::TooManyChannels { count } => {
                write!(f, "{count} channels exceeds the cap {MAX_CHANNELS}")
            }
            Self::EmptyChannel { channel } => write!(f, "channel {channel} has no keys"),
            Self::TooManyKeys { channel, count } => write!(
                f,
                "channel {channel} has {count} keys, exceeding the cap {MAX_KEYS_PER_CHANNEL}"
            ),
            Self::KeyTypeMismatch { channel } => {
                write!(f, "channel {channel}: key type does not match its target")
            }
            Self::DuplicateTarget { channel } => {
                write!(f, "channel {channel} repeats an earlier channel's target")
            }
            Self::KeysNotIncreasing { channel, key } => write!(
                f,
                "channel {channel}: key {key} is not strictly later than the previous key"
            ),
            Self::KeyBeyondDuration { channel, key } => {
                write!(f, "channel {channel}: key {key} is after the clip duration")
            }
            Self::NonFiniteValue { channel, key } => {
                write!(f, "channel {channel}: key {key} has a non-finite value")
            }
            Self::NonUnitQuaternion { channel, key } => {
                write!(f, "channel {channel}: key {key} is not a unit quaternion")
            }
            Self::UnknownJoint { name } => write!(f, "clip targets unknown joint {name:?}"),
            Self::UnknownMorph { name } => write!(f, "clip targets unknown morph {name:?}"),
        }
    }
}

impl std::error::Error for ClipError {}

/// A validated finite clip. Construct with [`Clip::new`].
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    name: String,
    duration: Duration,
    channels: Vec<Channel>,
}

/// Values of every channel at one time, addressed by target name, in channel order.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipSample {
    /// `(joint name, local translation)`.
    pub translations: Vec<(String, Vec3)>,
    /// `(joint name, local rotation)`, unit quaternions.
    pub rotations: Vec<(String, Quat)>,
    /// `(joint name, local scale)`.
    pub scales: Vec<(String, Vec3)>,
    /// `(morph name, weight)`.
    pub weights: Vec<(String, f32)>,
    /// True iff `t >= duration`: no further change will occur.
    pub settled: bool,
}

/// A clip resolved against a revision's joint and morph names. Construct with [`Clip::bind`].
#[derive(Debug, Clone, PartialEq)]
pub struct BoundClip {
    duration: Duration,
    translations: Vec<(usize, Vec<(Duration, Vec3)>)>,
    rotations: Vec<(usize, Vec<(Duration, Quat)>)>,
    scales: Vec<(usize, Vec<(Duration, Vec3)>)>,
    weights: Vec<(usize, Vec<(Duration, f32)>)>,
}

/// Values of every channel at one time, addressed by joint or morph index.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundSample {
    /// `(joint index, local translation)`.
    pub translations: Vec<(usize, Vec3)>,
    /// `(joint index, local rotation)`, unit quaternions.
    pub rotations: Vec<(usize, Quat)>,
    /// `(joint index, local scale)`.
    pub scales: Vec<(usize, Vec3)>,
    /// `(morph index, weight)`.
    pub weights: Vec<(usize, f32)>,
    /// True iff `t >= duration`: no further change will occur.
    pub settled: bool,
}

impl Clip {
    /// Validate and build a clip.
    pub fn new(
        _name: impl Into<String>,
        _duration: Duration,
        _channels: Vec<Channel>,
    ) -> Result<Self, ClipError> {
        todo!("RED stub")
    }

    /// The clip's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The clip's finite duration.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// The validated channels.
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// Evaluate every channel at `t`. No clock, no looping.
    pub fn sample(&self, _t: Duration) -> ClipSample {
        todo!("RED stub")
    }

    /// Resolve target names to indices. Fails on an unknown joint or morph. If a name appears
    /// twice in a list, the first occurrence wins.
    pub fn bind(
        &self,
        _joint_names: &[&str],
        _morph_names: &[&str],
    ) -> Result<BoundClip, ClipError> {
        todo!("RED stub")
    }
}

impl BoundClip {
    /// The clip's duration.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Evaluate every channel at `t`, index-addressed.
    pub fn sample(&self, _t: Duration) -> BoundSample {
        todo!("RED stub")
    }
}
