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
//!   for authored clips. Rotation keys are accepted within [`QUAT_UNIT_TOLERANCE`] of unit
//!   length and are returned as authored at and beyond the keys; any blended rotation is
//!   exactly renormalized.
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
        name: impl Into<String>,
        duration: Duration,
        channels: Vec<Channel>,
    ) -> Result<Self, ClipError> {
        if duration.is_zero() {
            return Err(ClipError::ZeroDuration);
        }
        if duration > MAX_DURATION {
            return Err(ClipError::DurationTooLong { duration });
        }
        if channels.len() > MAX_CHANNELS {
            return Err(ClipError::TooManyChannels {
                count: channels.len(),
            });
        }
        for (ci, ch) in channels.iter().enumerate() {
            if channels[..ci].iter().any(|p| p.target == ch.target) {
                return Err(ClipError::DuplicateTarget { channel: ci });
            }
            let matches = matches!(
                (&ch.target, &ch.keys),
                (ChannelTarget::JointTranslation(_), Keys::Translation(_))
                    | (ChannelTarget::JointRotation(_), Keys::Rotation(_))
                    | (ChannelTarget::JointScale(_), Keys::Scale(_))
                    | (ChannelTarget::MorphWeight(_), Keys::Weight(_))
            );
            if !matches {
                return Err(ClipError::KeyTypeMismatch { channel: ci });
            }
            match &ch.keys {
                Keys::Translation(k) | Keys::Scale(k) => validate_keys(ci, k, duration, |v| {
                    if v.x.is_finite() && v.y.is_finite() && v.z.is_finite() {
                        Ok(())
                    } else {
                        Err(KeyFault::NonFinite)
                    }
                })?,
                Keys::Rotation(k) => validate_keys(ci, k, duration, |q| {
                    if !(q.x.is_finite() && q.y.is_finite() && q.z.is_finite() && q.w.is_finite()) {
                        return Err(KeyFault::NonFinite);
                    }
                    let len = (q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w).sqrt();
                    if (len - 1.0).abs() <= QUAT_UNIT_TOLERANCE {
                        Ok(())
                    } else {
                        Err(KeyFault::NonUnit)
                    }
                })?,
                Keys::Weight(k) => validate_keys(ci, k, duration, |w| {
                    if w.is_finite() {
                        Ok(())
                    } else {
                        Err(KeyFault::NonFinite)
                    }
                })?,
            }
        }
        Ok(Self {
            name: name.into(),
            duration,
            channels,
        })
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
    pub fn sample(&self, t: Duration) -> ClipSample {
        let mut out = ClipSample {
            translations: Vec::new(),
            rotations: Vec::new(),
            scales: Vec::new(),
            weights: Vec::new(),
            settled: t >= self.duration,
        };
        for ch in &self.channels {
            match (&ch.target, &ch.keys) {
                (ChannelTarget::JointTranslation(n), Keys::Translation(k)) => {
                    out.translations.push((n.clone(), sample_keys(k, t)))
                }
                (ChannelTarget::JointRotation(n), Keys::Rotation(k)) => {
                    out.rotations.push((n.clone(), sample_keys(k, t)))
                }
                (ChannelTarget::JointScale(n), Keys::Scale(k)) => {
                    out.scales.push((n.clone(), sample_keys(k, t)))
                }
                (ChannelTarget::MorphWeight(n), Keys::Weight(k)) => {
                    out.weights.push((n.clone(), sample_keys(k, t)))
                }
                // `Clip::new` rejects mismatched target/key pairs.
                _ => {}
            }
        }
        out
    }

    /// Resolve target names to indices. Fails on an unknown joint or morph. If a name appears
    /// twice in a list, the first occurrence wins.
    pub fn bind(&self, joint_names: &[&str], morph_names: &[&str]) -> Result<BoundClip, ClipError> {
        let joint = |n: &str| {
            joint_names
                .iter()
                .position(|j| *j == n)
                .ok_or_else(|| ClipError::UnknownJoint {
                    name: n.to_string(),
                })
        };
        let morph = |n: &str| {
            morph_names
                .iter()
                .position(|m| *m == n)
                .ok_or_else(|| ClipError::UnknownMorph {
                    name: n.to_string(),
                })
        };
        let mut out = BoundClip {
            duration: self.duration,
            translations: Vec::new(),
            rotations: Vec::new(),
            scales: Vec::new(),
            weights: Vec::new(),
        };
        for ch in &self.channels {
            match (&ch.target, &ch.keys) {
                (ChannelTarget::JointTranslation(n), Keys::Translation(k)) => {
                    out.translations.push((joint(n)?, k.clone()))
                }
                (ChannelTarget::JointRotation(n), Keys::Rotation(k)) => {
                    out.rotations.push((joint(n)?, k.clone()))
                }
                (ChannelTarget::JointScale(n), Keys::Scale(k)) => {
                    out.scales.push((joint(n)?, k.clone()))
                }
                (ChannelTarget::MorphWeight(n), Keys::Weight(k)) => {
                    out.weights.push((morph(n)?, k.clone()))
                }
                _ => {}
            }
        }
        Ok(out)
    }
}

impl BoundClip {
    /// The clip's duration.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Evaluate every channel at `t`, index-addressed.
    pub fn sample(&self, t: Duration) -> BoundSample {
        BoundSample {
            translations: self
                .translations
                .iter()
                .map(|(i, k)| (*i, sample_keys(k, t)))
                .collect(),
            rotations: self
                .rotations
                .iter()
                .map(|(i, k)| (*i, sample_keys(k, t)))
                .collect(),
            scales: self
                .scales
                .iter()
                .map(|(i, k)| (*i, sample_keys(k, t)))
                .collect(),
            weights: self
                .weights
                .iter()
                .map(|(i, k)| (*i, sample_keys(k, t)))
                .collect(),
            settled: t >= self.duration,
        }
    }
}

enum KeyFault {
    NonFinite,
    NonUnit,
}

fn validate_keys<T>(
    channel: usize,
    keys: &[(Duration, T)],
    duration: Duration,
    check: impl Fn(&T) -> Result<(), KeyFault>,
) -> Result<(), ClipError> {
    if keys.is_empty() {
        return Err(ClipError::EmptyChannel { channel });
    }
    if keys.len() > MAX_KEYS_PER_CHANNEL {
        return Err(ClipError::TooManyKeys {
            channel,
            count: keys.len(),
        });
    }
    for (key, (time, value)) in keys.iter().enumerate() {
        if key > 0 && *time <= keys[key - 1].0 {
            return Err(ClipError::KeysNotIncreasing { channel, key });
        }
        if *time > duration {
            return Err(ClipError::KeyBeyondDuration { channel, key });
        }
        match check(value) {
            Ok(()) => {}
            Err(KeyFault::NonFinite) => return Err(ClipError::NonFiniteValue { channel, key }),
            Err(KeyFault::NonUnit) => return Err(ClipError::NonUnitQuaternion { channel, key }),
        }
    }
    Ok(())
}

/// Blend two keys by `u` in `[0, 1)`. `u == 0` returns `a` exactly.
trait Blend: Copy {
    fn blend(a: Self, b: Self, u: f32) -> Self;
}

impl Blend for f32 {
    fn blend(a: f32, b: f32, u: f32) -> f32 {
        (1.0 - u) * a + u * b
    }
}

impl Blend for Vec3 {
    fn blend(a: Vec3, b: Vec3, u: f32) -> Vec3 {
        Vec3 {
            x: f32::blend(a.x, b.x, u),
            y: f32::blend(a.y, b.y, u),
            z: f32::blend(a.z, b.z, u),
        }
    }
}

impl Blend for Quat {
    /// Normalized lerp with the hemisphere sign fix (see the module docs).
    fn blend(a: Quat, b: Quat, u: f32) -> Quat {
        if u == 0.0 {
            return a;
        }
        let dot = a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
        let s = if dot < 0.0 { -1.0 } else { 1.0 };
        let x = (1.0 - u) * a.x + u * s * b.x;
        let y = (1.0 - u) * a.y + u * s * b.y;
        let z = (1.0 - u) * a.z + u * s * b.z;
        let w = (1.0 - u) * a.w + u * s * b.w;
        let len = (x * x + y * y + z * z + w * w).sqrt();
        // After the sign fix dot >= 0, so for unit-ish keys len >= ~0.7 and this is safe; the
        // guard keeps sampling total (never NaN, never a panic) even so.
        if len > 0.0 && len.is_finite() {
            Quat {
                x: x / len,
                y: y / len,
                z: z / len,
                w: w / len,
            }
        } else {
            a
        }
    }
}

/// Sample one non-empty, strictly increasing key list at `t`: hold outside, blend inside.
fn sample_keys<T: Blend>(keys: &[(Duration, T)], t: Duration) -> T {
    // Index of the first key strictly after `t`.
    let next = keys.partition_point(|(kt, _)| *kt <= t);
    if next == 0 {
        return keys[0].1;
    }
    if next == keys.len() {
        return keys[keys.len() - 1].1;
    }
    let (t0, a) = keys[next - 1];
    let (t1, b) = keys[next];
    // t0 <= t < t1, so neither subtraction underflows and the span is non-zero.
    let u = ((t - t0).as_secs_f64() / (t1 - t0).as_secs_f64()) as f32;
    T::blend(a, b, u)
}
