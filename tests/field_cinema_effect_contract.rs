//! RXIV Gate A — the cinematic effect contract: TRANSFORM is never earned by an ACK alone, nor by
//! an unchanged measurement that happens to be high.
//!
//! # Criteria, declared before the controls
//!
//! | outcome      | evidence classes                                                     | TRANSFORM |
//! |--------------|----------------------------------------------------------------------|-----------|
//! | PASS         | `Consistent` (newer cursor, same epoch, changed > tol, within tol of target) | yes |
//! | FAIL         | `Unchanged`, `Inconsistent`, producer `Refused`                      | no        |
//! | AMBIGUOUS    | `Stale`, `EpochBreak`, `Unmeasurable`, `UnmeasurableBaseline`, `Uncalibrated`, `NonStationary`, `Indiscriminable` | no |
//!
//! The A1 counterexample (baseline 2.0, target 2.0, ACK, observed 2.0) transformed under the RXIII
//! `peak >= 0.8·target` rule — recorded RED against the untouched arc at `c55d277` (3/3 probes,
//! including the real producer's no-op `set-amplitude 1.0`). Every scenario of that RED run is
//! carried here as a permanent regression, alongside the full A3 hostile battery, a second source
//! with a different geometry, and the matched-counterfactual attribution witness (the only place
//! causal attribution is claimed: the deterministic reference producer, no-edit vs edited at
//! identical model ticks).

#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;
#[path = "../examples/field_cinema/live.rs"]
mod live;
#[path = "../examples/field_cinema/live_link.rs"]
mod live_link;

use cinema::{observe, SourceId};
use gibson::field_view::{FieldExtent, FieldView};
use gibson::wire::AckStatus;
use gibson::Point;
use live::{classify, Arc, Beat, CycleOutcome, Evidence, ExpectedEffect, ObsSummary, Verdict};
use live_link::{arc_summary, stage_live, verdict_of, Link, LiveField};

fn obs(seq: u64, v: Option<f64>) -> ObsSummary {
    ObsSummary::new(1, seq, seq, v)
}

/// Establish at `base`, discover, and calibrate with a second observation at `base2`.
fn calibrated(base: f64, base2: f64) -> Arc {
    let mut a = Arc::new();
    a.observe(obs(1, Some(base)));
    a.discover();
    a.observe(obs(2, Some(base2)));
    a
}

/// Run one acknowledged cycle: intervene `target`, ACK, then observe each of `after` in order.
fn acked(base: f64, target: f64, after: &[Option<f64>]) -> (Arc, Vec<Option<Evidence>>) {
    let mut a = calibrated(base, base);
    a.intervene(target);
    a.verdict(Verdict::Acknowledged);
    let evs = after
        .iter()
        .enumerate()
        .map(|(i, v)| a.observe(obs(3 + i as u64, *v)))
        .collect();
    (a, evs)
}

/// The refuted RXIII rule, kept only as the discriminating contrast.
fn legacy_rule(target: f64, observed: f64) -> bool {
    observed >= 0.8 * target
}

// ---------------------------------------------------------------------------------------------
// A1 — the counterexample, now a permanent regression
// ---------------------------------------------------------------------------------------------

#[test]
fn a1_unchanged_baseline_already_at_target_never_transforms() {
    let mut a = calibrated(2.0, 2.0);
    assert_eq!(
        a.intervene(2.0),
        Evidence::Indiscriminable,
        "a request equal to the current value cannot be evidenced"
    );
    a.verdict(Verdict::Acknowledged);
    a.observe(obs(3, Some(2.0)));
    assert!(!a.transformed());
    assert_ne!(a.beat(), Beat::Transform);
    assert_eq!(
        a.last_outcome(),
        Some(CycleOutcome::Unevidenced {
            last: Evidence::Indiscriminable
        })
    );
    assert!(
        legacy_rule(2.0, 2.0),
        "contrast: the refuted rule fires on this exact case"
    );
}

#[test]
fn a1_unchanged_baseline_above_the_old_threshold_never_transforms() {
    // baseline 2.0, target 2.5 (0.8·2.5 = 2.0): the old rule fires on the UNCHANGED 2.0.
    let (a, evs) = acked(2.0, 2.5, &[Some(2.0)]);
    assert_eq!(evs, vec![Some(Evidence::Unchanged)]);
    assert!(!a.transformed());
    assert!(
        legacy_rule(2.5, 2.0),
        "contrast: the refuted rule is fooled"
    );
}

#[test]
fn a1_a_lower_target_is_judged_by_the_change_not_the_magnitude() {
    // baseline 2.0, target 0.5: unchanged 2.0 is NOT evidence (old rule: 2.0 >= 0.4 fired) …
    let (a, evs) = acked(2.0, 0.5, &[Some(2.0)]);
    assert_eq!(evs, vec![Some(Evidence::Unchanged)]);
    assert!(!a.transformed());
    assert!(
        legacy_rule(0.5, 2.0),
        "contrast: the refuted rule is fooled"
    );
    // … but a genuine decrease that lands on the request IS.
    let (b, evs) = acked(2.0, 0.5, &[Some(0.52)]);
    assert_eq!(evs, vec![Some(Evidence::Consistent)]);
    assert!(b.transformed());
}

// ---------------------------------------------------------------------------------------------
// A3 — hostile controls (pure arc)
// ---------------------------------------------------------------------------------------------

#[test]
fn a3_accepted_edit_with_a_genuinely_changed_quantity_transforms() {
    let (a, evs) = acked(1.0, 2.0, &[Some(1.998)]);
    assert_eq!(evs, vec![Some(Evidence::Consistent)]);
    assert_eq!(a.beat(), Beat::Transform);
    assert_eq!(a.transforms(), 1);
}

#[test]
fn a3_refused_edit_arms_nothing_and_later_matches_cannot_transform() {
    let mut a = calibrated(1.0, 1.0);
    a.intervene(2.0);
    a.verdict(Verdict::Refused);
    assert_eq!(a.beat(), Beat::Discover);
    assert_eq!(a.last_outcome(), Some(CycleOutcome::Refused));
    // Even an observation that happens to equal the refused target is not a transform.
    assert_eq!(a.observe(obs(3, Some(2.0))), None);
    assert!(!a.transformed());
}

#[test]
fn a3_acknowledged_but_suppressed_closes_unevidenced_after_the_window() {
    let (a, evs) = acked(1.0, 2.0, &[Some(1.0), Some(0.999), Some(1.001)]);
    assert_eq!(evs, vec![Some(Evidence::Unchanged); 3]);
    assert!(!a.transformed());
    assert_eq!(a.beat(), Beat::Discover);
    assert_eq!(
        a.last_outcome(),
        Some(CycleOutcome::Unevidenced {
            last: Evidence::Unchanged
        })
    );
}

#[test]
fn a3_unrelated_natural_evolution_is_inconsistent_not_a_transform() {
    // After the ACK the quantity drifts on its own to 1.6 — a change, but not the requested 2.0.
    let (a, evs) = acked(1.0, 2.0, &[Some(1.6)]);
    assert_eq!(evs, vec![Some(Evidence::Inconsistent)]);
    assert!(!a.transformed());
    // A source whose natural variation already exceeds the tolerance before the intervention is
    // refused as NON-STATIONARY: nothing could be attributed against it.
    let mut b = calibrated(1.0, 1.4);
    assert_eq!(b.intervene(2.0), Evidence::NonStationary);
    b.verdict(Verdict::Acknowledged);
    b.observe(obs(3, Some(2.0)));
    assert!(
        !b.transformed(),
        "a non-stationary source cannot earn a transform"
    );
}

#[test]
fn a3_missing_or_nonfinite_measurements_are_ambiguous() {
    for bad in [
        None,
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(f64::NEG_INFINITY),
    ] {
        let (a, evs) = acked(1.0, 2.0, &[bad]);
        assert_eq!(evs, vec![Some(Evidence::Unmeasurable)], "{bad:?}");
        assert!(!a.transformed());
    }
    // An unmeasurable BASELINE cannot arm an effect at all.
    let mut b = Arc::new();
    b.observe(obs(1, Some(1.0)));
    b.discover();
    b.observe(obs(2, None));
    assert_eq!(b.intervene(2.0), Evidence::UnmeasurableBaseline);
    let mut c = Arc::new();
    c.observe(obs(1, Some(1.0)));
    c.discover();
    c.observe(obs(2, Some(1.0)));
    assert_eq!(
        c.intervene(f64::NAN),
        Evidence::Indiscriminable,
        "a non-finite target"
    );
}

#[test]
fn a3_stale_or_repeated_cursor_never_evidences_even_with_the_target_value() {
    let mut a = calibrated(1.0, 1.0);
    a.intervene(2.0);
    a.verdict(Verdict::Acknowledged);
    // A repeated cursor (seq 2 = the baseline's) carrying the target value is NOT newer evidence.
    assert_eq!(a.observe(obs(2, Some(2.0))), Some(Evidence::Stale));
    // An older cursor likewise.
    assert_eq!(a.observe(obs(1, Some(2.0))), Some(Evidence::Stale));
    assert!(!a.transformed());
    assert_eq!(
        a.beat(),
        Beat::Await,
        "stale frames do not consume the evidence window"
    );
    // The genuine newer observation still evidences.
    assert_eq!(a.observe(obs(3, Some(2.0))), Some(Evidence::Consistent));
}

#[test]
fn a3_an_epoch_break_voids_the_baseline() {
    let mut a = calibrated(1.0, 1.0);
    a.intervene(2.0);
    a.verdict(Verdict::Acknowledged);
    // A restarted producer (epoch 2) reporting exactly the target is not evidence about epoch 1.
    assert_eq!(
        a.observe(ObsSummary::new(2, 1, 1, Some(2.0))),
        Some(Evidence::EpochBreak)
    );
    assert!(!a.transformed());
    assert_eq!(
        a.last_outcome(),
        Some(CycleOutcome::Void {
            reason: Evidence::EpochBreak
        })
    );
}

#[test]
fn a3_an_uncalibrated_intervention_cannot_be_evidenced() {
    let mut a = Arc::new();
    a.observe(obs(1, Some(1.0)));
    assert_eq!(a.intervene(2.0), Evidence::Uncalibrated);
    a.verdict(Verdict::Acknowledged);
    a.observe(obs(2, Some(2.0)));
    assert!(!a.transformed());
}

#[test]
fn classify_is_the_pure_contract() {
    let e = ExpectedEffect {
        baseline: obs(5, Some(1.0)),
        baseline_value: 1.0,
        target: 2.0,
        tolerance: 0.1,
    };
    assert_eq!(classify(&e, &obs(6, Some(2.05))), Evidence::Consistent);
    assert_eq!(classify(&e, &obs(6, Some(1.05))), Evidence::Unchanged);
    assert_eq!(classify(&e, &obs(6, Some(1.5))), Evidence::Inconsistent);
    assert_eq!(classify(&e, &obs(6, Some(3.0))), Evidence::Inconsistent);
    assert_eq!(classify(&e, &obs(5, Some(2.0))), Evidence::Stale);
    assert_eq!(classify(&e, &obs(6, None)), Evidence::Unmeasurable);
    assert_eq!(
        classify(&e, &ObsSummary::new(2, 6, 6, Some(2.0))),
        Evidence::EpochBreak
    );
}

// ---------------------------------------------------------------------------------------------
// A3 — a second source model with a different geometry (anisotropic, offset origin, masked)
// ---------------------------------------------------------------------------------------------

/// A rotating bright arc on a ring: non-Gaussian, anisotropic cells, negative origin, sparse mask.
/// The amplitude scales the whole field; natural evolution rotates the arc.
fn ring(amp: f64, t: u64) -> (FieldExtent, Vec<f64>, Vec<bool>) {
    let (cols, rows) = (37usize, 19usize);
    let ext = FieldExtent::new(Point::new(-7.5, 3.0), 0.5, 1.25, cols, rows).unwrap();
    let (cx, cy, r, s) = (18.0, 9.0, 6.0, 2.5);
    let phi = t as f64 * 0.4;
    let mut data = Vec::with_capacity(cols * rows);
    let mut mask = Vec::with_capacity(cols * rows);
    for iy in 0..rows {
        for ix in 0..cols {
            let (dx, dy) = (ix as f64 - cx, iy as f64 - cy);
            let d = (dx * dx + dy * dy).sqrt();
            let theta = dy.atan2(dx);
            let radial = (-(d - r) * (d - r) / (2.0 * s * s)).exp();
            data.push(amp * radial * (0.75 + 0.25 * (theta - phi).cos()));
            mask.push((ix * 7 + iy * 3) % 11 != 0);
        }
    }
    (ext, data, mask)
}

fn ring_obs(amp: f64, t: u64) -> ObsSummary {
    let (ext, data, mask) = ring(amp, t);
    let view = FieldView::new(ext, &data, Some(&mask)).unwrap();
    let o = observe(&view, SourceId(9), 4, t);
    ObsSummary::new(4, t, t, o.peak_value)
}

#[test]
fn a3_a_second_geometry_obeys_the_same_contract() {
    // Natural evolution (the arc rotates) stays inside the declared tolerance on this geometry.
    let mut drift: f64 = 0.0;
    for t in 1..12 {
        let (a, b) = (
            ring_obs(1.0, t).peak_value.unwrap(),
            ring_obs(1.0, t + 1).peak_value.unwrap(),
        );
        drift = drift.max((b - a).abs());
    }
    assert!(
        drift < 0.05,
        "ring natural drift {drift} must sit inside the 5% band for the positive path"
    );

    // accepted: amplitude 1 → 3 is evidenced on the new geometry.
    let mut acc = Arc::new();
    acc.observe(ring_obs(1.0, 1));
    acc.discover();
    acc.observe(ring_obs(1.0, 2));
    assert_eq!(acc.intervene(3.0), Evidence::Pending);
    acc.verdict(Verdict::Acknowledged);
    assert_eq!(acc.observe(ring_obs(3.0, 3)), Some(Evidence::Consistent));
    assert!(acc.transformed());

    // suppressed: identical ack, amplitude held — the rotation alone never transforms.
    let mut sup = Arc::new();
    sup.observe(ring_obs(1.0, 1));
    sup.discover();
    sup.observe(ring_obs(1.0, 2));
    sup.intervene(3.0);
    sup.verdict(Verdict::Acknowledged);
    for t in 3..6 {
        assert_eq!(sup.observe(ring_obs(1.0, t)), Some(Evidence::Unchanged));
    }
    assert!(!sup.transformed());
}

// ---------------------------------------------------------------------------------------------
// The real producer — the A1 counterexample, the four histories, and the attribution witness
// ---------------------------------------------------------------------------------------------

fn stage(f: &LiveField) -> ObsSummary {
    let (o, _, _) = stage_live(&f.field);
    arc_summary(&o, f.cursor)
}

#[test]
fn real_producer_noop_edit_is_indiscriminable_and_never_transforms() {
    let mut link = Link::spawn(false);
    let mut arc = Arc::new();
    arc.observe(stage(&link.recv_field()));
    arc.discover();
    link.send_advance();
    arc.observe(stage(&link.recv_field()));
    assert_eq!(arc.intervene(1.0), Evidence::Indiscriminable);
    link.send_amplitude(1.0);
    let ack = link.recv_ack();
    assert_eq!(
        ack,
        AckStatus::Acknowledged,
        "the producer genuinely acks it"
    );
    arc.verdict(verdict_of(ack));
    link.send_advance();
    arc.observe(stage(&link.recv_field()));
    link.finish();
    assert!(
        !arc.transformed(),
        "a real acknowledged no-op edit is not a transform"
    );
}

/// The declared stationarity of the reference source, MEASURED: over 12 natural ticks the peak
/// sample moves by far less than the 5% contract band (grid discretization only).
#[test]
fn real_producer_natural_variation_is_inside_the_declared_tolerance() {
    let mut link = Link::spawn(false);
    let mut prev = stage(&link.recv_field()).peak_value.unwrap();
    let mut worst: f64 = 0.0;
    for _ in 0..12 {
        link.send_advance();
        let v = stage(&link.recv_field()).peak_value.unwrap();
        worst = worst.max((v - prev).abs() / prev.abs());
        prev = v;
    }
    link.finish();
    assert!(
        worst < 0.01,
        "measured relative natural drift {worst:.5} (declared band 0.05)"
    );
}

/// THE ATTRIBUTION WITNESS. Matched no-edit and edited histories at IDENTICAL model ticks against
/// the deterministic producer: the edited field is exactly 2× the counterfactual, cell for cell,
/// and the counterfactual itself is UNCHANGED against the baseline. Only here — where a matched
/// counterfactual exists — is the change attributed to the command.
#[test]
fn real_producer_matched_counterfactual_attributes_the_change_to_the_command() {
    // edited: seq1, advance → seq2 (calibration), set-amp 2.0 → ACK, advance → seq3.
    let mut ed = Link::spawn(false);
    let e1 = ed.recv_field();
    ed.send_advance();
    let e2 = ed.recv_field();
    ed.send_amplitude(2.0);
    assert_eq!(ed.recv_ack(), AckStatus::Acknowledged);
    ed.send_advance();
    let e3 = ed.recv_field();
    ed.finish();

    // counterfactual: the same script with the edit removed.
    let mut cf = Link::spawn(false);
    let c1 = cf.recv_field();
    cf.send_advance();
    let c2 = cf.recv_field();
    cf.send_advance();
    let c3 = cf.recv_field();
    cf.finish();

    // Identical model ticks and identical pre-edit history.
    assert_eq!(e3.field.model_tick, c3.field.model_tick);
    assert_eq!(e1.field.data, c1.field.data);
    assert_eq!(e2.field.data, c2.field.data);

    // The edited field is exactly the counterfactual scaled by 2 — every cell.
    assert_eq!(e3.field.data.len(), c3.field.data.len());
    for (e, c) in e3.field.data.iter().zip(&c3.field.data) {
        assert!(
            (e - 2.0 * c).abs() <= 1e-12 * c.abs().max(1.0),
            "cell {e} != 2 x {c}"
        );
    }

    // And the arc's judgment agrees: edited Consistent, counterfactual Unchanged.
    let tol = live::EffectContract::REFERENCE.tolerance(1.0, 2.0);
    let base = stage(&e2);
    let effect = ExpectedEffect {
        baseline: base,
        baseline_value: base.peak_value.unwrap(),
        target: 2.0,
        tolerance: tol,
    };
    assert_eq!(classify(&effect, &stage(&e3)), Evidence::Consistent);
    assert_eq!(classify(&effect, &stage(&c3)), Evidence::Unchanged);
}

#[test]
fn real_producer_a_decrease_after_an_increase_is_a_second_evidenced_transform() {
    let mut link = Link::spawn(false);
    let mut arc = Arc::new();
    arc.observe(stage(&link.recv_field()));
    arc.discover();
    link.send_advance();
    arc.observe(stage(&link.recv_field()));

    for target in [2.0, 0.5] {
        assert_eq!(arc.intervene(target), Evidence::Pending, "target {target}");
        link.send_amplitude(target);
        arc.verdict(verdict_of(link.recv_ack()));
        link.send_advance();
        assert_eq!(
            arc.observe(stage(&link.recv_field())),
            Some(Evidence::Consistent),
            "target {target}"
        );
        // Recalibrate on the new state before the next intervention.
        arc.discover();
        link.send_advance();
        arc.observe(stage(&link.recv_field()));
    }
    link.finish();
    assert_eq!(arc.transforms(), 2);
}

#[test]
fn real_producer_refused_and_suppressed_never_transform() {
    // refused (out of range)
    let mut link = Link::spawn(false);
    let mut arc = Arc::new();
    arc.observe(stage(&link.recv_field()));
    arc.discover();
    link.send_advance();
    arc.observe(stage(&link.recv_field()));
    arc.intervene(99.0);
    link.send_amplitude(99.0);
    let ack = link.recv_ack();
    assert_eq!(ack, AckStatus::Refused);
    arc.verdict(verdict_of(ack));
    link.send_advance();
    arc.observe(stage(&link.recv_field()));
    link.finish();
    assert!(!arc.transformed());

    // suppressed (the mutation that removes the authoritative update)
    let mut link = Link::spawn(true);
    let mut arc = Arc::new();
    arc.observe(stage(&link.recv_field()));
    arc.discover();
    link.send_advance();
    arc.observe(stage(&link.recv_field()));
    arc.intervene(2.0);
    link.send_amplitude(2.0);
    let ack = link.recv_ack();
    assert_eq!(ack, AckStatus::Acknowledged);
    arc.verdict(verdict_of(ack));
    link.send_advance();
    assert_eq!(
        arc.observe(stage(&link.recv_field())),
        Some(Evidence::Unchanged)
    );
    link.finish();
    assert!(!arc.transformed());
}
