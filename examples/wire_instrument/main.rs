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

mod capture;
mod channel;
mod input;
mod layout;
mod transport;
mod viewport;

use std::collections::VecDeque;
use std::io::{self, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use gibson::context::Context;
use gibson::field_view::{Colormap, FieldView, RangePolicy, Sample};
use gibson::input::Event;
use gibson::instrument_telemetry::{InstrumentTelemetry, Stage};
use gibson::node::Node;
use gibson::renderer::RenderMode;
use gibson::wire::{encode_command, FieldInbox, SourceCursor, WireBody, WireCommand, WireMessage};
use gibson::Style;

use channel::CommandChannel;
use input::{InputAction, SourceVerdict};
use transport::{ProducerOwner, Reader};

/// The producer's one command kind: an 8-byte LE f64 target amplitude in [0, 10].
const KIND_SET_AMPLITUDE: u32 = 1;
/// Lockstep control (selfcheck only): advance the model one tick. Never acked, so it must NOT be
/// routed through the ledger — `run_selfcheck` sends it raw. Mirrors `KIND_ADVANCE` in the
/// producer and `tests/wire_instrument_loop.rs`.
const KIND_ADVANCE: u32 = 2;

struct Args {
    selfcheck: bool,
    /// RXII Checkpoint 2 / Gate B (C4): capture the observed field stream to this path (headless,
    /// deterministic lockstep), then print the content digest.
    capture: Option<String>,
    /// Replay a capture from this path, re-rendering each frame and printing the content digest —
    /// which reproduces the capture's digest regardless of the replay terminal's color depth.
    replay: Option<String>,
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
            capture: None,
            replay: None,
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
                "--capture" => a.capture = it.next(),
                "--replay" => a.replay = it.next(),
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

/// How many field observations a `--capture` run records (bounded + deterministic).
const CAPTURE_FRAMES: usize = 3;

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
///
/// Two layouts, chosen by terminal size ([`layout::is_compact`]). At a normal size the classic
/// full layout renders unchanged: the raster over five full-width HUD lines (`status` composed with
/// `xport` into the transport line exactly as before). At the compact 40×15 target the full-width
/// lines would wrap past the screen and push the edit/transport readouts off the bottom, so the
/// COMPACT layout (RXII Checkpoint 2 / Gate B, C1) pins the essentials — source, probe, verdict,
/// status — at the top, caps the raster to the rows left (with an honest truncation marker, C3),
/// and pages the full detail (edit ledger, transport counters, render report) into `inspect_page`
/// focus pages, so every readout stays accessible on a short screen.
// Every parameter is a display input the prompt's readouts require (field, source, edit counts,
// probe, zoom, pending amplitude, status, transport line, the command-dispatch leg) plus the
// terminal geometry and inspector page that select the compact layout.
#[allow(clippy::too_many_arguments)]
fn build_root(
    inbox: &FieldInbox,
    src: Option<u64>,
    ch: &CommandChannel,
    probe: (usize, usize),
    ppc: u16,
    pending_amp: f64,
    status: &str,
    xport: &str,
    cmd_leg: &str,
    term_w: u16,
    term_h: u16,
    inspect_page: usize,
) -> Node {
    let compact = layout::is_compact(term_w, term_h);
    // The non-compact transport line, composed exactly as the pre-compact code did: `status` alone
    // when there is no transport detail (the selfcheck), else `status  |  xport`.
    let status_line = if xport.is_empty() {
        status.to_string()
    } else {
        format!("{status}  |  {xport}")
    };
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
                // The compact verdict essentials: the edit fate every operator needs on-screen at
                // 40×15, spelled with the same `ack N` / `refused N` tokens the full ledger uses.
                let verdict = format!(
                    "ack {} refused {} out {}",
                    ch.acknowledged(),
                    ch.refused(),
                    ch.outstanding()
                );
                // Gate G/D3: derive the display transport from the SAME source geometry the probe
                // indexes — origin (pan), anisotropic cell sizes (per-axis zoom), and ppc
                // magnification — via checked, clamp-before-narrow sizing. A field dimension past
                // u16::MAX saturates to the cap; a nonzero-origin or anisotropic field no longer
                // renders off-view/distorted while the probe reads real values. And a geometry
                // whose zoom overflows to non-finite (D3) has NO invertible mapping: show the
                // still-exact probe value and an explicit notice, never a blank that contradicts it.
                // RXIII Gate D (consumer navigation): the ROOMY path follows the probe with a
                // region-of-interest pan, so a probe navigated past the display budget on a large
                // field stays on-screen — the Gate G correspondence, extended to fields larger than
                // the viewport. The COMPACT path keeps the origin-anchored top-left clip that its
                // certified honest-truncation marker describes (panning would contradict the marker).
                let staged = if compact {
                    viewport::world_view_for(&ext, ppc)
                } else {
                    viewport::world_view_for_probe(&ext, ppc, probe)
                };
                match staged {
                    Ok((wv, area)) => {
                        let frame = view.render(&wv, area, Colormap::Density, RangePolicy::Auto);
                        let r = &frame.report;
                        if compact {
                            // Compact (C1): essentials pinned on top, raster capped below, detail
                            // paged. The render report + geometry live on the `render` inspector
                            // page; `status` alone is the essentials status (bare — the transport
                            // counters are the `xport` page), so a resolved-ack verdict like
                            // "ack: edit resolved → Acknowledged" stays intact at 40 columns.
                            let ess = layout::Essentials {
                                source: format!(
                                    "source {src} t{} {}x{} z{ppc}x",
                                    field.model_tick,
                                    ext.cols(),
                                    ext.rows()
                                ),
                                probe: format!(
                                    "probe ({pix},{piy})={sample_txt} pend {pending_amp:.1}"
                                ),
                                verdict,
                                status: layout::clip(status, term_w),
                            };
                            let render_detail = format!(
                                "render[value {} masked {} nonfinite {} off {} conserves {}]",
                                r.value_cells,
                                r.masked_cells,
                                r.nonfinite_cells,
                                r.off_field_cells,
                                r.conserves(),
                            );
                            let geom_detail = format!(
                                "origin ({:.3},{:.3}) cell {:e}x{:e} zoom {ppc}x",
                                ext.origin().x,
                                ext.origin().y,
                                ext.cell_w(),
                                ext.cell_h(),
                            );
                            let details = [
                                layout::DetailPage {
                                    name: "edits",
                                    lines: vec![hud_edit, cmd_leg.to_string()],
                                },
                                layout::DetailPage {
                                    name: "xport",
                                    lines: vec![if xport.is_empty() {
                                        status_line.clone()
                                    } else {
                                        xport.to_string()
                                    }],
                                },
                                layout::DetailPage {
                                    name: "render",
                                    lines: vec![render_detail, geom_detail],
                                },
                            ];
                            let plan = layout::compact_plan(
                                term_w,
                                term_h,
                                inspect_page,
                                area.height,
                                &ess,
                                &details,
                            );
                            for line in &plan.header {
                                root = root.child(Node::text(line.clone(), Style::default()));
                            }
                            if plan.show_raster {
                                // Re-render into the capped viewport so the field is clipped from
                                // the bottom and the footer (marker + hint) stays on-screen.
                                let capped = gibson::Rect::new(0, 0, area.width, plan.raster_rows);
                                let cframe =
                                    view.render(&wv, capped, Colormap::Density, RangePolicy::Auto);
                                root = root.child(Node::raster(cframe.surface));
                            }
                            for line in &plan.footer {
                                root = root.child(Node::text(line.clone(), Style::default()));
                            }
                            return root;
                        }
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
                        // When the field is larger than the viewport, the ROI pan shows only a
                        // WINDOW of it — so state which columns/rows are on-screen on their own line,
                        // else a panned view silently hides that cells 0.. exist. Omitted entirely
                        // for a field that fits (the window is the whole field), so the ordinary
                        // readout is unchanged. Its own row (starting at column 0) keeps the readout
                        // legible rather than trailing off the end of the long probe line.
                        let (pan_col, pan_row) =
                            viewport::roi_origin(ext.cols(), ext.rows(), area, ppc, probe);
                        let vppc = ppc.max(1) as usize;
                        let view_cols = (area.width as usize / vppc).max(1);
                        let view_rows = (area.height as usize / vppc).max(1);
                        let windowed = ext.cols() > view_cols || ext.rows() > view_rows;
                        let win_line = windowed.then(|| {
                            format!(
                                "window cols [{}..{}) rows [{}..{}) of {}x{}  (arrows pan)",
                                pan_col,
                                (pan_col + view_cols).min(ext.cols()),
                                pan_row,
                                (pan_row + view_rows).min(ext.rows()),
                                ext.cols(),
                                ext.rows(),
                            )
                        });
                        let mut root = root
                            .child(Node::raster(frame.surface))
                            .child(Node::text(hud_src, Style::default()))
                            .child(Node::text(hud_probe, Style::default()));
                        if let Some(win_line) = win_line {
                            root = root.child(Node::text(win_line, Style::default()));
                        }
                        return root
                            .child(Node::text(hud_edit, Style::default()))
                            .child(Node::text(cmd_leg.to_string(), Style::default()))
                            .child(Node::text(status_line, Style::default()));
                    }
                    Err(u) => {
                        if compact {
                            // Compact unsupported-geometry (C3): the essentials still render — the
                            // source line carries the VIEW UNSUPPORTED notice and the probe line the
                            // still-exact sample — never a blank that contradicts the probe. No
                            // raster (the geometry has no invertible mapping); detail paged as usual.
                            let ess = layout::Essentials {
                                source: format!(
                                    "source {src} t{} {}x{} VIEW UNSUPPORTED z{ppc}x",
                                    field.model_tick,
                                    ext.cols(),
                                    ext.rows()
                                ),
                                probe: format!("probe ({pix},{piy})={sample_txt} (exact)"),
                                verdict,
                                status: layout::clip(status, term_w),
                            };
                            let render_detail = format!(
                                "VIEW UNSUPPORTED non-invertible zoom {:e}x{:e}",
                                u.zoom_x, u.zoom_y
                            );
                            let geom_detail = format!(
                                "cell {:e}x{:e} at zoom {ppc}x",
                                ext.cell_w(),
                                ext.cell_h()
                            );
                            let details = [
                                layout::DetailPage {
                                    name: "edits",
                                    lines: vec![hud_edit, cmd_leg.to_string()],
                                },
                                layout::DetailPage {
                                    name: "xport",
                                    lines: vec![if xport.is_empty() {
                                        status_line.clone()
                                    } else {
                                        xport.to_string()
                                    }],
                                },
                                layout::DetailPage {
                                    name: "render",
                                    lines: vec![render_detail, geom_detail],
                                },
                            ];
                            let plan = layout::compact_plan(
                                term_w,
                                term_h,
                                inspect_page,
                                0,
                                &ess,
                                &details,
                            );
                            for line in &plan.header {
                                root = root.child(Node::text(line.clone(), Style::default()));
                            }
                            for line in &plan.footer {
                                root = root.child(Node::text(line.clone(), Style::default()));
                            }
                            return root;
                        }
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
                            .child(Node::text(status_line, Style::default()));
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
    // The DISPLAYED source. It is never bound to a stranger: it becomes `Some(args.source)` only
    // when a valid observation from the configured source actually arrives (A1).
    let mut src: Option<u64> = None;
    // The highest PRODUCER observation epoch seen from the configured source (its own epoch space,
    // distinct from the presenter's command-stamp epoch). A strictly higher epoch is a restart.
    let mut observed_epoch: Option<u32> = None;
    let mut probe: (usize, usize) = (0, 0);
    let mut ppc: u16 = 1;
    let mut pending_amp: f64 = 2.0;
    let mut status = String::from("connected — streaming…");
    let mut disconnected = false;
    // Gate B3 / A1: messages from a source other than the CONFIGURED destination (`args.source`)
    // are out of contract (single-source instrument). Counted and surfaced, never silently stored.
    let mut foreign_source: u64 = 0;
    // RXII Checkpoint 2 / Gate B (C1): the compact inspector page (0 = field view; 1..=3 = edit /
    // transport / render detail), cycled with Tab, and the live terminal geometry that selects the
    // compact layout. Seeded from the real terminal and refreshed on every Resize event.
    let mut inspect_page: usize = 0;
    let (mut term_w, mut term_h) = ctx.session.terminal_size();

    loop {
        let ev = ctx.run_once(Duration::from_millis(16))?;

        // Drain decoded messages from the bounded reader. Snapshots arrive already coalesced to
        // the latest per source; acks and retires are preserved in order; a recoverable decode
        // error was skipped in the reader, a fatal one is surfaced here. The UI never holds an
        // unbounded backlog and never pushes a chunk-burst into the decoder (Gate B 2A/2B).
        let mut batch: Vec<WireMessage> = Vec::new();
        let outcome = owner.reader().drain(&mut |m| batch.push(m));
        for msg in batch {
            // Source identity has ONE authority: the CONFIGURED source `args.source` — NOT whichever
            // source speaks first (A1). Binding the display to the first-observed source was a
            // defect: a custom producer could publish a stranger B before the configured source A
            // ever appeared, and the display would then show B while every command stayed addressed
            // to A — two authorities over identity, disagreeing. Classify against the configured
            // source and the producer epoch observed so far:
            //   * Foreign  — a different source: out of contract for this single-source instrument.
            //     Refuse (never admit / display / resolve), count, surface. A command is never
            //     redirected to it; the display is never reassigned to a stranger.
            //   * Restart  — the configured source at a strictly higher producer epoch than seen so
            //     far (a restart): its field is still A's latest, so it is shown (not fatal) and
            //     surfaced as a restart. Acks still correlate by `command_id`, not epoch, so
            //     pre-restart commands may simply remain outstanding — they are never falsely
            //     resolved, and the restart is not fatal.
            //   * InSession — the configured source at the displayed epoch: admit / display /
            //     resolve normally.
            match input::classify_source(args.source, observed_epoch, msg.source, msg.cursor.epoch)
            {
                SourceVerdict::Foreign => {
                    foreign_source += 1;
                    continue;
                }
                SourceVerdict::Restart => {
                    status = format!(
                        "source {} restarted (epoch {} -> {}) - prior pending edits may be unresolvable",
                        msg.source,
                        observed_epoch.unwrap_or(msg.cursor.epoch),
                        msg.cursor.epoch
                    );
                }
                SourceVerdict::InSession => {}
            }
            // Track the highest producer epoch observed for the configured source.
            observed_epoch =
                Some(observed_epoch.map_or(msg.cursor.epoch, |e| e.max(msg.cursor.epoch)));
            match msg.body {
                WireBody::Field(f) => {
                    inbox.admit(msg.source, msg.cursor, f);
                    // The display becomes visible only once a valid observation from the configured
                    // source actually arrives — never before, and never bound to a stranger.
                    src = Some(args.source);
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

        // Track the live terminal geometry so the compact layout engages at the small-terminal
        // target and re-flows on resize.
        if let Some(Event::Resize(w, h)) = &ev {
            term_w = *w;
            term_h = *h;
        }

        if let Some(Event::Key(k)) = &ev {
            // Input-state reduction is separated from command I/O and rendering so the precedence
            // contract `fatal transport > disconnect > command submission > routine UI input`
            // holds at the boundary (Gate B1). When this iteration latched `disconnected` above,
            // `reduce_key` suppresses every edit and navigation — so no command is admitted or
            // written and the fatal `status` set above survives untouched into the final frame.
            match input::reduce_key(k.code, disconnected, &mut probe, &mut ppc, &mut pending_amp) {
                InputAction::Quit => break,
                // Cycle the compact inspector (C1). Suppressed under a terminal state by
                // `reduce_key` (Tab is matched only on a live transport), so paging keeps the same
                // low precedence as navigation.
                InputAction::Inspect => inspect_page = (inspect_page + 1) % layout::INSPECT_PAGES,
                InputAction::SubmitEdit => {
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
                InputAction::Nothing => {}
            }
            clamp_probe(&inbox, src, &mut probe);
        }

        // L7 telemetry: the reader's lawful snapshot coalescing and any expendable field drops are
        // shown, never hidden — a growing `fields_dropped` is a misbehaving producer, not a leak.
        // Retained FIELD bytes and their high-water are surfaced against the budget so a byte-bound
        // flood is visible as a real memory envelope, not just a message count (Gate C1). A lost
        // *control* verdict is never a counter here; it is the terminal control-overflow above.
        let reader = owner.reader();
        // The transport line, kept SEPARATE from `status` so the compact layout can pin the bare
        // status among the essentials and page the transport counters to the `xport` inspector
        // page. The non-compact layout re-composes them into the identical `status  |  xport` line.
        let xport = format!(
            "xport: coalesced {} fields_dropped {} skips {} wrong-dir {} foreign-src {} bytes {}/{} KiB (hw {} KiB)",
            reader.coalesced(),
            reader.fields_dropped(),
            reader.decode_skips(),
            // Gate B2: inbound COMMAND frames rejected as out of contract (0 on a conformant
            // producer). Surfaced so a hostile producer's direction violation is visible to the
            // operator, not counted invisibly inside the buffer.
            reader.out_of_contract(),
            // Gate B3: messages from a source other than the bound one, refused by the
            // single-source contract (0 on a conformant producer).
            foreign_source,
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
            &status,
            &xport,
            &dispatch_readout(&telemetry),
            term_w,
            term_h,
            inspect_page,
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
                "",
                &dispatch_readout(&telemetry),
                term_w,
                term_h,
                inspect_page,
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
        "",
        &dispatch_readout(&telemetry),
        80,
        30,
        0,
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
        "",
        &dispatch_readout(&telemetry),
        80,
        30,
        0,
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

/// Capture `CAPTURE_FRAMES` field observations from the reference producer in deterministic
/// `--lockstep` (baseline, then one `ADVANCE` per subsequent frame), write them to `path`, and
/// print the content digest (RXII Checkpoint 2 / Gate B, C4). Headless and bounded, reusing the
/// selfcheck's non-blocking reader + wall-clock message wait so a stalled producer fails fast.
fn run_capture(args: &Args, path: &str) -> io::Result<()> {
    let mut child = spawn_producer(args, true)?;
    let mut stdin_w = child.stdin.take().expect("stdin piped");
    let out = child.stdout.take().expect("stdout piped");
    let reader = Reader::spawn(out, transport::DEFAULT_CONTROL_CAP);
    let mut pending: VecDeque<WireMessage> = VecDeque::new();
    // A headless context solely to satisfy `selfcheck_next`'s terminal-restore-on-failure path.
    let mut ctx = Context::headless(RenderMode::Inline, 80, 30);
    let src = args.source;

    let mut frames: Vec<capture::CapturedFrame> = Vec::with_capacity(CAPTURE_FRAMES);
    for i in 0..CAPTURE_FRAMES {
        if i > 0 {
            // Advance the model one tick: a raw control frame, never a tracked edit.
            let advance = WireCommand {
                command_id: 0,
                kind: KIND_ADVANCE,
                arg: Vec::new(),
            };
            let adv = encode_command(src, SourceCursor::new(args.epoch, u64::MAX), &advance);
            if stdin_w
                .write_all(&adv)
                .and_then(|_| stdin_w.flush())
                .is_err()
            {
                selfcheck_fail(
                    &mut child,
                    &mut ctx,
                    "could not send the advance control frame",
                );
            }
        }
        let msg = selfcheck_next(&reader, &mut pending, &mut child, &mut ctx);
        match msg.body {
            WireBody::Field(f) => frames.push(capture::CapturedFrame::from_field(
                msg.source, msg.cursor, &f,
            )),
            other => selfcheck_fail(
                &mut child,
                &mut ctx,
                &format!("capture expected a FIELD, got {other:?}"),
            ),
        }
    }
    drop(stdin_w);
    let _ = child.kill();
    let _ = child.wait();

    let digest = capture::content_digest(&frames);
    let mut file = std::fs::File::create(path)?;
    capture::write_capture(&mut file, &frames)?;
    file.flush()?;
    println!("wire_instrument capture: {} frames -> {path}", frames.len());
    println!("capture-digest {digest:016x}");
    Ok(())
}

/// Replay a capture from `path`: re-render each frame through the real composition path
/// (`FieldView::render → Node::raster → Context`) and print the content digest. The digest
/// reproduces the capture's regardless of the replay terminal's color depth, because it is taken
/// over the field CONTENT, not the rendered bytes (RXII Checkpoint 2 / Gate B, C4).
fn run_replay(_args: &Args, path: &str) -> io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let frames = capture::read_capture(&mut file)?;
    let digest = capture::content_digest(&frames);
    let mut ctx = Context::headless(RenderMode::Inline, 80, 30);
    let mut rendered = 0usize;
    // Gate B1 replay honesty: read_capture has already rejected malformed frames, so a frame that
    // fails to show here is one that parses but cannot be DISPLAYED (a non-invertible display
    // geometry — the D3 UnsupportedView case). Record it explicitly; never drop it silently.
    let mut unsupported: Vec<usize> = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        let mut shown = false;
        if let Some(ext) = f.extent() {
            if let Some(view) = FieldView::new(ext, &f.data, f.mask.as_deref()) {
                if let Ok((wv, area)) = viewport::world_view_for(&ext, 1) {
                    let frame = view.render(&wv, area, Colormap::Density, RangePolicy::Auto);
                    ctx.set_root(Node::raster(frame.surface));
                    ctx.render()?;
                    rendered += 1;
                    shown = true;
                }
            }
        }
        if !shown {
            unsupported.push(i);
        }
    }
    // The checksum is over content and reproduces regardless of what rendered, so print it either way.
    println!("replay-digest {digest:016x}");
    if unsupported.is_empty() {
        println!(
            "wire_instrument replay: {} frames from {path} ({rendered} rendered)",
            frames.len()
        );
        Ok(())
    } else {
        // An explicit partial/unsupported outcome — NOT a success receipt (Gate B1).
        eprintln!(
            "wire_instrument replay INCOMPLETE: {rendered}/{} rendered; unsupported display geometry at frames {unsupported:?}",
            frames.len()
        );
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "replay incomplete: {} of {} frames have an unsupported display geometry",
                unsupported.len(),
                frames.len()
            ),
        ))
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    if let Some(path) = args.replay.clone() {
        run_replay(&args, &path)
    } else if let Some(path) = args.capture.clone() {
        run_capture(&args, &path)
    } else if args.selfcheck {
        run_selfcheck(&args)
    } else {
        run_interactive(&args)
    }
}
