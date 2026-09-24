use std::time::Duration;

use gibson::raster::{RgbRaster, MAX_RASTER_DIMENSION};
use gibson::raster_fx::{
    metaballs, palette, radial_glow, ring, vortex, FeedbackBuffer, RasterFx, RasterFxWorkspace,
};

fn impulse() -> RgbRaster {
    let mut raster = RgbRaster::new(9, 7);
    raster.set(4, 3, (240, 120, 60));
    raster
}

#[test]
fn empty_chain_is_exact_identity() {
    let mut raster = impulse();
    let original = raster.clone();
    RasterFx::apply_chain(&mut raster, &[]);
    assert_eq!(raster, original);
}

#[test]
fn glow_has_bounded_support_and_preserves_source() {
    let mut raster = impulse();
    RasterFx::apply_chain(
        &mut raster,
        &[RasterFx::Glow {
            radius: 1,
            threshold: 200,
            strength: 1.0,
        }],
    );
    assert_eq!(raster.get(0, 0), Some((0, 0, 0)));
    assert_eq!(raster.get(4, 3), Some((255, 133, 67)));
    assert_eq!(raster.get(3, 2), Some((27, 13, 7)));
    assert_eq!(raster.get(2, 3), Some((0, 0, 0)));
}

#[test]
fn effect_order_is_observable() {
    let glow = RasterFx::Glow {
        radius: 1,
        threshold: 200,
        strength: 1.0,
    };
    let scan = RasterFx::Scanlines { strength: 1.0 };
    let mut a = impulse();
    let mut b = a.clone();
    RasterFx::apply_chain(&mut a, &[glow, scan]);
    RasterFx::apply_chain(&mut b, &[scan, glow]);
    assert_ne!(a, b);
    assert!(b.pixels().iter().all(|p| *p == (0, 0, 0)));
}

#[test]
fn sequential_and_chain_composition_match() {
    let a = RasterFx::ChromaticSplit { offset: 1 };
    let b = RasterFx::Vignette { strength: 0.7 };
    let mut together = impulse();
    let mut sequential = together.clone();
    RasterFx::apply_chain(&mut together, &[a, b]);
    RasterFx::apply_chain(&mut sequential, &[a]);
    RasterFx::apply_chain(&mut sequential, &[b]);
    assert_eq!(together, sequential);
}

#[test]
fn chromatic_split_displaces_channels_and_clamps_edges() {
    let mut raster = impulse();
    RasterFx::apply_chain(&mut raster, &[RasterFx::ChromaticSplit { offset: 1 }]);
    assert_eq!(raster.get(5, 3), Some((240, 0, 0)));
    assert_eq!(raster.get(4, 3), Some((0, 120, 0)));
    assert_eq!(raster.get(3, 3), Some((0, 0, 60)));
    let mut flat = RgbRaster::new(1, 1);
    flat.set(0, 0, (12, 34, 56));
    RasterFx::apply_chain(&mut flat, &[RasterFx::ChromaticSplit { offset: i16::MIN }]);
    assert_eq!(flat.get(0, 0), Some((12, 34, 56)));
}

#[test]
fn normalized_vignette_works_across_dimensions() {
    for (w, h) in [(3, 3), (17, 9), (57, 29)] {
        let mut raster = RgbRaster::new(w, h);
        raster.clear((200, 200, 200));
        RasterFx::apply_chain(&mut raster, &[RasterFx::Vignette { strength: 1.0 }]);
        let center = raster.get(i32::from(w / 2), i32::from(h / 2)).unwrap();
        assert_eq!(center, (200, 200, 200));
        assert!(raster.get(0, 0).unwrap().0 < 120);
    }
}

#[test]
fn workspace_reuse_and_resize_are_deterministic() {
    let mut workspace = RasterFxWorkspace::default();
    let effects = [
        RasterFx::SineWarp {
            amplitude: 2.0,
            frequency: 0.4,
            phase: 0.9,
        },
        RasterFx::Glow {
            radius: 2,
            threshold: 90,
            strength: 0.8,
        },
    ];
    let mut a = impulse();
    let mut b = a.clone();
    workspace.apply(&mut a, &effects);
    workspace.apply(&mut RgbRaster::new(2, 3), &effects);
    workspace.apply(&mut b, &effects);
    assert_eq!(a, b);
}

#[test]
fn nonfinite_effect_parameters_are_identity() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut raster = impulse();
        let original = raster.clone();
        RasterFx::apply_chain(
            &mut raster,
            &[
                RasterFx::Glow {
                    radius: 1,
                    threshold: 0,
                    strength: bad,
                },
                RasterFx::SineWarp {
                    amplitude: bad,
                    frequency: 1.0,
                    phase: 1.0,
                },
                RasterFx::SineWarp {
                    amplitude: 1.0,
                    frequency: bad,
                    phase: 1.0,
                },
                RasterFx::SineWarp {
                    amplitude: 1.0,
                    frequency: 1.0,
                    phase: bad,
                },
                RasterFx::Vignette { strength: bad },
                RasterFx::Scanlines { strength: bad },
            ],
        );
        assert_eq!(raster, original);
    }
}

#[test]
fn hostile_finite_effects_and_empty_images_are_bounded() {
    let effects = [
        RasterFx::Glow {
            radius: u8::MAX,
            threshold: 0,
            strength: f32::MAX,
        },
        RasterFx::SineWarp {
            amplitude: f32::MAX,
            frequency: f32::MAX,
            phase: f32::MAX,
        },
        RasterFx::ChromaticSplit { offset: i16::MAX },
    ];
    for (w, h) in [(0, 0), (0, 3), (3, 0), (1, 1), (9, 7)] {
        let mut raster = RgbRaster::new(w, h);
        raster.clear((12, 40, 80));
        RasterFx::apply_chain(&mut raster, &effects);
        assert_eq!(raster.pixels().len(), usize::from(w) * usize::from(h));
    }
}

#[test]
fn feedback_replays_exact_irregular_sequence() {
    let mut a = FeedbackBuffer::new(9, 7, Duration::from_millis(300));
    let mut b = a.clone();
    for (i, ms) in [16, 33, 4, 0, 81, 120, 5].into_iter().enumerate() {
        let mut input = RgbRaster::new(9, 7);
        input.set(i as i32, 2, (0, 180, 255));
        a.update(Duration::from_millis(ms), &input);
        b.update(Duration::from_millis(ms), &input);
        assert_eq!(a, b);
    }
    assert_eq!(a.updates(), 6);
}

#[test]
fn feedback_half_life_and_black_decay_are_predictable() {
    let mut feedback = FeedbackBuffer::new(9, 7, Duration::from_secs(1));
    feedback.update(Duration::from_secs(1), &impulse());
    let black = RgbRaster::new(9, 7);
    feedback.update(Duration::from_secs(1), &black);
    assert_eq!(feedback.raster().get(4, 3), Some((120, 60, 30)));
    for _ in 0..12 {
        feedback.update(Duration::from_secs(1), &black);
    }
    assert!(feedback.raster().pixels().iter().all(|c| *c == (0, 0, 0)));
}

#[test]
fn feedback_zero_dt_is_identity_even_with_new_input_dimensions() {
    let mut feedback = FeedbackBuffer::new(9, 7, Duration::from_secs(1));
    feedback.update(Duration::from_secs(1), &impulse());
    let before = feedback.clone();
    feedback.update(Duration::ZERO, &RgbRaster::new(2, 2));
    assert_eq!(feedback, before);
}

#[test]
fn feedback_resize_preserves_history_when_clamped_dimensions_match() {
    // Exercise each axis independently: only 2,048 pixels per buffer, not 2048².
    for (width, height) in [(3000, 1), (1, 3000)] {
        let mut feedback = FeedbackBuffer::new(1, 1, Duration::from_secs(1));
        feedback.resize(width, height);
        let dimensions = (
            width.min(MAX_RASTER_DIMENSION),
            height.min(MAX_RASTER_DIMENSION),
        );
        let mut input = RgbRaster::new(width, height);
        input.set(0, 0, (240, 120, 60));
        feedback.update(Duration::from_millis(137), &input);
        input.clear((0, 0, 0));
        feedback.update(Duration::from_millis(71), &input);
        assert_eq!(feedback.updates(), 2);
        assert_ne!(feedback.raster().get(0, 0), Some((0, 0, 0)));
        let before = feedback.clone();

        for requested in [(width, height), (width, height), dimensions] {
            feedback.resize(requested.0, requested.1);
            assert_eq!(
                (feedback.raster().width(), feedback.raster().height()),
                dimensions
            );
            // Includes floating energy, output, half-life, and update counter.
            assert_eq!(feedback, before);
        }
    }
}

#[test]
fn feedback_reset_resize_and_extreme_durations_are_safe() {
    let mut feedback = FeedbackBuffer::new(9, 7, Duration::from_nanos(1));
    feedback.update(Duration::from_secs(1), &impulse());
    feedback.resize(9, 7);
    assert_eq!(feedback.updates(), 1);
    feedback.update(Duration::MAX, &RgbRaster::new(9, 7));
    assert!(feedback.raster().pixels().iter().all(|p| *p == (0, 0, 0)));
    feedback.update(Duration::from_secs(1), &impulse());
    feedback.resize(3, 2);
    assert_eq!(feedback.raster().pixels().len(), 6);
    assert_eq!(feedback.updates(), 0);
    feedback.reset();
    assert_eq!(feedback, FeedbackBuffer::new(3, 2, Duration::from_nanos(1)));
    let mut immediate = FeedbackBuffer::new(9, 7, Duration::ZERO);
    immediate.update(Duration::from_secs(1), &impulse());
    assert_eq!(immediate.raster(), &impulse());
    immediate.update(Duration::from_nanos(1), &RgbRaster::new(9, 7));
    assert!(immediate.raster().pixels().iter().all(|p| *p == (0, 0, 0)));
}

#[test]
fn fields_have_expected_spatial_geometry() {
    assert_eq!(radial_glow(0.0, 0.0, 1.0), 1.0);
    assert!(radial_glow(2.0, 0.0, 1.0) < radial_glow(1.0, 0.0, 1.0));
    assert_eq!(radial_glow(2.0, 0.0, 1.0), radial_glow(0.0, 2.0, 1.0));
    assert_eq!(ring(1.0, 0.0, 1.0, 0.1), 1.0);
    assert!(ring(0.0, 0.0, 1.0, 0.1) < 0.001);
    let sources = [(0.0, 0.0, 0.3), (2.0, 0.0, 0.3)];
    assert_eq!(metaballs(0.5, 0.0, &sources), metaballs(1.5, 0.0, &sources));
    assert!(metaballs(0.0, 0.0, &sources) > metaballs(1.0, 0.0, &sources));
    for i in 0..100 {
        let t = i as f32 * 0.13;
        let v = vortex(0.7, -0.9, t, 3.0);
        assert!((0.0..=1.0).contains(&v));
        assert_eq!(v, vortex(0.7, -0.9, t, 3.0));
    }
}

#[test]
fn fields_reject_nan_and_remain_finite_for_hostile_coordinates() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(radial_glow(bad, 0.0, 1.0), 0.0);
        assert_eq!(ring(0.0, 0.0, bad, 1.0), 0.0);
        assert_eq!(vortex(0.0, 0.0, bad, 3.0), 0.0);
        assert_eq!(metaballs(0.0, 0.0, &[(bad, 0.0, 1.0)]), 0.0);
        assert_eq!(palette(bad, (2, 3, 4), (255, 255, 255)), (2, 3, 4));
    }
    assert!(radial_glow(f32::MAX, f32::MIN, f32::MIN_POSITIVE).is_finite());
    assert!(vortex(f32::MAX, f32::MIN, f32::MAX, f32::MAX).is_finite());
    assert!(ring(f32::MAX, f32::MIN, f32::MAX, f32::MIN_POSITIVE).is_finite());
    assert_eq!(metaballs(0.0, 0.0, &[(0.0, 0.0, f32::MAX)]), 1_000_000.0);
    assert_eq!(palette(0.5, (0, 100, 200), (100, 200, 0)), (50, 150, 100));
    assert_eq!(palette(-100.0, (1, 2, 3), (4, 5, 6)), (1, 2, 3));
    assert_eq!(palette(100.0, (1, 2, 3), (4, 5, 6)), (4, 5, 6));
}

#[test]
fn zero_effects_are_identity_and_glow_radius_is_capped() {
    let original = impulse();
    let mut raster = original.clone();
    RasterFx::apply_chain(
        &mut raster,
        &[
            RasterFx::Glow {
                radius: 0,
                threshold: 0,
                strength: 1.0,
            },
            RasterFx::Glow {
                radius: 3,
                threshold: 0,
                strength: 0.0,
            },
            RasterFx::ChromaticSplit { offset: 0 },
            RasterFx::SineWarp {
                amplitude: 0.0,
                frequency: 1.0,
                phase: 1.0,
            },
            RasterFx::Vignette { strength: 0.0 },
            RasterFx::Scanlines { strength: 0.0 },
        ],
    );
    assert_eq!(raster, original);
    let mut capped = original.clone();
    let mut explicit = original;
    RasterFx::apply_chain(
        &mut capped,
        &[RasterFx::Glow {
            radius: 255,
            threshold: 0,
            strength: 1.0,
        }],
    );
    RasterFx::apply_chain(
        &mut explicit,
        &[RasterFx::Glow {
            radius: 3,
            threshold: 0,
            strength: 1.0,
        }],
    );
    assert_eq!(capped, explicit);
}

#[test]
fn feedback_saturates_without_hiding_excess_energy_and_resizes_on_input() {
    let mut feedback = FeedbackBuffer::new(9, 7, Duration::from_secs(1));
    for _ in 0..100 {
        feedback.update(Duration::from_millis(1), &impulse());
    }
    assert_eq!(feedback.raster().get(4, 3), Some((255, 255, 255)));
    feedback.update(Duration::from_secs(1), &RgbRaster::new(9, 7));
    assert_eq!(feedback.raster().get(4, 3), Some((128, 128, 128)));
    let mut input = RgbRaster::new(2, 1);
    input.set(1, 0, (40, 80, 120));
    feedback.update(Duration::from_secs(1), &input);
    assert_eq!(feedback.raster(), &input);
    assert_eq!(feedback.updates(), 1);
}
