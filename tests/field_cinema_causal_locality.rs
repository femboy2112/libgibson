//! Temporal causal-locality battery for the source-driven cinematic slice (#191, v0.6 Gate I).
//!
//! `field_cinema_mutation.rs::determinism_across_a_sequence` proves a sequence *replays*
//! byte-identically. That is necessary but NOT sufficient for causal locality: a pipeline can be
//! perfectly deterministic and still leak a perturbation BACKWARD in time (a global / look-ahead
//! colour normalization) or smear it across the whole trace. Determinism is blind to that bug
//! class; these tests are not.
//!
//! The property, stated for this synthetic Model: an edit injected at tick `T` through the
//! frame-buffer seam must not change the shot, observation or pixels at any tick `< T` (no
//! retroactive effect), and a *point* edit (only tick `T`) must leave every tick `!= T` untouched
//! (no cross-tick memory in either direction).
//!
//! HONEST FRAMING: today's cinema pipeline is memoryless — `stage_tick` rebuilds each frame purely
//! from `(tick, selection)` with no carried state — so these properties hold *by construction*.
//! The value of this battery is as a REGRESSION FENCE for the first stateful thing #191 adds (a
//! camera easing over prior ticks, a cross-tick fixed colour range, history-dependent smoothing):
//! the teeth test exhibits exactly such a pipeline (a deterministic look-ahead colour range) and
//! shows the locality assertion catches it while the determinism law does not. Scope: TEMPORAL
//! locality only; spatial confinement of a sub-threshold tweak is deliberately out of scope (it
//! depends on the shot's zoom/frame mapping and is a separate, later probe).
//!
//! Perturbation goes through a test-local `stage_from` seam that mirrors `stage_tick`'s body over
//! caller-supplied buffers (the shipped `stage_tick` builds its own buffers and has no injection
//! point); a control test pins the twin to the real `stage_tick`.

#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;

use cinema::{
    compose, observe, render_shot, stage_tick, view_focused_on, Model, SourceId, Staged, COLS,
};
use gibson::field_view::{FieldView, RangePolicy};
use gibson::shape::Point;
use gibson::{Rect, Surface};

const ANCHOR: Rect = Rect {
    x: 0,
    y: 0,
    width: 56,
    height: 18,
};

const T: usize = 4;
const N: u64 = 10;
/// An unmasked cell well away from the dropout patch (cols 2..8 × rows 16..22), dominating enough
/// to hijack the camera when present — the same shape `global_observation` uses.
const SPIKE: usize = COLS + (COLS - 2); // row 1, col COLS-2
const SPIKE_VALUE: f64 = 1.0e6;

fn model() -> Model {
    Model::new(SourceId(7), 1)
}

/// A faithful twin of `cinema::stage_tick` (examples/field_cinema/cinema.rs) that renders from
/// CALLER-supplied buffers, so a perturbation can be injected. The control test asserts this
/// equals `stage_tick` on unperturbed buffers.
fn stage_from(
    m: &Model,
    tick: u64,
    data: &[f64],
    mask: &[bool],
    sel: Point,
    anchor: Rect,
) -> Staged {
    let view = FieldView::new(m.extent(), data, Some(mask)).expect("geometry matches data/mask");
    let observation = observe(&view, m.source(), m.epoch(), tick);
    let probe = view.probe(sel);
    let shot = compose(&observation, &probe, m.extent());
    let frame = render_shot(&view, &shot, anchor);
    Staged {
        observation,
        shot,
        frame,
    }
}

/// Stage ticks 0..n, applying `perturb` to each tick's data buffer before rendering (real
/// per-frame `RangePolicy::Auto`).
fn trace(m: &Model, n: u64, perturb: impl Fn(u64, &mut Vec<f64>)) -> Vec<Staged> {
    (0..n)
        .map(|t| {
            let (mut data, mask) = m.frame_buffers(t);
            perturb(t, &mut data);
            stage_from(m, t, &data, &mask, m.hotspot_center(t), ANCHOR)
        })
        .collect()
}

// --- control: the test-local seam is a faithful twin of the shipped stage_tick ---------------

#[test]
fn stage_from_twin_matches_stage_tick_unperturbed() {
    let m = model();
    for t in 0..8u64 {
        let (data, mask) = m.frame_buffers(t);
        let sel = m.hotspot_center(t);
        let twin = stage_from(&m, t, &data, &mask, sel, ANCHOR);
        let real = stage_tick(&m, t, sel, ANCHOR);
        assert_eq!(
            twin.observation, real.observation,
            "twin observation diverged @ tick {t}"
        );
        assert_eq!(twin.shot, real.shot, "twin shot diverged @ tick {t}");
        assert_eq!(
            twin.frame.surface, real.frame.surface,
            "twin pixels diverged @ tick {t} — the seam is not faithful"
        );
    }
}

// --- temporal locality: a STEP edit from T cannot leak backward ------------------------------

#[test]
fn step_edit_from_t_never_leaks_into_earlier_ticks() {
    let m = model();
    let (_, mask0) = m.frame_buffers(T as u64);
    assert!(
        mask0[SPIKE],
        "precondition: the spike cell must be unmasked (so it is actually read)"
    );

    let base = trace(&m, N, |_, _| {});
    let step = trace(&m, N, |t, d| {
        if t as usize >= T {
            d[SPIKE] = SPIKE_VALUE;
        }
    });

    // Non-vacuity: the edit actually does something at T and onward (it hijacks the camera).
    assert_ne!(
        base[T].frame.surface, step[T].frame.surface,
        "the step edit must change tick T, or the test proves nothing"
    );
    for t in T..(N as usize) {
        assert_ne!(
            base[t].frame.surface, step[t].frame.surface,
            "tick {t} >= T must reflect the persistent step edit"
        );
    }

    // Causal locality: ticks before T are byte-identical — observation, shot AND pixels.
    for t in 0..T {
        assert_eq!(
            base[t].observation, step[t].observation,
            "backward leak into observation @ tick {t}"
        );
        assert_eq!(
            base[t].shot, step[t].shot,
            "backward leak into shot @ tick {t}"
        );
        assert_eq!(
            base[t].frame.surface, step[t].frame.surface,
            "backward leak into pixels @ tick {t} < T — a future edit changed the past"
        );
    }
}

// --- temporal locality: a POINT edit at T touches only T (no cross-tick memory) --------------

#[test]
fn point_edit_at_t_touches_only_tick_t() {
    let m = model();
    let base = trace(&m, N, |_, _| {});
    let point = trace(&m, N, |t, d| {
        if t as usize == T {
            d[SPIKE] = SPIKE_VALUE;
        }
    });

    assert_ne!(
        base[T].frame.surface, point[T].frame.surface,
        "the point edit must change tick T (non-vacuous)"
    );
    for t in 0..(N as usize) {
        if t == T {
            continue;
        }
        assert_eq!(
            base[t].frame.surface, point[t].frame.surface,
            "tick {t} != T changed — a point edit at T has nonzero cross-tick memory (leak forward or backward)"
        );
    }
}

// --- teeth: a deterministic look-ahead pipeline leaks backward; determinism cannot see it -----

/// A FIXED colour policy pinned to the resolved range of one perturbed tick (rendered with
/// `Auto`) — the scale a look-ahead / cross-tick pipeline would reuse across the whole run.
fn global_fixed_policy(m: &Model, tick: u64, perturb: impl Fn(u64, &mut Vec<f64>)) -> RangePolicy {
    let (mut data, mask) = m.frame_buffers(tick);
    perturb(tick, &mut data);
    let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let obs = observe(&view, m.source(), m.epoch(), tick);
    let shot = compose(&obs, &view.probe(m.hotspot_center(tick)), m.extent());
    let wv = view_focused_on(shot.focus, shot.zoom, ANCHOR);
    let range = view
        .render(&wv, ANCHOR, shot.colormap, RangePolicy::Auto)
        .report
        .resolved_range
        .expect("an Auto render of a finite field discloses a range");
    RangePolicy::Fixed(range)
}

/// A deterministic but NON-local pipeline: every tick is rendered with one fixed colour `policy`
/// (as a cross-tick / look-ahead normalization would), so a future perturbation that inflates the
/// range recolours earlier ticks.
fn trace_fixed(
    m: &Model,
    n: u64,
    perturb: impl Fn(u64, &mut Vec<f64>),
    policy: RangePolicy,
) -> Vec<Surface> {
    (0..n)
        .map(|t| {
            let (mut data, mask) = m.frame_buffers(t);
            perturb(t, &mut data);
            let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
            let obs = observe(&view, m.source(), m.epoch(), t);
            let shot = compose(&obs, &view.probe(m.hotspot_center(t)), m.extent());
            let wv = view_focused_on(shot.focus, shot.zoom, ANCHOR);
            view.render(&wv, ANCHOR, shot.colormap, policy).surface
        })
        .collect()
}

#[test]
fn determinism_is_blind_to_a_look_ahead_leak_but_locality_is_not() {
    let m = model();
    let step = |t: u64, d: &mut Vec<f64>| {
        if t as usize >= T {
            d[SPIKE] = SPIKE_VALUE;
        }
    };

    // The future spike inflates the global colour range — the mechanism of the backward leak.
    let base_policy = global_fixed_policy(&m, T as u64, |_, _| {});
    let step_policy = global_fixed_policy(&m, T as u64, step);
    assert_ne!(
        base_policy, step_policy,
        "teeth precondition: the future spike must inflate the global colour range"
    );

    let base_leaky = trace_fixed(&m, N, |_, _| {}, base_policy);
    let step_leaky = trace_fixed(&m, N, step, step_policy);

    // Determinism is BLIND: the leaky pipeline still replays byte-identically, so
    // determinism_across_a_sequence would happily certify it.
    let step_leaky_again = trace_fixed(&m, N, step, step_policy);
    assert_eq!(
        step_leaky, step_leaky_again,
        "the look-ahead pipeline is still deterministic — the determinism law cannot flag it"
    );

    // Causal locality CATCHES it: at least one tick < T recolours under the inflated range.
    let leaked = (0..T).any(|t| base_leaky[t] != step_leaky[t]);
    assert!(
        leaked,
        "the locality battery must flag a look-ahead pipeline that recolours ticks < T from a future edit"
    );
}
