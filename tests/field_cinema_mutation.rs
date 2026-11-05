//! Mutation battery for the source-driven cinematic slice (#191).
//!
//! These tests are the gate: they prove the on-screen output is *caused by* the
//! claimed state source and nothing else (Law L10 — claims scoped to evidence).
//!
//! * **positive** — moving the model moves the shot and the pixels; a probe on the
//!   peak changes the shot and the pixels;
//! * **negative control** — perturbing a cell the grammar provably does not read
//!   (a masked cell's hidden value) changes nothing;
//! * **teeth** — a field-*blind* grammar fails the positive dependency, so the
//!   positive test is non-vacuous;
//! * **anti-depiction** — holding the framing fixed and varying only the data
//!   still changes the pixels, so the pixels ARE the field, not a canned depiction;
//! * **determinism** — the same `(tick, selection)` renders byte-identically.
//!
//! The example's modules are included by path so the tests exercise exactly the
//! code the binary runs (the in-module `#[cfg(test)]` unit laws come along too).

#[path = "../examples/field_cinema/cinema.rs"]
mod cinema;

use cinema::{
    compose, compose_blind, field_center, observe, render_shot, stage_tick, Emphasis, Model, Shot,
    SourceId, COLS, DROPOUT_DENSITY_THRESHOLD, WIDE_ZOOM,
};
use gibson::field_view::{Colormap, FieldView};
use gibson::shape::Point;
use gibson::Rect;

const ANCHOR: Rect = Rect {
    x: 0,
    y: 0,
    width: 56,
    height: 18,
};

fn model() -> Model {
    Model::new(SourceId(7), 1)
}

// --- determinism -----------------------------------------------------------

#[test]
fn determinism_same_tick_same_shot_and_pixels() {
    let m = model();
    let sel = m.hotspot_center(3);
    let a = stage_tick(&m, 3, sel, ANCHOR);
    let b = stage_tick(&m, 3, sel, ANCHOR);
    assert_eq!(
        a.shot, b.shot,
        "shot must be a pure function of (tick, selection)"
    );
    assert_eq!(
        a.frame.surface, b.frame.surface,
        "pixels must replay identically — no wall-clock, no RNG on the frame path"
    );
}

#[test]
fn determinism_across_a_sequence() {
    let m = model();
    let run = |()| -> Vec<gibson::Surface> {
        (0..8)
            .map(|t| stage_tick(&m, t, m.hotspot_center(t), ANCHOR).frame.surface)
            .collect()
    };
    assert_eq!(
        run(()),
        run(()),
        "a replayed sequence must be byte-identical"
    );
}

// --- positive dependency on the model --------------------------------------

#[test]
fn field_dependency_focus_tracks_the_moving_peak() {
    let m = model();
    let sel = m.hotspot_center(0); // hold the selection fixed; only the model moves
    let early = stage_tick(&m, 1, sel, ANCHOR);
    let late = stage_tick(&m, 6, sel, ANCHOR);
    // precondition: the model's peak actually marched right across these ticks.
    assert!(
        m.hotspot_center(6).x > m.hotspot_center(1).x,
        "model precondition: hotspot marches right"
    );
    // consequence: the focus marched right, and the pixels changed.
    assert!(
        late.shot.focus.x > early.shot.focus.x,
        "focus must track the peak right: {} !> {}",
        late.shot.focus.x,
        early.shot.focus.x
    );
    assert_ne!(
        early.frame.surface, late.frame.surface,
        "a moved model must produce a different frame"
    );
}

// --- positive dependency on the selection ----------------------------------

#[test]
fn selection_dependency_probe_on_peak_punches_in() {
    let m = model();
    let (data, mask) = m.frame_buffers(4);
    let view = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let obs = observe(&view, m.source(), m.epoch(), 4);
    let on = obs.peak_center.expect("tick 4 has a finite peak");
    let cold = Point::new(1.0, 1.0);

    let shot_on = compose(&obs, &view.probe(on), m.extent());
    let shot_cold = compose(&obs, &view.probe(cold), m.extent());

    assert_eq!(
        shot_on.emphasis,
        Emphasis::PunchIn,
        "probe on the peak punches in"
    );
    assert_eq!(
        shot_cold.emphasis,
        Emphasis::Wide,
        "a cold probe holds wide"
    );
    assert!(
        shot_on.zoom > shot_cold.zoom,
        "the punch-in must zoom tighter: {} !> {}",
        shot_on.zoom,
        shot_cold.zoom
    );

    let f_on = render_shot(&view, &shot_on, ANCHOR);
    let f_cold = render_shot(&view, &shot_cold, ANCHOR);
    assert_ne!(
        f_on.surface, f_cold.surface,
        "the selection must change the painted frame"
    );
}

// --- negative control: a masked cell's hidden value is never read ----------

#[test]
fn decoy_invariance_masked_cell_value_never_reaches_the_shot() {
    let m = model();
    let (mut data, mask) = m.frame_buffers(4);
    let sel = Point::new(1.0, 1.0); // cold, so emphasis is Wide and stays Wide

    let v0 = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let o0 = observe(&v0, m.source(), m.epoch(), 4);
    let s0 = compose(&o0, &v0.probe(sel), m.extent());
    let f0 = render_shot(&v0, &s0, ANCHOR);

    // The dropout patch is cols 2..8 × rows 16..22; (col 2, row 16) is masked.
    let decoy = 16 * COLS + 2;
    assert!(!mask[decoy], "test precondition: the decoy cell is masked");

    // A value that, if read, would dominate every other cell by nine orders of
    // magnitude — hijacking the peak, the range, the focus and the whole frame.
    data[decoy] = 1.0e9;

    let v1 = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let o1 = observe(&v1, m.source(), m.epoch(), 4);
    let s1 = compose(&o1, &v1.probe(sel), m.extent());
    let f1 = render_shot(&v1, &s1, ANCHOR);

    assert_eq!(o0, o1, "a masked cell must not change the observation");
    assert_eq!(s0, s1, "a masked cell must not change the shot");
    assert_eq!(
        f0.surface, f1.surface,
        "a masked cell must not change a single painted pixel"
    );
}

// --- teeth: a field-blind grammar fails the positive dependency ------------

#[test]
fn field_blind_grammar_freezes_the_focus_so_the_positive_test_has_teeth() {
    let m = model();
    let sel = m.hotspot_center(0);
    let (d1, mk) = m.frame_buffers(1);
    let (d6, _) = m.frame_buffers(6);
    let v1 = FieldView::new(m.extent(), &d1, Some(&mk)).unwrap();
    let v6 = FieldView::new(m.extent(), &d6, Some(&mk)).unwrap();
    let o1 = observe(&v1, m.source(), m.epoch(), 1);
    let o6 = observe(&v6, m.source(), m.epoch(), 6);

    // The real grammar tracks the field — focus moves (this is the property
    // `field_dependency_focus_tracks_the_moving_peak` asserts end to end).
    let real1 = compose(&o1, &v1.probe(sel), m.extent());
    let real6 = compose(&o6, &v6.probe(sel), m.extent());
    assert_ne!(
        real1.focus, real6.focus,
        "the real grammar must track the field"
    );

    // A field-blind grammar does NOT: focus is frozen. If the causal wiring were
    // ever severed, the positive dependency test would catch exactly this shape.
    let blind1 = compose_blind(&o1, &v1.probe(sel), m.extent());
    let blind6 = compose_blind(&o6, &v6.probe(sel), m.extent());
    assert_eq!(
        blind1.focus, blind6.focus,
        "a field-blind grammar must freeze the focus (the failure the test guards)"
    );
}

// --- anti-depiction: the pixels ARE the field ------------------------------

#[test]
fn pixels_are_the_field_not_a_depiction() {
    // Hold the SHOT fixed (identical focus / zoom / colormap) and vary only the
    // underlying data: the rendered pixels still differ, so the pixels are the
    // field itself, not a canned image keyed off the shot parameters.
    let m = model();
    let (d2, mk) = m.frame_buffers(2);
    let (d8, _) = m.frame_buffers(8);
    let v2 = FieldView::new(m.extent(), &d2, Some(&mk)).unwrap();
    let v8 = FieldView::new(m.extent(), &d8, Some(&mk)).unwrap();

    let shot = Shot {
        focus: field_center(m.extent()),
        zoom: WIDE_ZOOM,
        colormap: Colormap::Density,
        emphasis: Emphasis::Wide,
        source: m.source(),
        epoch: m.epoch(),
        tick: 0,
        probe_reading: None,
    };
    let f2 = render_shot(&v2, &shot, ANCHOR);
    let f8 = render_shot(&v8, &shot, ANCHOR);
    assert_ne!(
        f2.surface, f8.surface,
        "same framing, different field ⇒ different pixels"
    );
}

// --- global observation: the camera finds a peak anywhere, even off-frame ---

#[test]
fn global_observation_camera_finds_an_off_frame_peak() {
    // The honest converse of the masked-cell negative control. The instrument
    // observes the WHOLE field, not just the framed ROI, so a dominating *unmasked*
    // sample planted far from the current hotspot — well outside the frame — is a
    // positive dependency: it pulls the camera to it. (This is the exact case the
    // module docstring must NOT claim as an invariance; only masked cells are.)
    let m = model();
    let (mut data, mask) = m.frame_buffers(4);
    let sel = Point::new(1.0, 1.0);

    let v0 = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let base_focus = observe(&v0, m.source(), m.epoch(), 4)
        .peak_center
        .expect("tick 4 has a finite peak");

    // A dominating spike in the far top-right corner — unmasked, and nowhere near
    // the dropout patch (cols 2..8 × rows 16..22).
    let (sx, sy) = (COLS - 2, 1);
    let spike = sy * COLS + sx;
    assert!(mask[spike], "test precondition: the spike cell is unmasked");
    data[spike] = 1.0e6;

    let v1 = FieldView::new(m.extent(), &data, Some(&mask)).unwrap();
    let o1 = observe(&v1, m.source(), m.epoch(), 4);
    let s1 = compose(&o1, &v1.probe(sel), m.extent());

    assert_ne!(
        s1.focus, base_focus,
        "a dominating sample anywhere in the field must move the camera"
    );
    assert_eq!(
        s1.focus,
        m.extent().cell_center(sx, sy).unwrap(),
        "the camera must focus on the global peak wherever it lives"
    );
}

// --- the dropout rule is real ----------------------------------------------

#[test]
fn dropout_fraction_drives_the_colormap() {
    let m = model();
    let data = m.field_data(4);

    // Low dropout ⇒ Heat.
    let mut small = vec![true; COLS * cinema::ROWS];
    small[0] = false;
    let v_lo = FieldView::new(m.extent(), &data, Some(&small)).unwrap();
    let o_lo = observe(&v_lo, m.source(), m.epoch(), 4);
    let s_lo = compose(&o_lo, &v_lo.probe(Point::new(1.0, 1.0)), m.extent());
    assert!(o_lo.masked_fraction < DROPOUT_DENSITY_THRESHOLD);
    assert_eq!(
        s_lo.colormap,
        Colormap::Heat,
        "light dropout keeps the heat ramp"
    );

    // Heavy dropout ⇒ the colour-free density ramp.
    let mut big = vec![true; COLS * cinema::ROWS];
    for c in big.iter_mut().take((COLS * cinema::ROWS) / 5) {
        *c = false; // 20% masked
    }
    let v_hi = FieldView::new(m.extent(), &data, Some(&big)).unwrap();
    let o_hi = observe(&v_hi, m.source(), m.epoch(), 4);
    let s_hi = compose(&o_hi, &v_hi.probe(Point::new(1.0, 1.0)), m.extent());
    assert!(o_hi.masked_fraction > DROPOUT_DENSITY_THRESHOLD);
    assert_eq!(
        s_hi.colormap,
        Colormap::Density,
        "heavy dropout falls back to the colour-free ramp"
    );
}

// --- the field_view conservation law survives staging ----------------------

#[test]
fn staged_frames_conserve_cells() {
    let m = model();
    for tick in 0..10 {
        let sel = m.hotspot_center(tick);
        let staged = stage_tick(&m, tick, sel, ANCHOR);
        assert!(
            staged.frame.report.conserves(),
            "tick {tick}: every rendered cell must be accounted for"
        );
    }
}
