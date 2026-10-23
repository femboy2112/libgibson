//! `scope_editor` core — a 1-D oscilloscope / signal-trace viewer-editor model.
//!
//! This is the cold-start transfer-test consumer (acceptance #4 of the workbench
//! recipes): a *different* instrument from the 2-D field workbench, built only from
//! the public `gibson` surface + `docs/WORKBENCH_RECIPES.md`. It is deliberately
//! domain-neutral and **synthetic**: the "signal" is a closed-form sum of sines —
//! there is no solver and no DSP here (Recipe limitation: the library owns
//! realization, never analysis).
//!
//! Everything in this file is pure (no terminal, no I/O except `--selfcheck`'s
//! println) so the seam oracles in `tests/scope_editor_slice.rs` drive the exact
//! same model the runnable example uses, included directly with `#[path]`.
//!
//! The split across the public surface:
//!   * amplitude is a validated [`gibson::numeric::NumericField`] — it REFUSES
//!     out-of-range input, never clamps (Recipe 4);
//!   * an amplitude edit is a [`gibson::edit::Compound`] recorded in an
//!     [`gibson::edit::UndoLog`] (Recipe 3);
//!   * the trace is a `gibson::plot` line series; pan/zoom replaces the
//!     [`gibson::plot::PlotView`], never the data (Recipe 5);
//!   * a pointer probe maps a painted cell back to a time via the plot's public
//!     `transform` (Recipe 5, Law L3);
//!   * the clock is an app-owned tick advanced by [`gibson::FixedTimestep`]
//!     (Recipe 6);
//!   * `--selfcheck` replays a scripted intent trace twice and proves an identical
//!     model digest (Recipe 7).

#![allow(dead_code)]

use gibson::edit::{Compound, Reversible, UndoLog};
use gibson::numeric::NumericField;
use gibson::plot::{
    self, Annotation, AxisScale, AxisSpec, FiniteRange, PlotSpec, PlotView, Series,
};
use gibson::Rect;

// ---- constants -----------------------------------------------------------------

/// Seconds of signal time per model tick (Law L6: the field owns its own tick).
pub const TICK_DT: f64 = 0.02;
/// Wall-clock milliseconds per model tick, fed to `FixedTimestep` (Recipe 6).
pub const TICK_MS: u64 = 20;
/// How many samples the trace series carries across the visible window.
pub const SAMPLES: usize = 480;

/// Amplitude field bounds + step (Recipe 4: refuse, never clamp).
pub const AMP_MIN: f64 = 0.0;
pub const AMP_MAX: f64 = 5.0;
pub const AMP_STEP: f64 = 0.5;

/// Zoom clamps on the visible time span (seconds).
pub const MIN_SPAN: f64 = 0.25;
pub const MAX_SPAN: f64 = 20.0;

/// Start a couple of seconds in so the initial window is `[0, 2]`, not `[-2, 0]`.
pub const INITIAL_TICK: u64 = 100;

/// The validated schema for the amplitude parameter.
pub fn amp_field() -> NumericField {
    NumericField::new()
        .with_range(AMP_MIN, AMP_MAX)
        .with_step(AMP_STEP)
}

// ---- the synthetic signal ------------------------------------------------------

/// A closed-form trace value at absolute time `t`: fundamental + a quiet third
/// harmonic. Pure function of `(t, amplitude, frequency)` — deterministic, no state.
pub fn sample(t: f64, amplitude: f64, frequency: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    amplitude * ((tau * frequency * t).sin() + 0.25 * (tau * 3.0 * frequency * t).sin())
}

// ---- model ---------------------------------------------------------------------

/// The whole authoritative scope state. The consumer owns it; the library never
/// mutates it behind our back.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeModel {
    /// App-owned model clock in ticks (never wall time; Law L6).
    pub tick: u64,
    /// Validated parameter (edited through `amp_field()`), undoable.
    pub amplitude: f64,
    /// Signal frequency in Hz (fixed in this instrument).
    pub frequency: f64,
    /// Transport state (Recipe 6).
    pub playing: bool,
    /// When true the view window tracks `now`; a pan turns it off.
    pub follow: bool,
    /// Visible time span in seconds (zoom).
    pub view_span: f64,
    /// Right edge of the window when NOT following (seconds).
    pub view_end: f64,
    /// Probe cursor position in absolute time (seconds).
    pub cursor_t: f64,
    /// Last human-readable status line (e.g. a refusal message).
    pub status: String,
}

impl ScopeModel {
    pub fn new() -> Self {
        Self {
            tick: INITIAL_TICK,
            amplitude: 1.0,
            frequency: 2.0,
            playing: false,
            follow: true,
            view_span: 2.0,
            view_end: 2.0,
            cursor_t: 1.0,
            status: String::new(),
        }
    }

    /// Current model time in seconds.
    pub fn now_t(&self) -> f64 {
        self.tick as f64 * TICK_DT
    }

    /// The visible time window `[start, end]`.
    pub fn window(&self) -> (f64, f64) {
        let end = if self.follow {
            self.now_t()
        } else {
            self.view_end
        };
        (end - self.view_span, end)
    }

    /// Half-height of the value axis, always strictly positive.
    pub fn y_half(&self) -> f64 {
        self.amplitude.max(0.1) * 1.3 + 0.5
    }
}

impl Default for ScopeModel {
    fn default() -> Self {
        Self::new()
    }
}

// ---- reversible amplitude edit (Recipe 3) --------------------------------------

/// The error an amplitude command could report. Amplitude moves can't fail here
/// (the value was already validated before the command was built), but the
/// `Reversible` contract still requires a named error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeError;

/// A single reversible amplitude change. Wrapped in a `Compound` so the atomic
/// rollback is always in force (Recipe 3 / Finding B).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmpCmd {
    pub from: f64,
    pub to: f64,
}

impl Reversible for AmpCmd {
    type Model = ScopeModel;
    type Error = ScopeError;

    fn apply(&self, model: &mut ScopeModel) -> Result<(), ScopeError> {
        model.amplitude = self.to;
        Ok(())
    }

    fn revert(&self, model: &mut ScopeModel) -> Result<(), ScopeError> {
        model.amplitude = self.from;
        Ok(())
    }
}

/// The ordered intent vocabulary. A scripted `&[Intent]` replays to an identical
/// model, which is what `--selfcheck` and the tests lean on (Recipe 7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Intent {
    Play,
    Pause,
    Step,
    AmpUp,
    AmpDown,
    SetAmp(f64),
    CursorLeft,
    CursorRight,
    ZoomIn,
    ZoomOut,
    PanLeft,
    PanRight,
    Follow,
    Undo,
    Redo,
    Tick,
}

/// Applies one intent to the model, threading the undo log. This is the single
/// place model state changes, so replay is a pure function of the intent list.
pub fn apply_intent(model: &mut ScopeModel, undo: &mut UndoLog<Compound<AmpCmd>>, intent: Intent) {
    match intent {
        Intent::Play => model.playing = true,
        Intent::Pause => model.playing = false,
        Intent::Step => model.tick = model.tick.saturating_add(1),
        Intent::Tick => model.tick = model.tick.saturating_add(1),
        Intent::AmpUp => amp_edit(model, undo, AMP_STEP),
        Intent::AmpDown => amp_edit(model, undo, -AMP_STEP),
        Intent::SetAmp(v) => amp_set(model, undo, v),
        Intent::CursorLeft => model.cursor_t -= model.view_span / 100.0,
        Intent::CursorRight => model.cursor_t += model.view_span / 100.0,
        Intent::ZoomIn => model.view_span = (model.view_span * 0.5).max(MIN_SPAN),
        Intent::ZoomOut => model.view_span = (model.view_span * 2.0).min(MAX_SPAN),
        Intent::PanLeft => pan(model, -0.25),
        Intent::PanRight => pan(model, 0.25),
        Intent::Follow => model.follow = true,
        Intent::Undo => {
            if undo.undo(model).is_some() {
                model.status = "undo".to_string();
            }
        }
        Intent::Redo => {
            if undo.redo(model).is_some() {
                model.status = "redo".to_string();
            }
        }
    }
}

/// Steps amplitude through the validated schema. An out-of-range result is REFUSED
/// (status reports why, model unchanged) — never clamped to the boundary.
fn amp_edit(model: &mut ScopeModel, undo: &mut UndoLog<Compound<AmpCmd>>, delta: f64) {
    let requested = model.amplitude + delta;
    amp_set(model, undo, requested);
}

fn amp_set(model: &mut ScopeModel, undo: &mut UndoLog<Compound<AmpCmd>>, requested: f64) {
    match amp_field().validate(requested) {
        Ok(v) => {
            let cmd = Compound::new(vec![AmpCmd {
                from: model.amplitude,
                to: v,
            }]);
            if cmd.apply(model).is_ok() {
                undo.record(cmd);
                model.status = format!("amplitude = {v}");
            }
        }
        Err(e) => {
            // Refused, not clamped: the authoritative value is left exactly as-is.
            model.status = format!("refused: {e}");
        }
    }
}

/// Pans the window by a fraction of the current span, dropping follow mode and
/// seeding the free edge from the current `now` so the view does not jump.
fn pan(model: &mut ScopeModel, fraction: f64) {
    if model.follow {
        model.view_end = model.now_t();
        model.follow = false;
    }
    model.view_end += model.view_span * fraction;
}

// ---- derived plot (realization, not analysis) ----------------------------------

/// The trace points `(t, value)` across the visible window.
pub fn trace_points(model: &ScopeModel, n: usize) -> Vec<(f64, f64)> {
    let (t0, t1) = model.window();
    let denom = (n.saturating_sub(1)).max(1) as f64;
    (0..n)
        .map(|i| {
            let t = t0 + (t1 - t0) * (i as f64 / denom);
            (t, sample(t, model.amplitude, model.frequency))
        })
        .collect()
}

/// Builds the semantic plot spec: a line series plus the cursor VLine and a zero
/// HLine reference. Annotations ride the same axis transform as the data, so
/// pan/zoom moves data and cursor together (Recipe 5).
pub fn plot_spec(model: &ScopeModel) -> PlotSpec {
    PlotSpec::new(
        AxisSpec::new(AxisScale::Linear, "t").unit("s"),
        AxisSpec::new(AxisScale::Linear, "v"),
    )
    .title("scope_editor - 1-D signal trace")
    .series(Series::line(trace_points(model, SAMPLES)).color((90, 220, 160)))
    .annotate(Annotation::HLine {
        y: 0.0,
        color: (70, 80, 100),
    })
    .annotate(Annotation::VLine {
        x: model.cursor_t,
        color: (240, 180, 80),
    })
}

/// The current view window as a `PlotView`. `None` only if the window collapses to
/// a non-finite / degenerate range, which the `FiniteRange` gate refuses.
pub fn plot_view(model: &ScopeModel) -> Option<PlotView> {
    let (t0, t1) = model.window();
    let x = FiniteRange::new(t0, t1)?;
    let yh = model.y_half();
    let y = FiniteRange::new(-yh, yh)?;
    Some(PlotView::new(x, y))
}

/// The probed value at the cursor (reads the signal at `cursor_t`).
pub fn probe(model: &ScopeModel) -> f64 {
    sample(model.cursor_t, model.amplitude, model.frequency)
}

/// Maps a painted cell (local to a plot surface of `area_w x area_h`) back to a
/// time value, using the plot's OWN public transform + plot rectangle so the
/// probed time is the time the renderer actually painted at that cell (Law L3).
/// `None` if the cell is outside the interior plot rectangle (axis gutters, etc.).
pub fn time_at_cell(
    model: &ScopeModel,
    area_w: u16,
    area_h: u16,
    local_col: u16,
    local_row: u16,
) -> Option<f64> {
    let spec = plot_spec(model);
    let view = plot_view(model)?;
    let (layout, _report) = plot::compile(&spec, &view, Rect::new(0, 0, area_w, area_h)).ok()?;
    let pr = layout.plot_rect;
    let tf = layout.transform?;
    if local_col < pr.x
        || local_col >= pr.x.saturating_add(pr.width)
        || local_row < pr.y
        || local_row >= pr.y.saturating_add(pr.height)
    {
        return None;
    }
    // The transform's device space is the braille 2x4 subpixel grid: px_w/px_h
    // cover plot_rect.width/height cells. Derive the per-cell subpixel factor from
    // the layout itself rather than hard-coding 2x4, then probe the cell centre.
    let sx = layout.px_w as f64 / pr.width.max(1) as f64;
    let sy = layout.px_h as f64 / pr.height.max(1) as f64;
    let px = (local_col - pr.x) as f64 * sx + (sx - 1.0) / 2.0;
    let py = (local_row - pr.y) as f64 * sy + (sy - 1.0) / 2.0;
    let (t, _v) = tf.unproject(px, py);
    Some(t)
}

// ---- deterministic replay (Recipe 7) -------------------------------------------

/// A stable FNV-1a digest over the authoritative model fields. Hashes the MODEL,
/// not rendered pixels (the invariance point from the recipe's §"known limitations").
pub fn digest(model: &ScopeModel) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    h = fnv(h, &model.tick.to_le_bytes());
    h = fnv(h, &model.amplitude.to_bits().to_le_bytes());
    h = fnv(h, &model.frequency.to_bits().to_le_bytes());
    h = fnv(h, &model.view_span.to_bits().to_le_bytes());
    h = fnv(h, &model.view_end.to_bits().to_le_bytes());
    h = fnv(h, &model.cursor_t.to_bits().to_le_bytes());
    h = fnv(h, &[model.playing as u8, model.follow as u8]);
    h
}

fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Replays a scripted intent trace onto a fresh model + undo log and returns the
/// resulting model.
pub fn replay(script: &[Intent]) -> ScopeModel {
    let (model, _undo) = replay_with(script);
    model
}

/// Like [`replay`] but also returns the undo log, so a test can inspect depth.
pub fn replay_with(script: &[Intent]) -> (ScopeModel, UndoLog<Compound<AmpCmd>>) {
    let mut model = ScopeModel::new();
    let mut undo: UndoLog<Compound<AmpCmd>> = UndoLog::new();
    for &intent in script {
        apply_intent(&mut model, &mut undo, intent);
    }
    (model, undo)
}

/// A scripted trace exercising every feature, including driving amplitude to its
/// ceiling so a refusal is observed, and an undo/redo round trip.
pub fn default_script() -> Vec<Intent> {
    use Intent::*;
    let mut s = vec![
        Play,
        Tick,
        Tick,
        Tick,
        Pause,
        CursorRight,
        CursorRight,
        ZoomIn,
        PanLeft,
    ];
    // Default amplitude 1.0 + 0.5 steps: 8 raises reach the 5.0 ceiling; a 9th is
    // refused (proving refuse-not-clamp), leaving the value pinned at 5.0.
    for _ in 0..9 {
        s.push(AmpUp);
    }
    s.extend([Undo, Undo, Redo, AmpDown, Follow, Tick, Tick]);
    s
}

/// Headless, deterministic smoke test: replay twice, prove identical digests, and
/// confirm the plot pipeline accepts the trace. Prints a short receipt. Returns
/// whether every check passed.
pub fn run_selfcheck() -> bool {
    let script = default_script();
    let d1 = digest(&replay(&script));
    let d2 = digest(&replay(&script));
    let replay_matches = d1 == d2;

    let model = replay(&script);
    let plot_ok = plot_receipt_ok(&model);

    println!("replay_matches={replay_matches}");
    println!("digest=0x{d1:016x}");
    println!(
        "final: tick={} amplitude={} cursor_t={:.3} span={:.3} follow={}",
        model.tick, model.amplitude, model.cursor_t, model.view_span, model.follow
    );
    println!("plot_receipt_ok={plot_ok}");

    replay_matches && plot_ok
}

/// Compiles the plot for a model at a fixed area and checks the execution receipt:
/// the trace samples were accepted and a device transform exists.
pub fn plot_receipt_ok(model: &ScopeModel) -> bool {
    let spec = plot_spec(model);
    let Some(view) = plot_view(model) else {
        return false;
    };
    match plot::compile(&spec, &view, Rect::new(0, 0, 100, 30)) {
        Ok((layout, report)) => report.finite_samples > 0 && layout.transform.is_some(),
        Err(_) => false,
    }
}
