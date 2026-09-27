//! Regression coverage for the v0.2.2 output/observability patch.
//!
//!   #45 — safe preformatted scrollback: `*_with_mode(NoWrap)` preserves aligned
//!         wide rows (no silent truncation) and still neutralizes control chars.
//!   #47 — `Context::last_exact_changed_cells()` exposes the exact semantic delta.
//!   #48 E-02 — `Context::last_frame_lines()` gives visible text of the frame.
//!   #48 E-07 — canonical loop is documented (see the `run_once` doc example) and
//!         `Event::Tick` is not emitted by the core loop.

use gibson::context::{Context, RenderMode};
use gibson::node::WrapMode;
use gibson::{Node, Style};

// ---- #47: exact semantic changed-cell scalar on Context -----------------

#[test]
fn issue_47_exact_changed_cells_exposed_and_discriminating() {
    let mut ctx = Context::headless(RenderMode::Inline, 20, 3);
    ctx.set_root(Node::col().child(Node::text("HELLO", Style::default())));
    ctx.render_now().unwrap();
    assert!(
        ctx.last_exact_changed_cells() > 0,
        "the first painted frame must report a nonzero exact changed-cell count"
    );

    // A settled re-render of the identical tree changes nothing.
    ctx.render_now().unwrap();
    assert_eq!(
        ctx.last_exact_changed_cells(),
        0,
        "a settled frame must report zero exact changed cells"
    );

    // Changing the content produces a nonzero exact delta again.
    ctx.set_root(Node::col().child(Node::text("WORLD", Style::default())));
    ctx.render_now().unwrap();
    assert!(
        ctx.last_exact_changed_cells() > 0,
        "changed content must report a nonzero exact changed-cell count"
    );
}

// ---- #48 E-02: visible-text snapshot of the composed frame --------------

#[test]
fn issue_48_e02_last_frame_lines_are_visible_text() {
    let mut ctx = Context::headless(RenderMode::Inline, 20, 3);
    ctx.set_root(
        Node::col()
            .child(Node::text("ALPHA", Style::default()))
            .child(Node::text("BETA", Style::default())),
    );
    ctx.render_now().unwrap();
    let lines = ctx.last_frame_lines();
    let joined = lines.join("\n");
    assert!(
        joined.contains("ALPHA"),
        "frame lines missing ALPHA: {lines:?}"
    );
    assert!(
        joined.contains("BETA"),
        "frame lines missing BETA: {lines:?}"
    );
    assert!(
        !joined.contains('\x1b'),
        "last_frame_lines must be visible text, not wire bytes"
    );
}

#[test]
fn issue_48_e02_no_frame_before_first_render() {
    let ctx = Context::headless(RenderMode::Inline, 10, 2);
    assert!(
        ctx.last_frame_lines().iter().all(|l| l.is_empty()),
        "a context that has not rendered has no visible frame"
    );
}

#[test]
fn issue_48_e02_wide_glyph_not_split_in_frame_lines() {
    let mut ctx = Context::headless(RenderMode::Inline, 10, 1);
    ctx.set_root(Node::text("界A", Style::default()));
    ctx.render_now().unwrap();
    let lines = ctx.last_frame_lines();
    assert!(
        lines.iter().any(|l| l.contains("界A")),
        "wide-glyph continuation collapsed incorrectly: {lines:?}"
    );
}

// ---- #45: safe preformatted scrollback insertion ------------------------

#[test]
fn issue_45_nowrap_preserves_wide_preformatted_row() {
    // A 96-column aligned ledger row inserted at an 80-column terminal.
    let row = format!(
        "{:<12}{:>28}{:>28}{:>28}",
        "a1b2c3d", "Grace Hopper", "14", "+332/-14"
    );
    assert_eq!(row.chars().count(), 96);

    let mut ctx = Context::headless(RenderMode::Inline, 80, 24);
    ctx.set_root(Node::col().child(Node::text("LIVE", Style::default())));
    ctx.render_now().unwrap();

    // NoWrap: the full aligned row survives — no silent truncation of the last column.
    ctx.insert_text_before_live_with_mode(&row, WrapMode::NoWrap)
        .unwrap();
    let out = ctx.take_output();
    assert!(out.contains("a1b2c3d"), "oid column dropped");
    assert!(out.contains("Grace Hopper"), "author column dropped");
    assert!(
        out.contains("+332/-14"),
        "trailing churn column silently truncated (the #45 bug)"
    );
}

#[test]
fn issue_45_nowrap_still_neutralizes_control_sequences() {
    let mut ctx = Context::headless(RenderMode::Inline, 80, 24);
    ctx.set_root(Node::text("LIVE", Style::default()));
    ctx.render_now().unwrap();

    let hostile = "col1\x1b[2Jcol2\x1b]0;pwned\x07tail";
    ctx.insert_text_before_live_with_mode(hostile, WrapMode::NoWrap)
        .unwrap();
    let out = ctx.take_output();
    assert!(
        !out.contains("\x1b[2J"),
        "NoWrap must not let a raw clear-screen sequence reach the terminal"
    );
    assert!(
        !out.contains("\x1b]0;"),
        "NoWrap must not let a raw OSC sequence reach the terminal"
    );
}

#[test]
fn issue_45_commit_text_with_mode_nowrap_preserves_columns() {
    let row = format!(
        "{:<10}{:>40}",
        "left", "right-edge-aligned-value-past-eighty"
    );
    let mut ctx = Context::headless(RenderMode::Inline, 40, 10);
    ctx.commit_text_with_mode(&row, WrapMode::NoWrap).unwrap();
    let out = ctx.take_output();
    assert!(out.contains("left"), "left column dropped");
    assert!(
        out.contains("right-edge-aligned-value-past-eighty"),
        "right column truncated under NoWrap commit"
    );
}
