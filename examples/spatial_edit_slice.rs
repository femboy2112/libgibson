//! Wave C demo — a real interaction, driven by injected input.
//!
//! A minimal application-owned scene of world-space rectangles, edited through the
//! LibGibson substrate: injected events -> hit test / pointer capture / world
//! geometry -> an application command. Run it with:
//!
//! ```sh
//! cargo run --example spatial_edit_slice
//! ```
//!
//! It prints the observable command log. The rigorous, assertion-backed version of
//! this slice lives in `tests/spatial_edit_slice.rs`.

use gibson::hit_test::{HitLayers, PointerCapture, TargetId};
use gibson::world_view::WorldView;
use gibson::{
    Context, Event, KeyModifiers, MouseButton, MouseEvent, MouseKind, Rect, RenderMode,
    ScriptedSource,
};
use std::time::Duration;

struct RectObj {
    id: u64,
    wx: f64,
    wy: f64,
    ww: f64,
    wh: f64,
}

fn layers(view: &WorldView, objs: &[RectObj]) -> HitLayers {
    let mut l = HitLayers::new();
    for o in objs {
        if let Some((cx, cy)) = view.world_to_cell(o.wx, o.wy).value().copied() {
            l.add_target(
                TargetId(o.id),
                Rect {
                    x: cx,
                    y: cy,
                    width: o.ww as u16,
                    height: o.wh as u16,
                },
                0,
            );
        }
    }
    l
}

fn mouse(kind: MouseKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })
}

fn main() -> std::io::Result<()> {
    // A headless context proves the injected-input path works with no live TTY.
    let mut ctx = Context::headless(RenderMode::Inline, 80, 24);
    let view = WorldView::new(Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    })
    .with_pixels_per_cell(1, 1);
    let mut objs = vec![RectObj {
        id: 1,
        wx: 10.0,
        wy: 5.0,
        ww: 4.0,
        wh: 3.0,
    }];
    let hits = layers(&view, &objs);
    let mut capture = PointerCapture::new();
    let mut drag_start: Option<(f64, f64)> = None;

    // A scripted drag: grab obj 1, drag it, release 5 cells to the right.
    let mut src = ScriptedSource::from_events(vec![
        mouse(MouseKind::Down(MouseButton::Left), 11, 6),
        mouse(MouseKind::Drag(MouseButton::Left), 13, 6),
        mouse(MouseKind::Up(MouseButton::Left), 16, 6),
    ]);

    println!("spatial edit slice — injected input -> world-space edit\n");
    while let Some(ev) = ctx.poll_from(&mut src, Duration::from_millis(1))? {
        if let Event::Mouse(m) = ev {
            match m.kind {
                MouseKind::Down(MouseButton::Left) => {
                    if let Some(id) = hits.hit_test(m.column, m.row) {
                        capture.capture(id);
                        drag_start = view.cell_to_world(m.column, m.row).value().copied();
                        println!("  down   -> capture {:?} at world {:?}", id, drag_start);
                    }
                }
                MouseKind::Up(MouseButton::Left) => {
                    if let Some(target) = capture.route(&hits, m.column, m.row) {
                        if let (Some((sx, sy)), Some((ex, ey))) = (
                            drag_start,
                            view.cell_to_world(m.column, m.row).value().copied(),
                        ) {
                            let (dx, dy) = (ex - sx, ey - sy);
                            if let Some(o) = objs.iter_mut().find(|o| o.id == target.0) {
                                o.wx += dx;
                                o.wy += dy;
                                println!(
                                    "  up     -> COMMIT Move {{ id: {}, dx: {dx}, dy: {dy} }}  (obj now at {}, {})",
                                    target.0, o.wx, o.wy
                                );
                            }
                        }
                    }
                    capture.release();
                    drag_start = None;
                }
                _ => {
                    let _ = capture.route(&hits, m.column, m.row);
                    println!("  drag   -> capture stays {:?}", capture.captured());
                }
            }
        }
    }

    println!("\ndone — the edit was produced by injected input through the public substrate.");
    Ok(())
}
