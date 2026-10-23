//! Wave 3, second deliverable: the scene editor driven through a REAL terminal.
//!
//! `tests/scene_editor_slice.rs` is the load-bearing headless proof that the v0.5
//! interactive-instrument primitives compose into an editor. This binary is the
//! other half the plan asks for — the **PTY walkthrough**: the *same* composition,
//! now wired to a live terminal through [`gibson::Context`], so a person (or the
//! `tests/scene_editor_pty.rs` harness) can actually drive it with real keystrokes
//! and a real mouse and watch the painted frame change.
//!
//! Everything here is still built from nothing but LibGibson's public surface:
//!
//!   * [`Context`] owns the terminal session (alt-screen, raw mode, bracketed
//!     paste) and renders the UI; opt-in [`input::set_mouse_capture`] /
//!     [`input::set_focus_reporting`] turn pointer and focus reporting on for the
//!     session and are turned back off on exit (the Finding-F session-owned
//!     lifecycle, proven restored by the PTY test);
//!   * `ui::compile` + `compute_layout` + [`Compiled::hit_map`] resolve the pointer
//!     to the *painted* canvas target (Law L2);
//!   * [`WorldView::cell_to_world`] + [`shape::Bounds::contains`] pick the shape
//!     under that world point — the SAME mapping used to rasterise the canvas, so
//!     what you see and what you can click agree by construction (Law L3);
//!   * [`DragGesture`] groups a drag into one episode and cancels it on
//!     Escape / focus-loss / resize; a [`Reversible`] `Cmd` + [`UndoLog`] make
//!     every edit one undo step; [`UndoKeymap`] drives Ctrl-Z/Y; [`NumericField`]
//!     refuses an out-of-range source value instead of clamping it (Finding D).
//!
//! The HUD and inspector render the editor's *canonical model state* as plain
//! text, so the walkthrough is observable without scraping glyphs: it is exactly
//! what a human reads off the screen.
//!
//! Controls: arrow keys move the selection · Tab cycles the selection ·
//! `[` / `]` adjust the bounded source · Ctrl-Z / Ctrl-Y undo/redo ·
//! left-click picks the shape under the pointer · `q` (or Ctrl-C) quits.

use gibson::cell::Style;
use gibson::edit::{EditPhase, Reversible, UndoIntent, UndoKeymap, UndoLog};
use gibson::gesture::{DragGesture, DragIntent};
use gibson::hit_test::{HitLayers, TargetId};
use gibson::input::{self, Event, KeyCode, MouseButton, MouseKind};
use gibson::numeric::NumericField;
use gibson::selection::Selection;
use gibson::shape::{Point, Shape, ShapeId};
use gibson::ui::prelude::*;
use gibson::world_view::{Transport, WorldView};
use gibson::{compute_layout, Color, Context, Node, Rect, RenderMode};
use std::io::{self, Write};
use std::time::Duration;

// The canvas is a fixed size so the painted rect is deterministic across frames
// and terminal sizes; the editor discloses it in the HUD regardless.
const CANVAS_W: u16 = 24;
const CANVAS_H: u16 = 24;

// --- The app-owned model + its reversible commands (mirrors the headless slice) --

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

// --- Identity → name, for the human-readable HUD / inspector -------------------

const FIELD: ShapeId = ShapeId(1);
const CHAMBER_A: ShapeId = ShapeId(2);
const CHAMBER_B: ShapeId = ShapeId(3);
const SOURCE: ShapeId = ShapeId(4);

fn name_of(id: ShapeId) -> String {
    match id {
        FIELD => "FIELD".into(),
        CHAMBER_A => "CHAMBER_A".into(),
        CHAMBER_B => "CHAMBER_B".into(),
        SOURCE => "SOURCE".into(),
        ShapeId(n) => format!("shape#{n}"),
    }
}

/// Integers print without a trailing `.0`, so `source=2`, not `source=2.0`.
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn kind_name(s: &Shape) -> &'static str {
    use gibson::shape::ShapeKind::*;
    match s.kind() {
        Rect => "rect",
        Line => "line",
        Stroke => "stroke",
    }
}

// --- The editor ----------------------------------------------------------------

struct Editor {
    scene: Scene,
    selection: Selection,
    view: WorldView,
    undo: UndoLog<Cmd>,
    drag: DragGesture,
    keymap: UndoKeymap,
    source_field: NumericField,
    layers: HitLayers,
    canvas_id: TargetId,
    canvas_rect: Rect,
    step: f64,
    last: String,
}

impl Editor {
    fn new(w: u16, h: u16) -> Self {
        let (rect, layers, canvas_id) = Self::resolve_canvas(w, h);
        let view = WorldView::new(rect)
            .with_zoom(1.0, 1.0)
            .with_pixels_per_cell(1, 1);
        let mut ed = Self {
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
            layers,
            canvas_id,
            canvas_rect: rect,
            step: 1.0,
            last: "ready".into(),
        };
        ed.seed();
        ed
    }

    /// Lays out the structural UI (content-independent, so the rect is stable) and
    /// returns the painted canvas rect, its hit layers and its target id.
    fn resolve_canvas(w: u16, h: u16) -> (Rect, HitLayers, TargetId) {
        let el = Self::build_el(w, h, Vec::new(), "", &[], "");
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
        (canvas.rect, hit.layers().clone(), canvas.id)
    }

    fn seed(&mut self) {
        let p = Point::new;
        self.add(Shape::rect(FIELD, p(0.0, 0.0), p(20.0, 20.0)).unwrap());
        self.add(Shape::rect(CHAMBER_A, p(2.0, 2.0), p(8.0, 8.0)).unwrap());
        self.add(Shape::rect(CHAMBER_B, p(12.0, 2.0), p(18.0, 8.0)).unwrap());
        self.add(Shape::rect(SOURCE, p(9.0, 9.0), p(11.0, 11.0)).unwrap());
        self.last = "ready".into();
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
            self.last = format!("moved {} by ({},{})", name_of(id), fmt_num(dx), fmt_num(dy));
        } else {
            self.last = "nothing selected".into();
        }
    }

    fn adjust_source(&mut self, delta: f64) {
        let want = self.scene.source + delta;
        match self.source_field.validate(want) {
            Ok(v) => {
                let from = self.scene.source;
                self.record(Cmd::SetSource { from, to: v });
                self.last = format!("source {} -> {}", fmt_num(from), fmt_num(v));
            }
            Err(e) => {
                // Refused, not clamped: the model does not change (Finding D).
                self.last = format!("source refused: {e}");
            }
        }
    }

    fn cycle_select(&mut self) {
        if self.scene.shapes.is_empty() {
            return;
        }
        let ids: Vec<ShapeId> = self.scene.shapes.iter().map(|s| s.id()).collect();
        let next = match self.selection.primary() {
            None => ids[0],
            Some(cur) => {
                let i = ids
                    .iter()
                    .position(|&id| id == cur)
                    .map(|i| (i + 1) % ids.len())
                    .unwrap_or(0);
                ids[i]
            }
        };
        self.selection.clear();
        self.selection.insert(next);
        self.last = format!("selected {}", name_of(next));
    }

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
                    self.last = format!(
                        "dragged {} by ({},{})",
                        name_of(id),
                        fmt_num(dx),
                        fmt_num(dy)
                    );
                }
            }
        }
    }

    /// The whole input surface — exactly a consumer's event-loop body.
    fn process(&mut self, event: &Event) {
        match event {
            Event::Key(ke) => {
                if let Some(intent) = self.keymap.classify(*ke) {
                    match intent {
                        UndoIntent::Undo => {
                            let _ = self.undo.undo(&mut self.scene);
                            self.last = "undo".into();
                        }
                        UndoIntent::Redo => {
                            let _ = self.undo.redo(&mut self.scene);
                            self.last = "redo".into();
                        }
                    }
                    return;
                }
                match ke.code {
                    KeyCode::Left => self.nudge(-self.step, 0.0),
                    KeyCode::Right => self.nudge(self.step, 0.0),
                    KeyCode::Up => self.nudge(0.0, -self.step),
                    KeyCode::Down => self.nudge(0.0, self.step),
                    KeyCode::Tab => self.cycle_select(),
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
                if let MouseKind::Down(MouseButton::Left) = me.kind {
                    if self.layers.hit_test(me.column, me.row) == Some(self.canvas_id) {
                        if let Transport::Mapped {
                            value: (wx, wy), ..
                        } = self.view.cell_to_world(me.column, me.row)
                        {
                            self.selection.clear();
                            match self.pick(Point::new(wx, wy)) {
                                Some(id) => {
                                    self.selection.insert(id);
                                    self.last = format!("picked {}", name_of(id));
                                }
                                None => self.last = "picked nothing".into(),
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

    // --- rendering ---

    /// One glyph for the cell at world point `p`: the topmost shape's fill, or a
    /// space. The selected shape fills with `#`. This reuses the exact pick walk,
    /// so a glyph you see is a glyph you can click.
    fn glyph_at(&self, p: Point) -> char {
        const PALETTE: [char; 6] = ['.', 'o', 'x', '+', '=', '~'];
        for (idx, s) in self.scene.shapes.iter().enumerate().rev() {
            if s.bounds().contains(p) {
                if self.selection.primary() == Some(s.id()) {
                    return '#';
                }
                return PALETTE[idx % PALETTE.len()];
            }
        }
        ' '
    }

    fn canvas_rows(&self) -> Vec<Node> {
        let r = self.canvas_rect;
        let fill = Style::new()
            .fg(Color::Rgb(200, 200, 200))
            .bg(Color::Rgb(16, 16, 16));
        let mut rows = Vec::with_capacity(r.height as usize);
        for ry in 0..r.height {
            let mut line = String::with_capacity(r.width as usize);
            for rx in 0..r.width {
                let (col, row) = (r.x + rx, r.y + ry);
                let g = match self.view.cell_to_world(col, row) {
                    Transport::Mapped {
                        value: (wx, wy), ..
                    } => self.glyph_at(Point::new(wx, wy)),
                    _ => ' ',
                };
                line.push(g);
            }
            rows.push(Node::text(line, fill));
        }
        rows
    }

    fn hud_line(&self) -> String {
        let sel = self
            .selection
            .primary()
            .map(name_of)
            .unwrap_or_else(|| "none".into());
        let r = self.canvas_rect;
        format!(
            "SCENE EDITOR  sel={}  source={}  shapes={}  undo={} redo={}  canvas={},{},{},{}  {}",
            sel,
            fmt_num(self.scene.source),
            self.scene.shapes.len(),
            self.undo.undo_depth(),
            self.undo.redo_depth(),
            r.x,
            r.y,
            r.width,
            r.height,
            self.last,
        )
    }

    fn inspector_lines(&self) -> Vec<String> {
        let mut out = vec!["shapes:".to_string()];
        for s in &self.scene.shapes {
            let b = s.bounds();
            let marker = if self.selection.primary() == Some(s.id()) {
                ">"
            } else {
                " "
            };
            out.push(format!(
                "{} {} {} ({},{})-({},{})",
                marker,
                name_of(s.id()),
                kind_name(s),
                fmt_num(b.min.x),
                fmt_num(b.min.y),
                fmt_num(b.max.x),
                fmt_num(b.max.y),
            ));
        }
        out
    }

    /// Builds the whole UI element tree. Used both for the structural resolve
    /// (empty content) and for each rendered frame.
    fn build_el(
        w: u16,
        h: u16,
        canvas_rows: Vec<Node>,
        hud: &str,
        inspector: &[String],
        help: &str,
    ) -> Element<()> {
        let canvas = Node::col()
            .width(CANVAS_W as f32)
            .height(CANVAS_H as f32)
            .background(Color::Rgb(16, 16, 16))
            .children(canvas_rows);
        let mut inspector_col = column::<()>();
        for line in inspector {
            inspector_col = inspector_col.child(text(line.clone()));
        }
        screen().child(
            column::<()>()
                .width(w)
                .height(h)
                .child(text(hud.to_string()))
                .child(
                    row::<()>()
                        .child(raw(canvas).key("canvas").on_press(()))
                        .child(inspector_col),
                )
                .child(text(help.to_string())),
        )
    }
}

// --- the live loop -------------------------------------------------------------

fn is_quit(event: &Event) -> bool {
    use gibson::input::KeyModifiers;
    matches!(event,
        Event::Key(k) if (k.code == KeyCode::Char('q') && k.modifiers.is_empty())
            || (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL)))
}

fn render(ed: &mut Editor, ctx: &mut Context, w: u16, h: u16) -> io::Result<()> {
    // Rebuild the element, recompute layout + hit map, and refresh the hit layers
    // from the painted frame (so pointer targeting tracks the real canvas).
    let hud = ed.hud_line();
    let inspector = ed.inspector_lines();
    let help = "keys: arrows=move  Tab=select  [ ]=source  Ctrl-Z/Y=undo/redo  click=pick  q=quit";
    let el = Editor::build_el(w, h, ed.canvas_rows(), &hud, &inspector, help);
    let env = UiEnvironment {
        width: w,
        height: h,
        ..UiEnvironment::default()
    };
    let cx = BuildCx::new(skins::SWISS_SIGNAL, env);
    let mut compiled = compile(&el, &cx).expect("scene UI compiles");
    compute_layout(&mut compiled.node, w, h).expect("layout resolves");
    if let Ok(hit) = compiled.hit_map(Rect::new(0, 0, w, h)) {
        if let Some(canvas) = hit
            .targets()
            .iter()
            .find(|t| matches!(&t.key, Key::Named(s) if s == "canvas"))
        {
            ed.canvas_id = canvas.id;
            ed.canvas_rect = canvas.rect;
        }
        ed.layers = hit.layers().clone();
    }
    ctx.set_root(compiled.node);
    ctx.render()?;
    Ok(())
}

fn main() -> io::Result<()> {
    let mut ctx = Context::new(RenderMode::Fullscreen)?;
    let (w, h) = ctx.session.terminal_size();

    // Opt into pointer and focus reporting for the session (Finding F): on now,
    // off in the teardown below, which the PTY test checks is honoured.
    input::set_mouse_capture(true)?;
    input::set_focus_reporting(true)?;

    let mut ed = Editor::new(w, h);
    let mut w = w;
    let mut h = h;
    render(&mut ed, &mut ctx, w, h)?;

    let result = (|| -> io::Result<()> {
        loop {
            if let Some(event) = ctx.poll_event(Duration::from_millis(100))? {
                if is_quit(&event) {
                    return Ok(());
                }
                if let Event::Resize(cols, rows) = event {
                    w = cols;
                    h = rows;
                }
                ed.process(&event);
                render(&mut ed, &mut ctx, w, h)?;
            }
        }
    })();

    // Session-owned teardown: drop pointer/focus reporting, then restore the
    // terminal (alt-screen, bracketed paste, cursor, raw mode). `restore` is a
    // one-shot latch, so the Context's Drop is then a clean no-op.
    let _ = input::set_mouse_capture(false);
    let _ = input::set_focus_reporting(false);
    io::stdout().flush().ok();
    ctx.session.restore()?;

    result
}
