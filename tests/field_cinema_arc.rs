//! RXIII Checkpoint B / Gate C3 — the cinematic arc's causal-honesty gate, headless.
//!
//! The four Gate C3 histories (no-edit / accepted / refused / suppressed) are driven through the
//! six-beat `Arc` and must resolve to distinct causal outcomes — and, the crux, the earned
//! TRANSFORM must be decided by the OBSERVED authoritative change, never by the producer's verdict.
//! Accepted and suppressed carry the identical `Acknowledged` verdict; only the observation that
//! follows tells them apart. This is the logical heart of the live oracle, proven without the
//! producer so the rule itself is pinned; the full live-producer oracle drives this same arc.
//!
//! RXIV Gate A: the arc now judges a typed effect contract (baseline → target within a declared
//! tolerance, newer cursor, same epoch) instead of the refuted `peak >= 0.8·target` threshold, and
//! requires one natural-variation calibration observation before an intervention. Each history
//! below therefore observes twice before intervening. The four-history claims are unchanged.

#[path = "../examples/field_cinema/live.rs"]
mod live;

use live::{Arc, Beat, CycleOutcome, Evidence, ObsSummary, Verdict};

const BASE: f64 = 1.0;
const TARGET: f64 = 2.0;

/// An in-session observation at producer epoch 1, sequence/tick `seq`.
fn obs(seq: u64, v: f64) -> ObsSummary {
    ObsSummary::new(1, seq, seq, Some(v))
}

/// establish (seq 1) + discover with one calibration observation (seq 2).
fn calibrated() -> Arc {
    let mut a = Arc::new();
    a.observe(obs(1, BASE));
    a.discover();
    a.observe(obs(2, BASE));
    a
}

/// no-edit: establish, discover, let the model advance naturally (peak value unchanged), resolve.
/// Ordinary motion never earns a transformation.
fn history_no_edit() -> Arc {
    let mut a = calibrated();
    a.observe(obs(3, BASE));
    a.resolve();
    a
}

/// accepted: an acked set-amplitude whose effect is then OBSERVED (peak ~doubles) — the earned
/// transformation.
fn history_accepted() -> Arc {
    let mut a = calibrated();
    a.intervene(TARGET);
    a.verdict(Verdict::Acknowledged);
    a.observe(obs(3, TARGET)); // authoritative evidence consistent with the edit
    a.resolve();
    a
}

/// refused: an out-of-range edit the producer refuses — nothing is armed, no transformation.
fn history_refused() -> Arc {
    let mut a = calibrated();
    a.intervene(99.0);
    a.verdict(Verdict::Refused);
    a.observe(obs(3, BASE));
    a.resolve();
    a
}

/// suppressed: the producer ACKNOWLEDGES the edit but the model is frozen, so the peak never moves.
/// The ack is identical to the accepted case; the absent observation is the whole difference.
fn history_suppressed() -> Arc {
    let mut a = calibrated();
    a.intervene(TARGET);
    a.verdict(Verdict::Acknowledged);
    a.observe(obs(3, BASE)); // acked, but frozen — no authoritative change
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
    assert_eq!(
        history_refused().last_outcome(),
        Some(CycleOutcome::Refused)
    );
    assert_eq!(
        history_suppressed().last_outcome(),
        Some(CycleOutcome::Unevidenced {
            last: Evidence::Unchanged
        })
    );
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
/// until authoritative evidence arrives (within the contract's evidence window), and here it never
/// does — so the window closes the cycle as UNEVIDENCED, never as a transformation.
#[test]
fn a_suppressed_ack_stays_in_await_without_transforming() {
    let mut a = calibrated();
    a.intervene(TARGET);
    a.verdict(Verdict::Acknowledged);
    assert_eq!(a.beat(), Beat::Await);
    assert!(a.awaiting(), "armed for the authoritative change");
    assert_eq!(a.observe(obs(3, BASE)), Some(Evidence::Unchanged)); // frozen
    assert_eq!(
        a.beat(),
        Beat::Await,
        "no evidence ⇒ still awaiting, not transformed"
    );
    assert!(a.awaiting());
    assert!(!a.transformed());
    // The window (3 newer observations) closes the cycle honestly.
    a.observe(obs(4, BASE));
    a.observe(obs(5, BASE));
    assert_eq!(a.beat(), Beat::Discover);
    assert_eq!(
        a.last_outcome(),
        Some(CycleOutcome::Unevidenced {
            last: Evidence::Unchanged
        })
    );
    assert!(!a.transformed());
}

/// The evidence rule requires the observation to land ON the requested effect (within the declared
/// tolerance), not merely to cross a fraction of it. RXIII's `0.8·target` threshold accepted a
/// partial 1.7 for a requested 2.0; under the contract a partial move is a CHANGE that is
/// INCONSISTENT with the request, and only a consistent observation transforms.
#[test]
fn the_transform_requires_the_observation_to_land_within_tolerance_of_the_request() {
    // tolerance for 1.0 → 2.0 under the reference contract = 0.05 · 2.0 = 0.1.
    for partial in [1.5, 1.7, 1.85] {
        let mut a = calibrated();
        a.intervene(TARGET);
        a.verdict(Verdict::Acknowledged);
        assert_eq!(a.observe(obs(3, partial)), Some(Evidence::Inconsistent));
        assert!(
            !a.transformed(),
            "{partial} changed, but is not the requested 2.0"
        );
        assert_eq!(a.beat(), Beat::Await);
    }

    for landed in [1.91, 2.0, 2.09] {
        let mut a = calibrated();
        a.intervene(TARGET);
        a.verdict(Verdict::Acknowledged);
        assert_eq!(a.observe(obs(3, landed)), Some(Evidence::Consistent));
        assert!(a.transformed(), "{landed} is within tolerance of 2.0");
        assert_eq!(a.beat(), Beat::Transform);
    }
}

/// The beat order holds: ESTABLISH → DISCOVER → INTERVENE → AWAIT → TRANSFORM for the accepted arc.
#[test]
fn the_accepted_arc_walks_the_six_beats_in_order() {
    let mut a = Arc::new();
    a.observe(obs(1, BASE));
    assert_eq!(a.beat(), Beat::Establish);
    a.discover();
    assert_eq!(a.beat(), Beat::Discover);
    a.observe(obs(2, BASE));
    assert_eq!(
        a.beat(),
        Beat::Discover,
        "calibration does not change the beat"
    );
    assert_eq!(a.intervene(TARGET), Evidence::Pending);
    assert_eq!(a.beat(), Beat::Intervene);
    a.verdict(Verdict::Acknowledged);
    assert_eq!(a.beat(), Beat::Await);
    a.observe(obs(3, TARGET));
    assert_eq!(a.beat(), Beat::Transform);
    a.resolve();
    assert_eq!(a.beat(), Beat::Resolve);
    assert!(a.transformed(), "the transform latch survives resolve");
}
