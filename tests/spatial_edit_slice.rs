//! Wave C — the first *real* interaction, proven end to end.
//!
//! The round's gate: not a hit-test returning a plausible id, but an **observed
//! end-to-end edit command** produced by driving the public LibGibson substrate
//! with injected input. A tiny application-owned scene of world-space rectangles is
//! edited through the normalized pipeline:
//!
//! ```text
//! ScriptedSource (injected input)
//!   -> decoded Event (mouse/key/focus/resize)
//!     -> hit_test / PointerCapture (modal-aware routing, world geometry)
//!       -> typed intent
//!         -> app-owned, validated command (the observable edit log)
//! ```
//!
//! Every assertion is on the **app-owned command log**, never on an intermediate
//! hit-test. The model is owned by the test (the "application"); LibGibson owns only
//! the generic interaction/geometry substrate. These run headlessly; the same
//! dispatch is exercised again through a replayed input trace to prove determinism.

use gibson::hit_test::{HitLayers, PointerCapture, TargetId};
use gibson::world_view::WorldView;
use gibson::{
    Context, Event, EventSource, FocusChange, KeyCode, KeyEvent, KeyModifiers, MouseButton,
    MouseEvent, MouseKind, Recorder, Rect, RenderMode, ScriptedSource,
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Application-owned model (NOT a LibGibson type)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct RectObj {
    id: u64,
    wx: f64,
    wy: f64,
    ww: f64,
    wh: f64,
}

/// A validated, committed application command — the observable unit of state change.
#[derive(Clone, Debug, PartialEq)]
enum Cmd {
    Move { id: u64, dx: f64, dy: f64 },
}

struct App {
    objs: Vec<RectObj>,
    view: WorldView,
    log: Vec<Cmd>,
}

impl App {
    /// The ONLY path that advances application state. Presentation/observation never
    /// calls this; only a resolved, non-degenerate edit does.
    fn apply_move(&mut self, id: u64, dx: f64, dy: f64) {
        if let Some(o) = self.objs.iter_mut().find(|o| o.id == id) {
            o.wx += dx;
            o.wy += dy;
            self.log.push(Cmd::Move { id, dx, dy });
        }
    }
}

fn scene() -> App {
    App {
        objs: vec![
            RectObj {
                id: 1,
                wx: 10.0,
                wy: 5.0,
                ww: 4.0,
                wh: 3.0,
            },
            RectObj {
                id: 2,
                wx: 30.0,
                wy: 10.0,
                ww: 4.0,
                wh: 3.0,
            },
        ],
        // 1:1 world<->cell so the test arithmetic is exact and legible.
        view: WorldView::new(Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        })
        .with_pixels_per_cell(1, 1),
        log: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Interaction session + dispatch (the app-side binding of input -> command)
// ---------------------------------------------------------------------------

struct Session {
    capture: PointerCapture,
    drag_start: Option<(f64, f64)>,
    selected: Option<u64>,
}

impl Session {
    fn new() -> Self {
        Self {
            capture: PointerCapture::new(),
            drag_start: None,
            selected: None,
        }
    }
}

/// Hit targets derived from the CURRENT view mapping of the app's world objects —
/// no hand-guessed rectangles (Law II, in miniature).
fn base_layers(app: &App) -> HitLayers {
    let mut l = HitLayers::new();
    for o in &app.objs {
        if let Some((cx, cy)) = app.view.world_to_cell(o.wx, o.wy).value().copied() {
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

/// Base targets plus a modal layer on top (occludes everything below it).
fn layers_with_modal(app: &App, modal: Rect) -> HitLayers {
    let mut l = base_layers(app);
    l.push_layer(true);
    l.add_target(TargetId(99), modal, 0);
    l
}

fn dispatch(app: &mut App, sess: &mut Session, layers: &HitLayers, ev: &Event) {
    match ev {
        Event::Mouse(m) => match m.kind {
            MouseKind::Down(MouseButton::Left) => {
                if let Some(id) = layers.hit_test(m.column, m.row) {
                    sess.capture.capture(id);
                    sess.selected = Some(id.0);
                    sess.drag_start = app.view.cell_to_world(m.column, m.row).value().copied();
                }
            }
            MouseKind::Up(MouseButton::Left) => {
                // route() revalidates the capture against the live layers: a modal
                // above, or a removed target, revokes it (defect B).
                if let Some(target) = sess.capture.route(layers, m.column, m.row) {
                    if let (Some((sx, sy)), Some((ex, ey))) = (
                        sess.drag_start,
                        app.view.cell_to_world(m.column, m.row).value().copied(),
                    ) {
                        let (dx, dy) = (ex - sx, ey - sy);
                        // A zero-delta press/release is a select, not an edit.
                        if dx != 0.0 || dy != 0.0 {
                            app.apply_move(target.0, dx, dy);
                        }
                    }
                }
                sess.capture.release();
                sess.drag_start = None;
            }
            _ => {
                // Drag/move: keep the capture warm; route still honours modal/removal.
                let _ = sess.capture.route(layers, m.column, m.row);
            }
        },
        Event::Key(k) => {
            if k.code == KeyCode::Esc {
                sess.capture.release();
                sess.drag_start = None;
            } else if let Some(id) = sess.selected {
                // Keyboard equivalent: one world unit per arrow, same command path.
                let delta = match k.code {
                    KeyCode::Left => Some((-1.0, 0.0)),
                    KeyCode::Right => Some((1.0, 0.0)),
                    KeyCode::Up => Some((0.0, -1.0)),
                    KeyCode::Down => Some((0.0, 1.0)),
                    _ => None,
                };
                if let Some((dx, dy)) = delta {
                    app.apply_move(id, dx, dy);
                }
            }
        }
        Event::Focus(FocusChange::Lost) => {
            sess.capture.cancel_on_focus_loss();
            sess.drag_start = None;
        }
        Event::Resize(_, _) => {
            // A relayout invalidates an in-flight drag rather than mis-applying an
            // old pointer position to a freshly laid-out object (Law III).
            sess.capture.cancel_on_resize();
            sess.drag_start = None;
        }
        _ => {}
    }
}

fn mouse(kind: MouseKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })
}

/// Drive an injected source through the same dispatch an interactive loop would use.
fn drain<S: EventSource>(src: &mut S, app: &mut App, sess: &mut Session) {
    let slice = Duration::from_millis(1);
    for _ in 0..10_000 {
        match src
            .poll_event(slice)
            .expect("scripted/replay source never errors")
        {
            Some(ev) => {
                let layers = base_layers(app);
                dispatch(app, sess, &layers, &ev);
            }
            None => break,
        }
    }
}

// ---------------------------------------------------------------------------
// Acceptance tests — each asserts on the observable command log
// ---------------------------------------------------------------------------

#[test]
fn mouse_drag_moves_rect_in_world_space_and_commits() {
    let mut app = scene();
    let mut sess = Session::new();

    // Press inside obj 1 (cells 10..14 x 5..8), drag, release 5 cells right.
    let layers = base_layers(&app);
    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &mouse(MouseKind::Down(MouseButton::Left), 11, 6),
    );
    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &mouse(MouseKind::Drag(MouseButton::Left), 13, 6),
    );
    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &mouse(MouseKind::Up(MouseButton::Left), 16, 6),
    );

    assert_eq!(
        app.log,
        vec![Cmd::Move {
            id: 1,
            dx: 5.0,
            dy: 0.0
        }]
    );
    assert_eq!(app.objs[0].wx, 15.0, "obj 1 moved +5 world units in x");
    assert_eq!(app.objs[0].wy, 5.0, "y unchanged");
}

#[test]
fn modal_blocks_background_edit() {
    let mut app = scene();
    let mut sess = Session::new();

    // Begin a drag on obj 1.
    let base = base_layers(&app);
    dispatch(
        &mut app,
        &mut sess,
        &base,
        &mouse(MouseKind::Down(MouseButton::Left), 11, 6),
    );
    assert_eq!(sess.capture.captured(), Some(TargetId(1)));

    // A modal opens above (a dialog in the top-left corner, NOT over the pointer).
    let modal = layers_with_modal(
        &app,
        Rect {
            x: 0,
            y: 0,
            width: 5,
            height: 2,
        },
    );
    dispatch(
        &mut app,
        &mut sess,
        &modal,
        &mouse(MouseKind::Up(MouseButton::Left), 16, 6),
    );

    assert!(
        app.log.is_empty(),
        "modal must revoke the stale capture: no background edit"
    );
    assert_eq!(
        app.objs[0].wx, 10.0,
        "obj 1 must not have moved under the modal"
    );
}

#[test]
fn esc_cancels_drag_without_committing() {
    let mut app = scene();
    let mut sess = Session::new();
    let layers = base_layers(&app);

    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &mouse(MouseKind::Down(MouseButton::Left), 11, 6),
    );
    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())),
    );

    assert!(app.log.is_empty(), "Esc mid-drag commits nothing");
    assert_eq!(sess.capture.captured(), None, "capture released by Esc");
}

#[test]
fn focus_loss_cancels_drag_without_committing() {
    let mut app = scene();
    let mut sess = Session::new();
    let layers = base_layers(&app);

    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &mouse(MouseKind::Down(MouseButton::Left), 11, 6),
    );
    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &Event::Focus(FocusChange::Lost),
    );

    assert!(app.log.is_empty(), "focus loss mid-drag commits nothing");
    assert_eq!(
        sess.capture.captured(),
        None,
        "capture released by focus loss"
    );
}

#[test]
fn keyboard_equivalent_moves_selected_object() {
    let mut app = scene();
    let mut sess = Session::new();
    let layers = base_layers(&app);

    // Click to select (zero-delta press/release commits nothing).
    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &mouse(MouseKind::Down(MouseButton::Left), 11, 6),
    );
    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &mouse(MouseKind::Up(MouseButton::Left), 11, 6),
    );
    assert!(
        app.log.is_empty(),
        "a click with no drag is a select, not an edit"
    );
    assert_eq!(sess.selected, Some(1));

    // Keyboard nudge: one world unit right, through the same command path.
    dispatch(
        &mut app,
        &mut sess,
        &layers,
        &Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty())),
    );

    assert_eq!(
        app.log,
        vec![Cmd::Move {
            id: 1,
            dx: 1.0,
            dy: 0.0
        }]
    );
    assert_eq!(app.objs[0].wx, 11.0);
}

#[test]
fn resize_mid_drag_does_not_mistarget() {
    let mut app = scene();
    let mut sess = Session::new();

    let before = base_layers(&app);
    dispatch(
        &mut app,
        &mut sess,
        &before,
        &mouse(MouseKind::Down(MouseButton::Left), 11, 6),
    );
    assert_eq!(sess.capture.captured(), Some(TargetId(1)));

    // Terminal resizes: the in-flight drag is cancelled.
    dispatch(&mut app, &mut sess, &before, &Event::Resize(100, 30));
    assert_eq!(sess.capture.captured(), None, "resize cancels the capture");

    // A release after the resize lands on a freshly-built hit map (obj 1 is at
    // 10..14; cell 16 is empty), so nothing is mis-edited.
    let after = base_layers(&app);
    dispatch(
        &mut app,
        &mut sess,
        &after,
        &mouse(MouseKind::Up(MouseButton::Left), 16, 6),
    );
    assert!(
        app.log.is_empty(),
        "no stale move applied to obj 1 after a resize"
    );
    assert_eq!(app.objs[0].wx, 10.0);
}

#[test]
fn scripted_source_drives_an_observed_edit_end_to_end() {
    // The same drag, but delivered entirely through an injected EventSource.
    let events = vec![
        mouse(MouseKind::Down(MouseButton::Left), 11, 6),
        mouse(MouseKind::Drag(MouseButton::Left), 13, 6),
        mouse(MouseKind::Up(MouseButton::Left), 16, 6),
    ];
    let mut src = ScriptedSource::from_events(events);
    let mut app = scene();
    let mut sess = Session::new();

    drain(&mut src, &mut app, &mut sess);

    assert_eq!(
        app.log,
        vec![Cmd::Move {
            id: 1,
            dx: 5.0,
            dy: 0.0
        }]
    );
}

#[test]
fn replay_reproduces_identical_action_log() {
    let events = vec![
        mouse(MouseKind::Down(MouseButton::Left), 11, 6),
        mouse(MouseKind::Drag(MouseButton::Left), 13, 6),
        mouse(MouseKind::Up(MouseButton::Left), 16, 6),
    ];

    // Run A: record the delivered trace while driving the app.
    let mut rec = Recorder::new(ScriptedSource::from_events(events));
    let mut app_a = scene();
    let mut sess_a = Session::new();
    drain(&mut rec, &mut app_a, &mut sess_a);
    let trace = rec.into_trace();

    // Run B: replay the captured trace through the SAME dispatch.
    let mut replay = trace.replay();
    let mut app_b = scene();
    let mut sess_b = Session::new();
    drain(&mut replay, &mut app_b, &mut sess_b);

    assert_eq!(
        app_a.log,
        vec![Cmd::Move {
            id: 1,
            dx: 5.0,
            dy: 0.0
        }]
    );
    assert_eq!(
        app_a.log, app_b.log,
        "replayed trace yields an identical command log"
    );
}

#[test]
fn context_poll_from_injects_events_headlessly() {
    // Defect G: terminal input is bound to a real TTY — a headless context's
    // `poll_event` cannot initialize an input reader — so `poll_from` is the
    // general injected path that drives any context, headless included.
    let mut ctx = Context::headless(RenderMode::Inline, 40, 8);

    // An injected EventSource drives the context through the same event path.
    let mut src =
        ScriptedSource::from_events(vec![Event::Key(KeyEvent::char('a')), Event::Resize(50, 10)]);
    let first = ctx.poll_from(&mut src, Duration::from_millis(1)).unwrap();
    let second = ctx.poll_from(&mut src, Duration::from_millis(1)).unwrap();
    let third = ctx.poll_from(&mut src, Duration::from_millis(1)).unwrap();

    assert_eq!(first, Some(Event::Key(KeyEvent::char('a'))));
    assert_eq!(second, Some(Event::Resize(50, 10)));
    assert_eq!(third, None);
}
