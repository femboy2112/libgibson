//! Mutation battery for `examples/instrument_bench` — the teeth of v0.6 R6.5 (#143).
//!
//! The example claims its numbers are *built into the real instrument loop* and
//! are *honest*. This battery proves both, and proves them SPECIFICALLY:
//!
//! * the deterministic quantities (funnel, bytes, changed cells, provenance)
//!   replay exactly — no latency VALUE is asserted (the detector checks use coarse
//!   sanity ceilings far above any realistic time, never a measured nanosecond),
//!   which keeps the battery reproducible across machines (no toolchain pin, unlike
//!   the audio PCM golden);
//! * the numbers TRACK REAL WORK — more churn moves strictly more cells and bytes,
//!   so they are measured, not canned;
//! * every detector FIRES — an injected stall is caught on the right stage and
//!   only that stage, a source restart is observed as a disconnect, coalescing is
//!   bounded and fully accounted;
//! * the report refuses to synthesise a frames-per-second / presentation claim.
//!
//! The harness source is compiled directly into this test crate (its `#[cfg(test)]
//! mod unit` laws run here too).

#[path = "../examples/instrument_bench/bench.rs"]
mod bench;

use std::time::Duration;

use bench::{format_report, run_workload, run_workload_with_restart, selfcheck, Stall, Workload};
use gibson::instrument_telemetry::{LatencyBudget, Stage};
use gibson::ColorDepth;

const COLS: u16 = 48;
const ROWS: u16 = 24;
const TICKS: u64 = 16;
const COLOR: ColorDepth = ColorDepth::TrueColor;

/// The deterministic quantities replay byte-for-byte — NOT latency.
#[test]
fn determinism_counts_replay_exactly() {
    for w in Workload::ALL {
        let a = run_workload(w, TICKS, COLS, ROWS, COLOR, None);
        let b = run_workload(w, TICKS, COLS, ROWS, COLOR, None);
        assert_eq!(
            a.telemetry.frames_generated(),
            b.telemetry.frames_generated(),
            "{}: generated not deterministic",
            w.label()
        );
        assert_eq!(
            a.telemetry.frames_coalesced(),
            b.telemetry.frames_coalesced(),
            "{}: coalesced not deterministic",
            w.label()
        );
        assert_eq!(
            a.telemetry.frames_written(),
            b.telemetry.frames_written(),
            "{}: written not deterministic",
            w.label()
        );
        assert_eq!(
            a.telemetry.emitted_bytes(),
            b.telemetry.emitted_bytes(),
            "{}: emitted bytes not deterministic",
            w.label()
        );
        assert_eq!(
            a.total_changed_cells,
            b.total_changed_cells,
            "{}: changed cells not deterministic",
            w.label()
        );
    }
}

/// The numbers track real work: more churn → strictly more changed cells and more
/// emitted bytes, for the same ticks and geometry. A canned metric could not do
/// this; it is the #143 anti-pattern (a synthetic number) made impossible.
#[test]
fn churn_is_monotonic_in_changed_cells_and_bytes() {
    let sparse = run_workload(Workload::Sparse, TICKS, COLS, ROWS, COLOR, None);
    let moderate = run_workload(Workload::Moderate, TICKS, COLS, ROWS, COLOR, None);
    let full = run_workload(Workload::FullChurn, TICKS, COLS, ROWS, COLOR, None);

    assert!(
        full.total_changed_cells > moderate.total_changed_cells
            && moderate.total_changed_cells > sparse.total_changed_cells,
        "changed cells not monotonic: full={} moderate={} sparse={}",
        full.total_changed_cells,
        moderate.total_changed_cells,
        sparse.total_changed_cells
    );
    assert!(
        full.telemetry.emitted_bytes() > moderate.telemetry.emitted_bytes()
            && moderate.telemetry.emitted_bytes() > sparse.telemetry.emitted_bytes(),
        "emitted bytes not monotonic: full={} moderate={} sparse={}",
        full.telemetry.emitted_bytes(),
        moderate.telemetry.emitted_bytes(),
        sparse.telemetry.emitted_bytes()
    );
}

/// The loop really drove the renderer — a churning workload emits real bytes and
/// records a terminal-write sample every rendered frame. Without this, every
/// honesty claim above would be vacuously true over a no-op.
#[test]
fn the_loop_actually_renders_not_a_noop() {
    let full = run_workload(Workload::FullChurn, TICKS, COLS, ROWS, COLOR, None);
    assert!(full.telemetry.emitted_bytes() > 0, "no bytes were emitted");
    assert_eq!(
        full.telemetry.stage(Stage::TerminalWrite).count(),
        TICKS,
        "terminal-write was not measured once per rendered frame"
    );
    assert_eq!(
        full.telemetry.frames_written(),
        TICKS,
        "not every attempted frame was written"
    );
}

/// The honest funnel holds for every workload: written ≤ attempted ≤ generated.
#[test]
fn funnel_is_honest_for_every_workload() {
    for w in Workload::ALL {
        let r = run_workload(w, TICKS, COLS, ROWS, COLOR, None);
        assert!(r.funnel_honest(), "{}: funnel dishonest", w.label());
    }
}

/// Coalescing is bounded and FULLY accounted: every generated frame is either
/// attempted-and-written or coalesced — none vanish, none are double-counted, and
/// nothing is queued unbounded (Law L7). The 1:1 workloads coalesce nothing.
#[test]
fn coalescing_is_bounded_and_fully_counted() {
    let back = run_workload(Workload::Backpressure, TICKS, COLS, ROWS, COLOR, None);
    let t = &back.telemetry;
    assert!(t.frames_coalesced() > 0, "backpressure coalesced nothing");
    assert_eq!(
        t.frames_generated(),
        t.frames_attempted() + t.frames_coalesced(),
        "a generated frame was neither attempted nor coalesced (leaked)"
    );
    assert_eq!(
        t.frames_written(),
        t.frames_attempted(),
        "an attempted frame was lost on the write path"
    );

    for w in [Workload::Sparse, Workload::Moderate, Workload::FullChurn] {
        let r = run_workload(w, TICKS, COLS, ROWS, COLOR, None);
        assert_eq!(
            r.telemetry.frames_coalesced(),
            0,
            "{}: 1:1 cadence wrongly coalesced",
            w.label()
        );
        assert_eq!(
            r.telemetry.frames_generated(),
            r.telemetry.frames_attempted(),
            "{}: generated != attempted at 1:1 cadence",
            w.label()
        );
    }
}

/// A provenance discontinuity is OBSERVED, not swallowed — positive and negative.
#[test]
fn source_restart_is_observed_not_swallowed() {
    let steady = run_workload_with_restart(Workload::Sparse, TICKS, COLS, ROWS, COLOR, None, None);
    assert_eq!(
        steady.telemetry.disconnects(),
        0,
        "a steady source reported a phantom disconnect"
    );

    let restarted = run_workload_with_restart(
        Workload::Sparse,
        TICKS,
        COLS,
        ROWS,
        COLOR,
        None,
        Some(TICKS / 2),
    );
    assert!(
        restarted.telemetry.disconnects() >= 1,
        "a source restart was not observed"
    );
}

/// A latency budget DETECTS an injected stall, on the RIGHT stage only, and a
/// healthy run passes a generous budget. This exercises the loop-level wiring
/// (stage → its histogram → its budget → a surfaced breach), not the primitive.
#[test]
fn budget_detects_stall_on_the_right_stage_only() {
    // The stall is injected into the recorded histogram (not slept), so a 50ms
    // signal is deterministic; the 10ms detection budget sits ~40x above a real
    // microsecond write (so the off-stage check is not wall-clock-flaky) yet far
    // below the 50ms signal, so a stall LEAKED onto terminal-write (floor 33.5ms)
    // would trip the off-stage check — the negative guard has real teeth now.
    let detect = LatencyBudget::p95(Duration::from_millis(10));
    let stalled = run_workload(
        Workload::Moderate,
        TICKS,
        COLS,
        ROWS,
        COLOR,
        Some(Stall {
            stage: Stage::FrameGeneration,
            extra: Duration::from_millis(50),
        }),
    );
    let checks = stalled.check_budgets(&[
        (Stage::FrameGeneration, detect),
        (Stage::TerminalWrite, detect),
    ]);
    let over = |stage: Stage| checks.iter().find(|(s, _)| *s == stage).unwrap().1;
    assert!(
        over(Stage::FrameGeneration),
        "stall on frame-generation not detected"
    );
    assert!(
        !over(Stage::TerminalWrite),
        "stall leaked onto terminal-write (a 50ms leak floors to 33.5ms > 10ms)"
    );

    // The SAME detection budget passes a healthy run — a real compose is ~µs, 40x
    // under 10ms, so this is a meaningful negative control, not a vacuous ceiling.
    let healthy = run_workload(Workload::Moderate, TICKS, COLS, ROWS, COLOR, None);
    assert!(
        !healthy.check_budgets(&[(Stage::FrameGeneration, detect)])[0].1,
        "a healthy run exceeded the 10ms detection budget"
    );
}

/// Stages are MEASURED, never invented: snapshot-age only where a real pipeline
/// lag exists, raster-conversion never under a headless cell render.
#[test]
fn only_measured_stages_have_samples() {
    for w in Workload::ALL {
        let r = run_workload(w, TICKS, COLS, ROWS, COLOR, None);
        // Raster conversion is not a stage of a headless cell render — ever.
        assert_eq!(
            r.telemetry.stage(Stage::RasterConversion).count(),
            0,
            "{}: fabricated a raster-conversion sample",
            w.label()
        );
        // Snapshot age exists iff the workload introduces a lag (backpressure).
        let has_age = r.telemetry.stage(Stage::SnapshotAge).count() > 0;
        assert_eq!(
            has_age,
            w == Workload::Backpressure,
            "{}: snapshot-age exercised = {}, expected only under backpressure",
            w.label(),
            has_age
        );
    }
}

/// The report refuses the universal-FPS lie: no "fps" metric anywhere, and the
/// presentation caveat is stated in words.
#[test]
fn report_makes_no_fps_or_presentation_claim() {
    let r = run_workload(Workload::Moderate, TICKS, COLS, ROWS, COLOR, None);
    let report = format_report(&r).to_lowercase();
    assert!(
        !report.contains("fps"),
        "report synthesised an fps number:\n{report}"
    );
    assert!(
        report.contains("never screen presentation"),
        "report omitted the presentation caveat"
    );
    assert!(
        report.contains("no frames-per-second number is synthesised"),
        "report omitted the explicit no-FPS disclosure"
    );
}

/// The live `--selfcheck` invariants (the CLI's runtime guard) all hold — mirrored
/// into CI so the guard and the gate can never silently diverge.
#[test]
fn live_selfcheck_invariants_all_hold() {
    let bad = selfcheck();
    assert!(bad.is_empty(), "selfcheck violations: {bad:#?}");
}
