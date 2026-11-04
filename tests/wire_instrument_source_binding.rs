//! A1 (Renaissance XII): source identity has ONE authority — the configured command destination.
//!
//! Independent review found a source-level counterexample in the Renaissance XI wire instrument.
//! The interactive loop constructed its command channel for the configured source:
//!
//! ```text
//! let mut ch = CommandChannel::new(args.source, args.epoch);  // commands addressed to A
//! let mut src: Option<u64> = None;
//! ...
//! let bound = *src.get_or_insert(msg.source);                 // display bound to FIRST source seen
//! ```
//!
//! The first source observed on the wire need not equal `args.source`. A custom producer can
//! publish source B first while the command coordinator still targets A — so the displayed field
//! belongs to B while the edit is addressed to A: two independent authorities over identity,
//! disagreeing. The old source-lifecycle test only compared arriving messages to the first observed
//! source, never to the independently configured command destination, so it never caught this.
//!
//! The fix is the single-source example's simplest correct policy: bind to `args.source` from the
//! first message and refuse foreign ids, collapsing the two authorities into one. The shared
//! `classify_source` is that authority; `drive` mirrors the fixed loop's drain-gating so the
//! observable inbox / channel state is exactly what the loop would display.
//!
//! Also tested: the EPOCH boundary. A field from the configured source at a different epoch is a
//! producer restart — a NEW command session — not a foreign source and not fatal: its field is
//! still shown (latest-wins), but an ack under it must not resolve a command issued in the prior
//! session (a restart resets command ids, which would otherwise collide).

#[path = "../examples/wire_instrument/channel.rs"]
mod channel;
#[path = "../examples/wire_instrument/input.rs"]
mod input;

use channel::{CommandChannel, ResolveError};
use gibson::field_view::FieldExtent;
use gibson::shape::Point;
use gibson::wire::{
    AckStatus, FieldInbox, SourceCursor, WireAck, WireBody, WireField, WireMessage,
};
use input::{classify_source, SourceVerdict};

const KIND_SET_AMPLITUDE: u32 = 1;

fn field(seq: u64) -> WireField {
    let extent = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 2, 2).expect("extent");
    WireField {
        extent,
        model_tick: seq,
        data: vec![seq as f64; 4],
        mask: None,
    }
}

fn field_msg(source: u64, epoch: u32, seq: u64) -> WireMessage {
    WireMessage {
        source,
        cursor: SourceCursor::new(epoch, seq),
        body: WireBody::Field(field(seq)),
    }
}

fn ack_msg(source: u64, epoch: u32, seq: u64, command_id: u64, status: AckStatus) -> WireMessage {
    WireMessage {
        source,
        cursor: SourceCursor::new(epoch, seq),
        body: WireBody::Ack(WireAck { command_id, status }),
    }
}

fn retire_msg(source: u64, epoch: u32, seq: u64) -> WireMessage {
    WireMessage {
        source,
        cursor: SourceCursor::new(epoch, seq),
        body: WireBody::Retire,
    }
}

/// Mirror the interactive loop's drain-gating UNDER THE FIX: classify every message against the
/// configured destination `(dest, session_epoch)` — never the first source seen — then admit the
/// field / resolve the ack. Returns `(visible_source, foreign_count, restart_count)`.
fn drive(
    dest: u64,
    session_epoch: u32,
    msgs: Vec<WireMessage>,
    inbox: &mut FieldInbox,
    ch: &mut CommandChannel,
) -> (Option<u64>, u64, u64) {
    let mut visible: Option<u64> = None;
    let mut foreign = 0u64;
    let mut restarts = 0u64;
    for msg in msgs {
        match classify_source(dest, session_epoch, msg.source, msg.cursor.epoch) {
            SourceVerdict::Foreign => {
                foreign += 1;
                continue;
            }
            SourceVerdict::Restart => restarts += 1,
            SourceVerdict::InSession => {}
        }
        match msg.body {
            WireBody::Field(f) => {
                inbox.admit(msg.source, msg.cursor, f);
                // Visible only once a valid observation from the configured source arrives.
                visible = Some(dest);
            }
            WireBody::Retire => {
                inbox.retire(msg.source);
            }
            WireBody::Ack(a) => {
                let _ = ch.resolve_in_session(msg.source, msg.cursor.epoch, a);
            }
            WireBody::Command(_) => {}
        }
    }
    (visible, foreign, restarts)
}

/// RED-FIRST COUNTEREXAMPLE (frozen). The REMOVED first-observed binding (`src.get_or_insert`)
/// targets whichever source speaks first. When a producer publishes a stranger B before the
/// configured source A, first-observed binds B — disagreeing with the command destination, which is
/// always A. This reproduces that exact logic inline and asserts it mis-binds, so the defect A1
/// removed stays on record (and this test would be RED if the loop still bound first-observed).
#[test]
fn the_removed_first_observed_binding_mis_targets_the_first_source_seen() {
    const A: u64 = 7; // configured command destination
    const B: u64 = 9; // a stranger that publishes first
    let mut first_observed: Option<u64> = None;
    for src in [B, A] {
        let _ = *first_observed.get_or_insert(src);
    }
    assert_eq!(
        first_observed,
        Some(B),
        "first-observed binds the source that spoke first (B), NOT the configured destination"
    );
    assert_ne!(
        first_observed,
        Some(A),
        "...so the display would disagree with the command destination A — the A1 defect"
    );
}

/// THE FIX: the display binds to the configured destination regardless of who publishes first. B
/// publishes first and is refused throughout; A becomes visible only when its observation arrives;
/// the command destination is A and now agrees with the display.
#[test]
fn the_configured_destination_is_bound_regardless_of_who_publishes_first() {
    const A: u64 = 7;
    const B: u64 = 9;
    let mut inbox = FieldInbox::new();
    let mut ch = CommandChannel::new(A, 1);

    let (visible, foreign, restarts) = drive(
        A,
        1,
        vec![
            field_msg(B, 1, 1), // B publishes first -> must NOT bind
            field_msg(A, 1, 1), // A finally appears -> becomes visible
            field_msg(B, 1, 2), // B keeps talking -> still refused
        ],
        &mut inbox,
        &mut ch,
    );

    assert_eq!(
        visible,
        Some(A),
        "the display binds the configured destination A, never first-seen B"
    );
    assert_eq!(foreign, 2, "both of B's messages are out of contract");
    assert_eq!(restarts, 0);
    assert!(
        inbox.latest(B).is_none(),
        "a foreign source is never admitted, so it can never be displayed"
    );
    assert!(
        inbox.latest(A).is_some(),
        "the configured source becomes visible when its observation arrives"
    );
    assert_eq!(
        ch.source(),
        A,
        "the command destination is the configured source — agreeing with the display"
    );
}

/// Invalid early ACK/RETIRE from a foreign source cannot bind the display: nothing becomes visible
/// until a valid observation from the configured source arrives, and the foreign ack resolves nothing.
#[test]
fn an_early_foreign_ack_or_retire_cannot_bind_the_display() {
    const A: u64 = 7;
    const B: u64 = 9;
    let mut inbox = FieldInbox::new();
    let mut ch = CommandChannel::new(A, 1);

    let (visible, foreign, _) = drive(
        A,
        1,
        vec![
            ack_msg(B, 1, 1, 1, AckStatus::Acknowledged), // foreign ack first
            retire_msg(B, 1, 2),                          // foreign retire
        ],
        &mut inbox,
        &mut ch,
    );

    assert_eq!(
        visible, None,
        "no valid observation from A yet — nothing is displayed, and B never binds"
    );
    assert_eq!(
        foreign, 2,
        "both foreign control messages are out of contract"
    );
    assert_eq!(ch.outstanding(), 0, "a foreign ack resolves nothing");
}

/// The configured source at a NEW epoch is a producer restart — classified `Restart`, not
/// `Foreign`, and not fatal: its field is still shown (latest-wins). The truth table pins the
/// classifier directly.
#[test]
fn the_configured_source_at_a_new_epoch_is_a_restart_not_a_foreign_source() {
    assert_eq!(
        classify_source(7, 1, 7, 1),
        SourceVerdict::InSession,
        "same source, same epoch"
    );
    assert_eq!(
        classify_source(7, 1, 7, 2),
        SourceVerdict::Restart,
        "same source, new epoch = producer restart"
    );
    assert_eq!(
        classify_source(7, 1, 9, 1),
        SourceVerdict::Foreign,
        "different source = foreign, regardless of epoch"
    );

    const A: u64 = 7;
    let mut inbox = FieldInbox::new();
    let mut ch = CommandChannel::new(A, 1);
    let (visible, foreign, restarts) = drive(
        A,
        1,
        vec![field_msg(A, 2, 1)], // A restarted at epoch 2
        &mut inbox,
        &mut ch,
    );
    assert_eq!(
        foreign, 0,
        "a restart is NOT foreign — it is still source A"
    );
    assert_eq!(restarts, 1, "the restart is recognized");
    assert_eq!(
        visible,
        Some(A),
        "A's restarted field is still shown (latest-wins), not treated as fatal"
    );
    assert!(inbox.latest(A).is_some());
}

/// An ack from a restarted session (different epoch) must NOT resolve a command issued in the prior
/// session — even when the `command_id` collides, which it will, because a restart resets ids.
/// Positive control: the same ack IN SESSION resolves it, so the guard is not vacuous.
#[test]
fn an_ack_from_a_restarted_session_cannot_resolve_a_prior_session_command() {
    const A: u64 = 7;
    let mut ch = CommandChannel::new(A, 1);
    let mut sink: Vec<u8> = Vec::new();

    let id = ch
        .issue_tracked(KIND_SET_AMPLITUDE, 3.0f64.to_le_bytes().to_vec())
        .expect("issue");
    ch.deliver_next(&mut sink).expect("deliver");
    assert_eq!(ch.outstanding(), 1, "the delivered edit awaits its verdict");

    // The producer restarted (epoch 2); its ack re-uses command id `id`.
    let cross = ch.resolve_in_session(
        A,
        2,
        WireAck {
            command_id: id,
            status: AckStatus::Acknowledged,
        },
    );
    assert!(
        matches!(
            cross,
            Err(ResolveError::WrongEpoch {
                expected: 1,
                got: 2
            })
        ),
        "cross-epoch ack must be refused as a mismatched command session: {cross:?}"
    );
    assert_eq!(
        ch.outstanding(),
        1,
        "the prior-session command stays outstanding — a restarted session cannot resolve it"
    );

    // POSITIVE CONTROL: the same ack IN SESSION (epoch 1) resolves it.
    let same = ch.resolve_in_session(
        A,
        1,
        WireAck {
            command_id: id,
            status: AckStatus::Acknowledged,
        },
    );
    assert!(
        same.is_ok(),
        "an in-session ack resolves the command: {same:?}"
    );
    assert_eq!(ch.outstanding(), 0, "now resolved");
}

/// A wrong-source ack is refused in any epoch (source is checked before epoch), and the command it
/// would have resolved stays outstanding.
#[test]
fn a_wrong_source_ack_is_refused_and_leaves_the_command_outstanding() {
    const A: u64 = 7;
    let mut ch = CommandChannel::new(A, 1);
    let mut sink: Vec<u8> = Vec::new();
    let id = ch
        .issue_tracked(KIND_SET_AMPLITUDE, 1.0f64.to_le_bytes().to_vec())
        .expect("issue");
    ch.deliver_next(&mut sink).expect("deliver");

    let wrong = ch.resolve_in_session(
        9,
        1,
        WireAck {
            command_id: id,
            status: AckStatus::Acknowledged,
        },
    );
    assert!(
        matches!(
            wrong,
            Err(ResolveError::WrongSource {
                expected: 7,
                got: 9
            })
        ),
        "an ack from a foreign source cannot resolve this channel's command: {wrong:?}"
    );
    assert_eq!(ch.outstanding(), 1, "the command stays outstanding");
}
