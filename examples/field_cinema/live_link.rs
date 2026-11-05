// Shared by the `field_cinema` example (`--live`) and by the Gate A/B/C tests via `#[path]`. It
// bridges the REAL cross-process producer to the pure cinema grammar and the six-beat `Arc`, so the
// executable and the oracles drive the *same* live-staging code.
#![allow(dead_code)]
//! The live-wire session (RXIII Checkpoint B / Gate C; reunified with the verified command channel
//! in RXIV Gate B).
//!
//! RXIII's `Link` was an independent raw subprocess channel: `recv_ack()` took the next ACK as the
//! verdict without checking its command id or source, every FIELD was staged under a fixed
//! `source 1 / epoch 1` label, and every irregular frame was an untyped panic over a blocking
//! `stdout.read()`. RXIV Gate B1 executed that weakness (9/9 hostile streams accepted or crashed).
//!
//! [`LiveSession`] instead drives the film through the SAME presentation-side machinery the
//! scientific instrument certified (`examples/wire_instrument`, included by `#[path]`):
//!
//! * `transport::Reader` — a bounded, coalescing, non-blocking background decoder (so every wait
//!   here is a WALL-CLOCK deadline, never a blocking read), with clean-EOF vs read-fault vs
//!   truncated-mid-frame provenance and a fail-closed control lane;
//! * `transport::ProducerOwner` — supervised child ownership: stdin EOF, bounded graceful shutdown,
//!   `kill` + OS `wait` backstop on every exit path including panic unwind;
//! * `channel::CommandChannel` — typed command lifecycle: source-bound, id-correlated ACKs through
//!   the `CommandLedger` (unknown, duplicate, and ack-before-delivery verdicts are rejected),
//!   transport-uncertain deliveries tracked, never retried;
//! * `input::classify_source` — the configured source is the ONLY authority: a foreign source is
//!   counted and never staged or resolving; a strictly higher producer epoch is a restart.
//!
//! Failure is typed ([`LiveError`]) and **fail-closed**: a protocol violation, fatal decode,
//! control overflow, torn write, or a restart while commands are unresolved latches a terminal
//! error every later call returns. Ordered ends (RETIRE, disconnect) are delivered after the frames
//! that preceded them. The producer's stderr is captured (bounded tail) and attached to every
//! disconnect/fatal report rather than discarded.
//!
//! Includers must declare `mod cinema; mod live; mod transport; mod channel; mod input;` (the last
//! three by `#[path]` to `examples/wire_instrument/`) alongside `mod live_link;`.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io::{self, ErrorKind, Read, Write};
use std::process::{ChildStderr, Command, ExitStatus, Stdio};
use std::sync::{Arc as SyncArc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::channel::{CommandChannel, LocalRefusal, ResolveError};
use crate::cinema::{
    compose, field_center, observe, render_shot, view_focused_on, FieldObservation, Shot, SourceId,
};
use crate::input::{classify_source, SourceVerdict};
use crate::live::{ObsSummary, Verdict};
use crate::transport::{ProducerOwner, Reader, DEFAULT_CONTROL_CAP};
use gibson::field_view::{Colormap, FieldView, RangePolicy};
use gibson::surface::Surface;
use gibson::wire::{
    encode_command, AckStatus, SourceCursor, WireBody, WireCommand, WireError, WireField,
    WireMessage,
};
use gibson::Rect;

/// Command kinds the lockstep producer understands (mirrors `bindings/python/producer.py`).
pub const KIND_SET_AMPLITUDE: u32 = 1;
pub const KIND_ADVANCE: u32 = 2;

/// The source the reference film addresses, and the presenter's command-stamp epoch.
pub const LIVE_SOURCE: u64 = 1;
pub const LIVE_EPOCH: u32 = 1;

/// The terminal rectangle every scripted live shot is framed into.
pub const ANCHOR: Rect = Rect {
    x: 0,
    y: 0,
    width: 48,
    height: 16,
};

/// Per-wait wall-clock deadline. A healthy lockstep producer answers in milliseconds; this only
/// bites a producer that stalls or never writes.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(10);

/// Bounded grace for a producer to exit on its own after stdin EOF before it is killed.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// Retained tail of the producer's stderr (bytes).
const STDERR_TAIL: usize = 4096;

/// Admitted-but-unconsumed FIELDs the session retains (oldest dropped and counted past it).
const FIELD_QUEUE_CAP: usize = 64;

/// The Python this build drives the producer with (`GIBSON_PYTHON` overrides `python3`).
pub fn python() -> String {
    std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// How to launch a producer process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerSpec {
    pub program: String,
    pub args: Vec<String>,
}

impl ProducerSpec {
    /// The reference lockstep producer. `freeze` passes `--freeze-amplitude` (ACK edits but never
    /// apply them — the causal mutation control).
    pub fn reference(freeze: bool) -> Self {
        Self::reference_with(freeze, &[])
    }

    /// The reference lockstep producer with extra producer arguments (e.g. `--shape ring`).
    pub fn reference_with(freeze: bool, extra: &[&str]) -> Self {
        let mut args: Vec<String> = [
            "bindings/python/producer.py",
            "--lockstep",
            "--frames",
            "4096",
            "--cols",
            "48",
            "--rows",
            "24",
            "--source",
            "1",
            "--epoch",
            "1",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        if freeze {
            args.push("--freeze-amplitude".into());
        }
        args.extend(extra.iter().map(|s| s.to_string()));
        Self {
            program: python(),
            args,
        }
    }

    /// An arbitrary program (hostile-producer tests).
    pub fn custom(program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// One authoritative FIELD as admitted: the frame's wire identity (source + cursor) and payload.
#[derive(Debug, Clone)]
pub struct LiveField {
    pub source: u64,
    pub cursor: SourceCursor,
    pub field: WireField,
}

/// Why a live-session operation failed. Typed, cloneable, and latched where fail-closed.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveError {
    /// The producer process could not be launched.
    Spawn { program: String, kind: ErrorKind },
    /// No qualifying message arrived before the wall-clock deadline.
    Timeout {
        waiting_for: &'static str,
        elapsed: Duration,
    },
    /// The producer's stream ended. `read_error` = a read fault (vs a clean close);
    /// `truncated_bytes` > 0 = it ended mid-frame; `exit`/`stderr` = the process's own account.
    Disconnected {
        read_error: Option<ErrorKind>,
        truncated_bytes: usize,
        exit: Option<ExitStatus>,
        stderr: String,
    },
    /// A fatal decode poisoned the stream.
    Fatal { error: WireError, stderr: String },
    /// The control lane failed closed: an authoritative verdict could not be retained.
    ControlOverflow,
    /// An in-session ACK the command channel refused (unknown or duplicate id, ack before
    /// delivery, wrong source, non-terminal status). Never resolves, never arms anything.
    Protocol(ResolveError),
    /// The producer retired its source.
    Retired { unresolved: usize },
    /// The producer restarted (strictly higher epoch) while commands were unresolved. Their fate
    /// is unknowable; they are never retried.
    Restarted {
        from: u32,
        to: u32,
        unresolved: usize,
    },
    /// The command was refused locally before anything left the presenter.
    Local(LocalRefusal),
    /// A command/control write failed. `uncertain` = the whole frame was handed off but the flush
    /// failed (a genuine ACK may still reconcile it); otherwise it is known not delivered.
    Transport {
        command_id: Option<u64>,
        uncertain: bool,
        kind: ErrorKind,
    },
}

impl fmt::Display for LiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiveError::Spawn { program, kind } => write!(f, "could not launch {program}: {kind:?}"),
            LiveError::Timeout {
                waiting_for,
                elapsed,
            } => write!(f, "timed out after {elapsed:?} waiting for {waiting_for}"),
            LiveError::Disconnected {
                read_error,
                truncated_bytes,
                exit,
                stderr,
            } => {
                match read_error {
                    Some(k) => write!(f, "producer stream faulted ({k:?})")?,
                    None if *truncated_bytes > 0 => write!(
                        f,
                        "producer stream truncated mid-frame ({truncated_bytes} bytes)"
                    )?,
                    None => write!(f, "producer closed its stream")?,
                }
                if let Some(st) = exit {
                    write!(f, "; exit {st}")?;
                }
                if !stderr.trim().is_empty() {
                    write!(f, "; stderr: {}", stderr.trim())?;
                }
                Ok(())
            }
            LiveError::Fatal { error, stderr } => {
                write!(f, "fatal decode: {error}")?;
                if !stderr.trim().is_empty() {
                    write!(f, "; stderr: {}", stderr.trim())?;
                }
                Ok(())
            }
            LiveError::ControlOverflow => write!(f, "control lane overflow — a verdict was lost"),
            LiveError::Protocol(e) => write!(f, "protocol violation: {e:?}"),
            LiveError::Retired { unresolved } => {
                write!(f, "producer retired its source ({unresolved} unresolved)")
            }
            LiveError::Restarted {
                from,
                to,
                unresolved,
            } => write!(
                f,
                "producer restarted (epoch {from} -> {to}) with {unresolved} unresolved command(s)"
            ),
            LiveError::Local(r) => write!(f, "refused locally: {r:?}"),
            LiveError::Transport {
                command_id,
                uncertain,
                kind,
            } => write!(
                f,
                "{} write failed ({kind:?}){}",
                command_id.map_or("control".to_string(), |id| format!("command {id}")),
                if *uncertain {
                    " — delivery uncertain"
                } else {
                    " — not delivered"
                }
            ),
        }
    }
}

/// Provenance counters the session accumulates (all monotonic; 0 on a conformant producer except
/// `coalesced`, which a streaming producer legitimately raises).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiveCounters {
    /// Messages from a source other than the configured one (never staged, never resolving).
    pub foreign: u64,
    /// FIELDs older than (or equal to) the last admitted cursor, or from a lower epoch.
    pub stale: u64,
    /// Restarts observed (strictly higher producer epoch).
    pub restarts: u64,
    /// Admitted FIELDs dropped from the session queue at its cap.
    pub fields_dropped: u64,
    /// Recoverable malformed frames the decoder skipped (from the reader).
    pub decode_skips: u64,
    /// Wrong-direction COMMAND frames from the producer (from the reader).
    pub out_of_contract: u64,
    /// Consecutive same-source FIELDs the reader lawfully coalesced (from the reader).
    pub coalesced: u64,
}

/// What a finished session reports about its producer.
#[derive(Debug, Clone)]
pub struct SessionEnd {
    /// The reaped exit status (`None` for an in-memory pipe session).
    pub exit: Option<ExitStatus>,
    /// The producer exited on its own within the grace period after stdin EOF.
    pub graceful: bool,
    pub stderr: String,
    pub counters: LiveCounters,
}

/// A bounded tail of a child's stderr, collected on a background thread.
struct StderrTail {
    buf: SyncArc<Mutex<VecDeque<u8>>>,
    handle: Option<JoinHandle<()>>,
}

impl StderrTail {
    fn spawn(stderr: Option<ChildStderr>) -> Self {
        let buf = SyncArc::new(Mutex::new(VecDeque::new()));
        let handle = stderr.map(|mut s| {
            let sink = SyncArc::clone(&buf);
            thread::spawn(move || {
                let mut chunk = [0u8; 1024];
                while let Ok(n) = s.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    let mut b = sink.lock().unwrap();
                    b.extend(&chunk[..n]);
                    while b.len() > STDERR_TAIL {
                        b.pop_front();
                    }
                }
            })
        });
        Self { buf, handle }
    }

    fn text(&self) -> String {
        let b = self.buf.lock().unwrap();
        let bytes: Vec<u8> = b.iter().copied().collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn join(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for StderrTail {
    fn drop(&mut self) {
        self.join();
    }
}

/// The two transports a session can run over. Field order is drop order: the producer is reaped
/// (closing its stderr) before the stderr collector is joined.
enum Io {
    Process {
        owner: ProducerOwner,
        stderr: StderrTail,
    },
    Pipe {
        reader: Reader,
        writer: Box<dyn Write + Send>,
    },
}

/// A live producer session over the certified presentation-side transport.
pub struct LiveSession {
    io: Io,
    channel: CommandChannel,
    dest: u64,
    deadline: Duration,
    fields: VecDeque<LiveField>,
    verdicts: HashMap<u64, AckStatus>,
    observed_epoch: Option<u32>,
    last_cursor: Option<SourceCursor>,
    /// Fail-closed terminal error: every later call returns it.
    terminal: Option<LiveError>,
    /// Ordered end (RETIRE / disconnect): returned once the frames before it are consumed.
    ended: Option<LiveError>,
    counters: LiveCounters,
}

impl LiveSession {
    /// Launch `spec` as a supervised producer process addressed as source `dest`.
    pub fn spawn(spec: &ProducerSpec, dest: u64) -> Result<Self, LiveError> {
        let mut child = Command::new(&spec.program)
            .args(&spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| LiveError::Spawn {
                program: spec.program.clone(),
                kind: e.kind(),
            })?;
        let stdin = child.stdin.take();
        let stderr = StderrTail::spawn(child.stderr.take());
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(LiveError::Spawn {
                program: spec.program.clone(),
                kind: ErrorKind::BrokenPipe,
            });
        };
        let reader = Reader::spawn(stdout, DEFAULT_CONTROL_CAP);
        let owner = ProducerOwner::new(child, stdin, reader);
        Ok(Self::with_io(Io::Process { owner, stderr }, dest))
    }

    /// A session over in-memory byte streams (the hostile-producer harness).
    pub fn over_pipes(
        producer_out: impl Read + Send + 'static,
        producer_in: impl Write + Send + 'static,
        dest: u64,
    ) -> Self {
        let reader = Reader::spawn(producer_out, DEFAULT_CONTROL_CAP);
        Self::with_io(
            Io::Pipe {
                reader,
                writer: Box::new(producer_in),
            },
            dest,
        )
    }

    fn with_io(io: Io, dest: u64) -> Self {
        Self {
            io,
            channel: CommandChannel::new(dest, LIVE_EPOCH),
            dest,
            deadline: DEFAULT_DEADLINE,
            fields: VecDeque::new(),
            verdicts: HashMap::new(),
            observed_epoch: None,
            last_cursor: None,
            terminal: None,
            ended: None,
            counters: LiveCounters::default(),
        }
    }

    /// Override the per-wait wall-clock deadline.
    pub fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }

    /// The producer's OS pid (process sessions only).
    pub fn producer_pid(&self) -> Option<u32> {
        match &self.io {
            Io::Process { owner, .. } => Some(owner.child_id()),
            Io::Pipe { .. } => None,
        }
    }

    pub fn counters(&self) -> LiveCounters {
        self.counters
    }

    /// The command channel (read-only telemetry: outstanding, acknowledged, conservation, …).
    pub fn channel(&self) -> &CommandChannel {
        &self.channel
    }

    /// The latched terminal error, if the session has failed closed.
    pub fn terminal(&self) -> Option<&LiveError> {
        self.terminal.as_ref()
    }

    /// The current tail of the producer's stderr (process sessions only).
    pub fn stderr_tail(&self) -> String {
        match &self.io {
            Io::Process { stderr, .. } => stderr.text(),
            Io::Pipe { .. } => String::new(),
        }
    }

    fn reader(&self) -> &Reader {
        match &self.io {
            Io::Process { owner, .. } => owner.reader(),
            Io::Pipe { reader, .. } => reader,
        }
    }

    /// Drain everything the reader has decoded and classify it, in stream order. Returns whether
    /// any message was processed.
    fn pump(&mut self) -> bool {
        if self.terminal.is_some() {
            return false;
        }
        let mut batch = Vec::new();
        let outcome = self.reader().drain(&mut |m| batch.push(m));
        let any = !batch.is_empty();
        for msg in batch {
            self.process(msg);
            if self.terminal.is_some() {
                return any;
            }
        }
        self.counters.decode_skips = outcome.decode_skips;
        self.counters.out_of_contract = self.reader().out_of_contract();
        self.counters.coalesced = self.reader().coalesced();
        if outcome.control_overflow.is_some() {
            self.terminal = Some(LiveError::ControlOverflow);
        } else if let Some(error) = outcome.fatal {
            self.terminal = Some(LiveError::Fatal {
                error,
                stderr: self.settled_stderr(),
            });
        } else if outcome.disconnected && self.ended.is_none() {
            let exit = self.settled_exit();
            self.ended = Some(LiveError::Disconnected {
                read_error: outcome.read_error,
                truncated_bytes: outcome.partial_frame_bytes,
                exit,
                stderr: self.settled_stderr(),
            });
        }
        any
    }

    /// The producer's exit status, allowing it a brief bounded moment to finish exiting after its
    /// stdout closed.
    fn settled_exit(&mut self) -> Option<ExitStatus> {
        let Io::Process { owner, .. } = &mut self.io else {
            return None;
        };
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            if let Ok(Some(st)) = owner.try_wait() {
                return Some(st);
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// The stderr tail, letting the collector finish if the producer has exited.
    fn settled_stderr(&mut self) -> String {
        if let Io::Process { owner, stderr } = &mut self.io {
            if matches!(owner.try_wait(), Ok(Some(_))) {
                stderr.join();
            }
            return stderr.text();
        }
        String::new()
    }

    fn process(&mut self, msg: WireMessage) {
        match classify_source(self.dest, self.observed_epoch, msg.source, msg.cursor.epoch) {
            SourceVerdict::Foreign => {
                self.counters.foreign += 1;
                return;
            }
            SourceVerdict::Restart => {
                self.counters.restarts += 1;
                let unresolved = self.channel.outstanding();
                if unresolved > 0 {
                    self.terminal = Some(LiveError::Restarted {
                        from: self.observed_epoch.unwrap_or(0),
                        to: msg.cursor.epoch,
                        unresolved,
                    });
                    return;
                }
                // No command in flight: a restart is a display signal. Adopt the new epoch.
                self.observed_epoch = Some(msg.cursor.epoch);
                self.last_cursor = None;
            }
            SourceVerdict::InSession => {
                if matches!(self.observed_epoch, Some(seen) if msg.cursor.epoch < seen) {
                    self.counters.stale += 1;
                    return;
                }
                if self.observed_epoch.is_none() {
                    self.observed_epoch = Some(msg.cursor.epoch);
                }
            }
        }
        match msg.body {
            WireBody::Field(field) => {
                if matches!(self.last_cursor, Some(last) if msg.cursor <= last) {
                    self.counters.stale += 1;
                    return;
                }
                self.last_cursor = Some(msg.cursor);
                if self.fields.len() >= FIELD_QUEUE_CAP {
                    self.fields.pop_front();
                    self.counters.fields_dropped += 1;
                }
                self.fields.push_back(LiveField {
                    source: msg.source,
                    cursor: msg.cursor,
                    field,
                });
            }
            WireBody::Ack(ack) => match self.channel.resolve(msg.source, ack) {
                Ok(_) => {
                    self.verdicts.insert(ack.command_id, ack.status);
                }
                Err(e) => self.terminal = Some(LiveError::Protocol(e)),
            },
            WireBody::Retire => {
                if self.ended.is_none() {
                    self.ended = Some(LiveError::Retired {
                        unresolved: self.channel.outstanding(),
                    });
                }
            }
            WireBody::Command(_) => self.counters.out_of_contract += 1,
        }
    }

    /// Wait (bounded) until `ready` yields, pumping the reader. Terminal errors win; ordered ends
    /// are returned only once nothing earlier is pending.
    fn wait_for<T>(
        &mut self,
        waiting_for: &'static str,
        mut ready: impl FnMut(&mut Self) -> Option<T>,
    ) -> Result<T, LiveError> {
        let start = Instant::now();
        loop {
            let progressed = self.pump();
            if let Some(t) = &self.terminal {
                return Err(t.clone());
            }
            if let Some(v) = ready(self) {
                return Ok(v);
            }
            if let Some(e) = &self.ended {
                return Err(e.clone());
            }
            let elapsed = start.elapsed();
            if elapsed >= self.deadline {
                return Err(LiveError::Timeout {
                    waiting_for,
                    elapsed,
                });
            }
            if !progressed {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    /// The next admitted authoritative FIELD from the configured source.
    pub fn next_field(&mut self) -> Result<LiveField, LiveError> {
        self.wait_for("a FIELD", |s| s.fields.pop_front())
    }

    /// The newest admitted FIELD, discarding older queued ones (streaming consumers).
    pub fn latest_field(&mut self) -> Result<Option<LiveField>, LiveError> {
        self.pump();
        if let Some(t) = &self.terminal {
            return Err(t.clone());
        }
        let latest = self.fields.drain(..).next_back();
        if latest.is_none() {
            if let Some(e) = &self.ended {
                return Err(e.clone());
            }
        }
        Ok(latest)
    }

    /// Admit and deliver a tracked edit through the command channel. Anything already decoded is
    /// classified FIRST, so an ACK that precedes this command can never be mistaken for its
    /// verdict.
    pub fn submit(&mut self, kind: u32, arg: Vec<u8>) -> Result<u64, LiveError> {
        self.pump();
        if let Some(t) = &self.terminal {
            return Err(t.clone());
        }
        if let Some(fault) = self.channel.fault() {
            return Err(LiveError::Transport {
                command_id: Some(fault.command_id),
                uncertain: self.channel.transport_uncertain() > 0,
                kind: ErrorKind::BrokenPipe,
            });
        }
        let id = self
            .channel
            .issue_tracked(kind, arg)
            .map_err(LiveError::Local)?;
        let uncertain_before = self.channel.transport_uncertain();
        let delivered = match &mut self.io {
            Io::Process { owner, .. } => match owner.stdin_mut() {
                Some(w) => self.channel.deliver_next(w),
                None => Err(io::Error::new(
                    ErrorKind::BrokenPipe,
                    "producer stdin closed",
                )),
            },
            Io::Pipe { writer, .. } => self.channel.deliver_next(writer),
        };
        match delivered {
            Ok(Some(d)) => Ok(d),
            Ok(None) => unreachable!("an issued command is always pending for delivery"),
            Err(e) => {
                let uncertain = self.channel.transport_uncertain() > uncertain_before;
                let err = LiveError::Transport {
                    command_id: Some(id),
                    uncertain,
                    kind: e.kind(),
                };
                if !uncertain {
                    // Known not delivered: the outbound stream may carry a torn frame.
                    self.terminal = Some(err.clone());
                }
                Err(err)
            }
        }
    }

    /// Request a set-amplitude edit.
    pub fn submit_amplitude(&mut self, v: f64) -> Result<u64, LiveError> {
        self.submit(KIND_SET_AMPLITUDE, v.to_le_bytes().to_vec())
    }

    /// The producer's verdict for `command_id`, correlated through the ledger.
    pub fn await_verdict(&mut self, command_id: u64) -> Result<AckStatus, LiveError> {
        self.wait_for("the command's ACK", |s| s.verdicts.remove(&command_id))
    }

    /// A verdict already resolved for `command_id`, without waiting (streaming consumers).
    pub fn poll_verdict(&mut self, command_id: u64) -> Result<Option<AckStatus>, LiveError> {
        self.pump();
        if let Some(t) = &self.terminal {
            return Err(t.clone());
        }
        Ok(self.verdicts.remove(&command_id))
    }

    /// Lockstep control: advance the model one tick. Never tracked by the ledger (never acked).
    /// A failed write tears the outbound stream and fails the session closed.
    pub fn advance(&mut self) -> Result<(), LiveError> {
        if let Some(t) = &self.terminal {
            return Err(t.clone());
        }
        let frame = encode_command(
            self.dest,
            SourceCursor::new(LIVE_EPOCH, u64::MAX),
            &WireCommand {
                command_id: 0,
                kind: KIND_ADVANCE,
                arg: Vec::new(),
            },
        );
        let res = match &mut self.io {
            Io::Process { owner, .. } => match owner.stdin_mut() {
                Some(w) => w.write_all(&frame).and_then(|_| w.flush()),
                None => Err(io::Error::new(
                    ErrorKind::BrokenPipe,
                    "producer stdin closed",
                )),
            },
            Io::Pipe { writer, .. } => writer.write_all(&frame).and_then(|_| writer.flush()),
        };
        res.map_err(|e| {
            let err = LiveError::Transport {
                command_id: None,
                uncertain: false,
                kind: e.kind(),
            };
            self.terminal = Some(err.clone());
            err
        })
    }

    /// Advance and return the FIELD it produces.
    pub fn step(&mut self) -> Result<LiveField, LiveError> {
        self.advance()?;
        self.next_field()
    }

    /// Close the session: stdin EOF, bounded graceful wait, kill backstop, reap, join.
    pub fn finish(mut self, grace: Duration) -> SessionEnd {
        self.pump();
        let counters = self.counters;
        match &mut self.io {
            Io::Process { owner, stderr } => {
                let (exit, graceful) = match owner.shutdown(grace) {
                    Ok((st, g)) => (Some(st), g),
                    Err(_) => (None, false),
                };
                stderr.join();
                SessionEnd {
                    exit,
                    graceful,
                    stderr: stderr.text(),
                    counters,
                }
            }
            Io::Pipe { .. } => SessionEnd {
                exit: None,
                graceful: true,
                stderr: String::new(),
                counters,
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Staging: a REAL admitted frame through the unmodified cinema grammar
// ---------------------------------------------------------------------------------------------

/// Stage one admitted frame through the UNMODIFIED cinema grammar: observe → compose → render,
/// attributed to the frame's ACTUAL wire identity (never a fixed label). `None` only if the
/// geometry is not presentable (a decoded frame's geometry is already validated by the decoder).
pub fn stage_live(f: &LiveField) -> Option<(FieldObservation, Shot, Surface)> {
    stage_live_at(f, ANCHOR, field_center(f.field.extent))
}

/// [`stage_live`] into an explicit `anchor`, probing world point `probe`.
pub fn stage_live_at(
    f: &LiveField,
    anchor: Rect,
    probe: gibson::Point,
) -> Option<(FieldObservation, Shot, Surface)> {
    let view = FieldView::new(f.field.extent, &f.field.data, f.field.mask.as_deref())?;
    let source = SourceId(u32::try_from(f.source).unwrap_or(u32::MAX));
    let obs = observe(&view, source, f.cursor.epoch, f.field.model_tick);
    let probe = view.probe(probe);
    let shot = compose(&obs, &probe, f.field.extent);
    let frame = render_shot(&view, &shot, anchor);
    Some((obs, shot, frame.surface))
}

/// A colour-free preview of the shot's framing, rendered with the density ramp so the intensity is
/// legible in ANY terminal — including a piped/non-TTY stream. Not a second render engine: the same
/// `FieldView` and the same shot framing, only the colour-free ramp.
pub fn mono_preview(f: &WireField, shot: &Shot) -> Option<Surface> {
    let view = FieldView::new(f.extent, &f.data, f.mask.as_deref())?;
    let wv = view_focused_on(shot.focus, shot.zoom, ANCHOR);
    Some(
        view.render(&wv, ANCHOR, Colormap::Density, RangePolicy::Auto)
            .surface,
    )
}

/// Bridge a real observation into the arc's presentation-free summary, carrying the producer's
/// wire cursor (its observation chronology — what makes a stale or repeated frame detectable).
pub fn arc_summary(obs: &FieldObservation, cursor: SourceCursor) -> ObsSummary {
    ObsSummary::new(cursor.epoch, cursor.sequence, obs.tick, obs.peak_value)
}

/// Map the producer's correlated wire verdict onto the arc's verdict. `Acknowledged` arms AWAIT;
/// `Refused`/`Failed` do not. `Accepted`/`Delivered` are presenter-local ledger states the
/// `CommandLedger` refuses as an ACK status before this is ever reached, so they map to `Refused`
/// (never arming) rather than panicking.
pub fn verdict_of(status: AckStatus) -> Verdict {
    match status {
        AckStatus::Acknowledged => Verdict::Acknowledged,
        AckStatus::Refused | AckStatus::Failed | AckStatus::Accepted | AckStatus::Delivered => {
            Verdict::Refused
        }
    }
}
