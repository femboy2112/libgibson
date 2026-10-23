#![allow(dead_code)]
//! Wave 6 — the instrument workbench CORE (#144), as pure consumer code.
//!
//! This module is composed from **public `gibson` primitives only** — it adds no
//! crate symbols and knows nothing of any domain (there is no chemistry, material,
//! or physics here; the field is a synthetic moving Gaussian). It is shared, byte
//! for byte, by three independent consumers:
//!
//!   * `examples/instrument_workbench/main.rs` — the runnable real-terminal flagship;
//!   * `tests/instrument_workbench_slice.rs`   — the headless seam oracles;
//!   * (indirectly) `tests/instrument_workbench_pty.rs`, which drives the built binary.
//!
//! One authoritative composition, three consumers: the strongest proof that the
//! primitives compose into a real interactive instrument without a bespoke
//! widget/event framework.
//!
//! ## The laws it keeps
//! * **L1 presentation purity** — rendering/probing never mutates the model.
//! * **L2 target soundness** — a pointer targets a shape only through the *painted*
//!   hit-map (`Compiled::hit_map`) and the painted canvas rect; never an app rect.
//! * **L3 identity transport** — world↔cell↔field-index go through the same
//!   `WorldView`/`FieldExtent` the renderer uses; clipping is reported, not faked.
//! * **L4 command admission** — every edit is one recorded transaction.
//! * **L5 transaction truth** — undo/redo is a real inverse via `Reversible`.
//! * **L6 time separation** — the field's model tick is its own clock, never wall time.
//! * **L7 receipt composition** — field/probe receipts disclose masked/non-finite/clipped.

use std::time::Duration;

use gibson::edit::{Compound, EditPhase, Reversible, UndoKeymap, UndoLog};
use gibson::field_view::{
    Colormap, FieldExtent, FieldFrame, FieldView, ProbeReport, RangePolicy, Sample,
};
use gibson::gesture::{DragGesture, DragIntent};
use gibson::hit_test::{HitLayers, TargetId};
use gibson::input::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseKind};
use gibson::numeric::{NumericError, NumericField};
use gibson::selection::Selection;
use gibson::shape::{Point, Shape, ShapeId, ShapeKind};
use gibson::ui::prelude::*;
use gibson::world_view::{Transport, WorldView};
use gibson::{compute_layout, FixedTimestep, Node, Rect, TickDemand};

// ---- synthetic world geometry (domain-neutral) ---------------------------------------

/// Field grid width in cells.
pub const FIELD_COLS: usize = 24;
/// Field grid height in cells.
pub const FIELD_ROWS: usize = 12;
/// Fixed model step: one field tick = 16 ms of app-owned logical time.
pub const FIELD_STEP: Duration = Duration::from_millis(16);
/// Source parameter bounds (a bounded numeric property, not a material quantity).
pub const SOURCE_MIN: f64 = 0.0;
pub const SOURCE_MAX: f64 = 10.0;
pub const SOURCE_STEP: f64 = 1.0;

/// The field's fixed extent: origin at world (0,0), one world unit per cell.
pub fn field_extent() -> FieldExtent {
    FieldExtent::new(Point::new(0.0, 0.0), 1.0, 1.0, FIELD_COLS, FIELD_ROWS)
        .expect("constant field extent is valid")
}

/// The closed-form field value at a cell for a given tick and source amplitude: a
/// moving Gaussian whose centre orbits with the model tick. Pure, deterministic, and
/// domain-neutral — this is the only "physics" in the workbench, and it is invented.
fn field_value_at(source_value: f64, ix: usize, iy: usize, tick: u64) -> f64 {
    let (cols, rows) = (FIELD_COLS as f64, FIELD_ROWS as f64);
    let t = tick as f64;
    let omega = 0.05_f64;
    let cx = cols / 2.0 + (cols / 4.0) * (omega * t).cos();
    let cy = rows / 2.0 + (rows / 4.0) * (omega * t).sin();
    let sigma = 2.5_f64;
    let two_s2 = 2.0 * sigma * sigma;
    let dx = ix as f64 + 0.5 - cx;
    let dy = iy as f64 + 0.5 - cy;
    source_value * (-(dx * dx + dy * dy) / two_s2).exp()
}

// ---- the undoable model + its reversible commands (L4/L5) -----------------------------

/// The authoritative, undoable slice of state. Everything a `Cmd` touches lives here.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneModel {
    pub shapes: Vec<Shape>,
    pub source_value: f64,
}

impl SceneModel {
    fn pos_of(&self, id: ShapeId) -> Option<usize> {
        self.shapes.iter().position(|s| s.id() == id)
    }
}

/// Why a command could not be applied/reverted — surfaced, never swallowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdError {
    ShapeNotFound,
    BadGeometry,
}

/// A single reversible edit. Grouped edits are a [`Compound`] of these, so EVERY
/// recorded transaction is a `Compound<Cmd>` and `Compound`'s honest atomic rollback
/// (Wave 1 / Finding B) is load-bearing for even a one-shape edit.
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    AddShape(Shape),
    Move { id: ShapeId, dx: f64, dy: f64 },
    SetSource { from: f64, to: f64 },
}

fn translate_in_place(m: &mut SceneModel, id: ShapeId, dx: f64, dy: f64) -> Result<(), CmdError> {
    let pos = m.pos_of(id).ok_or(CmdError::ShapeNotFound)?;
    let moved = m.shapes[pos]
        .translate(dx, dy)
        .ok_or(CmdError::BadGeometry)?;
    m.shapes[pos] = moved;
    Ok(())
}

impl Reversible for Cmd {
    type Model = SceneModel;
    type Error = CmdError;

    fn apply(&self, model: &mut SceneModel) -> Result<(), CmdError> {
        match self {
            Cmd::AddShape(s) => {
                model.shapes.push(s.clone());
                Ok(())
            }
            Cmd::Move { id, dx, dy } => translate_in_place(model, *id, *dx, *dy),
            Cmd::SetSource { to, .. } => {
                model.source_value = *to;
                Ok(())
            }
        }
    }

    fn revert(&self, model: &mut SceneModel) -> Result<(), CmdError> {
        match self {
            Cmd::AddShape(s) => {
                let pos = model.pos_of(s.id()).ok_or(CmdError::ShapeNotFound)?;
                model.shapes.remove(pos);
                Ok(())
            }
            Cmd::Move { id, dx, dy } => translate_in_place(model, *id, -*dx, -*dy),
            Cmd::SetSource { from, .. } => {
                model.source_value = *from;
                Ok(())
            }
        }
    }
}

// ---- intents: the ONE canonical surface both pointer and keyboard route through -------

/// Transport over the app-owned deterministic field (L6: its own chronology).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportCmd {
    Play,
    Pause,
    Step(i64),
    Scrub(u64),
}

/// A semantic intent. Pointer events and keyboard events are both lowered to these,
/// so "the same operation" from either device produces the identical model transition
/// (keyboard ≡ pointer equivalence). Only [`Workbench::apply_intent`] mutates state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Intent {
    DrawRect { a: Point, b: Point },
    DrawLine { a: Point, b: Point },
    SelectAt(Point),
    ClearSelection,
    MoveSelected { dx: f64, dy: f64 },
    SetSource(f64),
    Pan { dx: f64, dy: f64 },
    Zoom { factor: f64 },
    ProbeAt(Point),
    Transport(TransportCmd),
    Undo,
    Redo,
    Capture,
}

impl Intent {
    /// A canonical byte encoding, hashed into the command digest. Two runs that apply
    /// the same intents in the same order get the same command digest.
    fn canonical(&self) -> Vec<u8> {
        let mut v = Vec::new();
        let f = |v: &mut Vec<u8>, x: f64| v.extend_from_slice(&x.to_bits().to_le_bytes());
        match *self {
            Intent::DrawRect { a, b } => {
                v.push(1);
                f(&mut v, a.x);
                f(&mut v, a.y);
                f(&mut v, b.x);
                f(&mut v, b.y);
            }
            Intent::DrawLine { a, b } => {
                v.push(2);
                f(&mut v, a.x);
                f(&mut v, a.y);
                f(&mut v, b.x);
                f(&mut v, b.y);
            }
            Intent::SelectAt(p) => {
                v.push(3);
                f(&mut v, p.x);
                f(&mut v, p.y);
            }
            Intent::ClearSelection => v.push(4),
            Intent::MoveSelected { dx, dy } => {
                v.push(5);
                f(&mut v, dx);
                f(&mut v, dy);
            }
            Intent::SetSource(x) => {
                v.push(6);
                f(&mut v, x);
            }
            Intent::Pan { dx, dy } => {
                v.push(7);
                f(&mut v, dx);
                f(&mut v, dy);
            }
            Intent::Zoom { factor } => {
                v.push(8);
                f(&mut v, factor);
            }
            Intent::ProbeAt(p) => {
                v.push(9);
                f(&mut v, p.x);
                f(&mut v, p.y);
            }
            Intent::Transport(tc) => {
                v.push(10);
                match tc {
                    TransportCmd::Play => v.push(0),
                    TransportCmd::Pause => v.push(1),
                    TransportCmd::Step(n) => {
                        v.push(2);
                        v.extend_from_slice(&n.to_le_bytes());
                    }
                    TransportCmd::Scrub(t) => {
                        v.push(3);
                        v.extend_from_slice(&t.to_le_bytes());
                    }
                }
            }
            Intent::Undo => v.push(11),
            Intent::Redo => v.push(12),
            Intent::Capture => v.push(13),
        }
        v
    }
}

// ---- FNV-1a (no new dependency; a stable, trace-reproducible digest) ------------------

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(bytes: &[u8], mut h: u64) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn kind_byte(k: ShapeKind) -> u8 {
    match k {
        ShapeKind::Rect => 0,
        ShapeKind::Line => 1,
        ShapeKind::Stroke => 2,
    }
}

// ---- the workbench -------------------------------------------------------------------

/// Metrics reported per the plan — frame generation / write / input / queue, kept as
/// independently meaningful counters (never collapsed into one score).
#[derive(Debug, Clone, Copy, Default)]
pub struct Metrics {
    pub frames_generated: u64,
    pub frames_written: u64,
    pub input_events: u64,
    pub last_exec: u32,
    pub last_deferred: u32,
    pub last_discarded: u32,
    pub last_owed: u32,
    pub captures: u64,
}

pub struct Workbench {
    pub model: SceneModel,
    pub selection: Selection,
    pub source_field: NumericField,
    pub last_numeric_error: Option<NumericError>,
    pub field_tick: u64,
    pub playing: bool,
    pub view: WorldView,
    pub probe_world: Option<Point>,
    pub color_policy: Colormap,
    pub range_policy: RangePolicy,

    /// Running hash of every applied intent, in order (the "command digest", L4).
    pub command_digest: u64,

    // paint-derived targeting (refreshed from the hit-map each layout)
    canvas_id: Option<TargetId>,
    canvas_rect: Rect,
    layers: HitLayers,

    // editing machinery
    drag: DragGesture,
    undo: UndoLog<Compound<Cmd>>,
    keymap: UndoKeymap,
    next_id: u64,

    // deterministic time
    timestep: FixedTimestep,

    pub metrics: Metrics,
    pub should_quit: bool,
}

impl Default for Workbench {
    fn default() -> Self {
        Self::new()
    }
}

impl Workbench {
    pub fn new() -> Self {
        Workbench {
            model: SceneModel {
                shapes: Vec::new(),
                source_value: 5.0,
            },
            selection: Selection::new(),
            source_field: NumericField::new()
                .with_range(SOURCE_MIN, SOURCE_MAX)
                .with_step(SOURCE_STEP),
            last_numeric_error: None,
            field_tick: 0,
            playing: false,
            view: WorldView::new(Rect::new(0, 1, FIELD_COLS as u16, FIELD_ROWS as u16))
                .with_pixels_per_cell(1, 1),
            probe_world: None,
            color_policy: Colormap::Heat,
            range_policy: RangePolicy::Auto,
            command_digest: FNV_OFFSET,
            canvas_id: None,
            canvas_rect: Rect::new(0, 1, FIELD_COLS as u16, FIELD_ROWS as u16),
            layers: HitLayers::new(),
            drag: DragGesture::new(),
            undo: UndoLog::new(),
            keymap: UndoKeymap::standard(),
            next_id: 1,
            timestep: FixedTimestep::new(FIELD_STEP),
            metrics: Metrics::default(),
            should_quit: false,
        }
    }

    // -- digests (over the MODEL only; field values are derived, so excluded) -----------

    /// Stable hash of the authoritative model + view + transport state (shapes, source,
    /// tick, pan/zoom, playing). Excludes ephemeral UI state (selection, probe cursor) and
    /// derived field values, so it is invariant under render rate, terminal size and
    /// colour/glyph policy, and round-trips exactly across undo/redo.
    pub fn digest(&self) -> u64 {
        let mut h = FNV_OFFSET;
        h = fnv1a(&(self.model.shapes.len() as u64).to_le_bytes(), h);
        for s in &self.model.shapes {
            h = fnv1a(&s.id().0.to_le_bytes(), h);
            h = fnv1a(&[kind_byte(s.kind())], h);
            let b = s.bounds();
            for v in [b.min.x, b.min.y, b.max.x, b.max.y] {
                h = fnv1a(&v.to_bits().to_le_bytes(), h);
            }
        }
        // Selection is EPHEMERAL UI state (like the probe cursor): it is driven by the
        // intent trace but is not part of the undoable authoritative model, so it is not
        // in the model digest. Undo/redo therefore round-trips the digest cleanly, and
        // selection operations are still captured in the command digest.
        h = fnv1a(&self.model.source_value.to_bits().to_le_bytes(), h);
        h = fnv1a(&self.field_tick.to_le_bytes(), h);
        let (px, py) = self.view.pan();
        let (zx, zy) = self.view.zoom();
        for v in [px, py, zx, zy] {
            h = fnv1a(&v.to_bits().to_le_bytes(), h);
        }
        h = fnv1a(&[self.playing as u8], h);
        h
    }

    // -- the single authoritative transition δ(S, I) → S' -------------------------------

    pub fn apply_intent(&mut self, intent: Intent) -> u64 {
        self.command_digest = fnv1a(&intent.canonical(), self.command_digest);
        match intent {
            Intent::DrawRect { a, b } => {
                let id = self.mint_id();
                if let Some(shape) = Shape::rect(id, a, b) {
                    self.record(Compound::new(vec![Cmd::AddShape(shape)]));
                    self.select_only(id);
                }
            }
            Intent::DrawLine { a, b } => {
                let id = self.mint_id();
                if let Some(shape) = Shape::line(id, a, b) {
                    self.record(Compound::new(vec![Cmd::AddShape(shape)]));
                    self.select_only(id);
                }
            }
            Intent::SelectAt(p) => match self.pick(p) {
                Some(id) => self.select_only(id),
                None => self.selection.clear(),
            },
            Intent::ClearSelection => self.selection.clear(),
            Intent::MoveSelected { dx, dy } => {
                let ids: Vec<ShapeId> = self.selection.ids().to_vec();
                if !ids.is_empty() && (dx != 0.0 || dy != 0.0) {
                    let cmds = ids.into_iter().map(|id| Cmd::Move { id, dx, dy }).collect();
                    self.record(Compound::new(cmds));
                }
            }
            Intent::SetSource(want) => match self.source_field.validate(want) {
                Ok(valid) => {
                    self.last_numeric_error = None;
                    let from = self.model.source_value;
                    if valid != from {
                        self.record(Compound::new(vec![Cmd::SetSource { from, to: valid }]));
                    }
                }
                // Refuse, never clamp (Finding D): the model is untouched.
                Err(e) => self.last_numeric_error = Some(e),
            },
            Intent::Pan { dx, dy } => self.view = self.view.pan_by(dx, dy),
            Intent::Zoom { factor } => {
                let (zx, zy) = self.view.zoom();
                self.view = self.view.with_zoom(zx * factor, zy * factor);
            }
            Intent::ProbeAt(p) => self.probe_world = Some(p),
            Intent::Transport(tc) => self.apply_transport(tc),
            Intent::Undo => {
                let _ = self.undo.undo(&mut self.model);
                self.prune_selection();
            }
            Intent::Redo => {
                let _ = self.undo.redo(&mut self.model);
                self.prune_selection();
            }
            Intent::Capture => self.metrics.captures += 1,
        }
        self.command_digest
    }

    fn record(&mut self, cmd: Compound<Cmd>) {
        // Apply first; only a clean apply is recorded, so the log is always replayable.
        if cmd.apply(&mut self.model).is_ok() {
            let _ = self.undo.record(cmd);
        }
    }

    fn apply_transport(&mut self, tc: TransportCmd) {
        match tc {
            TransportCmd::Play => self.playing = true,
            TransportCmd::Pause => self.playing = false,
            TransportCmd::Step(n) => {
                self.field_tick = (self.field_tick as i64 + n).max(0) as u64;
            }
            TransportCmd::Scrub(to) => self.field_tick = to,
        }
    }

    fn mint_id(&mut self) -> ShapeId {
        let id = ShapeId(self.next_id);
        self.next_id += 1;
        id
    }

    fn select_only(&mut self, id: ShapeId) {
        self.selection.clear();
        self.selection.insert(id);
    }

    /// Drop any selected id whose shape no longer exists — e.g. after an undo removed it,
    /// so a stale selection can never target a shape that isn't there (L3/L5 hygiene).
    fn prune_selection(&mut self) {
        let current: Vec<ShapeId> = self.selection.ids().to_vec();
        self.selection.clear();
        for id in current {
            if self.model.shapes.iter().any(|s| s.id() == id) {
                self.selection.insert(id);
            }
        }
    }

    /// Pick the topmost shape whose bounds contain a world point (L3). Nothing is
    /// invented from coordinates — a shape is hit only if its real bounds contain `p`.
    pub fn pick(&self, p: Point) -> Option<ShapeId> {
        self.model
            .shapes
            .iter()
            .rev()
            .find(|s| s.bounds().contains(p))
            .map(|s| s.id())
    }

    // -- undo/redo availability (for the HUD) -------------------------------------------

    pub fn undo_depth(&self) -> usize {
        self.undo.undo_depth()
    }
    pub fn redo_depth(&self) -> usize {
        self.undo.redo_depth()
    }

    // -- deterministic field (closed form; L1 — pure, never mutated by rendering) --------

    /// The field data + mask for the current tick. Masked cells are the drawn rectangle
    /// "walls" that carve the surface into compartments; everything else is a moving
    /// Gaussian whose amplitude is the source value. Pure function of the model.
    pub fn field_data(&self) -> (Vec<f64>, Vec<bool>) {
        let (cols, rows) = (FIELD_COLS, FIELD_ROWS);
        let mut data = vec![0.0f64; cols * rows];
        let mut mask = vec![true; cols * rows];

        for iy in 0..rows {
            for ix in 0..cols {
                data[iy * cols + ix] =
                    field_value_at(self.model.source_value, ix, iy, self.field_tick);
            }
        }
        // Imprint rectangle shapes as masked walls.
        for s in &self.model.shapes {
            if s.kind() == ShapeKind::Rect {
                let b = s.bounds();
                for iy in 0..rows {
                    for ix in 0..cols {
                        let c = Point::new(ix as f64 + 0.5, iy as f64 + 0.5);
                        if b.contains(c) {
                            mask[iy * cols + ix] = false;
                        }
                    }
                }
            }
        }
        (data, mask)
    }

    /// Render the field to a surface through the SAME `WorldView` pointer selection uses
    /// (L2/L3). Returns the renderer's conservation receipt (L7).
    pub fn render_field(&self, area: Rect) -> FieldFrame {
        let (data, mask) = self.field_data();
        let fv = FieldView::new(field_extent(), &data, Some(mask.as_slice()))
            .expect("field data/mask lengths match the extent");
        fv.render(&self.view, area, self.color_policy, self.range_policy)
    }

    /// Probe the field at a world point (L3/L7 — reports index, sample, quantization).
    pub fn probe(&self, world: Point) -> ProbeReport {
        let (data, mask) = self.field_data();
        let fv = FieldView::new(field_extent(), &data, Some(mask.as_slice()))
            .expect("field data/mask lengths match the extent");
        fv.probe(world)
    }

    /// The linked time-series: the field value at the probed world cell over the last
    /// `n` ticks, as `(tick, value)` — a deterministic closed-form trace.
    pub fn probe_series(&self, world: Point, n: u64) -> Vec<(f64, f64)> {
        let ext = field_extent();
        let Some((ix, iy)) = ext.world_to_index(world) else {
            return Vec::new();
        };
        let lo = self.field_tick.saturating_sub(n);
        (lo..=self.field_tick)
            .map(|tk| {
                (
                    tk as f64,
                    field_value_at(self.model.source_value, ix, iy, tk),
                )
            })
            .collect()
    }

    // -- the world↔terminal mapping, derived from the PAINTED canvas rect ---------------

    /// Turn an absolute terminal cell into a world point, or `None` if it falls outside
    /// the painted canvas (L3 clip reporting — no bogus in-bounds coordinate, S5).
    pub fn cell_to_world_point(&self, col: u16, row: u16) -> Option<Point> {
        match self.view.cell_to_world(col, row) {
            Transport::Mapped {
                value: (wx, wy), ..
            } => Some(Point::new(wx, wy)),
            Transport::Clipped(_) => None,
        }
    }

    /// Is this terminal cell on the painted canvas target? (L2 — paint-derived.)
    pub fn canvas_hit(&self, col: u16, row: u16) -> bool {
        match self.canvas_id {
            Some(id) => self.layers.hit_test(col, row) == Some(id),
            None => false,
        }
    }

    pub fn canvas_rect(&self) -> Rect {
        self.canvas_rect
    }

    // -- the UI tree (the canvas is a real hittable, keyed target) ----------------------

    pub fn build_el(&self, w: u16, h: u16) -> Element<()> {
        let compact = h < 18 || w < 60;
        let mut heat: Vec<Element<()>> = Vec::with_capacity(FIELD_ROWS);
        for line in self.heatmap_lines() {
            heat.push(text(line));
        }
        let canvas = column().key("canvas").on_press(()).children(heat);
        let mut col: Element<()> = column()
            .child(text(self.hud_line()))
            .child(canvas)
            .child(text(self.probe_line()))
            .child(text(self.inspector_line()))
            .child(text(self.metrics_line()));
        if !compact {
            col = col.child(text(self.help_line()));
        }
        screen().child(col)
    }

    /// Recompile + relayout + rebuild the hit-map, refreshing the paint-derived canvas
    /// id/rect/layers and anchoring the `WorldView` to the PAINTED canvas rect (L2).
    /// Returns the laid-out root node, ready for `Context::set_root`.
    pub fn refresh_layout(&mut self, w: u16, h: u16) -> Node {
        let el = self.build_el(w, h);
        let env = UiEnvironment {
            width: w,
            height: h,
            ..Default::default()
        };
        let cx = BuildCx::new(skins::SWISS_SIGNAL, env);
        let mut compiled = compile(&el, &cx).expect("workbench UI compiles");
        compute_layout(&mut compiled.node, w, h).expect("workbench layout resolves");
        let hit = compiled
            .hit_map(Rect::new(0, 0, w, h))
            .expect("hit map (layout has run)");
        if let Some(t) = hit
            .targets()
            .iter()
            .find(|t| matches!(&t.key, Key::Named(s) if s == "canvas"))
        {
            self.canvas_id = Some(t.id);
            self.canvas_rect = t.rect;
            let (px, py) = self.view.pan();
            let (zx, zy) = self.view.zoom();
            self.view = WorldView::new(t.rect)
                .with_pixels_per_cell(1, 1)
                .with_zoom(zx, zy)
                .with_pan(px, py);
        }
        self.layers = hit.layers().clone();
        self.metrics.frames_generated += 1;
        compiled.node
    }

    // -- real-event handling: lower every Event to Intents (pointer ≡ keyboard) ----------

    pub fn process(&mut self, event: &Event) {
        self.metrics.input_events += 1;

        // The drag gesture sees every event; it self-cancels on Esc/focus/resize/modal.
        if let Some(phase) = self.drag.on_event(event, &self.layers, &self.view) {
            self.handle_drag_phase(phase);
        }

        match event {
            Event::Key(ke) => self.handle_key(*ke),
            Event::Mouse(me) => self.handle_mouse(*me),
            _ => {}
        }
    }

    fn handle_drag_phase(&mut self, phase: EditPhase<DragIntent>) {
        if let EditPhase::Commit { intent, .. } = phase {
            if let (
                Transport::Mapped {
                    value: (sx, sy), ..
                },
                Transport::Mapped {
                    value: (ex, ey), ..
                },
            ) = (intent.start_world, intent.current_world)
            {
                let (dx, dy) = (ex - sx, ey - sy);
                if dx != 0.0 || dy != 0.0 {
                    self.apply_intent(Intent::MoveSelected { dx, dy });
                }
            }
        }
    }

    fn handle_mouse(&mut self, me: MouseEvent) {
        if me.kind == MouseKind::Down(MouseButton::Left) && self.canvas_hit(me.column, me.row) {
            if let Some(p) = self.cell_to_world_point(me.column, me.row) {
                // Pick-on-press so the subsequent drag moves what was clicked.
                self.apply_intent(Intent::SelectAt(p));
                self.apply_intent(Intent::ProbeAt(p));
            }
        }
    }

    fn handle_key(&mut self, ke: KeyEvent) {
        // Undo/redo first (Ctrl-Z / Ctrl-Y / Ctrl-Shift-Z).
        if let Some(intent) = self.keymap.classify(ke) {
            match intent {
                gibson::edit::UndoIntent::Undo => {
                    self.apply_intent(Intent::Undo);
                }
                gibson::edit::UndoIntent::Redo => {
                    self.apply_intent(Intent::Redo);
                }
            }
            return;
        }
        let shift = ke.modifiers.contains(KeyModifiers::SHIFT);
        match ke.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('n') => {
                // Draw a unit rectangle "wall" at the cursor/probe cell.
                let o = self.probe_world.unwrap_or(Point::new(2.0, 2.0));
                self.apply_intent(Intent::DrawRect {
                    a: Point::new(o.x.floor(), o.y.floor()),
                    b: Point::new(o.x.floor() + 3.0, o.y.floor() + 2.0),
                });
            }
            KeyCode::Char('l') => {
                let o = self.probe_world.unwrap_or(Point::new(2.0, 2.0));
                self.apply_intent(Intent::DrawLine {
                    a: Point::new(o.x.floor(), o.y.floor()),
                    b: Point::new(o.x.floor() + 4.0, o.y.floor() + 2.0),
                });
            }
            KeyCode::Char(']') => self.bump_source(SOURCE_STEP),
            KeyCode::Char('[') => self.bump_source(-SOURCE_STEP),
            KeyCode::Char(' ') => {
                let tc = if self.playing {
                    TransportCmd::Pause
                } else {
                    TransportCmd::Play
                };
                self.apply_intent(Intent::Transport(tc));
            }
            KeyCode::Char('.') => {
                self.apply_intent(Intent::Transport(TransportCmd::Step(1)));
            }
            KeyCode::Char(',') => {
                self.apply_intent(Intent::Transport(TransportCmd::Step(-1)));
            }
            KeyCode::Char('0') => {
                self.apply_intent(Intent::Transport(TransportCmd::Scrub(0)));
            }
            KeyCode::Char('c') => {
                self.apply_intent(Intent::Capture);
            }
            KeyCode::Char('p') => {
                // Keyboard probe of the field centre — probing is a core workflow and
                // must be reachable without a pointer (#144 keyboard-only equivalence).
                self.apply_intent(Intent::ProbeAt(Point::new(
                    FIELD_COLS as f64 / 2.0,
                    FIELD_ROWS as f64 / 2.0,
                )));
            }
            KeyCode::Char('m') => {
                self.color_policy = match self.color_policy {
                    Colormap::Heat => Colormap::Grayscale,
                    _ => Colormap::Heat,
                };
            }
            // Arrow keys: keyboard-equivalent move of the selection (≡ a drag).
            KeyCode::Left => self.arrow_move(-1.0, 0.0, shift),
            KeyCode::Right => self.arrow_move(1.0, 0.0, shift),
            KeyCode::Up => self.arrow_move(0.0, -1.0, shift),
            KeyCode::Down => self.arrow_move(0.0, 1.0, shift),
            _ => {}
        }
    }

    fn arrow_move(&mut self, dx: f64, dy: f64, shift: bool) {
        if shift {
            // Shift+arrow pans the view instead of moving the selection.
            self.apply_intent(Intent::Pan { dx, dy });
        } else {
            self.apply_intent(Intent::MoveSelected { dx, dy });
        }
    }

    fn bump_source(&mut self, delta: f64) {
        self.apply_intent(Intent::SetSource(self.model.source_value + delta));
    }

    // -- the deterministic clock step, for the real loop (returns None when paused) -----

    pub fn tick_wall(&mut self, real_dt: Duration) -> Option<TickDemand> {
        if !self.playing {
            return None;
        }
        let demand = self.timestep.advance(real_dt);
        self.field_tick += demand.executed as u64;
        self.metrics.last_exec = demand.executed;
        self.metrics.last_deferred = demand.deferred;
        self.metrics.last_discarded = demand.discarded;
        self.metrics.last_owed = demand.owed;
        Some(demand)
    }

    pub fn note_write(&mut self) {
        self.metrics.frames_written += 1;
    }

    // -- text rendering (so model state is readable by a vt100 oracle) -------------------

    pub fn hud_line(&self) -> String {
        let mut line = format!(
            "WORKBENCH  sel={}  source={:.1}  shapes={}  tick={}  play={}  undo={} redo={}",
            self.selection.primary().map(|i| i.0 as i64).unwrap_or(-1),
            self.model.source_value,
            self.model.shapes.len(),
            self.field_tick,
            if self.playing { "on" } else { "off" },
            self.undo_depth(),
            self.redo_depth(),
        );
        // Test affordance (off unless WORKBENCH_EMIT_CANVAS is set): expose the
        // painted canvas rect so a PTY test can click a known on-canvas cell without
        // guessing the layout. Absent from normal runs, so the HUD is unchanged.
        if std::env::var_os("WORKBENCH_EMIT_CANVAS").is_some() {
            let r = self.canvas_rect;
            line.push_str(&format!(
                "  canvas={},{},{},{}",
                r.x, r.y, r.width, r.height
            ));
        }
        line
    }

    pub fn heatmap_lines(&self) -> Vec<String> {
        const RAMP: &[u8] = b" .:-=+*#%@";
        let (data, mask) = self.field_data();
        let fv =
            FieldView::new(field_extent(), &data, Some(mask.as_slice())).expect("lengths match");
        let (lo, hi) = fv.finite_extremes().unwrap_or((0.0, 1.0));
        let span = if hi > lo { hi - lo } else { 1.0 };
        let mono = self.color_policy == Colormap::Grayscale;
        let mut lines = Vec::with_capacity(FIELD_ROWS);
        for iy in 0..FIELD_ROWS {
            // Build each row as ASCII bytes so a vertex overlay is a plain index write
            // (no unsafe): every glyph used here is ASCII.
            let mut row = vec![b' '; FIELD_COLS];
            for (ix, cell) in row.iter_mut().enumerate() {
                *cell = match fv.sample(ix, iy) {
                    Some(Sample::Value(v)) => {
                        let t = ((v - lo) / span).clamp(0.0, 1.0);
                        let idx = (t * (RAMP.len() - 1) as f64).round() as usize;
                        if mono && idx > 0 {
                            b'o'
                        } else {
                            RAMP[idx]
                        }
                    }
                    Some(Sample::Masked) => b'#', // a wall compartment boundary
                    Some(Sample::NonFinite) => b'?',
                    None => b' ',
                };
            }
            // Overlay line/stroke shape vertices as '+'.
            for s in &self.model.shapes {
                if s.kind() != ShapeKind::Rect {
                    for p in s.vertices() {
                        let (vx, vy) = (p.x.floor() as isize, p.y.floor() as isize);
                        if vy == iy as isize && vx >= 0 && (vx as usize) < FIELD_COLS {
                            row[vx as usize] = b'+';
                        }
                    }
                }
            }
            lines.push(String::from_utf8(row).expect("row is ASCII"));
        }
        lines
    }

    pub fn probe_line(&self) -> String {
        match self.probe_world {
            Some(p) => {
                let r = self.probe(p);
                let sample = match r.sample {
                    Some(Sample::Value(v)) => format!("value={v:.3}"),
                    Some(Sample::Masked) => "masked".to_string(),
                    Some(Sample::NonFinite) => "nonfinite".to_string(),
                    None => "off-field".to_string(),
                };
                let idx = r
                    .index
                    .map(|(x, y)| format!("({x},{y})"))
                    .unwrap_or_else(|| "none".to_string());
                format!(
                    "probe  world=({:.1},{:.1})  cell={}  {}",
                    p.x, p.y, idx, sample
                )
            }
            None => "probe  (click the canvas or press a shape key to probe)".to_string(),
        }
    }

    pub fn inspector_line(&self) -> String {
        let err = match &self.last_numeric_error {
            Some(e) => format!("  REFUSED: {e}"),
            None => String::new(),
        };
        format!(
            "inspector  source [{:.0}..{:.0}] step {:.0} = {:.1}{}",
            SOURCE_MIN, SOURCE_MAX, SOURCE_STEP, self.model.source_value, err
        )
    }

    pub fn metrics_line(&self) -> String {
        format!(
            "metrics  gen={} write={} input={} | ticks exec={} deferred={} discarded={} owed={} | captures={}",
            self.metrics.frames_generated,
            self.metrics.frames_written,
            self.metrics.input_events,
            self.metrics.last_exec,
            self.metrics.last_deferred,
            self.metrics.last_discarded,
            self.metrics.last_owed,
            self.metrics.captures,
        )
    }

    pub fn help_line(&self) -> String {
        "keys  n:rect l:line []:source space:play .,:step 0:rewind p:probe m:mono c:capture arrows:move ^Z/^Y:undo q:quit".to_string()
    }

    /// A plain-text still capture of the current frame (HUD + heatmap + readouts),
    /// suitable for `--selfcheck` goldens and the capture/export workflow.
    pub fn capture_still(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.hud_line());
        out.push('\n');
        for l in self.heatmap_lines() {
            out.push_str(&l);
            out.push('\n');
        }
        out.push_str(&self.probe_line());
        out.push('\n');
        out.push_str(&self.inspector_line());
        out.push('\n');
        out
    }
}
