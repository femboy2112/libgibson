//! RXII Checkpoint 2 / Gate B (C1/C3): the compact small-terminal layout planner.
//!
//! The end-to-end 40×15 workflow is proven on a live PTY in `wire_instrument_pty.rs`
//! (`pty_full_workflow_at_40x15_mono`). These are the fast, deterministic, PTY-free proofs of the
//! pure planner `build_root` drives: the ESSENTIALS are always the header (no required readout can
//! be pushed off, C1); the field raster is capped under the essentials+hint budget with an HONEST
//! truncation marker when it does not fit (C3 partial-visibility); the detail is paged into
//! focus/inspector pages (C1 "every readout accessible"); and an UNSUPPORTED-geometry notice in the
//! source essential rides through on every page (C3 unsupported extreme).

#[path = "../examples/wire_instrument/layout.rs"]
mod layout;

use layout::{clip, compact_plan, is_compact, wrapped_rows, DetailPage, Essentials, INSPECT_PAGES};

/// Terminal display columns of a string, via the public API (the same `UnicodeWidthStr`-backed
/// measure the renderer uses). The oracle the width-honesty tests check `clip`/`wrapped_rows`
/// against — a wide CJK glyph is two columns, not one `char`.
fn display_cols(s: &str) -> usize {
    gibson::Span::raw(s).display_width()
}

fn essentials() -> Essentials {
    Essentials {
        source: "source 1 t4 8x4 z1x".to_string(),
        probe: "probe (0,0)=0.0025 pend 2.0".to_string(),
        verdict: "ack 1 refused 1 out 0".to_string(),
        status: "ack: edit resolved → Acknowledged".to_string(),
    }
}

fn details() -> Vec<DetailPage> {
    vec![
        DetailPage {
            name: "edits",
            lines: vec![
                "edits: issued 2 pending 0 outstanding 0 | ack 1 refused 1 failed 0".to_string(),
                "command-dispatch leg: count 2 p50 ... max ...".to_string(),
            ],
        },
        DetailPage {
            name: "xport",
            lines: vec!["xport: coalesced 3 fields_dropped 0 bytes 1/64 KiB".to_string()],
        },
        DetailPage {
            name: "render",
            lines: vec!["render[value 32 masked 0 nonfinite 0 off 0 conserves true]".to_string()],
        },
    ]
}

/// The compact layout engages exactly at the small-terminal target: shorter than 18 rows OR
/// narrower than 60 columns. The 40×15 target is compact; a roomy 80×30 is not.
#[test]
fn compact_engages_only_at_the_small_terminal_target() {
    assert!(is_compact(40, 15), "the 40×15 target is compact");
    assert!(is_compact(59, 24), "narrower than 60 columns is compact");
    assert!(is_compact(100, 17), "shorter than 18 rows is compact");
    assert!(!is_compact(80, 30), "a roomy terminal is not compact");
    assert!(!is_compact(60, 18), "exactly the threshold is not compact");
}

/// The essentials are ALWAYS the header, in order — on the field page AND on every inspector page.
/// This is the core C1 guarantee: source, probe, verdict and status can never be paged away.
#[test]
fn the_essentials_are_always_the_header_on_every_page() {
    let ess = essentials();
    let want = vec![
        ess.source.clone(),
        ess.probe.clone(),
        ess.verdict.clone(),
        ess.status.clone(),
    ];
    for page in 0..INSPECT_PAGES {
        let plan = compact_plan(40, 15, page, 4, &ess, &details());
        assert_eq!(
            plan.header, want,
            "the essentials lead the layout on page {page} — never pushed off"
        );
    }
}

/// Page 0 shows the raster. When the field fits the rows left after the essentials + hint, it is
/// shown whole with no marker; when it does not fit, it is CLIPPED from the bottom to the available
/// rows and an honest `… field N/M rows …` marker is emitted — never silently dropped (C3).
#[test]
fn page_zero_caps_the_raster_and_marks_an_honest_truncation() {
    // A short field fits: shown whole, no marker, footer is just the hint.
    let fits = compact_plan(40, 15, 0, 5, &essentials(), &details());
    assert!(fits.show_raster);
    assert_eq!(fits.raster_rows, 5, "a field that fits is shown whole");
    assert_eq!(
        fits.footer,
        vec!["[Tab] inspect".to_string()],
        "no truncation marker when the field fits"
    );

    // A tall field is clipped: 4 essentials + 1 hint + 1 margin = 6 reserved of 15 → 9 avail; a
    // 20-row field is capped to 8 (one row reserved for the marker) and the marker says so.
    let clipped = compact_plan(40, 15, 0, 20, &essentials(), &details());
    assert!(clipped.show_raster);
    assert_eq!(
        clipped.raster_rows, 8,
        "the raster is capped to the rows left"
    );
    assert_eq!(clipped.footer.len(), 2, "marker + hint");
    assert!(
        clipped.footer[0].contains("8/20"),
        "the marker states shown/total rows honestly: {:?}",
        clipped.footer[0]
    );
    assert!(clipped.footer[0].contains("enlarge"));
    assert_eq!(clipped.footer[1], "[Tab] inspect");
}

/// An inspector page (1..=3) replaces the raster with one detail group, keeps the essentials as the
/// header, and labels the hint with the page index and name — so the full ledger / transport /
/// render detail is one `Tab` away and nothing is lost, only relocated (C1).
#[test]
fn an_inspector_page_shows_its_detail_and_hides_the_raster() {
    let d = details();
    let plan = compact_plan(40, 15, 1, 4, &essentials(), &d);
    assert!(!plan.show_raster, "the detail replaces the raster");
    // footer = the page's detail lines, then the hint.
    assert_eq!(plan.footer[0], d[0].lines[0]);
    assert_eq!(plan.footer[1], d[0].lines[1]);
    assert_eq!(
        plan.footer.last().unwrap(),
        "[Tab] inspect 1/3 edits",
        "the hint names the page index and group"
    );

    // Page 3 is the render group.
    let p3 = compact_plan(40, 15, 3, 4, &essentials(), &d);
    assert_eq!(p3.footer[0], d[2].lines[0]);
    assert_eq!(p3.footer.last().unwrap(), "[Tab] inspect 3/3 render");
}

/// The page index wraps modulo the page count, matching the loop's `(inspect_page + 1) %
/// INSPECT_PAGES` cycling — so Tab returns to the field view after the last detail page.
#[test]
fn the_page_index_wraps() {
    let d = details();
    let p_wrap = compact_plan(40, 15, INSPECT_PAGES, 4, &essentials(), &d); // == page 0
    assert!(
        p_wrap.show_raster,
        "page {INSPECT_PAGES} wraps back to the field view"
    );
    let p_wrap1 = compact_plan(40, 15, INSPECT_PAGES + 1, 4, &essentials(), &d); // == page 1
    assert_eq!(p_wrap1.footer.last().unwrap(), "[Tab] inspect 1/3 edits");
}

/// C3 unsupported extreme: when the geometry has no invertible mapping, `build_root` puts a
/// "VIEW UNSUPPORTED" notice in the SOURCE essential. Because the essentials are always the header,
/// that notice (and the still-exact probe) stays visible on every page — never a silent blank.
#[test]
fn the_unsupported_view_notice_rides_through_on_every_page() {
    let mut ess = essentials();
    ess.source = "source 1 t4 4x3 VIEW UNSUPPORTED z1x".to_string();
    ess.probe = "probe (0,0)=1.0000 (exact)".to_string();
    for page in 0..INSPECT_PAGES {
        let plan = compact_plan(40, 15, page, 0, &ess, &details());
        assert!(
            plan.header[0].contains("VIEW UNSUPPORTED"),
            "the unsupported notice stays visible on page {page}"
        );
        assert!(
            plan.header[1].contains("(exact)"),
            "the still-exact probe stays visible on page {page}"
        );
    }
}

/// `wrapped_rows` counts soft-wrapped rows at a width — the budget primitive. A line within the
/// width is one row; a line wider than the width wraps to more.
#[test]
fn wrapped_rows_counts_soft_wrapped_rows() {
    assert_eq!(wrapped_rows("short line", 40), 1);
    assert_eq!(wrapped_rows("", 40), 1, "the empty string is one row");
    // 50 one-char words at width 10: words pack ~5 per row (word + leading space) → >1 row.
    let many = (0..50).map(|_| "x").collect::<Vec<_>>().join(" ");
    assert!(
        wrapped_rows(&many, 10) > 1,
        "a long line wraps to several rows"
    );
}

/// `clip` truncates to the width with an ellipsis, so a long status never soft-wraps and disturbs
/// the compact row budget; a short string is untouched.
#[test]
fn clip_truncates_long_lines_with_an_ellipsis() {
    assert_eq!(clip("short", 40), "short", "a fitting string is untouched");
    let long = "this status line is definitely wider than ten columns";
    let out = clip(long, 10);
    assert_eq!(out.chars().count(), 10, "clipped to exactly the width");
    assert!(out.ends_with('…'), "an ellipsis marks the cut: {out:?}");
}

/// Gate D width honesty. A wide (CJK) glyph is ONE `char` but TWO terminal columns, so a char-count
/// budget under-counts the rows a wide readout occupies and would let it silently wrap past the
/// compact budget. `wrapped_rows` must measure DISPLAY COLUMNS: six CJK glyphs are twelve columns,
/// which overflow a width-10 line onto a second row — a char-count measure (6 ≤ 10) wrongly says one.
#[test]
fn wrapped_rows_measures_display_width_not_char_count() {
    // Six fullwidth CJK glyphs = 12 display columns.
    let cjk = "一二三四五六";
    assert_eq!(
        display_cols(cjk),
        12,
        "sanity: six CJK glyphs are 12 columns"
    );
    assert_eq!(cjk.chars().count(), 6, "sanity: but only six chars");
    assert_eq!(
        wrapped_rows(cjk, 10),
        2,
        "12 display columns overflow a width-10 line onto a second row"
    );
    // A single wide glyph still fits a width-2 line on one row.
    assert_eq!(wrapped_rows("一", 2), 1);
    // ASCII is unchanged (char count == display width).
    assert_eq!(wrapped_rows("short line", 40), 1);
}

/// Gate D width honesty. `clip` must truncate by DISPLAY COLUMNS and never split a wide glyph: a
/// char-count budget (6 CJK chars ≤ 6) would leave a 12-column string in place and blow the row
/// budget. The clipped result must fit the width in COLUMNS and still end in an ellipsis.
#[test]
fn clip_truncates_by_display_width_not_char_count() {
    let cjk = "一二三四五六"; // 12 columns, 6 chars
    let out = clip(cjk, 6);
    assert!(
        display_cols(&out) <= 6,
        "clipped to fit 6 display columns, got {} cols in {out:?}",
        display_cols(&out)
    );
    assert!(out.ends_with('…'), "an ellipsis marks the cut: {out:?}");
    // The content before the ellipsis is whole wide glyphs — no half-glyph was emitted.
    let body: String = out.chars().take_while(|&c| c != '…').collect();
    assert_eq!(
        display_cols(&body) + 1,
        display_cols(&out),
        "the ellipsis is one column and nothing was split"
    );
    // A string that already fits in columns is untouched, even with wide glyphs.
    assert_eq!(
        clip("一二", 4),
        "一二",
        "two CJK glyphs fit exactly in 4 columns"
    );
}
