//! Ghost-Key mode: replay the REAL crossterm #1126 evidence (PR #18's
//! `docs/fixtures/event-pressure/`) through an EVENT PIPELINE panel instead of
//! a paragraph. This drives nothing — there's no live process to drive. It's
//! a pure, deterministic replay of receipts that were actually recorded on a
//! real PTY, at a real `--freeze-at` timestamp, same idiom as
//! `event_pressure_lab`'s `--replay-trace --freeze-at --dump`.
//!
//! The bug, for anyone skimming: a resize (SIGWINCH) and a key byte share one
//! mio readiness batch — `epoll_wait` hands back BOTH the SIGNAL token and
//! the TTY token in one call. Crossterm looks at the batch, sees the resize,
//! returns `Event::Resize` to the app, and — because epoll is edge-triggered
//! (EPOLLET) — never goes back for the TTY token in that same wake. The byte
//! just sits there. `FIONREAD` says `1` before AND after. Nobody reads it.
//!
//! Two independent instruments back this up, and this file keeps them
//! separate on purpose:
//!   - `crossterm-gate.tsv` / `raw-gate.tsv`: real release-lab application
//!     receipts (PTY/TTY/CROSSTERM/RAW/APP/CHILD lanes), same format
//!     `event_pressure_lab` replays, with real monotonic microseconds.
//!   - `readiness-witness.json`: an independent `strace` capture of the raw
//!     syscalls (`epoll_wait`, `FIONREAD`, `read`) with PIDs/fds/epoch
//!     timestamps redacted. It has NO timestamp on the same clock as the tsv
//!     — the source only preserves syscall *order*, not wall time — so it is
//!     never time-correlated with `--freeze-at`; it's a static reference
//!     block, always shown in full for the crossterm gate, never merged into
//!     a per-tick series it can't actually support.

use gibson::ColorDepth;

use crate::diag::{Category, DiagLog};
use crate::visual::{self, PanelRow, Tone, View};

const CROSSTERM_GATE_TSV: &str =
    include_str!("../../docs/fixtures/event-pressure/crossterm-gate.tsv");
const RAW_GATE_TSV: &str = include_str!("../../docs/fixtures/event-pressure/raw-gate.tsv");

pub const LOG_CAP: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Crossterm,
    Raw,
}

impl Gate {
    fn tsv(self) -> &'static str {
        match self {
            Gate::Crossterm => CROSSTERM_GATE_TSV,
            Gate::Raw => RAW_GATE_TSV,
        }
    }
    fn fixture_name(self) -> &'static str {
        match self {
            Gate::Crossterm => "crossterm-gate.tsv",
            Gate::Raw => "raw-gate.tsv",
        }
    }
    /// The lane name this gate's backend writes its own receipts under.
    fn backend_source(self) -> &'static str {
        match self {
            Gate::Crossterm => "CROSSTERM",
            Gate::Raw => "RAW",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Gate::Crossterm => "crossterm",
            Gate::Raw => "raw",
        }
    }
}

pub fn parse_gate(s: &str) -> Result<Gate, String> {
    match s {
        "crossterm" => Ok(Gate::Crossterm),
        "raw" => Ok(Gate::Raw),
        other => Err(format!("invalid --gate `{other}` (want crossterm|raw)")),
    }
}

/// One line of a `seq\tus\tsource\tkind\tvalue` release-lab trace — the exact
/// same record shape `examples/event_pressure_lab/trace.rs` writes and reads,
/// reimplemented small and local here rather than pulled in wholesale.
#[derive(Clone, Debug)]
struct GateRecord {
    us: u64,
    source: String,
    kind: String,
    value: String,
}

fn parse_records(text: &str) -> Vec<GateRecord> {
    text.lines()
        .filter_map(|line| {
            let p: Vec<&str> = line.split('\t').collect();
            if p.len() != 5 {
                return None;
            }
            Some(GateRecord {
                us: p[1].parse().ok()?,
                source: p[2].to_string(),
                kind: p[3].to_string(),
                value: p[4].to_string(),
            })
        })
        .collect()
}

fn category_for(source: &str) -> Category {
    match source {
        "CHILD" => Category::Session,
        _ => Category::Input,
    }
}

/// Verbatim from `docs/fixtures/event-pressure/readiness-witness.json`, trace
/// `original-resize-key` (source-line numbers and syscall text copied
/// unchanged; only the file's own already-redacted PIDs/fds remain redacted).
/// This is the resize-then-key ordering that matches `crossterm-gate.tsv`.
/// There is no comparably-named witness excerpt for the raw control — the
/// README's summary table states a raw result but ships no syscall trace for
/// it, so Ghost-Key doesn't invent one.
const WITNESS_ORIGINAL_RESIZE_KEY: &[(u32, &str)] = &[
    (16, "ioctl(0>, FIONREAD, [1]) = 0"),
    (
        21,
        "epoll_wait(4, [{events=EPOLLIN, data={u32=1}}, {events=EPOLLIN, data={u32=0}}], 3, 20) = 2",
    ),
    (
        22,
        "ioctl(7>, TIOCGWINSZ, {ws_row=24, ws_col=80}) = 0   // consumes SIGNAL token only",
    ),
    (32, "ioctl(0>, FIONREAD, [1]) = 0   // byte still pending"),
    (37, "epoll_wait(4, [], 3, 20) = 0   // edge-triggered: no re-alert on TTY"),
];

/// A snapshot loaded (and frozen) once. `build_view` only ever reads this —
/// same pure-render contract as Million-Tick.
pub struct Snapshot {
    gate: Gate,
    freeze_at_us: u64,
    trace_end_us: u64,
    records_total: usize,
    log: DiagLog,
    app_key: Option<GateRecord>,
    tty_readable: Option<GateRecord>,
    backend_last: Option<GateRecord>,
    backend_resize: Option<GateRecord>,
    pty_write: Option<GateRecord>,
    verdict: Option<GateRecord>,
}

/// Loads the embedded fixture, filters it to `us <= freeze_at`, and derives
/// the honest per-stage facts. No inference across the freeze boundary: a
/// record after `freeze_at_us` simply isn't in `filtered` and can't leak in.
pub fn load(gate: Gate, freeze_at: Option<u64>) -> Snapshot {
    let records = parse_records(gate.tsv());
    let trace_end_us = records.last().map_or(0, |r| r.us);
    let freeze_at_us = freeze_at.unwrap_or(trace_end_us);
    let filtered: Vec<&GateRecord> = records.iter().filter(|r| r.us <= freeze_at_us).collect();

    let mut log = DiagLog::new(LOG_CAP);
    for r in &filtered {
        log.push(
            r.us,
            category_for(&r.source),
            r.kind.clone(),
            format!("{}:{}", gate.fixture_name(), r.source),
            if r.value.is_empty() {
                "(none)".to_string()
            } else {
                r.value.clone()
            },
            None,
        );
    }

    let find = |source: &str, kind: Option<&str>| -> Option<GateRecord> {
        filtered
            .iter()
            .rev()
            .find(|r| r.source == source && kind.is_none_or(|k| r.kind == k))
            .map(|r| (*r).clone())
    };

    Snapshot {
        gate,
        freeze_at_us,
        trace_end_us,
        records_total: records.len(),
        app_key: find("APP", Some("Key")),
        tty_readable: find("TTY", Some("Readable")),
        backend_last: find(gate.backend_source(), None),
        backend_resize: find(gate.backend_source(), Some("Resize")),
        pty_write: find("PTY", Some("Write")),
        verdict: find("PTY", Some("VerdictWindow")),
        log,
    }
}

fn fmt_rec(r: &Option<GateRecord>, empty: &str) -> String {
    match r {
        Some(r) => format!("t={}us {}", r.us, r.value),
        None => empty.to_string(),
    }
}

pub fn build_view(s: &Snapshot) -> View<'_> {
    // The collision, told honestly from what's actually in the filtered
    // window — never "the app got a key later so the backend must have read
    // it," always "here's the last receipt we actually have."
    let stranded = s.tty_readable.is_some() && s.backend_resize.is_some() && s.app_key.is_none();
    let (hero_state, hero_tone) = if s.app_key.is_some() {
        ("KEY DELIVERED".to_string(), Tone::Nominal)
    } else if stranded {
        ("INPUT STARVED".to_string(), Tone::Fault)
    } else if s.tty_readable.is_some() {
        ("READINESS PENDING".to_string(), Tone::Warn)
    } else {
        ("AWAITING COLLISION".to_string(), Tone::Nominal)
    };

    let panel_rows = vec![
        PanelRow::new("Gate", s.gate.label(), Tone::Nominal),
        PanelRow::new(
            "Freeze",
            format!("{}/{}us", s.freeze_at_us, s.trace_end_us),
            Tone::Nominal,
        ),
        PanelRow::new(
            "Verdict",
            s.verdict
                .as_ref()
                .map_or_else(|| "? not reached yet".to_string(), |r| r.value.clone()),
            if s.verdict.as_ref().is_some_and(|r| r.value == "deadline") {
                Tone::Fault
            } else {
                Tone::Nominal
            },
        ),
        PanelRow::new(
            "Records",
            format!("{}/{}", s.log.retained(), s.records_total),
            Tone::Nominal,
        ),
    ];

    let mut stage_rows = vec![
        PanelRow::new(
            "PTY WRITE",
            fmt_rec(&s.pty_write, "? not written by this freeze point"),
            Tone::Nominal,
        ),
        PanelRow::new(
            "TTY SAMPLE",
            fmt_rec(&s.tty_readable, "? no FIONREAD probe observed yet"),
            if s.tty_readable.is_some() {
                Tone::Warn
            } else {
                Tone::Nominal
            },
        ),
        PanelRow::new(
            format!("{} POLL", s.gate.backend_source()),
            fmt_rec(&s.backend_last, "? backend not probed yet"),
            Tone::Nominal,
        ),
        PanelRow::new(
            "APP",
            fmt_rec(
                &s.app_key,
                "? UNOBSERVED \u{2014} never inferred from a later receipt",
            ),
            if s.app_key.is_some() {
                Tone::Nominal
            } else if stranded {
                Tone::Fault
            } else {
                Tone::Nominal
            },
        ),
    ];

    if s.gate == Gate::Crossterm {
        stage_rows.push(PanelRow::new(
            "\u{2014}",
            "SYSCALL WITNESS (independent strace, no shared clock w/ tsv)",
            Tone::Warn,
        ));
        for (line, syscall) in WITNESS_ORIGINAL_RESIZE_KEY {
            stage_rows.push(PanelRow::new(
                format!("SYS L{line}"),
                *syscall,
                Tone::Nominal,
            ));
        }
        stage_rows.push(PanelRow::new(
            "ABSENCE",
            "no read(0>,...) for the TTY token anywhere in this witness excerpt",
            Tone::Fault,
        ));
    } else {
        stage_rows.push(PanelRow::new(
            "WITNESS",
            "? no strace excerpt keyed to the raw control in readiness-witness.json",
            Tone::Nominal,
        ));
    }

    View {
        monotonic_us: s.freeze_at_us,
        mode_label: "GHOST-KEY".to_string(),
        hero_state,
        hero_tone,
        subtitle: format!(
            "mode=ghost-key gate={} fixture={} freeze_at={}us",
            s.gate.label(),
            s.gate.fixture_name(),
            s.freeze_at_us
        ),
        panel_title: format!(
            "EVENT PIPELINE \u{2014} {} GATE",
            s.gate.label().to_uppercase()
        ),
        panel_rows,
        stage_rows,
        sparks: Vec::new(),
        log: &s.log,
        footer: "RESOURCE/RENDER/OUTPUT unobserved by this replay \u{2014} see SUBSYSTEM STATUS."
            .to_string(),
        controls: "--gate crossterm|raw --freeze-at US --dump".to_string(),
    }
}

pub fn dump(
    gate: Gate,
    freeze_at: Option<u64>,
    width: u16,
    height: u16,
    depth: ColorDepth,
) -> String {
    let snapshot = load(gate, freeze_at);
    let view = build_view(&snapshot);
    // Deterministic replay keeps the hero Braille sparklines (byte-identical).
    let surface = visual::frame(
        &view,
        width,
        height,
        depth,
        gibson::SubcellGlyphMode::Braille2x4,
    );
    render_to_text(&surface, width, height)
}

fn render_to_text(surface: &gibson::Surface, width: u16, height: u16) -> String {
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(surface.get(x, y).unwrap().glyph.grapheme.as_str());
        }
        out.push('\n');
    }
    out
}
