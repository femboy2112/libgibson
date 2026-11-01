//! The command-loop half of the #143 observatory: a per-stage latency witness for the command
//! leg (`Stage::CommandDispatch`), driven deterministically.
//!
//! Gate E wired the RENDER-loop stages (input/snapshot/frame/raster/write) into the real renderer
//! and explicitly ceded the command funnel because "there is no command Stage in the library".
//! This file exercises the stage that closes that gap. The command COUNTS (issued / delivered /
//! acknowledged) are already witnessed by the wire `CommandChannel` and its hostile battery; what
//! was missing was a place to record the command leg's *latency distribution* and a budget that
//! can DETECT a command stall. That is what `Stage::CommandDispatch` adds.
//!
//! Honesty discipline, identical to `tests/instrument_telemetry.rs`:
//! * Every sample is an INJECTED, known `Duration` — no clock is ever read — so the percentile
//!   math and the gate are reproducible and never flake (law L10: no wall-clock latency *value*
//!   is asserted as a cross-host truth; only the relative DETECTION of an injected distribution).
//! * The gate's worth is proven by calibration: the SAME budget a healthy command distribution
//!   passes is breached by an injected command-stall distribution. A gate that could not fail
//!   would certify nothing.
//! * The stage witnesses the DELIVERY leg (issue -> confirmed write+flush), NOT producer
//!   acknowledgement — keeping #143's issued/delivered/acknowledged legs distinct (law L7).

use gibson::instrument_telemetry::{InstrumentTelemetry, LatencyBudget, Stage};
use std::time::Duration;

fn ns(n: u64) -> Duration {
    Duration::from_nanos(n)
}

/// A healthy command distribution: every command delivers fast (well under budget).
fn record_healthy(t: &mut InstrumentTelemetry, stage: Stage) {
    for _ in 0..100 {
        t.record(stage, ns(1024)); // 2^10 -> bucket floor 1024
    }
}

/// An injected command stall: half the commands take ~1ms, so p95 sits in the slow tail.
fn record_stalled(t: &mut InstrumentTelemetry, stage: Stage) {
    for _ in 0..50 {
        t.record(stage, ns(1024));
    }
    for _ in 0..50 {
        t.record(stage, ns(1_048_576)); // 2^20 -> bucket floor 1_048_576
    }
}

/// The new stage is part of the catalog and labels the leg it witnesses honestly: a *delivery*,
/// explicitly not an acknowledgement. This is what keeps the three command legs distinct.
#[test]
fn command_stage_is_in_the_catalog_with_an_honest_delivery_note() {
    assert!(
        Stage::ALL.contains(&Stage::CommandDispatch),
        "the command stage must be enumerable alongside the render-loop stages"
    );
    assert_eq!(Stage::ALL.len(), Stage::COUNT);

    let note = Stage::CommandDispatch.tool_relative_note();
    assert!(
        note.contains("confirmed delivery"),
        "the note must name the leg it witnesses (delivery), got: {note:?}"
    );
    assert!(
        note.contains("NOT producer acknowledgement"),
        "the note must disclaim acknowledgement so delivery is never read as a round-trip, got: {note:?}"
    );
}

/// THE calibration: the same budget a healthy command loop passes is BREACHED by an injected
/// command-stall distribution, recorded THROUGH the per-stage telemetry (so the new stage's array
/// slot is actually exercised — a missing array bump would panic here, a missing `ALL` bump would
/// fail the catalog test above).
#[test]
fn command_budget_detects_injected_stall_but_passes_healthy() {
    let budget = LatencyBudget::p95(ns(4096));

    let mut healthy = InstrumentTelemetry::new();
    record_healthy(&mut healthy, Stage::CommandDispatch);
    assert_eq!(healthy.stage(Stage::CommandDispatch).p95(), Some(ns(1024)));
    assert!(
        !budget.exceeded_by(healthy.stage(Stage::CommandDispatch)),
        "a healthy command distribution must pass the budget"
    );

    let mut stalled = InstrumentTelemetry::new();
    record_stalled(&mut stalled, Stage::CommandDispatch);
    assert_eq!(
        stalled.stage(Stage::CommandDispatch).p95(),
        Some(ns(1_048_576))
    );
    assert!(
        budget.exceeded_by(stalled.stage(Stage::CommandDispatch)),
        "an injected command-stall distribution MUST breach the budget the healthy one passed"
    );

    // An empty command stage never exceeds — nothing was measured, so nothing is claimed.
    let empty = InstrumentTelemetry::new();
    assert!(!budget.exceeded_by(empty.stage(Stage::CommandDispatch)));
}

/// The teeth against conflation: a stall on an UNRELATED stage must not read as a command stall.
/// One telemetry carries a healthy command loop while the render write is stalled; the command
/// budget stays quiet on the command stage (stages do not bleed) even though the identical stall
/// is plainly detected on the stage that actually has it.
#[test]
fn a_stall_on_another_stage_does_not_breach_the_command_budget() {
    let budget = LatencyBudget::p95(ns(4096));

    let mut t = InstrumentTelemetry::new();
    record_healthy(&mut t, Stage::CommandDispatch);
    record_stalled(&mut t, Stage::TerminalWrite);

    assert!(
        budget.exceeded_by(t.stage(Stage::TerminalWrite)),
        "control: the stall is real and the budget detects it on the stage that has it"
    );
    assert!(
        !budget.exceeded_by(t.stage(Stage::CommandDispatch)),
        "the command budget must stay quiet when the delay is on a render stage — no bleed, no conflation"
    );

    // Symmetric control: a command stall must not be read off a healthy render stage either.
    let mut u = InstrumentTelemetry::new();
    record_stalled(&mut u, Stage::CommandDispatch);
    record_healthy(&mut u, Stage::TerminalWrite);
    assert!(budget.exceeded_by(u.stage(Stage::CommandDispatch)));
    assert!(!budget.exceeded_by(u.stage(Stage::TerminalWrite)));
}
