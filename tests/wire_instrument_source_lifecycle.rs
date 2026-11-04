//! Gate B3 (Renaissance XI) / A1 (Renaissance XII): the single-source contract keeps source
//! identity truthful.
//!
//! `interactive_loop()` binds to its CONFIGURED destination `args.source` (the source its
//! `CommandChannel` addresses) — not to whichever source speaks first (A1). The audit asked what
//! happens when a second source B publishes while A is the configured source, and when A then
//! retires while B stays live: does the UI keep showing a retired source, or silently switch to /
//! store B? The `wire_instrument` example is deliberately a SINGLE-source instrument, so the
//! truthful answer is a refusal — a message from any source other than the configured one is out of
//! contract: never admitted to the inbox, never displayed, never resolving a command, and counted.
//! A second source can therefore neither be silently stored-and-hidden nor silently switched to, and
//! a command is never redirected. When the configured source retires it is shown as retired; the
//! display is never reassigned to a live stranger.
//!
//! The classifier `classify_source` is the single identity authority shared with the loop; `drive`
//! mirrors the loop's drain-gating (classify against the configured destination, refuse the rest)
//! over a `FieldInbox`, so the observable inbox state is exactly what the loop would display. (The
//! configured-destination-vs-first-observed distinction is proven in
//! `tests/wire_instrument_source_binding.rs`.)

#[path = "../examples/wire_instrument/input.rs"]
mod input;

use gibson::field_view::FieldExtent;
use gibson::shape::Point;
use gibson::wire::{FieldInbox, SourceCursor, WireBody, WireField, WireMessage};
use input::{classify_source, source_in_contract, SourceVerdict};

fn field(seq_val: usize) -> WireField {
    let extent = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 2, 2).expect("extent");
    WireField {
        extent,
        model_tick: seq_val as u64,
        data: vec![seq_val as f64; 4],
        mask: None,
    }
}

fn field_msg(source: u64, seq: u64) -> WireMessage {
    WireMessage {
        source,
        cursor: SourceCursor::new(0, seq),
        body: WireBody::Field(field(seq as usize)),
    }
}

fn retire_msg(source: u64, seq: u64) -> WireMessage {
    WireMessage {
        source,
        cursor: SourceCursor::new(0, seq),
        body: WireBody::Retire,
    }
}

/// Drive the loop's drain-gating over a message sequence: classify every message against the
/// CONFIGURED destination `dest` (session epoch 0 here), refuse every foreign one (Gate B3 / A1, via
/// the shared `classify_source`). Returns the displayed source (`Some(dest)` once a valid
/// observation arrives) and the out-of-contract count; the `FieldInbox` ends in exactly the state
/// the loop would display.
fn drive(dest: u64, msgs: Vec<WireMessage>, inbox: &mut FieldInbox) -> (Option<u64>, u64) {
    let mut visible: Option<u64> = None;
    let mut foreign = 0u64;
    let mut observed_epoch: Option<u32> = None;
    for msg in msgs {
        match classify_source(dest, observed_epoch, msg.source, msg.cursor.epoch) {
            SourceVerdict::Foreign => {
                foreign += 1;
                continue;
            }
            SourceVerdict::Restart | SourceVerdict::InSession => {}
        }
        observed_epoch = Some(observed_epoch.map_or(msg.cursor.epoch, |e| e.max(msg.cursor.epoch)));
        match msg.body {
            WireBody::Field(f) => {
                inbox.admit(msg.source, msg.cursor, f);
                visible = Some(dest);
            }
            WireBody::Retire => {
                inbox.retire(msg.source);
            }
            _ => {}
        }
    }
    (visible, foreign)
}

/// The predicate: only the bound source is in contract.
#[test]
fn only_the_bound_source_is_in_contract() {
    assert!(source_in_contract(1, 1));
    assert!(!source_in_contract(1, 2));
    assert!(!source_in_contract(2, 1));
}

/// The audit's scenario: A binds, B publishes, A retires, B continues. B is refused throughout, and
/// after A retires the display shows A retired — never B, never a stale misidentification.
#[test]
fn a_second_source_is_refused_and_the_bound_source_retirement_is_truthful() {
    const A: u64 = 1;
    const B: u64 = 2;
    let mut inbox = FieldInbox::new();

    let (src, foreign) = drive(
        A, // the CONFIGURED destination
        vec![
            field_msg(A, 1),  // A publishes -> the configured source, shown
            field_msg(B, 1),  // B publishes -> out of contract, refused
            retire_msg(A, 2), // A retires
            field_msg(B, 2),  // B keeps publishing -> still refused
        ],
        &mut inbox,
    );

    assert_eq!(
        src,
        Some(A),
        "the instrument binds to the configured source and never switches to a stranger"
    );
    assert_eq!(
        foreign, 2,
        "both of B's messages are refused as out of contract"
    );
    assert!(
        inbox.latest(B).is_none(),
        "a foreign source is never admitted, so it can never be displayed"
    );
    assert!(
        inbox.latest(A).is_none(),
        "after retiring, the bound source has no live field — shown as retired"
    );
    assert_eq!(
        inbox.live_sources(),
        0,
        "no live source remains — the display shows A retired, not a silent switch to live B"
    );
}

/// POSITIVE CONTROL: a conformant single-source stream is fully admitted, nothing refused — so the
/// refusal above is specific to the foreign source, not a blanket drop.
#[test]
fn a_conformant_single_source_stream_is_fully_admitted() {
    const A: u64 = 1;
    let mut inbox = FieldInbox::new();

    let (src, foreign) = drive(
        A,
        vec![field_msg(A, 1), field_msg(A, 2), field_msg(A, 3)],
        &mut inbox,
    );

    assert_eq!(src, Some(A));
    assert_eq!(
        foreign, 0,
        "a conformant single-source stream has no out-of-contract messages"
    );
    assert!(
        inbox.latest(A).is_some(),
        "the bound source's field is live and displayable"
    );
    assert_eq!(inbox.live_sources(), 1);
}
