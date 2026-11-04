//! Gate B3 (Renaissance XI): the single-source contract keeps source identity truthful.
//!
//! `interactive_loop()` binds to the FIRST source it observes (`src.get_or_insert`) and its
//! `CommandChannel` targets that same source. The audit asked what happens when a second source B
//! publishes while A is bound, and when A then retires while B stays live: does the UI keep showing
//! a retired source, or silently switch to / store B? The `wire_instrument` example is deliberately
//! a SINGLE-source instrument, so the truthful answer is a refusal — a message from any source
//! other than the bound one is out of contract: never admitted to the inbox, never displayed, never
//! resolving a command, and counted. A second source can therefore neither be silently
//! stored-and-hidden nor silently switched to, and a command is never redirected. When the bound
//! source retires it is shown as retired; the display is never reassigned to a live stranger.
//!
//! The contract predicate `source_in_contract` is shared with the loop; `drive` mirrors the loop's
//! drain-gating (bind the first source, refuse the rest) over a `FieldInbox`, so the observable
//! inbox state is exactly what the loop would display.

#[path = "../examples/wire_instrument/input.rs"]
mod input;

use gibson::field_view::FieldExtent;
use gibson::shape::Point;
use gibson::wire::{FieldInbox, SourceCursor, WireBody, WireField, WireMessage};
use input::source_in_contract;

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

/// Drive the loop's drain-gating over a message sequence: bind to the first source, refuse every
/// other one (Gate B3, via the shared `source_in_contract`). Returns the bound/displayed source and
/// the out-of-contract count; the `FieldInbox` ends in exactly the state the loop would display.
fn drive(msgs: Vec<WireMessage>, inbox: &mut FieldInbox) -> (Option<u64>, u64) {
    let mut src: Option<u64> = None;
    let mut foreign = 0u64;
    for msg in msgs {
        let bound = *src.get_or_insert(msg.source);
        if !source_in_contract(bound, msg.source) {
            foreign += 1;
            continue;
        }
        match msg.body {
            WireBody::Field(f) => {
                inbox.admit(msg.source, msg.cursor, f);
            }
            WireBody::Retire => {
                inbox.retire(msg.source);
            }
            _ => {}
        }
    }
    (src, foreign)
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
        vec![
            field_msg(A, 1),  // A publishes -> the instrument binds to A
            field_msg(B, 1),  // B publishes -> out of contract, refused
            retire_msg(A, 2), // A retires
            field_msg(B, 2),  // B keeps publishing -> still refused
        ],
        &mut inbox,
    );

    assert_eq!(
        src,
        Some(A),
        "the instrument binds to the first source and never switches to a stranger"
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
