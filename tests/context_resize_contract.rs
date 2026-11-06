//! v0.7 node G (library level) — the resize contract a raster consumer relies on.
//!
//! A full-size `Node::canvas` is painted at whatever rect layout gives it. When the terminal is
//! resized (here: a headless session's size changed, and the `Resize` event delivered through an
//! injected source, as a terminal would deliver it), the next due render must:
//! - hand the canvas exactly the new rect,
//! - produce a frame of exactly the new geometry,
//! - leave no cell from an earlier frame: every cell carries the current paint.
//!
//! Growth and shrinkage both, including to 1×1, from a context that had already rendered.
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
        // Nothing has changed yet: a due render draws nothing.
        assert!(
            !ctx.render_if_due().expect("render_if_due"),
            "{what}: drew before the resize arrived"
        );
        generation.store(step as u32 + 1, Ordering::SeqCst);
        ctx.session.set_terminal_size(cols, rows);
        queue.push(Event::Resize(cols, rows)).expect("queue");
        assert_eq!(
            ctx.poll_from(&mut queue, Duration::ZERO).expect("poll"),
            Some(Event::Resize(cols, rows))
        );
        rects.lock().unwrap().clear();
        render_when_due(&mut ctx);

        let painted = rects.lock().unwrap().clone();
        assert_eq!(
            painted.last().map(|r| (r.width, r.height)),
            Some((cols, rows)),
            "{what}: the canvas was not handed the new geometry: {painted:?}"
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
