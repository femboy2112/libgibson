//! Integration tests for the `cinematic_showcase` flagship's render core — the
//! demo-local art-direction layers that turn the edit clock into a 3-D flight:
//! the Surface→texture adapter ([`texture`]), the constellation geometry and
//! moving camera ([`stage`]), and the deep-space look ([`look`]). Each module
//! carries its own `#[cfg(test)]` suite (pure, bounded, deterministic — no TTY);
//! this file pulls them into the test crate the same way `tests/cinematic_showcase.rs`
//! pulls in the timeline law.

#[path = "../examples/cinematic_showcase/look.rs"]
mod look;
#[path = "../examples/cinematic_showcase/stage.rs"]
mod stage;
#[path = "../examples/cinematic_showcase/texture.rs"]
mod texture;

use gibson::raster::RgbRaster;

/// A cross-module smoke test: a real-shaped frame composes bounded and
/// deterministically through all three layers (backdrop → panels → bloom/grade).
#[test]
fn full_look_and_stage_pipeline_is_bounded_and_deterministic() {
    let (pw, ph) = (160u16, 96u16);
    let texes: Vec<RgbRaster> = (0..stage::N_PLANES)
        .map(|i| {
            let mut r = RgbRaster::new(24, 18);
            r.clear((30 + 20 * i as u8, 40, 90));
            r
        })
        .collect();
    let refls: Vec<RgbRaster> = texes.iter().map(texture::reflection_of).collect();

    let compose = || {
        let bg = look::starfield(pw, ph, 18.0, 0x1234);
        let mut frame = stage::render_frame(18.0, pw, ph, bg, &texes, &refls);
        look::bloom(&mut frame, 168, 9, 1.8);
        look::grade(&mut frame, 0.34);
        frame
    };
    let a = compose();
    let b = compose();
    assert_eq!((a.width(), a.height()), (pw, ph));
    assert_eq!(a.pixels(), b.pixels(), "the whole frame path must be pure");
}

/// The interactive congruence law: a camera settled on a panel's dwell pose projects
/// that panel to *exactly* the baked `hold_rect`. This is what lets the live UI blit
/// 1:1 over its own texture when `interactive_showcase` zooms in — the same congruence
/// the film relies on, now reached by a hand-driven camera rather than the edit clock.
#[test]
fn interactive_dwell_pose_projects_to_the_hold_rect() {
    for &(w, h) in &[(120u16, 36u16), (180, 48), (100, 30)] {
        for i in 0..stage::N_PLANES {
            let settled = stage::panel_cell_bounds_cam(i, &stage::dwell_camera(i), w, h);
            let hold = stage::hold_rect(i, w, h);
            assert_eq!(
                settled, hold,
                "panel {i} at {w}x{h}: settled dwell pose must equal the baked hold rect"
            );
        }
    }
}
