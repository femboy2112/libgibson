//! Issue #403: an inline live region after a terminal resize, under the opt-in
//! `InlineResizePolicy::ReclaimLiveRows`. (The default `PreserveAbove` is pinned byte for byte to
//! v0.7.2 by `tests/inline_resize_preserve_above_golden.rs`.)
//!
//! Before any further frame, a resize may already have changed the physical screen. vt100 0.18
//! models xterm here. A shrink first drops blank rows below the cursor, then pushes rows off the
//! top into scrollback. A grow pulls rows back down out of scrollback. Either way the cursor stays
//! on its own line, so the live region's rows that are still on screen sit directly above it,
//! exactly where the renderer last left them relative to the cursor.
//!
//! The contract tested here, through the public `Context` API only:
//!
//! * **Clean live viewport.** After a resize, the next frame leaves no row of an earlier live frame
//!   on the visible screen.
//! * **History preserved.** Committed text survives every resize, every repaint and every
//!   `clear_live_region()`, exactly once, on screen or in scrollback.
//! * **No stale rows the renderer could have reached.** A grow pushes nothing off screen, so
//!   afterwards no stale live row exists anywhere, scrollback included.
//!
//! **Limitation (documented, not a bug).** Rows that the *terminal* pushes into scrollback during a
//! shrink are out of reach of any escape sequence. They stay in scrollback as the last image of
//! that frame. The shrink tests pin that number exactly, so the limitation can't silently grow.
//!
//! Every byte goes through a `vt100` 0.18 virtual terminal with 512 rows of scrollback. The
//! assertions read the reconstructed screen and scrollback, never the renderer's own belief.

use core::num::NonZeroU16;
use gibson::cell::Style;
use gibson::context::{Context, RenderMode};
use gibson::node::Node;
use gibson::InlineResizePolicy;
use std::collections::BTreeSet;

const HISTORY: &str = "SYNTHETIC-HISTORY";

struct Term {
    ctx: Context,
    vt: vt100::Parser,
    cols: u16,
    rows: u16,
    /// Labels of live-frame rows that the *terminal* pushed into scrollback during a resize,
    /// observed between the resize and the next frame, so before the renderer acted at all.
    stranded: BTreeSet<String>,
}

fn nz(v: u16) -> NonZeroU16 {
    NonZeroU16::new(v).expect("non-zero terminal size")
}

impl Term {
    fn new(cols: u16, rows: u16) -> Self {
        Self {
            ctx: {
                let mut ctx = Context::headless(RenderMode::Inline, cols, rows);
                ctx.set_inline_resize_policy(InlineResizePolicy::ReclaimLiveRows);
                ctx
            },
            vt: vt100::Parser::new(nz(rows), nz(cols), 512),
            cols,
            rows,
            stranded: BTreeSet::new(),
        }
    }

    fn pump(&mut self) {
        let out = self.ctx.take_output();
        self.vt.process(out.as_bytes());
    }

    fn commit(&mut self, text: &str) {
        self.ctx.commit_text(text).expect("commit");
        self.pump();
    }

    /// The terminal resizes first, as a real one does before the application hears about it.
    fn resize(&mut self, cols: u16, rows: u16) {
        let before = self.scrollback().len();
        self.vt.set_size(nz(rows), nz(cols));
        self.ctx.resize(cols, rows).expect("resize");
        self.cols = cols;
        self.rows = rows;
        // Only the rows THIS resize pushed (a grow may pull rows back and push none).
        let pushed: Vec<String> = self
            .scrollback()
            .into_iter()
            .skip(before)
            .map(|r| r.trim_end().to_owned())
            .filter(|r| is_any_frame_row(r))
            .collect();
        self.stranded.extend(pushed);
    }

    fn clear_live(&mut self) {
        self.ctx.clear_live_region().expect("clear_live_region");
        self.pump();
    }

    /// A full-height live frame (`rows - 1` labelled rows, the inline maximum).
    fn frame(&mut self, step: usize) {
        self.frame_rows(step, self.rows - 1);
    }

    fn frame_rows(&mut self, step: usize, n: u16) {
        self.ctx.set_root(
            Node::col().children(
                (0..n)
                    .map(|line| Node::text(format!("f{step}-r{line}"), Style::default()))
                    .collect(),
            ),
        );
        self.ctx.render().expect("render");
        self.pump();
    }

    fn visible(&self) -> Vec<String> {
        self.vt.screen().rows(0, self.cols).collect()
    }

    /// Scrollback rows, oldest first.
    fn scrollback(&mut self) -> Vec<String> {
        let cols = self.cols;
        let screen = self.vt.screen_mut();
        screen.set_scrollback(usize::MAX);
        let len = screen.scrollback();
        let mut out = Vec::with_capacity(len);
        for offset in (1..=len).rev() {
            screen.set_scrollback(offset);
            out.push(screen.rows(0, cols).next().unwrap_or_default());
        }
        screen.set_scrollback(0);
        out
    }

    fn transcript(&mut self) -> Vec<String> {
        let mut all = self.scrollback();
        all.extend(self.visible());
        all
    }
}

fn is_frame_row(row: &str, step: usize) -> bool {
    row.trim_end().starts_with(&format!("f{step}-r"))
}

fn is_any_frame_row(row: &str) -> bool {
    let row = row.trim_end();
    row.starts_with('f')
        && row.contains("-r")
        && row[1..].split("-r").all(|p| p.parse::<u32>().is_ok())
}

fn frame_step(row: &str) -> Option<usize> {
    let row = row.trim_end();
    if !is_any_frame_row(row) {
        return None;
    }
    row[1..].split("-r").next()?.parse().ok()
}

fn count(rows: &[String], pred: impl Fn(&str) -> bool) -> usize {
    rows.iter().filter(|r| pred(r)).count()
}

fn history_count(rows: &[String]) -> usize {
    count(rows, |r| r.trim_end() == HISTORY)
}

/// Asserts the live region is clean: from the first row of the current frame down to the bottom
/// of the screen, the current frame's rows appear in order and every other row is blank. No row
/// of an older frame is inside or below the live region.
fn assert_live_region_clean(t: &Term, current: usize, label: &str) {
    let vis = t.visible();
    let screen = || vis.join("\n");
    let first = vis
        .iter()
        .position(|r| frame_step(r) == Some(current))
        .unwrap_or_else(|| panic!("{label}: current frame not visible\nscreen:\n{}", screen()));
    let mut next = 0;
    for row in &vis[first..] {
        let row = row.trim_end();
        if row.is_empty() {
            continue;
        }
        assert_eq!(
            row,
            format!("f{current}-r{next}"),
            "{label}: unexpected row in or below the live region\nscreen:\n{}",
            screen()
        );
        next += 1;
    }
}

/// Asserts the only older-frame rows anywhere (screen or scrollback) are rows the terminal
/// itself pushed out during a resize, each exactly once. The renderer may strand nothing it
/// could reach.
fn assert_only_terminal_stranded_rows(t: &mut Term, current: usize, label: &str) {
    let all = t.transcript();
    let mut seen = BTreeSet::new();
    for row in &all {
        let row = row.trim_end();
        match frame_step(row) {
            Some(s) if s != current => {
                assert!(
                    t.stranded.contains(row),
                    "{label}: stale row {row:?} was reachable by the renderer but survived\n{}",
                    all.join("\n")
                );
                assert!(
                    seen.insert(row.to_owned()),
                    "{label}: stale row {row:?} duplicated"
                );
            }
            _ => {}
        }
    }
}

/// Visible screen holds no row of any older frame at all.
fn assert_no_stale_visible(t: &Term, current: usize, label: &str) {
    let vis = t.visible();
    let stale: Vec<&String> = vis
        .iter()
        .filter(|r| frame_step(r).is_some_and(|s| s != current))
        .collect();
    assert!(
        stale.is_empty(),
        "{label}: stale rows visible: {stale:?}\nscreen:\n{}",
        vis.join("\n")
    );
}

#[test]
fn grow_leaves_no_stale_live_rows_anywhere_and_keeps_history() {
    let mut t = Term::new(40, 15);
    t.commit(HISTORY);
    t.frame(0);
    t.resize(80, 24);
    t.frame(1);

    assert_live_region_clean(&t, 1, "40x15 -> 80x24");
    assert_no_stale_visible(&t, 1, "40x15 -> 80x24");
    assert!(t.stranded.is_empty(), "a grow pushes nothing");
    assert_only_terminal_stranded_rows(&mut t, 1, "40x15 -> 80x24");
    let all = t.transcript();
    assert_eq!(history_count(&all), 1, "history must survive exactly once");
    assert_eq!(
        count(&all, |r| is_frame_row(r, 0)),
        0,
        "a grow pushes nothing off screen, so no stale frame-0 row may exist anywhere:\n{}",
        all.join("\n")
    );
    // History sits directly above the new live region.
    let vis = t.visible();
    let h = vis.iter().position(|r| r.trim_end() == HISTORY);
    let f = vis.iter().position(|r| is_frame_row(r, 1));
    assert_eq!(
        h.map(|h| h + 1),
        f,
        "history must directly precede the live region"
    );
}

#[test]
fn shrink_cleans_the_viewport_keeps_history_and_strands_only_terminal_pushed_rows() {
    let mut t = Term::new(80, 24);
    t.commit(HISTORY);
    t.frame(0); // history on row 0, frame-0 rows 0..=22 on rows 1..=23, cursor on row 23
    t.resize(40, 15);
    // vt100 0.18 (xterm semantics): the cursor is on the bottom row, so the shrink by 9 pushes
    // 9 rows into scrollback: the history row plus frame-0 rows 0..=7.
    let pushed = t.scrollback();
    assert_eq!(
        history_count(&pushed),
        1,
        "precondition: the shrink pushed history"
    );
    assert_eq!(
        count(&pushed, |r| is_frame_row(r, 0)),
        8,
        "precondition: 8 frame rows pushed"
    );

    t.frame(1);
    assert_live_region_clean(&t, 1, "80x24 -> 40x15");
    assert_no_stale_visible(&t, 1, "80x24 -> 40x15");
    assert_only_terminal_stranded_rows(&mut t, 1, "80x24 -> 40x15");
    let all = t.transcript();
    assert_eq!(history_count(&all), 1, "history must survive exactly once");
    // The limitation, pinned: exactly the 8 rows the terminal itself pushed remain stale, in
    // scrollback. Nothing the renderer could reach was abandoned.
    assert_eq!(
        count(&all, |r| is_frame_row(r, 0)),
        8,
        "only the rows the terminal pushed may stay stale:\n{}",
        all.join("\n")
    );
}

/// The issue's own sequence, with and without `clear_live_region()` after each resize.
fn run_issue_sequence(clear: bool) -> Term {
    let mut t = Term::new(40, 15);
    t.commit(HISTORY);
    t.frame(0);
    for (step, (cols, rows)) in [(80, 24), (40, 15), (120, 40), (79, 23)]
        .into_iter()
        .enumerate()
    {
        let step = step + 1;
        t.resize(cols, rows);
        if clear {
            t.clear_live();
        }
        t.frame(step);
        let label = format!("clear={clear} step {step}");
        assert_live_region_clean(&t, step, &label);
        assert_only_terminal_stranded_rows(&mut t, step, &label);
        let all = t.transcript();
        assert_eq!(
            history_count(&all),
            1,
            "clear={clear} step {step}: committed history lost or duplicated:\n{}",
            all.join("\n")
        );
    }
    t
}

#[test]
fn issue_sequence_without_cleanup_keeps_history_and_a_clean_viewport() {
    run_issue_sequence(false);
}

#[test]
fn issue_sequence_with_clear_live_region_after_resize_keeps_history() {
    run_issue_sequence(true);
}

#[test]
fn threshold_jitter_never_stacks_stale_rows_or_duplicates_history() {
    let mut t = Term::new(80, 24);
    t.commit(HISTORY);
    t.frame(0);
    let sizes = [(79, 24), (80, 24), (80, 23), (80, 24), (81, 25), (80, 24)];
    for round in 0..3 {
        for (i, (cols, rows)) in sizes.into_iter().enumerate() {
            let step = 1 + round * sizes.len() + i;
            t.resize(cols, rows);
            t.frame(step);
            let label = format!("jitter step {step}");
            assert_live_region_clean(&t, step, &label);
            assert_only_terminal_stranded_rows(&mut t, step, &label);
            assert_eq!(
                history_count(&t.transcript()),
                1,
                "jitter step {step}: history"
            );
        }
    }
}

#[test]
fn short_live_region_grow_and_shrink_keep_history_adjacent() {
    // A 3-row region far from the screen edges: nothing is pushed by either resize, so the
    // whole transcript must be exactly history + the current frame.
    let mut t = Term::new(80, 24);
    t.commit(HISTORY);
    t.frame_rows(0, 3);
    for (step, (cols, rows)) in [(100, 30), (60, 20), (80, 24)].into_iter().enumerate() {
        let step = step + 1;
        t.resize(cols, rows);
        t.frame_rows(step, 3);
        let all: Vec<String> = t
            .transcript()
            .into_iter()
            .map(|r| r.trim_end().to_owned())
            .filter(|r| !r.is_empty())
            .collect();
        let expected: Vec<String> = std::iter::once(HISTORY.to_owned())
            .chain((0..3).map(|i| format!("f{step}-r{i}")))
            .collect();
        assert_eq!(
            all, expected,
            "step {step}: transcript must be history + current frame"
        );
    }
}

#[test]
fn commit_after_resize_before_any_frame_keeps_history_and_viewport_clean() {
    let mut t = Term::new(40, 15);
    t.commit(HISTORY);
    t.frame(0);
    t.resize(80, 24);
    t.commit("AFTER-RESIZE");
    t.frame(1);
    assert_live_region_clean(&t, 1, "commit after resize");
    assert_no_stale_visible(&t, 1, "commit after resize");
    let all = t.transcript();
    assert_eq!(history_count(&all), 1, "history must survive exactly once");
    assert_eq!(count(&all, |r| r.trim_end() == "AFTER-RESIZE"), 1);
    assert_eq!(
        count(&all, |r| is_frame_row(r, 0)),
        0,
        "grow: nothing stranded"
    );
}
