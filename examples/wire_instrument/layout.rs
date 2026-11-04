// Shared by the example (`main.rs`) and by `tests/wire_instrument_layout.rs` via `#[path]`; like
// the sibling `viewport`/`input` modules it depends only on `std`, so it stays independently
// includable. Quiet dead_code for the whole module.
#![allow(dead_code)]
//! Compact (small-terminal) layout planning for `wire_instrument` (RXII Checkpoint 2 / Gate B, C1).
//!
//! At the 40×15 monochrome target the field raster plus the full-width HUD lines wrap past a
//! 15-row screen and push the edit-verdict and transport readouts off the bottom — the exact defect
//! `tests/wire_instrument_pty.rs::pty_full_workflow_at_40x15_mono` catches (RED: the `ack N` /
//! `refused N` receipts are sheared off the 15 rows). This module is the pure, PTY-free planner the
//! loop's `build_root` uses in compact mode:
//!
//!   * the ESSENTIALS (source identity, probe value, edit verdict, transport status) are pinned at
//!     the TOP, so they can never be pushed off — on any inspector page, at any size;
//!   * the field raster is capped to the rows left after the essentials + hint, with an HONEST
//!     truncation marker when the field does not fit (C3 partial-visibility honesty);
//!   * the full detail (edit ledger, transport counters, render report) is paged into
//!     focus/inspector pages cycled with `Tab` (C1 "every readout accessible via focus/inspector
//!     pages"), so nothing is dropped — only relocated one page-flip away.
//!
//! The width budgeting reuses the merged `instrument_workbench` compact idiom (#140): `wrapped_rows`
//! measures how many terminal rows a readout occupies at the real width.

/// Number of inspector pages the compact layout cycles through with `Tab`: page 0 is the field view
/// (raster shown); pages 1..=3 each surface one detail group (edits, transport, render). Kept in
/// sync with the `detail_pages` the loop builds, and used by the loop to cycle `inspect_page`.
pub const INSPECT_PAGES: usize = 4; // field + {edits, xport, render}

/// The compact layout engages at the small-terminal target — the merged workbench threshold (#140):
/// a screen shorter than 18 rows or narrower than 60 columns. At larger sizes the classic full
/// layout (raster over five full-width HUD lines) is used unchanged.
pub const fn is_compact(w: u16, h: u16) -> bool {
    h < 18 || w < 60
}

/// The ESSENTIALS: the readouts that must stay visible at every terminal size and on every
/// inspector page — source identity, probe value, the edit verdict (`ack`/`refused`/`outstanding`),
/// and the transport status. Each is built short enough to fit the compact width on one row.
pub struct Essentials {
    pub source: String,
    pub probe: String,
    pub verdict: String,
    pub status: String,
}

impl Essentials {
    fn lines(&self) -> Vec<String> {
        vec![
            self.source.clone(),
            self.probe.clone(),
            self.verdict.clone(),
            self.status.clone(),
        ]
    }
}

/// One inspector detail page: a short name (shown in the `Tab` hint) and its full-fidelity lines.
pub struct DetailPage {
    pub name: &'static str,
    pub lines: Vec<String>,
}

/// The assembled compact layout: `header` (the essentials, ALWAYS first), an optional capped raster
/// (page 0 only — `show_raster` with `raster_rows`), then `footer` (a truncation marker when the
/// raster is clipped, the detail lines on an inspector page, and the `Tab` hint). `build_root`
/// renders header text, then the capped raster if any, then footer text — so the essentials lead and
/// the field is clipped from the bottom, never the readouts.
pub struct CompactPlan {
    pub header: Vec<String>,
    pub show_raster: bool,
    pub raster_rows: u16,
    pub footer: Vec<String>,
}

/// Plan the compact layout for `page` on a `w`×`h` terminal, given the field raster's full height
/// and the detail pages. `page` is taken modulo `1 + details.len()` (defensive). Page 0 shows the
/// raster capped under the essentials + hint budget — with a truncation marker when the field does
/// not fit; pages 1.. show one detail group IN PLACE of the raster. The essentials are always the
/// header, so no required readout is ever pushed off a short screen.
pub fn compact_plan(
    w: u16,
    h: u16,
    page: usize,
    raster_full_rows: u16,
    ess: &Essentials,
    details: &[DetailPage],
) -> CompactPlan {
    let pages = 1 + details.len();
    let page = page % pages;
    let header = ess.lines();
    let mut footer: Vec<String> = Vec::new();

    if page == 0 {
        // Field view: cap the raster to the rows left after the essentials + hint actually wrap at
        // this width, reserving one row for an honest truncation marker when the field overflows.
        let hint = "[Tab] inspect".to_string();
        let header_rows: usize = header.iter().map(|l| wrapped_rows(l, w)).sum();
        let hint_rows = wrapped_rows(&hint, w);
        let margin = 1usize;
        let avail = (h as usize).saturating_sub(header_rows + hint_rows + margin);
        let full = raster_full_rows as usize;
        let rows = if full <= avail {
            full
        } else {
            avail.saturating_sub(1).max(1)
        };
        if rows < full {
            // The field is CLIPPED, not silently dropped — say so (C3 honesty).
            footer.push(format!("… field {rows}/{full} rows — enlarge to see all …"));
        }
        footer.push(hint);
        CompactPlan {
            header,
            show_raster: true,
            raster_rows: rows as u16,
            footer,
        }
    } else {
        // Inspector page: the full detail group replaces the raster; the essentials still lead.
        let d = &details[page - 1];
        let hint = format!("[Tab] inspect {}/{} {}", page, details.len(), d.name);
        footer.extend(d.lines.iter().cloned());
        footer.push(hint);
        CompactPlan {
            header,
            show_raster: false,
            raster_rows: 0,
            footer,
        }
    }
}

/// Count the terminal rows a string occupies when soft-wrapped at width `w` on whitespace
/// boundaries (a word longer than `w` hard-breaks). Used only to budget the compact layout so the
/// essentials are never pushed off a short screen (#140 / L8). Mirrors the workbench helper.
pub fn wrapped_rows(s: &str, w: u16) -> usize {
    let w = (w as usize).max(1);
    let mut rows = 1usize;
    let mut col = 0usize;
    for word in s.split_whitespace() {
        let wl = word.chars().count().max(1);
        if col == 0 {
            rows += (wl - 1) / w;
            col = ((wl - 1) % w) + 1;
        } else if col + 1 + wl <= w {
            col += 1 + wl;
        } else {
            rows += 1 + (wl - 1) / w;
            col = ((wl - 1) % w) + 1;
        }
    }
    rows
}

/// Truncate `s` to at most `w` display columns (counted in `char`s), appending an ellipsis when it
/// is cut, so a long status line never soft-wraps and disturbs the compact row budget.
pub fn clip(s: &str, w: u16) -> String {
    let w = w as usize;
    if s.chars().count() <= w {
        return s.to_string();
    }
    if w == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(w - 1).collect();
    out.push('…');
    out
}
