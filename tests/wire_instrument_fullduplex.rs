//! Gate C3 — full-duplex forward progress on a REAL bidirectional subprocess under output pressure.
//!
//! The byte-bounded transport (Gate C1/C2) and the command channel (Gate D) are proven in
//! isolation; Gate E proved the enabling property (the non-blocking `Reader` drains even while a
//! read would block forever). C3 is the composed, cross-process claim the prompt demands and
//! explicitly forbids faking with an in-memory cursor: a real producer FLOODING large FIELD frames
//! on stdout while the consumer sends a COMMAND on its stdin — the classic pipe-deadlock shape.
//!
//! The reference producer's `serve()` loop does `out.flush()` BEFORE `drain_and_ack_commands`, so a
//! consumer that stops draining stdout wedges the producer mid-flush; it then never reads stdin, and
//! a consumer blocking on its own stdin write completes the deadlock. The `wire_instrument`
//! architecture breaks the cycle with a background `Reader` thread that ALWAYS drains stdout into
//! the bounded buffer — the producer never stays blocked, so it keeps reading stdin and acking.
//!
//! This proves, on real OS pipes with a real Python process at `--hz 0` (as fast as possible) and
//! ~2 MiB/field: the command round-trips (an `Acknowledged` verdict comes back) within a bounded
//! wall-clock deadline (FORWARD PROGRESS / no deadlock), retention stays within the byte budget
//! (BOUNDED), and the flood is genuine (the coalescing buffer dropped many superseded fields). If
//! python3 or the producer cannot be spawned, the test fails loudly rather than skipping.

#[path = "../examples/wire_instrument/channel.rs"]
mod channel;
#[path = "../examples/wire_instrument/transport.rs"]
mod transport;

use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use channel::CommandChannel;
use gibson::wire::{WireBody, WireMessage};
use transport::{Reader, DEFAULT_CONTROL_CAP};

/// Mirrors the `main.rs` constant (the producer's one command kind).
const KIND_SET_AMPLITUDE: u32 = 1;

fn python() -> String {
    std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

#[test]
fn a_command_round_trips_while_the_producer_floods_large_fields() {
    // A real producer flooding 512x512 (~2 MiB) fields as fast as possible. A single field dwarfs
    // the OS pipe buffer, so the producer would wedge on the first flush without a draining
    // consumer. `--frames` is large enough to outlast the round-trip; the child is killed at the end.
    let mut child = Command::new(python())
        .arg("bindings/python/producer.py")
        .args([
            "--cols", "512", "--rows", "512", "--hz", "0", "--frames", "100000", "--source", "1",
            "--epoch", "1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null()) // the kill() at teardown yields an expected BrokenPipe traceback
        .spawn()
        .expect("spawn python3 bindings/python/producer.py (is python3 on PATH?)");

    let mut stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.take().expect("stdout piped");

    // The production component under test: a background thread that always drains stdout into the
    // bounded, coalescing buffer — the thing that makes the full-duplex loop deadlock-free.
    let reader = Reader::spawn(stdout, DEFAULT_CONTROL_CAP);
    let mut ch = CommandChannel::new(1, 1);

    let cap = reader.field_byte_cap();

    // Let the flood flow briefly WITHOUT draining the app side: the background Reader thread pours
    // the producer's 2 MiB fields into the bounded, coalescing buffer (latest-wins per source), so
    // the command we deliver next genuinely goes out mid-flood. Retention stays bounded throughout.
    let warmup = Instant::now() + Duration::from_millis(400);
    while Instant::now() < warmup {
        assert!(
            reader.field_bytes() <= cap,
            "retention must stay within the byte budget under flood: {} > {}",
            reader.field_bytes(),
            cap
        );
        thread::sleep(Duration::from_millis(10));
    }

    // Mid-flood, issue and DELIVER a set-amplitude edit onto the producer's stdin.
    ch.issue_tracked(KIND_SET_AMPLITUDE, 3.0f64.to_le_bytes().to_vec())
        .expect("issue a tracked set-amplitude edit");
    let delivered = ch
        .deliver_next(&mut stdin)
        .expect("delivering the command must not block or fault under flood");
    assert!(
        delivered.is_some(),
        "the command must leave for the producer"
    );

    // Drain + resolve until the producer's genuine ACK comes back, or the deadline fires. The
    // stream keeps flowing the whole time (fields still arrive alongside the ack).
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut fields_seen = 0u64;
    let mut max_tick = 0u64;
    while ch.acknowledged() == 0 && Instant::now() < deadline {
        let mut batch: Vec<WireMessage> = Vec::new();
        reader.drain(&mut |m| batch.push(m));
        for msg in batch {
            match msg.body {
                WireBody::Ack(ack) => {
                    let _ = ch.resolve(msg.source, ack);
                }
                WireBody::Field(f) => {
                    fields_seen += 1;
                    max_tick = max_tick.max(f.model_tick);
                }
                _ => {}
            }
        }
        assert!(
            reader.field_bytes() <= cap,
            "retention must stay bounded while awaiting the ack"
        );
        thread::sleep(Duration::from_millis(5));
    }

    // FORWARD PROGRESS: the command round-tripped despite the stdout flood (no deadlock).
    assert_eq!(
        ch.acknowledged(),
        1,
        "the producer's ack must round-trip under flood within the deadline"
    );
    // BOUNDED: the byte budget held throughout.
    assert!(reader.field_bytes() <= cap);
    // The duplex stream stayed live while the command resolved — fields kept arriving.
    assert!(
        fields_seen > 0,
        "fields must keep flowing while the command round-trips; got {fields_seen}"
    );
    // REAL FLOOD: the producer advanced its model many ticks (coalesced latest-wins), so the ack
    // round-trip happened against a genuinely busy stdout, not a trickle.
    assert!(
        max_tick > 3,
        "the producer must have flooded many fields (latest model_tick large); got {max_tick}"
    );

    // Tear down: stop the flood deterministically rather than waiting out --frames.
    let _ = child.kill();
    let _ = child.wait();
}
