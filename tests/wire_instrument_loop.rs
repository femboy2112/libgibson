//! Cross-process causal proof for the wire instrument (Renaissance V, Milestone A §1).
//!
//! The in-process `bidirectional_acked_loop_all_verdicts_and_mutation` in `wire_transport.rs`
//! proves the *protocol* deterministically. This proves the real **two-process** loop: a Rust
//! consumer spawns the Python reference producer (`--lockstep`), issues edits as COMMAND frames
//! on its stdin, reads ACKs and FIELDs back on its stdout, and verifies that an *accepted* edit
//! changes a later authoritative field while a *suppressed* one (the producer's
//! `--freeze-amplitude` mutation control, which still acks `Acknowledged`) does not — and that
//! an out-of-range edit is `Refused` with the field untouched.
//!
//! Determinism without sleeps: `--lockstep` emits one field per `KIND_ADVANCE`, and the ACK is
//! the synchronization receipt. Edited and control runs are compared at the SAME sequence (same
//! gaussian center), so the only difference is the amplitude factor. If python3 or the producer
//! cannot be spawned, the test fails loudly rather than skipping — the loop is the thing under
//! test, so a missing counterpart is a real failure, not a pass.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use gibson::wire::{
    encode_command, AckStatus, FrameDecoder, SourceCursor, WireBody, WireCommand, WireMessage,
};

const KIND_SET_AMPLITUDE: u32 = 1;
const KIND_ADVANCE: u32 = 2;

fn python() -> String {
    std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// A live lockstep producer plus the wire plumbing to drive it one frame at a time.
struct Link {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    dec: FrameDecoder,
    out_seq: u64,
}

impl Link {
    fn spawn(freeze: bool, cols: u32, rows: u32) -> Link {
        let mut cmd = Command::new(python());
        cmd.arg("bindings/python/producer.py")
            .arg("--lockstep")
            .args(["--frames", "10"])
            .args(["--cols", &cols.to_string()])
            .args(["--rows", &rows.to_string()])
            .args(["--source", "1"])
            .args(["--epoch", "1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if freeze {
            cmd.arg("--freeze-amplitude");
        }
        let mut child = cmd
            .spawn()
            .expect("spawn python3 bindings/python/producer.py --lockstep (is python3 on PATH?)");
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");
        Link {
            child,
            stdin,
            stdout,
            dec: FrameDecoder::new(),
            out_seq: 0,
        }
    }

    fn send(&mut self, kind: u32, arg: Vec<u8>) {
        self.out_seq += 1;
        let cmd = WireCommand {
            command_id: self.out_seq,
            kind,
            arg,
        };
        let bytes = encode_command(1, SourceCursor::new(1, self.out_seq), &cmd);
        self.stdin.write_all(&bytes).expect("write command");
        self.stdin.flush().expect("flush command");
    }

    fn advance(&mut self) {
        self.send(KIND_ADVANCE, vec![]);
    }

    fn next_message(&mut self) -> WireMessage {
        let mut buf = [0u8; 65536];
        loop {
            if let Some(res) = self.dec.next_message() {
                return res.expect("a clean frame from the producer");
            }
            let n = self.stdout.read(&mut buf).expect("read producer stdout");
            assert!(n > 0, "producer closed its stream before a message arrived");
            self.dec.push(&buf[..n]).expect("decoder push");
        }
    }

    fn next_field(&mut self) -> (u64, Vec<f64>) {
        match self.next_message().body {
            WireBody::Field(f) => (f.model_tick, f.data),
            other => panic!("expected a FIELD, got {other:?}"),
        }
    }

    fn next_ack(&mut self) -> AckStatus {
        match self.next_message().body {
            WireBody::Ack(a) => a.status,
            other => panic!("expected an ACK, got {other:?}"),
        }
    }

    fn finish(mut self) {
        drop(self.stdin); // EOF to the producer
        let _ = self.child.kill(); // hard backstop (finite deadline)
        let _ = self.child.wait();
    }
}

/// Read seq 1, run the per-mode protocol, advance to seq 2, return that field's samples.
fn run(mode: &str, cols: u32, rows: u32) -> Vec<f64> {
    let mut link = Link::spawn(mode == "freeze", cols, rows);
    let (t1, _) = link.next_field();
    assert_eq!(t1, 1, "{mode}: first field must be tick 1");

    if mode == "edit" || mode == "freeze" {
        link.send(KIND_SET_AMPLITUDE, 2.0f64.to_le_bytes().to_vec());
        assert_eq!(
            link.next_ack(),
            AckStatus::Acknowledged,
            "{mode}: a valid set-amplitude must be Acknowledged (freeze still ACKs)"
        );
    }

    link.advance();
    let (t2, data) = link.next_field();
    assert_eq!(t2, 2, "{mode}: second field must be tick 2");
    link.finish();
    data
}

#[test]
fn accepted_edit_changes_a_later_field_but_a_suppressed_one_does_not() {
    let (cols, rows) = (4u32, 3u32);
    let ctrl = run("control", cols, rows);
    let edit = run("edit", cols, rows);
    let frozen = run("freeze", cols, rows);

    assert_eq!(edit.len(), ctrl.len());
    assert_eq!(frozen.len(), ctrl.len());

    // Compared at equal sequence: the only difference is the amplitude. The accepted edit must
    // double the field; the suppressed edit (acked but not applied) must leave it == control.
    let mut compared = 0usize;
    for ((c, e), z) in ctrl.iter().zip(&edit).zip(&frozen) {
        if c.abs() > 1e-9 {
            compared += 1;
            assert!(
                (e / c - 2.0).abs() < 1e-9,
                "accepted edit must double the field: {e} vs control {c}"
            );
            assert!(
                (z / c - 1.0).abs() < 1e-9,
                "suppressed edit must NOT change the field: {z} vs control {c}"
            );
        }
    }
    assert!(compared > 0, "no non-zero control cells to compare");
}

#[test]
fn out_of_range_edit_is_refused_and_the_field_is_unchanged() {
    let (cols, rows) = (4u32, 3u32);
    let ctrl = run("control", cols, rows);

    let mut link = Link::spawn(false, cols, rows);
    let (t1, _) = link.next_field();
    assert_eq!(t1, 1);
    // 99.0 is outside the producer's [0, 10] contract.
    link.send(KIND_SET_AMPLITUDE, 99.0f64.to_le_bytes().to_vec());
    assert_eq!(
        link.next_ack(),
        AckStatus::Refused,
        "an out-of-range edit must be Refused, not Acknowledged"
    );
    link.advance();
    let (t2, after) = link.next_field();
    assert_eq!(t2, 2);
    link.finish();

    assert_eq!(after.len(), ctrl.len());
    for (a, c) in after.iter().zip(&ctrl) {
        assert!(
            (a - c).abs() < 1e-12,
            "a refused edit must leave the field byte-identical to the control: {a} vs {c}"
        );
    }
}
