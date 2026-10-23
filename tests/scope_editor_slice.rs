//! Cross-layer SEAM oracles for the `scope_editor` cold-start transfer-test instrument.
//!
//! Each oracle drives the REAL public-API composition the runnable example uses — the
//! `scope` core module is included directly with `#[path]`, the same trick the flagship
//! workbench slice uses — and, where it can, pairs a passing control with a failing
//! mutant so the check is never a tautology.
//!
//! The seams exercised: validated numeric edit ↔ model (refuse, not clamp), edit
//! episode ↔ undo transaction, frame tick ↔ wall time, view ↔ data (pan/zoom), painted
//! cell ↔ probed time, and painted layout ↔ pointer target (the hit-map seam).

#[path = "../examples/scope_editor/scope.rs"]
mod scope;

use scope::{
    apply_intent, default_script, digest, plot_spec, plot_view, probe, replay, replay_with, sample,
    time_at_cell, AmpCmd, Intent, ScopeModel,
};

use gibson::edit::{Compound, UndoLog};
use gibson::{FixedTimestep, Rect, SubcellGlyphMode};
use std::time::Duration;

fn fresh() -> (ScopeModel, UndoLog<Compound<AmpCmd>>) {
    (ScopeModel::new(), UndoLog::new())
}

/// Float equality for the oracles below — avoids a bare `==` on `f64` while the
/// values compared here are exact (step multiples) or bit-identical by construction.
fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

// ---- seam: validated numeric edit ↔ model (Recipe 4 — refuse, never clamp) ------

#[test]
fn amplitude_refuses_out_of_range_and_never_clamps() {
    let (mut m, mut u) = fresh();

    // Control: a legal step commits a new value.
    let before = m.amplitude;
    apply_intent(&mut m, &mut u, Intent::AmpUp);
    assert!(m.amplitude > before, "a legal step must commit");

    // Drive to the ceiling: more than enough raises to pin at AMP_MAX.
    for _ in 0..30 {
        apply_intent(&mut m, &mut u, Intent::AmpUp);
    }
    assert!(
        approx(m.amplitude, scope::AMP_MAX),
        "value pins at the ceiling"
    );

    // Mutant watch: one more raise is REFUSED (status says so) and the authoritative
    // value is unchanged — it is not clamped to some value past the ceiling.
    apply_intent(&mut m, &mut u, Intent::AmpUp);
    assert!(
        approx(m.amplitude, scope::AMP_MAX),
        "out-of-range step changes nothing"
    );
    assert!(
        m.status.contains("refused"),
        "the refusal is surfaced, got: {:?}",
        m.status
    );

    // A direct over-range set is refused the same way.
    let held = m.amplitude;
    apply_intent(&mut m, &mut u, Intent::SetAmp(scope::AMP_MAX + 2.0));
    assert!(
        approx(m.amplitude, held),
        "a direct over-range set is refused"
    );
}

// ---- seam: edit episode ↔ undo transaction (Recipe 3) ---------------------------

#[test]
fn undo_redo_round_trips_an_amplitude_edit() {
    let (mut m, mut u) = fresh();
    let d0 = digest(&m);

    apply_intent(&mut m, &mut u, Intent::AmpUp);
    let d1 = digest(&m);
    assert_ne!(d0, d1, "an edit changes the model digest");
    assert!(u.can_undo(), "the edit is on the undo stack");

    apply_intent(&mut m, &mut u, Intent::Undo);
    assert_eq!(digest(&m), d0, "undo restores the exact prior state");

    apply_intent(&mut m, &mut u, Intent::Redo);
    assert_eq!(digest(&m), d1, "redo re-applies the edit");
}

// ---- seam: frame tick ↔ wall time (Recipe 6) ------------------------------------

#[test]
fn fixed_timestep_advances_the_model_clock_with_conserved_debt() {
    let (mut m, mut u) = fresh();
    m.playing = true;
    let start = m.tick;

    let mut ts = FixedTimestep::new(Duration::from_millis(scope::TICK_MS));
    let demand = ts.advance(Duration::from_millis(scope::TICK_MS * 5));
    for _ in 0..demand.ticks {
        apply_intent(&mut m, &mut u, Intent::Tick);
    }

    assert!(demand.ticks >= 1, "a five-step span owes at least one tick");
    assert_eq!(
        m.tick,
        start + demand.ticks as u64,
        "ticks advance the clock"
    );
    // Law IV: owed == executed + deferred + discarded.
    assert_eq!(
        demand.owed,
        demand.executed + demand.deferred + demand.discarded,
        "tick debt is conserved"
    );

    // Step advances exactly one tick even while paused.
    let (mut m2, mut u2) = fresh();
    let t = m2.tick;
    apply_intent(&mut m2, &mut u2, Intent::Step);
    assert_eq!(m2.tick, t + 1);
}

// ---- seam: view ↔ data (Recipe 5 — pan/zoom change the view, not the signal) -----

#[test]
fn zoom_and_pan_change_only_the_view() {
    let (mut m, mut u) = fresh();
    let (amp, freq) = (m.amplitude, m.frequency);
    let span0 = m.view_span;

    apply_intent(&mut m, &mut u, Intent::ZoomIn);
    assert!(m.view_span < span0, "zoom in shrinks the visible span");
    assert!(
        m.view_span >= scope::MIN_SPAN,
        "span is clamped to MIN_SPAN"
    );
    assert!(m.follow, "zoom does not disturb follow mode");

    apply_intent(&mut m, &mut u, Intent::PanLeft);
    assert!(!m.follow, "a pan drops follow mode");

    // The underlying signal is untouched by any view op.
    for &t in &[0.0, 0.37, 1.1, 2.5] {
        assert!(approx(
            sample(t, amp, freq),
            sample(t, m.amplitude, m.frequency)
        ));
    }
}

// ---- seam: cursor ↔ signal (probe reads the value at the cursor time) ------------

#[test]
fn probe_reads_the_signal_at_the_cursor() {
    let (mut m, mut u) = fresh();
    for _ in 0..5 {
        apply_intent(&mut m, &mut u, Intent::CursorRight);
    }
    assert!(
        approx(probe(&m), sample(m.cursor_t, m.amplitude, m.frequency)),
        "the probe is exactly the signal at the cursor"
    );

    // Cursor motion is reversible arithmetic.
    let c = m.cursor_t;
    apply_intent(&mut m, &mut u, Intent::CursorLeft);
    apply_intent(&mut m, &mut u, Intent::CursorRight);
    assert!((m.cursor_t - c).abs() < 1e-12);
}

// ---- seam: painted cell ↔ probed time (Law L3, via the plot's own transform) -----

#[test]
fn pointer_maps_a_painted_cell_back_to_time() {
    let m = ScopeModel::new();
    let (t0, t1) = m.window();
    let area = Rect::new(0, 0, 100, 30);

    // Compile the same plot the surface would draw to recover the interior rect.
    let (layout, _report) = gibson::plot::compile(&plot_spec(&m), &plot_view(&m).unwrap(), area)
        .expect("a valid plot configuration");
    let pr = layout.plot_rect;
    assert!(pr.width > 2 && pr.height > 2, "a real plot interior exists");

    let mid_row = pr.y + pr.height / 2;
    let left = time_at_cell(&m, area.width, area.height, pr.x, mid_row).unwrap();
    let right = time_at_cell(&m, area.width, area.height, pr.x + pr.width - 1, mid_row).unwrap();

    assert!(
        left < right,
        "time increases left→right ({left} !< {right})"
    );
    assert!(
        left >= t0 - 0.1 && right <= t1 + 0.1,
        "mapped times stay inside the window [{t0}, {t1}]: {left}..{right}"
    );

    // Control vs mutant: the top-left origin sits in the axis gutter, outside the
    // interior, so it must return None rather than a fabricated time.
    assert!(
        time_at_cell(&m, area.width, area.height, 0, 0).is_none(),
        "a cell outside the plot interior maps to no time"
    );
}

// ---- seam: deterministic replay (Recipe 7) --------------------------------------

#[test]
fn scripted_replay_is_deterministic_and_exercises_the_features() {
    let script = default_script();
    assert_eq!(
        digest(&replay(&script)),
        digest(&replay(&script)),
        "the same trace replays to the same model digest"
    );

    let (model, undo) = replay_with(&script);
    assert!(undo.can_undo(), "the script left undo history");
    // The script climbs to the ceiling, undoes twice, redoes once, then steps down —
    // landing at 4.0 (proving the ceiling path and the undo/redo seam together).
    assert!(
        approx(model.amplitude, 4.0),
        "replay lands at the expected amplitude, got {}",
        model.amplitude
    );
}

#[test]
fn selfcheck_passes() {
    assert!(scope::run_selfcheck(), "headless --selfcheck must pass");
}

// ---- seam: painted layout ↔ pointer target (the hit-map / painted-frame seam) ----

#[test]
fn a_keyed_raster_resolves_as_a_painted_hit_target() {
    use gibson::ui::prelude::*;
    use gibson::{compute_layout, Surface};

    let (w, h) = (100u16, 30u16);
    let scope_h = h - 2;

    // The raster-as-target pattern from Recipe 1, applied to a Surface-backed plot:
    // a handler makes it an eligible, painted, hittable target.
    let el: Element<()> = screen().child(
        column()
            .padding(0)
            .gap(0)
            .child(raster(Surface::new(w, scope_h)).key("scope").on_press(()))
            .child(text("hud line one"))
            .child(text("hud line two")),
    );

    let env = UiEnvironment {
        width: w,
        height: h,
        ..Default::default()
    };
    let mut compiled = compile(&el, &BuildCx::new(skins::SWISS_SIGNAL, env)).expect("compiles");
    compute_layout(&mut compiled.node, w, h).expect("layout resolves");
    let hit = compiled
        .hit_map(Rect::new(0, 0, w, h))
        .expect("the painted layout resolves into a hit map");

    let target = hit
        .targets()
        .iter()
        .find(|t| matches!(&t.key, Key::Named(s) if s == "scope"))
        .expect("the keyed raster is a resolved painted target");

    // Honest about chrome: we assert the painted rect is a real, bounded region, not
    // that it equals the full width (a skin may inset it — exactly why the runtime
    // uses a plain-Node split instead of reading this back every frame).
    assert!(target.rect.width > 0 && target.rect.height > 0, "non-empty");
    assert!(target.rect.height <= h, "within the terminal");

    // A cell inside the resolved rect hit-tests back to the scope target.
    let cx = target.rect.x + target.rect.width / 2;
    let cy = target.rect.y + target.rect.height / 2;
    let landed = hit.hit(cx, cy).expect("a click inside lands on a target");
    assert!(
        matches!(&landed.key, Key::Named(s) if s == "scope"),
        "the interior hit resolves to the scope, not empty space"
    );
}

// ---- seam: plot realization draws real trace ink --------------------------------

#[test]
fn plot_realization_draws_braille_trace_ink() {
    let m = ScopeModel::new();
    let (surface, report) = gibson::plot::plot(
        &plot_spec(&m),
        &plot_view(&m).unwrap(),
        Rect::new(0, 0, 100, 30),
        SubcellGlyphMode::Braille2x4,
    )
    .expect("the trace realizes");
    assert!(report.finite_samples > 0, "the trace samples were accepted");

    // A braille subcell glyph (U+2800..=U+28FF) must actually appear — the trace drew.
    let ink = surface.to_visible_lines().join("\n");
    assert!(
        ink.chars().any(|c| ('\u{2800}'..='\u{28FF}').contains(&c)),
        "the realized surface must carry braille trace ink"
    );
}
