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
//! `--selfcheck` runs a bounded, headless, scripted proof for CI (no TTY): first field →
//! set-amplitude 2.0 → ack → a later field whose peak has grown, with every store's
//! conservation law intact. The rigorous equal-sequence 2.0× causal proof lives in
//! `tests/wire_instrument_loop.rs`; the untracked-desync + wrong-source-ack proofs live in
//! `tests/wire_instrument_channel.rs`.

mod channel;

use std::io::{self, Read};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use gibson::context::Context;
use gibson::field_view::{Colormap, FieldView, RangePolicy, Sample};
use gibson::input::{Event, KeyCode};
use gibson::node::Node;
use gibson::renderer::RenderMode;
use gibson::wire::{FieldInbox, FrameDecoder, WireBody};
use gibson::world_view::WorldView;
use gibson::{Rect, Style};

use channel::CommandChannel;

/// The producer's one command kind: an 8-byte LE f64 target amplitude in [0, 10].
const KIND_SET_AMPLITUDE: u32 = 1;

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

/// Spawn the reference producer in streaming (`serve`) mode with both pipes connected.
fn spawn_producer(args: &Args) -> io::Result<Child> {
    Command::new(&args.python)
        .arg(&args.producer)
        .args(["--frames", &args.frames.to_string()])
        .args(["--hz", &args.hz.to_string()])
        .args(["--cols", &args.cols.to_string()])
        .args(["--rows", &args.rows.to_string()])
        .args(["--source", &args.source.to_string()])
        .args(["--epoch", &args.epoch.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
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
    let mut child = spawn_producer(args)?;
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

/// Bounded, headless, scripted proof for CI: first field (amplitude 1) → set-amplitude 2.0 →
/// Acknowledged ack → a later field whose peak has grown ≥1.5×, every store conserving. Reads
/// the producer synchronously (the script is deterministic given the ack as the sync receipt)
/// and renders each field through the full `Node::raster → Context` path, asserting the frame
/// buffer is non-empty so the on-screen surface path is actually exercised.
/// Tear down the producer and terminal, then exit non-zero — the selfcheck's single failure
/// path, so no branch leaves a child or a raw terminal behind.
fn selfcheck_fail(child: &mut Child, ctx: &mut Context, msg: &str) -> ! {
    let _ = child.kill();
    let _ = child.wait();
    let _ = ctx.restore();
    eprintln!("wire_instrument selfcheck FAILED: {msg}");
    std::process::exit(1);
}

fn run_selfcheck(args: &Args) -> io::Result<()> {
    let mut child = spawn_producer(args)?;
    let mut stdin_w = child.stdin.take().expect("stdin piped");
    let mut out = child.stdout.take().expect("stdout piped");

    let mut ctx = Context::headless(RenderMode::Inline, 80, 30);
    let mut dec = FrameDecoder::new();
    let mut inbox = FieldInbox::new();
    let mut ch = CommandChannel::new(args.source, args.epoch);
    let mut src: Option<u64> = None;

    let mut baseline_peak: Option<f64> = None;
    let mut post_peak: Option<f64> = None;
    let mut fields_seen = 0u32;
    let mut edited = false;
    let mut acked = false;
    let mut ack_state = String::from("<none>");

    let mut rbuf = [0u8; 65536];
    let mut guard = 0u32; // finite deadline: bound total reads

    'read: loop {
        guard += 1;
        if guard > 4000 {
            selfcheck_fail(
                &mut child,
                &mut ctx,
                "exceeded read budget without completing the loop",
            );
        }
        let n = out.read(&mut rbuf)?;
        if n == 0 {
            break; // producer EOF
        }
        if dec.push(&rbuf[..n]).is_err() {
            selfcheck_fail(&mut child, &mut ctx, "transport fatal on push");
        }
        loop {
            match dec.next_message() {
                None => break,
                Some(Ok(msg)) => {
                    src.get_or_insert(msg.source);
                    match msg.body {
                        WireBody::Field(f) => {
                            inbox.admit(msg.source, msg.cursor, f);
                            fields_seen += 1;
                            if let Some(s) = src {
                                if let Some(field) = inbox.latest(s) {
                                    if let Some(view) = FieldView::new(
                                        field.extent,
                                        &field.data,
                                        field.mask.as_deref(),
                                    ) {
                                        let peak = view.finite_extremes().map(|(_, hi)| hi);
                                        // Exercise the real render → raster → Context path.
                                        let root = build_root(
                                            &inbox,
                                            src,
                                            &ch,
                                            (0, 0),
                                            1,
                                            2.0,
                                            "selfcheck",
                                        );
                                        ctx.set_root(root);
                                        ctx.run_once(Duration::from_millis(0))?;
                                        if baseline_peak.is_none() {
                                            baseline_peak = peak;
                                        } else if acked && post_peak.is_none() {
                                            post_peak = peak;
                                            break 'read; // got the post-ack field; done
                                        }
                                    }
                                }
                            }
                            // After a couple of baseline frames, issue the edit once.
                            if !edited && fields_seen >= 2 {
                                match ch.issue_tracked(
                                    KIND_SET_AMPLITUDE,
                                    2.0f64.to_le_bytes().to_vec(),
                                ) {
                                    Ok(_) => {
                                        if ch.deliver_next(&mut stdin_w)?.is_some() {
                                            edited = true;
                                        }
                                    }
                                    Err(e) => {
                                        selfcheck_fail(
                                            &mut child,
                                            &mut ctx,
                                            &format!("could not issue edit: {e:?}"),
                                        );
                                    }
                                }
                            }
                        }
                        WireBody::Ack(ack) => match ch.resolve(msg.source, ack) {
                            Ok(state) => {
                                acked = true;
                                ack_state = format!("{state:?}");
                            }
                            Err(e) => selfcheck_fail(
                                &mut child,
                                &mut ctx,
                                &format!("unexpected ack: {e:?}"),
                            ),
                        },
                        WireBody::Retire => {}
                        WireBody::Command(_) => {}
                    }
                }
                Some(Err(e)) => {
                    if e.is_fatal() {
                        selfcheck_fail(&mut child, &mut ctx, &format!("fatal decode: {e}"));
                    }
                }
            }
        }
    }

    drop(stdin_w);
    let _ = child.kill();
    let _ = child.wait();
    let frame_nonempty = !ctx.last_frame_lines().is_empty();
    ctx.restore()?;

    // Verdict.
    let base = baseline_peak.unwrap_or(f64::NAN);
    let post = post_peak.unwrap_or(f64::NAN);
    let causal = post.is_finite() && base.is_finite() && post >= 1.5 * base;
    println!("wire_instrument selfcheck:");
    println!("  fields_seen   = {fields_seen}");
    println!("  baseline_peak = {base:.4}");
    println!("  post_edit_peak= {post:.4}  (ack {ack_state})");
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

    if !(acked && ack_state == "Acknowledged") {
        eprintln!("selfcheck FAILED: edit was not Acknowledged (got {ack_state})");
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
