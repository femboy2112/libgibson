//! RXIII Checkpoint B / Gate C3 — the cinematic arc's causal-honesty gate, headless.
//!
//! The four Gate C3 histories (no-edit / accepted / refused / suppressed) are driven through the
//! six-beat `Arc` and must resolve to distinct causal outcomes — and, the crux, the earned
//! TRANSFORM must be decided by the OBSERVED authoritative change, never by the producer's verdict.
//! Accepted and suppressed carry the identical `Acknowledged` verdict; only the observation that
//! follows tells them apart. This is the logical heart of the live oracle, proven without the
//! producer so the rule itself is pinned; the full live-producer oracle drives this same arc.

#[path = "../examples/field_cinema/live.rs"]
mod live;

use live::{Arc, Beat, ObsSummary, Verdict};

const BASE: f64 = 1.0;
const TARGET: f64 = 2.0;

/// no-edit: establish, discover, let the model advance naturally (peak value unchanged), resolve.
/// Ordinary motion never earns a transformation.
fn history_no_edit() -> Arc {
    let mut a = Arc::new();
    a.observe(ObsSummary::new(1, Some(BASE)));
    a.discover();
    a.observe(ObsSummary::new(2, Some(BASE)));
    a.observe(ObsSummary::new(3, Some(BASE)));
    a.resolve();
    a
}

/// accepted: an acked set-amplitude whose effect is then OBSERVED (peak ~doubles) — the earned
/// transformation.
fn history_accepted() -> Arc {
    let mut a = Arc::new();
    a.observe(ObsSummary::new(1, Some(BASE)));
    a.discover();
    a.intervene();
    a.verdict(Verdict::Acknowledged, TARGET);
    a.observe(ObsSummary::new(2, Some(TARGET))); // authoritative evidence the edit landed
    a.resolve();
    a
}

/// refused: an out-of-range edit the producer refuses — nothing is armed, no transformation.
fn history_refused() -> Arc {
    let mut a = Arc::new();
    a.observe(ObsSummary::new(1, Some(BASE)));
    a.discover();
    a.intervene();
    a.verdict(Verdict::Refused, 99.0);
    a.observe(ObsSummary::new(2, Some(BASE)));
    a.resolve();
    a
}

/// suppressed: the producer ACKNOWLEDGES the edit but the model is frozen, so the peak never moves.
/// The ack is identical to the accepted case; the absent observation is the whole difference.
fn history_suppressed() -> Arc {
    let mut a = Arc::new();
    a.observe(ObsSummary::new(1, Some(BASE)));
    a.discover();
    a.intervene();
    a.verdict(Verdict::Acknowledged, TARGET);
    a.observe(ObsSummary::new(2, Some(BASE))); // acked, but frozen — no authoritative change
    a.resolve();
    a
}

/// Only the accepted history earns a transformation; the other three resolve without one.
#[test]
fn only_the_accepted_history_earns_a_transformation() {
    assert!(
        !history_no_edit().transformed(),
        "ordinary motion is not a transformation"
    );
    assert!(
        history_accepted().transformed(),
        "an observed, accepted change IS a transformation"
    );
    assert!(
        !history_refused().transformed(),
        "a refused edit cannot transform"
    );
    assert!(
        !history_suppressed().transformed(),
        "an acked-but-suppressed edit must NOT transform"
    );

    // All four end resolved.
    for a in [
        history_no_edit(),
        history_accepted(),
        history_refused(),
        history_suppressed(),
    ] {
        assert_eq!(a.beat(), Beat::Resolve);
    }
}

/// THE CRUX (C3 teeth). Accepted and suppressed receive the IDENTICAL `Acknowledged` verdict, yet
/// only accepted transforms — the transformation is decided by the observed authoritative change,
/// not the ack. A field-blind "transform on ack" rule would fire for BOTH and is thereby refuted.
#[test]
fn an_ack_is_not_proof_of_a_transformation() {
    let accepted = history_accepted();
    let suppressed = history_suppressed();

    // Same verdict path (both acked), opposite outcome.
    assert!(accepted.transformed());
    assert!(!suppressed.transformed());

    // The mutation control: a rule that transformed on the ack alone would make these two equal.
    let transform_on_ack = true; // both histories were acknowledged
    assert!(
        transform_on_ack && accepted.transformed() != suppressed.transformed(),
        "the real gate separates acked-applied from acked-suppressed; a transform-on-ack rule could not"
    );
}

/// The arc awaits across a suppressed ack rather than resolving it as done: it stays armed in AWAIT
/// until authoritative evidence arrives, and here it never does.
#[test]
fn a_suppressed_ack_stays_in_await_without_transforming() {
    let mut a = Arc::new();
    a.observe(ObsSummary::new(1, Some(BASE)));
    a.intervene();
    a.verdict(Verdict::Acknowledged, TARGET);
    assert_eq!(a.beat(), Beat::Await);
    assert!(a.awaiting(), "armed for the authoritative change");
    a.observe(ObsSummary::new(2, Some(BASE))); // frozen
    assert_eq!(
        a.beat(),
        Beat::Await,
        "no evidence ⇒ still awaiting, not transformed"
    );
    assert!(a.awaiting());
    assert!(!a.transformed());
}

/// The evidence threshold requires the authoritative change to actually land: a partial move below
/// the threshold does not transform; reaching it does.
#[test]
fn the_transform_requires_the_change_to_cross_the_evidence_threshold() {
    // Below threshold (0.8·2.0 = 1.6): 1.5 is not yet evidence.
    let mut below = Arc::new();
    below.observe(ObsSummary::new(1, Some(BASE)));
    below.intervene();
    below.verdict(Verdict::Acknowledged, TARGET);
    below.observe(ObsSummary::new(2, Some(1.5)));
    assert!(
        !below.transformed(),
        "1.5 < 1.6 is not yet the authoritative change"
    );
    assert_eq!(below.beat(), Beat::Await);

    // At/above threshold: evidence lands, the transform is earned.
    let mut at = Arc::new();
    at.observe(ObsSummary::new(1, Some(BASE)));
    at.intervene();
    at.verdict(Verdict::Acknowledged, TARGET);
    at.observe(ObsSummary::new(2, Some(1.7)));
    assert!(at.transformed(), "1.7 ≥ 1.6 is the authoritative change");
    assert_eq!(at.beat(), Beat::Transform);
}

/// The beat order holds: ESTABLISH → DISCOVER → INTERVENE → AWAIT → TRANSFORM for the accepted arc.
#[test]
fn the_accepted_arc_walks_the_six_beats_in_order() {
    let mut a = Arc::new();
    a.observe(ObsSummary::new(1, Some(BASE)));
    assert_eq!(a.beat(), Beat::Establish);
    a.discover();
    assert_eq!(a.beat(), Beat::Discover);
    a.intervene();
    assert_eq!(a.beat(), Beat::Intervene);
    a.verdict(Verdict::Acknowledged, TARGET);
    assert_eq!(a.beat(), Beat::Await);
    a.observe(ObsSummary::new(2, Some(TARGET)));
    assert_eq!(a.beat(), Beat::Transform);
    a.resolve();
    assert_eq!(a.beat(), Beat::Resolve);
    assert!(a.transformed(), "the transform latch survives resolve");
}
