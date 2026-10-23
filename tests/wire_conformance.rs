//! Cross-language wire conformance (Wave 5).
//!
//! The golden vector `bindings/python/testdata/golden_field_v1.bin` is produced by the
//! **Python** reference producer (`python3 bindings/python/producer.py --golden`). This
//! test proves the Rust transport agrees with it two ways:
//!
//! 1. The Rust `FrameDecoder` decodes the Python-emitted bytes into exactly the expected
//!    messages — so the Rust *reader* understands the Python *writer*.
//! 2. The Rust encoders reproduce the golden bytes **byte-for-byte** — so the Rust *writer*
//!    and the Python *writer* emit the identical wire representation.
//!
//! Together these pin both halves of the contract across the language boundary with a
//! committed artifact, no flaky subprocess. The vector is finite + ±∞ only (deterministic
//! on every platform); NaN bit-preservation is covered per-language in the round-trip tests.

use gibson::field_view::FieldExtent;
use gibson::shape::Point;
use gibson::wire::{
    encode_ack, encode_command, encode_field, encode_retire, AckStatus, FrameDecoder, SourceCursor,
    WireAck, WireBody, WireCommand, WireField, WireLimits, WireMessage,
};

fn golden() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/bindings/python/testdata/golden_field_v1.bin"
    );
    std::fs::read(path).unwrap_or_else(|e| panic!("cannot read golden vector {path}: {e}"))
}

fn f1() -> WireField {
    WireField {
        extent: FieldExtent::new(Point::new(-1.0, 2.0), 0.5, 0.25, 3, 2).unwrap(),
        model_tick: 100,
        data: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        mask: None,
    }
}

fn f2() -> WireField {
    WireField {
        extent: FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 2, 2).unwrap(),
        model_tick: 101,
        data: vec![42.0, f64::INFINITY, f64::NEG_INFINITY, 7.5],
        mask: Some(vec![true, false, true, true]),
    }
}

fn f3() -> WireCommand {
    WireCommand {
        command_id: 7,
        kind: 2,
        arg: b"hi".to_vec(),
    }
}

fn f4() -> WireAck {
    WireAck {
        command_id: 7,
        status: AckStatus::Acknowledged,
    }
}

fn decode_all(bytes: &[u8]) -> Vec<WireMessage> {
    let mut dec = FrameDecoder::new();
    dec.push(bytes).unwrap();
    let mut out = Vec::new();
    while let Some(res) = dec.next_message() {
        out.push(res.expect("golden vector must decode cleanly"));
    }
    dec.finish()
        .expect("golden vector must end on a frame boundary");
    out
}

#[test]
fn rust_decodes_the_python_golden_vector() {
    let msgs = decode_all(&golden());
    assert_eq!(msgs.len(), 5, "golden vector has five frames");

    // F1 — plain gradient field.
    assert_eq!(msgs[0].source, 1);
    assert_eq!(msgs[0].cursor, SourceCursor::new(1, 1));
    assert_eq!(msgs[0].body, WireBody::Field(f1()));

    // F2 — masked field carrying ±∞ (deterministic bits, so == is valid: no NaN).
    assert_eq!(msgs[1].cursor, SourceCursor::new(1, 2));
    assert_eq!(msgs[1].body, WireBody::Field(f2()));

    // F3 — command on the return channel.
    assert_eq!(msgs[2].cursor, SourceCursor::new(0, 1));
    assert_eq!(msgs[2].body, WireBody::Command(f3()));

    // F4 — ack.
    assert_eq!(msgs[3].cursor, SourceCursor::new(0, 2));
    assert_eq!(msgs[3].body, WireBody::Ack(f4()));

    // F5 — retire.
    assert_eq!(msgs[4].cursor, SourceCursor::new(1, 3));
    assert_eq!(msgs[4].body, WireBody::Retire);
}

#[test]
fn rust_encoder_reproduces_the_python_golden_bytes() {
    let limits = WireLimits::default();
    let mut rebuilt = Vec::new();
    rebuilt.extend(encode_field(1, SourceCursor::new(1, 1), &f1(), limits).unwrap());
    rebuilt.extend(encode_field(1, SourceCursor::new(1, 2), &f2(), limits).unwrap());
    rebuilt.extend(encode_command(1, SourceCursor::new(0, 1), &f3()));
    rebuilt.extend(encode_ack(1, SourceCursor::new(0, 2), f4()));
    rebuilt.extend(encode_retire(1, SourceCursor::new(1, 3)));

    let golden = golden();
    assert_eq!(
        rebuilt, golden,
        "Rust encoder bytes diverge from the Python-produced golden vector"
    );
}
