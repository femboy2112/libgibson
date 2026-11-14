//! Modal sizing contract (v0.5 spatial-UI axis).
//!
//! Regression for the PULSAR-2 FRICTION §5 defect: `modal()` defaulted to a
//! FIXED height of 12 cells, so title + border + padding left ~9 content rows
//! visible — 12 children showed 9, and *30* children on a 40-row screen still
//! showed 9, with no scroll and no warning. An explicit `.height(n)` worked.
//!
//! Desired contract:
//! * an explicit `.height(n)` is authoritative;
//! * with no explicit height the modal sizes to its CONTENT, bounded by 90% of
//!   the viewport (it never pretends all children fit when they do not — it
//!   shows exactly as many as physically fit in that hard bound);
//! * it is not a scroll container / window manager — overflow beyond 90% of the
//!   viewport is clipped, deterministically.

use gibson::ui::prelude::*;
use gibson::{Context, RenderMode};
use std::time::Duration;

/// Render a modal of `children` text rows on a `screen_h`-row screen (optionally
/// with an explicit modal height) and return how many "line NN" rows are
/// actually painted — the PULSAR-2 reproducer's measurement, verbatim.
fn visible(screen_h: u16, children: usize, explicit: Option<u16>) -> usize {
    let env = UiEnvironment {
        width: 60,
        height: screen_h,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    };
    let mut rt: UiRuntime<()> = UiRuntime::new(skins::BLACK_ICE);
    let mut ctx = Context::headless(RenderMode::Fullscreen, 60, screen_h);
    let mut body = column();
    for i in 0..children {
        body = body.child(text(format!("line {i:02}")));
    }
    let mut m = modal("TITLE").key("m").child(body);
    if let Some(h) = explicit {
        m = m.height(h);
    }
    let tree: Element<()> = screen().height(screen_h).child(text("base")).overlay(m);
    let f = rt.frame(&tree, env, Duration::ZERO).unwrap();
    ctx.set_root(f.node);
    ctx.render_now().unwrap();
    ctx.last_frame_lines()
        .iter()
        .filter(|l| l.contains("line "))
        .count()
}

// ---- content-driven default (the fix) ------------------------------------

#[test]
fn one_row_shows_one() {
    assert_eq!(visible(24, 1, None), 1);
}

#[test]
fn nine_rows_show_nine() {
    assert_eq!(visible(24, 9, None), 9);
}

#[test]
fn twelve_rows_on_a_24_row_screen_show_all_twelve() {
    // The headline defect: this returned 9 under the fixed-12 default. Twelve
    // rows + chrome fit well inside 90% of a 24-row screen, so all show.
    assert_eq!(visible(24, 12, None), 12);
}

#[test]
fn thirty_rows_on_a_40_row_screen_show_all_thirty() {
    // The egregious defect: 30 children still showed 9. 30 + chrome fit inside
    // 90% of 40 (= 36 cells), so all thirty show.
    assert_eq!(visible(40, 30, None), 30);
}

#[test]
fn twelve_rows_on_a_40_row_screen_show_all_twelve() {
    assert_eq!(visible(40, 12, None), 12);
}

// ---- explicit height stays authoritative ---------------------------------

#[test]
fn explicit_height_is_authoritative() {
    // Unchanged from before the fix: an explicit height shows what fits in it.
    assert_eq!(visible(24, 12, Some(16)), 12);
}

#[test]
fn explicit_small_height_still_caps() {
    // An explicit height smaller than the content still bounds it (authoritative
    // request honoured; no silent growth to fit content).
    let v = visible(40, 30, Some(8));
    assert!(v < 30, "explicit small height must cap content, got {v}");
    assert!(v >= 1, "but still shows what fits, got {v}");
}

// ---- honest clipping at the viewport bound (not a scroll container) -------

#[test]
fn content_taller_than_viewport_clips_but_shows_more_than_the_old_cap() {
    // 30 rows cannot fit in 90% of a 24-row screen. The modal clips at that hard
    // bound — but it shows as many as physically fit, NOT the old fixed 9, and
    // never silently claims all 30 fit.
    let v = visible(24, 30, None);
    assert!(v < 30, "must clip honestly at the viewport bound, got {v}");
    assert!(
        v > 9,
        "must show more than the old fixed-12 (~9 visible) cap, got {v}"
    );
    assert!(v as u16 <= 24, "never exceeds the screen, got {v}");
}

// ---- hostile / degenerate sizes: no panic, bounded -----------------------

#[test]
fn tiny_hostile_screen_does_not_panic_and_stays_bounded() {
    for h in [1u16, 2, 3, 4] {
        let v = visible(h, 12, None);
        assert!(v as u16 <= h, "visible {v} must fit screen height {h}");
    }
}

#[test]
fn zero_children_is_fine() {
    assert_eq!(visible(24, 0, None), 0);
}

// ---- resize recomputes content height ------------------------------------

#[test]
fn resize_recomputes_against_the_new_viewport() {
    // Same 20-child modal: fits entirely on a tall screen, clips on a short one.
    let tall = visible(40, 20, None);
    let short = visible(16, 20, None);
    assert_eq!(tall, 20, "20 rows fit inside 90% of 40");
    assert!(
        short < 20,
        "20 rows cannot fit inside 90% of 16, got {short}"
    );
    assert!(short >= 1);
}

// ---- nested modal: inner modal also sizes to content ---------------------

#[test]
fn nested_modal_sizes_to_content_without_panic() {
    let env = UiEnvironment {
        width: 60,
        height: 40,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    };
    let mut rt: UiRuntime<()> = UiRuntime::new(skins::BLACK_ICE);
    let mut ctx = Context::headless(RenderMode::Fullscreen, 60, 40);
    let mut inner_body = column();
    for i in 0..10 {
        inner_body = inner_body.child(text(format!("line {i:02}")));
    }
    let inner = modal("INNER").key("inner").child(inner_body);
    let outer = modal("OUTER")
        .key("outer")
        .child(text("outer body"))
        .overlay(inner);
    let tree: Element<()> = screen().height(40).child(text("base")).overlay(outer);
    let f = rt.frame(&tree, env, Duration::ZERO).unwrap();
    ctx.set_root(f.node);
    ctx.render_now().unwrap();
    let shown = ctx
        .last_frame_lines()
        .iter()
        .filter(|l| l.contains("line "))
        .count();
    assert_eq!(shown, 10, "the nested modal's 10 rows all fit and render");
}
