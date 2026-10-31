//! `wire_instrument` — a domain-neutral 2-D scalar-field inspector (Renaissance V, Milestone A).
//!
//! The real cross-process loop the `smartchem_canary` stops short of: this presenter *spawns*
//! the reference Python producer with **both** pipes connected, decodes its frame stream
//! incrementally through a bounded [`FrameDecoder`] (never `read_to_end`), and renders the
//! latest observation's **actual** `FieldFrame.surface` to the terminal via
//! `FieldView::render → Node::raster → Context` — not a hand-rolled ASCII dump. Edits the user
//! makes are serialized back as COMMAND frames on the producer's stdin; the producer validates,
//! mutates its authoritative model, and ACKs; the returned verdict is correlated to the exact
//! edit through an application-owned [`CommandChannel`] (see `channel.rs`), and a *later* field
//! carries the accepted change. The instrument knows nothing about what the field *means* — no
//! "chem", no gaussian assumption — it inspects whatever source-identified scalar field arrives.
//!
//! Run interactively against the reference producer:
//! ```text
//! cargo run --example wire_instrument
//! ```
//! Keys: arrows move the probe · `+`/`-` zoom · `[`/`]` adjust the pending amplitude ·
//! Enter submits a set-amplitude edit · `q`/Esc quits. Submitting > 10 is refused by the
//! producer and the field stays put — visible in the edit receipts.
//!
//! `--selfcheck` runs a bounded, deterministic, headless proof for CI (no TTY): it drives the
//! producer in `--lockstep` (one field per advance, the ack the only sync receipt — no hz timer,
//! no sleeps), proving the full application path end to end: a baseline field (tick 1) rendered
//! through `FieldView::render → Node::raster → Context`; a set-amplitude 2.0 edit admitted and
//! delivered through the app-owned `CommandChannel`; its `Acknowledged` ack correlated back; and
//! a later field (tick 2) whose peak has grown, every store conserving. The rigorous
//! equal-sequence 2.0× causal proof (with the `--freeze-amplitude` suppression control) lives in
//! `tests/wire_instrument_loop.rs`; the untracked-desync + wrong-source-ack proofs live in
//! `tests/wire_instrument_channel.rs`; `tests/wire_instrument_selfcheck.rs` executes this
//! `--selfcheck` binary in CI; `tests/wire_instrument_pty.rs` walks the interactive path on a
//! real PTY.

mod channel;

use std::io::{self, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use gibson::context::Context;
use gibson::field_view::{Colormap, FieldView, RangePolicy, Sample};
use gibson::input::{Event, KeyCode};
use gibson::node::Node;
use gibson::renderer::RenderMode;
use gibson::wire::{
    encode_command, FieldInbox, FrameDecoder, SourceCursor, WireBody, WireCommand, WireMessage,
};
use gibson::world_view::WorldView;
use gibson::{Rect, Style};

use channel::CommandChannel;

/// The producer's one command kind: an 8-byte LE f64 target amplitude in [0, 10].
const KIND_SET_AMPLITUDE: u32 = 1;
/// Lockstep control (selfcheck only): advance the model one tick. Never acked, so it must NOT be
/// routed through the ledger — `run_selfcheck` sends it raw. Mirrors `KIND_ADVANCE` in the
/// producer and `tests/wire_instrument_loop.rs`.
const KIND_ADVANCE: u32 = 2;

struct Args {
    selfcheck: bool,
    producer: String,
    python: String,
    cols: u32,
    rows: u32,
    frames: u32,
    hz: f64,
    source: u64,
    epoch: u32,
}

impl Args {
    fn parse() -> Args {
        let mut a = Args {
            selfcheck: false,
            producer: "bindings/python/producer.py".to_string(),
            python: std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string()),
            cols: 36,
            rows: 18,
            frames: 240,
            hz: 20.0,
            source: 1,
            epoch: 1,
        };
        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--selfcheck" => a.selfcheck = true,
                "--producer" => a.producer = it.next().unwrap_or(a.producer),
                "--cols" => a.cols = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.cols),
                "--rows" => a.rows = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.rows),
                "--frames" => a.frames = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.frames),
                "--hz" => a.hz = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.hz),
                _ => {}
            }
        }
        a
    }
}

/// Spawn the reference producer with both pipes connected. Streaming (`serve`) mode for the
/// interactive UI; deterministic `--lockstep` (one field per advance, no hz timer, no sleeps)
/// for the selfcheck.
fn spawn_producer(args: &Args, lockstep: bool) -> io::Result<Child> {
    let mut cmd = Command::new(&args.python);
    cmd.arg(&args.producer)
        .args(["--frames", &args.frames.to_string()])
        .args(["--hz", &args.hz.to_string()])
        .args(["--cols", &args.cols.to_string()])
        .args(["--rows", &args.rows.to_string()])
        .args(["--source", &args.source.to_string()])
        .args(["--epoch", &args.epoch.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if lockstep {
        cmd.arg("--lockstep");
    }
    cmd.spawn()
}

/// A background reader draining the child's stdout into a channel, so the UI loop never blocks
/// on producer I/O. The sender drops on EOF, which the loop reads as producer disconnect.
fn spawn_reader(child: &mut Child) -> (Receiver<Vec<u8>>, JoinHandle<()>) {
    let mut out = child.stdout.take().expect("stdout piped");
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let handle = thread::spawn(move || {
        let mut buf = [0u8; 65536];
        loop {
            match out.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    (rx, handle)
}

/// Build the UI: the real rendered field surface stacked over the readouts the prompt requires
/// — source / tick / probe value (with missing-state identity) and the edit lifecycle counts.
fn build_root(
    inbox: &FieldInbox,
    src: Option<u64>,
    ch: &CommandChannel,
    probe: (usize, usize),
    ppc: u16,
    pending_amp: f64,
    status: &str,
) -> Node {
    let mut root = Node::col();
    if let Some(src) = src {
        if let Some(field) = inbox.latest(src) {
            let ext = field.extent;
            if let Some(view) = FieldView::new(ext, &field.data, field.mask.as_deref()) {
                let w = (ext.cols() as u16).saturating_mul(ppc).clamp(1, 240);
                let h = (ext.rows() as u16).saturating_mul(ppc).clamp(1, 80);
                let wv = WorldView::new(Rect::new(0, 0, w, h)).with_pixels_per_cell(ppc, ppc);
                let frame = view.render(
                    &wv,
                    Rect::new(0, 0, w, h),
                    Colormap::Density,
                    RangePolicy::Auto,
                );
                let (pix, piy) = probe;
                let sample_txt = match view.sample(pix, piy) {
                    Some(Sample::Value(v)) => format!("{v:.4}"),
                    Some(Sample::Masked) => "masked".to_string(),
                    Some(Sample::NonFinite) => "non-finite".to_string(),
                    None => "off-field".to_string(),
                };
                let r = &frame.report;
                let hud_src = format!(
                    "source {src}  model_tick {}  {}x{} cells  zoom {ppc}x  render[value {} masked {} nonfinite {} off {} conserves {}]",
                    field.model_tick,
                    ext.cols(),
                    ext.rows(),
                    r.value_cells,
                    r.masked_cells,
                    r.nonfinite_cells,
                    r.off_field_cells,
                    r.conserves(),
                );
                let hud_probe =
                    format!("probe ({pix},{piy}) = {sample_txt}    pending set-amplitude = {pending_amp:.1}  [ / ] adjust · Enter send");
                let hud_edit = format!(
                    "edits: issued {} pending {} outstanding {} | ack {} refused {} failed {} | conserves {}",
                    ch.issued(),
                    ch.pending(),
                    ch.outstanding(),
                    ch.acknowledged(),
                    ch.refused(),
                    ch.failed(),
                    ch.conserves(),
                );
                return root
                    .child(Node::raster(frame.surface))
                    .child(Node::text(hud_src, Style::default()))
                    .child(Node::text(hud_probe, Style::default()))
                    .child(Node::text(hud_edit, Style::default()))
                    .child(Node::text(status.to_string(), Style::default()));
            }
        }
    }
    root = root.child(Node::text(
        "wire_instrument — waiting for the first field…",
        Style::default(),
    ));
    root
}

/// Clamp the probe to the current field's bounds.
fn clamp_probe(inbox: &FieldInbox, src: Option<u64>, probe: &mut (usize, usize)) {
    if let Some(src) = src {
        if let Some(field) = inbox.latest(src) {
            let ext = field.extent;
            probe.0 = probe.0.min(ext.cols().saturating_sub(1));
            probe.1 = probe.1.min(ext.rows().saturating_sub(1));
        }
    }
}

fn run_interactive(args: &Args) -> io::Result<()> {
    let mut child = spawn_producer(args, false)?;
    let mut stdin_w: Option<ChildStdin> = child.stdin.take();
    let (rx, reader) = spawn_reader(&mut child);

    let mut ctx = Context::fullscreen()?;
    let mut dec = FrameDecoder::new();
    let mut inbox = FieldInbox::new();
    let mut ch = CommandChannel::new(args.source, args.epoch);
    let mut src: Option<u64> = None;
    let mut probe: (usize, usize) = (0, 0);
    let mut ppc: u16 = 1;
    let mut pending_amp: f64 = 2.0;
    let mut status = String::from("connected — streaming…");
    let mut disconnected = false;

    loop {
        let ev = ctx.run_once(Duration::from_millis(16))?;

        // Drain whatever the producer has sent, without blocking the UI.
        loop {
            match rx.try_recv() {
                Ok(bytes) => {
                    if let Err(e) = dec.push(&bytes) {
                        status = format!("transport fatal: {e}");
                        disconnected = true;
                        break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        // Decode every complete frame now buffered.
        loop {
            match dec.next_message() {
                None => break,
                Some(Ok(msg)) => {
                    src.get_or_insert(msg.source);
                    match msg.body {
                        WireBody::Field(f) => {
                            inbox.admit(msg.source, msg.cursor, f);
                        }
                        WireBody::Retire => {
                            inbox.retire(msg.source);
                            status = format!("source {} retired", msg.source);
                        }
                        WireBody::Ack(ack) => match ch.resolve(msg.source, ack) {
                            Ok(state) => status = format!("ack: edit resolved → {state:?}"),
                            Err(e) => status = format!("ack ignored ({e:?})"),
                        },
                        WireBody::Command(_) => {}
                    }
                }
                Some(Err(e)) => {
                    if e.is_fatal() {
                        status = format!("fatal decode: {e}");
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        if let Some(Event::Key(k)) = ev {
            match k.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Left => probe.0 = probe.0.saturating_sub(1),
                KeyCode::Right => probe.0 += 1,
                KeyCode::Up => probe.1 = probe.1.saturating_sub(1),
                KeyCode::Down => probe.1 += 1,
                KeyCode::Char('+') | KeyCode::Char('=') => ppc = (ppc + 1).min(4),
                KeyCode::Char('-') => ppc = ppc.saturating_sub(1).max(1),
                KeyCode::Char('[') => pending_amp = (pending_amp - 0.5).max(0.0),
                KeyCode::Char(']') => pending_amp = (pending_amp + 0.5).min(12.0),
                KeyCode::Enter => {
                    match ch.issue_tracked(KIND_SET_AMPLITUDE, pending_amp.to_le_bytes().to_vec()) {
                        Ok(id) => {
                            if let Some(sink) = stdin_w.as_mut() {
                                match ch.deliver_next(sink) {
                                    Ok(Some(_)) => {
                                        status =
                                            format!("sent set-amplitude {pending_amp:.1} (id {id})")
                                    }
                                    Ok(None) => status = "nothing to deliver".to_string(),
                                    Err(e) => status = format!("delivery uncertain (pipe: {e})"),
                                }
                            } else {
                                status = "producer stdin closed".to_string();
                            }
                        }
                        Err(e) => status = format!("edit refused locally ({e:?})"),
                    }
                }
                _ => {}
            }
            clamp_probe(&inbox, src, &mut probe);
        }

        let root = build_root(&inbox, src, &ch, probe, ppc, pending_amp, &status);
        ctx.set_root(root);

        if disconnected {
            // One final paint of the terminal state, then exit (bounded).
            let root = build_root(
                &inbox,
                src,
                &ch,
                probe,
                ppc,
                pending_amp,
                "producer disconnected — press q",
            );
            ctx.set_root(root);
            ctx.run_once(Duration::from_millis(16))?;
            break;
        }
    }

    let _ = dec.finish();
    drop(stdin_w); // EOF to the producer for a graceful exit…
    let _ = child.kill(); // …with a hard backstop so no child outlives us.
    let _ = child.wait();
    let _ = reader.join();
    ctx.restore()?;
    Ok(())
}

/// Tear the producer and terminal down, then exit non-zero — the selfcheck's single failure
/// path, so no branch leaves a child or a raw terminal behind.
fn selfcheck_fail(child: &mut Child, ctx: &mut Context, msg: &str) -> ! {
    let _ = child.kill();
    let _ = child.wait();
    let _ = ctx.restore();
    eprintln!("wire_instrument selfcheck FAILED: {msg}");
    std::process::exit(1);
}

/// The finite peak (max finite sample) of the latest field for `src`, if any.
fn selfcheck_peak(inbox: &FieldInbox, src: u64) -> Option<f64> {
    let field = inbox.latest(src)?;
    let view = FieldView::new(field.extent, &field.data, field.mask.as_deref())?;
    view.finite_extremes().map(|(_, hi)| hi)
}

/// Blocking read of the next whole wire message from the lockstep producer, bounded by a finite
/// read budget so a stalled producer fails loudly instead of hanging CI. A missing counterpart
/// is a real failure, never a skip — the loop is the thing under test.
fn selfcheck_next(
    out: &mut impl Read,
    dec: &mut FrameDecoder,
    child: &mut Child,
    ctx: &mut Context,
) -> WireMessage {
    let mut rbuf = [0u8; 65536];
    let mut reads = 0u32;
    loop {
        match dec.next_message() {
            Some(Ok(msg)) => return msg,
            Some(Err(e)) => {
                if e.is_fatal() {
                    selfcheck_fail(child, ctx, &format!("fatal decode: {e}"));
                }
            }
            None => {}
        }
        reads += 1;
        if reads > 4000 {
            selfcheck_fail(child, ctx, "exceeded read budget waiting for a message");
        }
        match out.read(&mut rbuf) {
            Ok(0) => selfcheck_fail(
                child,
                ctx,
                "producer closed its stream before a message arrived",
            ),
            Ok(n) => {
                if dec.push(&rbuf[..n]).is_err() {
                    selfcheck_fail(child, ctx, "transport fatal on push");
                }
            }
            Err(e) => selfcheck_fail(child, ctx, &format!("read error: {e}")),
        }
    }
}

fn run_selfcheck(args: &Args) -> io::Result<()> {
    // Drive the producer in deterministic --lockstep mode: one field per advance, the ack the
    // only synchronization receipt. No hz timer, no sleeps, no read-count heuristics.
    let mut child = spawn_producer(args, true)?;
    let mut stdin_w = child.stdin.take().expect("stdin piped");
    let mut out = child.stdout.take().expect("stdout piped");

    let mut ctx = Context::headless(RenderMode::Inline, 80, 30);
    let mut dec = FrameDecoder::new();
    let mut inbox = FieldInbox::new();
    let mut ch = CommandChannel::new(args.source, args.epoch);
    let src = args.source;

    // --- 1) Baseline FIELD (tick 1, amplitude 1.0): identity, then the real render path. ---
    let msg = selfcheck_next(&mut out, &mut dec, &mut child, &mut ctx);
    if msg.source != src || msg.cursor.epoch != args.epoch {
        selfcheck_fail(
            &mut child,
            &mut ctx,
            &format!(
                "baseline frame identity mismatch: got source {} epoch {}, want source {} epoch {}",
                msg.source, msg.cursor.epoch, src, args.epoch
            ),
        );
    }
    match msg.body {
        WireBody::Field(f) => {
            inbox.admit(msg.source, msg.cursor, f);
        }
        other => selfcheck_fail(
            &mut child,
            &mut ctx,
            &format!("expected baseline FIELD, got {other:?}"),
        ),
    }
    let baseline_tick = inbox.latest(src).map(|f| f.model_tick).unwrap_or_default();
    ctx.set_root(build_root(
        &inbox,
        Some(src),
        &ch,
        (0, 0),
        1,
        2.0,
        "selfcheck baseline",
    ));
    ctx.run_once(Duration::from_millis(0))?;
    let baseline_peak = selfcheck_peak(&inbox, src);

    // --- 2) Admit + deliver a set-amplitude 2.0 edit through the app-owned CommandChannel. ---
    match ch.issue_tracked(KIND_SET_AMPLITUDE, 2.0f64.to_le_bytes().to_vec()) {
        Ok(_) => match ch.deliver_next(&mut stdin_w) {
            Ok(Some(_)) => {}
            Ok(None) => selfcheck_fail(
                &mut child,
                &mut ctx,
                "nothing to deliver after a successful issue",
            ),
            Err(e) => selfcheck_fail(&mut child, &mut ctx, &format!("edit delivery failed: {e}")),
        },
        Err(e) => selfcheck_fail(
            &mut child,
            &mut ctx,
            &format!("edit refused locally: {e:?}"),
        ),
    }

    // --- 3) Correlate the producer's ACK back through the channel. ---
    let ack_msg = selfcheck_next(&mut out, &mut dec, &mut child, &mut ctx);
    let ack_state = match ack_msg.body {
        WireBody::Ack(ack) => match ch.resolve(ack_msg.source, ack) {
            Ok(state) => format!("{state:?}"),
            Err(e) => selfcheck_fail(&mut child, &mut ctx, &format!("ack did not resolve: {e:?}")),
        },
        other => selfcheck_fail(
            &mut child,
            &mut ctx,
            &format!("expected an ACK, got {other:?}"),
        ),
    };

    // --- 4) Advance the model one tick: a raw control frame, never a tracked edit (so it must
    //        not touch the ledger — its sequence is monotonically beyond the delivered edit). ---
    let advance = WireCommand {
        command_id: 0,
        kind: KIND_ADVANCE,
        arg: Vec::new(),
    };
    let adv_bytes = encode_command(src, SourceCursor::new(args.epoch, u64::MAX), &advance);
    if stdin_w
        .write_all(&adv_bytes)
        .and_then(|_| stdin_w.flush())
        .is_err()
    {
        selfcheck_fail(
            &mut child,
            &mut ctx,
            "could not send the advance control frame",
        );
    }

    // --- 5) Post-edit FIELD (tick 2, amplitude 2.0): identity, render, peak. ---
    let msg2 = selfcheck_next(&mut out, &mut dec, &mut child, &mut ctx);
    if msg2.source != src || msg2.cursor.epoch != args.epoch {
        selfcheck_fail(
            &mut child,
            &mut ctx,
            &format!(
                "post-edit frame identity mismatch: got source {} epoch {}",
                msg2.source, msg2.cursor.epoch
            ),
        );
    }
    match msg2.body {
        WireBody::Field(f) => {
            inbox.admit(msg2.source, msg2.cursor, f);
        }
        other => selfcheck_fail(
            &mut child,
            &mut ctx,
            &format!("expected post-edit FIELD, got {other:?}"),
        ),
    }
    let post_tick = inbox.latest(src).map(|f| f.model_tick).unwrap_or_default();
    ctx.set_root(build_root(
        &inbox,
        Some(src),
        &ch,
        (0, 0),
        1,
        2.0,
        "selfcheck post",
    ));
    ctx.run_once(Duration::from_millis(0))?;
    let post_peak = selfcheck_peak(&inbox, src);

    // --- teardown: EOF, reap, capture the rendered-frame receipt. ---
    drop(stdin_w);
    let _ = child.kill();
    let _ = child.wait();
    let frame_nonempty = !ctx.last_frame_lines().is_empty();
    ctx.restore()?;

    // --- verdict ---
    let base = baseline_peak.unwrap_or(f64::NAN);
    let post = post_peak.unwrap_or(f64::NAN);
    let causal = base.is_finite() && post.is_finite() && post >= 1.5 * base;
    println!("wire_instrument selfcheck (lockstep):");
    println!("  baseline  tick {baseline_tick}  peak {base:.4}");
    println!("  post-edit tick {post_tick}  peak {post:.4}  (ack {ack_state})");
    println!(
        "  edits: issued {} ack {} refused {} failed {} outstanding {} conserves {}",
        ch.issued(),
        ch.acknowledged(),
        ch.refused(),
        ch.failed(),
        ch.outstanding(),
        ch.conserves()
    );
    println!("  surface_rendered = {frame_nonempty}");

    if ack_state != "Acknowledged" {
        eprintln!("selfcheck FAILED: edit was not Acknowledged (got {ack_state})");
        std::process::exit(1);
    }
    if post_tick <= baseline_tick {
        eprintln!(
            "selfcheck FAILED: post-edit field did not advance the model tick ({baseline_tick} -> {post_tick})"
        );
        std::process::exit(1);
    }
    if !causal {
        eprintln!("selfcheck FAILED: accepted edit did not raise the field peak (base {base:.4} -> post {post:.4})");
        std::process::exit(1);
    }
    if ch.outstanding() != 0 || !ch.conserves() {
        eprintln!("selfcheck FAILED: command accounting did not conserve");
        std::process::exit(1);
    }
    if !frame_nonempty {
        eprintln!("selfcheck FAILED: no surface was rendered through Context");
        std::process::exit(1);
    }
    println!("wire_instrument selfcheck PASSED");
    Ok(())
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    if args.selfcheck {
        run_selfcheck(&args)
    } else {
        run_interactive(&args)
    }
}
