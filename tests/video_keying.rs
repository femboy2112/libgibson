//! Unit tests for the deterministic chroma-key primitives (the moonshot's key
//! module), exercised with no `gibson`, no ffmpeg, and no terminal.

#[path = "../examples/temporal_video_compositor/key.rs"]
mod key;

use key::{alpha_of, composite_over, despill, greenness, key_pixel, Bbox, KeyParams};

const KEY_GREEN: [u8; 3] = [0, 254, 2]; // measured clean-key value on the clip

#[test]
fn pure_green_is_fully_transparent() {
    let p = KeyParams::default();
    assert_eq!(alpha_of(KEY_GREEN, &p), 0.0);
    assert_eq!(alpha_of([0, 255, 0], &p), 0.0);
}

#[test]
fn non_green_foreground_is_fully_opaque() {
    let p = KeyParams::default();
    for px in [
        [200u8, 180, 170], // skin
        [30, 30, 30],      // dark cloth
        [255, 0, 0],       // red
        [120, 120, 128],   // grey shirt
        [10, 40, 90],      // blue
    ] {
        assert_eq!(alpha_of(px, &p), 1.0, "expected opaque for {px:?}");
    }
}

#[test]
fn alpha_is_monotonic_nonincreasing_in_greenness() {
    let p = KeyParams::default();
    // Sweep green up while holding r,b: green-ness rises, alpha must not rise.
    let mut last = 2.0f32;
    for g in 0u16..=255 {
        let a = alpha_of([40, g as u8, 40], &p);
        assert!(a <= last + 1e-6, "alpha rose at g={g}: {a} > {last}");
        assert!((0.0..=1.0).contains(&a));
        last = a;
    }
}

#[test]
fn alpha_is_deterministic() {
    let p = KeyParams::default();
    let px = [60, 140, 55];
    assert_eq!(alpha_of(px, &p), alpha_of(px, &p));
}

#[test]
fn greenness_metric_signs() {
    assert!(greenness([0, 254, 2]) > 200.0);
    assert!(greenness([200, 180, 170]) < 0.0); // warm skin: not green
    assert_eq!(greenness([50, 50, 50]), 0.0); // neutral
}

#[test]
fn despill_clamps_green_only() {
    // Green excess is clamped down to max(r,b); nothing else moves.
    assert_eq!(despill([100, 200, 100], true), [100, 100, 100]);
    assert_eq!(despill([80, 200, 120], true), [80, 120, 120]);
    // No green excess -> unchanged.
    assert_eq!(despill([200, 50, 180], true), [200, 50, 180]);
    // Disabled -> identity.
    assert_eq!(despill([100, 200, 100], false), [100, 200, 100]);
}

#[test]
fn key_pixel_bundles_despill_and_alpha() {
    let p = KeyParams::default();
    let (fg, a) = key_pixel([0, 254, 2], &p);
    assert_eq!(a, 0.0);
    // Despill applied even to keyed pixels (color irrelevant at alpha 0, but stable).
    assert_eq!(fg, [0, 2, 2]);
}

#[test]
fn composite_endpoints_are_exact() {
    let fg = [200, 100, 50];
    let bg = [10, 20, 30];
    assert_eq!(composite_over(fg, 0.0, bg), bg);
    assert_eq!(composite_over(fg, 1.0, bg), fg);
    // Out-of-range alpha is clamped, never NaN/overflow.
    assert_eq!(composite_over(fg, -5.0, bg), bg);
    assert_eq!(composite_over(fg, 5.0, bg), fg);
}

#[test]
fn composite_midpoint_rounds() {
    assert_eq!(
        composite_over([100, 100, 100], 0.5, [0, 0, 0]),
        [50, 50, 50]
    );
    assert_eq!(
        composite_over([255, 255, 255], 0.5, [0, 0, 0]),
        [128, 128, 128]
    );
}

#[test]
fn degenerate_thresholds_are_a_hard_cut_not_a_nan() {
    let p = KeyParams {
        thr_lo: 50.0,
        thr_hi: 50.0,
        despill: true,
    };
    assert_eq!(alpha_of([40, 100, 40], &p), 0.0); // greenness 60 -> above -> keyed
    assert_eq!(alpha_of([40, 90, 40], &p), 1.0); // greenness 50 -> at threshold -> opaque
    assert_eq!(alpha_of([40, 80, 40], &p), 1.0); // greenness 40 -> below -> opaque
}

#[test]
fn bbox_union_treats_empty_as_identity() {
    let a = Bbox {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
    };
    assert_eq!(a.union(&Bbox::EMPTY), a);
    assert_eq!(Bbox::EMPTY.union(&a), a);
    let b = Bbox {
        x: 25,
        y: 5,
        w: 10,
        h: 10,
    };
    // Union spans (10..35, 5..30).
    assert_eq!(
        a.union(&b),
        Bbox {
            x: 10,
            y: 5,
            w: 25,
            h: 25
        }
    );
}
