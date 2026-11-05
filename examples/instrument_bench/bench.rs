#![allow(dead_code)]
//! # `instrument_bench` — measurement wired INTO the instrument loop (v0.6 R6.5, #143)
//!
//! `InstrumentTelemetry` has shipped as a library primitive since v0.5, but until
//! this slice it lived only in its own unit test: no example ever drove a *real*
//! application loop through it. A primitive that is only unit-tested measures
//! nothing — issue #143 asks for measurement **built into the application loop**,
//! reported truthfully, with controls that prove the observatory *detects* a
//! defect rather than papering over it. This is that loop.
//!
//! ## What is honest here, and why
//!
//! The loop drives the **real renderer** (`Renderer -> TerminalTransaction ->
//! bytes`) through `TerminalSession::headless` — the library's own blessed
//! controlled-geometry abstraction (one renderer, Law L1; no second pipeline).
//! Every number is scoped to exactly what it witnessed (Law L10):
//!
//! * **Stages are measured, never invented.** Only stages this loop genuinely
//!   times are recorded. `InputDispatch`, `FrameGeneration` and `TerminalWrite`
//!   are exercised every tick. `SnapshotAge` is recorded **only** under a workload
//!   that introduces a real pipeline lag; `RasterConversion` is **not** exercised
//!   by a headless cell renderer, so it is reported as *not exercised*, with the
//!   reason — never as a fabricated zero that would read as "instant".
//! * **`TerminalWrite` is bytes handed to a sink, not a PTY, and never a screen
//!   presentation** — it is in-memory serialization cost, not a device write.
//!   There is deliberately **no frames-per-second number**: presentation cannot be
//!   proven from a headless write, so none is synthesised (#143 — no universal FPS
//!   lie). The report says so in words.
//! * **No latency VALUE is asserted.** Wall-clock latency is reported only as a
//!   coarse, disclosed indicator; the mutation battery asserts only quantities that
//!   are deterministic functions of the model and the renderer's policy — the frame
//!   funnel, emitted bytes, exact changed cells, provenance accounting — and the
//!   detector checks use sanity ceilings set far above any realistic time, never a
//!   measured nanosecond count. That is what keeps the battery reproducible across
//!   machines (the discipline that makes it immune to the libm drift the audio PCM
//!   golden has to pin a toolchain for).
//!
//! ## Scope — the R6.5 timing half of #143
//!
//! This slice measures the render loop's frame / latency / byte / provenance
//! funnel. It deliberately does **not** cover the command funnel (issued /
//! delivered / acknowledged) — that is the wire bridge's domain (#142) — nor
//! entity-retention / queue high-water / memory footprint (#77 and later). Those
//! are ceded, not silently omitted.
//!
//! ## The loop, one tick
//!
//! ```text
//!   synthetic input ─InputDispatch→ intent ─apply→ model.tick++
//!        model ─FrameGeneration(compose)→ Surface ─Node::raster→ Renderer::render
//!                                                        │ (into a byte sink)
//!                              FrameReport{ generation, write, bytes, changed } ◄┘
//!        funnel: generated → (coalesced | attempted → written)
//! ```

use std::time::{Duration, Instant};

use gibson::instrument_telemetry::{InstrumentTelemetry, LatencyBudget, LatencyHistogram, Stage};
use gibson::node::Node;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;
use gibson::{Cell, ColorDepth, Glyph, RenderMode, Style, Surface};

/// A console-safe intensity ramp: byte-stable, no sub-cell glyph dependency, so a
/// changed-cell count never hinges on terminal glyph realisation.
const RAMP: &[u8] = b" .:-=+*#%@";

/// The declared logical period of one model tick. This is a *modelled* cadence
/// used to convert a logical snapshot lag into a `SnapshotAge` duration — it is
/// **not** a measured frame time and makes no claim about real display cadence.
pub const TICK_PERIOD: Duration = Duration::from_millis(16);

/// An opaque producer/source identity. A change in `(id, epoch)` is a provenance
/// discontinuity the loop must *observe*, not silently absorb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceId(pub u32);

/// The controlled workloads. Each is a deterministic function of the tick, so a
/// replay reproduces the same funnel, bytes and changed-cell totals exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workload {
    /// A small blob moves one cell per tick over a quiet field — few changed cells.
    Sparse,
    /// A horizontal band animates — roughly a third of the field changes per tick.
    Moderate,
    /// Every cell's glyph cycles every tick — a full-repaint stress, maximal churn.
    FullChurn,
    /// Moderate churn, but the producer advances several ticks per rendered frame:
    /// the extra snapshots are coalesced away (bounded, Law L7) and the rendered
    /// frame carries a real, constant staleness recorded as `SnapshotAge`.
    Backpressure,
}

impl Workload {
    /// How many model ticks the producer advances per rendered frame.
    pub fn produce_per_render(self) -> u64 {
        match self {
            Workload::Backpressure => 4,
            _ => 1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Workload::Sparse => "sparse",
            Workload::Moderate => "moderate",
            Workload::FullChurn => "full-churn",
            Workload::Backpressure => "backpressure",
        }
    }

    pub fn parse(s: &str) -> Option<Workload> {
        Some(match s {
            "sparse" => Workload::Sparse,
            "moderate" => Workload::Moderate,
            "full-churn" | "full" => Workload::FullChurn,
            "backpressure" | "back" => Workload::Backpressure,
            _ => return None,
        })
    }

    pub const ALL: [Workload; 4] = [
        Workload::Sparse,
        Workload::Moderate,
        Workload::FullChurn,
        Workload::Backpressure,
    ];
}

/// The application's authoritative model. Deterministic in `(tick, source, epoch)`.
#[derive(Debug, Clone)]
pub struct Model {
    pub cols: u16,
    pub rows: u16,
    pub source: SourceId,
    pub epoch: u32,
    pub tick: u64,
    pub workload: Workload,
}

impl Model {
    pub fn new(cols: u16, rows: u16, workload: Workload) -> Self {
        Self {
            cols: cols.max(1),
            rows: rows.max(1),
            source: SourceId(1),
            epoch: 0,
            tick: 0,
            workload,
        }
    }

    /// The intensity (ramp index) of one cell at a given logical tick. Pure.
    fn intensity(&self, x: u16, y: u16, tick: u64) -> usize {
        let (x, y) = (x as u64, y as u64);
        match self.workload {
            Workload::FullChurn => ((x + y + tick) % RAMP.len() as u64) as usize,
            Workload::Sparse => {
                // A 3x3 blob sweeping right over an empty (space) field.
                let span = (self.cols.saturating_sub(4)).max(1) as u64;
                let hx = 2 + tick % span;
                let hy = (self.rows / 2) as u64;
                let near = x.abs_diff(hx) <= 1 && y.abs_diff(hy) <= 1;
                if near {
                    RAMP.len() - 1
                } else {
                    0
                }
            }
            Workload::Moderate | Workload::Backpressure => {
                // A band across the middle third animates; the rest is quiet.
                let lo = (self.rows / 3) as u64;
                let hi = (2 * self.rows as u64 / 3).max(lo + 1);
                if (lo..hi).contains(&y) {
                    ((x + tick) % RAMP.len() as u64) as usize
                } else {
                    0
                }
            }
        }
    }

    /// Compose the model's state at `tick` into a Surface — the `FrameGeneration`
    /// stage of #143: "authoritative model state to a composed Surface".
    pub fn compose(&self, tick: u64) -> Surface {
        let mut surface = Surface::new(self.cols, self.rows);
        for y in 0..self.rows {
            for x in 0..self.cols {
                let level = self.intensity(x, y, tick).min(RAMP.len() - 1);
                let glyph = Glyph::from_char(RAMP[level] as char);
                surface.set_cell(x, y, Cell::new(glyph, Style::new()));
            }
        }
        surface
    }

    /// Restart the producer: a new epoch (and a bumped source id). The loop must
    /// observe this as a provenance discontinuity.
    pub fn restart_source(&mut self) {
        self.source = SourceId(self.source.0 + 1);
        self.epoch += 1;
    }
}

/// A synthetic raw input event, decoded each tick into an application intent. The
/// decode is the `InputDispatch` stage — real (if tiny) work, timed honestly.
#[derive(Debug, Clone, Copy)]
pub enum RawInput {
    Advance,
    Nudge(i8),
    Idle,
}

/// What the application decided a raw input *means*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Advance,
    Pan(i8),
    None,
}

/// Decode a raw input into an intent. Deliberately a real `match` (not a no-op),
/// so `InputDispatch` witnesses genuine dispatch work.
pub fn dispatch(raw: RawInput) -> Intent {
    match raw {
        RawInput::Advance => Intent::Advance,
        RawInput::Nudge(d) if d != 0 => Intent::Pan(d.signum()),
        RawInput::Nudge(_) => Intent::None,
        RawInput::Idle => Intent::None,
    }
}

/// An optional, **disclosed** latency injection into one stage — the knob the
/// mutation battery uses to prove a `LatencyBudget` detects a regression. The
/// extra time is *added to the recorded histogram* (never slept), so detection is
/// deterministic and fast; the report always states that the stall was injected,
/// so an injected number is never mistaken for a measurement.
#[derive(Debug, Clone, Copy)]
pub struct Stall {
    pub stage: Stage,
    pub extra: Duration,
}

/// The aggregated receipt of one bench session.
#[derive(Debug, Clone)]
pub struct SessionReport {
    pub workload: Workload,
    pub ticks: u64,
    pub cols: u16,
    pub rows: u16,
    pub color: ColorDepth,
    pub stall: Option<Stall>,
    /// The cross-layer telemetry: per-stage latency histograms + the honest funnel.
    pub telemetry: InstrumentTelemetry,
    /// The renderer's *own* generation-duration clock (layout+paint+diff+ANSI),
    /// kept distinct from the app's `FrameGeneration(compose)` stage (Law L6 —
    /// independent clocks are different quantities, not one restated).
    pub renderer_generation: LatencyHistogram,
    /// Sum of exact semantic changed cells across all rendered frames — a
    /// deterministic function of the model, not a timer.
    pub total_changed_cells: u64,
    /// Whether `SnapshotAge` was exercised at all (only the backpressure pipeline
    /// lag populates it).
    pub snapshot_age_exercised: bool,
}

impl SessionReport {
    /// The honest frame funnel `written <= attempted <= generated` holds.
    pub fn funnel_honest(&self) -> bool {
        self.telemetry.frame_funnel_consistent()
    }

    /// Check a set of per-stage budgets, returning `(stage, exceeded)` for each.
    /// This is the *loop-level wiring* under test: the right stage's histogram is
    /// fed to the right budget and the breach is surfaced.
    pub fn check_budgets(&self, budgets: &[(Stage, LatencyBudget)]) -> Vec<(Stage, bool)> {
        budgets
            .iter()
            .map(|(stage, budget)| (*stage, budget.exceeded_by(self.telemetry.stage(*stage))))
            .collect()
    }

    /// Stages this session actually produced samples for, in canonical order.
    pub fn exercised_stages(&self) -> Vec<Stage> {
        Stage::ALL
            .into_iter()
            .filter(|s| self.telemetry.stage(*s).count() > 0)
            .collect()
    }
}

/// Run one controlled workload to completion and return its receipt.
///
/// `ticks` is the number of *rendered* frames. Under backpressure the producer
/// advances faster, so more model ticks elapse than frames are rendered; the
/// surplus is coalesced (counted), never queued unbounded (Law L7).
pub fn run_workload(
    workload: Workload,
    ticks: u64,
    cols: u16,
    rows: u16,
    color: ColorDepth,
    stall: Option<Stall>,
) -> SessionReport {
    run_workload_with_restart(workload, ticks, cols, rows, color, stall, None)
}

/// As [`run_workload`], but optionally restart the producer at a given rendered
/// frame index — used to exercise provenance-discontinuity detection.
pub fn run_workload_with_restart(
    workload: Workload,
    ticks: u64,
    cols: u16,
    rows: u16,
    color: ColorDepth,
    stall: Option<Stall>,
    restart_at: Option<u64>,
) -> SessionReport {
    let mut model = Model::new(cols, rows, workload);
    let mut telemetry = InstrumentTelemetry::new();
    let mut renderer_generation = LatencyHistogram::new();

    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(model.cols, model.rows);
    session.set_color_depth(color);
    let mut sink: Vec<u8> = Vec::new();

    let produce = workload.produce_per_render();
    let mut total_changed_cells: u64 = 0;
    let mut snapshot_age_exercised = false;

    // The frame currently "on screen" (for the pipeline-lag staleness model).
    let mut on_screen_tick: u64 = 0;
    let mut last_source = model.source;
    let mut last_epoch = model.epoch;

    // A tiny deterministic input script, cycled — real dispatch work every tick.
    let script = [RawInput::Advance, RawInput::Nudge(1), RawInput::Idle];

    for frame in 0..ticks {
        if restart_at == Some(frame) {
            model.restart_source();
        }

        // 1. InputDispatch — decode one raw event into an application intent.
        let raw = script[(frame % script.len() as u64) as usize];
        let t0 = Instant::now();
        let intent = dispatch(raw);
        record(&mut telemetry, &stall, Stage::InputDispatch, t0.elapsed());
        let _ = intent; // the intent drives the model; its effect is the tick advance below

        // 2. Provenance: observe any source/epoch discontinuity, don't swallow it.
        if model.source != last_source || model.epoch != last_epoch {
            telemetry.note_disconnect();
            last_source = model.source;
            last_epoch = model.epoch;
            // A fresh source forces a full repaint on the next frame.
            renderer = Renderer::new(RenderMode::Fullscreen);
        }

        // 3. The producer advances `produce` ticks; intermediate snapshots are
        //    coalesced away (bounded). We render only the newest.
        let newest = model.tick + produce;
        for _ in 0..produce {
            telemetry.note_frame_generated();
        }
        for _ in 1..produce {
            telemetry.note_frame_coalesced();
        }
        model.tick = newest;

        // SnapshotAge: how stale the frame that WAS on screen is, relative to the
        // newest snapshot now being painted. Zero under 1:1 cadence (no staleness);
        // a real, constant lag under backpressure.
        if produce > 1 {
            let age = TICK_PERIOD * ((newest - on_screen_tick) as u32);
            telemetry.record(Stage::SnapshotAge, age);
            snapshot_age_exercised = true;
        }
        on_screen_tick = newest;

        // 4. FrameGeneration — compose authoritative model state into a Surface.
        let t1 = Instant::now();
        let surface = model.compose(newest);
        record(&mut telemetry, &stall, Stage::FrameGeneration, t1.elapsed());

        // 5. Render the newest surface through the real pipeline into a byte sink.
        telemetry.note_frame_attempted();
        sink.clear();
        let mut node = Node::raster(surface);
        renderer
            .render(&mut node, &mut session, &mut sink)
            .expect("headless render never fails");
        let report = renderer.last_frame_report();
        telemetry.note_frame_written();

        // The renderer's own two clocks, kept distinct.
        renderer_generation.record(report.generation_duration);
        record(
            &mut telemetry,
            &stall,
            Stage::TerminalWrite,
            report.write_duration,
        );

        telemetry.add_emitted_bytes(report.bytes_emitted as u64);
        total_changed_cells += report.exact_changed_cells as u64;
        if report.full_repaint {
            telemetry.note_redraw();
        }
    }

    SessionReport {
        workload,
        ticks,
        cols,
        rows,
        color,
        stall,
        telemetry,
        renderer_generation,
        total_changed_cells,
        snapshot_age_exercised,
    }
}

/// Record a stage latency, adding a disclosed injected stall if one targets it.
fn record(
    telemetry: &mut InstrumentTelemetry,
    stall: &Option<Stall>,
    stage: Stage,
    measured: Duration,
) {
    let extra = match stall {
        Some(s) if s.stage == stage => s.extra,
        _ => Duration::ZERO,
    };
    telemetry.record(stage, measured + extra);
}

/// Stage → stable kebab label (shared by the CLI and the report).
pub fn stage_label(stage: Stage) -> &'static str {
    match stage {
        Stage::InputDispatch => "input-dispatch",
        Stage::SnapshotAge => "snapshot-age",
        Stage::FrameGeneration => "frame-generation",
        Stage::RasterConversion => "raster-conversion",
        Stage::TerminalWrite => "terminal-write",
        Stage::CommandDispatch => "command-dispatch",
    }
}

fn color_label(c: ColorDepth) -> &'static str {
    match c {
        ColorDepth::Mono => "mono",
        ColorDepth::Ansi16 => "ansi16",
        ColorDepth::Ansi256 => "ansi256",
        ColorDepth::TrueColor => "truecolor",
    }
}

fn fmt_dur(d: Option<Duration>) -> String {
    match d {
        Some(d) => format!("{:>9.3}µs", d.as_secs_f64() * 1e6),
        None => format!("{:>11}", "—"),
    }
}

/// Render one session's receipt. Deliberately carries **no** frames-per-second
/// number and labels every stage with what it actually witnessed. A stage with
/// no samples is reported as *not exercised* (with the reason) rather than as a
/// fabricated zero.
pub fn format_report(r: &SessionReport) -> String {
    let mut out = String::new();
    let t = &r.telemetry;

    out.push_str(&format!(
        "── workload: {:<12} ticks: {:<6} size: {}x{}  color: {} ──\n",
        r.workload.label(),
        r.ticks,
        r.cols,
        r.rows,
        color_label(r.color),
    ));
    out.push_str(
        "method: headless simulation (TerminalSession::headless -> real Renderer -> byte sink).\n\
         NOT a PTY and NOT a display: terminal-write is bytes handed to a sink, never screen presentation.\n\
         No frames-per-second number is synthesised — presentation cannot be proven from a headless write (#143).\n",
    );
    if let Some(s) = r.stall {
        out.push_str(&format!(
            "INJECTED STALL (disclosed, not measured): +{:.3}us into {} every tick.\n",
            s.extra.as_secs_f64() * 1e6,
            stage_label(s.stage),
        ));
    }

    out.push_str(
        "\nper-stage timing (wall-clock where measured; snapshot-age is a modelled age, not a clock; \
         quantiles are log2-nanosecond bucket floors — rounded DOWN, ~2x coarse; indicative, not a certified budget):\n",
    );
    out.push_str(&format!(
        "  {:<18} {:>5}  {:>11} {:>11} {:>11}   witnesses\n",
        "stage", "n", "p50", "p95", "p99",
    ));
    for stage in Stage::ALL {
        let h = t.stage(stage);
        if h.count() == 0 {
            let reason = match stage {
                Stage::RasterConversion => {
                    "not exercised: headless cell render has no RGB raster stage"
                }
                Stage::SnapshotAge => "not exercised: 1:1 cadence, no staleness",
                _ => "not exercised",
            };
            out.push_str(&format!(
                "  {:<18} {:>5}  {}\n",
                stage_label(stage),
                0,
                reason
            ));
        } else {
            // SnapshotAge is a modelled age (TICK_PERIOD x lag), not a measured
            // clock, so it does not sit under the "wall-clock" sense of the header.
            let note = if matches!(stage, Stage::SnapshotAge) {
                "modelled age (TICK_PERIOD x lag), not wall-clock; bucket floor under-reports"
            } else {
                stage.tool_relative_note()
            };
            out.push_str(&format!(
                "  {:<18} {:>5}  {} {} {}   {}\n",
                stage_label(stage),
                h.count(),
                fmt_dur(h.p50()),
                fmt_dur(h.p95()),
                fmt_dur(h.p99()),
                note,
            ));
        }
    }

    out.push_str(
        "\nrenderer's own generation clock (layout+paint+diff+ANSI; distinct from compose; \
         same log2 bucket floors, ~2x coarse):\n",
    );
    out.push_str(&format!(
        "  {:>5}  {:>11} {:>11} {:>11}\n",
        "n", "p50", "p95", "p99",
    ));
    out.push_str(&format!(
        "  {:>5}  {} {} {}\n",
        r.renderer_generation.count(),
        fmt_dur(r.renderer_generation.p50()),
        fmt_dur(r.renderer_generation.p95()),
        fmt_dur(r.renderer_generation.p99()),
    ));

    out.push_str(
        "\nframe funnel (counts are deterministic — the honest accounting, not a timer):\n",
    );
    out.push_str(&format!(
        "  generated {}  ->  coalesced {}  ->  attempted {}  ->  written {}\n",
        t.frames_generated(),
        t.frames_coalesced(),
        t.frames_attempted(),
        t.frames_written(),
    ));
    out.push_str(&format!(
        "  full-repaints {}   provenance-disconnects {}   emitted-bytes {}   exact-changed-cells {}\n",
        t.redraws(),
        t.disconnects(),
        t.emitted_bytes(),
        r.total_changed_cells,
    ));
    out.push_str(&format!(
        "  funnel honest (written <= attempted <= generated): {}\n",
        r.funnel_honest(),
    ));

    out
}

/// Drive every workload and assert the honesty invariants. Returns the list of
/// violations (empty ⇒ all held). Shared by the CLI's `--selfcheck` and the
/// mutation battery, so the live guard and the CI gate can never drift apart.
pub fn selfcheck() -> Vec<String> {
    let mut bad = Vec::new();
    let (cols, rows, ticks) = (48u16, 24u16, 16u64);
    let color = ColorDepth::TrueColor;

    // 1. The funnel is honest for every workload.
    for w in Workload::ALL {
        let r = run_workload(w, ticks, cols, rows, color, None);
        if !r.funnel_honest() {
            bad.push(format!("funnel dishonest for {}", w.label()));
        }
    }

    let sparse = run_workload(Workload::Sparse, ticks, cols, rows, color, None);
    let full = run_workload(Workload::FullChurn, ticks, cols, rows, color, None);
    let back = run_workload(Workload::Backpressure, ticks, cols, rows, color, None);

    // 2. Churn drives REAL work: full-churn moves strictly more cells and bytes
    //    than sparse for the same ticks and size (numbers track the model).
    if full.total_changed_cells <= sparse.total_changed_cells {
        bad.push(format!(
            "full-churn changed cells ({}) did not exceed sparse ({})",
            full.total_changed_cells, sparse.total_changed_cells
        ));
    }
    if full.telemetry.emitted_bytes() <= sparse.telemetry.emitted_bytes() {
        bad.push(format!(
            "full-churn emitted bytes ({}) did not exceed sparse ({})",
            full.telemetry.emitted_bytes(),
            sparse.telemetry.emitted_bytes()
        ));
    }

    // 3. Backpressure coalesces (bounded) and carries a real staleness; 1:1 neither.
    if back.telemetry.frames_coalesced() == 0 || !back.snapshot_age_exercised {
        bad.push("backpressure did not coalesce or recorded no snapshot age".into());
    }
    if sparse.telemetry.frames_coalesced() != 0 || sparse.snapshot_age_exercised {
        bad.push("1:1 cadence wrongly reported coalescing or staleness".into());
    }

    // 4. Determinism: the deterministic quantities replay exactly (NOT latency).
    let a = run_workload(Workload::Sparse, ticks, cols, rows, color, None);
    let b = run_workload(Workload::Sparse, ticks, cols, rows, color, None);
    if a.telemetry.frames_written() != b.telemetry.frames_written()
        || a.telemetry.emitted_bytes() != b.telemetry.emitted_bytes()
        || a.total_changed_cells != b.total_changed_cells
    {
        bad.push("replay was not deterministic in counts/bytes/changed-cells".into());
    }

    // 5. A budget DETECTS an injected stall on the RIGHT stage ONLY, and a healthy
    //    run passes the same detection budget. The 50ms stall is added to the
    //    recorded histogram (not slept), so detection is deterministic; the 10ms
    //    budget sits ~40x above a real microsecond write (no flakiness) yet far
    //    below the 50ms signal, so a stall that leaked onto the wrong stage would
    //    trip the off-stage check too — the negative guard has teeth.
    let detect = LatencyBudget::p95(Duration::from_millis(10));
    let stalled = run_workload(
        Workload::Moderate,
        ticks,
        cols,
        rows,
        color,
        Some(Stall {
            stage: Stage::FrameGeneration,
            extra: Duration::from_millis(50),
        }),
    );
    let checks = stalled.check_budgets(&[
        (Stage::FrameGeneration, detect),
        (Stage::TerminalWrite, detect),
    ]);
    let over = |stage: Stage| checks.iter().find(|(s, _)| *s == stage).map(|(_, o)| *o);
    if over(Stage::FrameGeneration) != Some(true) {
        bad.push("budget failed to detect the injected stall on frame-generation".into());
    }
    if over(Stage::TerminalWrite) != Some(false) {
        bad.push("the injected stall leaked onto terminal-write".into());
    }
    let healthy = run_workload(Workload::Moderate, ticks, cols, rows, color, None);
    if healthy
        .check_budgets(&[(Stage::FrameGeneration, detect)])
        .iter()
        .any(|(_, over)| *over)
    {
        bad.push("a healthy run exceeded the 10ms detection budget".into());
    }

    // 6. Provenance discontinuity is observed, not swallowed.
    let restarted = run_workload_with_restart(
        Workload::Sparse,
        ticks,
        cols,
        rows,
        color,
        None,
        Some(ticks / 2),
    );
    if restarted.telemetry.disconnects() == 0 {
        bad.push("a source restart was not observed as a provenance disconnect".into());
    }

    bad
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn dispatch_is_real_work_not_a_noop() {
        assert_eq!(dispatch(RawInput::Advance), Intent::Advance);
        assert_eq!(dispatch(RawInput::Nudge(1)), Intent::Pan(1));
        assert_eq!(dispatch(RawInput::Nudge(-5)), Intent::Pan(-1));
        assert_eq!(dispatch(RawInput::Nudge(0)), Intent::None);
        assert_eq!(dispatch(RawInput::Idle), Intent::None);
    }

    #[test]
    fn compose_changes_with_the_tick_under_churn() {
        let model = Model::new(48, 24, Workload::FullChurn);
        assert_ne!(model.compose(0), model.compose(1));
    }

    #[test]
    fn one_to_one_cadence_records_no_snapshot_age() {
        let r = run_workload(Workload::Sparse, 6, 48, 24, ColorDepth::TrueColor, None);
        assert_eq!(r.telemetry.stage(Stage::SnapshotAge).count(), 0);
        assert!(!r.snapshot_age_exercised);
    }
}
