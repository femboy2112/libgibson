// Shared by the `field_cinema` example (`--live`) and by the Gate C causal-oracle test via
// `#[path]`. It bridges the REAL cross-process producer to the pure cinema grammar and the
// six-beat `Arc`, so the executable and the oracle drive the *same* live-staging code.
#![allow(dead_code)]
//! The live-wire link (RXIII Checkpoint B / Gate C) — the real Python↔Rust producer wired into
//! `field_cinema`'s grammar and the causal arc.
//!
//! `cinema.rs` films a deterministic in-process `Model`; `live.rs` is the presentation-free arc.
//! This module is the seam between them and the actual instrument: it spawns the reference
//! producer over real OS pipes (lockstep, sleep-free), decodes its authoritative FIELD frames, and
//! stages each one through the *unmodified* `observe → compose → render_shot` pipeline. The arc
//! then advances on what was actually OBSERVED — never on a producer verdict alone — so an
//! acknowledged-but-suppressed edit (`--freeze-amplitude`) can never reach TRANSFORM.
//!
//! Depends on sibling modules `crate::cinema` and `crate::live`, so any includer must declare
//! `mod cinema;` and `mod live;` alongside `mod live_link;`.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::cinema::{
    compose, field_center, observe, render_shot, view_focused_on, FieldObservation, Shot, SourceId,
};
use crate::live::{ObsSummary, Verdict};
use gibson::field_view::{Colormap, FieldView, RangePolicy};
use gibson::surface::Surface;
use gibson::wire::{
    encode_command, AckStatus, FrameDecoder, SourceCursor, WireBody, WireCommand, WireField,
    WireMessage,
};
use gibson::Rect;

/// Command kinds the lockstep producer understands (mirrors `bindings/python/producer.py`).
pub const KIND_SET_AMPLITUDE: u32 = 1;
pub const KIND_ADVANCE: u32 = 2;

/// The source/epoch the live film attributes its frames to (the producer is launched with these).
pub const LIVE_SOURCE: u32 = 1;
pub const LIVE_EPOCH: u32 = 1;

/// The terminal rectangle every live shot is framed into.
pub const ANCHOR: Rect = Rect {
    x: 0,
    y: 0,
    width: 48,
    height: 16,
};

/// The Python this build drives the producer with (`GIBSON_PYTHON` overrides `python3`).
pub fn python() -> String {
    std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// A live lockstep producer: one real frame at a time, over real pipes, no sleeps anywhere.
pub struct Link {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    dec: FrameDecoder,
    out_seq: u64,
}

impl Link {
    /// Spawn the reference producer. `freeze` passes `--freeze-amplitude`, which acks edits
    /// `Acknowledged` but suppresses the model update — the causal mutation control.
    pub fn spawn(freeze: bool) -> Link {
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
    pub fn recv_field(&mut self) -> WireField {
        match self.recv().body {
            WireBody::Field(f) => f,
            other => panic!("expected a FIELD, got {other:?}"),
        }
    }

    /// Read the producer's TRUE verdict for the last submitted edit.
    pub fn recv_ack(&mut self) -> AckStatus {
        match self.recv().body {
            WireBody::Ack(a) => a.status,
            other => panic!("expected an ACK, got {other:?}"),
        }
    }

    pub fn send_advance(&mut self) {
        self.send(KIND_ADVANCE, vec![]);
    }

    pub fn send_amplitude(&mut self, v: f64) {
        self.send(KIND_SET_AMPLITUDE, v.to_le_bytes().to_vec());
    }

    pub fn finish(mut self) {
        drop(self.stdin); // EOF → the producer retires and exits
        let _ = self.child.wait();
    }
}

/// Stage one real frame through the UNMODIFIED cinema grammar: observe → compose → render.
/// Returns the observation, the composed shot, and the rendered cell surface (pure cells — no
/// media, rights-clear by construction).
pub fn stage_live(f: &WireField) -> (FieldObservation, Shot, Surface) {
    let view = FieldView::new(f.extent, &f.data, f.mask.as_deref()).expect("real field geometry");
    let obs = observe(&view, SourceId(LIVE_SOURCE), LIVE_EPOCH, f.model_tick);
    // Probe the field center deterministically (no clock, no RNG).
    let probe = view.probe(field_center(f.extent));
    let shot = compose(&obs, &probe, f.extent);
    let frame = render_shot(&view, &shot, ANCHOR);
    (obs, shot, frame.surface)
}

/// A colour-free preview of the shot's framing, rendered with the density ramp so the intensity
/// is legible in ANY terminal — including a piped/non-TTY stream where the heat colormap's
/// background-colour cells would dump as blank. Not a second render engine: the same `FieldView`
/// and the same shot framing, only the colour-free ramp (mirrors `main.rs`'s synthetic preview).
pub fn mono_preview(f: &WireField, shot: &Shot) -> Surface {
    let view = FieldView::new(f.extent, &f.data, f.mask.as_deref()).expect("real field geometry");
    let wv = view_focused_on(shot.focus, shot.zoom, ANCHOR);
    view.render(&wv, ANCHOR, Colormap::Density, RangePolicy::Auto)
        .surface
}

/// Bridge a real observation into the arc's presentation-free summary. The arc judges only the
/// peak VALUE (amplitude) — exactly what a set-amplitude edit moves — never the field bytes.
pub fn arc_summary(obs: &FieldObservation) -> ObsSummary {
    ObsSummary::new(obs.tick, obs.peak_value)
}

/// Map the producer's wire verdict onto the arc's verdict. The producer's authoritative fates are
/// `Acknowledged` (it will act — arm AWAIT) and `Refused`/`Failed` (no authoritative change will
/// follow — do not arm). Suppression is NOT a verdict: it is an `Acknowledged` whose authoritative
/// change is simply never observed, so there is no `Suppressed` arm. `Accepted`/`Delivered` are
/// presenter-local ledger states that never travel the wire as a producer ACK; one arriving here
/// is a protocol violation, so it panics rather than silently mis-arming the arc.
pub fn verdict_of(status: AckStatus) -> Verdict {
    match status {
        AckStatus::Acknowledged => Verdict::Acknowledged,
        AckStatus::Refused | AckStatus::Failed => Verdict::Refused,
        AckStatus::Accepted | AckStatus::Delivered => {
            panic!("producer wire ACK carried a presenter-local status: {status:?}")
        }
    }
}
