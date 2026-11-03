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
mod transport;
mod viewport;

use std::collections::VecDeque;
use std::io::{self, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use gibson::context::Context;
use gibson::field_view::{Colormap, FieldView, RangePolicy, Sample};
use gibson::input::{Event, KeyCode};
use gibson::instrument_telemetry::{InstrumentTelemetry, Stage};
use gibson::node::Node;
use gibson::renderer::RenderMode;
use gibson::wire::{encode_command, FieldInbox, SourceCursor, WireBody, WireCommand, WireMessage};
use gibson::Style;

use channel::CommandChannel;
use transport::{ProducerOwner, Reader};

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
///
/// The producer's stderr is **inherited**, not discarded. A nulled stderr once turned a
/// producer that failed to launch (it was spawned in `$HOME`, where the repo-relative
/// `producer.py` did not exist) into a silent, causeless timeout: the consumer saw no frames
/// and could not say why. The consumer already fails loudly and boundedly on a dead producer
/// (`selfcheck_next` has a finite read budget); inheriting stderr makes the *cause* visible —
/// the producer's own diagnostic reaches the CI log and the terminal — rather than hidden (L7:
/// a producer failure is a distinct, reportable event, never an invisible stall).
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
        .stderr(Stdio::inherit());
    if lockstep {
        cmd.arg("--lockstep");
    }
    cmd.spawn()
}

/// Build the UI: the real rendered field surface stacked over the readouts the prompt requires
/// — source / tick / probe value (with missing-state identity) and the edit lifecycle counts.
// A UI builder: every parameter is a display input the prompt's readouts require (field, source,
// edit counts, probe, zoom, pending amplitude, status, and the command-dispatch leg).
#[allow(clippy::too_many_arguments)]
fn build_root(
    inbox: &FieldInbox,
    src: Option<u64>,
    ch: &CommandChannel,
    probe: (usize, usize),
    ppc: u16,
    pending_amp: f64,
    status: &str,
    cmd_leg: &str,
) -> Node {
    let mut root = Node::col();
    if let Some(src) = src {
        if let Some(field) = inbox.latest(src) {
            let ext = field.extent;
            if let Some(view) = FieldView::new(ext, &field.data, field.mask.as_deref()) {
                let (pix, piy) = probe;
                let sample_txt = match view.sample(pix, piy) {
                    Some(Sample::Value(v)) => format!("{v:.4}"),
                    Some(Sample::Masked) => "masked".to_string(),
                    Some(Sample::NonFinite) => "non-finite".to_string(),
                    None => "off-field".to_string(),
                };
                let hud_edit = format!(
                    "edits: issued {} pending {} outstanding {} | ack {} refused {} failed {} | delivered {} xfail {} faulted {} | conserves {}",
                    ch.issued(),
                    ch.pending(),
                    ch.outstanding(),
                    ch.acknowledged(),
                    ch.refused(),
                    ch.failed(),
                    ch.delivered_frames(),
                    ch.transport_failed(),
                    ch.faulted(),
                    ch.conserves(),
                );
                // Gate G/D3: derive the display transport from the SAME source geometry the probe
                // indexes — origin (pan), anisotropic cell sizes (per-axis zoom), and ppc
                // magnification — via checked, clamp-before-narrow sizing. A field dimension past
                // u16::MAX saturates to the cap; a nonzero-origin or anisotropic field no longer
                // renders off-view/distorted while the probe reads real values. And a geometry
                // whose zoom overflows to non-finite (D3) has NO invertible mapping: show the
                // still-exact probe value and an explicit notice, never a blank that contradicts it.
                match viewport::world_view_for(&ext, ppc) {
                    Ok((wv, area)) => {
                        let frame = view.render(&wv, area, Colormap::Density, RangePolicy::Auto);
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
                        let hud_probe = format!(
                            "probe ({pix},{piy}) = {sample_txt}    pending set-amplitude = {pending_amp:.1}  [ / ] adjust · Enter send"
                        );
                        return root
                            .child(Node::raster(frame.surface))
                            .child(Node::text(hud_src, Style::default()))
                            .child(Node::text(hud_probe, Style::default()))
                            .child(Node::text(hud_edit, Style::default()))
                            .child(Node::text(cmd_leg.to_string(), Style::default()))
                            .child(Node::text(status.to_string(), Style::default()));
                    }
                    Err(u) => {
                        let hud_src = format!(
                            "source {src}  model_tick {}  {}x{} cells  VIEW UNSUPPORTED at zoom {ppc}x — cell {:e}x{:e} -> non-invertible zoom {:e}x{:e}",
                            field.model_tick,
                            ext.cols(),
                            ext.rows(),
                            ext.cell_w(),
                            ext.cell_h(),
                            u.zoom_x,
                            u.zoom_y,
                        );
                        let hud_probe = format!(
                            "probe ({pix},{piy}) = {sample_txt}  (geometry not displayable at this zoom; the sample is still exact)"
                        );
                        return root
                            .child(Node::text(hud_src, Style::default()))
                            .child(Node::text(hud_probe, Style::default()))
                            .child(Node::text(hud_edit, Style::default()))
                            .child(Node::text(cmd_leg.to_string(), Style::default()))
                            .child(Node::text(status.to_string(), Style::default()));
                    }
                }
            }
        }
    }
    root = root.child(Node::text(
        "wire_instrument — waiting for the first field…",
        Style::default(),
    ));
    root
}

/// The command-dispatch latency readout, shared by the selfcheck verdict and the interactive HUD
/// so their wording cannot drift. The `command-dispatch leg: count N` prefix is asserted by both
/// tests/wire_instrument_selfcheck.rs and tests/wire_instrument_pty.rs.
fn dispatch_readout(telemetry: &InstrumentTelemetry) -> String {
    let cmd = telemetry.stage(Stage::CommandDispatch);
    format!(
        "command-dispatch leg: count {} p50 {:?} max {:?}  (issue->confirmed write+flush; NOT producer ack)",
        cmd.count(),
        cmd.p50(),
        cmd.max(),
    )
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

/// Spawn the producer, wire up the bounded reader, and drive the interactive loop — with the
/// child, its stdin, and the reader thread owned by a [`ProducerOwner`] so they are reaped on
/// every exit path, and the terminal restored whether the loop returns `Ok` or a `?` error (Gate
/// E). Original-error precedence: a loop error is returned even if terminal restoration also fails.
fn run_interactive(args: &Args) -> io::Result<()> {
    let mut child = spawn_producer(args, false)?;
    let stdin_w = child.stdin.take();
    let stdout = child.stdout.take().expect("stdout piped");
    // Decode + coalesce in a bounded background reader that never blocks on the UI, so the
    // producer's stdout is always drained (no full-duplex deadlock, Gate B 2C) and memory stays
    // bounded to one pending snapshot per source (2A).
    let reader = Reader::spawn(stdout, transport::DEFAULT_CONTROL_CAP);
    // Supervised ownership: the owner's Drop reaps the child (via the OS wait handle) and joins
    // the reader on EVERY exit — a `?` error from terminal init below or any in-loop render, a
    // normal quit, or a panic unwind. Neither `Reader` nor `Context` cleans up on Drop, so without
    // this a `?` early return orphaned the Python child and detached the reader thread (Gate E).
    let mut owner = ProducerOwner::new(child, stdin_w, reader);

    // If terminal init fails, `owner` drops here and reaps the child; the terminal was never put
    // into raw mode, so there is nothing to restore.
    let mut ctx = Context::fullscreen()?;

    // Run the fallible loop, then ALWAYS restore the terminal — the restore is a plain statement,
    // not `?`, so it runs even when the loop propagated an error. The owner drops at scope end,
    // reaping the child and joining the reader regardless of how we leave.
    let loop_result = interactive_loop(args, &mut ctx, &mut owner);
    let restore_result = ctx.restore();
    loop_result.and(restore_result)
}

/// The interactive render/command loop. Fallible (`ctx.run_once` can error); every `?` here
/// returns into [`run_interactive`], which still restores the terminal and (via the owner's Drop)
/// reaps the child before propagating.
fn interactive_loop(args: &Args, ctx: &mut Context, owner: &mut ProducerOwner) -> io::Result<()> {
    let mut inbox = FieldInbox::new();
    let mut ch = CommandChannel::new(args.source, args.epoch);
    // The #143 latency funnel, now also wired through the INTERACTIVE delivery path (H1 instrumented
    // the selfcheck leg only). Mirrors the selfcheck exactly: only the local issue->write+flush leg,
    // never the producer ACK (L7/L10).
    let mut telemetry = InstrumentTelemetry::new();
    let mut src: Option<u64> = None;
    let mut probe: (usize, usize) = (0, 0);
    let mut ppc: u16 = 1;
    let mut pending_amp: f64 = 2.0;
    let mut status = String::from("connected — streaming…");
    let mut disconnected = false;

    loop {
        let ev = ctx.run_once(Duration::from_millis(16))?;

        // Drain decoded messages from the bounded reader. Snapshots arrive already coalesced to
        // the latest per source; acks and retires are preserved in order; a recoverable decode
        // error was skipped in the reader, a fatal one is surfaced here. The UI never holds an
        // unbounded backlog and never pushes a chunk-burst into the decoder (Gate B 2A/2B).
        let mut batch: Vec<WireMessage> = Vec::new();
        let outcome = owner.reader().drain(&mut |m| batch.push(m));
        for msg in batch {
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
        if let Some(ov) = outcome.control_overflow {
            // The control lane failed closed: an authoritative verdict was lost, so command fate
            // is unresolved. Treat it as a fatal transport condition — do not keep running and
            // claiming command-lifecycle correctness behind a drop counter.
            status = format!(
                "transport fatal: control lane overflow (cap {}, {:?}) — command fate lost",
                ov.control_cap, ov.kind
            );
            disconnected = true;
        } else if let Some(e) = outcome.fatal {
            status = format!("transport fatal: {e}");
            disconnected = true;
        } else if outcome.disconnected {
            // Distinguish a hard read fault from an orderly shutdown (Gate C4): the producer
            // closing its stdout cleanly and the pipe faulting both end the stream, but they are
            // not the same event, and a disconnect handler that conflates them is lying.
            status = match outcome.read_error {
                Some(kind) => format!("producer stream faulted ({kind:?}) — disconnected"),
                None => String::from("producer disconnected (clean EOF)"),
            };
            disconnected = true;
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
                            if let Some(sink) = owner.stdin_mut() {
                                // Time ONLY the delivery leg (issue->confirmed write+flush),
                                // mirroring the selfcheck; record on Ok(Some) alone — a failed or
                                // absent delivery has no honest latency (L7/L10).
                                let t0 = Instant::now();
                                let delivered = ch.deliver_next(sink);
                                let leg = t0.elapsed();
                                match delivered {
                                    Ok(Some(_)) => {
                                        telemetry.record(Stage::CommandDispatch, leg);
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

        // L7 telemetry: the reader's lawful snapshot coalescing and any expendable field drops are
        // shown, never hidden — a growing `fields_dropped` is a misbehaving producer, not a leak.
        // Retained FIELD bytes and their high-water are surfaced against the budget so a byte-bound
        // flood is visible as a real memory envelope, not just a message count (Gate C1). A lost
        // *control* verdict is never a counter here; it is the terminal control-overflow above.
        let reader = owner.reader();
        let shown = format!(
            "{status}  |  xport: coalesced {} fields_dropped {} skips {} bytes {}/{} KiB (hw {} KiB)",
            reader.coalesced(),
            reader.fields_dropped(),
            reader.decode_skips(),
            reader.field_bytes() / 1024,
            reader.field_byte_cap() / 1024,
            reader.bytes_high_water() / 1024,
        );
        let root = build_root(
            &inbox,
            src,
            &ch,
            probe,
            ppc,
            pending_amp,
            &shown,
            &dispatch_readout(&telemetry),
        );
        ctx.set_root(root);

        if disconnected {
            // One final paint of the terminal state, then exit (bounded). Carry the real reason
            // (clean EOF vs fault vs fatal vs control-overflow) into the final frame (Gate C4).
            let final_note = format!("{status} — press q");
            let root = build_root(
                &inbox,
                src,
                &ch,
                probe,
                ppc,
                pending_amp,
                &final_note,
                &dispatch_readout(&telemetry),
            );
            ctx.set_root(root);
            ctx.run_once(Duration::from_millis(16))?;
            break;
        }
    }

    // Teardown is NOT here: the terminal is restored by `run_interactive` on every exit, and the
    // child is reaped + the reader joined by the `ProducerOwner`'s Drop — including the `?` returns
    // above. A normal quit falls through to this `Ok(())`; an error returns early and is handled
    // identically by the caller.
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

/// Wall-clock deadline for a single lockstep message wait. A healthy producer answers in
/// milliseconds; this bound only ever bites a producer that never writes — which must fail the
/// selfcheck loudly and promptly, not hang CI (Gate E).
const SELFCHECK_MSG_DEADLINE: Duration = Duration::from_secs(10);

/// Wait for the next decoded message from the lockstep producer, draining the **non-blocking**
/// background [`Reader`] and bounding the wait by a WALL-CLOCK deadline rather than a read count.
///
/// # Why this is not a blocking read (Gate E)
///
/// The previous implementation blocked directly on `stdout.read()` with only a read-*count* budget.
/// A producer that connects but never writes blocks the very first `read()` forever, so the count
/// never advances and the budget is no deadline at all — the selfcheck (and CI) hangs indefinitely.
/// Draining the [`Reader`] instead never blocks on the producer: [`Reader::drain`] returns promptly
/// whether or not a frame has arrived (the reader thread owns the blocking read, off this thread),
/// so a true wall-clock deadline is enforceable here and a stalled or silent producer fails fast.
/// A clean EOF is distinguished from a read fault via the drain's `read_error` provenance (Gate C4).
///
/// Decoded messages drain in order; `pending` buffers any the caller has not yet consumed (the
/// reader's lawful FIELD coalescing does not apply across the lockstep's command round-trips).
fn selfcheck_next(
    reader: &Reader,
    pending: &mut VecDeque<WireMessage>,
    child: &mut Child,
    ctx: &mut Context,
) -> WireMessage {
    if let Some(msg) = pending.pop_front() {
        return msg;
    }
    let deadline = Instant::now() + SELFCHECK_MSG_DEADLINE;
    loop {
        let outcome = reader.drain(&mut |m| pending.push_back(m));
        if let Some(msg) = pending.pop_front() {
            return msg;
        }
        // No message yet. Handle terminal conditions, then enforce the wall-clock deadline.
        if let Some(ov) = outcome.control_overflow {
            selfcheck_fail(
                child,
                ctx,
                &format!(
                    "control lane overflow (cap {}, {:?}) — a verdict was lost",
                    ov.control_cap, ov.kind
                ),
            );
        }
        if let Some(e) = outcome.fatal {
            selfcheck_fail(child, ctx, &format!("fatal decode: {e}"));
        }
        if outcome.disconnected {
            // Clean close vs. severed pipe — report the real provenance (Gate C4).
            match outcome.read_error {
                Some(kind) => selfcheck_fail(
                    child,
                    ctx,
                    &format!("producer stream faulted ({kind:?}) before a message arrived"),
                ),
                None => selfcheck_fail(
                    child,
                    ctx,
                    "producer closed its stream before a message arrived",
                ),
            }
        }
        if Instant::now() >= deadline {
            selfcheck_fail(
                child,
                ctx,
                "timed out waiting for a message (wall-clock) — a producer that never writes no longer hangs the selfcheck",
            );
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn run_selfcheck(args: &Args) -> io::Result<()> {
    // Drive the producer in deterministic --lockstep mode: one field per advance, the ack the
    // only synchronization receipt. No hz timer, no sleeps, no read-count heuristics.
    let mut child = spawn_producer(args, true)?;
    let mut stdin_w = child.stdin.take().expect("stdin piped");
    let out = child.stdout.take().expect("stdout piped");
    // Decode on the SAME bounded, non-blocking reader the interactive path uses, so the selfcheck's
    // message waits can be bounded by a wall-clock deadline (Gate E) instead of a blocking read.
    let reader = Reader::spawn(out, transport::DEFAULT_CONTROL_CAP);
    let mut pending: VecDeque<WireMessage> = VecDeque::new();

    let mut ctx = Context::headless(RenderMode::Inline, 80, 30);
    let mut inbox = FieldInbox::new();
    let mut ch = CommandChannel::new(args.source, args.epoch);
    // The #143 latency funnel, run end to end through this real consumer loop — not an
    // injected-Duration detection test. Only the command-delivery leg is exercised here; the
    // render-side stages belong to `instrument_bench`.
    let mut telemetry = InstrumentTelemetry::new();
    let src = args.source;

    // --- 1) Baseline FIELD (tick 1, amplitude 1.0): identity, then the real render path. ---
    let msg = selfcheck_next(&reader, &mut pending, &mut child, &mut ctx);
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
        &dispatch_readout(&telemetry),
    ));
    // Headless: render the composition path directly (FieldFrame.surface → Node::raster →
    // Context). `run_once` polls input — a headless session keeps is_tty true, so it does not
    // early-return — and crossterm's reader init fails on a non-TTY stdin (e.g. --selfcheck's).
    ctx.render()?;
    let baseline_peak = selfcheck_peak(&inbox, src);

    // --- 2) Admit + deliver a set-amplitude 2.0 edit through the app-owned CommandChannel. ---
    // Time the DELIVERY leg — issue→confirmed write+flush on THIS side of the pipe — into
    // Stage::CommandDispatch. This is a local monotonic measurement of a local operation (the
    // write to the producer's stdin and its flush); it is NOT a producer round-trip. The
    // producer's ACK is correlated separately in step 3 and is never folded into this leg —
    // delivery and acknowledgement are distinct events on distinct clocks (L7/L10). Only a
    // confirmed delivery (`Ok(Some)`) is recorded.
    match ch.issue_tracked(KIND_SET_AMPLITUDE, 2.0f64.to_le_bytes().to_vec()) {
        Ok(_) => {
            let t0 = Instant::now();
            let delivered = ch.deliver_next(&mut stdin_w);
            let leg = t0.elapsed();
            match delivered {
                Ok(Some(_)) => telemetry.record(Stage::CommandDispatch, leg),
                Ok(None) => selfcheck_fail(
                    &mut child,
                    &mut ctx,
                    "nothing to deliver after a successful issue",
                ),
                Err(e) => {
                    selfcheck_fail(&mut child, &mut ctx, &format!("edit delivery failed: {e}"))
                }
            }
        }
        Err(e) => selfcheck_fail(
            &mut child,
            &mut ctx,
            &format!("edit refused locally: {e:?}"),
        ),
    }

    // --- 3) Correlate the producer's ACK back through the channel. ---
    let ack_msg = selfcheck_next(&reader, &mut pending, &mut child, &mut ctx);
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
    let msg2 = selfcheck_next(&reader, &mut pending, &mut child, &mut ctx);
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
        &dispatch_readout(&telemetry),
    ));
    // Headless: render the composition path directly (FieldFrame.surface → Node::raster →
    // Context). `run_once` polls input — a headless session keeps is_tty true, so it does not
    // early-return — and crossterm's reader init fails on a non-TTY stdin (e.g. --selfcheck's).
    ctx.render()?;
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
        "  edits: issued {} ack {} refused {} failed {} outstanding {} delivered {} xfail {} faulted {} conserves {}",
        ch.issued(),
        ch.acknowledged(),
        ch.refused(),
        ch.failed(),
        ch.outstanding(),
        ch.delivered_frames(),
        ch.transport_failed(),
        ch.faulted(),
        ch.conserves()
    );
    println!("  surface_rendered = {frame_nonempty}");
    println!("  {}", dispatch_readout(&telemetry));

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
    if ch.faulted() {
        eprintln!("selfcheck FAILED: command channel faulted on a healthy pipe");
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
    if telemetry.stage(Stage::CommandDispatch).count() == 0 {
        eprintln!(
            "selfcheck FAILED: command-dispatch latency was never recorded — the #143 funnel did not run end to end"
        );
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
