//! RXIII Checkpoint B / Gate C — the four-history causal oracle, driven by the REAL producer.
//!
//! `field_cinema_arc.rs` pins the arc's causal-honesty rule with *scripted* observations. This
//! test closes the loop: it drives the SAME six-beat `Arc` from the actual Python↔Rust producer
//! over real OS pipes, so the film's load-bearing claim — *only an observed, accepted change earns
//! a TRANSFORM* — is proven against real fields and real verdicts, not hand-fed summaries.
//!
//! Four histories, three on the normal producer and one on `--freeze-amplitude`:
//!
//! | history    | edit            | real ack        | field after      | transforms? |
//! |------------|-----------------|-----------------|------------------|-------------|
//! | no-edit    | none            | —               | drifts at amp 1  | no          |
//! | accepted   | set-amp 2.0     | Acknowledged    | peak ~doubles    | **yes**     |
//! | refused    | set-amp 99.0    | Refused         | unchanged        | no          |
//! | suppressed | set-amp 2.0     | Acknowledged    | frozen at amp 1  | no          |
//!
//! The crux: accepted and suppressed carry the IDENTICAL real `Acknowledged` ack; only the
//! observation that follows separates them. The mutation control proves the transform is caused by
//! the observed field, not the ack — remove the authoritative update and the positive film fails.
//! Lockstep, so the proof is deterministic and sleep-free; if python3 or the producer cannot be
//! spawned, the test fails loudly rather than skipping.

#[path = "../examples/wire_instrument/channel.rs"]
mod channel;
#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;
#[path = "../examples/wire_instrument/input.rs"]
mod input;
#[path = "../examples/field_cinema/live.rs"]
mod live;
#[path = "../examples/field_cinema/live_link.rs"]
mod live_link;
#[path = "../examples/wire_instrument/transport.rs"]
mod transport;

use cinema::Shot;
use gibson::surface::Surface;
use gibson::wire::AckStatus;
use live::{Arc, Beat, Verdict};
use live_link::{
    arc_summary, stage_live, verdict_of, LiveSession, ProducerSpec, LIVE_SOURCE, SHUTDOWN_GRACE,
};

/// Spawn the reference lockstep producer through the certified session.
fn live_session(freeze: bool) -> LiveSession {
    LiveSession::spawn(&ProducerSpec::reference(freeze), LIVE_SOURCE)
        .expect("spawn python3 bindings/python/producer.py --lockstep (is python3 on PATH?)")
}

/// Submit a set-amplitude edit and return the verdict correlated to ITS command id.
fn edit(s: &mut LiveSession, v: f64) -> AckStatus {
    let id = s.submit_amplitude(v).expect("deliver the edit");
    s.await_verdict(id).expect("the edit's own verdict")
}

/// The presenter's intended target for the set-amplitude edit (its own command stamp — the arc's
/// effect contract judges the observation against it within a declared tolerance). The producer's
/// authoritative value is observed, not trusted.
const TARGET: f64 = 2.0;

/// Everything one live history produces, so histories can be compared on every bearing the film
/// asserts: the causal beat path, the real ack, the final rendered surface, the composed shot, the
/// observed peaks, and the earned-transform latch.
struct Film {
    beats: Vec<Beat>,
    ack: Option<AckStatus>,
    final_surface: Surface,
    final_shot: Shot,
    baseline_peak: Option<f64>,
    final_peak: Option<f64>,
    transformed: bool,
    final_beat: Beat,
}

/// Drive an EDIT history (accepted / refused / suppressed) end to end through the real producer and
/// the arc. `freeze` selects the suppressing producer; `amp` is the edit value submitted.
fn drive_edit(freeze: bool, amp: f64) -> Film {
    let mut link = live_session(freeze);
    let mut arc = Arc::new();
    let mut beats = Vec::new();

    // ESTABLISH — the unsolicited seq=1 authoritative field sets the baseline.
    let f0 = link.next_field().unwrap();
    let (o0, _s0, _surf0) = stage_live(&f0).unwrap();
    arc.observe(arc_summary(&o0, f0.cursor));
    beats.push(arc.beat());

    // DISCOVER — the viewer probes while one natural tick calibrates the source's variation (the
    // effect contract requires it before any change can be attributed). Then INTERVENE.
    arc.discover();
    link.advance().unwrap();
    let fc = link.next_field().unwrap();
    let (oc, _sc, _surfc) = stage_live(&fc).unwrap();
    arc.observe(arc_summary(&oc, fc.cursor));
    beats.push(arc.beat());
    arc.intervene(amp);
    beats.push(arc.beat());

    // The producer renders its TRUE verdict. ACK arms AWAIT; REFUSED disarms to DISCOVER.
    let ack = edit(&mut link, amp);
    arc.verdict(verdict_of(ack));
    beats.push(arc.beat());

    // Advance one tick and OBSERVE the authoritative field that follows. This — not the ack — is
    // the only thing that can carry the arc into TRANSFORM.
    link.advance().unwrap();
    let f1 = link.next_field().unwrap();
    let (o1, s1, surf1) = stage_live(&f1).unwrap();
    arc.observe(arc_summary(&o1, f1.cursor));
    beats.push(arc.beat());

    // RESOLVE — the transform latch (if earned) survives the close.
    arc.resolve();
    beats.push(arc.beat());
    link.finish(SHUTDOWN_GRACE);

    Film {
        beats,
        ack: Some(ack),
        final_surface: surf1,
        final_shot: s1,
        baseline_peak: o0.peak_value,
        final_peak: o1.peak_value,
        transformed: arc.transformed(),
        final_beat: arc.beat(),
    }
}

/// Drive the NO-EDIT history: establish, discover, let the model drift two ticks at amplitude 1,
/// resolve. Ordinary motion (the peak travels, its value does not) never earns a transform.
fn drive_no_edit() -> Film {
    let mut link = live_session(false);
    let mut arc = Arc::new();
    let mut beats = Vec::new();

    let f0 = link.next_field().unwrap();
    let (o0, _s0, _surf0) = stage_live(&f0).unwrap();
    arc.observe(arc_summary(&o0, f0.cursor));
    beats.push(arc.beat());
    arc.discover();
    beats.push(arc.beat());

    link.advance().unwrap();
    let f1 = link.next_field().unwrap();
    let (o1, _s1, _surf1) = stage_live(&f1).unwrap();
    arc.observe(arc_summary(&o1, f1.cursor));
    beats.push(arc.beat());

    link.advance().unwrap();
    let f2 = link.next_field().unwrap();
    let (o2, s2, surf2) = stage_live(&f2).unwrap();
    arc.observe(arc_summary(&o2, f2.cursor));
    beats.push(arc.beat());

    arc.resolve();
    beats.push(arc.beat());
    link.finish(SHUTDOWN_GRACE);

    Film {
        beats,
        ack: None,
        final_surface: surf2,
        final_shot: s2,
        baseline_peak: o0.peak_value,
        final_peak: o2.peak_value,
        transformed: arc.transformed(),
        final_beat: arc.beat(),
    }
}

fn accepted() -> Film {
    drive_edit(false, TARGET)
}
fn refused() -> Film {
    drive_edit(false, 99.0)
}
fn suppressed() -> Film {
    drive_edit(true, TARGET)
}

/// Only the accepted history earns a transformation; the other three resolve without one — and all
/// four reach RESOLVE. This is the scripted `field_cinema_arc` claim, now against the real wire.
#[test]
fn only_the_accepted_real_history_earns_the_transform() {
    let n = drive_no_edit();
    let a = accepted();
    let r = refused();
    let s = suppressed();

    assert!(
        !n.transformed,
        "ordinary real motion is not a transformation"
    );
    assert!(
        a.transformed,
        "an observed, accepted real change IS a transformation"
    );
    assert!(!r.transformed, "a refused real edit cannot transform");
    assert!(
        !s.transformed,
        "an acked-but-suppressed real edit must NOT transform"
    );

    for f in [&n, &a, &r, &s] {
        assert_eq!(f.final_beat, Beat::Resolve, "every history resolves");
    }

    // The real verdicts are what the table claims.
    assert_eq!(a.ack, Some(AckStatus::Acknowledged));
    assert_eq!(r.ack, Some(AckStatus::Refused));
    assert_eq!(s.ack, Some(AckStatus::Acknowledged));
}

/// THE CRUX, against the real producer. Accepted and suppressed receive the byte-identical real
/// `Acknowledged` verdict, yet only accepted transforms — the transformation is decided by the
/// observed authoritative field, never the ack. And the film LOOKS different: the earned-transform
/// history renders a different surface and a different shot than the suppressed one. A field-blind
/// "transform on ack" rule would fire for both and is thereby refuted on the wire.
#[test]
fn the_identical_real_ack_is_separated_only_by_the_observed_field() {
    let a = accepted();
    let s = suppressed();

    // Same real verdict path — both producers acknowledged the edit.
    assert_eq!(a.ack, Some(AckStatus::Acknowledged));
    assert_eq!(s.ack, Some(AckStatus::Acknowledged));
    assert_eq!(a.ack, s.ack, "the acks are identical");

    // Opposite causal outcome, decided by the field that followed.
    assert!(a.transformed && !s.transformed);

    // The accepted field actually ~doubled; the suppressed field stayed at the baseline.
    let ap = a.final_peak.expect("accepted peak");
    let sp = s.final_peak.expect("suppressed peak");
    assert!(
        ap > 1.5 && sp < 1.2,
        "accepted peak ~2.0 ({ap}) vs suppressed ~1.0 ({sp}) — the real fields diverge"
    );

    // The beat paths diverge at the final observation: accepted reaches TRANSFORM, suppressed
    // stays in AWAIT (armed, never evidenced) before RESOLVE.
    assert_eq!(a.beats[4], Beat::Transform);
    assert_eq!(s.beats[4], Beat::Await);

    // The film is visibly different and the composed shot differs — the transform is on screen.
    assert_ne!(
        a.final_surface, s.final_surface,
        "the earned transform renders a different cell surface than the suppressed history"
    );
    assert_ne!(
        a.final_shot, s.final_shot,
        "the composed shot differs (the doubled range reframes the camera)"
    );
}

/// THE ORACLE'S TEETH — the mutation control. Run the real accepted producer, capture its real
/// `Acknowledged` ack AND the real post-ack field, then drive two arcs that differ in ONE thing:
/// whether the authoritative update is observed. The truth arc observes the real doubled field and
/// transforms; the mutant arc — identical path, identical real ack — has the authoritative update
/// removed (it re-observes the baseline) and must NOT transform. This is the control that would
/// have to turn red if the film ever decided the transform on the ack.
#[test]
fn removing_the_authoritative_update_breaks_the_positive_film() {
    let mut link = live_session(false);
    let f0 = link.next_field().unwrap();
    let (o0, _s0, _surf0) = stage_live(&f0).unwrap();
    let baseline = arc_summary(&o0, f0.cursor);
    link.advance().unwrap();
    let fc = link.next_field().unwrap();
    let (oc, _sc, _surfc) = stage_live(&fc).unwrap();
    let calibration = arc_summary(&oc, fc.cursor);

    let ack = edit(&mut link, TARGET);
    assert_eq!(
        ack,
        AckStatus::Acknowledged,
        "the real producer acked the edit"
    );

    link.advance().unwrap();
    let f1 = link.next_field().unwrap();
    let (o1, _s1, _surf1) = stage_live(&f1).unwrap();
    let authoritative = arc_summary(&o1, f1.cursor); // the real doubled field
    link.finish(SHUTDOWN_GRACE);

    assert!(
        o1.peak_value.unwrap() > 1.5,
        "the authoritative post-ack field really did ~double"
    );

    // Truth: observe the authoritative change → TRANSFORM.
    let mut truth = Arc::new();
    truth.observe(baseline);
    truth.discover();
    truth.observe(calibration);
    truth.intervene(TARGET);
    truth.verdict(Verdict::Acknowledged);
    truth.observe(authoritative);
    truth.resolve();

    // Mutant: identical beats, identical real ack — but the authoritative update is REMOVED: the
    // post-ack frame carries the newer cursor but the pre-edit (calibration) measurement. The ONLY
    // difference from truth is the observed value.
    let mut mutant = Arc::new();
    mutant.observe(baseline);
    mutant.discover();
    mutant.observe(calibration);
    mutant.intervene(TARGET);
    mutant.verdict(Verdict::Acknowledged);
    let mut dropped = authoritative;
    dropped.peak_value = calibration.peak_value; // authoritative update dropped
    mutant.observe(dropped);
    mutant.resolve();

    assert!(
        truth.transformed(),
        "observing the authoritative change earns the transform"
    );
    assert!(
        !mutant.transformed(),
        "dropping the authoritative update must break the transform — the ack alone is not proof"
    );
    assert!(
        truth.transformed() != mutant.transformed(),
        "the gate is decided by the observed field; a transform-on-ack rule could not tell these apart"
    );
}

/// The live film replays deterministically through the arc: the same lockstep accepted script,
/// against the deterministic producer, yields the identical beat path, the identical transform
/// latch, and byte-identical final surfaces.
#[test]
fn the_accepted_live_film_replays_identically() {
    let a = accepted();
    let b = accepted();

    assert_eq!(a.beats, b.beats, "the causal beat path replays identically");
    assert_eq!(a.transformed, b.transformed);
    assert!(a.transformed, "and it is the transformed arc");
    assert_eq!(
        a.final_surface, b.final_surface,
        "the live-wire film replays byte-identically — deterministic producer, pure grammar"
    );
    assert_eq!(a.final_shot, b.final_shot);
}

/// The accepted arc walks the six beats in order on the real wire:
/// ESTABLISH → DISCOVER → INTERVENE → AWAIT → TRANSFORM → RESOLVE.
#[test]
fn the_accepted_real_arc_walks_the_six_beats_in_order() {
    let a = accepted();
    assert_eq!(
        a.beats,
        vec![
            Beat::Establish,
            Beat::Discover,
            Beat::Intervene,
            Beat::Await,
            Beat::Transform,
            Beat::Resolve,
        ],
        "the earned-transform history walks the full arc in order"
    );
    assert_eq!(a.baseline_peak.map(|p| p > 0.8), Some(true));
}
