//! RXIV Gates C + D4 — the interactive film's causal oracle on RENDERED SURFACES, and its causal
//! record / exact semantic replay.
//!
//! The PTY suite (`pty_field_cinema_interactive.rs`) drives the human path; this drives the SAME
//! `Film` type headlessly against the real lockstep producer at matched model ticks, so the oracle
//! can compare what the viewer would actually see — the shot-framed field surface — across
//! histories, not just peak values:
//!
//! * accepted ≠ no-edit (the verified change reframes the shot) and only accepted transforms;
//! * suppressed ≡ no-edit and refused ≡ no-edit, byte-for-byte on the final surface — an
//!   acknowledged-but-suppressed edit LOOKS exactly like no intervention, as it must;
//! * the same holds on the non-Gaussian ring holdout.
//!
//! D4: every film's record replays (no producer) to the identical per-event beat sequence, final
//! shot and final surface; field projection vs causal digests separate histories with equal
//! observations; transport uncertainty and stale/repeated frames are handled and replayed; tampered
//! records are refused; and the record is colour-depth independent while the rendered bytes are not.
#![cfg(unix)]

#[path = "../examples/wire_instrument/capture.rs"]
mod capture;
#[path = "../examples/wire_instrument/channel.rs"]
mod channel;
#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;
#[path = "../examples/field_cinema/film.rs"]
mod film;
#[path = "../examples/wire_instrument/input.rs"]
mod input;
#[path = "../examples/field_cinema/live.rs"]
mod live;
#[path = "../examples/field_cinema/live_link.rs"]
mod live_link;
#[path = "../examples/wire_instrument/trace.rs"]
mod trace;
#[path = "../examples/wire_instrument/transport.rs"]
mod transport;

use std::collections::VecDeque;
use std::io::{self, ErrorKind, Read, Write};
use std::sync::mpsc::{channel as mpsc_channel, Receiver, Sender};
use std::time::Duration;

use film::{replay_film, Film, FilmAction, FilmConfig, ReplayError};
use gibson::capability::ColorDepth;
use gibson::context::Context;
use gibson::field_view::FieldExtent;
use gibson::input::KeyCode;
use gibson::renderer::RenderMode;
use gibson::wire::{
    encode_ack, encode_field, AckStatus, FrameDecoder, SourceCursor, WireAck, WireBody, WireField,
    WireLimits,
};
use gibson::{Node, Point};
use live::Beat;
use live_link::{LiveError, LiveSession, ProducerSpec, LIVE_SOURCE, SHUTDOWN_GRACE};
use trace::Admission;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[derive(Clone, Copy, PartialEq)]
enum History {
    NoEdit,
    Accepted,
    Refused,
    Suppressed,
}

/// Drive the interactive film exactly as the PTY loop does, but headlessly: frame 1, a probe move,
/// a calibration step, (an intervention), two more steps, quit. Every history advances the model
/// through the SAME ticks, so final surfaces are comparable at an identical model tick.
fn drive(h: History, shape: &[&str], probe_key: KeyCode) -> Film {
    let spec = ProducerSpec::reference_with(h == History::Suppressed, shape);
    let mut s = LiveSession::spawn(&spec, LIVE_SOURCE).expect("spawn the reference producer");
    let mut film = Film::new(FilmConfig {
        reduced_motion: true,
        auto_advance: false,
    });
    let mut t = 0u64;
    let mut now = || {
        t += 16;
        ms(t)
    };
    film.on_field(s.next_field().unwrap(), now());
    assert_eq!(film.on_key(probe_key, now()), FilmAction::Nothing);
    assert_eq!(film.on_key(KeyCode::Char(' '), now()), FilmAction::Step);
    film.on_field(s.step().unwrap(), now());
    if h != History::NoEdit {
        if h == History::Refused {
            for _ in 0..20 {
                film.on_key(KeyCode::Char(']'), now());
            }
        }
        let FilmAction::Submit(v) = film.on_key(KeyCode::Enter, now()) else {
            panic!("Enter must request a submission");
        };
        let id = s.submit_amplitude(v).unwrap();
        film.on_submitted(id, Admission::Accepted, now());
        let st = s.await_verdict(id).unwrap();
        film.on_verdict(id, st, now());
    }
    for _ in 0..2 {
        assert_eq!(film.on_key(KeyCode::Char(' '), now()), FilmAction::Step);
        film.on_field(s.step().unwrap(), now());
    }
    film.finish(now());
    let end = s.finish(SHUTDOWN_GRACE);
    assert!(end.graceful);
    film
}

fn four(shape: &[&str]) -> [Film; 4] {
    [
        drive(History::NoEdit, shape, KeyCode::Right),
        drive(History::Accepted, shape, KeyCode::Right),
        drive(History::Refused, shape, KeyCode::Right),
        drive(History::Suppressed, shape, KeyCode::Right),
    ]
}

fn assert_surface_oracle(shape: &[&str]) {
    let [n, a, r, s] = four(shape);
    assert!(!n.arc().transformed(), "{shape:?}: no-edit");
    assert!(a.arc().transformed(), "{shape:?}: accepted transforms");
    assert!(!r.arc().transformed(), "{shape:?}: refused");
    assert!(!s.arc().transformed(), "{shape:?}: suppressed");
    assert!(a.beats().contains(&Beat::Transform));
    for f in [&n, &r, &s] {
        assert!(!f.beats().contains(&Beat::Transform));
    }

    let (sn, sa, sr, ss) = (
        n.field_surface().unwrap(),
        a.field_surface().unwrap(),
        r.field_surface().unwrap(),
        s.field_surface().unwrap(),
    );
    assert_ne!(sa, sn, "{shape:?}: the verified change is ON SCREEN");
    assert_eq!(
        ss, sn,
        "{shape:?}: an acknowledged-but-suppressed edit renders EXACTLY like no edit"
    );
    assert_eq!(
        sr, sn,
        "{shape:?}: a refused edit renders exactly like no edit"
    );
    assert_ne!(a.shot(), n.shot());
    assert_eq!(s.shot(), n.shot());
    assert_eq!(r.shot(), n.shot());
}

#[test]
fn the_rendered_surfaces_satisfy_the_causal_oracle_on_the_gaussian() {
    assert_surface_oracle(&[]);
}

#[test]
fn the_rendered_surfaces_satisfy_the_causal_oracle_on_the_ring_holdout() {
    assert_surface_oracle(&["--shape", "ring", "--mask-every", "5"]);
}

// ---------------------------------------------------------------------------------------------
// D4 — exact semantic replay and the faithful-history digests
// ---------------------------------------------------------------------------------------------

fn assert_replays(f: &Film) {
    let r = replay_film(&f.record().frames, &f.record().trace).expect("replayable record");
    assert_eq!(r.beats, f.beats(), "per-event beat sequence");
    assert_eq!(r.transforms, f.arc().transforms());
    assert_eq!(r.last_outcome, f.arc().last_outcome());
    assert_eq!(r.final_shot, f.shot(), "final shot");
    assert_eq!(
        r.final_surface,
        f.field_surface(),
        "final rendered field surface"
    );
}

#[test]
fn every_history_replays_exactly_from_its_record_alone() {
    for f in four(&[]) {
        assert_replays(&f);
    }
    for f in four(&["--shape", "ring", "--mask-every", "5"]) {
        assert_replays(&f);
    }
}

#[test]
fn equal_observations_with_different_command_histories_are_causally_distinct() {
    let n = drive(History::NoEdit, &[], KeyCode::Right);
    let r = drive(History::Refused, &[], KeyCode::Right);
    let (tn, tr) = (&n.record().trace, &r.record().trace);
    assert_eq!(
        capture::content_digest(&n.record().frames),
        capture::content_digest(&r.record().frames),
        "the refused edit changed no observation"
    );
    assert_eq!(tn.field_projection_digest(), tr.field_projection_digest());
    assert_ne!(
        tn.causal_digest(),
        tr.causal_digest(),
        "the causal trace records the refused command (with its argument)"
    );
}

#[test]
fn different_probe_histories_are_causally_distinct_and_replay_to_their_own_frames() {
    let right = drive(History::NoEdit, &[], KeyCode::Right);
    let left = drive(History::NoEdit, &[], KeyCode::Left);
    assert_eq!(
        right.record().trace.field_projection_digest(),
        left.record().trace.field_projection_digest()
    );
    assert_ne!(
        right.record().trace.causal_digest(),
        left.record().trace.causal_digest()
    );
    assert_replays(&right);
    assert_replays(&left);
    assert_ne!(
        right.field_surface(),
        left.field_surface(),
        "the probe is on screen"
    );
}

#[test]
fn accepted_and_suppressed_share_the_command_history_but_not_the_observations() {
    let a = drive(History::Accepted, &[], KeyCode::Right);
    let s = drive(History::Suppressed, &[], KeyCode::Right);
    let non_obs = |f: &Film| -> Vec<trace::TraceEvent> {
        f.record()
            .trace
            .events()
            .iter()
            .filter(|e| !matches!(e, trace::TraceEvent::Observation { .. }))
            .cloned()
            .collect()
    };
    assert_eq!(
        non_obs(&a),
        non_obs(&s),
        "identical commands, arguments and ACKs"
    );
    assert_ne!(
        a.record().trace.causal_digest(),
        s.record().trace.causal_digest()
    );
    assert_ne!(
        a.record().trace.field_projection_digest(),
        s.record().trace.field_projection_digest(),
        "the difference is in what was OBSERVED"
    );
}

#[test]
fn a_tampered_record_is_refused_not_replayed() {
    let a = drive(History::Accepted, &[], KeyCode::Right);
    let (frames, tr) = (&a.record().frames, &a.record().trace);

    let mut changed = frames.clone();
    changed[2].data[0] += 1.0;
    assert!(matches!(
        replay_film(&changed, tr),
        Err(ReplayError::ChecksumMismatch { .. })
    ));

    let mut swapped = frames.clone();
    swapped.swap(1, 2);
    assert!(matches!(
        replay_film(&swapped, tr),
        Err(ReplayError::FrameMismatch { .. })
    ));

    let short = frames[..frames.len() - 1].to_vec();
    assert!(matches!(
        replay_film(&short, tr),
        Err(ReplayError::FrameMissing { .. })
    ));

    let mut extra = frames.clone();
    extra.push(frames[0].clone());
    assert!(matches!(
        replay_film(&extra, tr),
        Err(ReplayError::UnusedFrames { count: 1 })
    ));
}

#[test]
fn the_record_is_colour_depth_independent_while_the_rendered_bytes_are_not() {
    let a = drive(History::Accepted, &[], KeyCode::Right);
    let r = replay_film(&a.record().frames, &a.record().trace).unwrap();
    let surface = r.final_surface.clone().unwrap();
    let render = |depth: ColorDepth| {
        let mut ctx = Context::headless(RenderMode::Inline, 80, 24);
        ctx.set_color_depth(depth);
        ctx.set_root(Node::raster(surface.clone()));
        ctx.render().expect("headless render");
        ctx.rendered_bytes().to_vec()
    };
    assert_ne!(render(ColorDepth::Mono), render(ColorDepth::TrueColor));
    // The record never saw a terminal: its digests are a function of content and history only.
    let again = replay_film(&a.record().frames, &a.record().trace).unwrap();
    assert_eq!(again.beats, r.beats);
    assert_eq!(
        capture::content_digest(&a.record().frames),
        capture::content_digest(&a.record().frames)
    );
}

// ---------------------------------------------------------------------------------------------
// Transport uncertainty and stale/repeated frames, over a scripted in-memory producer
// ---------------------------------------------------------------------------------------------

fn bump(amp: f64, seq: u64) -> Vec<u8> {
    let extent = FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, 12, 8).unwrap();
    let data = (0..96)
        .map(|i| {
            // Centred ON a grid point, so the sampled peak equals the amplitude (the effect
            // contract measures the peak SAMPLE; see the limitations ledger).
            let (x, y) = ((i % 12) as f64 - 6.0, (i / 12) as f64 - 4.0);
            amp * (-(x * x + y * y) / 8.0).exp()
        })
        .collect();
    let f = WireField {
        extent,
        model_tick: seq,
        data,
        mask: None,
    };
    encode_field(1, SourceCursor::new(1, seq), &f, WireLimits::default()).unwrap()
}

fn ack1(seq: u64) -> Vec<u8> {
    encode_ack(
        1,
        SourceCursor::new(1, seq),
        WireAck {
            command_id: 1,
            status: AckStatus::Acknowledged,
        },
    )
}

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
struct Responder {
    tx: Sender<Vec<u8>>,
    script: VecDeque<Vec<u8>>,
    dec: FrameDecoder,
    /// Fail the flush of the set-amplitude EDIT only (its whole frame was written).
    flush_fails: bool,
    fail_this_flush: bool,
}
impl Write for Responder {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.dec.push(b).unwrap();
        while let Some(m) = self.dec.next_message() {
            if let WireBody::Command(c) = m.unwrap().body {
                self.fail_this_flush = self.flush_fails && c.kind == 1;
                if let Some(chunk) = self.script.pop_front() {
                    let _ = self.tx.send(chunk);
                }
            }
        }
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if std::mem::take(&mut self.fail_this_flush) {
            Err(io::Error::other("scripted flush failure"))
        } else {
            Ok(())
        }
    }
}

/// A scripted lockstep film: frame 1, probe, step → frame 2, edit (the script answers the command
/// with an ACK), step → `after_step` chunk. `flush_fails` makes the edit transport-uncertain.
fn scripted(flush_fails: bool, after_step: Vec<u8>) -> (Film, live_link::LiveCounters) {
    let (tx, rx) = mpsc_channel();
    tx.send(bump(1.0, 1)).unwrap();
    let responder = Responder {
        tx,
        script: VecDeque::from(vec![bump(1.0, 2), ack1(3), after_step]),
        dec: FrameDecoder::new(),
        flush_fails,
        fail_this_flush: false,
    };
    let reader = ChanReader {
        rx,
        buf: Vec::new(),
        pos: 0,
    };
    let mut s = LiveSession::over_pipes(reader, responder, LIVE_SOURCE)
        .with_deadline(Duration::from_secs(5));
    let mut film = Film::new(FilmConfig {
        reduced_motion: true,
        auto_advance: false,
    });
    film.on_field(s.next_field().unwrap(), ms(1));
    film.on_key(KeyCode::Right, ms(2));
    film.on_field(s.step().unwrap(), ms(3));
    let FilmAction::Submit(v) = film.on_key(KeyCode::Enter, ms(4)) else {
        panic!("submit")
    };
    match s.submit_amplitude(v) {
        Ok(id) => film.on_submitted(id, Admission::Accepted, ms(5)),
        Err(LiveError::Transport {
            command_id: Some(id),
            uncertain: true,
            kind: ErrorKind::Other,
        }) => film.on_submitted(id, Admission::TransportUncertain, ms(5)),
        Err(e) => panic!("unexpected {e}"),
    }
    let st = s.await_verdict(1).unwrap();
    film.on_verdict(1, st, ms(6));
    if !flush_fails {
        film.on_field(s.step().unwrap(), ms(7));
    }
    film.finish(ms(8));
    (film, s.counters())
}

#[test]
fn a_transport_uncertain_edit_is_recorded_reconciled_and_replayed() {
    // After a failed flush the channel is faulted: no further frame is requested, so the film
    // resolves on the reconciled verdict alone.
    let (film, _) = scripted(true, bump(2.0, 3));
    let cmd = film
        .record()
        .trace
        .events()
        .iter()
        .find_map(|e| match e {
            trace::TraceEvent::Command { admission, .. } => Some(*admission),
            _ => None,
        })
        .unwrap();
    assert_eq!(cmd, Admission::TransportUncertain);
    assert!(
        film.beats().contains(&Beat::Await),
        "the genuine ACK reconciled it"
    );
    assert_replays(&film);
}

#[test]
fn stale_repeated_frames_never_enter_the_record_and_replay_is_unchanged() {
    let (clean, c0) = scripted(false, bump(2.0, 3));
    let (noisy, c1) = scripted(false, [bump(1.0, 2), bump(2.0, 3)].concat());
    assert_eq!((c0.stale, c0.coalesced), (0, 0));
    // Arriving next to the newer frame, the repeat is dropped by the reader's lawful latest-wins
    // coalescing; arriving alone it is dropped by the session's stale-cursor check
    // (`field_cinema_session_provenance::a_repeated_cursor_is_stale_not_new`). Never admitted.
    assert_eq!(
        c1.stale + c1.coalesced,
        1,
        "the repeated cursor was dropped before admission"
    );
    assert_eq!(
        clean.record().trace.causal_digest(),
        noisy.record().trace.causal_digest(),
        "a stale repeat is not part of the causal history"
    );
    assert!(clean.arc().transformed() && noisy.arc().transformed());
    assert_replays(&noisy);
}

// ---------------------------------------------------------------------------------------------
// Reduced motion — the contract, headlessly
// ---------------------------------------------------------------------------------------------

fn transformed_film(reduced: bool) -> (Film, Duration) {
    let (tx, rx) = mpsc_channel();
    tx.send(bump(1.0, 1)).unwrap();
    let responder = Responder {
        tx,
        script: VecDeque::from(vec![bump(1.0, 2), ack1(3), bump(2.0, 3)]),
        dec: FrameDecoder::new(),
        flush_fails: false,
        fail_this_flush: false,
    };
    let reader = ChanReader {
        rx,
        buf: Vec::new(),
        pos: 0,
    };
    let mut s = LiveSession::over_pipes(reader, responder, LIVE_SOURCE);
    let mut film = Film::new(FilmConfig {
        reduced_motion: reduced,
        auto_advance: false,
    });
    film.on_field(s.next_field().unwrap(), ms(10));
    film.on_field(s.step().unwrap(), ms(20));
    let FilmAction::Submit(v) = film.on_key(KeyCode::Enter, ms(30)) else {
        panic!()
    };
    let id = s.submit_amplitude(v).unwrap();
    film.on_submitted(id, Admission::Accepted, ms(40));
    film.on_verdict(id, s.await_verdict(id).unwrap(), ms(50));
    let t = ms(1000);
    film.on_field(s.step().unwrap(), t);
    assert_eq!(film.beat(), Beat::Transform);
    (film, t)
}

#[test]
fn reduced_motion_cuts_instantly_while_full_motion_animates_the_transform() {
    let frame = |f: &Film, t: Duration| format!("{:?}", f.view(t, 100, 32, false));
    let (full, t) = transformed_film(false);
    assert_ne!(
        frame(&full, t + ms(40)),
        frame(&full, t + ms(2000)),
        "full motion: the camera eases / narration slides / the field jolts after TRANSFORM"
    );
    let (reduced, t) = transformed_film(true);
    assert_eq!(
        frame(&reduced, t + ms(40)),
        frame(&reduced, t + ms(2000)),
        "reduced motion: the post-TRANSFORM frame is final immediately (a cut, no movement)"
    );
}
