//! STUB (RED phase).
use crate::raster::RgbaRaster;
use crate::stage::{PixelRect, RasterPlane};
use crate::ui::{Key, MotionPreference};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct FrameSequence {}
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SequenceFrame {
    pub source: Arc<RgbaRaster>,
    pub hold: Duration,
}
impl SequenceFrame {
    pub fn new(source: Arc<RgbaRaster>, hold: Duration) -> Self {
        Self { source, hold }
    }
}
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SequenceSample {
    pub index: usize,
    pub source: Arc<RgbaRaster>,
    pub next_in: Option<Duration>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SequenceError {
    Empty,
    TooManyFrames { len: usize },
    HoldTooShort { index: usize },
    RestOutOfRange { rest: usize, len: usize },
    DurationOverflow,
    SizeMismatch { index: usize },
}
impl std::fmt::Display for SequenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SequenceError {}
impl FrameSequence {
    pub fn new(_frames: Vec<SequenceFrame>, _rest: usize) -> Result<Self, SequenceError> {
        todo!()
    }
    pub fn sample(&self, _elapsed: Duration, _motion: MotionPreference) -> SequenceSample {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LayerSlot {
    pub name: String,
    pub rect_px: PixelRect,
}
impl LayerSlot {
    pub fn new(name: impl Into<String>, rect_px: PixelRect) -> Self {
        Self { name: name.into(), rect_px }
    }
}
#[derive(Debug, Clone)]
pub struct LayerSlots {}
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SlotError {
    DuplicateName { name: String },
    EmptyRect { name: String },
    OutsideBase { name: String },
    BaseSizeMismatch { expected: (u32, u32), actual: (u32, u32) },
    UnknownSlot { name: String },
    FillSizeMismatch { name: String },
    DuplicateFill { name: String },
}
impl std::fmt::Display for SlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SlotError {}
impl LayerSlots {
    pub fn new(_base_size: (u32, u32), _slots: Vec<LayerSlot>) -> Result<Self, SlotError> {
        todo!()
    }
    pub fn planes(
        &self,
        _anchor: Key,
        _base: Arc<RgbaRaster>,
        _fills: &[(&str, Arc<RgbaRaster>)],
        _z: i32,
    ) -> Result<Vec<RasterPlane>, SlotError> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct CueSchedule {}
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CueError {
    ZeroPeriod,
    GapExceedsPeriod,
    PeriodTooLong,
}
impl std::fmt::Display for CueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CueError {}
impl CueSchedule {
    pub fn new(
        _seed: u64,
        _cue: u64,
        _origin: Duration,
        _period: Duration,
        _min_gap: Duration,
    ) -> Result<Self, CueError> {
        todo!()
    }
    pub fn nth(&self, _k: u64) -> Option<Duration> {
        todo!()
    }
    pub fn next_after(&self, _t: Duration) -> Option<(u64, Duration)> {
        todo!()
    }
}
