//! `scope_editor` — a runnable 1-D oscilloscope / signal-trace viewer-editor.
//!
//! The cold-start transfer-test instrument (acceptance #4): a *different* consumer
//! from `instrument_workbench`, built only from the public `gibson` surface and
//! `docs/WORKBENCH_RECIPES.md`. The model lives in `scope.rs`; this file is the two
//! consumers of it:
//!   * `run_interactive` — the real-terminal loop (alt screen, opt-in mouse capture
//!     leased through the `TerminalSession`), always restored, and
//!   * `--selfcheck` — the deterministic headless replay in `scope::run_selfcheck`.
//!
//! The trace is rendered to a `Surface` via `gibson::plot` and shown as a `Node::raster`
//! above a two-line HUD, in a trivial vertical split so the scope's painted rectangle
//! is the one the app computes (see the friction notes in the transfer-test report:
//! for a Surface-backed plot the painted-frame-targeting recipe adds chrome-inset
//! complexity without benefit at this layout; the hit-map seam is instead validated in
//! `tests/scope_editor_slice.rs`).
//!
//! Keys: `space` play/pause · `s` step · `[` / `]` amplitude -/+ (refuses out of
//! range) · `<-` / `->` move cursor · `-` / `=` zoom out/in · `,` / `.` pan ·
//! `f` follow · Ctrl-Z / Ctrl-Y undo/redo · click the trace to probe · `q` quit.

use std::io;
use std::time::{Duration, Instant};

use gibson::{
    Compound, Context, Event, FixedTimestep, KeyCode, KeyEvent, MouseButton, MouseEvent, MouseKind,
    Node, Rect, Style, SubcellGlyphMode, Surface, UndoIntent, UndoKeymap, UndoLog,
};

mod scope;
use scope::{AmpCmd, Intent, ScopeModel};

/// Glyph fidelity for the trace. Recipe 9: a mono / reduced-glyph terminal can drop
/// this to `HalfBlock1x2`, `Block`, or `Ascii` with no other change.
const GLYPH_MODE: SubcellGlyphMode = SubcellGlyphMode::Braille2x4;

fn main() -> io::Result<()> {
    if std::env::args().any(|a| a == "--selfcheck") {
        return if scope::run_selfcheck() {
            Ok(())
        } else {
            Err(io::Error::other("scope_editor selfcheck failed"))
        };
    }
    run_interactive()
}

fn run_interactive() -> io::Result<()> {
    let mut ctx = Context::fullscreen()?;

    // Non-TTY (issue #46): a live loop would render to nothing and never get input.
    // Fall back to the headless selfcheck rather than idle blank (Recipe 9).
    if !ctx.is_interactive() {
        eprintln!("scope_editor: stdout is not an interactive terminal; running --selfcheck.");
        ctx.restore()?;
        return if scope::run_selfcheck() {
            Ok(())
        } else {
            Err(io::Error::other("selfcheck failed"))
        };
    }

    // Opt-in SGR mouse capture, leased through the session so restore/Drop/panic all
    // unwind it (Finding F). A terminal without mouse support still has the full
    // keyboard workflow, so a failure here is non-fatal.
    let _ = ctx.session.set_mouse_capture(true);

    let (w, h) = ctx.session.terminal_size();
    let result = drive(&mut ctx, w, h);

    let _ = ctx.session.set_mouse_capture(false);
    let _ = ctx.restore();
    result
}

/// The input-driven loop. Rebuilds the view on change and lets `run_once` drive both
/// input and frame pacing (the canonical loop from `Context::run_once`).
fn drive(ctx: &mut Context, mut w: u16, mut h: u16) -> io::Result<()> {
    let mut model = ScopeModel::new();
    let mut undo: UndoLog<Compound<AmpCmd>> = UndoLog::new();
    let keymap = UndoKeymap::standard();
    let mut timestep = FixedTimestep::new(Duration::from_millis(scope::TICK_MS));
    let mut last = Instant::now();

    // Render the first frame up front so `scope_rect` holds a real painted region
    // (not a throwaway) before any pointer event can read it.
    let (node, mut scope_rect) = build_frame(&model, w, h);
    ctx.set_root(node);
    let mut dirty = false;

    loop {
        // App-owned clock -> model ticks (Recipe 6). Wall deltas become capped ticks;
        // the trace scrolls only when playing.
        let now = Instant::now();
        let real_dt = now.saturating_duration_since(last);
        last = now;
        if model.playing {
            let demand = timestep.advance(real_dt);
            for _ in 0..demand.ticks {
                scope::apply_intent(&mut model, &mut undo, Intent::Tick);
            }
            if demand.ticks > 0 {
                dirty = true;
            }
        }

        if dirty {
            let (node, rect) = build_frame(&model, w, h);
            scope_rect = rect;
            ctx.set_root(node);
            dirty = false;
        }

        match ctx.run_once(Duration::from_millis(33))? {
            Some(Event::Key(k)) => {
                if matches!(k.code, KeyCode::Char('q')) {
                    break;
                }
                if let Some(intent) = map_key(k, &keymap, &model) {
                    scope::apply_intent(&mut model, &mut undo, intent);
                    dirty = true;
                }
            }
            Some(Event::Mouse(m)) => {
                if handle_mouse(m, scope_rect, &mut model) {
                    dirty = true;
                }
            }
            Some(Event::Resize(nw, nh)) => {
                w = nw;
                h = nh;
                dirty = true;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Builds one frame: the trace surface as a `Node::raster` above a two-line HUD, and
/// the scope's rectangle (origin + size), which the probe maps pointer cells against.
/// The split is trivial (full-width raster on top), so the painted rectangle equals
/// this computed one.
fn build_frame(model: &ScopeModel, w: u16, h: u16) -> (Node, Rect) {
    let hud_rows = h.min(2);
    let scope_h = h.saturating_sub(hud_rows).max(1);
    let scope_rect = Rect::new(0, 0, w, scope_h);

    let surface = render_scope_surface(model, w, scope_h);
    let node = Node::col()
        .child(Node::raster(surface))
        .child(Node::text(hud_line1(model), Style::default()))
        .child(Node::text(hud_line2(model), Style::default()));

    (node, scope_rect)
}

/// Renders the trace into a surface sized to the area it will occupy (Recipe 5).
fn render_scope_surface(model: &ScopeModel, w: u16, h: u16) -> Surface {
    let spec = scope::plot_spec(model);
    match scope::plot_view(model) {
        Some(view) => match gibson::plot::plot(&spec, &view, Rect::new(0, 0, w, h), GLYPH_MODE) {
            Ok((surface, _report)) => surface,
            Err(_) => Surface::new(w.max(1), h.max(1)),
        },
        None => Surface::new(w.max(1), h.max(1)),
    }
}

fn hud_line1(model: &ScopeModel) -> String {
    let (t0, t1) = model.window();
    format!(
        "{}  amp={:.2}  freq={:.2}Hz  cursor t={:.3}s v={:.3}  view=[{:.2},{:.2}]s {}",
        if model.playing { "PLAY " } else { "PAUSE" },
        model.amplitude,
        model.frequency,
        model.cursor_t,
        scope::probe(model),
        t0,
        t1,
        if model.follow { "(follow)" } else { "(pan)" },
    )
}

fn hud_line2(model: &ScopeModel) -> String {
    let base = "space play/pause  s step  [ ] amp  <-/-> cursor  - = zoom  , . pan  f follow  ^Z/^Y undo  click probe  q quit";
    if model.status.is_empty() {
        base.to_string()
    } else {
        format!("{base}    {}", model.status)
    }
}

/// Translates a key event into an intent. Ctrl-Z / Ctrl-Y are resolved by the
/// library's `UndoKeymap` so we do not reinvent the chord handling (Recipe 3).
fn map_key(k: KeyEvent, keymap: &UndoKeymap, model: &ScopeModel) -> Option<Intent> {
    if let Some(u) = keymap.classify(k) {
        return Some(match u {
            UndoIntent::Undo => Intent::Undo,
            UndoIntent::Redo => Intent::Redo,
        });
    }
    match k.code {
        KeyCode::Char(' ') => Some(if model.playing {
            Intent::Pause
        } else {
            Intent::Play
        }),
        KeyCode::Char('s') => Some(Intent::Step),
        KeyCode::Char('[') => Some(Intent::AmpDown),
        KeyCode::Char(']') => Some(Intent::AmpUp),
        KeyCode::Left => Some(Intent::CursorLeft),
        KeyCode::Right => Some(Intent::CursorRight),
        KeyCode::Char('-') | KeyCode::Char('_') => Some(Intent::ZoomOut),
        KeyCode::Char('=') | KeyCode::Char('+') => Some(Intent::ZoomIn),
        KeyCode::Char(',') => Some(Intent::PanLeft),
        KeyCode::Char('.') => Some(Intent::PanRight),
        KeyCode::Char('f') => Some(Intent::Follow),
        _ => None,
    }
}

/// A left-click inside the scope rectangle probes the time under the pointer (Law L3):
/// the cell is mapped back to a time through the plot's own transform.
fn handle_mouse(m: MouseEvent, scope_rect: Rect, model: &mut ScopeModel) -> bool {
    if let MouseKind::Down(MouseButton::Left) = m.kind {
        if scope_rect.contains(m.column, m.row) {
            let local_col = m.column - scope_rect.x;
            let local_row = m.row - scope_rect.y;
            if let Some(t) = scope::time_at_cell(
                model,
                scope_rect.width,
                scope_rect.height,
                local_col,
                local_row,
            ) {
                model.cursor_t = t;
                model.status = format!(
                    "probe @ t={:.3}s  v={:.3}",
                    t,
                    scope::sample(t, model.amplitude, model.frequency)
                );
                return true;
            }
        }
    }
    false
}
