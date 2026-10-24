//! The timing half of the #143 observatory, driven deterministically.
//!
//! Every sample here is an INJECTED, known duration — never a wall-clock measurement — so the
//! percentile math and the defect-detection gate are reproducible and cannot flake. The gate's
//! worth is proven by the calibration: an injected delayed-renderer distribution is DETECTED
//! while a healthy one passes, so a green result means the budget was actually exercised.

use gibson::instrument_telemetry::{InstrumentTelemetry, LatencyBudget, LatencyHistogram, Stage};
use std::time::Duration;

fn ns(n: u64) -> Duration {
    Duration::from_nanos(n)
}

/// An empty histogram reports nothing rather than a fabricated zero.
#[test]
fn empty_histogram_reports_none() {
    let h = LatencyHistogram::new();
    assert_eq!(h.count(), 0);
    assert_eq!(h.min(), None);
    assert_eq!(h.max(), None);
    assert_eq!(h.mean(), None);
    assert_eq!(h.p50(), None);
    assert_eq!(h.p95(), None);
    assert_eq!(h.p99(), None);
}

/// Quantiles are deterministic and at bucket resolution. Samples are exact powers of two, so a
/// sample equals its own bucket floor and the expected percentile is exact.
#[test]
fn quantiles_are_deterministic_on_known_samples() {
    let mut h = LatencyHistogram::new();
    for _ in 0..95 {
        h.record(ns(1024)); // 2^10 -> bucket 11, floor 1024
    }
    for _ in 0..5 {
        h.record(ns(1_048_576)); // 2^20 -> bucket 21, floor 1_048_576
    }
    assert_eq!(h.count(), 100);
    assert_eq!(h.min(), Some(ns(1024)));
    assert_eq!(h.max(), Some(ns(1_048_576)));
    // mean is exact (summed in u128), not bucketed: (95*1024 + 5*1_048_576)/100 = 53401.
    assert_eq!(h.mean(), Some(ns(53_401)));
    // p50 and p95 land in the dense low bucket; p99 reaches the slow tail.
    assert_eq!(h.p50(), Some(ns(1024)));
    assert_eq!(h.p95(), Some(ns(1024)));
    assert_eq!(h.p99(), Some(ns(1_048_576)));
}

/// Zero is its own bucket; recording is bounded regardless of sample count.
#[test]
fn zero_and_single_sample_quantiles() {
    let mut h = LatencyHistogram::new();
    h.record_nanos(0);
    assert_eq!(h.count(), 1);
    assert_eq!(h.p50(), Some(ns(0)));
    assert_eq!(h.p99(), Some(ns(0)));
    // A huge single sample saturates into a high bucket, never panics.
    let mut g = LatencyHistogram::new();
    g.record(Duration::from_secs(60));
    assert!(g.p50().unwrap() > ns(1_000_000_000));
}

/// Each pipeline stage is measured on its own histogram — they do not bleed into one another.
#[test]
fn stages_are_measured_distinctly() {
    let mut t = InstrumentTelemetry::new();
    t.record(Stage::InputDispatch, ns(1024));
    t.record(Stage::TerminalWrite, ns(1_048_576));
    assert_eq!(t.stage(Stage::InputDispatch).count(), 1);
    assert_eq!(t.stage(Stage::TerminalWrite).count(), 1);
    assert_eq!(t.stage(Stage::SnapshotAge).count(), 0);
    assert_eq!(t.stage(Stage::InputDispatch).p50(), Some(ns(1024)));
    assert_eq!(t.stage(Stage::TerminalWrite).p50(), Some(ns(1_048_576)));
    // Every stage has a tool-relative caveat; the terminal write names its honesty limit.
    assert!(Stage::TerminalWrite
        .tool_relative_note()
        .contains("NOT proof of screen presentation"));
    assert_eq!(Stage::ALL.len(), 5);
}

/// The honest frame funnel: written <= attempted <= generated. A presenter that claimed more
/// written than attempted would be inventing frames; the receipt catches it. There is no
/// `presented` count at all — no FPS is synthesized from headless write counts.
#[test]
fn frame_funnel_is_honest() {
    let mut t = InstrumentTelemetry::new();
    for _ in 0..10 {
        t.note_frame_generated();
    }
    for _ in 0..7 {
        t.note_frame_attempted();
    }
    for _ in 0..5 {
        t.note_frame_written();
    }
    for _ in 0..3 {
        t.note_frame_coalesced();
    }
    t.add_emitted_bytes(4096);
    t.note_redraw();
    t.note_disconnect();
    assert_eq!(t.frames_generated(), 10);
    assert_eq!(t.frames_attempted(), 7);
    assert_eq!(t.frames_written(), 5);
    assert_eq!(t.frames_coalesced(), 3);
    assert_eq!(t.emitted_bytes(), 4096);
    assert_eq!(t.redraws(), 1);
    assert_eq!(t.disconnects(), 1);
    assert!(t.frame_funnel_consistent());

    // A funnel that claims more written than attempted is inconsistent — detected, not trusted.
    let mut bad = InstrumentTelemetry::new();
    bad.note_frame_generated();
    bad.note_frame_attempted();
    bad.note_frame_written();
    bad.note_frame_written(); // 2 written vs 1 attempted
    assert!(!bad.frame_funnel_consistent());
}

/// THE calibration (issue #143 anti-green-lighting): the SAME budget that a healthy distribution
/// passes is BREACHED by an injected delayed-renderer distribution. If the gate could not fail
/// here it would be certifying nothing.
#[test]
fn budget_detects_injected_delay_but_passes_healthy() {
    let budget = LatencyBudget::p95(ns(4096));

    // Healthy: every frame renders fast (well under budget).
    let mut healthy = LatencyHistogram::new();
    for _ in 0..100 {
        healthy.record(ns(1024));
    }
    assert_eq!(healthy.p95(), Some(ns(1024)));
    assert!(
        !budget.exceeded_by(&healthy),
        "a healthy distribution must pass the budget"
    );

    // Injected delay: half the frames are stalled ~1ms — p95 now sits in the slow tail.
    let mut delayed = LatencyHistogram::new();
    for _ in 0..50 {
        delayed.record(ns(1024));
    }
    for _ in 0..50 {
        delayed.record(ns(1_048_576));
    }
    assert_eq!(delayed.p95(), Some(ns(1_048_576)));
    assert!(
        budget.exceeded_by(&delayed),
        "an injected delayed-renderer distribution MUST breach the budget the healthy one passed"
    );

    // An empty histogram never exceeds — nothing was measured, so nothing is claimed.
    assert!(!budget.exceeded_by(&LatencyHistogram::new()));
}
