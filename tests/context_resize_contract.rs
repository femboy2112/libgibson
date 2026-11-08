//! v0.7 node G (library level) — the resize contract a raster consumer relies on.
//!
//! A full-size `Node::canvas` is painted at whatever rect layout gives it. When the terminal is
//! resized (here: a headless session's size changed, and the `Resize` event delivered through an
//! injected source, as a terminal would deliver it), the next due render must:
//! - hand the canvas exactly the new rect,
//! - produce a frame of exactly the new geometry,
//! - leave no cell from an earlier frame: every cell carries the current paint.
//!
//! On a headless context the resize takes effect when `poll_from` delivers it: nothing but the
//! event sets the new geometry (#293), as with the C ABI's `gibson_poll_event`.
//!
//! Growth and shrinkage both, including to 1×1, from a context that had already rendered. The
//! canvas covers the whole frame, so this checks the glyph readback of a fully repainted frame;
//! physical clearing of uncovered terminal areas on resize is covered in
//! `tests/whole_renderer_vt100.rs`.
use gibson::cell::{Cell, Glyph, Style};
use gibson::context::{Context, RenderMode};
use gibson::event_source::InjectedQueue;
use gibson::input::Event;
use gibson::node::Node;
use gibson::surface::{Rect, Surface};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The glyph a generation paints everywhere: 'A' for the first, 'B' for the next, and so on.
fn generation_glyph(generation: u32) -> char {
    char::from(b'A' + (generation % 26) as u8)
}

/// Waits until the scheduler's frame budget allows a frame, so a `false` from `render_if_due`
/// means "nothing to draw", not "too soon".
fn wait_until_due(ctx: &Context) {
    let end = Instant::now() + Duration::from_secs(2);
    while !ctx.scheduler.time_until_next_frame().is_zero() {
        assert!(Instant::now() < end, "the frame budget never elapsed");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Renders when due, never forced; fails if nothing is drawn within two seconds.
fn render_when_due(ctx: &mut Context) {
    let end = Instant::now() + Duration::from_secs(2);
    while Instant::now() < end {
        if ctx.render_if_due().expect("render") {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("a delivered resize did not lead to a render");
}

#[test]
fn a_resize_repaints_a_full_size_canvas_at_exactly_the_new_geometry() {
    let generation = Arc::new(AtomicU32::new(0));
    let rects: Arc<Mutex<Vec<Rect>>> = Arc::default();
    let root = {
        let generation = Arc::clone(&generation);
        let rects = Arc::clone(&rects);
        Node::canvas(move |r: Rect| {
            rects.lock().unwrap().push(r);
            let g = generation_glyph(generation.load(Ordering::SeqCst));
            let mut s = Surface::new(r.width, r.height);
            s.fill_rect(
                Rect {
                    x: 0,
                    y: 0,
                    width: r.width,
                    height: r.height,
                },
                Cell::new(Glyph::from_char(g), Style::default()),
            );
            // The last cell marks the corner the canvas believes it has.
            if r.width > 0 && r.height > 0 {
                s.set_cell(
                    r.width - 1,
                    r.height - 1,
                    Cell::new(Glyph::from_char('#'), Style::default()),
                );
            }
            s
        })
        .percent_width(100.0)
        .percent_height(100.0)
    };
    let mut ctx = Context::headless(RenderMode::Fullscreen, 40, 12);
    ctx.set_root(root);
    ctx.render().expect("first render");
    let mut queue = InjectedQueue::with_capacity(4);

    let mut size = (40u16, 12u16);
    for (step, (cols, rows)) in [(80u16, 24u16), (20, 6), (1, 1), (120, 40), (33, 7)]
        .into_iter()
        .enumerate()
    {
        let what = format!("{}x{} -> {cols}x{rows}", size.0, size.1);
        // Nothing has changed yet: a render that is due draws nothing.
        wait_until_due(&ctx);
        assert!(
            !ctx.render_if_due().expect("render_if_due"),
            "{what}: drew before the resize arrived"
        );
        generation.store(step as u32 + 1, Ordering::SeqCst);
        rects.lock().unwrap().clear();
        // No manual `ctx.session.set_terminal_size`: on a headless context the delivered
        // resize itself sets the geometry, as `gibson_poll_event` does (#293).
        queue.push(Event::Resize(cols, rows)).expect("queue");
        // Queued but not yet delivered: still nothing to draw, and the canvas is not called.
        wait_until_due(&ctx);
        assert!(
            !ctx.render_if_due().expect("render_if_due"),
            "{what}: a queued, undelivered resize already drew"
        );
        assert!(
            rects.lock().unwrap().is_empty(),
            "{what}: painted before delivery"
        );
        assert_eq!(
            ctx.poll_from(&mut queue, Duration::ZERO).expect("poll"),
            Some(Event::Resize(cols, rows))
        );
        render_when_due(&mut ctx);

        let painted = rects.lock().unwrap().clone();
        assert_eq!(
            painted.last().copied(),
            Some(Rect {
                x: 0,
                y: 0,
                width: cols,
                height: rows
            }),
            "{what}: the canvas was not handed the new rect: {painted:?}"
        );
        assert_eq!(ctx.frame_size(), Some((cols, rows)), "{what}: frame size");
        let g = generation_glyph(step as u32 + 1);
        for y in 0..rows {
            for x in 0..cols {
                let cell = ctx.frame_cell(x, y).expect("cell inside the frame");
                let want = if (x, y) == (cols - 1, rows - 1) {
                    "#".to_string()
                } else {
                    g.to_string()
                };
                assert_eq!(cell.glyph, want, "{what}: cell ({x},{y}) is stale or wrong");
            }
        }
        assert!(
            ctx.frame_cell(cols, 0).is_none() && ctx.frame_cell(0, rows).is_none(),
            "{what}: cells beyond the new geometry are still readable"
        );
        size = (cols, rows);
    }
}

#[test]
fn a_delivered_resize_alone_sets_the_headless_geometry() {
    // #293: before the fix `poll_from` only marked the scheduler dirty, so the next render laid
    // out at the old size (r02 of the v0.8 parity slice: 40x12 where the C ABI gives 60x16).
    let mut ctx = Context::headless(RenderMode::Inline, 40, 12);
    ctx.set_root(Node::text("x", gibson::cell::Style::default()));
    ctx.render().expect("first render");
    let mut queue = InjectedQueue::with_capacity(4);
    queue.push(Event::Resize(60, 16)).expect("queue");
    assert_eq!(
        ctx.session.terminal_size(),
        (40, 12),
        "queued, not delivered"
    );
    assert_eq!(
        ctx.poll_from(&mut queue, Duration::ZERO).expect("poll"),
        Some(Event::Resize(60, 16))
    );
    assert_eq!(
        ctx.session.terminal_size(),
        (60, 16),
        "a delivered resize must set a headless context's geometry"
    );
    ctx.render().expect("render");
    assert_eq!(ctx.frame_size().map(|(c, _)| c), Some(60), "frame width");
}

#[test]
fn a_zero_dimension_resize_is_delivered_but_not_applied_on_headless() {
    // The contract for hostile sizes: the C ABI refuses to queue a resize with a zero
    // dimension (`gibson_push_event` -> INVALID_PARAM), so a headless context never takes a
    // zero geometry from a resize there. The Rust path agrees: `poll_from` still hands the event
    // out (the source is the authority on what was delivered), but the geometry is unchanged.
    let mut ctx = Context::headless(RenderMode::Fullscreen, 40, 12);
    ctx.set_root(Node::text("x", gibson::cell::Style::default()));
    ctx.render().expect("first render");
    let mut queue = InjectedQueue::with_capacity(4);
    for (cols, rows) in [(0u16, 12u16), (12, 0), (0, 0)] {
        queue.push(Event::Resize(cols, rows)).expect("queue");
        assert_eq!(
            ctx.poll_from(&mut queue, Duration::ZERO).expect("poll"),
            Some(Event::Resize(cols, rows)),
            "{cols}x{rows}: the event is still delivered"
        );
        assert_eq!(
            ctx.session.terminal_size(),
            (40, 12),
            "{cols}x{rows}: a zero dimension must not be applied"
        );
        ctx.render().expect("render");
        assert_eq!(
            ctx.frame_size(),
            Some((40, 12)),
            "{cols}x{rows}: frame size"
        );
    }
    // A valid resize after the refused ones still applies.
    queue.push(Event::Resize(21, 5)).expect("queue");
    ctx.poll_from(&mut queue, Duration::ZERO).expect("poll");
    assert_eq!(ctx.session.terminal_size(), (21, 5));
}

#[test]
fn a_terminal_context_keeps_observing_its_geometry_from_the_terminal() {
    // Control: a non-headless context's size comes from the terminal (or the fixed 80x24 of a
    // non-TTY stdout), never from an event. An injected resize is delivered and schedules a
    // render, but the geometry stays what the terminal reports.
    let mut ctx = Context::inline().expect("inline context");
    assert!(!ctx.session.is_headless());
    let observed = ctx.session.terminal_size();
    let odd = (
        observed.0.wrapping_add(17).max(1),
        observed.1.wrapping_add(5).max(1),
    );
    let mut queue = InjectedQueue::with_capacity(1);
    queue.push(Event::Resize(odd.0, odd.1)).expect("queue");
    assert_eq!(
        ctx.poll_from(&mut queue, Duration::ZERO).expect("poll"),
        Some(Event::Resize(odd.0, odd.1))
    );
    assert_eq!(
        ctx.session.terminal_size(),
        observed,
        "a terminal context's geometry must stay observed from the terminal"
    );
}
