//! Ownership-Duel mode: replay the REAL process-global terminal-lease contract
//! (issue #11, `src/session.rs`) through a TERMINAL OWNERSHIP panel. Same
//! discipline as Ghost-Key: this is a pure replay of captured evidence, not a
//! live subprocess inside `--dump`. There is no process here to drive.
//!
//! The fixture (`docs/fixtures/ownership/duel.tsv`) was captured by running
//! `examples/terminal_ownership_probe.rs`'s `duel-trace` scenario under a
//! REAL PTY (`script -qec './target/release/examples/terminal_ownership_probe
//! duel-trace' /tmp/duel-capture.typescript`, then stripped of the shell's
//! ANSI reset bytes and `script`'s own header/footer lines — see the doc
//! comment on `duel_trace()` in that example for exactly what it measures).
//! Two `TerminalSession`s really fight over the one process-global lease:
//! ctx#1 acquires, ctx#2 is rejected while ctx#1 still holds it, ctx#1
//! restores, ctx#2 reacquires, teardown. `TerminalSession::lease_state()` is
//! read at each real transition — nothing here is staged.

use gibson::ColorDepth;

use crate::diag::{Category, DiagLog};
use crate::visual::{self, PanelRow, Tone, View};

const DUEL_TSV: &str = include_str!("../../docs/fixtures/ownership/duel.tsv");

pub const LOG_CAP: usize = 256;

#[derive(Clone, Debug)]
struct DuelRecord {
    us: u64,
    kind: String,
    value: String,
}

fn parse_records(text: &str) -> Vec<DuelRecord> {
    text.lines()
        .filter_map(|line| {
            let p: Vec<&str> = line.split('\t').collect();
            if p.len() != 5 {
                return None;
            }
            Some(DuelRecord {
                us: p[1].parse().ok()?,
                kind: p[3].to_string(),
                value: p[4].to_string(),
            })
        })
        .collect()
}

pub struct Snapshot {
    freeze_at_us: u64,
    trace_end_us: u64,
    records_total: usize,
    log: DiagLog,
    ctx1_acquire: Option<DuelRecord>,
    ctx2_reject: Option<DuelRecord>,
    ctx1_restore_begin: Option<DuelRecord>,
    ctx1_restore_result: Option<DuelRecord>,
    ctx2_reacquire: Option<DuelRecord>,
    teardown: Option<DuelRecord>,
    last_lease_state: Option<DuelRecord>,
    witnessed_restoring: bool,
}

/// Loads the embedded fixture, filters to `us <= freeze_at`, and derives the
/// honest per-milestone facts. A milestone with no matching record at or
/// before the freeze point is `None` — never backfilled from what happens
/// later in the trace.
pub fn load(freeze_at: Option<u64>) -> Snapshot {
    let records = parse_records(DUEL_TSV);
    let trace_end_us = records.last().map_or(0, |r| r.us);
    let freeze_at_us = freeze_at.unwrap_or(trace_end_us);
    let filtered: Vec<&DuelRecord> = records.iter().filter(|r| r.us <= freeze_at_us).collect();

    let mut log = DiagLog::new(LOG_CAP);
    for r in &filtered {
        log.push(
            r.us,
            Category::Session,
            r.kind.clone(),
            "duel.tsv:SESSION",
            r.value.clone(),
            None,
        );
    }

    let find_value = |kind: &str, needle: &str| -> Option<DuelRecord> {
        filtered
            .iter()
            .find(|r| r.kind == kind && r.value.starts_with(needle))
            .map(|r| (*r).clone())
    };

    let witnessed_restoring = filtered
        .iter()
        .any(|r| r.kind == "LeaseStateWitness" && r.value == "Restoring");

    Snapshot {
        freeze_at_us,
        trace_end_us,
        records_total: records.len(),
        ctx1_acquire: find_value("Acquire", "ctx1:"),
        ctx2_reject: find_value("Acquire", "ctx2:rejected"),
        ctx1_restore_begin: find_value("RestoreBegin", "ctx1"),
        ctx1_restore_result: find_value("RestoreResult", "ctx1"),
        ctx2_reacquire: find_value("Acquire", "ctx2:ok"),
        teardown: find_value("Teardown", "ctx2"),
        last_lease_state: filtered
            .iter()
            .rev()
            .find(|r| r.kind == "LeaseState")
            .map(|r| (*r).clone()),
        witnessed_restoring,
        log,
    }
}

fn fmt_rec(r: &Option<DuelRecord>, empty: &str) -> String {
    match r {
        Some(r) => format!("t={}us {}", r.us, r.value),
        None => empty.to_string(),
    }
}

pub fn build_view(s: &Snapshot) -> View<'_> {
    let lease_now = s
        .last_lease_state
        .as_ref()
        .map(|r| r.value.rsplit(':').next().unwrap_or(""));

    // The header has to say what the lease IS at this freeze point, not who
    // acquired it most recently — ctx2 reacquiring doesn't mean ctx2 still
    // holds it three records later, after Teardown hands it back. So this
    // reads off `lease_now` (the actual current state), not a milestone flag,
    // except for the one case where "current state" alone is ambiguous: right
    // after a rejection, the lease is still `Owned` same as any normal hold —
    // the conflict itself is only visible by also checking a rejection
    // happened and hasn't been resolved by a restore yet.
    let unresolved_conflict = s.ctx2_reject.is_some() && s.ctx1_restore_result.is_none();
    let (hero_state, hero_tone) = if unresolved_conflict {
        ("OWNERSHIP CONFLICT".to_string(), Tone::Fault)
    } else {
        match lease_now {
            Some("Owned") => {
                let holder = if s.ctx2_reacquire.is_some() {
                    "ctx2"
                } else {
                    "ctx1"
                };
                (format!("LEASE HELD ({holder})"), Tone::Nominal)
            }
            Some("Restoring") => ("LEASE RESTORING".to_string(), Tone::Warn),
            Some("Available") if s.ctx1_acquire.is_some() => {
                ("LEASE RELEASED".to_string(), Tone::Warn)
            }
            _ => ("AWAITING ACQUIRE".to_string(), Tone::Nominal),
        }
    };

    let panel_rows = vec![
        PanelRow::new(
            "Freeze",
            format!("{}/{}us", s.freeze_at_us, s.trace_end_us),
            Tone::Nominal,
        ),
        PanelRow::new(
            "Lease now",
            lease_now.unwrap_or("? unobserved").to_string(),
            Tone::Nominal,
        ),
        PanelRow::new(
            "Records",
            format!("{}/{}", s.log.retained(), s.records_total),
            Tone::Nominal,
        ),
        PanelRow::new(
            "Restoring seen",
            if s.witnessed_restoring {
                "yes"
            } else {
                "? not caught by poller"
            }
            .to_string(),
            Tone::Nominal,
        ),
    ];

    let stage_rows = vec![
        PanelRow::new(
            "CTX1 ACQUIRE",
            fmt_rec(&s.ctx1_acquire, "? not yet acquired"),
            Tone::Nominal,
        ),
        PanelRow::new(
            "CTX2 ACQUIRE",
            fmt_rec(&s.ctx2_reject, "? not yet attempted"),
            if s.ctx2_reject.is_some() {
                Tone::Fault
            } else {
                Tone::Nominal
            },
        ),
        PanelRow::new(
            "CTX1 RESTORE",
            fmt_rec(&s.ctx1_restore_begin, "? not yet begun"),
            Tone::Nominal,
        ),
        PanelRow::new(
            "CTX1 RELEASED",
            fmt_rec(&s.ctx1_restore_result, "? restore not yet complete"),
            if s.ctx1_restore_result.is_some() {
                Tone::Warn
            } else {
                Tone::Nominal
            },
        ),
        PanelRow::new(
            "CTX2 REACQUIRE",
            fmt_rec(&s.ctx2_reacquire, "? not yet reacquired"),
            Tone::Nominal,
        ),
        PanelRow::new(
            "TEARDOWN",
            fmt_rec(&s.teardown, "? not yet torn down"),
            Tone::Nominal,
        ),
    ];

    View {
        monotonic_us: s.freeze_at_us,
        mode_label: "OWNERSHIP-DUEL".to_string(),
        hero_state,
        hero_tone,
        subtitle: format!(
            "mode=ownership-duel fixture=duel.tsv freeze_at={}us",
            s.freeze_at_us
        ),
        panel_title: "TERMINAL OWNERSHIP".to_string(),
        panel_rows,
        stage_rows,
        sparks: Vec::new(),
        log: &s.log,
        footer:
            "RESOURCE/RENDER/OUTPUT/INPUT unobserved by this replay \u{2014} see SUBSYSTEM STATUS."
                .to_string(),
        controls: "--freeze-at US --dump".to_string(),
    }
}

pub fn dump(freeze_at: Option<u64>, width: u16, height: u16, depth: ColorDepth) -> String {
    let snapshot = load(freeze_at);
    let view = build_view(&snapshot);
    let surface = visual::frame(&view, width, height, depth);
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
