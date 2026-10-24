//! Cross-layer instrument telemetry — the **timing** half of the observatory (issue #143).
//!
//! The retention half (`wire::RetentionEnvelope`, delivered alongside this one) bounds *memory*
//! deterministically.
//! This bounds how the pipeline is *measured in time*, with two disciplines that keep it honest
//! and keep its gates from flaking:
//!
//! * **Tool-relative, never conflated.** Each pipeline stage is measured on its own
//!   ([`Stage`]); the numbers are not folded into a single "health" score, and a measurement is
//!   labelled with what it actually witnesses. In particular, [`Stage::TerminalWrite`] times the
//!   bytes handed to the terminal — which is **not** proof of screen presentation — so there is
//!   no `presented` count and no FPS is ever synthesized from headless write counts.
//! * **Deterministic percentiles.** [`LatencyHistogram`] is a fixed-bucket (bounded-memory,
//!   `O(1)`-record) histogram whose quantiles are pure arithmetic over the recorded samples. It
//!   carries no clock of its own: production code feeds it real measured [`Duration`]s, and a
//!   test feeds it *known* durations, so the percentile math — and any gate built on it — is
//!   reproducible and cannot flake. The gate is phrased as a *budget to exceed* ([`LatencyBudget`]),
//!   so a CI check feeds an injected distribution and asserts the budget is breached or not; it
//!   never asserts a real wall-clock measurement against a brittle universal number (issue #143
//!   is explicit: set limits after observing representative hosts, and avoid brittle universals).

use std::time::Duration;

/// A bounded, deterministic latency histogram.
///
/// Samples are bucketed by their magnitude in nanoseconds — bucket `0` is exactly `0 ns`, and
/// bucket `b` (`1..=64`) covers `[2^(b-1), 2^b)` ns — so recording is `O(1)`, memory is a fixed
/// `BUCKETS` counters regardless of how many samples flow, and quantiles are reproducible. A
/// quantile is reported at its bucket's lower bound, so it is accurate to bucket resolution
/// (roughly a factor of two) and never fabricated beyond what was recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyHistogram {
    buckets: [u64; Self::BUCKETS],
    count: u64,
    sum_nanos: u128,
    min_nanos: u64,
    max_nanos: u64,
}

impl LatencyHistogram {
    /// Bucket `0` holds `0 ns`; buckets `1..=64` hold `[2^(b-1), 2^b)` ns — 65 in all, enough
    /// for any `u64` nanosecond value.
    pub const BUCKETS: usize = 65;

    /// An empty histogram.
    pub const fn new() -> Self {
        Self {
            buckets: [0; Self::BUCKETS],
            count: 0,
            sum_nanos: 0,
            min_nanos: 0,
            max_nanos: 0,
        }
    }

    /// The bucket a nanosecond value falls in: `0` for `0`, else `64 - leading_zeros` (the
    /// 1-based index of its highest set bit).
    fn bucket_for(nanos: u64) -> usize {
        if nanos == 0 {
            0
        } else {
            (64 - nanos.leading_zeros()) as usize
        }
    }

    /// The lower bound (ns) of a bucket — the representative value a quantile in it reports.
    fn bucket_floor(bucket: usize) -> u64 {
        if bucket == 0 {
            0
        } else {
            1u64 << (bucket - 1)
        }
    }

    /// Record one measured latency. A duration beyond `u64::MAX` nanoseconds (≈ 584 years) is
    /// saturated rather than wrapped.
    pub fn record(&mut self, latency: Duration) {
        let nanos = u64::try_from(latency.as_nanos()).unwrap_or(u64::MAX);
        self.record_nanos(nanos);
    }

    /// Record one measured latency given directly in nanoseconds.
    pub fn record_nanos(&mut self, nanos: u64) {
        self.buckets[Self::bucket_for(nanos)] += 1;
        if self.count == 0 {
            self.min_nanos = nanos;
            self.max_nanos = nanos;
        } else {
            self.min_nanos = self.min_nanos.min(nanos);
            self.max_nanos = self.max_nanos.max(nanos);
        }
        self.count += 1;
        self.sum_nanos += u128::from(nanos);
    }

    /// How many samples were recorded.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// The smallest sample, or `None` if empty.
    pub fn min(&self) -> Option<Duration> {
        (self.count > 0).then_some(Duration::from_nanos(self.min_nanos))
    }

    /// The largest sample, or `None` if empty.
    pub fn max(&self) -> Option<Duration> {
        (self.count > 0).then_some(Duration::from_nanos(self.max_nanos))
    }

    /// The arithmetic mean, or `None` if empty. Exact (summed in `u128`), not bucketed.
    pub fn mean(&self) -> Option<Duration> {
        if self.count == 0 {
            return None;
        }
        let mean = (self.sum_nanos / u128::from(self.count)) as u64;
        Some(Duration::from_nanos(mean))
    }

    /// The approximate `q`-quantile (`0.0..=1.0`), reported at the lower bound of the bucket that
    /// holds the sample at rank `ceil(q * count)`. `None` if empty.
    pub fn quantile(&self, q: f64) -> Option<Duration> {
        if self.count == 0 {
            return None;
        }
        let q = q.clamp(0.0, 1.0);
        // 1-based rank, at least 1.
        let target = ((q * self.count as f64).ceil() as u64).max(1);
        let mut cumulative = 0u64;
        for (bucket, &hits) in self.buckets.iter().enumerate() {
            cumulative += hits;
            if cumulative >= target {
                return Some(Duration::from_nanos(Self::bucket_floor(bucket)));
            }
        }
        // Unreachable when count > 0 (the loop sums to `count` >= target), but stay total.
        Some(Duration::from_nanos(self.max_nanos))
    }

    /// The median (50th percentile).
    pub fn p50(&self) -> Option<Duration> {
        self.quantile(0.50)
    }

    /// The 95th percentile.
    pub fn p95(&self) -> Option<Duration> {
        self.quantile(0.95)
    }

    /// The 99th percentile.
    pub fn p99(&self) -> Option<Duration> {
        self.quantile(0.99)
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

/// A distinct stage of the interactive-instrument pipeline, measured on its own. Keeping these
/// separate (rather than summed into one latency) is the honesty discipline of issue #143: each
/// witnesses a different thing, and one cannot stand in for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// From a raw input event received to the application intent it was dispatched as.
    InputDispatch,
    /// The age of the producer snapshot being rendered (how stale the data on screen is).
    SnapshotAge,
    /// From authoritative model state to a composed [`crate::surface::Surface`].
    FrameGeneration,
    /// From a surface to its RGB raster.
    RasterConversion,
    /// Time spent handing frame bytes to the terminal. **Not** proof of screen presentation.
    TerminalWrite,
}

impl Stage {
    /// Every stage, in index order.
    pub const ALL: [Stage; 5] = [
        Stage::InputDispatch,
        Stage::SnapshotAge,
        Stage::FrameGeneration,
        Stage::RasterConversion,
        Stage::TerminalWrite,
    ];

    /// This stage's index into a per-stage array.
    fn index(self) -> usize {
        match self {
            Stage::InputDispatch => 0,
            Stage::SnapshotAge => 1,
            Stage::FrameGeneration => 2,
            Stage::RasterConversion => 3,
            Stage::TerminalWrite => 4,
        }
    }

    /// What a measurement of this stage actually witnesses — the tool-relative caveat that keeps
    /// a number from being over-claimed (e.g. a terminal write is not a screen presentation).
    pub fn tool_relative_note(self) -> &'static str {
        match self {
            Stage::InputDispatch => "input event received to application intent dispatched",
            Stage::SnapshotAge => "age of the rendered producer snapshot; not a render latency",
            Stage::FrameGeneration => "authoritative model state to a composed Surface",
            Stage::RasterConversion => "Surface to RGB raster",
            Stage::TerminalWrite => {
                "bytes handed to the terminal; NOT proof of screen presentation"
            }
        }
    }
}

/// Opt-in, reusable cross-layer telemetry for one interactive-instrument session.
///
/// Per-stage latency is kept in distinct [`LatencyHistogram`]s; the frame tallies record the
/// honest funnel `written <= attempted <= generated`. There is deliberately **no** "frames
/// presented" count — presentation cannot be proven from writes, so no FPS is synthesized from
/// headless counts (issue #143).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstrumentTelemetry {
    stages: [LatencyHistogram; 5],
    frames_generated: u64,
    frames_attempted: u64,
    frames_written: u64,
    frames_coalesced: u64,
    emitted_bytes: u64,
    redraws: u64,
    disconnects: u64,
}

impl InstrumentTelemetry {
    /// Fresh, empty telemetry.
    pub const fn new() -> Self {
        Self {
            stages: [LatencyHistogram::new(); 5],
            frames_generated: 0,
            frames_attempted: 0,
            frames_written: 0,
            frames_coalesced: 0,
            emitted_bytes: 0,
            redraws: 0,
            disconnects: 0,
        }
    }

    /// Record a latency measurement for one stage.
    pub fn record(&mut self, stage: Stage, latency: Duration) {
        self.stages[stage.index()].record(latency);
    }

    /// The latency histogram for a stage.
    pub fn stage(&self, stage: Stage) -> &LatencyHistogram {
        &self.stages[stage.index()]
    }

    /// A frame was generated from model state.
    pub fn note_frame_generated(&mut self) {
        self.frames_generated += 1;
    }
    /// A generated frame was handed toward the terminal.
    pub fn note_frame_attempted(&mut self) {
        self.frames_attempted += 1;
    }
    /// An attempted frame's write completed.
    pub fn note_frame_written(&mut self) {
        self.frames_written += 1;
    }
    /// A generated frame was coalesced away before it was written (superseded by a newer one).
    pub fn note_frame_coalesced(&mut self) {
        self.frames_coalesced += 1;
    }
    /// Count emitted bytes.
    pub fn add_emitted_bytes(&mut self, bytes: u64) {
        self.emitted_bytes += bytes;
    }
    /// A full redraw (as opposed to a diffed update) was issued.
    pub fn note_redraw(&mut self) {
        self.redraws += 1;
    }
    /// A producer disconnect/retirement was observed.
    pub fn note_disconnect(&mut self) {
        self.disconnects += 1;
    }

    /// Frames generated from model state.
    pub fn frames_generated(&self) -> u64 {
        self.frames_generated
    }
    /// Frames handed toward the terminal.
    pub fn frames_attempted(&self) -> u64 {
        self.frames_attempted
    }
    /// Frames whose write completed. This is **not** a count of frames the human saw.
    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }
    /// Frames coalesced away before being written.
    pub fn frames_coalesced(&self) -> u64 {
        self.frames_coalesced
    }
    /// Total emitted bytes.
    pub fn emitted_bytes(&self) -> u64 {
        self.emitted_bytes
    }
    /// Full redraws issued.
    pub fn redraws(&self) -> u64 {
        self.redraws
    }
    /// Producer disconnects observed.
    pub fn disconnects(&self) -> u64 {
        self.disconnects
    }

    /// The honest frame funnel holds: `written <= attempted <= generated`. A presenter that
    /// claimed more written than it attempted (or more attempted than generated) would be
    /// inventing frames; this is the receipt that it did not.
    pub fn frame_funnel_consistent(&self) -> bool {
        self.frames_written <= self.frames_attempted
            && self.frames_attempted <= self.frames_generated
    }
}

impl Default for InstrumentTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

/// A per-stage latency budget, checked at a chosen quantile.
///
/// Phrased as a budget to **exceed** so a regression is *detected* rather than a universal
/// pass/fail number being baked into the library: a CI gate feeds an injected distribution and
/// asserts the budget is breached (or not), which is deterministic, while a real deployment sets
/// `max` from observed hosts (issue #143 — no brittle universal numbers).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatencyBudget {
    /// The quantile to check (`0.0..=1.0`), e.g. `0.95` for p95.
    pub quantile: f64,
    /// The largest tolerated value at that quantile.
    pub max: Duration,
}

impl LatencyBudget {
    /// A p95 budget.
    pub fn p95(max: Duration) -> Self {
        Self {
            quantile: 0.95,
            max,
        }
    }

    /// A p99 budget.
    pub fn p99(max: Duration) -> Self {
        Self {
            quantile: 0.99,
            max,
        }
    }

    /// Whether the histogram's value at [`quantile`](LatencyBudget::quantile) exceeds
    /// [`max`](LatencyBudget::max). An empty histogram never exceeds (nothing was measured).
    pub fn exceeded_by(&self, histogram: &LatencyHistogram) -> bool {
        histogram
            .quantile(self.quantile)
            .is_some_and(|value| value > self.max)
    }
}
