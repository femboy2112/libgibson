//! Pure, clock-free motion primitives for authored terminal motion. EXPERIMENTAL.
//!
//! Everything here is a pure function of caller-supplied time. Nothing owns a thread,
//! a timer or a clock, nothing calls a model, and no policy is decided: the host picks
//! what moves and when, and feeds the returned deadlines to
//! [`SourceDemand`](crate::stage::SourceDemand) as `FrameDemand::After(..)`. The module
//! is purely additive; it adds nothing to any existing Stage type. It is experimental,
//! so its shape may change in a minor release.
//!
//! - [`FrameSequence`] (G1): a finite, authored flipbook of equally sized rasters. It plays
//!   once in order over half-open intervals, then holds its rest frame with no further
//!   deadline. Under `Reduced`/`None` motion it is at rest immediately. It holds no
//!   state, so interruption is just sampling another sequence from a new origin.
//! - [`LayerSlots`] (G2): registered overlay geometry. It validates that a base raster and
//!   named overlay rasters agree with a declared layout, then builds ordinary
//!   [`RasterPlane`]s. It is a validation helper, not a renderer, and does no scaling.
//! - [`CueSchedule`] (G3): a seeded, deadline-driven jittered cue (blink, gaze, nod).
//!   Occurrence `k` is a pure function of `(seed, cue, k)`, so there is no iteration from
//!   the origin and no state. It only proposes times; the host admits or drops them.
//!   **The jitter hash is contract.** Occurrence times are observable behavior that hosts
//!   and goldens depend on, so the hash below must not change in a patch release, and any
//!   change needs a minor release. `mix` is the SplitMix64 step (golden gamma `0x9e3779b97f4a7c15`
//!   added, then the xor-shift/multiply finalizer with `0xbf58476d1ce4e5b9` and
//!   `0x94d049bb133111eb`); `h = mix(mix(mix(seed) ^ cue) ^ k)`; the jitter in nanoseconds is
//!   `h % (span_ns + 1)` with `span_ns = (period - min_gap)` in whole nanoseconds. The
//!   modulo bias is accepted and is part of the contract.
//!
//! G4 (hidden means zero scheduled work) adds no API: it is pinned by `tests/stage_motion.rs`.
use crate::raster::RgbaRaster;
use crate::stage::{PixelRect, RasterAnchor, RasterPlane};
use crate::ui::{Key, MotionPreference};
use std::sync::Arc;
use std::time::Duration;

const MAX_FRAMES: usize = 64;
const MIN_HOLD: Duration = Duration::from_millis(1);

/// One authored frame: its pixels and how long it is shown.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SequenceFrame {
    /// Straight-alpha pixels. All frames of one sequence share one size.
    pub source: Arc<RgbaRaster>,
    /// How long this frame is shown; at least 1 ms.
    pub hold: Duration,
}
impl SequenceFrame {
    /// A frame showing `source` for `hold`.
    pub fn new(source: Arc<RgbaRaster>, hold: Duration) -> Self {
        Self { source, hold }
    }
}

/// The result of sampling a [`FrameSequence`] at an elapsed time.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SequenceSample {
    /// Index of the frame to show.
    pub index: usize,
    /// That frame's pixels.
    pub source: Arc<RgbaRaster>,
    /// Time until the next frame change; `None` once at rest (no further work).
    pub next_in: Option<Duration>,
}

/// Why a [`FrameSequence`] was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SequenceError {
    /// No frames.
    Empty,
    /// More than 64 frames.
    TooManyFrames { len: usize },
    /// A hold under 1 ms.
    HoldTooShort { index: usize },
    /// `rest` is not a valid frame index.
    RestOutOfRange { rest: usize, len: usize },
    /// The holds sum past `Duration::MAX`.
    DurationOverflow,
    /// A frame's dimensions differ from frame 0.
    SizeMismatch { index: usize },
}
impl std::fmt::Display for SequenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "frame sequence has no frames"),
            Self::TooManyFrames { len } => {
                write!(f, "frame sequence has {len} frames (max {MAX_FRAMES})")
            }
            Self::HoldTooShort { index } => write!(f, "frame {index} holds under 1 ms"),
            Self::RestOutOfRange { rest, len } => {
                write!(f, "rest frame {rest} is out of range for {len} frames")
            }
            Self::DurationOverflow => write!(f, "total frame duration overflows"),
            Self::SizeMismatch { index } => {
                write!(f, "frame {index} differs in size from frame 0")
            }
        }
    }
}
impl std::error::Error for SequenceError {}

/// A finite authored flipbook with a rest frame. Plays once, then holds `frames[rest]`.
/// Frame `i` covers the half-open interval `[start_i, start_i + hold_i)`. There is no
/// implicit loop and no state: a repeat is a new host cue with a new origin.
#[derive(Debug, Clone)]
pub struct FrameSequence {
    frames: Vec<SequenceFrame>,
    rest: usize,
    total: Duration,
}
impl FrameSequence {
    /// Validates and builds a sequence. Refuses an empty sequence, more than 64 frames, a
    /// hold under 1 ms, an out-of-range `rest`, differing frame sizes, and a total
    /// duration that overflows `Duration`.
    pub fn new(frames: Vec<SequenceFrame>, rest: usize) -> Result<Self, SequenceError> {
        if frames.is_empty() {
            return Err(SequenceError::Empty);
        }
        if frames.len() > MAX_FRAMES {
            return Err(SequenceError::TooManyFrames { len: frames.len() });
        }
        if rest >= frames.len() {
            return Err(SequenceError::RestOutOfRange {
                rest,
                len: frames.len(),
            });
        }
        let size = (frames[0].source.width(), frames[0].source.height());
        let mut total = Duration::ZERO;
        for (index, f) in frames.iter().enumerate() {
            if f.hold < MIN_HOLD {
                return Err(SequenceError::HoldTooShort { index });
            }
            if (f.source.width(), f.source.height()) != size {
                return Err(SequenceError::SizeMismatch { index });
            }
            total = total
                .checked_add(f.hold)
                .ok_or(SequenceError::DurationOverflow)?;
        }
        Ok(Self {
            frames,
            rest,
            total,
        })
    }

    /// Samples at `elapsed` since the cue's origin. Late sampling jumps to the current
    /// frame and never replays. `Reduced` and `None` return the rest frame with no deadline.
    pub fn sample(&self, elapsed: Duration, motion: MotionPreference) -> SequenceSample {
        let at_rest = || SequenceSample {
            index: self.rest,
            source: Arc::clone(&self.frames[self.rest].source),
            next_in: None,
        };
        if motion != MotionPreference::Full || elapsed >= self.total {
            return at_rest();
        }
        let mut end = Duration::ZERO;
        for (index, f) in self.frames.iter().enumerate() {
            // Cannot overflow: the running sum is bounded by `total`, checked in `new`.
            end += f.hold;
            if elapsed < end {
                return SequenceSample {
                    index,
                    source: Arc::clone(&f.source),
                    next_in: Some(end - elapsed),
                };
            }
        }
        at_rest()
    }
}

/// A named overlay region of a base raster, in raster pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LayerSlot {
    /// Unique slot name.
    pub name: String,
    /// The region, relative to the base's top-left. Must lie inside the base and be non-empty.
    pub rect_px: PixelRect,
}
impl LayerSlot {
    /// A slot called `name` covering `rect_px`.
    pub fn new(name: impl Into<String>, rect_px: PixelRect) -> Self {
        Self {
            name: name.into(),
            rect_px,
        }
    }
}

/// Why [`LayerSlots`] refused a layout or a fill.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SlotError {
    /// Two slots share a name.
    DuplicateName { name: String },
    /// A slot has zero width or height.
    EmptyRect { name: String },
    /// A slot is not entirely inside the base.
    OutsideBase { name: String },
    /// The base raster is not the declared base size.
    BaseSizeMismatch {
        expected: (u32, u32),
        actual: (u32, u32),
    },
    /// A fill names a slot that does not exist.
    UnknownSlot { name: String },
    /// A fill is not exactly its slot's size.
    FillSizeMismatch { name: String },
    /// The same slot is filled twice.
    DuplicateFill { name: String },
}
impl std::fmt::Display for SlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateName { name } => write!(f, "duplicate slot name {name:?}"),
            Self::EmptyRect { name } => write!(f, "slot {name:?} has an empty rect"),
            Self::OutsideBase { name } => write!(f, "slot {name:?} lies outside the base"),
            Self::BaseSizeMismatch { expected, actual } => {
                write!(f, "base is {actual:?} px, expected {expected:?}")
            }
            Self::UnknownSlot { name } => write!(f, "unknown slot {name:?}"),
            Self::FillSizeMismatch { name } => {
                write!(f, "fill for slot {name:?} is not exactly the slot size")
            }
            Self::DuplicateFill { name } => write!(f, "slot {name:?} is filled twice"),
        }
    }
}
impl std::error::Error for SlotError {}

/// Registered overlay geometry: a base size plus named slots. A validation helper that
/// builds ordinary [`RasterPlane`]s; compositing, clipping and limits stay with Stage.
#[derive(Debug, Clone)]
pub struct LayerSlots {
    base_size: (u32, u32),
    slots: Vec<LayerSlot>,
}
impl LayerSlots {
    /// Refuses slots outside the base, duplicate names and empty rects.
    pub fn new(base_size: (u32, u32), slots: Vec<LayerSlot>) -> Result<Self, SlotError> {
        for (i, s) in slots.iter().enumerate() {
            let r = s.rect_px;
            if slots[..i].iter().any(|o| o.name == s.name) {
                return Err(SlotError::DuplicateName {
                    name: s.name.clone(),
                });
            }
            if r.width == 0 || r.height == 0 {
                return Err(SlotError::EmptyRect {
                    name: s.name.clone(),
                });
            }
            let (x, y) = (i64::from(r.x), i64::from(r.y));
            let inside = x >= 0
                && y >= 0
                && x + i64::from(r.width) <= i64::from(base_size.0)
                && y + i64::from(r.height) <= i64::from(base_size.1);
            if !inside {
                return Err(SlotError::OutsideBase {
                    name: s.name.clone(),
                });
            }
        }
        Ok(Self { base_size, slots })
    }

    /// The base plane at `z`, then each filled slot at `z + 1 + i` (`i` = declaration
    /// index), all anchored to `RasterAnchor::Key(anchor)`. Refuses a wrong-size base, an
    /// unknown or twice-filled slot, and a fill not exactly the slot's size. An unfilled
    /// slot shows the base.
    pub fn planes(
        &self,
        anchor: Key,
        base: Arc<RgbaRaster>,
        fills: &[(&str, Arc<RgbaRaster>)],
        z: i32,
    ) -> Result<Vec<RasterPlane>, SlotError> {
        let actual = (u32::from(base.width()), u32::from(base.height()));
        if actual != self.base_size {
            return Err(SlotError::BaseSizeMismatch {
                expected: self.base_size,
                actual,
            });
        }
        let mut chosen: Vec<Option<&Arc<RgbaRaster>>> = vec![None; self.slots.len()];
        for (name, raster) in fills {
            let Some(i) = self.slots.iter().position(|s| s.name == *name) else {
                return Err(SlotError::UnknownSlot {
                    name: (*name).to_owned(),
                });
            };
            if chosen[i].is_some() {
                return Err(SlotError::DuplicateFill {
                    name: (*name).to_owned(),
                });
            }
            let r = self.slots[i].rect_px;
            if (u32::from(raster.width()), u32::from(raster.height())) != (r.width, r.height) {
                return Err(SlotError::FillSizeMismatch {
                    name: (*name).to_owned(),
                });
            }
            chosen[i] = Some(raster);
        }
        let key = RasterAnchor::Key(anchor);
        let mut out = vec![RasterPlane {
            anchor: key.clone(),
            source: base,
            offset_px: (0, 0),
            z,
        }];
        for (i, (slot, fill)) in self.slots.iter().zip(chosen).enumerate() {
            if let Some(fill) = fill {
                out.push(RasterPlane {
                    anchor: key.clone(),
                    source: Arc::clone(fill),
                    offset_px: (slot.rect_px.x, slot.rect_px.y),
                    // Slot count is caller-bounded; saturate rather than wrap.
                    z: z.saturating_add(1)
                        .saturating_add(i32::try_from(i).unwrap_or(i32::MAX)),
                });
            }
        }
        Ok(out)
    }
}

/// Why a [`CueSchedule`] was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CueError {
    /// `period` is zero.
    ZeroPeriod,
    /// `min_gap` exceeds `period`.
    GapExceedsPeriod,
    /// `period` exceeds `u64::MAX` nanoseconds (about 584 years).
    PeriodTooLong,
}
impl std::fmt::Display for CueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroPeriod => write!(f, "cue period is zero"),
            Self::GapExceedsPeriod => write!(f, "cue min_gap exceeds its period"),
            Self::PeriodTooLong => write!(f, "cue period exceeds u64::MAX nanoseconds"),
        }
    }
}
impl std::error::Error for CueError {}

/// The SplitMix64 step. Part of the G3 contract; do not change in a patch release.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z ^= z >> 30;
    z = z.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A seeded, deadline-driven jittered cue. Occurrence `k` lies in the slot
/// `[origin + k*period, origin + k*period + (period - min_gap)]`, at a jitter fixed by a
/// pure hash of `(seed, cue, k)`. Consecutive occurrences are at least `min_gap` and at
/// most `2*period - min_gap` apart. It only proposes times; the host admits or drops them.
#[derive(Debug, Clone)]
pub struct CueSchedule {
    seed: u64,
    cue: u64,
    origin: Duration,
    period_ns: u64,
    span_ns: u64,
}
impl CueSchedule {
    /// Refuses a zero `period`, `min_gap > period`, and a `period` above `u64::MAX` ns.
    pub fn new(
        seed: u64,
        cue: u64,
        origin: Duration,
        period: Duration,
        min_gap: Duration,
    ) -> Result<Self, CueError> {
        if period.is_zero() {
            return Err(CueError::ZeroPeriod);
        }
        if min_gap > period {
            return Err(CueError::GapExceedsPeriod);
        }
        let period_ns = u64::try_from(period.as_nanos()).map_err(|_| CueError::PeriodTooLong)?;
        // min_gap <= period, so this fits in u64 too.
        let span_ns =
            u64::try_from((period - min_gap).as_nanos()).map_err(|_| CueError::PeriodTooLong)?;
        Ok(Self {
            seed,
            cue,
            origin,
            period_ns,
            span_ns,
        })
    }

    /// Occurrence `k`: its slot start plus the hashed jitter. `None` only if the time
    /// overflows `Duration`; it never wraps or saturates.
    pub fn nth(&self, k: u64) -> Option<Duration> {
        let h = mix(mix(mix(self.seed) ^ self.cue) ^ k);
        // `span_ns + 1` wraps to 0 only when span_ns == u64::MAX; the modulus is then 2^64,
        // i.e. the whole of `h`.
        let jitter = u128::from(h.checked_rem(self.span_ns.wrapping_add(1)).unwrap_or(h));
        self.slot_start_ns(k)?
            .checked_add(jitter)
            .and_then(ns_to_duration)
    }

    fn slot_start_ns(&self, k: u64) -> Option<u128> {
        u128::from(k)
            .checked_mul(u128::from(self.period_ns))?
            .checked_add(self.origin.as_nanos())
    }

    /// The first occurrence strictly after `t`, as `(k, time)`. O(1): jitter stays within
    /// its own slot, so only `k_c` or `k_c + 1` can qualify, where `k_c` is the slot
    /// containing `t`. For `t < origin` this is `(0, nth(0))`. `None` only on overflow.
    pub fn next_after(&self, t: Duration) -> Option<(u64, Duration)> {
        let kc = match t.checked_sub(self.origin) {
            None => 0,
            Some(rel) => u64::try_from(rel.as_nanos() / u128::from(self.period_ns)).ok()?,
        };
        let at = self.nth(kc)?;
        if at > t {
            return Some((kc, at));
        }
        // Slot kc+1 starts strictly after t, so its occurrence is strictly after t.
        let k1 = kc.checked_add(1)?;
        Some((k1, self.nth(k1)?))
    }
}

fn ns_to_duration(ns: u128) -> Option<Duration> {
    let secs = u64::try_from(ns / 1_000_000_000).ok()?;
    Some(Duration::new(secs, (ns % 1_000_000_000) as u32))
}
