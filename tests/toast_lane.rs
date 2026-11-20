//! RED slice for the v0.9 toast lane (`docs/design/v0.9-toast-lane.md`).
//!
//! `toast_lane(toasts)` is a noninteractive overlay host mounted with `.overlay(...)` on the
//! element whose rectangle the toasts must stay inside. Toasts stack from the top-right of that
//! host, newest first, each in its own box of the toast's height, so they never overlap and
//! never cover anything outside the host. When the host holds fewer boxes than there are toasts,
//! the last box becomes a "+N more" row; toasts are never dropped silently.
//!
//! The `red_` tests fail by assertion against the stub (a plain 100% column of the toasts, which
//! stacks them in the host's full height, so the first toast lands bottom-right and the rest fall
//! outside the host). The `guard_` tests pass today and pin the behavior GREEN must keep.
//!
//! Scene: a keyed one-row header on top, then a row of a growing body (a sentinel-colored `raw`
//! column, the lane's host) and a fixed-width right rail (another sentinel). The body's rect is
//! recovered from the no-toast frame by its sentinel background, so no test hard-codes layout.

use gibson::ui::prelude::*;
use gibson::{compute_layout, paint, Color, Node, Surface};

const BODY: Color = Color::Rgb(200, 30, 120);
const RAIL: Color = Color::Rgb(30, 120, 200);
const RAIL_W: f32 = 12.0;
const TERM_H: u16 = 24;
/// The default toast height in rows (design: honour `.height()`, default 3).
const TOAST_H: u16 = 3;

/// The body's painted rectangle, `x1`/`y1` exclusive.
#[derive(Debug, Clone, Copy)]
struct BodyRect {
    x0: u16,
    y0: u16,
    x1: u16,
    y1: u16,
}

fn app(body_h: u16, overlay: Element<()>) -> Element<()> {
    let body = raw(Node::col().background(BODY).height(body_h as f32))
        .grow(1.0)
        .overlay(overlay);
    let rail_node = Node::col()
        .background(RAIL)
        .width(RAIL_W)
        .height(body_h as f32);
    let rail = raw(rail_node);
    screen().height(TERM_H).child(
        column()
            .child(label("HEADER").key("header"))
            .child(row().height(body_h).child(body).child(rail)),
    )
}

/// An overlay that paints nothing, so the no-toast frame has the same tree shape.
fn inert() -> Element<()> {
    raw(Node::col().width(0.0).height(0.0))
}

fn lane_of(names: &[&str]) -> Element<()> {
    let toasts: Vec<Element<()>> = names.iter().map(|n| toast(*n)).collect();
    toast_lane(toasts)
}

fn render(el: Element<()>, w: u16) -> Surface {
    let env = UiEnvironment {
        width: w,
        height: TERM_H,
        ..UiEnvironment::default()
    };
    let cx = BuildCx::new(skins::SWISS_SIGNAL, env);
    let mut compiled = compile(&el, &cx).expect("scene compiles");
    compute_layout(&mut compiled.node, w, TERM_H).expect("layout resolves");
    let mut surface = Surface::new(w, TERM_H);
    paint(&compiled.node, &mut surface);
    surface
}

/// The bounding box of the body sentinel in the no-toast frame.
fn body_rect(base: &Surface) -> BodyRect {
    let mut rect = BodyRect {
        x0: u16::MAX,
        y0: u16::MAX,
        x1: 0,
        y1: 0,
    };
    for y in 0..base.height {
        for x in 0..base.width {
            if base.get(x, y).is_some_and(|c| c.style.bg == Some(BODY)) {
                rect.x0 = rect.x0.min(x);
                rect.y0 = rect.y0.min(y);
                rect.x1 = rect.x1.max(x + 1);
                rect.y1 = rect.y1.max(y + 1);
            }
        }
    }
    assert!(
        rect.x1 > rect.x0 && rect.y1 > rect.y0,
        "no body sentinel painted"
    );
    rect
}

fn rows_with(surface: &Surface, needle: &str) -> Vec<u16> {
    surface
        .to_visible_lines()
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(y, _)| y as u16)
        .collect()
}

/// Cells that differ between two frames, optionally only those outside the body rect.
fn changed(base: &Surface, now: &Surface, outside: Option<BodyRect>) -> Vec<(u16, u16)> {
    let mut out = vec![];
    for y in 0..base.height {
        for x in 0..base.width {
            if let Some(r) = outside {
                if x >= r.x0 && x < r.x1 && y >= r.y0 && y < r.y1 {
                    continue;
                }
            }
            if base.get(x, y) != now.get(x, y) {
                out.push((x, y));
            }
        }
    }
    out
}

fn row_changed(base: &Surface, now: &Surface, y: u16, x0: u16, x1: u16) -> bool {
    (x0..x1).any(|x| base.get(x, y) != now.get(x, y))
}

/// The single row a toast's text landed on, which must lie inside the host.
fn text_row(now: &Surface, name: &str, body: BodyRect) -> u16 {
    let rows = rows_with(now, name);
    assert_eq!(
        rows.len(),
        1,
        "{name} should be drawn exactly once, found rows {rows:?}"
    );
    assert!(
        rows[0] >= body.y0 && rows[0] < body.y1,
        "{name} drawn at row {} outside the host rows {}..{}",
        rows[0],
        body.y0,
        body.y1
    );
    rows[0]
}

/// Asserts `name` is drawn inside box `slot` (0 = topmost) and flush with the host's right edge.
fn assert_in_slot(base: &Surface, now: &Surface, name: &str, slot: u16, body: BodyRect) {
    let y = text_row(now, name, body);
    let top = body.y0 + slot * TOAST_H;
    assert!(
        y >= top && y < top + TOAST_H,
        "{name} should sit in box {slot} (rows {top}..{}), drawn at row {y}",
        top + TOAST_H
    );
    assert!(
        base.get(body.x1 - 1, y) != now.get(body.x1 - 1, y),
        "{name} is not flush with the host's right edge"
    );
    assert!(
        base.get(body.x0, y) == now.get(body.x0, y),
        "{name} spans the whole host width instead of its own"
    );
}

fn assert_outside_unchanged(base: &Surface, now: &Surface, body: BodyRect, why: &str) {
    let diff = changed(base, now, Some(body));
    assert!(
        diff.is_empty(),
        "{why}: {} cells outside the host changed, first {:?}",
        diff.len(),
        &diff[..diff.len().min(8)]
    );
}

fn assert_rows_unchanged(base: &Surface, now: &Surface, body: BodyRect, ys: std::ops::Range<u16>) {
    for y in ys {
        assert!(
            !row_changed(base, now, y, body.x0, body.x1),
            "host row {y} should be untouched"
        );
    }
}

// ---- guards: pass today ----

#[test]
fn guard_plain_overlay_toast_stays_bottom_right() {
    let base = render(app(10, inert()), 80);
    let now = render(app(10, toast("PLAIN")), 80);
    let body = body_rect(&base);
    assert_eq!(body.y1 - body.y0, 10, "body height");
    let y = text_row(&now, "PLAIN", body);
    assert!(
        y >= body.y1 - TOAST_H,
        "ordinary toast() must stay in the host's last {TOAST_H} rows, drawn at row {y}"
    );
    assert!(
        base.get(body.x1 - 1, y) != now.get(body.x1 - 1, y),
        "ordinary toast() must stay flush right"
    );
    assert_outside_unchanged(&base, &now, body, "ordinary toast");
}

#[test]
fn guard_empty_lane_draws_nothing() {
    let base = render(app(16, inert()), 80);
    let now = render(app(16, toast_lane(Vec::<Element<()>>::new())), 80);
    let diff = changed(&base, &now, None);
    assert!(
        diff.is_empty(),
        "an empty lane changed {} cells",
        diff.len()
    );
}

// ---- red: fail by assertion against the stub ----

#[test]
fn red_header_and_rail_are_never_covered_by_the_lane() {
    for w in [80u16, 100] {
        let base = render(app(16, inert()), w);
        let now = render(app(16, lane_of(&["TOAST-A", "TOAST-B", "TOAST-C"])), w);
        let body = body_rect(&base);
        assert!(
            !rows_with(&base, "HEADER").is_empty(),
            "header missing at width {w}"
        );
        // Non-vacuous: the lane really drew all three toasts inside the host.
        for name in ["TOAST-A", "TOAST-B", "TOAST-C"] {
            text_row(&now, name, body);
        }
        assert_outside_unchanged(&base, &now, body, &format!("width {w}"));
    }
}

#[test]
fn red_toasts_occupy_disjoint_boxes_stacked_from_the_top_right() {
    let base = render(app(16, inert()), 80);
    let now = render(app(16, lane_of(&["TOAST-A", "TOAST-B", "TOAST-C"])), 80);
    let body = body_rect(&base);
    assert_in_slot(&base, &now, "TOAST-A", 0, body);
    assert_in_slot(&base, &now, "TOAST-B", 1, body);
    assert_in_slot(&base, &now, "TOAST-C", 2, body);
    let rows: Vec<u16> = ["TOAST-A", "TOAST-B", "TOAST-C"]
        .iter()
        .map(|n| text_row(&now, n, body))
        .collect();
    assert!(
        rows[0] < rows[1] && rows[1] < rows[2],
        "newest first: rows {rows:?}"
    );
    // Three boxes only: nothing below them, and no "+N more" row when everything fits.
    assert_rows_unchanged(&base, &now, body, body.y0 + 3 * TOAST_H..body.y1);
    assert!(
        rows_with(&now, "more").is_empty(),
        "no overflow row expected"
    );
    assert_outside_unchanged(&base, &now, body, "stacked lane");
}

#[test]
fn red_overflow_collapses_the_oldest_into_a_plus_n_more_row() {
    // Host of 9 rows: capacity k = 3. Six toasts: the k-1 = 2 newest are drawn, and the last
    // box becomes "+4 more" (N = 6 - 2). Nothing is dropped silently.
    let names = ["N-1", "N-2", "N-3", "N-4", "N-5", "N-6"];
    let base = render(app(9, inert()), 80);
    let now = render(app(9, lane_of(&names)), 80);
    let body = body_rect(&base);
    assert_eq!(body.y1 - body.y0, 9, "body height");
    assert_in_slot(&base, &now, "N-1", 0, body);
    assert_in_slot(&base, &now, "N-2", 1, body);
    for name in &names[2..] {
        assert!(
            rows_with(&now, name).is_empty(),
            "{name} must be collapsed into the more row"
        );
    }
    assert_in_slot(&base, &now, "+4 more", 2, body);
    assert_outside_unchanged(&base, &now, body, "overflowing lane");
}

#[test]
fn red_capacity_one_shows_only_the_more_row() {
    // Provisional choice (open question in the design note): with k == 1 the single box is the
    // "+N more" row with N = total, so no toast text is shown.
    let names = ["TOAST-1", "TOAST-2", "TOAST-3"];
    let base = render(app(4, inert()), 80);
    let now = render(app(4, lane_of(&names)), 80);
    let body = body_rect(&base);
    assert_eq!(body.y1 - body.y0, 4, "body height");
    assert_in_slot(&base, &now, "+3 more", 0, body);
    for name in names {
        assert!(
            rows_with(&now, name).is_empty(),
            "{name} must be collapsed into the more row"
        );
    }
    assert_rows_unchanged(&base, &now, body, body.y0 + TOAST_H..body.y1);
    assert_outside_unchanged(&base, &now, body, "capacity-one lane");
}

#[test]
fn red_host_shorter_than_one_toast_draws_nothing() {
    // Two rows cannot hold a three-row toast: the frame is identical to the no-toast frame.
    let base = render(app(2, inert()), 80);
    let now = render(app(2, lane_of(&["TOAST-A", "TOAST-B", "TOAST-C"])), 80);
    let diff = changed(&base, &now, None);
    assert!(
        diff.is_empty(),
        "a host shorter than one toast changed {} cells",
        diff.len()
    );

    // Companion: a host that fits exactly one toast (4 rows) draws it in the top box only.
    let base = render(app(4, inert()), 80);
    let now = render(app(4, lane_of(&["TOAST-A"])), 80);
    let body = body_rect(&base);
    assert_in_slot(&base, &now, "TOAST-A", 0, body);
    assert_rows_unchanged(&base, &now, body, body.y0 + TOAST_H..body.y1);
    assert_outside_unchanged(&base, &now, body, "one-toast host");
}
