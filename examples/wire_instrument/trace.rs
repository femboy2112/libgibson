// Shared by the example (`main.rs`) and by `tests/wire_instrument_trace.rs` via `#[path]`; like the
// sibling capture/viewport/layout modules it depends only on `std`, so it stays independently
// includable. Quiet dead_code for the whole module.
#![allow(dead_code)]
//! Causal interaction trace (RXIII Checkpoint A / Gate B3).
//!
//! The field-only capture (`capture.rs`) records *what the instrument observed*. It cannot record
//! *why*: two sessions can emit the identical field sequence yet have different interaction
//! histories — e.g. a session that made no edit, and a session whose edit was refused without
//! changing the model. Their field checksums are equal; their causal histories are not. A field
//! checksum is therefore NOT a faithful fingerprint of interaction history.
//!
//! A [`CausalTrace`] is the faithful one: a bounded, versioned, ordered log of the semantic events
//! that actually drove the session — admitted observations (by reference: identity + the field's
//! content checksum, never the bulk field), user probe/focus intent, issued commands with their
//! local admission, producer verdicts, and the terminal outcome. It is deliberately **not** an
//! unbounded event-sourcing subsystem: recording is capped by a [`TraceLimits`] budget and a trace
//! that hits the cap carries an explicit `truncated` marker rather than growing without bound.
//!
//! Two digests are exposed. [`CausalTrace::causal_digest`] fingerprints the whole causal history;
//! [`CausalTrace::field_projection_digest`] fingerprints ONLY the observation references — i.e.
//! exactly the information the field-only capture carries. The B3 tests turn the gap between them
//! into assertions: equal field projection, different causal digest.

use std::io::{self, Read, Write};

// ---- semantic enums ---------------------------------------------------------------------------

/// The local fate of a command at the moment it was issued, before any producer verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Handed to transport and believed delivered.
    Accepted,
    /// Rejected locally (e.g. channel faulted, id exhausted) — never reached transport.
    Rejected,
    /// Written to transport but the flush outcome was uncertain (see Gate D / transport-uncertain).
    TransportUncertain,
}

/// The producer's authoritative verdict on a command — exactly what the producer sent, nothing the
/// presenter inferred. Whether the model actually changed is carried by the OBSERVATIONS that
/// follow, not by this verdict: an `Acknowledged` edit that applied and an `Acknowledged` edit that
/// was silently suppressed share this verdict and are told apart only by the next observation. That
/// is the distinction Gate C3 turns on — an ack is not proof of a model change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The producer acknowledged the command.
    Acknowledged,
    /// The producer explicitly refused the command.
    Refused,
}

/// How the session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeKind {
    /// The user quit cleanly.
    Quit,
    /// The producer disconnected.
    Disconnected,
    /// A fatal transport error terminated the session.
    FatalTransport,
}

/// One semantic step of the session, in logical order.
#[derive(Debug, Clone, PartialEq)]
pub enum TraceEvent {
    /// An authoritative field observation admitted into the session, referenced by identity and its
    /// field content checksum (never the bulk samples — that is the capture's job).
    Observation {
        source: u64,
        epoch: u32,
        sequence: u64,
        model_tick: u64,
        field_checksum: u64,
    },
    /// The user selected a probe / world position.
    Probe { world_x: f64, world_y: f64 },
    /// The user focused an inspector page.
    Focus { page: u32 },
    /// The user issued a command: its id, destination source, kind, and local admission.
    Command {
        command_id: u64,
        dest_source: u64,
        kind: u32,
        admission: Admission,
    },
    /// A producer verdict reconciling a previously issued command.
    Verdict { command_id: u64, verdict: Verdict },
    /// The explicit terminal outcome, with the count of commands still unresolved at that point.
    Outcome {
        kind: OutcomeKind,
        pending_commands: u32,
    },
}

// ---- bounds and errors ------------------------------------------------------------------------

/// Resource budget for recording and reading a trace. A trace is bounded by `max_events`; recording
/// past the cap sets the `truncated` marker instead of growing without bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceLimits {
    pub max_events: usize,
}

impl TraceLimits {
    pub const DEFAULT_MAX_EVENTS: usize = 1 << 16;
}

impl Default for TraceLimits {
    fn default() -> Self {
        TraceLimits {
            max_events: Self::DEFAULT_MAX_EVENTS,
        }
    }
}

/// Why a trace stream was rejected — always a typed, bounded refusal.
#[derive(Debug)]
pub enum TraceError {
    BadMagic,
    EventsExceedBudget { declared: usize, cap: usize },
    InvalidEventTag(u8),
    InvalidEnumTag { field: &'static str, tag: u8 },
    InvalidBooleanByte(u8),
    LengthNotRepresentable(usize),
    Truncated,
    Io(io::Error),
}

impl std::fmt::Display for TraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TraceError::BadMagic => write!(f, "not a wire_instrument trace (bad magic / version)"),
            TraceError::EventsExceedBudget { declared, cap } => {
                write!(f, "declared {declared} events exceeds budget {cap}")
            }
            TraceError::InvalidEventTag(t) => write!(f, "invalid event tag {t}"),
            TraceError::InvalidEnumTag { field, tag } => {
                write!(f, "invalid {field} tag {tag}")
            }
            TraceError::InvalidBooleanByte(b) => write!(f, "invalid boolean byte {b} (want 0/1)"),
            TraceError::LengthNotRepresentable(n) => {
                write!(f, "length {n} does not fit the u32 on-disk field")
            }
            TraceError::Truncated => write!(f, "trace stream truncated"),
            TraceError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for TraceError {}

impl From<io::Error> for TraceError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            TraceError::Truncated
        } else {
            TraceError::Io(e)
        }
    }
}

impl From<TraceError> for io::Error {
    fn from(e: TraceError) -> Self {
        match e {
            TraceError::Io(io) => io,
            other => io::Error::new(io::ErrorKind::InvalidData, other.to_string()),
        }
    }
}

// ---- the trace --------------------------------------------------------------------------------

/// A bounded, ordered log of the session's causal events.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CausalTrace {
    events: Vec<TraceEvent>,
    truncated: bool,
}

impl CausalTrace {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one event if under `limits.max_events`; otherwise set the `truncated` marker and drop
    /// it. Returns whether the event was recorded. This is what keeps the trace bounded.
    pub fn record(&mut self, ev: TraceEvent, limits: &TraceLimits) -> bool {
        if self.events.len() >= limits.max_events {
            self.truncated = true;
            false
        } else {
            self.events.push(ev);
            true
        }
    }

    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// FNV-1a over the WHOLE causal history, in order — the faithful interaction fingerprint.
    pub fn causal_digest(&self) -> u64 {
        let mut h = FNV_OFFSET;
        fnv(&mut h, b"WIRETRC1");
        fnv(&mut h, &(self.events.len() as u64).to_le_bytes());
        fnv(&mut h, &[self.truncated as u8]);
        for ev in &self.events {
            digest_event(&mut h, ev);
        }
        h
    }

    /// FNV-1a over ONLY the observation references, in order — exactly the information the field-only
    /// capture carries. Two histories with equal field projections can still differ causally.
    pub fn field_projection_digest(&self) -> u64 {
        let mut h = FNV_OFFSET;
        let obs = self
            .events
            .iter()
            .filter(|e| matches!(e, TraceEvent::Observation { .. }));
        fnv(&mut h, &(obs.clone().count() as u64).to_le_bytes());
        for e in obs {
            if let TraceEvent::Observation {
                source,
                epoch,
                sequence,
                model_tick,
                field_checksum,
            } = e
            {
                fnv(&mut h, &source.to_le_bytes());
                fnv(&mut h, &epoch.to_le_bytes());
                fnv(&mut h, &sequence.to_le_bytes());
                fnv(&mut h, &model_tick.to_le_bytes());
                fnv(&mut h, &field_checksum.to_le_bytes());
            }
        }
        h
    }
}

// ---- digest + serialization -------------------------------------------------------------------

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(FNV_PRIME);
    }
}

const MAGIC: &[u8; 8] = b"WIRETRC1";

const TAG_OBSERVATION: u8 = 1;
const TAG_PROBE: u8 = 2;
const TAG_FOCUS: u8 = 3;
const TAG_COMMAND: u8 = 4;
const TAG_VERDICT: u8 = 5;
const TAG_OUTCOME: u8 = 6;

fn admission_tag(a: Admission) -> u8 {
    match a {
        Admission::Accepted => 0,
        Admission::Rejected => 1,
        Admission::TransportUncertain => 2,
    }
}
fn admission_from(tag: u8) -> Result<Admission, TraceError> {
    match tag {
        0 => Ok(Admission::Accepted),
        1 => Ok(Admission::Rejected),
        2 => Ok(Admission::TransportUncertain),
        t => Err(TraceError::InvalidEnumTag {
            field: "admission",
            tag: t,
        }),
    }
}
fn verdict_tag(v: Verdict) -> u8 {
    match v {
        Verdict::Acknowledged => 0,
        Verdict::Refused => 1,
    }
}
fn verdict_from(tag: u8) -> Result<Verdict, TraceError> {
    match tag {
        0 => Ok(Verdict::Acknowledged),
        1 => Ok(Verdict::Refused),
        t => Err(TraceError::InvalidEnumTag {
            field: "verdict",
            tag: t,
        }),
    }
}
fn outcome_tag(o: OutcomeKind) -> u8 {
    match o {
        OutcomeKind::Quit => 0,
        OutcomeKind::Disconnected => 1,
        OutcomeKind::FatalTransport => 2,
    }
}
fn outcome_from(tag: u8) -> Result<OutcomeKind, TraceError> {
    match tag {
        0 => Ok(OutcomeKind::Quit),
        1 => Ok(OutcomeKind::Disconnected),
        2 => Ok(OutcomeKind::FatalTransport),
        t => Err(TraceError::InvalidEnumTag {
            field: "outcome",
            tag: t,
        }),
    }
}

fn digest_event(h: &mut u64, ev: &TraceEvent) {
    match ev {
        TraceEvent::Observation {
            source,
            epoch,
            sequence,
            model_tick,
            field_checksum,
        } => {
            fnv(h, &[TAG_OBSERVATION]);
            fnv(h, &source.to_le_bytes());
            fnv(h, &epoch.to_le_bytes());
            fnv(h, &sequence.to_le_bytes());
            fnv(h, &model_tick.to_le_bytes());
            fnv(h, &field_checksum.to_le_bytes());
        }
        TraceEvent::Probe { world_x, world_y } => {
            fnv(h, &[TAG_PROBE]);
            fnv(h, &world_x.to_bits().to_le_bytes());
            fnv(h, &world_y.to_bits().to_le_bytes());
        }
        TraceEvent::Focus { page } => {
            fnv(h, &[TAG_FOCUS]);
            fnv(h, &page.to_le_bytes());
        }
        TraceEvent::Command {
            command_id,
            dest_source,
            kind,
            admission,
        } => {
            fnv(h, &[TAG_COMMAND]);
            fnv(h, &command_id.to_le_bytes());
            fnv(h, &dest_source.to_le_bytes());
            fnv(h, &kind.to_le_bytes());
            fnv(h, &[admission_tag(*admission)]);
        }
        TraceEvent::Verdict {
            command_id,
            verdict,
        } => {
            fnv(h, &[TAG_VERDICT]);
            fnv(h, &command_id.to_le_bytes());
            fnv(h, &[verdict_tag(*verdict)]);
        }
        TraceEvent::Outcome {
            kind,
            pending_commands,
        } => {
            fnv(h, &[TAG_OUTCOME]);
            fnv(h, &[outcome_tag(*kind)]);
            fnv(h, &pending_commands.to_le_bytes());
        }
    }
}

fn len_u32(n: usize) -> Result<u32, TraceError> {
    u32::try_from(n).map_err(|_| TraceError::LengthNotRepresentable(n))
}

/// Serialize a trace to a self-describing little-endian stream. `read_trace` is its exact inverse.
pub fn write_trace<W: Write>(w: &mut W, trace: &CausalTrace) -> Result<(), TraceError> {
    w.write_all(MAGIC)?;
    w.write_all(&len_u32(trace.events.len())?.to_le_bytes())?;
    w.write_all(&[trace.truncated as u8])?;
    for ev in &trace.events {
        match ev {
            TraceEvent::Observation {
                source,
                epoch,
                sequence,
                model_tick,
                field_checksum,
            } => {
                w.write_all(&[TAG_OBSERVATION])?;
                w.write_all(&source.to_le_bytes())?;
                w.write_all(&epoch.to_le_bytes())?;
                w.write_all(&sequence.to_le_bytes())?;
                w.write_all(&model_tick.to_le_bytes())?;
                w.write_all(&field_checksum.to_le_bytes())?;
            }
            TraceEvent::Probe { world_x, world_y } => {
                w.write_all(&[TAG_PROBE])?;
                w.write_all(&world_x.to_bits().to_le_bytes())?;
                w.write_all(&world_y.to_bits().to_le_bytes())?;
            }
            TraceEvent::Focus { page } => {
                w.write_all(&[TAG_FOCUS])?;
                w.write_all(&page.to_le_bytes())?;
            }
            TraceEvent::Command {
                command_id,
                dest_source,
                kind,
                admission,
            } => {
                w.write_all(&[TAG_COMMAND])?;
                w.write_all(&command_id.to_le_bytes())?;
                w.write_all(&dest_source.to_le_bytes())?;
                w.write_all(&kind.to_le_bytes())?;
                w.write_all(&[admission_tag(*admission)])?;
            }
            TraceEvent::Verdict {
                command_id,
                verdict,
            } => {
                w.write_all(&[TAG_VERDICT])?;
                w.write_all(&command_id.to_le_bytes())?;
                w.write_all(&[verdict_tag(*verdict)])?;
            }
            TraceEvent::Outcome {
                kind,
                pending_commands,
            } => {
                w.write_all(&[TAG_OUTCOME])?;
                w.write_all(&[outcome_tag(*kind)])?;
                w.write_all(&pending_commands.to_le_bytes())?;
            }
        }
    }
    Ok(())
}

fn read_n<R: Read, const N: usize>(r: &mut R) -> Result<[u8; N], TraceError> {
    let mut b = [0u8; N];
    r.read_exact(&mut b)?;
    Ok(b)
}
fn read_u32<R: Read>(r: &mut R) -> Result<u32, TraceError> {
    Ok(u32::from_le_bytes(read_n::<_, 4>(r)?))
}
fn read_u64<R: Read>(r: &mut R) -> Result<u64, TraceError> {
    Ok(u64::from_le_bytes(read_n::<_, 8>(r)?))
}
fn read_f64<R: Read>(r: &mut R) -> Result<f64, TraceError> {
    Ok(f64::from_bits(u64::from_le_bytes(read_n::<_, 8>(r)?)))
}

/// Deserialize a trace with the default [`TraceLimits`].
pub fn read_trace<R: Read>(r: &mut R) -> Result<CausalTrace, TraceError> {
    read_trace_with(r, &TraceLimits::default())
}

/// Deserialize a trace under an explicit budget, treating the stream as hostile: the declared event
/// count is checked against `limits` before allocation, and a false count hits `Truncated` on EOF.
pub fn read_trace_with<R: Read>(
    r: &mut R,
    limits: &TraceLimits,
) -> Result<CausalTrace, TraceError> {
    let magic = read_n::<_, 8>(r)?;
    if &magic != MAGIC {
        return Err(TraceError::BadMagic);
    }
    let n = read_u32(r)? as usize;
    if n > limits.max_events {
        return Err(TraceError::EventsExceedBudget {
            declared: n,
            cap: limits.max_events,
        });
    }
    let truncated = match read_n::<_, 1>(r)?[0] {
        0 => false,
        1 => true,
        other => return Err(TraceError::InvalidBooleanByte(other)),
    };
    let mut events = Vec::with_capacity(n);
    for _ in 0..n {
        let ev = match read_n::<_, 1>(r)?[0] {
            TAG_OBSERVATION => TraceEvent::Observation {
                source: read_u64(r)?,
                epoch: read_u32(r)?,
                sequence: read_u64(r)?,
                model_tick: read_u64(r)?,
                field_checksum: read_u64(r)?,
            },
            TAG_PROBE => TraceEvent::Probe {
                world_x: read_f64(r)?,
                world_y: read_f64(r)?,
            },
            TAG_FOCUS => TraceEvent::Focus { page: read_u32(r)? },
            TAG_COMMAND => TraceEvent::Command {
                command_id: read_u64(r)?,
                dest_source: read_u64(r)?,
                kind: read_u32(r)?,
                admission: admission_from(read_n::<_, 1>(r)?[0])?,
            },
            TAG_VERDICT => TraceEvent::Verdict {
                command_id: read_u64(r)?,
                verdict: verdict_from(read_n::<_, 1>(r)?[0])?,
            },
            TAG_OUTCOME => TraceEvent::Outcome {
                kind: outcome_from(read_n::<_, 1>(r)?[0])?,
                pending_commands: read_u32(r)?,
            },
            other => return Err(TraceError::InvalidEventTag(other)),
        };
        events.push(ev);
    }
    Ok(CausalTrace { events, truncated })
}
