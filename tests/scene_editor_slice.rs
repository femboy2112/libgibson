//! Wave 3 — the first complete editor, composed entirely from public primitives.
//!
//! This is the payoff of the v0.5 interactive-instrument work: a tiny, app-owned
//! scene editor — a rectangular field with two chambers and an adjustable source —
//! built with NOTHING but LibGibson's public surface. No bespoke widget or event
//! framework is invented here; the point is that the shipped primitives already
//! compose into something a developer could actually edit a scene with.
//!
//! The composition, end to end:
//!   * `ui::compile` + `compute_layout` + `Compiled::hit_map` (Wave 2) resolve a
//!     pointer cell to the *painted* canvas target — so interaction can only reach
//!     a canvas that is actually on screen (Law L2);
//!   * `WorldView::cell_to_world` + `shape::Bounds::contains` pick the shape under
//!     that world point (identity transport, Law L3);
//!   * `DragGesture` groups a press/drag/release into ONE edit episode, cancelling
//!     on Escape / focus loss / resize / modal takeover (Law L5 preconditions);
//!   * a `Reversible` command enum + `UndoLog` make every edit a single undoable
//!     transaction; `UndoKeymap` maps Ctrl-Z/Y to undo/redo;
//!   * `NumericField` validates the bounded source parameter and REFUSES
//!     out-of-range input rather than clamping (Finding D's no-silent-clamp law).
//!
//! The headless oracle: the same operation expressed by pointer and by keyboard
//! reaches the *same canonical model state* (not the same raw trace), edits undo
//! and redo exactly, and a replayed event stream is deterministic.

use gibson::edit::{EditPhase, Reversible, UndoIntent, UndoKeymap, UndoLog};
use gibson::gesture::{DragGesture, DragIntent};
use gibson::hit_test::{HitLayers, TargetId};
use gibson::input::{
    Event, FocusChange, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseKind,
};
use gibson::numeric::NumericField;
use gibson::selection::Selection;
use gibson::shape::{Point, Shape, ShapeId};
use gibson::ui::prelude::*;
use gibson::world_view::{Transport, WorldView};
use gibson::{compute_layout, Color, Node, Rect};

// --- The app-owned model + its reversible commands ----------------------------

/// The undoable model: the shapes in draw order and the source parameter. The
/// editor's selection and view are presentation state and live outside it, so
/// undo never resurrects a stale selection.
#[derive(Debug, Clone, PartialEq)]
struct Scene {
    shapes: Vec<Shape>,
    source: f64,
}

impl Scene {
    fn shape(&self, id: ShapeId) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id() == id)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct EditFail;

/// A reversible edit. Every mutation the editor makes is one of these, so every
/// edit is one undo step and nothing mutates the model off the ledger.
#[derive(Debug, Clone)]
enum Cmd {
    Add(Shape),
    Move { id: ShapeId, dx: f64, dy: f64 },
    SetSource { from: f64, to: f64 },
}

fn move_shape(scene: &mut Scene, id: ShapeId, dx: f64, dy: f64) -> Result<(), EditFail> {
    let idx = scene
        .shapes
        .iter()
        .position(|s| s.id() == id)
        .ok_or(EditFail)?;
    let moved = scene.shapes[idx].translate(dx, dy).ok_or(EditFail)?;
    scene.shapes[idx] = moved;
    Ok(())
}

impl Reversible for Cmd {
    type Model = Scene;
    type Error = EditFail;

    fn apply(&self, scene: &mut Scene) -> Result<(), EditFail> {
        match self {
            Cmd::Add(s) => {
                scene.shapes.push(s.clone());
                Ok(())
            }
            Cmd::Move { id, dx, dy } => move_shape(scene, *id, *dx, *dy),
            Cmd::SetSource { to, .. } => {
                scene.source = *to;
                Ok(())
            }
        }
    }

    fn revert(&self, scene: &mut Scene) -> Result<(), EditFail> {
        match self {
            Cmd::Add(s) => {
                scene.shapes.retain(|x| x.id() != s.id());
                Ok(())
            }
            Cmd::Move { id, dx, dy } => move_shape(scene, *id, -*dx, -*dy),
            Cmd::SetSource { from, .. } => {
                scene.source = *from;
                Ok(())
            }
        }
    }
}

// --- The editor: the whole thing a consumer writes ----------------------------

struct Editor {
    scene: Scene,
    selection: Selection,
    view: WorldView,
    undo: UndoLog<Cmd>,
    drag: DragGesture,
    keymap: UndoKeymap,
    source_field: NumericField,
    // The painted spatial index and the canvas's target identity, resolved once
    // from the compiled UI. A real app recomputes these on resize.
    layers: HitLayers,
    canvas_id: TargetId,
    step: f64,
}

impl Editor {
    /// Builds the UI (a keyed canvas beside a param control), resolves its painted
    /// hit map, and anchors a 1-cell-per-world-unit view on the canvas rect.
    fn new(w: u16, h: u16) -> Self {
        let el: Element<()> = screen().child(
            row()
                .height(h)
                .child(
                    raw(Node::col()
                        .background(Color::Rgb(20, 20, 20))
                        .width(24.0)
                        .height(24.0))
                    .key("canvas")
                    .on_press(()),
                )
                .child(
                    raw(Node::col()
                        .background(Color::Rgb(40, 40, 40))
                        .width(10.0)
                        .height(3.0))
                    .key("param")
                    .on_press(()),
                ),
        );
        let env = UiEnvironment {
            width: w,
            height: h,
            ..UiEnvironment::default()
        };
        let cx = BuildCx::new(skins::SWISS_SIGNAL, env);
        let mut compiled = compile(&el, &cx).expect("scene UI compiles");
        compute_layout(&mut compiled.node, w, h).expect("layout resolves");
        let hit = compiled
            .hit_map(Rect::new(0, 0, w, h))
            .expect("hit map (layout ran)");
        let canvas = hit
            .targets()
            .iter()
            .find(|t| matches!(&t.key, Key::Named(s) if s == "canvas"))
            .expect("canvas target present");
        assert!(!canvas.rect.is_empty(), "canvas must paint to a real rect");
        // 1 cell == 1 world unit keeps a 1-cell pointer step and a keyboard nudge
        // the same displacement, which is what the equivalence oracle rests on.
        let view = WorldView::new(canvas.rect)
            .with_zoom(1.0, 1.0)
            .with_pixels_per_cell(1, 1);
        Self {
            scene: Scene {
                shapes: Vec::new(),
                source: 1.0,
            },
            selection: Selection::new(),
            view,
            undo: UndoLog::new(),
            drag: DragGesture::new(),
            keymap: UndoKeymap::standard(),
            source_field: NumericField::new().with_range(0.0, 10.0).with_step(1.0),
            layers: hit.layers().clone(),
            canvas_id: canvas.id,
            step: 1.0,
        }
    }

    // --- model mutation, always through the ledger ---

    fn record(&mut self, cmd: Cmd) {
        if cmd.apply(&mut self.scene).is_ok() {
            self.undo.record(cmd);
        }
    }

    fn add(&mut self, shape: Shape) {
        self.record(Cmd::Add(shape));
    }

    fn nudge(&mut self, dx: f64, dy: f64) {
        if let Some(id) = self.selection.primary() {
            self.record(Cmd::Move { id, dx, dy });
        }
    }

    fn adjust_source(&mut self, delta: f64) {
        let want = self.scene.source + delta;
        // Validated construction: an out-of-range value is REFUSED (no clamp), so
        // the model simply does not change — the honest Finding-D behaviour.
        if let Ok(v) = self.source_field.validate(want) {
            let from = self.scene.source;
            self.record(Cmd::SetSource { from, to: v });
        }
    }

    /// Topmost shape (last drawn) whose bounds contain `p`.
    fn pick(&self, p: Point) -> Option<ShapeId> {
        self.scene
            .shapes
            .iter()
            .rev()
            .find(|s| s.bounds().contains(p))
            .map(|s| s.id())
    }

    fn handle_phase(&mut self, phase: Option<EditPhase<DragIntent>>) {
        if let Some(EditPhase::Commit { intent, .. }) = phase {
            if let (
                Some(id),
                Transport::Mapped {
                    value: (sx, sy), ..
                },
                Transport::Mapped {
                    value: (cx, cy), ..
                },
            ) = (
                self.selection.primary(),
                intent.start_world,
                intent.current_world,
            ) {
                let (dx, dy) = (cx - sx, cy - sy);
                if dx != 0.0 || dy != 0.0 {
                    self.record(Cmd::Move { id, dx, dy });
                }
            }
        }
        // Begin / Preview / Cancel carry no authoritative change: a cancelled drag
        // (Escape, focus loss, resize, modal takeover) leaves the model untouched.
    }

    /// The whole input surface: one method, exactly as a real consumer's loop body.
    fn process(&mut self, event: &Event) {
        match event {
            Event::Key(ke) => {
                if let Some(intent) = self.keymap.classify(*ke) {
                    match intent {
                        UndoIntent::Undo => {
                            let _ = self.undo.undo(&mut self.scene);
                        }
                        UndoIntent::Redo => {
                            let _ = self.undo.redo(&mut self.scene);
                        }
                    }
                    return;
                }
                match ke.code {
                    KeyCode::Left => self.nudge(-self.step, 0.0),
                    KeyCode::Right => self.nudge(self.step, 0.0),
                    KeyCode::Up => self.nudge(0.0, -self.step),
                    KeyCode::Down => self.nudge(0.0, self.step),
                    KeyCode::Char('[') => self.adjust_source(-1.0),
                    KeyCode::Char(']') => self.adjust_source(1.0),
                    KeyCode::Esc => {
                        let p = self.drag.on_event(event, &self.layers, &self.view);
                        self.handle_phase(p);
                    }
                    _ => {}
                }
            }
            Event::Mouse(me) => {
                // On a press over the canvas, pick the shape under the pointer first
                // (selection is presentation state, set directly), then let the drag
                // machine open its episode on the canvas target.
                if let MouseKind::Down(MouseButton::Left) = me.kind {
                    if self.layers.hit_test(me.column, me.row) == Some(self.canvas_id) {
                        if let Transport::Mapped {
                            value: (wx, wy), ..
                        } = self.view.cell_to_world(me.column, me.row)
                        {
                            self.selection.clear();
                            if let Some(id) = self.pick(Point::new(wx, wy)) {
                                self.selection.insert(id);
                            }
                        }
                    }
                }
                let p = self.drag.on_event(event, &self.layers, &self.view);
                self.handle_phase(p);
            }
            Event::Focus(_) | Event::Resize(..) => {
                let p = self.drag.on_event(event, &self.layers, &self.view);
                self.handle_phase(p);
            }
            _ => {}
        }
    }

    // --- test conveniences ---

    /// The cell a world point maps to under the current view (panics if off-canvas,
    /// which would be a broken test fixture, not a library fault).
    fn cell_of(&self, p: Point) -> (u16, u16) {
        match self.view.world_to_cell(p.x, p.y) {
            Transport::Mapped { value, .. } => value,
            other => panic!("world point {p:?} does not map onto the canvas: {other:?}"),
        }
    }
}

// --- Fixture: seed the synthetic reaction scene -------------------------------

const FIELD: ShapeId = ShapeId(1);
const CHAMBER_A: ShapeId = ShapeId(2);
const CHAMBER_B: ShapeId = ShapeId(3);
const SOURCE: ShapeId = ShapeId(4);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Draws the field, two chambers and the source — exercising the Add path and
/// leaving a scene whose shapes nest (chambers and source inside the field).
fn seed(ed: &mut Editor) {
    ed.add(Shape::rect(FIELD, pt(0.0, 0.0), pt(20.0, 20.0)).unwrap());
    ed.add(Shape::rect(CHAMBER_A, pt(2.0, 2.0), pt(8.0, 8.0)).unwrap());
    ed.add(Shape::rect(CHAMBER_B, pt(12.0, 2.0), pt(18.0, 8.0)).unwrap());
    ed.add(Shape::rect(SOURCE, pt(9.0, 9.0), pt(11.0, 11.0)).unwrap());
}

fn down(c: u16, r: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseKind::Down(MouseButton::Left),
        column: c,
        row: r,
        modifiers: KeyModifiers::empty(),
    })
}
fn drag_to(c: u16, r: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseKind::Drag(MouseButton::Left),
        column: c,
        row: r,
        modifiers: KeyModifiers::empty(),
    })
}
fn up(c: u16, r: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseKind::Up(MouseButton::Left),
        column: c,
        row: r,
        modifiers: KeyModifiers::empty(),
    })
}
fn press(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}
fn ctrl(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn draws_every_shape_kind_and_the_synthetic_scene() {
    let mut ed = Editor::new(48, 30);
    seed(&mut ed);
    assert_eq!(ed.scene.shapes.len(), 4);
    assert!(ed.scene.shape(SOURCE).is_some());
    // The other two primitive kinds compose through the same Add path.
    ed.add(Shape::line(ShapeId(5), pt(0.0, 0.0), pt(5.0, 5.0)).unwrap());
    ed.add(Shape::stroke(ShapeId(6), vec![pt(1.0, 1.0), pt(2.0, 3.0), pt(4.0, 2.0)]).unwrap());
    assert_eq!(ed.scene.shapes.len(), 6);
}

#[test]
fn pointer_over_the_painted_canvas_selects_the_shape_under_it() {
    let mut ed = Editor::new(48, 30);
    seed(&mut ed);

    // A press on the source's centre selects the source (topmost at that point).
    let (c, r) = ed.cell_of(pt(10.0, 10.0));
    ed.process(&down(c, r));
    ed.process(&up(c, r)); // a click: down+up, zero-delta -> selection only, no move
    assert_eq!(ed.selection.primary(), Some(SOURCE));

    // A press on chamber A's centre selects chamber A, not the field beneath it.
    let (c, r) = ed.cell_of(pt(5.0, 5.0));
    ed.process(&down(c, r));
    ed.process(&up(c, r));
    assert_eq!(ed.selection.primary(), Some(CHAMBER_A));

    // A press on bare field selects the field.
    let (c, r) = ed.cell_of(pt(1.0, 1.0));
    ed.process(&down(c, r));
    ed.process(&up(c, r));
    assert_eq!(ed.selection.primary(), Some(FIELD));

    // Pure selection (three zero-delta clicks) records no edits: only the 4 seeds.
    assert_eq!(ed.undo.undo_depth(), 4, "selection must not record edits");
}

#[test]
fn a_pointer_drag_moves_the_selected_shape_as_one_transaction() {
    let mut ed = Editor::new(48, 30);
    seed(&mut ed);
    let before = ed.scene.shape(SOURCE).cloned().unwrap();
    let depth_before = ed.undo.undo_depth();

    let (c, r) = ed.cell_of(pt(10.0, 10.0));
    ed.process(&down(c, r)); // select + begin
    ed.process(&drag_to(c + 1, r));
    ed.process(&drag_to(c + 2, r));
    ed.process(&up(c + 3, r)); // commit: +3 cells == +3 world units

    let after = ed.scene.shape(SOURCE).cloned().unwrap();
    assert_ne!(after, before, "the source must have moved");
    // The whole drag collapsed into exactly ONE undo entry.
    assert_eq!(ed.undo.undo_depth(), depth_before + 1);
    // One undo restores the pre-drag geometry exactly.
    let _ = ed.undo.undo(&mut ed.scene);
    assert_eq!(ed.scene.shape(SOURCE).cloned().unwrap(), before);
}

#[test]
fn keyboard_and_pointer_reach_the_same_canonical_state() {
    // Pointer: drag the source +3 cells right.
    let mut a = Editor::new(48, 30);
    seed(&mut a);
    let (c, r) = a.cell_of(pt(10.0, 10.0));
    a.process(&down(c, r));
    a.process(&up(c + 3, r));

    // Keyboard: select the source with a click, then three Right nudges.
    let mut b = Editor::new(48, 30);
    seed(&mut b);
    let (c, r) = b.cell_of(pt(10.0, 10.0));
    b.process(&down(c, r));
    b.process(&up(c, r)); // click selects, no move
    b.process(&press(KeyCode::Right));
    b.process(&press(KeyCode::Right));
    b.process(&press(KeyCode::Right));

    // Same canonical model state, though the raw traces and undo granularity differ.
    assert_eq!(
        a.scene.shape(SOURCE).unwrap(),
        b.scene.shape(SOURCE).unwrap(),
        "pointer and keyboard must agree on the resulting geometry"
    );
}

#[test]
fn numeric_param_commits_in_range_and_refuses_out_of_range() {
    let mut ed = Editor::new(48, 30);
    seed(&mut ed);
    assert_eq!(ed.scene.source, 1.0);

    ed.process(&press(KeyCode::Char(']'))); // 1 -> 2 (valid)
    assert_eq!(ed.scene.source, 2.0);
    ed.process(&press(KeyCode::Char('['))); // 2 -> 1 (valid)
    assert_eq!(ed.scene.source, 1.0);

    // Drive to the max, then one past it: the over-range step is REFUSED, not
    // clamped — the value simply stays at the ceiling.
    for _ in 0..20 {
        ed.process(&press(KeyCode::Char(']')));
    }
    assert_eq!(
        ed.scene.source, 10.0,
        "source saturates at its validated max"
    );

    // And below the floor.
    for _ in 0..20 {
        ed.process(&press(KeyCode::Char('[')));
    }
    assert_eq!(
        ed.scene.source, 0.0,
        "source saturates at its validated min"
    );
}

#[test]
fn undo_and_redo_round_trip_the_whole_session() {
    let mut ed = Editor::new(48, 30);
    seed(&mut ed);
    let seeded = ed.scene.clone();

    // A move and a param change.
    let (c, r) = ed.cell_of(pt(5.0, 5.0));
    ed.process(&down(c, r));
    ed.process(&up(c + 2, r)); // move chamber A +2
    ed.process(&press(KeyCode::Char(']'))); // source 1 -> 2
    let edited = ed.scene.clone();
    assert_ne!(edited, seeded);

    // Undo both edits -> back to the seeded scene.
    let _ = ed.undo.undo(&mut ed.scene);
    let _ = ed.undo.undo(&mut ed.scene);
    assert_eq!(
        ed.scene, seeded,
        "undo must restore the seeded scene exactly"
    );

    // Redo both -> back to the edited scene.
    let _ = ed.undo.redo(&mut ed.scene);
    let _ = ed.undo.redo(&mut ed.scene);
    assert_eq!(
        ed.scene, edited,
        "redo must restore the edited scene exactly"
    );
}

#[test]
fn ctrl_z_and_ctrl_y_drive_undo_and_redo() {
    let mut ed = Editor::new(48, 30);
    seed(&mut ed);
    let seeded = ed.scene.clone();
    ed.process(&press(KeyCode::Char(']'))); // source 1 -> 2
    assert_eq!(ed.scene.source, 2.0);
    ed.process(&ctrl('z'));
    assert_eq!(ed.scene, seeded);
    ed.process(&ctrl('y'));
    assert_eq!(ed.scene.source, 2.0);
}

#[test]
fn escape_cancels_a_drag_and_leaves_the_model_untouched() {
    let mut ed = Editor::new(48, 30);
    seed(&mut ed);
    let before = ed.scene.clone();
    let depth = ed.undo.undo_depth();

    let (c, r) = ed.cell_of(pt(10.0, 10.0));
    ed.process(&down(c, r)); // select + begin
    ed.process(&drag_to(c + 3, r)); // in-flight preview
    ed.process(&press(KeyCode::Esc)); // cancel

    assert!(!ed.drag.is_active(), "the drag episode must be closed");
    assert_eq!(ed.scene, before, "a cancelled drag records no edit");
    assert_eq!(
        ed.undo.undo_depth(),
        depth,
        "no undo entry from a cancelled drag"
    );
}

#[test]
fn focus_loss_cancels_an_in_flight_drag() {
    let mut ed = Editor::new(48, 30);
    seed(&mut ed);
    let before = ed.scene.clone();

    let (c, r) = ed.cell_of(pt(15.0, 5.0));
    ed.process(&down(c, r));
    ed.process(&drag_to(c + 2, r));
    ed.process(&Event::Focus(FocusChange::Lost));

    assert!(!ed.drag.is_active());
    assert_eq!(ed.scene, before, "focus loss ends the drag with no edit");
}

#[test]
fn panning_the_view_keeps_pointer_picks_correct() {
    let mut ed = Editor::new(48, 30);
    seed(&mut ed);
    // Pan the world; the pick must follow the pan, not the stale screen position.
    ed.view = ed.view.pan_by(3.0, 2.0);

    // Chamber B's centre is world (15,5); after the pan its cell moves, and a click
    // on that new cell must still resolve to chamber B.
    let (c, r) = ed.cell_of(pt(15.0, 5.0));
    ed.process(&down(c, r));
    ed.process(&up(c, r));
    assert_eq!(ed.selection.primary(), Some(CHAMBER_B));
}

#[test]
fn replaying_the_same_events_is_deterministic() {
    let events = vec![
        down(0, 0),
        up(0, 0),
        press(KeyCode::Char(']')),
        press(KeyCode::Right),
    ];
    let run = || {
        let mut ed = Editor::new(48, 30);
        seed(&mut ed);
        // Select the source first (so the Right nudge has a subject), then replay.
        let (c, r) = ed.cell_of(pt(10.0, 10.0));
        ed.process(&down(c, r));
        ed.process(&up(c, r));
        for e in &events {
            ed.process(e);
        }
        ed.scene
    };
    assert_eq!(
        run(),
        run(),
        "the same event stream must yield the same model"
    );
}
