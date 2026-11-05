//! RXIII Checkpoint A / Gate B3 — the causal interaction trace is faithful where the field-only
//! capture is not.
//!
//! The load-bearing claim: a field checksum does not fingerprint interaction history. These tests
//! turn that into evidence — two sessions with the IDENTICAL field sequence (equal field-projection
//! digest) but different command histories have different causal digests; the accepted / refused /
//! suppressed edit histories are all distinguished; the trace round-trips preserving order; a
//! malformed trace is rejected within finite resources; and the digest catches exactly the mutations
//! a causal oracle must catch — an omitted command, a changed verdict.

#[path = "../examples/wire_instrument/trace.rs"]
mod trace;

use trace::{
    read_trace, read_trace_with, write_trace, Admission, CausalTrace, OutcomeKind, TraceError,
    TraceEvent, TraceLimits, Verdict,
};

// Field content checksums (as capture::content_digest would produce), by role:
const C0: u64 = 0x0000_0000_0000_000A; // baseline field at tick 1
const C1_NATURAL: u64 = 0x0000_0000_0000_000B; // tick 2 on the undisturbed trajectory
const C1_CHANGED: u64 = 0x0000_0000_0000_000C; // tick 2 after an edit actually applied

fn obs(seq: u64, tick: u64, checksum: u64) -> TraceEvent {
    TraceEvent::Observation {
        source: 1,
        epoch: 1,
        sequence: seq,
        model_tick: tick,
        field_checksum: checksum,
    }
}
fn cmd(id: u64) -> TraceEvent {
    TraceEvent::Command {
        command_id: id,
        dest_source: 1,
        kind: 7,
        admission: Admission::Accepted,
    }
}
fn verdict(id: u64, v: Verdict) -> TraceEvent {
    TraceEvent::Verdict {
        command_id: id,
        verdict: v,
    }
}

fn build(events: &[TraceEvent]) -> CausalTrace {
    let mut t = CausalTrace::new();
    let lim = TraceLimits::default();
    for e in events {
        assert!(t.record(e.clone(), &lim), "within budget");
    }
    t
}

// The four Gate C3 histories. Note every edit is ADMITTED (Accepted) and the producer verdict is
// explicit; the model's fate is carried entirely by the observation that follows.
fn history_no_edit() -> Vec<TraceEvent> {
    vec![obs(1, 1, C0), obs(2, 2, C1_NATURAL)]
}
fn history_refused() -> Vec<TraceEvent> {
    // The edit was refused; the model stayed on its natural trajectory — identical fields to no-edit.
    vec![
        obs(1, 1, C0),
        cmd(1),
        verdict(1, Verdict::Refused),
        obs(2, 2, C1_NATURAL),
    ]
}
fn history_accepted() -> Vec<TraceEvent> {
    // Acked AND applied: the tick-2 field is the changed one.
    vec![
        obs(1, 1, C0),
        cmd(1),
        verdict(1, Verdict::Acknowledged),
        obs(2, 2, C1_CHANGED),
    ]
}
fn history_suppressed() -> Vec<TraceEvent> {
    // Acked but the model update was suppressed: SAME ack as accepted, but the field did not change.
    vec![
        obs(1, 1, C0),
        cmd(1),
        verdict(1, Verdict::Acknowledged),
        obs(2, 2, C1_NATURAL),
    ]
}

/// The headline B3 property: identical field sequence, different causal history. No-edit and
/// refused-edit observe exactly the same fields (equal field-projection digest), so the field-only
/// capture cannot tell them apart — but the causal trace can.
#[test]
fn same_fields_different_command_history_share_a_field_projection_but_differ_causally() {
    let no_edit = build(&history_no_edit());
    let refused = build(&history_refused());

    assert_eq!(
        no_edit.field_projection_digest(),
        refused.field_projection_digest(),
        "the observed field sequence is identical — the capture digest would match"
    );
    assert_ne!(
        no_edit.causal_digest(),
        refused.causal_digest(),
        "the causal trace records the refused command the field projection cannot"
    );
}

/// Accepted / refused / suppressed are all distinguished — and the crux: accepted and suppressed
/// carry the SAME Acknowledged verdict, told apart ONLY by the observation that follows, while
/// refused and suppressed observe the SAME field, told apart ONLY by the verdict.
#[test]
fn accepted_refused_and_suppressed_edit_histories_are_distinguished() {
    let accepted = build(&history_accepted());
    let refused = build(&history_refused());
    let suppressed = build(&history_suppressed());

    assert_ne!(accepted.causal_digest(), refused.causal_digest());
    assert_ne!(accepted.causal_digest(), suppressed.causal_digest());
    assert_ne!(refused.causal_digest(), suppressed.causal_digest());

    // accepted vs suppressed: same Acknowledged verdict, different observation — an ack is not proof
    // of a model change.
    assert_ne!(
        accepted.field_projection_digest(),
        suppressed.field_projection_digest(),
        "accepted applied; suppressed did not — only the observation shows it"
    );

    // refused vs suppressed: identical observed fields, different producer verdict.
    assert_eq!(
        refused.field_projection_digest(),
        suppressed.field_projection_digest(),
        "refused and suppressed observe identical fields"
    );
    assert_ne!(
        refused.causal_digest(),
        suppressed.causal_digest(),
        "but the verdict distinguishes them"
    );
}

/// The trace round-trips through its binary form preserving exact event order and content — so a
/// replay reproduces the semantic command state and ordering, not an approximation.
#[test]
fn a_trace_round_trips_preserving_exact_order() {
    let t = build(&[
        obs(1, 1, C0),
        TraceEvent::Probe {
            world_x: 1.5,
            world_y: -2.5,
        },
        TraceEvent::Focus { page: 2 },
        cmd(1),
        verdict(1, Verdict::Acknowledged),
        obs(2, 2, C1_CHANGED),
        TraceEvent::Outcome {
            kind: OutcomeKind::Quit,
            pending_commands: 0,
        },
    ]);

    let mut buf = Vec::new();
    write_trace(&mut buf, &t).expect("write trace");
    let back = read_trace(&mut buf.as_slice()).expect("read trace");

    assert_eq!(
        back.events(),
        t.events(),
        "event order and content are preserved"
    );
    assert_eq!(back, t, "the whole trace round-trips");
    assert_eq!(
        back.causal_digest(),
        t.causal_digest(),
        "the causal digest survives the round-trip"
    );
}

/// A renderer capability change cannot mutate the causal history: the trace records only semantic
/// events — no color depth, terminal size or render mode is an input — so the same history
/// fingerprints identically however the session was rendered, yet a genuine semantic change does
/// move the digest (it is not a vacuous constant).
#[test]
fn renderer_capability_changes_do_not_mutate_the_causal_history() {
    // "Rendered" at two different depths (a choice the trace type cannot even represent): identical.
    let as_if_mono = build(&history_accepted());
    let as_if_truecolor = build(&history_accepted());
    assert_eq!(
        as_if_mono.causal_digest(),
        as_if_truecolor.causal_digest(),
        "rendering is not an input to the causal digest"
    );
    // But a real semantic difference does change it.
    assert_ne!(
        as_if_mono.causal_digest(),
        build(&history_refused()).causal_digest(),
        "the digest is a real function of the semantic history"
    );
}

/// A malformed trace is rejected within finite resources: a count beyond budget is refused before
/// allocation, bad magic / truncation / invalid tags are typed refusals.
#[test]
fn a_malformed_trace_is_rejected_within_finite_resources() {
    // Count beyond budget — refused before any event allocation.
    let mut over = b"WIRETRC1".to_vec();
    over.extend_from_slice(&1_000_000u32.to_le_bytes());
    let tiny = TraceLimits { max_events: 4 };
    assert!(matches!(
        read_trace_with(&mut over.as_slice(), &tiny),
        Err(TraceError::EventsExceedBudget {
            declared: 1_000_000,
            cap: 4
        })
    ));

    // Bad magic.
    let mut wrong = b"WIRETRC2".to_vec();
    wrong.extend_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        read_trace(&mut wrong.as_slice()),
        Err(TraceError::BadMagic)
    ));

    // Truncated header.
    assert!(matches!(
        read_trace(&mut b"WIR".as_slice()),
        Err(TraceError::Truncated)
    ));

    // Invalid event tag.
    let mut bad_tag = b"WIRETRC1".to_vec();
    bad_tag.extend_from_slice(&1u32.to_le_bytes()); // one event
    bad_tag.push(0); // truncated=false
    bad_tag.push(99); // bogus event tag
    assert!(matches!(
        read_trace(&mut bad_tag.as_slice()),
        Err(TraceError::InvalidEventTag(99))
    ));

    // Invalid enum tag inside a Command's admission.
    let mut bad_enum = b"WIRETRC1".to_vec();
    bad_enum.extend_from_slice(&1u32.to_le_bytes());
    bad_enum.push(0); // truncated=false
    bad_enum.push(4); // TAG_COMMAND
    bad_enum.extend_from_slice(&1u64.to_le_bytes()); // command_id
    bad_enum.extend_from_slice(&1u64.to_le_bytes()); // dest_source
    bad_enum.extend_from_slice(&7u32.to_le_bytes()); // kind
    bad_enum.push(9); // bogus admission tag
    assert!(matches!(
        read_trace(&mut bad_enum.as_slice()),
        Err(TraceError::InvalidEnumTag {
            field: "admission",
            tag: 9
        })
    ));
}

/// The mutation oracle Gate C3 depends on: the causal digest catches an omitted command and a
/// changed verdict — the two ways a replay could lie about causality.
#[test]
fn the_causal_digest_catches_an_omitted_command_or_a_changed_verdict() {
    let base = build(&history_accepted());

    // Omit the command entirely.
    let omitted = build(&[
        obs(1, 1, C0),
        verdict(1, Verdict::Acknowledged),
        obs(2, 2, C1_CHANGED),
    ]);
    assert_ne!(
        base.causal_digest(),
        omitted.causal_digest(),
        "an omitted command is caught"
    );

    // Flip the verdict Acknowledged → Refused, keeping everything else.
    let flipped = build(&[
        obs(1, 1, C0),
        cmd(1),
        verdict(1, Verdict::Refused),
        obs(2, 2, C1_CHANGED),
    ]);
    assert_ne!(
        base.causal_digest(),
        flipped.causal_digest(),
        "a changed verdict is caught"
    );
}

/// Recording is bounded: past the budget the trace drops events and flips its `truncated` marker
/// rather than growing without bound, and the marker is part of the fingerprint.
#[test]
fn recording_is_bounded_and_marks_truncation() {
    let lim = TraceLimits { max_events: 2 };
    let mut t = CausalTrace::new();
    assert!(t.record(obs(1, 1, C0), &lim));
    assert!(t.record(obs(2, 2, C1_NATURAL), &lim));
    assert!(
        !t.record(obs(3, 3, C1_CHANGED), &lim),
        "past the cap the event is dropped"
    );
    assert!(t.is_truncated(), "and the truncation marker is set");
    assert_eq!(t.len(), 2, "bounded at the cap");

    // The marker participates in the digest: an identical-events-but-untruncated trace differs.
    let untruncated = build(&[obs(1, 1, C0), obs(2, 2, C1_NATURAL)]);
    assert_eq!(untruncated.events(), t.events());
    assert_ne!(
        untruncated.causal_digest(),
        t.causal_digest(),
        "truncation is fingerprinted"
    );
}
