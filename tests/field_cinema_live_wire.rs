//! Gate H — the first complete live-wire cinematic: the `field_cinema` grammar driven by the REAL
//! Python↔Rust producer, not the synthetic `Model`.
//!
//! `field_cinema` normally films a deterministic in-process `Model`. H asks for the film to be
//! CAUSED BY an actual cross-process field stream: the same pure pipeline
//! (`FieldView → observe → compose → render_shot`) fed by frames the reference producer emits over
//! real OS pipes, with the edit histories (accepted / refused / suppressed) coming from real
//! producer verdicts, and the film replaying deterministically. The medium is pure terminal cells
//! (`FieldFrame.surface`) — no audio, no third-party media, no HumanMusic — so it is rights-clear by
//! construction. Lockstep so the producer is deterministic and the proof is sleep-free; if python3
//! or the producer cannot be spawned, the test fails loudly rather than skipping.

#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use cinema::{compose, field_center, observe, render_shot, FieldObservation, SourceId};
use gibson::field_view::FieldView;
use gibson::surface::Surface;
use gibson::wire::{
    encode_command, AckStatus, FrameDecoder, SourceCursor, WireBody, WireCommand, WireField,
    WireMessage,
};
use gibson::Rect;

const KIND_SET_AMPLITUDE: u32 = 1;
const KIND_ADVANCE: u32 = 2;
const ANCHOR: Rect = Rect {
    x: 0,
    y: 0,
    width: 48,
    height: 16,
};

fn python() -> String {
    std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// A live lockstep producer, driving the cinema grammar one real frame at a time.
struct Link {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    dec: FrameDecoder,
    out_seq: u64,
}

impl Link {
    fn spawn(freeze: bool) -> Link {
        let mut cmd = Command::new(python());
        cmd.arg("bindings/python/producer.py")
            .arg("--lockstep")
            .args(["--frames", "40"])
            .args(["--cols", "48"])
            .args(["--rows", "24"])
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

    fn recv(&mut self) -> WireMessage {
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

    /// Read the next FIELD. The producer emits seq=1 UNSOLICITED on start; afterward each ADVANCE
    /// yields exactly one FIELD (a control signal, never acked).
    fn recv_field(&mut self) -> WireField {
        match self.recv().body {
            WireBody::Field(f) => f,
            other => panic!("expected a FIELD, got {other:?}"),
        }
    }

    /// Read the producer's TRUE verdict for the last submitted edit.
    fn recv_ack(&mut self) -> AckStatus {
        match self.recv().body {
            WireBody::Ack(a) => a.status,
            other => panic!("expected an ACK, got {other:?}"),
        }
    }

    fn send_advance(&mut self) {
        self.send(KIND_ADVANCE, vec![]);
    }

    fn send_amplitude(&mut self, v: f64) {
        self.send(KIND_SET_AMPLITUDE, v.to_le_bytes().to_vec());
    }

    fn finish(mut self) {
        drop(self.stdin); // EOF → the producer retires and exits
        let _ = self.child.wait();
    }
}

/// Stage one real frame through the UNMODIFIED cinema grammar: observe → compose → render.
fn stage_live(f: &WireField) -> (FieldObservation, Surface) {
    let view = FieldView::new(f.extent, &f.data, f.mask.as_deref()).expect("real field geometry");
    let obs = observe(&view, SourceId(1), 1, f.model_tick);
    // Probe the field center deterministically (no clock, no RNG).
    let probe = view.probe(field_center(f.extent));
    let shot = compose(&obs, &probe, f.extent);
    let frame = render_shot(&view, &shot, ANCHOR);
    (obs, frame.surface)
}

/// A film frame is staged from a REAL cross-process field, not the synthetic model.
#[test]
fn a_film_frame_is_staged_from_a_real_producer_field() {
    let mut link = Link::spawn(false);
    let f = link.recv_field(); // the unsolicited seq=1 field
    let (obs, surface) = stage_live(&f);

    assert_eq!(
        obs.tick, f.model_tick,
        "the shot's tick is the field's own model tick"
    );
    assert!(
        obs.peak_value.is_some(),
        "a real gaussian field has a finite peak the director can frame on"
    );
    assert!(
        surface.width > 0 && surface.height > 0,
        "the film frame is a non-empty cell surface (pure cells — no media, rights-clear)"
    );
    link.finish();
}

/// The edit histories come from REAL producer verdicts: an accepted set-amplitude grows the peak in
/// a later real field; an out-of-range one is refused and the field is unchanged; and under
/// `--freeze-amplitude` an accepted edit is SUPPRESSED (acknowledged, but the field does not move).
#[test]
fn accepted_refused_and_suppressed_edits_drive_the_live_history() {
    // --- accepted + refused on a normal producer ---
    let mut link = Link::spawn(false);
    let base = link.recv_field(); // unsolicited seq=1
    let (base_obs, _) = stage_live(&base);
    let base_peak = base_obs.peak_value.expect("baseline peak");
    assert!(
        (0.8..1.1).contains(&base_peak),
        "amplitude 1.0 baseline peak ~1.0; got {base_peak}"
    );

    // Accepted: 2.0 is in the producer's [0,10] contract.
    link.send_amplitude(2.0);
    assert_eq!(link.recv_ack(), AckStatus::Acknowledged);
    link.send_advance();
    let (acc_obs, _) = stage_live(&link.recv_field());
    let acc_peak = acc_obs.peak_value.expect("accepted peak");
    assert!(
        (1.8..2.2).contains(&acc_peak),
        "an ACCEPTED set-amplitude 2.0 must ~double the peak in the next real field; got {acc_peak}"
    );

    // Refused: 99.0 is out of range — the field must NOT jump to ~99.
    link.send_amplitude(99.0);
    assert_eq!(link.recv_ack(), AckStatus::Refused);
    link.send_advance();
    let (ref_obs, _) = stage_live(&link.recv_field());
    let ref_peak = ref_obs.peak_value.expect("refused peak");
    assert!(
        ref_peak < 10.0 && (1.6..2.4).contains(&ref_peak),
        "a REFUSED edit leaves the model at 2.0, never ~99; got {ref_peak}"
    );
    link.finish();

    // --- suppressed on a frozen producer ---
    let mut frozen = Link::spawn(true);
    let _ = frozen.recv_field(); // unsolicited seq=1
                                 // The producer acknowledges the edit but --freeze-amplitude suppresses its effect.
    frozen.send_amplitude(2.0);
    assert_eq!(frozen.recv_ack(), AckStatus::Acknowledged);
    frozen.send_advance();
    let (sup_obs, _) = stage_live(&frozen.recv_field());
    let sup_peak = sup_obs.peak_value.expect("suppressed peak");
    assert!(
        (0.8..1.1).contains(&sup_peak),
        "a SUPPRESSED (acknowledged-but-frozen) edit leaves the peak at ~1.0; got {sup_peak}"
    );
    frozen.finish();
}

/// The live film replays deterministically: the same lockstep script against the deterministic
/// producer, through the pure grammar, renders byte-identical cell surfaces.
#[test]
fn the_live_film_replays_byte_identically() {
    fn film() -> Vec<Surface> {
        let mut link = Link::spawn(false);
        let mut frames = Vec::new();
        // The unsolicited seq=1 field, then five advanced ticks.
        let (_, surface) = stage_live(&link.recv_field());
        frames.push(surface);
        for _ in 0..5 {
            link.send_advance();
            let (_, surface) = stage_live(&link.recv_field());
            frames.push(surface);
        }
        link.finish();
        frames
    }

    let a = film();
    let b = film();
    assert_eq!(a.len(), 6);
    assert_eq!(
        a, b,
        "a live-wire film replays byte-identically — the producer is deterministic and the grammar is pure"
    );
}
