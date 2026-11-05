//! RXIV Gate B — the live film's command provenance and process lifecycle, on the SAME
//! presentation-side transport the scientific instrument certified.
//!
//! B1 executed the RXIII `Link` against nine hostile streams (recorded RED: wrong-command-id ACK,
//! wrong-source ACK, duplicate ACK and stale-epoch FIELD silently ACCEPTED; a foreign FIELD
//! relabelled as source 1; an early ACK, RETIRE, malformed frame and restart each an untyped
//! PANIC). This battery pins the repaired behavior of `LiveSession`, where every one of those is a
//! typed outcome and none can resolve, stage, or arm anything it should not.
//!
//! The protocol cases run over an in-memory LOCKSTEP RESPONDER — a scripted hostile producer that
//! emits its opening frames, then exactly one scripted chunk per command frame it decodes — so
//! each case is deterministic (no reader coalescing races, no sleeps gating the proof). The
//! lifecycle cases (B3) run real processes and measure reaping through the OS (`/proc/<pid>`),
//! never through an escape-stream heuristic.
#![cfg(unix)]

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

use std::collections::VecDeque;
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel as mpsc_channel, Receiver, Sender};
use std::sync::{Arc as SyncArc, Mutex};
use std::time::{Duration, Instant};

use channel::ResolveError;
use gibson::field_view::FieldExtent;
use gibson::wire::{
    encode_ack, encode_field, encode_retire, AckStatus, FrameDecoder, LedgerReject, SourceCursor,
    WireAck, WireBody, WireError, WireField, WireLimits,
};
use gibson::Point;
use live::{Arc, Beat, Verdict};
use live_link::{
    arc_summary, stage_live, verdict_of, LiveError, LiveSession, ProducerSpec, LIVE_SOURCE,
    SHUTDOWN_GRACE,
};

// ---------------------------------------------------------------------------------------------
// The in-memory lockstep responder
// ---------------------------------------------------------------------------------------------

fn field(source: u64, epoch: u32, seq: u64, scale: f64) -> Vec<u8> {
    let extent = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 4, 3).unwrap();
    let f = WireField {
        extent,
        model_tick: seq,
        data: (0..12).map(|i| scale * (1.0 + i as f64) / 12.0).collect(),
        mask: None,
    };
    encode_field(
        source,
        SourceCursor::new(epoch, seq),
        &f,
        WireLimits::default(),
    )
    .unwrap()
}

fn ack(source: u64, seq: u64, id: u64, status: AckStatus) -> Vec<u8> {
    encode_ack(
        source,
        SourceCursor::new(1, seq),
        WireAck {
            command_id: id,
            status,
        },
    )
}

fn acked(id: u64, seq: u64) -> Vec<u8> {
    ack(LIVE_SOURCE, seq, id, AckStatus::Acknowledged)
}

/// A well-formed frame with an UNKNOWN message type: recoverable — the decoder skips exactly it.
fn unknown_type_frame() -> Vec<u8> {
    let mut f = encode_retire(LIVE_SOURCE, SourceCursor::new(1, 50));
    f[6] = 0xEE;
    f[7] = 0x00;
    f
}

/// The command frames a scripted producer decoded: (command_id, kind).
type Received = SyncArc<Mutex<Vec<(u64, u32)>>>;

struct ChanReader {
    rx: Receiver<Vec<u8>>,
    buf: Vec<u8>,
    pos: usize,
}

impl Read for ChanReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        while self.pos == self.buf.len() {
            match self.rx.recv() {
                Ok(b) => {
                    self.buf = b;
                    self.pos = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

#[derive(Clone, Copy, Default)]
struct Faults {
    write_fails: bool,
    flush_fails: bool,
    /// Close the producer's stream (EOF) once the script runs out.
    close_after_script: bool,
}

struct Responder {
    tx: Option<Sender<Vec<u8>>>,
    script: VecDeque<Vec<u8>>,
    dec: FrameDecoder,
    faults: Faults,
    /// Every command frame the producer actually decoded: (command_id, kind).
    received: Received,
}

impl Write for Responder {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.faults.write_fails {
            return Err(io::Error::new(
                ErrorKind::BrokenPipe,
                "scripted write failure",
            ));
        }
        self.dec.push(b).expect("presenter frames decode");
        while let Some(m) = self.dec.next_message() {
            let m = m.expect("presenter frames are well-formed");
            if let WireBody::Command(c) = m.body {
                self.received.lock().unwrap().push((c.command_id, c.kind));
            }
            match self.script.pop_front() {
                Some(chunk) => {
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(chunk);
                    }
                }
                None if self.faults.close_after_script => self.tx = None,
                None => {}
            }
        }
        Ok(b.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.faults.flush_fails {
            Err(io::Error::other("scripted flush failure"))
        } else {
            Ok(())
        }
    }
}

/// A session against a scripted hostile producer. Returns the session and the record of command
/// frames the producer decoded.
fn hostile(opening: Vec<u8>, script: Vec<Vec<u8>>, faults: Faults) -> (LiveSession, Received) {
    let (tx, rx) = mpsc_channel();
    if !opening.is_empty() {
        tx.send(opening).unwrap();
    }
    let received = SyncArc::new(Mutex::new(Vec::new()));
    let responder = Responder {
        tx: Some(tx),
        script: script.into(),
        dec: FrameDecoder::new(),
        faults,
        received: SyncArc::clone(&received),
    };
    let reader = ChanReader {
        rx,
        buf: Vec::new(),
        pos: 0,
    };
    let s = LiveSession::over_pipes(reader, responder, LIVE_SOURCE)
        .with_deadline(Duration::from_millis(300));
    (s, received)
}

fn cat(parts: &[Vec<u8>]) -> Vec<u8> {
    parts.concat()
}

// ---------------------------------------------------------------------------------------------
// B1/B2 — the provenance battery (each was accepted or crashed on the RXIII link)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_wrong_command_id_ack_is_a_protocol_violation_never_a_verdict() {
    let (mut s, _) = hostile(field(1, 1, 1, 1.0), vec![acked(999, 2)], Faults::default());
    s.next_field().unwrap();
    let id = s.submit_amplitude(2.0).unwrap();
    assert_eq!(id, 1);
    assert_eq!(
        s.await_verdict(id),
        Err(LiveError::Protocol(ResolveError::Ledger(
            LedgerReject::UnknownCommand { command_id: 999 }
        )))
    );
    assert_eq!(s.channel().acknowledged(), 0, "nothing was resolved");
    assert_eq!(
        s.channel().outstanding(),
        1,
        "command 1 stays honestly unresolved"
    );
    // Fail-closed: every later call reports the same violation.
    assert!(matches!(s.next_field(), Err(LiveError::Protocol(_))));
}

#[test]
fn a_wrong_source_ack_is_counted_foreign_and_never_resolves() {
    let (mut s, _) = hostile(
        field(1, 1, 1, 1.0),
        vec![ack(7, 2, 1, AckStatus::Acknowledged)],
        Faults::default(),
    );
    s.next_field().unwrap();
    let id = s.submit_amplitude(2.0).unwrap();
    assert!(matches!(
        s.await_verdict(id),
        Err(LiveError::Timeout { .. })
    ));
    assert_eq!(s.counters().foreign, 1);
    assert_eq!(s.channel().acknowledged(), 0);
}

#[test]
fn a_wrong_identity_verdict_never_arms_the_transformation() {
    // The film's own sequence against a producer that acks the WRONG command and then publishes a
    // field at exactly the requested value: the arc must never leave INTERVENE.
    let (mut s, _) = hostile(
        field(1, 1, 1, 1.0),
        vec![
            field(1, 1, 2, 1.0),
            cat(&[acked(999, 3), field(1, 1, 3, 2.0)]),
        ],
        Faults::default(),
    );
    let mut arc = Arc::new();
    let f = s.next_field().unwrap();
    let (o, _, _) = stage_live(&f).unwrap();
    arc.observe(arc_summary(&o, f.cursor));
    arc.discover();
    let f = s.step().unwrap();
    let (o, _, _) = stage_live(&f).unwrap();
    arc.observe(arc_summary(&o, f.cursor));
    arc.intervene(2.0);
    let id = s.submit_amplitude(2.0).unwrap();
    match s.await_verdict(id) {
        Ok(st) => arc.verdict(verdict_of(st)),
        Err(e) => assert!(matches!(e, LiveError::Protocol(_)), "{e}"),
    }
    assert_eq!(
        arc.beat(),
        Beat::Intervene,
        "no correlated verdict, no AWAIT"
    );
    assert!(
        s.next_field().is_err(),
        "the session failed closed; the doubled field is never staged"
    );
    assert!(!arc.transformed());
}

#[test]
fn a_stale_epoch_field_is_never_the_next_observation() {
    let (mut s, _) = hostile(
        field(1, 2, 1, 1.0),
        vec![field(1, 1, 9, 1.0)],
        Faults::default(),
    );
    assert_eq!(s.next_field().unwrap().cursor, SourceCursor::new(2, 1));
    assert!(matches!(s.step(), Err(LiveError::Timeout { .. })));
    assert_eq!(s.counters().stale, 1);
}

#[test]
fn a_repeated_cursor_is_stale_not_new() {
    let (mut s, _) = hostile(
        field(1, 1, 5, 1.0),
        vec![field(1, 1, 5, 2.0)],
        Faults::default(),
    );
    s.next_field().unwrap();
    assert!(matches!(s.step(), Err(LiveError::Timeout { .. })));
    assert_eq!(s.counters().stale, 1);
}

#[test]
fn a_duplicate_ack_cannot_resolve_the_next_command() {
    let (mut s, _) = hostile(
        field(1, 1, 1, 1.0),
        vec![acked(1, 2), acked(1, 3)],
        Faults::default(),
    );
    s.next_field().unwrap();
    let a = s.submit_amplitude(2.0).unwrap();
    assert_eq!(s.await_verdict(a), Ok(AckStatus::Acknowledged));
    let b = s.submit_amplitude(3.0).unwrap();
    assert_eq!(
        s.await_verdict(b),
        Err(LiveError::Protocol(ResolveError::Ledger(
            LedgerReject::UnknownCommand { command_id: 1 }
        )))
    );
    assert_eq!(s.channel().acknowledged(), 1);
}

#[test]
fn an_ack_before_its_command_exists_is_a_typed_violation() {
    // The producer volunteers a verdict for command 1 before command 1 was ever issued.
    let (mut s, received) = hostile(
        cat(&[field(1, 1, 1, 1.0), acked(1, 2)]),
        vec![],
        Faults::default(),
    );
    assert_eq!(
        s.next_field().map(|f| f.cursor),
        Err(LiveError::Protocol(ResolveError::Ledger(
            LedgerReject::UnknownCommand { command_id: 1 }
        )))
    );
    // And the pre-sent ACK can never become the verdict of a later command 1: the session refuses
    // to issue on a violated stream (nothing is written to the producer).
    assert!(matches!(
        s.submit_amplitude(2.0),
        Err(LiveError::Protocol(_))
    ));
    assert!(received.lock().unwrap().is_empty());
}

#[test]
fn a_foreign_field_is_counted_and_never_staged() {
    let (mut s, _) = hostile(
        cat(&[field(7, 1, 1, 9.0), field(1, 1, 1, 1.0)]),
        vec![],
        Faults::default(),
    );
    let f = s.next_field().unwrap();
    assert_eq!(f.source, 1);
    let (obs, _, _) = stage_live(&f).unwrap();
    assert_eq!(obs.source.0, 1, "staged under its ACTUAL wire identity");
    assert_eq!(s.counters().foreign, 1);
}

#[test]
fn a_field_is_staged_under_its_actual_identity_not_a_fixed_label() {
    // A session configured for source 3 at producer epoch 5 stages exactly that identity.
    let (tx, rx) = mpsc_channel();
    tx.send(field(3, 5, 1, 1.0)).unwrap();
    let reader = ChanReader {
        rx,
        buf: Vec::new(),
        pos: 0,
    };
    let mut s = LiveSession::over_pipes(reader, io::sink(), 3);
    let f = s.next_field().unwrap();
    let (obs, _, _) = stage_live(&f).unwrap();
    assert_eq!((obs.source.0, obs.epoch), (3, 5));
    drop(tx);
}

#[test]
fn retire_before_the_expected_field_is_a_typed_end() {
    let (mut s, _) = hostile(
        field(1, 1, 1, 1.0),
        vec![encode_retire(1, SourceCursor::new(1, 2))],
        Faults::default(),
    );
    s.next_field().unwrap();
    assert_eq!(
        s.step().map(|f| f.cursor),
        Err(LiveError::Retired { unresolved: 0 })
    );
}

#[test]
fn frames_before_a_retire_are_still_delivered_in_order() {
    let (mut s, _) = hostile(
        cat(&[
            field(1, 1, 1, 1.0),
            encode_retire(1, SourceCursor::new(1, 2)),
        ]),
        vec![],
        Faults::default(),
    );
    assert_eq!(s.next_field().unwrap().cursor, SourceCursor::new(1, 1));
    assert!(matches!(s.next_field(), Err(LiveError::Retired { .. })));
}

#[test]
fn a_fatal_malformed_frame_is_a_typed_fatal() {
    let (mut s, _) = hostile(field(1, 1, 1, 1.0), vec![vec![0xFF; 64]], Faults::default());
    s.next_field().unwrap();
    match s.step() {
        Err(LiveError::Fatal {
            error: WireError::BadMagic { .. },
            ..
        }) => {}
        other => panic!(
            "expected a typed fatal BadMagic, got {:?}",
            other.map(|f| f.cursor)
        ),
    }
}

#[test]
fn a_recoverable_malformed_frame_is_counted_and_the_stream_continues() {
    let (mut s, _) = hostile(
        field(1, 1, 1, 1.0),
        vec![cat(&[unknown_type_frame(), field(1, 1, 2, 1.0)])],
        Faults::default(),
    );
    s.next_field().unwrap();
    assert_eq!(s.step().unwrap().cursor, SourceCursor::new(1, 2));
    assert_eq!(
        s.counters().decode_skips,
        1,
        "the skipped frame is a receipt"
    );
}

#[test]
fn a_full_frame_write_with_a_failed_flush_is_uncertain_and_reconcilable() {
    let (mut s, received) = hostile(
        field(1, 1, 1, 1.0),
        vec![acked(1, 2)],
        Faults {
            flush_fails: true,
            ..Faults::default()
        },
    );
    s.next_field().unwrap();
    assert_eq!(
        s.submit_amplitude(2.0),
        Err(LiveError::Transport {
            command_id: Some(1),
            uncertain: true,
            kind: ErrorKind::Other
        })
    );
    // The producer DID receive the whole frame and genuinely acked it: the uncertain command is
    // reconciled, never retried.
    assert_eq!(s.await_verdict(1), Ok(AckStatus::Acknowledged));
    assert_eq!(s.channel().transport_uncertain(), 1);
    assert!(s.channel().conserves());
    // The channel faulted: no further command is issued or written.
    assert!(matches!(
        s.submit_amplitude(3.0),
        Err(LiveError::Transport { .. })
    ));
    assert_eq!(s.channel().issued(), 1);
    assert_eq!(
        received.lock().unwrap().len(),
        1,
        "exactly one frame, never retried"
    );
}

#[test]
fn a_failed_write_is_known_undelivered_and_fails_closed() {
    let (mut s, received) = hostile(
        field(1, 1, 1, 1.0),
        vec![],
        Faults {
            write_fails: true,
            ..Faults::default()
        },
    );
    s.next_field().unwrap();
    let e = s.submit_amplitude(2.0).unwrap_err();
    assert_eq!(
        e,
        LiveError::Transport {
            command_id: Some(1),
            uncertain: false,
            kind: ErrorKind::BrokenPipe
        }
    );
    assert_eq!(s.next_field().map(|f| f.cursor), Err(e));
    assert!(received.lock().unwrap().is_empty());
}

#[test]
fn a_restart_with_an_unresolved_command_is_typed_and_never_retried() {
    let (mut s, received) = hostile(
        field(1, 1, 1, 1.0),
        vec![field(1, 2, 1, 1.0)], // restarted producer instead of the verdict
        Faults::default(),
    );
    s.next_field().unwrap();
    let id = s.submit_amplitude(2.0).unwrap();
    assert_eq!(
        s.await_verdict(id),
        Err(LiveError::Restarted {
            from: 1,
            to: 2,
            unresolved: 1
        })
    );
    assert_eq!(received.lock().unwrap().as_slice(), &[(1, 1)]);
}

#[test]
fn a_restart_with_nothing_in_flight_is_adopted_as_a_display_signal() {
    let (mut s, _) = hostile(
        field(1, 1, 1, 1.0),
        vec![field(1, 2, 1, 1.0)],
        Faults::default(),
    );
    s.next_field().unwrap();
    assert_eq!(s.step().unwrap().cursor, SourceCursor::new(2, 1));
    assert_eq!(s.counters().restarts, 1);
}

#[test]
fn a_clean_close_after_the_script_is_a_typed_disconnect() {
    let (mut s, _) = hostile(
        field(1, 1, 1, 1.0),
        vec![],
        Faults {
            close_after_script: true,
            ..Faults::default()
        },
    );
    s.next_field().unwrap();
    assert!(matches!(
        s.step(),
        Err(LiveError::Disconnected {
            read_error: None,
            truncated_bytes: 0,
            ..
        })
    ));
}

// ---------------------------------------------------------------------------------------------
// B3 — real-process lifetime: every failure path reaps the producer, observed through the OS
// ---------------------------------------------------------------------------------------------

fn alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// The OS no longer has the process (reaped; not a zombie), within a bounded deadline.
fn assert_reaped(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while alive(pid) {
        assert!(
            Instant::now() < deadline,
            "producer pid {pid} was not reaped"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn scratch(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rxiv-b3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn a_silent_producer_times_out_and_is_reaped_on_drop() {
    let s = LiveSession::spawn(&ProducerSpec::custom("sh", &["-c", "exec sleep 30"]), 1)
        .unwrap()
        .with_deadline(Duration::from_millis(200));
    let mut s = s;
    let pid = s.producer_pid().unwrap();
    let t0 = Instant::now();
    assert!(matches!(s.next_field(), Err(LiveError::Timeout { .. })));
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "the wait is wall-clock bounded"
    );
    assert!(alive(pid));
    drop(s);
    assert_reaped(pid);
}

#[test]
fn a_truncated_frame_is_reported_as_truncated_not_clean() {
    let whole = field(1, 1, 1, 1.0);
    let path = scratch("truncated.bin", &whole[..20]);
    let p = path.to_str().unwrap();
    let mut s =
        LiveSession::spawn(&ProducerSpec::custom("sh", &["-c", "cat \"$0\"", p]), 1).unwrap();
    let pid = s.producer_pid().unwrap();
    match s.next_field() {
        Err(LiveError::Disconnected {
            read_error: None,
            truncated_bytes: 20,
            exit,
            ..
        }) => assert!(exit.map(|e| e.success()).unwrap_or(true)),
        other => panic!(
            "expected a truncated disconnect, got {:?}",
            other.map(|f| f.cursor)
        ),
    }
    drop(s);
    assert_reaped(pid);
}

#[test]
fn a_producer_that_exits_reports_its_status_and_stderr_and_a_write_breaks_cleanly() {
    let mut s = LiveSession::spawn(
        &ProducerSpec::custom("sh", &["-c", "echo boom-from-producer >&2; exit 3"]),
        1,
    )
    .unwrap();
    let pid = s.producer_pid().unwrap();
    match s.next_field() {
        Err(LiveError::Disconnected { exit, stderr, .. }) => {
            assert_eq!(exit.and_then(|e| e.code()), Some(3));
            assert!(
                stderr.contains("boom-from-producer"),
                "stderr kept: {stderr:?}"
            );
        }
        other => panic!("expected a disconnect, got {:?}", other.map(|f| f.cursor)),
    }
    // Writing to the dead producer is a typed, known-undelivered transport failure (EPIPE).
    assert!(matches!(
        s.submit_amplitude(2.0),
        Err(LiveError::Transport {
            uncertain: false,
            kind: ErrorKind::BrokenPipe,
            ..
        })
    ));
    drop(s);
    assert_reaped(pid);
}

#[test]
fn a_crashing_python_producer_surfaces_its_traceback() {
    let path = scratch("one-field.bin", &field(1, 1, 1, 1.0));
    let code = "import sys\nsys.stdout.buffer.write(open(sys.argv[1],'rb').read())\nsys.stdout.flush()\nraise RuntimeError('producer exploded mid-session')";
    let py = live_link::python();
    let mut s = LiveSession::spawn(
        &ProducerSpec::custom(&py, &["-c", code, path.to_str().unwrap()]),
        1,
    )
    .unwrap();
    let pid = s.producer_pid().unwrap();
    assert_eq!(s.next_field().unwrap().cursor, SourceCursor::new(1, 1));
    match s.next_field() {
        Err(LiveError::Disconnected { exit, stderr, .. }) => {
            assert_eq!(exit.and_then(|e| e.code()), Some(1));
            assert!(stderr.contains("RuntimeError: producer exploded mid-session"));
            assert!(stderr.contains("Traceback"));
        }
        other => panic!(
            "expected a crash disconnect, got {:?}",
            other.map(|f| f.cursor)
        ),
    }
    drop(s);
    assert_reaped(pid);
}

#[test]
fn a_panic_while_holding_the_session_still_reaps_the_producer() {
    let pid = SyncArc::new(Mutex::new(0u32));
    let p2 = SyncArc::clone(&pid);
    let r = std::panic::catch_unwind(move || {
        let s =
            LiveSession::spawn(&ProducerSpec::custom("sh", &["-c", "exec sleep 30"]), 1).unwrap();
        *p2.lock().unwrap() = s.producer_pid().unwrap();
        panic!("the film panicked mid-session");
    });
    assert!(r.is_err());
    assert_reaped(*pid.lock().unwrap());
}

#[test]
fn finish_is_graceful_for_a_conformant_producer_and_bounded_for_a_stubborn_one() {
    // The reference producer exits on stdin EOF by itself.
    let mut s = LiveSession::spawn(&ProducerSpec::reference(false), LIVE_SOURCE).unwrap();
    let pid = s.producer_pid().unwrap();
    s.next_field().unwrap();
    let end = s.finish(SHUTDOWN_GRACE);
    assert!(end.graceful, "the reference producer exits on EOF");
    assert!(end.exit.unwrap().success());
    assert_reaped(pid);

    // A producer that ignores EOF is killed after the bounded grace — never waited on forever.
    let s = LiveSession::spawn(&ProducerSpec::custom("sh", &["-c", "exec sleep 30"]), 1).unwrap();
    let pid = s.producer_pid().unwrap();
    let t0 = Instant::now();
    let end = s.finish(Duration::from_millis(150));
    assert!(!end.graceful);
    assert!(t0.elapsed() < Duration::from_secs(3));
    assert_reaped(pid);
}

#[test]
fn the_real_reference_film_correlates_its_verdict_by_command_id() {
    let mut s = LiveSession::spawn(&ProducerSpec::reference(false), LIVE_SOURCE).unwrap();
    let mut arc = Arc::new();
    let f = s.next_field().unwrap();
    let (o, _, _) = stage_live(&f).unwrap();
    arc.observe(arc_summary(&o, f.cursor));
    arc.discover();
    let f = s.step().unwrap();
    let (o, _, _) = stage_live(&f).unwrap();
    arc.observe(arc_summary(&o, f.cursor));
    arc.intervene(2.0);
    let id = s.submit_amplitude(2.0).unwrap();
    let st = s.await_verdict(id).unwrap();
    assert_eq!(st, AckStatus::Acknowledged);
    arc.verdict(verdict_of(st));
    assert_eq!(arc.beat(), Beat::Await);
    let f = s.step().unwrap();
    let (o, _, _) = stage_live(&f).unwrap();
    arc.observe(arc_summary(&o, f.cursor));
    assert!(arc.transformed());
    assert!(s.channel().conserves());
    assert_eq!(s.channel().acknowledged(), 1);
    assert_eq!(s.counters(), live_link::LiveCounters::default());
    let end = s.finish(SHUTDOWN_GRACE);
    assert!(end.graceful);
    let _ = Verdict::Refused; // (the refused path is pinned in field_cinema_effect_contract.rs)
}

/// RXIV review finding 7: a frame AFTER the producer retired its source is out of contract — it
/// must never be admitted (it could otherwise reach the arc) and the ordered end follows the frames
/// that PRECEDED the RETIRE only.
#[test]
fn review_frames_after_retire_are_never_admitted() {
    let (mut s, _) = hostile(
        cat(&[
            field(1, 1, 1, 1.0),
            encode_retire(1, SourceCursor::new(1, 2)),
            field(1, 1, 3, 2.0),
        ]),
        vec![],
        Faults::default(),
    );
    assert_eq!(s.next_field().unwrap().cursor, SourceCursor::new(1, 1));
    assert!(matches!(s.next_field(), Err(LiveError::Retired { .. })));
    assert_eq!(s.counters().after_retire, 1);
}

/// RXIV review finding 8: a producer that leaves a DESCENDANT holding its stdout/stderr must not
/// stall cleanup — reaping the direct child does not close a pipe a descendant still holds. The
/// session kills the producer's whole process group, and its joins are bounded.
#[test]
fn review_a_descendant_holding_the_pipes_cannot_stall_cleanup() {
    let s = LiveSession::spawn(
        &ProducerSpec::custom("sh", &["-c", "sleep 6 & echo bg=$! >&2; exec sleep 30"]),
        1,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let bg: u32 = loop {
        let t = s.stderr_tail();
        if let Some(v) = t.split("bg=").nth(1) {
            if let Some(n) = v.split_whitespace().next().and_then(|d| d.parse().ok()) {
                break n;
            }
        }
        assert!(Instant::now() < deadline, "no bg pid in stderr: {t:?}");
        std::thread::sleep(Duration::from_millis(10));
    };
    let pid = s.producer_pid().unwrap();
    let t0 = Instant::now();
    drop(s);
    let took = t0.elapsed();
    assert!(
        took < Duration::from_secs(3),
        "cleanup took {took:?} — a descendant holding the pipe stalled the join"
    );
    assert_reaped(pid);
    assert_reaped(bg);
}
